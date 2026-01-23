//! CLI argument definitions for Binnacle.

use clap::{Parser, Subcommand};

/// Binnacle - A project state tracking tool for AI agents and humans.
///
/// Start with `bn orient` to understand project state, then `bn ready` to find work.
#[derive(Parser, Debug)]
#[command(name = "bn")]
#[command(author, version, about = "A CLI tool for AI agents and humans to track project state", long_about = None)]
pub struct Cli {
    /// Output in human-readable format instead of JSON
    #[arg(short = 'H', long = "human", global = true)]
    pub human_readable: bool,

    /// Run as if bn was started in <path> instead of the current directory.
    /// The path must exist. Bypasses git root detection - uses the path literally.
    /// Can also be set via BN_REPO environment variable.
    #[arg(short = 'C', long = "repo", global = true, env = "BN_REPO")]
    pub repo_path: Option<std::path::PathBuf>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Top-level commands
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Get project overview and current state (start here!)
    ///
    /// Shows tasks, bugs, milestones, and their relationships.
    /// For new projects, humans should use `bn system init` (interactive).
    /// The --init flag is for AI agents needing non-interactive setup.
    Orient {
        /// Initialize database non-interactively (conservative defaults, for AI agents)
        #[arg(long)]
        init: bool,
    },

    /// Show any entity by ID (auto-detects type)
    Show {
        /// Entity ID (e.g., bn-a1b2, bnt-0001)
        id: String,
    },

    /// Task management commands
    Task {
        #[command(subcommand)]
        command: TaskCommands,
    },

    /// Bug tracking commands
    Bug {
        #[command(subcommand)]
        command: BugCommands,
    },

    /// Idea management commands (low-stakes seeds that can be promoted to tasks)
    Idea {
        #[command(subcommand)]
        command: IdeaCommands,
    },

    /// Milestone management commands
    Milestone {
        #[command(subcommand)]
        command: MilestoneCommands,
    },

    /// Link management commands (relationships between entities)
    Link {
        #[command(subcommand)]
        command: LinkCommands,
    },

    /// Test node management commands
    Test {
        #[command(subcommand)]
        command: TestCommands,
    },

    /// Commit tracking commands
    Commit {
        #[command(subcommand)]
        command: CommitCommands,
    },

    /// Show tasks ready to work on (no incomplete dependencies)
    Ready,

    /// Show tasks waiting on dependencies
    Blocked,

    /// Health check and issue detection
    Doctor {
        /// Migrate legacy depends_on fields to edge relationships
        #[arg(long)]
        migrate_edges: bool,

        /// Remove depends_on fields after migration (only works with --migrate-edges)
        #[arg(long)]
        clean_unused: bool,

        /// Preview changes without making them
        #[arg(long)]
        dry_run: bool,
    },

    /// Show audit trail of changes
    Log {
        /// Optional task ID to filter logs
        task_id: Option<String>,
    },

    /// Summarize old closed tasks
    Compact,

    /// Push/pull when sharing is enabled
    Sync,

    /// Configuration management
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },

    /// MCP server commands
    Mcp {
        #[command(subcommand)]
        command: McpCommands,
    },

    /// Graph analysis commands
    Graph {
        #[command(subcommand)]
        command: GraphCommands,
    },

    /// Search commands (query edges, tasks, etc.)
    Search {
        #[command(subcommand)]
        command: SearchCommands,
    },

    /// System administration commands (human-operated)
    ///
    /// Note: These commands are for human operators, not AI agents.
    /// Agents should use 'bn orient' which auto-initializes.
    System {
        #[command(subcommand)]
        command: SystemCommands,
    },

    /// Start the web GUI (requires 'gui' feature)
    #[cfg(feature = "gui")]
    Gui {
        /// Port to listen on (default: 3030, or BN_GUI_PORT env var)
        #[arg(short, long, env = "BN_GUI_PORT", default_value = "3030")]
        port: u16,

        /// Host address to bind to (default: 127.0.0.1, use 0.0.0.0 for network access)
        #[arg(long, env = "BN_GUI_HOST", default_value = "127.0.0.1")]
        host: String,

        /// Show status of running GUI server and exit
        #[arg(long)]
        status: bool,

        /// Stop a running GUI server gracefully (SIGTERM, then SIGKILL after timeout)
        #[arg(long)]
        stop: bool,

        /// Stop any running GUI server and start a new one (combines --stop + start)
        #[arg(long)]
        replace: bool,
    },
}

