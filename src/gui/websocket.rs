//! WebSocket handler for live updates

use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use futures::{sink::SinkExt, stream::StreamExt};

use super::server::AppState;

/// WebSocket upgrade handler
pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

/// Handle WebSocket connection
async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let metrics = state.ws_metrics.clone();
    let current_version = state.version.current();

    // Track connection opened
    metrics.connection_opened();

    // Send initial connection message with current version
    let connected_msg = serde_json::json!({
        "type": "connected",
        "version": current_version,
        "timestamp": chrono::Utc::now().to_rfc3339()
    })
    .to_string();

    if sender.send(Message::Text(connected_msg)).await.is_err() {
        metrics.connection_closed();
        return;
    }
    metrics.message_sent();

    // Subscribe to updates
    let mut rx = state.update_tx.subscribe();

    // Clone metrics for the send task
    let send_metrics = metrics.clone();

    // Spawn a task to forward updates to the client
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg)).await.is_err() {
                break;
            }
            send_metrics.message_sent();
        }
    });

    // Handle incoming messages (for now, just echo or ignore)
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            // Handle ping/pong
            if let Message::Ping(data) = msg {
                // Axum handles pong automatically, but we can log it
                tracing::debug!("Received ping: {:?}", data);
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }

    // Track connection closed
    metrics.connection_closed();
}
