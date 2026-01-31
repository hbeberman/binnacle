//! Web server for serving the GUI and API endpoints

use axum::{
    Json, Router,
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::Deserialize;
use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{Mutex, broadcast};
use tower_http::services::ServeDir;

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

use crate::models::{Edge, EdgeType, LogAnnotation, Queue, TaskStatus};
use crate::storage::{Storage, generate_id};
use chrono::Utc;

/// Error response for readonly mode rejection
fn readonly_error() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::FORBIDDEN,
        Json(serde_json::json!({
            "error": "Server is in readonly mode - write operations are disabled"
        })),
    )
}

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

/// Monotonic version counter for state synchronization
///
/// This counter increments on any state change and is included in all WebSocket
/// messages. Clients can use this to detect missed updates and request resync.
#[derive(Clone, Default)]
pub struct StateVersion {
    counter: Arc<AtomicU64>,
}

impl StateVersion {
    /// Create a new version counter starting at 0
    pub fn new() -> Self {
        Self {
            counter: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Increment the version and return the new value
    pub fn increment(&self) -> u64 {
        self.counter.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Get the current version without incrementing
    pub fn current(&self) -> u64 {
        self.counter.load(Ordering::SeqCst)
    }
}

/// Maximum number of messages to keep in history for catch-up
const MESSAGE_HISTORY_SIZE: usize = 100;

/// Stored message with its version number
#[derive(Clone)]
pub struct VersionedMessage {
    /// Version number when this message was created
    pub version: u64,
    /// The serialized message JSON
    pub message: String,
}

/// Circular buffer for storing recent incremental messages
///
/// This enables clients to catch up on missed messages without reloading
/// the entire graph state.
#[derive(Clone)]
pub struct MessageHistory {
    /// Internal buffer wrapped for thread-safe access
    buffer: Arc<Mutex<MessageHistoryInner>>,
}

struct MessageHistoryInner {
    /// Circular buffer of recent messages
    messages: Vec<VersionedMessage>,
    /// Oldest version number we can serve (start of buffer)
    oldest_version: u64,
    /// Maximum buffer size
    capacity: usize,
}

impl MessageHistory {
    /// Create a new message history with the given capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: Arc::new(Mutex::new(MessageHistoryInner {
                messages: Vec::with_capacity(capacity),
                oldest_version: 1,
                capacity,
            })),
        }
    }

    /// Create a new message history with default capacity
    pub fn default() -> Self {
        Self::new(MESSAGE_HISTORY_SIZE)
    }

    /// Add a message to the history
    pub async fn push(&self, version: u64, message: String) {
        let mut inner = self.buffer.lock().await;

        // Add message to buffer
        inner.messages.push(VersionedMessage { version, message });

        // If we exceed capacity, remove oldest messages
        if inner.messages.len() > inner.capacity {
            let excess = inner.messages.len() - inner.capacity;
            inner.messages.drain(0..excess);

            // Update oldest_version to reflect what's available
            if let Some(first) = inner.messages.first() {
                inner.oldest_version = first.version;
            }
        }
    }

    /// Clear the message history (e.g., when a full reload is sent)
    pub async fn clear(&self) {
        let mut inner = self.buffer.lock().await;
        inner.messages.clear();
        // Keep track of where we are, even though buffer is empty
        // This will be updated when new messages arrive
    }

    /// Get messages since the given version (inclusive)
    ///
    /// Returns None if the requested version is too old (not in history)
    /// Returns Some(vec) with the messages if available
    pub async fn get_since(&self, since_version: u64) -> Option<Vec<VersionedMessage>> {
        let inner = self.buffer.lock().await;

        // Check if we can serve this request
        if inner.messages.is_empty() {
            return Some(Vec::new());
        }

        // If requested version is older than our oldest, we can't help
        if since_version < inner.oldest_version {
            return None;
        }

        // Find messages with version > since_version
        let messages: Vec<VersionedMessage> = inner
            .messages
            .iter()
            .filter(|m| m.version > since_version)
            .cloned()
            .collect();

        Some(messages)
    }
}

/// Summarize agent session state
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SummarizeSession {
    /// Agent ID for this session
    pub agent_id: String,
    /// When the session was started
    pub started_at: chrono::DateTime<Utc>,
    /// Chat history (messages exchanged with the agent)
    pub messages: Vec<SummarizeMessage>,
}

/// A message in the summarize chat session
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SummarizeMessage {
    /// Role: "user" or "assistant"
    pub role: String,
    /// Message content
    pub content: String,
    /// When the message was sent
    pub timestamp: chrono::DateTime<Utc>,
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
    /// Repository path for git operations
    pub repo_path: PathBuf,
    /// Whether the server is in readonly mode (write operations disabled)
    pub readonly: bool,
    /// Monotonic version counter for state synchronization
    pub version: StateVersion,
    /// Message history for catch-up synchronization
    pub message_history: MessageHistory,
    /// Summarize agent session (max 1 concurrent)
    pub summarize_session: Arc<Mutex<Option<SummarizeSession>>>,
}

