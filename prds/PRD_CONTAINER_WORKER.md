# PRD: Binnacle Container Worker

**Status:** Draft
**Created:** 2026-01-24
**Related Ideas:** bn-8918, bn-bdf0, bn-dc17

## Overview

This PRD defines how to run AI agents (Copilot CLI) inside Docker containers with full access to the binnacle task graph. The container worker model provides isolation, reproducibility, and a clean separation between the host system and agent execution environment.

## Motivation

Running agents in containers provides several benefits:

1. **Isolation** - Agent has limited blast radius; can't affect host system outside mounted paths
2. **Reproducibility** - Consistent environment across different host machines
3. **Resource Control** - Can limit CPU, memory, and other resources per agent
4. **Security Foundation** - Future PRDs can layer in write-protection and graph isolation
5. **Operational Simplicity** - Single command to spin up a working agent

## Goals

- Run Copilot CLI with `--allow-all -p` inside a Fedora 43 container
- Share binnacle graph data between host and container (full read/write)
- Mount repository worktree into container
- Auto-merge completed work back to main branch
- Detect container mode in `bn` and use well-known paths

## Non-Goals (Future PRDs)

- Multi-agent orchestration and coordination
- Write-protected or isolated graph partitions
- Conflict resolution / automatic rebase
- CI validation gates before merge
- Container image registry publishing (ghcr.io)

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
│  │ ~/worktrees/    │             │                              │
│  │   agent-coder-1 │             │                              │
│  └────────┬────────┘             │                              │
│           │                      │                              │
└───────────┼──────────────────────┼──────────────────────────────┘
            │                      │
            │ mount                │ mount
            ▼                      ▼
┌─────────────────────────────────────────────────────────────────┐
│                     DOCKER CONTAINER                            │
│                                                                 │
│  ┌─────────────────┐    ┌─────────────────┐                    │
│  │ /workspace      │    │ /binnacle       │                    │
│  │ (repo worktree) │    │ (graph data)    │                    │
│  └─────────────────┘    └─────────────────┘                    │
│                                                                 │
│  ┌─────────────────────────────────────────┐                   │
│  │            entrypoint.sh                │                   │
│  │  1. bn orient --type $BN_AGENT_TYPE     │                   │
│  │  2. copilot --allow-all -p "..."       │                   │
│  │  3. git merge to $BN_MERGE_TARGET       │                   │
│  └─────────────────────────────────────────┘                   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Container Paths

| Container Path | Host Mount Source | Mode | Purpose |
|----------------|-------------------|------|---------|
| `/workspace` | Repo worktree (e.g., `~/worktrees/agent-1`) | r/w | Agent working directory |
| `/binnacle` | `~/.local/share/binnacle/<repo-hash>/` | r/w | Shared binnacle graph |
| `/copilot-config` | `~/.config/github-copilot/` | r/o | Copilot CLI config |

### Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `BN_AGENT_TYPE` | No | `coder` | Agent type passed to `bn orient --type` |
| `BN_AGENT_NAME` | No | (auto) | Agent name for identification |
| `BN_CONTAINER_MODE` | No | `true` | Enables container path detection |
| `BN_MERGE_TARGET` | No | `main` | Branch to fast-forward merge into on exit |
| `BN_INITIAL_PROMPT` | No | (default) | Custom prompt for Copilot startup |
| `COPILOT_GITHUB_TOKEN` | Yes | - | GitHub token for Copilot auth |

### Binnacle Container Mode Detection

Update `bn` to detect container mode and use `/binnacle` as the data directory:

```rust
fn get_data_directory(repo_hash: &str) -> PathBuf {
    // Check for container mode
    if std::env::var("BN_CONTAINER_MODE").is_ok() || Path::new("/binnacle").exists() {
        return PathBuf::from("/binnacle");
    }

    // Default: ~/.local/share/binnacle/<repo-hash>/
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("binnacle")
        .join(repo_hash)
}
```

### Entrypoint Script

```bash
#!/bin/bash
set -e

# Configuration with defaults
BN_AGENT_TYPE="${BN_AGENT_TYPE:-coder}"
BN_MERGE_TARGET="${BN_MERGE_TARGET:-main}"
BN_INITIAL_PROMPT="${BN_INITIAL_PROMPT:-Run bn ready to see available tasks, pick one, and complete it. Call bn goodbye when done.}"

cd /workspace

# Orient the agent
echo "🧭 Orienting agent (type: $BN_AGENT_TYPE)..."
bn orient --type "$BN_AGENT_TYPE" -H

# Get current branch for merge later
WORK_BRANCH=$(git rev-parse --abbrev-ref HEAD)
echo "📍 Working on branch: $WORK_BRANCH"

# Run Copilot agent
echo "🤖 Starting Copilot agent..."
copilot --allow-all -p "$BN_INITIAL_PROMPT"
COPILOT_EXIT=$?

if [ $COPILOT_EXIT -ne 0 ]; then
    echo "❌ Copilot exited with error code $COPILOT_EXIT"
    exit $COPILOT_EXIT
fi

# Attempt fast-forward merge
echo "🔀 Merging $WORK_BRANCH into $BN_MERGE_TARGET..."

# Fetch latest target branch
git fetch origin "$BN_MERGE_TARGET" 2>/dev/null || true

# Checkout target and attempt fast-forward merge
git checkout "$BN_MERGE_TARGET"
if git merge --ff-only "$WORK_BRANCH"; then
    echo "✅ Successfully merged $WORK_BRANCH into $BN_MERGE_TARGET"
else
    echo "❌ Fast-forward merge failed - manual intervention required"
    echo "   Branch $WORK_BRANCH has diverged from $BN_MERGE_TARGET"
    git checkout "$WORK_BRANCH"  # Return to work branch
    exit 1
fi

echo "🎉 Agent work complete!"
```

