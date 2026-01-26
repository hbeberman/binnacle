<p align="center">
  <img src="binnaclebanner.png" alt="Binnacle Banner" width="100%">
</p>

# binnacle

Task tracker for AI agents. Stores data outside your repo so it doesn't pollute your codebase.

> [!WARNING]
> Early alpha. Things *will* break.

## Build Prerequisites

### Rust Toolchain

Install Rust via [rustup](https://rustup.rs/):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### System Dependencies

**Fedora/RHEL/Rocky Linux:**

```bash
sudo dnf install gcc make pkg-config openssl-devel
```

**Ubuntu/Debian:**

```bash
sudo apt-get update
sudo apt-get install build-essential pkg-config libssl-dev
```

**For WASM viewer builds** (optional):

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

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

## Agent Supervisor (`bn serve`)

Run a daemon that continuously manages containerized AI agents based on your scaling configuration:

```bash
sudo bn serve
```

**Why sudo?** The `bn serve` command needs to access the containerd socket at `/run/containerd/containerd.sock`, which requires root privileges. However, binnacle automatically detects when running under sudo and:

1. Opens the containerd socket while elevated
2. Drops privileges back to your user (via `SUDO_USER` detection)
3. Sets `HOME` to your user's home directory
4. Creates all files with your user's ownership

This means you run as sudo but **all files remain owned by you**, not root. The process only retains elevated privileges for the containerd socket connection.

If you see a warning like "Running as root without SUDO_USER", it means you're running directly as root (not via sudo), and files will be owned by root. To preserve user ownership, always run with `sudo bn serve`.

For rootless operation without sudo, see the [Rootless Setup](#rootless-setup) section in [container/README.md](container/README.md).

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

### Connection Modes

The viewer supports two modes via URL parameters:

- **Archive mode**: `viewer.html?archive=./data.bng` - Load exported `.bng` file (read-only)
- **Live mode**: `viewer.html?ws=localhost:3030` - Connect to running `bn gui` server

Add `#bn-xxxx` to focus on a specific entity: `viewer.html?archive=./data.bng#bn-a1b2`

### Local Hosting

Serve the viewer locally with a pre-loaded archive:

```bash
just serve-wasm                       # serve on port 8080
just serve-wasm 3000                  # serve on custom port
just serve-wasm 8080 path/to/data.bng # serve with pre-loaded archive
```

Then open `http://localhost:8080` in your browser. If you provided an archive path, it will be auto-loaded via URL parameter.

See [docs/embedding-viewer.md](docs/embedding-viewer.md) for embedding in web pages.

## Building

```bash
just install                    # recommended, includes GUI
cargo build --release           # without GUI
```

## License

MIT
