# PRD: Agent Lifecycle Management (`bn goodbye`)

**Idea:** bni-5666  
**Status:** Ready for implementation  
**Priority:** P1  
**Tags:** agent, lifecycle, process-management

## Problem Statement

AI agents working with binnacle have no standardized way to:
1. Register themselves when starting work
2. Gracefully terminate when their work is complete
3. Be tracked and monitored by humans
4. Clean up after themselves or other crashed agents

Currently, agents just... exist. There's no visibility into which agents are active, what they're working on, or how to manage them. When agents crash or hang, there's no cleanup mechanism.

## Solution

Implement agent lifecycle management with two core commands:
- **`bn orient`** (enhanced) - Registers the agent with binnacle, tracking PID, name, and associated tasks
- **`bn goodbye`** - Gracefully terminates the agent's parent process after logging the exit

Plus supporting infrastructure:
- **`bn agent list`** - List active agents
- **`bn agent kill`** - Terminate a specific agent (human-only, blocked for agents)
- **GUI Agents Pane** - Visual dashboard of active agents

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Registration requirement | Required via `bn orient` | Ensures all agents are tracked; goodbye warns if not registered |
| Target process | Parent PID of `bn goodbye` caller | Agent shells typically spawn bn as child; killing parent terminates agent |
| Termination signal | SIGTERM, then SIGKILL after 5s | Graceful first, force if necessary |
| Agent identification | Optional `--name` flag, auto-generate if not provided | Flexible; agents can self-identify or remain anonymous |
| Task association | Auto-detect from `bn task update --status in_progress` | No extra work for agents; natural workflow |
| Multi-task handling | Allow with warning + `--force` | Agents sometimes legitimately work on multiple tasks |
| Stale PID cleanup | On `bn orient` and `bn goodbye` only | Minimal overhead; cleans up when agents interact |
| Storage | `agents.jsonl` in binnacle data directory | Consistent with other binnacle data; persists across restarts |
| MCP exposure | Read-only (`bn_agent_list`) | Goodbye is dangerous; don't expose to MCP |
| Agent kill protection | Block `bn agent kill` in agent.sh | Prevent agents from killing each other |

## Data Model

### Agent Registration (`agents.jsonl`)

```json
{
  "pid": 12345,
  "parent_pid": 12340,
  "name": "claude-session-1",
  "started_at": "2026-01-23T07:00:00Z",
  "last_activity_at": "2026-01-23T07:15:00Z",
  "tasks": ["bn-a1b2", "bn-c3d4"],
  "command_count": 42
}
```