### Dockerfile

```dockerfile
# Binnacle Container Worker
# Base: Fedora 43
FROM docker.io/library/fedora:43

LABEL org.opencontainers.image.title="binnacle-worker"
LABEL org.opencontainers.image.description="AI agent worker with binnacle task tracking"
LABEL org.opencontainers.image.source="https://github.com/henry/binnacle"

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

# Install Rust (for building binnacle if needed)
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Install binnacle
# Option 1: From crates.io (when published)
# RUN cargo install binnacle
# Option 2: Build from source (during development)
COPY . /build/binnacle
WORKDIR /build/binnacle
RUN cargo build --release && \
    cp target/release/bn /usr/local/bin/bn && \
    rm -rf /build

# Install GitHub Copilot CLI
RUN curl -fsSL https://cli.github.com/packages/rpm/gh-cli.repo | tee /etc/yum.repos.d/gh-cli.repo && \
    dnf install -y gh && \
    gh extension install github/copilot-cli || true

# Alternative: Direct Copilot CLI install (if available)
# RUN curl -fsSL https://copilot.github.com/install.sh | sh

# Create working directories
RUN mkdir -p /workspace /binnacle /copilot-config

# Set container mode
ENV BN_CONTAINER_MODE=true
ENV BN_AGENT_TYPE=coder
ENV BN_MERGE_TARGET=main

# Copy entrypoint
COPY container/entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

WORKDIR /workspace
ENTRYPOINT ["/entrypoint.sh"]
```

### Docker Compose

```yaml
# docker-compose.yml
# Usage: docker-compose up binnacle-worker

version: "3.8"

services:
  binnacle-worker:
    build:
      context: .
      dockerfile: Dockerfile
    container_name: binnacle-worker

    environment:
      - BN_AGENT_TYPE=${BN_AGENT_TYPE:-coder}
      - BN_AGENT_NAME=${BN_AGENT_NAME:-container-agent}
      - BN_MERGE_TARGET=${BN_MERGE_TARGET:-main}
      - BN_CONTAINER_MODE=true
      - COPILOT_GITHUB_TOKEN=${COPILOT_GITHUB_TOKEN}
      # Alternative auth methods
      - GH_TOKEN=${GH_TOKEN:-}
      - GITHUB_TOKEN=${GITHUB_TOKEN:-}

    volumes:
      # Repository worktree (REQUIRED - set WORKTREE_PATH)
      - ${WORKTREE_PATH:?Set WORKTREE_PATH to your git worktree}:/workspace:rw

      # Binnacle data directory (REQUIRED - set BINNACLE_DATA_PATH)
      - ${BINNACLE_DATA_PATH:?Set BINNACLE_DATA_PATH}:/binnacle:rw

      # Copilot config (optional, for custom settings)
      - ${COPILOT_CONFIG_PATH:-~/.config/github-copilot}:/copilot-config:ro

      # SSH keys for git operations (optional)
      - ${SSH_KEY_PATH:-~/.ssh}:/root/.ssh:ro

    # Resource limits (adjust as needed)
    deploy:
      resources:
        limits:
          cpus: '4'
          memory: 8G
        reservations:
          cpus: '2'
          memory: 4G

    # Keep stdin open for interactive debugging
    stdin_open: true
    tty: true

  # Debug shell variant - doesn't auto-start agent
  binnacle-shell:
    extends:
      service: binnacle-worker
    container_name: binnacle-shell
    entrypoint: /bin/bash
    command: []
```

### Helper Script for Launching

