# PRD: Agent Deployment Model Rework (v2)

## Overview

Rework binnacle's agent deployment model so that `bn` manages agent container lifecycles **declaratively**. Operators set desired agent counts (min/max per type), and binnacle maintains that count automatically - spawning containers when below minimum, stopping them when above maximum or when work is exhausted.

## Related Ideas

- **bn-1f83** (Carrier-style orchestrator) - Future: dispatching work to agent swarms
- **bn-19aa** (Arbiter agent) - Future: privileged governance roles  
- **bn-5077** (Agent commit pools) - Future: staging branches for multi-agent work

This PRD focuses on **lifecycle management only** - the foundation that orchestration will build upon.

## Problem Statement

Currently:
1. Agents self-register via `bn orient` using parent PID as identifier
2. Binnacle only knows about agents after they call `bn orient`
3. Container lifecycle is managed externally (via `containeragent.sh`)
4. `bn goodbye` relies on process termination to end parent processes
5. No way to spawn agents from binnacle itself
6. No automatic scaling based on work availability

This creates issues:
- GUI can't show agents until they self-register
- No central control over agent spawning/termination
- Agent IDs are PIDs (not stable, not meaningful)
- Manual intervention required to start/stop agents
- Agents run even when no work is available

## Solution

Make binnacle the **authority** for agent lifecycle with **declarative scaling**:

1. **Agents as Graph Nodes**: Agent IDs use standard `bn-xxxx` format, stored in the graph like any other entity
2. **Declarative Counts**: `bn agent scale <type> --min N --max M` sets desired agent counts
3. **Auto-reconciliation**: Binnacle spawns/stops containers to maintain desired count
4. **Work-aware Scaling**: Workers scale to 0 when no tasks/bugs are ready (regardless of min)
5. **Graceful Shutdown**: `bn goodbye` signals readiness to terminate; binnacle stops container

## Detailed Design

### Agent as Graph Node

Agents are first-class entities in the task graph:

```rust
pub struct Agent {
    pub id: String,           // bn-xxxx (standard node ID format)
    pub entity_type: String,  // "agent"
    pub name: String,         // Human-friendly name (e.g., "worker-alpha")
    pub agent_type: AgentType,
    pub container_id: Option<String>,  // containerd container ID
    pub pid: Option<u32>,     // PID inside container (optional, for debugging)
    pub status: AgentStatus,
    pub created_at: DateTime<Utc>,
    pub last_heartbeat: DateTime<Utc>,
    pub goodbye_at: Option<DateTime<Utc>>,
    pub tasks: Vec<String>,
    pub current_action: Option<String>,
    // ... existing fields
}
```

### Agent Scaling Configuration

Stored in binnacle config:

```toml
[agents.worker]
min = 1
max = 3

[agents.prd]
min = 0
max = 1

[agents.buddy]
min = 0
max = 1

[agents.free]
min = 0
max = 2
```

### CLI Commands

#### `bn agent scale <type> [options]`

Set desired agent count for a type.

```bash
bn agent scale worker --min 1 --max 3   # Keep 1-3 workers running
bn agent scale worker --min 0 --max 0   # Disable workers entirely
bn agent scale prd --max 1              # Allow up to 1 PRD agent
bn agent scale buddy --min 1            # Always keep 1 buddy alive

# View current scaling config
bn agent scale                          # Show all scaling configs
bn agent scale worker                   # Show worker scaling config
```

**Work-aware behavior for workers:**
- If `min = 1` but no ready tasks/bugs exist, actual count = 0
- When work becomes available, binnacle spawns up to `min` workers
- This prevents idle workers consuming resources

#### `bn agent ls [options]`

List agents (replaces `bn agent list`).

```bash
bn agent ls                              # List running agents
bn agent ls --all                        # Include stopped/exited
bn agent ls -H                           # Human-readable format
```

Output includes:
- Agent ID (bn-xxxx)
- Name
- Type
- Status (spawning/running/active/idle/goodbye/stopped)
- Container ID
- Current task (if any)

#### `bn agent spawn <type> [options]` (Manual Override)

Manually spawn an agent (bypasses min/max, useful for testing).

```bash
bn agent spawn worker                    # Spawn one worker
bn agent spawn worker --name "debug-1"   # With custom name
bn agent spawn worker --cpus 2 --memory 4g
```

#### `bn agent rm <id> [options]` (Manual Override)

Manually stop an agent (bypasses min count).

```bash
bn agent rm bn-1234                      # Graceful stop
bn agent rm bn-1234 --force              # Immediate force stop
bn agent rm --type worker --all          # Stop all workers
```

### Reconciliation Loop

Binnacle maintains agent counts via a reconciliation process:

```
Every 30 seconds (or on-demand trigger):
  For each agent_type in config:
    current = count running agents of type
    desired = calculate_desired(type, min, max, work_available)
    
    if current < desired:
      spawn (desired - current) agents
    elif current > desired:
      stop (current - desired) agents (prefer idle, then oldest)
```

**calculate_desired logic:**
```
if type == worker:
  work_count = count(ready_tasks) + count(ready_bugs)
  if work_count == 0:
    return 0  # No work, no workers
  else:
    return clamp(work_count, min, max)
else:
  return min  # Non-workers: just maintain min
```