/// Start the GUI web server
pub async fn start_server(
    repo_path: &Path,
    port: u16,
    host: &str,
    readonly: bool,
    dev: bool,
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

    let version = StateVersion::new();
    let message_history = MessageHistory::default();

    let state = AppState {
        storage: Arc::new(Mutex::new(storage)),
        update_tx,
        project_name,
        ws_metrics: Arc::new(WebSocketMetrics::new()),
        repo_path: repo_path.to_path_buf(),
        readonly,
        version,
        message_history: message_history.clone(),
        summarize_session: Arc::new(Mutex::new(None)),
    };

    // Start file watcher in background
    let watcher_tx = state.update_tx.clone();
    let watcher_version = state.version.clone();
    let watcher_message_history = message_history;
    let watcher_storage_path = storage_dir.clone();
    let watcher_repo_path = repo_path.to_path_buf();
    tokio::spawn(async move {
        if let Err(e) = crate::gui::watcher::watch_storage(
            watcher_storage_path,
            watcher_repo_path,
            watcher_tx,
            watcher_version,
            watcher_message_history,
        )
        .await
        {
            eprintln!("File watcher error: {}", e);
        }
    });

    // Build the app router with API routes
    let mut app = Router::new()
        .route("/api/config", get(get_config))
        .route("/api/tasks", get(get_tasks))
        .route("/api/bugs", get(get_bugs))
        .route("/api/issues", get(get_issues))
        .route("/api/ideas", get(get_ideas))
        .route("/api/milestones", get(get_milestones))
        .route("/api/ready", get(get_ready))
        .route("/api/available-work", get(get_available_work))
        .route("/api/tests", get(get_tests))
        .route("/api/docs", get(get_docs))
        .route("/api/docs/:id", get(get_doc))
        .route("/api/docs/:id/history", get(get_doc_history))
        .route("/api/node/:id", get(get_node))
        .route("/api/queue", get(get_queue))
        .route("/api/queue/toggle", post(toggle_queue_membership))
        .route("/api/batch/close", post(batch_close))
        .route("/api/batch/queue-add", post(batch_queue_add))
        .route("/api/batch/queue-remove", post(batch_queue_remove))
        .route("/api/edges", get(get_edges))
        .route("/api/edges", post(add_edge))
        .route("/api/links/batch", post(batch_add_links))
        .route("/api/log", get(get_log))
        .route("/api/log/owners", get(get_log_owners))
        .route("/api/changes", get(get_changes))
        .route("/api/log/annotations", get(get_log_annotations))
        .route("/api/log/annotations", post(add_log_annotation))
        .route("/api/log/annotations/:id", get(get_log_annotation_by_id))
        .route(
            "/api/log/annotations/:id",
            axum::routing::delete(delete_log_annotation),
        )
        .route("/api/agents", get(get_agents))
        .route("/api/agents/:pid/kill", post(kill_agent))
        .route("/api/agents/:id/terminate", post(terminate_agent))
        .route("/api/commits", get(get_git_commits))
        .route("/api/metrics/ws", get(get_ws_metrics))
        .route("/api/version", get(get_version))
        .route("/api/summarize/start", post(summarize_start))
        .route("/api/summarize/chat", post(summarize_chat))
        .route("/api/summarize/action", post(summarize_action))
        .route("/ws", get(crate::gui::websocket::ws_handler))
        .with_state(state);

    // Add asset service based on dev mode
    if dev {
        // Development mode: serve from filesystem
        let (web_dir, project_root) = {
            let mut current = repo_path;
            loop {
                let web_path = current.join("web");
                if web_path.exists() && web_path.is_dir() {
                    break Some((web_path, current.to_path_buf()));
                }
                current = match current.parent() {
                    Some(p) => p,
                    None => break None,
                };
            }
        }
        .or_else(|| {
            // Fallback: try relative to current directory
            let cwd = std::env::current_dir().ok()?;
            let web_path = cwd.join("web");
            if web_path.exists() && web_path.is_dir() {
                Some((web_path, cwd))
            } else {
                None
            }
        })
        .ok_or("Could not find web/ directory (use --dev only during development)")?;

        // Serve node_modules for import map resolution (dev mode only)
        let node_modules_dir = project_root.join("node_modules");
        if node_modules_dir.exists() {
            app = app.nest_service("/node_modules", ServeDir::new(&node_modules_dir));
        }

        app = app.fallback_service(ServeDir::new(&web_dir));
    } else {
        // Production mode: serve embedded assets
        use crate::gui::embedded::EmbeddedAssetService;
        app = app.fallback_service(EmbeddedAssetService::new());
    }

    let host_addr: std::net::IpAddr = host
        .parse()
        .map_err(|e| format!("Invalid host address '{}': {}", host, e))?;
    let addr = SocketAddr::from((host_addr, port));
    if readonly {
        println!("Starting binnacle GUI at http://{} (READONLY MODE)", addr);
    } else {
        println!("Starting binnacle GUI at http://{}", addr);
    }
    println!("Press Ctrl+C to stop");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Get configuration info (project name, readonly mode, etc.)
async fn get_config(State(state): State<AppState>) -> Json<serde_json::Value> {
    // Get current git branch
    let branch = std::process::Command::new("git")
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .current_dir(&state.repo_path)
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        });

    // Get repo name from path
    let repo_name = state
        .repo_path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string());

    Json(serde_json::json!({
        "project_name": state.project_name,
        "readonly": state.readonly,
        "repo_name": repo_name,
        "branch": branch
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

/// Get all issues
async fn get_issues(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let storage = state.storage.lock().await;
    let issues = storage
        .list_issues(None, None, None, true) // Include closed issues for GUI
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({ "issues": issues })))
}

/// Get all ideas
async fn get_ideas(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let storage = state.storage.lock().await;
    let ideas = storage
        .list_ideas(None, None)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({ "ideas": ideas })))
}

/// Get all milestones
async fn get_milestones(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let storage = state.storage.lock().await;
    let milestones = storage
        .list_milestones(None, None, None)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({ "milestones": milestones })))
}

