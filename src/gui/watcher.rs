//! File system watcher for binnacle data changes
//!
//! This module watches the binnacle storage directory for changes and sends
//! incremental entity messages to connected WebSocket clients.

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::Instant;

use super::server::StateVersion;
use crate::models::Edge;
use crate::storage::Storage;

/// Debounce duration - wait this long after last event before sending update
const DEBOUNCE_MS: u64 = 100;

/// Maximum number of incremental messages before falling back to reload
const MAX_INCREMENTAL_MESSAGES: usize = 50;

/// Represents a snapshot of all entities for diffing
#[derive(Default)]
struct EntitySnapshot {
    /// Map from entity ID to serialized JSON value
    entities: HashMap<String, Value>,
}

/// Represents a snapshot of all edges for diffing
#[derive(Default)]
struct EdgeSnapshot {
    /// Map from edge ID to Edge
    edges: HashMap<String, Edge>,
}

impl EdgeSnapshot {
    /// Load a snapshot of all edges from storage
    fn load(storage: &Storage) -> Self {
        let mut edges = HashMap::new();

        if let Ok(edge_list) = storage.list_edges(None, None, None) {
            for edge in edge_list {
                edges.insert(edge.id.clone(), edge);
            }
        }

        Self { edges }
    }

    /// Compute the diff between this snapshot and a new one
    fn diff(&self, new: &EdgeSnapshot) -> EdgeDiff {
        let mut added = Vec::new();
        let mut removed = Vec::new();

        // Find added edges
        for (id, edge) in &new.edges {
            if !self.edges.contains_key(id) {
                added.push(edge.clone());
            }
            // Note: Edges are typically immutable - no "updated" case needed
            // If edge properties change, we'd need to add updated handling
        }

        // Find removed edges
        for (id, edge) in &self.edges {
            if !new.edges.contains_key(id) {
                removed.push(edge.clone());
            }
        }

        EdgeDiff { added, removed }
    }
}

/// Diff between two edge snapshots
struct EdgeDiff {
    added: Vec<Edge>,
    removed: Vec<Edge>,
}

impl EdgeDiff {
    /// Check if the diff is empty
    fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty()
    }

    /// Total number of changes
    fn total_changes(&self) -> usize {
        self.added.len() + self.removed.len()
    }
}

impl EntitySnapshot {
    /// Load a snapshot of all entities from storage
    fn load(storage: &Storage) -> Self {
        let mut entities = HashMap::new();

        // Load tasks
        if let Ok(tasks) = storage.list_tasks(None, None, None) {
            for task in tasks {
                if let Ok(value) = serde_json::to_value(&task) {
                    entities.insert(task.core.id.clone(), value);
                }
            }
        }

        // Load bugs
        if let Ok(bugs) = storage.list_bugs(None, None, None, None, true) {
            for bug in bugs {
                if let Ok(value) = serde_json::to_value(&bug) {
                    entities.insert(bug.core.id.clone(), value);
                }
            }
        }

        // Load ideas
        if let Ok(ideas) = storage.list_ideas(None, None) {
            for idea in ideas {
                if let Ok(value) = serde_json::to_value(&idea) {
                    entities.insert(idea.core.id.clone(), value);
                }
            }
        }

        // Load milestones
        if let Ok(milestones) = storage.list_milestones(None, None, None) {
            for milestone in milestones {
                if let Ok(value) = serde_json::to_value(&milestone) {
                    entities.insert(milestone.core.id.clone(), value);
                }
            }
        }

        // Load tests
        if let Ok(tests) = storage.list_tests(None) {
            for test in tests {
                if let Ok(value) = serde_json::to_value(&test) {
                    entities.insert(test.id.clone(), value);
                }
            }
        }

        // Load docs
        if let Ok(docs) = storage.list_docs(None, None, None, None) {
            for doc in docs {
                if let Ok(value) = serde_json::to_value(&doc) {
                    entities.insert(doc.core.id.clone(), value);
                }
            }
        }

        // Load queue
        if let Ok(queue) = storage.get_queue()
            && let Ok(value) = serde_json::to_value(&queue)
        {
            entities.insert(queue.id.clone(), value);
        }

        // Load agents
        if let Ok(agents) = storage.list_agents(None) {
            for agent in agents {
                if let Ok(value) = serde_json::to_value(&agent) {
                    entities.insert(agent.id.clone(), value);
                }
            }
        }

        Self { entities }
    }

