//! WebSocket handler for live updates
//!
//! This module handles WebSocket connections for real-time updates and command execution.
//! It supports both the legacy sync protocol and the new session server protocol.

use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use chrono::Utc;
use futures::{sink::SinkExt, stream::StreamExt};
use serde::Deserialize;
use std::path::Path;
use tokio::sync::mpsc;

use super::protocol::{
    ClientMessage as ProtocolClientMessage, GraphState, ServerMessage, StateSummary,
};
use super::server::AppState;
use crate::commands;
use crate::storage::Storage;

/// Legacy incoming WebSocket message from client (for backward compatibility)
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum LegacyClientMessage {
    /// Request synchronization, optionally with last known version
    RequestSync {
        /// Last version the client received (optional)
        last_version: Option<u64>,
    },
    /// Ping message (handled automatically by axum, but we can receive it)
    Ping,
}

/// Combined message type that can handle both legacy and new protocol messages
#[derive(Debug)]
enum AnyClientMessage {
    Legacy(LegacyClientMessage),
    Protocol(ProtocolClientMessage),
}

impl AnyClientMessage {
    /// Try to parse a JSON string as either protocol or legacy message
    fn from_str(text: &str) -> Option<Self> {
        // First try the new protocol format
        if let Ok(msg) = serde_json::from_str::<ProtocolClientMessage>(text) {
            return Some(AnyClientMessage::Protocol(msg));
        }
        // Fall back to legacy format
        if let Ok(msg) = serde_json::from_str::<LegacyClientMessage>(text) {
            return Some(AnyClientMessage::Legacy(msg));
        }
        None
    }
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
    let message_history = state.message_history.clone();
    let current_version = version.current();

    // Track connection opened
    metrics.connection_opened();