Fields:
- `pid` - The PID of the process that called `bn orient` (typically the agent's shell)
- `parent_pid` - Parent PID, used by `bn goodbye` to know what to terminate
- `name` - Optional human-readable identifier
- `started_at` - When `bn orient` was called
- `last_activity_at` - Updated on every `bn` command from this agent
- `tasks` - Tasks currently in_progress by this agent
- `command_count` - Number of `bn` commands executed (activity metric)

## Command Specifications

### `bn orient` (Enhanced)

Current behavior plus agent registration.

```bash
bn orient [--name "agent-name"]
```

**New behavior:**
1. Auto-initialize binnacle if needed (existing)
2. Register agent in `agents.jsonl`:
   - Store PID, parent PID, name (or auto-generated), timestamp
3. Clean up stale PIDs (check if registered PIDs are still running)
4. Output includes agent registration confirmation

**Example output:**
```
$ bn orient -H --name "claude-1"
Agent registered: claude-1 (PID 12345)

Binnacle - AI agent task tracker
...existing orient output...
```

**JSON output:**
```json
{
  "agent": {
    "pid": 12345,
    "parent_pid": 12340,
    "name": "claude-1",
    "registered": true
  },
  "initialized": true,
  "total_tasks": 42,
  ...
}
```

### `bn goodbye`

Gracefully terminate the agent.

```bash
bn goodbye ["reason message"]
```

**Behavior:**
1. Look up agent registration by current PID
2. If not registered: log warning, proceed anyway
3. Log termination to `action_log` with optional reason
4. Clean up stale PIDs from `agents.jsonl`
5. Remove own registration from `agents.jsonl`
6. Send SIGTERM to parent PID
7. Wait up to 5 seconds
8. If parent still running, send SIGKILL

**Example:**
```bash
$ bn goodbye "Task bn-a1b2 complete, all tests passing"
Goodbye logged. Terminating agent (PID 12340)...
```

**If not registered:**
```bash
$ bn goodbye "done"
Warning: Agent not registered (did you run bn orient?). Terminating anyway.
Goodbye logged. Terminating agent (PID 12340)...
```

### `bn agent list`

List active agents.

```bash
bn agent list
```

**Human-readable output:**
```
Active Agents (2):

  claude-1 (PID 12345)
    Started: 2h ago
    Last activity: 5m ago
    Tasks: bn-a1b2, bn-c3d4
    Commands: 42

  agent-2 (PID 12400)
    Started: 30m ago
    Last activity: 2m ago
    Tasks: bn-e5f6
    Commands: 15
```

**JSON output:**
```json
{
  "agents": [
    {
      "pid": 12345,
      "parent_pid": 12340,
      "name": "claude-1",
      "started_at": "2026-01-23T05:00:00Z",
      "last_activity_at": "2026-01-23T06:55:00Z",
      "tasks": ["bn-a1b2", "bn-c3d4"],
      "command_count": 42
    }
  ],
  "count": 2
}
```

### `bn agent kill`

Terminate a specific agent (human-only).

```bash
bn agent kill <pid-or-name>
```

**Behavior:**
1. Look up agent by PID or name
2. If not found: error
3. Send SIGTERM to agent's parent PID
4. Wait up to 5 seconds
5. If still running, send SIGKILL
6. Remove from `agents.jsonl`
7. Log to `action_log`

**Example:**
```bash
$ bn agent kill claude-1
Terminated agent claude-1 (PID 12345)
```

**Protection:** Add `bn agent kill` to blocked commands in `agent.sh` to prevent agents from killing each other.

### Task Association (Enhanced `bn task update`)

When an agent runs `bn task update <id> --status in_progress`:

1. Check if agent is registered (by PID)
2. If registered:
   - Check if agent already has another task in_progress
   - If yes: prompt for confirmation (unless `--force`)
   - Add task to agent's `tasks` list
3. Proceed with normal task update

**Multi-task warning:**
```
$ bn task update bn-c3d4 --status in_progress
Warning: You already have bn-a1b2 in progress. 
Are you sure you want to work on multiple tasks? [y/N]

Use --force to skip this confirmation.
```

**With --force:**
```
$ bn task update bn-c3d4 --status in_progress --force
Task bn-c3d4 updated (note: agent now has 2 tasks in_progress)
```

## GUI: Agents Pane

Add a new "Agents" tab alongside "Graph" and "Ready Tasks".

### Display

| Column | Description |
|--------|-------------|
| Name | Agent name or "agent-{pid}" |
| Status | 🟢 Active / 🟡 Idle (no activity >5min) / 🔴 Stale |
| Tasks | Linked task IDs, clickable |
| Started | Relative time (e.g., "2h ago") |
| Last Activity | Relative time |
| Commands | Count of bn commands |
| Actions | "Terminate" button |

### Activity Log

Below the agent list, show a scrollable activity log:
```
[07:15:32] claude-1: bn task update bn-a1b2 --status in_progress
[07:15:45] claude-1: bn test run --task bn-a1b2
[07:16:02] agent-2: bn task show bn-e5f6
```

### Terminate Confirmation

When clicking "Terminate" button:
```
⚠️ Terminate agent "claude-1"?

This will send SIGTERM to PID 12340.
The agent is currently working on: bn-a1b2, bn-c3d4

[Cancel] [Terminate]
```

## Activity Tracking

Track activity metrics per agent:
- **Command count** - Increment on every `bn` command from registered PID
- **Last activity** - Update timestamp on every `bn` command
- **Commands log** - Store recent commands in memory for GUI display (not persisted)

This information helps humans understand what agents are doing and detect stuck agents.

## Implementation Tasks

### Phase 1: Core Agent Registry
- [ ] Create `Agent` model struct
- [ ] Create `agents.jsonl` storage (read/write/delete)
- [ ] Implement stale PID detection and cleanup
- [ ] Add `--name` flag to `bn orient`
- [ ] Register agent on `bn orient`
- [ ] Update `last_activity_at` on every `bn` command from registered agent
- [ ] Increment `command_count` on every `bn` command

### Phase 2: Goodbye Command
- [ ] Implement `bn goodbye` command
- [ ] Add optional reason argument
- [ ] Implement SIGTERM + SIGKILL logic with 5s timeout
- [ ] Log termination to `action_log`
- [ ] Handle unregistered agent case (warning)
- [ ] Remove agent from registry on goodbye

### Phase 3: Agent Management
- [ ] Implement `bn agent list` command
- [ ] Implement `bn agent kill` command
- [ ] Add `bn agent kill` to blocked commands in `agent.sh`
- [ ] Add `bn_agent_list` MCP tool (read-only)

### Phase 4: Task Association
- [ ] Track task association on `bn task update --status in_progress`
- [ ] Remove task association on task close/done
- [ ] Add multi-task warning with `--force` override
- [ ] Log multi-task force to `action_log`

### Phase 5: GUI Agents Pane
- [ ] Add "Agents" tab to GUI navigation
- [ ] Implement agents list component
- [ ] Add activity log component
- [ ] Implement terminate button with confirmation modal
- [ ] Add WebSocket updates for real-time agent status
- [ ] Style idle/stale status indicators

## Testing Strategy

### Unit Tests
- Agent model serialization/deserialization
- Stale PID detection logic
- SIGTERM/SIGKILL timeout logic (mocked)
- Multi-task detection logic

### Integration Tests
- `bn orient --name X` registers agent
- `bn goodbye` removes registration and signals parent
- `bn agent list` shows registered agents
- `bn agent kill` terminates target agent
- Stale PID cleanup on orient
- Task association tracking
- Multi-task warning behavior

### Manual Testing
- Verify goodbye actually terminates agent shell
- Verify GUI updates in real-time
- Verify agent.sh blocks `bn agent kill`

## Open Questions

1. **Parent PID reliability** - Need to experiment to confirm killing parent PID is the right approach for various agent setups (Claude, Codex, etc.)

## Future Considerations (Out of Scope for v1)

- [ ] **AGENT_NAME environment variable** - Auto-detect agent name from `AGENT_NAME` or `BN_AGENT_NAME` env var (useful for containerized agents)
- [ ] **External monitoring integration** - Connect to Prometheus, Datadog, etc. for resource usage metrics
- [ ] **Agent health checks** - Heartbeat mechanism to detect stuck agents
- [ ] **Agent quotas** - Limit number of concurrent agents or tasks per agent
