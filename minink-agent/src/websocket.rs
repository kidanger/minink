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
use minink_common::LogEntry;
use tokio::sync::broadcast;

use std::{net::SocketAddr, path::PathBuf};
use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, TraceLayer},
};

#[derive(Debug, Clone)]
struct AppState {
    log_watcher: broadcast::Sender<LogEntry>,
}

pub async fn main(log_watcher: broadcast::Sender<LogEntry>) -> Result<()> {
    let appstate = AppState { log_watcher };

    let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");

    // build our application with some routes
    let app = Router::new()
        .fallback_service(ServeDir::new(assets_dir).append_index_html_on_directories(true))
        .route("/ws/live", get(ws_handler))
        .with_state(appstate)
        // logging so we can see whats going on
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        );

    // run it with hyper
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await?;
    Ok(())
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    let rx = state.log_watcher.subscribe();
    ws.on_upgrade(move |socket| handle_socket(socket, rx))
}

async fn handle_socket(mut socket: WebSocket, mut log_receiver: broadcast::Receiver<LogEntry>) {
    loop {
        let entry = log_receiver.recv().await.unwrap();
        let payload = serde_json::to_string(&entry).unwrap();
        socket.send(Message::Text(payload)).await.unwrap();
    }
}
