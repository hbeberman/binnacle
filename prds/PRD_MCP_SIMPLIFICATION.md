# PRD: MCP Server Simplification

## Overview

Replace the current complex MCP server implementation (~3,500 lines, 38+ tools) with a simple subprocess wrapper approach (~200 lines, 2-3 tools). Additionally, integrate MCP client configuration into `bn system init` for easy setup.

## Problem Statement

The current Rust MCP server implementation has critical issues:

1. **Unreliable** - "Hangs unpredictably" according to actual usage
2. **High maintenance burden** - Every new CLI command requires a new MCP tool handler with schema definitions
3. **Complexity explosion** - 3,561 lines of code, 38+ individual tool definitions with JSON schemas
4. **Duplication** - Each tool essentially re-implements CLI argument parsing in MCP format

A Python wrapper (`binnacle_wrap`) was created as a workaround that "Just Works" by taking a fundamentally different approach: call the CLI as a subprocess with a timeout.

| Aspect | Current Rust MCP | Python Wrapper |
|--------|------------------|----------------|
| Lines of code | 3,561 | ~144 |
| Tools | 38+ individual | 2 (`set_cwd`, `bn_run`) |
| Maintenance | High (new tool per command) | Zero |
| Reliability | Hangs unpredictably | Reliable |

## Proposed Solution

### Part 1: Simplified MCP Server

Replace the MCP implementation with a subprocess wrapper approach, built into the Rust binary.

#### Tools

| Tool | Description |
|------|-------------|
| `binnacle-set_agent` | Initialize MCP session with working directory and optional agent identity |
| `binnacle-orient` | Register agent (blocked in agents.sh to force shell usage) |
| `binnacle-goodbye` | Terminate agent (blocked in agents.sh to force shell usage) |
| `bn_run` | Execute any bn CLI command as subprocess, returns stdout/stderr/exit_code |

#### Tool: `binnacle-set_agent`

This tool replaces `set_cwd` with a more general purpose session initialization tool that also handles agent identity.

```json
{
  "name": "binnacle-set_agent",
  "description": "Initialize binnacle MCP session with working directory and optional agent identity. Must be called before using bn_run.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "path": {
        "type": "string",
        "description": "Absolute path to a binnacle-managed repository"
      },
      "agent_id": {
        "type": "string",
        "description": "Optional agent ID (bna-xxxx) from a shell-based 'bn orient' call. If provided, MCP calls will be attributed to this agent for tracking and goodbye."
      }
    },
    "required": ["path"]
  }
}
```

**Response:**

```json
{
  "success": true,
  "message": "Session initialized for agent bna-1234",
  "cwd": "/home/user/myrepo",
  "agent_id": "bna-1234"
}
```

If no `agent_id` is provided, the MCP server generates a session-scoped ID for tracking but cannot perform process termination on goodbye.

#### Tool: `bn_run`

```json
{
  "name": "bn_run",
  "description": "Run a binnacle (bn) CLI command. Returns stdout, stderr, and exit code.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "args": {
        "type": "array",
        "items": { "type": "string" },
        "description": "Arguments to pass to bn (e.g., [\"ready\", \"-H\"])"
      }
    },
    "required": ["args"]
  }
}
```

**Response:**

```json
{
  "stdout": "...",
  "stderr": "...",
  "exit_code": 0
}
```

#### Blocked Subcommands

Certain subcommands are blocked in `bn_run` because they cause hangs, are inappropriate for MCP, or require shell-based execution for proper functionality:

| Subcommand | Reason | Alternative |
|------------|--------|-------------|
| `gui` | Launches GUI, inappropriate for MCP | N/A |
| `mcp` | Recursive/meta, causes issues | N/A |
| `orient` | Requires shell PID for agent registration | Use dedicated `binnacle-orient` MCP tool |
| `goodbye` | Requires shell PID for process termination | Use dedicated `binnacle-goodbye` MCP tool |

The MCP server exposes `orient` and `goodbye` as **separate MCP tools** (`binnacle-orient`, `binnacle-goodbye`) which can be blocked via tool configuration in agents.sh. This allows operators to enforce shell-based lifecycle while still permitting `bn_run` for all other commands.

#### Resources (Optional Enhancement)

Keep a minimal set of resources for context injection:

