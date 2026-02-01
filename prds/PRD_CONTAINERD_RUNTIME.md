# PRD: Direct containerd/ctr Runtime

**Status:** Draft  
**Created:** 2026-01-24  
**Related Ideas:** bn-e01c, bn-8918, bn-bdf0, bn-ad78 (separate)

## Overview

Replace the Docker Compose-based container workflow with direct containerd integration using `ctr` for container runtime and `buildah` for image building. This provides a more fundamental, system-level dependency without requiring the Docker daemon.

## Motivation

1. **Lighter dependency** - containerd is a lower-level runtime that many systems already have (it's what Docker uses underneath). No need for the full Docker daemon.

2. **System service focus** - Better integration with systemd and programmatic control. containerd is designed for embedding and automation.

3. **OCI-native** - buildah produces OCI-compliant images without requiring a daemon, making builds more portable and scriptable.

4. **Simpler mental model** - `bn container run` is clearer than "set env vars, cd to container/, run docker compose up".

## Goals

- Replace Docker Compose with direct `ctr` commands for container runtime
- Use `buildah` for OCI-compliant image building
- Provide simple CLI: `bn container build`, `bn container run`, `bn container stop`, `bn container list`
- Run containers in interactive "headed mode" (TTY attached, live output)
- Use a `binnacle` namespace in containerd for isolation
- Fail gracefully with helpful install instructions if dependencies missing

## Non-Goals

- Kubernetes integration (future PRD)
- Auto-detect project and generate Containerfile (bn-ad78, separate)
- Docker fallback (removing Docker Compose entirely)
- Rootless containerd support (v1 assumes root/sudo)
- Registry publishing (ghcr.io - future consideration)

## Design

### Architecture

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
│  │  - Calls ctr (containerd CLI)                            │  │
│  │  - Uses binnacle namespace                               │  │
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
│  │              binnacle-self container                   │   │
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

### Dependencies

| Tool | Purpose | Install |
|------|---------|---------|
| `containerd` | Container runtime | `dnf install containerd` / `apt install containerd` |
| `ctr` | containerd CLI (bundled with containerd) | Included with containerd |
| `buildah` | OCI image building | `dnf install buildah` / `apt install buildah` |

### CLI Commands

#### `bn container build`

Build the binnacle worker image using buildah.

```bash
bn container build [--tag TAG] [--no-cache]

# Examples:
bn container build                    # Build localhost/binnacle-self:latest
bn container build --tag v0.5.0       # Build with specific tag
bn container build --no-cache         # Force rebuild without cache
```

**Implementation:**
```bash
# Equivalent to:
buildah bud -t localhost/binnacle-self:latest -f container/Containerfile .
buildah push localhost/binnacle-self:latest oci-archive:/tmp/binnacle-self.tar
sudo ctr -n binnacle images import /tmp/binnacle-self.tar
```

#### `bn container run`

Run a worker container in headed (interactive) mode.

```bash
bn container run [WORKTREE_PATH] [OPTIONS]

Options:
  --type TYPE         Agent type: worker, planner, buddy (default: worker)
  --name NAME         Container name (default: auto-generated)
  --merge-target BR   Branch to merge into on exit (default: main)
  --no-merge          Disable auto-merge on exit
  --detach            Run in background (non-headed mode)

# Examples:
bn container run ../agent-worktree                    # Interactive worker
bn container run ../agent-worktree --type planner    # Planner agent
bn container run ../agent-worktree --detach          # Background mode
```

**Implementation:**
```bash
# Equivalent to (simplified):
sudo ctr -n binnacle run \
  --rm \
  --tty \
  --mount type=bind,src=$WORKTREE_PATH,dst=/workspace,options=rbind:rw \
  --mount type=bind,src=$BINNACLE_DATA_PATH,dst=/binnacle,options=rbind:rw \
  --env BN_AGENT_TYPE=worker \
  --env BN_CONTAINER_MODE=true \
  --env COPILOT_GITHUB_TOKEN=$COPILOT_GITHUB_TOKEN \
  localhost/binnacle-self:latest \
  binnacle-self-$(date +%s)
```

#### `bn container stop`

Stop a running binnacle container.

```bash
bn container stop [NAME]

# Examples:
bn container stop binnacle-self-1706083200    # Stop specific container
bn container stop --all                          # Stop all binnacle containers
```

**Implementation:**
```bash
sudo ctr -n binnacle tasks kill $CONTAINER_NAME
sudo ctr -n binnacle containers rm $CONTAINER_NAME
```

#### `bn container list`

List binnacle containers.

```bash
bn container list [OPTIONS]

Options:
  --all        Show all containers (including stopped)
  --quiet      Only show container names

# Examples:
bn container list                  # Show running containers
bn container list --all            # Include stopped
```

**Implementation:**
```bash
sudo ctr -n binnacle containers list
sudo ctr -n binnacle tasks list    # For running status
```

### Containerfile (replaces Dockerfile)

Rename `container/Dockerfile` to `container/Containerfile` for OCI compliance:

```dockerfile
# Binnacle Worker Container
# Build with: buildah bud -t binnacle-self -f container/Containerfile .
FROM docker.io/library/fedora:43

LABEL org.opencontainers.image.title="binnacle-self"
LABEL org.opencontainers.image.description="AI agent worker with binnacle task tracking"

# Install system dependencies
RUN dnf install -y \
    git \
    curl \
    wget \
    jq \
    ripgrep \
    fd-find \
    tree \
    vim \
    && dnf clean all

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Build binnacle from source
COPY . /build/binnacle
WORKDIR /build/binnacle
RUN cargo build --release && \
    cp target/release/bn /usr/local/bin/bn && \
    rm -rf /build

# Install GitHub Copilot CLI
RUN curl -fsSL https://cli.github.com/packages/rpm/gh-cli.repo | \
    tee /etc/yum.repos.d/gh-cli.repo && \
    dnf install -y gh && \
    gh extension install github/copilot-cli || true

# Create working directories
RUN mkdir -p /workspace /binnacle

# Container mode defaults
ENV BN_CONTAINER_MODE=true
ENV BN_AGENT_TYPE=worker
ENV BN_MERGE_TARGET=main

# Copy entrypoint
COPY container/entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

WORKDIR /workspace
ENTRYPOINT ["/entrypoint.sh"]
```

### Namespace Isolation

All binnacle containers run in the `binnacle` namespace:

```bash
# List binnacle containers only
sudo ctr -n binnacle containers list

# Won't see binnacle containers in default namespace
sudo ctr containers list  # Empty or other containers
```

Benefits:
- Clean separation from other containerd workloads
- Easy cleanup: `sudo ctr -n binnacle containers rm $(sudo ctr -n binnacle containers list -q)`
- No interference with k8s.io namespace if Kubernetes is present

### Error Handling

When dependencies are missing, provide helpful messages:

```
$ bn container run ../worktree
Error: containerd not found

To use binnacle containers, install containerd and buildah:

  # Fedora/RHEL
  sudo dnf install containerd buildah
  sudo systemctl enable --now containerd

  # Debian/Ubuntu  
  sudo apt install containerd buildah
  sudo systemctl enable --now containerd

For more info, see: https://github.com/containerd/containerd
```

### Migration from Docker Compose

1. **Remove files:**
   - `container/docker-compose.yml` (deleted)
   - `container/Dockerfile` → renamed to `container/Containerfile`
   - `container/launch-worker.sh` (deleted, replaced by `bn container run`)

2. **Update documentation:**
   - `container/README.md` - rewrite for containerd workflow
   - `README.md` - update container section

3. **Keep entrypoint.sh** - mostly unchanged, works with both runtimes

## Implementation Plan

### Phase 1: Core Runtime Commands

- [ ] Add `bn container` subcommand group
- [ ] Implement `bn container list` (simplest, good for testing)
- [ ] Implement `bn container run` with basic mounts
- [ ] Implement `bn container stop`
- [ ] Add dependency detection and helpful error messages

### Phase 2: Image Building

- [ ] Rename Dockerfile → Containerfile
- [ ] Implement `bn container build` using buildah
- [ ] Add image import to containerd binnacle namespace
- [ ] Handle build caching

### Phase 3: Full Feature Parity

- [ ] Environment variable passthrough (tokens, agent config)
- [ ] Interactive TTY mode (headed)
- [ ] Detached mode
- [ ] Auto-merge functionality (via entrypoint.sh, mostly unchanged)
- [ ] Resource limits (CPU, memory)

### Phase 4: Cleanup & Documentation

- [ ] Remove docker-compose.yml
- [ ] Remove launch-worker.sh
- [ ] Update container/README.md
- [ ] Update main README.md
- [ ] Update PRD_CONTAINER_WORKER.md or mark superseded

## Testing Strategy

### Unit Tests

1. **Dependency detection** - Verify `bn` detects missing containerd/buildah
2. **Namespace handling** - Verify binnacle namespace is used
3. **Mount path resolution** - Verify worktree and data paths resolve correctly
4. **Error messages** - Verify helpful install instructions shown

### Integration Tests

1. **Build image** - `bn container build` produces valid OCI image
2. **Import to containerd** - Image appears in binnacle namespace
3. **Run container** - Container starts with correct mounts
4. **Stop container** - Container is properly terminated
5. **List containers** - Shows running/stopped containers correctly

### Manual Testing Checklist

```bash
# 1. Verify dependencies
which ctr buildah  # Both should exist

# 2. Build image
bn container build
sudo ctr -n binnacle images list  # Should show binnacle-self

# 3. Create test worktree
git worktree add ../test-worker -b test-agent

# 4. Run container (headed)
bn container run ../test-worker
# Should see interactive output, bn orient, etc.

# 5. In another terminal, list containers
bn container list

# 6. Stop container
bn container stop <name>

# 7. Verify cleanup
bn container list --all
```

## Security Considerations

1. **Root required** - containerd typically requires root/sudo. Future work could explore rootless containerd.

2. **Token handling** - Same as current: pass via environment, not baked into image.

3. **Namespace isolation** - binnacle namespace prevents interference with other workloads.

4. **Mount security** - Only specified paths mounted; no full filesystem access.

## Open Questions

1. **Rootless support** - Should v1 attempt rootless containerd, or defer to v2?
   - **Decision:** Defer. Assume root/sudo for v1.

2. **Image registry** - Should `bn container build` optionally push to ghcr.io?
   - **Decision:** Defer to future PRD. Local-only for v1.

3. **Multiple simultaneous containers** - How to handle naming conflicts?
   - **Decision:** Auto-generate unique names with timestamp suffix.

## Success Criteria

1. `bn container build` builds a working image using buildah
2. `bn container run` starts an interactive container that can complete tasks
3. `bn container stop` cleanly terminates containers
4. `bn container list` shows container status
5. Container can read/write binnacle graph data
6. Auto-merge works (via existing entrypoint.sh)
7. Helpful error messages when containerd/buildah not installed
8. Docker Compose files removed from repository

## Related Work

- **bn-e01c**: Original idea this PRD addresses
- **bn-8918**: Container worker with unconfined tool access (implemented via this)
- **bn-bdf0**: Graph mounting (implemented via this)
- **bn-ad78**: Auto-detect project and generate Containerfile (SEPARATE - not included)
- **PRD_CONTAINER_WORKER.md**: Previous Docker Compose implementation (SUPERSEDED)