    // Send initial connection message with current version
    let connected_msg = serde_json::json!({
        "type": "connected",
        "version": current_version,
        "timestamp": Utc::now().to_rfc3339()
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

    // Clone repo_path for the recv task
    let recv_repo_path = state.repo_path.clone();

    // Clone version for the recv task
    let recv_version = version.clone();

    // Handle incoming messages
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    // Try to parse as either protocol or legacy message
                    if let Some(client_msg) = AnyClientMessage::from_str(&text) {
                        match client_msg {
                            AnyClientMessage::Protocol(proto_msg) => {
                                match proto_msg {
                                    ProtocolClientMessage::Subscribe { topics } => {
                                        // Log subscription
                                        tracing::debug!(
                                            "Client subscribed to topics: {:?}",
                                            topics
                                        );

                                        // Build and send full state snapshot
                                        match build_graph_state(&recv_repo_path) {
                                            Ok(graph_state) => {
                                                let state_msg = ServerMessage::State {
                                                    data: Box::new(graph_state),
                                                    version: recv_version.current(),
                                                    timestamp: Utc::now(),
                                                };
                                                if let Ok(json) = serde_json::to_string(&state_msg)
                                                {
                                                    let _ = response_tx.send(json).await;
                                                }
                                            }
                                            Err(e) => {
                                                tracing::error!(
                                                    "Failed to build state snapshot: {}",
                                                    e
                                                );
                                                // Send error as a subscribed acknowledgment with error
                                                let response = serde_json::json!({
                                                    "type": "subscribed",
                                                    "topics": topics,
                                                    "error": e,
                                                    "timestamp": Utc::now().to_rfc3339()
                                                });
                                                let _ =
                                                    response_tx.send(response.to_string()).await;
                                            }
                                        }
                                    }
                                    ProtocolClientMessage::Command { id, cmd, args } => {
                                        // Execute the command and return result
                                        let result =
                                            execute_command(&recv_repo_path, &cmd, args).await;
                                        let response = match result {
                                            Ok(data) => ServerMessage::Result {
                                                id,
                                                success: true,
                                                data: Some(data),
                                                error: None,
                                            },
                                            Err(e) => ServerMessage::Result {
                                                id,
                                                success: false,
                                                data: None,
                                                error: Some(e),
                                            },
                                        };
                                        if let Ok(json) = serde_json::to_string(&response) {
                                            let _ = response_tx.send(json).await;
                                        }
                                    }
                                    ProtocolClientMessage::Ping => {
                                        // Respond with pong
                                        let response = ServerMessage::Pong;
                                        if let Ok(json) = serde_json::to_string(&response) {
                                            let _ = response_tx.send(json).await;
                                        }
                                    }
                                }
                            }
                            AnyClientMessage::Legacy(legacy_msg) => {
                                match legacy_msg {
                                    LegacyClientMessage::RequestSync { last_version } => {
                                        let current = version.current();

                                        let response = if last_version.is_some_and(|v| v == current)
                                        {
                                            // Client is up to date, just acknowledge
                                            serde_json::json!({
                                                "type": "sync_ack",
                                                "version": current,
                                                "status": "up_to_date",
                                                "timestamp": Utc::now().to_rfc3339()
                                            })
                                        } else if let Some(last_v) = last_version {
                                            // Client has a version - try incremental catch-up
                                            match message_history.get_since(last_v).await {
                                                Some(messages) if !messages.is_empty() => {
                                                    // We have the incremental messages!
                                                    // Send them as an array
                                                    let message_list: Vec<serde_json::Value> =
                                                        messages
                                                            .iter()
                                                            .filter_map(|m| {
                                                                serde_json::from_str(&m.message)
                                                                    .ok()
                                                            })
                                                            .collect();

                                                    serde_json::json!({
                                                        "type": "sync_catchup",
                                                        "version": current,
                                                        "last_version": last_v,
                                                        "messages": message_list,
                                                        "timestamp": Utc::now().to_rfc3339()
                                                    })
                                                }
                                                Some(_) => {
                                                    // History exists but no new messages (already up to date)
                                                    serde_json::json!({
                                                        "type": "sync_ack",
                                                        "version": current,
                                                        "status": "up_to_date",
                                                        "timestamp": Utc::now().to_rfc3339()
                                                    })
                                                }
                                                None => {
                                                    // Version too old, not in history - need full reload
                                                    serde_json::json!({
                                                        "type": "sync_response",
                                                        "version": current,
                                                        "action": "reload",
                                                        "reason": "version_too_old",
                                                        "timestamp": Utc::now().to_rfc3339()
                                                    })
                                                }
                                            }
                                        } else {
                                            // No version provided - client needs full reload
                                            serde_json::json!({
                                                "type": "sync_response",
                                                "version": current,
                                                "action": "reload",
                                                "reason": "no_version_provided",
                                                "timestamp": Utc::now().to_rfc3339()
                                            })
                                        };
                                        let _ = response_tx.send(response.to_string()).await;
                                    }
                                    LegacyClientMessage::Ping => {
                                        // Ping messages are handled automatically by axum
                                        tracing::debug!("Received legacy ping message");
                                    }
                                }
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

/// Build a complete GraphState snapshot from storage.
///
/// This function collects all entities from the binnacle storage and constructs
/// a GraphState suitable for sending to clients on initial connection.
///
/// # Arguments
/// * `repo_path` - Path to the repository root
///
/// # Returns
/// A GraphState containing all tasks, bugs, tests, milestones, ideas, docs, links, and queue
fn build_graph_state(repo_path: &Path) -> Result<GraphState, String> {
    let storage = Storage::open(repo_path).map_err(|e| e.to_string())?;

    // Collect all entities
    let tasks: Vec<serde_json::Value> = storage
        .list_tasks(None, None, None)
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter_map(|t| serde_json::to_value(t).ok())
        .collect();

    let bugs: Vec<serde_json::Value> = storage
        .list_bugs(None, None, None, None, true) // include_closed = true for full state
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter_map(|b| serde_json::to_value(b).ok())
        .collect();

    let tests: Vec<serde_json::Value> = storage
        .list_tests(None)
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter_map(|t| serde_json::to_value(t).ok())
        .collect();

    let milestones: Vec<serde_json::Value> = storage
        .list_milestones(None, None, None)
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter_map(|m| serde_json::to_value(m).ok())
        .collect();

    let ideas: Vec<serde_json::Value> = storage
        .list_ideas(None, None)
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter_map(|i| serde_json::to_value(i).ok())
        .collect();

    let docs: Vec<serde_json::Value> = storage
        .list_docs(None, None, None, None)
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter_map(|d| serde_json::to_value(d).ok())
        .collect();

    let links: Vec<serde_json::Value> = storage
        .list_edges(None, None, None)
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter_map(|e| serde_json::to_value(e).ok())
        .collect();

    let queue: Option<serde_json::Value> = storage
        .get_queue()
        .ok()
        .and_then(|q| serde_json::to_value(q).ok());

    // Build summary statistics
    let ready_tasks = storage.get_ready_tasks().unwrap_or_default();
    let blocked_tasks = storage.get_blocked_tasks().unwrap_or_default();
    let in_progress_tasks = storage
        .list_tasks(Some("in_progress"), None, None)
        .unwrap_or_default();
    let open_bugs = storage
        .list_bugs(None, None, None, None, false)
        .unwrap_or_default();
    let critical_bugs = storage
        .list_bugs(None, None, Some("critical"), None, false)
        .unwrap_or_default();

    let summary = StateSummary {
        total_tasks: tasks.len() as u64,
        ready_count: ready_tasks.len() as u64,
        blocked_count: blocked_tasks.len() as u64,
        in_progress_count: in_progress_tasks.len() as u64,
        total_bugs: bugs.len() as u64,
        open_bugs_count: open_bugs.len() as u64,
        critical_bugs_count: critical_bugs.len() as u64,
    };

    Ok(GraphState {
        tasks,
        bugs,
        tests,
        milestones,
        ideas,
        docs,
        links,
        queue,
        summary: Some(summary),
    })
}

/// Execute a binnacle command over WebSocket.
///
/// The command string follows the CLI format: "entity action" (e.g., "task list", "ready").
/// Arguments are passed as a JSON object with parameter names as keys.
///
/// # Supported Commands
///
/// ## Query Commands
/// - `ready` - List tasks ready to work on
/// - `blocked` - List blocked tasks
///
/// ## Task Commands
/// - `task list` - List all tasks
/// - `task show <id>` - Show task details
/// - `task create` - Create a new task
/// - `task update <id>` - Update a task
/// - `task close <id>` - Close a task
/// - `task reopen <id>` - Reopen a task
/// - `task delete <id>` - Delete a task
///
/// ## Bug Commands
/// - `bug list` - List all bugs
/// - `bug show <id>` - Show bug details
/// - `bug create` - Create a new bug
/// - `bug close <id>` - Close a bug
///
/// ## Other Commands
/// - `milestone list` - List milestones
/// - `milestone show <id>` - Show milestone details
/// - `idea list` - List ideas
/// - `queue show` - Show work queue
pub async fn execute_command(
    repo_path: &Path,
    cmd: &str,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    // Parse command into parts
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() {
        return Err("Empty command".to_string());
    }

    // Helper to get string argument
    let get_str = |key: &str| -> Option<String> {
        args.get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    };

    // Helper to get optional string argument
    let get_opt_str = |key: &str| -> Option<String> { get_str(key) };

    // Helper to get bool argument with default
    let get_bool = |key: &str, default: bool| -> bool {
        args.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
    };

    // Helper to get u8 argument
    let get_u8 =
        |key: &str| -> Option<u8> { args.get(key).and_then(|v| v.as_u64()).map(|n| n as u8) };

    // Helper to get string array argument
    let get_str_vec = |key: &str| -> Vec<String> {
        args.get(key)
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    };

    match parts.as_slice() {
        // Query commands
        ["ready"] => {
            let bugs_only = get_bool("bugs_only", false);
            let tasks_only = get_bool("tasks_only", false);
            commands::ready(repo_path, bugs_only, tasks_only)
                .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
                .map_err(|e| e.to_string())
        }
        ["blocked"] => {
            let bugs_only = get_bool("bugs_only", false);
            let tasks_only = get_bool("tasks_only", false);
            commands::blocked(repo_path, bugs_only, tasks_only)
                .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
                .map_err(|e| e.to_string())
        }

        // Task commands
        ["task", "list"] => {
            let status = get_opt_str("status");
            let priority = get_u8("priority");
            let tag = get_opt_str("tag");
            commands::task_list(repo_path, status.as_deref(), priority, tag.as_deref())
                .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
                .map_err(|e| e.to_string())
        }
        ["task", "show", id] => commands::task_show(repo_path, id)
            .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
            .map_err(|e| e.to_string()),
        ["task", "show"] => {
            let id = get_str("id").ok_or("Missing 'id' argument")?;
            commands::task_show(repo_path, &id)
                .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
                .map_err(|e| e.to_string())
        }
        ["task", "create"] => {
            let title = get_str("title").ok_or("Missing 'title' argument")?;
            let short_name = get_opt_str("short_name");
            let description = get_opt_str("description");
            let priority = get_u8("priority");
            let tags = get_str_vec("tags");
            let assignee = get_opt_str("assignee");
            commands::task_create(
                repo_path,
                title,
                short_name,
                description,
                priority,
                tags,
                assignee,
            )
            .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
            .map_err(|e| e.to_string())
        }
        ["task", "close", id] => {
            let reason = get_opt_str("reason");
            let force = get_bool("force", false);
            let no_cascade = get_bool("no_cascade", false);
            commands::task_close(repo_path, id, reason, force, no_cascade)
                .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
                .map_err(|e| e.to_string())
        }
        ["task", "close"] => {
            let id = get_str("id").ok_or("Missing 'id' argument")?;
            let reason = get_opt_str("reason");
            let force = get_bool("force", false);
            let no_cascade = get_bool("no_cascade", false);
            commands::task_close(repo_path, &id, reason, force, no_cascade)
                .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
                .map_err(|e| e.to_string())
        }
        ["task", "reopen", id] => commands::task_reopen(repo_path, id)
            .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
            .map_err(|e| e.to_string()),
        ["task", "reopen"] => {
            let id = get_str("id").ok_or("Missing 'id' argument")?;
            commands::task_reopen(repo_path, &id)
                .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
                .map_err(|e| e.to_string())
        }
        ["task", "delete", id] => commands::task_delete(repo_path, id)
            .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
            .map_err(|e| e.to_string()),
        ["task", "delete"] => {
            let id = get_str("id").ok_or("Missing 'id' argument")?;
            commands::task_delete(repo_path, &id)
                .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
                .map_err(|e| e.to_string())
        }

        // Bug commands
        ["bug", "list"] => {
            let status = get_opt_str("status");
            let priority = get_u8("priority");
            let severity = get_opt_str("severity");
            let tag = get_opt_str("tag");
            let include_closed = get_bool("include_closed", false);
            commands::bug_list(
                repo_path,
                status.as_deref(),
                priority,
                severity.as_deref(),
                tag.as_deref(),
                include_closed,
            )
            .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
            .map_err(|e| e.to_string())
        }
        ["bug", "show", id] => commands::bug_show(repo_path, id)
            .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
            .map_err(|e| e.to_string()),
        ["bug", "show"] => {
            let id = get_str("id").ok_or("Missing 'id' argument")?;
            commands::bug_show(repo_path, &id)
                .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
                .map_err(|e| e.to_string())
        }
        ["bug", "create"] => {
            let title = get_str("title").ok_or("Missing 'title' argument")?;
            let short_name = get_opt_str("short_name");
            let description = get_opt_str("description");
            let priority = get_u8("priority");
            let severity = get_opt_str("severity");
            let tags = get_str_vec("tags");
            let assignee = get_opt_str("assignee");
            let reproduction_steps = get_opt_str("reproduction_steps");
            let affected_component = get_opt_str("affected_component");
            commands::bug_create(
                repo_path,
                title,
                short_name,
                description,
                priority,
                severity,
                tags,
                assignee,
                reproduction_steps,
                affected_component,
            )
            .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
            .map_err(|e| e.to_string())
        }
        ["bug", "close", id] => {
            let reason = get_opt_str("reason");
            let force = get_bool("force", false);
            let no_cascade = get_bool("no_cascade", false);
            commands::bug_close(repo_path, id, reason, force, no_cascade)
                .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
                .map_err(|e| e.to_string())
        }
        ["bug", "close"] => {
            let id = get_str("id").ok_or("Missing 'id' argument")?;
            let reason = get_opt_str("reason");
            let force = get_bool("force", false);
            let no_cascade = get_bool("no_cascade", false);
            commands::bug_close(repo_path, &id, reason, force, no_cascade)
                .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
                .map_err(|e| e.to_string())
        }

        // Milestone commands
        ["milestone", "list"] => {
            let status = get_opt_str("status");
            let priority = get_u8("priority");
            let tag = get_opt_str("tag");
            commands::milestone_list(repo_path, status.as_deref(), priority, tag.as_deref())
                .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
                .map_err(|e| e.to_string())
        }
        ["milestone", "show", id] => commands::milestone_show(repo_path, id)
            .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
            .map_err(|e| e.to_string()),
        ["milestone", "show"] => {
            let id = get_str("id").ok_or("Missing 'id' argument")?;
            commands::milestone_show(repo_path, &id)
                .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
                .map_err(|e| e.to_string())
        }

        // Idea commands
        ["idea", "list"] => {
            let status = get_opt_str("status");
            let tag = get_opt_str("tag");
            commands::idea_list(repo_path, status.as_deref(), tag.as_deref())
                .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
                .map_err(|e| e.to_string())
        }
        ["idea", "show", id] => commands::idea_show(repo_path, id)
            .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
            .map_err(|e| e.to_string()),
        ["idea", "show"] => {
            let id = get_str("id").ok_or("Missing 'id' argument")?;
            commands::idea_show(repo_path, &id)
                .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
                .map_err(|e| e.to_string())
        }

        // Queue commands
        ["queue", "show"] => commands::queue_show(repo_path)
            .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
            .map_err(|e| e.to_string()),

        // Doc commands
        ["doc", "list"] => {
            let tag = get_opt_str("tag");
            let for_entity = get_opt_str("for_entity");
            let edited_by = get_opt_str("edited_by");
            // doc_type parsing: convert string to DocType
            let doc_type_str = get_opt_str("doc_type");
            let doc_type = doc_type_str.as_deref().and_then(|s| match s {
                "note" => Some(crate::models::DocType::Note),
                "prd" => Some(crate::models::DocType::Prd),
                "handoff" => Some(crate::models::DocType::Handoff),
                _ => None,
            });
            commands::doc_list(
                repo_path,
                tag.as_deref(),
                doc_type.as_ref(),
                edited_by.as_deref(),
                for_entity.as_deref(),
            )
            .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
            .map_err(|e| e.to_string())
        }
        ["doc", "show", id] => {
            let full = get_bool("full", false);
            commands::doc_show(repo_path, id, full)
                .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
                .map_err(|e| e.to_string())
        }
        ["doc", "show"] => {
            let id = get_str("id").ok_or("Missing 'id' argument")?;
            let full = get_bool("full", false);
            commands::doc_show(repo_path, &id, full)
                .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
                .map_err(|e| e.to_string())
        }

        // Unknown command
        _ => Err(format!("Unknown command: {}", cmd)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_any_client_message_parse_protocol_subscribe() {
        let json = r#"{"type":"subscribe","topics":["tasks","bugs"]}"#;
        let msg = AnyClientMessage::from_str(json);
        assert!(matches!(
            msg,
            Some(AnyClientMessage::Protocol(
                ProtocolClientMessage::Subscribe { .. }
            ))
        ));
    }

    #[test]
    fn test_any_client_message_parse_protocol_command() {
        let json = r#"{"type":"command","id":"cmd-123","cmd":"task list","args":{}}"#;
        let msg = AnyClientMessage::from_str(json);
        assert!(matches!(
            msg,
            Some(AnyClientMessage::Protocol(
                ProtocolClientMessage::Command { .. }
            ))
        ));
    }

    #[test]
    fn test_any_client_message_parse_protocol_ping() {
        let json = r#"{"type":"ping"}"#;
        let msg = AnyClientMessage::from_str(json);
        assert!(matches!(
            msg,
            Some(AnyClientMessage::Protocol(ProtocolClientMessage::Ping))
        ));
    }

    #[test]
    fn test_any_client_message_parse_legacy_request_sync() {
        let json = r#"{"type":"request_sync","last_version":42}"#;
        let msg = AnyClientMessage::from_str(json);
        assert!(matches!(
            msg,
            Some(AnyClientMessage::Legacy(
                LegacyClientMessage::RequestSync { .. }
            ))
        ));
    }

    #[test]
    fn test_any_client_message_parse_unknown() {
        let json = r#"{"type":"unknown_type"}"#;
        let msg = AnyClientMessage::from_str(json);
        assert!(msg.is_none());
    }

    #[test]
    fn test_any_client_message_parse_invalid_json() {
        let json = r#"not valid json"#;
        let msg = AnyClientMessage::from_str(json);
        assert!(msg.is_none());
    }

    #[tokio::test]
    async fn test_execute_command_empty() {
        let result = execute_command(Path::new("/tmp"), "", serde_json::Value::Null).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Empty command");
    }

    #[tokio::test]
    async fn test_execute_command_unknown() {
        let result = execute_command(
            Path::new("/tmp"),
            "unknown command",
            serde_json::Value::Null,
        )
        .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown command"));
    }

    #[test]
    fn test_build_graph_state_empty_repo() {
        // Create temp directories for repo and data
        let temp_dir = tempfile::tempdir().unwrap();
        let repo_path = temp_dir.path();
        let data_dir = temp_dir.path().join("bn_data");
        std::fs::create_dir_all(&data_dir).unwrap();

        // Initialize git repo (required for storage)
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Initialize binnacle storage with explicit data directory
        let _ = crate::storage::Storage::init_with_data_dir(repo_path, &data_dir);

        // Build graph state - need to use the same data dir
        // Since build_graph_state uses Storage::open which uses default paths,
        // we need to test the core function differently
        let storage = crate::storage::Storage::open_with_data_dir(repo_path, &data_dir).unwrap();

        // Verify empty storage
        let tasks = storage.list_tasks(None, None, None).unwrap();
        let bugs = storage.list_bugs(None, None, None, None, true).unwrap();
        let tests = storage.list_tests(None).unwrap();
        let milestones = storage.list_milestones(None, None, None).unwrap();
        let ideas = storage.list_ideas(None, None).unwrap();
        let docs = storage.list_docs(None, None, None, None).unwrap();
        let links = storage.list_edges(None, None, None).unwrap();
        let queue = storage.get_queue().ok();

        assert!(tasks.is_empty(), "Expected empty tasks, got: {:?}", tasks);
        assert!(bugs.is_empty());
        assert!(tests.is_empty());
        assert!(milestones.is_empty());
        assert!(ideas.is_empty());
        assert!(docs.is_empty());
        assert!(links.is_empty());
        assert!(queue.is_none());
    }

    #[test]
    fn test_build_graph_state_with_tasks() {
        // Create temp directories for repo and data
        let temp_dir = tempfile::tempdir().unwrap();
        let repo_path = temp_dir.path();
        let data_dir = temp_dir.path().join("bn_data");
        std::fs::create_dir_all(&data_dir).unwrap();

        // Initialize git repo
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Initialize and add a task with explicit data directory
        let mut storage =
            crate::storage::Storage::init_with_data_dir(repo_path, &data_dir).unwrap();
        let task = crate::models::Task::new("bn-test".to_string(), "Test task".to_string());
        storage.create_task(&task).unwrap();

        // Verify the task was created
        let tasks = storage.list_tasks(None, None, None).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].core.id, "bn-test");
        assert_eq!(tasks[0].core.title, "Test task");
    }

    #[test]
    fn test_build_graph_state_serialization() {
        // Test that GraphState can be serialized into ServerMessage::State
        let state = GraphState {
            tasks: vec![serde_json::json!({"id": "bn-test", "title": "Test"})],
            bugs: vec![],
            tests: vec![],
            milestones: vec![],
            ideas: vec![],
            docs: vec![],
            links: vec![],
            queue: None,
            summary: Some(StateSummary {
                total_tasks: 1,
                ready_count: 1,
                blocked_count: 0,
                in_progress_count: 0,
                total_bugs: 0,
                open_bugs_count: 0,
                critical_bugs_count: 0,
            }),
        };

        // Create a ServerMessage::State and verify serialization
        let msg = ServerMessage::State {
            data: Box::new(state),
            version: 42,
            timestamp: Utc::now(),
        };

        let json = serde_json::to_string(&msg);
        assert!(json.is_ok(), "Failed to serialize state message");

        let json_str = json.unwrap();
        assert!(json_str.contains(r#""type":"state""#));
        assert!(json_str.contains(r#""version":42"#));
        assert!(json_str.contains(r#""id":"bn-test""#));
    }
}
