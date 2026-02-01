//! File system watcher for binnacle data changes
//!
//! This module watches the binnacle storage directory for changes and sends
//! incremental entity messages to connected WebSocket clients.
//!
//! ## Protocol Support
//!
//! The watcher supports both legacy and new protocol messages:
//!
//! - **Legacy messages**: `entity_added`, `entity_updated`, `entity_removed`, `edge_added`, `edge_removed`
//!   These are sent for backward compatibility with older clients.
//!
//! - **New protocol**: [`ServerMessage::Delta`] containing [`Change`] events
//!   This is the recommended format for new clients subscribing via the session server protocol.
//!
//! Both message types are sent simultaneously to support mixed client populations.

use chrono::Utc;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::Instant;

use super::protocol::{Change, ServerMessage};
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

        // Load issues
        if let Ok(issues) = storage.list_issues(None, None, None, true) {
            for issue in issues {
                if let Ok(value) = serde_json::to_value(&issue) {
                    entities.insert(issue.core.id.clone(), value);
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
            "issue" => return "issue",
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

/// Extract actor information from an entity JSON value.
/// Returns (actor, actor_type) tuple. Checks assignee for tasks, assignee for bugs,
/// and falls back to environment variables (BN_AGENT_ID or USER).
fn extract_actor_info(entity: &Value) -> (Option<String>, Option<String>) {
    // Try to get assignee field (used in tasks, bugs)
    if let Some(assignee) = entity.get("assignee").and_then(|a| a.as_str())
        && !assignee.is_empty()
    {
        // Determine if assignee is an agent (starts with "bn-" or "bna-")
        let actor_type = if assignee.starts_with("bn-") || assignee.starts_with("bna-") {
            "agent"
        } else {
            "user"
        };
        return (Some(assignee.to_string()), Some(actor_type.to_string()));
    }

    // Fallback to environment variables
    if let Ok(agent_id) = std::env::var("BN_AGENT_ID") {
        return (Some(agent_id), Some("agent".to_string()));
    }

    if let Ok(user) = std::env::var("USER") {
        return (Some(user), Some("user".to_string()));
    }

    (None, None)
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

    /// Convert the diff to a Vec of Change events for the new protocol.
    fn to_changes(&self) -> Vec<Change> {
        let mut changes = Vec::with_capacity(self.total_changes());

        for change in &self.added {
            changes.push(Change::Create {
                entity_type: change.entity_type.to_string(),
                data: change.entity.clone(),
            });
        }

        for change in &self.updated {
            changes.push(Change::Update {
                entity_type: change.entity_type.to_string(),
                id: change.id.clone(),
                changes: change.entity.clone(),
            });
        }

        for removal in &self.removed {
            changes.push(Change::Delete {
                entity_type: removal.entity_type.to_string(),
                id: removal.id.clone(),
            });
        }

        changes
    }
}

impl EdgeDiff {
    /// Convert the edge diff to a Vec of Change events for the new protocol.
    fn to_changes(&self) -> Vec<Change> {
        let mut changes = Vec::with_capacity(self.total_changes());

        for edge in &self.added {
            if let Ok(data) = serde_json::to_value(edge) {
                changes.push(Change::Create {
                    entity_type: "link".to_string(),
                    data,
                });
            }
        }

        for edge in &self.removed {
            changes.push(Change::Delete {
                entity_type: "link".to_string(),
                id: edge.id.clone(),
            });
        }

        changes
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

                                // Also broadcast a log_entry message
                                let (actor, actor_type) = extract_actor_info(&change.entity);
                                let log_msg = serde_json::json!({
                                    "type": "log_entry",
                                    "entry": {
                                        "timestamp": timestamp,
                                        "entity_type": change.entity_type,
                                        "entity_id": change.id,
                                        "action": "created",
                                        "details": null,
                                        "actor": actor,
                                        "actor_type": actor_type,
                                    },
                                    "version": new_version,
                                })
                                .to_string();
                                let _ = update_tx.send(log_msg.clone());
                                message_history.push(new_version, log_msg).await;
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

                                // Also broadcast a log_entry message
                                let (actor, actor_type) = extract_actor_info(&change.entity);
                                // Determine the action based on entity state
                                let action = if change.entity.get("status").and_then(|s| s.as_str()) == Some("done")
                                    && change.entity.get("closed_at").is_some()
                                {
                                    "closed"
                                } else if change.entity.get("status").and_then(|s| s.as_str()) == Some("reopened") {
                                    "reopened"
                                } else {
                                    "updated"
                                };

                                let details = if action == "closed" {
                                    change.entity.get("closed_reason").and_then(|r| r.as_str()).map(|s| s.to_string())
                                } else {
                                    change.entity.get("status").and_then(|s| s.as_str()).map(|s| format!("status: {}", s))
                                };

                                let log_msg = serde_json::json!({
                                    "type": "log_entry",
                                    "entry": {
                                        "timestamp": timestamp,
                                        "entity_type": change.entity_type,
                                        "entity_id": change.id,
                                        "action": action,
                                        "details": details,
                                        "actor": actor,
                                        "actor_type": actor_type,
                                    },
                                    "version": new_version,
                                })
                                .to_string();
                                let _ = update_tx.send(log_msg.clone());
                                message_history.push(new_version, log_msg).await;
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

                                // Also broadcast a log_entry message for deletion
                                // Note: For removed entities, we don't have the entity data to extract actor
                                // Use environment fallback
                                let (actor, actor_type) = if let Ok(agent_id) = std::env::var("BN_AGENT_ID") {
                                    (Some(agent_id), Some("agent".to_string()))
                                } else if let Ok(user) = std::env::var("USER") {
                                    (Some(user), Some("user".to_string()))
                                } else {
                                    (None, None)
                                };

                                let log_msg = serde_json::json!({
                                    "type": "log_entry",
                                    "entry": {
                                        "timestamp": timestamp,
                                        "entity_type": removal.entity_type,
                                        "entity_id": removal.id,
                                        "action": "deleted",
                                        "details": null,
                                        "actor": actor,
                                        "actor_type": actor_type,
                                    },
                                    "version": new_version,
                                })
                                .to_string();
                                let _ = update_tx.send(log_msg.clone());
                                message_history.push(new_version, log_msg).await;
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

                            // Send new protocol Delta message with all changes bundled
                            // This is for clients using the new session server protocol
                            let mut all_changes = diff.to_changes();
                            all_changes.extend(edge_diff.to_changes());

                            let delta_msg = ServerMessage::Delta {
                                changes: all_changes,
                                version: new_version,
                                timestamp: Utc::now(),
                            };
                            if let Ok(json) = serde_json::to_string(&delta_msg) {
                                let _ = update_tx.send(json.clone());
                                message_history.push(new_version, json).await;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_diff_to_changes_empty() {
        let diff = EntityDiff {
            added: vec![],
            updated: vec![],
            removed: vec![],
        };

        let changes = diff.to_changes();
        assert!(changes.is_empty());
    }

    #[test]
    fn test_entity_diff_to_changes_added() {
        let diff = EntityDiff {
            added: vec![EntityChange {
                id: "bn-1234".to_string(),
                entity_type: "task",
                entity: serde_json::json!({"id": "bn-1234", "title": "Test task", "type": "task"}),
            }],
            updated: vec![],
            removed: vec![],
        };

        let changes = diff.to_changes();
        assert_eq!(changes.len(), 1);

        match &changes[0] {
            Change::Create { entity_type, data } => {
                assert_eq!(entity_type, "task");
                assert_eq!(data["id"], "bn-1234");
                assert_eq!(data["title"], "Test task");
            }
            _ => panic!("Expected Create change"),
        }
    }

    #[test]
    fn test_entity_diff_to_changes_updated() {
        let diff = EntityDiff {
            added: vec![],
            updated: vec![EntityChange {
                id: "bn-5678".to_string(),
                entity_type: "task",
                entity: serde_json::json!({
                    "id": "bn-5678",
                    "title": "Updated task",
                    "status": "done",
                    "type": "task"
                }),
            }],
            removed: vec![],
        };

        let changes = diff.to_changes();
        assert_eq!(changes.len(), 1);

        match &changes[0] {
            Change::Update {
                entity_type,
                id,
                changes: change_data,
            } => {
                assert_eq!(entity_type, "task");
                assert_eq!(id, "bn-5678");
                assert_eq!(change_data["status"], "done");
            }
            _ => panic!("Expected Update change"),
        }
    }

    #[test]
    fn test_entity_diff_to_changes_removed() {
        let diff = EntityDiff {
            added: vec![],
            updated: vec![],
            removed: vec![EntityRemoval {
                id: "bn-abcd".to_string(),
                entity_type: "task",
            }],
        };

        let changes = diff.to_changes();
        assert_eq!(changes.len(), 1);

        match &changes[0] {
            Change::Delete { entity_type, id } => {
                assert_eq!(entity_type, "task");
                assert_eq!(id, "bn-abcd");
            }
            _ => panic!("Expected Delete change"),
        }
    }

    #[test]
    fn test_entity_diff_to_changes_mixed() {
        let diff = EntityDiff {
            added: vec![EntityChange {
                id: "bn-new".to_string(),
                entity_type: "task",
                entity: serde_json::json!({"id": "bn-new", "type": "task"}),
            }],
            updated: vec![EntityChange {
                id: "bn-upd".to_string(),
                entity_type: "bug",
                entity: serde_json::json!({"id": "bn-upd", "type": "bug"}),
            }],
            removed: vec![EntityRemoval {
                id: "bn-del".to_string(),
                entity_type: "milestone",
            }],
        };

        let changes = diff.to_changes();
        assert_eq!(changes.len(), 3);

        // Changes should be in order: added, updated, removed
        assert!(matches!(&changes[0], Change::Create { .. }));
        assert!(matches!(&changes[1], Change::Update { .. }));
        assert!(matches!(&changes[2], Change::Delete { .. }));
    }

    #[test]
    fn test_edge_diff_to_changes_empty() {
        let diff = EdgeDiff {
            added: vec![],
            removed: vec![],
        };

        let changes = diff.to_changes();
        assert!(changes.is_empty());
    }

    #[test]
    fn test_edge_diff_to_changes_added() {
        use crate::models::EdgeType;

        let diff = EdgeDiff {
            added: vec![Edge {
                id: "edge-123".to_string(),
                entity_type: "edge".to_string(),
                source: "bn-src".to_string(),
                target: "bn-tgt".to_string(),
                edge_type: EdgeType::DependsOn,
                weight: 1.0,
                reason: None,
                created_at: Utc::now(),
                created_by: None,
                pinned: false,
            }],
            removed: vec![],
        };

        let changes = diff.to_changes();
        assert_eq!(changes.len(), 1);

        match &changes[0] {
            Change::Create { entity_type, data } => {
                assert_eq!(entity_type, "link");
                assert_eq!(data["id"], "edge-123");
                assert_eq!(data["source"], "bn-src");
            }
            _ => panic!("Expected Create change"),
        }
    }

    #[test]
    fn test_edge_diff_to_changes_removed() {
        use crate::models::EdgeType;

        let diff = EdgeDiff {
            added: vec![],
            removed: vec![Edge {
                id: "edge-456".to_string(),
                entity_type: "edge".to_string(),
                source: "bn-src".to_string(),
                target: "bn-tgt".to_string(),
                edge_type: EdgeType::ChildOf,
                weight: 1.0,
                reason: None,
                created_at: Utc::now(),
                created_by: None,
                pinned: false,
            }],
        };

        let changes = diff.to_changes();
        assert_eq!(changes.len(), 1);

        match &changes[0] {
            Change::Delete { entity_type, id } => {
                assert_eq!(entity_type, "link");
                assert_eq!(id, "edge-456");
            }
            _ => panic!("Expected Delete change"),
        }
    }

    #[test]
    fn test_delta_message_serialization() {
        // Test that the delta message can be serialized to JSON
        let diff = EntityDiff {
            added: vec![EntityChange {
                id: "bn-test".to_string(),
                entity_type: "task",
                entity: serde_json::json!({"id": "bn-test", "type": "task"}),
            }],
            updated: vec![],
            removed: vec![],
        };

        let changes = diff.to_changes();
        let delta_msg = ServerMessage::Delta {
            changes,
            version: 42,
            timestamp: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&delta_msg).unwrap();
        assert!(json.contains(r#""type":"delta""#));
        assert!(json.contains(r#""version":42"#));
        assert!(json.contains(r#""op":"create""#));
        assert!(json.contains(r#""entity_type":"task""#));

        // Verify round-trip
        let parsed: ServerMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            ServerMessage::Delta {
                changes, version, ..
            } => {
                assert_eq!(version, 42);
                assert_eq!(changes.len(), 1);
            }
            _ => panic!("Expected Delta message"),
        }
    }
}
