# binnacle

A CLI tool for AI agents and humans to track project state. Think of it as a lightweight, JSON-first task tracker that lives outside your repo.

> [!WARNING]
> Binnacle is very early in development, proceed with caution!

## Quick Start

```bash
# Install (from source for now)
cargo install --path .

# Initialize in your project
cd your-project
bn system init      # Interactive setup with prompts

# Create a task
bn task create "Implement user authentication"

# See what's ready to work on
bn ready

# Update task status as you work
bn task update bn-xxxx --status in_progress
bn task close bn-xxxx --reason "Implemented JWT auth"
```

## Agent Automation (agent.sh)

Binnacle includes `agent.sh`, a launcher script for running AI agents with pre-configured tool permissions. This is the recommended way to run autonomous agents that work on binnacle-tracked projects.

```bash
# Auto-pick and work on the highest priority task
./agent.sh auto

# Work on a specific task description
./agent.sh do "fix the login validation bug"

# Loop mode - restart agent when it exits
./agent.sh --loop auto

# Other agent types
./agent.sh prd      # Generate PRDs from ideas
./agent.sh buddy    # Quick task/bug insertion helper
./agent.sh free     # General purpose with binnacle access
```

The agents automatically:
- Read `PRD.md` and orient themselves with `bn orient`
- Claim tasks, work on them, and mark them complete
- Commit their changes and terminate gracefully with `bn goodbye`

## Features

- **JSON-first output** - Machine-readable by default, `-H` for human-readable
- **No repo pollution** - Data stored externally in `~/.local/share/binnacle/<repo-hash>/`
- **Task dependencies** - Block tasks on other tasks, query what's ready
- **Test tracking** - Link tests to tasks, auto-reopen tasks on regression
- **Action logging** - Comprehensive audit log of all commands with timestamps and metadata
- **MCP server** - Expose all operations as MCP tools for AI agents
- **Work queue** - Prioritize tasks for agents with a global work queue

## Queue (Agent Prioritization)

The queue feature allows operators to signal which tasks agents should work on first:

```bash
# Create a queue (one per repo)
bn queue create "Sprint 1"

# Add tasks to the queue
bn link add bn-xxxx bnq-yyyy --type queued

# View queue and its tasks
bn queue show

# bn ready now shows queued tasks first
bn ready -H
# Ready tasks (5):
#   [QUEUED]
#     [P1] bn-xxxx: Fix auth bug
#   [OTHER]
#     [P2] bn-yyyy: Refactor utils

# Tasks are automatically removed from queue when closed
bn task close bn-xxxx --reason "Fixed"
```

## Commands

```
bn                    Status summary
bn orient             Onboarding for AI agents
bn system init        Initialize database (interactive, recommended)
bn task create/list/show/update/close/delete
bn link add/rm/list   Manage relationships (dependencies, etc.)
bn ready              Tasks with no blockers
bn blocked            Tasks waiting on dependencies
bn queue create/show/delete  Agent work prioritization
bn test create/run    Test node management
bn commit link/list   Associate commits with tasks
bn mcp serve          Start MCP server
bn gui                Start web GUI (requires gui feature)
```

Use `bn --help` or `bn <command> --help` for full details.

## Web GUI

Binnacle includes an optional web-based GUI for visualizing tasks, dependencies, tests, and activity logs. The GUI provides a real-time, interactive view of your project state with a modern dark blue interface.

**Features:**

- **Interactive task graph** - Spring-physics based visualization of task dependencies
- **Ready tasks view** - Quick access to tasks ready to work on
- **Test dashboard** - Monitor test status and history
- **Activity log** - Track all changes and actions
- **Live updates** - WebSocket-based real-time synchronization

**Building with GUI:**

```bash
# Build with GUI feature
cargo build --release --features gui

# Or use just (includes GUI by default)
just install

# Or install with cargo
cargo install --path . --features gui
```

**Running the GUI:**

```bash
# Start on default port (3030)
bn gui

# Start on custom port
bn gui --port 8080

# Then open http://localhost:3030 in your browser
```

