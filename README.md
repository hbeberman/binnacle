<p align="center">
  <img src="binnaclebanner.png" alt="Binnacle Banner" width="100%">
</p>

# binnacle

Task tracker for AI agents. Stores data outside your repo so it doesn't pollute your codebase.

> [!WARNING]
> Early alpha. Things may break.

## Install

```bash
cargo install --path .
```

## Usage

```bash
bn system init                  # set up in your project
bn task create "Do the thing"   # create a task
bn ready                        # see what's actionable
bn task close bn-xxxx           # mark done
```

For AI agents:
```bash
bn orient                       # get up to speed on project state
bn goodbye "summary"            # graceful exit
```

## Running Agents

```bash
./agent.sh auto                 # pick highest priority task and work on it
./agent.sh --loop auto          # keep going until queue is empty
./agent.sh buddy                # helper for adding tasks interactively
```

### Containerized Agents (Quick Start)

Run AI agents in isolated containers with full access to the binnacle task graph:

```bash
# 1. Install prerequisites (Fedora/RHEL)
sudo dnf install containerd buildah
sudo systemctl enable --now containerd

# 2. Create a worktree for the agent to work in
git worktree add ../agent-work -b agent-feature

# 3. Build the container image
bn container build

# 4. Run the container
bn container run ../agent-work
```

The container mounts your worktree and binnacle data, runs an AI agent (copilot or claude), and auto-merges completed work back to `main`.

See [container/README.md](container/README.md) for full documentation including resource limits, environment variables, and troubleshooting.

## What It Tracks

- **Tasks** (`bn-xxxx`) with priorities, dependencies, tags
- **Bugs** (`bn-xxxx`) with severity levels
- **Ideas** (`bn-xxxx`) that can be promoted to tasks
- **Milestones** (`bn-xxxx`) with progress tracking
- **Tests** (`bnt-xxxx`) linked to tasks, auto-reopen on regression
- **Docs** (`bn-xxxx`) for attached documentation
- **Queue** (`bnq-xxxx`) for agent prioritization

## Quick Reference

```bash
bn                              # status summary
bn ready                        # actionable tasks
bn blocked                      # what's waiting on dependencies
bn show <id>                    # details on any entity

bn task create/list/update/close
bn bug create/list/update/close
bn link add <src> <tgt> --type depends_on
bn queue show                   # see prioritized work

bn gui                          # web interface (needs --features gui)
bn mcp serve                    # MCP server for agents
```

Run `bn --help` for everything else.

## GUI

```bash
cargo install --path . --features gui
bn gui
# open http://localhost:3030
```

Interactive graph of tasks and dependencies with live updates.

## Viewer (WASM)

Export your task graph and view it in any browser—no server needed:

```bash
just build-viewer                # build the standalone viewer
bn system store export data.bng  # export your project data
# open target/viewer/viewer.html, drop in data.bng
```

See [docs/embedding-viewer.md](docs/embedding-viewer.md) for embedding in web pages.

## Building

```bash
just install                    # recommended, includes GUI
cargo build --release           # without GUI
```

## License

MIT