| URI | Description |
|-----|-------------|
| `binnacle://status` | Current project status (task counts, queue state) |
| `binnacle://agents` | Content of AGENTS.md if present |

Resources are read-only and useful for agents to get context without tool calls.

#### Prompts

Reference agents.sh for prompts. Refactor the prompts so they are available via system emit and use that inside the agents.sh.

### Part 2: MCP Config Installation

Extend `bn system init` to offer MCP client configuration installation.

#### Supported Clients

| Client | Config Location | Format |
|--------|-----------------|--------|
| VS Code | `.vscode/mcp.json` (in repo) | JSON |
| GitHub Copilot CLI | `~/.copilot/mcp-config.json` | JSON |

> **Note:** Claude Desktop MCP config support was removed because we cannot validate it in CI.

#### Interactive Flow

When running `bn system init` interactively:

```
$ bn system init

Binnacle System Initialization
==============================

[1/3] Write AGENTS.md section? [Y/n] y
✓ Updated AGENTS.md

[2/3] Write VS Code MCP config? [Y/n] y
✓ Created .vscode/mcp.json

[3/3] Write GitHub Copilot CLI MCP config? [Y/n] n
⊘ Skipped

Done! Run 'bn orient' to verify setup.
```

#### Non-Interactive Flags

```bash
bn system init --write-mcp-vscode     # VS Code only
bn system init --write-mcp-copilot    # GitHub Copilot CLI only
bn system init --write-mcp-all        # All MCP configs (VS Code + Copilot CLI)
bn system init -y                     # Skip prompts, use defaults (no MCP)
bn system init -y --write-mcp-all     # Skip prompts, install all MCP
```

#### Generated Configs

**VS Code** (`.vscode/mcp.json`):

```json
{
  "servers": {
    "binnacle": {
      "type": "stdio",
      "command": "bn",
      "args": ["mcp", "serve"],
      "cwd": "${workspaceFolder}"
    }
  }
}
```

**GitHub Copilot CLI** (`~/.copilot/mcp-config.json`):

```json
{
  "mcpServers": {
    "binnacle": {
      "type": "local",
      "command": "bn",
      "args": ["mcp", "serve"],
      "tools": ["*"]
    }
  }
}
```

#### Config Merging

When updating existing config files:

1. Parse existing JSON
2. Add/update `binnacle` entry under `mcpServers` (or `servers` for VS Code)
3. Preserve all other entries
4. Write back with proper formatting

If the file doesn't exist, create it with just the binnacle config.

## Implementation Details

### Architecture

```
src/mcp/
├── mod.rs           # Simplified MCP server (~200 lines)
├── subprocess.rs    # Subprocess execution with timeout
└── resources.rs     # Optional resource handlers
```

### Subprocess Execution

Key considerations:

- Timeout (default: 30 seconds, configurable)
- Capture stdout/stderr separately
- Return exit code
- Set `BN_MCP_SESSION` environment variable for agent tracking
- Block dangerous subcommands

### State Management

The server maintains minimal state:

- `cwd: Option<PathBuf>` - Working directory (required before `bn_run`)
- `session_id: String` - UUID for MCP session tracking
- `agent_id: Option<String>` - Linked agent ID from shell-based registration (enables tracking attribution)

### Error Handling

| Scenario | Response |
|----------|----------|
| `bn_run` before `binnacle-set_agent` | Error: "Session not initialized. Call binnacle-set_agent first." |
| `binnacle-set_agent` with invalid `agent_id` | Error: "Agent bna-xxxx not found in registry" |
| Blocked subcommand (`orient`/`goodbye`) | Warning: "Use shell tool for 'bn {cmd}' to enable agent termination" |
| Command timeout | Error: "Command timed out after 30 seconds", exit_code: 124 |
| bn binary not found | Error: "bn binary not found", exit_code: 127 |

## Migration Plan

1. **Phase 1**: Implement new simplified MCP server alongside existing
2. **Phase 2**: Test with Claude Desktop, VS Code, Copilot CLI
3. **Phase 3**: Remove old implementation entirely
4. **Phase 4**: Remove Python wrapper from reference

The old `bn mcp manifest` command can be removed - the new server is self-describing via MCP's `tools/list`.

## Success Criteria

- [ ] MCP server code reduced to <300 lines
- [ ] Zero maintenance for new CLI commands (automatic passthrough)
- [ ] No hangs in production use
- [ ] `bn system init` successfully configures VS Code
- [ ] `bn system init` successfully configures GitHub Copilot CLI
- [ ] Existing agent workflows continue to work

