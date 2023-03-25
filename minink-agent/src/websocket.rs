use anyhow::Result;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};

use std::{net::SocketAddr, path::PathBuf};

use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, TraceLayer},
};

use crate::{journald::JournaldLogSource, logstream::LogStream};

#[derive(Debug, Clone)]
struct AppState {
    logsource: JournaldLogSource,
}

pub async fn main(logsource: JournaldLogSource) -> Result<()> {
    let appstate = AppState { logsource };

    let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");

    let app = Router::new()
        .fallback_service(ServeDir::new(assets_dir).append_index_html_on_directories(true))
        .route("/ws/live", get(ws_handler))
        .with_state(appstate)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        );

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await?;
    Ok(())
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    let logstream = state.logsource.subscribe();
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
