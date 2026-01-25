//! Web server for serving the GUI and API endpoints

use axum::{
    Json, Router,
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
};
use serde::Deserialize;
use std::net::{SocketAddr, TcpListener};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{Mutex, broadcast};

/// Default starting port for the GUI server
pub const DEFAULT_PORT: u16 = 3030;

/// Maximum number of ports to try when auto-selecting
const MAX_PORT_ATTEMPTS: u16 = 100;

/// Find an available port starting from the given base port.
/// Tries ports sequentially until one is available or max attempts reached.
pub fn find_available_port(host: &str, start_port: u16) -> Option<u16> {
    let host_addr: std::net::IpAddr = match host.parse() {
        Ok(addr) => addr,
        Err(_) => return None,
    };

    for offset in 0..MAX_PORT_ATTEMPTS {
        let port = start_port.saturating_add(offset);
        let addr = SocketAddr::from((host_addr, port));

        // Try to bind to check if port is available
        if TcpListener::bind(addr).is_ok() {
            return Some(port);
        }
    }
    None
}

use crate::models::{Edge, EdgeType, Queue, TaskStatus};
use crate::storage::{Storage, generate_id};

/// WebSocket performance metrics
#[derive(Default)]
pub struct WebSocketMetrics {
    /// Number of currently connected WebSocket clients
    pub connected_clients: AtomicU64,
    /// Total number of messages sent to clients
    pub messages_sent: AtomicU64,
    /// Total number of WebSocket connections ever made
    pub total_connections: AtomicU64,
}

impl WebSocketMetrics {
    /// Create new metrics instance
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a new connection
    pub fn connection_opened(&self) {
        self.connected_clients.fetch_add(1, Ordering::Relaxed);
        self.total_connections.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a closed connection
    pub fn connection_closed(&self) {
        self.connected_clients.fetch_sub(1, Ordering::Relaxed);
    }

    /// Record a message sent
    pub fn message_sent(&self) {
        self.messages_sent.fetch_add(1, Ordering::Relaxed);
    }

    /// Get current snapshot of metrics
    pub fn snapshot(&self) -> WebSocketMetricsSnapshot {
        WebSocketMetricsSnapshot {
            connected_clients: self.connected_clients.load(Ordering::Relaxed),
            messages_sent: self.messages_sent.load(Ordering::Relaxed),
            total_connections: self.total_connections.load(Ordering::Relaxed),
        }
    }
}

/// Snapshot of WebSocket metrics (for JSON serialization)
#[derive(serde::Serialize)]
pub struct WebSocketMetricsSnapshot {
    pub connected_clients: u64,
    pub messages_sent: u64,
    pub total_connections: u64,
}

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    /// Storage instance for reading binnacle data (wrapped in Mutex for thread safety)
    pub storage: Arc<Mutex<Storage>>,
    /// Broadcast channel for sending updates to WebSocket clients
    pub update_tx: broadcast::Sender<String>,
    /// Name of the project folder (for display in GUI title)
    pub project_name: String,
    /// WebSocket performance metrics
    pub ws_metrics: Arc<WebSocketMetrics>,
}

