use std::{ops::Bound, str::FromStr, sync::Arc};

use anyhow::Result;

use chrono::NaiveDateTime;

use minink_common::{Filter, LogEntry};

use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteRow},
    ConnectOptions, QueryBuilder, Row, Sqlite, SqlitePool,
};

use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct LogDatabase {
    pool: SqlitePool,
    entries: Arc<Mutex<Vec<LogEntry>>>,
}

fn convert_to_fts_match<S: AsRef<str>>(filter: &[S]) -> String {
    let fts_escape = |s: String| {
        let s = s.replace(|c: char| !c.is_alphanumeric(), " ");
        let s = s.trim();
        if !s.is_empty() {
            Some(format!("\"{s}\"*"))
        } else {
            None
        }
    };
    filter
        .iter()
        .map(|s| str::to_lowercase(s.as_ref()))
        .filter_map(fts_escape)
        .collect::<Vec<_>>()
        .join(" OR ")
}

fn to_sqlite_phrase(m: &[String]) -> Option<String> {
    let s = convert_to_fts_match(m);
    if !s.is_empty() {
        Some(format!("({s})"))
    } else {
        None
    }
}

trait PushToQuery {
    fn push_to_query(&self, column: &str, query: &mut QueryBuilder<Sqlite>);
}

impl PushToQuery for (Bound<NaiveDateTime>, Bound<NaiveDateTime>) {
    fn push_to_query(&self, column: &str, query: &mut QueryBuilder<Sqlite>) {
        match self.0 {
            std::ops::Bound::Included(t) => {
                query
                    .push(format!(" and unixepoch({column}) >= "))
                    .push_bind(t.timestamp());
            }
            std::ops::Bound::Excluded(t) => {
                query
                    .push(format!(" and unixepoch({column}) > "))
                    .push_bind(t.timestamp());
            }
            std::ops::Bound::Unbounded => (),
        }
        match self.1 {
            std::ops::Bound::Included(t) => {
                query
                    .push(format!(" and unixepoch({column}) <= "))
                    .push_bind(t.timestamp());
            }
            std::ops::Bound::Excluded(t) => {
                query
                    .push(format!(" and unixepoch({column}) < "))
                    .push_bind(t.timestamp());
            }
            std::ops::Bound::Unbounded => (),
        }
    }
}

