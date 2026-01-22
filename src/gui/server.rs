//! Web server for serving the GUI and API endpoints

use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
    Json, Router,
};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

use crate::storage::Storage;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    /// Storage instance for reading binnacle data (wrapped in Mutex for thread safety)
    pub storage: Arc<Mutex<Storage>>,
    /// Broadcast channel for sending updates to WebSocket clients
    pub update_tx: broadcast::Sender<String>,
}

/// Start the GUI web server
pub async fn start_server(repo_path: &Path, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let storage = Storage::open(repo_path)?;
    let storage_dir = crate::storage::get_storage_dir(repo_path)?;
    let (update_tx, _) = broadcast::channel(100);

    let state = AppState {
        storage: Arc::new(Mutex::new(storage)),
        update_tx,
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
        .route("/api/tasks", get(get_tasks))
        .route("/api/ready", get(get_ready))
        .route("/api/tests", get(get_tests))
        .route("/api/edges", get(get_edges))
        .route("/api/log", get(get_log))
        .route("/ws", get(crate::gui::websocket::ws_handler))
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
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

/// Get all tasks
async fn get_tasks(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let storage = state.storage.lock().await;
    let tasks = storage
        .list_tasks(None, None, None)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({ "tasks": tasks })))
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

/// Get activity log
async fn get_log(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let storage = state.storage.lock().await;

    // Read the action log file directly since there's no get_log method
    let log_path = storage.root.join("../action.log");
    let log_entries = if log_path.exists() {
        std::fs::read_to_string(&log_path)
            .map(|content| {
                content
                    .lines()
                    .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    } else {
        vec![]
    };

    Ok(Json(serde_json::json!({ "log": log_entries })))
}