/// Get ready tasks (no blockers)
async fn get_ready(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let storage = state.storage.lock().await;

    // Use the same logic as CLI `bn ready` - checks both legacy depends_on and edge-based dependencies
    let ready = storage
        .get_ready_tasks()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Include recently completed items for TUI display
    let recently_completed_tasks = storage.get_recently_completed_tasks().unwrap_or_default();
    let recently_completed_bugs = storage.get_recently_completed_bugs().unwrap_or_default();

    Ok(Json(serde_json::json!({
        "tasks": ready,
        "recently_completed_tasks": recently_completed_tasks,
        "recently_completed_bugs": recently_completed_bugs
    })))
}

/// Get available work counts broken down by entity type
async fn get_available_work(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let storage = state.storage.lock().await;

    // Count ready tasks using the same logic as CLI `bn ready`
    let ready_tasks = storage
        .get_ready_tasks()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let ready_task_count = ready_tasks.len();

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

    // Only count ready tasks and open bugs as available work
    // Ideas are speculative and should not count
    let total = ready_task_count + open_bug_count;

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

    // Check if this doc has been superseded by another doc
    let superseded_by = storage.get_doc_superseded_by(&id).unwrap_or(None);

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
            "superseded_by": superseded_by,
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

/// Get any node by ID with its edges
/// This endpoint returns the node data regardless of type (task, bug, idea, etc.)
/// along with its edges for navigation purposes
async fn get_node(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let storage = state.storage.lock().await;

    // Determine the entity type
    let entity_type = storage
        .get_entity_type(&id)
        .map_err(|_| StatusCode::NOT_FOUND)?;

    // Get the node data based on type
    let node_data: serde_json::Value = match entity_type {
        crate::storage::EntityType::Task => {
            let task = storage.get_task(&id).map_err(|_| StatusCode::NOT_FOUND)?;
            serde_json::json!({
                "id": task.core.id,
                "type": "task",
                "title": task.core.title,
                "short_name": task.core.short_name,
                "description": task.core.description,
                "status": task.status,
                "priority": task.priority,
                "assignee": task.assignee,
                "tags": task.core.tags,
                "created_at": task.core.created_at,
                "updated_at": task.core.updated_at,
                "closed_at": task.closed_at,
                "closed_reason": task.closed_reason
            })
        }
        crate::storage::EntityType::Bug => {
            let bug = storage.get_bug(&id).map_err(|_| StatusCode::NOT_FOUND)?;
            serde_json::json!({
                "id": bug.core.id,
                "type": "bug",
                "title": bug.core.title,
                "short_name": bug.core.short_name,
                "description": bug.core.description,
                "status": bug.status,
                "priority": bug.priority,
                "severity": bug.severity,
                "assignee": bug.assignee,
                "tags": bug.core.tags,
                "created_at": bug.core.created_at,
                "updated_at": bug.core.updated_at,
                "closed_at": bug.closed_at,
                "closed_reason": bug.closed_reason
            })
        }
        crate::storage::EntityType::Idea => {
            let idea = storage.get_idea(&id).map_err(|_| StatusCode::NOT_FOUND)?;
            serde_json::json!({
                "id": idea.core.id,
                "type": "idea",
                "title": idea.core.title,
                "short_name": idea.core.short_name,
                "description": idea.core.description,
                "status": idea.status,
                "tags": idea.core.tags,
                "created_at": idea.core.created_at,
                "updated_at": idea.core.updated_at
            })
        }
        crate::storage::EntityType::Milestone => {
            let ms = storage
                .get_milestone(&id)
                .map_err(|_| StatusCode::NOT_FOUND)?;
            serde_json::json!({
                "id": ms.core.id,
                "type": "milestone",
                "title": ms.core.title,
                "short_name": ms.core.short_name,
                "description": ms.core.description,
                "status": ms.status,
                "due_date": ms.due_date,
                "tags": ms.core.tags,
                "created_at": ms.core.created_at,
                "updated_at": ms.core.updated_at,
                "closed_at": ms.closed_at,
                "closed_reason": ms.closed_reason
            })
        }
        crate::storage::EntityType::Doc => {
            let doc = storage.get_doc(&id).map_err(|_| StatusCode::NOT_FOUND)?;
            let summary = doc.get_summary().unwrap_or_default();
            serde_json::json!({
                "id": doc.core.id,
                "type": "doc",
                "title": doc.core.title,
                "short_name": doc.core.short_name,
                "description": doc.core.description,
                "doc_type": doc.doc_type,
                "summary": summary,
                "tags": doc.core.tags,
                "created_at": doc.core.created_at,
                "updated_at": doc.core.updated_at
            })
        }
        crate::storage::EntityType::Test => {
            let test = storage.get_test(&id).map_err(|_| StatusCode::NOT_FOUND)?;
            serde_json::json!({
                "id": test.id,
                "type": "test",
                "name": test.name,
                "command": test.command,
                "working_dir": test.working_dir,
                "pattern": test.pattern,
                "created_at": test.created_at
            })
        }
        crate::storage::EntityType::Issue => {
            let issue = storage.get_issue(&id).map_err(|_| StatusCode::NOT_FOUND)?;
            serde_json::json!({
                "id": issue.core.id,
                "type": "issue",
                "title": issue.core.title,
                "short_name": issue.core.short_name,
                "description": issue.core.description,
                "status": issue.status,
                "priority": issue.priority,
                "assignee": issue.assignee,
                "tags": issue.core.tags,
                "created_at": issue.core.created_at,
                "updated_at": issue.core.updated_at,
                "closed_at": issue.closed_at,
                "closed_reason": issue.closed_reason
            })
        }
        crate::storage::EntityType::Queue => {
            let queue = storage
                .get_queue_by_id(&id)
                .map_err(|_| StatusCode::NOT_FOUND)?;
            serde_json::json!({
                "id": queue.id,
                "type": "queue",
                "title": queue.title,
                "description": queue.description,
                "created_at": queue.created_at
            })
        }
        crate::storage::EntityType::Agent => {
            let agent = storage
                .get_agent_by_id(&id)
                .map_err(|_| StatusCode::NOT_FOUND)?;
            serde_json::json!({
                "id": agent.id,
                "type": "agent",
                "pid": agent.pid,
                "agent_type": agent.agent_type,
                "status": agent.status,
                "started_at": agent.started_at,
                "last_activity_at": agent.last_activity_at
            })
        }
        crate::storage::EntityType::Edge => {
            // Edges are not nodes, return error
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    // Get edges for this node
    let edges = storage.get_edges_for_entity(&id).unwrap_or_default();

    // Enrich edges with related node titles
    let enriched_edges: Vec<serde_json::Value> = edges
        .iter()
        .map(|he| {
            let (related_id, direction_str) = match he.direction {
                crate::models::EdgeDirection::Outbound => (he.edge.target.clone(), "outbound"),
                crate::models::EdgeDirection::Inbound => (he.edge.source.clone(), "inbound"),
                crate::models::EdgeDirection::Both => (he.edge.target.clone(), "both"),
            };

            // Try to get the related node's title
            let related_title = get_entity_title(&storage, &related_id);

            serde_json::json!({
                "edge_type": he.edge.edge_type,
                "direction": direction_str,
                "related_id": related_id,
                "related_title": related_title
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "node": node_data,
        "edges": enriched_edges
    })))
}

/// Helper function to get the title of any entity by ID
fn get_entity_title(storage: &Storage, id: &str) -> Option<String> {
    // Try each entity type
    if let Ok(task) = storage.get_task(id) {
        return Some(task.core.title);
    }
    if let Ok(bug) = storage.get_bug(id) {
        return Some(bug.core.title);
    }
    if let Ok(idea) = storage.get_idea(id) {
        return Some(idea.core.title);
    }
    if let Ok(ms) = storage.get_milestone(id) {
        return Some(ms.core.title);
    }
    if let Ok(doc) = storage.get_doc(id) {
        return Some(doc.core.title);
    }
    if let Ok(test) = storage.get_test(id) {
        return Some(test.name);
    }
    if let Ok(issue) = storage.get_issue(id) {
        return Some(issue.core.title);
    }
    if let Ok(queue) = storage.get_queue_by_id(id) {
        return Some(queue.title);
    }
    None
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

    // Add health status to each agent
    let agents_with_health: Vec<serde_json::Value> = agents
        .into_iter()
        .map(|agent| {
            let health = agent.compute_health();
            let mut agent_json = serde_json::to_value(&agent).unwrap_or_default();
            if let Some(obj) = agent_json.as_object_mut() {
                obj.insert(
                    "health".to_string(),
                    serde_json::to_value(&health).unwrap_or_default(),
                );
            }
            agent_json
        })
        .collect();

    Ok(Json(serde_json::json!({ "agents": agents_with_health })))
}

/// Kill an agent by PID
async fn kill_agent(
    State(state): State<AppState>,
    AxumPath(pid): AxumPath<u32>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Check readonly mode
    if state.readonly {
        return Err(readonly_error());
    }

    let mut storage = state.storage.lock().await;

    // Verify the agent exists
    let agent = storage.get_agent(pid).map_err(|_| {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Agent not found"})),
        )
    })?;

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

/// Terminate an agent by ID
async fn terminate_agent(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Check readonly mode
    if state.readonly {
        return Err(readonly_error());
    }

    let mut storage = state.storage.lock().await;

    // Look up agent by ID
    let agent = storage.get_agent_by_id(&id).map_err(|_| {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Agent not found"})),
        )
    })?;

    let pid = agent.pid;
    let agent_name = agent.name.clone();
    let container_id = agent.container_id.clone();

    // Terminate the agent (container or process)
    if let Some(cid) = container_id {
        // Containerized agent: use containerd to stop the container
        #[cfg(unix)]
        {
            use std::process::Command;
            let result = Command::new("ctr")
                .args(["task", "kill", "--signal", "SIGTERM", &cid])
                .status();

            if let Err(e) = result {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "error": format!("Failed to terminate container {}: {}", cid, e)
                    })),
                ));
            }
        }
    } else {
        // Regular process: send SIGTERM
        #[cfg(unix)]
        {
            use std::process::Command;
            let _ = Command::new("kill")
                .args(["-TERM", &pid.to_string()])
                .status();
        }
    }

    // Remove the agent from the registry
    let _ = storage.remove_agent(pid);

    // Broadcast update to WebSocket clients
    let version = state.version.increment();
    let timestamp = chrono::Utc::now().to_rfc3339();
    let update_msg = serde_json::json!({
        "type": "entity_removed",
        "entity_type": "agent",
        "id": id,
        "version": version,
        "timestamp": timestamp
    })
    .to_string();
    let _ = state.update_tx.send(update_msg.clone());
    state.message_history.push(version, update_msg).await;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("Terminated agent {} (PID: {})", agent_name, pid)
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

