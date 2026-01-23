# PRD: Copilot Workflow Agent Provisioning

**Status:** Draft
**Author:** GitHub Copilot
**Date:** 2026-01-23

## Overview

Extend `bn system emit` and `bn system init` to provision VS Code Copilot workflow prompts (plan → PRD → tasks) as repo-local files. These prompts will be deeply integrated with binnacle commands, enabling a structured AI-assisted development workflow where agents use `bn` to track planning artifacts.

## Motivation

AI agents work best with structured workflows. The current binnacle setup provisions `AGENTS.md` and skill files for context, but lacks opinionated workflow prompts that guide agents through the idea → plan → PRD → tasks → execute pipeline.

By shipping binnacle-native Copilot prompts, users get:

1. **Consistent workflows** across projects using binnacle
2. **Integrated planning** where agents create real `bn` artifacts (ideas, tasks, PRDs)
3. **Guardrails** that keep planning agents from prematurely executing
4. **Handoffs** that chain agents together naturally

## Non-Goals

- **Other IDEs**: Focus on Copilot only. Claude Code and Codex skills files exist but won't get workflow prompts yet.
- **Review agent**: Defer integration—may become a `bn review` command rather than a prompt.
- **MCP integration**: Include tool references as placeholders; actual MCP functionality is out of scope (blocked by existing MCP issues).
- **User-home installation**: Keep prompts repo-local; no `~/.config` provisioning.

## Dependencies

- None. This is additive to existing `bn system emit` / `bn system init` functionality.

---

## Specification

### Agent Prompt Files

Three new prompt files for `.github/prompts/`:

| File | Purpose | Key Behaviors |
|------|---------|---------------|
| `binnacle-plan.prompt.md` | Research and outline multi-step plans | Uses subagents for deep research; outputs concise plan; hands off to PRD agent |
| `binnacle-prd.prompt.md` | Convert approved plans to PRDs | Asks clarifying questions; writes PRD to specified path; hands off to tasks agent |
| `binnacle-tasks.prompt.md` | Convert PRDs to binnacle tasks | Creates milestone + tasks with dependencies; uses `bn task create`, `bn link add` |

One new instructions file for `.github/instructions/`:

| File | Purpose |
|------|---------|
| `binnacle.instructions.md` | Base instructions applied to all prompts; references `bn` commands and workflow |

### Prompt Design Principles

1. **Planning-only**: Each prompt includes stopping rules that prevent premature implementation
2. **bn-native**: Prompts reference actual `bn` commands, not generic task management
3. **Handoff chain**: Each prompt declares handoffs to the next stage via YAML frontmatter
4. **MCP placeholder**: Include `#tool:mcp_binnacle_bn_run` in tools list with a note that MCP setup is required
5. **Subagent-aware**: Plan and PRD agents use `#tool:runSubagent` for deep research

### YAML Frontmatter Structure

```yaml
---
name: Binnacle Plan
description: Research and outline multi-step plans for binnacle-tracked projects
tools: ['search', 'runSubagent', 'fetch', 'mcp_binnacle_bn_run']
handoffs:
  - label: Create PRD
    agent: Binnacle PRD
    prompt: Create a full PRD based on this research and plan.
    send: true
---
```

### Emit Variants

Extend `EmitTemplate` enum:

| Variant | Output |
|---------|--------|
| `Agents` | (existing) AGENTS.md blurb |
| `Skill` | (existing) SKILL.md content |
| `PlanAgent` | binnacle-plan.prompt.md content |
| `PrdAgent` | binnacle-prd.prompt.md content |
| `TasksAgent` | binnacle-tasks.prompt.md content |
| `CopilotInstructions` | binnacle.instructions.md content |

### Init Flag

Add `--write-copilot-prompts` flag to `bn system init`:

```
bn system init --write-copilot-prompts
```

This writes all four files to the repo:

- `.github/prompts/binnacle-plan.prompt.md`
- `.github/prompts/binnacle-prd.prompt.md`
- `.github/prompts/binnacle-tasks.prompt.md`
- `.github/instructions/binnacle.instructions.md`

Creates directories if they don't exist. Overwrites existing files (these are meant to be updated with new bn versions).

---

## Implementation

### Files to Modify

