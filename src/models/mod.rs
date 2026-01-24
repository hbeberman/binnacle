//! Data models for Binnacle entities.
//!
//! This module defines the core data structures:
//! - `Task` - Work items with status, priority, dependencies
//! - `Bug` - Defects with severity, reproduction steps, and components
//! - `Milestone` - Collection of tasks/bugs with progress tracking
//! - `TestNode` - Test definitions linked to tasks
//! - `CommitLink` - Associations between commits and tasks
//! - `Edge` - Relationships between entities (dependencies, blocks, related, etc.)
//! - `Agent` - AI agent registration for lifecycle management

pub mod graph;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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

/// Idea status in the workflow.
/// Ideas have a distinct lifecycle from tasks - they start as seeds
/// and can be germinated, promoted to tasks/PRDs, or discarded.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdeaStatus {
    /// Just captured, raw thought
    #[default]
    Seed,
    /// Being fleshed out, gaining detail
    Germinating,
    /// Has graduated to a task or PRD
    Promoted,
    /// Decided not to pursue
    Discarded,
}

// =============================================================================
// EntityCore - Common fields for all primary entities
// =============================================================================

/// Common fields shared by all primary entity types (Task, Bug, Idea, Milestone).
///
/// Use `#[serde(flatten)]` when embedding in entity structs to maintain
/// flat JSON serialization. This struct reduces boilerplate when adding new
/// entity types or new common fields.
///
/// # Example
/// ```ignore
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// pub struct MyEntity {
///     #[serde(flatten)]
///     pub core: EntityCore,
///     // Entity-specific fields...
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityCore {
    /// Unique identifier (e.g., "bn-a1b2")
    pub id: String,

    /// Entity type marker (e.g., "task", "bug", "idea", "milestone")
    #[serde(rename = "type")]
    pub entity_type: String,

    /// Entity title
    pub title: String,

    /// Optional short display name (shown in GUI instead of ID)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_name: Option<String>,

    /// Detailed description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

impl EntityCore {
    /// Create a new EntityCore with the given type, ID, and title.
    ///
    /// Sets `created_at` and `updated_at` to now, and all optional fields to None/empty.
    pub fn new(entity_type: &str, id: String, title: String) -> Self {
        let now = Utc::now();
        Self {
            id,
            entity_type: entity_type.to_string(),
            title,
            short_name: None,
            description: None,
            tags: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }
}

// =============================================================================
// Entity Trait - Consistent interface for primary entities
// =============================================================================

/// Core trait that all primary entities must implement.
///
/// This trait ensures consistency across entity types (Task, Bug, Idea, Milestone).
/// If a new field like `short_name` is added to one entity, the compiler will
/// require it on all entities that implement this trait.
///
/// # Example
/// ```ignore
/// let task: &dyn Entity = &my_task;
/// println!("ID: {}, Title: {}", task.id(), task.title());
/// if let Some(name) = task.short_name() {
///     println!("Short name: {}", name);
/// }
/// ```
pub trait Entity {
    /// Returns the unique identifier (e.g., "bn-a1b2").
    fn id(&self) -> &str;

    /// Returns the entity type string (e.g., "task", "bug", "idea", "milestone").
    fn entity_type(&self) -> &str;

    /// Returns the entity's title.
    fn title(&self) -> &str;

    /// Returns the optional short display name.
    fn short_name(&self) -> Option<&str>;

    /// Returns the optional description.
    fn description(&self) -> Option<&str>;

    /// Returns the creation timestamp.
    fn created_at(&self) -> DateTime<Utc>;

    /// Returns the last update timestamp.
    fn updated_at(&self) -> DateTime<Utc>;

    /// Returns the tags for this entity.
    fn tags(&self) -> &[String];
}

impl Entity for EntityCore {
    fn id(&self) -> &str {
        &self.id
    }
    fn entity_type(&self) -> &str {
        &self.entity_type
    }
    fn title(&self) -> &str {
        &self.title
    }
    fn short_name(&self) -> Option<&str> {
        self.short_name.as_deref()
    }
    fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
    fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
    fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }
    fn tags(&self) -> &[String] {
        &self.tags
    }
}

// =============================================================================
// Task
// =============================================================================

/// A work item tracked by Binnacle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Common entity fields (id, type, title, short_name, description, tags, timestamps)
    #[serde(flatten)]
    pub core: EntityCore,

    /// Priority level (0-4, lower is higher priority)
    #[serde(default)]
    pub priority: u8,

    /// Current status
    #[serde(default)]
    pub status: TaskStatus,

    /// Parent task ID for hierarchical organization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,

    /// Assigned user or agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,

    /// Task IDs this task depends on
    #[serde(default)]
    pub depends_on: Vec<String>,

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
        Self {
            core: EntityCore::new("task", id, title),
            priority: 2, // Default middle priority
            status: TaskStatus::default(),
            parent: None,
            assignee: None,
            depends_on: Vec::new(),
            closed_at: None,
            closed_reason: None,
            imported_on: None,
        }
    }
}

impl Entity for Task {
    fn id(&self) -> &str {
        self.core.id()
    }
    fn entity_type(&self) -> &str {
        self.core.entity_type()
    }
    fn title(&self) -> &str {
        self.core.title()
    }
    fn short_name(&self) -> Option<&str> {
        self.core.short_name()
    }
    fn description(&self) -> Option<&str> {
        self.core.description()
    }
    fn created_at(&self) -> DateTime<Utc> {
        self.core.created_at()
    }
    fn updated_at(&self) -> DateTime<Utc> {
        self.core.updated_at()
    }
    fn tags(&self) -> &[String] {
        self.core.tags()
    }
}