## Out of Scope

- Claude Desktop MCP config (removed - cannot validate in CI)
- MCP prompts (removed for simplicity)
- MCP subscriptions/notifications
- Multiple simultaneous working directories
- Remote MCP server (HTTP transport)

## Test Plan

### Unit Tests

- Subprocess execution with timeout
- Blocked subcommand detection
- Config file merging logic

### Integration Tests

- MCP initialize handshake
- `binnacle-set_agent` → `bn_run` workflow
- `binnacle-set_agent` with `agent_id` links session
- Resource reading
- Config file generation for each client

### Manual Testing

- End-to-end with Claude Desktop
- End-to-end with VS Code
- End-to-end with GitHub Copilot CLI

## Related Work

- Python wrapper: `binnacle_wrap/` (reference implementation)
- Current MCP: `src/mcp/mod.rs` (to be replaced)
- System init: `src/commands/system.rs`
- Agent Lifecycle: `prds/PRD_AGENT_LIFECYCLE.md`

---

## Part 3: Agent Lifecycle for MCP (AMENDMENT)

### Problem Statement

The current agent lifecycle design assumes agents call `bn goodbye` directly via shell, allowing binnacle to terminate the agent's parent process. This breaks for MCP-hosted agents because:

1. **MCP server is not the agent** - The MCP server is spawned by the host application (VS Code, Copilot CLI), not by the agent
2. **Multiple agents, one MCP session** - Several agent sessions may share a single MCP server
3. **No process to kill** - The MCP server doesn't know the agent's actual process ID
4. **`BN_MCP_SESSION` is server-scoped** - Currently identifies the MCP server, not individual agents

**Current (broken) flow:**

```
Host App (VS Code) → spawns MCP server (bn mcp serve)
                  → spawns Agent shell (separate process)

Agent calls bn_run(["goodbye"]) → MCP server can't terminate agent
```

### Solution: Shell-Based Registration with MCP Identity Handoff

**Supported agents** (configured via agents.sh) MUST use shell tools for lifecycle commands:

1. **Registration via shell**: Agent runs `bn orient` directly in its shell
   - This registers the **shell's PID** (or grandparent) as the termination target
   - Returns `agent_id: "bna-xxxx"` in JSON output

2. **Identity handoff to MCP**: Agent calls `binnacle-set_agent` with the returned `agent_id`
   - MCP server now knows which registered agent is making calls
   - All subsequent `bn_run` calls are attributed to this agent

3. **Termination via shell**: Agent runs `bn goodbye "reason"` directly in shell
   - Binnacle looks up the registered agent by PID
   - Terminates the agent's shell process (grandparent of bn goodbye)

**New flow:**

```
1. Agent shell starts
2. Agent runs: shell("bn orient --name myagent")
   → Returns: {"agent_id": "bna-1234", "pid": 12345, ...}
3. Agent calls: binnacle-set_agent(path="/repo", agent_id="bna-1234")
   → MCP session linked to agent bna-1234
4. Agent calls: bn_run(["task", "list"])
   → Attributed to agent bna-1234 for tracking
5. Agent runs: shell("bn goodbye 'task complete'")
   → Terminates shell process (PID 12345's grandparent)
```

### Changes to `bn orient`

Add clear instructions in orient output for MCP integration:

```json
{
  "agent_id": "bna-1234",
  "pid": 12345,
  "parent_pid": 12340,
  "mcp_hint": "Call binnacle-set_agent with this agent_id to link MCP calls to this registration",
  ...
}
```

Human-readable:

```
Agent registered: myagent (bna-1234)
PID: 12345 | Parent: 12340

To link MCP calls to this agent, call:
  binnacle-set_agent(path="/repo", agent_id="bna-1234")

...rest of orient output...
```

### Changes to `bn goodbye`

When called via MCP (detected by `BN_MCP_SESSION` env var):

1. **With linked agent_id**: Look up registered agent, return termination instructions

   ```json
   {
     "should_terminate": true,
     "use_shell": true,
     "hint": "Agent must call 'bn goodbye' via shell tool to terminate"
   }
   ```

