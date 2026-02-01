//! WebSocket protocol types for session server communication.
//!
//! This module defines the message types exchanged between clients (GUI, TUI, orchestrators)
//! and the session server over WebSocket connections.
//!
//! # Protocol Overview
//!
//! The session server exposes a WebSocket endpoint at `/ws`. Messages are JSON-encoded
//! and use a `type` field for discrimination.
//!
//! ## Client → Server Messages ([`ClientMessage`])
//! - `subscribe`: Subscribe to state change notifications
//! - `command`: Execute a binnacle command
//! - `ping`: Keepalive ping
//!
//! ## Server → Client Messages ([`ServerMessage`])
//! - `state`: Full state snapshot
//! - `delta`: Incremental state update
//! - `result`: Command execution result
//! - `pong`: Keepalive response

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ============================================================================
// Client → Server Messages
// ============================================================================

/// Messages sent from clients to the session server.
///
/// All messages are JSON-encoded with a `type` field for discrimination.
///
/// # Examples
///
/// ```json
/// {"type": "subscribe", "topics": ["tasks", "bugs"]}
/// {"type": "command", "id": "cmd-123", "cmd": "task list", "args": {"status": "open"}}
/// {"type": "ping"}
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Subscribe to state change notifications.
    ///
    /// Topics control which updates the client receives:
    /// - `"tasks"`: Task create/update/delete events
    /// - `"bugs"`: Bug create/update/delete events
    /// - `"tests"`: Test node and result events
    /// - `"*"` or empty: All events
    Subscribe {
        /// Topics to subscribe to. Empty or `["*"]` means all.
        topics: Vec<String>,
    },

    /// Execute a binnacle command.
    ///
    /// The server will execute the command and send back a `result` message
    /// with the same `id` for correlation.
    Command {
        /// Client-generated correlation ID for matching responses.
        id: String,
        /// The command to execute (e.g., "task list", "task create").
        cmd: String,
        /// Command arguments as key-value pairs.
        #[serde(default)]
        args: serde_json::Value,
    },

    /// Keepalive ping message.
    ///
    /// The server responds with a `pong` message.
    Ping,
}

// ============================================================================
// Server → Client Messages
// ============================================================================

/// Messages sent from the session server to clients.
///
/// All messages are JSON-encoded with a `type` field for discrimination.
///
/// # Examples
///
/// ```json
/// {"type": "state", "data": {...}, "version": 42, "timestamp": "2026-01-31T22:00:00Z"}
/// {"type": "delta", "changes": [...], "version": 43, "timestamp": "2026-01-31T22:01:00Z"}
/// {"type": "result", "id": "cmd-123", "success": true, "data": {...}}
/// {"type": "pong"}
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Full state snapshot.
    ///
    /// Sent on initial connection or when incremental updates aren't possible
    /// (e.g., client version too old).
    State {
        /// The complete graph state (boxed to reduce enum size).
        data: Box<GraphState>,
        /// Monotonic version number for delta tracking.
        version: u64,
        /// Server timestamp when this snapshot was generated.
        timestamp: DateTime<Utc>,
    },

    /// Incremental state update.
    ///
    /// Contains only the changes since the client's last known version.
    Delta {
        /// List of changes to apply.
        changes: Vec<Change>,
        /// New version number after applying changes.
        version: u64,
        /// Server timestamp when this delta was generated.
        timestamp: DateTime<Utc>,
    },

    /// Command execution result.
    ///
    /// Sent in response to a `command` message.
    Result {
        /// Correlation ID from the original command.
        id: String,
        /// Whether the command succeeded.
        success: bool,
        /// Result data on success, or null on failure.
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<serde_json::Value>,
        /// Error message on failure.
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },

    /// Keepalive response to a ping.
    Pong,
}

// ============================================================================
// State Types
// ============================================================================

