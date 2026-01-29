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

## Copilot CLI Management

Binnacle can manage GitHub Copilot CLI binaries to provide version stability for agent workflows:

```bash
bn system copilot install --upstream   # Install binnacle-preferred version
bn system copilot install v0.0.396     # Install specific version
bn system copilot path                 # Show active binary location
bn system copilot version              # List installed versions
```

**Version Pinning:**
- Binnacle ships with a preferred Copilot version (embedded at build time)
- Agents use `--no-auto-update` flag to prevent runtime updates
- Containers pre-install the pinned version during image build
- Host agents resolve binaries via `bn system copilot path`

This prevents unexpected behavior from automatic Copilot updates mid-workflow.

## Running Agents

```bash
./agent.sh auto                 # pick highest priority task and work on it
./agent.sh --loop auto          # keep going until queue is empty
./agent.sh buddy                # helper for adding tasks interactively
./agent.sh ask                  # interactive Q&A for exploring the codebase (read-only)
```

**Note:** `agent.sh` automatically resolves the Copilot binary via `bn system copilot path` and runs it with `--no-auto-update` to prevent mid-workflow updates. Install a pinned version with `bn system copilot install --upstream` before running agents.

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

The container mounts your worktree and binnacle data, runs an AI agent (copilot or claude with pinned versions), and auto-merges completed work back to `main`.

**Copilot Version Management:**
- Containers use a pinned GitHub Copilot CLI version pre-installed during image build
- The pinned version is managed by `bn system copilot` commands
- Copilot runs with `--no-auto-update` to prevent runtime updates
- This ensures consistent agent behavior across container runs

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

### Building with GUI Support

The web interface requires the `gui` feature flag:

```bash
# Install to ~/.local/bin with GUI enabled (recommended)
just install

# Or build manually
cargo build --release --features gui
cargo install --path . --features gui
```

**How bundling works:**
- During `cargo build --features gui`, the build script (`build.rs`) automatically runs `scripts/bundle-web.sh`
- Web assets from `web/` are minified and compressed into `target/web-bundle.tar.zst`
- The bundle is embedded into the binary at compile time
- Bundle is cached using a hash of `web/` contents - only rebuilds when files change
- Development builds can skip bundling with `--dev` flag to serve directly from filesystem

### Launching the GUI

**Option 1: Using `bn gui` (direct command)**

```bash
bn gui                           # Start on default port (3030)
bn gui -p 8080                   # Start on custom port
bn gui --readonly                # Start in read-only mode
bn gui --tunnel                  # Create public URL via devtunnel (read-only)
bn gui --archive data.bng        # Load a .bng archive file (read-only snapshot)
```

The server will start and print the URL to access the interface. Open it in your browser:
```
http://localhost:3030
```

**Note**: The `--archive` flag must be specified **before** any subcommand (e.g., `bn gui --archive file.bng serve`). It appears in `bn gui --help` but not in `bn gui serve --help` because it's a top-level option.

**Option 2: Using `just gui` (with hot reload)**

For development, use the justfile recipe which provides hot restart:

```bash
just gui            # Build, install, and launch with hot restart
just gui nobuild    # Launch without rebuilding (uses existing binary)
```

The `just gui` recipe offers faster iteration during development:
- Starts immediately with the existing binary
- Rebuilds in the background
- Automatically restarts with the new build

**Option 3: Development mode (for frontend work)**

When working on JavaScript/CSS changes, use development mode for instant updates:

```bash
just dev-gui        # Serves from web/ directory, no bundle needed
```

Development mode (`--dev` flag):
- Serves assets directly from `web/` directory
- Edit files and refresh browser to see changes instantly
- No bundling/rebuilding required for frontend changes
- Uses `cargo run --features gui -- gui --dev` under the hood