impl LogDatabase {
    pub async fn new(url: &str) -> Result<Self> {
        let mut options = SqliteConnectOptions::from_str(url)?.journal_mode(SqliteJournalMode::Wal);
        options.log_statements(tracing::log::LevelFilter::Info);
        let pool = SqlitePool::connect_with(options).await?;
        sqlx::migrate!().run(&pool).await?;
        Ok(Self {
            pool,
            entries: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub async fn last_timestamp(&self) -> Result<Option<NaiveDateTime>> {
        // for some reasons the type cannot be inferred correctly on 'timestamp'
        let record =
            sqlx::query!(r#"select max(timestamp) as 'timestamp: NaiveDateTime' from logs"#)
                .fetch_one(&self.pool)
                .await?;

        Ok(record.timestamp)
    }

    async fn insert_logs(&self, entries: &[LogEntry]) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }

        assert!(entries.len() < 65535 / 4);

        let mut tx = self.pool.begin().await?;
        let r = QueryBuilder::new("insert into logsfts(service, message) ")
            .push_values(entries, |mut b, entry| {
                b.push_bind(&entry.service).push_bind(&entry.message);
            })
            .build()
            .execute(&mut tx)
            .await?;

        // I can't find a way to fetch the rowid for each inserts, so instead
        // I'm assuming that the rowid is only increasing on a bulk insert
        let lastid = r.last_insert_rowid();
        let numinserts = r.rows_affected();
        assert!(numinserts == entries.len() as u64);
        let mut firstid = (lastid + 1).wrapping_sub(numinserts.try_into().unwrap());

        QueryBuilder::new("insert into logs(hostname, timestamp, logsfts_id) ")
            .push_values(entries, |mut b, entry| {
                b.push_bind(&entry.hostname)
                    .push_bind(entry.timestamp)
                    .push_bind(firstid);
                firstid += 1;
            })
            .build()
            .execute(&mut tx)
            .await?;
        Ok(tx.commit().await?)
    }

    pub async fn add_log(&self, entry: LogEntry) -> Result<()> {
        let sync = {
            let mut entries = self.entries.lock().await;
            entries.push(entry);
            entries.len() > 1024
        };
        if sync {
            self.sync_logs().await?;
        }
        Ok(())
    }

    async fn sync_logs(&self) -> Result<()> {
        let mut entries = self.entries.lock().await;
        self.insert_logs(&entries).await?;
        entries.clear();
        Ok(())
    }

    pub async fn extract(&self, filter: &Filter) -> Result<Vec<LogEntry>> {
        self.sync_logs().await?;

        let message = filter
            .message_keywords
            .as_ref()
            .and_then(|a| to_sqlite_phrase(a))
            .map(|p| format!("(message: {p})"));
        let service = filter
            .services
            .as_ref()
            .and_then(|a| to_sqlite_phrase(a))
            .map(|p| format!("(service: {p})"));

        let mut matches = vec![];
        matches.extend(message);
        matches.extend(service);
        let matches = matches.join(" AND ");

        let mut query = QueryBuilder::new(
            r#"
            select message as "message!: String", hostname as 'hostname!', service as 'service!: String', timestamp as 'timestamp!'
            from logs
            join logsfts fts on fts.rowid == logs.logsfts_id
            where 1"#,
        );
        if !matches.is_empty() {
            query.push(" and logsfts = ").push_bind(matches);
        }
        filter.timerange.push_to_query("timestamp", &mut query);
        query.push(
            r#" order by timestamp desc
            limit 100;"#,
        );

        let mut entries: Vec<LogEntry> = query
            .build()
            .map(|a: SqliteRow| LogEntry {
                message: a.get(0),
                hostname: a.get(1),
                service: a.get(2),
                timestamp: a.get(3),
            })
            .fetch_all(&self.pool)
            .await?;

        entries.reverse();
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use chrono::NaiveDateTime;
    use minink_common::{Filter, LogEntry};

    use crate::database::convert_to_fts_match;

    use super::LogDatabase;

    async fn prep_db(entries: &[LogEntry]) -> Result<LogDatabase> {
        let db = LogDatabase::new(":memory:").await?;
        db.insert_logs(entries).await?;
        Ok(db)
    }

    fn default_entries() -> Vec<LogEntry> {
        vec![
            LogEntry {
                message: "toto".to_string(),
                hostname: "localhost".to_string(),
                service: "nginx".to_string(),
                timestamp: NaiveDateTime::from_timestamp_micros(0).unwrap(),
            },
            LogEntry {
                message: "TOTO-200".to_string(),
                hostname: "localhost".to_string(),
                service: "NGINX".to_string(),
                timestamp: NaiveDateTime::from_timestamp_micros(1).unwrap(),
            },
            LogEntry {
                message: "titi 20020".to_string(),
                hostname: "localhost".to_string(),
                service: "kernel".to_string(),
                timestamp: NaiveDateTime::from_timestamp_micros(2).unwrap(),
            },
        ]
    }

    #[tokio::test]
    async fn test_empty_insert() -> Result<()> {
        prep_db(&[]).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_extract_all() -> Result<()> {
        let db = prep_db(&default_entries()).await?;
        let filter = Filter::default();
        let found = db.extract(&filter).await?;
        assert_eq!(found.len(), 3);
        let found2 = default_entries()
            .into_iter()
            .filter(|e| filter.accept(e))
            .collect::<Vec<_>>();
        assert_eq!(found, found2);
        Ok(())
    }

    #[tokio::test]
    async fn test_extract_filter_by_message() -> Result<()> {
        let db = prep_db(&default_entries()).await?;
        let filter = Filter {
            message_keywords: Some(vec!["200".to_string()]),
            ..Filter::default()
        };
        let found = db.extract(&filter).await?;
        assert_eq!(found.len(), 2);
        let found2 = default_entries()
            .into_iter()
            .filter(|e| filter.accept(e))
            .collect::<Vec<_>>();
        assert_eq!(found, found2);
        Ok(())
    }

    #[tokio::test]
    async fn test_extract_filter_by_service() -> Result<()> {
        let db = prep_db(&default_entries()).await?;
        let filter = Filter {
            services: Some(vec!["n".to_string()]),
            ..Filter::default()
        };
        let found = db.extract(&filter).await?;
        assert_eq!(found.len(), 2);
        let found2 = default_entries()
            .into_iter()
            .filter(|e| filter.accept(e))
            .collect::<Vec<_>>();
        assert_eq!(found, found2);
        Ok(())
    }

    #[tokio::test]
    async fn test_extract_filter_by_service_and_message() -> Result<()> {
        let db = prep_db(&default_entries()).await?;
        let filter = Filter {
            services: Some(vec!["n".to_string()]),
            message_keywords: Some(vec!["200".to_string()]),
            ..Default::default()
        };
        let found = db.extract(&filter).await?;
        assert_eq!(found.len(), 1);
        let found2 = default_entries()
            .into_iter()
            .filter(|e| filter.accept(e))
            .collect::<Vec<_>>();
        assert_eq!(found, found2);
        Ok(())
    }

    #[test]
    fn test_convert_to_fts_match() {
        assert_eq!(convert_to_fts_match::<&str>(&[]), "");
        assert_eq!(convert_to_fts_match(&["bla"]), "\"bla\"*");
        assert_eq!(convert_to_fts_match(&["bla bla"]), "\"bla bla\"*");
        assert_eq!(
            convert_to_fts_match(&["bla ", " blo"]),
            "\"bla\"* OR \"blo\"*"
        );
        assert_eq!(convert_to_fts_match(&["AB©d"]), "\"ab d\"*");
        assert_eq!(
            convert_to_fts_match(&["A OR B", "AND C"]),
            "\"a or b\"* OR \"and c\"*"
        );
        assert_eq!(convert_to_fts_match(&["\""]), "");
    }
}