| File | Changes |
|------|---------|
| [src/cli/mod.rs](../src/cli/mod.rs) | Add `PlanAgent`, `PrdAgent`, `TasksAgent`, `CopilotInstructions` to `EmitTemplate`; add `--write-copilot-prompts` flag to `SystemCommands::Init` |
| [src/commands/mod.rs](../src/commands/mod.rs) | Add `pub const PLAN_AGENT_CONTENT`, `PRD_AGENT_CONTENT`, `TASKS_AGENT_CONTENT`, `COPILOT_INSTRUCTIONS_CONTENT`; add `create_copilot_prompt_files()` function |
| [src/main.rs](../src/main.rs) | Handle new emit variants and init flag in command dispatch |

### Prompt Content Templates

Below are the embedded prompt templates. These are inspired by the `_agents/` drafts but rewritten for bn integration.

#### binnacle.instructions.md

```markdown
---
applyTo: '**'
---
# Binnacle Project Instructions

This project uses **binnacle** (`bn`) for task and workflow tracking.

## Quick Reference

- `bn orient` - Get project overview and current state
- `bn ready` - Show tasks ready to work on
- `bn task update <id> --status in_progress` - Start a task
- `bn task close <id> --reason "description"` - Complete a task
- `bn goodbye` - End your session gracefully

## Workflow Stages

This project supports a structured planning workflow:

1. **Plan** (@binnacle-plan) - Research and outline
2. **PRD** (@binnacle-prd) - Detailed specification
3. **Tasks** (@binnacle-tasks) - Create bn tasks
4. **Execute** - Implement with task tracking

Always update task status to keep the graph accurate.
```

#### binnacle-plan.prompt.md

```markdown
---
name: Binnacle Plan
description: Research and outline multi-step plans for binnacle-tracked projects
tools: ['search', 'runSubagent', 'fetch', 'mcp_binnacle_bn_run']
handoffs:
  - label: Create PRD
    agent: Binnacle PRD
    prompt: Create a full PRD based on this research and plan.
    send: true
---
You are a PLANNING AGENT for a binnacle-tracked project.

Your SOLE responsibility is planning. NEVER start implementation.

<stopping_rules>
STOP IMMEDIATELY if you consider:
- Running file editing tools
- Creating or modifying binnacle tasks
- Switching to implementation mode

Plans describe steps for ANOTHER agent to execute later.
</stopping_rules>

<workflow>
## 1. Gather Context

Use #tool:runSubagent to research comprehensively:
- Search codebase for relevant patterns
- Read existing PRDs in `prds/` folder
- Check `bn ready` for related tasks
- Review recent commits if relevant

Stop at 80% confidence.

## 2. Present Plan

Summarize your proposed plan using the format below.
MANDATORY: Pause for user feedback.

## 3. Iterate

Incorporate feedback and repeat until approved.
Once approved, hand off to PRD agent.
</workflow>

<plan_format>
## Plan: {Title (2-10 words)}

{Brief summary: what, how, why. (20-100 words)}

### Steps (3-6)
1. {Action with [file](path) links and `symbol` references}
2. {Next step}
3. {...}

### Considerations (0-3)
1. {Question or tradeoff? Option A / Option B}
2. {...}
</plan_format>

<!-- NOTE: #tool:mcp_binnacle_bn_run requires MCP setup. See bn docs. -->
```

#### binnacle-prd.prompt.md

```markdown
---
name: Binnacle PRD
description: Convert approved plans into detailed PRDs for binnacle-tracked projects
tools: ['search', 'runSubagent', 'fetch', 'edit', 'mcp_binnacle_bn_run']
handoffs:
  - label: Create Tasks
    agent: Binnacle Tasks
    prompt: Split the approved PRD into binnacle tasks for implementation.
    send: true
---
You are a technical program manager creating PRDs for a binnacle-tracked project.

Your SOLE responsibility is documentation. NEVER start implementation.

<stopping_rules>
STOP IMMEDIATELY if you consider:
- Editing files OTHER than the PRD document
- Creating or modifying binnacle tasks
- Switching to implementation mode
</stopping_rules>

<workflow>
## Before Drafting

ASK clarifying questions if:
- Behavior is ambiguous
- Edge cases aren't addressed
- "Done" state is unclear
- Multiple interpretations exist

3 questions now saves 3 revision rounds later.

## 1. Research (if needed)

Use #tool:runSubagent for deep codebase research.

## 2. Summarize Plan

Present a concise plan summary for user approval.
MANDATORY: Pause for feedback before writing PRD.

## 3. Write PRD

Once approved, ask for file path if not specified.
Write PRD to `prds/PRD_<NAME>.md` using the template below.
</workflow>

<prd_template>
# PRD: {Title}

**Status:** Draft
**Author:** {Your name}
**Date:** {YYYY-MM-DD}

## Overview
{1 paragraph summary}

## Motivation
{Why is this needed? What problem does it solve?}

## Non-Goals
- {What is out of scope}

## Dependencies
- {Required features or changes}

---

## Specification
{Detailed description with examples, tables, diagrams as needed}

## Implementation
{Files to modify, code patterns to follow}

## Testing
{How to validate the implementation}

## Open Questions
- {Unresolved decisions}
</prd_template>

<!-- NOTE: #tool:mcp_binnacle_bn_run requires MCP setup. See bn docs. -->
```