/// Start the GUI web server
pub async fn start_server(
    repo_path: &Path,
    port: u16,
    host: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let storage = Storage::open(repo_path)?;
    let storage_dir = crate::storage::get_storage_dir(repo_path)?;
    let (update_tx, _) = broadcast::channel(100);

    // Extract project name from repo path
    let project_name = repo_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Unknown")
        .to_string();

    let state = AppState {
        storage: Arc::new(Mutex::new(storage)),
        update_tx,
        project_name,
        ws_metrics: Arc::new(WebSocketMetrics::new()),
    };

    // Start file watcher in background
    let watcher_tx = state.update_tx.clone();
    let watcher_path = storage_dir.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::gui::watcher::watch_storage(watcher_path, watcher_tx).await {
            eprintln!("File watcher error: {}", e);
        }
    });

    let app = Router::new()
        .route("/", get(serve_index))
        .route("/api/config", get(get_config))
        .route("/api/tasks", get(get_tasks))
        .route("/api/bugs", get(get_bugs))
        .route("/api/ideas", get(get_ideas))
        .route("/api/ready", get(get_ready))
        .route("/api/available-work", get(get_available_work))
        .route("/api/tests", get(get_tests))
        .route("/api/docs", get(get_docs))
        .route("/api/docs/:id", get(get_doc))
        .route("/api/docs/:id/history", get(get_doc_history))
        .route("/api/queue", get(get_queue))
        .route("/api/queue/toggle", post(toggle_queue_membership))
        .route("/api/edges", get(get_edges))
        .route("/api/edges", post(add_edge))
        .route("/api/log", get(get_log))
        .route("/api/agents", get(get_agents))
        .route("/api/agents/:pid/kill", post(kill_agent))
        .route("/api/metrics/ws", get(get_ws_metrics))
        .route("/ws", get(crate::gui::websocket::ws_handler))
        .with_state(state);

    let host_addr: std::net::IpAddr = host
        .parse()
        .map_err(|e| format!("Invalid host address '{}': {}", host, e))?;
    let addr = SocketAddr::from((host_addr, port));
    println!("Starting binnacle GUI at http://{}", addr);
    println!("Press Ctrl+C to stop");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Serve the main HTML page
async fn serve_index() -> impl IntoResponse {
    Html(include_str!("index.html"))
}

/// Get configuration info (project name, etc.)
async fn get_config(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "project_name": state.project_name
    }))
}

/// Get all tasks
async fn get_tasks(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let storage = state.storage.lock().await;
    let tasks = storage
        .list_tasks(None, None, None)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({ "tasks": tasks })))
}

/// Get all bugs
async fn get_bugs(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let storage = state.storage.lock().await;
    let bugs = storage
        .list_bugs(None, None, None, None, true) // Include closed bugs for GUI
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({ "bugs": bugs })))
}

/// Get all ideas
async fn get_ideas(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let storage = state.storage.lock().await;
    let ideas = storage
        .list_ideas(None, None)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({ "ideas": ideas })))
}

/// Get ready tasks (no blockers)
async fn get_ready(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let storage = state.storage.lock().await;
    let tasks = storage
        .list_tasks(Some("pending"), None, None)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Filter for tasks with no dependencies
    let ready: Vec<_> = tasks
        .into_iter()
        .filter(|t| t.depends_on.is_empty())
        .collect();

    Ok(Json(serde_json::json!({ "tasks": ready })))
}

/// Get available work counts broken down by entity type
async fn get_available_work(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let storage = state.storage.lock().await;

    // Count ready tasks (pending with no blockers)
    let tasks = storage
        .list_tasks(Some("pending"), None, None)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let ready_task_count = tasks.iter().filter(|t| t.depends_on.is_empty()).count();

    // Count open bugs (not done, not cancelled)
    let bugs = storage
        .list_bugs(None, None, None, None, false) // false excludes done/cancelled
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let open_bug_count = bugs.len();

    // Count open ideas (seed or germinating status - not promoted or discarded)
    let ideas = storage
        .list_ideas(None, None)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let open_idea_count = ideas
        .iter()
        .filter(|i| {
            matches!(
                i.status,
                crate::models::IdeaStatus::Seed | crate::models::IdeaStatus::Germinating
            )
        })
        .count();

    let total = ready_task_count + open_bug_count + open_idea_count;

    Ok(Json(serde_json::json!({
        "total": total,
        "tasks": ready_task_count,
        "bugs": open_bug_count,
        "ideas": open_idea_count
    })))
}

/// Get all tests
async fn get_tests(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let storage = state.storage.lock().await;
    let tests = storage
        .list_tests(None)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({ "tests": tests })))
}