// =============================================================================
// Entity Changes Endpoint
// =============================================================================

/// Query parameters for entity changes endpoint.
#[derive(Debug, Deserialize)]
struct ChangesQueryParams {
    /// Maximum entries to return (default: 100, max: 1000)
    limit: Option<u32>,
    /// Offset for pagination
    offset: Option<u32>,
    /// Only return entries before this timestamp (ISO 8601 or formatted string)
    before: Option<String>,
    /// Only return entries after this timestamp (ISO 8601 or formatted string)
    after: Option<String>,
    /// Filter by entity type (task, test, commit, etc.)
    entity_type: Option<String>,
    /// Filter by specific entity ID
    entity_id: Option<String>,
    /// Filter by actor (username or agent ID)
    actor: Option<String>,
    /// Filter by actor type (user or agent)
    actor_type: Option<String>,
}

/// Get entity change log with pagination and filtering support.
///
/// Query parameters:
/// - `limit`: Maximum entries to return (default: 100, max: 1000)
/// - `offset`: Number of entries to skip
/// - `before`: Only return entries before this timestamp
/// - `after`: Only return entries after this timestamp
/// - `entity_type`: Filter by entity type (task, test, commit, etc.)
/// - `entity_id`: Filter by specific entity ID
/// - `actor`: Filter by actor (username or agent ID)
/// - `actor_type`: Filter by actor type (user or agent)
async fn get_changes(
    State(state): State<AppState>,
    Query(params): Query<ChangesQueryParams>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let storage = state.storage.lock().await;

    // Build filter struct
    use crate::storage::LogEntryFilters;
    let filters = LogEntryFilters {
        limit: params.limit.map(|l| l as usize),
        offset: params.offset.map(|o| o as usize),
        before: params.before.as_deref(),
        after: params.after.as_deref(),
        entity_type: params.entity_type.as_deref(),
        entity_id: params.entity_id.as_deref(),
        actor: params.actor.as_deref(),
        actor_type: params.actor_type.as_deref(),
    };

    // Query entity changes from storage
    let entries_result = storage.query_log_entries(filters);

    if let Ok(entries) = entries_result {
        // Build filter struct for count (need to recreate since filters was moved)
        let count_filters = LogEntryFilters {
            limit: None, // Don't need limit/offset for count
            offset: None,
            before: params.before.as_deref(),
            after: params.after.as_deref(),
            entity_type: params.entity_type.as_deref(),
            entity_id: params.entity_id.as_deref(),
            actor: params.actor.as_deref(),
            actor_type: params.actor_type.as_deref(),
        };

        // Get total count for pagination info
        let total = storage.count_log_entries(count_filters).unwrap_or(0);

        let changes: Vec<serde_json::Value> = entries
            .into_iter()
            .map(|entry| {
                serde_json::json!({
                    "timestamp": entry.timestamp,
                    "entity_type": entry.entity_type,
                    "entity_id": entry.entity_id,
                    "action": entry.action,
                    "details": entry.details,
                    "actor": entry.actor,
                    "actor_type": entry.actor_type,
                })
            })
            .collect();

        return Ok(Json(serde_json::json!({
            "changes": changes,
            "total": total,
            "limit": params.limit.unwrap_or(100).min(1000),
            "offset": params.offset.unwrap_or(0),
        })));
    }

    // Fallback: return empty result
    Ok(Json(serde_json::json!({
        "changes": [],
        "total": 0,
        "limit": params.limit.unwrap_or(100).min(1000),
        "offset": params.offset.unwrap_or(0),
    })))
}

