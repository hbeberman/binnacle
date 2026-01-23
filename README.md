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
bn system init

# Create a task
bn task create "Implement user authentication"

# See what's ready to work on
bn ready

# Update task status as you work
bn task update bn-xxxx --status in_progress
bn task close bn-xxxx --reason "Implemented JWT auth"
```

## Features

- **JSON-first output** - Machine-readable by default, `-H` for human-readable
- **No repo pollution** - Data stored externally in `~/.local/share/binnacle/<repo-hash>/`
- **Task dependencies** - Block tasks on other tasks, query what's ready
- **Test tracking** - Link tests to tasks, auto-reopen tasks on regression
- **Action logging** - Comprehensive audit log of all commands with timestamps and metadata
- **MCP server** - Expose all operations as MCP tools for AI agents

## Commands

```
bn                    Status summary
bn orient             Onboarding for AI agents
bn task create/list/show/update/close/delete
bn link add/rm/list   Manage relationships (dependencies, etc.)
bn ready              Tasks with no blockers
bn blocked            Tasks waiting on dependencies
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

### Environment Variables

- `BN_DATA_DIR` - Override the base directory for binnacle data storage. By default, data is stored in `~/.local/share/binnacle/`. When set, binnacle stores data in `$BN_DATA_DIR/<repo-hash>/` instead. Useful for testing or isolating data between environments.

## Status

Core functionality is complete (Phases 0-7). The project tracks its own development with binnacle.

What works:

- Task CRUD with priorities, tags, assignees
- Dependency graph with cycle detection
- Test nodes with regression detection
- Commit tracking
- Action logging with sanitization
- MCP server with 30 tools
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