/// Complete graph state for a binnacle instance.
///
/// This represents the full state of all entities in the repository,
/// suitable for initial sync or full refresh.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GraphState {
    /// All tasks in the repository.
    pub tasks: Vec<serde_json::Value>,
    /// All bugs in the repository.
    pub bugs: Vec<serde_json::Value>,
    /// All test nodes.
    pub tests: Vec<serde_json::Value>,
    /// All milestones.
    pub milestones: Vec<serde_json::Value>,
    /// All ideas.
    pub ideas: Vec<serde_json::Value>,
    /// All documentation nodes.
    pub docs: Vec<serde_json::Value>,
    /// All links between entities.
    pub links: Vec<serde_json::Value>,
    /// The work queue (if present).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue: Option<serde_json::Value>,
    /// Summary statistics.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<StateSummary>,
}

/// Summary statistics for the graph state.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateSummary {
    /// Total number of tasks.
    pub total_tasks: u64,
    /// Number of tasks ready to work on.
    pub ready_count: u64,
    /// Number of blocked tasks.
    pub blocked_count: u64,
    /// Number of tasks in progress.
    pub in_progress_count: u64,
    /// Total number of bugs.
    pub total_bugs: u64,
    /// Number of open bugs.
    pub open_bugs_count: u64,
    /// Number of critical bugs.
    pub critical_bugs_count: u64,
}

// ============================================================================
// Delta/Change Types
// ============================================================================

/// A single change in the graph.
///
/// Changes are used for incremental updates to avoid sending full state
/// on every modification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Change {
    /// An entity was created.
    Create {
        /// Type of entity ("task", "bug", "test", "milestone", "idea", "doc", "link", "queue").
        entity_type: String,
        /// The created entity data.
        data: serde_json::Value,
    },

    /// An entity was updated.
    Update {
        /// Type of entity.
        entity_type: String,
        /// Entity ID.
        id: String,
        /// Only the fields that changed.
        changes: serde_json::Value,
    },

    /// An entity was deleted.
    Delete {
        /// Type of entity.
        entity_type: String,
        /// Entity ID.
        id: String,
    },
}

// ============================================================================
// Upstream Hub Protocol
// ============================================================================

