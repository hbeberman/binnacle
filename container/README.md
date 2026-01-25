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
| `COPILOT_GITHUB_TOKEN` | - | Passed through if set on host |

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

## Troubleshooting

### containerd not found

Install containerd and ensure the service is running:

```bash
sudo systemctl status containerd
sudo systemctl start containerd
```

### Image not found

Build the image first:

```bash
bn container build
```

Verify it's imported to containerd:

```bash
sudo ctr -n binnacle images list
```

### Authentication errors

Ensure your GitHub token is set:

```bash
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
- Currently requires root/sudo for containerd access

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
