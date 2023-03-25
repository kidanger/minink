use async_process::{Command, Stdio};
use futures_lite::io::BufReader;
use futures_lite::{AsyncBufReadExt, StreamExt};

use anyhow::Result;

use chrono::NaiveDateTime;

use serde::Deserialize;

use sqlx::{Connection, SqliteConnection};

use minink_agent::LogEntry;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

/// see journalctl(1) json format
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum JournaldMessage {
    String(String),
    Bytes(Vec<u8>),
    Multiple(Vec<JournaldMessage>),
}

impl std::fmt::Display for JournaldMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JournaldMessage::String(s) => s.fmt(f),
            JournaldMessage::Bytes(b) => String::from_utf8_lossy(b).fmt(f),
            JournaldMessage::Multiple(messages) => {
                for message in messages {
                    message.fmt(f)?;
                    ";".fmt(f)?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct JournaldRawLogEntry {
    #[serde(rename = "MESSAGE")]
    message: JournaldMessage,
    #[serde(rename = "_HOSTNAME")]
    hostname: String,
    #[serde(rename = "_SYSTEMD_UNIT")]
    systemd_unit: Option<String>,
    #[serde(rename = "__REALTIME_TIMESTAMP")]
    timestamp: String,
}

async fn get_db() -> Result<SqliteConnection> {
    Ok(SqliteConnection::connect("sqlite://logs.db").await?)
}

async fn follow_logs(
    sender: UnboundedSender<LogEntry>,
    since_timestamp: Option<NaiveDateTime>,
) -> Result<()> {
    let since_format = if let Some(since) = since_timestamp {
        let now = chrono::Utc::now().naive_utc();
        dbg!(now);
        dbg!(since);
        let duration = now - since;
        let ms = duration.num_milliseconds();
        format!("-{}s{}ms", ms / 1000, ms % 1000)
    } else {
        "1 day ago".to_string()
    };
    dbg!(&since_format);
    let mut child = Command::new("journalctl")
        .arg("--follow")
        .arg("--output=json")
        .arg("--output-fields=MESSAGE,_HOSTNAME,_SYSTEMD_UNIT,__REALTIME_TIMESTAMP")
        .arg("--all")
        .arg(&format!("--since={}", since_format))
        .stdout(Stdio::piped())
        .spawn()?;

    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();

    while let Some(line) = lines.next().await {
        let entry = parse_log_entry(&line?)?;
        sender.send(entry)?;
    }

    Ok(())
}

fn parse_log_entry(line: &str) -> Result<LogEntry> {
    //dbg!(&line);
    let raw: JournaldRawLogEntry = serde_json::from_str(line)?;
    let timestamp: i64 = raw.timestamp.parse()?;
    let timestamp = NaiveDateTime::from_timestamp_micros(timestamp).unwrap();
    let message = raw.message.to_string();
    Ok(LogEntry {
        message,
        hostname: raw.hostname,
        systemd_unit: raw.systemd_unit.unwrap_or_default(),
        timestamp,
    })
}

async fn get_last_timestamp() -> Result<Option<NaiveDateTime>> {
    let mut conn = get_db().await?;

    // for some reasons the type cannot be inferred correctly on 'timestamp'
    let record = sqlx::query!(r#"select max(timestamp) as 'timestamp: NaiveDateTime' from logs"#)
        .fetch_one(&mut conn)
        .await?;

    dbg!(&record);
    Ok(record.timestamp)
}

async fn follow_logs_job(sender: UnboundedSender<LogEntry>) -> Result<()> {
    let last_timestamp = get_last_timestamp().await?;
    follow_logs(sender, last_timestamp).await
}

async fn process_log(conn: &mut SqliteConnection, entry: LogEntry) -> Result<()> {
    let mut tx = conn.begin().await?;
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
    tx.commit().await?;
    Ok(())
}

async fn ingest_logs_job(mut receiver: UnboundedReceiver<LogEntry>) -> Result<()> {
    let mut conn = get_db().await?;
    while let Some(entry) = receiver.recv().await {
        process_log(&mut conn, entry).await?;
    }
    Ok(())
}

//// NOTE XXX https://github.com/tokio-rs/axum/blob/main/examples/chat/src/main.rs

#[tokio::main]
async fn main() -> Result<()> {
    let mut conn = get_db().await?;
    sqlx::migrate!().run(&mut conn).await?;

    let (tx, rx) = mpsc::unbounded_channel();

    let j1 = tokio::spawn(async { follow_logs_job(tx).await });
    let j2 = tokio::spawn(async { ingest_logs_job(rx).await });
    tokio::try_join!(j1, j2)?;

    Ok(())
}
