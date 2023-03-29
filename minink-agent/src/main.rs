use std::path::PathBuf;

use anyhow::Result;

use clap::Parser;

use journald::JournaldLogSource;

use logstream::LogStream;

use server::ServerArgs;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod database;
mod journald;
mod logstream;
mod server;

use database::LogDatabase;

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long, default_value = "sqlite://logs.db")]
    database_path: String,
    #[arg(short, long, default_value = "3000")]
    port: u16,
    #[arg(short, long)]
    assets_dir: Option<PathBuf>,
}

async fn ingest_logs_job(mut db: LogDatabase, mut logstream: LogStream) -> Result<()> {
    let mut batch = Vec::with_capacity(1024);
    loop {
        while batch.len() < batch.capacity() {
            let entry = logstream.pull_one().await?;
            batch.push(entry);
        }
        db.insert_logs(&batch).await?;
        batch.clear();
    }
}

async fn flatten<T>(handle: tokio::task::JoinHandle<Result<T>>) -> Result<T> {
    match handle.await {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(err)) => Err(err),
        Err(err) => Err(err.into()),
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

    let args = Args::parse();

    let mut database = database::get_database(&args.database_path, false).await?;
    let last_timestamp = database.last_timestamp().await?;

    let (logsource, logstream) = JournaldLogSource::new();

    let j1 = tokio::spawn(ingest_logs_job(database, logstream.clone()));
    let j2 = tokio::spawn(logsource.follow(last_timestamp));

    let server_args = ServerArgs {
        port: args.port,
        assets_dir: args.assets_dir,
    };
    let database = database::get_database(&args.database_path, true).await?;
    let j3 = tokio::spawn(server::main(logstream, database, server_args));

    tokio::try_join!(flatten(j1), flatten(j2), flatten(j3))?;

    Ok(())
}