/// Task subcommands
#[derive(Subcommand, Debug)]
pub enum TaskCommands {
    /// Create a new task
    Create {
        /// Task title
        title: String,

        /// Short display name for GUI (recommended: 1-2 words, ~12 chars max)
        #[arg(short = 's', long)]
        short_name: Option<String>,

        /// Priority (0-4, lower is higher priority)
        #[arg(short, long)]
        priority: Option<u8>,

        /// Tags for the task
        #[arg(short, long)]
        tag: Vec<String>,

        /// Assignee
        #[arg(short, long)]
        assignee: Option<String>,

        /// Task description
        #[arg(short, long)]
        description: Option<String>,
    },

    /// List tasks
    List {
        /// Filter by status
        #[arg(long)]
        status: Option<String>,

        /// Filter by priority
        #[arg(long)]
        priority: Option<u8>,

        /// Filter by tag
        #[arg(long)]
        tag: Option<String>,
    },

    /// Show task details with blocker analysis
    Show {
        /// Task ID (e.g., bn-a1b2)
        id: String,
    },

    /// Update a task (status: pending, in_progress, partial, blocked, done)
    Update {
        /// Task ID
        id: String,

        /// New title
        #[arg(long)]
        title: Option<String>,

        /// New short display name for GUI (recommended: 1-2 words, ~12 chars max)
        #[arg(short = 's', long)]
        short_name: Option<String>,

        /// New description
        #[arg(long)]
        description: Option<String>,

        /// New priority
        #[arg(long)]
        priority: Option<u8>,

        /// New status (pending, in_progress, partial, blocked, done)
        #[arg(long)]
        status: Option<String>,

        /// Add a tag
        #[arg(long)]
        add_tag: Vec<String>,

        /// Remove a tag
        #[arg(long)]
        remove_tag: Vec<String>,

        /// New assignee
        #[arg(long)]
        assignee: Option<String>,

        /// When setting status to done, bypass commit requirement (use with caution)
        #[arg(long)]
        force: bool,
    },

    /// Close a task (marks as done)
    Close {
        /// Task ID
        id: String,

        /// Reason for closing (describe what was accomplished)
        #[arg(long)]
        reason: Option<String>,

        /// Force close even with incomplete dependencies or missing commits (use with caution)
        #[arg(long)]
        force: bool,
    },

    /// Reopen a closed task
    Reopen {
        /// Task ID
        id: String,
    },

    /// Delete a task
    Delete {
        /// Task ID
        id: String,
    },
}

/// Bug subcommands
#[derive(Subcommand, Debug)]
pub enum BugCommands {
    /// Create a new bug
    Create {
        /// Bug title
        title: String,

        /// Priority (0-4, lower is higher priority)
        #[arg(short, long)]
        priority: Option<u8>,

        /// Severity (triage, low, medium, high, critical)
        #[arg(long, value_parser = ["triage", "low", "medium", "high", "critical"])]
        severity: Option<String>,

        /// Tags for the bug
        #[arg(short, long)]
        tag: Vec<String>,

        /// Assignee
        #[arg(short, long)]
        assignee: Option<String>,

        /// Bug description
        #[arg(short, long)]
        description: Option<String>,

        /// Steps to reproduce
        #[arg(long)]
        reproduction_steps: Option<String>,

        /// Affected component or area
        #[arg(long)]
        affected_component: Option<String>,
    },

