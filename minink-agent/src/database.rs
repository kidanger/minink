use std::str::FromStr;

use anyhow::Result;

use chrono::NaiveDateTime;

use futures::TryStreamExt;

use minink_common::{Filter, LogEntry};

use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode},
    ConnectOptions, Connection, QueryBuilder, SqliteConnection,
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
        let len = 100;
        let mut iter = sqlx::query_as!(
            LogEntry,
            r#"
            select message as "message!: String", hostname, service as 'service!: String', timestamp
            from logs
            join logsfts fts on fts.rowid == logsfts_id
            order by timestamp desc;
            "#
        )
        .fetch(&mut self.conn);

        let mut entries = Vec::with_capacity(len);
        while let Some(entry) = iter.try_next().await? {
            if filter.accept(&entry) {
                entries.push(entry)
            }
            if entries.len() >= len {
                break;
            }
        }
        entries.reverse();
        Ok(entries)
    }
}

pub async fn get_database(url: &str, read_only: bool) -> Result<LogDatabase> {
    let mut conn = SqliteConnectOptions::from_str(url)?
        .journal_mode(SqliteJournalMode::Wal)
        .read_only(read_only)
        .connect()
        .await?;
    sqlx::migrate!().run(&mut conn).await?;
    Ok(LogDatabase { conn })
}