#### binnacle-tasks.prompt.md

```markdown
---
name: Binnacle Tasks
description: Convert PRDs into binnacle tasks with dependencies
tools: ['search', 'runSubagent', 'mcp_binnacle_bn_run']
handoffs:
  - label: Start Implementation
    agent: agent
    prompt: >
      Start implementation. Run `bn ready` to see tasks, pick one,
      mark it in_progress, and begin working. Update task status as you go.
    send: true
---
You are a dev lead converting PRDs into binnacle tasks.

Your SOLE responsibility is task creation. NEVER start implementation.

<stopping_rules>
STOP IMMEDIATELY if you consider:
- Editing source code files
- Running tests
- Switching to implementation mode
</stopping_rules>

<workflow>
## 1. Review PRD

Read the PRD carefully. Ask clarifying questions if ambiguous.

## 2. Check Existing Tasks

Use `bn orient` and `bn task list` to understand current state.
Look for related tasks to avoid duplicates or find dependencies.

## 3. Create Tasks

Create a parent milestone, then individual tasks:

```bash
# Create milestone
bn milestone create "PRD: Feature Name" -d "Implements PRD at prds/PRD_NAME.md"

# Create tasks with short names for GUI visibility
bn task create -s "short name" -p 2 -d "Description" "Full task title"

# Link to milestone
bn link add <task-id> <milestone-id> -t child_of

# Set dependencies between tasks
bn link add <task-id> <blocker-id> -t depends_on -r "reason"
```

## 4. Iterate

Add tasks incrementally. Pause for user feedback on structure.
Only mark complete when all tasks are clear and properly linked.
</workflow>

<task_guidelines>

- **Actionable**: Each task = one clear action
- **Specific**: Include enough detail to implement
- **Short names**: Always use `-s` flag (appears in GUI)
- **Dependencies**: Model blockers explicitly with `depends_on`
- **Hierarchy**: Link tasks to milestones with `child_of`
</task_guidelines>

<!-- NOTE: #tool:mcp_binnacle_bn_run requires MCP setup. See bn docs. -->
```

### Function Signature

```rust
/// Create Copilot prompt files in the repository
pub fn create_copilot_prompt_files(repo_path: &Path) -> Result<bool> {
    let prompts_dir = repo_path.join(".github/prompts");
    let instructions_dir = repo_path.join(".github/instructions");

    fs::create_dir_all(&prompts_dir)?;
    fs::create_dir_all(&instructions_dir)?;

    fs::write(prompts_dir.join("binnacle-plan.prompt.md"), PLAN_AGENT_CONTENT)?;
    fs::write(prompts_dir.join("binnacle-prd.prompt.md"), PRD_AGENT_CONTENT)?;
    fs::write(prompts_dir.join("binnacle-tasks.prompt.md"), TASKS_AGENT_CONTENT)?;
    fs::write(instructions_dir.join("binnacle.instructions.md"), COPILOT_INSTRUCTIONS_CONTENT)?;

    Ok(true)
}
```

---

## Testing

### Unit Tests

- Verify each constant parses as valid markdown with YAML frontmatter
- Test `create_copilot_prompt_files()` creates expected directory structure

### Integration Tests

- `bn system emit plan-agent` outputs valid prompt content
- `bn system init --write-copilot-prompts` creates files in correct locations
- Files are overwritten on re-run (idempotent)

### Manual QA

1. Run `bn system init --write-copilot-prompts` in a test repo
2. Open VS Code, verify prompts appear in `@workspace` slash commands
3. Test handoff chain: Plan → PRD → Tasks

---

## Open Questions

1. **Prompt naming**: Should files be `binnacle-*.prompt.md` or just `bn-*.prompt.md` for brevity?
2. **Interactive init**: Should `--write-copilot-prompts` be added to the interactive prompt flow, or remain flag-only?
3. **Version header**: Should prompts include a version comment for tracking which bn version generated them?