// =============================================================================
// Log Annotation Endpoints
// =============================================================================

/// Query parameters for listing log annotations
#[derive(Debug, Deserialize)]
struct LogAnnotationQueryParams {
    /// Filter by log timestamp (to get annotations for a specific log entry)
    log_timestamp: Option<String>,
    /// Filter by author
    author: Option<String>,
    /// Search in content
    search: Option<String>,
}

/// Get distinct log owners (users and agents)
async fn get_log_owners(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let storage = state.storage.lock().await;

    let owners = storage
        .get_distinct_log_owners()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({ "owners": owners })))
}

/// Get log annotations with optional filtering
async fn get_log_annotations(
    State(state): State<AppState>,
    Query(params): Query<LogAnnotationQueryParams>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let storage = state.storage.lock().await;

    // If log_timestamp is provided, get annotations for that specific log entry
    if let Some(ref timestamp) = params.log_timestamp {
        let annotations = storage
            .get_annotations_for_log(timestamp)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        return Ok(Json(serde_json::json!({ "annotations": annotations })));
    }

    // Otherwise, list all annotations with optional filters
    let annotations = storage
        .list_log_annotations(params.author.as_deref(), params.search.as_deref())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({ "annotations": annotations })))
}

/// Get a single log annotation by ID
async fn get_log_annotation_by_id(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let storage = state.storage.lock().await;
    let annotation = storage
        .get_log_annotation(&id)
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(serde_json::json!({ "annotation": annotation })))
}

/// Request body for adding a log annotation
#[derive(Deserialize)]
struct AddLogAnnotationRequest {
    log_timestamp: String,
    content: String,
    author: Option<String>,
}

/// Add a new log annotation
async fn add_log_annotation(
    State(state): State<AppState>,
    Json(request): Json<AddLogAnnotationRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Check readonly mode
    if state.readonly {
        return Err(readonly_error());
    }

    let storage = state.storage.lock().await;

    // Get the current user if author not provided
    let author = request.author.unwrap_or_else(|| {
        std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "unknown".to_string())
    });

    // Generate ID and create annotation
    let id = storage.generate_annotation_id(&request.log_timestamp);
    let annotation = LogAnnotation::new(
        id.clone(),
        request.log_timestamp.clone(),
        request.content.clone(),
        author,
    );

    // Add to storage
    storage.add_log_annotation(&annotation).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
    })?;

    Ok(Json(serde_json::json!({
        "success": true,
        "annotation": annotation
    })))
}

