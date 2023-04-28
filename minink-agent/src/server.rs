use anyhow::Result;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use chrono::NaiveDateTime;
use minink_common::{Filter, LogEntry, ServiceName};
use serde::Deserialize;

use std::{
    net::SocketAddr,
    ops::Bound,
    path::PathBuf,
    sync::{Arc},
};

use tower_http::{
    cors::CorsLayer,
    services::ServeDir,
    trace::{DefaultMakeSpan, TraceLayer},
};

use crate::{database::LogDatabase, logdispatcher::LogDispatcher, logstream::LogStream};

pub struct ServerArgs {
    pub port: u16,
    pub assets_dir: Option<PathBuf>,
}

#[derive(Clone)]
struct AppState {
    dispatcher: Arc<LogDispatcher>,
    database: LogDatabase,
}

pub async fn main(
    logdispatcher: Arc<LogDispatcher>,
    database: LogDatabase,
    args: ServerArgs,
) -> Result<()> {
    let appstate = AppState {
        dispatcher: logdispatcher,
        database,
    };

    let assets_dir = args
        .assets_dir
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"));

    let cors = CorsLayer::new()
        // allow `GET` and `POST` when accessing the resource
        .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
        // allow requests from any origin
        .allow_origin(tower_http::cors::Any);

    let app = Router::new()
        .fallback_service(ServeDir::new(assets_dir).append_index_html_on_directories(true))
        .route("/ws/live", get(ws_handler))
        .route("/api/extract", get(extract))
        .with_state(appstate)
        .layer(cors)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        );

    let addr = SocketAddr::from(([127, 0, 0, 1], args.port));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await?;
    Ok(())
}

#[derive(Debug, Deserialize)]
struct WSParams {
    #[serde(default)]
    services: Option<String>,
    #[serde(default)]
    message_keywords: Option<String>,
}

fn parse_query_list(services: Option<String>) -> Option<Vec<String>> {
    services.as_ref().map(|services| {
        services
            .split(',')
            .map(str::to_string)
            .collect::<Vec<ServiceName>>()
    })
}

#[axum_macros::debug_handler]
async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<WSParams>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let filter = Filter {
        services: parse_query_list(params.services),
        message_keywords: parse_query_list(params.message_keywords),
        ..Default::default()
    };
    let logstream = state.dispatcher.stream();
    let logstream = logstream.with_filter(filter);
    ws.on_upgrade(move |socket| handle_socket(socket, logstream))
}

async fn handle_socket(socket: WebSocket, logstream: LogStream) {
    async fn work(mut socket: WebSocket, mut logstream: LogStream) -> Result<()> {
        loop {
            let entry = logstream.pull_one().await?;
            let payload = serde_json::to_string(&entry)?;
            socket.send(Message::Text(payload)).await?;
        }
    }

    if let Err(err) = work(socket, logstream).await {
        tracing::info!("{}", err);
    }
}

#[derive(Debug, Deserialize)]
struct ExtractParams {
    #[serde(default)]
    services: Option<String>,
    #[serde(default)]
    message_keywords: Option<String>,
    #[serde(default)]
    start: Option<i64>,
    #[serde(default)]
    end: Option<i64>,
}

impl ExtractParams {
    fn timerange(&self) -> (Bound<NaiveDateTime>, Bound<NaiveDateTime>) {
        let timefrom = if let Some(start) = self.start {
            NaiveDateTime::from_timestamp_micros(start)
                .map(Bound::Excluded)
                .unwrap_or(Bound::Unbounded)
        } else {
            Bound::Unbounded
        };

        let timeto = if let Some(end) = self.end {
            NaiveDateTime::from_timestamp_micros(end)
                .map(Bound::Excluded)
                .unwrap_or(Bound::Unbounded)
        } else {
            Bound::Unbounded
        };

        (timefrom, timeto)
    }
}

#[axum_macros::debug_handler]
async fn extract(
    Query(params): Query<ExtractParams>,
    State(state): State<AppState>,
) -> Json<Vec<LogEntry>> {
    let timerange = params.timerange();
    let filter = Filter {
        services: parse_query_list(params.services),
        message_keywords: parse_query_list(params.message_keywords),
        timerange,
    };

    let db = state.database;
    let entries = { db.extract(&filter).await.unwrap() };

    Json(entries)
}