    /// Compute the diff between this snapshot and a new one
    fn diff(&self, new: &EntitySnapshot) -> EntityDiff {
        let mut added = Vec::new();
        let mut updated = Vec::new();
        let mut removed = Vec::new();

        // Find added and updated entities
        for (id, new_value) in &new.entities {
            match self.entities.get(id) {
                Some(old_value) => {
                    if old_value != new_value {
                        updated.push(EntityChange {
                            id: id.clone(),
                            entity_type: entity_type_from_value(new_value),
                            entity: new_value.clone(),
                        });
                    }
                }
                None => {
                    added.push(EntityChange {
                        id: id.clone(),
                        entity_type: entity_type_from_value(new_value),
                        entity: new_value.clone(),
                    });
                }
            }
        }

        // Find removed entities
        // Note: For removed entities, we use the old entity's type (stored before removal)
        for (id, old_value) in &self.entities {
            if !new.entities.contains_key(id) {
                removed.push(EntityRemoval {
                    id: id.clone(),
                    entity_type: entity_type_from_value(old_value),
                });
            }
        }

        EntityDiff {
            added,
            updated,
            removed,
        }
    }
}

/// Determine entity type from entity JSON value.
/// Extracts the "type" field from the entity, falling back to ID prefix detection.
fn entity_type_from_value(value: &Value) -> &'static str {
    // Try to get the type from the entity's "type" field
    if let Some(type_str) = value.get("type").and_then(|t| t.as_str()) {
        match type_str {
            "task" => return "task",
            "bug" => return "bug",
            "idea" => return "idea",
            "test" => return "test",
            "milestone" => return "milestone",
            "edge" => return "edge",
            "queue" => return "queue",
            "doc" => return "doc",
            "agent" => return "agent",
            _ => {}
        }
    }

    // Fallback to ID prefix detection
    if let Some(id) = value.get("id").and_then(|i| i.as_str()) {
        entity_type_from_id(id)
    } else {
        "unknown"
    }
}

/// Determine entity type from ID prefix (fallback for legacy IDs)
fn entity_type_from_id(id: &str) -> &'static str {
    if id.starts_with("bnt-") {
        "test"
    } else if id.starts_with("bnd-") {
        "doc"
    } else if id.starts_with("bnq-") {
        "queue"
    } else if id.starts_with("bnm-") {
        "milestone"
    } else if id.starts_with("bnb-") {
        "bug"
    } else if id.starts_with("bni-") {
        "idea"
    } else if id.starts_with("bn-") {
        // Note: bn- prefix now includes both tasks and agents
        // The entity_type_from_value function handles this correctly
        // This is a fallback only
        "task"
    } else {
        "unknown"
    }
}

/// Represents a change to an entity (add or update)
#[derive(Serialize)]
struct EntityChange {
    id: String,
    entity_type: &'static str,
    entity: Value,
}

/// Represents an entity removal
#[derive(Serialize)]
struct EntityRemoval {
    id: String,
    entity_type: &'static str,
}

/// Diff between two entity snapshots
struct EntityDiff {
    added: Vec<EntityChange>,
    updated: Vec<EntityChange>,
    removed: Vec<EntityRemoval>,
}

impl EntityDiff {
    /// Check if the diff is empty
    fn is_empty(&self) -> bool {
        self.added.is_empty() && self.updated.is_empty() && self.removed.is_empty()
    }

    /// Total number of changes
    fn total_changes(&self) -> usize {
        self.added.len() + self.updated.len() + self.removed.len()
    }
}

