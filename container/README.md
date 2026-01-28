# Binnacle Container Worker

Run AI agents in isolated containers with full access to the binnacle task graph.

## Quick Start

```bash
# 1. Create a git worktree for the agent to work in
git worktree add ../agent-worktree -b agent-feature

# 2. Build the worker image
bn container build

# 3. Run the container (interactive mode)
bn container run ../agent-worktree
```

## Prerequisites

Binnacle containers use **containerd** (runtime) and **buildah** (image building):

```bash
# Fedora/RHEL
sudo dnf install containerd buildah
sudo systemctl enable --now containerd

# Debian/Ubuntu
sudo apt install containerd buildah
sudo systemctl enable --now containerd
```

> **Tip:** For rootless operation without `sudo`, see the [Rootless Setup](#rootless-setup) section below.

## Overview

The container worker provides:

- **Isolation** - Agent runs in a sandboxed environment
- **Reproducibility** - Consistent environment across machines
- **Resource Control** - CPU and memory limits (--cpus, --memory)
- **Auto-merge** - Completed work merged back to target branch

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                          HOST SYSTEM                            │
│                                                                 │
│  ┌─────────────────┐    ┌─────────────────┐                    │
│  │ Main Repo       │    │ Binnacle Data   │                    │
│  │ ~/repos/project │    │ ~/.local/share/ │                    │
│  │                 │    │   binnacle/     │                    │
│  └────────┬────────┘    └────────┬────────┘                    │
│           │                      │                              │
│           │ git worktree         │                              │
│           ▼                      │                              │
│  ┌─────────────────┐             │                              │
│  │ Agent Worktree  │             │                              │
│  └────────┬────────┘             │                              │
│           │                      │                              │
│  ┌────────┴──────────────────────┴──────────────────────────┐  │
│  │                    bn container run                       │  │
│  │  - Uses containerd (ctr) for runtime                     │  │
│  │  - Uses binnacle namespace for isolation                 │  │
│  │  - Mounts worktree + binnacle data                       │  │
│  └──────────────────────────────────────────────────────────┘  │
│                              │                                  │
└──────────────────────────────┼──────────────────────────────────┘
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                    CONTAINERD (ctr)                             │
│                    namespace: binnacle                          │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              binnacle-worker container                   │   │
│  │                                                          │   │
│  │  /workspace (repo worktree)    [bind mount, r/w]        │   │
│  │  /binnacle  (graph data)       [bind mount, r/w]        │   │
│  │                                                          │   │
│  │  entrypoint.sh:                                         │   │
│  │    1. bn orient --type $BN_AGENT_TYPE                   │   │
│  │    2. Run AI agent (copilot/claude)                     │   │
│  │    3. git merge to $BN_MERGE_TARGET                     │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

## Commands

### Build the Worker Image

The `bn container build` command automatically packs the currently running `bn` binary into the container image. This means you can build directly from an installed `bn`:

```bash
# Build using the currently running bn binary
bn container build

# Or with a custom tag
bn container build --tag v1.0
```

For development builds using the justfile:

```bash
just container                   # Build binary + container with tag 'binnacle-worker:latest'
just container myimage:v1.0      # Build with custom tag
```

Or manually:

```bash
# 1. Build the release binary
cargo build --release

# 2. Copy it to the container directory
cp target/release/bn container/bn

# 3. Build the container image
podman build -t binnacle-worker:latest -f container/Containerfile .

# 4. Clean up
rm container/bn
```

**Copilot CLI Version Pinning:**

The container image includes a pre-installed, pinned version of the GitHub Copilot CLI to ensure consistent agent behavior:

- During `bn container build`, the image runs `bn system copilot install --upstream`
- This installs the binnacle-preferred Copilot version (embedded in the `bn` binary at build time)
- The pinned binary is stored at `BN_DATA_DIR/utils/copilot/<version>/copilot`
- At runtime, `entrypoint.sh` finds and uses this pinned binary with `--no-auto-update` flag
- This prevents unexpected behavior from automatic Copilot updates during container execution

To verify the installed version:
```bash
# Check what version will be installed
bn system copilot version

# Or inspect a running container
ctr -n binnacle task exec -t <container-id> sh
find /usr/local/share/binnacle/utils/copilot -name copilot
```

### Run a Container

```bash
# Basic usage (interactive TTY)
bn container run ../agent-worktree

# With agent type
bn container run ../agent-worktree --type planner

# With resource limits
bn container run ../agent-worktree --cpus 2 --memory 4g

# Run in background
bn container run ../agent-worktree --detach

# Disable auto-merge
bn container run ../agent-worktree --no-merge

# Custom container name
bn container run ../agent-worktree --name my-agent
```

### List Containers

```bash
bn container list          # Show running containers
bn container list --all    # Include stopped containers
```

### Stop Containers

```bash
bn container stop <name>   # Stop specific container
bn container stop --all    # Stop all binnacle containers
```

## Environment Variables

Environment variables are automatically passed to the container:

| Variable | Default | Description |
|----------|---------|-------------|
| `BN_AGENT_TYPE` | worker | Agent type (worker, planner, buddy) |
| `BN_CONTAINER_MODE` | true | Indicates running in container |
| `BN_MERGE_TARGET` | main | Branch to merge into on exit |
| `BN_NO_MERGE` | - | Set when --no-merge is used |
| `GH_TOKEN` | - | GitHub token (passed through if set on host) |
| `COPILOT_GITHUB_TOKEN` | - | Copilot CLI token (passed through if set on host) |

## Files

| File | Description |
|------|-------------|
| `Containerfile` | Fedora 43 base with binnacle, npm, @github/copilot, nss_wrapper, and dev tools |
| `entrypoint.sh` | Orchestrates agent setup (with nss_wrapper), execution, and merge |

## Workflow

1. **Setup**: Create a git worktree for isolated work
2. **Build**: Run `bn container build` to create the worker image
3. **Launch**: Run `bn container run <worktree>` to start the container
4. **Orient**: Container runs `bn orient` to load task state
5. **Execute**: AI agent picks and completes tasks
6. **Merge**: On success, work is merged to target branch

## Agent Supervisor Daemon

For continuous operation, use the `bn serve` daemon to automatically manage agent containers:

```bash
sudo bn serve
```

The supervisor watches your scaling configuration and spawns/stops containers to match desired agent counts.

### Why sudo?

The `bn serve` command needs access to the containerd socket at `/run/containerd/containerd.sock`, which requires root privileges. However, binnacle is designed to preserve your user's file ownership:

1. **SUDO_USER Detection**: When you run `sudo bn serve`, binnacle reads the `SUDO_USER`, `SUDO_UID`, and `SUDO_GID` environment variables
2. **Socket Access**: Opens the containerd socket while running as root
3. **Privilege Drop**: Immediately drops privileges back to your user account
4. **HOME Setting**: Sets `HOME` to your user's home directory
5. **File Ownership**: All files created by the daemon (logs, configs, container mounts) are owned by you, not root

This means you invoke the command with `sudo`, but the process runs as your user after initialization.

### Example Output

```bash
$ sudo bn serve
Agent supervisor starting (interval: 10s)
Dropping privileges to user alice (UID: 1000, GID: 1000)
Reconciling agents...
  Current: 0 workers
  Desired: 2 workers
  Action: spawn 2 worker containers
Container worker-1 started
Container worker-2 started
```

### Running Without sudo

For rootless operation, see the [Rootless Setup](#rootless-setup) section below. With rootless containerd configured, you can run:

```bash
bn serve  # no sudo needed
```

## Rootless Setup

By default, binnacle uses system containerd which requires `sudo`. For a better experience without `sudo`, set up rootless containerd:

### Prerequisites

Rootless containerd requires:
- Linux kernel 5.11+ (for unprivileged user namespaces)
- containerd 1.5+ with rootless support

### Installation (Fedora/RHEL)

```bash
# Install containerd with rootless support
sudo dnf install containerd rootlesskit slirp4netns

# Enable user namespaces (if not already enabled)
sudo sysctl -w kernel.unprivileged_userns_clone=1
echo 'kernel.unprivileged_userns_clone=1' | sudo tee /etc/sysctl.d/userns.conf

# Set up subuid/subgid ranges for your user
sudo usermod --add-subuids 100000-165535 --add-subgids 100000-165535 $USER
```

### Installation (Debian/Ubuntu)

```bash
# Install containerd with rootless support
sudo apt install containerd rootlesskit slirp4netns uidmap

# Set up subuid/subgid ranges for your user
sudo usermod --add-subuids 100000-165535 --add-subgids 100000-165535 $USER
```

### Start Rootless Containerd

```bash
# Create the rootless containerd directory
mkdir -p ~/.local/share/containerd

# Start rootless containerd (runs in foreground, use & or a separate terminal)
containerd-rootless-setuptool.sh install

# Or manually start it:
# XDG_RUNTIME_DIR must be set (usually /run/user/$(id -u))
containerd --config ~/.config/containerd/config.toml --root ~/.local/share/containerd --state $XDG_RUNTIME_DIR/containerd &

# Verify the socket exists
ls $XDG_RUNTIME_DIR/containerd/containerd.sock
```

### Enable Rootless Containerd at Login

To start rootless containerd automatically:

```bash
# Using systemd user service
mkdir -p ~/.config/systemd/user
cat > ~/.config/systemd/user/containerd.service << 'EOF'
[Unit]
Description=Rootless containerd

[Service]
ExecStart=/usr/bin/containerd --config %h/.config/containerd/config.toml --root %h/.local/share/containerd --state %t/containerd
Restart=always

[Install]
WantedBy=default.target
EOF

# Enable and start
systemctl --user daemon-reload
systemctl --user enable --now containerd

# Enable lingering so it starts on boot
loginctl enable-linger $USER
```

### Verify Rootless Setup

```bash
# Check if binnacle detects rootless containerd
bn container list
# Should NOT show "⚠️ Using system containerd (requires sudo)"

# Manually verify the socket
ctr -a $XDG_RUNTIME_DIR/containerd/containerd.sock version
```

### How Binnacle Detects Rootless Mode

Binnacle automatically detects rootless containerd by checking for:
1. `$XDG_RUNTIME_DIR/containerd/containerd.sock`

If found, it uses `ctr -a <socket_path>` without `sudo`. Otherwise, it falls back to system containerd with `sudo ctr`.

## Troubleshooting

### containerd not found

Install containerd and ensure the service is running:

**System containerd:**
```bash
sudo systemctl status containerd
sudo systemctl start containerd
```

**Rootless containerd:**
```bash
systemctl --user status containerd
systemctl --user start containerd
```

### Image not found

Build the image first:

```bash
bn container build
```

Verify it's imported to containerd:

**System containerd:**
```bash
sudo ctr -n binnacle images list
```

**Rootless containerd:**
```bash
ctr -a $XDG_RUNTIME_DIR/containerd/containerd.sock -n binnacle images list
```

### Authentication errors

Ensure your GitHub token is set:

```bash
# For GitHub CLI and general API access
export GH_TOKEN="your-token"
# Or for Copilot CLI specifically
export COPILOT_GITHUB_TOKEN="your-token"
bn container run ../agent-worktree
```

### Merge fails

If the target branch has diverged:

```bash
# The container will exit with error, leaving work on the worktree branch
# Manually merge or rebase:
git checkout main
git merge --no-ff agent-feature
```

## Security Notes

- Tokens are passed via environment, not baked into the image
- Container only accesses the specific worktree, not entire filesystem
- Uses dedicated `binnacle` namespace to isolate from other workloads
- Resource limits prevent runaway processes
- Supports both rootless (no sudo) and system containerd (requires sudo)
- **`--no-verify` is blocked** - agents cannot bypass commit hooks

### Git Hook Enforcement

The container includes a git wrapper (`/usr/local/bin/git`) that intercepts and blocks `git commit --no-verify` and `git push --no-verify`. This ensures:

- Pre-commit hooks ALWAYS run (formatting, linting, security audits)
- Agents must fix issues rather than bypass checks
- Code quality standards are enforced automatically

If hooks fail, the wrapper provides guidance on how to fix common issues (e.g., `cargo fmt`, `cargo clippy --fix`).

### Container Mode

The container runs as your host user (via `--user UID:GID`):

- File ownership preserved correctly on mounted workspace
- Uses `nss_wrapper` to provide user identity for Node.js, git, etc.
- **No sudo access** - agent cannot install packages at runtime

If you need packages not in the base image, add them to the Containerfile and rebuild.

### Network Access

Uses `--net-host` for network access (required for AI agent API calls). The container can access host network interfaces including localhost services.

### User Identity

The container uses `nss_wrapper` with `LD_PRELOAD` to provide user identity without modifying system files. This satisfies tools like `git` and Node.js `os.userInfo()` that call `getpwuid()`.

## Related

- [PRD: Containerd Runtime](../prds/PRD_CONTAINERD_RUNTIME.md) - Full specification
- [Binnacle README](../README.md) - Main project documentation
