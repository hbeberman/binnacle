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
//! - `Editor` - Attribution for document version editors (agent or user)
//! - `complexity` - Heuristics for detecting complex task descriptions

pub mod complexity;
pub mod graph;
pub mod prompts;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

/// Default empty string for serde deserialization.
fn default_empty_string() -> String {
    String::new()
}

/// Default timestamp for missing fields (Unix epoch).
fn default_timestamp() -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(0, 0).expect("Unix epoch timestamp is valid")
}

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

/// Type of documentation node.
/// Used to categorize docs for filtering and discovery.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocType {
    /// Product requirements, specifications
    #[default]
    Prd,
    /// General notes, observations
    Note,
    /// Context for session handoffs when partial progress made
    Handoff,
}

impl fmt::Display for DocType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocType::Prd => write!(f, "prd"),
            DocType::Note => write!(f, "note"),
            DocType::Handoff => write!(f, "handoff"),
        }
    }
}

/// Type of editor that modified a document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditorType {
    /// An AI agent edited the document
    Agent,
    /// A human user edited the document
    User,
}

impl fmt::Display for EditorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditorType::Agent => write!(f, "agent"),
            EditorType::User => write!(f, "user"),
        }
    }
}

/// Attribution for who edited a document version.
///
/// Used in `Doc.editors` to track the chain of editors for each version.
///
/// # Example
/// ```
/// use binnacle::models::{Editor, EditorType};
///
/// let agent_editor = Editor::agent("bn-57f9".to_string());
/// let user_editor = Editor::user("henry".to_string());
///
/// assert_eq!(agent_editor.editor_type, EditorType::Agent);
/// assert_eq!(agent_editor.identifier, "bn-57f9");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Editor {
    /// Whether this is an agent or user edit
    pub editor_type: EditorType,
    /// Identifier: agent ID (e.g., "bn-57f9") or username (e.g., "henry")
    pub identifier: String,
}

impl Editor {
    /// Create a new editor attribution.
    pub fn new(editor_type: EditorType, identifier: String) -> Self {
        Self {
            editor_type,
            identifier,
        }
    }

    /// Create an agent editor.
    pub fn agent(identifier: String) -> Self {
        Self::new(EditorType::Agent, identifier)
    }

    /// Create a user editor.
    pub fn user(identifier: String) -> Self {
        Self::new(EditorType::User, identifier)
    }
}

impl fmt::Display for Editor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.editor_type, self.identifier)
    }
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
    #[serde(default = "default_empty_string")]
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
    #[serde(default = "default_timestamp")]
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    #[serde(default = "default_timestamp")]
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
/// They use the standard ID format (bn-xxxx) like other entities.
///
/// ## Content Storage
///
/// Content is stored compressed (zstd + base64) in JSONL to minimize storage.
/// The `content` field holds the encoded string, while methods like
/// `get_content()` and `set_content()` handle compression/decompression.
///
/// ## Versioning
///
/// Each `bn doc update` creates a new Doc entity with `supersedes` pointing
/// to the previous version. This provides an audit trail of changes.
///
/// ## Summary Dirty Detection
///
/// The `summary_dirty` flag tracks when content changes but the `# Summary`
/// section doesn't, signaling that an agent should update the summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Doc {
    /// Common entity fields (id, type, title, short_name, description, tags, timestamps)
    #[serde(flatten)]
    pub core: EntityCore,

    /// Type of documentation (prd, note, handoff)
    #[serde(default)]
    pub doc_type: DocType,

    /// Markdown content (zstd compressed + base64 encoded)
    /// Use `get_content()` and `set_content()` for transparent compression.
    #[serde(default)]
    pub content: String,

    /// True if content changed but # Summary section didn't
    #[serde(default)]
    pub summary_dirty: bool,

    /// List of editors who contributed to this version
    #[serde(default)]
    pub editors: Vec<Editor>,

    /// ID of previous version (if this is an update)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,
}