/// Messages sent from session server to an upstream hub.
///
/// These messages are sent when `--upstream <URL>` is specified for
/// `bn session serve`. The session registers with the hub, sends
/// periodic heartbeats, and forwards graph events.
///
/// # Examples
///
/// ```json
/// {"type": "register", "session_id": "03d0394e228f", "display_name": "binnacle@main", "repo_path": "/path/to/repo", "branch": "main"}
/// {"type": "heartbeat", "ready_count": 5, "in_progress": ["bn-1234", "bn-5678"]}
/// {"type": "event", "event": {...}}
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UpstreamMessage {
    /// Register this session with the hub.
    ///
    /// Sent immediately after connecting to the upstream hub.
    Register {
        /// Canonical session ID (repo hash).
        session_id: String,
        /// Human-readable display name (e.g., "binnacle@main").
        display_name: String,
        /// Absolute path to the repository.
        repo_path: String,
        /// Current git branch.
        branch: String,
    },

    /// Periodic heartbeat with current state summary.
    ///
    /// Sent every 30 seconds while connected.
    Heartbeat {
        /// Number of tasks ready to work on.
        ready_count: u64,
        /// IDs of tasks currently in progress.
        in_progress: Vec<String>,
    },

    /// Graph event notification.
    ///
    /// Sent when the graph state changes (task created, updated, etc.).
    Event {
        /// The change that occurred.
        event: Change,
    },

    /// Command execution result.
    ///
    /// Sent in response to a `command` message from the hub.
    CommandResult {
        /// Correlation ID from the original command.
        id: String,
        /// Whether the command succeeded.
        success: bool,
        /// Result data on success, or null on failure.
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<serde_json::Value>,
        /// Error message on failure.
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
}

/// Messages sent from an upstream hub to session servers.
///
/// These messages allow the hub to issue commands or push configuration
/// to connected sessions.
///
/// # Examples
///
/// ```json
/// {"type": "command", "id": "hub-123", "cmd": "task create", "args": {"title": "From hub"}}
/// {"type": "config", "settings": {"max_concurrent": 3}}
/// {"type": "ack"}
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DownstreamMessage {
    /// Execute a command on this session.
    Command {
        /// Hub-generated correlation ID.
        id: String,
        /// The command to execute.
        cmd: String,
        /// Command arguments.
        #[serde(default)]
        args: serde_json::Value,
    },

    /// Configuration push from hub.
    Config {
        /// Configuration settings to apply.
        settings: serde_json::Value,
    },

    /// Acknowledgment of a message (e.g., register success).
    Ack,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_message_subscribe_serialization() {
        let msg = ClientMessage::Subscribe {
            topics: vec!["tasks".to_string(), "bugs".to_string()],
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"subscribe""#));
        assert!(json.contains(r#""topics":["tasks","bugs"]"#));

        let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn test_client_message_command_serialization() {
        let msg = ClientMessage::Command {
            id: "cmd-123".to_string(),
            cmd: "task list".to_string(),
            args: serde_json::json!({"status": "open"}),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"command""#));
        assert!(json.contains(r#""id":"cmd-123""#));
        assert!(json.contains(r#""cmd":"task list""#));

        let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn test_client_message_ping_serialization() {
        let msg = ClientMessage::Ping;
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"type":"ping"}"#);

        let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn test_server_message_state_serialization() {
        let msg = ServerMessage::State {
            data: Box::new(GraphState::default()),
            version: 42,
            timestamp: DateTime::parse_from_rfc3339("2026-01-31T22:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"state""#));
        assert!(json.contains(r#""version":42"#));

        let parsed: ServerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn test_server_message_delta_serialization() {
        let msg = ServerMessage::Delta {
            changes: vec![Change::Create {
                entity_type: "task".to_string(),
                data: serde_json::json!({"id": "bn-1234", "title": "Test task"}),
            }],
            version: 43,
            timestamp: DateTime::parse_from_rfc3339("2026-01-31T22:01:00Z")
                .unwrap()
                .with_timezone(&Utc),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"delta""#));
        assert!(json.contains(r#""changes""#));

        let parsed: ServerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn test_server_message_result_success_serialization() {
        let msg = ServerMessage::Result {
            id: "cmd-123".to_string(),
            success: true,
            data: Some(serde_json::json!({"tasks": []})),
            error: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"result""#));
        assert!(json.contains(r#""success":true"#));
        assert!(!json.contains(r#""error""#)); // Skipped when None

        let parsed: ServerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn test_server_message_result_error_serialization() {
        let msg = ServerMessage::Result {
            id: "cmd-456".to_string(),
            success: false,
            data: None,
            error: Some("Task not found".to_string()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"result""#));
        assert!(json.contains(r#""success":false"#));
        assert!(json.contains(r#""error":"Task not found""#));
        assert!(!json.contains(r#""data""#)); // Skipped when None

        let parsed: ServerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn test_server_message_pong_serialization() {
        let msg = ServerMessage::Pong;
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"type":"pong"}"#);

        let parsed: ServerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn test_change_create_serialization() {
        let change = Change::Create {
            entity_type: "task".to_string(),
            data: serde_json::json!({"id": "bn-abcd", "title": "New task"}),
        };
        let json = serde_json::to_string(&change).unwrap();
        assert!(json.contains(r#""op":"create""#));
        assert!(json.contains(r#""entity_type":"task""#));

        let parsed: Change = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, change);
    }

    #[test]
    fn test_change_update_serialization() {
        let change = Change::Update {
            entity_type: "task".to_string(),
            id: "bn-abcd".to_string(),
            changes: serde_json::json!({"status": "done"}),
        };
        let json = serde_json::to_string(&change).unwrap();
        assert!(json.contains(r#""op":"update""#));
        assert!(json.contains(r#""id":"bn-abcd""#));

        let parsed: Change = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, change);
    }

    #[test]
    fn test_change_delete_serialization() {
        let change = Change::Delete {
            entity_type: "task".to_string(),
            id: "bn-abcd".to_string(),
        };
        let json = serde_json::to_string(&change).unwrap();
        assert!(json.contains(r#""op":"delete""#));
        assert!(json.contains(r#""id":"bn-abcd""#));

        let parsed: Change = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, change);
    }

    #[test]
    fn test_graph_state_default() {
        let state = GraphState::default();
        assert!(state.tasks.is_empty());
        assert!(state.bugs.is_empty());
        assert!(state.tests.is_empty());
        assert!(state.milestones.is_empty());
        assert!(state.ideas.is_empty());
        assert!(state.docs.is_empty());
        assert!(state.links.is_empty());
        assert!(state.queue.is_none());
        assert!(state.summary.is_none());
    }

    #[test]
    fn test_state_summary_serialization() {
        let summary = StateSummary {
            total_tasks: 100,
            ready_count: 10,
            blocked_count: 5,
            in_progress_count: 3,
            total_bugs: 20,
            open_bugs_count: 8,
            critical_bugs_count: 2,
        };
        let json = serde_json::to_string(&summary).unwrap();
        let parsed: StateSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, summary);
    }

    #[test]
    fn test_client_message_command_default_args() {
        // Test that args defaults to null/empty when not provided
        let json = r#"{"type":"command","id":"test","cmd":"task list"}"#;
        let parsed: ClientMessage = serde_json::from_str(json).unwrap();
        match parsed {
            ClientMessage::Command { args, .. } => {
                assert!(args.is_null());
            }
            _ => panic!("Expected Command variant"),
        }
    }

    // ========================================================================
    // Upstream Protocol Tests
    // ========================================================================

    #[test]
    fn test_upstream_message_register_serialization() {
        let msg = UpstreamMessage::Register {
            session_id: "03d0394e228f".to_string(),
            display_name: "binnacle@main".to_string(),
            repo_path: "/home/user/binnacle".to_string(),
            branch: "main".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"register""#));
        assert!(json.contains(r#""session_id":"03d0394e228f""#));
        assert!(json.contains(r#""display_name":"binnacle@main""#));

        let parsed: UpstreamMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn test_upstream_message_heartbeat_serialization() {
        let msg = UpstreamMessage::Heartbeat {
            ready_count: 5,
            in_progress: vec!["bn-1234".to_string(), "bn-5678".to_string()],
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"heartbeat""#));
        assert!(json.contains(r#""ready_count":5"#));
        assert!(json.contains(r#""in_progress""#));

        let parsed: UpstreamMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn test_upstream_message_event_serialization() {
        let msg = UpstreamMessage::Event {
            event: Change::Create {
                entity_type: "task".to_string(),
                data: serde_json::json!({"id": "bn-abcd", "title": "Test"}),
            },
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"event""#));
        assert!(json.contains(r#""event""#));

        let parsed: UpstreamMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn test_upstream_message_command_result_success_serialization() {
        let msg = UpstreamMessage::CommandResult {
            id: "hub-123".to_string(),
            success: true,
            data: Some(serde_json::json!({"tasks": []})),
            error: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"command_result""#));
        assert!(json.contains(r#""id":"hub-123""#));
        assert!(json.contains(r#""success":true"#));
        assert!(!json.contains(r#""error""#)); // Skipped when None

        let parsed: UpstreamMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn test_upstream_message_command_result_error_serialization() {
        let msg = UpstreamMessage::CommandResult {
            id: "hub-456".to_string(),
            success: false,
            data: None,
            error: Some("Task not found".to_string()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"command_result""#));
        assert!(json.contains(r#""success":false"#));
        assert!(json.contains(r#""error":"Task not found""#));
        assert!(!json.contains(r#""data""#)); // Skipped when None

        let parsed: UpstreamMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn test_downstream_message_command_serialization() {
        let msg = DownstreamMessage::Command {
            id: "hub-123".to_string(),
            cmd: "task create".to_string(),
            args: serde_json::json!({"title": "From hub"}),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"command""#));
        assert!(json.contains(r#""id":"hub-123""#));

        let parsed: DownstreamMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn test_downstream_message_config_serialization() {
        let msg = DownstreamMessage::Config {
            settings: serde_json::json!({"max_concurrent": 3}),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"config""#));

        let parsed: DownstreamMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn test_downstream_message_ack_serialization() {
        let msg = DownstreamMessage::Ack;
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"type":"ack"}"#);

        let parsed: DownstreamMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }
}