// =============================================================================
// Bug
// =============================================================================

/// A defect tracked by Binnacle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bug {
    /// Common entity fields (id, type, title, short_name, description, tags, timestamps)
    #[serde(flatten)]
    pub core: EntityCore,

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

    /// Assigned user or agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,

    /// IDs this bug depends on
    #[serde(default)]
    pub depends_on: Vec<String>,

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
        Self {
            core: EntityCore::new("bug", id, title),
            priority: 2,
            status: TaskStatus::default(),
            severity: BugSeverity::default(),
            reproduction_steps: None,
            affected_component: None,
            assignee: None,
            depends_on: Vec::new(),
            closed_at: None,
            closed_reason: None,
        }
    }
}

impl Entity for Bug {
    fn id(&self) -> &str {
        self.core.id()
    }
    fn entity_type(&self) -> &str {
        self.core.entity_type()
    }
    fn title(&self) -> &str {
        self.core.title()
    }
    fn short_name(&self) -> Option<&str> {
        self.core.short_name()
    }
    fn description(&self) -> Option<&str> {
        self.core.description()
    }
    fn created_at(&self) -> DateTime<Utc> {
        self.core.created_at()
    }
    fn updated_at(&self) -> DateTime<Utc> {
        self.core.updated_at()
    }
    fn tags(&self) -> &[String] {
        self.core.tags()
    }
}

/// A low-stakes idea or rough concept tracked by Binnacle.
/// Ideas are distinct from tasks - they represent early-stage notions
/// that can be captured quickly and potentially grown into full PRDs or tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Idea {
    /// Common entity fields (id, type, title, short_name, description, tags, timestamps)
    #[serde(flatten)]
    pub core: EntityCore,

    /// Current status
    #[serde(default)]
    pub status: IdeaStatus,

    /// Task ID if promoted (e.g., "bn-a1b2") or PRD path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub promoted_to: Option<String>,
}

impl Idea {
    /// Create a new idea with the given ID and title.
    pub fn new(id: String, title: String) -> Self {
        Self {
            core: EntityCore::new("idea", id, title),
            status: IdeaStatus::default(),
            promoted_to: None,
        }
    }
}

impl Entity for Idea {
    fn id(&self) -> &str {
        self.core.id()
    }
    fn entity_type(&self) -> &str {
        self.core.entity_type()
    }
    fn title(&self) -> &str {
        self.core.title()
    }
    fn short_name(&self) -> Option<&str> {
        self.core.short_name()
    }
    fn description(&self) -> Option<&str> {
        self.core.description()
    }
    fn created_at(&self) -> DateTime<Utc> {
        self.core.created_at()
    }
    fn updated_at(&self) -> DateTime<Utc> {
        self.core.updated_at()
    }
    fn tags(&self) -> &[String] {
        self.core.tags()
    }
}

/// A documentation node for storing markdown content linked to entities.
/// Docs provide a way to attach rich documentation to any entity in the graph.
/// They use a separate ID format (bnd-a1b2) to distinguish from other entities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Doc {
    /// Common entity fields (id, type, title, short_name, description, tags, timestamps)
    /// Note: For Doc, the `description` field in EntityCore is used for brief summaries,
    /// while `content` holds the full markdown documentation.
    #[serde(flatten)]
    pub core: EntityCore,

    /// Full markdown content of the documentation
    #[serde(default)]
    pub content: String,
}

impl Doc {
    /// Create a new doc with the given ID and title.
    pub fn new(id: String, title: String) -> Self {
        Self {
            core: EntityCore::new("doc", id, title),
            content: String::new(),
        }
    }

    /// Create a new doc with content.
    pub fn with_content(id: String, title: String, content: String) -> Self {
        Self {
            core: EntityCore::new("doc", id, title),
            content,
        }
    }
}

impl Entity for Doc {
    fn id(&self) -> &str {
        self.core.id()
    }
    fn entity_type(&self) -> &str {
        self.core.entity_type()
    }
    fn title(&self) -> &str {
        self.core.title()
    }
    fn short_name(&self) -> Option<&str> {
        self.core.short_name()
    }
    fn description(&self) -> Option<&str> {
        self.core.description()
    }
    fn created_at(&self) -> DateTime<Utc> {
        self.core.created_at()
    }
    fn updated_at(&self) -> DateTime<Utc> {
        self.core.updated_at()
    }
    fn tags(&self) -> &[String] {
        self.core.tags()
    }
}

/// A milestone for grouping and tracking progress of tasks and bugs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestone {
    /// Common entity fields (id, type, title, short_name, description, tags, timestamps)
    #[serde(flatten)]
    pub core: EntityCore,

    /// Priority level (0-4, lower is higher priority)
    #[serde(default)]
    pub priority: u8,

    /// Current status
    #[serde(default)]
    pub status: TaskStatus,

    /// Target completion date
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_date: Option<DateTime<Utc>>,

    /// Assigned user or agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,

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
        Self {
            core: EntityCore::new("milestone", id, title),
            priority: 2,
            status: TaskStatus::default(),
            due_date: None,
            assignee: None,
            closed_at: None,
            closed_reason: None,
        }
    }
}