```bash
#!/bin/bash
# launch-worker.sh
# Usage: ./launch-worker.sh /path/to/worktree [agent-type]

set -e

WORKTREE_PATH="${1:?Usage: $0 /path/to/worktree [agent-type]}"
AGENT_TYPE="${2:-coder}"

# Validate worktree
if [ ! -d "$WORKTREE_PATH/.git" ] && [ ! -f "$WORKTREE_PATH/.git" ]; then
    echo "Error: $WORKTREE_PATH is not a git repository or worktree"
    exit 1
fi

# Get repo root and hash for binnacle data path
REPO_ROOT=$(cd "$WORKTREE_PATH" && git rev-parse --show-toplevel)
REPO_HASH=$(echo -n "$REPO_ROOT" | sha256sum | cut -c1-16)
BINNACLE_DATA="${XDG_DATA_HOME:-$HOME/.local/share}/binnacle/$REPO_HASH"

# Ensure binnacle data directory exists
mkdir -p "$BINNACLE_DATA"

echo "🚀 Launching binnacle worker..."
echo "   Worktree: $WORKTREE_PATH"
echo "   Agent type: $AGENT_TYPE"
echo "   Binnacle data: $BINNACLE_DATA"

# Check for auth token
if [ -z "$COPILOT_GITHUB_TOKEN" ] && [ -z "$GH_TOKEN" ]; then
    echo "⚠️  Warning: No COPILOT_GITHUB_TOKEN or GH_TOKEN set"
    echo "   Set one of these environment variables for Copilot auth"
fi

# Launch container
export WORKTREE_PATH
export BINNACLE_DATA_PATH="$BINNACLE_DATA"
export BN_AGENT_TYPE="$AGENT_TYPE"

docker-compose up binnacle-worker
```

## Implementation Plan

### Phase 1: Binnacle Container Mode (bn changes)

- [ ] Add `BN_CONTAINER_MODE` environment variable detection
- [ ] Add `/binnacle` path detection fallback
- [ ] Update `get_data_directory()` to use container paths when detected
- [ ] Add `BN_AGENT_NAME` env var support in `bn orient`
- [ ] Unit tests for container mode detection

### Phase 2: Container Files

- [ ] Create `container/` directory in repo
- [ ] Write `container/Dockerfile` (Fedora 43 base)
- [ ] Write `container/entrypoint.sh`
- [ ] Write `container/docker-compose.yml`
- [ ] Write `container/launch-worker.sh` helper script

### Phase 3: Integration & Testing

- [ ] Test container build on clean system
- [ ] Test worktree mount and `bn` operations
- [ ] Test Copilot CLI auth with token
- [ ] Test auto-merge on successful exit
- [ ] Test merge failure handling
- [ ] Document troubleshooting steps

### Phase 4: Documentation

- [ ] Update README with container usage
- [ ] Add `container/README.md` with detailed instructions
- [ ] Document environment variables
- [ ] Add examples for common workflows

## Testing Strategy

### Unit Tests

1. **Container mode detection** - Verify `bn` uses `/binnacle` when `BN_CONTAINER_MODE=true`
2. **Agent name from env** - Verify `BN_AGENT_NAME` is used in `bn orient`
3. **Fallback behavior** - Verify default paths used when not in container mode

### Integration Tests

1. **Container build** - `docker build` succeeds
2. **Mount validation** - Container can read/write to mounted paths
3. **bn operations** - `bn ready`, `bn task update`, etc. work inside container
4. **Merge success** - Fast-forward merge works when no conflicts
5. **Merge failure** - Proper error when merge cannot fast-forward

### Manual Testing Checklist

```bash
# 1. Build container
docker build -t binnacle-worker -f container/Dockerfile .

# 2. Create test worktree
git worktree add ../test-worktree -b test-agent

# 3. Run container with shell
docker run -it --rm \
  -v $(pwd)/../test-worktree:/workspace \
  -v ~/.local/share/binnacle/$(echo -n $(pwd) | sha256sum | cut -c1-16):/binnacle \
  -e COPILOT_GITHUB_TOKEN=$COPILOT_GITHUB_TOKEN \
  binnacle-worker /bin/bash

# 4. Inside container, verify:
bn orient -H              # Should show project state
bn ready                  # Should list ready tasks
git status                # Should show clean worktree
echo $BN_CONTAINER_MODE   # Should be "true"

# 5. Clean up
git worktree remove ../test-worktree
```

## Security Considerations

1. **Token exposure** - `COPILOT_GITHUB_TOKEN` should be passed via env, not baked into image
2. **SSH key access** - SSH mount is read-only to prevent modification
3. **Worktree isolation** - Container only has access to the specific worktree, not entire filesystem
4. **Future hardening** - Graph write-protection and isolation planned for future PRD

## Open Questions

1. **Copilot CLI installation method** - Is there a direct install script, or must we use `gh extension`?
2. **Worktree creation** - Should the launch script auto-create worktrees, or require pre-created ones?
3. **Container registry** - Should we publish to ghcr.io in a future iteration?

## Success Criteria

1. `docker-compose up binnacle-worker` launches an agent that can complete tasks
2. Agent can read and modify the binnacle graph
3. Completed work is automatically merged to target branch
4. Merge failures are clearly reported without data loss
5. Container can be used repeatedly without host-side cleanup

## Related Work

- **bn-8918**: Original idea for container worker with unconfined tool access
- **bn-bdf0**: Graph mounting concept this PRD implements
- **bn-dc17**: Agent name from environment variable (incorporated here)
