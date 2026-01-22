//! Data models for Binnacle entities.
//!
//! This module defines the core data structures:
//! - `Task` - Work items with status, priority, dependencies
//! - `Bug` - Defects with severity, reproduction steps, and components
//! - `Milestone` - Collection of tasks/bugs with progress tracking
//! - `TestNode` - Test definitions linked to tasks
//! - `CommitLink` - Associations between commits and tasks
//! - `Edge` - Relationships between entities (dependencies, blocks, related, etc.)

pub mod graph;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

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

/// Bug severity in the workflow.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BugSeverity {
    #[default]
    Triage,
    Low,
    Medium,
    High,
    Critical,
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

    /// Optional short display name (shown in GUI instead of ID)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_name: Option<String>,

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

    /// Timestamp when this task was imported from another store.
    /// None for tasks created locally.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imported_on: Option<DateTime<Utc>>,
}

impl Task {
    /// Create a new task with the given ID and title.
    pub fn new(id: String, title: String) -> Self {
        let now = Utc::now();
        Self {
            id,
            entity_type: "task".to_string(),
            title,
            short_name: None,
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
            imported_on: None,
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
    #[serde(default)]
    pub severity: BugSeverity,

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
            severity: BugSeverity::default(),
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

/// A milestone for grouping and tracking progress of tasks and bugs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestone {
    /// Unique identifier (e.g., "bn-m1b2")
    pub id: String,

    /// Entity type marker
    #[serde(rename = "type")]
    pub entity_type: String,

    /// Milestone title
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

    /// Target completion date
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_date: Option<DateTime<Utc>>,

    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,

    /// Assigned user or agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,

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

impl Milestone {
    /// Create a new milestone with the given ID and title.
    pub fn new(id: String, title: String) -> Self {
        let now = Utc::now();
        Self {
            id,
            entity_type: "milestone".to_string(),
            title,
            description: None,
            priority: 2,
            status: TaskStatus::default(),
            due_date: None,
            tags: Vec::new(),
            assignee: None,
            created_at: now,
            updated_at: now,
            closed_at: None,
            closed_reason: None,
        }
    }
}

/// Progress statistics for a milestone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MilestoneProgress {
    /// Total number of child items (tasks + bugs)
    pub total: usize,
    /// Number of completed items
    pub completed: usize,
    /// Completion percentage (0-100)
    pub percentage: f64,
}

impl MilestoneProgress {
    /// Create new progress stats.
    pub fn new(total: usize, completed: usize) -> Self {
        let percentage = if total > 0 {
            (completed as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        Self {
            total,
            completed,
            percentage,
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

/// Type of relationship between entities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    /// Source blocks until target completes (Task/Bug/Milestone → Task/Bug)
    DependsOn,
    /// Source prevents target from progressing (Task/Bug → Task/Bug/Milestone)
    Blocks,
    /// Informational bidirectional link (Any ↔ Any)
    RelatedTo,
    /// Source is duplicate of target (same type only: Task→Task or Bug→Bug)
    Duplicates,
    /// Task fixes the bug (Task → Bug)
    Fixes,
    /// Bug was caused by this work (Bug → Task/Commit)
    CausedBy,
    /// Source replaces target (same type only: Task→Task or Bug→Bug)
    Supersedes,
    /// Containment relationship (Task/Milestone → Task/Bug)
    ParentOf,
    /// Inverse of parent_of (Task/Bug → Task/Milestone)
    ChildOf,
    /// Test verifies this work (Test → Task/Bug)
    Tests,
}

impl EdgeType {
    /// Returns true if this edge type is bidirectional.
    pub fn is_bidirectional(&self) -> bool {
        matches!(self, EdgeType::RelatedTo)
    }

    /// Returns true if this edge type affects blocking/ready status.
    pub fn is_blocking(&self) -> bool {
        matches!(self, EdgeType::DependsOn | EdgeType::Blocks)
    }

    /// Get all edge types.
    pub fn all() -> &'static [EdgeType] {
        &[
            EdgeType::DependsOn,
            EdgeType::Blocks,
            EdgeType::RelatedTo,
            EdgeType::Duplicates,
            EdgeType::Fixes,
            EdgeType::CausedBy,
            EdgeType::Supersedes,
            EdgeType::ParentOf,
            EdgeType::ChildOf,
            EdgeType::Tests,
        ]
    }
}

impl fmt::Display for EdgeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            EdgeType::DependsOn => "depends_on",
            EdgeType::Blocks => "blocks",
            EdgeType::RelatedTo => "related_to",
            EdgeType::Duplicates => "duplicates",
            EdgeType::Fixes => "fixes",
            EdgeType::CausedBy => "caused_by",
            EdgeType::Supersedes => "supersedes",
            EdgeType::ParentOf => "parent_of",
            EdgeType::ChildOf => "child_of",
            EdgeType::Tests => "tests",
        };
        write!(f, "{}", s)
    }
}

impl std::str::FromStr for EdgeType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "depends_on" => Ok(EdgeType::DependsOn),
            "blocks" => Ok(EdgeType::Blocks),
            "related_to" => Ok(EdgeType::RelatedTo),
            "duplicates" => Ok(EdgeType::Duplicates),
            "fixes" => Ok(EdgeType::Fixes),
            "caused_by" => Ok(EdgeType::CausedBy),
            "supersedes" => Ok(EdgeType::Supersedes),
            "parent_of" => Ok(EdgeType::ParentOf),
            "child_of" => Ok(EdgeType::ChildOf),
            "tests" => Ok(EdgeType::Tests),
            _ => Err(format!("Unknown edge type: {}", s)),
        }
    }
}