impl Entity for Milestone {
    fn id(&self) -> &str {
        self.core.id()
    }
    fn entity_type(&self) -> &str {
        self.core.entity_type()
    }
    fn title(&self) -> &str {
        self.core.title()
    }
    fn short_name(&self) -> Option<&str> {
        self.core.short_name()
    }
    fn description(&self) -> Option<&str> {
        self.core.description()
    }
    fn created_at(&self) -> DateTime<Utc> {
        self.core.created_at()
    }
    fn updated_at(&self) -> DateTime<Utc> {
        self.core.updated_at()
    }
    fn tags(&self) -> &[String] {
        self.core.tags()
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

    /// Bug IDs this test is linked to (for verifying bug fixes)
    #[serde(default)]
    pub linked_bugs: Vec<String>,

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
            linked_bugs: Vec::new(),
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

/// Association between a commit and a task or bug.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitLink {
    /// Git commit SHA
    pub sha: String,

    /// Entity ID (task or bug)
    /// Serializes as "task_id" for backward compatibility with existing data.
    #[serde(alias = "entity_id")]
    pub task_id: String,

    /// When the link was created
    pub linked_at: DateTime<Utc>,
}

/// Agent status for lifecycle management.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    /// Agent is actively running commands
    #[default]
    Active,
    /// Agent hasn't run commands recently
    Idle,
    /// Agent process appears to have exited or is unresponsive
    Stale,
}

/// Agent type for categorizing agent roles.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentType {
    /// Worker agents execute tasks and make code changes
    #[default]
    Worker,
    /// Planner agents create PRDs, break down features, and organize work
    Planner,
    /// Buddy agents assist humans with code review, questions, and guidance
    Buddy,
}

/// An AI agent registered with Binnacle for lifecycle management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    /// Unique identifier (e.g., "bna-a1b2")
    /// Generated from PID and start time for uniqueness
    /// For backward compatibility, defaults to a placeholder that gets replaced on registration
    #[serde(default = "agent_placeholder_id")]
    pub id: String,

    /// Entity type marker
    #[serde(rename = "type", default = "agent_entity_type")]
    pub entity_type: String,

    /// Process ID of the agent
    pub pid: u32,

    /// Parent process ID (e.g., the shell or terminal running the agent)
    pub parent_pid: u32,

    /// Agent name (e.g., "claude", "copilot", or custom name)
    pub name: String,

    /// Agent type (worker, planner, buddy)
    #[serde(default)]
    pub agent_type: AgentType,

    /// Agent's purpose/role (e.g., "Task Worker", "PRD Generator")
    /// Agents that don't register a purpose are labeled "UNREGISTERED"
    #[serde(default)]
    pub purpose: Option<String>,

    /// When the agent was registered
    pub started_at: DateTime<Utc>,

    /// When the agent last ran a binnacle command
    pub last_activity_at: DateTime<Utc>,

    /// Task IDs the agent is currently working on
    #[serde(default)]
    pub tasks: Vec<String>,

    /// Number of binnacle commands the agent has run
    #[serde(default)]
    pub command_count: u64,

    /// Current status of the agent
    #[serde(default)]
    pub status: AgentStatus,
}

fn agent_entity_type() -> String {
    "agent".to_string()
}

fn agent_placeholder_id() -> String {
    // Placeholder ID for backward compatibility during deserialization
    // Gets replaced with a proper bna-xxxx ID when calling ensure_id()
    String::new()
}

impl Agent {
    /// Generate a unique agent ID from PID and timestamp.
    fn generate_id(pid: u32, started_at: &DateTime<Utc>) -> String {
        let seed = format!("{}:{}", pid, started_at.timestamp_nanos_opt().unwrap_or(0));
        let mut hasher = Sha256::new();
        hasher.update(seed.as_bytes());
        let hash = hasher.finalize();
        let hash_hex = format!("{:x}", hash);
        format!("bna-{}", &hash_hex[..4])
    }

    /// Create a new agent with the given PID, name, and type.
    pub fn new(pid: u32, parent_pid: u32, name: String, agent_type: AgentType) -> Self {
        let now = Utc::now();
        let id = Self::generate_id(pid, &now);
        Self {
            id,
            entity_type: "agent".to_string(),
            pid,
            parent_pid,
            name,
            agent_type,
            purpose: None,
            started_at: now,
            last_activity_at: now,
            tasks: Vec::new(),
            command_count: 0,
            status: AgentStatus::default(),
        }
    }

    /// Create a new agent with a purpose.
    pub fn new_with_purpose(
        pid: u32,
        parent_pid: u32,
        name: String,
        agent_type: AgentType,
        purpose: String,
    ) -> Self {
        let now = Utc::now();
        let id = Self::generate_id(pid, &now);
        Self {
            id,
            entity_type: "agent".to_string(),
            pid,
            parent_pid,
            name,
            agent_type,
            purpose: Some(purpose),
            started_at: now,
            last_activity_at: now,
            tasks: Vec::new(),
            command_count: 0,
            status: AgentStatus::default(),
        }
    }

    /// Returns the display purpose - "UNREGISTERED" if no purpose was provided.
    pub fn display_purpose(&self) -> &str {
        self.purpose.as_deref().unwrap_or("UNREGISTERED")
    }

    /// Returns true if the agent has registered a purpose.
    pub fn is_registered(&self) -> bool {
        self.purpose.is_some()
    }

    /// Ensure the agent has a valid binnacle ID.
    /// For backward compatibility with agents deserialized from old format.
    pub fn ensure_id(&mut self) {
        if self.id.is_empty() {
            self.id = Self::generate_id(self.pid, &self.started_at);
        }
    }

    /// Update the agent's last activity timestamp.
    pub fn touch(&mut self) {
        self.last_activity_at = Utc::now();
        self.command_count += 1;
    }

    /// Check if the agent process is still alive.
    #[cfg(unix)]
    pub fn is_alive(&self) -> bool {
        use std::path::Path;
        // On Linux/Unix, check if /proc/<pid> exists
        Path::new(&format!("/proc/{}", self.pid)).exists()
    }

