//! CLI argument definitions for Binnacle.

use clap::{Parser, Subcommand};

/// Build timestamp injected by build.rs
const BUILD_TIMESTAMP: &str = env!("BN_BUILD_TIMESTAMP");

/// Git commit hash injected by build.rs
const GIT_COMMIT: &str = env!("BN_GIT_COMMIT");

/// Copilot version injected by build.rs
const COPILOT_VERSION: &str = env!("BN_COPILOT_VERSION");

/// Get build timestamp (public accessor for build metadata)
pub fn build_timestamp() -> &'static str {
    BUILD_TIMESTAMP
}

/// Get git commit hash (public accessor for build metadata)
pub fn git_commit() -> &'static str {
    GIT_COMMIT
}

/// Get Copilot version (public accessor for build metadata)
pub fn copilot_version() -> &'static str {
    COPILOT_VERSION
}

/// Get package version
pub fn package_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Build version string with timestamp and commit
fn build_version() -> &'static str {
    // Use a static to ensure we only format this once
    static VERSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    VERSION.get_or_init(|| {
        format!(
            "{} ({} {})",
            env!("CARGO_PKG_VERSION"),
            GIT_COMMIT,
            BUILD_TIMESTAMP
        )
    })
}

/// Binnacle - A project state tracking tool for AI agents and humans.
///
/// Start with `bn orient` to understand project state, then `bn ready` to find work.
#[derive(Parser, Debug)]
#[command(name = "bn")]
#[command(author, version = build_version(), about = "A CLI tool for AI agents and humans to track project state", long_about = None)]
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
        /// Agent type (required): worker, planner, buddy, or ask
        #[arg(long = "type", short = 't', value_parser = ["worker", "planner", "buddy", "ask"], value_name = "worker|planner|buddy|ask")]
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
        /// Skip agent registration (for testing)
        #[arg(long)]
        dry_run: bool,
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

    /// Documentation node commands (attach markdown docs to entities)
    Doc {
        #[command(subcommand)]
        command: DocCommands,
    },

    /// Milestone management commands
    Milestone {
        #[command(subcommand)]
        command: MilestoneCommands,
    },

    /// Mission management commands (high-level organizational units above milestones)
    Mission {
        #[command(subcommand)]
        command: MissionCommands,
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

        /// Migrate old .tar.gz archives to new .bng format (non-destructive)
        #[arg(long)]
        fix_archives: bool,
    },

    /// Show audit trail of changes or export action logs
    Log {
        #[command(subcommand)]
        command: Option<LogCommands>,

        /// Optional task ID to filter audit log (shorthand for `bn log show <task_id>`)
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
    #[command(
        long_about = "Container management commands (requires containerd/buildah)

ROOTLESS SETUP (recommended to avoid sudo):

By default, binnacle uses system containerd which requires 'sudo'. For a better experience
without sudo, set up rootless containerd:

1. Install containerd with rootless support:
   # Fedora/RHEL
   sudo dnf install containerd rootlesskit slirp4netns

   # Debian/Ubuntu
   sudo apt install containerd rootlesskit slirp4netns uidmap

2. Set up subuid/subgid ranges for your user:
   sudo usermod --add-subuids 100000-165535 --add-subgids 100000-165535 $USER

3. Enable and start rootless containerd:
   containerd-rootless-setuptool.sh install

   # Or use systemd user service:
   systemctl --user enable --now containerd
   loginctl enable-linger $USER

4. Verify the socket exists:
   ls $XDG_RUNTIME_DIR/containerd/containerd.sock

For detailed instructions, see: container/README.md
"
    )]
    Container {
        #[command(subcommand)]
        command: ContainerCommands,
    },

    /// Web GUI management commands (requires 'gui' feature)
    #[cfg(feature = "gui")]
    Gui {
        #[command(subcommand)]
        command: Option<GuiCommands>,

        /// Port to listen on (default: auto-select starting from 3030, or BN_GUI_PORT env var)
        /// Used when no subcommand is given (starts server)
        #[arg(short, long, env = "BN_GUI_PORT", global = true)]
        port: Option<u16>,

        /// Host address to bind to (default: 0.0.0.0 for network access, use 127.0.0.1 for local only)
        /// Used when no subcommand is given (starts server)
        #[arg(long, env = "BN_GUI_HOST", default_value = "0.0.0.0", global = true)]
        host: String,

        /// Start in readonly mode (disables all write operations)
        /// Used when no subcommand is given (starts server)
        #[arg(long, env = "BN_GUI_READONLY", global = true)]
        readonly: bool,

        /// Development mode: serve assets from filesystem instead of embedded bundle
        /// Used when no subcommand is given (starts server)
        #[arg(long, global = true)]
        dev: bool,

        /// Create a public URL via devtunnel
        /// Used when no subcommand is given (starts server)
        #[arg(long, env = "BN_GUI_TUNNEL", global = true)]
        tunnel: bool,

        /// Load from a .bng archive file (imports to temp directory and serves from there)
        #[arg(long, global = true)]
        archive: Option<String>,
    },

    /// Run agent supervisor daemon (continuously reconciles agent counts)
    ///
    /// Monitors agent scaling configuration and spawns/stops containers
    /// to match desired counts. Prints status updates on each reconciliation
    /// or every 30 seconds when idle.
    ///
    /// IMPORTANT: Run with sudo for system containerd access:
    ///     sudo bn serve
    ///
    /// When running via sudo, binnacle automatically:
    /// 1. Detects SUDO_USER environment variable
    /// 2. Opens the containerd socket while elevated
    /// 3. Drops privileges back to your user
    /// 4. Creates all files with your ownership (not root)
    ///
    /// For rootless operation without sudo, see:
    ///     container/README.md#rootless-setup
    Serve {
        /// Reconciliation interval in seconds (default: 10)
        #[arg(long, default_value = "10")]
        interval: u64,

        /// Show what would be done without making changes
        #[arg(long)]
        dry_run: bool,
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
        #[arg(short, long, visible_alias = "desc")]
        description: Option<String>,

        /// Add to work queue immediately after creation
        #[arg(short = 'q', long)]
        queue: bool,

        /// Check for complexity and suggest filing as idea if detected
        /// (useful for buddy agents to soft-gate complex task descriptions)
        #[arg(long)]
        check_complexity: bool,

        /// Force task creation even if complexity is detected
        #[arg(long)]
        force: bool,
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
        #[arg(long, visible_alias = "desc")]
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

        /// Allow updating a closed task without reopening it
        #[arg(long)]
        keep_closed: bool,

        /// Reopen a closed task and set status to pending
        #[arg(long)]
        reopen: bool,
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
        #[arg(short, long, visible_alias = "desc")]
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

        /// Include closed bugs (done/cancelled) in the list
        #[arg(long)]
        all: bool,
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
        #[arg(long, visible_alias = "desc")]
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

        /// Allow updating a closed bug without reopening it
        #[arg(long)]
        keep_closed: bool,

        /// Reopen a closed bug and set status to pending
        #[arg(long)]
        reopen: bool,
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
        #[arg(short, long, visible_alias = "desc")]
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
        #[arg(long, visible_alias = "desc")]
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

/// Documentation node subcommands
#[derive(Subcommand, Debug)]
pub enum DocCommands {
    /// Create a new documentation node linked to entities
    ///
    /// Docs must be linked to at least one entity on creation.
    /// Content can be provided inline, from a file, or from stdin.
    Create {
        /// Entity IDs to link this doc to (at least one required)
        #[arg(required = true)]
        entity_ids: Vec<String>,

        /// Doc title
        #[arg(short = 'T', long)]
        title: String,

        /// Doc type (prd, note, handoff)
        #[arg(long = "type", short = 'y', default_value = "note")]
        doc_type: String,

        /// Short display name (shown in GUI instead of ID)
        #[arg(short = 's', long)]
        short_name: Option<String>,

        /// Inline markdown content
        #[arg(short = 'c', long)]
        content: Option<String>,

        /// Read content from file
        #[arg(long, conflicts_with = "content")]
        file: Option<std::path::PathBuf>,

        /// Read content from stdin
        #[arg(long, conflicts_with_all = ["content", "file"])]
        stdin: bool,

        /// Agent-provided summary (prepended as # Summary section)
        #[arg(long)]
        short: Option<String>,

        /// Tags for the doc
        #[arg(short, long)]
        tag: Vec<String>,
    },

    /// Show a documentation node
    Show {
        /// Doc ID (e.g., bn-a1b2)
        id: String,

        /// Show full content (syntax-highlighted markdown) instead of just summary
        #[arg(long)]
        full: bool,
    },

    /// List documentation nodes
    List {
        /// Filter by tag
        #[arg(long)]
        tag: Option<String>,

        /// Filter by doc type (prd, note, handoff)
        #[arg(long, value_name = "TYPE")]
        doc_type: Option<String>,

        /// Filter by editor (format: "agent:id" or "user:name")
        #[arg(long, value_name = "EDITOR")]
        edited_by: Option<String>,

        /// Filter by linked entity ID
        #[arg(long, value_name = "ENTITY_ID", visible_alias = "for")]
        for_entity: Option<String>,
    },

    /// Edit/update a documentation node
    Edit {
        /// Doc ID
        id: String,

        /// New title
        #[arg(short = 'T', long)]
        title: Option<String>,

        /// New short name
        #[arg(short = 's', long)]
        short_name: Option<String>,

        /// New description
        #[arg(short, long, visible_alias = "desc")]
        description: Option<String>,

        /// New content (full replacement)
        #[arg(short, long)]
        content: Option<String>,

        /// Add a tag
        #[arg(long)]
        add_tag: Vec<String>,

        /// Remove a tag
        #[arg(long)]
        remove_tag: Vec<String>,
    },

    /// Create a new version of a doc (preserves history via supersedes chain)
    Update {
        /// Doc ID to update
        id: String,

        /// New content (full replacement)
        #[arg(short = 'c', long)]
        content: Option<String>,

        /// Read content from file
        #[arg(long, conflicts_with = "content")]
        file: Option<std::path::PathBuf>,

        /// Read content from stdin
        #[arg(long, conflicts_with_all = ["content", "file"])]
        stdin: bool,

        /// New title
        #[arg(short = 'T', long)]
        title: Option<String>,

        /// New short name
        #[arg(short = 's', long)]
        short_name: Option<String>,

        /// New description
        #[arg(short, long, visible_alias = "desc")]
        description: Option<String>,

        /// Editor attribution (format: "agent:id" or "user:name")
        #[arg(long)]
        editor: Option<String>,

        /// Clear the summary_dirty flag
        #[arg(long)]
        clear_dirty: bool,
    },

    /// Show version history for a doc
    History {
        /// Doc ID
        id: String,
    },

    /// Attach a doc to another entity (creates 'documents' edge)
    Attach {
        /// Doc ID (e.g., bn-a1b2)
        doc_id: String,

        /// Target entity ID (e.g., bn-a1b2, bnt-c3d4)
        target_id: String,
    },

    /// Detach a doc from an entity (removes 'documents' edge)
    Detach {
        /// Doc ID (e.g., bn-a1b2)
        doc_id: String,

        /// Target entity ID (e.g., bn-a1b2, bnt-c3d4)
        target_id: String,
    },

    /// Delete a documentation node
    Delete {
        /// Doc ID
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
        #[arg(short, long, visible_alias = "desc")]
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
        #[arg(long, visible_alias = "desc")]
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

/// Mission subcommands
#[derive(Debug, Subcommand)]
pub enum MissionCommands {
    /// Create a new mission
    Create {
        /// Mission title
        title: String,

        /// Short display name (shown in GUI instead of ID)
        #[arg(short = 's', long)]
        short_name: Option<String>,

        /// Priority (0-4, lower is higher priority)
        #[arg(short, long)]
        priority: Option<u8>,

        /// Tags for the mission
        #[arg(short, long)]
        tag: Vec<String>,

        /// Assignee
        #[arg(short, long)]
        assignee: Option<String>,

        /// Mission description
        #[arg(short, long, visible_alias = "desc")]
        description: Option<String>,

        /// Target due date (ISO 8601 format, e.g., 2026-02-01T00:00:00Z)
        #[arg(long)]
        due_date: Option<String>,
    },

    /// List missions
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

    /// Show mission details with progress
    Show {
        /// Mission ID (e.g., bn-xxxx with type mission)
        id: String,
    },

    /// Update a mission (status: pending, in_progress, partial, blocked)
    Update {
        /// Mission ID
        id: String,

        /// New title
        #[arg(long)]
        title: Option<String>,

        /// New short display name for GUI (recommended: 1-2 words, ~12 chars max)
        #[arg(short = 's', long)]
        short_name: Option<String>,

        /// New description
        #[arg(long, visible_alias = "desc")]
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

    /// Close a mission (marks as done)
    Close {
        /// Mission ID
        id: String,

        /// Reason for closing (describe what was accomplished)
        #[arg(long)]
        reason: Option<String>,

        /// Force close even with incomplete children (use with caution)
        #[arg(long)]
        force: bool,
    },

    /// Reopen a closed mission
    Reopen {
        /// Mission ID
        id: String,
    },

    /// Delete a mission
    Delete {
        /// Mission ID
        id: String,
    },

    /// Show progress for a mission
    Progress {
        /// Mission ID
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
        #[arg(short, long, visible_alias = "desc")]
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

/// GUI subcommands (web interface management)
#[cfg(feature = "gui")]
#[derive(Subcommand, Debug)]
pub enum GuiCommands {
    /// Start the web GUI server (default if no subcommand given)
    #[command(name = "serve", visible_alias = "start")]
    Serve {
        /// Port to listen on (default: auto-select starting from 3030, or BN_GUI_PORT env var)
        #[arg(short, long, env = "BN_GUI_PORT")]
        port: Option<u16>,

        /// Host address to bind to (default: 0.0.0.0 for network access, use 127.0.0.1 for local only)
        #[arg(long, env = "BN_GUI_HOST", default_value = "0.0.0.0")]
        host: String,

        /// Stop any running GUI server first and start a new one
        #[arg(long)]
        replace: bool,

        /// Start in readonly mode (disables all write operations)
        #[arg(long, env = "BN_GUI_READONLY")]
        readonly: bool,

        /// Development mode: serve assets from filesystem instead of embedded bundle
        #[arg(long)]
        dev: bool,

        /// Create a public URL via devtunnel
        #[arg(long, env = "BN_GUI_TUNNEL")]
        tunnel: bool,
    },

    /// Show status of running GUI server
    Status,

    /// Stop a running GUI server gracefully (SIGTERM, then SIGKILL after timeout)
    Stop {
        /// Force immediate termination with SIGKILL (skip SIGTERM, 500ms wait)
        #[arg(short = '9', long)]
        force: bool,
    },

    /// Kill a running GUI server immediately
    Kill {
        /// Force immediate termination with SIGKILL (same as -9)
        #[arg(short = '9', long)]
        force: bool,
    },

    /// Export static viewer bundle (for hosting on GitHub Pages, etc.)
    Export {
        /// Output directory (default: target/static-viewer/)
        #[arg(long, short = 'o', default_value = "target/static-viewer")]
        output: String,

        /// Archive to embed (default: creates archive from current state)
        #[arg(long)]
        archive: Option<String>,
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
        #[arg(long = "type", short = 't', value_parser = ["depends_on", "blocks", "related_to", "duplicates", "fixes", "caused_by", "supersedes", "parent_of", "child_of", "tests", "queued", "impacts", "documents"])]
        edge_type: String,
        /// Reason for creating this relationship (required for depends_on)
        #[arg(long)]
        reason: Option<String>,
        /// Pin this edge to a specific version (won't transfer when doc is updated)
        #[arg(long)]
        pinned: bool,
    },

    /// Remove a link between two entities
    Rm {
        /// Source entity ID
        source: String,
        /// Target entity ID
        target: String,
        /// Type of relationship (required)
        #[arg(long = "type", short = 't', value_parser = ["depends_on", "blocks", "related_to", "duplicates", "fixes", "caused_by", "supersedes", "parent_of", "child_of", "tests", "queued", "impacts", "documents"])]
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
        #[arg(long = "type", short = 't', value_parser = ["depends_on", "blocks", "related_to", "duplicates", "fixes", "caused_by", "supersedes", "parent_of", "child_of", "tests", "queued", "impacts", "documents"])]
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
        /// Entity ID (task or bug, e.g., bn-a1b2)
        entity_id: String,
    },

    /// Unlink a commit from a task or bug
    Unlink {
        /// Commit SHA
        sha: String,
        /// Entity ID (task or bug, e.g., bn-a1b2)
        entity_id: String,
    },

    /// List commits linked to a task or bug
    List {
        /// Entity ID (task or bug, e.g., bn-a1b2)
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
    Serve {
        /// Pre-set the working directory (makes binnacle-set_agent optional)
        #[arg(long)]
        cwd: Option<std::path::PathBuf>,
    },

    /// Output tool definitions
    Manifest,
}

/// Log subcommands
#[derive(Subcommand, Debug)]
pub enum LogCommands {
    /// Show audit trail of changes (entity change history)
    Show {
        /// Optional task ID to filter logs
        task_id: Option<String>,
    },

    /// Export action logs to file
    Export {
        /// Export format: json, csv, or markdown
        #[arg(short, long, default_value = "json", value_parser = ["json", "csv", "markdown"])]
        format: String,

        /// Filter by command name (partial match)
        #[arg(long)]
        command: Option<String>,

        /// Filter by user name (exact match)
        #[arg(long)]
        user: Option<String>,

        /// Filter by success status (true/false)
        #[arg(long)]
        success: Option<bool>,

        /// Filter for entries after this ISO 8601 timestamp
        #[arg(long)]
        after: Option<String>,

        /// Filter for entries before this ISO 8601 timestamp
        #[arg(long)]
        before: Option<String>,

        /// Maximum number of entries to export (default: all)
        #[arg(short = 'n', long)]
        limit: Option<u32>,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Compact action logs by enforcing retention limits
    ///
    /// Deletes old entries based on configured retention settings.
    /// Uses `action_log_max_entries` and `action_log_max_age_days` config keys.
    Compact {
        /// Override max entries to keep (ignores config if set)
        #[arg(long)]
        max_entries: Option<u32>,

        /// Override max age in days (ignores config if set)
        #[arg(long)]
        max_age_days: Option<u32>,

        /// Show what would be deleted without actually deleting
        #[arg(long)]
        dry_run: bool,
    },
}

/// Graph analysis subcommands
#[derive(Subcommand, Debug)]
pub enum GraphCommands {
    /// Analyze task graph for disconnected components
    Components,

    /// Walk ancestry chain from a task up to its PRD document
    Lineage {
        /// Entity ID to find lineage for
        id: String,

        /// Maximum hops to traverse (default: 10)
        #[arg(long, default_value = "10")]
        depth: usize,

        /// Include descriptions in output
        #[arg(long)]
        verbose: bool,
    },

    /// Find sibling and cousin tasks through shared parents
    Peers {
        /// Entity ID to find peers for
        id: String,

        /// Peer depth: 1=siblings only, 2=siblings+cousins (default: 1)
        #[arg(long, default_value = "1")]
        depth: usize,

        /// Include closed/done tasks
        #[arg(long)]
        include_closed: bool,

        /// Include descriptions in output
        #[arg(long)]
        verbose: bool,
    },

    /// Explore subtree below a node
    Descendants {
        /// Entity ID to explore
        id: String,

        /// Maximum depth to explore (default: 3)
        #[arg(long, default_value = "3")]
        depth: usize,

        /// Show all descendants regardless of depth
        #[arg(long)]
        all: bool,

        /// Include closed/done tasks
        #[arg(long)]
        include_closed: bool,

        /// Include descriptions in output
        #[arg(long)]
        verbose: bool,
    },
}

/// Search subcommands
#[derive(Subcommand, Debug)]
pub enum SearchCommands {
    /// Search for links/edges by type, source, or target
    Link {
        /// Filter by edge type
        #[arg(long = "type", short = 't', value_parser = ["depends_on", "blocks", "related_to", "duplicates", "fixes", "caused_by", "supersedes", "parent_of", "child_of", "tests", "queued", "impacts", "documents"])]
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

        /// Write VS Code MCP config to .vscode/mcp.json in the repository
        #[arg(long)]
        write_mcp_vscode: bool,

        /// Write GitHub Copilot CLI MCP config to ~/.copilot/mcp-config.json
        #[arg(long)]
        write_mcp_copilot: bool,

        /// Write all MCP configs (VS Code, Copilot CLI)
        #[arg(long)]
        write_mcp_all: bool,

        /// Install GitHub Copilot CLI with binnacle-preferred version
        #[arg(long)]
        install_copilot: bool,

        /// Install bn-agent script to ~/.local/bin/bn-agent
        #[arg(long)]
        install_bn_agent: bool,

        /// Build binnacle container image if not already built
        #[arg(long)]
        build_container: bool,

        /// Skip interactive prompts (use flags to control what gets written)
        #[arg(long, short = 'y')]
        yes: bool,
    },

    /// Re-run full interactive initialization (global + repo)
    ///
    /// Use this to reconfigure global binnacle setup (skills files, MCP configs,
    /// Copilot CLI installation) or if you want to change previous answers.
    Reinit,

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

    /// Display build metadata (timestamp, commit hash)
    BuildInfo,

    /// Manage GitHub Copilot CLI binaries
    Copilot {
        #[command(subcommand)]
        command: CopilotCommands,
    },

    /// Tmux layout management (requires --features tmux)
    #[cfg(feature = "tmux")]
    Tmux {
        #[command(subcommand)]
        command: TmuxCommands,
    },
}

/// Copilot management subcommands
#[derive(Subcommand, Debug)]
pub enum CopilotCommands {
    /// Download and install a specific Copilot CLI version
    Install {
        /// Version to install (e.g., v0.0.396). Use --upstream for binnacle-preferred version.
        #[arg(conflicts_with = "upstream")]
        version: Option<String>,

        /// Install the binnacle-preferred version (from COPILOT_VERSION file)
        #[arg(long)]
        upstream: bool,
    },
    /// Print path to the active Copilot CLI binary
    Path,
    /// List all installed Copilot CLI versions with active indicator
    Version,
}

/// Tmux layout management subcommands (feature-gated)
#[cfg(feature = "tmux")]
#[derive(Subcommand, Debug)]
pub enum TmuxCommands {
    /// Save current tmux layout to a KDL file
    Save {
        /// Layout name (default: current session name)
        name: Option<String>,
        /// Save to project-level (.binnacle/tmux/)
        #[arg(long, conflicts_with = "user")]
        project: bool,
        /// Save to user-level (~/.config/binnacle/tmux/)
        #[arg(long)]
        user: bool,
    },
    /// Load a tmux layout from a KDL file
    Load {
        /// Layout name to load
        name: String,
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
    /// Auto-worker agent prompt (picks from bn ready)
    AutoWorker,
    /// Directed task agent prompt (works on specific description)
    DoAgent,
    /// PRD writer agent prompt (renders ideas into PRDs)
    PrdWriter,
    /// Buddy agent prompt (quick entry for bugs/tasks/ideas)
    Buddy,
    /// Free agent prompt (general purpose with binnacle)
    Free,
    /// Ask agent prompt (read-only interactive Q&A)
    AskAgent,
    /// Claude Desktop MCP configuration JSON
    McpClaude,
    /// VS Code MCP configuration JSON
    McpVscode,
    /// GitHub Copilot CLI MCP configuration JSON
    McpCopilot,
    /// MCP lifecycle guidance for worker agents (orient + goodbye must use shell)
    McpLifecycle,
    /// MCP lifecycle guidance for planner agents (orient only, no goodbye)
    McpLifecyclePlanner,
    /// bn-agent unified agent launcher script
    BnAgent,
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
        /// Input path: archive file (.bng, .tar.zst, or .tar.gz), storage folder, or '-' for stdin
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

    /// Generate archive for a commit snapshot (used by hooks)
    Archive {
        /// Git commit hash to archive
        commit_hash: String,
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

    /// View or set min/max scaling configuration for agent types
    Scale {
        /// Agent type (worker, planner, buddy). If omitted, shows all types.
        #[arg(value_parser = ["worker", "planner", "buddy"])]
        agent_type: Option<String>,

        /// Minimum number of agents to maintain
        #[arg(long)]
        min: Option<u32>,

        /// Maximum number of agents to allow
        #[arg(long)]
        max: Option<u32>,
    },

    /// Remove agent(s) from the registry (bypasses min count)
    Rm {
        /// Agent identifier (ID, PID, or name). Required unless --all is used.
        target: Option<String>,

        /// Force removal even if process is still running (sends SIGKILL immediately)
        #[arg(long)]
        force: bool,

        /// Remove all agents of the specified type (requires --type)
        #[arg(long)]
        all: bool,

        /// Agent type filter for --all (worker, planner, buddy)
        #[arg(long = "type", short = 't', value_parser = ["worker", "planner", "buddy"])]
        agent_type: Option<String>,
    },

    /// Manually spawn an agent container (bypasses min/max scaling)
    Spawn {
        /// Agent type: worker, planner, buddy
        #[arg(value_parser = ["worker", "planner", "buddy"])]
        agent_type: String,

        /// Custom name for the agent (auto-generated if not provided)
        #[arg(long)]
        name: Option<String>,

        /// CPU limit (e.g., 1.5 for 1.5 CPUs, 0.5 for half a CPU)
        #[arg(long)]
        cpus: Option<f64>,

        /// Memory limit (e.g., "512m", "1g", "2048m")
        #[arg(long)]
        memory: Option<String>,

        /// Path to git worktree to use (defaults to current repo's directory)
        #[arg(long)]
        worktree: Option<String>,

        /// Branch to merge into on exit (default: main)
        #[arg(long, default_value = "main")]
        merge_target: String,

        /// Disable auto-merge on exit
        #[arg(long)]
        no_merge: bool,

        /// Custom initial prompt for the AI agent
        #[arg(long)]
        prompt: Option<String>,
    },

    /// Run reconciliation loop once (spawn/stop containers to match desired counts)
    Reconcile {
        /// Show what would be done without making changes
        #[arg(long)]
        dry_run: bool,
    },
}

/// Container management subcommands (requires containerd/buildah)
#[derive(Subcommand, Debug)]
pub enum ContainerCommands {
    /// Build container image(s) using buildah
    ///
    /// With no arguments, lists available definitions.
    /// With a definition name, builds that definition and its dependencies.
    /// With --all, builds all available definitions in dependency order.
    Build {
        /// Container definition name to build (omit to list definitions)
        definition: Option<String>,

        /// Build all definitions in dependency order
        #[arg(long, conflicts_with = "definition")]
        all: bool,

        /// Image tag (default: latest)
        #[arg(short, long, default_value = "latest")]
        tag: String,

        /// Force rebuild without cache
        #[arg(long)]
        no_cache: bool,

        /// Skip mount validation (useful for CI environments)
        #[arg(long)]
        skip_mount_validation: bool,

        /// Use project-level definition (.binnacle/containers/) when name conflicts exist
        #[arg(long, conflicts_with = "host")]
        project: bool,

        /// Use host-level definition (~/.local/share/binnacle/<hash>/containers/) when name conflicts exist
        #[arg(long, conflicts_with = "project")]
        host: bool,
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

        /// Mount workspace as read-only (sets BN_READONLY_WORKSPACE=true)
        #[arg(long)]
        readonly_workspace: bool,

        /// CPU limit (e.g., 1.5 for 1.5 CPUs, 0.5 for half a CPU)
        #[arg(long)]
        cpus: Option<f64>,

        /// Memory limit (e.g., "512m", "1g", "2048m")
        #[arg(long)]
        memory: Option<String>,

        /// Start an interactive shell instead of the AI agent
        #[arg(long)]
        shell: bool,

        /// Custom initial prompt for the AI agent
        #[arg(long)]
        prompt: Option<String>,

        /// Container definition to use (default: first in config.kdl or embedded binnacle)
        #[arg(long)]
        definition: Option<String>,

        /// Use project-level definition (.binnacle/containers/) when name conflicts exist
        #[arg(long, conflicts_with = "host")]
        project: bool,

        /// Use host-level definition (~/.local/share/binnacle/<hash>/containers/) when name conflicts exist
        #[arg(long, conflicts_with = "project")]
        host: bool,
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

    /// List all container definitions
    ///
    /// Shows container definitions from all sources (project-level, user-level, embedded)
    /// with their origin, description, and parent chain.
    ListDefinitions,
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
