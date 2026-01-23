//! Web server for serving the GUI and API endpoints

use axum::{
    Json, Router,
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
};
use serde::Deserialize;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast};

use crate::models::{Edge, EdgeType};
use crate::storage::Storage;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    /// Storage instance for reading binnacle data (wrapped in Mutex for thread safety)
    pub storage: Arc<Mutex<Storage>>,
    /// Broadcast channel for sending updates to WebSocket clients
    pub update_tx: broadcast::Sender<String>,
    /// Name of the project folder (for display in GUI title)
    pub project_name: String,
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
        .route("/api/tests", get(get_tests))
        .route("/api/queue", get(get_queue))
        .route("/api/edges", get(get_edges))
        .route("/api/edges", post(add_edge))
        .route("/api/log", get(get_log))
        .route("/api/agents", get(get_agents))
        .route("/api/agents/{pid}/kill", post(kill_agent))
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
        .list_bugs(None, None, None, None)
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

/// Get all tests
async fn get_tests(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let storage = state.storage.lock().await;
    let tests = storage
        .list_tests(None)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({ "tests": tests })))
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
    // Clean up stale agents before returning
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

/// Get activity log (limited to most recent entries to reduce bandwidth)
const MAX_LOG_ENTRIES: usize = 100;

async fn get_log(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let storage = state.storage.lock().await;

    // Read the action log file directly since there's no get_log method
    let log_path = storage.root.join("../action.log");
    let log_entries = if log_path.exists() {
        std::fs::read_to_string(&log_path)
            .map(|content| {
                let entries: Vec<_> = content
                    .lines()
                    .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
                    .collect();
                // Return only the most recent entries to limit bandwidth
                let start = entries.len().saturating_sub(MAX_LOG_ENTRIES);
                entries[start..].to_vec()
            })
            .unwrap_or_default()
    } else {
        vec![]
    };

    Ok(Json(serde_json::json!({ "log": log_entries })))
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