/// A relationship between two entities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    /// Unique identifier (e.g., "bne-a1b2")
    pub id: String,

    /// Entity type marker
    #[serde(rename = "type")]
    pub entity_type: String,

    /// Source entity ID (e.g., "bn-1234")
    pub source: String,

    /// Target entity ID (e.g., "bn-5678")
    pub target: String,

    /// Type of relationship
    pub edge_type: EdgeType,

    /// Weight for prioritization (default 1.0, reserved for future use)
    #[serde(default = "default_weight")]
    pub weight: f64,

    /// Reason for creating this relationship
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,

    /// When the edge was created
    pub created_at: DateTime<Utc>,

    /// Who created the edge (user or agent)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
}

fn default_weight() -> f64 {
    1.0
}

impl Edge {
    /// Create a new edge with the given source, target, and type.
    pub fn new(id: String, source: String, target: String, edge_type: EdgeType) -> Self {
        Self {
            id,
            entity_type: "edge".to_string(),
            source,
            target,
            edge_type,
            weight: 1.0,
            reason: None,
            created_at: Utc::now(),
            created_by: None,
        }
    }

    /// Returns true if this is a bidirectional edge.
    pub fn is_bidirectional(&self) -> bool {
        self.edge_type.is_bidirectional()
    }

    /// Returns true if this edge affects blocking/ready status.
    pub fn is_blocking(&self) -> bool {
        self.edge_type.is_blocking()
    }

    /// Create a flipped version of this edge (for bidirectional display).
    pub fn flip(&self) -> Edge {
        Edge {
            id: self.id.clone(),
            entity_type: self.entity_type.clone(),
            source: self.target.clone(),
            target: self.source.clone(),
            edge_type: self.edge_type,
            weight: self.weight,
            reason: self.reason.clone(),
            created_at: self.created_at,
            created_by: self.created_by.clone(),
        }
    }
}

/// Direction of an edge relative to a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeDirection {
    /// This node is the source
    Outbound,
    /// This node is the target
    Inbound,
    /// Bidirectional relationship
    Both,
}