/// Get all docs
async fn get_docs(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let storage = state.storage.lock().await;
    let docs = storage
        .list_docs(None, None, None, None)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Transform docs to include extracted summary for display in list view
    let docs_with_summary: Vec<serde_json::Value> = docs
        .into_iter()
        .map(|doc| {
            let summary = doc.get_summary().unwrap_or_default();
            serde_json::json!({
                "id": doc.core.id,
                "title": doc.core.title,
                "short_name": doc.core.short_name,
                "description": doc.core.description,
                "tags": doc.core.tags,
                "doc_type": doc.doc_type,
                "summary": summary,
                "summary_dirty": doc.summary_dirty,
                "editors": doc.editors,
                "supersedes": doc.supersedes,
                "created_at": doc.core.created_at,
                "updated_at": doc.core.updated_at
            })
        })
        .collect();

    Ok(Json(serde_json::json!({ "docs": docs_with_summary })))
}

/// Get a single doc by ID with decompressed content
async fn get_doc(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let storage = state.storage.lock().await;
    let doc = storage.get_doc(&id).map_err(|_| StatusCode::NOT_FOUND)?;

    // Decompress the content for the response
    let content = doc
        .get_content()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get the summary section separately
    let summary = doc.get_summary().unwrap_or_default();

    Ok(Json(serde_json::json!({
        "doc": {
            "id": doc.core.id,
            "title": doc.core.title,
            "short_name": doc.core.short_name,
            "description": doc.core.description,
            "tags": doc.core.tags,
            "doc_type": doc.doc_type,
            "content": content,
            "summary": summary,
            "summary_dirty": doc.summary_dirty,
            "editors": doc.editors,
            "supersedes": doc.supersedes,
            "created_at": doc.core.created_at,
            "updated_at": doc.core.updated_at
        }
    })))
}

/// Get version history for a doc
async fn get_doc_history(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let storage = state.storage.lock().await;

    // Start from the given ID and collect all versions
    let mut versions = Vec::new();
    let mut current_id = id.clone();
    let mut seen_ids = std::collections::HashSet::new();

    // Walk backwards through the supersedes chain to find all previous versions
    loop {
        if seen_ids.contains(&current_id) {
            // Prevent infinite loops from circular references
            break;
        }
        seen_ids.insert(current_id.clone());

        let doc = match storage.get_doc(&current_id) {
            Ok(d) => d,
            Err(_) => break,
        };

        versions.push(serde_json::json!({
            "id": doc.core.id,
            "title": doc.core.title,
            "editors": doc.editors,
            "created_at": doc.core.created_at,
            "is_current": versions.is_empty()
        }));

        if let Some(prev_id) = doc.supersedes {
            current_id = prev_id;
        } else {
            break;
        }
    }

    Ok(Json(serde_json::json!({
        "current_id": id,
        "versions": versions
    })))
}

/// Get the queue (if it exists)
async fn get_queue(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let storage = state.storage.lock().await;
    match storage.get_queue() {
        Ok(queue) => Ok(Json(serde_json::json!({ "queue": queue }))),
        Err(_) => Ok(Json(serde_json::json!({ "queue": null }))),
    }
}

/// Get all agents
async fn get_agents(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut storage = state.storage.lock().await;
    // Update agent statuses (active/idle/stale) and clean up stale agents before returning
    let _ = storage.update_agent_statuses();
    let _ = storage.cleanup_stale_agents();
    let agents = storage
        .list_agents(None)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({ "agents": agents })))
}

/// Kill an agent by PID
async fn kill_agent(
    State(state): State<AppState>,
    AxumPath(pid): AxumPath<u32>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut storage = state.storage.lock().await;

    // Verify the agent exists
    let agent = storage.get_agent(pid).map_err(|_| StatusCode::NOT_FOUND)?;

    // Send SIGTERM to the process
    #[cfg(unix)]
    {
        use std::process::Command;
        let _ = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
    }

    // Remove the agent from the registry
    let _ = storage.remove_agent(pid);

    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("Terminated agent {} (PID: {})", agent.name, pid)
    })))
}

/// Get all edges
async fn get_edges(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let storage = state.storage.lock().await;
    let edges = storage
        .list_edges(None, None, None)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Transform edges to include bidirectional flag for GUI rendering
    let edges_with_meta: Vec<serde_json::Value> = edges
        .iter()
        .map(|e| {
            serde_json::json!({
                "id": e.id,
                "source": e.source,
                "target": e.target,
                "edge_type": e.edge_type,
                "weight": e.weight,
                "reason": e.reason,
                "bidirectional": e.is_bidirectional(),
                "created_at": e.created_at
            })
        })
        .collect();

    Ok(Json(serde_json::json!({ "edges": edges_with_meta })))
}

