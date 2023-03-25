use anyhow::Result;

use tokio::sync::mpsc::{self, UnboundedReceiver};

use minink_common::LogEntry;

mod database;
mod journald;

use database::LogDatabase;

async fn ingest_logs_job(
    db: &mut LogDatabase,
    mut receiver: UnboundedReceiver<LogEntry>,
) -> Result<()> {
    while let Some(entry) = receiver.recv().await {
        db.insert_log(&entry).await?;
    }
    Ok(())
}

//// NOTE XXX https://github.com/tokio-rs/axum/blob/main/examples/chat/src/main.rs

#[tokio::main]
async fn main() -> Result<()> {
    let mut db = database::get_database("sqlite://logs.db").await?;

    let last_timestamp = db.last_timestamp().await?;

    let (tx, rx) = mpsc::unbounded_channel();
    let j1 = tokio::spawn(async move { journald::follow_logs(tx, last_timestamp).await });
    let j2 = tokio::spawn(async move { ingest_logs_job(&mut db, rx).await });

    tokio::try_join!(j1, j2)?;

    Ok(())
}