    #[cfg(not(unix))]
    pub fn is_alive(&self) -> bool {
        // On non-Unix systems, assume alive (conservative)
        true
    }
}

/// Session state for commit-msg hook detection.
/// Written to session.json in the storage directory when `bn orient` is called.
/// Used by git hooks to detect active agent sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    /// Process ID of the agent (parent PID of bn process)
    pub agent_pid: u32,

    /// Type of agent (worker, planner, buddy)
    pub agent_type: AgentType,

    /// When the session started (orient was called)
    pub started_at: DateTime<Utc>,

    /// Whether orient was called in this session
    pub orient_called: bool,
}

impl SessionState {
    /// Create a new session state.
    pub fn new(agent_pid: u32, agent_type: AgentType) -> Self {
        Self {
            agent_pid,
            agent_type,
            started_at: Utc::now(),
            orient_called: true,
        }
    }
}

/// A work pool for agent task prioritization.
/// Only one queue can exist per repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Queue {
    /// Unique identifier (e.g., "bnq-a1b2")
    pub id: String,

    /// Entity type marker
    #[serde(rename = "type")]
    pub entity_type: String,

    /// Queue title (e.g., "Sprint 1", "Urgent")
    pub title: String,

    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

impl Queue {
    /// Create a new queue with the given ID and title.
    pub fn new(id: String, title: String) -> Self {
        let now = Utc::now();
        Self {
            id,
            entity_type: "queue".to_string(),
            title,
            description: None,
            created_at: now,
            updated_at: now,
        }
    }
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
    /// Task is in the queue for prioritized work (Task → Queue)
    Queued,
    /// Bug impacts this entity (Bug → Task/PRD/Milestone)
    Impacts,
    /// Agent is working on this task/bug (Agent → Task/Bug)
    WorkingOn,
    /// Agent previously worked on this task/bug (Agent → Task/Bug)
    WorkedOn,
    /// Doc provides documentation for this entity (Doc → Any)
    Documents,
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
            EdgeType::Queued,
            EdgeType::Impacts,
            EdgeType::WorkingOn,
            EdgeType::WorkedOn,
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
            EdgeType::Queued => "queued",
            EdgeType::Impacts => "impacts",
            EdgeType::WorkingOn => "working_on",
            EdgeType::WorkedOn => "worked_on",
            EdgeType::Documents => "documents",
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
            "queued" => Ok(EdgeType::Queued),
            "impacts" => Ok(EdgeType::Impacts),
            "working_on" => Ok(EdgeType::WorkingOn),
            "worked_on" => Ok(EdgeType::WorkedOn),
            "documents" => Ok(EdgeType::Documents),
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
        assert_eq!(task.core.id, deserialized.core.id);
        assert_eq!(task.core.title, deserialized.core.title);
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
        assert_eq!(bug.core.id, deserialized.core.id);
        assert_eq!(bug.core.title, deserialized.core.title);
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
    fn test_bug_backward_compatibility() {
        // This test verifies that pre-refactor Bug JSON (flat fields) parses correctly
        // into the new EntityCore-based structure with #[serde(flatten)]
        let pre_refactor_json = r#"{
            "id": "bn-legacy",
            "type": "bug",
            "title": "Legacy Bug Title",
            "short_name": "legacy-bug",
            "description": "A bug created before EntityCore refactoring",
            "tags": ["urgent", "backend"],
            "priority": 1,
            "status": "in_progress",
            "severity": "high",
            "reproduction_steps": "1. Do X\n2. See Y",
            "affected_component": "api/auth",
            "assignee": "alice",
            "depends_on": ["bn-other"],
            "created_at": "2026-01-15T10:30:00Z",
            "updated_at": "2026-01-16T14:45:00Z",
            "closed_at": null,
            "closed_reason": null
        }"#;

        let bug: Bug = serde_json::from_str(pre_refactor_json).unwrap();

        // Verify EntityCore fields are correctly populated
        assert_eq!(bug.core.id, "bn-legacy");
        assert_eq!(bug.core.entity_type, "bug");
        assert_eq!(bug.core.title, "Legacy Bug Title");
        assert_eq!(bug.core.short_name, Some("legacy-bug".to_string()));
        assert_eq!(
            bug.core.description,
            Some("A bug created before EntityCore refactoring".to_string())
        );
        assert_eq!(bug.core.tags, vec!["urgent", "backend"]);
        assert!(bug.core.created_at.to_rfc3339().starts_with("2026-01-15"));
        assert!(bug.core.updated_at.to_rfc3339().starts_with("2026-01-16"));

        // Verify Bug-specific fields are correctly populated
        assert_eq!(bug.priority, 1);
        assert_eq!(bug.status, TaskStatus::InProgress);
        assert_eq!(bug.severity, BugSeverity::High);
        assert_eq!(
            bug.reproduction_steps,
            Some("1. Do X\n2. See Y".to_string())
        );
        assert_eq!(bug.affected_component, Some("api/auth".to_string()));
        assert_eq!(bug.assignee, Some("alice".to_string()));
        assert_eq!(bug.depends_on, vec!["bn-other"]);
        assert!(bug.closed_at.is_none());
        assert!(bug.closed_reason.is_none());

        // Verify serialization back to JSON maintains flat structure
        let reserialized = serde_json::to_string(&bug).unwrap();
        let reparsed: serde_json::Value = serde_json::from_str(&reserialized).unwrap();

