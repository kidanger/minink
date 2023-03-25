use anyhow::Result;

use chrono::NaiveDateTime;

use minink_common::LogEntry;

use sqlx::{Connection, SqliteConnection};

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
            "insert into logs(message, hostname, systemd_unit, timestamp) values($1, $2, $3, $4);",
            entry.message,
            entry.hostname,
            entry.systemd_unit,
            entry.timestamp,
        )
        .execute(&mut tx)
        .await?;
        Ok(tx.commit().await?)
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
