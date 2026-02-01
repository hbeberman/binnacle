//! Upstream hub client for session servers.
//!
//! This module implements the WebSocket client that connects a session server
//! to an upstream hub when `--upstream <URL>` is specified.
//!
//! # Protocol
//!
//! 1. Connect to upstream WebSocket URL
//! 2. Send `register` message with session identity
//! 3. Send `heartbeat` every 30 seconds with ready_count and in_progress tasks
//! 4. Forward graph events as they occur
//! 5. Handle downstream commands from hub

use futures::{SinkExt, StreamExt};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, connect_async};

use crate::gui::protocol::{Change, DownstreamMessage, UpstreamMessage};
use crate::storage::Storage;

/// Type alias for the WebSocket stream type.
type WsStream = tokio_tungstenite::WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

/// Information about the current session for upstream registration.
#[derive(Debug, Clone)]
pub struct SessionInfo {
    /// Canonical session ID (repo hash).
    pub session_id: String,
    /// Human-readable display name (e.g., "binnacle@main").
    pub display_name: String,
    /// Absolute path to the repository.
    pub repo_path: String,
    /// Current git branch.
    pub branch: String,
}

/// Upstream connection manager.
///
/// Maintains a WebSocket connection to an upstream hub, handling:
/// - Initial registration
/// - Periodic heartbeats
/// - Event forwarding
/// - Reconnection on disconnect
pub struct UpstreamClient {
    /// Upstream hub URL (e.g., "wss://hub.example.com/sessions").
    url: String,
    /// Session information for registration.
    session_info: SessionInfo,
    /// Storage for reading current state.
    storage: Arc<Mutex<Storage>>,
    /// Channel for receiving graph updates to forward upstream.
    update_rx: broadcast::Receiver<String>,
}

impl UpstreamClient {
    /// Create a new upstream client.
    pub fn new(
        url: String,
        session_info: SessionInfo,
        storage: Arc<Mutex<Storage>>,
        update_rx: broadcast::Receiver<String>,
    ) -> Self {
        Self {
            url,
            session_info,
            storage,
            update_rx,
        }
    }