        // The JSON should have flat fields, not nested under "core"
        assert!(reparsed.get("id").is_some());
        assert!(reparsed.get("title").is_some());
        assert!(reparsed.get("core").is_none()); // Should NOT have a "core" field
    }

    #[test]
    fn test_task_backward_compatibility() {
        // This test verifies that pre-refactor Task JSON (flat fields) parses correctly
        // into the new EntityCore-based structure with #[serde(flatten)]
        let pre_refactor_json = r#"{
            "id": "bn-task1",
            "type": "task",
            "title": "Legacy Task Title",
            "short_name": "legacy-task",
            "description": "A task created before EntityCore refactoring",
            "tags": ["feature", "v2"],
            "priority": 0,
            "status": "in_progress",
            "parent": "bn-parent",
            "assignee": "bob",
            "depends_on": ["bn-dep1", "bn-dep2"],
            "created_at": "2026-01-10T08:00:00Z",
            "updated_at": "2026-01-12T16:30:00Z",
            "closed_at": null,
            "closed_reason": null,
            "imported_on": null
        }"#;

        let task: Task = serde_json::from_str(pre_refactor_json).unwrap();

        // Verify EntityCore fields
        assert_eq!(task.core.id, "bn-task1");
        assert_eq!(task.core.entity_type, "task");
        assert_eq!(task.core.title, "Legacy Task Title");
        assert_eq!(task.core.short_name, Some("legacy-task".to_string()));
        assert_eq!(
            task.core.description,
            Some("A task created before EntityCore refactoring".to_string())
        );
        assert_eq!(task.core.tags, vec!["feature", "v2"]);

        // Verify Task-specific fields
        assert_eq!(task.priority, 0);
        assert_eq!(task.status, TaskStatus::InProgress);
        assert_eq!(task.parent, Some("bn-parent".to_string()));
        assert_eq!(task.assignee, Some("bob".to_string()));
        assert_eq!(task.depends_on, vec!["bn-dep1", "bn-dep2"]);

        // Verify round-trip maintains flat structure
        let reserialized = serde_json::to_string(&task).unwrap();
        let reparsed: serde_json::Value = serde_json::from_str(&reserialized).unwrap();
        assert!(reparsed.get("id").is_some());
        assert!(reparsed.get("core").is_none());
    }

    #[test]
    fn test_idea_backward_compatibility() {
        // This test verifies that pre-refactor Idea JSON (flat fields) parses correctly
        let pre_refactor_json = r#"{
            "id": "bn-idea1",
            "type": "idea",
            "title": "Legacy Idea Title",
            "description": "An idea captured before refactoring",
            "tags": ["ux", "research"],
            "status": "germinating",
            "promoted_to": null,
            "created_at": "2026-01-05T14:00:00Z",
            "updated_at": "2026-01-06T09:15:00Z"
        }"#;

        let idea: Idea = serde_json::from_str(pre_refactor_json).unwrap();

        // Verify EntityCore fields
        assert_eq!(idea.core.id, "bn-idea1");
        assert_eq!(idea.core.entity_type, "idea");
        assert_eq!(idea.core.title, "Legacy Idea Title");
        assert_eq!(
            idea.core.description,
            Some("An idea captured before refactoring".to_string())
        );
        assert_eq!(idea.core.tags, vec!["ux", "research"]);

        // Verify Idea-specific fields
        assert_eq!(idea.status, IdeaStatus::Germinating);
        assert!(idea.promoted_to.is_none());

        // Verify round-trip maintains flat structure
        let reserialized = serde_json::to_string(&idea).unwrap();
        let reparsed: serde_json::Value = serde_json::from_str(&reserialized).unwrap();
        assert!(reparsed.get("id").is_some());
        assert!(reparsed.get("core").is_none());
    }

    #[test]
    fn test_milestone_backward_compatibility() {
        // This test verifies that pre-refactor Milestone JSON (flat fields) parses correctly
        let pre_refactor_json = r#"{
            "id": "bn-mile1",
            "type": "milestone",
            "title": "Legacy Milestone",
            "description": "Q1 2026 Release",
            "tags": ["release", "q1"],
            "priority": 1,
            "status": "pending",
            "due_date": "2026-03-31T23:59:59Z",
            "assignee": "team-lead",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-02T12:00:00Z",
            "closed_at": null,
            "closed_reason": null
        }"#;

        let milestone: Milestone = serde_json::from_str(pre_refactor_json).unwrap();

        // Verify EntityCore fields
        assert_eq!(milestone.core.id, "bn-mile1");
        assert_eq!(milestone.core.entity_type, "milestone");
        assert_eq!(milestone.core.title, "Legacy Milestone");
        assert_eq!(
            milestone.core.description,
            Some("Q1 2026 Release".to_string())
        );
        assert_eq!(milestone.core.tags, vec!["release", "q1"]);

        // Verify Milestone-specific fields
        assert_eq!(milestone.priority, 1);
        assert_eq!(milestone.status, TaskStatus::Pending);
        assert!(milestone.due_date.is_some());
        assert_eq!(milestone.assignee, Some("team-lead".to_string()));

        // Verify round-trip maintains flat structure
        let reserialized = serde_json::to_string(&milestone).unwrap();
        let reparsed: serde_json::Value = serde_json::from_str(&reserialized).unwrap();
        assert!(reparsed.get("id").is_some());
        assert!(reparsed.get("core").is_none());
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
    fn test_idea_serialization_roundtrip() {
        let idea = Idea::new("bn-test".to_string(), "Test idea".to_string());
        let json = serde_json::to_string(&idea).unwrap();
        let deserialized: Idea = serde_json::from_str(&json).unwrap();
        assert_eq!(idea.core.id, deserialized.core.id);
        assert_eq!(idea.core.title, deserialized.core.title);
        assert_eq!(idea.core.entity_type, "idea");
        assert_eq!(idea.status, IdeaStatus::Seed);
    }

    #[test]
    fn test_idea_status_serialization() {
        let status = IdeaStatus::Germinating;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, r#""germinating""#);

        let status = IdeaStatus::Promoted;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, r#""promoted""#);

        let status = IdeaStatus::Discarded;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, r#""discarded""#);
    }

    #[test]
    fn test_idea_default_values() {
        let json = r#"{"id":"bn-test","type":"idea","title":"Test","created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z"}"#;
        let idea: Idea = serde_json::from_str(json).unwrap();
        assert_eq!(idea.status, IdeaStatus::Seed);
        assert!(idea.core.tags.is_empty());
        assert!(idea.promoted_to.is_none());
    }

    #[test]
    fn test_milestone_serialization_roundtrip() {
        let milestone = Milestone::new("bn-mile".to_string(), "Test milestone".to_string());
        let json = serde_json::to_string(&milestone).unwrap();
        let deserialized: Milestone = serde_json::from_str(&json).unwrap();
        assert_eq!(milestone.core.id, deserialized.core.id);
        assert_eq!(milestone.core.title, deserialized.core.title);
        assert_eq!(milestone.core.entity_type, "milestone");
    }

    #[test]
    fn test_milestone_default_values() {
        let json = r#"{"id":"bn-mile","type":"milestone","title":"M1","created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z"}"#;
        let milestone: Milestone = serde_json::from_str(json).unwrap();
        // serde(default) for u8 is 0; Milestone::new() uses 2 for creation
        assert_eq!(milestone.priority, 0);
        assert_eq!(milestone.status, TaskStatus::Pending);
        assert!(milestone.core.tags.is_empty());
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
        assert_eq!(all.len(), 14);
        assert!(all.contains(&EdgeType::DependsOn));
        assert!(all.contains(&EdgeType::Tests));
        assert!(all.contains(&EdgeType::Queued));
        assert!(all.contains(&EdgeType::Impacts));
        assert!(all.contains(&EdgeType::WorkingOn));
        assert!(all.contains(&EdgeType::WorkedOn));
    }

    #[test]
    fn test_queue_serialization_roundtrip() {
        let queue = Queue::new("bnq-test".to_string(), "Sprint 1".to_string());
        let json = serde_json::to_string(&queue).unwrap();
        let deserialized: Queue = serde_json::from_str(&json).unwrap();
        assert_eq!(queue.id, deserialized.id);
        assert_eq!(queue.title, deserialized.title);
        assert_eq!(queue.entity_type, "queue");
    }

    #[test]
    fn test_queue_new() {
        let queue = Queue::new("bnq-a1b2".to_string(), "Urgent Work".to_string());
        assert_eq!(queue.id, "bnq-a1b2");
        assert_eq!(queue.title, "Urgent Work");
        assert_eq!(queue.entity_type, "queue");
        assert!(queue.description.is_none());
    }

    #[test]
    fn test_queue_default_values() {
        let json = r#"{"id":"bnq-test","type":"queue","title":"Q1","created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z"}"#;
        let queue: Queue = serde_json::from_str(json).unwrap();
        assert_eq!(queue.id, "bnq-test");
        assert_eq!(queue.title, "Q1");
        assert!(queue.description.is_none());
    }

    #[test]
    fn test_queued_edge_type() {
        let edge_type = EdgeType::Queued;
        assert!(!edge_type.is_bidirectional());
        assert!(!edge_type.is_blocking());
        assert_eq!(edge_type.to_string(), "queued");

        let parsed: EdgeType = "queued".parse().unwrap();
        assert_eq!(parsed, EdgeType::Queued);
    }

    #[test]
    fn test_agent_new() {
        let agent = Agent::new(1234, 1000, "test-agent".to_string(), AgentType::Worker);
        assert_eq!(agent.pid, 1234);
        assert_eq!(agent.parent_pid, 1000);
        assert_eq!(agent.name, "test-agent");
        assert_eq!(agent.agent_type, AgentType::Worker);
        assert!(agent.tasks.is_empty());
        assert_eq!(agent.command_count, 0);
        assert_eq!(agent.status, AgentStatus::Active);
    }

    #[test]
    fn test_agent_serialization_roundtrip() {
        let agent = Agent::new(1234, 1000, "test-agent".to_string(), AgentType::Planner);
        let json = serde_json::to_string(&agent).unwrap();
        let deserialized: Agent = serde_json::from_str(&json).unwrap();
        assert_eq!(agent.pid, deserialized.pid);
        assert_eq!(agent.parent_pid, deserialized.parent_pid);
        assert_eq!(agent.name, deserialized.name);
        assert_eq!(agent.agent_type, deserialized.agent_type);
        assert_eq!(agent.status, deserialized.status);
    }

    #[test]
    fn test_agent_status_serialization() {
        let status = AgentStatus::Active;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, r#""active""#);

        let status = AgentStatus::Idle;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, r#""idle""#);

        let status = AgentStatus::Stale;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, r#""stale""#);
    }

    #[test]
    fn test_agent_touch() {
        let mut agent = Agent::new(1234, 1000, "test-agent".to_string(), AgentType::Worker);
        let initial_time = agent.last_activity_at;
        assert_eq!(agent.command_count, 0);

        // Small delay to ensure time changes
        std::thread::sleep(std::time::Duration::from_millis(10));
        agent.touch();

        assert_eq!(agent.command_count, 1);
        assert!(agent.last_activity_at >= initial_time);
    }

    #[test]
    fn test_agent_default_values() {
        let json = r#"{"pid":1234,"parent_pid":1000,"name":"test","started_at":"2026-01-01T00:00:00Z","last_activity_at":"2026-01-01T00:00:00Z"}"#;
        let mut agent: Agent = serde_json::from_str(json).unwrap();
        assert!(agent.tasks.is_empty());
        assert_eq!(agent.command_count, 0);
        assert_eq!(agent.status, AgentStatus::Active);
        // ID should be empty before ensure_id
        assert!(agent.id.is_empty());
        // After ensure_id, it should have a proper bna-xxxx ID
        agent.ensure_id();
        assert!(agent.id.starts_with("bna-"));
    }

    #[test]
    fn test_session_state_new() {
        let state = SessionState::new(1234, AgentType::Worker);
        assert_eq!(state.agent_pid, 1234);
        assert_eq!(state.agent_type, AgentType::Worker);
        assert!(state.orient_called);
    }

    #[test]
    fn test_session_state_serialization_roundtrip() {
        let state = SessionState::new(5678, AgentType::Planner);
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: SessionState = serde_json::from_str(&json).unwrap();
        assert_eq!(state.agent_pid, deserialized.agent_pid);
        assert_eq!(state.agent_type, deserialized.agent_type);
        assert_eq!(state.orient_called, deserialized.orient_called);
    }

    #[test]
    fn test_session_state_json_format() {
        let state = SessionState::new(9999, AgentType::Buddy);
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("\"agent_pid\":9999"));
        assert!(json.contains("\"agent_type\":\"buddy\""));
        assert!(json.contains("\"orient_called\":true"));
        assert!(json.contains("\"started_at\""));
    }

    // ==========================================================================
    // SCHEMA FINGERPRINT TEST
    //
    // This test catches accidental schema changes in serialized data models.
    // When fields are added/removed/renamed, this test WILL FAIL.
    //
    // If this test fails after you modified a model:
    // 1. VERIFY the change is intentional
    // 2. CONSIDER backwards compatibility (can old data still be read?)
    // 3. UPDATE the expected fingerprint below
    // 4. DOCUMENT the schema change in your commit message
    //
    // The fingerprint is a sorted list of all JSON keys that appear when
    // serializing each model with all optional fields populated.
    // ==========================================================================

    /// Extract all JSON keys from a serialized value, sorted alphabetically.
    fn extract_json_keys(json: &str) -> Vec<String> {
        let value: serde_json::Value = serde_json::from_str(json).unwrap();
        let mut keys = Vec::new();
        collect_keys(&value, "", &mut keys);
        keys.sort();
        keys
    }

    fn collect_keys(value: &serde_json::Value, prefix: &str, keys: &mut Vec<String>) {
        if let serde_json::Value::Object(map) = value {
            for (k, v) in map {
                let full_key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{}.{}", prefix, k)
                };
                keys.push(full_key.clone());
                collect_keys(v, &full_key, keys);
            }
        }
    }

    /// Create a fingerprint string from a list of keys.
    fn fingerprint(keys: &[String]) -> String {
        keys.join("|")
    }

    #[test]
    fn test_schema_fingerprint_task() {
        // Create a Task with ALL optional fields populated
        let mut task = super::Task::new("bn-test".to_string(), "Test Task".to_string());
        task.core.short_name = Some("short".to_string());
        task.core.description = Some("desc".to_string());
        task.core.tags = vec!["tag1".to_string()];
        task.parent = Some("bn-parent".to_string());
        task.assignee = Some("user".to_string());
        task.depends_on = vec!["bn-dep".to_string()];
        task.closed_at = Some(chrono::Utc::now());
        task.closed_reason = Some("done".to_string());
        task.imported_on = Some(chrono::Utc::now());

        let json = serde_json::to_string(&task).unwrap();
        let keys = extract_json_keys(&json);
        let fp = fingerprint(&keys);

        // Expected schema fingerprint for Task
        // If this fails, you've changed the Task schema - see comment above!
        let expected = "assignee|closed_at|closed_reason|created_at|depends_on|description|id|imported_on|parent|priority|short_name|status|tags|title|type|updated_at";
        assert_eq!(
            fp, expected,
            "Task schema changed! Update expected fingerprint if intentional."
        );
    }

    #[test]
    fn test_schema_fingerprint_bug() {
        let mut bug = super::Bug::new("bn-bug".to_string(), "Test Bug".to_string());
        bug.core.short_name = Some("short".to_string());
        bug.core.description = Some("desc".to_string());
        bug.core.tags = vec!["tag1".to_string()];
        bug.reproduction_steps = Some("steps".to_string());
        bug.affected_component = Some("component".to_string());
        bug.assignee = Some("user".to_string());
        bug.depends_on = vec!["bn-dep".to_string()];
        bug.closed_at = Some(chrono::Utc::now());
        bug.closed_reason = Some("fixed".to_string());

        let json = serde_json::to_string(&bug).unwrap();
        let keys = extract_json_keys(&json);
        let fp = fingerprint(&keys);

        let expected = "affected_component|assignee|closed_at|closed_reason|created_at|depends_on|description|id|priority|reproduction_steps|severity|short_name|status|tags|title|type|updated_at";
        assert_eq!(
            fp, expected,
            "Bug schema changed! Update expected fingerprint if intentional."
        );
    }

    #[test]
    fn test_schema_fingerprint_idea() {
        let mut idea = super::Idea::new("bn-idea".to_string(), "Test Idea".to_string());
        idea.core.short_name = Some("short".to_string());
        idea.core.description = Some("desc".to_string());
        idea.core.tags = vec!["tag1".to_string()];
        idea.promoted_to = Some("bn-task".to_string());

        let json = serde_json::to_string(&idea).unwrap();
        let keys = extract_json_keys(&json);
        let fp = fingerprint(&keys);

        let expected =
            "created_at|description|id|promoted_to|short_name|status|tags|title|type|updated_at";
        assert_eq!(
            fp, expected,
            "Idea schema changed! Update expected fingerprint if intentional."
        );
    }

    #[test]
    fn test_schema_fingerprint_milestone() {
        let mut milestone =
            super::Milestone::new("bn-ms".to_string(), "Test Milestone".to_string());
        milestone.core.short_name = Some("short".to_string());
        milestone.core.description = Some("desc".to_string());
        milestone.core.tags = vec!["tag1".to_string()];
        milestone.due_date = Some(chrono::Utc::now());
        milestone.assignee = Some("user".to_string());
        milestone.closed_at = Some(chrono::Utc::now());
        milestone.closed_reason = Some("completed".to_string());

        let json = serde_json::to_string(&milestone).unwrap();
        let keys = extract_json_keys(&json);
        let fp = fingerprint(&keys);

        let expected = "assignee|closed_at|closed_reason|created_at|description|due_date|id|priority|short_name|status|tags|title|type|updated_at";
        assert_eq!(
            fp, expected,
            "Milestone schema changed! Update expected fingerprint if intentional."
        );
    }

    #[test]
    fn test_schema_fingerprint_test_node() {
        let mut test_node = super::TestNode::new(
            "bnt-test".to_string(),
            "Test Node".to_string(),
            "cargo test".to_string(),
        );
        test_node.pattern = Some("test_*".to_string());
        test_node.linked_tasks = vec!["bn-task".to_string()];
        test_node.linked_bugs = vec!["bn-bug".to_string()];

        let json = serde_json::to_string(&test_node).unwrap();
        let keys = extract_json_keys(&json);
        let fp = fingerprint(&keys);

        let expected =
            "command|created_at|id|linked_bugs|linked_tasks|name|pattern|type|working_dir";
        assert_eq!(
            fp, expected,
            "TestNode schema changed! Update expected fingerprint if intentional."
        );
    }

    #[test]
    fn test_schema_fingerprint_test_result() {
        let test_result = super::TestResult {
            test_id: "bnt-test".to_string(),
            passed: true,
            exit_code: 0,
            stdout: Some("output".to_string()),
            stderr: Some("errors".to_string()),
            duration_ms: 100,
            executed_at: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&test_result).unwrap();
        let keys = extract_json_keys(&json);
        let fp = fingerprint(&keys);

        let expected = "duration_ms|executed_at|exit_code|passed|stderr|stdout|test_id";
        assert_eq!(
            fp, expected,
            "TestResult schema changed! Update expected fingerprint if intentional."
        );
    }

    #[test]
    fn test_schema_fingerprint_agent() {
        let mut agent = super::Agent::new_with_purpose(
            1234,
            1000,
            "test-agent".to_string(),
            super::AgentType::Worker,
            "Testing".to_string(),
        );
        agent.tasks = vec!["bn-task".to_string()];

        let json = serde_json::to_string(&agent).unwrap();
        let keys = extract_json_keys(&json);
        let fp = fingerprint(&keys);

        let expected = "agent_type|command_count|id|last_activity_at|name|parent_pid|pid|purpose|started_at|status|tasks|type";
        assert_eq!(
            fp, expected,
            "Agent schema changed! Update expected fingerprint if intentional."
        );
    }

    #[test]
    fn test_schema_fingerprint_queue() {
        let mut queue = super::Queue::new("bnq-test".to_string(), "Test Queue".to_string());
        queue.description = Some("desc".to_string());

        let json = serde_json::to_string(&queue).unwrap();
        let keys = extract_json_keys(&json);
        let fp = fingerprint(&keys);

        let expected = "created_at|description|id|title|type|updated_at";
        assert_eq!(
            fp, expected,
            "Queue schema changed! Update expected fingerprint if intentional."
        );
    }

    #[test]
    fn test_schema_fingerprint_edge() {
        let mut edge = super::Edge::new(
            "bne-test".to_string(),
            "bn-source".to_string(),
            "bn-target".to_string(),
            super::EdgeType::DependsOn,
        );
        edge.reason = Some("because".to_string());
        edge.created_by = Some("user".to_string());

        let json = serde_json::to_string(&edge).unwrap();
        let keys = extract_json_keys(&json);
        let fp = fingerprint(&keys);

        let expected = "created_at|created_by|edge_type|id|reason|source|target|type|weight";
        assert_eq!(
            fp, expected,
            "Edge schema changed! Update expected fingerprint if intentional."
        );
    }

    #[test]
    fn test_schema_fingerprint_commit_link() {
        let commit_link = super::CommitLink {
            sha: "abc123".to_string(),
            task_id: "bn-task".to_string(),
            linked_at: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&commit_link).unwrap();
        let keys = extract_json_keys(&json);
        let fp = fingerprint(&keys);

        let expected = "linked_at|sha|task_id";
        assert_eq!(
            fp, expected,
            "CommitLink schema changed! Update expected fingerprint if intentional."
        );
    }

    #[test]
    fn test_schema_fingerprint_session_state() {
        let session = super::SessionState::new(1234, super::AgentType::Worker);

        let json = serde_json::to_string(&session).unwrap();
        let keys = extract_json_keys(&json);
        let fp = fingerprint(&keys);

        let expected = "agent_pid|agent_type|orient_called|started_at";
        assert_eq!(
            fp, expected,
            "SessionState schema changed! Update expected fingerprint if intentional."
        );
    }
}