2. **Without linked agent_id**: Graceful degradation

   ```json
   {
     "should_terminate": true,
     "use_shell": true,
     "warning": "No agent_id linked to MCP session. Agent should call goodbye via shell.",
     "hint": "Use binnacle-set_agent with agent_id from shell-based orient call"
   }
   ```

### Changes to `binnacle-set_agent` Tool

The tool accepts an optional `agent_id` parameter to link MCP calls to a shell-registered agent:

```json
{
  "name": "binnacle-set_agent",
  "inputSchema": {
    "properties": {
      "path": { "type": "string", "description": "Repo path" },
      "agent_id": {
        "type": "string",
        "description": "Agent ID (bna-xxxx) from shell-based bn orient. Links MCP calls to this agent for tracking."
      }
    },
    "required": ["path"]
  }
}
```

When `agent_id` is provided:

1. Validate it exists in `agents.jsonl`
2. Store the mapping in MCP server state
3. Set `BN_AGENT_ID` env var for all subprocess calls

### Blocked Commands via MCP

Lifecycle commands are handled through a two-layer blocking approach:

**Layer 1: `bn_run` blocks `orient` and `goodbye`**

The MCP server's `bn_run` tool refuses to execute `orient` or `goodbye` subcommands:

```rust
const BLOCKED_SUBCOMMANDS: &[&str] = &["gui", "mcp", "orient", "goodbye"];

if let Some(cmd) = args.first() {
    if BLOCKED_SUBCOMMANDS.contains(&cmd.as_str()) {
        return Err(format!(
            "Subcommand '{}' is blocked in bn_run. Use the dedicated MCP tool instead.",
            cmd
        ));
    }
}
```

**Layer 2: Dedicated MCP tools blocked via agents.sh**

The MCP server exposes `binnacle-orient` and `binnacle-goodbye` as separate tools. In agents.sh, these are excluded from allowed tools:

```bash
# agents.sh MCP tool configuration
# Allow bn_run but block lifecycle tools to force shell usage
BLOCKED_MCP_TOOLS="binnacle-orient binnacle-goodbye"
```

This forces supported agents to use shell tools for lifecycle commands while still allowing `bn_run` for all other operations.

**Result:**

- `bn_run(["orient", ...])` → Error: "Subcommand 'orient' is blocked in bn_run"
- `binnacle-orient(...)` → Blocked by agents.sh tool config
- `shell("bn orient ...")` → ✅ Works, registers with real PID

### Generic/Unsupported Agents

For agents not configured via agents.sh (generic MCP clients):

1. **`bn orient` via MCP**: Works but warns about limited functionality

   ```json
   {
     "agent_id": "bna-xxxx",
     "warning": "Agent registered via MCP. Goodbye will not terminate process. Use shell tools for full lifecycle support."
   }
   ```

2. **`bn goodbye` via MCP**: Cleans up registration but does NOT terminate

   ```json
   {
     "terminated": false,
     "should_terminate": true,
     "warning": "Cannot terminate agent via MCP. Agent should self-terminate."
   }
   ```

This maintains backward compatibility while encouraging proper shell-based lifecycle.

### Success Criteria (Amended)

- [ ] `binnacle-set_agent` accepts optional `agent_id` parameter
- [ ] MCP server links calls to registered agent when `agent_id` provided
- [ ] `bn orient` output includes MCP integration hint
- [ ] `bn goodbye` via shell terminates agent (existing behavior preserved)
- [ ] `bn goodbye` via MCP returns appropriate warnings/hints
- [ ] agents.sh enforces shell usage for orient/goodbye
- [ ] Generic MCP clients degrade gracefully

### Test Plan (Amended)

#### Integration Tests

- `binnacle-set_agent` with valid `agent_id` links session
- `binnacle-set_agent` with invalid `agent_id` returns error
- `bn_run(["orient", ...])` returns blocked subcommand error
- `bn_run(["goodbye", ...])` returns blocked subcommand error
- `binnacle-orient` tool works when not blocked by config
- `binnacle-goodbye` tool works when not blocked by config
- Shell-based goodbye terminates correct process

#### Manual Testing

- Full flow: shell orient → MCP set_agent → MCP work → shell goodbye
- Verify `bn_run` blocking works for orient/goodbye
- Verify agents.sh can block `binnacle-orient` and `binnacle-goodbye` tools
- Verify agent termination works with VS Code MCP
- Verify agent termination works with Copilot CLI MCP
