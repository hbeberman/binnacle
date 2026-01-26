//! WebSocket handler for live updates

use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::Deserialize;
use tokio::sync::mpsc;

use super::server::AppState;

/// Incoming WebSocket message from client
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientMessage {
    /// Request synchronization, optionally with last known version
    RequestSync {
        /// Last version the client received (optional)
        last_version: Option<u64>,
    },
    /// Ping message (handled automatically by axum, but we can receive it)
    Ping,
}

/// WebSocket upgrade handler
pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

/// Handle WebSocket connection
async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let metrics = state.ws_metrics.clone();
    let version = state.version.clone();
    let current_version = version.current();

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

    // Create channel for sending responses from recv_task to send_task
    let (response_tx, mut response_rx) = mpsc::channel::<String>(32);

    // Clone metrics for the send task
    let send_metrics = metrics.clone();

    // Spawn a task to forward updates and responses to the client
    let mut send_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                // Handle broadcast updates from file watcher
                msg = rx.recv() => {
                    match msg {
                        Ok(msg) => {
                            if sender.send(Message::Text(msg)).await.is_err() {
                                break;
                            }
                            send_metrics.message_sent();
                        }
                        Err(_) => break,
                    }
                }
                // Handle direct responses from recv_task
                Some(msg) = response_rx.recv() => {
                    if sender.send(Message::Text(msg)).await.is_err() {
                        break;
                    }
                    send_metrics.message_sent();
                }
            }
        }
    });

    // Handle incoming messages
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    // Try to parse as a client message
                    if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                        match client_msg {
                            ClientMessage::RequestSync { last_version } => {
                                let current = version.current();
                                // For now, always send full sync since incremental messages
                                // aren't implemented yet. In the future, if last_version is
                                // recent enough and we have delta history, send incremental.
                                let response = if last_version.is_some_and(|v| v == current) {
                                    // Client is up to date, just acknowledge
                                    serde_json::json!({
                                        "type": "sync_ack",
                                        "version": current,
                                        "status": "up_to_date",
                                        "timestamp": chrono::Utc::now().to_rfc3339()
                                    })
                                } else {
                                    // Client needs to reload - send reload message
                                    // Future: could send delta if gap is small
                                    serde_json::json!({
                                        "type": "sync_response",
                                        "version": current,
                                        "action": "reload",
                                        "reason": if last_version.is_none() {
                                            "no_version_provided"
                                        } else {
                                            "version_gap"
                                        },
                                        "timestamp": chrono::Utc::now().to_rfc3339()
                                    })
                                };
                                let _ = response_tx.send(response.to_string()).await;
                            }
                            ClientMessage::Ping => {
                                // Ping messages are handled automatically by axum
                                tracing::debug!("Received ping message");
                            }
                        }
                    } else {
                        tracing::debug!("Received unknown message: {}", text);
                    }
                }
                Message::Ping(data) => {
                    // Axum handles pong automatically
                    tracing::debug!("Received ping: {:?}", data);
                }
                _ => {}
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