    /// List bugs
    List {
        /// Filter by status
        #[arg(long)]
        status: Option<String>,

        /// Filter by priority
        #[arg(long)]
        priority: Option<u8>,

        /// Filter by severity
        #[arg(long, value_parser = ["triage", "low", "medium", "high", "critical"])]
        severity: Option<String>,

        /// Filter by tag
        #[arg(long)]
        tag: Option<String>,
    },

    /// Show bug details
    Show {
        /// Bug ID (e.g., bn-b1b2)
        id: String,
    },

    /// Update a bug (status: pending, in_progress, partial, blocked)
    Update {
        /// Bug ID
        id: String,

        /// New title
        #[arg(long)]
        title: Option<String>,

        /// New description
        #[arg(long)]
        description: Option<String>,

        /// New priority
        #[arg(long)]
        priority: Option<u8>,

        /// New status (pending, in_progress, partial, blocked)
        #[arg(long)]
        status: Option<String>,

        /// New severity (triage, low, medium, high, critical)
        #[arg(long, value_parser = ["triage", "low", "medium", "high", "critical"])]
        severity: Option<String>,

        /// Add a tag
        #[arg(long)]
        add_tag: Vec<String>,

        /// Remove a tag
        #[arg(long)]
        remove_tag: Vec<String>,

        /// New assignee
        #[arg(long)]
        assignee: Option<String>,

        /// New steps to reproduce
        #[arg(long)]
        reproduction_steps: Option<String>,

        /// New affected component or area
        #[arg(long)]
        affected_component: Option<String>,
    },

    /// Close a bug (marks as done)
    Close {
        /// Bug ID
        id: String,

        /// Reason for closing (describe what was accomplished)
        #[arg(long)]
        reason: Option<String>,

        /// Force close even with incomplete dependencies (use with caution)
        #[arg(long)]
        force: bool,
    },

    /// Reopen a closed bug
    Reopen {
        /// Bug ID
        id: String,
    },

    /// Delete a bug
    Delete {
        /// Bug ID
        id: String,
    },
}

/// Idea subcommands
#[derive(Subcommand, Debug)]
pub enum IdeaCommands {
    /// Create a new idea (quick capture of rough thoughts)
    Create {
        /// Idea title
        title: String,

        /// Tags for the idea
        #[arg(short, long)]
        tag: Vec<String>,

        /// Idea description
        #[arg(short, long)]
        description: Option<String>,
    },

    /// List ideas
    List {
        /// Filter by status (seed, germinating, promoted, discarded)
        #[arg(long, value_parser = ["seed", "germinating", "promoted", "discarded"])]
        status: Option<String>,

        /// Filter by tag
        #[arg(long)]
        tag: Option<String>,
    },

    /// Show idea details
    Show {
        /// Idea ID (e.g., bni-a1b2)
        id: String,
    },

    /// Update an idea
    Update {
        /// Idea ID
        id: String,

        /// New title
        #[arg(long)]
        title: Option<String>,

        /// New description
        #[arg(long)]
        description: Option<String>,

        /// New status (seed, germinating, promoted, discarded)
        #[arg(long, value_parser = ["seed", "germinating", "promoted", "discarded"])]
        status: Option<String>,

        /// Add a tag
        #[arg(long)]
        add_tag: Vec<String>,

        /// Remove a tag
        #[arg(long)]
        remove_tag: Vec<String>,
    },

    /// Close an idea (marks as discarded)
    Close {
        /// Idea ID
        id: String,

        /// Reason for discarding
        #[arg(long)]
        reason: Option<String>,
    },

    /// Delete an idea permanently
    Delete {
        /// Idea ID
        id: String,
    },
}

/// Milestone subcommands
#[derive(Subcommand, Debug)]
pub enum MilestoneCommands {
    /// Create a new milestone
    Create {
        /// Milestone title
        title: String,

        /// Priority (0-4, lower is higher priority)
        #[arg(short, long)]
        priority: Option<u8>,

        /// Tags for the milestone
        #[arg(short, long)]
        tag: Vec<String>,

        /// Assignee
        #[arg(short, long)]
        assignee: Option<String>,

        /// Milestone description
        #[arg(short, long)]
        description: Option<String>,

        /// Target due date (ISO 8601 format, e.g., 2026-02-01T00:00:00Z)
        #[arg(long)]
        due_date: Option<String>,
    },