    /// Start the upstream connection loop.
    ///
    /// This method runs indefinitely, connecting to the upstream hub and
    /// handling reconnection on disconnect. Call this in a spawned task.
    pub async fn run(&mut self) {
        loop {
            match self.connect_and_run().await {
                Ok(()) => {
                    // Clean shutdown requested
                    eprintln!("[upstream] Disconnected from hub");
                    break;
                }
                Err(e) => {
                    eprintln!("[upstream] Connection error: {}. Reconnecting in 5s...", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }

    /// Connect to upstream and run the message loop.
    async fn connect_and_run(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        eprintln!("[upstream] Connecting to {}...", self.url);

        let (ws_stream, _response) = connect_async(&self.url).await?;
        let (mut write, mut read): (
            futures::stream::SplitSink<WsStream, Message>,
            futures::stream::SplitStream<WsStream>,
        ) = ws_stream.split();

        eprintln!("[upstream] Connected, sending register...");

        // Send registration message
        let register_msg = UpstreamMessage::Register {
            session_id: self.session_info.session_id.clone(),
            display_name: self.session_info.display_name.clone(),
            repo_path: self.session_info.repo_path.clone(),
            branch: self.session_info.branch.clone(),
        };
        let register_json = serde_json::to_string(&register_msg)?;
        write.send(Message::Text(register_json)).await?;

        // Set up heartbeat interval
        let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(30));

        loop {
            tokio::select! {
                // Heartbeat timer
                _ = heartbeat_interval.tick() => {
                    let heartbeat = self.build_heartbeat().await;
                    let heartbeat_json = serde_json::to_string(&heartbeat)?;
                    write.send(Message::Text(heartbeat_json)).await?;
                }

                // Incoming message from hub
                msg = read.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            self.handle_downstream_message(&text).await;
                        }
                        Some(Ok(Message::Ping(data))) => {
                            write.send(Message::Pong(data)).await?;
                        }
                        Some(Ok(Message::Close(_))) | None => {
                            // Hub closed connection
                            return Err("Hub closed connection".into());
                        }
                        Some(Err(e)) => {
                            return Err(format!("WebSocket error: {}", e).into());
                        }
                        _ => {}
                    }
                }

                // Graph update to forward
                update = self.update_rx.recv() => {
                    if let Ok(update_json) = update {
                        if let Some(event) = self.parse_graph_event(&update_json) {
                            let event_msg = UpstreamMessage::Event { event };
                            if let Ok(event_json) = serde_json::to_string(&event_msg) {
                                if let Err(e) = write.send(Message::Text(event_json)).await {
                                    eprintln!("[upstream] Failed to send event: {}", e);
                                    return Err(e.into());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Build a heartbeat message with current state.
    async fn build_heartbeat(&self) -> UpstreamMessage {
        let (ready_count, in_progress) = {
            let storage = self.storage.lock().await;

            // Count ready tasks
            let ready_count = storage
                .get_ready_tasks()
                .map(|v| v.len() as u64)
                .unwrap_or(0);

            // Get in-progress task IDs
            let in_progress: Vec<String> = storage
                .list_tasks(Some("in_progress"), None, None)
                .map(|tasks| tasks.iter().map(|t| t.core.id.clone()).collect())
                .unwrap_or_default();

            (ready_count, in_progress)
        };

        UpstreamMessage::Heartbeat {
            ready_count,
            in_progress,
        }
    }

    /// Handle a message received from the upstream hub.
    async fn handle_downstream_message(&self, text: &str) {
        match serde_json::from_str::<DownstreamMessage>(text) {
            Ok(DownstreamMessage::Command { id, cmd, args }) => {
                eprintln!(
                    "[upstream] Received command from hub: id={}, cmd={}, args={}",
                    id, cmd, args
                );
                // TODO: Execute command and send result back
                // This would require integrating with the command execution system
            }
            Ok(DownstreamMessage::Config { settings }) => {
                eprintln!("[upstream] Received config from hub: {}", settings);
                // TODO: Apply configuration
            }
            Ok(DownstreamMessage::Ack) => {
                eprintln!("[upstream] Received ack from hub");
            }
            Err(e) => {
                eprintln!("[upstream] Failed to parse hub message: {}", e);
            }
        }
    }

    /// Parse a graph update message into a Change for forwarding upstream.
    ///
    /// See [`parse_graph_event`] for details.
    fn parse_graph_event(&self, json: &str) -> Option<Change> {
        parse_graph_event(json)
    }
}

/// Parse a graph update message into a Change for forwarding upstream.
///
/// The broadcast channel sends JSON messages with types:
/// - `entity_added`: A new entity was created
/// - `entity_updated`: An existing entity was modified
/// - `entity_removed`: An entity was deleted
/// - `reload`, `log_entry`, `delta`: Non-graph events (ignored)
///
/// Returns `None` for messages that shouldn't be forwarded (log entries, reloads, etc.)
fn parse_graph_event(json: &str) -> Option<Change> {
    let msg: serde_json::Value = serde_json::from_str(json).ok()?;

    let msg_type = msg.get("type")?.as_str()?;

    match msg_type {
        "entity_added" => {
            let entity_type = msg.get("entity_type")?.as_str()?.to_string();
            let data = msg.get("entity")?.clone();
            Some(Change::Create { entity_type, data })
        }
        "entity_updated" => {
            let entity_type = msg.get("entity_type")?.as_str()?.to_string();
            let id = msg.get("id")?.as_str()?.to_string();
            // For updates, we send the full entity as "changes" since we don't
            // have the diff - the hub can compute the actual diff if needed
            let changes = msg.get("entity")?.clone();
            Some(Change::Update {
                entity_type,
                id,
                changes,
            })
        }
        "entity_removed" => {
            let entity_type = msg.get("entity_type")?.as_str()?.to_string();
            let id = msg.get("id")?.as_str()?.to_string();
            Some(Change::Delete { entity_type, id })
        }
        // Skip non-graph events
        "reload" | "log_entry" | "delta" => None,
        _ => None,
    }
}

/// Spawn an upstream connection task.
///
/// Returns a handle to the spawned task that can be used to abort it on shutdown.
pub fn spawn_upstream_client(
    url: String,
    session_info: SessionInfo,
    storage: Arc<Mutex<Storage>>,
    update_rx: broadcast::Receiver<String>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut client = UpstreamClient::new(url, session_info, storage, update_rx);
        client.run().await;
    })
}

/// Derive session ID from repo path (the repo hash used for storage).
pub fn derive_session_id(repo_path: &Path) -> String {
    crate::storage::compute_repo_hash(repo_path).unwrap_or_else(|_| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_info_creation() {
        let info = SessionInfo {
            session_id: "abc123".to_string(),
            display_name: "test@main".to_string(),
            repo_path: "/tmp/test".to_string(),
            branch: "main".to_string(),
        };
        assert_eq!(info.session_id, "abc123");
        assert_eq!(info.display_name, "test@main");
    }

    #[test]
    fn test_derive_session_id() {
        // Use current directory which is guaranteed to exist
        let path = std::env::current_dir().expect("current dir");
        let id = derive_session_id(&path);
        // Session ID should be a hex string (repo hash)
        assert!(!id.is_empty());
        // If it's "unknown", that's okay too (happens when canonicalize fails)
        if id != "unknown" {
            assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
        }
    }

    #[test]
    fn test_parse_graph_event_entity_added() {
        let json = r#"{
            "type": "entity_added",
            "entity_type": "task",
            "id": "bn-1234",
            "entity": {"id": "bn-1234", "title": "Test task", "status": "pending"},
            "version": 42,
            "timestamp": "2026-01-31T22:00:00Z"
        }"#;

        let change = parse_graph_event(json).expect("should parse entity_added");
        match change {
            Change::Create { entity_type, data } => {
                assert_eq!(entity_type, "task");
                assert_eq!(data["id"], "bn-1234");
                assert_eq!(data["title"], "Test task");
            }
            _ => panic!("expected Change::Create"),
        }
    }

    #[test]
    fn test_parse_graph_event_entity_updated() {
        let json = r#"{
            "type": "entity_updated",
            "entity_type": "task",
            "id": "bn-1234",
            "entity": {"id": "bn-1234", "title": "Test task", "status": "done"},
            "version": 43,
            "timestamp": "2026-01-31T22:01:00Z"
        }"#;

        let change = parse_graph_event(json).expect("should parse entity_updated");
        match change {
            Change::Update {
                entity_type,
                id,
                changes,
            } => {
                assert_eq!(entity_type, "task");
                assert_eq!(id, "bn-1234");
                assert_eq!(changes["status"], "done");
            }
            _ => panic!("expected Change::Update"),
        }
    }

    #[test]
    fn test_parse_graph_event_entity_removed() {
        let json = r#"{
            "type": "entity_removed",
            "entity_type": "task",
            "id": "bn-1234",
            "version": 44,
            "timestamp": "2026-01-31T22:02:00Z"
        }"#;

        let change = parse_graph_event(json).expect("should parse entity_removed");
        match change {
            Change::Delete { entity_type, id } => {
                assert_eq!(entity_type, "task");
                assert_eq!(id, "bn-1234");
            }
            _ => panic!("expected Change::Delete"),
        }
    }

    #[test]
    fn test_parse_graph_event_skips_reload() {
        let json = r#"{
            "type": "reload",
            "version": 45,
            "timestamp": "2026-01-31T22:03:00Z"
        }"#;

        assert!(parse_graph_event(json).is_none());
    }

    #[test]
    fn test_parse_graph_event_skips_log_entry() {
        let json = r#"{
            "type": "log_entry",
            "entry": {
                "timestamp": "2026-01-31T22:00:00Z",
                "entity_type": "task",
                "entity_id": "bn-1234",
                "action": "created"
            },
            "version": 46
        }"#;

        assert!(parse_graph_event(json).is_none());
    }

    #[test]
    fn test_parse_graph_event_skips_delta() {
        let json = r#"{
            "type": "delta",
            "changes": [],
            "version": 47,
            "timestamp": "2026-01-31T22:00:00Z"
        }"#;

        assert!(parse_graph_event(json).is_none());
    }

    #[test]
    fn test_parse_graph_event_invalid_json() {
        let json = "not valid json";
        assert!(parse_graph_event(json).is_none());
    }

    #[test]
    fn test_parse_graph_event_unknown_type() {
        let json = r#"{"type": "unknown_type"}"#;
        assert!(parse_graph_event(json).is_none());
    }

    #[test]
    fn test_parse_graph_event_missing_required_fields() {
        // Missing entity_type
        let json = r#"{"type": "entity_added", "entity": {}}"#;
        assert!(parse_graph_event(json).is_none());

        // Missing entity
        let json = r#"{"type": "entity_added", "entity_type": "task"}"#;
        assert!(parse_graph_event(json).is_none());

        // Missing id for update
        let json = r#"{"type": "entity_updated", "entity_type": "task", "entity": {}}"#;
        assert!(parse_graph_event(json).is_none());
    }
}
