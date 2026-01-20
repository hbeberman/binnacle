//! File system watcher for binnacle data changes

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use tokio::sync::broadcast;

/// Watch the binnacle storage directory for changes
pub async fn watch_storage(
    storage_path: PathBuf,
    update_tx: broadcast::Sender<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);

    // Create watcher in a blocking task
    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                let _ = tx.blocking_send(event);
            }
        },
        Config::default(),
    )?;

    // Watch the storage directory
    watcher.watch(&storage_path, RecursiveMode::Recursive)?;

    // Process events
    while let Some(event) = rx.recv().await {
        // Check if the event is a write/create/remove
        match event.kind {
            notify::EventKind::Create(_)
            | notify::EventKind::Modify(_)
            | notify::EventKind::Remove(_) => {
                // Send update notification to WebSocket clients
                let _ = update_tx.send(serde_json::json!({
                    "type": "reload",
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }).to_string());
            }
            _ => {}
        }
    }

    Ok(())
}
