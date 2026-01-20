# binnacle

A CLI tool for AI agents and humans to track project state. Think of it as a lightweight, JSON-first task tracker that lives outside your repo.

## Quick Start

```bash
# Install (from source for now)
cargo install --path .

# Initialize in your project
cd your-project
bn init

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
- **MCP server** - Expose all operations as MCP tools for AI agents

## Commands

```
bn                    Status summary
bn orient             Onboarding for AI agents
bn task create/list/show/update/close/delete
bn dep add/rm/show    Manage dependencies
bn ready              Tasks with no blockers
bn blocked            Tasks waiting on dependencies
bn test create/run    Test node management
bn commit link/list   Associate commits with tasks
bn mcp serve          Start MCP server
```

Use `bn --help` or `bn <command> --help` for full details.

## Status

Core functionality is complete (Phases 0-7). The project tracks its own development with binnacle.

What works:
- Task CRUD with priorities, tags, assignees
- Dependency graph with cycle detection
- Test nodes with regression detection
- Commit tracking
- MCP server with 30 tools
- CI/CD via GitHub Actions

In progress:
- Alternative storage backends (orphan branch done, git notes planned)
- Sync for shared mode

## Building

```bash
cargo build --release
```

The binary is `bn` (short for binnacle).

## License

MIT
