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

    let args = Args::parse();

    let mut database = database::get_database(&args.database_path).await?;
    let last_timestamp = database.last_timestamp().await?;

    let logsource = JournaldLogSource::new();

    // subscribe before starting the `follow` task
    let logstream = logsource.subscribe();
    let j1 = tokio::spawn(async move { ingest_logs_job(&mut database, logstream).await });

    let j2 = {
        let logsource = logsource.clone();
        tokio::spawn(async move { logsource.follow(last_timestamp).await })
    };

    let database = database::get_database(&args.database_path).await?;

    let server_args = ServerArgs {
        port: args.port,
        assets_dir: args.assets_dir,
    };
    let j3 = tokio::spawn(server::main(logsource, database, server_args));

    tokio::try_join!(j1, j2, j3)?;

    Ok(())
}
