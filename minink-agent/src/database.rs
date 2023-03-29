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
        let to_sqlite_phrase = |s: &Vec<String>| {
            "(".to_owned()
                + &s.iter()
                    .map(|s| s.to_owned() + "*")
                    .collect::<Vec<_>>()
                    .join(" OR ")
                + ")"
        };
        let message = filter.message_keywords.as_ref().map(to_sqlite_phrase);
        let service = filter.services.as_ref().map(to_sqlite_phrase);

        let mut query = QueryBuilder::new(
            r#"
            select message as "message!: String", hostname as 'hostname!', service as 'service!: String', timestamp as 'timestamp!'
            from logs
            join logsfts fts on fts.rowid == logs.logsfts_id
            where 1 "#,
        );
        if let Some(message) = &message {
            query
                .push("and logsfts match (('message: ' || ")
                .push_bind(message)
                .push(")");
        }
        if let Some(service) = &service {
            if message.is_some() {
                query.push(" AND ")
            } else {
                query.push("and logsfts match (")
            }
            .push("('service: ' ||")
            .push_bind(service)
            .push(")");
        }
        if service.is_some() || message.is_some() {
            query.push(")");
        }
        query.push(
            r#"order by timestamp desc
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