/// Delete a log annotation
async fn delete_log_annotation(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Check readonly mode
    if state.readonly {
        return Err(readonly_error());
    }

    let storage = state.storage.lock().await;

    storage.delete_log_annotation(&id).map_err(|e| {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
    })?;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("Annotation {} deleted", id)
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
    // Check readonly mode
    if state.readonly {
        return Err(readonly_error());
    }

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

/// Request body for batch link creation
#[derive(Deserialize)]
struct BatchAddLinksRequest {
    links: Vec<AddEdgeRequest>,
}

/// Result for a single link creation
#[derive(serde::Serialize)]
struct LinkResult {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    edge: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// Create multiple links in a single transaction
///
/// This endpoint accepts an array of links and creates them all, returning
/// individual success/failure status for each link. This is useful for batch
/// operations like creating multiple dependencies at once.
///
/// # Request Body
/// ```json
/// {
///   "links": [
///     {"source": "bn-xxxx", "target": "bn-yyyy", "edge_type": "depends_on"},
///     {"source": "bn-yyyy", "target": "bn-zzzz", "edge_type": "child_of"}
///   ]
/// }
/// ```
///
/// # Response
/// ```json
/// {
///   "success": true,
///   "total": 2,
///   "success_count": 2,
///   "error_count": 0,
///   "results": [
///     {"success": true, "edge": {"id": "...", "source": "...", "target": "...", "edge_type": "..."}},
///     {"success": true, "edge": {"id": "...", "source": "...", "target": "...", "edge_type": "..."}}
///   ]
/// }
/// ```
///
/// On partial failure, `success` will be false but results will include details for each link.
async fn batch_add_links(
    State(state): State<AppState>,
    Json(request): Json<BatchAddLinksRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Check readonly mode
    if state.readonly {
        return Err(readonly_error());
    }

    let mut storage = state.storage.lock().await;
    let mut results = Vec::new();
    let mut success_count = 0;
    let mut error_count = 0;

    // Process each link
    for link in request.links {
        // Parse edge type
        let edge_type: EdgeType = match link.edge_type.parse() {
            Ok(et) => et,
            Err(_) => {
                results.push(LinkResult {
                    success: false,
                    edge: None,
                    error: Some(format!("Invalid edge type: {}", link.edge_type)),
                });
                error_count += 1;
                continue;
            }
        };

        // Generate ID and create edge
        let id = storage.generate_edge_id(&link.source, &link.target, edge_type);
        let edge = Edge::new(
            id.clone(),
            link.source.clone(),
            link.target.clone(),
            edge_type,
        );

        // Add edge to storage
        match storage.add_edge(&edge) {
            Ok(_) => {
                results.push(LinkResult {
                    success: true,
                    edge: Some(serde_json::json!({
                        "id": id,
                        "source": link.source,
                        "target": link.target,
                        "edge_type": link.edge_type
                    })),
                    error: None,
                });
                success_count += 1;
            }
            Err(e) => {
                results.push(LinkResult {
                    success: false,
                    edge: None,
                    error: Some(e.to_string()),
                });
                error_count += 1;
            }
        }
    }

    Ok(Json(serde_json::json!({
        "success": error_count == 0,
        "total": results.len(),
        "success_count": success_count,
        "error_count": error_count,
        "results": results
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
    // Check readonly mode
    if state.readonly {
        return Err(readonly_error());
    }

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

/// Request body for batch closing entities
#[derive(Deserialize)]
struct BatchCloseRequest {
    node_ids: Vec<String>,
    reason: String,
}

/// Batch close multiple tasks and/or bugs
async fn batch_close(
    State(state): State<AppState>,
    Json(request): Json<BatchCloseRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Check readonly mode
    if state.readonly {
        return Err(readonly_error());
    }

    let mut storage = state.storage.lock().await;
    let mut closed = Vec::new();
    let mut failed = Vec::new();
    let mut skipped = Vec::new();

    for node_id in &request.node_ids {
        // Try to close as task
        if let Ok(mut task) = storage.get_task(node_id) {
            // Skip if already closed
            if task.status == TaskStatus::Done || task.status == TaskStatus::Cancelled {
                skipped.push(serde_json::json!({
                    "id": node_id,
                    "reason": "already closed"
                }));
                continue;
            }

            // Close the task
            task.status = TaskStatus::Done;
            task.closed_at = Some(Utc::now());
            task.closed_reason = Some(request.reason.clone());
            task.core.updated_at = Utc::now();

            match storage.update_task(&task) {
                Ok(_) => {
                    closed.push(serde_json::json!({
                        "id": node_id,
                        "type": "task"
                    }));
                }
                Err(e) => {
                    failed.push(serde_json::json!({
                        "id": node_id,
                        "error": e.to_string()
                    }));
                }
            }
            continue;
        }

        // Try to close as bug
        if let Ok(mut bug) = storage.get_bug(node_id) {
            // Skip if already closed
            if bug.status == TaskStatus::Done || bug.status == TaskStatus::Cancelled {
                skipped.push(serde_json::json!({
                    "id": node_id,
                    "reason": "already closed"
                }));
                continue;
            }

            // Close the bug
            bug.status = TaskStatus::Done;
            bug.closed_at = Some(Utc::now());
            bug.closed_reason = Some(request.reason.clone());
            bug.core.updated_at = Utc::now();

            match storage.update_bug(&bug) {
                Ok(_) => {
                    closed.push(serde_json::json!({
                        "id": node_id,
                        "type": "bug"
                    }));
                }
                Err(e) => {
                    failed.push(serde_json::json!({
                        "id": node_id,
                        "error": e.to_string()
                    }));
                }
            }
            continue;
        }

        // If we get here, the entity wasn't found or isn't closeable
        failed.push(serde_json::json!({
            "id": node_id,
            "error": "entity not found or not closeable"
        }));
    }

    Ok(Json(serde_json::json!({
        "success": true,
        "closed": closed,
        "failed": failed,
        "skipped": skipped,
        "summary": {
            "total": request.node_ids.len(),
            "closed_count": closed.len(),
            "failed_count": failed.len(),
            "skipped_count": skipped.len()
        }
    })))
}

/// Request body for batch queue operations
#[derive(Deserialize)]
struct BatchQueueRequest {
    node_ids: Vec<String>,
}

/// Batch add multiple tasks/bugs to the queue
async fn batch_queue_add(
    State(state): State<AppState>,
    Json(request): Json<BatchQueueRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Check readonly mode
    if state.readonly {
        return Err(readonly_error());
    }

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

    let mut added = Vec::new();
    let mut failed = Vec::new();
    let mut skipped = Vec::new();

    for node_id in &request.node_ids {
        // Check if already queued
        let edges = storage
            .list_edges(Some(EdgeType::Queued), Some(node_id), Some(&queue.id))
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
            })?;

        if !edges.is_empty() {
            skipped.push(serde_json::json!({
                "id": node_id,
                "reason": "already in queue"
            }));
            continue;
        }

        // Check if the item is closed before adding to queue
        let is_closed = if let Ok(task) = storage.get_task(node_id) {
            task.status == TaskStatus::Done || task.status == TaskStatus::Cancelled
        } else if let Ok(bug) = storage.get_bug(node_id) {
            bug.status == TaskStatus::Done || bug.status == TaskStatus::Cancelled
        } else {
            false
        };

        if is_closed {
            failed.push(serde_json::json!({
                "id": node_id,
                "error": "item is closed"
            }));
            continue;
        }

        // Verify item exists (task, bug, or milestone)
        let exists = storage.get_task(node_id).is_ok()
            || storage.get_bug(node_id).is_ok()
            || storage.get_milestone(node_id).is_ok();

        if !exists {
            failed.push(serde_json::json!({
                "id": node_id,
                "error": "entity not found"
            }));
            continue;
        }

        // Add to queue
        let edge_id = storage.generate_edge_id(node_id, &queue.id, EdgeType::Queued);
        let edge = Edge::new(edge_id, node_id.clone(), queue.id.clone(), EdgeType::Queued);

        match storage.add_edge(&edge) {
            Ok(_) => {
                added.push(serde_json::json!({
                    "id": node_id
                }));
            }
            Err(e) => {
                failed.push(serde_json::json!({
                    "id": node_id,
                    "error": e.to_string()
                }));
            }
        }
    }

    Ok(Json(serde_json::json!({
        "success": true,
        "added": added,
        "failed": failed,
        "skipped": skipped,
        "summary": {
            "total": request.node_ids.len(),
            "added_count": added.len(),
            "failed_count": failed.len(),
            "skipped_count": skipped.len()
        }
    })))
}

/// Batch remove multiple tasks/bugs from the queue
async fn batch_queue_remove(
    State(state): State<AppState>,
    Json(request): Json<BatchQueueRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Check readonly mode
    if state.readonly {
        return Err(readonly_error());
    }

    let mut storage = state.storage.lock().await;

    // Get the queue
    let queue = match storage.get_queue() {
        Ok(q) => q,
        Err(e) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": format!("Queue not found: {}", e) })),
            ));
        }
    };

    let mut removed = Vec::new();
    let mut failed = Vec::new();
    let mut skipped = Vec::new();

    for node_id in &request.node_ids {
        // Check if the node is queued
        let edges = storage
            .list_edges(Some(EdgeType::Queued), Some(node_id), Some(&queue.id))
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
            })?;

        if edges.is_empty() {
            skipped.push(serde_json::json!({
                "id": node_id,
                "reason": "not in queue"
            }));
            continue;
        }

        // Remove from queue
        match storage.remove_edge(node_id, &queue.id, EdgeType::Queued) {
            Ok(_) => {
                removed.push(serde_json::json!({
                    "id": node_id
                }));
            }
            Err(e) => {
                failed.push(serde_json::json!({
                    "id": node_id,
                    "error": e.to_string()
                }));
            }
        }
    }

    Ok(Json(serde_json::json!({
        "success": true,
        "removed": removed,
        "failed": failed,
        "skipped": skipped,
        "summary": {
            "total": request.node_ids.len(),
            "removed_count": removed.len(),
            "failed_count": failed.len(),
            "skipped_count": skipped.len()
        }
    })))
}

