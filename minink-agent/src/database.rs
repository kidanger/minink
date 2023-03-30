use std::str::FromStr;

use anyhow::Result;

use chrono::NaiveDateTime;

use minink_common::{Filter, LogEntry};

use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteRow},
    ConnectOptions, Connection, QueryBuilder, Row, SqliteConnection,
};

#[derive(Debug)]
pub struct LogDatabase {
    conn: SqliteConnection,
}

impl LogDatabase {
    pub async fn last_timestamp(&mut self) -> Result<Option<NaiveDateTime>> {
        // for some reasons the type cannot be inferred correctly on 'timestamp'
        let record =
            sqlx::query!(r#"select max(timestamp) as 'timestamp: NaiveDateTime' from logs"#)
                .fetch_one(&mut self.conn)
                .await?;

        Ok(record.timestamp)
    }

    pub async fn insert_logs(&mut self, entries: &[LogEntry]) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }

        assert!(entries.len() < 65535 / 4);

        let mut tx = self.conn.begin().await?;
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

    pub async fn extract(&mut self, filter: &Filter) -> Result<Vec<LogEntry>> {
        dbg!(&filter);
        let fts_escape = |s: String| {
            let s = s.replace(|c: char| !c.is_alphanumeric(), " ");
            let s = s.trim();
            if !s.is_empty() {
                Some(format!("\"{s}\"*"))
            } else {
                None
            }
        };
        // TODO: test this extensively
        let to_sqlite_phrase = |s: &Vec<String>| {
            let s = &s
                .iter()
                .map(|s| s.to_lowercase())
                .filter_map(fts_escape)
                .collect::<Vec<_>>()
                .join(" OR ");
            if !s.is_empty() {
                Some(format!("({s})"))
            } else {
                None
            }
        };
        let message = filter
            .message_keywords
            .as_ref()
            .and_then(to_sqlite_phrase)
            .map(|p| format!("(message: {p})"));
        let service = filter
            .services
            .as_ref()
            .and_then(to_sqlite_phrase)
            .map(|p| format!("(service: {p})"));

        let mut query = QueryBuilder::new(
            r#"
            select message as "message!: String", hostname as 'hostname!', service as 'service!: String', timestamp as 'timestamp!'
            from logs
            join logsfts fts on fts.rowid == logs.logsfts_id
            where 1 "#,
        );
        let mut matches = vec![];
        matches.extend(message);
        matches.extend(service);
        let matches = matches.join(" AND ");
        dbg!(&matches);
        if !matches.is_empty() {
            query.push("and logsfts = ").push_bind(matches);
        }
        query.push(
            r#"order by timestamp desc
            limit 100;"#,
        );
        dbg!(query.sql());

        let mut entries: Vec<LogEntry> = query
            .build()
            .map(|a: SqliteRow| LogEntry {
                message: a.get(0),
                hostname: a.get(1),
                service: a.get(2),
                timestamp: a.get(3),
            })
            .fetch_all(&mut self.conn)
            .await?;

        entries.reverse();
        Ok(entries)
    }
}

pub async fn get_database(url: &str, read_only: bool) -> Result<LogDatabase> {
    let mut conn = SqliteConnectOptions::from_str(url)?
        .journal_mode(SqliteJournalMode::Wal)
        .read_only(read_only)
        .log_statements(tracing::log::LevelFilter::Info)
        .connect()
        .await?;
    sqlx::migrate!().run(&mut conn).await?;
    Ok(LogDatabase { conn })
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use chrono::NaiveDateTime;
    use minink_common::{Filter, LogEntry};

    use super::{get_database, LogDatabase};

    async fn prep_db(entries: &[LogEntry]) -> Result<LogDatabase> {
        let mut db = get_database(":memory:", false).await?;
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
        let mut db = prep_db(&default_entries()).await?;
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
        let mut db = prep_db(&default_entries()).await?;
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
        let mut db = prep_db(&default_entries()).await?;
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
        //assert_eq!(found, found2);
        Ok(())
    }

    #[tokio::test]
    async fn test_extract_filter_by_service_and_message() -> Result<()> {
        let mut db = prep_db(&default_entries()).await?;
        let filter = Filter {
            services: Some(vec!["n".to_string()]),
            message_keywords: Some(vec!["200".to_string()]),
        };
        let found = db.extract(&filter).await?;
        assert_eq!(found.len(), 1);
        let found2 = default_entries()
            .into_iter()
            .filter(|e| filter.accept(e))
            .collect::<Vec<_>>();
        //assert_eq!(found, found2);
        Ok(())
    }
}