/// An edge with direction info for display purposes.
#[derive(Debug, Clone)]
pub struct HydratedEdge {
    /// The underlying edge
    pub edge: Edge,
    /// Direction relative to the queried node
    pub direction: EdgeDirection,
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
        assert_eq!(bug.severity, BugSeverity::Triage);
    }

    #[test]
    fn test_bug_severity_serialization() {
        let severity = BugSeverity::High;
        let json = serde_json::to_string(&severity).unwrap();
        assert_eq!(json, r#""high""#);
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
    fn test_milestone_serialization_roundtrip() {
        let milestone = Milestone::new("bn-mile".to_string(), "Test milestone".to_string());
        let json = serde_json::to_string(&milestone).unwrap();
        let deserialized: Milestone = serde_json::from_str(&json).unwrap();
        assert_eq!(milestone.id, deserialized.id);
        assert_eq!(milestone.title, deserialized.title);
        assert_eq!(milestone.entity_type, "milestone");
    }

    #[test]
    fn test_milestone_default_values() {
        let json = r#"{"id":"bn-mile","type":"milestone","title":"M1","created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z"}"#;
        let milestone: Milestone = serde_json::from_str(json).unwrap();
        // serde(default) for u8 is 0; Milestone::new() uses 2 for creation
        assert_eq!(milestone.priority, 0);
        assert_eq!(milestone.status, TaskStatus::Pending);
        assert!(milestone.tags.is_empty());
    }

    #[test]
    fn test_milestone_progress_calculation() {
        // No items
        let progress = MilestoneProgress::new(0, 0);
        assert_eq!(progress.percentage, 0.0);

        // 3 of 5 done
        let progress = MilestoneProgress::new(5, 3);
        assert_eq!(progress.percentage, 60.0);

        // All done
        let progress = MilestoneProgress::new(4, 4);
        assert_eq!(progress.percentage, 100.0);
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

    #[test]
    fn test_edge_serialization_roundtrip() {
        let edge = Edge::new(
            "bne-test".to_string(),
            "bn-1234".to_string(),
            "bn-5678".to_string(),
            EdgeType::DependsOn,
        );
        let json = serde_json::to_string(&edge).unwrap();
        let deserialized: Edge = serde_json::from_str(&json).unwrap();
        assert_eq!(edge.id, deserialized.id);
        assert_eq!(edge.source, deserialized.source);
        assert_eq!(edge.target, deserialized.target);
        assert_eq!(edge.edge_type, deserialized.edge_type);
        assert_eq!(edge.entity_type, "edge");
    }

    #[test]
    fn test_edge_type_serialization() {
        let edge_type = EdgeType::DependsOn;
        let json = serde_json::to_string(&edge_type).unwrap();
        assert_eq!(json, r#""depends_on""#);

        let edge_type = EdgeType::RelatedTo;
        let json = serde_json::to_string(&edge_type).unwrap();
        assert_eq!(json, r#""related_to""#);
    }

    #[test]
    fn test_edge_type_from_str() {
        assert_eq!(
            "depends_on".parse::<EdgeType>().unwrap(),
            EdgeType::DependsOn
        );
        assert_eq!("blocks".parse::<EdgeType>().unwrap(), EdgeType::Blocks);
        assert_eq!(
            "related_to".parse::<EdgeType>().unwrap(),
            EdgeType::RelatedTo
        );
        assert_eq!(
            "duplicates".parse::<EdgeType>().unwrap(),
            EdgeType::Duplicates
        );
        assert_eq!("fixes".parse::<EdgeType>().unwrap(), EdgeType::Fixes);
        assert_eq!("caused_by".parse::<EdgeType>().unwrap(), EdgeType::CausedBy);
        assert_eq!(
            "supersedes".parse::<EdgeType>().unwrap(),
            EdgeType::Supersedes
        );
        assert_eq!("parent_of".parse::<EdgeType>().unwrap(), EdgeType::ParentOf);
        assert_eq!("child_of".parse::<EdgeType>().unwrap(), EdgeType::ChildOf);
        assert_eq!("tests".parse::<EdgeType>().unwrap(), EdgeType::Tests);
        assert!("invalid".parse::<EdgeType>().is_err());
    }

    #[test]
    fn test_edge_type_display() {
        assert_eq!(EdgeType::DependsOn.to_string(), "depends_on");
        assert_eq!(EdgeType::Blocks.to_string(), "blocks");
        assert_eq!(EdgeType::RelatedTo.to_string(), "related_to");
    }

    #[test]
    fn test_edge_type_is_bidirectional() {
        assert!(EdgeType::RelatedTo.is_bidirectional());
        assert!(!EdgeType::DependsOn.is_bidirectional());
        assert!(!EdgeType::Blocks.is_bidirectional());
        assert!(!EdgeType::Fixes.is_bidirectional());
    }

    #[test]
    fn test_edge_type_is_blocking() {
        assert!(EdgeType::DependsOn.is_blocking());
        assert!(EdgeType::Blocks.is_blocking());
        assert!(!EdgeType::RelatedTo.is_blocking());
        assert!(!EdgeType::Fixes.is_blocking());
    }

    #[test]
    fn test_edge_flip() {
        let edge = Edge::new(
            "bne-test".to_string(),
            "bn-1234".to_string(),
            "bn-5678".to_string(),
            EdgeType::RelatedTo,
        );
        let flipped = edge.flip();
        assert_eq!(flipped.source, "bn-5678");
        assert_eq!(flipped.target, "bn-1234");
        assert_eq!(flipped.edge_type, EdgeType::RelatedTo);
    }

    #[test]
    fn test_edge_default_weight() {
        let json = r#"{"id":"bne-test","type":"edge","source":"bn-1","target":"bn-2","edge_type":"depends_on","created_at":"2026-01-01T00:00:00Z"}"#;
        let edge: Edge = serde_json::from_str(json).unwrap();
        assert_eq!(edge.weight, 1.0);
    }

    #[test]
    fn test_edge_type_all() {
        let all = EdgeType::all();
        assert_eq!(all.len(), 10);
        assert!(all.contains(&EdgeType::DependsOn));
        assert!(all.contains(&EdgeType::Tests));
    }
}