/// Get WebSocket performance metrics
async fn get_ws_metrics(State(state): State<AppState>) -> Json<serde_json::Value> {
    let metrics = state.ws_metrics.snapshot();
    Json(serde_json::json!({ "websocket": metrics }))
}

/// Get current binnacle version
async fn get_version(State(_state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Query parameters for git commits endpoint
#[derive(Deserialize)]
struct CommitsQueryParams {
    /// Maximum commits to return (default: 100, max: 500)
    limit: Option<u32>,
    /// Only return commits after this ISO 8601 timestamp
    after: Option<String>,
    /// Only return commits before this ISO 8601 timestamp
    before: Option<String>,
}

/// Get recent git commits for timeline correlation
///
/// Query parameters:
/// - `limit`: Maximum commits to return (default: 100, max: 500)
/// - `after`: Only return commits after this ISO 8601 timestamp
/// - `before`: Only return commits before this ISO 8601 timestamp
async fn get_git_commits(
    State(state): State<AppState>,
    Query(params): Query<CommitsQueryParams>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let limit = params.limit.unwrap_or(100).min(500);

    // Build git log command with format: sha|timestamp|author|subject
    let mut cmd = std::process::Command::new("git");
    cmd.arg("-C")
        .arg(&state.repo_path)
        .arg("--no-pager")
        .arg("log")
        .arg(format!("-{}", limit))
        .arg("--format=%H|%aI|%an|%s");

    // Add date range filters if provided
    if let Some(ref after) = params.after {
        cmd.arg(format!("--after={}", after));
    }
    if let Some(ref before) = params.before {
        cmd.arg(format!("--before={}", before));
    }

    let output = cmd
        .output()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !output.status.success() {
        // Git command failed (might not be a git repo)
        return Ok(Json(serde_json::json!({
            "commits": [],
            "count": 0,
            "error": "Not a git repository or git command failed"
        })));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let commits: Vec<serde_json::Value> = stdout
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(4, '|').collect();
            if parts.len() == 4 {
                Some(serde_json::json!({
                    "sha": parts[0],
                    "timestamp": parts[1],
                    "author": parts[2],
                    "subject": parts[3],
                    "short_sha": &parts[0][..7.min(parts[0].len())]
                }))
            } else {
                None
            }
        })
        .collect();

    let count = commits.len();
    Ok(Json(serde_json::json!({
        "commits": commits,
        "count": count
    })))
}

/// Request body for starting a summarize session
#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
struct SummarizeStartRequest {
    /// Selection context from the frontend (entities, edges, metadata)
    context: serde_json::Value,
}

/// Start a new summarize agent session (max 1 concurrent)
async fn summarize_start(
    State(state): State<AppState>,
    Json(_req): Json<SummarizeStartRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    if state.readonly {
        return Err(readonly_error());
    }

    let mut session = state.summarize_session.lock().await;

    // Check if there's already an active session
    if session.is_some() {
        return Err((
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error": "A summarize session is already active. Only one session allowed at a time."
            })),
        ));
    }

    // Generate agent ID
    let timestamp = Utc::now().timestamp_nanos_opt().unwrap_or(0).to_string();
    let agent_id = generate_id("bn", &format!("summarize-{}", timestamp));

    // Create new session
    let new_session = SummarizeSession {
        agent_id: agent_id.clone(),
        started_at: Utc::now(),
        messages: vec![],
    };

    *session = Some(new_session);

    Ok(Json(serde_json::json!({
        "success": true,
        "agent_id": agent_id,
        "message": "Summarize session started. Ready to analyze selection."
    })))
}

