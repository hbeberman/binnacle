# Binnacle Container Worker

Run AI agents in Docker containers with full access to the binnacle task graph.

## Quick Start

```bash
# 1. Create a git worktree for the agent to work in
git worktree add ../agent-worktree -b agent-feature

# 2. Set your GitHub token for AI agent auth
export COPILOT_GITHUB_TOKEN="your-token"

# 3. Launch the worker
./container/launch-worker.sh ../agent-worktree
```

## Overview

The container worker provides:

- **Isolation** - Agent runs in a sandboxed environment
- **Reproducibility** - Consistent environment across machines
- **Resource Control** - CPU and memory limits
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
└───────────┼──────────────────────┼──────────────────────────────┘
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
│  entrypoint.sh:                                                 │
│    1. bn orient --type $BN_AGENT_TYPE                          │
│    2. Run AI agent (copilot/claude)                            │
│    3. git merge to $BN_MERGE_TARGET                            │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `WORKTREE_PATH` | Yes | - | Path to git worktree |
| `BINNACLE_DATA_PATH` | Yes | - | Path to binnacle data directory |
| `COPILOT_GITHUB_TOKEN` | Yes* | - | GitHub token for AI auth |
| `BN_AGENT_TYPE` | No | `worker` | Agent type (worker, planner, buddy) |
| `BN_MERGE_TARGET` | No | `main` | Branch to merge into on exit |
| `BN_AUTO_MERGE` | No | `true` | Enable auto-merge on success |
| `BN_AGENT_NAME` | No | auto | Agent name for identification |

*Or `GH_TOKEN` / `GITHUB_TOKEN`

## Usage

### Using the Launch Script (Recommended)

```bash
# Basic usage
./container/launch-worker.sh /path/to/worktree

# With specific agent type
./container/launch-worker.sh /path/to/worktree planner

# With custom settings
BN_AUTO_MERGE=false ./container/launch-worker.sh /path/to/worktree
```

### Using Docker Compose Directly

```bash
cd container

# Set required variables
export WORKTREE_PATH=/path/to/worktree
export BINNACLE_DATA_PATH=~/.local/share/binnacle/abc123def456

# Run worker
docker compose up binnacle-worker

# Or start a debug shell
docker compose up binnacle-shell
```

### Manual Docker Run

```bash
docker build -t binnacle-worker -f container/Dockerfile .

docker run -it --rm \
  -v /path/to/worktree:/workspace \
  -v ~/.local/share/binnacle/HASH:/binnacle \
  -e COPILOT_GITHUB_TOKEN=$COPILOT_GITHUB_TOKEN \
  binnacle-worker
```

## Files

| File | Description |
|------|-------------|
| `Dockerfile` | Fedora 43 base with binnacle, git, and dev tools |
| `entrypoint.sh` | Orchestrates agent setup, execution, and merge |
| `docker-compose.yml` | Service definitions for worker and shell |
| `launch-worker.sh` | Helper to launch with correct mounts |

## Workflow

1. **Setup**: Create a git worktree for isolated work
2. **Launch**: Run the container with appropriate mounts
3. **Orient**: Container runs `bn orient` to load task state
4. **Execute**: AI agent picks and completes tasks
5. **Merge**: On success, work is merged to target branch

## Troubleshooting

### Container can't find binnacle data

Ensure `BINNACLE_DATA_PATH` points to the correct hash directory:

```bash
# Find the hash for your repo
REPO_ROOT=$(git rev-parse --show-toplevel)
REPO_HASH=$(echo -n "$REPO_ROOT" | sha256sum | cut -c1-12)
echo ~/.local/share/binnacle/$REPO_HASH
```

### Authentication errors

Check that your token is set:

```bash
echo $COPILOT_GITHUB_TOKEN  # Should not be empty
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
- SSH mount is read-only
- Container only accesses the specific worktree, not entire filesystem
- Resource limits prevent runaway processes

## Related

- [PRD: Container Worker](../prds/PRD_CONTAINER_WORKER.md) - Full specification
- [Binnacle README](../README.md) - Main project documentation