**Environment Variables:**
- `BN_GUI_PORT`: Override default port (default: 3030)
- `BN_GUI_HOST`: Override bind address (default: 0.0.0.0)
- `BN_GUI_READONLY`: Start in read-only mode
- `BN_GUI_TUNNEL`: Enable tunnel mode (see below)

### Public URL via Dev Tunnels

Share your GUI publicly without port forwarding using Microsoft Dev Tunnels:

```bash
bn gui --tunnel              # Start with a public URL
```

This spawns a `devtunnel` process that creates a temporary public URL (e.g., `https://abc123-3030.use.devtunnels.ms`) proxying to your local GUI.

**Requirements:**
1. Install devtunnel: `just install-devtunnel`
2. Authenticate (one-time): `devtunnel user login`

The authentication step is required before first use and supports GitHub, Microsoft, or Azure AD accounts.

**Security:**
- Tunnel mode **automatically enables read-only mode** to prevent remote modifications
- The URL is randomly generated and not easily guessable
- Tunnel terminates when the GUI server stops

**Use cases:**
- Share project status with remote collaborators
- Demo your task graph without exposing your network
- Quick reviews without VPN setup

### GUI Management Commands

```bash
bn gui status       # Check if GUI is running
bn gui stop         # Gracefully stop the GUI (SIGTERM)
bn gui kill         # Force kill the GUI immediately
```

### Static Viewer Export (GitHub Pages Hosting)

You can create a static HTML bundle of your project's current state for hosting on GitHub Pages or any static site host:

```bash
# Export to default location (target/static-viewer/)
bn gui export

# Export to custom location
bn gui export -o docs/viewer

# Export with specific archive
bn gui export --archive path/to/snapshot.bng
```

The exported bundle includes:
- A standalone web viewer (all HTML, CSS, JS assets)
- A `.bng` archive snapshot of your project
- Auto-load script that opens the archive on page load

**Hosting on GitHub Pages:**

1. Export to a directory that will be committed:
   ```bash
   bn gui export -o docs/viewer
   ```

2. Commit the exported files:
   ```bash
   git add docs/viewer
   git commit -m "Add static project viewer"
   git push
   ```

3. Enable GitHub Pages in your repository settings:
   - Go to Settings → Pages
   - Set source to your branch (e.g., `main`)
   - Set folder to `/docs` (or root if you exported to root)

4. Visit your viewer at:
   ```
   https://<username>.github.io/<repo>/docs/viewer/
   ```

**Remote Archive URLs:**

The viewer can also load archives from remote URLs. Share a viewer URL with an archive parameter:
```
https://<username>.github.io/<repo>/viewer/?url=https://example.com/snapshot.bng
```

This allows you to:
- Host multiple snapshots and switch between them
- Share project state with collaborators
- Create time-based archives for project history

### Testing the GUI

1. **Launch the GUI:**
   ```bash
   just gui
   ```

2. **Open in browser:**
   ```
   http://localhost:3030
   ```

3. **Verify features:**
   - Graph visualization displays tasks and their relationships
   - Nodes are color-coded by status (pending, in_progress, done, blocked)
   - Click nodes to see details in the info panel
   - Use the search bar to filter nodes
   - Use filters to show/hide node and edge types
   - Live updates reflect changes made via CLI

4. **Test live updates:**
   ```bash
   # In another terminal, make changes
   bn task create "Test task"
   bn task update bn-xxxx --status in_progress
   ```
   
   The GUI should update automatically without refresh.

5. **Test connection recovery:**
   - Stop the server: `bn gui stop`
   - Restart it: `bn gui`
   - The viewer should automatically reconnect

### Troubleshooting

**Port already in use:**
```bash
# Use a different port
BN_GUI_PORT=8080 bn gui
```

**Can't connect from another machine:**
- Ensure the server is bound to 0.0.0.0 (default)
- Check firewall settings
- Use `bn gui --host 0.0.0.0` explicitly if needed

**GUI won't stop:**
```bash
# Force kill
bn gui kill
```

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