/// Request body for chatting with summarize agent
#[derive(Debug, serde::Deserialize)]
struct SummarizeChatRequest {
    /// User's message/question
    message: String,
}

/// Chat with the active summarize agent
async fn summarize_chat(
    State(state): State<AppState>,
    Json(req): Json<SummarizeChatRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let mut session = state.summarize_session.lock().await;

    // Check if there's an active session
    let sess = session.as_mut().ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "No active summarize session. Call /api/summarize/start first."
            })),
        )
    })?;

    // Add user message to history
    sess.messages.push(SummarizeMessage {
        role: "user".to_string(),
        content: req.message.clone(),
        timestamp: Utc::now(),
    });

    // TODO: In a real implementation, this would communicate with an actual agent
    // For now, return a placeholder response
    let response = format!(
        "I've analyzed your selection. You sent: '{}'. In a full implementation, I would provide insights, suggest relationships, or offer to create links between entities.",
        req.message
    );

    // Add assistant response to history
    sess.messages.push(SummarizeMessage {
        role: "assistant".to_string(),
        content: response.clone(),
        timestamp: Utc::now(),
    });

    Ok(Json(serde_json::json!({
        "success": true,
        "response": response,
        "suggested_actions": []
    })))
}

/// Request body for executing an action suggested by the summarize agent
#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
struct SummarizeActionRequest {
    /// Action type (e.g., "create_link", "export")
    action_type: String,
    /// Action parameters
    params: serde_json::Value,
}

/// Execute an action suggested by the summarize agent
async fn summarize_action(
    State(state): State<AppState>,
    Json(req): Json<SummarizeActionRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    if state.readonly {
        return Err(readonly_error());
    }

    let session = state.summarize_session.lock().await;

    // Check if there's an active session
    if session.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "No active summarize session. Call /api/summarize/start first."
            })),
        ));
    }

    // TODO: In a real implementation, this would execute actual actions
    // For now, return a placeholder response
    match req.action_type.as_str() {
        "create_link" => {
            // Extract source, target, link_type from params
            // Call storage.add_edge() to create the link
            Ok(Json(serde_json::json!({
                "success": true,
                "message": "Link creation not yet implemented in this phase"
            })))
        }
        "export" => {
            // Generate export of selection context
            Ok(Json(serde_json::json!({
                "success": true,
                "message": "Export not yet implemented in this phase"
            })))
        }
        _ => Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!("Unknown action type: {}", req.action_type)
            })),
        )),
    }
}
