use anyhow::Result;

use tokio::sync::broadcast;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use minink_common::LogEntry;

mod database;
mod journald;
mod websocket;

use database::LogDatabase;

async fn ingest_logs_job(
    db: &mut LogDatabase,
    mut receiver: broadcast::Receiver<LogEntry>,
) -> Result<()> {
    loop {
        let entry = receiver.recv().await?;
        db.insert_log(&entry).await?;
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "minink_agent=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let mut db = database::get_database("sqlite://logs.db").await?;

    let last_timestamp = db.last_timestamp().await?;

    let (tx, rx) = broadcast::channel(100000);
    let j1 = {
        let tx = tx.clone();
        tokio::spawn(async move { journald::follow_logs(tx, last_timestamp).await })
    };
    let j2 = tokio::spawn(async move { ingest_logs_job(&mut db, rx).await });

    websocket::main(tx).await?;

    tokio::try_join!(j1, j2)?;

    Ok(())
}