/// Query parameters for log pagination endpoint
#[derive(Debug, Deserialize)]
struct LogQueryParams {
    /// Maximum entries to return (default: 100, max: 1000)
    limit: Option<u32>,
    /// Offset for pagination
    offset: Option<u32>,
    /// Only return entries before this ISO 8601 timestamp
    before: Option<String>,
    /// Only return entries after this ISO 8601 timestamp (for live streaming)
    after: Option<String>,
    /// Filter by command name (partial match)
    command: Option<String>,
    /// Filter by user (exact match)
    user: Option<String>,
    /// Filter by success status (true/false)
    success: Option<bool>,
}

/// Get activity log with pagination and filtering support.
///
/// Query parameters:
/// - `limit`: Maximum entries to return (default: 100, max: 1000)
/// - `offset`: Number of entries to skip
/// - `before`: Only return entries before this ISO 8601 timestamp
/// - `after`: Only return entries after this ISO 8601 timestamp (for live updates)
/// - `command`: Filter by command name (partial match)
/// - `user`: Filter by user (exact match)
/// - `success`: Filter by success status (true/false)
async fn get_log(
    State(state): State<AppState>,
    Query(params): Query<LogQueryParams>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let storage = state.storage.lock().await;

    // Try to query from SQLite cache first (fast, supports pagination)
    let logs_result = storage.query_action_logs(
        params.limit,
        params.offset,
        params.before.as_deref(),
        params.after.as_deref(),
        params.command.as_deref(),
        params.user.as_deref(),
        params.success,
    );

    if let Ok(logs) = logs_result {
        // Get total count for pagination info
        let total = storage
            .count_action_logs(
                params.before.as_deref(),
                params.after.as_deref(),
                params.command.as_deref(),
                params.user.as_deref(),
                params.success,
            )
            .unwrap_or(0);

        let entries: Vec<serde_json::Value> = logs
            .into_iter()
            .map(|log| {
                serde_json::json!({
                    "timestamp": log.timestamp.to_rfc3339(),
                    "repo_path": log.repo_path,
                    "command": log.command,
                    "args": log.args,
                    "success": log.success,
                    "error": log.error,
                    "duration_ms": log.duration_ms,
                    "user": log.user,
                })
            })
            .collect();

        return Ok(Json(serde_json::json!({
            "log": entries,
            "total": total,
            "limit": params.limit.unwrap_or(100).min(1000),
            "offset": params.offset.unwrap_or(0),
        })));
    }

    // Fallback: read from JSONL file directly (for backward compatibility)
    let log_path = storage.root.join("../action.log");
    let log_entries = if log_path.exists() {
        std::fs::read_to_string(&log_path)
            .map(|content| {
                let mut entries: Vec<_> = content
                    .lines()
                    .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
                    .collect();

                // Apply filters manually for fallback path
                if let Some(ref cmd_filter) = params.command {
                    entries.retain(|e| {
                        e.get("command")
                            .and_then(|c| c.as_str())
                            .map(|c| c.contains(cmd_filter))
                            .unwrap_or(false)
                    });
                }

                if let Some(ref user_filter) = params.user {
                    entries.retain(|e| {
                        e.get("user")
                            .and_then(|u| u.as_str())
                            .map(|u| u == user_filter)
                            .unwrap_or(false)
                    });
                }

                if let Some(success_filter) = params.success {
                    entries.retain(|e| {
                        e.get("success")
                            .and_then(|s| s.as_bool())
                            .map(|s| s == success_filter)
                            .unwrap_or(false)
                    });
                }

                // Sort by timestamp descending (newest first)
                entries.reverse();

                // Apply pagination
                let total = entries.len();
                let offset = params.offset.unwrap_or(0) as usize;
                let limit = params.limit.unwrap_or(100).min(1000) as usize;
                let paginated: Vec<_> = entries.into_iter().skip(offset).take(limit).collect();

                (paginated, total)
            })
            .unwrap_or_else(|_| (vec![], 0))
    } else {
        (vec![], 0)
    };

    Ok(Json(serde_json::json!({
        "log": log_entries.0,
        "total": log_entries.1,
        "limit": params.limit.unwrap_or(100).min(1000),
        "offset": params.offset.unwrap_or(0),
    })))
}

