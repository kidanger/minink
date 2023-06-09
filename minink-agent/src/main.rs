use std::path::PathBuf;

use anyhow::Result;

use clap::Parser;

use journald::JournaldLogSource;

use logstream::LogStream;

use server::ServerArgs;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod database;
mod journald;
mod logdispatcher;
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

async fn ingest_logs_job(db: LogDatabase, mut logstream: LogStream) -> Result<()> {
    loop {
        let entry = logstream.pull_one().await?;
        db.add_log(entry).await?;
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

    let database = LogDatabase::new(&args.database_path).await?;
    let last_timestamp = database.last_timestamp().await?;

    let (logsource, dispatcher) = JournaldLogSource::new();

    let j1 = tokio::spawn(ingest_logs_job(database.clone(), dispatcher.stream()));
    let j2 = tokio::spawn(logsource.follow(last_timestamp));

    let server_args = ServerArgs {
        port: args.port,
        assets_dir: args.assets_dir,
    };
    let j3 = tokio::spawn(server::main(dispatcher, database, server_args));

    tokio::try_join!(flatten(j1), flatten(j2), flatten(j3))?;

    Ok(())
}
