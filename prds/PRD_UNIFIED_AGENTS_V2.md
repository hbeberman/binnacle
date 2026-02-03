# PRD: Unified Agent Definitions v2

**Status:** Implemented
**Author:** Binnacle PRD Agent
**Date:** 2026-02-02
**Related Ideas:** bn-6019 (Unify agent.sh and VSCode MCP agent definitions)
**Supersedes:** bn-74d3 (PRD: Unified Agent Definitions v1)

## Overview

Unify the fragmented agent definition system into a declarative KDL-based model that follows the established container definition pattern. This enables consistent agent behavior across all execution contexts (CLI, containers, VSCode MCP) while allowing progressive customization without modifying the bn binary.

## Motivation

The current agent system has multiple sources of truth:

- **Rust code** (`src/models/prompts.rs`): 5 prompt constants (WORKER, DO, PRD, BUDDY, FREE) + ASK
- **Shell script** (`scripts/bn-agent`): Tool permission arrays (TOOLS_FULL, TOOLS_PRD, TOOLS_BUDDY, TOOLS_QA)
- **GitHub agents** (`.github/agents/`): Old 3-agent model (plan, prd, tasks) that's out of sync
- **EmitTemplate enum** (`src/cli/mod.rs`): ~25 templates for various agent outputs

This fragmentation causes:

1. **Inconsistency** - Tool permissions in bash don't match prompts in Rust
2. **Maintenance burden** - Changes require updating multiple locations
3. **No user customization** - Can't override agent behavior without recompiling
4. **VSCode disconnect** - MCP config generated separately from CLI agents
5. **No native Copilot --agent support** - Can't use copilot --agent binnacle-worker

## Goals