/// Maximum compressed+encoded content size (7.5KB)
pub const DOC_CONTENT_MAX_SIZE: usize = 7680;

impl Doc {
    /// Create a new doc with the given ID and title.
    pub fn new(id: String, title: String) -> Self {
        Self {
            core: EntityCore::new("doc", id, title),
            doc_type: DocType::default(),
            content: String::new(),
            summary_dirty: false,
            editors: Vec::new(),
            supersedes: None,
        }
    }

    /// Create a new doc with all fields specified.
    pub fn with_content(
        id: String,
        title: String,
        doc_type: DocType,
        content: &str,
        editors: Vec<Editor>,
    ) -> Result<Self, DocCompressionError> {
        let mut doc = Self {
            core: EntityCore::new("doc", id, title),
            doc_type,
            content: String::new(),
            summary_dirty: false,
            editors,
            supersedes: None,
        };
        doc.set_content(content)?;
        Ok(doc)
    }

    /// Get the decompressed content.
    ///
    /// Returns an empty string if content is empty, otherwise decompresses.
    pub fn get_content(&self) -> Result<String, DocCompressionError> {
        if self.content.is_empty() {
            return Ok(String::new());
        }
        decompress_content(&self.content)
    }

    /// Set content with automatic compression.
    ///
    /// Returns an error if the compressed content exceeds the size limit.
    pub fn set_content(&mut self, content: &str) -> Result<(), DocCompressionError> {
        if content.is_empty() {
            self.content = String::new();
            return Ok(());
        }
        let compressed = compress_content(content)?;
        if compressed.len() > DOC_CONTENT_MAX_SIZE {
            return Err(DocCompressionError::ContentTooLarge {
                size: compressed.len(),
                max: DOC_CONTENT_MAX_SIZE,
            });
        }
        self.content = compressed;
        Ok(())
    }

    /// Check if content changed but summary didn't between two doc contents.
    ///
    /// Used when updating a doc to detect if `summary_dirty` should be set.
    pub fn is_summary_dirty(old_content: &str, new_content: &str) -> bool {
        let old_hash = hash_excluding_summary(old_content);
        let new_hash = hash_excluding_summary(new_content);
        let old_summary_hash = hash_summary_section(old_content);
        let new_summary_hash = hash_summary_section(new_content);

        // Content changed but summary didn't
        old_hash != new_hash && old_summary_hash == new_summary_hash
    }

    /// Add an editor to this doc version.
    pub fn add_editor(&mut self, editor: Editor) {
        // Don't duplicate editors
        if !self
            .editors
            .iter()
            .any(|e| e.identifier == editor.identifier)
        {
            self.editors.push(editor);
        }
    }

    /// Get the summary section from the doc content.
    ///
    /// Extracts the text between `# Summary` and the next top-level heading.
    /// Returns an empty string if no summary section exists or content is empty.
    pub fn get_summary(&self) -> Result<String, DocCompressionError> {
        let content = self.get_content()?;
        Ok(extract_summary_section(&content))
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

// =============================================================================
// Doc Compression Utilities
// =============================================================================

/// Errors that can occur during doc content compression/decompression.
#[derive(Debug, thiserror::Error)]
pub enum DocCompressionError {
    #[error("Failed to compress content: {0}")]
    CompressionFailed(String),

    #[error("Failed to decompress content: {0}")]
    DecompressionFailed(String),

    #[error("Failed to decode base64: {0}")]
    Base64DecodeFailed(String),

    #[error("Content is not valid UTF-8: {0}")]
    InvalidUtf8(String),

    #[error("Compressed content too large: {size} bytes (max: {max} bytes)")]
    ContentTooLarge { size: usize, max: usize },
}

/// Compress content using zstd and encode as base64.
pub fn compress_content(content: &str) -> Result<String, DocCompressionError> {
    use base64::Engine;

    let compressed = zstd::stream::encode_all(content.as_bytes(), 3)
        .map_err(|e| DocCompressionError::CompressionFailed(e.to_string()))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&compressed))
}