    /// List milestones
    List {
        /// Filter by status
        #[arg(long)]
        status: Option<String>,

        /// Filter by priority
        #[arg(long)]
        priority: Option<u8>,

        /// Filter by tag
        #[arg(long)]
        tag: Option<String>,
    },

    /// Show milestone details with progress
    Show {
        /// Milestone ID (e.g., bn-m1b2)
        id: String,
    },

    /// Update a milestone (status: pending, in_progress, partial, blocked)
    Update {
        /// Milestone ID
        id: String,

        /// New title
        #[arg(long)]
        title: Option<String>,

        /// New description
        #[arg(long)]
        description: Option<String>,

        /// New priority
        #[arg(long)]
        priority: Option<u8>,

        /// New status (pending, in_progress, partial, blocked)
        #[arg(long)]
        status: Option<String>,

        /// Add a tag
        #[arg(long)]
        add_tag: Vec<String>,

        /// Remove a tag
        #[arg(long)]
        remove_tag: Vec<String>,

        /// New assignee
        #[arg(long)]
        assignee: Option<String>,

        /// New due date (ISO 8601 format, e.g., 2026-02-01T00:00:00Z)
        #[arg(long)]
        due_date: Option<String>,
    },

    /// Close a milestone (marks as done)
    Close {
        /// Milestone ID
        id: String,

        /// Reason for closing (describe what was accomplished)
        #[arg(long)]
        reason: Option<String>,

        /// Force close even with incomplete children (use with caution)
        #[arg(long)]
        force: bool,
    },

    /// Reopen a closed milestone
    Reopen {
        /// Milestone ID
        id: String,
    },

    /// Delete a milestone
    Delete {
        /// Milestone ID
        id: String,
    },

    /// Show progress for a milestone
    Progress {
        /// Milestone ID
        id: String,
    },
}

/// Link subcommands (relationship management)
#[derive(Subcommand, Debug)]
pub enum LinkCommands {
    /// Create a link between two entities
    #[command(name = "add", visible_alias = "create")]
    Add {
        /// Source entity ID (e.g., bn-1234)
        source: String,
        /// Target entity ID (e.g., bn-5678)
        target: String,
        /// Type of relationship
        #[arg(long = "type", short = 't', value_parser = ["depends_on", "blocks", "related_to", "duplicates", "fixes", "caused_by", "supersedes", "parent_of", "child_of", "tests"])]
        edge_type: String,
        /// Reason for creating this relationship
        #[arg(long)]
        reason: Option<String>,
    },

    /// Remove a link between two entities
    Rm {
        /// Source entity ID
        source: String,
        /// Target entity ID
        target: String,
        /// Type of relationship (required)
        #[arg(long = "type", short = 't', value_parser = ["depends_on", "blocks", "related_to", "duplicates", "fixes", "caused_by", "supersedes", "parent_of", "child_of", "tests"])]
        edge_type: Option<String>,
    },

    /// List links for an entity or all links
    List {
        /// Entity ID to list links for (omit for --all)
        id: Option<String>,
        /// List all links in the system
        #[arg(long)]
        all: bool,
        /// Filter by edge type
        #[arg(long = "type", short = 't', value_parser = ["depends_on", "blocks", "related_to", "duplicates", "fixes", "caused_by", "supersedes", "parent_of", "child_of", "tests"])]
        edge_type: Option<String>,
    },
}

/// Test node subcommands
#[derive(Subcommand, Debug)]
pub enum TestCommands {
    /// Create a new test node
    Create {
        /// Test name
        name: String,

        /// Command to run
        #[arg(long)]
        cmd: String,

        /// Working directory
        #[arg(long, default_value = ".")]
        dir: String,

        /// Link to a task
        #[arg(long)]
        task: Option<String>,
    },