1. **Single source of truth** - Agent definitions in KDL parsed at runtime
2. **Follow container pattern** - Same layered resolution (embedded -> system -> session -> project)
3. **Zero-config default** - Embedded agents work out of the box
4. **Progressive customization** - Users can override at any level
5. **Copilot --agent integration** - Generate proper .md agent files for native Copilot support
6. **Update GitHub agents** - Sync .github/agents/*.md with unified model
7. **VSCode MCP unification** - Generate MCP config from same agent definitions

## Non-Goals

- Changing agent behavior/capabilities (scope is infrastructure only)
- Adding new agent types (keep current 6)
- Container runtime changes (orthogonal to agent definitions)

## Dependencies

- Existing KDL parsing infrastructure (kdl crate, src/config/)
- Container definition pattern (src/container/mod.rs)
- Current bn-agent script and bn system emit commands
- Copilot CLI --agent flag (available in v0.0.398+)

---

## Specification

### Agent Types (Current 6)

| Type | Name | Execution | Lifecycle | Description |
|------|------|-----------|-----------|-------------|
| worker | Auto Worker | container | stateful | Picks from bn ready, works autonomously |
| do | Directed Task | host | stateful | Works on user-specified task |
| prd | PRD Writer | host | stateless | Converts ideas to PRDs (planner) |
| buddy | Quick Entry | host | stateful | Creates bugs/tasks/ideas |
| ask | Interactive Q&A | host | stateless | Read-only codebase exploration |
| free | General Purpose | host | stateful | Full access, user-directed |

**Lifecycle definitions:**

- **Stateful**: Manages binnacle session state, calls bn goodbye on completion
- **Stateless**: Produces artifacts/answers, does not manage session state or call goodbye

### KDL Schema

```kdl
// .binnacle/agents/config.kdl

agent "worker" {
    description "AI worker that picks tasks from bn ready"
    execution "container"  // "container" | "host"
    lifecycle "stateful"   // "stateful" (calls goodbye) | "stateless" (no goodbye)

    tools {
        // Allowed tools (merged with embedded defaults)
        allow "write"
        allow "shell(bn:*)"
        allow "shell(cargo:*)"
        allow "shell(git:*)"
        allow "binnacle"

        // Denied tools (always blocked)
        deny "shell(bn agent terminate:*)"
        deny "binnacle(binnacle-orient)"
        deny "binnacle(binnacle-goodbye)"
    }

    // Optional: override prompt from file
    prompt-file "worker/prompt.md"
}

agent "prd" {
    description "Converts ideas into detailed PRDs"
    execution "host"
    lifecycle "stateless"  // PRD agents don't call goodbye

    tools {
        allow "write"
        allow "shell(bn:*)"
        allow "shell(git:*)"
        allow "binnacle"
    }
}
```

### File Structure

```
.binnacle/agents/
├── config.kdl              # Agent definitions (metadata + tools)
├── worker/
│   └── prompt.md           # Custom prompt (optional, overrides embedded)
├── prd/
│   └── prompt.md           # Custom prompt (optional)
└── ...
```

### Resolution Order

Same as containers - later sources override earlier:

1. **Embedded** (in bn binary) - Default agent definitions from src/agents/embedded.rs
2. **System** (~/.config/binnacle/agents/) - Global user customizations
3. **Session** (~/.local/share/binnacle/<hash>/agents/) - Per-repo user customizations
4. **Project** (.binnacle/agents/) - Repo-specific customizations (committed)

### Rust Implementation

#### New Module: src/agents/

```rust
// src/agents/mod.rs
pub mod definitions;  // AgentDefinition struct, parsing
pub mod embedded;     // Embedded default prompts and tool permissions
pub mod resolver;     // Layered resolution logic

// AgentDefinition parsed from KDL
pub struct AgentDefinition {
    pub name: String,
    pub description: String,
    pub execution: ExecutionMode,  // Host | Container
    pub lifecycle: LifecycleMode,  // Stateful | Stateless
    pub tools: ToolPermissions,
    pub prompt: String,            // Resolved prompt content
}

pub struct ToolPermissions {
    pub allow: Vec<String>,
    pub deny: Vec<String>,
}

pub enum ExecutionMode { Host, Container }
pub enum LifecycleMode { Stateful, Stateless }
```

#### Embedded Defaults

```rust
// src/agents/embedded.rs
// Current prompts.rs content moves here with additional metadata

pub const EMBEDDED_AGENTS: &[EmbeddedAgent] = &[
    EmbeddedAgent {
        name: "worker",
        description: "AI worker that picks tasks from bn ready",
        execution: ExecutionMode::Container,
        lifecycle: LifecycleMode::Stateful,
        tools_allow: &["write", "shell(bn:*)", "shell(cargo:*)", ...],
        tools_deny: &["shell(bn agent terminate:*)"],
        prompt: include_str!("embedded/worker.md"),
    },
    // ... other agents
];
```

### CLI Changes

#### bn config agents list

```bash
$ bn config agents list -H
6 agent definitions:

  worker   [container, stateful]   AI worker that picks tasks from bn ready
           Source: embedded

  prd      [host, stateless]       Converts ideas into detailed PRDs
           Source: .binnacle/agents/config.kdl (overrides embedded)

  buddy    [host, stateful]        Creates bugs/tasks/ideas quickly
           Source: embedded
  ...
```

#### bn config agents show <name>

```bash
$ bn config agents show worker -H
Agent: worker
Description: AI worker that picks tasks from bn ready
Execution: container
Lifecycle: stateful (calls bn goodbye)
Source: embedded

Tools Allowed:
  write
  shell(bn:*)
  shell(cargo:*)
  ...

Tools Denied:
  shell(bn agent terminate:*)
  binnacle(binnacle-orient)
  binnacle(binnacle-goodbye)

Prompt (first 500 chars):
  Run bn orient --type worker to get oriented with the project...
```

#### bn config agents emit <name>

Emits the raw prompt content for use with -i fallback:

```bash
$ bn config agents emit worker
Run bn orient --type worker to get oriented with the project...
```

### Copilot --agent Integration

#### Agent File Generation

Copilot searches for agent files in:

1. .github/agents/ (repo-level)
2. ~/.copilot/agents/ (user-level)

We generate files to both locations:

**bn session init --write-copilot-prompts** -> .github/agents/binnacle-*.agent.md
**bn system host-init --write-copilot-agents** (new flag) -> ~/.copilot/agents/binnacle-*.md

#### Generated Agent File Format

```markdown
---
name: Binnacle Worker
description: AI worker that picks tasks from bn ready
tools: ['binnacle/*', 'edit', 'execute', 'agent', 'search', 'read']
---
Run bn orient --type worker to get oriented with the project...

[Full prompt content from KDL-resolved definition]
```

**Note:** VSCode-specific fields like argument-hint and handoffs are omitted until the Copilot CLI bug (versions > 0.0.397) is fixed. These can be added later.

### bn-agent Script Updates

The scripts/bn-agent script uses `-i` mode consistently for prompt injection across all execution contexts (host and container). This provides predictable behavior regardless of whether agent files exist.

The script:

1. **Read tool permissions from bn config agents show --json**
2. **Use bn config agents emit for prompt content**
3. **Always use -i mode** for consistency

```bash
#!/usr/bin/env bash
# bn-agent - Unified agent launcher

AGENT_TYPE="$1"

# Get resolved agent definition
AGENT_DEF=$(bn config agents show "$AGENT_TYPE" --json)
TOOLS_ALLOW=$(echo "$AGENT_DEF" | jq -r '.tools.allow[]')
TOOLS_DENY=$(echo "$AGENT_DEF" | jq -r '.tools.deny[]')

# Build tool permission arrays
declare -a ALLOW_ARGS DENY_ARGS
for tool in $TOOLS_ALLOW; do
    ALLOW_ARGS+=(--allow-tool "$tool")
done
for tool in $TOOLS_DENY; do
    DENY_ARGS+=(--deny-tool "$tool")
done

# Always use -i with prompt injection for consistency across host/container
PROMPT=$(bn config agents emit "$AGENT_TYPE")
exec copilot "${ALLOW_ARGS[@]}" "${DENY_ARGS[@]}" -i "$PROMPT"
```

**Note:** Agent files in `.github/agents/` are useful for VS Code `@binnacle-do` invocations, but bn-agent uses `-i` mode for consistency.

### Container Mode

In container mode, agent files are always available (baked into image or mounted), so --agent always works. The container entrypoint uses:

```bash
copilot --agent "binnacle-$AGENT_TYPE" "${TOOL_ARGS[@]}"
```

### GitHub Agents Update

bn session init --write-copilot-prompts generates interactive chat agents from KDL definitions. Only host-mode interactive agents are generated as `.github/agents/` files since these are designed for VS Code chat invocation.

**Generated (4 files):**

- .github/agents/binnacle-do.agent.md
- .github/agents/binnacle-prd.agent.md
- .github/agents/binnacle-buddy.agent.md
- .github/agents/binnacle-ask.agent.md

**Not generated as GitHub agents:**

- `worker` - Container-mode agent, invoked via bn-agent script
- `free` - General purpose agent, invoked via bn-agent script

**Removed (obsolete):**

- .github/agents/binnacle-plan.agent.md
- .github/agents/binnacle-tasks.agent.md

### VSCode MCP Unification

bn system emit mcp-vscode generates MCP config that references the same agent definitions, ensuring VSCode agents have consistent tool permissions with CLI agents.

---

## Implementation Plan

### Phase 1: Core Infrastructure

1. Create src/agents/ module with types and embedded defaults
2. Move prompts from prompts.rs to src/agents/embedded/
3. Implement KDL parsing for agent definitions
4. Implement layered resolution (embedded -> system -> session -> project)

### Phase 2: CLI Integration

1. Add bn config agents list, show, emit commands
2. Update bn system emit to delegate to new agent resolver

### Phase 3: bn-agent Script Update

1. Update bn-agent to use bn config agents show --json
2. Use -i mode consistently for prompt injection
3. Remove hardcoded TOOLS_* arrays
4. Test all 6 agent types

### Phase 4: Copilot Agent Files

1. Add agent file generation to bn session init --write-copilot-prompts
2. Add bn system host-init --write-copilot-agents flag
3. Update container image build to include agent files

### Phase 5: GitHub Agents & MCP

1. Remove old plan/tasks agents from .github/agents/
2. Update bn system emit mcp-vscode for consistency

### Phase 6: Documentation & Cleanup

1. Update AGENTS.md with unified model
2. Update getting-started.md
3. Remove deprecated code from prompts.rs
4. Add tests for agent resolution

---

## Testing

### Unit Tests

- KDL parsing for agent definitions
- Layered resolution (embedded, system, session, project)
- Tool permission merging
- Prompt file loading
- Agent file generation format

### Integration Tests

- bn config agents list output format
- bn config agents show for each agent type
- bn config agents emit produces valid prompts
- bn session init --write-copilot-prompts generates correct files (4 interactive agents)
- bn-agent script works with -i mode for all agent types

### Manual Testing

- Run each agent type via bn-agent
- Run agent via copilot --agent binnacle-worker directly
- Verify custom prompt override works
- Verify tool permission customization works

---

## Open Questions

1. **Prompt inheritance** - Can agent prompts inherit from a base template?
2. **Tool permission inheritance** - Should tools blocks merge or replace parent definitions?
3. **Custom agent types** - Should users be able to define entirely new agent types via KDL?
4. **VSCode fields** - When Copilot fixes the bug, should we auto-add argument-hint and handoffs fields?