/// Decode base64 and decompress using zstd.
pub fn decompress_content(encoded: &str) -> Result<String, DocCompressionError> {
    use base64::Engine;

    let compressed = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| DocCompressionError::Base64DecodeFailed(e.to_string()))?;
    let decompressed = zstd::stream::decode_all(&compressed[..])
        .map_err(|e| DocCompressionError::DecompressionFailed(e.to_string()))?;
    String::from_utf8(decompressed).map_err(|e| DocCompressionError::InvalidUtf8(e.to_string()))
}

/// Hash content excluding the # Summary section.
fn hash_excluding_summary(content: &str) -> String {
    let without_summary = remove_summary_section(content);
    let mut hasher = Sha256::new();
    hasher.update(without_summary.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Hash only the # Summary section.
fn hash_summary_section(content: &str) -> String {
    let summary = extract_summary_section(content);
    let mut hasher = Sha256::new();
    hasher.update(summary.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Remove the # Summary section from content.
fn remove_summary_section(content: &str) -> String {
    let mut result = String::new();
    let mut in_summary = false;

    for line in content.lines() {
        if line.starts_with("# Summary") {
            in_summary = true;
            continue;
        }
        // Another top-level heading ends the summary section
        if in_summary && line.starts_with("# ") && !line.starts_with("# Summary") {
            in_summary = false;
        }
        if !in_summary {
            result.push_str(line);
            result.push('\n');
        }
    }
    result
}

/// Extract just the # Summary section from content.
pub fn extract_summary_section(content: &str) -> String {
    let mut result = String::new();
    let mut in_summary = false;

    for line in content.lines() {
        if line.starts_with("# Summary") {
            in_summary = true;
            continue;
        }
        // Another top-level heading ends the summary section
        if in_summary && line.starts_with("# ") && !line.starts_with("# Summary") {
            break;
        }
        if in_summary {
            result.push_str(line);
            result.push('\n');
        }
    }
    result
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

// =============================================================================
// Mission
// =============================================================================

/// A mission is a high-level organizational unit that groups related milestones.
/// Missions sit above milestones in the hierarchy: Mission → Milestone → Task/Bug.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mission {
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

impl Mission {
    /// Create a new mission with the given ID and title.
    pub fn new(id: String, title: String) -> Self {
        Self {
            core: EntityCore::new("mission", id, title),
            priority: 2,
            status: TaskStatus::default(),
            due_date: None,
            assignee: None,
            closed_at: None,
            closed_reason: None,
        }
    }
}

impl Entity for Mission {
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

/// Progress statistics for a mission.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionProgress {
    /// Total number of child milestones
    pub total: usize,
    /// Number of completed milestones
    pub completed: usize,
    /// Completion percentage (0-100)
    pub percentage: f64,
}

impl MissionProgress {
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
    #[serde(default = "default_timestamp")]
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
    #[serde(default = "default_timestamp")]
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
    #[serde(default = "default_timestamp")]
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
    /// Ask agents provide read-only Q&A assistance for exploring the repository
    Ask,
}

/// Health status for an agent, including stuck detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHealth {
    /// True if agent is stuck (idle >30min AND has assigned tasks)
    pub is_stuck: bool,
    /// Minutes since last activity
    pub idle_minutes: u64,
    /// Task IDs the agent is stuck on (empty if not stuck)
    pub stuck_task_ids: Vec<String>,
}

/// An AI agent registered with Binnacle for lifecycle management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    /// Unique identifier (e.g., "bn-a1b2")
    /// Uses standard bn- prefix with entity_type=agent to distinguish from tasks.
    /// Generated from PID and start time for uniqueness.
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

    /// MCP session ID for non-PID-based agent tracking.
    /// When set, goodbye uses this instead of parent PID for lookup.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_session_id: Option<String>,

    /// When the agent was registered
    #[serde(default = "default_timestamp")]
    pub started_at: DateTime<Utc>,

    /// When the agent last ran a binnacle command
    #[serde(default = "default_timestamp")]
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

    /// Current action the agent is performing (e.g., "working", "goodbye", task short_name)
    /// Displayed as a status indicator over the agent node in the GUI
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_action: Option<String>,

    /// When the agent called goodbye (for fade-out animation timing)
    /// GUI uses this to show agent for 5 seconds after goodbye before fading out
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goodbye_at: Option<DateTime<Utc>>,

    /// Container ID if the agent is running inside a container.
    /// Used for tracking containerized agent lifecycles.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container_id: Option<String>,

    /// Copilot session GUID for session recovery.
    /// Extracted from ~/.copilot/session-state/<GUID>/ directory.
    /// Enables reanimating dormant/failed agents by identifying their Copilot session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copilot_session_guid: Option<String>,
}

fn agent_entity_type() -> String {
    "agent".to_string()
}

fn agent_placeholder_id() -> String {
    // Placeholder ID for backward compatibility during deserialization
    // Gets replaced with a proper bn-xxxx ID when calling ensure_id()
    String::new()
}

impl Agent {
    /// Generate a unique agent ID from PID and timestamp.
    /// Uses standard bn- prefix with entity_type=agent to distinguish from tasks.
    fn generate_id(pid: u32, started_at: &DateTime<Utc>) -> String {
        let seed = format!("{}:{}", pid, started_at.timestamp_nanos_opt().unwrap_or(0));
        let mut hasher = Sha256::new();
        hasher.update(seed.as_bytes());
        let hash = hasher.finalize();
        let hash_hex = format!("{:x}", hash);
        format!("bn-{}", &hash_hex[..4])
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
            mcp_session_id: None,
            started_at: now,
            last_activity_at: now,
            tasks: Vec::new(),
            command_count: 0,
            status: AgentStatus::default(),
            current_action: None,
            goodbye_at: None,
            container_id: None,
            copilot_session_guid: None,
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
            mcp_session_id: None,
            started_at: now,
            last_activity_at: now,
            tasks: Vec::new(),
            command_count: 0,
            status: AgentStatus::default(),
            current_action: None,
            goodbye_at: None,
            container_id: None,
            copilot_session_guid: None,
        }
    }

    /// Create a new agent with a specific ID (for BN_AGENT_ID env var support).
    pub fn new_with_id(
        id: String,
        pid: u32,
        parent_pid: u32,
        name: String,
        agent_type: AgentType,
    ) -> Self {
        let now = Utc::now();
        Self {
            id,
            entity_type: "agent".to_string(),
            pid,
            parent_pid,
            name,
            agent_type,
            purpose: None,
            mcp_session_id: None,
            started_at: now,
            last_activity_at: now,
            tasks: Vec::new(),
            command_count: 0,
            status: AgentStatus::default(),
            current_action: None,
            goodbye_at: None,
            container_id: None,
            copilot_session_guid: None,
        }
    }

    /// Create a new agent with a specific ID and purpose.
    pub fn new_with_id_and_purpose(
        id: String,
        pid: u32,
        parent_pid: u32,
        name: String,
        agent_type: AgentType,
        purpose: String,
    ) -> Self {
        let now = Utc::now();
        Self {
            id,
            entity_type: "agent".to_string(),
            pid,
            parent_pid,
            name,
            agent_type,
            purpose: Some(purpose),
            mcp_session_id: None,
            started_at: now,
            last_activity_at: now,
            tasks: Vec::new(),
            command_count: 0,
            status: AgentStatus::default(),
            current_action: None,
            goodbye_at: None,
            container_id: None,
            copilot_session_guid: None,
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
    /// For agents with pid=0 (container agents or external agents identified by explicit ID),
    /// this returns true since PID-based liveness detection doesn't apply to them.
    /// Container agent lifecycle is managed by the reconcile loop via goodbye_at and container_stop.
    /// External agents (e.g., MCP sessions) manage their own lifecycle.
    #[cfg(unix)]
    pub fn is_alive(&self) -> bool {
        use std::path::Path;
        // Agents with pid=0 are identified by explicit ID (BN_AGENT_ID), not by PID.
        // This includes container agents and external agents (e.g., MCP sessions).
        // PID-based liveness check doesn't apply - they manage their own lifecycle.
        if self.pid == 0 {
            return true;
        }
        // For regular agents, check if /proc/<pid> exists
        Path::new(&format!("/proc/{}", self.pid)).exists()
    }

    #[cfg(not(unix))]
    pub fn is_alive(&self) -> bool {
        // On non-Unix systems, assume alive (conservative)
        true
    }

    /// Compute health status for this agent.
    /// Agent is stuck if idle >30min AND has assigned tasks.
    pub fn compute_health(&self) -> AgentHealth {
        let now = Utc::now();
        let idle_duration = now.signed_duration_since(self.last_activity_at);
        let idle_minutes = idle_duration.num_minutes().max(0) as u64;

        let is_stuck = idle_minutes > 30 && !self.tasks.is_empty();

        AgentHealth {
            is_stuck,
            idle_minutes,
            stuck_task_ids: if is_stuck {
                self.tasks.clone()
            } else {
                Vec::new()
            },
        }
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
    #[serde(default = "default_timestamp")]
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
    #[serde(default = "default_timestamp")]
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    #[serde(default = "default_timestamp")]
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

// =============================================================================
// Log Annotation
// =============================================================================

/// An annotation attached to an activity log entry.
/// Used to add context, explanations, or notes to log entries (e.g., explain failures).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogAnnotation {
    /// Unique identifier (e.g., "bnl-a1b2")
    pub id: String,

    /// Entity type marker
    #[serde(rename = "type")]
    pub entity_type: String,

    /// Timestamp of the log entry this annotation is attached to (ISO 8601)
    /// This serves as the foreign key to the action log entry
    pub log_timestamp: String,

    /// The annotation text/note
    pub content: String,

    /// User who created the annotation
    pub author: String,

    /// When the annotation was created
    #[serde(default = "default_timestamp")]
    pub created_at: DateTime<Utc>,

    /// When the annotation was last updated
    #[serde(default = "default_timestamp")]
    pub updated_at: DateTime<Utc>,
}

impl LogAnnotation {
    /// Create a new log annotation.
    pub fn new(id: String, log_timestamp: String, content: String, author: String) -> Self {
        let now = Utc::now();
        Self {
            id,
            entity_type: "log_annotation".to_string(),
            log_timestamp,
            content,
            author,
            created_at: now,
            updated_at: now,
        }
    }

    /// Update the annotation content.
    pub fn update_content(&mut self, content: String) {
        self.content = content;
        self.updated_at = Utc::now();
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
    /// Task/Milestone is in the queue for prioritized work (Task/Milestone → Queue)
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
    #[serde(default = "default_timestamp")]
    pub created_at: DateTime<Utc>,

    /// Who created the edge (user or agent)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,

    /// Whether this edge is pinned to a specific version.
    /// Pinned edges don't transfer when a doc is updated - allows pointing to specific doc versions.
    #[serde(default, skip_serializing_if = "is_false")]
    pub pinned: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
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
            pinned: false,
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
            pinned: self.pinned,
        }
    }

    /// Returns true if this edge is pinned to a specific version.
    pub fn is_pinned(&self) -> bool {
        self.pinned
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
    fn test_task_missing_title_field() {
        // This test verifies that Task JSON without a title field can be parsed
        // The title field has #[serde(default)] and should default to empty string
        let json_without_title = r#"{
            "id": "bn-test1",
            "type": "task",
            "priority": 2,
            "status": "pending",
            "created_at": "2026-01-27T08:00:00Z",
            "updated_at": "2026-01-27T08:00:00Z"
        }"#;

        let task: Task = serde_json::from_str(json_without_title).unwrap();

        // Verify the task parses correctly with empty title
        assert_eq!(task.core.id, "bn-test1");
        assert_eq!(task.core.entity_type, "task");
        assert_eq!(task.core.title, ""); // Should be empty string, not error
        assert_eq!(task.priority, 2);
        assert_eq!(task.status, TaskStatus::Pending);
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
        // After ensure_id, it should have a proper bn-xxxx ID (with entity_type=agent)
        agent.ensure_id();
        assert!(agent.id.starts_with("bn-"));
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

    #[test]
    fn test_schema_fingerprint_editor() {
        let editor = super::Editor::agent("bn-57f9".to_string());

        let json = serde_json::to_string(&editor).unwrap();
        let keys = extract_json_keys(&json);
        let fp = fingerprint(&keys);

        let expected = "editor_type|identifier";
        assert_eq!(
            fp, expected,
            "Editor schema changed! Update expected fingerprint if intentional."
        );
    }

    #[test]
    fn test_editor_serialization_roundtrip() {
        let agent_editor = super::Editor::agent("bn-57f9".to_string());
        let json = serde_json::to_string(&agent_editor).unwrap();
        let parsed: super::Editor = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.editor_type, super::EditorType::Agent);
        assert_eq!(parsed.identifier, "bn-57f9");

        let user_editor = super::Editor::user("henry".to_string());
        let json = serde_json::to_string(&user_editor).unwrap();
        let parsed: super::Editor = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.editor_type, super::EditorType::User);
        assert_eq!(parsed.identifier, "henry");
    }

    #[test]
    fn test_editor_display() {
        let agent = super::Editor::agent("bn-57f9".to_string());
        assert_eq!(format!("{}", agent), "agent:bn-57f9");

        let user = super::Editor::user("henry".to_string());
        assert_eq!(format!("{}", user), "user:henry");
    }

    #[test]
    fn test_editor_type_display() {
        assert_eq!(format!("{}", super::EditorType::Agent), "agent");
        assert_eq!(format!("{}", super::EditorType::User), "user");
    }

    // =============================================================================
    // Doc model tests
    // =============================================================================

    #[test]
    fn test_doc_type_display() {
        assert_eq!(format!("{}", super::DocType::Prd), "prd");
        assert_eq!(format!("{}", super::DocType::Note), "note");
        assert_eq!(format!("{}", super::DocType::Handoff), "handoff");
    }

    #[test]
    fn test_doc_type_default() {
        assert_eq!(super::DocType::default(), super::DocType::Prd);
    }

    #[test]
    fn test_doc_new() {
        let doc = super::Doc::new("bn-abc1".to_string(), "Test Doc".to_string());
        assert_eq!(doc.core.id, "bn-abc1");
        assert_eq!(doc.core.title, "Test Doc");
        assert_eq!(doc.core.entity_type, "doc");
        assert_eq!(doc.doc_type, super::DocType::Prd);
        assert!(!doc.summary_dirty);
        assert!(doc.editors.is_empty());
        assert!(doc.supersedes.is_none());
        assert!(doc.content.is_empty());
    }

    #[test]
    fn test_doc_compression_roundtrip() {
        let original = "# Summary\nThis is a test document.\n\n# Content\nSome content here.";
        let compressed = super::compress_content(original).unwrap();
        let decompressed = super::decompress_content(&compressed).unwrap();
        assert_eq!(decompressed, original);
    }

    #[test]
    fn test_doc_set_get_content() {
        let mut doc = super::Doc::new("bn-abc1".to_string(), "Test Doc".to_string());
        let content = "# Summary\nBrief summary.\n\n# Details\nMore details here.";
        doc.set_content(content).unwrap();

        // Content should be compressed (not plain text)
        assert!(!doc.content.is_empty());
        assert_ne!(doc.content, content);

        // Get content should decompress
        let retrieved = doc.get_content().unwrap();
        assert_eq!(retrieved, content);
    }

    #[test]
    fn test_doc_empty_content() {
        let mut doc = super::Doc::new("bn-abc1".to_string(), "Test Doc".to_string());
        doc.set_content("").unwrap();
        assert!(doc.content.is_empty());
        assert_eq!(doc.get_content().unwrap(), "");
    }

    #[test]
    fn test_doc_content_size_limit() {
        let mut doc = super::Doc::new("bn-abc1".to_string(), "Test Doc".to_string());
        // Create content that will compress to more than 5KB
        // Note: Random data doesn't compress well, so this should exceed the limit
        let large_content = (0..50_000)
            .map(|i| format!("line{:06}\n", i))
            .collect::<String>();
        let result = doc.set_content(&large_content);
        assert!(result.is_err());
        match result {
            Err(super::DocCompressionError::ContentTooLarge { size, max }) => {
                assert!(size > max);
                assert_eq!(max, super::DOC_CONTENT_MAX_SIZE);
            }
            _ => panic!("Expected ContentTooLarge error"),
        }
    }

    #[test]
    fn test_doc_with_content() {
        let doc = super::Doc::with_content(
            "bn-abc1".to_string(),
            "Test Doc".to_string(),
            super::DocType::Note,
            "# Summary\nTest",
            vec![super::Editor::user("henry".to_string())],
        )
        .unwrap();

        assert_eq!(doc.core.id, "bn-abc1");
        assert_eq!(doc.doc_type, super::DocType::Note);
        assert_eq!(doc.get_content().unwrap(), "# Summary\nTest");
        assert_eq!(doc.editors.len(), 1);
        assert_eq!(doc.editors[0].identifier, "henry");
    }

    #[test]
    fn test_doc_add_editor_no_duplicates() {
        let mut doc = super::Doc::new("bn-abc1".to_string(), "Test Doc".to_string());
        doc.add_editor(super::Editor::user("henry".to_string()));
        doc.add_editor(super::Editor::user("henry".to_string())); // Duplicate
        doc.add_editor(super::Editor::agent("bn-57f9".to_string()));
        assert_eq!(doc.editors.len(), 2);
    }

    #[test]
    fn test_doc_summary_dirty_detection() {
        let old_content = "# Summary\nOld summary.\n\n# Content\nOld content.";
        let new_content_same_summary = "# Summary\nOld summary.\n\n# Content\nNew content changed!";
        let new_content_new_summary = "# Summary\nNew summary.\n\n# Content\nNew content changed!";

        // Content changed but summary didn't -> dirty
        assert!(super::Doc::is_summary_dirty(
            old_content,
            new_content_same_summary
        ));

        // Both content and summary changed -> not dirty
        assert!(!super::Doc::is_summary_dirty(
            old_content,
            new_content_new_summary
        ));

        // Nothing changed -> not dirty
        assert!(!super::Doc::is_summary_dirty(old_content, old_content));
    }

    #[test]
    fn test_doc_get_summary() {
        let mut doc = super::Doc::new("bn-abc1".to_string(), "Test Doc".to_string());
        doc.set_content("# Summary\nThis is the summary text.\n\n# Content\nMain content here.")
            .unwrap();

        let summary = doc.get_summary().unwrap();
        assert!(summary.contains("This is the summary text."));
        // Summary should not include the Content section
        assert!(!summary.contains("Main content"));
    }

    #[test]
    fn test_doc_get_summary_empty() {
        let doc = super::Doc::new("bn-abc1".to_string(), "Test Doc".to_string());
        let summary = doc.get_summary().unwrap();
        assert!(summary.is_empty());
    }

    #[test]
    fn test_doc_get_summary_no_summary_section() {
        let mut doc = super::Doc::new("bn-abc1".to_string(), "Test Doc".to_string());
        doc.set_content("# Title\nSome content without a summary section.")
            .unwrap();

        let summary = doc.get_summary().unwrap();
        assert!(summary.is_empty());
    }

    #[test]
    fn test_doc_schema_fingerprint() {
        let mut doc = super::Doc::new("bn-abc1".to_string(), "Test Doc".to_string());
        doc.doc_type = super::DocType::Prd;
        doc.summary_dirty = true;
        doc.editors = vec![super::Editor::user("henry".to_string())];
        doc.supersedes = Some("bn-old1".to_string());
        doc.set_content("test content").unwrap();

        let json = serde_json::to_string(&doc).unwrap();
        let keys = extract_json_keys(&json);
        let fp = fingerprint(&keys);

        // Note: description and short_name only appear when set (skip_serializing_if)
        let expected = "content|created_at|doc_type|editors|id|summary_dirty|supersedes|tags|title|type|updated_at";
        assert_eq!(
            fp, expected,
            "Doc schema changed! Update expected fingerprint if intentional."
        );
    }

    #[test]
    fn test_doc_serialization_roundtrip() {
        let mut doc = super::Doc::new("bn-abc1".to_string(), "Test Doc".to_string());
        doc.doc_type = super::DocType::Handoff;
        doc.summary_dirty = true;
        doc.editors = vec![
            super::Editor::user("henry".to_string()),
            super::Editor::agent("bn-57f9".to_string()),
        ];
        doc.supersedes = Some("bn-old1".to_string());
        doc.set_content("# Summary\nTest summary\n\n# Content\nTest content")
            .unwrap();

        let json = serde_json::to_string(&doc).unwrap();
        let parsed: super::Doc = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.core.id, "bn-abc1");
        assert_eq!(parsed.core.title, "Test Doc");
        assert_eq!(parsed.doc_type, super::DocType::Handoff);
        assert!(parsed.summary_dirty);
        assert_eq!(parsed.editors.len(), 2);
        assert_eq!(parsed.supersedes, Some("bn-old1".to_string()));

        // Verify content can be decompressed
        let content = parsed.get_content().unwrap();
        assert!(content.contains("# Summary"));
        assert!(content.contains("Test content"));
    }

    #[test]
    fn test_doc_entity_trait() {
        use super::Entity;
        let mut doc = super::Doc::new("bn-abc1".to_string(), "Test Doc".to_string());
        doc.core.short_name = Some("short".to_string());
        doc.core.description = Some("desc".to_string());
        doc.core.tags = vec!["tag1".to_string()];

        assert_eq!(doc.id(), "bn-abc1");
        assert_eq!(doc.entity_type(), "doc");
        assert_eq!(doc.title(), "Test Doc");
        assert_eq!(doc.short_name(), Some("short"));
        assert_eq!(doc.description(), Some("desc"));
        assert_eq!(doc.tags(), &["tag1".to_string()]);
    }

    #[test]
    fn test_hash_summary_section() {
        let content1 = "# Summary\nFirst summary\n\n# Other\nOther content";
        let content2 = "# Summary\nFirst summary\n\n# Other\nDifferent content";
        let content3 = "# Summary\nDifferent summary\n\n# Other\nOther content";

        // Same summary section should have same hash
        let hash1 = super::hash_summary_section(content1);
        let hash2 = super::hash_summary_section(content2);
        assert_eq!(hash1, hash2);

        // Different summary section should have different hash
        let hash3 = super::hash_summary_section(content3);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_hash_excluding_summary() {
        let content1 = "# Summary\nFirst summary\n\n# Other\nOther content";
        let content2 = "# Summary\nDifferent summary\n\n# Other\nOther content";
        let content3 = "# Summary\nFirst summary\n\n# Other\nDifferent content";

        // Same content (excluding summary) should have same hash
        let hash1 = super::hash_excluding_summary(content1);
        let hash2 = super::hash_excluding_summary(content2);
        assert_eq!(hash1, hash2);

        // Different content (excluding summary) should have different hash
        let hash3 = super::hash_excluding_summary(content3);
        assert_ne!(hash1, hash3);
    }
}