    /// List test nodes
    List {
        /// Filter by linked task
        #[arg(long)]
        task: Option<String>,
    },

    /// Show test node details
    Show {
        /// Test ID (e.g., bnt-0001)
        id: String,
    },

    /// Link a test to a task
    Link {
        /// Test ID
        test_id: String,
        /// Task ID
        task_id: String,
    },

    /// Unlink a test from a task
    Unlink {
        /// Test ID
        test_id: String,
        /// Task ID
        task_id: String,
    },

    /// Run tests
    Run {
        /// Specific test ID to run
        id: Option<String>,

        /// Run tests linked to a specific task
        #[arg(long)]
        task: Option<String>,

        /// Run all tests
        #[arg(long)]
        all: bool,

        /// Run only previously failed tests
        #[arg(long)]
        failed: bool,
    },
}

/// Commit tracking subcommands
#[derive(Subcommand, Debug)]
pub enum CommitCommands {
    /// Link a commit to a task
    Link {
        /// Commit SHA
        sha: String,
        /// Task ID
        task_id: String,
    },

    /// Unlink a commit from a task
    Unlink {
        /// Commit SHA
        sha: String,
        /// Task ID
        task_id: String,
    },

    /// List commits linked to a task
    List {
        /// Task ID
        task_id: String,
    },
}

/// Configuration subcommands
#[derive(Subcommand, Debug)]
pub enum ConfigCommands {
    /// Get a configuration value
    Get {
        /// Configuration key
        key: String,
    },

    /// Set a configuration value
    Set {
        /// Configuration key
        key: String,
        /// Configuration value
        value: String,
    },

    /// List all configuration values
    List,
}

/// MCP server subcommands
#[derive(Subcommand, Debug)]
pub enum McpCommands {
    /// Start stdio MCP server
    Serve,

    /// Output tool definitions
    Manifest,
}

/// Graph analysis subcommands
#[derive(Subcommand, Debug)]
pub enum GraphCommands {
    /// Analyze task graph for disconnected components
    Components,
}

/// Search subcommands
#[derive(Subcommand, Debug)]
pub enum SearchCommands {
    /// Search for links/edges by type, source, or target
    Link {
        /// Filter by edge type
        #[arg(long = "type", short = 't', value_parser = ["depends_on", "blocks", "related_to", "duplicates", "fixes", "caused_by", "supersedes", "parent_of", "child_of", "tests"])]
        edge_type: Option<String>,

        /// Filter by source entity ID
        #[arg(long)]
        source: Option<String>,

        /// Filter by target entity ID
        #[arg(long)]
        target: Option<String>,
    },
}

/// System administration subcommands (human-operated)
#[derive(Subcommand, Debug)]
pub enum SystemCommands {
    /// Initialize binnacle for this repository
    Init,

    /// Data store management (import/export/inspect)
    Store {
        #[command(subcommand)]
        command: StoreCommands,
    },
}

/// Store management subcommands
#[derive(Subcommand, Debug)]
pub enum StoreCommands {
    /// Display summary of current store contents
    Show,

    /// Export store to archive file
    Export {
        /// Output path (use '-' for stdout)
        output: String,

        /// Export format (currently only 'archive' is supported)
        #[arg(long, default_value = "archive")]
        format: String,
    },

    /// Import store from archive file or storage folder
    Import {
        /// Input path: archive file (.tar.gz), storage folder, or '-' for stdin
        input: String,

        /// Import type: 'replace' (error if initialized) or 'merge' (append with ID conflict handling)
        #[arg(long, default_value = "replace", value_parser = ["replace", "merge"])]
        r#type: String,

        /// Preview import without making changes (shows ID remappings)
        #[arg(long)]
        dry_run: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn verify_cli() {
        // This will panic if the CLI is misconfigured
        Cli::command().debug_assert();
    }
}