/// Watch the binnacle storage directory for changes
pub async fn watch_storage(
    storage_path: PathBuf,
    repo_path: PathBuf,
    update_tx: broadcast::Sender<String>,
    version: StateVersion,
    message_history: super::server::MessageHistory,
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

    // Load initial snapshots
    let storage = Storage::open(&repo_path)?;
    let mut current_snapshot = EntitySnapshot::load(&storage);
    let mut current_edge_snapshot = EdgeSnapshot::load(&storage);
    drop(storage);

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
                // Debounce timeout expired, compute and send incremental updates
                let new_version = version.increment();
                let timestamp = chrono::Utc::now().to_rfc3339();

                // Load new snapshot and compute diff
                match Storage::open(&repo_path) {
                    Ok(storage) => {
                        let new_snapshot = EntitySnapshot::load(&storage);
                        let new_edge_snapshot = EdgeSnapshot::load(&storage);
                        drop(storage);

                        let diff = current_snapshot.diff(&new_snapshot);
                        let edge_diff = current_edge_snapshot.diff(&new_edge_snapshot);

                        let total_changes = diff.total_changes() + edge_diff.total_changes();

                        if diff.is_empty() && edge_diff.is_empty() {
                            // No actual changes, skip sending anything
                        } else if total_changes > MAX_INCREMENTAL_MESSAGES {
                            // Too many changes, fall back to reload
                            let reload_msg = serde_json::json!({
                                "type": "reload",
                                "version": new_version,
                                "timestamp": timestamp
                            })
                            .to_string();
                            let _ = update_tx.send(reload_msg);
                            // Clear message history on reload - clients will sync from scratch
                            message_history.clear().await;
                        } else {
                            // Send incremental messages and record them in history
                            for change in &diff.added {
                                let msg = serde_json::json!({
                                    "type": "entity_added",
                                    "entity_type": change.entity_type,
                                    "id": change.id,
                                    "entity": change.entity,
                                    "version": new_version,
                                    "timestamp": timestamp
                                })
                                .to_string();
                                let _ = update_tx.send(msg.clone());
                                message_history.push(new_version, msg).await;
                            }

                            for change in &diff.updated {
                                let msg = serde_json::json!({
                                    "type": "entity_updated",
                                    "entity_type": change.entity_type,
                                    "id": change.id,
                                    "entity": change.entity,
                                    "version": new_version,
                                    "timestamp": timestamp
                                })
                                .to_string();
                                let _ = update_tx.send(msg.clone());
                                message_history.push(new_version, msg).await;
                            }

                            for removal in &diff.removed {
                                let msg = serde_json::json!({
                                    "type": "entity_removed",
                                    "entity_type": removal.entity_type,
                                    "id": removal.id,
                                    "version": new_version,
                                    "timestamp": timestamp
                                })
                                .to_string();
                                let _ = update_tx.send(msg.clone());
                                message_history.push(new_version, msg).await;
                            }

                            // Send edge messages
                            for edge in &edge_diff.added {
                                let msg = serde_json::json!({
                                    "type": "edge_added",
                                    "id": edge.id,
                                    "edge": edge,
                                    "version": new_version,
                                    "timestamp": timestamp
                                })
                                .to_string();
                                let _ = update_tx.send(msg.clone());
                                message_history.push(new_version, msg).await;
                            }

                            for edge in &edge_diff.removed {
                                let msg = serde_json::json!({
                                    "type": "edge_removed",
                                    "id": edge.id,
                                    "edge": edge,
                                    "version": new_version,
                                    "timestamp": timestamp
                                })
                                .to_string();
                                let _ = update_tx.send(msg.clone());
                                message_history.push(new_version, msg).await;
                            }
                        }

                        // Update current snapshots for next diff
                        current_snapshot = new_snapshot;
                        current_edge_snapshot = new_edge_snapshot;
                    }
                    Err(_) => {
                        // Couldn't open storage, fall back to reload
                        let _ = update_tx.send(
                            serde_json::json!({
                                "type": "reload",
                                "version": new_version,
                                "timestamp": timestamp
                            })
                            .to_string(),
                        );
                    }
                }

                pending_update = false;
            }
        }
    }

    Ok(())
}
