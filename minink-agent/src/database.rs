use anyhow::Result;

use chrono::NaiveDateTime;

use futures::TryStreamExt;

use minink_common::{Filter, LogEntry};

use sqlx::{Connection, QueryBuilder, Sqlite, SqliteConnection};

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

    pub async fn insert_log(&mut self, entry: &LogEntry) -> Result<()> {
        let mut tx = self.conn.begin().await?;
        dbg!(&entry);
        sqlx::query!(
            r#"insert into logs(message, hostname, service, timestamp)
            values($1, $2, $3, $4);"#,
            entry.message,
            entry.hostname,
            entry.service,
            entry.timestamp,
        )
        .execute(&mut tx)
        .await?;
        Ok(tx.commit().await?)
    }

    pub async fn insert_logs(&mut self, entries: &[LogEntry]) -> Result<()> {
        assert!(entries.len() < 65535 / 4);
        dbg!(&entries);

        let mut tx = self.conn.begin().await?;
        QueryBuilder::new("insert into logs(message, hostname, service, timestamp) ")
            .push_values(entries, |mut b, entry| {
                b.push_bind(&entry.message)
                    .push_bind(&entry.hostname)
                    .push_bind(&entry.service)
                    .push_bind(entry.timestamp);
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
            select *
            from logs
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

async fn connect(url: &str) -> Result<SqliteConnection> {
    Ok(SqliteConnection::connect(url).await?)
}

pub async fn get_database(url: &str) -> Result<LogDatabase> {
    let mut conn = connect(url).await?;
    sqlx::migrate!().run(&mut conn).await?;
    Ok(LogDatabase { conn })
}