### Environment Variables

Passed to every agent container:

| Variable | Description |
|----------|-------------|
| `BN_AGENT_ID` | Pre-assigned agent ID (bn-xxxx) |
| `BN_AGENT_NAME` | Human-friendly name |
| `BN_AGENT_TYPE` | worker/prd/buddy/free |
| `BN_DATA_DIR` | Path to binnacle data inside container |
| `BN_STORAGE_HASH` | Storage hash for this repo |
| `BN_CONTAINER_MODE` | Always "true" |

### Goodbye Flow (Reworked)

**Current flow:**
1. Agent calls `bn goodbye "reason"`
2. `bn` looks up agent by parent PID
3. `bn` terminates parent/grandparent processes

**New flow:**
1. Agent calls `bn goodbye "reason"`
2. `bn` reads `BN_AGENT_ID` from environment
3. `bn` updates agent record: `goodbye_at = now()`, `status = goodbye`
4. `bn` returns success (does NOT terminate anything)
5. Reconciliation loop detects `goodbye_at` timestamp
6. Binnacle stops container gracefully (SIGTERM, wait 15s, then force stop)
7. If below min count (and work available for workers), spawn replacement

### Timeout / Force Stop

- **Heartbeat timeout**: 30 minutes without heartbeat -> mark stale, force stop
- **Goodbye timeout**: 15 seconds after goodbye -> force stop if still running
- **Manual force**: `bn agent rm --force` -> immediate force stop

### Agent Status Flow

```
spawning -> running -> active/idle -> goodbye -> stopped
                    \-> stale (no heartbeat) -> force_stopped
```

### Rootless Containerd

Current code uses `sudo ctr`. For rootless:

1. Check if user has rootless containerd configured
2. Use `ctr` without sudo if rootless socket exists
3. Fall back to sudo if needed (with warning)

Rootless containerd setup:
- Socket at `$XDG_RUNTIME_DIR/containerd/containerd.sock`
- Namespace: `binnacle` (user-owned)

### Storage Changes

- Agents stored as graph nodes in `tasks.jsonl` with `entity_type: "agent"`
- Agent scaling config in `config.toml`
- Remove PID-based agent identification

### Backward Compatibility

- Keep `bn orient` working (for non-containerized agents)
- If `BN_AGENT_ID` is set, use it; otherwise fall back to PID-based registration
- Deprecate PID-based agent identification over time

## Agent Types

| Type | Description | Work-Aware | Default Min/Max |
|------|-------------|------------|-----------------|
| `worker` | Pick tasks from queue, implement, test, commit | Yes (scales to 0 when no work) | 0/1 |
| `prd` | Render ideas into PRDs | No | 0/1 |
| `buddy` | Quick bug/task/idea insertion | No | 0/1 |
| `free` | General purpose with binnacle access | No | 0/1 |

## Success Criteria

1. `bn agent scale worker --min 1 --max 3` persists scaling config
2. Binnacle automatically spawns workers when tasks are ready
3. Workers scale to 0 when no work is available (even if min > 0)
4. `bn goodbye` triggers graceful container shutdown + replacement if needed
5. `bn agent rm` stops containers reliably
6. Stale agents (30min no heartbeat) are auto-terminated
7. Rootless containerd works without sudo

## Out of Scope

- GUI integration (future PRD)
- Advanced orchestration / work assignment (future PRD, see bn-1f83)
- Multi-repo agent pools
- Agent-to-agent communication
- Privileged agent roles (bn-19aa)

## Implementation Phases

### Phase 1: Agent as Graph Node
- Use standard `bn-xxxx` ID format for agents
- Store agents in task graph as entities
- Add `BN_AGENT_ID` environment variable support
- Update `bn orient` and `bn goodbye` to use agent ID from env

### Phase 2: Scaling Configuration
- Add agent scaling config to config.toml
- Implement `bn agent scale` command
- Store min/max per agent type

### Phase 3: Container Spawn/Stop
- Implement container spawning with pre-assigned agent ID
- Implement graceful shutdown (15s timeout)
- Integrate with existing container_run logic

### Phase 4: Reconciliation Loop
- Implement reconciliation logic (spawn/stop to match desired)
- Add work-aware scaling for workers
- Run reconciliation on timer and on-demand

### Phase 5: Heartbeat & Stale Detection
- Track last_heartbeat from agent commands
- Auto-terminate after 30min timeout
- Detect goodbye and trigger container shutdown

### Phase 6: Rootless Support
- Detect rootless containerd
- Remove sudo requirement where possible
- Document rootless setup

## Testing Strategy

- Unit tests: Agent ID generation, scaling config, reconciliation logic
- Integration tests: Scale up/down, goodbye flow, work-aware scaling
- Container tests: Actual containerd operations (CI matrix)
- Timeout tests: Stale detection, force stop

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Rootless containerd complexity | Phase 6 is separate; sudo fallback always works |
| Container orphans on crash | Reconciliation loop cleans up orphans |
| Race conditions in goodbye | Use container ID as ground truth, not agent status |
| Thrashing (rapid spawn/stop) | Cooldown period between reconciliation actions |
