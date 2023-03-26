use anyhow::Result;

use journald::JournaldLogSource;

use logstream::LogStream;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod database;
mod journald;
mod logstream;
mod websocket;

use database::LogDatabase;

async fn ingest_logs_job(db: &mut LogDatabase, mut logstream: LogStream) -> Result<()> {
    loop {
        let entry = logstream.pull_one().await?;
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

    let mut database = database::get_database("sqlite://logs.db").await?;
    let last_timestamp = database.last_timestamp().await?;

    let logsource = JournaldLogSource::new();

    // subscribe before starting the `follow` task
    let logstream = logsource.subscribe();
    let j1 = tokio::spawn(async move { ingest_logs_job(&mut database, logstream).await });

    let j2 = {
        let logsource = logsource.clone();
        tokio::spawn(async move { logsource.follow(last_timestamp).await })
    };

    let database = database::get_database("sqlite://logs.db").await?;
    let j3 = tokio::spawn(websocket::main(logsource, database));

    tokio::try_join!(j1, j2, j3)?;

    Ok(())
}