/// Request body for adding an edge
#[derive(Deserialize)]
struct AddEdgeRequest {
    source: String,
    target: String,
    edge_type: String,
}

/// Add a new edge (link) between nodes
async fn add_edge(
    State(state): State<AppState>,
    Json(request): Json<AddEdgeRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let mut storage = state.storage.lock().await;

    // Parse edge type
    let edge_type: EdgeType = request.edge_type.parse().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!("Invalid edge type: {}", request.edge_type)
            })),
        )
    })?;

    // Generate ID and create edge
    let id = storage.generate_edge_id(&request.source, &request.target, edge_type);
    let edge = Edge::new(
        id.clone(),
        request.source.clone(),
        request.target.clone(),
        edge_type,
    );

    // Add edge to storage
    storage.add_edge(&edge).map_err(|e| {
        (
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error": e.to_string()
            })),
        )
    })?;

    Ok(Json(serde_json::json!({
        "success": true,
        "edge": {
            "id": id,
            "source": request.source,
            "target": request.target,
            "edge_type": request.edge_type
        }
    })))
}

/// Request body for toggling queue membership
#[derive(Deserialize)]
struct ToggleQueueRequest {
    node_id: String,
}

/// Toggle a node's membership in the queue
async fn toggle_queue_membership(
    State(state): State<AppState>,
    Json(request): Json<ToggleQueueRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let mut storage = state.storage.lock().await;

    // Get or create the queue
    let queue = match storage.get_queue() {
        Ok(q) => q,
        Err(_) => {
            // Create default queue if it doesn't exist
            let title = "Work Queue".to_string();
            let queue_id = generate_id("bnq", &title);
            let new_queue = Queue::new(queue_id.clone(), title);
            storage.create_queue(&new_queue).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
            })?;
            new_queue
        }
    };

    // Check if the node is already queued
    let edges = storage
        .list_edges(
            Some(EdgeType::Queued),
            Some(&request.node_id),
            Some(&queue.id),
        )
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        })?;

    let is_queued = !edges.is_empty();

    if is_queued {
        // Remove from queue
        storage
            .remove_edge(&request.node_id, &queue.id, EdgeType::Queued)
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
            })?;

        Ok(Json(serde_json::json!({
            "success": true,
            "queued": false,
            "message": format!("{} removed from queue", request.node_id)
        })))
    } else {
        // Check if the item is closed before adding to queue
        let is_closed = if let Ok(task) = storage.get_task(&request.node_id) {
            task.status == TaskStatus::Done || task.status == TaskStatus::Cancelled
        } else if let Ok(bug) = storage.get_bug(&request.node_id) {
            bug.status == TaskStatus::Done || bug.status == TaskStatus::Cancelled
        } else {
            false
        };

        if is_closed {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("Cannot add {} to queue: item is closed", request.node_id)
                })),
            ));
        }

        // Add to queue
        let edge_id = storage.generate_edge_id(&request.node_id, &queue.id, EdgeType::Queued);
        let edge = Edge::new(
            edge_id,
            request.node_id.clone(),
            queue.id.clone(),
            EdgeType::Queued,
        );

        storage.add_edge(&edge).map_err(|e| {
            (
                StatusCode::CONFLICT,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        })?;

        Ok(Json(serde_json::json!({
            "success": true,
            "queued": true,
            "message": format!("{} added to queue", request.node_id)
        })))
    }
}

/// Get WebSocket performance metrics
async fn get_ws_metrics(State(state): State<AppState>) -> Json<serde_json::Value> {
    let metrics = state.ws_metrics.snapshot();
    Json(serde_json::json!({ "websocket": metrics }))
}