The GUI watches for changes to binnacle data and automatically updates all connected clients.

## Configuration

Binnacle supports configuration via `bn config set/get/list`:

```bash
# Action logging (default: enabled)
bn config set action_log_enabled true
bn config set action_log_path ~/.local/share/binnacle/action.log
bn config set action_log_sanitize true

# View configuration
bn config get action_log_enabled
bn config list
```

### Action Logging

All binnacle commands are automatically logged to a JSONL file with:

- Timestamp
- Command name and arguments
- Success/failure status
- Execution duration
- Current user

**Config keys:**

- `action_log_enabled` - Enable/disable logging (default: `true`)
- `action_log_path` - Log file path (default: `~/.local/share/binnacle/action.log`)
- `action_log_sanitize` - Sanitize sensitive data and paths (default: `true`)

Sanitization automatically:

- Converts file paths to basenames
- Redacts passwords, tokens, and secrets
- Truncates long strings
- Summarizes large arrays

### Require Commit for Closure

Enable enforcement that tasks must have at least one linked commit before being closed:

```bash
# Enable the requirement
bn config set require_commit_for_close true

# Now tasks require a linked commit to close
bn task close bn-xxxx  # Error: no commits linked
bn commit link abc1234 bn-xxxx
bn task close bn-xxxx  # Success

# Bypass with --force when needed (e.g., docs-only tasks)
bn task close bn-xxxx --force --reason "Documentation only"
```

**Config key:**

- `require_commit_for_close` - Require linked commits before closing tasks as done (default: `false`)

When enabled:
- `bn task close` and `bn task update --status done` check for linked commits
- Use `--force` to bypass the check for legitimate cases (config changes, documentation)
- `cancelled` status is exempt (no commit required)

### Environment Variables

- `BN_DATA_DIR` - Override the base directory for binnacle data storage. By default, data is stored in `~/.local/share/binnacle/`. When set, binnacle stores data in `$BN_DATA_DIR/<repo-hash>/` instead. Useful for testing or isolating data between environments.

## Releases

Binnacle uses [semantic versioning](https://semver.org/) with alpha suffixes during early development:

- **Format**: `0.x.y-alpha.z` (e.g., `0.0.1-alpha.2`)
- **Alpha releases** indicate the API is unstable and breaking changes may occur

### Creating a Release

1. Update the version in `Cargo.toml`
2. Commit the version change
3. Create a GitHub Release with a tag matching the version (e.g., `v0.0.1-alpha.3`)

The CI/CD pipeline will:
- Verify the git tag matches `Cargo.toml` version
- Run all tests, formatting, and linting checks
- Build and upload binaries to the GitHub Release
- Publish to [crates.io](https://crates.io/crates/binnacle)

> [!NOTE]
> The tag **must** match the `Cargo.toml` version exactly (with a `v` prefix). 
> For example, version `0.0.1-alpha.3` requires tag `v0.0.1-alpha.3`.

### Installing Releases

```bash
# From crates.io (when published)
cargo install binnacle --features gui

# From GitHub Release (download binary)
# See https://github.com/hbeberman/binnacle/releases
```

## Status

Core functionality is complete (Phases 0-7). The project tracks its own development with binnacle.

What works:

- Task CRUD with priorities, tags, assignees
- Dependency graph with cycle detection
- Test nodes with regression detection
- Commit tracking
- Action logging with sanitization
- Work queue for agent task prioritization
- MCP server with 38+ tools
- Web GUI with live updates (behind gui feature flag)
- CI/CD via GitHub Actions

In progress:

- Alternative storage backends (orphan branch done, git notes planned)
- Sync for shared mode

## Building

```bash
# Recommended: Install with GUI feature using just
just install

# Or build manually
cargo build --release --features gui

# Or without GUI
cargo build --release
```

The binary is `bn` (short for binnacle). The `just install` command builds with the GUI feature enabled and installs to `~/.local/bin`.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, code style guidelines, and how to submit pull requests.

## License

MIT
