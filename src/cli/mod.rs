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

    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Top-level commands
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Initialize binnacle for this repository
    Init,

    /// Get project overview and current state (start here!)
    Orient,

    /// Task management commands
    Task {
        #[command(subcommand)]
        command: TaskCommands,
    },

    /// Dependency management commands
    Dep {
        #[command(subcommand)]
        command: DepCommands,
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
    Doctor,

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

    /// Start the web GUI (requires 'gui' feature)
    #[cfg(feature = "gui")]
    Gui {
        /// Port to listen on (default: 3030)
        #[arg(short, long, default_value = "3030")]
        port: u16,
    },
}

/// Task subcommands
#[derive(Subcommand, Debug)]
pub enum TaskCommands {
    /// Create a new task
    Create {
        /// Task title
        title: String,

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

    /// Update a task (status: pending, in_progress, partial, blocked)
    Update {
        /// Task ID
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
    },

    /// Close a task (marks as done)
    Close {
        /// Task ID
        id: String,

        /// Reason for closing (describe what was accomplished)
        #[arg(long)]
        reason: Option<String>,

        /// Force close even with incomplete dependencies (use with caution)
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

/// Dependency subcommands
#[derive(Subcommand, Debug)]
pub enum DepCommands {
    /// Add a dependency (child depends on parent)
    Add {
        /// Child task ID (the task that depends on another)
        child: String,
        /// Parent task ID (the task being depended on)
        parent: String,
    },

    /// Remove a dependency
    Rm {
        /// Child task ID
        child: String,
        /// Parent task ID
        parent: String,
    },

    /// Show dependency graph for a task
    Show {
        /// Task ID
        id: String,
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
