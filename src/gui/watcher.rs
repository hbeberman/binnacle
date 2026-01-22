//! File system watcher for binnacle data changes

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::Instant;

/// Debounce duration - wait this long after last event before sending update
const DEBOUNCE_MS: u64 = 100;

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

    // Debounce state: track when we last saw a relevant event
    let mut pending_update = false;
    let mut last_event_time = Instant::now();

    loop {
        // If we have a pending update, wait with timeout for more events
        let timeout = if pending_update {
            let elapsed = last_event_time.elapsed();
            let debounce = Duration::from_millis(DEBOUNCE_MS);
            if elapsed >= debounce {
                Duration::ZERO
            } else {
                debounce - elapsed
            }
        } else {
            // No pending update, wait indefinitely for next event
            Duration::from_secs(3600)
        };

        tokio::select! {
            event = rx.recv() => {
                match event {
                    Some(event) => {
                        // Check if the event is a write/create/remove
                        match event.kind {
                            notify::EventKind::Create(_)
                            | notify::EventKind::Modify(_)
                            | notify::EventKind::Remove(_) => {
                                pending_update = true;
                                last_event_time = Instant::now();
                            }
                            _ => {}
                        }
                    }
                    None => break, // Channel closed
                }
            }
            _ = tokio::time::sleep(timeout), if pending_update => {
                // Debounce timeout expired, send the update
                let _ = update_tx.send(
                    serde_json::json!({
                        "type": "reload",
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    })
                    .to_string(),
                );
                pending_update = false;
            }
        }
    }

    Ok(())
}
