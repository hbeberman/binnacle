//! Data models for Binnacle entities.
//!
//! This module defines the core data structures:
//! - `Task` - Work items with status, priority, dependencies
//! - `Bug` - Defects with severity, reproduction steps, and components
//! - `TestNode` - Test definitions linked to tasks
//! - `CommitLink` - Associations between commits and tasks

pub mod graph;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Task status in the workflow.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    #[default]
    Pending,
    InProgress,
    Done,
    Blocked,
    Cancelled,
    Reopened,
    /// Started but incomplete because dependencies aren't done
    Partial,
}

/// A work item tracked by Binnacle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique identifier (e.g., "bn-a1b2")
    pub id: String,

    /// Entity type marker
    #[serde(rename = "type")]
    pub entity_type: String,

    /// Task title
    pub title: String,

    /// Detailed description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Priority level (0-4, lower is higher priority)
    #[serde(default)]
    pub priority: u8,

    /// Current status
    #[serde(default)]
    pub status: TaskStatus,

    /// Parent task ID for hierarchical organization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,

    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,

    /// Assigned user or agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,

    /// Task IDs this task depends on
    #[serde(default)]
    pub depends_on: Vec<String>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,

    /// Closure timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed_at: Option<DateTime<Utc>>,

    /// Reason for closing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed_reason: Option<String>,
}

impl Task {
    /// Create a new task with the given ID and title.
    pub fn new(id: String, title: String) -> Self {
        let now = Utc::now();
        Self {
            id,
            entity_type: "task".to_string(),
            title,
            description: None,
            priority: 2, // Default middle priority
            status: TaskStatus::default(),
            parent: None,
            tags: Vec::new(),
            assignee: None,
            depends_on: Vec::new(),
            created_at: now,
            updated_at: now,
            closed_at: None,
            closed_reason: None,
        }
    }
}

/// A defect tracked by Binnacle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bug {
    /// Unique identifier (e.g., "bn-b1b2")
    pub id: String,

    /// Entity type marker
    #[serde(rename = "type")]
    pub entity_type: String,

    /// Bug title
    pub title: String,

    /// Detailed description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Priority level (0-4, lower is higher priority)
    #[serde(default)]
    pub priority: u8,

    /// Current status
    #[serde(default)]
    pub status: TaskStatus,

    /// Severity level (e.g., "low", "medium", "high", "critical")
    #[serde(default = "default_bug_severity")]
    pub severity: String,

    /// Steps to reproduce
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reproduction_steps: Option<String>,

    /// Affected component or area
    #[serde(skip_serializing_if = "Option::is_none")]
    pub affected_component: Option<String>,

    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,

    /// Assigned user or agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,

    /// IDs this bug depends on
    #[serde(default)]
    pub depends_on: Vec<String>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,

    /// Closure timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed_at: Option<DateTime<Utc>>,

    /// Reason for closing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed_reason: Option<String>,
}

impl Bug {
    /// Create a new bug with the given ID and title.
    pub fn new(id: String, title: String) -> Self {
        let now = Utc::now();
        Self {
            id,
            entity_type: "bug".to_string(),
            title,
            description: None,
            priority: 2,
            status: TaskStatus::default(),
            severity: default_bug_severity(),
            reproduction_steps: None,
            affected_component: None,
            tags: Vec::new(),
            assignee: None,
            depends_on: Vec::new(),
            created_at: now,
            updated_at: now,
            closed_at: None,
            closed_reason: None,
        }
    }
}

/// A test node linked to tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestNode {
    /// Unique identifier (e.g., "bnt-0001")
    pub id: String,

    /// Entity type marker
    #[serde(rename = "type")]
    pub entity_type: String,

    /// Test name
    pub name: String,

    /// Command to execute
    pub command: String,

    /// Working directory for execution
    #[serde(default = "default_working_dir")]
    pub working_dir: String,

    /// Optional pattern for matching test files
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,

    /// Task IDs this test is linked to
    #[serde(default)]
    pub linked_tasks: Vec<String>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

fn default_working_dir() -> String {
    ".".to_string()
}

fn default_bug_severity() -> String {
    "medium".to_string()
}

impl TestNode {
    /// Create a new test node with the given ID, name, and command.
    pub fn new(id: String, name: String, command: String) -> Self {
        Self {
            id,
            entity_type: "test".to_string(),
            name,
            command,
            working_dir: ".".to_string(),
            pattern: None,
            linked_tasks: Vec::new(),
            created_at: Utc::now(),
        }
    }
}

/// Result of a test run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    /// Test node ID
    pub test_id: String,

    /// Whether the test passed
    pub passed: bool,

    /// Exit code
    pub exit_code: i32,

    /// Standard output
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,

    /// Standard error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,

    /// Duration in milliseconds
    pub duration_ms: u64,

    /// Execution timestamp
    pub executed_at: DateTime<Utc>,
}

/// Association between a commit and a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitLink {
    /// Git commit SHA
    pub sha: String,

    /// Task ID
    pub task_id: String,

    /// When the link was created
    pub linked_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_serialization_roundtrip() {
        let task = Task::new("bn-test".to_string(), "Test task".to_string());
        let json = serde_json::to_string(&task).unwrap();
        let deserialized: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(task.id, deserialized.id);
        assert_eq!(task.title, deserialized.title);
    }

    #[test]
    fn test_task_status_serialization() {
        let status = TaskStatus::InProgress;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, r#""in_progress""#);
    }

    #[test]
    fn test_bug_serialization_roundtrip() {
        let bug = Bug::new("bn-bug".to_string(), "Test bug".to_string());
        let json = serde_json::to_string(&bug).unwrap();
        let deserialized: Bug = serde_json::from_str(&json).unwrap();
        assert_eq!(bug.id, deserialized.id);
        assert_eq!(bug.title, deserialized.title);
        assert_eq!(bug.severity, deserialized.severity);
    }

    #[test]
    fn test_bug_default_severity() {
        let json = r#"{"id":"bn-bug","type":"bug","title":"Bug","priority":1,"status":"pending","created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z"}"#;
        let bug: Bug = serde_json::from_str(json).unwrap();
        assert_eq!(bug.severity, "medium");
    }

    #[test]
    fn test_partial_status_serialization() {
        let status = TaskStatus::Partial;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, r#""partial""#);

        // Test deserialization
        let deserialized: TaskStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, TaskStatus::Partial);
    }

    #[test]
    fn test_test_node_serialization_roundtrip() {
        let test = TestNode::new(
            "bnt-0001".to_string(),
            "Unit tests".to_string(),
            "cargo test".to_string(),
        );
        let json = serde_json::to_string(&test).unwrap();
        let deserialized: TestNode = serde_json::from_str(&json).unwrap();
        assert_eq!(test.id, deserialized.id);
        assert_eq!(test.command, deserialized.command);
    }
}
