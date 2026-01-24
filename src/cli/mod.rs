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
        /// Agent type (required): worker, planner, or buddy
        #[arg(long = "type", short = 't', value_parser = ["worker", "planner", "buddy"])]
        agent_type: String,
        /// Initialize database non-interactively (conservative defaults, for AI agents)
        #[arg(long)]
        init: bool,
        /// Agent name for registration (auto-generated if not provided)
        #[arg(long)]
        name: Option<String>,
        /// Register agent's purpose (e.g., "Task Worker", "PRD Generator")
        /// Agents without a purpose are labeled "UNREGISTERED"
        #[arg(long)]
        register: Option<String>,
    },

    /// Gracefully terminate this agent (signals parent process)
    ///
    /// Note: Planner agents (PRD generators) should not normally call goodbye.
    /// They produce artifacts but don't run long-lived sessions. If a planner
    /// agent must terminate, use --force.
    Goodbye {
        /// Optional reason for termination
        reason: Option<String>,

        /// Log goodbye without actually terminating (for testing)
        #[arg(long)]
        dry_run: bool,

        /// Force termination even for planner agents (normally disallowed)
        #[arg(long)]
        force: bool,
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

    /// Queue management commands (work prioritization)
    Queue {
        #[command(subcommand)]
        command: QueueCommands,
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
    Ready {
        /// Show only bugs (exclude tasks)
        #[arg(long)]
        bugs_only: bool,

        /// Show only tasks (exclude bugs)
        #[arg(long)]
        tasks_only: bool,
    },

    /// Show tasks waiting on dependencies
    Blocked {
        /// Show only bugs (exclude tasks)
        #[arg(long)]
        bugs_only: bool,

        /// Show only tasks (exclude bugs)
        #[arg(long)]
        tasks_only: bool,
    },

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

        /// Automatically fix issues that can be repaired
        #[arg(long)]
        fix: bool,
    },

    /// Show audit trail of changes
    Log {
        /// Optional task ID to filter logs
        task_id: Option<String>,
    },

    /// Push/pull binnacle data with remote (requires orphan-branch backend)
    Sync {
        /// Remote name (default: origin)
        #[arg(short, long)]
        remote: Option<String>,

        /// Only push, don't pull
        #[arg(long, conflicts_with = "pull")]
        push: bool,

        /// Only pull, don't push
        #[arg(long, conflicts_with = "push")]
        pull: bool,
    },

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

    /// Agent lifecycle management
    Agent {
        #[command(subcommand)]
        command: AgentCommands,
    },

    /// Container management commands (requires containerd/buildah)
    Container {
        #[command(subcommand)]
        command: ContainerCommands,
    },

    /// Start the web GUI (requires 'gui' feature)
    #[cfg(feature = "gui")]
    Gui {
        /// Port to listen on (default: 3030, or BN_GUI_PORT env var)
        #[arg(short, long, env = "BN_GUI_PORT", default_value = "3030")]
        port: u16,

        /// Host address to bind to (default: 0.0.0.0 for network access, use 127.0.0.1 for local only)
        #[arg(long, env = "BN_GUI_HOST", default_value = "0.0.0.0")]
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

        /// Add to work queue immediately after creation
        #[arg(short = 'q', long)]
        queue: bool,
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

        /// Short display name (shown in GUI instead of ID)
        #[arg(short = 's', long)]
        short_name: Option<String>,

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

        /// Add to work queue immediately after creation
        #[arg(short = 'q', long)]
        queue: bool,
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

        /// New short display name for GUI (recommended: 1-2 words, ~12 chars max)
        #[arg(short = 's', long)]
        short_name: Option<String>,

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

        /// Force status update even if agent already has active tasks
        #[arg(long)]
        force: bool,
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

        /// Short display name (shown in GUI instead of ID)
        #[arg(short = 's', long)]
        short_name: Option<String>,

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
        /// Idea ID (e.g., bn-a1b2)
        id: String,
    },

    /// Update an idea
    Update {
        /// Idea ID
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

    /// Promote an idea to a task or PRD
    Promote {
        /// Idea ID
        id: String,

        /// Generate a PRD file instead of creating a task
        #[arg(long)]
        as_prd: bool,

        /// Priority for the new task (0-4, lower is higher priority)
        #[arg(short, long)]
        priority: Option<u8>,
    },

    /// Mark an idea as germinating (being developed)
    Germinate {
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

        /// Short display name (shown in GUI instead of ID)
        #[arg(short = 's', long)]
        short_name: Option<String>,

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

        /// New short display name for GUI (recommended: 1-2 words, ~12 chars max)
        #[arg(short = 's', long)]
        short_name: Option<String>,

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

/// Queue subcommands (work prioritization)
#[derive(Subcommand, Debug)]
pub enum QueueCommands {
    /// Create a new queue (only one per repository)
    Create {
        /// Queue title (e.g., "Sprint 1", "Urgent")
        title: String,

        /// Optional description
        #[arg(short, long)]
        description: Option<String>,
    },

    /// Show the queue and its tasks
    Show,

    /// Delete the queue
    Delete,

    /// Add a task or bug to the queue
    Add {
        /// Task or bug ID to add to queue (e.g., bn-xxxx)
        item_id: String,
    },

    /// Remove a task or bug from the queue
    Rm {
        /// Task or bug ID to remove from queue (e.g., bn-xxxx)
        item_id: String,
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
        #[arg(long = "type", short = 't', value_parser = ["depends_on", "blocks", "related_to", "duplicates", "fixes", "caused_by", "supersedes", "parent_of", "child_of", "tests", "queued", "impacts"])]
        edge_type: String,
        /// Reason for creating this relationship (required for depends_on)
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
        #[arg(long = "type", short = 't', value_parser = ["depends_on", "blocks", "related_to", "duplicates", "fixes", "caused_by", "supersedes", "parent_of", "child_of", "tests", "queued", "impacts"])]
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
        #[arg(long = "type", short = 't', value_parser = ["depends_on", "blocks", "related_to", "duplicates", "fixes", "caused_by", "supersedes", "parent_of", "child_of", "tests", "queued", "impacts"])]
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

        /// Link to a bug (for verifying bug fixes)
        #[arg(long)]
        bug: Option<String>,
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

    /// Link a test to a bug
    LinkBug {
        /// Test ID
        test_id: String,
        /// Bug ID
        bug_id: String,
    },

    /// Unlink a test from a bug
    UnlinkBug {
        /// Test ID
        test_id: String,
        /// Bug ID
        bug_id: String,
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
    /// Link a commit to a task or bug
    Link {
        /// Commit SHA
        sha: String,
        /// Entity ID (task bn-xxxx or bug bnb-xxxx)
        entity_id: String,
    },

    /// Unlink a commit from a task or bug
    Unlink {
        /// Commit SHA
        sha: String,
        /// Entity ID (task bn-xxxx or bug bnb-xxxx)
        entity_id: String,
    },

    /// List commits linked to a task or bug
    List {
        /// Entity ID (task bn-xxxx or bug bnb-xxxx)
        entity_id: String,
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
        #[arg(long = "type", short = 't', value_parser = ["depends_on", "blocks", "related_to", "duplicates", "fixes", "caused_by", "supersedes", "parent_of", "child_of", "tests", "queued", "impacts"])]
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
    Init {
        /// Write binnacle section to AGENTS.md (creates file if needed)
        #[arg(long)]
        write_agents_md: bool,

        /// Write Claude Code skills file to ~/.claude/skills/binnacle/SKILL.md
        #[arg(long)]
        write_claude_skills: bool,

        /// Write Codex skills file to ~/.codex/skills/binnacle/SKILL.md
        #[arg(long)]
        write_codex_skills: bool,

        /// Write Copilot workflow agents to .github/agents/ and .github/instructions/
        #[arg(long)]
        write_copilot_prompts: bool,

        /// Install commit-msg hook for co-author attribution
        #[arg(long)]
        install_hook: bool,

        /// Skip interactive prompts (use flags to control what gets written)
        #[arg(long, short = 'y')]
        yes: bool,
    },

    /// Data store management (import/export/inspect)
    Store {
        #[command(subcommand)]
        command: StoreCommands,
    },

    /// Emit embedded templates to stdout (no side effects, no init required)
    Emit {
        /// Which template to emit
        #[arg(value_enum)]
        template: EmitTemplate,
    },

    /// Migrate data between storage backends
    Migrate {
        /// Target backend type (file, orphan-branch, git-notes)
        #[arg(long)]
        to: String,

        /// Preview migration without making changes
        #[arg(long)]
        dry_run: bool,
    },

    /// Manage git hooks installed by binnacle
    Hooks {
        #[command(subcommand)]
        command: HooksCommands,
    },

    /// Convert tasks tagged as 'bug' into Bug entities
    MigrateBugs {
        /// Preview migration without making changes
        #[arg(long)]
        dry_run: bool,

        /// Delete the 'bug' tag from converted tasks (default: keep tag for reference)
        #[arg(long)]
        remove_tag: bool,
    },
}

/// Hooks management subcommands
#[derive(Subcommand, Debug)]
pub enum HooksCommands {
    /// Uninstall binnacle hooks from this repository
    Uninstall,
}

/// Template types for the emit command
#[derive(Clone, Debug, clap::ValueEnum)]
pub enum EmitTemplate {
    /// AGENTS.md binnacle section content
    Agents,
    /// SKILL.md file content
    Skill,
    /// Binnacle plan agent prompt (binnacle-plan.prompt.md)
    PlanAgent,
    /// Binnacle PRD agent prompt (binnacle-prd.prompt.md)
    PrdAgent,
    /// Binnacle tasks agent prompt (binnacle-tasks.prompt.md)
    TasksAgent,
    /// Binnacle Copilot instructions (binnacle.instructions.md)
    CopilotInstructions,
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

    /// Dump all JSONL files to console with headers
    Dump,

    /// Clear all data from the current repository's store
    Clear {
        /// Skip confirmation prompt (REQUIRED for non-interactive use)
        #[arg(long)]
        force: bool,

        /// Skip creating backup before clearing
        #[arg(long)]
        no_backup: bool,
    },
}

/// Agent lifecycle management subcommands
#[derive(Subcommand, Debug)]
pub enum AgentCommands {
    /// List active agents
    List {
        /// Filter by status (active, idle, stale)
        #[arg(long)]
        status: Option<String>,
    },

    /// Terminate a specific agent by PID or name
    Kill {
        /// Agent identifier (PID or name)
        target: String,

        /// Seconds to wait after SIGTERM before sending SIGKILL
        #[arg(long, default_value = "5")]
        timeout: u64,
    },
}

/// Container management subcommands (requires containerd/buildah)
#[derive(Subcommand, Debug)]
pub enum ContainerCommands {
    /// Build the binnacle worker image using buildah
    Build {
        /// Image tag (default: latest)
        #[arg(short, long, default_value = "latest")]
        tag: String,

        /// Force rebuild without cache
        #[arg(long)]
        no_cache: bool,
    },

    /// Run a worker container in headed (interactive) mode
    Run {
        /// Path to git worktree to mount (required)
        worktree_path: String,

        /// Agent type: worker, planner, buddy (default: worker)
        #[arg(long = "type", short = 't', default_value = "worker", value_parser = ["worker", "planner", "buddy"])]
        agent_type: String,

        /// Container name (auto-generated if not provided)
        #[arg(long)]
        name: Option<String>,

        /// Branch to merge into on exit (default: main)
        #[arg(long, default_value = "main")]
        merge_target: String,

        /// Disable auto-merge on exit
        #[arg(long)]
        no_merge: bool,

        /// Run in background (non-headed mode)
        #[arg(long)]
        detach: bool,

        /// CPU limit (e.g., 1.5 for 1.5 CPUs, 0.5 for half a CPU)
        #[arg(long)]
        cpus: Option<f64>,

        /// Memory limit (e.g., "512m", "1g", "2048m")
        #[arg(long)]
        memory: Option<String>,
    },

    /// Stop a running binnacle container
    Stop {
        /// Container name (omit for --all)
        name: Option<String>,

        /// Stop all binnacle containers
        #[arg(long)]
        all: bool,
    },

    /// List binnacle containers
    List {
        /// Show all containers (including stopped)
        #[arg(long)]
        all: bool,

        /// Only show container names
        #[arg(long)]
        quiet: bool,
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
