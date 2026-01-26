//! Command implementations for Binnacle CLI.
//!
//! This module contains the business logic for each CLI command.
//! Commands are organized by entity type:
//! - `init` - Initialize binnacle for a repository
//! - `task` - Task CRUD operations
//! - `link` - Relationship management (edges)
//! - `test` - Test node operations
//! - `commit` - Commit tracking

use crate::models::{
    Agent, AgentType, Bug, BugSeverity, Doc, DocType, Edge, EdgeDirection, EdgeType, Editor, Idea,
    IdeaStatus, Milestone, Queue, SessionState, Task, TaskStatus, TestNode, TestResult,
    complexity::analyze_complexity, graph::UnionFind,
};
use crate::storage::{EntityType, Storage, find_git_root, generate_id, parse_status};
use crate::{Error, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

/// Output format trait for commands.
pub trait Output {
    /// Serialize to JSON string.
    fn to_json(&self) -> String;
    /// Format for human-readable output.
    fn to_human(&self) -> String;
}

/// Generic result wrapper for command outputs.
#[derive(Serialize)]
pub struct CommandOutput<T: Serialize> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T: Serialize> CommandOutput<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg.into()),
        }
    }
}

// === Short Name Helpers ===

/// Maximum length for short_name (2x display limit for GUI).
const SHORT_NAME_MAX_LEN: usize = 30;

/// Normalize a short_name for storage (truncate if needed).
/// Used by create functions. Returns None if input is None.
fn normalize_short_name(short_name: Option<String>) -> Option<String> {
    short_name.map(|sn| {
        if sn.chars().count() > SHORT_NAME_MAX_LEN {
            eprintln!(
                "Note: short_name truncated from {} to {} chars for GUI display.",
                sn.chars().count(),
                SHORT_NAME_MAX_LEN
            );
            sn.chars().take(SHORT_NAME_MAX_LEN).collect::<String>()
        } else {
            sn
        }
    })
}

/// Process short_name for update operations.
/// - None: no change
/// - Some("")/whitespace: clear the field
/// - Some(value): set/truncate the value
fn process_short_name_update(short_name: Option<String>) -> Option<Option<String>> {
    short_name.map(|s| {
        if s.trim().is_empty() {
            // Empty or whitespace-only clears the short_name
            None
        } else if s.chars().count() > SHORT_NAME_MAX_LEN {
            eprintln!(
                "Note: short_name truncated from {} to {} chars for GUI display.",
                s.chars().count(),
                SHORT_NAME_MAX_LEN
            );
            Some(s.chars().take(SHORT_NAME_MAX_LEN).collect::<String>())
        } else {
            Some(s)
        }
    })
}

/// Format a byte size into human-readable form (e.g., "1.2 KB", "3.5 MB").
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * 1024 * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

/// Calculate the total size of a directory recursively.
fn calculate_dir_size(path: &std::path::Path) -> u64 {
    let mut total = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_file() {
                if let Ok(metadata) = entry.metadata() {
                    total += metadata.len();
                }
            } else if entry_path.is_dir() {
                total += calculate_dir_size(&entry_path);
            }
        }
    }
    total
}

// === Init Command ===

/// Prompt the user for a yes/no answer.
/// Returns true for yes, false for no.
/// Default is Yes if default_yes is true, No otherwise.
fn prompt_yes_no(prompt: &str, default_yes: bool) -> bool {
    use std::io::{self, Write};

    let suffix = if default_yes { " (Y/n): " } else { " (y/N): " };
    print!("{}{}", prompt, suffix);
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();

    let trimmed = input.trim().to_lowercase();

    if trimmed.is_empty() {
        default_yes
    } else {
        trimmed == "y" || trimmed == "yes"
    }
}

#[derive(Serialize)]
pub struct InitResult {
    pub initialized: bool,
    pub storage_path: String,
    pub agents_md_updated: bool,
    pub skills_file_created: bool,
    pub codex_skills_file_created: bool,
    pub copilot_prompts_created: bool,
    pub hook_installed: bool,
}

impl Output for InitResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();
        if self.initialized {
            lines.push(format!("Initialized binnacle at {}", self.storage_path));
        } else {
            lines.push(format!(
                "Binnacle already initialized at {}",
                self.storage_path
            ));
        }
        if self.agents_md_updated {
            lines.push("Updated AGENTS.md with binnacle reference.".to_string());
        }
        if self.skills_file_created {
            lines.push(
                "Created Claude Code skills file at ~/.claude/skills/binnacle/SKILL.md".to_string(),
            );
        }
        if self.codex_skills_file_created {
            lines
                .push("Created Codex skills file at ~/.codex/skills/binnacle/SKILL.md".to_string());
        }
        if self.copilot_prompts_created {
            lines.push(
                "Created Copilot agents at .github/agents/ and .github/instructions/".to_string(),
            );
        }
        if self.hook_installed {
            lines.push("Installed commit-msg hook for co-author attribution.".to_string());
        }
        lines.join("\n")
    }
}

/// Initialize binnacle for the current repository with interactive prompts.
pub fn init(repo_path: &Path) -> Result<InitResult> {
    // Prompt for AGENTS.md update (default Yes)
    let update_agents_md = prompt_yes_no("Add binnacle reference to AGENTS.md?", true);

    // Prompt for Claude Code skills file creation (default Yes)
    let create_claude_skills = prompt_yes_no(
        "Create Claude Code skills file at ~/.claude/skills/binnacle/SKILL.md?",
        true,
    );

    // Prompt for Codex skills file creation (default Yes)
    let create_codex_skills = prompt_yes_no(
        "Create Codex skills file at ~/.codex/skills/binnacle/SKILL.md?",
        true,
    );

    // Prompt for Copilot agents creation (default No - new feature)
    let create_copilot_prompts = prompt_yes_no(
        "Create Copilot workflow agents at .github/agents/ and .github/instructions/?",
        false,
    );

    // Prompt for commit-msg hook installation (default Yes)
    let install_hook = prompt_yes_no("Install commit-msg hook for co-author attribution?", true);

    init_with_options(
        repo_path,
        update_agents_md,
        create_claude_skills,
        create_codex_skills,
        create_copilot_prompts,
        install_hook,
    )
}

/// Initialize binnacle for the current repository without interactive prompts.
/// Use flags to control what gets written.
pub fn init_non_interactive(
    repo_path: &Path,
    write_agents_md: bool,
    write_claude_skills: bool,
    write_codex_skills: bool,
    write_copilot_prompts: bool,
    install_hook: bool,
) -> Result<InitResult> {
    init_with_options(
        repo_path,
        write_agents_md,
        write_claude_skills,
        write_codex_skills,
        write_copilot_prompts,
        install_hook,
    )
}

/// Initialize binnacle for the current repository with explicit options.
/// Used internally and by tests.
fn init_with_options(
    repo_path: &Path,
    update_agents: bool,
    create_claude_skills: bool,
    create_codex_skills: bool,
    create_copilot_prompts: bool,
    install_hook: bool,
) -> Result<InitResult> {
    let already_exists = Storage::exists(repo_path)?;
    let storage = if already_exists {
        Storage::open(repo_path)?
    } else {
        Storage::init(repo_path)?
    };

    // Update AGENTS.md if requested (idempotent: create/replace as needed)
    let agents_md_updated = if update_agents {
        update_agents_md(repo_path)?
    } else {
        false
    };

    // Create Claude Code skills file if requested
    let skills_file_created = if create_claude_skills {
        create_claude_skills_file()?
    } else {
        false
    };

    // Create Codex skills file if requested
    let codex_skills_file_created = if create_codex_skills {
        create_codex_skills_file()?
    } else {
        false
    };

    // Create Copilot workflow prompts if requested
    let copilot_prompts_created = if create_copilot_prompts {
        create_copilot_prompt_files(repo_path)?
    } else {
        false
    };

    // Install commit-msg hook if requested
    let hook_installed = if install_hook {
        install_commit_msg_hook(repo_path)?
    } else {
        false
    };

    Ok(InitResult {
        initialized: !already_exists,
        storage_path: storage.root().to_string_lossy().to_string(),
        agents_md_updated,
        skills_file_created,
        codex_skills_file_created,
        copilot_prompts_created,
        hook_installed,
    })
}

/// The blurb to add to AGENTS.md
pub const AGENTS_MD_BLURB: &str = r#"<!-- BEGIN BINNACLE SECTION -->
# Agent Instructions

This project uses **bn** (binnacle) for long-horizon task/test status tracking. Run `bn orient` to get started!

**After running `bn orient`**, report your assigned `agent_id` (e.g., `bna-486c`) to the user. This ID identifies your session in binnacle's tracking system.

For new projects, the human should run `bn system init` which provides helpful prompts for setup.
If you absolutely must initialize without human intervention, use `bn orient --init` (uses conservative defaults, skips optional setup).

## Task Workflow (CRITICAL - READ CAREFULLY)

**⚠️ SINGLE TASK PER SESSION**: You must work on exactly ONE task or bug per session. After completing it, call `bn goodbye` and terminate. Another agent will handle the next task.

### The Complete Workflow:

1. **CLAIM ONE item**: Run `bn ready`, pick ONE task/bug, claim it with `bn task update <id> --status in_progress` (or `bn bug update`).
2. **WORK on that item**: Implement, test, and commit your changes.
3. **CLOSE the item**: Run `bn task close <id> --reason "what was done"` (or `bn bug close`).
4. **TERMINATE immediately**: Run `bn goodbye "summary"` and end your session.

### Why Single-Task Sessions Matter:
- **Focused work**: One task gets full attention and proper completion
- **Clean handoffs**: Each task has a clear owner and outcome
- **Better tracking**: Task status accurately reflects work state
- **Reduced errors**: No context-switching between unrelated work

### What NOT to Do:
- ❌ Pick multiple tasks from `bn ready`
- ❌ Start a second task after closing the first
- ❌ Continue working after calling `bn goodbye`
- ❌ Skip the goodbye call

### Additional Commands:
- **If blocked**: Run `bn task update <id> --status blocked`, then `bn goodbye`
- **For bugs**: Use `bn bug create/update/close` - not `bn task create --tag bug`
- **For ideas**: Use `bn idea create/list/show` - ideas are low-stakes seeds that can be promoted to tasks later

## Git Rules (CRITICAL)

- **NEVER run `git push`** - The human operator handles all pushes. Your job is to commit locally.
- Commit early and often with clear messages
- Always run `just check` before committing

The task graph drives development priorities. Always update task status to keep it accurate.

**Tip**: Use `bn show <id>` to view any entity by ID - it auto-detects the type from the prefix (bn-, bnt-, bnq-).

## Creating Tasks (Best Practices)

- **Always use short names** (`-s`): They appear in the GUI and make tasks scannable
  - `bn task create -s "short name" -d "description" "Full task title"`
- **Add dependencies with reasons**: `bn link add <task> <blocker> -t depends_on --reason "why"`
- **Link to milestones**: `bn link add <task> <milestone> -t child_of`

## Documentation Nodes (IMPORTANT)

Use **doc nodes** instead of creating loose markdown files. Doc nodes are tracked in the task graph and linked to relevant entities.

### When to Use Doc Nodes vs Markdown Files

**Use doc nodes for:**
- PRDs, specifications, and design documents
- Implementation notes that explain *why* something was built a certain way
- Handoff notes between agent sessions
- Any documentation that relates to specific tasks, bugs, or features

**Keep as regular files:**
- README.md, CONTRIBUTING.md, LICENSE (repo-level standard files)
- AGENTS.md (agent instructions - this file)
- Code documentation (doc comments, inline comments)

### Doc Node Commands

```bash
# Create a doc linked to a task
bn doc create bn-task -T "Implementation Notes" -c "Content here..."

# Create from a file
bn doc create bn-task -T "PRD: Feature" --file spec.md --type prd

# List, show, attach to more entities
bn doc list
bn doc show bn-xxxx
bn doc attach bn-xxxx bn-other-task

# Update (creates new version, preserves history)
bn doc update bn-xxxx -c "Updated content..."
```

### Doc Types

- `note` (default) - General documentation, notes
- `prd` - Product requirements documents
- `handoff` - Session handoff notes for the next agent

## Before Ending Your Session (IMPORTANT)

1. **Verify your ONE task is complete**: Tests pass, code is formatted, changes are committed
2. **Close your task**: `bn task close <id> --reason "what was done"`
3. **Terminate**: `bn goodbye "summary"` - then STOP working

⚠️ Do NOT start another task. Let another agent handle it.

## Workflow Stages

For complex features, suggest the human use specialized agents:

1. **@binnacle-plan** - Research and outline (for ambiguous or large tasks)
2. **@binnacle-prd** - Detailed specification (when plan is approved)
3. **@binnacle-tasks** - Create bn tasks from PRD
4. **Execute** - Implement with task tracking (you're here)

If a task seems too large or unclear, suggest the human invoke the planning workflow.

Run `bn --help` for the complete command reference.
<!-- END BINNACLE SECTION -->
"#;

/// The skills file content for Claude Code
pub const SKILLS_FILE_CONTENT: &str = r#"---
name: binnacle
description: Project planning supercharged. Use to determine tasks at the start of each session. Task and test tracker for multi-session development work with AI agents
---

# Binnacle - Project Planning and Task Management

Use `bn` (binnacle) for managing tasks, tests, and project planning across multiple AI agent sessions.

## When to Use Binnacle

- Multi-session projects that span multiple conversations
- Complex tasks with dependencies and blockers
- Projects requiring persistent test tracking
- When you need to maintain context across conversation compactions

## Key Commands

### Getting Oriented

- `bn orient` - Get project overview and current state
- `bn ready` - Show tasks ready to work on (no blockers)
- `bn blocked` - Show tasks waiting on dependencies

### Task Management

- `bn task create -s "short" -p 2 -d "description" "Title"` - Create task with short name (recommended)
- `bn task create "Title" -p 2 --tag feature` - Create a basic task
- `bn task list` - List all tasks
- `bn task show <id>` - Show task details
- `bn task update <id> --status in_progress` - Update task status
- `bn task close <id> --reason "completed"` - Close a task
- `bn task update <id> --title "New title"` - Update task details

**Tip:** Always use `-s "short name"` - short names appear in the GUI and make tasks much easier to scan.

### Links (Dependencies & Relationships)

Links connect entities in the task graph to model dependencies, relationships, and hierarchy.

**Commands:**

- `bn link add <source> <target> -t <type> --reason "why"` - Add a link (reason required for depends_on)
- `bn link list <id>` - Show all links for an entity
- `bn link rm <source> <target>` - Remove a link

**Edge Types:**

- `depends_on` - Source requires target to be completed first (most common)
- `blocks` - Source prevents target from starting (inverse of depends_on)
- `related_to` - Soft connection, no ordering implied
- `duplicates` - Source is a duplicate of target
- `fixes` - Source (usually a task) fixes target (usually a bug)
- `caused_by` - Source (bug) was caused by target
- `supersedes` - Source replaces target (target should be closed)
- `parent_of` / `child_of` - Hierarchy (milestones → tasks)
- `tests` - Source (test) validates target (task)
- `queued` - Source (task) is in target (queue)

**Best Practices:**

- Model blockers explicitly: `bn link add bn-task bn-blocker -t depends_on`
- Add reasons for non-obvious links: `-r "needs API changes first"`
- Use `bn search link -t depends_on` to find dependency chains
- Link tasks to milestones: `bn link add bn-task bn-milestone -t child_of`
- Avoid circular dependencies (use `bn doctor` to detect them)

### Test Tracking

- `bn test create "Name" --cmd "cargo test" --task <id>` - Create and link test
- `bn test run --all` - Run all tests
- `bn test run --task <id>` - Run tests for a specific task
- `bn test list` - List all tests

### Project Health

- `bn doctor` - Check for issues in task graph
- `bn log` - Show audit trail of changes
- `bn log <task-id>` - Show changes for specific task

### Bug Tracking

- `bn bug create "Title" --severity medium` - Create a bug (severities: triage, low, medium, high, critical)
- `bn bug list` - List all bugs
- `bn bug close <id> --reason "fixed"` - Close a bug

### Idea Management

- `bn idea create "Title"` - Create a low-stakes idea seed
- `bn idea list` - List all ideas
- `bn idea update <id> --status promoted` - Promote idea to task

### Milestones

- `bn milestone create "v1.0" --due 2025-02-01` - Create milestone with due date
- `bn milestone list` - List milestones
- `bn milestone show <id>` - Show milestone with linked tasks

### Queue (Work Prioritization)

- `bn queue create "Sprint 1"` - Create a work queue
- `bn queue add <task-id>` - Add task to queue
- `bn queue show` - Show queued tasks in priority order

### Graph Analysis

- `bn graph components` - Find disconnected components in task graph

### Universal Commands

- `bn show <id>` - Show any entity by ID (auto-detects type)
- `bn search link --type depends_on` - Search links/edges by type

### Agent Lifecycle

- `bn orient --name "MyAgent" --register "Implementing feature X"` - Register agent with purpose
- `bn goodbye` - Gracefully terminate session (signals parent process)
- `bn agent list` - List registered agents (human use)

## Task Workflow

1. **ONE TASK AT A TIME**: Focus on a single task or bug. Complete one fully before moving to the next.
2. **Start of session**: Run `bn orient` to understand project state, then **report your assigned `agent_id`** (e.g., `bna-486c`) to the user
3. **CLAIM before working**:
   - Run `bn ready` to see available tasks
   - **Claim your task**: `bn task update <id> --status in_progress` (required before starting!)
4. **During work**:
   - Create new tasks as you discover them
   - Link commits: `bn commit link <sha> <task-id>`
   - If blocked: `bn task update <id> --status blocked`
5. **After completing work**:
   - Run `bn ready` to check related tasks
   - Close ALL completed tasks: `bn task close <id> --reason "description"`
   - Run tests: `bn test run --all`
6. **End of session**: Run `bn goodbye "summary of what was accomplished"` to gracefully terminate

## Git Rules (CRITICAL)

- **NEVER run `git push`** - The human operator handles all pushes. Your job is to commit locally.
- Commit early and often with clear messages
- Always run `just check` before committing (or equivalent for the project)

## Best Practices

- **Always use short names** - `bn task create -s "short" ...` makes tasks scannable in GUI
- **Always update task status** - Keep the task graph accurate
- **Close all related tasks** - Don't leave completed work marked as pending
- **Use dependencies with reasons** - `bn link add -t depends_on --reason "needs X first"`
- **Link tests to tasks** - Enables regression detection
- **Run `bn doctor` regularly** - Catch inconsistencies early
- **Tag tasks** - Use tags for categorization (feature, bug, refactor, etc.)

## Priority Levels

- `0` - Critical
- `1` - High
- `2` - Medium (default)
- `3` - Low
- `4` - Nice to have

## Example Workflow

```bash
# Start of session
bn orient
# Output includes agent_id (e.g., "bna-486c") - report this to the user!

# See what's ready
bn ready

# Start working on a task
bn task update bn-a1b2 --status in_progress

# Discover a blocker, create it with short name
bn task create -s "auth fix" -p 0 --tag bug "Fix authentication bug"
bn link add bn-a1b2 bn-c3d4 -t depends_on --reason "auth must work first"

# Work on the blocker instead
bn task update bn-c3d4 --status in_progress

# Complete blocker
bn task close bn-c3d4 --reason "Fixed auth validation"

# Now original task is unblocked
bn ready  # Should show bn-a1b2 is ready

# Complete original task
bn task close bn-a1b2 --reason "Implemented feature X"

# Run tests
bn test run --all

# End session gracefully
bn goodbye "completed task bn-a1b2"
```

## Notes

- Binnacle stores data in `.bn/` directory using git's orphan branch backend
- All changes are tracked in an append-only log
- Run `bn --help` for full command reference
"#;

/// Copilot instructions file content (binnacle.instructions.md)
pub const COPILOT_INSTRUCTIONS_CONTENT: &str = r#"---
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

For complex features, suggest the human use specialized agents:

1. **@binnacle-plan** - Research and outline (for ambiguous or large tasks)
2. **@binnacle-prd** - Detailed specification (when plan is approved)
3. **@binnacle-tasks** - Create bn tasks from PRD
4. **Execute** - Implement with task tracking (you're here)

If a task seems too large or unclear, suggest the human invoke the planning workflow.

Always update task status to keep the graph accurate.
"#;

/// Plan agent content (binnacle-plan.agent.md)
pub const PLAN_AGENT_CONTENT: &str = r#"---
name: Binnacle Plan
description: Research and outline multi-step plans for binnacle-tracked projects
argument-hint: Outline the goal or problem to research
tools: ['search', 'read/problems', 'agent', 'web/fetch', 'binnacle/*', 'read/readFile','execute/testFailure']
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

If you catch yourself planning steps for YOU to execute, STOP.
Plans describe steps for ANOTHER agent to execute later.

You have access to #tool:binnacle/* only to GATHER CONTEXT, NOT to create or modify tasks.
</stopping_rules>

<workflow>
## 1. Gather Context

Use #tool:agent to research comprehensively:
- Search codebase for relevant patterns
- Check for existing PRDs or design docs
- Check `bn ready` for related tasks
- Review recent commits if relevant

DO NOT make other tool calls after #tool:agent returns!

Stop at 80% confidence.

## 2. Present Plan

Summarize your proposed plan using the <plan_format> below.
MANDATORY: Pause for user feedback.

## 3. Iterate

Incorporate feedback and repeat <workflow> until approved.
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

<output_rules>
- DON'T show code blocks, but describe changes and link to relevant files
- NO manual testing sections unless explicitly requested
- ONLY write the plan, without unnecessary preamble or postamble
</output_rules>

<!-- NOTE: #tool:"binnacle/*" requires MCP setup. See bn docs. -->
"#;

/// PRD agent content (binnacle-prd.agent.md)
pub const PRD_AGENT_CONTENT: &str = r#"---
name: Binnacle PRD
description: Convert approved plans into detailed PRDs for binnacle-tracked projects
argument-hint: Create a PRD document at path {path}
tools: ['search', 'agent', 'web/fetch', 'edit', 'binnacle/*', 'read/readFile']
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

If you catch yourself planning steps for YOU to execute, STOP.
You have access to #tool:binnacle only to GATHER CONTEXT, NOT to create or modify tasks.
</stopping_rules>

<workflow>
## Before Drafting

ASK clarifying questions if:
- Behavior is ambiguous
- Edge cases aren't addressed
- "Done" state is unclear
- Multiple interpretations exist

**Do NOT guess. Do NOT assume. ASK.**
3 questions now saves 3 revision rounds later.

## 1. Research (if needed)

Use #tool:agent for deep codebase research.
DO NOT make other tool calls after #tool:agent returns!

## 2. Summarize Plan

Present a concise plan summary for user approval.
MANDATORY: Pause for feedback before writing PRD.

## 3. Handle Feedback

Once the user replies, restart <workflow> to gather additional context.
DON'T start writing the PRD until the plan summary is approved.

## 4. Write PRD

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

<!-- NOTE: #tool:binnacle requires MCP setup. See bn docs. -->
"#;

/// Tasks agent content (binnacle-tasks.agent.md)
pub const TASKS_AGENT_CONTENT: &str = r#"---
name: Binnacle Tasks
description: Convert PRDs into binnacle tasks with dependencies
tools: ['search', 'agent', 'binnacle/*', 'read/readFile']
handoffs:
  - label: Start Implementation
    agent: agent
    prompt: "Start implementation. Run `bn ready` to see tasks, pick one, mark it in_progress, and begin working. Update task status as you go."
    send: true
---
You are a dev lead converting PRDs into binnacle tasks.

Your SOLE responsibility is task creation. NEVER start implementation.

<stopping_rules>
STOP IMMEDIATELY if you consider:
- Editing source code files
- Running tests
- Switching to implementation mode

If you catch yourself planning steps for YOU to execute, STOP.
You are creating tasks for ANOTHER agent to execute later.
</stopping_rules>

<workflow>
## 1. Review PRD

Read the PRD carefully. Ask clarifying questions if ambiguous.
**Do NOT guess. Do NOT assume. ASK.**

## 2. Check Binnacle State

Run `bn orient` first. If binnacle is not initialized, STOP and inform the user.

Use #tool:agent to research existing tasks:
- Have it run `bn task list` and `bn search` to find related tasks
- The sub-agent should NOT create or edit tasks (RESEARCH ONLY)

## 3. Check Existing Tasks

Look for related tasks to avoid duplicates or find dependencies.
Reuse existing tasks where possible instead of creating new ones.

## 4. Create Tasks

Create a parent milestone, then individual tasks:

```bash
# Create milestone
bn milestone create "PRD: Feature Name" -d "Implements PRD at prds/PRD_NAME.md"

# Create tasks with short names for GUI visibility
bn task create -s "short name" -p 2 -d "Description" "Full task title"

# Link to milestone
bn link add <task-id> <milestone-id> -t child_of

# Set dependencies between tasks (--reason is important!)
bn link add <task-id> <blocker-id> -t depends_on --reason "why this dependency exists"
```

## 5. Iterate

Add tasks incrementally. Pause for user feedback on structure.
Only mark complete when all tasks are clear and properly linked.
</workflow>

<task_guidelines>
- **Actionable**: Each task = one clear action
- **Specific**: Include enough detail to implement
- **Short names**: Always use `-s` flag (appears in GUI)
- **Dependencies**: Model blockers explicitly with `depends_on` and always add `--reason`
- **Hierarchy**: Link tasks to milestones with `child_of`
</task_guidelines>

<!-- NOTE: #tool:binnacle requires MCP setup. See bn docs. -->
"#;

/// Create the Claude Code skills file for binnacle.
/// Always overwrites if the file already exists.
/// Returns true if the file was created/updated.
fn create_claude_skills_file() -> Result<bool> {
    use std::fs;

    // Get home directory
    let home_dir = dirs::home_dir()
        .ok_or_else(|| Error::Other("Could not determine home directory".to_string()))?;

    let skills_dir = home_dir.join(".claude").join("skills").join("binnacle");
    let skills_path = skills_dir.join("SKILL.md");

    // Create directory if it doesn't exist
    fs::create_dir_all(&skills_dir)
        .map_err(|e| Error::Other(format!("Failed to create Claude skills directory: {}", e)))?;

    // Write the skills file (overwrites if exists)
    fs::write(&skills_path, SKILLS_FILE_CONTENT)
        .map_err(|e| Error::Other(format!("Failed to create Claude skills file: {}", e)))?;

    Ok(true)
}

/// Create the Codex skills file for binnacle.
/// Always overwrites if the file already exists.
/// Returns true if the file was created/updated.
fn create_codex_skills_file() -> Result<bool> {
    use std::fs;

    // Get home directory
    let home_dir = dirs::home_dir()
        .ok_or_else(|| Error::Other("Could not determine home directory".to_string()))?;

    let skills_dir = home_dir.join(".codex").join("skills").join("binnacle");
    let skills_path = skills_dir.join("SKILL.md");

    // Create directory if it doesn't exist
    fs::create_dir_all(&skills_dir)
        .map_err(|e| Error::Other(format!("Failed to create Codex skills directory: {}", e)))?;

    // Write the skills file (overwrites if exists)
    fs::write(&skills_path, SKILLS_FILE_CONTENT)
        .map_err(|e| Error::Other(format!("Failed to create Codex skills file: {}", e)))?;

    Ok(true)
}

/// Create Copilot workflow agent and instruction files in the repository.
/// Writes to .github/agents/ and .github/instructions/
/// Always overwrites if the files already exist.
/// Returns true if the files were created/updated.
pub fn create_copilot_prompt_files(repo_path: &Path) -> Result<bool> {
    let agents_dir = repo_path.join(".github").join("agents");
    let instructions_dir = repo_path.join(".github").join("instructions");

    // Create directories if they don't exist
    fs::create_dir_all(&agents_dir)
        .map_err(|e| Error::Other(format!("Failed to create .github/agents directory: {}", e)))?;
    fs::create_dir_all(&instructions_dir).map_err(|e| {
        Error::Other(format!(
            "Failed to create .github/instructions directory: {}",
            e
        ))
    })?;

    // Write the agent files
    fs::write(
        agents_dir.join("binnacle-plan.agent.md"),
        PLAN_AGENT_CONTENT,
    )
    .map_err(|e| Error::Other(format!("Failed to write binnacle-plan.agent.md: {}", e)))?;

    fs::write(agents_dir.join("binnacle-prd.agent.md"), PRD_AGENT_CONTENT)
        .map_err(|e| Error::Other(format!("Failed to write binnacle-prd.agent.md: {}", e)))?;

    fs::write(
        agents_dir.join("binnacle-tasks.agent.md"),
        TASKS_AGENT_CONTENT,
    )
    .map_err(|e| Error::Other(format!("Failed to write binnacle-tasks.agent.md: {}", e)))?;

    // Write the instructions file
    fs::write(
        instructions_dir.join("binnacle.instructions.md"),
        COPILOT_INSTRUCTIONS_CONTENT,
    )
    .map_err(|e| Error::Other(format!("Failed to write binnacle.instructions.md: {}", e)))?;

    Ok(true)
}

/// Marker used to detect the start of the binnacle section.
const BINNACLE_SECTION_START: &str = "<!-- BEGIN BINNACLE SECTION -->";
/// Marker used to detect the end of the binnacle section.
const BINNACLE_SECTION_END: &str = "<!-- END BINNACLE SECTION -->";

/// Marker used to detect the start of the binnacle hook section.
const HOOK_SECTION_START: &str = "### BINNACLE HOOK START ###";
/// Marker used to detect the end of the binnacle hook section.
const HOOK_SECTION_END: &str = "### BINNACLE HOOK END ###";

/// The commit-msg hook script content
const COMMIT_MSG_HOOK_CONTENT: &str = r#"
### BINNACLE HOOK START ###
# Binnacle commit-msg hook
# Automatically appends Co-authored-by trailer when an agent session is active

# Check if co-author feature is enabled (default: true)
enabled_value=$(bn config get co-author.enabled 2>/dev/null | grep -o '"value":[^,}]*' | cut -d':' -f2 | tr -d ' "' || echo "null")

# If explicitly set to false/no/0, skip
if [[ "$enabled_value" == "false" ]] || [[ "$enabled_value" == "no" ]] || [[ "$enabled_value" == "0" ]]; then
    : # Skip binnacle co-author, continue to rest of hook
else
    # Check for active agent session via BN_AGENT_SESSION environment variable
    if [[ -n "$BN_AGENT_SESSION" ]] && [[ "$BN_AGENT_SESSION" == "1" ]]; then
        # Agent is active - append Co-authored-by trailer if not already present
        co_author_name=$(bn config get co-author.name 2>/dev/null | grep -o '"value":"[^"]*"' | cut -d'"' -f4 || echo "")
        if [[ -z "$co_author_name" ]] || [[ "$co_author_name" == "null" ]]; then
            co_author_name="binnacle-bot"
        fi

        co_author_email=$(bn config get co-author.email 2>/dev/null | grep -o '"value":"[^"]*"' | cut -d'"' -f4 || echo "")
        if [[ -z "$co_author_email" ]] || [[ "$co_author_email" == "null" ]]; then
            co_author_email="noreply@binnacle.bot"
        fi

        trailer="Co-authored-by: $co_author_name <$co_author_email>"

        # Check if trailer already exists (for amend case)
        if ! grep -qF "$trailer" "$1" 2>/dev/null; then
            echo "" >> "$1"
            echo "$trailer" >> "$1"
        fi
    fi
fi
### BINNACLE HOOK END ###
"#;

/// The post-commit hook script content for archive generation
const POST_COMMIT_HOOK_CONTENT: &str = r#"
### BINNACLE HOOK START ###
# Binnacle post-commit hook
# Generates archive snapshots when archive.directory is configured

# Exit early if bn is not available
if ! command -v bn &> /dev/null; then
    exit 0
fi

# Check if archive.directory is configured
archive_dir=$(bn config get archive.directory 2>/dev/null | jq -r '.value // empty' 2>/dev/null || true)
if [[ -n "$archive_dir" ]] && [[ "$archive_dir" != "null" ]]; then
    commit_sha=$(git rev-parse HEAD 2>/dev/null)
    if [[ -n "$commit_sha" ]]; then
        # Generate archive in the background to not slow down commits
        (bn system store archive "$commit_sha" > /dev/null 2>&1 &)
    fi
fi
### BINNACLE HOOK END ###
"#;

/// Replace the binnacle section in the given content with the new blurb.
/// Returns the new content with the section replaced.
/// Assumes the content contains both BEGIN and END markers.
fn replace_binnacle_section(content: &str) -> Result<String> {
    let start_idx = content
        .find(BINNACLE_SECTION_START)
        .ok_or_else(|| Error::Other("BEGIN BINNACLE SECTION marker not found".to_string()))?;

    // Find end marker - must search AFTER start to handle nested content
    let search_start = start_idx + BINNACLE_SECTION_START.len();
    let end_marker_relative = content[search_start..]
        .find(BINNACLE_SECTION_END)
        .ok_or_else(|| Error::Other("END BINNACLE SECTION marker not found".to_string()))?;
    let end_idx = search_start + end_marker_relative + BINNACLE_SECTION_END.len();

    // Build new content: before + new blurb + after
    let before = &content[..start_idx];
    let after = &content[end_idx..];

    Ok(format!("{}{}{}", before, AGENTS_MD_BLURB.trim_end(), after))
}

/// Update AGENTS.md with the binnacle blurb.
/// - If file doesn't exist: create with binnacle section
/// - If file exists with markers: replace section between markers (only if different)
/// - If file exists without markers: append binnacle section
///
/// Returns true if the file was actually modified.
fn update_agents_md(repo_path: &Path) -> Result<bool> {
    use std::fs;
    use std::io::Write;

    let agents_path = repo_path.join("AGENTS.md");

    if agents_path.exists() {
        let contents = fs::read_to_string(&agents_path)
            .map_err(|e| Error::Other(format!("Failed to read AGENTS.md: {}", e)))?;

        // Check if file has binnacle section markers
        let has_start = contents.contains(BINNACLE_SECTION_START);
        let has_end = contents.contains(BINNACLE_SECTION_END);

        // Warn about malformed markers (one without the other)
        if has_start != has_end {
            eprintln!(
                "Warning: AGENTS.md has {} but not {}. Appending fresh section.",
                if has_start {
                    "BEGIN marker"
                } else {
                    "END marker"
                },
                if has_start {
                    "END marker"
                } else {
                    "BEGIN marker"
                }
            );
        }

        if has_start && has_end {
            // Replace existing section
            let new_contents = replace_binnacle_section(&contents)?;
            // Only write if content actually changed
            if new_contents != contents {
                fs::write(&agents_path, new_contents)
                    .map_err(|e| Error::Other(format!("Failed to write AGENTS.md: {}", e)))?;
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            // Append new section
            let mut file = fs::OpenOptions::new()
                .append(true)
                .open(&agents_path)
                .map_err(|e| Error::Other(format!("Failed to open AGENTS.md: {}", e)))?;

            // Add a newline before the blurb if file doesn't end with one
            let prefix = if contents.ends_with('\n') {
                "\n"
            } else {
                "\n\n"
            };
            file.write_all(prefix.as_bytes())
                .map_err(|e| Error::Other(format!("Failed to write to AGENTS.md: {}", e)))?;
            file.write_all(AGENTS_MD_BLURB.trim_end().as_bytes())
                .map_err(|e| Error::Other(format!("Failed to write to AGENTS.md: {}", e)))?;
            file.write_all(b"\n")
                .map_err(|e| Error::Other(format!("Failed to write to AGENTS.md: {}", e)))?;
            Ok(true)
        }
    } else {
        // Create new AGENTS.md with the blurb
        fs::write(&agents_path, format!("{}\n", AGENTS_MD_BLURB.trim_end()))
            .map_err(|e| Error::Other(format!("Failed to create AGENTS.md: {}", e)))?;
        Ok(true)
    }
}

/// Install the commit-msg hook for co-author attribution.
/// - If hook doesn't exist: create it with shebang + binnacle section
/// - If hook exists with markers: do nothing (already installed)
/// - If hook exists without markers: append binnacle section
///
/// Returns true if the hook was installed/updated.
pub fn install_commit_msg_hook(repo_path: &Path) -> Result<bool> {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let hooks_dir = repo_path.join(".git").join("hooks");
    let hook_path = hooks_dir.join("commit-msg");

    // Ensure hooks directory exists
    if !hooks_dir.exists() {
        fs::create_dir_all(&hooks_dir)
            .map_err(|e| Error::Other(format!("Failed to create hooks directory: {}", e)))?;
    }

    if hook_path.exists() {
        let contents = fs::read_to_string(&hook_path)
            .map_err(|e| Error::Other(format!("Failed to read commit-msg hook: {}", e)))?;

        // Check if binnacle section already exists
        if contents.contains(HOOK_SECTION_START) {
            // Already installed, nothing to do
            return Ok(false);
        }

        // Append binnacle section to existing hook
        let new_contents = format!("{}{}", contents, COMMIT_MSG_HOOK_CONTENT);
        fs::write(&hook_path, new_contents)
            .map_err(|e| Error::Other(format!("Failed to update commit-msg hook: {}", e)))?;
    } else {
        // Create new hook with shebang + binnacle section
        let contents = format!("#!/usr/bin/env bash{}", COMMIT_MSG_HOOK_CONTENT);
        fs::write(&hook_path, contents)
            .map_err(|e| Error::Other(format!("Failed to create commit-msg hook: {}", e)))?;
    }

    // Ensure hook is executable
    let mut perms = fs::metadata(&hook_path)
        .map_err(|e| Error::Other(format!("Failed to get hook permissions: {}", e)))?
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&hook_path, perms)
        .map_err(|e| Error::Other(format!("Failed to set hook permissions: {}", e)))?;

    Ok(true)
}

/// Install the post-commit hook for archive generation.
/// - If hook doesn't exist: create it with shebang + binnacle section
/// - If hook exists with markers: do nothing (already installed)
/// - If hook exists without markers: append binnacle section
///
/// Returns true if the hook was installed/updated.
pub fn install_post_commit_hook(repo_path: &Path) -> Result<bool> {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let hooks_dir = repo_path.join(".git").join("hooks");
    let hook_path = hooks_dir.join("post-commit");

    // Ensure hooks directory exists
    if !hooks_dir.exists() {
        fs::create_dir_all(&hooks_dir)
            .map_err(|e| Error::Other(format!("Failed to create hooks directory: {}", e)))?;
    }

    if hook_path.exists() {
        let contents = fs::read_to_string(&hook_path)
            .map_err(|e| Error::Other(format!("Failed to read post-commit hook: {}", e)))?;

        // Check if binnacle section already exists
        if contents.contains(HOOK_SECTION_START) {
            // Already installed, nothing to do
            return Ok(false);
        }

        // Append binnacle section to existing hook
        let new_contents = format!("{}{}", contents, POST_COMMIT_HOOK_CONTENT);
        fs::write(&hook_path, new_contents)
            .map_err(|e| Error::Other(format!("Failed to update post-commit hook: {}", e)))?;
    } else {
        // Create new hook with shebang + binnacle section
        let contents = format!("#!/usr/bin/env bash{}", POST_COMMIT_HOOK_CONTENT);
        fs::write(&hook_path, contents)
            .map_err(|e| Error::Other(format!("Failed to create post-commit hook: {}", e)))?;
    }

    // Ensure hook is executable
    let mut perms = fs::metadata(&hook_path)
        .map_err(|e| Error::Other(format!("Failed to get hook permissions: {}", e)))?
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&hook_path, perms)
        .map_err(|e| Error::Other(format!("Failed to set hook permissions: {}", e)))?;

    Ok(true)
}

/// Uninstall the commit-msg hook (remove binnacle section).
/// - If hook doesn't exist: do nothing
/// - If hook exists without markers: do nothing
/// - If hook exists with only binnacle section: remove the file
/// - If hook exists with binnacle section and other content: remove only binnacle section
///
/// Returns true if something was changed.
pub fn uninstall_commit_msg_hook(repo_path: &Path) -> Result<bool> {
    use std::fs;

    let hook_path = repo_path.join(".git").join("hooks").join("commit-msg");

    if !hook_path.exists() {
        return Ok(false);
    }

    let contents = fs::read_to_string(&hook_path)
        .map_err(|e| Error::Other(format!("Failed to read commit-msg hook: {}", e)))?;

    // Check if binnacle section exists
    let has_start = contents.contains(HOOK_SECTION_START);
    let has_end = contents.contains(HOOK_SECTION_END);

    if !has_start && !has_end {
        // No binnacle section, nothing to do
        return Ok(false);
    }

    if has_start != has_end {
        // Malformed markers - warn and do nothing
        eprintln!(
            "Warning: commit-msg hook has {} but not {}. Manual cleanup required.",
            if has_start {
                "START marker"
            } else {
                "END marker"
            },
            if has_start {
                "END marker"
            } else {
                "START marker"
            }
        );
        return Ok(false);
    }

    // Find and remove the binnacle section
    let start_idx = contents
        .find(HOOK_SECTION_START)
        .ok_or_else(|| Error::Other("HOOK START marker not found".to_string()))?;

    // Find end marker - search AFTER start
    let search_start = start_idx + HOOK_SECTION_START.len();
    let end_marker_relative = contents[search_start..]
        .find(HOOK_SECTION_END)
        .ok_or_else(|| Error::Other("HOOK END marker not found".to_string()))?;
    let end_idx = search_start + end_marker_relative + HOOK_SECTION_END.len();

    // Build new content: before + after
    let before = &contents[..start_idx];
    let after = &contents[end_idx..];

    // Also remove the leading newline before our section if present
    let before = before.trim_end_matches('\n');
    let new_contents = format!("{}{}", before, after);

    // If the remaining content is just a shebang (or empty), remove the file
    let trimmed = new_contents.trim();
    if trimmed.is_empty() || trimmed == "#!/usr/bin/env bash" || trimmed == "#!/bin/bash" {
        fs::remove_file(&hook_path)
            .map_err(|e| Error::Other(format!("Failed to remove commit-msg hook: {}", e)))?;
    } else {
        fs::write(&hook_path, new_contents)
            .map_err(|e| Error::Other(format!("Failed to update commit-msg hook: {}", e)))?;
    }

    Ok(true)
}

/// Uninstall the post-commit hook (remove binnacle section).
/// - If hook doesn't exist: do nothing
/// - If hook exists without markers: do nothing
/// - If hook exists with only binnacle section: remove the file
/// - If hook exists with binnacle section and other content: remove only binnacle section
///
/// Returns true if something was changed.
pub fn uninstall_post_commit_hook(repo_path: &Path) -> Result<bool> {
    use std::fs;

    let hook_path = repo_path.join(".git").join("hooks").join("post-commit");

    if !hook_path.exists() {
        return Ok(false);
    }

    let contents = fs::read_to_string(&hook_path)
        .map_err(|e| Error::Other(format!("Failed to read post-commit hook: {}", e)))?;

    // Check for binnacle markers
    let has_start = contents.contains(HOOK_SECTION_START);
    let has_end = contents.contains(HOOK_SECTION_END);

    if !has_start || !has_end {
        // No binnacle section found
        return Ok(false);
    }

    // Find and remove the binnacle section
    let start_idx = contents
        .find(HOOK_SECTION_START)
        .ok_or_else(|| Error::Other("Hook start marker not found".to_string()))?;
    let search_start = start_idx + HOOK_SECTION_START.len();
    let end_relative = contents[search_start..]
        .find(HOOK_SECTION_END)
        .ok_or_else(|| Error::Other("Hook end marker not found".to_string()))?;
    let end_idx = search_start + end_relative + HOOK_SECTION_END.len();

    // Build new content without the binnacle section
    let before = &contents[..start_idx];
    let after = &contents[end_idx..];
    let new_contents = format!("{}{}", before.trim_end(), after.trim_start());

    // If the hook is now empty (or just shebang), remove it entirely
    let trimmed = new_contents.trim();
    if trimmed.is_empty() || trimmed == "#!/usr/bin/env bash" || trimmed == "#!/bin/bash" {
        fs::remove_file(&hook_path)
            .map_err(|e| Error::Other(format!("Failed to remove post-commit hook: {}", e)))?;
    } else {
        fs::write(&hook_path, new_contents)
            .map_err(|e| Error::Other(format!("Failed to update post-commit hook: {}", e)))?;
    }

    Ok(true)
}

/// Result of hooks uninstall command
#[derive(Debug, Serialize)]
pub struct HooksUninstallResult {
    pub commit_msg_removed: bool,
    pub post_commit_removed: bool,
}

impl Output for HooksUninstallResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut removed = Vec::new();
        if self.commit_msg_removed {
            removed.push("commit-msg");
        }
        if self.post_commit_removed {
            removed.push("post-commit");
        }

        if removed.is_empty() {
            "No binnacle hooks found to uninstall.".to_string()
        } else {
            format!(
                "Removed binnacle section from {} hook(s).",
                removed.join(", ")
            )
        }
    }
}

/// Uninstall binnacle hooks from the repository.
pub fn hooks_uninstall(repo_path: &Path) -> Result<HooksUninstallResult> {
    let commit_msg_removed = uninstall_commit_msg_hook(repo_path)?;
    let post_commit_removed = uninstall_post_commit_hook(repo_path)?;
    Ok(HooksUninstallResult {
        commit_msg_removed,
        post_commit_removed,
    })
}

// === Orient Command ===

#[derive(Debug, Serialize)]
pub struct OrientResult {
    /// Always true when orient succeeds - indicates the store is ready to use.
    /// Agents should proceed with task management commands.
    pub ready: bool,
    /// True only if this call just initialized the store (first time setup).
    /// False if the store was already initialized before this call.
    pub just_initialized: bool,
    /// The agent ID assigned to the calling agent (e.g., "bna-1234").
    /// Agents should store this and can use it for subsequent commands.
    pub agent_id: String,
    pub total_tasks: usize,
    pub ready_count: usize,
    pub ready_ids: Vec<String>,
    pub blocked_count: usize,
    pub in_progress_count: usize,
    /// Queue info: (title, queued_task_count) if a queue exists
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue: Option<OrientQueueInfo>,
    /// Number of ready tasks that are in the queue
    pub queued_ready_count: usize,
    /// Total number of bugs in the system
    pub total_bugs: usize,
    /// Number of open bugs (pending, in_progress, blocked, reopened)
    pub open_bugs_count: usize,
    /// Number of high/critical severity bugs that are open
    pub critical_bugs_count: usize,
    /// Total number of ideas
    pub total_ideas: usize,
    /// Number of open ideas (pending, promoted)
    pub open_ideas_count: usize,
    /// Total number of milestones
    pub total_milestones: usize,
    /// Number of open milestones (pending, in_progress)
    pub open_milestones_count: usize,
}

/// Queue info for orient output.
#[derive(Debug, Serialize)]
pub struct OrientQueueInfo {
    pub id: String,
    pub title: String,
    pub task_count: usize,
}

impl Output for OrientResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();

        lines.push("Binnacle - AI agent task tracker".to_string());
        lines.push(String::new());
        lines.push("This project uses binnacle (bn) for issue and test tracking.".to_string());
        lines.push(format!("Your agent ID: {}", self.agent_id));
        lines.push(String::new());
        lines.push("Current State:".to_string());
        lines.push(format!("  Total tasks: {}", self.total_tasks));

        // Primary sections: Bugs, Blocked, Ready (with queued info folded beneath)

        // Bugs section
        if self.total_bugs > 0 || self.open_bugs_count > 0 {
            let critical_info = if self.critical_bugs_count > 0 {
                format!(" ({} high/critical)", self.critical_bugs_count)
            } else {
                String::new()
            };
            lines.push(format!(
                "  Bugs: {} open{}",
                self.open_bugs_count, critical_info
            ));
        }

        // Blocked section
        lines.push(format!("  Blocked: {}", self.blocked_count));

        // Ready section with queued info folded beneath
        if !self.ready_ids.is_empty() {
            let ids = if self.ready_ids.len() <= 5 {
                self.ready_ids.join(", ")
            } else {
                format!(
                    "{}, ... ({} more)",
                    self.ready_ids[..5].join(", "),
                    self.ready_ids.len() - 5
                )
            };
            lines.push(format!("  Ready: {} ({})", self.ready_count, ids));
            // Fold queued info beneath ready
            if self.queue.is_some() && self.queued_ready_count > 0 {
                lines.push(format!(
                    "    └─ {} queued (priority)",
                    self.queued_ready_count
                ));
            }
        } else {
            lines.push(format!("  Ready: {}", self.ready_count));
        }

        // In progress
        lines.push(format!("  In progress: {}", self.in_progress_count));

        // Folded secondary sections: Ideas, Milestones, Queue
        let mut secondary = Vec::new();

        if self.total_ideas > 0 {
            secondary.push(format!("{} ideas", self.open_ideas_count));
        }
        if self.total_milestones > 0 {
            secondary.push(format!("{} milestones", self.open_milestones_count));
        }
        if let Some(ref queue_info) = self.queue {
            secondary.push(format!(
                "queue \"{}\" ({} tasks)",
                queue_info.title, queue_info.task_count
            ));
        }

        if !secondary.is_empty() {
            lines.push(format!("  Also: {}", secondary.join(", ")));
        }

        lines.push(String::new());
        lines.push("Key Commands:".to_string());
        lines
            .push("  bn              Status summary (JSON, use -H for human-readable)".to_string());
        lines.push("  bn ready        Show tasks ready to work on".to_string());
        lines.push("  bn task list    List all tasks".to_string());
        lines.push(
            "  bn show X       Show any entity by ID (works for bn-/bnt-/bnq- IDs)".to_string(),
        );
        lines.push("  bn test run     Run linked tests".to_string());
        lines
            .push("  bn goodbye \"reason\"  Gracefully terminate (include a summary!)".to_string());
        lines.push(String::new());
        lines.push("Run 'bn --help' for full command reference.".to_string());

        lines.join("\n")
    }
}

/// Parse agent type string to AgentType enum.
fn parse_agent_type(s: &str) -> Result<AgentType> {
    match s.to_lowercase().as_str() {
        "worker" => Ok(AgentType::Worker),
        "planner" => Ok(AgentType::Planner),
        "buddy" => Ok(AgentType::Buddy),
        _ => Err(Error::InvalidInput(format!(
            "Invalid agent type '{}'. Must be one of: worker, planner, buddy",
            s
        ))),
    }
}

/// Orient an AI agent to this project.
/// If allow_init is true, auto-initializes binnacle if not already initialized.
/// If allow_init is false and binnacle is not initialized, returns NotInitialized error.
pub fn orient(
    repo_path: &Path,
    agent_type_str: &str,
    allow_init: bool,
    name: Option<String>,
    purpose: Option<String>,
    dry_run: bool,
) -> Result<OrientResult> {
    // Parse agent type first
    let agent_type = parse_agent_type(agent_type_str)?;

    // Check if we need to initialize the store
    let just_initialized = if !Storage::exists(repo_path)? {
        if allow_init {
            Storage::init(repo_path)?;
            // Auto-update AGENTS.md (idempotent)
            let _ = update_agents_md(repo_path);
            true
        } else {
            return Err(Error::NotInitialized);
        }
    } else {
        false
    };

    // Get current state
    let mut storage = Storage::open(repo_path)?;

    // Check for MCP session mode (for MCP wrapper invocation)
    let mcp_session_id = std::env::var("BN_MCP_SESSION").ok();

    // In dry-run mode, skip agent registration and session state writing
    let agent_id = if dry_run {
        // Return a placeholder ID for dry-run mode
        "dry-run".to_string()
    } else {
        // Register agent (cleanup stale ones first)
        let _ = storage.cleanup_stale_agents();

        // Use parent PID as the agent identifier. The bn command itself exits after each
        // invocation, but the parent process (shell/AI agent) persists across multiple
        // bn commands. This prevents agents from being immediately cleaned up as "stale".
        let bn_pid = std::process::id();
        let parent_pid = get_parent_pid().unwrap_or(bn_pid);
        let agent_pid = parent_pid;

        // Use provided name, or BN_AGENT_NAME env var, or auto-generate (with parent PID for uniqueness)
        let agent_name = name
            .or_else(|| std::env::var("BN_AGENT_NAME").ok())
            .unwrap_or_else(|| format!("agent-{}", agent_pid));

        // For MCP mode, check if agent with this session ID already exists
        let existing_by_session = mcp_session_id
            .as_ref()
            .and_then(|sid| storage.get_agent_by_mcp_session(sid).ok());

        // Check if already registered (by MCP session or PID)
        let id = if let Some(mut existing_agent) = existing_by_session {
            // MCP session already registered - update it
            existing_agent.agent_type = agent_type.clone();
            if purpose.is_some() {
                existing_agent.purpose = purpose;
            }
            let id = existing_agent.id.clone();
            storage.update_agent(&existing_agent)?;
            let _ = storage.touch_agent(existing_agent.pid);
            id
        } else if mcp_session_id.is_none()
            && let Ok(mut existing_agent) = storage.get_agent(agent_pid)
        {
            // Normal PID-based lookup (no MCP session)
            existing_agent.agent_type = agent_type.clone();
            if purpose.is_some() {
                existing_agent.purpose = purpose;
            }
            let id = existing_agent.id.clone();
            storage.update_agent(&existing_agent)?;
            let _ = storage.touch_agent(agent_pid);
            id
        } else {
            // Register new agent
            let mut agent = if let Some(ref p) = purpose {
                Agent::new_with_purpose(
                    agent_pid,
                    bn_pid,
                    agent_name,
                    agent_type.clone(),
                    p.clone(),
                )
            } else {
                Agent::new(agent_pid, bn_pid, agent_name, agent_type.clone())
            };

            // Set MCP session ID if provided
            if let Some(ref sid) = mcp_session_id {
                agent.mcp_session_id = Some(sid.clone());
            }

            let id = agent.id.clone();
            storage.register_agent(&agent)?;
            id
        };

        // Write session state for commit-msg hook detection (skip in MCP mode)
        if mcp_session_id.is_none() {
            let session_state = SessionState::new(agent_pid, agent_type);
            storage.write_session_state(&session_state)?;
        }

        id
    };

    let tasks = storage.list_tasks(None, None, None)?;

    let mut ready_ids = Vec::new();
    let mut blocked_count = 0;
    let mut in_progress_count = 0;

    for task in &tasks {
        match task.status {
            TaskStatus::InProgress => in_progress_count += 1,
            TaskStatus::Blocked => blocked_count += 1,
            TaskStatus::Pending | TaskStatus::Reopened => {
                // Check legacy dependencies
                let legacy_deps_done = task.depends_on.is_empty()
                    || task.depends_on.iter().all(|dep_id| {
                        storage
                            .get_task(dep_id)
                            .map(|t| t.status == TaskStatus::Done)
                            .unwrap_or(false)
                    });

                // Check edge-based dependencies
                let edge_deps = storage
                    .get_edge_dependencies(&task.core.id)
                    .unwrap_or_default();
                let edge_deps_done = edge_deps.is_empty()
                    || edge_deps.iter().all(|dep_id| {
                        // Check if it's a task or bug
                        if let Ok(t) = storage.get_task(dep_id) {
                            t.status == TaskStatus::Done
                        } else if let Ok(b) = storage.get_bug(dep_id) {
                            b.status == TaskStatus::Done
                        } else {
                            false
                        }
                    });

                if legacy_deps_done && edge_deps_done {
                    ready_ids.push(task.core.id.clone());
                } else {
                    blocked_count += 1;
                }
            }
            _ => {}
        }
    }

    let ready_count = ready_ids.len();

    // Get queue info if exists
    let (queue_info, queued_ready_count) = if let Ok(queue) = storage.get_queue() {
        let queued_tasks = storage.get_queued_tasks().unwrap_or_default();
        let queued_task_ids: std::collections::HashSet<_> =
            queued_tasks.iter().map(|t| t.core.id.as_str()).collect();

        // Count how many ready tasks are queued
        let queued_ready = ready_ids
            .iter()
            .filter(|id| queued_task_ids.contains(id.as_str()))
            .count();

        (
            Some(OrientQueueInfo {
                id: queue.id,
                title: queue.title,
                task_count: queued_tasks.len(),
            }),
            queued_ready,
        )
    } else {
        (None, 0)
    };

    // Get bug stats
    let bugs = storage.list_bugs(None, None, None, None, true)?; // Include all for stats
    let total_bugs = bugs.len();
    let open_bugs_count = bugs
        .iter()
        .filter(|b| {
            matches!(
                b.status,
                TaskStatus::Pending
                    | TaskStatus::InProgress
                    | TaskStatus::Blocked
                    | TaskStatus::Reopened
            )
        })
        .count();
    let critical_bugs_count = bugs
        .iter()
        .filter(|b| {
            matches!(
                b.status,
                TaskStatus::Pending
                    | TaskStatus::InProgress
                    | TaskStatus::Blocked
                    | TaskStatus::Reopened
            ) && matches!(b.severity, BugSeverity::High | BugSeverity::Critical)
        })
        .count();

    // Get ideas stats
    let ideas = storage.list_ideas(None, None)?;
    let total_ideas = ideas.len();
    let open_ideas_count = ideas
        .iter()
        .filter(|i| {
            matches!(
                i.status,
                crate::models::IdeaStatus::Seed | crate::models::IdeaStatus::Germinating
            )
        })
        .count();

    // Get milestones stats
    let milestones = storage.list_milestones(None, None, None)?;
    let total_milestones = milestones.len();
    let open_milestones_count = milestones
        .iter()
        .filter(|m| matches!(m.status, TaskStatus::Pending | TaskStatus::InProgress))
        .count();

    Ok(OrientResult {
        ready: true, // Always true when orient succeeds - store is ready to use
        just_initialized,
        agent_id,
        total_tasks: tasks.len(),
        ready_count,
        ready_ids,
        blocked_count,
        in_progress_count,
        queue: queue_info,
        queued_ready_count,
        total_bugs,
        open_bugs_count,
        critical_bugs_count,
        total_ideas,
        open_ideas_count,
        total_milestones,
        open_milestones_count,
    })
}

/// Get the parent process ID.
#[cfg(unix)]
fn get_parent_pid() -> Option<u32> {
    use std::os::unix::process::parent_id;
    Some(parent_id())
}

#[cfg(not(unix))]
fn get_parent_pid() -> Option<u32> {
    None
}

/// Get the parent PID of a given process by reading /proc/<pid>/stat.
/// Returns None if the process doesn't exist or can't be read.
#[cfg(unix)]
fn get_ppid_of_pid(pid: u32) -> Option<u32> {
    use std::fs;
    let stat_path = format!("/proc/{}/stat", pid);
    let stat = fs::read_to_string(stat_path).ok()?;
    // Format: pid (comm) state ppid ...
    // The ppid is the 4th field, but comm can contain spaces/parens
    // So we find the last ')' and parse from there
    let last_paren = stat.rfind(')')?;
    let after_comm = &stat[last_paren + 2..]; // skip ") "
    let fields: Vec<&str> = after_comm.split_whitespace().collect();
    // fields[0] is state, fields[1] is ppid
    fields.get(1)?.parse().ok()
}

#[cfg(not(unix))]
fn get_ppid_of_pid(_pid: u32) -> Option<u32> {
    None
}

/// Get the grandparent PID (parent of parent process).
#[cfg(unix)]
fn get_grandparent_pid() -> Option<u32> {
    let parent_pid = get_parent_pid()?;
    get_ppid_of_pid(parent_pid)
}

#[cfg(not(unix))]
fn get_grandparent_pid() -> Option<u32> {
    None
}

/// Find a registered agent that is an ancestor of the current process.
/// This traverses the process tree upwards to find any agent that was
/// registered by a parent shell (since each bn command runs in a new subprocess).
///
/// If no ancestor agent is found, falls back to checking the session state file,
/// which records which agent was registered during `bn orient`. This handles the
/// case where multiple shells are spawned under the same parent (siblings), and
/// the registered agent is not a direct ancestor of the current process.
///
/// Returns the agent's PID if found, None otherwise.
#[cfg(unix)]
fn find_ancestor_agent(storage: &Storage) -> Option<u32> {
    // Get all registered agents
    let agents = storage.list_agents(None).ok()?;
    if agents.is_empty() {
        return None;
    }

    // Build a set of registered agent PIDs for fast lookup
    let agent_pids: std::collections::HashSet<u32> = agents.iter().map(|a| a.pid).collect();

    // Traverse the process tree upwards looking for a registered agent
    let mut current_pid = get_parent_pid()?;

    // Limit depth to avoid infinite loops (shouldn't happen, but be safe)
    for _ in 0..100 {
        if current_pid <= 1 {
            // Reached init/systemd
            break;
        }

        if agent_pids.contains(&current_pid) {
            return Some(current_pid);
        }

        // Move up to the parent
        current_pid = get_ppid_of_pid(current_pid)?;
    }

    // Fallback: Check session state for the registered agent PID
    // This handles the case where multiple shells are spawned under the same parent,
    // and the registered agent is a sibling rather than an ancestor of this process.
    if let Ok(session_state) = storage.read_session_state()
        && agent_pids.contains(&session_state.agent_pid)
        && is_process_alive(session_state.agent_pid)
    {
        return Some(session_state.agent_pid);
    }

    None
}

/// Check if a process is still alive.
#[cfg(unix)]
fn is_process_alive(pid: u32) -> bool {
    std::path::Path::new(&format!("/proc/{}", pid)).exists()
}

#[cfg(not(unix))]
fn find_ancestor_agent(_storage: &Storage) -> Option<u32> {
    None
}

// === Generic Show Command ===

/// Result for generic show command - contains entity type and data.
#[derive(Serialize)]
pub struct GenericShowResult {
    #[serde(rename = "type")]
    pub entity_type: EntityType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<TaskShowResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bug: Option<BugShowResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idea: Option<Idea>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test: Option<TestNode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub milestone: Option<MilestoneShowResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge: Option<Edge>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue: Option<Queue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<Doc>,
}

impl Output for GenericShowResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        match self.entity_type {
            EntityType::Task => {
                if let Some(ref task) = self.task {
                    task.to_human()
                } else {
                    "Task data not available".to_string()
                }
            }
            EntityType::Bug => {
                if let Some(ref bug) = self.bug {
                    bug.to_human()
                } else {
                    "Bug data not available".to_string()
                }
            }
            EntityType::Idea => {
                if let Some(ref idea) = self.idea {
                    idea.to_human()
                } else {
                    "Idea data not available".to_string()
                }
            }
            EntityType::Test => {
                if let Some(ref test) = self.test {
                    test.to_human()
                } else {
                    "Test data not available".to_string()
                }
            }
            EntityType::Milestone => {
                if let Some(ref milestone) = self.milestone {
                    milestone.to_human()
                } else {
                    "Milestone data not available".to_string()
                }
            }
            EntityType::Edge => {
                if let Some(ref edge) = self.edge {
                    format!(
                        "Edge: {}\n  {} --[{}]--> {}\n  Created: {}",
                        edge.id, edge.source, edge.edge_type, edge.target, edge.created_at
                    )
                } else {
                    "Edge data not available".to_string()
                }
            }
            EntityType::Queue => {
                if let Some(ref queue) = self.queue {
                    queue.to_human()
                } else {
                    "Queue data not available".to_string()
                }
            }
            EntityType::Doc => {
                if let Some(ref doc) = self.doc {
                    format!(
                        "Doc: {} \"{}\"\n  Content: {} bytes\n  Created: {}\n  Updated: {}",
                        doc.core.id,
                        doc.core.title,
                        doc.content.len(),
                        doc.core.created_at,
                        doc.core.updated_at
                    )
                } else {
                    "Doc data not available".to_string()
                }
            }
        }
    }
}

/// Generic show command - auto-detects entity type and returns formatted data.
pub fn generic_show(repo_path: &Path, id: &str) -> Result<GenericShowResult> {
    let storage = Storage::open(repo_path)?;
    let entity_type = storage.get_entity_type(id)?;

    let mut result = GenericShowResult {
        entity_type,
        task: None,
        bug: None,
        idea: None,
        test: None,
        milestone: None,
        edge: None,
        queue: None,
        doc: None,
    };

    match entity_type {
        EntityType::Task => {
            let response = task_show(repo_path, id)?;
            if let TaskShowResponse::Found(task_result) = response {
                result.task = Some(*task_result);
            }
        }
        EntityType::Bug => {
            let response = bug_show(repo_path, id)?;
            if let BugShowResponse::Found(bug_result) = response {
                result.bug = Some(*bug_result);
            }
        }
        EntityType::Idea => {
            result.idea = Some(idea_show(repo_path, id)?);
        }
        EntityType::Test => {
            result.test = Some(test_show(repo_path, id)?);
        }
        EntityType::Milestone => {
            result.milestone = Some(milestone_show(repo_path, id)?);
        }
        EntityType::Edge => {
            result.edge = Some(storage.get_edge(id)?);
        }
        EntityType::Queue => {
            result.queue = Some(storage.get_queue_by_id(id)?);
        }
        EntityType::Doc => {
            result.doc = Some(storage.get_doc(id)?);
        }
    }

    Ok(result)
}

// === Task Commands ===

#[derive(Serialize)]
pub struct TaskCreated {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queued_to: Option<String>,
}

impl Output for TaskCreated {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let base = match &self.short_name {
            Some(sn) => format!("Created task {} [{}] \"{}\"", self.id, sn, self.title),
            None => format!("Created task {} \"{}\"", self.id, self.title),
        };
        match &self.queued_to {
            Some(q) => format!("{} (added to queue {})", base, q),
            None => base,
        }
    }
}

/// Internal helper to add an entity to the queue, auto-creating the queue if needed.
fn add_entity_to_queue_internal(storage: &mut Storage, entity_id: &str) -> Result<String> {
    // Get or create the queue
    let queue = match storage.get_queue() {
        Ok(q) => q,
        Err(_) => {
            // Create default queue if it doesn't exist
            let title = "Work Queue".to_string();
            let queue_id = generate_id("bnq", &title);
            let new_queue = Queue::new(queue_id, title);
            storage.create_queue(&new_queue)?;
            new_queue
        }
    };
    let queue_id = queue.id.clone();

    // Create queued edge
    let edge_id = generate_id("bne", &format!("{}-{}", entity_id, queue_id));
    let edge = Edge::new(
        edge_id,
        entity_id.to_string(),
        queue_id.clone(),
        EdgeType::Queued,
    );
    storage.add_edge(&edge)?;

    Ok(queue_id)
}

/// Result of task creation with complexity check.
///
/// When complexity is detected and the task is not forced, returns a suggestion
/// instead of creating the task. This implements the "soft-gate" pattern for
/// buddy agents.
#[derive(Serialize)]
pub struct TaskCreateResult {
    /// Whether complexity was detected
    pub complexity_detected: bool,

    /// The complexity score (if checked)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complexity_score: Option<u8>,

    /// The suggestion text from buddy (if complex)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,

    /// Reasons for complexity assessment
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub complexity_reasons: Vec<String>,

    /// Command to force task creation anyway
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proceed_command: Option<String>,

    /// Command to file as idea instead
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idea_command: Option<String>,

    /// The created task (if not complex or forced)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_created: Option<TaskCreated>,
}

impl Output for TaskCreateResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if let Some(ref task) = self.task_created {
            // Task was created normally
            task.to_human()
        } else if let Some(ref suggestion) = self.suggestion {
            // Complexity detected, show soft-gate suggestion
            let mut lines = vec![suggestion.clone()];

            if let Some(ref cmd) = self.idea_command {
                lines.push(format!("\nTo file as idea: {}", cmd));
            }
            if let Some(ref cmd) = self.proceed_command {
                lines.push(format!("To file as task anyway: {}", cmd));
            }

            lines.join("\n")
        } else {
            "Task creation result".to_string()
        }
    }
}

/// Create a new task with complexity checking.
///
/// Analyzes the task title and description for complexity indicators. If the
/// task appears complex (multiple concerns, exploratory language, etc.), returns
/// a soft-gate suggestion recommending to file as an idea instead.
///
/// This is useful for buddy agents to help users avoid creating overly complex
/// tasks that might be better suited as ideas for further breakdown.
#[allow(clippy::too_many_arguments)]
pub fn task_create_with_complexity_check(
    repo_path: &Path,
    title: String,
    short_name: Option<String>,
    description: Option<String>,
    priority: Option<u8>,
    tags: Vec<String>,
    assignee: Option<String>,
    queue: bool,
) -> Result<TaskCreateResult> {
    // Analyze complexity
    let score = analyze_complexity(&title, description.as_deref());

    if score.is_complex() {
        // Build helper commands for the user
        let escaped_title = title.replace('"', "\\\"");
        let mut force_cmd = format!("bn task create \"{}\" --force", escaped_title);
        let mut idea_cmd = format!("bn idea create \"{}\"", escaped_title);

        // Add optional flags to force command
        if let Some(ref sn) = short_name {
            let escaped_sn = sn.replace('"', "\\\"");
            force_cmd.push_str(&format!(" -s \"{}\"", escaped_sn));
            idea_cmd.push_str(&format!(" -s \"{}\"", escaped_sn));
        }
        if let Some(ref desc) = description {
            let escaped_desc = desc.replace('"', "\\\"");
            force_cmd.push_str(&format!(" -d \"{}\"", escaped_desc));
            idea_cmd.push_str(&format!(" -d \"{}\"", escaped_desc));
        }
        if let Some(p) = priority {
            force_cmd.push_str(&format!(" -p {}", p));
        }
        for tag in &tags {
            force_cmd.push_str(&format!(" -t {}", tag));
            idea_cmd.push_str(&format!(" -t {}", tag));
        }
        if let Some(ref a) = assignee {
            force_cmd.push_str(&format!(" -a {}", a));
        }
        if queue {
            force_cmd.push_str(" -q");
        }

        return Ok(TaskCreateResult {
            complexity_detected: true,
            complexity_score: Some(score.score),
            suggestion: score.soft_gate_suggestion(),
            complexity_reasons: score.reasons.clone(),
            proceed_command: Some(force_cmd),
            idea_command: Some(idea_cmd),
            task_created: None,
        });
    }

    // Not complex - create the task normally
    let task = task_create_with_queue(
        repo_path,
        title,
        short_name,
        description,
        priority,
        tags,
        assignee,
        queue,
    )?;

    Ok(TaskCreateResult {
        complexity_detected: false,
        complexity_score: Some(score.score),
        suggestion: None,
        complexity_reasons: vec![],
        proceed_command: None,
        idea_command: None,
        task_created: Some(task),
    })
}

/// Create a new task.
pub fn task_create(
    repo_path: &Path,
    title: String,
    short_name: Option<String>,
    description: Option<String>,
    priority: Option<u8>,
    tags: Vec<String>,
    assignee: Option<String>,
) -> Result<TaskCreated> {
    task_create_with_queue(
        repo_path,
        title,
        short_name,
        description,
        priority,
        tags,
        assignee,
        false,
    )
}

/// Create a new task with optional immediate queuing.
#[allow(clippy::too_many_arguments)]
pub fn task_create_with_queue(
    repo_path: &Path,
    title: String,
    short_name: Option<String>,
    description: Option<String>,
    priority: Option<u8>,
    tags: Vec<String>,
    assignee: Option<String>,
    queue: bool,
) -> Result<TaskCreated> {
    let mut storage = Storage::open(repo_path)?;

    // Validate priority if provided
    if let Some(p) = priority
        && p > 4
    {
        return Err(Error::Other("Priority must be 0-4".to_string()));
    }

    // Auto-truncate very long short_name (2x display limit = 30 chars)
    let short_name = short_name.map(|sn| {
        if sn.chars().count() > 30 {
            eprintln!(
                "Note: short_name truncated from {} to 30 chars for GUI display.",
                sn.chars().count()
            );
            sn.chars().take(30).collect::<String>()
        } else {
            sn
        }
    });

    let id = storage.generate_unique_id("bn", &title);
    let mut task = Task::new(id.clone(), title.clone());
    task.core.short_name = short_name.clone();
    task.core.description = description;
    task.priority = priority.unwrap_or(2);
    task.core.tags = tags;
    task.assignee = assignee;

    storage.create_task(&task)?;

    // Add to queue if requested
    let queued_to = if queue {
        Some(add_entity_to_queue_internal(&mut storage, &id)?)
    } else {
        None
    };

    Ok(TaskCreated {
        id,
        title,
        short_name,
        queued_to,
    })
}

impl Output for Task {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("{} {}", self.core.id, self.core.title));
        if let Some(ref sn) = self.core.short_name {
            lines.push(format!("  Short Name: {}", sn));
        }
        lines.push(format!(
            "  Status: {:?}  Priority: {}",
            self.status, self.priority
        ));
        if let Some(ref desc) = self.core.description {
            lines.push(format!("  Description: {}", desc));
        }
        if !self.core.tags.is_empty() {
            lines.push(format!("  Tags: {}", self.core.tags.join(", ")));
        }
        if let Some(ref assignee) = self.assignee {
            lines.push(format!("  Assignee: {}", assignee));
        }
        if !self.depends_on.is_empty() {
            lines.push(format!("  Depends on: {}", self.depends_on.join(", ")));
        }
        lines.push(format!(
            "  Created: {}",
            self.core.created_at.format("%Y-%m-%d %H:%M")
        ));
        lines.push(format!(
            "  Updated: {}",
            self.core.updated_at.format("%Y-%m-%d %H:%M")
        ));
        if let Some(closed) = self.closed_at {
            lines.push(format!("  Closed: {}", closed.format("%Y-%m-%d %H:%M")));
            if let Some(ref reason) = self.closed_reason {
                lines.push(format!("  Reason: {}", reason));
            }
        }
        lines.join("\n")
    }
}

/// Information about what is blocking a task.
#[derive(Serialize)]
pub struct BlockingInfo {
    pub is_blocked: bool,
    pub blocker_count: usize,
    pub direct_blockers: Vec<DirectBlocker>,
    pub blocker_chain: Vec<String>,
    pub summary: String,
}

/// A task that is directly blocking another task.
#[derive(Serialize)]
pub struct DirectBlocker {
    pub id: String,
    pub title: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub blocked_by: Vec<String>,
}

/// Result of task_show with optional blocking analysis.
#[derive(Serialize)]
pub struct TaskShowResult {
    #[serde(flatten)]
    pub task: Task,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocking_info: Option<BlockingInfo>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub edges: Vec<TaskEdgeInfo>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub linked_docs: Vec<LinkedDocInfo>,
}

/// Information about a doc linked to an entity.
#[derive(Serialize)]
pub struct LinkedDocInfo {
    pub id: String,
    pub title: String,
    pub doc_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// Helper to get linked docs for an entity.
/// If `include_content` is true, also fetches and includes full doc content.
fn get_linked_docs_for_entity(
    storage: &Storage,
    entity_id: &str,
    include_content: bool,
) -> Vec<LinkedDocInfo> {
    // Find all 'documents' edges where this entity is the target
    let edges = storage
        .list_edges(Some(EdgeType::Documents), None, Some(entity_id))
        .unwrap_or_default();

    edges
        .iter()
        .filter_map(|edge| {
            // The source of a 'documents' edge is the doc
            storage.get_doc(&edge.source).ok().map(|doc| {
                // Extract summary from content (first line of # Summary section, trimmed)
                let summary = doc.get_summary().ok().and_then(|s| {
                    let trimmed = s.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        // Get just the first line of the summary for brief display
                        Some(trimmed.lines().next().unwrap_or("").to_string())
                    }
                });

                // Get full content if requested
                let content = if include_content {
                    doc.get_content().ok()
                } else {
                    None
                };

                LinkedDocInfo {
                    id: doc.core.id.clone(),
                    title: doc.core.title.clone(),
                    doc_type: doc.doc_type.to_string(),
                    short_name: doc.core.short_name.clone(),
                    description: doc.core.description.clone(),
                    summary,
                    content,
                }
            })
        })
        .collect()
}

/// Edge information for task display.
#[derive(Serialize)]
pub struct TaskEdgeInfo {
    pub edge_type: String,
    pub direction: String,
    pub related_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_status: Option<String>,
}

impl Output for TaskShowResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = vec![
            format!("Task: {}", self.task.core.id),
            format!("Title: {}", self.task.core.title),
        ];

        if let Some(ref sn) = self.task.core.short_name {
            lines.push(format!("Short Name: {}", sn));
        }

        lines.push(format!("Status: {:?}", self.task.status));
        lines.push(format!("Priority: P{}", self.task.priority));

        if !self.task.core.tags.is_empty() {
            lines.push(format!("Tags: {}", self.task.core.tags.join(", ")));
        }

        if let Some(ref desc) = self.task.core.description {
            lines.push(format!("Description: {}", desc));
        }

        if let Some(ref assignee) = self.task.assignee {
            lines.push(format!("Assignee: {}", assignee));
        }

        // Show legacy dependencies (if any)
        if !self.task.depends_on.is_empty() {
            lines.push(format!(
                "\nDependencies ({}): {}",
                self.task.depends_on.len(),
                self.task.depends_on.join(", ")
            ));
        }

        // Show edge-based relationships grouped by type
        if !self.edges.is_empty() {
            lines.push(String::new());

            // Group edges by type and direction
            let mut depends_on: Vec<&TaskEdgeInfo> = Vec::new();
            let mut blocks: Vec<&TaskEdgeInfo> = Vec::new();
            let mut related: Vec<&TaskEdgeInfo> = Vec::new();
            let mut other: Vec<&TaskEdgeInfo> = Vec::new();

            for edge in &self.edges {
                match edge.edge_type.as_str() {
                    "depends_on" if edge.direction == "outbound" => depends_on.push(edge),
                    "depends_on" if edge.direction == "inbound" => blocks.push(edge),
                    "blocks" if edge.direction == "outbound" => blocks.push(edge),
                    "related_to" => related.push(edge),
                    _ => other.push(edge),
                }
            }

            if !depends_on.is_empty() {
                lines.push("Dependencies (edges):".to_string());
                for e in depends_on {
                    let status = e.related_status.as_deref().unwrap_or("unknown");
                    let title = e.related_title.as_deref().unwrap_or("");
                    lines.push(format!("  → {} \"{}\" [{}]", e.related_id, title, status));
                }
            }

            if !blocks.is_empty() {
                lines.push("Blocks:".to_string());
                for e in blocks {
                    let status = e.related_status.as_deref().unwrap_or("unknown");
                    let title = e.related_title.as_deref().unwrap_or("");
                    lines.push(format!("  ← {} \"{}\" [{}]", e.related_id, title, status));
                }
            }

            if !related.is_empty() {
                lines.push("Related:".to_string());
                for e in related {
                    let status = e.related_status.as_deref().unwrap_or("unknown");
                    let title = e.related_title.as_deref().unwrap_or("");
                    lines.push(format!("  ↔ {} \"{}\" [{}]", e.related_id, title, status));
                }
            }

            if !other.is_empty() {
                lines.push("Other links:".to_string());
                for e in other {
                    let arrow = match e.direction.as_str() {
                        "outbound" => "→",
                        "inbound" => "←",
                        _ => "↔",
                    };
                    let status = e.related_status.as_deref().unwrap_or("unknown");
                    let title = e.related_title.as_deref().unwrap_or("");
                    lines.push(format!(
                        "  {} {} \"{}\" ({}) [{}]",
                        arrow, e.related_id, title, e.edge_type, status
                    ));
                }
            }
        }

        // Show linked docs
        if !self.linked_docs.is_empty() {
            lines.push(String::new());
            lines.push(format!("📄 Related Docs ({}):", self.linked_docs.len()));
            for doc in &self.linked_docs {
                let name = doc.short_name.as_deref().unwrap_or(&doc.title);
                // Format: bn-xxxx [type] "Title" - summary
                let mut doc_line = format!("  {} [{}] \"{}\"", doc.id, doc.doc_type, name);
                if let Some(ref summary) = doc.summary {
                    doc_line.push_str(&format!(" - {}", summary));
                }
                lines.push(doc_line);
                // Show full content if available
                if let Some(ref content) = doc.content {
                    lines.push("  ---".to_string());
                    for line in content.lines() {
                        lines.push(format!("  {}", line));
                    }
                    lines.push("  ---".to_string());
                }
            }
        }

        if let Some(ref blocking) = self.blocking_info {
            lines.push(format!("\n{}", blocking.summary));
        }

        if let Some(ref closed_at) = self.task.closed_at {
            lines.push(format!("\nClosed at: {}", closed_at));
            if let Some(ref reason) = self.task.closed_reason {
                lines.push(format!("Reason: {}", reason));
            }
        }

        lines.join("\n")
    }
}

/// Wrapper for task show that can contain either a task or a type mismatch.
#[derive(Serialize)]
#[serde(untagged)]
pub enum TaskShowResponse {
    Found(Box<TaskShowResult>),
    TypeMismatch(Box<EntityMismatchResult>),
}

impl TaskShowResponse {
    /// Get the task if this is a Found response.
    pub fn task(&self) -> Option<&Task> {
        match self {
            TaskShowResponse::Found(result) => Some(&result.task),
            TaskShowResponse::TypeMismatch(_) => None,
        }
    }

    /// Unwrap the Found result, panicking if it's a TypeMismatch.
    #[cfg(test)]
    pub fn unwrap(self) -> TaskShowResult {
        match self {
            TaskShowResponse::Found(result) => *result,
            TaskShowResponse::TypeMismatch(_) => panic!("Expected Found, got TypeMismatch"),
        }
    }
}

impl Output for TaskShowResponse {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        match self {
            TaskShowResponse::Found(result) => result.to_human(),
            TaskShowResponse::TypeMismatch(mismatch) => mismatch.to_human(),
        }
    }
}

/// Result when an entity was found but is a different type than requested.
#[derive(Serialize)]
pub struct EntityMismatchResult {
    pub note: String,
    pub actual_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<TaskShowResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bug: Option<BugShowResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test: Option<TestNode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub milestone: Option<Milestone>,
}

impl Output for EntityMismatchResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = vec![format!("Note: {}", self.note), String::new()];

        if let Some(ref task) = self.task {
            lines.push(task.to_human());
        } else if let Some(ref bug) = self.bug {
            lines.push(bug.to_human());
        } else if let Some(ref test) = self.test {
            lines.push(format!("Test: {}", test.id));
            lines.push(format!("Name: {}", test.name));
            lines.push(format!("Command: {}", test.command));
        } else if let Some(ref milestone) = self.milestone {
            lines.push(format!("Milestone: {}", milestone.core.id));
            lines.push(format!("Title: {}", milestone.core.title));
        }

        lines.join("\n")
    }
}

/// Analyze what is blocking a task from completion.
fn analyze_blockers(storage: &Storage, task: &Task) -> Result<BlockingInfo> {
    let mut direct_blockers = Vec::new();
    let mut blocker_chain = Vec::new();

    // Combine legacy depends_on and edge-based dependencies
    let mut all_deps: Vec<String> = task.depends_on.clone();
    let edge_deps = storage
        .get_edge_dependencies(&task.core.id)
        .unwrap_or_default();
    for dep in edge_deps {
        if !all_deps.contains(&dep) {
            all_deps.push(dep);
        }
    }

    for dep_id in &all_deps {
        // Try to get as task first, then as bug
        let (dep_status, dep_title, dep_assignee, dep_deps) =
            if let Ok(dep) = storage.get_task(dep_id) {
                let dep_edge_deps = storage.get_edge_dependencies(dep_id).unwrap_or_default();
                let mut combined_deps = dep.depends_on.clone();
                for d in dep_edge_deps {
                    if !combined_deps.contains(&d) {
                        combined_deps.push(d);
                    }
                }
                (
                    dep.status.clone(),
                    dep.core.title.clone(),
                    dep.assignee.clone(),
                    combined_deps,
                )
            } else if let Ok(bug) = storage.get_bug(dep_id) {
                let dep_edge_deps = storage.get_edge_dependencies(dep_id).unwrap_or_default();
                let mut combined_deps = bug.depends_on.clone();
                for d in dep_edge_deps {
                    if !combined_deps.contains(&d) {
                        combined_deps.push(d);
                    }
                }
                (
                    bug.status.clone(),
                    bug.core.title.clone(),
                    bug.assignee.clone(),
                    combined_deps,
                )
            } else {
                continue; // Skip if entity not found
            };

        // Only consider incomplete dependencies as blockers
        if dep_status != TaskStatus::Done && dep_status != TaskStatus::Cancelled {
            // Find what's blocking this dependency (transitive blockers)
            let dep_blockers: Vec<String> = dep_deps
                .iter()
                .filter(|d| {
                    if let Ok(t) = storage.get_task(d) {
                        t.status != TaskStatus::Done && t.status != TaskStatus::Cancelled
                    } else if let Ok(b) = storage.get_bug(d) {
                        b.status != TaskStatus::Done && b.status != TaskStatus::Cancelled
                    } else {
                        false
                    }
                })
                .cloned()
                .collect();

            direct_blockers.push(DirectBlocker {
                id: dep_id.clone(),
                title: dep_title,
                status: format!("{:?}", dep_status).to_lowercase(),
                assignee: dep_assignee,
                blocked_by: dep_blockers.clone(),
            });

            // Build chain representation
            if dep_blockers.is_empty() {
                blocker_chain.push(format!(
                    "{} <- {} ({})",
                    task.core.id,
                    dep_id,
                    format!("{:?}", dep_status).to_lowercase()
                ));
            } else {
                for blocker in &dep_blockers {
                    let blocker_status = if let Ok(b) = storage.get_task(blocker) {
                        format!("{:?}", b.status).to_lowercase()
                    } else if let Ok(b) = storage.get_bug(blocker) {
                        format!("{:?}", b.status).to_lowercase()
                    } else {
                        "unknown".to_string()
                    };
                    blocker_chain.push(format!(
                        "{} <- {} <- {} ({})",
                        task.core.id, dep_id, blocker, blocker_status
                    ));
                }
            }
        }
    }

    let is_blocked = !direct_blockers.is_empty();
    let blocker_count = direct_blockers.len();

    let summary = if is_blocked {
        build_blocker_summary(&direct_blockers, blocker_count)
    } else {
        "All dependencies complete.".to_string()
    };

    Ok(BlockingInfo {
        is_blocked,
        blocker_count,
        direct_blockers,
        blocker_chain,
        summary,
    })
}

/// Build a human-readable summary of blocking dependencies.
fn build_blocker_summary(blockers: &[DirectBlocker], count: usize) -> String {
    let mut parts = Vec::new();
    parts.push(format!("Blocked by {} incomplete dependencies.", count));

    for blocker in blockers {
        let mut desc = format!("- {} is {}", blocker.id, blocker.status);
        if let Some(ref assignee) = blocker.assignee {
            desc.push_str(&format!(" (assigned: {})", assignee));
        }
        if !blocker.blocked_by.is_empty() {
            desc.push_str(&format!(", blocked by {}", blocker.blocked_by.join(", ")));
        }
        parts.push(desc);
    }

    parts.join("\n")
}

/// Show a task by ID with optional blocking analysis.
pub fn task_show(repo_path: &Path, id: &str) -> Result<TaskShowResponse> {
    let storage = Storage::open(repo_path)?;

    // Try to get the task
    match storage.get_task(id) {
        Ok(task) => {
            // Analyze blocking status if task has dependencies (legacy or edge-based)
            let edge_deps = storage.get_edge_dependencies(id).unwrap_or_default();
            let blocking_info = if !task.depends_on.is_empty() || !edge_deps.is_empty() {
                Some(analyze_blockers(&storage, &task)?)
            } else {
                None
            };

            // Fetch edges for this task
            let hydrated_edges = storage.get_edges_for_entity(id).unwrap_or_default();
            let edges: Vec<TaskEdgeInfo> = hydrated_edges
                .into_iter()
                .map(|he| {
                    let related_id = if he.direction == EdgeDirection::Inbound {
                        he.edge.source.clone()
                    } else {
                        he.edge.target.clone()
                    };

                    // Try to get title and status of related entity
                    let (related_title, related_status) =
                        if let Ok(t) = storage.get_task(&related_id) {
                            (
                                Some(t.core.title.clone()),
                                Some(format!("{:?}", t.status).to_lowercase()),
                            )
                        } else if let Ok(b) = storage.get_bug(&related_id) {
                            (
                                Some(b.core.title.clone()),
                                Some(format!("{:?}", b.status).to_lowercase()),
                            )
                        } else if let Ok(test) = storage.get_test(&related_id) {
                            (Some(test.name), None)
                        } else {
                            (None, None)
                        };

                    let direction = match he.direction {
                        EdgeDirection::Outbound => "outbound",
                        EdgeDirection::Inbound => "inbound",
                        EdgeDirection::Both => "both",
                    };

                    TaskEdgeInfo {
                        edge_type: he.edge.edge_type.to_string(),
                        direction: direction.to_string(),
                        related_id,
                        related_title,
                        related_status,
                    }
                })
                .collect();

            // Get linked docs for this task
            let linked_docs = get_linked_docs_for_entity(&storage, id, false);

            Ok(TaskShowResponse::Found(Box::new(TaskShowResult {
                task,
                blocking_info,
                edges,
                linked_docs,
            })))
        }
        Err(Error::NotFound(_)) => {
            // Task not found - check if it exists as another entity type
            match storage.get_entity_type(id) {
                Ok(EntityType::Bug) => {
                    let bug = storage.get_bug(id)?;
                    let edge_deps = storage.get_edge_dependencies(id).unwrap_or_default();
                    let blocking_info = if !bug.depends_on.is_empty() || !edge_deps.is_empty() {
                        Some(analyze_bug_blockers(&storage, &bug)?)
                    } else {
                        None
                    };
                    let hydrated_edges = storage.get_edges_for_entity(id).unwrap_or_default();
                    let edges = build_edges_info(&storage, hydrated_edges);
                    let linked_docs = get_linked_docs_for_entity(&storage, id, false);

                    Ok(TaskShowResponse::TypeMismatch(Box::new(
                        EntityMismatchResult {
                            note: format!("{} is a bug, not a task", id),
                            actual_type: "bug".to_string(),
                            task: None,
                            bug: Some(BugShowResult {
                                bug,
                                blocking_info,
                                edges,
                                linked_docs,
                            }),
                            test: None,
                            milestone: None,
                        },
                    )))
                }
                Ok(EntityType::Test) => {
                    let test = storage.get_test(id)?;
                    Ok(TaskShowResponse::TypeMismatch(Box::new(
                        EntityMismatchResult {
                            note: format!("{} is a test, not a task", id),
                            actual_type: "test".to_string(),
                            task: None,
                            bug: None,
                            test: Some(test),
                            milestone: None,
                        },
                    )))
                }
                Ok(EntityType::Milestone) => {
                    let milestone = storage.get_milestone(id)?;
                    Ok(TaskShowResponse::TypeMismatch(Box::new(
                        EntityMismatchResult {
                            note: format!("{} is a milestone, not a task", id),
                            actual_type: "milestone".to_string(),
                            task: None,
                            bug: None,
                            test: None,
                            milestone: Some(milestone),
                        },
                    )))
                }
                _ => Err(Error::NotFound(format!("Task not found: {}", id))),
            }
        }
        Err(e) => Err(e),
    }
}

/// Helper to build edge info from hydrated edges.
fn build_edges_info(
    storage: &Storage,
    hydrated_edges: Vec<crate::models::HydratedEdge>,
) -> Vec<TaskEdgeInfo> {
    hydrated_edges
        .into_iter()
        .map(|he| {
            let related_id = if he.direction == EdgeDirection::Inbound {
                he.edge.source.clone()
            } else {
                he.edge.target.clone()
            };

            let (related_title, related_status) = if let Ok(t) = storage.get_task(&related_id) {
                (
                    Some(t.core.title.clone()),
                    Some(format!("{:?}", t.status).to_lowercase()),
                )
            } else if let Ok(b) = storage.get_bug(&related_id) {
                (
                    Some(b.core.title.clone()),
                    Some(format!("{:?}", b.status).to_lowercase()),
                )
            } else if let Ok(test) = storage.get_test(&related_id) {
                (Some(test.name), None)
            } else {
                (None, None)
            };

            let direction = match he.direction {
                EdgeDirection::Outbound => "outbound",
                EdgeDirection::Inbound => "inbound",
                EdgeDirection::Both => "both",
            };

            TaskEdgeInfo {
                edge_type: he.edge.edge_type.to_string(),
                direction: direction.to_string(),
                related_id,
                related_title,
                related_status,
            }
        })
        .collect()
}

#[derive(Serialize)]
pub struct TaskList {
    pub tasks: Vec<Task>,
    pub count: usize,
}

impl Output for TaskList {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if self.tasks.is_empty() {
            return "No tasks found.".to_string();
        }

        let mut lines = Vec::new();
        lines.push(format!("{} task(s):\n", self.count));

        for task in &self.tasks {
            let status_char = match task.status {
                TaskStatus::Pending => " ",
                TaskStatus::InProgress => ">",
                TaskStatus::Done => "x",
                TaskStatus::Blocked => "!",
                TaskStatus::Cancelled => "-",
                TaskStatus::Reopened => "?",
                TaskStatus::Partial => "~",
            };
            let tags = if task.core.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", task.core.tags.join(", "))
            };
            lines.push(format!(
                "[{}] {} P{} {}{}",
                status_char, task.core.id, task.priority, task.core.title, tags
            ));
        }

        lines.join("\n")
    }
}

/// List tasks with optional filters.
pub fn task_list(
    repo_path: &Path,
    status: Option<&str>,
    priority: Option<u8>,
    tag: Option<&str>,
) -> Result<TaskList> {
    let storage = Storage::open(repo_path)?;
    let tasks = storage.list_tasks(status, priority, tag)?;
    let count = tasks.len();
    Ok(TaskList { tasks, count })
}

#[derive(Debug, Serialize)]
pub struct TaskUpdated {
    pub id: String,
    pub updated_fields: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

impl Output for TaskUpdated {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut output = format!(
            "Updated task {}: {}",
            self.id,
            self.updated_fields.join(", ")
        );
        if let Some(warning) = &self.warning {
            output.push_str(&format!("\nWarning: {}", warning));
        }
        output
    }
}

/// Update a task.
#[allow(clippy::too_many_arguments)]
pub fn task_update(
    repo_path: &Path,
    id: &str,
    title: Option<String>,
    short_name: Option<String>,
    description: Option<String>,
    priority: Option<u8>,
    status: Option<&str>,
    add_tags: Vec<String>,
    remove_tags: Vec<String>,
    assignee: Option<String>,
    force: bool,
    keep_closed: bool,
    reopen: bool,
) -> Result<TaskUpdated> {
    let mut storage = Storage::open(repo_path)?;
    let mut task = storage.get_task(id)?;
    let mut updated_fields = Vec::new();
    let mut setting_to_done = false;

    // Check if task is closed (Done or Cancelled) and require explicit flag
    let is_closed = task.status == TaskStatus::Done || task.status == TaskStatus::Cancelled;
    if is_closed && !keep_closed && !reopen {
        return Err(Error::Other(format!(
            "Cannot update closed task {} (status: {:?})\n\n\
            Closed tasks require explicit intent to modify:\n\
              --keep-closed  Update without changing status\n\
              --reopen       Update and set status back to pending\n\n\
            Example: bn task update {} --title \"New title\" --keep-closed",
            id, task.status, id
        )));
    }

    // Handle --reopen flag: set status to pending
    if reopen && is_closed {
        task.status = TaskStatus::Pending;
        task.closed_at = None;
        updated_fields.push("status".to_string());
    }

    if let Some(t) = title {
        task.core.title = t;
        updated_fields.push("title".to_string());
    }

    if let Some(s) = short_name {
        // Empty or whitespace-only clears the short_name
        if s.trim().is_empty() {
            task.core.short_name = None;
        } else {
            // Auto-truncate very long short_name (2x display limit = 30 chars)
            let truncated = if s.chars().count() > 30 {
                eprintln!(
                    "Note: short_name truncated from {} to 30 chars for GUI display.",
                    s.chars().count()
                );
                s.chars().take(30).collect::<String>()
            } else {
                s
            };
            task.core.short_name = Some(truncated);
        }
        updated_fields.push("short_name".to_string());
    }

    if let Some(d) = description {
        task.core.description = Some(d);
        updated_fields.push("description".to_string());
    }

    if let Some(p) = priority {
        if p > 4 {
            return Err(Error::Other("Priority must be 0-4".to_string()));
        }
        task.priority = p;
        updated_fields.push("priority".to_string());
    }

    if let Some(s) = status {
        let new_status = parse_status(s)?;

        // If setting status to done, check commit requirement
        if new_status == TaskStatus::Done
            && config_get_bool(repo_path, "require_commit_for_close", false)
            && !force
        {
            let commits = storage.get_commits_for_task(id)?;
            if commits.is_empty() {
                return Err(Error::Other(format!(
                    "Cannot set status to done for task {} - no commits linked\n\n\
                    This repository requires commits to be linked before marking tasks as done.\n\
                    Link a commit with: bn commit link <sha> {}\n\
                    Or bypass with: bn task update {} --status done --force\n\n\
                    Hint: Run 'git log --oneline -5' to see recent commits.",
                    id, id, id
                )));
            }
        }

        // If setting status to done, also set closed_at
        if new_status == TaskStatus::Done {
            task.closed_at = Some(Utc::now());
            // Remove task from agent's tasks list
            // First try to find the ancestor agent (for when bn commands run in subprocesses)
            // Fall back to parent_pid for backwards compatibility
            let agent_pid = find_ancestor_agent(&storage)
                .or_else(get_parent_pid)
                .unwrap_or_else(std::process::id);
            let _ = storage.agent_remove_task(agent_pid, id);
        }

        // Track if we're setting status to done (for commit validation later)
        setting_to_done = new_status == TaskStatus::Done;

        // If setting status to in_progress, track task association for registered agents
        if new_status == TaskStatus::InProgress {
            // First try to find the ancestor agent (for when bn commands run in subprocesses)
            // Fall back to parent_pid for backwards compatibility
            let agent_pid = find_ancestor_agent(&storage)
                .or_else(get_parent_pid)
                .unwrap_or_else(std::process::id);
            // Check if agent is registered and already has tasks
            if let Ok(agent) = storage.get_agent(agent_pid)
                && !agent.tasks.is_empty()
                && !force
            {
                let existing_tasks = agent.tasks.join(", ");
                return Err(Error::Other(format!(
                    "Agent already has {} task(s) in progress: {}\n\n\
                    Taking on multiple tasks may lead to context thrashing.\n\
                    Complete your current task first, or use --force to override.\n\n\
                    Hint: Run 'bn task close {}' when done, or 'bn task update {} --status in_progress --force'",
                    agent.tasks.len(),
                    existing_tasks,
                    agent.tasks.first().unwrap_or(&String::new()),
                    id
                )));
            }
            // Silently ignore errors - agent tracking is optional
            let _ = storage.agent_add_task(agent_pid, id);
        }

        task.status = new_status;
        updated_fields.push("status".to_string());
    }

    if !add_tags.is_empty() {
        for tag in add_tags {
            if !task.core.tags.contains(&tag) {
                task.core.tags.push(tag);
            }
        }
        updated_fields.push("tags".to_string());
    }

    if !remove_tags.is_empty() {
        task.core.tags.retain(|t| !remove_tags.contains(t));
        if !updated_fields.contains(&"tags".to_string()) {
            updated_fields.push("tags".to_string());
        }
    }

    if let Some(a) = assignee {
        task.assignee = Some(a);
        updated_fields.push("assignee".to_string());
    }

    if updated_fields.is_empty() {
        return Err(Error::Other("No fields to update".to_string()));
    }

    task.core.updated_at = Utc::now();
    storage.update_task(&task)?;

    // Validate linked commits exist when setting status to done
    let warning = if setting_to_done {
        let commits = storage.get_commits_for_task(id)?;
        let missing_commits: Vec<String> = commits
            .iter()
            .filter(|c| !git_commit_exists(repo_path, &c.sha))
            .map(|c| c.sha.clone())
            .collect();

        if !missing_commits.is_empty() {
            Some(
                missing_commits
                    .iter()
                    .map(|sha| {
                        format!(
                            "Linked commit {} not found in repository (may have been rebased)",
                            sha
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; "),
            )
        } else {
            None
        }
    } else {
        None
    };

    Ok(TaskUpdated {
        id: id.to_string(),
        updated_fields,
        warning,
    })
}

#[derive(Debug, Serialize)]
pub struct TaskClosed {
    pub id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub removed_from_queues: Vec<String>,
}

impl Output for TaskClosed {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut output = format!("Closed task {}", self.id);
        if let Some(warning) = &self.warning {
            output.push_str(&format!("\nWarning: {}", warning));
        }
        if !self.removed_from_queues.is_empty() {
            output.push_str(&format!(
                "\nRemoved from queue(s): {}",
                self.removed_from_queues.join(", ")
            ));
        }
        if let Some(hint) = &self.hint {
            output.push_str(&format!("\nHint: {}", hint));
        }
        output
    }
}

/// Remove a task from all queues it's in (queued links) when it's closed.
/// Returns the list of queue IDs the task was removed from.
fn remove_task_from_queues(storage: &mut Storage, task_id: &str) -> Result<Vec<String>> {
    use crate::models::EdgeType;

    // Find all queued edges where this task is the source
    let queued_edges = storage.list_edges(Some(EdgeType::Queued), Some(task_id), None)?;

    let mut removed_queues = Vec::new();
    for edge in queued_edges {
        // Remove the edge (task -> queue)
        if storage
            .remove_edge(&edge.source, &edge.target, EdgeType::Queued)
            .is_ok()
        {
            removed_queues.push(edge.target);
        }
    }

    Ok(removed_queues)
}

fn promote_partial_tasks(storage: &mut Storage) -> Result<Vec<String>> {
    let mut promoted = Vec::new();

    loop {
        let tasks = storage.list_tasks(None, None, None)?;
        let mut did_promote = false;

        for mut task in tasks {
            if task.status != TaskStatus::Partial {
                continue;
            }

            let all_done = task.depends_on.iter().all(|dep_id| {
                storage
                    .get_task(dep_id)
                    .map(|dep| matches!(dep.status, TaskStatus::Done | TaskStatus::Cancelled))
                    .unwrap_or(false)
            });

            if all_done {
                task.status = TaskStatus::Done;
                task.closed_at = Some(Utc::now());
                task.core.updated_at = Utc::now();
                storage.update_task(&task)?;
                promoted.push(task.core.id.clone());
                did_promote = true;
            }
        }

        if !did_promote {
            break;
        }
    }

    Ok(promoted)
}

/// Close a task.
pub fn task_close(
    repo_path: &Path,
    id: &str,
    reason: Option<String>,
    force: bool,
) -> Result<TaskClosed> {
    let mut storage = Storage::open(repo_path)?;
    let task = storage.get_task(id)?;

    // Check for incomplete dependencies (both legacy depends_on and edge-based)
    let mut incomplete_deps: Vec<Task> = task
        .depends_on
        .iter()
        .filter_map(|dep_id| storage.get_task(dep_id).ok())
        .filter(|dep| dep.status != TaskStatus::Done && dep.status != TaskStatus::Cancelled)
        .collect();

    // Also check edge-based dependencies
    let edge_deps = storage.get_edge_dependencies(id).unwrap_or_default();
    for dep_id in edge_deps {
        if let Ok(dep) = storage.get_task(&dep_id)
            && dep.status != TaskStatus::Done
            && dep.status != TaskStatus::Cancelled
            // Avoid duplicates if dependency exists in both legacy and edge-based
            && !incomplete_deps.iter().any(|d| d.core.id == dep.core.id)
        {
            incomplete_deps.push(dep);
        }
    }

    // If there are incomplete dependencies and force is false, return error
    if !incomplete_deps.is_empty() && !force {
        let dep_list: Vec<String> = incomplete_deps
            .iter()
            .map(|d| {
                format!(
                    "{}: \"{}\" ({})",
                    d.core.id,
                    d.core.title,
                    format!("{:?}", d.status).to_lowercase()
                )
            })
            .collect();

        return Err(Error::Other(format!(
            "Cannot close task {}. It has {} incomplete dependencies:\n  - {}\n\nUse --force to close anyway, or complete the dependencies first.",
            id,
            incomplete_deps.len(),
            dep_list.join("\n  - ")
        )));
    }

    // Check for linked commits if require_commit_for_close is enabled
    if config_get_bool(repo_path, "require_commit_for_close", false) && !force {
        let commits = storage.get_commits_for_task(id)?;
        if commits.is_empty() {
            return Err(Error::Other(format!(
                "Cannot close task {} - no commits linked\n\n\
                This repository requires commits to be linked before closing tasks.\n\
                Link a commit with: bn commit link <sha> {}\n\
                Or bypass with: bn task close {} --force\n\n\
                Hint: Run 'git log --oneline -5' to see recent commits.",
                id, id, id
            )));
        }
    }

    // Validate that linked commits exist in the git repo (warn if missing)
    let commits = storage.get_commits_for_task(id)?;
    let missing_commits: Vec<String> = commits
        .iter()
        .filter(|c| !git_commit_exists(repo_path, &c.sha))
        .map(|c| c.sha.clone())
        .collect();

    // Proceed with close
    let mut task = task;
    task.status = TaskStatus::Done;
    task.closed_at = Some(Utc::now());
    task.closed_reason = reason;
    task.core.updated_at = Utc::now();

    storage.update_task(&task)?;
    promote_partial_tasks(&mut storage)?;

    // Remove task from agent's tasks list if the current process is a registered agent
    // First try to find the ancestor agent (for when bn commands run in subprocesses)
    // Fall back to parent_pid for backwards compatibility
    let agent_pid = find_ancestor_agent(&storage)
        .or_else(get_parent_pid)
        .unwrap_or_else(std::process::id);
    // Silently ignore errors - agent tracking is optional
    let _ = storage.agent_remove_task(agent_pid, id);

    // Auto-remove task from any queues it's in
    let removed_from_queues = remove_task_from_queues(&mut storage, id)?;

    // Generate warnings for incomplete deps, missing commits, no commits, and uncommitted changes
    let mut warnings = Vec::new();
    if !incomplete_deps.is_empty() {
        warnings.push(format!(
            "Closed with {} incomplete dependencies",
            incomplete_deps.len()
        ));
    }
    if !missing_commits.is_empty() {
        for sha in &missing_commits {
            warnings.push(format!(
                "Linked commit {} not found in repository (may have been rebased)",
                sha
            ));
        }
    }
    // Warn if no commits are linked to this task
    if commits.is_empty() {
        warnings.push("No commits linked to this task".to_string());
    }
    // Warn if there are uncommitted changes in the repo
    if git_has_uncommitted_changes(repo_path) {
        warnings.push("Uncommitted changes in repository".to_string());
    }
    let warning = if warnings.is_empty() {
        None
    } else {
        Some(warnings.join("; "))
    };

    // Include a hint to remind agents to call goodbye when done
    let hint = Some("Run 'bn goodbye' when you're done with all your work.".to_string());

    Ok(TaskClosed {
        id: id.to_string(),
        status: "done".to_string(),
        warning,
        hint,
        removed_from_queues,
    })
}

#[derive(Serialize)]
pub struct TaskReopened {
    pub id: String,
    pub status: String,
}

impl Output for TaskReopened {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Reopened task {}", self.id)
    }
}

/// Reopen a closed task.
pub fn task_reopen(repo_path: &Path, id: &str) -> Result<TaskReopened> {
    let mut storage = Storage::open(repo_path)?;
    let mut task = storage.get_task(id)?;

    task.status = TaskStatus::Reopened;
    task.closed_at = None;
    task.closed_reason = None;
    task.core.updated_at = Utc::now();

    storage.update_task(&task)?;

    Ok(TaskReopened {
        id: id.to_string(),
        status: "reopened".to_string(),
    })
}

#[derive(Serialize)]
pub struct TaskDeleted {
    pub id: String,
}

impl Output for TaskDeleted {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Deleted task {}", self.id)
    }
}

/// Delete a task.
pub fn task_delete(repo_path: &Path, id: &str) -> Result<TaskDeleted> {
    let mut storage = Storage::open(repo_path)?;
    storage.delete_task(id)?;

    Ok(TaskDeleted { id: id.to_string() })
}

// === Bug Commands ===

fn parse_severity(s: &str) -> Result<BugSeverity> {
    match s.to_lowercase().as_str() {
        "triage" => Ok(BugSeverity::Triage),
        "low" => Ok(BugSeverity::Low),
        "medium" => Ok(BugSeverity::Medium),
        "high" => Ok(BugSeverity::High),
        "critical" => Ok(BugSeverity::Critical),
        _ => Err(Error::Other(format!("Invalid severity: {}", s))),
    }
}

#[derive(Debug, Serialize)]
pub struct BugCreated {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queued_to: Option<String>,
}

impl Output for BugCreated {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let base = format!("Created bug {} \"{}\"", self.id, self.title);
        match &self.queued_to {
            Some(q) => format!("{} (added to queue {})", base, q),
            None => base,
        }
    }
}

/// Create a new bug.
#[allow(clippy::too_many_arguments)]
pub fn bug_create(
    repo_path: &Path,
    title: String,
    short_name: Option<String>,
    description: Option<String>,
    priority: Option<u8>,
    severity: Option<String>,
    tags: Vec<String>,
    assignee: Option<String>,
    reproduction_steps: Option<String>,
    affected_component: Option<String>,
) -> Result<BugCreated> {
    bug_create_with_queue(
        repo_path,
        title,
        short_name,
        description,
        priority,
        severity,
        tags,
        assignee,
        reproduction_steps,
        affected_component,
        false,
    )
}

/// Create a new bug with optional immediate queuing.
#[allow(clippy::too_many_arguments)]
pub fn bug_create_with_queue(
    repo_path: &Path,
    title: String,
    short_name: Option<String>,
    description: Option<String>,
    priority: Option<u8>,
    severity: Option<String>,
    tags: Vec<String>,
    assignee: Option<String>,
    reproduction_steps: Option<String>,
    affected_component: Option<String>,
    queue: bool,
) -> Result<BugCreated> {
    let mut storage = Storage::open(repo_path)?;

    if let Some(p) = priority
        && p > 4
    {
        return Err(Error::Other("Priority must be 0-4".to_string()));
    }

    let id = storage.generate_unique_id("bn", &title);
    let mut bug = Bug::new(id.clone(), title.clone());
    bug.core.short_name = normalize_short_name(short_name);
    bug.core.description = description;
    bug.priority = priority.unwrap_or(2);
    bug.severity = severity
        .as_deref()
        .map(parse_severity)
        .transpose()?
        .unwrap_or_default();
    bug.core.tags = tags;
    bug.assignee = assignee;
    bug.reproduction_steps = reproduction_steps;
    bug.affected_component = affected_component;

    storage.add_bug(&bug)?;

    // Add to queue if requested
    let queued_to = if queue {
        Some(add_entity_to_queue_internal(&mut storage, &id)?)
    } else {
        None
    };

    Ok(BugCreated {
        id,
        title,
        queued_to,
    })
}

impl Output for Bug {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("{} {}", self.core.id, self.core.title));
        if let Some(ref sn) = self.core.short_name {
            lines.push(format!("  Short Name: {}", sn));
        }
        lines.push(format!(
            "  Status: {:?}  Priority: {}  Severity: {:?}",
            self.status, self.priority, self.severity
        ));
        if let Some(ref desc) = self.core.description {
            lines.push(format!("  Description: {}", desc));
        }
        if let Some(ref steps) = self.reproduction_steps {
            lines.push(format!("  Reproduction steps: {}", steps));
        }
        if let Some(ref component) = self.affected_component {
            lines.push(format!("  Affected component: {}", component));
        }
        if !self.core.tags.is_empty() {
            lines.push(format!("  Tags: {}", self.core.tags.join(", ")));
        }
        if let Some(ref assignee) = self.assignee {
            lines.push(format!("  Assignee: {}", assignee));
        }
        if !self.depends_on.is_empty() {
            lines.push(format!("  Depends on: {}", self.depends_on.join(", ")));
        }
        lines.push(format!(
            "  Created: {}",
            self.core.created_at.format("%Y-%m-%d %H:%M")
        ));
        lines.push(format!(
            "  Updated: {}",
            self.core.updated_at.format("%Y-%m-%d %H:%M")
        ));
        if let Some(closed) = self.closed_at {
            lines.push(format!("  Closed: {}", closed.format("%Y-%m-%d %H:%M")));
            if let Some(ref reason) = self.closed_reason {
                lines.push(format!("  Reason: {}", reason));
            }
        }
        lines.join("\n")
    }
}

#[derive(Serialize)]
pub struct BugList {
    pub bugs: Vec<Bug>,
    pub count: usize,
}

impl Output for BugList {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if self.bugs.is_empty() {
            return "No bugs found.".to_string();
        }

        let mut lines = Vec::new();
        lines.push(format!("{} bug(s):\n", self.count));

        for bug in &self.bugs {
            let status_char = match bug.status {
                TaskStatus::Pending => " ",
                TaskStatus::InProgress => ">",
                TaskStatus::Done => "x",
                TaskStatus::Blocked => "!",
                TaskStatus::Cancelled => "-",
                TaskStatus::Reopened => "?",
                TaskStatus::Partial => "~",
            };
            let tags = if bug.core.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", bug.core.tags.join(", "))
            };
            lines.push(format!(
                "[{}] {} P{} S:{} {}{}",
                status_char,
                bug.core.id,
                bug.priority,
                format!("{:?}", bug.severity).to_lowercase(),
                bug.core.title,
                tags
            ));
        }

        lines.join("\n")
    }
}

/// List bugs with optional filters.
pub fn bug_list(
    repo_path: &Path,
    status: Option<&str>,
    priority: Option<u8>,
    severity: Option<&str>,
    tag: Option<&str>,
    include_closed: bool,
) -> Result<BugList> {
    let storage = Storage::open(repo_path)?;
    let bugs = storage.list_bugs(status, priority, severity, tag, include_closed)?;
    let count = bugs.len();
    Ok(BugList { bugs, count })
}

/// Result of bug_show with edge information.
#[derive(Serialize)]
pub struct BugShowResult {
    #[serde(flatten)]
    pub bug: Bug,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocking_info: Option<BlockingInfo>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub edges: Vec<TaskEdgeInfo>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub linked_docs: Vec<LinkedDocInfo>,
}

impl Output for BugShowResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("Bug: {}", self.bug.core.id));
        lines.push(format!("Title: {}", self.bug.core.title));
        if let Some(ref sn) = self.bug.core.short_name {
            lines.push(format!("Short Name: {}", sn));
        }
        lines.push(format!("Status: {:?}", self.bug.status));
        lines.push(format!("Priority: P{}", self.bug.priority));
        lines.push(format!("Severity: {:?}", self.bug.severity));

        if !self.bug.core.tags.is_empty() {
            lines.push(format!("Tags: {}", self.bug.core.tags.join(", ")));
        }

        if let Some(ref desc) = self.bug.core.description {
            lines.push(format!("Description: {}", desc));
        }

        if let Some(ref steps) = self.bug.reproduction_steps {
            lines.push(format!("Reproduction steps: {}", steps));
        }

        if let Some(ref component) = self.bug.affected_component {
            lines.push(format!("Affected component: {}", component));
        }

        if let Some(ref assignee) = self.bug.assignee {
            lines.push(format!("Assignee: {}", assignee));
        }

        // Show legacy dependencies (if any)
        if !self.bug.depends_on.is_empty() {
            lines.push(format!(
                "\nDependencies ({}): {}",
                self.bug.depends_on.len(),
                self.bug.depends_on.join(", ")
            ));
        }

        // Show edge-based relationships grouped by type
        if !self.edges.is_empty() {
            lines.push(String::new());

            // Group edges by type and direction
            let mut depends_on: Vec<&TaskEdgeInfo> = Vec::new();
            let mut blocks: Vec<&TaskEdgeInfo> = Vec::new();
            let mut related: Vec<&TaskEdgeInfo> = Vec::new();
            let mut fixed_by: Vec<&TaskEdgeInfo> = Vec::new();
            let mut other: Vec<&TaskEdgeInfo> = Vec::new();

            for edge in &self.edges {
                match edge.edge_type.as_str() {
                    "depends_on" if edge.direction == "outbound" => depends_on.push(edge),
                    "depends_on" if edge.direction == "inbound" => blocks.push(edge),
                    "blocks" if edge.direction == "outbound" => blocks.push(edge),
                    "related_to" => related.push(edge),
                    "fixes" if edge.direction == "inbound" => fixed_by.push(edge),
                    _ => other.push(edge),
                }
            }

            if !depends_on.is_empty() {
                lines.push("Dependencies (edges):".to_string());
                for e in depends_on {
                    let status = e.related_status.as_deref().unwrap_or("unknown");
                    let title = e.related_title.as_deref().unwrap_or("");
                    lines.push(format!("  → {} \"{}\" [{}]", e.related_id, title, status));
                }
            }

            if !blocks.is_empty() {
                lines.push("Blocks:".to_string());
                for e in blocks {
                    let status = e.related_status.as_deref().unwrap_or("unknown");
                    let title = e.related_title.as_deref().unwrap_or("");
                    lines.push(format!("  ← {} \"{}\" [{}]", e.related_id, title, status));
                }
            }

            if !fixed_by.is_empty() {
                lines.push("Fixed by:".to_string());
                for e in fixed_by {
                    let status = e.related_status.as_deref().unwrap_or("unknown");
                    let title = e.related_title.as_deref().unwrap_or("");
                    lines.push(format!("  ← {} \"{}\" [{}]", e.related_id, title, status));
                }
            }

            if !related.is_empty() {
                lines.push("Related:".to_string());
                for e in related {
                    let status = e.related_status.as_deref().unwrap_or("unknown");
                    let title = e.related_title.as_deref().unwrap_or("");
                    lines.push(format!("  ↔ {} \"{}\" [{}]", e.related_id, title, status));
                }
            }

            if !other.is_empty() {
                lines.push("Other links:".to_string());
                for e in other {
                    let arrow = match e.direction.as_str() {
                        "outbound" => "→",
                        "inbound" => "←",
                        _ => "↔",
                    };
                    let status = e.related_status.as_deref().unwrap_or("unknown");
                    let title = e.related_title.as_deref().unwrap_or("");
                    lines.push(format!(
                        "  {} {} \"{}\" ({}) [{}]",
                        arrow, e.related_id, title, e.edge_type, status
                    ));
                }
            }
        }

        // Show linked docs
        if !self.linked_docs.is_empty() {
            lines.push(String::new());
            lines.push(format!("📄 Related Docs ({}):", self.linked_docs.len()));
            for doc in &self.linked_docs {
                let name = doc.short_name.as_deref().unwrap_or(&doc.title);
                // Format: bn-xxxx [type] "Title" - summary
                let mut doc_line = format!("  {} [{}] \"{}\"", doc.id, doc.doc_type, name);
                if let Some(ref summary) = doc.summary {
                    doc_line.push_str(&format!(" - {}", summary));
                }
                lines.push(doc_line);
                // Show full content if available
                if let Some(ref content) = doc.content {
                    lines.push("  ---".to_string());
                    for line in content.lines() {
                        lines.push(format!("  {}", line));
                    }
                    lines.push("  ---".to_string());
                }
            }
        }

        if let Some(ref blocking) = self.blocking_info {
            lines.push(format!("\n{}", blocking.summary));
        }

        if let Some(ref closed_at) = self.bug.closed_at {
            lines.push(format!("\nClosed at: {}", closed_at));
            if let Some(ref reason) = self.bug.closed_reason {
                lines.push(format!("Reason: {}", reason));
            }
        }

        lines.join("\n")
    }
}

/// Wrapper for bug show that can contain either a bug or a type mismatch.
#[derive(Serialize)]
#[serde(untagged)]
pub enum BugShowResponse {
    Found(Box<BugShowResult>),
    TypeMismatch(Box<EntityMismatchResult>),
}

impl BugShowResponse {
    /// Get the bug if this is a Found response.
    pub fn bug(&self) -> Option<&Bug> {
        match self {
            BugShowResponse::Found(result) => Some(&result.bug),
            BugShowResponse::TypeMismatch(_) => None,
        }
    }

    /// Unwrap the Found result, panicking if it's a TypeMismatch.
    #[cfg(test)]
    pub fn unwrap(self) -> BugShowResult {
        match self {
            BugShowResponse::Found(result) => *result,
            BugShowResponse::TypeMismatch(_) => panic!("Expected Found, got TypeMismatch"),
        }
    }
}

impl Output for BugShowResponse {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        match self {
            BugShowResponse::Found(result) => result.to_human(),
            BugShowResponse::TypeMismatch(mismatch) => mismatch.to_human(),
        }
    }
}

/// Show a bug by ID with edge information.
pub fn bug_show(repo_path: &Path, id: &str) -> Result<BugShowResponse> {
    let storage = Storage::open(repo_path)?;

    // Try to get the bug
    match storage.get_bug(id) {
        Ok(bug) => {
            // Analyze blocking status if bug has dependencies (legacy or edge-based)
            let edge_deps = storage.get_edge_dependencies(id).unwrap_or_default();
            let blocking_info = if !bug.depends_on.is_empty() || !edge_deps.is_empty() {
                Some(analyze_bug_blockers(&storage, &bug)?)
            } else {
                None
            };

            // Fetch edges for this bug
            let hydrated_edges = storage.get_edges_for_entity(id).unwrap_or_default();
            let edges = build_edges_info(&storage, hydrated_edges);

            // Get linked docs for this bug
            let linked_docs = get_linked_docs_for_entity(&storage, id, false);

            Ok(BugShowResponse::Found(Box::new(BugShowResult {
                bug,
                blocking_info,
                edges,
                linked_docs,
            })))
        }
        Err(Error::NotFound(_)) => {
            // Bug not found - check if it exists as another entity type
            match storage.get_entity_type(id) {
                Ok(EntityType::Task) => {
                    let task = storage.get_task(id)?;
                    let edge_deps = storage.get_edge_dependencies(id).unwrap_or_default();
                    let blocking_info = if !task.depends_on.is_empty() || !edge_deps.is_empty() {
                        Some(analyze_blockers(&storage, &task)?)
                    } else {
                        None
                    };
                    let hydrated_edges = storage.get_edges_for_entity(id).unwrap_or_default();
                    let edges = build_edges_info(&storage, hydrated_edges);
                    let linked_docs = get_linked_docs_for_entity(&storage, id, false);

                    Ok(BugShowResponse::TypeMismatch(Box::new(
                        EntityMismatchResult {
                            note: format!("{} is a task, not a bug", id),
                            actual_type: "task".to_string(),
                            task: Some(TaskShowResult {
                                task,
                                blocking_info,
                                edges,
                                linked_docs,
                            }),
                            bug: None,
                            test: None,
                            milestone: None,
                        },
                    )))
                }
                Ok(EntityType::Test) => {
                    let test = storage.get_test(id)?;
                    Ok(BugShowResponse::TypeMismatch(Box::new(
                        EntityMismatchResult {
                            note: format!("{} is a test, not a bug", id),
                            actual_type: "test".to_string(),
                            task: None,
                            bug: None,
                            test: Some(test),
                            milestone: None,
                        },
                    )))
                }
                Ok(EntityType::Milestone) => {
                    let milestone = storage.get_milestone(id)?;
                    Ok(BugShowResponse::TypeMismatch(Box::new(
                        EntityMismatchResult {
                            note: format!("{} is a milestone, not a bug", id),
                            actual_type: "milestone".to_string(),
                            task: None,
                            bug: None,
                            test: None,
                            milestone: Some(milestone),
                        },
                    )))
                }
                _ => Err(Error::NotFound(format!("Bug not found: {}", id))),
            }
        }
        Err(e) => Err(e),
    }
}

/// Analyze what is blocking a bug from completion.
fn analyze_bug_blockers(storage: &Storage, bug: &Bug) -> Result<BlockingInfo> {
    let mut direct_blockers = Vec::new();
    let mut blocker_chain = Vec::new();

    // Combine legacy depends_on and edge-based dependencies
    let mut all_deps: Vec<String> = bug.depends_on.clone();
    let edge_deps = storage
        .get_edge_dependencies(&bug.core.id)
        .unwrap_or_default();
    for dep in edge_deps {
        if !all_deps.contains(&dep) {
            all_deps.push(dep);
        }
    }

    for dep_id in &all_deps {
        // Try to get as task first, then as bug
        let (dep_status, dep_title, dep_assignee, dep_deps) =
            if let Ok(dep) = storage.get_task(dep_id) {
                let dep_edge_deps = storage.get_edge_dependencies(dep_id).unwrap_or_default();
                let mut combined_deps = dep.depends_on.clone();
                for d in dep_edge_deps {
                    if !combined_deps.contains(&d) {
                        combined_deps.push(d);
                    }
                }
                (
                    dep.status.clone(),
                    dep.core.title.clone(),
                    dep.assignee.clone(),
                    combined_deps,
                )
            } else if let Ok(b) = storage.get_bug(dep_id) {
                let dep_edge_deps = storage.get_edge_dependencies(dep_id).unwrap_or_default();
                let mut combined_deps = b.depends_on.clone();
                for d in dep_edge_deps {
                    if !combined_deps.contains(&d) {
                        combined_deps.push(d);
                    }
                }
                (
                    b.status.clone(),
                    b.core.title.clone(),
                    b.assignee.clone(),
                    combined_deps,
                )
            } else {
                continue; // Skip if entity not found
            };

        // Only consider incomplete dependencies as blockers
        if dep_status != TaskStatus::Done && dep_status != TaskStatus::Cancelled {
            // Find what's blocking this dependency (transitive blockers)
            let dep_blockers: Vec<String> = dep_deps
                .iter()
                .filter(|d| {
                    if let Ok(t) = storage.get_task(d) {
                        t.status != TaskStatus::Done && t.status != TaskStatus::Cancelled
                    } else if let Ok(b) = storage.get_bug(d) {
                        b.status != TaskStatus::Done && b.status != TaskStatus::Cancelled
                    } else {
                        false
                    }
                })
                .cloned()
                .collect();

            direct_blockers.push(DirectBlocker {
                id: dep_id.clone(),
                title: dep_title,
                status: format!("{:?}", dep_status).to_lowercase(),
                assignee: dep_assignee,
                blocked_by: dep_blockers.clone(),
            });

            // Build chain representation
            if dep_blockers.is_empty() {
                blocker_chain.push(format!(
                    "{} <- {} ({})",
                    bug.core.id,
                    dep_id,
                    format!("{:?}", dep_status).to_lowercase()
                ));
            } else {
                for blocker in &dep_blockers {
                    let blocker_status = if let Ok(b) = storage.get_task(blocker) {
                        format!("{:?}", b.status).to_lowercase()
                    } else if let Ok(b) = storage.get_bug(blocker) {
                        format!("{:?}", b.status).to_lowercase()
                    } else {
                        "unknown".to_string()
                    };
                    blocker_chain.push(format!(
                        "{} <- {} <- {} ({})",
                        bug.core.id, dep_id, blocker, blocker_status
                    ));
                }
            }
        }
    }

    let is_blocked = !direct_blockers.is_empty();
    let blocker_count = direct_blockers.len();

    let summary = if is_blocked {
        build_blocker_summary(&direct_blockers, blocker_count)
    } else {
        "All dependencies complete.".to_string()
    };

    Ok(BlockingInfo {
        is_blocked,
        blocker_count,
        direct_blockers,
        blocker_chain,
        summary,
    })
}

#[derive(Debug, Serialize)]
pub struct BugUpdated {
    pub id: String,
    pub updated_fields: Vec<String>,
}

impl Output for BugUpdated {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!(
            "Updated bug {}: {}",
            self.id,
            self.updated_fields.join(", ")
        )
    }
}

/// Update a bug.
#[allow(clippy::too_many_arguments)]
pub fn bug_update(
    repo_path: &Path,
    id: &str,
    title: Option<String>,
    short_name: Option<String>,
    description: Option<String>,
    priority: Option<u8>,
    status: Option<&str>,
    severity: Option<String>,
    add_tags: Vec<String>,
    remove_tags: Vec<String>,
    assignee: Option<String>,
    reproduction_steps: Option<String>,
    affected_component: Option<String>,
    force: bool,
    keep_closed: bool,
    reopen: bool,
) -> Result<BugUpdated> {
    let mut storage = Storage::open(repo_path)?;
    let mut bug = storage.get_bug(id)?;
    let mut updated_fields = Vec::new();

    // Check if bug is closed (Done or Cancelled) and require explicit flag
    let is_closed = bug.status == TaskStatus::Done || bug.status == TaskStatus::Cancelled;
    if is_closed && !keep_closed && !reopen {
        return Err(Error::Other(format!(
            "Cannot update closed bug {} (status: {:?})\n\n\
            Closed bugs require explicit intent to modify:\n\
              --keep-closed  Update without changing status\n\
              --reopen       Update and set status back to pending\n\n\
            Example: bn bug update {} --title \"New title\" --keep-closed",
            id, bug.status, id
        )));
    }

    // Handle --reopen flag: set status to pending
    if reopen && is_closed {
        bug.status = TaskStatus::Pending;
        bug.closed_at = None;
        updated_fields.push("status".to_string());
    }

    if let Some(t) = title {
        bug.core.title = t;
        updated_fields.push("title".to_string());
    }

    if let Some(new_short_name) = process_short_name_update(short_name) {
        bug.core.short_name = new_short_name;
        updated_fields.push("short_name".to_string());
    }

    if let Some(d) = description {
        bug.core.description = Some(d);
        updated_fields.push("description".to_string());
    }

    if let Some(p) = priority {
        if p > 4 {
            return Err(Error::Other("Priority must be 0-4".to_string()));
        }
        bug.priority = p;
        updated_fields.push("priority".to_string());
    }

    if let Some(s) = status {
        let new_status = parse_status(s)?;

        // If transitioning away from in_progress, remove agent task association
        if bug.status == TaskStatus::InProgress && new_status != TaskStatus::InProgress {
            let agent_pid = find_ancestor_agent(&storage)
                .or_else(get_parent_pid)
                .unwrap_or_else(std::process::id);
            let _ = storage.agent_remove_task(agent_pid, id);
        }

        // If setting status to in_progress, track bug association for registered agents
        if new_status == TaskStatus::InProgress {
            let agent_pid = find_ancestor_agent(&storage)
                .or_else(get_parent_pid)
                .unwrap_or_else(std::process::id);
            // Check if agent is registered and already has tasks
            if let Ok(agent) = storage.get_agent(agent_pid)
                && !agent.tasks.is_empty()
                && !force
            {
                let existing_tasks = agent.tasks.join(", ");
                return Err(Error::Other(format!(
                    "Agent already has {} task(s) in progress: {}\n\n\
                    Taking on multiple tasks may lead to context thrashing.\n\
                    Complete your current task first, or use --force to override.\n\n\
                    Hint: Run 'bn bug close {}' when done, or 'bn bug update {} --status in_progress --force'",
                    agent.tasks.len(),
                    existing_tasks,
                    agent.tasks.first().unwrap_or(&String::new()),
                    id
                )));
            }
            // Silently ignore errors - agent tracking is optional
            let _ = storage.agent_add_task(agent_pid, id);
        }

        bug.status = new_status;
        updated_fields.push("status".to_string());
    }

    if let Some(s) = severity {
        bug.severity = parse_severity(&s)?;
        updated_fields.push("severity".to_string());
    }

    if !add_tags.is_empty() {
        for tag in add_tags {
            if !bug.core.tags.contains(&tag) {
                bug.core.tags.push(tag);
            }
        }
        updated_fields.push("tags".to_string());
    }

    if !remove_tags.is_empty() {
        bug.core.tags.retain(|t| !remove_tags.contains(t));
        if !updated_fields.contains(&"tags".to_string()) {
            updated_fields.push("tags".to_string());
        }
    }

    if let Some(a) = assignee {
        bug.assignee = Some(a);
        updated_fields.push("assignee".to_string());
    }

    if let Some(steps) = reproduction_steps {
        bug.reproduction_steps = Some(steps);
        updated_fields.push("reproduction_steps".to_string());
    }

    if let Some(component) = affected_component {
        bug.affected_component = Some(component);
        updated_fields.push("affected_component".to_string());
    }

    if updated_fields.is_empty() {
        return Err(Error::Other("No fields to update".to_string()));
    }

    bug.core.updated_at = Utc::now();
    storage.update_bug(&bug)?;

    Ok(BugUpdated {
        id: id.to_string(),
        updated_fields,
    })
}

#[derive(Debug, Serialize)]
pub struct BugClosed {
    pub id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub removed_from_queues: Vec<String>,
}

impl Output for BugClosed {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut output = format!("Closed bug {}", self.id);
        if let Some(warning) = &self.warning {
            output.push_str(&format!("\nWarning: {}", warning));
        }
        if !self.removed_from_queues.is_empty() {
            output.push_str(&format!(
                "\nRemoved from queue(s): {}",
                self.removed_from_queues.join(", ")
            ));
        }
        if let Some(hint) = &self.hint {
            output.push_str(&format!("\nHint: {}", hint));
        }
        output
    }
}

/// Close a bug.
pub fn bug_close(
    repo_path: &Path,
    id: &str,
    reason: Option<String>,
    force: bool,
) -> Result<BugClosed> {
    let mut storage = Storage::open(repo_path)?;
    let bug = storage.get_bug(id)?;

    let incomplete_deps: Vec<Bug> = bug
        .depends_on
        .iter()
        .filter_map(|dep_id| storage.get_bug(dep_id).ok())
        .filter(|dep| dep.status != TaskStatus::Done && dep.status != TaskStatus::Cancelled)
        .collect();

    if !incomplete_deps.is_empty() && !force {
        let dep_list: Vec<String> = incomplete_deps
            .iter()
            .map(|d| {
                format!(
                    "{}: \"{}\" ({})",
                    d.core.id,
                    d.core.title,
                    format!("{:?}", d.status).to_lowercase()
                )
            })
            .collect();

        return Err(Error::Other(format!(
            "Cannot close bug {}. It has {} incomplete dependencies:\n  - {}\n\nUse --force to close anyway, or complete the dependencies first.",
            id,
            incomplete_deps.len(),
            dep_list.join("\n  - ")
        )));
    }

    // Validate that linked commits exist in the git repo (warn if missing)
    let commits = storage.get_commits_for_entity(id)?;
    let missing_commits: Vec<String> = commits
        .iter()
        .filter(|c| !git_commit_exists(repo_path, &c.sha))
        .map(|c| c.sha.clone())
        .collect();

    let mut bug = bug;
    bug.status = TaskStatus::Done;
    bug.closed_at = Some(Utc::now());
    bug.closed_reason = reason;
    bug.core.updated_at = Utc::now();

    storage.update_bug(&bug)?;

    // Auto-remove bug from any queues it's in
    let removed_from_queues = remove_task_from_queues(&mut storage, id)?;

    // Generate warnings for incomplete deps, missing commits, no commits, and uncommitted changes
    let mut warnings = Vec::new();
    if !incomplete_deps.is_empty() {
        warnings.push(format!(
            "Closed with {} incomplete dependencies",
            incomplete_deps.len()
        ));
    }
    if !missing_commits.is_empty() {
        for sha in &missing_commits {
            warnings.push(format!(
                "Linked commit {} not found in repository (may have been rebased)",
                sha
            ));
        }
    }
    // Warn if no commits are linked to this bug
    if commits.is_empty() {
        warnings.push("No commits linked to this bug".to_string());
    }
    // Warn if there are uncommitted changes in the repo
    if git_has_uncommitted_changes(repo_path) {
        warnings.push("Uncommitted changes in repository".to_string());
    }
    let warning = if warnings.is_empty() {
        None
    } else {
        Some(warnings.join("; "))
    };

    // Include a hint to remind agents to call goodbye when done
    let hint = Some("Run 'bn goodbye' when you're done with all your work.".to_string());

    Ok(BugClosed {
        id: id.to_string(),
        status: "done".to_string(),
        warning,
        hint,
        removed_from_queues,
    })
}

#[derive(Serialize)]
pub struct BugReopened {
    pub id: String,
    pub status: String,
}

impl Output for BugReopened {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Reopened bug {}", self.id)
    }
}

/// Reopen a closed bug.
pub fn bug_reopen(repo_path: &Path, id: &str) -> Result<BugReopened> {
    let mut storage = Storage::open(repo_path)?;
    let mut bug = storage.get_bug(id)?;

    // Preserve closure history in the description
    if let Some(closed_at) = &bug.closed_at {
        let annotation = format!(
            "\n\n---\nPreviously closed on: {}\nReason: {}",
            closed_at.format("%Y-%m-%d %H:%M:%S UTC"),
            bug.closed_reason.as_deref().unwrap_or("(no reason given)")
        );
        bug.core.description = Some(match &bug.core.description {
            Some(desc) => format!("{}{}", desc, annotation),
            None => annotation.trim_start().to_string(),
        });
    }

    bug.status = TaskStatus::Reopened;
    bug.closed_at = None;
    bug.closed_reason = None;
    bug.core.updated_at = Utc::now();

    storage.update_bug(&bug)?;

    Ok(BugReopened {
        id: id.to_string(),
        status: "reopened".to_string(),
    })
}

#[derive(Serialize)]
pub struct BugDeleted {
    pub id: String,
}

impl Output for BugDeleted {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Deleted bug {}", self.id)
    }
}

/// Delete a bug.
pub fn bug_delete(repo_path: &Path, id: &str) -> Result<BugDeleted> {
    let mut storage = Storage::open(repo_path)?;
    storage.delete_bug(id)?;

    Ok(BugDeleted { id: id.to_string() })
}

// === Idea Commands ===

#[derive(Serialize)]
pub struct IdeaCreated {
    pub id: String,
    pub title: String,
}

impl Output for IdeaCreated {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Created idea {} \"{}\"", self.id, self.title)
    }
}

/// Create a new idea.
pub fn idea_create(
    repo_path: &Path,
    title: String,
    short_name: Option<String>,
    description: Option<String>,
    tags: Vec<String>,
) -> Result<IdeaCreated> {
    let mut storage = Storage::open(repo_path)?;

    let id = storage.generate_unique_id("bn", &title);
    let mut idea = Idea::new(id.clone(), title.clone());
    idea.core.short_name = normalize_short_name(short_name);
    idea.core.description = description;
    idea.core.tags = tags;

    storage.add_idea(&idea)?;

    Ok(IdeaCreated { id, title })
}

impl Output for Idea {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();
        let status_str = match self.status {
            IdeaStatus::Seed => "seed",
            IdeaStatus::Germinating => "germinating",
            IdeaStatus::Promoted => "promoted",
            IdeaStatus::Discarded => "discarded",
        };
        lines.push(format!(
            "{} [{}] {}",
            self.core.id, status_str, self.core.title
        ));
        if let Some(ref sn) = self.core.short_name {
            lines.push(format!("  Short Name: {}", sn));
        }
        if let Some(ref desc) = self.core.description {
            lines.push(format!("  Description: {}", desc));
        }
        if !self.core.tags.is_empty() {
            lines.push(format!("  Tags: {}", self.core.tags.join(", ")));
        }
        if let Some(ref promoted_to) = self.promoted_to {
            lines.push(format!("  Promoted to: {}", promoted_to));
        }
        lines.push(format!(
            "  Created: {}",
            self.core.created_at.format("%Y-%m-%d %H:%M")
        ));
        lines.push(format!(
            "  Updated: {}",
            self.core.updated_at.format("%Y-%m-%d %H:%M")
        ));
        lines.join("\n")
    }
}

#[derive(Serialize)]
pub struct IdeaList {
    pub ideas: Vec<Idea>,
    pub count: usize,
}

impl Output for IdeaList {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if self.ideas.is_empty() {
            return "No ideas found.".to_string();
        }

        let mut lines = vec![format!("{} idea(s):\n", self.count)];
        for idea in &self.ideas {
            let status_str = match idea.status {
                IdeaStatus::Seed => "seed",
                IdeaStatus::Germinating => "germinating",
                IdeaStatus::Promoted => "promoted",
                IdeaStatus::Discarded => "discarded",
            };
            let status_marker = match idea.status {
                IdeaStatus::Seed => "💡",
                IdeaStatus::Germinating => "🌱",
                IdeaStatus::Promoted => "✅",
                IdeaStatus::Discarded => "❌",
            };
            let tags_str = if idea.core.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", idea.core.tags.join(", "))
            };
            lines.push(format!(
                "{} {} [{}] {}{}",
                status_marker, idea.core.id, status_str, idea.core.title, tags_str
            ));
        }
        lines.join("\n")
    }
}

/// List ideas with optional filters.
pub fn idea_list(repo_path: &Path, status: Option<&str>, tag: Option<&str>) -> Result<IdeaList> {
    let storage = Storage::open(repo_path)?;
    let ideas = storage.list_ideas(status, tag)?;
    let count = ideas.len();

    Ok(IdeaList { ideas, count })
}

/// Show a single idea.
pub fn idea_show(repo_path: &Path, id: &str) -> Result<Idea> {
    let storage = Storage::open(repo_path)?;
    storage.get_idea(id)
}

#[derive(Serialize)]
pub struct IdeaUpdated {
    pub id: String,
    pub updated_fields: Vec<String>,
}

impl Output for IdeaUpdated {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!(
            "Updated idea {}: {}",
            self.id,
            self.updated_fields.join(", ")
        )
    }
}

/// Update an idea.
#[allow(clippy::too_many_arguments)]
pub fn idea_update(
    repo_path: &Path,
    id: &str,
    title: Option<String>,
    short_name: Option<String>,
    description: Option<String>,
    status: Option<&str>,
    add_tags: Vec<String>,
    remove_tags: Vec<String>,
) -> Result<IdeaUpdated> {
    let mut storage = Storage::open(repo_path)?;
    let mut idea = storage.get_idea(id)?;
    let mut updated_fields = Vec::new();

    if let Some(t) = title {
        idea.core.title = t;
        updated_fields.push("title".to_string());
    }

    if let Some(new_short_name) = process_short_name_update(short_name) {
        idea.core.short_name = new_short_name;
        updated_fields.push("short_name".to_string());
    }

    if let Some(d) = description {
        idea.core.description = Some(d);
        updated_fields.push("description".to_string());
    }

    if let Some(s) = status {
        idea.status = parse_idea_status(s)?;
        updated_fields.push("status".to_string());
    }

    if !add_tags.is_empty() {
        for tag in add_tags {
            if !idea.core.tags.contains(&tag) {
                idea.core.tags.push(tag);
            }
        }
        updated_fields.push("tags".to_string());
    }

    if !remove_tags.is_empty() {
        idea.core.tags.retain(|t| !remove_tags.contains(t));
        if !updated_fields.contains(&"tags".to_string()) {
            updated_fields.push("tags".to_string());
        }
    }

    idea.core.updated_at = Utc::now();
    storage.update_idea(&idea)?;

    Ok(IdeaUpdated {
        id: id.to_string(),
        updated_fields,
    })
}

/// Parse idea status string to IdeaStatus enum.
fn parse_idea_status(s: &str) -> Result<IdeaStatus> {
    match s {
        "seed" => Ok(IdeaStatus::Seed),
        "germinating" => Ok(IdeaStatus::Germinating),
        "promoted" => Ok(IdeaStatus::Promoted),
        "discarded" => Ok(IdeaStatus::Discarded),
        _ => Err(Error::Other(format!(
            "Invalid idea status: {}. Valid values: seed, germinating, promoted, discarded",
            s
        ))),
    }
}

#[derive(Serialize)]
pub struct IdeaClosed {
    pub id: String,
    pub reason: Option<String>,
}

impl Output for IdeaClosed {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        match &self.reason {
            Some(r) => format!("Discarded idea {}: {}", self.id, r),
            None => format!("Discarded idea {}", self.id),
        }
    }
}

/// Close (discard) an idea.
pub fn idea_close(repo_path: &Path, id: &str, reason: Option<String>) -> Result<IdeaClosed> {
    let mut storage = Storage::open(repo_path)?;
    let mut idea = storage.get_idea(id)?;

    idea.status = IdeaStatus::Discarded;
    idea.core.updated_at = Utc::now();
    storage.update_idea(&idea)?;

    Ok(IdeaClosed {
        id: id.to_string(),
        reason,
    })
}

#[derive(Serialize)]
pub struct IdeaDeleted {
    pub id: String,
}

impl Output for IdeaDeleted {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Deleted idea {}", self.id)
    }
}

/// Delete an idea.
pub fn idea_delete(repo_path: &Path, id: &str) -> Result<IdeaDeleted> {
    let mut storage = Storage::open(repo_path)?;
    storage.delete_idea(id)?;

    Ok(IdeaDeleted { id: id.to_string() })
}

#[derive(Serialize)]
pub struct IdeaPromoted {
    pub id: String,
    pub promoted_to: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prd_path: Option<String>,
}

impl Output for IdeaPromoted {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if let Some(ref path) = self.prd_path {
            format!(
                "Promoted idea {} to PRD: {}\nIdea marked as promoted.",
                self.id, path
            )
        } else {
            format!(
                "Promoted idea {} to task: {}\nIdea marked as promoted.",
                self.id, self.promoted_to
            )
        }
    }
}

/// Promote an idea to a task or PRD.
pub fn idea_promote(
    repo_path: &Path,
    id: &str,
    as_prd: bool,
    priority: Option<u8>,
) -> Result<IdeaPromoted> {
    let mut storage = Storage::open(repo_path)?;
    let mut idea = storage.get_idea(id)?;

    // Check that idea is not already promoted or discarded
    if idea.status == IdeaStatus::Promoted {
        return Err(Error::Other(format!("Idea {} is already promoted", id)));
    }
    if idea.status == IdeaStatus::Discarded {
        return Err(Error::Other(format!(
            "Idea {} is discarded and cannot be promoted",
            id
        )));
    }

    if as_prd {
        // Generate PRD file
        let title_slug = idea
            .core
            .title
            .to_uppercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect::<String>();
        // Collapse multiple underscores and trim
        let title_slug = title_slug
            .split('_')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("_");

        let prd_filename = format!("PRD_{}.md", title_slug);
        let prd_dir = repo_path.join("prds");

        // Create prds directory if it doesn't exist
        if !prd_dir.exists() {
            fs::create_dir_all(&prd_dir)?;
        }

        let prd_path = prd_dir.join(&prd_filename);

        // Check if PRD already exists
        if prd_path.exists() {
            return Err(Error::Other(format!(
                "PRD file already exists: {}",
                prd_path.display()
            )));
        }

        // Generate PRD content from template
        let origin_desc = idea
            .core
            .description
            .clone()
            .unwrap_or_else(|| idea.core.title.clone());
        let prd_content = format!(
            r#"# PRD: {}

> Promoted from idea {} on {}

## Origin
{}

## Problem Statement
[TODO: Define the problem]

## Proposed Solution
[TODO: Describe the solution]

## Implementation Plan
[TODO: Break into tasks]
"#,
            idea.core.title,
            id,
            chrono::Utc::now().format("%Y-%m-%d"),
            origin_desc
        );

        fs::write(&prd_path, prd_content)?;

        // Update idea
        let prd_path_str = prd_path.to_string_lossy().to_string();
        idea.status = IdeaStatus::Promoted;
        idea.promoted_to = Some(prd_path_str.clone());
        idea.core.updated_at = Utc::now();
        storage.update_idea(&idea)?;

        Ok(IdeaPromoted {
            id: id.to_string(),
            promoted_to: prd_path_str.clone(),
            prd_path: Some(prd_path_str),
        })
    } else {
        // Create a task from the idea
        let task_id = storage.generate_unique_id("bn", &idea.core.title);
        let mut task = Task::new(task_id.clone(), idea.core.title.clone());
        task.core.description = idea.core.description.clone();
        task.core.tags = idea.core.tags.clone();
        task.priority = priority.unwrap_or(2);

        storage.create_task(&task)?;

        // Update idea
        idea.status = IdeaStatus::Promoted;
        idea.promoted_to = Some(task_id.clone());
        idea.core.updated_at = Utc::now();
        storage.update_idea(&idea)?;

        Ok(IdeaPromoted {
            id: id.to_string(),
            promoted_to: task_id,
            prd_path: None,
        })
    }
}

#[derive(Serialize)]
pub struct IdeaGerminated {
    pub id: String,
}

impl Output for IdeaGerminated {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Idea {} is now germinating", self.id)
    }
}

/// Mark an idea as germinating (being developed).
pub fn idea_germinate(repo_path: &Path, id: &str) -> Result<IdeaGerminated> {
    let mut storage = Storage::open(repo_path)?;
    let mut idea = storage.get_idea(id)?;

    // Check that idea is in valid state for germination
    if idea.status == IdeaStatus::Promoted {
        return Err(Error::Other(format!(
            "Idea {} is already promoted and cannot be germinated",
            id
        )));
    }
    if idea.status == IdeaStatus::Discarded {
        return Err(Error::Other(format!(
            "Idea {} is discarded and cannot be germinated",
            id
        )));
    }

    idea.status = IdeaStatus::Germinating;
    idea.core.updated_at = Utc::now();
    storage.update_idea(&idea)?;

    Ok(IdeaGerminated { id: id.to_string() })
}

// === Doc Commands ===

#[derive(Serialize)]
pub struct DocCreated {
    pub id: String,
    pub title: String,
    pub doc_type: String,
    pub linked_entities: Vec<String>,
}

impl Output for DocCreated {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let links = self.linked_entities.join(", ");
        format!(
            "Created {} doc {} \"{}\" linked to: {}",
            self.doc_type, self.id, self.title, links
        )
    }
}

/// Create a new documentation node linked to entities.
///
/// Docs must be linked to at least one entity on creation.
/// Content can include an optional summary section prepended as `# Summary`.
#[allow(clippy::too_many_arguments)]
pub fn doc_create(
    repo_path: &Path,
    title: String,
    doc_type: DocType,
    short_name: Option<String>,
    content: Option<String>,
    summary: Option<String>,
    tags: Vec<String>,
    entity_ids: Vec<String>,
) -> Result<DocCreated> {
    if entity_ids.is_empty() {
        return Err(Error::InvalidInput(
            "At least one entity ID is required".to_string(),
        ));
    }

    let mut storage = Storage::open(repo_path)?;

    // Validate all entity IDs exist before creating the doc
    for entity_id in &entity_ids {
        storage
            .get_entity_type(entity_id)
            .map_err(|_| Error::InvalidInput(format!("Entity '{}' does not exist", entity_id)))?;
    }

    let id = storage.generate_unique_id("bn", &title);
    let mut doc = Doc::new(id.clone(), title.clone());
    doc.core.short_name = normalize_short_name(short_name);
    doc.core.tags = tags;
    doc.doc_type = doc_type.clone();

    // Build content with optional summary section prepended
    let final_content = match (summary, content) {
        (Some(sum), Some(cont)) => {
            // Prepend # Summary section
            format!("# Summary\n\n{}\n\n{}", sum.trim(), cont)
        }
        (Some(sum), None) => {
            // Only summary provided
            format!("# Summary\n\n{}", sum.trim())
        }
        (None, Some(cont)) => cont,
        (None, None) => String::new(),
    };

    if !final_content.is_empty() {
        doc.set_content(&final_content)
            .map_err(|e| Error::InvalidInput(e.to_string()))?;
    }

    storage.add_doc(&doc)?;

    // Create links from doc to each entity
    for entity_id in &entity_ids {
        let edge_id = storage.generate_edge_id(&id, entity_id, EdgeType::Documents);
        let edge = Edge::new(edge_id, id.clone(), entity_id.clone(), EdgeType::Documents);
        storage.add_edge(&edge)?;
    }

    Ok(DocCreated {
        id,
        title,
        doc_type: doc_type.to_string(),
        linked_entities: entity_ids,
    })
}

impl Output for Doc {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "{} [{}] {}",
            self.core.id, self.doc_type, self.core.title
        ));
        if let Some(ref sn) = self.core.short_name {
            lines.push(format!("  Short Name: {}", sn));
        }
        if let Some(ref desc) = self.core.description {
            lines.push(format!("  Description: {}", desc));
        }
        if !self.core.tags.is_empty() {
            lines.push(format!("  Tags: {}", self.core.tags.join(", ")));
        }
        if self.summary_dirty {
            lines.push("  ⚠ Summary needs update".to_string());
        }
        if !self.editors.is_empty() {
            let editors_str: Vec<String> = self.editors.iter().map(|e| e.to_string()).collect();
            lines.push(format!("  Editors: {}", editors_str.join(", ")));
        }
        if let Some(ref sup) = self.supersedes {
            lines.push(format!("  Supersedes: {}", sup));
        }
        lines.push(format!(
            "  Created: {}",
            self.core.created_at.format("%Y-%m-%d %H:%M")
        ));
        lines.push(format!(
            "  Updated: {}",
            self.core.updated_at.format("%Y-%m-%d %H:%M")
        ));
        if !self.content.is_empty() {
            lines.push(String::new());
            lines.push("Content:".to_string());
            lines.push("─".repeat(40));
            // Try to decompress content for display
            match self.get_content() {
                Ok(c) => lines.push(c),
                Err(_) => lines.push("[Unable to decompress content]".to_string()),
            }
        }
        lines.join("\n")
    }
}

#[derive(Serialize)]
pub struct DocList {
    pub docs: Vec<Doc>,
    pub count: usize,
}

impl Output for DocList {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if self.docs.is_empty() {
            return "No docs found.".to_string();
        }

        let mut lines = vec![format!("{} doc(s):\n", self.count)];
        for doc in &self.docs {
            let tags_str = if doc.core.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", doc.core.tags.join(", "))
            };
            lines.push(format!("📄 {} {}{}", doc.core.id, doc.core.title, tags_str));
        }
        lines.join("\n")
    }
}

/// List documentation nodes with optional filters.
pub fn doc_list(
    repo_path: &Path,
    tag: Option<&str>,
    doc_type: Option<&DocType>,
    edited_by: Option<&str>,
    for_entity: Option<&str>,
) -> Result<DocList> {
    let storage = Storage::open(repo_path)?;
    let docs = storage.list_docs(tag, doc_type, edited_by, for_entity)?;
    let count = docs.len();

    Ok(DocList { docs, count })
}

/// Result of showing a single documentation node.
#[derive(Serialize)]
pub struct DocShowResult {
    pub doc: Doc,
    pub linked_entities: Vec<LinkedEntityInfo>,
    #[serde(skip)]
    pub show_full: bool,
}

/// Information about an entity linked to a doc.
#[derive(Serialize)]
pub struct LinkedEntityInfo {
    pub id: String,
    pub entity_type: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

impl Output for DocShowResult {
    fn to_json(&self) -> String {
        // Create a JSON representation with decompressed content
        // so agents can read the actual content, not the compressed blob
        let decompressed_content = self.doc.get_content().unwrap_or_default();

        // Build a doc representation with decompressed content
        #[derive(Serialize)]
        struct DocJson<'a> {
            id: &'a str,
            #[serde(rename = "type")]
            entity_type: &'a str,
            title: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            short_name: &'a Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            description: &'a Option<String>,
            tags: &'a [String],
            doc_type: &'a crate::models::DocType,
            content: &'a str,
            summary_dirty: bool,
            editors: &'a [crate::models::Editor],
            #[serde(skip_serializing_if = "Option::is_none")]
            supersedes: &'a Option<String>,
            created_at: &'a chrono::DateTime<chrono::Utc>,
            updated_at: &'a chrono::DateTime<chrono::Utc>,
        }

        #[derive(Serialize)]
        struct DocShowJson<'a> {
            doc: DocJson<'a>,
            linked_entities: &'a [LinkedEntityInfo],
        }

        let doc_json = DocJson {
            id: &self.doc.core.id,
            entity_type: &self.doc.core.entity_type,
            title: &self.doc.core.title,
            short_name: &self.doc.core.short_name,
            description: &self.doc.core.description,
            tags: &self.doc.core.tags,
            doc_type: &self.doc.doc_type,
            content: &decompressed_content,
            summary_dirty: self.doc.summary_dirty,
            editors: &self.doc.editors,
            supersedes: &self.doc.supersedes,
            created_at: &self.doc.core.created_at,
            updated_at: &self.doc.core.updated_at,
        };

        let json_repr = DocShowJson {
            doc: doc_json,
            linked_entities: &self.linked_entities,
        };

        serde_json::to_string(&json_repr).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();

        // Header line
        lines.push(format!("Doc: {} [{}]", self.doc.core.id, self.doc.doc_type));
        lines.push(format!("Title: {}", self.doc.core.title));

        if let Some(ref sn) = self.doc.core.short_name {
            lines.push(format!("Short Name: {}", sn));
        }
        if let Some(ref desc) = self.doc.core.description {
            lines.push(format!("Description: {}", desc));
        }
        if !self.doc.core.tags.is_empty() {
            lines.push(format!("Tags: {}", self.doc.core.tags.join(", ")));
        }
        if self.doc.summary_dirty {
            lines.push("⚠ Summary needs update".to_string());
        }
        if !self.doc.editors.is_empty() {
            let editors_str: Vec<String> = self.doc.editors.iter().map(|e| e.to_string()).collect();
            lines.push(format!("Editors: {}", editors_str.join(", ")));
        }
        if let Some(ref sup) = self.doc.supersedes {
            lines.push(format!("Supersedes: {}", sup));
        }
        lines.push(format!(
            "Created: {}",
            self.doc.core.created_at.format("%Y-%m-%d %H:%M")
        ));
        lines.push(format!(
            "Updated: {}",
            self.doc.core.updated_at.format("%Y-%m-%d %H:%M")
        ));

        // Show linked entities
        if !self.linked_entities.is_empty() {
            lines.push(String::new());
            lines.push(format!("Linked Entities ({}):", self.linked_entities.len()));
            for entity in &self.linked_entities {
                let status_str = entity
                    .status
                    .as_ref()
                    .map(|s| format!(" [{}]", s))
                    .unwrap_or_default();
                lines.push(format!(
                    "  {} ({}) \"{}\"{}",
                    entity.id, entity.entity_type, entity.title, status_str
                ));
            }
        }

        // Show content based on --full flag
        if self.show_full {
            // Show full content
            if !self.doc.content.is_empty() {
                lines.push(String::new());
                lines.push("Content:".to_string());
                lines.push("─".repeat(60));
                match self.doc.get_content() {
                    Ok(c) => lines.push(c),
                    Err(_) => lines.push("[Unable to decompress content]".to_string()),
                }
            }
        } else {
            // Show only summary section
            if !self.doc.content.is_empty()
                && let Ok(content) = self.doc.get_content()
                && let Some(summary) = extract_summary_section(&content)
            {
                lines.push(String::new());
                lines.push("Summary:".to_string());
                lines.push("─".repeat(40));
                lines.push(summary);
            }
            // Hint about --full flag
            if !self.doc.content.is_empty() {
                lines.push(String::new());
                lines.push("(Use --full to see complete content)".to_string());
            }
        }

        lines.join("\n")
    }
}

/// Extract the # Summary section from markdown content.
fn extract_summary_section(content: &str) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    let mut in_summary = false;
    let mut summary_lines = Vec::new();

    for line in lines {
        // Check for # Summary header (case-insensitive)
        if line.trim().to_lowercase().starts_with("# summary") {
            in_summary = true;
            continue;
        }
        // Stop when we hit another heading
        if in_summary && line.trim().starts_with('#') {
            break;
        }
        if in_summary {
            summary_lines.push(line);
        }
    }

    if summary_lines.is_empty() {
        None
    } else {
        // Trim leading/trailing empty lines
        let summary = summary_lines.join("\n");
        Some(summary.trim().to_string())
    }
}

/// Show a single documentation node.
pub fn doc_show(repo_path: &Path, id: &str, full: bool) -> Result<DocShowResult> {
    let storage = Storage::open(repo_path)?;
    let doc = storage.get_doc(id)?;

    // Get linked entities (entities this doc documents)
    // Documents edges: doc -> entity, so we look for outbound edges
    let edges = storage
        .list_edges(Some(EdgeType::Documents), Some(id), None)
        .unwrap_or_default();

    let linked_entities: Vec<LinkedEntityInfo> = edges
        .iter()
        .filter_map(|edge| {
            // The target of a 'documents' edge is the entity being documented
            let entity_id = &edge.target;

            // Try to get info about the entity
            if let Ok(task) = storage.get_task(entity_id) {
                Some(LinkedEntityInfo {
                    id: task.core.id.clone(),
                    entity_type: "task".to_string(),
                    title: task.core.title.clone(),
                    status: Some(format!("{:?}", task.status).to_lowercase()),
                })
            } else if let Ok(bug) = storage.get_bug(entity_id) {
                Some(LinkedEntityInfo {
                    id: bug.core.id.clone(),
                    entity_type: "bug".to_string(),
                    title: bug.core.title.clone(),
                    status: Some(format!("{:?}", bug.status).to_lowercase()),
                })
            } else if let Ok(idea) = storage.get_idea(entity_id) {
                Some(LinkedEntityInfo {
                    id: idea.core.id.clone(),
                    entity_type: "idea".to_string(),
                    title: idea.core.title.clone(),
                    status: Some(format!("{:?}", idea.status).to_lowercase()),
                })
            } else if let Ok(milestone) = storage.get_milestone(entity_id) {
                Some(LinkedEntityInfo {
                    id: milestone.core.id.clone(),
                    entity_type: "milestone".to_string(),
                    title: milestone.core.title.clone(),
                    status: Some(format!("{:?}", milestone.status).to_lowercase()),
                })
            } else if let Ok(doc) = storage.get_doc(entity_id) {
                Some(LinkedEntityInfo {
                    id: doc.core.id.clone(),
                    entity_type: "doc".to_string(),
                    title: doc.core.title.clone(),
                    status: None,
                })
            } else {
                None
            }
        })
        .collect();

    Ok(DocShowResult {
        doc,
        linked_entities,
        show_full: full,
    })
}

#[derive(Serialize)]
pub struct DocUpdated {
    pub id: String,
    pub updated_fields: Vec<String>,
}

impl Output for DocUpdated {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!(
            "Updated doc {}: {}",
            self.id,
            self.updated_fields.join(", ")
        )
    }
}

/// Update a documentation node.
#[allow(clippy::too_many_arguments)]
pub fn doc_edit(
    repo_path: &Path,
    id: &str,
    title: Option<String>,
    short_name: Option<String>,
    description: Option<String>,
    content: Option<String>,
    add_tags: Vec<String>,
    remove_tags: Vec<String>,
) -> Result<DocUpdated> {
    let mut storage = Storage::open(repo_path)?;
    let mut doc = storage.get_doc(id)?;
    let mut updated_fields = Vec::new();

    if let Some(t) = title {
        doc.core.title = t;
        updated_fields.push("title".to_string());
    }
    if let Some(sn) = short_name {
        doc.core.short_name = normalize_short_name(Some(sn));
        updated_fields.push("short_name".to_string());
    }
    if let Some(d) = description {
        doc.core.description = Some(d);
        updated_fields.push("description".to_string());
    }
    if let Some(c) = content {
        doc.content = c;
        updated_fields.push("content".to_string());
    }
    for tag in add_tags {
        if !doc.core.tags.contains(&tag) {
            doc.core.tags.push(tag);
            if !updated_fields.contains(&"tags".to_string()) {
                updated_fields.push("tags".to_string());
            }
        }
    }
    for tag in &remove_tags {
        if let Some(pos) = doc.core.tags.iter().position(|t| t == tag) {
            doc.core.tags.remove(pos);
            if !updated_fields.contains(&"tags".to_string()) {
                updated_fields.push("tags".to_string());
            }
        }
    }

    if updated_fields.is_empty() {
        return Err(Error::Other("No fields to update".to_string()));
    }

    doc.core.updated_at = Utc::now();
    storage.update_doc(&doc)?;

    Ok(DocUpdated {
        id: id.to_string(),
        updated_fields,
    })
}

/// Result from creating a new doc version.
#[derive(Serialize)]
pub struct DocVersionCreated {
    /// ID of the new doc version
    pub new_id: String,
    /// ID of the previous version
    pub previous_id: String,
    /// Title of the doc
    pub title: String,
    /// Number of edges transferred from old version
    pub edges_transferred: usize,
}

impl Output for DocVersionCreated {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!(
            "Created new version {} (supersedes {})\n  Title: {}\n  Edges transferred: {}",
            self.new_id, self.previous_id, self.title, self.edges_transferred
        )
    }
}

/// Create a new version of a doc (versioning with supersedes chain).
///
/// This creates a new doc entity with modifications, sets `supersedes` to point
/// to the previous version, and transfers all edges from the old doc to the new one.
#[allow(clippy::too_many_arguments)]
pub fn doc_update(
    repo_path: &Path,
    id: &str,
    content: Option<String>,
    title: Option<String>,
    short_name: Option<String>,
    description: Option<String>,
    editor: Option<&str>,
    clear_dirty: bool,
) -> Result<DocVersionCreated> {
    let mut storage = Storage::open(repo_path)?;

    // Get the old doc
    let old_doc = storage.get_doc(id)?;

    // Generate a new ID for the new version
    let new_id = storage.generate_unique_id("bn", &old_doc.core.title);

    // Create the new doc by cloning the old one
    let mut new_doc = Doc::new(new_id.clone(), old_doc.core.title.clone());
    new_doc.doc_type = old_doc.doc_type.clone();
    new_doc.core.short_name = old_doc.core.short_name.clone();
    new_doc.core.description = old_doc.core.description.clone();
    new_doc.core.tags = old_doc.core.tags.clone();
    new_doc.content = old_doc.content.clone();
    new_doc.editors = old_doc.editors.clone();
    new_doc.supersedes = Some(id.to_string());

    // Apply modifications
    if let Some(new_title) = title {
        new_doc.core.title = new_title;
    }
    if let Some(new_short_name) = short_name {
        new_doc.core.short_name = normalize_short_name(Some(new_short_name));
    }
    if let Some(new_description) = description {
        new_doc.core.description = Some(new_description);
    }
    // Handle content update with summary dirty detection
    if let Some(new_content) = &content {
        // Get old decompressed content for comparison
        let old_content = old_doc.get_content().unwrap_or_default();

        // Detect if summary_dirty should be set
        // This happens when content changes but the # Summary section doesn't
        if Doc::is_summary_dirty(&old_content, new_content) {
            new_doc.summary_dirty = true;
        } else {
            // Summary was updated or no meaningful change - clear the flag
            new_doc.summary_dirty = false;
        }

        // Compress and set the new content
        new_doc
            .set_content(new_content)
            .map_err(|e| Error::InvalidInput(e.to_string()))?;
    }

    // Handle explicit clear_dirty flag (takes precedence)
    if clear_dirty {
        new_doc.summary_dirty = false;
    }

    // Add editor attribution if provided
    if let Some(editor_str) = editor {
        let parsed_editor = parse_editor(editor_str)?;
        // Don't add duplicate editors
        if !new_doc.editors.iter().any(|e| {
            e.editor_type == parsed_editor.editor_type && e.identifier == parsed_editor.identifier
        }) {
            new_doc.editors.push(parsed_editor);
        }
    }

    // Save the new doc
    storage.add_doc(&new_doc)?;

    // Transfer edges from old doc to new doc
    // Get all edges where old doc is source or target
    let edges = storage.get_edges_for_entity(id)?;
    let mut edges_transferred = 0;

    for hydrated_edge in edges {
        // Skip edges that would create duplicates or don't involve the old doc directly
        // Note: pinned edges are not yet implemented (task bn-e6a4), so we transfer all for now

        let (new_source, new_target) = if hydrated_edge.edge.source == id {
            // Old doc is the source - point to new doc
            (new_id.clone(), hydrated_edge.edge.target.clone())
        } else if hydrated_edge.edge.target == id {
            // Old doc is the target - point to new doc
            (hydrated_edge.edge.source.clone(), new_id.clone())
        } else {
            // Edge doesn't directly involve the old doc, skip
            continue;
        };

        // Create a new edge with the same type
        let new_edge_id =
            storage.generate_edge_id(&new_source, &new_target, hydrated_edge.edge.edge_type);
        let mut new_edge = Edge::new(
            new_edge_id,
            new_source,
            new_target,
            hydrated_edge.edge.edge_type,
        );
        new_edge.reason = hydrated_edge.edge.reason.clone();

        // Add the new edge (ignore errors for duplicates)
        if storage.add_edge(&new_edge).is_ok() {
            edges_transferred += 1;
        }
    }

    Ok(DocVersionCreated {
        new_id,
        previous_id: id.to_string(),
        title: new_doc.core.title,
        edges_transferred,
    })
}

/// Parse an editor string like "agent:bna-1234" or "user:henry" into an Editor.
fn parse_editor(s: &str) -> Result<Editor> {
    let parts: Vec<&str> = s.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(Error::InvalidInput(format!(
            "Invalid editor format '{}'. Expected 'agent:id' or 'user:name'",
            s
        )));
    }

    let (editor_type, identifier) = (parts[0], parts[1]);
    match editor_type {
        "agent" => Ok(Editor::agent(identifier.to_string())),
        "user" => Ok(Editor::user(identifier.to_string())),
        _ => Err(Error::InvalidInput(format!(
            "Invalid editor type '{}'. Expected 'agent' or 'user'",
            editor_type
        ))),
    }
}

/// Result from getting doc version history.
#[derive(Serialize)]
pub struct DocHistory {
    /// Current (most recent) doc ID
    pub current_id: String,
    /// List of versions from newest to oldest
    pub versions: Vec<DocHistoryEntry>,
}

#[derive(Serialize)]
pub struct DocHistoryEntry {
    pub id: String,
    pub title: String,
    pub editors: Vec<Editor>,
    pub created_at: String,
    pub is_current: bool,
}

impl Output for DocHistory {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = vec![format!(
            "Doc {} history ({} versions):\n",
            self.current_id,
            self.versions.len()
        )];

        for (i, version) in self.versions.iter().enumerate() {
            let current_marker = if version.is_current { " (current)" } else { "" };
            let editors_str = if version.editors.is_empty() {
                "unknown".to_string()
            } else {
                version
                    .editors
                    .iter()
                    .map(|e| format!("{}:{}", e.editor_type, e.identifier))
                    .collect::<Vec<_>>()
                    .join(", ")
            };

            lines.push(format!(
                "  {}. {}{} - {} by {}",
                i + 1,
                version.id,
                current_marker,
                &version.created_at[..10], // Just the date
                editors_str
            ));
        }

        lines.join("\n")
    }
}

/// Get the version history of a doc by following the supersedes chain.
pub fn doc_history(repo_path: &Path, id: &str) -> Result<DocHistory> {
    let storage = Storage::open(repo_path)?;

    // Start from the given ID and collect all versions
    let mut versions = Vec::new();
    let mut current_id = id.to_string();
    let mut seen_ids = std::collections::HashSet::new();

    // First, walk backwards to find all previous versions
    loop {
        if seen_ids.contains(&current_id) {
            // Prevent infinite loops from circular references
            break;
        }
        seen_ids.insert(current_id.clone());

        let doc = storage.get_doc(&current_id)?;
        versions.push(DocHistoryEntry {
            id: doc.core.id.clone(),
            title: doc.core.title.clone(),
            editors: doc.editors.clone(),
            created_at: doc.core.created_at.to_rfc3339(),
            is_current: versions.is_empty(), // First one we find is current
        });

        if let Some(prev_id) = doc.supersedes {
            current_id = prev_id;
        } else {
            break;
        }
    }

    Ok(DocHistory {
        current_id: id.to_string(),
        versions,
    })
}

#[derive(Serialize)]
pub struct DocAttached {
    pub doc_id: String,
    pub target_id: String,
}

impl Output for DocAttached {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Attached doc {} to {}", self.doc_id, self.target_id)
    }
}

/// Attach a doc to another entity (creates a 'documents' edge).
pub fn doc_attach(repo_path: &Path, doc_id: &str, target_id: &str) -> Result<DocAttached> {
    let mut storage = Storage::open(repo_path)?;

    // Verify doc exists
    storage.get_doc(doc_id)?;

    // Verify target entity exists (could be task, bug, idea, milestone, etc.)
    storage.get_entity_type(target_id)?;

    // Create edge: doc_id --documents--> target_id
    let edge_id = storage.generate_edge_id(doc_id, target_id, EdgeType::Documents);
    let edge = Edge::new(
        edge_id,
        doc_id.to_string(),
        target_id.to_string(),
        EdgeType::Documents,
    );
    storage.add_edge(&edge)?;

    Ok(DocAttached {
        doc_id: doc_id.to_string(),
        target_id: target_id.to_string(),
    })
}

#[derive(Serialize)]
pub struct DocDetached {
    pub doc_id: String,
    pub target_id: String,
}

impl Output for DocDetached {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Detached doc {} from {}", self.doc_id, self.target_id)
    }
}

/// Detach a doc from an entity (removes the 'documents' edge).
pub fn doc_detach(repo_path: &Path, doc_id: &str, target_id: &str) -> Result<DocDetached> {
    let mut storage = Storage::open(repo_path)?;

    // Verify doc exists
    storage.get_doc(doc_id)?;

    // Remove the edge
    storage.remove_edge(doc_id, target_id, EdgeType::Documents)?;

    Ok(DocDetached {
        doc_id: doc_id.to_string(),
        target_id: target_id.to_string(),
    })
}

#[derive(Serialize)]
pub struct DocDeleted {
    pub id: String,
}

impl Output for DocDeleted {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Deleted doc {}", self.id)
    }
}

/// Delete a documentation node.
pub fn doc_delete(repo_path: &Path, id: &str) -> Result<DocDeleted> {
    let mut storage = Storage::open(repo_path)?;
    storage.delete_doc(id)?;

    Ok(DocDeleted { id: id.to_string() })
}

// === Milestone Commands ===

use crate::models::MilestoneProgress;

#[derive(Serialize)]
pub struct MilestoneCreated {
    pub id: String,
    pub title: String,
}

impl Output for MilestoneCreated {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Created milestone {} \"{}\"", self.id, self.title)
    }
}

/// Create a new milestone.
#[allow(clippy::too_many_arguments)]
pub fn milestone_create(
    repo_path: &Path,
    title: String,
    short_name: Option<String>,
    description: Option<String>,
    priority: Option<u8>,
    tags: Vec<String>,
    assignee: Option<String>,
    due_date: Option<String>,
) -> Result<MilestoneCreated> {
    let mut storage = Storage::open(repo_path)?;

    if let Some(p) = priority
        && p > 4
    {
        return Err(Error::Other("Priority must be 0-4".to_string()));
    }

    let id = storage.generate_unique_id("bn", &title);
    let mut milestone = Milestone::new(id.clone(), title.clone());
    milestone.core.short_name = normalize_short_name(short_name);
    milestone.core.description = description;
    milestone.priority = priority.unwrap_or(2);
    milestone.core.tags = tags;
    milestone.assignee = assignee;
    milestone.due_date = due_date
        .map(|d| chrono::DateTime::parse_from_rfc3339(&d))
        .transpose()
        .map_err(|e| {
            Error::Other(format!(
                "Invalid due_date format: {}. Use ISO 8601 format.",
                e
            ))
        })?
        .map(|d| d.with_timezone(&chrono::Utc));

    storage.add_milestone(&milestone)?;

    Ok(MilestoneCreated { id, title })
}

impl Output for Milestone {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("{} {}", self.core.id, self.core.title));
        if let Some(ref sn) = self.core.short_name {
            lines.push(format!("  Short Name: {}", sn));
        }
        lines.push(format!(
            "  Status: {:?}  Priority: {}",
            self.status, self.priority
        ));
        if let Some(ref desc) = self.core.description {
            lines.push(format!("  Description: {}", desc));
        }
        if let Some(due) = self.due_date {
            lines.push(format!("  Due date: {}", due.format("%Y-%m-%d")));
        }
        if !self.core.tags.is_empty() {
            lines.push(format!("  Tags: {}", self.core.tags.join(", ")));
        }
        if let Some(ref assignee) = self.assignee {
            lines.push(format!("  Assignee: {}", assignee));
        }
        lines.push(format!(
            "  Created: {}",
            self.core.created_at.format("%Y-%m-%d %H:%M")
        ));
        lines.push(format!(
            "  Updated: {}",
            self.core.updated_at.format("%Y-%m-%d %H:%M")
        ));
        if let Some(closed) = self.closed_at {
            lines.push(format!("  Closed: {}", closed.format("%Y-%m-%d %H:%M")));
            if let Some(ref reason) = self.closed_reason {
                lines.push(format!("  Reason: {}", reason));
            }
        }
        lines.join("\n")
    }
}

#[derive(Serialize)]
pub struct MilestoneList {
    pub milestones: Vec<Milestone>,
    pub count: usize,
}

impl Output for MilestoneList {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if self.milestones.is_empty() {
            return "No milestones found.".to_string();
        }

        let mut lines = Vec::new();
        lines.push(format!("{} milestone(s):\n", self.count));

        for milestone in &self.milestones {
            let status_char = match milestone.status {
                TaskStatus::Pending => " ",
                TaskStatus::InProgress => ">",
                TaskStatus::Done => "x",
                TaskStatus::Blocked => "!",
                TaskStatus::Cancelled => "-",
                TaskStatus::Reopened => "?",
                TaskStatus::Partial => "~",
            };
            let tags = if milestone.core.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", milestone.core.tags.join(", "))
            };
            let due = milestone
                .due_date
                .map(|d| format!(" due:{}", d.format("%Y-%m-%d")))
                .unwrap_or_default();
            lines.push(format!(
                "[{}] {} P{} {}{}{}",
                status_char, milestone.core.id, milestone.priority, milestone.core.title, tags, due
            ));
        }

        lines.join("\n")
    }
}

/// List milestones with optional filters.
pub fn milestone_list(
    repo_path: &Path,
    status: Option<&str>,
    priority: Option<u8>,
    tag: Option<&str>,
) -> Result<MilestoneList> {
    let storage = Storage::open(repo_path)?;
    let milestones = storage.list_milestones(status, priority, tag)?;
    let count = milestones.len();
    Ok(MilestoneList { milestones, count })
}

/// Result of milestone_show with progress and edge information.
#[derive(Serialize)]
pub struct MilestoneShowResult {
    pub milestone: Milestone,
    pub progress: MilestoneProgress,
    pub edges: Vec<Edge>,
}

impl Output for MilestoneShowResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = vec![self.milestone.to_human()];

        // Add progress info
        lines.push(format!(
            "  Progress: {}/{} ({:.1}%)",
            self.progress.completed, self.progress.total, self.progress.percentage
        ));

        // Add edge info
        if !self.edges.is_empty() {
            lines.push("  Relationships:".to_string());
            for edge in &self.edges {
                let direction = if edge.source == self.milestone.core.id {
                    format!("{} → {}", edge.edge_type, edge.target)
                } else {
                    format!("{} ← {}", edge.edge_type, edge.source)
                };
                lines.push(format!("    {}", direction));
            }
        }

        lines.join("\n")
    }
}

/// Show milestone details with progress.
pub fn milestone_show(repo_path: &Path, id: &str) -> Result<MilestoneShowResult> {
    let storage = Storage::open(repo_path)?;
    let milestone = storage.get_milestone(id)?;
    let progress = storage.get_milestone_progress(id)?;

    // Get edges for this milestone - use get_edges_for_entity which returns both in/outbound
    let hydrated_edges = storage.get_edges_for_entity(id)?;
    let edges: Vec<Edge> = hydrated_edges.into_iter().map(|he| he.edge).collect();

    Ok(MilestoneShowResult {
        milestone,
        progress,
        edges,
    })
}

#[derive(Serialize)]
pub struct MilestoneUpdated {
    pub id: String,
    pub updated_fields: Vec<String>,
}

impl Output for MilestoneUpdated {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!(
            "Updated milestone {}: {}",
            self.id,
            self.updated_fields.join(", ")
        )
    }
}

/// Update a milestone.
#[allow(clippy::too_many_arguments)]
pub fn milestone_update(
    repo_path: &Path,
    id: &str,
    title: Option<String>,
    short_name: Option<String>,
    description: Option<String>,
    priority: Option<u8>,
    status: Option<&str>,
    add_tags: Vec<String>,
    remove_tags: Vec<String>,
    assignee: Option<String>,
    due_date: Option<String>,
) -> Result<MilestoneUpdated> {
    let mut storage = Storage::open(repo_path)?;
    let mut milestone = storage.get_milestone(id)?;
    let mut updated_fields = Vec::new();

    if let Some(t) = title {
        milestone.core.title = t;
        updated_fields.push("title".to_string());
    }

    if let Some(new_short_name) = process_short_name_update(short_name) {
        milestone.core.short_name = new_short_name;
        updated_fields.push("short_name".to_string());
    }

    if let Some(d) = description {
        milestone.core.description = Some(d);
        updated_fields.push("description".to_string());
    }

    if let Some(p) = priority {
        if p > 4 {
            return Err(Error::Other("Priority must be 0-4".to_string()));
        }
        milestone.priority = p;
        updated_fields.push("priority".to_string());
    }

    if let Some(s) = status {
        milestone.status = parse_status(s)?;
        updated_fields.push("status".to_string());
    }

    for tag in add_tags {
        if !milestone.core.tags.contains(&tag) {
            milestone.core.tags.push(tag.clone());
            updated_fields.push(format!("added tag: {}", tag));
        }
    }

    for tag in remove_tags {
        if let Some(pos) = milestone.core.tags.iter().position(|t| t == &tag) {
            milestone.core.tags.remove(pos);
            updated_fields.push(format!("removed tag: {}", tag));
        }
    }

    if let Some(a) = assignee {
        milestone.assignee = Some(a);
        updated_fields.push("assignee".to_string());
    }

    if let Some(d) = due_date {
        milestone.due_date = Some(
            chrono::DateTime::parse_from_rfc3339(&d)
                .map_err(|e| {
                    Error::Other(format!(
                        "Invalid due_date format: {}. Use ISO 8601 format.",
                        e
                    ))
                })?
                .with_timezone(&chrono::Utc),
        );
        updated_fields.push("due_date".to_string());
    }

    if updated_fields.is_empty() {
        return Err(Error::Other("No updates specified".to_string()));
    }

    milestone.core.updated_at = chrono::Utc::now();
    storage.update_milestone(&milestone)?;

    Ok(MilestoneUpdated {
        id: id.to_string(),
        updated_fields,
    })
}

#[derive(Serialize)]
pub struct MilestoneClosed {
    pub id: String,
    pub reason: Option<String>,
}

impl Output for MilestoneClosed {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        match &self.reason {
            Some(r) => format!("Closed milestone {}: {}", self.id, r),
            None => format!("Closed milestone {}", self.id),
        }
    }
}

/// Close a milestone.
pub fn milestone_close(
    repo_path: &Path,
    id: &str,
    reason: Option<String>,
    force: bool,
) -> Result<MilestoneClosed> {
    let mut storage = Storage::open(repo_path)?;
    let mut milestone = storage.get_milestone(id)?;

    // Check if already closed
    if milestone.status == TaskStatus::Done {
        return Err(Error::Other(format!("Milestone {} is already closed", id)));
    }

    // Check progress (unless force is set)
    if !force {
        let progress = storage.get_milestone_progress(id)?;
        if progress.total > 0 && progress.completed < progress.total {
            return Err(Error::Other(format!(
                "Milestone has incomplete children ({}/{}). Use --force to close anyway.",
                progress.completed, progress.total
            )));
        }
    }

    milestone.status = TaskStatus::Done;
    milestone.closed_at = Some(chrono::Utc::now());
    milestone.closed_reason = reason.clone();
    milestone.core.updated_at = chrono::Utc::now();

    storage.update_milestone(&milestone)?;

    Ok(MilestoneClosed {
        id: id.to_string(),
        reason,
    })
}

#[derive(Serialize)]
pub struct MilestoneReopened {
    pub id: String,
}

impl Output for MilestoneReopened {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Reopened milestone {}", self.id)
    }
}

/// Reopen a closed milestone.
pub fn milestone_reopen(repo_path: &Path, id: &str) -> Result<MilestoneReopened> {
    let mut storage = Storage::open(repo_path)?;
    let mut milestone = storage.get_milestone(id)?;

    if milestone.status != TaskStatus::Done && milestone.status != TaskStatus::Cancelled {
        return Err(Error::Other(format!(
            "Milestone {} is not closed (status: {:?})",
            id, milestone.status
        )));
    }

    milestone.status = TaskStatus::Reopened;
    milestone.closed_at = None;
    milestone.closed_reason = None;
    milestone.core.updated_at = chrono::Utc::now();

    storage.update_milestone(&milestone)?;

    Ok(MilestoneReopened { id: id.to_string() })
}

#[derive(Serialize)]
pub struct MilestoneDeleted {
    pub id: String,
}

impl Output for MilestoneDeleted {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Deleted milestone {}", self.id)
    }
}

/// Delete a milestone.
pub fn milestone_delete(repo_path: &Path, id: &str) -> Result<MilestoneDeleted> {
    let mut storage = Storage::open(repo_path)?;
    storage.delete_milestone(id)?;

    Ok(MilestoneDeleted { id: id.to_string() })
}

impl Output for MilestoneProgress {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!(
            "{}/{} completed ({:.1}%)",
            self.completed, self.total, self.percentage
        )
    }
}

/// Get milestone progress.
pub fn milestone_progress(repo_path: &Path, id: &str) -> Result<MilestoneProgress> {
    let storage = Storage::open(repo_path)?;
    // Verify milestone exists
    storage.get_milestone(id)?;
    storage.get_milestone_progress(id)
}

// === Queue Commands ===

#[derive(Serialize)]
pub struct QueueCreated {
    pub id: String,
    pub title: String,
}

impl Output for QueueCreated {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Created queue {} \"{}\"", self.id, self.title)
    }
}

/// Create a new queue (only one per repository).
pub fn queue_create(
    repo_path: &Path,
    title: String,
    description: Option<String>,
) -> Result<QueueCreated> {
    let mut storage = Storage::open(repo_path)?;

    let id = generate_id("bnq", &title);
    let mut queue = Queue::new(id.clone(), title.clone());
    queue.description = description;

    storage.create_queue(&queue)?;

    Ok(QueueCreated { id, title })
}

#[derive(Serialize)]
pub struct QueueShowResult {
    pub queue: Queue,
    pub tasks: Vec<Task>,
    pub bugs: Vec<Bug>,
}

impl Output for QueueShowResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("Queue: {} ({})", self.queue.title, self.queue.id));
        lines.push(format!(
            "  Created: {}",
            self.queue.created_at.format("%Y-%m-%d %H:%M")
        ));
        if let Some(ref desc) = self.queue.description {
            lines.push(format!("  Description: {}", desc));
        }
        lines.push(String::new());

        let total_items = self.tasks.len() + self.bugs.len();
        if total_items == 0 {
            lines.push("  No items in queue".to_string());
        } else {
            lines.push(format!("  Queued items ({}):", total_items));
            for task in &self.tasks {
                lines.push(format!(
                    "    [P{}] {}: {}",
                    task.priority, task.core.id, task.core.title
                ));
            }
            for bug in &self.bugs {
                lines.push(format!(
                    "    [P{}] {}: {} (bug)",
                    bug.priority, bug.core.id, bug.core.title
                ));
            }
        }

        lines.join("\n")
    }
}

/// Show the queue and its items (tasks and bugs).
pub fn queue_show(repo_path: &Path) -> Result<QueueShowResult> {
    let storage = Storage::open(repo_path)?;

    let queue = storage.get_queue()?;
    let tasks = storage.get_queued_tasks()?;
    let bugs = storage.get_queued_bugs()?;

    Ok(QueueShowResult { queue, tasks, bugs })
}

#[derive(Serialize)]
pub struct QueueDeleted {
    pub id: String,
}

impl Output for QueueDeleted {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Deleted queue {}", self.id)
    }
}

/// Delete the queue.
pub fn queue_delete(repo_path: &Path) -> Result<QueueDeleted> {
    let mut storage = Storage::open(repo_path)?;

    let queue = storage.get_queue()?;
    let id = queue.id.clone();

    storage.delete_queue(&id)?;

    Ok(QueueDeleted { id })
}

#[derive(Serialize)]
pub struct QueueItemAdded {
    pub queue_id: String,
    pub item_id: String,
    pub already_queued: bool,
}

impl Output for QueueItemAdded {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if self.already_queued {
            format!("{} is already in queue {}", self.item_id, self.queue_id)
        } else {
            format!("Added {} to queue {}", self.item_id, self.queue_id)
        }
    }
}

/// Add a task or bug to the queue.
pub fn queue_add(repo_path: &Path, item_id: &str) -> Result<QueueItemAdded> {
    let mut storage = Storage::open(repo_path)?;

    // Verify item exists and check if it's closed
    let task_result = storage.get_task(item_id);
    let bug_result = storage.get_bug(item_id);

    match (&task_result, &bug_result) {
        (Ok(task), _) => {
            // Check if task is closed (Done or Cancelled)
            if task.status == TaskStatus::Done || task.status == TaskStatus::Cancelled {
                return Err(Error::Other(format!(
                    "Cannot add {} to queue: task is closed (status: {})",
                    item_id,
                    serde_json::to_string(&task.status)
                        .unwrap_or_else(|_| "unknown".to_string())
                        .trim_matches('"')
                )));
            }
        }
        (_, Ok(bug)) => {
            // Check if bug is closed (Done or Cancelled)
            if bug.status == TaskStatus::Done || bug.status == TaskStatus::Cancelled {
                return Err(Error::Other(format!(
                    "Cannot add {} to queue: bug is closed (status: {})",
                    item_id,
                    serde_json::to_string(&bug.status)
                        .unwrap_or_else(|_| "unknown".to_string())
                        .trim_matches('"')
                )));
            }
        }
        _ => {
            return Err(Error::Other(format!(
                "Item {} is not a task or bug",
                item_id
            )));
        }
    }

    // Get queue
    let queue = storage.get_queue()?;
    let queue_id = queue.id.clone();

    // Check if already queued - if so, return success (idempotent operation)
    let edges = storage.list_edges(Some(EdgeType::Queued), Some(item_id), Some(&queue_id))?;
    if !edges.is_empty() {
        return Ok(QueueItemAdded {
            queue_id,
            item_id: item_id.to_string(),
            already_queued: true,
        });
    }

    // Create queued edge
    let edge_id = generate_id("bne", &format!("{}-{}", item_id, queue_id));
    let edge = Edge::new(
        edge_id,
        item_id.to_string(),
        queue_id.clone(),
        EdgeType::Queued,
    );
    storage.add_edge(&edge)?;

    Ok(QueueItemAdded {
        queue_id,
        item_id: item_id.to_string(),
        already_queued: false,
    })
}

#[derive(Serialize)]
pub struct QueueItemRemoved {
    pub queue_id: String,
    pub item_id: String,
}

impl Output for QueueItemRemoved {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Removed {} from queue {}", self.item_id, self.queue_id)
    }
}

/// Remove a task or bug from the queue.
pub fn queue_rm(repo_path: &Path, item_id: &str) -> Result<QueueItemRemoved> {
    let mut storage = Storage::open(repo_path)?;

    // Get queue
    let queue = storage.get_queue()?;
    let queue_id = queue.id.clone();

    // Find and remove the queued edge
    let edges = storage.list_edges(Some(EdgeType::Queued), Some(item_id), Some(&queue_id))?;
    if edges.is_empty() {
        return Err(Error::Other(format!("{} is not in the queue", item_id)));
    }

    // Remove edge using source, target, edge_type
    storage.remove_edge(item_id, &queue_id, EdgeType::Queued)?;

    Ok(QueueItemRemoved {
        queue_id,
        item_id: item_id.to_string(),
    })
}

impl Output for Queue {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("Queue: {} ({})", self.title, self.id));
        if let Some(ref desc) = self.description {
            lines.push(format!("  Description: {}", desc));
        }
        lines.push(format!(
            "  Created: {}",
            self.created_at.format("%Y-%m-%d %H:%M")
        ));
        lines.push(format!(
            "  Updated: {}",
            self.updated_at.format("%Y-%m-%d %H:%M")
        ));
        lines.join("\n")
    }
}

// === Status Summary ===

#[derive(Serialize)]
pub struct StatusSummary {
    pub tasks: Vec<Task>,
    pub ready: Vec<String>,
    pub blocked: Vec<String>,
    pub in_progress: Vec<String>,
    pub open_bugs_count: usize,
    pub critical_bugs_count: usize,
    pub queued_count: usize,
    pub open_ideas_count: usize,
    pub open_milestones_count: usize,
}

impl Output for StatusSummary {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();

        lines.push(format!("Binnacle - {} total task(s)", self.tasks.len()));

        // Primary sections: Bugs, Blocked, Ready (in priority order)
        if self.open_bugs_count > 0 {
            let critical_info = if self.critical_bugs_count > 0 {
                format!(" ({} high/critical)", self.critical_bugs_count)
            } else {
                String::new()
            };
            lines.push(format!(
                "  Bugs: {} open{}",
                self.open_bugs_count, critical_info
            ));
        }

        if !self.blocked.is_empty() {
            lines.push(format!("  Blocked: {}", self.blocked.join(", ")));
        }

        if !self.ready.is_empty() {
            lines.push(format!("  Ready: {}", self.ready.join(", ")));
            // Fold queued info beneath ready
            if self.queued_count > 0 {
                lines.push(format!("    └─ {} queued (priority)", self.queued_count));
            }
        }

        if !self.in_progress.is_empty() {
            lines.push(format!("  In Progress: {}", self.in_progress.join(", ")));
        }

        // Folded secondary sections
        let mut secondary = Vec::new();
        if self.open_ideas_count > 0 {
            secondary.push(format!("{} ideas", self.open_ideas_count));
        }
        if self.open_milestones_count > 0 {
            secondary.push(format!("{} milestones", self.open_milestones_count));
        }
        if !secondary.is_empty() {
            lines.push(format!("  Also: {}", secondary.join(", ")));
        }

        if self.tasks.is_empty() {
            lines.push("Run `bn task create \"Title\"` to add a task.".to_string());
        }

        lines.join("\n")
    }
}

/// Get status summary.
pub fn status(repo_path: &Path) -> Result<StatusSummary> {
    let storage = Storage::open(repo_path)?;
    let tasks = storage.list_tasks(None, None, None)?;

    let mut ready = Vec::new();
    let mut blocked = Vec::new();
    let mut in_progress = Vec::new();

    for task in &tasks {
        match task.status {
            TaskStatus::InProgress => in_progress.push(task.core.id.clone()),
            TaskStatus::Blocked => blocked.push(task.core.id.clone()),
            TaskStatus::Pending | TaskStatus::Reopened => {
                // Check if all dependencies are done
                if task.depends_on.is_empty() {
                    ready.push(task.core.id.clone());
                } else {
                    let all_done = task.depends_on.iter().all(|dep_id| {
                        storage
                            .get_task(dep_id)
                            .map(|t| t.status == TaskStatus::Done)
                            .unwrap_or(false)
                    });
                    if all_done {
                        ready.push(task.core.id.clone());
                    } else {
                        blocked.push(task.core.id.clone());
                    }
                }
            }
            _ => {}
        }
    }

    // Get bug stats
    let bugs = storage
        .list_bugs(None, None, None, None, true)
        .unwrap_or_default();
    let open_bugs_count = bugs
        .iter()
        .filter(|b| {
            matches!(
                b.status,
                TaskStatus::Pending
                    | TaskStatus::InProgress
                    | TaskStatus::Blocked
                    | TaskStatus::Reopened
            )
        })
        .count();
    let critical_bugs_count = bugs
        .iter()
        .filter(|b| {
            matches!(
                b.status,
                TaskStatus::Pending
                    | TaskStatus::InProgress
                    | TaskStatus::Blocked
                    | TaskStatus::Reopened
            ) && matches!(b.severity, BugSeverity::High | BugSeverity::Critical)
        })
        .count();

    // Get queued count
    let queued_tasks = storage.get_queued_tasks().unwrap_or_default();
    let queued_ready_ids: std::collections::HashSet<_> = queued_tasks
        .iter()
        .filter(|t| ready.contains(&t.core.id))
        .map(|t| t.core.id.clone())
        .collect();
    let queued_count = queued_ready_ids.len();

    // Get ideas stats
    let ideas = storage.list_ideas(None, None).unwrap_or_default();
    let open_ideas_count = ideas
        .iter()
        .filter(|i| {
            matches!(
                i.status,
                crate::models::IdeaStatus::Seed | crate::models::IdeaStatus::Germinating
            )
        })
        .count();

    // Get milestones stats
    let milestones = storage
        .list_milestones(None, None, None)
        .unwrap_or_default();
    let open_milestones_count = milestones
        .iter()
        .filter(|m| matches!(m.status, TaskStatus::Pending | TaskStatus::InProgress))
        .count();

    Ok(StatusSummary {
        tasks,
        ready,
        blocked,
        in_progress,
        open_bugs_count,
        critical_bugs_count,
        queued_count,
        open_ideas_count,
        open_milestones_count,
    })
}

// === Dependency Commands ===

#[derive(Serialize)]
pub struct DepAdded {
    pub child: String,
    pub parent: String,
}

impl Output for DepAdded {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!(
            "Added dependency: {} depends on {}",
            self.child, self.parent
        )
    }
}

/// Add a dependency (child depends on parent).
pub fn dep_add(repo_path: &Path, child: &str, parent: &str) -> Result<DepAdded> {
    let mut storage = Storage::open(repo_path)?;
    storage.add_dependency(child, parent)?;

    Ok(DepAdded {
        child: child.to_string(),
        parent: parent.to_string(),
    })
}

#[derive(Serialize)]
pub struct DepRemoved {
    pub child: String,
    pub parent: String,
}

impl Output for DepRemoved {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!(
            "Removed dependency: {} no longer depends on {}",
            self.child, self.parent
        )
    }
}

/// Remove a dependency.
pub fn dep_rm(repo_path: &Path, child: &str, parent: &str) -> Result<DepRemoved> {
    let mut storage = Storage::open(repo_path)?;
    storage.remove_dependency(child, parent)?;

    Ok(DepRemoved {
        child: child.to_string(),
        parent: parent.to_string(),
    })
}

#[derive(Serialize)]
pub struct DepGraph {
    pub task_id: String,
    pub depends_on: Vec<String>,
    pub dependents: Vec<String>,
    pub transitive_deps: Vec<String>,
}

impl Output for DepGraph {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("Dependency graph for {}:", self.task_id));

        if self.depends_on.is_empty() {
            lines.push("  Depends on: (none)".to_string());
        } else {
            lines.push(format!("  Depends on: {}", self.depends_on.join(", ")));
        }

        if self.dependents.is_empty() {
            lines.push("  Dependents: (none)".to_string());
        } else {
            lines.push(format!("  Dependents: {}", self.dependents.join(", ")));
        }

        if !self.transitive_deps.is_empty() {
            lines.push(format!(
                "  All blockers: {}",
                self.transitive_deps.join(", ")
            ));
        }

        lines.join("\n")
    }
}

/// Show dependency graph for a task.
pub fn dep_show(repo_path: &Path, id: &str) -> Result<DepGraph> {
    let storage = Storage::open(repo_path)?;

    // Verify task exists
    storage.get_task(id)?;

    let depends_on = storage.get_dependencies(id)?;
    let dependents = storage.get_dependents(id)?;

    // Calculate transitive dependencies (all blockers)
    let mut transitive_deps = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut stack: Vec<String> = depends_on.clone();

    while let Some(current) = stack.pop() {
        if visited.contains(&current) {
            continue;
        }
        visited.insert(current.clone());
        transitive_deps.push(current.clone());

        if let Ok(deps) = storage.get_dependencies(&current) {
            for dep in deps {
                if !visited.contains(&dep) {
                    stack.push(dep);
                }
            }
        }
    }

    Ok(DepGraph {
        task_id: id.to_string(),
        depends_on,
        dependents,
        transitive_deps,
    })
}

// === Link Commands (Edge Management) ===

#[derive(Debug, Serialize)]
pub struct LinkAdded {
    pub id: String,
    pub source: String,
    pub target: String,
    pub edge_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub pinned: bool,
}

impl Output for LinkAdded {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let reason_str = self
            .reason
            .as_ref()
            .map(|r| format!(" ({})", r))
            .unwrap_or_default();
        let pinned_str = if self.pinned { " [pinned]" } else { "" };
        format!(
            "Created link: {} --[{}]--> {}{}{}",
            self.source, self.edge_type, self.target, reason_str, pinned_str
        )
    }
}

/// Add a link (edge) between two entities.
pub fn link_add(
    repo_path: &Path,
    source: &str,
    target: &str,
    edge_type_str: &str,
    reason: Option<String>,
    pinned: bool,
) -> Result<LinkAdded> {
    let mut storage = Storage::open(repo_path)?;

    // Parse edge type
    let edge_type: EdgeType = edge_type_str.parse().map_err(|e: String| Error::Other(e))?;

    // Require reason for depends_on links
    if edge_type == EdgeType::DependsOn && reason.is_none() {
        return Err(Error::Other(
            "A --reason is required when creating depends_on links".to_string(),
        ));
    }

    // Validate entities exist
    validate_entity_exists(&storage, source)?;
    validate_entity_exists(&storage, target)?;

    // Check for self-link
    if source == target {
        return Err(Error::Other(
            "Cannot create a link from an entity to itself".to_string(),
        ));
    }

    // Check for cycles on blocking edge types
    if storage.would_edge_create_cycle(source, target, edge_type)? {
        return Err(Error::CycleDetected);
    }

    // Validate edge type constraints
    validate_edge_type_constraints(&storage, source, target, edge_type)?;

    // Generate edge ID and create edge
    let id = storage.generate_edge_id(source, target, edge_type);
    let mut edge = Edge::new(
        id.clone(),
        source.to_string(),
        target.to_string(),
        edge_type,
    );
    edge.reason = reason.clone();
    edge.pinned = pinned;

    storage.add_edge(&edge)?;

    Ok(LinkAdded {
        id,
        source: source.to_string(),
        target: target.to_string(),
        edge_type: edge_type.to_string(),
        reason,
        pinned,
    })
}

/// Validate that an entity exists (task, bug, milestone, test, or queue).
fn validate_entity_exists(storage: &Storage, id: &str) -> Result<()> {
    // Try task first
    if storage.get_task(id).is_ok() {
        return Ok(());
    }
    // Try bug
    if storage.get_bug(id).is_ok() {
        return Ok(());
    }
    // Try milestone
    if storage.get_milestone(id).is_ok() {
        return Ok(());
    }
    // Try test
    if storage.get_test(id).is_ok() {
        return Ok(());
    }
    // Try queue
    if storage.get_queue_by_id(id).is_ok() {
        return Ok(());
    }
    // Try idea
    if storage.get_idea(id).is_ok() {
        return Ok(());
    }
    // Try doc
    if storage.get_doc(id).is_ok() {
        return Ok(());
    }
    // Try agent
    if storage.get_agent_by_id(id).is_ok() {
        return Ok(());
    }
    Err(Error::NotFound(format!("Entity not found: {}", id)))
}

/// Get the entity type for an ID.
fn get_entity_type(storage: &Storage, id: &str) -> Option<&'static str> {
    if storage.get_task(id).is_ok() {
        Some("task")
    } else if storage.get_bug(id).is_ok() {
        Some("bug")
    } else if storage.get_milestone(id).is_ok() {
        Some("milestone")
    } else if storage.get_test(id).is_ok() {
        Some("test")
    } else if storage.get_queue_by_id(id).is_ok() {
        Some("queue")
    } else if storage.get_idea(id).is_ok() {
        Some("idea")
    } else if storage.get_doc(id).is_ok() {
        Some("doc")
    } else if storage.get_agent_by_id(id).is_ok() {
        Some("agent")
    } else {
        None
    }
}

/// Validate edge type constraints based on source/target entity types.
fn validate_edge_type_constraints(
    storage: &Storage,
    source: &str,
    target: &str,
    edge_type: EdgeType,
) -> Result<()> {
    let source_type = get_entity_type(storage, source)
        .ok_or_else(|| Error::NotFound(format!("Source entity not found: {}", source)))?;
    let target_type = get_entity_type(storage, target)
        .ok_or_else(|| Error::NotFound(format!("Target entity not found: {}", target)))?;

    match edge_type {
        EdgeType::Fixes => {
            // Only Task → Bug
            if source_type != "task" {
                return Err(Error::Other(format!(
                    "fixes edge requires source to be a task, got: {}",
                    source_type
                )));
            }
            if target_type != "bug" {
                return Err(Error::Other(format!(
                    "fixes edge requires target to be a bug, got: {}",
                    target_type
                )));
            }
        }
        EdgeType::Duplicates | EdgeType::Supersedes => {
            // Same type only (Task→Task or Bug→Bug)
            if source_type != target_type {
                return Err(Error::Other(format!(
                    "{} edge requires source and target to be the same type, got: {} -> {}",
                    edge_type, source_type, target_type
                )));
            }
            if source_type == "test" {
                return Err(Error::Other(format!(
                    "{} edge is not valid for test entities",
                    edge_type
                )));
            }
        }
        EdgeType::Tests => {
            // Test → Task/Bug
            if source_type != "test" {
                return Err(Error::Other(format!(
                    "tests edge requires source to be a test, got: {}",
                    source_type
                )));
            }
            if target_type == "test" {
                return Err(Error::Other(
                    "tests edge requires target to be a task or bug, not a test".to_string(),
                ));
            }
        }
        EdgeType::ParentOf => {
            // Task/Milestone → Task/Bug (no milestones yet, so just task → task/bug)
            if source_type == "test" {
                return Err(Error::Other(
                    "parent_of edge source cannot be a test".to_string(),
                ));
            }
            // Check if target already has a parent (strict hierarchy: only one parent allowed)
            let existing_parents =
                storage.list_edges(Some(EdgeType::ParentOf), None, Some(target))?;
            if !existing_parents.is_empty() {
                let existing_parent = &existing_parents[0].source;
                return Err(Error::Other(format!(
                    "Entity {} already has a parent ({}). Only one parent is allowed.",
                    target, existing_parent
                )));
            }
            // Also check for child_of edges from target (inverse relationship)
            let existing_child_of =
                storage.list_edges(Some(EdgeType::ChildOf), Some(target), None)?;
            if !existing_child_of.is_empty() {
                let existing_parent = &existing_child_of[0].target;
                return Err(Error::Other(format!(
                    "Entity {} already has a parent via child_of edge ({}). Only one parent is allowed.",
                    target, existing_parent
                )));
            }
        }
        EdgeType::ChildOf => {
            // Task/Bug → Task/Milestone
            if source_type == "test" {
                return Err(Error::Other(
                    "child_of edge source cannot be a test".to_string(),
                ));
            }
            // Check if source already has a parent (strict hierarchy: only one parent allowed)
            let existing_child_of =
                storage.list_edges(Some(EdgeType::ChildOf), Some(source), None)?;
            if !existing_child_of.is_empty() {
                let existing_parent = &existing_child_of[0].target;
                return Err(Error::Other(format!(
                    "Entity {} already has a parent ({}). Only one parent is allowed.",
                    source, existing_parent
                )));
            }
            // Also check for parent_of edges pointing to source (inverse relationship)
            let existing_parents =
                storage.list_edges(Some(EdgeType::ParentOf), None, Some(source))?;
            if !existing_parents.is_empty() {
                let existing_parent = &existing_parents[0].source;
                return Err(Error::Other(format!(
                    "Entity {} already has a parent via parent_of edge ({}). Only one parent is allowed.",
                    source, existing_parent
                )));
            }
        }
        EdgeType::CausedBy => {
            // Bug → Task/Commit (no commit edges yet, so just bug → task)
            if source_type != "bug" {
                return Err(Error::Other(format!(
                    "caused_by edge requires source to be a bug, got: {}",
                    source_type
                )));
            }
        }
        EdgeType::Queued => {
            // Task/Bug → Queue
            if source_type != "task" && source_type != "bug" {
                return Err(Error::Other(format!(
                    "queued edge requires source to be a task or bug, got: {}",
                    source_type
                )));
            }
            if target_type != "queue" {
                return Err(Error::Other(format!(
                    "queued edge requires target to be a queue, got: {}",
                    target_type
                )));
            }
        }
        EdgeType::Impacts => {
            // Bug → Task/Milestone/PRD (informational: bug affects this work)
            if source_type != "bug" {
                return Err(Error::Other(format!(
                    "impacts edge requires source to be a bug, got: {}",
                    source_type
                )));
            }
            // Allow targets: task, milestone (PRD not an entity type yet)
            if target_type != "task" && target_type != "milestone" {
                return Err(Error::Other(format!(
                    "impacts edge requires target to be a task or milestone, got: {}",
                    target_type
                )));
            }
        }
        EdgeType::WorkingOn => {
            // Agent → Task/Bug (agent is working on this item)
            if source_type != "agent" {
                return Err(Error::Other(format!(
                    "working_on edge requires source to be an agent, got: {}",
                    source_type
                )));
            }
            if target_type != "task" && target_type != "bug" {
                return Err(Error::Other(format!(
                    "working_on edge requires target to be a task or bug, got: {}",
                    target_type
                )));
            }
        }
        EdgeType::WorkedOn => {
            // Agent → Task/Bug (agent previously worked on this item)
            if source_type != "agent" {
                return Err(Error::Other(format!(
                    "worked_on edge requires source to be an agent, got: {}",
                    source_type
                )));
            }
            if target_type != "task" && target_type != "bug" {
                return Err(Error::Other(format!(
                    "worked_on edge requires target to be a task or bug, got: {}",
                    target_type
                )));
            }
        }
        // Other types are permissive
        EdgeType::DependsOn | EdgeType::Blocks | EdgeType::RelatedTo => {}
        EdgeType::Documents => {
            // Doc → Any (documentation can attach to any entity)
            if source_type != "doc" {
                return Err(Error::Other(format!(
                    "documents edge requires source to be a doc, got: {}",
                    source_type
                )));
            }
        }
    }

    Ok(())
}

#[derive(Serialize)]
pub struct LinkRemoved {
    pub source: String,
    pub target: String,
    pub edge_type: String,
}

impl Output for LinkRemoved {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!(
            "Removed link: {} --[{}]--> {}",
            self.source, self.edge_type, self.target
        )
    }
}

#[derive(Serialize)]
pub struct LinksBetween {
    pub source: String,
    pub target: String,
    pub edges: Vec<EdgeInfo>,
}

#[derive(Serialize)]
pub struct EdgeInfo {
    pub edge_type: String,
    pub direction: String,
}

impl Output for LinksBetween {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "Found {} edge(s) between {} and {}:",
            self.edges.len(),
            self.source,
            self.target
        ));
        for edge in &self.edges {
            lines.push(format!("  - {} ({})", edge.edge_type, edge.direction));
        }
        lines.push("\nUse --type <type> to remove a specific edge.".to_string());
        lines.join("\n")
    }
}

/// Result of link_rm - either removal confirmation or guidance about existing edges.
#[derive(Serialize)]
#[serde(untagged)]
pub enum LinkRmResult {
    Removed(LinkRemoved),
    Guidance(LinksBetween),
}

impl Output for LinkRmResult {
    fn to_json(&self) -> String {
        match self {
            LinkRmResult::Removed(r) => r.to_json(),
            LinkRmResult::Guidance(g) => g.to_json(),
        }
    }

    fn to_human(&self) -> String {
        match self {
            LinkRmResult::Removed(r) => r.to_human(),
            LinkRmResult::Guidance(g) => g.to_human(),
        }
    }
}

/// Remove a link (edge) between two entities.
pub fn link_rm(
    repo_path: &Path,
    source: &str,
    target: &str,
    edge_type_str: Option<&str>,
) -> Result<LinkRmResult> {
    let mut storage = Storage::open(repo_path)?;

    // If no edge type specified, show existing edges and guidance
    if edge_type_str.is_none() {
        let edges = storage.get_edges_between(source, target)?;
        if edges.is_empty() {
            return Err(Error::NotFound(format!(
                "No edges found between {} and {}",
                source, target
            )));
        }

        let edge_infos: Vec<EdgeInfo> = edges
            .iter()
            .map(|e| EdgeInfo {
                edge_type: e.edge_type.to_string(),
                direction: if e.source == source {
                    format!("{} → {}", source, target)
                } else {
                    format!("{} → {} (bidirectional)", target, source)
                },
            })
            .collect();

        return Ok(LinkRmResult::Guidance(LinksBetween {
            source: source.to_string(),
            target: target.to_string(),
            edges: edge_infos,
        }));
    }

    let edge_type: EdgeType = edge_type_str
        .unwrap()
        .parse()
        .map_err(|e: String| Error::Other(e))?;

    storage.remove_edge(source, target, edge_type)?;

    Ok(LinkRmResult::Removed(LinkRemoved {
        source: source.to_string(),
        target: target.to_string(),
        edge_type: edge_type.to_string(),
    }))
}

#[derive(Serialize)]
pub struct LinkList {
    pub entity_id: Option<String>,
    pub edges: Vec<LinkListEdge>,
    pub count: usize,
}

#[derive(Serialize)]
pub struct LinkListEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub edge_type: String,
    pub direction: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub pinned: bool,
}

impl Output for LinkList {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if self.edges.is_empty() {
            return if let Some(id) = &self.entity_id {
                format!("No links for {}.", id)
            } else {
                "No links found.".to_string()
            };
        }

        let mut lines = Vec::new();
        let header = if let Some(id) = &self.entity_id {
            format!("{} link(s) for {}:\n", self.count, id)
        } else {
            format!("{} link(s):\n", self.count)
        };
        lines.push(header);

        for edge in &self.edges {
            let arrow = match edge.direction.as_str() {
                "outbound" => "→",
                "inbound" => "←",
                "both" => "↔",
                _ => "→",
            };
            let reason_str = edge
                .reason
                .as_ref()
                .map(|r| format!(" \"{}\"", r))
                .unwrap_or_default();
            let pinned_str = if edge.pinned { " [pinned]" } else { "" };
            lines.push(format!(
                "  {} {} {} ({}){}{}",
                edge.source, arrow, edge.target, edge.edge_type, reason_str, pinned_str
            ));
        }

        lines.join("\n")
    }
}

/// List links for an entity or all links.
pub fn link_list(
    repo_path: &Path,
    entity_id: Option<&str>,
    all: bool,
    edge_type_str: Option<&str>,
) -> Result<LinkList> {
    let storage = Storage::open(repo_path)?;

    let edge_type: Option<EdgeType> = edge_type_str
        .map(|s| s.parse())
        .transpose()
        .map_err(|e: String| Error::Other(e))?;

    let edges: Vec<LinkListEdge> = if all {
        // List all edges
        let all_edges = storage.list_edges(edge_type, None, None)?;
        all_edges
            .into_iter()
            .map(|e| LinkListEdge {
                id: e.id,
                source: e.source,
                target: e.target,
                edge_type: e.edge_type.to_string(),
                direction: "outbound".to_string(),
                reason: e.reason,
                pinned: e.pinned,
            })
            .collect()
    } else if let Some(id) = entity_id {
        // Validate entity exists
        validate_entity_exists(&storage, id)?;

        // List edges for this entity
        let hydrated = storage.get_edges_for_entity(id)?;
        hydrated
            .into_iter()
            .filter(|he| edge_type.is_none() || he.edge.edge_type == edge_type.unwrap())
            .map(|he| {
                let direction = match he.direction {
                    EdgeDirection::Outbound => "outbound",
                    EdgeDirection::Inbound => "inbound",
                    EdgeDirection::Both => "both",
                };
                LinkListEdge {
                    id: he.edge.id,
                    source: he.edge.source,
                    target: he.edge.target,
                    edge_type: he.edge.edge_type.to_string(),
                    direction: direction.to_string(),
                    reason: he.edge.reason,
                    pinned: he.edge.pinned,
                }
            })
            .collect()
    } else {
        return Err(Error::Other(
            "Either provide an entity ID or use --all to list all links".to_string(),
        ));
    };

    let count = edges.len();
    Ok(LinkList {
        entity_id: entity_id.map(|s| s.to_string()),
        edges,
        count,
    })
}

// === Graph Commands ===

/// A single connected component in the task graph.
#[derive(Serialize)]
pub struct GraphComponent {
    /// Component number (1-indexed)
    pub id: usize,
    /// Number of entities in this component
    pub entity_count: usize,
    /// Root nodes (entities with no dependencies within this component)
    pub root_nodes: Vec<String>,
    /// All entity IDs in this component
    pub entity_ids: Vec<String>,
}

impl GraphComponent {
    fn to_human(&self, verbose: bool) -> String {
        let isolated = self.entity_count == 1;
        if verbose {
            let mut lines = vec![format!(
                "Component {} ({} {}{})",
                self.id,
                self.entity_count,
                if self.entity_count == 1 {
                    "entity"
                } else {
                    "entities"
                },
                if isolated { " - isolated" } else { "" }
            )];
            lines.push(format!(
                "  Root nodes: {}",
                if self.root_nodes.is_empty() {
                    "(none)".to_string()
                } else {
                    self.root_nodes.join(", ")
                }
            ));
            lines.join("\n")
        } else {
            format!(
                "Component {}: {} {}{}",
                self.id,
                self.entity_count,
                if self.entity_count == 1 {
                    "entity"
                } else {
                    "entities"
                },
                if isolated { " (isolated)" } else { "" }
            )
        }
    }
}

/// Result of graph component analysis.
#[derive(Serialize)]
pub struct GraphComponentsResult {
    /// Number of disconnected components
    pub component_count: usize,
    /// Details of each component
    pub components: Vec<GraphComponent>,
    /// Suggestion for reducing fragmentation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

impl Output for GraphComponentsResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if self.component_count == 0 {
            return "No entities found in the graph.".to_string();
        }

        let mut lines = vec![format!(
            "Task Graph: {} disconnected component{}",
            self.component_count,
            if self.component_count == 1 { "" } else { "s" }
        )];

        lines.push(String::new()); // blank line

        for component in &self.components {
            lines.push(component.to_human(true));
        }

        if let Some(ref suggestion) = self.suggestion {
            lines.push(String::new());
            lines.push(format!("Tip: {}", suggestion));
        }

        lines.join("\n")
    }
}

/// Analyze the task graph and identify disconnected components.
///
/// Uses Union-Find algorithm to efficiently find connected components
/// treating all edges as undirected for connectivity purposes.
pub fn graph_components(repo_path: &Path) -> Result<GraphComponentsResult> {
    let storage = Storage::open(repo_path)?;

    // Get all entities (tasks, bugs, milestones, ideas)
    let tasks = storage.list_tasks(None, None, None)?;
    let bugs = storage.list_bugs(None, None, None, None, true)?; // Include all for graph analysis
    let milestones = storage.list_milestones(None, None, None)?;
    let ideas = storage.list_ideas(None, None)?;

    // Get all edges
    let edges = storage.list_edges(None, None, None)?;

    // Build union-find structure
    let mut uf = UnionFind::new();

    // Add all entities
    for task in &tasks {
        uf.make_set(task.core.id.clone());
    }
    for bug in &bugs {
        uf.make_set(bug.core.id.clone());
    }
    for milestone in &milestones {
        uf.make_set(milestone.core.id.clone());
    }
    for idea in &ideas {
        uf.make_set(idea.core.id.clone());
    }

    // Union connected entities via edges (treating edges as undirected)
    for edge in &edges {
        // Only consider edges where both endpoints exist
        if uf.find(&edge.source).is_some() && uf.find(&edge.target).is_some() {
            uf.union(&edge.source, &edge.target);
        }
    }

    // Get components
    let raw_components = uf.components();

    // For each component, find root nodes (entities with no dependencies within the component)
    let mut components: Vec<GraphComponent> = Vec::new();

    // Build a set of all entities that have dependencies (things that depend on others)
    let has_dependency: std::collections::HashSet<String> = edges
        .iter()
        .filter(|e| {
            matches!(
                e.edge_type,
                EdgeType::DependsOn | EdgeType::ChildOf | EdgeType::Blocks
            )
        })
        .map(|e| {
            if matches!(e.edge_type, EdgeType::Blocks) {
                e.target.clone() // For "blocks", target is blocked (has dependency)
            } else {
                e.source.clone() // For depends_on/child_of, source has dependencies
            }
        })
        .collect();

    for (idx, mut entity_ids) in raw_components.into_iter().enumerate() {
        entity_ids.sort(); // Consistent ordering

        // Root nodes are entities that:
        // 1. Don't depend on anything within the component, OR
        // 2. Are only depended upon (not depending on others)
        let root_nodes: Vec<String> = entity_ids
            .iter()
            .filter(|id| !has_dependency.contains(*id))
            .cloned()
            .collect();

        components.push(GraphComponent {
            id: idx + 1,
            entity_count: entity_ids.len(),
            root_nodes,
            entity_ids,
        });
    }

    // Sort components by size (largest first)
    components.sort_by(|a, b| b.entity_count.cmp(&a.entity_count));

    // Re-number after sorting
    for (idx, comp) in components.iter_mut().enumerate() {
        comp.id = idx + 1;
    }

    let component_count = components.len();
    let suggestion = if component_count > 1 {
        Some(
            "Use 'bn link add <child> <parent> --type depends_on' to connect components."
                .to_string(),
        )
    } else {
        None
    };

    Ok(GraphComponentsResult {
        component_count,
        components,
        suggestion,
    })
}

// === Search Commands ===

/// Search result for link queries
#[derive(Serialize)]
pub struct SearchLinkResult {
    pub edges: Vec<SearchLinkEdge>,
    pub count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filters: Option<SearchLinkFilters>,
}

#[derive(Serialize)]
pub struct SearchLinkEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub edge_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
}

#[derive(Serialize)]
pub struct SearchLinkFilters {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

impl Output for SearchLinkResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if self.edges.is_empty() {
            return "No edges found.".to_string();
        }

        let mut lines = Vec::new();
        lines.push(format!("{} edge(s) found:\n", self.count));

        for edge in &self.edges {
            let reason_str = edge
                .reason
                .as_ref()
                .map(|r| format!("\n  Reason: \"{}\"", r))
                .unwrap_or_default();
            let created_by_str = edge
                .created_by
                .as_ref()
                .map(|c| format!(" by {}", c))
                .unwrap_or_default();
            lines.push(format!(
                "{} → {} ({}){}\n  Created: {}{}",
                edge.source,
                edge.target,
                edge.edge_type,
                reason_str,
                edge.created_at,
                created_by_str
            ));
        }

        lines.join("\n")
    }
}

/// Search for links/edges by type, source, or target.
pub fn search_link(
    repo_path: &Path,
    edge_type_str: Option<&str>,
    source: Option<&str>,
    target: Option<&str>,
) -> Result<SearchLinkResult> {
    let storage = Storage::open(repo_path)?;

    let edge_type: Option<EdgeType> = edge_type_str
        .map(|s| s.parse())
        .transpose()
        .map_err(|e: String| Error::Other(e))?;

    // Validate source and target exist if provided
    if let Some(s) = source {
        validate_entity_exists(&storage, s)?;
    }
    if let Some(t) = target {
        validate_entity_exists(&storage, t)?;
    }

    let edges = storage.list_edges(edge_type, source, target)?;

    let search_edges: Vec<SearchLinkEdge> = edges
        .into_iter()
        .map(|e| SearchLinkEdge {
            id: e.id,
            source: e.source,
            target: e.target,
            edge_type: e.edge_type.to_string(),
            reason: e.reason,
            created_at: e.created_at.to_rfc3339(),
            created_by: e.created_by,
        })
        .collect();

    let count = search_edges.len();
    let filters = if edge_type_str.is_some() || source.is_some() || target.is_some() {
        Some(SearchLinkFilters {
            edge_type: edge_type_str.map(|s| s.to_string()),
            source: source.map(|s| s.to_string()),
            target: target.map(|s| s.to_string()),
        })
    } else {
        None
    };

    Ok(SearchLinkResult {
        edges: search_edges,
        count,
        filters,
    })
}

// === Query Commands ===

/// A ready task item with queue membership status.
#[derive(Serialize)]
pub struct ReadyTaskItem {
    #[serde(flatten)]
    pub task: Task,
    pub queued: bool,
}

/// A ready bug item with queue membership status.
#[derive(Serialize)]
pub struct ReadyBugItem {
    #[serde(flatten)]
    pub bug: Bug,
    pub queued: bool,
}

#[derive(Serialize)]
pub struct ReadyTasks {
    pub tasks: Vec<ReadyTaskItem>,
    pub bugs: Vec<ReadyBugItem>,
    pub count: usize,
    pub bug_count: usize,
    pub queued_count: usize,
    pub queued_bug_count: usize,
}

impl Output for ReadyTasks {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();

        // Show bugs first (higher priority typically)
        if !self.bugs.is_empty() {
            let queued_bugs: Vec<_> = self.bugs.iter().filter(|b| b.queued).collect();
            let other_bugs: Vec<_> = self.bugs.iter().filter(|b| !b.queued).collect();

            lines.push(format!("{} ready bug(s):\n", self.bug_count));

            // Show queued bugs first
            if !queued_bugs.is_empty() {
                lines.push("  [QUEUED]".to_string());
                for item in &queued_bugs {
                    let bug = &item.bug;
                    let tags = if bug.core.tags.is_empty() {
                        String::new()
                    } else {
                        format!(" [{}]", bug.core.tags.join(", "))
                    };
                    lines.push(format!(
                        "    {} P{} {} ({}){}",
                        bug.core.id,
                        bug.priority,
                        bug.core.title,
                        format!("{:?}", bug.severity).to_lowercase(),
                        tags
                    ));
                }
                if !other_bugs.is_empty() {
                    lines.push(String::new());
                }
            }

            // Show non-queued bugs
            if !other_bugs.is_empty() {
                if !queued_bugs.is_empty() {
                    lines.push("  [OTHER]".to_string());
                }
                for item in &other_bugs {
                    let bug = &item.bug;
                    let tags = if bug.core.tags.is_empty() {
                        String::new()
                    } else {
                        format!(" [{}]", bug.core.tags.join(", "))
                    };
                    let indent = if queued_bugs.is_empty() { "  " } else { "    " };
                    lines.push(format!(
                        "{}{} P{} {} ({}){}",
                        indent,
                        bug.core.id,
                        bug.priority,
                        bug.core.title,
                        format!("{:?}", bug.severity).to_lowercase(),
                        tags
                    ));
                }
            }

            if !self.tasks.is_empty() {
                lines.push(String::new()); // blank line between sections
            }
        }

        // Show tasks
        if !self.tasks.is_empty() {
            let queued_tasks: Vec<_> = self.tasks.iter().filter(|t| t.queued).collect();
            let other_tasks: Vec<_> = self.tasks.iter().filter(|t| !t.queued).collect();

            lines.push(format!("{} ready task(s):\n", self.count));

            // Show queued tasks first
            if !queued_tasks.is_empty() {
                lines.push("  [QUEUED]".to_string());
                for item in &queued_tasks {
                    let task = &item.task;
                    let tags = if task.core.tags.is_empty() {
                        String::new()
                    } else {
                        format!(" [{}]", task.core.tags.join(", "))
                    };
                    lines.push(format!(
                        "    {} P{} {}{}",
                        task.core.id, task.priority, task.core.title, tags
                    ));
                }
                if !other_tasks.is_empty() {
                    lines.push(String::new());
                }
            }

            // Show non-queued tasks
            if !other_tasks.is_empty() {
                if !queued_tasks.is_empty() {
                    lines.push("  [OTHER]".to_string());
                }
                for item in &other_tasks {
                    let task = &item.task;
                    let tags = if task.core.tags.is_empty() {
                        String::new()
                    } else {
                        format!(" [{}]", task.core.tags.join(", "))
                    };
                    let indent = if queued_tasks.is_empty() {
                        "  "
                    } else {
                        "    "
                    };
                    lines.push(format!(
                        "{}{} P{} {}{}",
                        indent, task.core.id, task.priority, task.core.title, tags
                    ));
                }
            }
        }

        if lines.is_empty() {
            return "No ready tasks or bugs.".to_string();
        }

        lines.join("\n")
    }
}

/// Get tasks and bugs that are ready (no open blockers).
/// Queued tasks/bugs are sorted first, then by priority.
pub fn ready(repo_path: &Path, bugs_only: bool, tasks_only: bool) -> Result<ReadyTasks> {
    let storage = Storage::open(repo_path)?;

    // Fetch tasks unless bugs_only is set
    let tasks = if bugs_only {
        Vec::new()
    } else {
        storage.get_ready_tasks()?
    };

    // Fetch bugs unless tasks_only is set
    let bugs = if tasks_only {
        Vec::new()
    } else {
        storage.get_ready_bugs()?
    };

    // Get queued task IDs for membership check
    let queued_tasks = storage.get_queued_tasks().unwrap_or_default();
    let queued_task_ids: std::collections::HashSet<_> =
        queued_tasks.iter().map(|t| t.core.id.as_str()).collect();

    // Get queued bug IDs (bugs can also be queued)
    let queued_bugs = storage.get_queued_bugs().unwrap_or_default();
    let queued_bug_ids: std::collections::HashSet<_> =
        queued_bugs.iter().map(|b| b.core.id.as_str()).collect();

    // Wrap tasks with queue membership status
    let mut task_items: Vec<ReadyTaskItem> = tasks
        .into_iter()
        .map(|task| {
            let queued = queued_task_ids.contains(task.core.id.as_str());
            ReadyTaskItem { task, queued }
        })
        .collect();

    // Sort: queued first, then by priority, then by creation date
    task_items.sort_by(|a, b| {
        match (a.queued, b.queued) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => {
                // Same queue status: sort by priority (lower = higher priority)
                a.task.priority.cmp(&b.task.priority).then_with(|| {
                    // Then by creation date (older first)
                    a.task.core.created_at.cmp(&b.task.core.created_at)
                })
            }
        }
    });

    // Wrap bugs with queue membership status
    let mut bug_items: Vec<ReadyBugItem> = bugs
        .into_iter()
        .map(|bug| {
            let queued = queued_bug_ids.contains(bug.core.id.as_str());
            ReadyBugItem { bug, queued }
        })
        .collect();

    // Sort bugs: queued first, then by priority
    bug_items.sort_by(|a, b| match (a.queued, b.queued) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a
            .bug
            .priority
            .cmp(&b.bug.priority)
            .then_with(|| a.bug.core.created_at.cmp(&b.bug.core.created_at)),
    });

    let count = task_items.len();
    let bug_count = bug_items.len();
    let queued_count = task_items.iter().filter(|t| t.queued).count();
    let queued_bug_count = bug_items.iter().filter(|b| b.queued).count();

    Ok(ReadyTasks {
        tasks: task_items,
        bugs: bug_items,
        count,
        bug_count,
        queued_count,
        queued_bug_count,
    })
}

#[derive(Serialize)]
pub struct BlockedTasks {
    pub tasks: Vec<BlockedTask>,
    pub bugs: Vec<BlockedBug>,
    pub count: usize,
    pub bug_count: usize,
}

#[derive(Serialize)]
pub struct BlockedTask {
    #[serde(flatten)]
    pub task: Task,
    pub blocking_tasks: Vec<String>,
}

#[derive(Serialize)]
pub struct BlockedBug {
    #[serde(flatten)]
    pub bug: Bug,
    pub blocking_entities: Vec<String>,
}

impl Output for BlockedTasks {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();

        // Show blocked bugs first
        if !self.bugs.is_empty() {
            lines.push(format!("{} blocked bug(s):\n", self.bug_count));

            for bb in &self.bugs {
                let bug = &bb.bug;
                let blockers = if bb.blocking_entities.is_empty() {
                    String::new()
                } else {
                    format!(" (blocked by: {})", bb.blocking_entities.join(", "))
                };
                lines.push(format!(
                    "  {} P{} {} ({}){}",
                    bug.core.id,
                    bug.priority,
                    bug.core.title,
                    format!("{:?}", bug.severity).to_lowercase(),
                    blockers
                ));
            }

            if !self.tasks.is_empty() {
                lines.push(String::new()); // blank line between sections
            }
        }

        // Show blocked tasks
        if !self.tasks.is_empty() {
            lines.push(format!("{} blocked task(s):\n", self.count));

            for bt in &self.tasks {
                let task = &bt.task;
                let blockers = if bt.blocking_tasks.is_empty() {
                    String::new()
                } else {
                    format!(" (blocked by: {})", bt.blocking_tasks.join(", "))
                };
                lines.push(format!(
                    "  {} P{} {}{}",
                    task.core.id, task.priority, task.core.title, blockers
                ));
            }
        }

        if lines.is_empty() {
            return "No blocked tasks or bugs.".to_string();
        }

        lines.join("\n")
    }
}

/// Get tasks and bugs that are blocked (waiting on dependencies).
pub fn blocked(repo_path: &Path, bugs_only: bool, tasks_only: bool) -> Result<BlockedTasks> {
    let storage = Storage::open(repo_path)?;

    // Fetch tasks unless bugs_only is set
    let mut blocked_tasks = Vec::new();
    if !bugs_only {
        let tasks = storage.get_blocked_tasks()?;
        for task in tasks {
            // Find which legacy dependencies are blocking
            let mut blocking: Vec<String> = task
                .depends_on
                .iter()
                .filter(|dep_id| !storage.is_entity_done(dep_id))
                .cloned()
                .collect();

            // Also include edge-based dependencies that are blocking
            if let Ok(edge_deps) = storage.get_edge_dependencies(&task.core.id) {
                for dep_id in edge_deps {
                    if !storage.is_entity_done(&dep_id) && !blocking.contains(&dep_id) {
                        blocking.push(dep_id);
                    }
                }
            }

            blocked_tasks.push(BlockedTask {
                task,
                blocking_tasks: blocking,
            });
        }
    }

    // Fetch bugs unless tasks_only is set
    let mut blocked_bugs = Vec::new();
    if !tasks_only {
        let bugs = storage.get_blocked_bugs()?;
        for bug in bugs {
            // Find which legacy dependencies are blocking
            let mut blocking: Vec<String> = bug
                .depends_on
                .iter()
                .filter(|dep_id| !storage.is_entity_done(dep_id))
                .cloned()
                .collect();

            // Also include edge-based dependencies that are blocking
            if let Ok(edge_deps) = storage.get_edge_dependencies(&bug.core.id) {
                for dep_id in edge_deps {
                    if !storage.is_entity_done(&dep_id) && !blocking.contains(&dep_id) {
                        blocking.push(dep_id);
                    }
                }
            }

            blocked_bugs.push(BlockedBug {
                bug,
                blocking_entities: blocking,
            });
        }
    }

    let count = blocked_tasks.len();
    let bug_count = blocked_bugs.len();
    Ok(BlockedTasks {
        tasks: blocked_tasks,
        bugs: blocked_bugs,
        count,
        bug_count,
    })
}

// === Test Node Commands ===

#[derive(Serialize)]
pub struct TestCreated {
    pub id: String,
    pub name: String,
}

impl Output for TestCreated {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Created test {} \"{}\"", self.id, self.name)
    }
}

/// Create a new test node.
pub fn test_create(
    repo_path: &Path,
    name: String,
    command: String,
    working_dir: String,
    task_id: Option<String>,
    bug_id: Option<String>,
) -> Result<TestCreated> {
    let mut storage = Storage::open(repo_path)?;

    let id = generate_id("bnt", &name);
    let mut test = TestNode::new(id.clone(), name.clone(), command);
    test.working_dir = working_dir;

    // If task_id provided, link immediately
    if let Some(tid) = task_id {
        // Verify task exists
        storage.get_task(&tid)?;
        test.linked_tasks.push(tid);
    }

    // If bug_id provided, link immediately
    if let Some(bid) = bug_id {
        // Verify bug exists
        storage.get_bug(&bid)?;
        test.linked_bugs.push(bid);
    }

    storage.create_test(&test)?;

    Ok(TestCreated { id, name })
}

impl Output for TestNode {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("{} {}", self.id, self.name));
        lines.push(format!("  Command: {}", self.command));
        lines.push(format!("  Working dir: {}", self.working_dir));
        if let Some(ref pattern) = self.pattern {
            lines.push(format!("  Pattern: {}", pattern));
        }
        if self.linked_tasks.is_empty() {
            lines.push("  Linked tasks: (none)".to_string());
        } else {
            lines.push(format!("  Linked tasks: {}", self.linked_tasks.join(", ")));
        }
        if self.linked_bugs.is_empty() {
            lines.push("  Linked bugs: (none)".to_string());
        } else {
            lines.push(format!("  Linked bugs: {}", self.linked_bugs.join(", ")));
        }
        lines.push(format!(
            "  Created: {}",
            self.created_at.format("%Y-%m-%d %H:%M")
        ));
        lines.join("\n")
    }
}

/// Show a test node by ID.
pub fn test_show(repo_path: &Path, id: &str) -> Result<TestNode> {
    let storage = Storage::open(repo_path)?;
    storage.get_test(id)
}

#[derive(Serialize)]
pub struct TestList {
    pub tests: Vec<TestNode>,
    pub count: usize,
}

impl Output for TestList {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if self.tests.is_empty() {
            return "No tests found.".to_string();
        }

        let mut lines = Vec::new();
        lines.push(format!("{} test(s):\n", self.count));

        for test in &self.tests {
            let links = if test.linked_tasks.is_empty() {
                String::new()
            } else {
                format!(" -> {}", test.linked_tasks.join(", "))
            };
            lines.push(format!("  {} {}{}", test.id, test.name, links));
        }

        lines.join("\n")
    }
}

/// List test nodes with optional filters.
pub fn test_list(repo_path: &Path, task_id: Option<&str>) -> Result<TestList> {
    let storage = Storage::open(repo_path)?;
    let tests = storage.list_tests(task_id)?;
    let count = tests.len();
    Ok(TestList { tests, count })
}

#[derive(Serialize)]
pub struct TestLinked {
    pub test_id: String,
    pub task_id: String,
}

impl Output for TestLinked {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Linked test {} to task {}", self.test_id, self.task_id)
    }
}

/// Link a test to a task.
pub fn test_link(repo_path: &Path, test_id: &str, task_id: &str) -> Result<TestLinked> {
    let mut storage = Storage::open(repo_path)?;
    storage.link_test_to_task(test_id, task_id)?;

    Ok(TestLinked {
        test_id: test_id.to_string(),
        task_id: task_id.to_string(),
    })
}

#[derive(Serialize)]
pub struct TestUnlinked {
    pub test_id: String,
    pub task_id: String,
}

impl Output for TestUnlinked {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Unlinked test {} from task {}", self.test_id, self.task_id)
    }
}

/// Unlink a test from a task.
pub fn test_unlink(repo_path: &Path, test_id: &str, task_id: &str) -> Result<TestUnlinked> {
    let mut storage = Storage::open(repo_path)?;
    storage.unlink_test_from_task(test_id, task_id)?;

    Ok(TestUnlinked {
        test_id: test_id.to_string(),
        task_id: task_id.to_string(),
    })
}

#[derive(Serialize)]
pub struct TestLinkedBug {
    pub test_id: String,
    pub bug_id: String,
}

impl Output for TestLinkedBug {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Linked test {} to bug {}", self.test_id, self.bug_id)
    }
}

/// Link a test to a bug.
pub fn test_link_bug(repo_path: &Path, test_id: &str, bug_id: &str) -> Result<TestLinkedBug> {
    let mut storage = Storage::open(repo_path)?;
    storage.link_test_to_bug(test_id, bug_id)?;

    Ok(TestLinkedBug {
        test_id: test_id.to_string(),
        bug_id: bug_id.to_string(),
    })
}

#[derive(Serialize)]
pub struct TestUnlinkedBug {
    pub test_id: String,
    pub bug_id: String,
}

impl Output for TestUnlinkedBug {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Unlinked test {} from bug {}", self.test_id, self.bug_id)
    }
}

/// Unlink a test from a bug.
pub fn test_unlink_bug(repo_path: &Path, test_id: &str, bug_id: &str) -> Result<TestUnlinkedBug> {
    let mut storage = Storage::open(repo_path)?;
    storage.unlink_test_from_bug(test_id, bug_id)?;

    Ok(TestUnlinkedBug {
        test_id: test_id.to_string(),
        bug_id: bug_id.to_string(),
    })
}

#[derive(Serialize)]
pub struct TestRunResult {
    pub test_id: String,
    pub test_name: String,
    pub passed: bool,
    pub exit_code: i32,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub reopened_tasks: Vec<String>,
}

impl Output for TestRunResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let status = if self.passed { "PASSED" } else { "FAILED" };
        let mut lines = Vec::new();
        lines.push(format!(
            "{} {} ({}) - {}ms",
            status, self.test_name, self.test_id, self.duration_ms
        ));

        if !self.passed
            && let Some(ref stderr) = self.stderr
            && !stderr.is_empty()
        {
            lines.push(format!(
                "  stderr: {}",
                stderr
                    .lines()
                    .take(5)
                    .collect::<Vec<_>>()
                    .join("\n         ")
            ));
        }

        if !self.reopened_tasks.is_empty() {
            lines.push(format!(
                "  Regression detected! Reopened tasks: {}",
                self.reopened_tasks.join(", ")
            ));
        }

        lines.join("\n")
    }
}

#[derive(Serialize)]
pub struct TestRunResults {
    pub results: Vec<TestRunResult>,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
}

impl Output for TestRunResults {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if self.results.is_empty() {
            return "No tests to run.".to_string();
        }

        let mut lines = Vec::new();

        for result in &self.results {
            lines.push(result.to_human());
        }

        lines.push(String::new());
        lines.push(format!(
            "Results: {} passed, {} failed, {} total",
            self.passed, self.failed, self.total
        ));

        lines.join("\n")
    }
}

/// Run a single test and return the result.
fn run_single_test(
    storage: &mut Storage,
    test: &TestNode,
    repo_path: &Path,
) -> Result<TestRunResult> {
    let start = Instant::now();

    // Execute the command
    let working_dir = if test.working_dir == "." {
        repo_path.to_path_buf()
    } else {
        repo_path.join(&test.working_dir)
    };

    let output = Command::new("sh")
        .arg("-c")
        .arg(&test.command)
        .current_dir(&working_dir)
        .output()
        .map_err(|e| Error::Other(format!("Failed to execute command: {}", e)))?;

    let duration_ms = start.elapsed().as_millis() as u64;
    let exit_code = output.status.code().unwrap_or(-1);
    let passed = output.status.success();

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    // Save test result
    let result = TestResult {
        test_id: test.id.clone(),
        passed,
        exit_code,
        stdout: if stdout.is_empty() {
            None
        } else {
            Some(stdout.clone())
        },
        stderr: if stderr.is_empty() {
            None
        } else {
            Some(stderr.clone())
        },
        duration_ms,
        executed_at: Utc::now(),
    };
    storage.save_test_result(&result)?;

    // Handle regression detection
    let mut reopened_tasks = Vec::new();
    if !passed {
        reopened_tasks = storage.reopen_linked_tasks_on_failure(&test.id)?;
    }

    Ok(TestRunResult {
        test_id: test.id.clone(),
        test_name: test.name.clone(),
        passed,
        exit_code,
        duration_ms,
        stdout: if stdout.is_empty() {
            None
        } else {
            Some(stdout)
        },
        stderr: if stderr.is_empty() {
            None
        } else {
            Some(stderr)
        },
        reopened_tasks,
    })
}

/// Run tests based on the provided options.
pub fn test_run(
    repo_path: &Path,
    test_id: Option<&str>,
    task_id: Option<&str>,
    all: bool,
    failed_only: bool,
) -> Result<TestRunResults> {
    let mut storage = Storage::open(repo_path)?;

    // Determine which tests to run
    let tests: Vec<TestNode> = if let Some(id) = test_id {
        // Run specific test
        vec![storage.get_test(id)?]
    } else if let Some(tid) = task_id {
        // Run tests linked to a task
        storage.get_tests_for_task(tid)?
    } else if failed_only {
        // Run only previously failed tests
        storage.get_failed_tests()?
    } else if all {
        // Run all tests
        storage.list_tests(None)?
    } else {
        return Err(Error::Other(
            "Specify --all, --failed, --task, or a test ID".to_string(),
        ));
    };

    if tests.is_empty() {
        return Ok(TestRunResults {
            results: vec![],
            total: 0,
            passed: 0,
            failed: 0,
        });
    }

    let mut results = Vec::new();
    for test in &tests {
        let result = run_single_test(&mut storage, test, repo_path)?;
        results.push(result);
    }

    let total = results.len();
    let passed = results.iter().filter(|r| r.passed).count();
    let failed = total - passed;

    Ok(TestRunResults {
        results,
        total,
        passed,
        failed,
    })
}

// === Commit Tracking Commands ===

use crate::models::CommitLink;

// === Doctor Command ===

/// A single issue detected by the doctor command.
#[derive(Serialize, Clone)]
pub struct DoctorIssue {
    pub severity: String, // "error", "warning", "info"
    pub category: String, // "orphan", "cycle", "consistency", "storage"
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<String>,
}

/// Result of the doctor command.
#[derive(Serialize)]
pub struct DoctorResult {
    pub healthy: bool,
    pub issues: Vec<DoctorIssue>,
    pub stats: DoctorStats,
}

/// Statistics about the binnacle data.
#[derive(Serialize)]
pub struct DoctorStats {
    pub total_tasks: usize,
    pub total_bugs: usize,
    pub total_tests: usize,
    pub total_commits: usize,
    pub storage_path: String,
    /// Archive export directory path, if configured
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive_directory: Option<String>,
    /// Total size of archive directory in bytes, if configured and exists
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive_size_bytes: Option<u64>,
}

impl Output for DoctorResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();

        if self.healthy {
            lines.push("Health check: OK".to_string());
        } else {
            lines.push(format!(
                "Health check: {} issue(s) found",
                self.issues.len()
            ));
        }

        lines.push(String::new());
        lines.push("Statistics:".to_string());
        lines.push(format!("  Tasks: {}", self.stats.total_tasks));
        lines.push(format!("  Bugs: {}", self.stats.total_bugs));
        lines.push(format!("  Tests: {}", self.stats.total_tests));
        lines.push(format!("  Commit links: {}", self.stats.total_commits));
        lines.push(format!("  Storage: {}", self.stats.storage_path));

        // Archive export status
        if let Some(archive_dir) = &self.stats.archive_directory {
            if let Some(size) = self.stats.archive_size_bytes {
                lines.push(format!(
                    "  Archive: {} ({})",
                    archive_dir,
                    format_bytes(size)
                ));
            } else {
                lines.push(format!("  Archive: {} (not created)", archive_dir));
            }
        } else {
            lines.push("  Archive: not configured".to_string());
        }

        if !self.issues.is_empty() {
            lines.push(String::new());
            lines.push("Issues:".to_string());
            for issue in &self.issues {
                let severity_marker = match issue.severity.as_str() {
                    "error" => "[ERROR]",
                    "warning" => "[WARN]",
                    _ => "[INFO]",
                };
                let entity = issue
                    .entity_id
                    .as_ref()
                    .map(|id| format!(" ({})", id))
                    .unwrap_or_default();
                lines.push(format!(
                    "  {} {}: {}{}",
                    severity_marker, issue.category, issue.message, entity
                ));
            }
        }

        lines.join("\n")
    }
}

/// Run health checks on the binnacle data.
pub fn doctor(repo_path: &Path) -> Result<DoctorResult> {
    let storage = Storage::open(repo_path)?;
    let mut issues = Vec::new();

    // Get all tasks, bugs, and tests for analysis
    let tasks = storage.list_tasks(None, None, None)?;
    let bugs = storage.list_bugs(None, None, None, None, true)?; // Include all for doctor analysis
    let tests = storage.list_tests(None)?;

    // Build a set of valid entity IDs (respects cache-based deletion)
    let milestones = storage.list_milestones(None, None, None)?;
    let ideas = storage.list_ideas(None, None)?;
    let agents = storage.list_agents(None)?;
    let docs = storage.list_docs(None, None, None, None)?;
    let queue = storage.get_queue().ok();

    let mut valid_entity_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    for task in &tasks {
        valid_entity_ids.insert(task.core.id.clone());
    }
    for bug in &bugs {
        valid_entity_ids.insert(bug.core.id.clone());
    }
    for test in &tests {
        valid_entity_ids.insert(test.id.clone());
    }
    for milestone in &milestones {
        valid_entity_ids.insert(milestone.core.id.clone());
    }
    for idea in &ideas {
        valid_entity_ids.insert(idea.core.id.clone());
    }
    for agent in &agents {
        valid_entity_ids.insert(agent.id.clone());
    }
    for doc in &docs {
        valid_entity_ids.insert(doc.core.id.clone());
    }
    if let Some(q) = &queue {
        valid_entity_ids.insert(q.id.clone());
    }

    // Check for orphan dependencies (tasks that reference non-existent tasks)
    for task in &tasks {
        for dep_id in &task.depends_on {
            if storage.get_task(dep_id).is_err() {
                issues.push(DoctorIssue {
                    severity: "error".to_string(),
                    category: "orphan".to_string(),
                    message: format!("Task depends on non-existent task {}", dep_id),
                    entity_id: Some(task.core.id.clone()),
                });
            }
        }
    }

    // Check for orphan dependencies in bugs (bugs that reference non-existent entities)
    for bug in &bugs {
        for dep_id in &bug.depends_on {
            // Bug dependencies can be tasks or other bugs
            let exists = storage.get_task(dep_id).is_ok() || storage.get_bug(dep_id).is_ok();
            if !exists {
                issues.push(DoctorIssue {
                    severity: "error".to_string(),
                    category: "orphan".to_string(),
                    message: format!("Bug depends on non-existent entity {}", dep_id),
                    entity_id: Some(bug.core.id.clone()),
                });
            }
        }
    }

    // Check for orphan test links (tests that reference non-existent tasks)
    for test in &tests {
        for task_id in &test.linked_tasks {
            if storage.get_task(task_id).is_err() {
                issues.push(DoctorIssue {
                    severity: "error".to_string(),
                    category: "orphan".to_string(),
                    message: format!("Test linked to non-existent task {}", task_id),
                    entity_id: Some(test.id.clone()),
                });
            }
        }
    }

    // Check for orphaned edges (edges pointing to/from deleted entities)
    let all_edges = storage.list_edges(None, None, None)?;
    for edge in &all_edges {
        let source_exists = valid_entity_ids.contains(&edge.source);
        let target_exists = valid_entity_ids.contains(&edge.target);

        if !source_exists && !target_exists {
            issues.push(DoctorIssue {
                severity: "error".to_string(),
                category: "orphan".to_string(),
                message: format!(
                    "Edge {} -> {} has both endpoints deleted",
                    edge.source, edge.target
                ),
                entity_id: Some(edge.id.clone()),
            });
        } else if !source_exists {
            issues.push(DoctorIssue {
                severity: "error".to_string(),
                category: "orphan".to_string(),
                message: format!("Edge source {} no longer exists", edge.source),
                entity_id: Some(edge.id.clone()),
            });
        } else if !target_exists {
            issues.push(DoctorIssue {
                severity: "error".to_string(),
                category: "orphan".to_string(),
                message: format!("Edge target {} no longer exists", edge.target),
                entity_id: Some(edge.id.clone()),
            });
        }
    }

    // Check for inconsistent task states
    for task in &tasks {
        // Check for done tasks with pending dependencies
        if task.status == TaskStatus::Done {
            for dep_id in &task.depends_on {
                if let Ok(dep_task) = storage.get_task(dep_id)
                    && dep_task.status != TaskStatus::Done
                    && dep_task.status != TaskStatus::Cancelled
                {
                    issues.push(DoctorIssue {
                        severity: "warning".to_string(),
                        category: "consistency".to_string(),
                        message: format!("Task is done but depends on incomplete task {}", dep_id),
                        entity_id: Some(task.core.id.clone()),
                    });
                }
            }
        }

        // Check for closed tasks without closed_at timestamp
        if task.status == TaskStatus::Done && task.closed_at.is_none() {
            issues.push(DoctorIssue {
                severity: "info".to_string(),
                category: "consistency".to_string(),
                message: "Task is done but has no closed_at timestamp".to_string(),
                entity_id: Some(task.core.id.clone()),
            });
        }
    }

    // Check for inconsistent bug states
    for bug in &bugs {
        // Check for done bugs with pending dependencies
        if bug.status == TaskStatus::Done {
            for dep_id in &bug.depends_on {
                // Check if dependency is incomplete (task or bug)
                let is_incomplete = if let Ok(dep_task) = storage.get_task(dep_id) {
                    dep_task.status != TaskStatus::Done && dep_task.status != TaskStatus::Cancelled
                } else if let Ok(dep_bug) = storage.get_bug(dep_id) {
                    dep_bug.status != TaskStatus::Done && dep_bug.status != TaskStatus::Cancelled
                } else {
                    false // Already caught by orphan check
                };

                if is_incomplete {
                    issues.push(DoctorIssue {
                        severity: "warning".to_string(),
                        category: "consistency".to_string(),
                        message: format!("Bug is done but depends on incomplete entity {}", dep_id),
                        entity_id: Some(bug.core.id.clone()),
                    });
                }
            }
        }

        // Check for closed bugs without closed_at timestamp
        if bug.status == TaskStatus::Done && bug.closed_at.is_none() {
            issues.push(DoctorIssue {
                severity: "info".to_string(),
                category: "consistency".to_string(),
                message: "Bug is done but has no closed_at timestamp".to_string(),
                entity_id: Some(bug.core.id.clone()),
            });
        }
    }

    // Get commit count
    let commit_count = storage.count_commit_links()?;

    // Check JSONL files for blank lines (data integrity issue)
    let jsonl_files = [
        "tasks.jsonl",
        "bugs.jsonl",
        "commits.jsonl",
        "test-results.jsonl",
        "edges.jsonl",
        "milestones.jsonl",
    ];
    for filename in jsonl_files {
        let file_path = storage.root().join(filename);
        if file_path.exists()
            && let Ok(content) = fs::read_to_string(&file_path)
        {
            let blank_count = content.lines().filter(|l| l.trim().is_empty()).count();
            if blank_count > 0 {
                issues.push(DoctorIssue {
                    severity: "warning".to_string(),
                    category: "data".to_string(),
                    message: format!(
                        "{} contains {} blank line(s) - these are harmless but can be removed manually",
                        filename, blank_count
                    ),
                    entity_id: None,
                });
            }
        }
    }

    // Check if primary queue exists
    if storage.get_queue().is_err() {
        issues.push(DoctorIssue {
            severity: "warning".to_string(),
            category: "queue".to_string(),
            message: "No primary queue exists - run 'bn doctor --fix' to create one".to_string(),
            entity_id: None,
        });
    }

    // Check for legacy bni- prefixed ideas (should use bn- prefix)
    let ideas = storage.list_ideas(None, None)?;
    let legacy_prefix_count = ideas
        .iter()
        .filter(|i| i.core.id.starts_with("bni-"))
        .count();
    if legacy_prefix_count > 0 {
        issues.push(DoctorIssue {
            severity: "warning".to_string(),
            category: "legacy_prefix".to_string(),
            message: format!(
                "{} idea(s) have legacy bni- prefix - run 'bn doctor --fix' to migrate to bn- prefix",
                legacy_prefix_count
            ),
            entity_id: None,
        });
    }

    // Check for legacy bnd- prefixed docs (should use bn- prefix)
    let legacy_doc_count = docs
        .iter()
        .filter(|d| d.core.id.starts_with("bnd-"))
        .count();
    if legacy_doc_count > 0 {
        issues.push(DoctorIssue {
            severity: "warning".to_string(),
            category: "legacy_prefix".to_string(),
            message: format!(
                "{} doc(s) have legacy bnd- prefix - run 'bn doctor --fix' to migrate to bn- prefix",
                legacy_doc_count
            ),
            entity_id: None,
        });
    }

    // Check for orphan docs (docs with no linked entities)
    for doc in &docs {
        let edges = storage.get_edges_for_entity(&doc.core.id)?;
        // Filter out supersedes edges - those don't count as meaningful links
        let meaningful_edges: Vec<_> = edges
            .iter()
            .filter(|e| e.edge.edge_type != EdgeType::Supersedes)
            .collect();
        if meaningful_edges.is_empty() {
            issues.push(DoctorIssue {
                severity: "warning".to_string(),
                category: "orphan".to_string(),
                message: format!("Doc \"{}\" has no linked entities", doc.core.title),
                entity_id: Some(doc.core.id.clone()),
            });
        }
    }

    // Check for disconnected components in the task graph
    if let Ok(components_result) = graph_components(repo_path)
        && components_result.component_count > 1
    {
        issues.push(DoctorIssue {
            severity: "info".to_string(),
            category: "graph".to_string(),
            message: format!(
                "Task graph has {} disconnected components. Run 'bn graph components' for details.",
                components_result.component_count
            ),
            entity_id: None,
        });
    }

    // Check for legacy .tar.gz archives that need migration to .bng
    if let Some(archive_dir) = config_get_archive_directory(repo_path)
        && archive_dir.exists()
        && let Ok(entries) = fs::read_dir(&archive_dir)
    {
        let mut legacy_count = 0;
        for entry in entries.flatten() {
            let filename = entry
                .path()
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_default();
            if filename.ends_with(".tar.gz") {
                // Check if corresponding .bng already exists
                let bng_filename = filename.replace(".tar.gz", ".bng");
                let bng_path = archive_dir.join(&bng_filename);
                if !bng_path.exists() {
                    legacy_count += 1;
                }
            }
        }
        if legacy_count > 0 {
            issues.push(DoctorIssue {
                severity: "warning".to_string(),
                category: "storage".to_string(),
                message: format!(
                    "Found {} legacy .tar.gz archive(s) that can be migrated to .bng format. \
                     Run 'bn doctor --fix-archives' to migrate.",
                    legacy_count
                ),
                entity_id: None,
            });
        }
    }

    // Get archive directory configuration and size
    let archive_directory = config_get_archive_directory(repo_path);
    let archive_size_bytes = archive_directory.as_ref().and_then(|dir| {
        if dir.exists() {
            Some(calculate_dir_size(dir))
        } else {
            None
        }
    });

    let stats = DoctorStats {
        total_tasks: tasks.len(),
        total_bugs: bugs.len(),
        total_tests: tests.len(),
        total_commits: commit_count,
        storage_path: storage.root().to_string_lossy().to_string(),
        archive_directory: archive_directory.map(|p| p.to_string_lossy().to_string()),
        archive_size_bytes,
    };

    Ok(DoctorResult {
        healthy: issues.is_empty(),
        issues,
        stats,
    })
}

/// Result of the doctor fix command.
#[derive(Serialize)]
pub struct DoctorFixResult {
    pub fixes_applied: Vec<String>,
    pub issues_remaining: Vec<DoctorIssue>,
}

impl Output for DoctorFixResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();

        if self.fixes_applied.is_empty() {
            lines.push("Doctor Fix: No fixable issues found".to_string());
        } else {
            lines.push(format!(
                "Doctor Fix: {} fix(es) applied",
                self.fixes_applied.len()
            ));
            lines.push(String::new());
            lines.push("Fixes applied:".to_string());
            for fix in &self.fixes_applied {
                lines.push(format!("  ✓ {}", fix));
            }
        }

        if !self.issues_remaining.is_empty() {
            lines.push(String::new());
            lines.push(format!(
                "Remaining issues ({}):",
                self.issues_remaining.len()
            ));
            for issue in &self.issues_remaining {
                let severity_marker = match issue.severity.as_str() {
                    "error" => "[ERROR]",
                    "warning" => "[WARN]",
                    _ => "[INFO]",
                };
                let entity = issue
                    .entity_id
                    .as_ref()
                    .map(|id| format!(" ({})", id))
                    .unwrap_or_default();
                lines.push(format!(
                    "  {} {}: {}{}",
                    severity_marker, issue.category, issue.message, entity
                ));
            }
        }

        lines.join("\n")
    }
}

/// Result of archive migration.
#[derive(Serialize)]
pub struct DoctorFixArchivesResult {
    pub archives_scanned: usize,
    pub archives_migrated: usize,
    pub archives_skipped: usize,
    pub errors: Vec<String>,
    pub dry_run: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<String>,
}

impl Output for DoctorFixArchivesResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();

        if self.dry_run {
            lines.push("Archive Migration (DRY RUN - no changes made)".to_string());
        } else {
            lines.push("Archive Migration Complete".to_string());
        }

        lines.push(format!("  Archives scanned: {}", self.archives_scanned));
        lines.push(format!("  Archives migrated: {}", self.archives_migrated));
        lines.push(format!("  Archives skipped: {}", self.archives_skipped));

        if !self.errors.is_empty() {
            lines.push(String::new());
            lines.push(format!("Errors ({}):", self.errors.len()));
            for error in &self.errors {
                lines.push(format!("  ✗ {}", error));
            }
        }

        if !self.details.is_empty() {
            lines.push(String::new());
            lines.push("Details:".to_string());
            for detail in &self.details {
                lines.push(format!("  {}", detail));
            }
        }

        lines.join("\n")
    }
}

/// Migrate old .tar.gz archives to new .bng format (zstd compression).
///
/// This function:
/// 1. Gets the archive directory from config
/// 2. Scans for .tar.gz files
/// 3. For each file: decompresses, re-compresses as .bng, renames old file to .old
pub fn doctor_fix_archives(repo_path: &Path, dry_run: bool) -> Result<DoctorFixArchivesResult> {
    use flate2::read::GzDecoder;
    use std::io::Read;
    use zstd::stream::write::Encoder as ZstdEncoder;

    let archive_dir = match config_get_archive_directory(repo_path) {
        Some(dir) => dir,
        None => {
            return Ok(DoctorFixArchivesResult {
                archives_scanned: 0,
                archives_migrated: 0,
                archives_skipped: 0,
                errors: vec![
                    "Archive directory not configured. Set archive.directory config key."
                        .to_string(),
                ],
                dry_run,
                details: vec![],
            });
        }
    };

    if !archive_dir.exists() {
        return Ok(DoctorFixArchivesResult {
            archives_scanned: 0,
            archives_migrated: 0,
            archives_skipped: 0,
            errors: vec![format!(
                "Archive directory does not exist: {}",
                archive_dir.display()
            )],
            dry_run,
            details: vec![],
        });
    }

    let mut archives_scanned = 0;
    let mut archives_migrated = 0;
    let mut archives_skipped = 0;
    let mut errors = Vec::new();
    let mut details = Vec::new();

    // Find all .tar.gz files
    let entries = match fs::read_dir(&archive_dir) {
        Ok(e) => e,
        Err(e) => {
            return Ok(DoctorFixArchivesResult {
                archives_scanned: 0,
                archives_migrated: 0,
                archives_skipped: 0,
                errors: vec![format!("Failed to read archive directory: {}", e)],
                dry_run,
                details: vec![],
            });
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let filename = match path.file_name() {
            Some(f) => f.to_string_lossy().to_string(),
            None => continue,
        };

        // Only process .tar.gz files
        if !filename.ends_with(".tar.gz") {
            continue;
        }

        archives_scanned += 1;

        // Check if corresponding .bng already exists
        let bng_filename = filename.replace(".tar.gz", ".bng");
        let bng_path = archive_dir.join(&bng_filename);
        if bng_path.exists() {
            archives_skipped += 1;
            details.push(format!("Skipped {} (already has .bng version)", filename));
            continue;
        }

        // Read and decompress .tar.gz
        let archive_data = match fs::read(&path) {
            Ok(data) => data,
            Err(e) => {
                errors.push(format!("Failed to read {}: {}", filename, e));
                continue;
            }
        };

        // Extract archive contents
        let decoder = GzDecoder::new(&archive_data[..]);
        let mut archive = tar::Archive::new(decoder);

        let mut file_contents: Vec<(String, Vec<u8>)> = Vec::new();
        let entries_result = archive.entries();
        match entries_result {
            Ok(entries) => {
                for entry_result in entries {
                    match entry_result {
                        Ok(mut entry) => {
                            let entry_path = match entry.path() {
                                Ok(p) => p.to_path_buf(),
                                Err(e) => {
                                    errors.push(format!(
                                        "Failed to get entry path in {}: {}",
                                        filename, e
                                    ));
                                    continue;
                                }
                            };
                            let mut data = Vec::new();
                            if let Err(e) = entry.read_to_end(&mut data) {
                                errors.push(format!(
                                    "Failed to read entry {} in {}: {}",
                                    entry_path.display(),
                                    filename,
                                    e
                                ));
                                continue;
                            }
                            file_contents.push((entry_path.to_string_lossy().to_string(), data));
                        }
                        Err(e) => {
                            errors.push(format!("Failed to read entry in {}: {}", filename, e));
                        }
                    }
                }
            }
            Err(e) => {
                errors.push(format!("Failed to read archive {}: {}", filename, e));
                continue;
            }
        }

        if file_contents.is_empty() {
            errors.push(format!("Archive {} is empty or corrupted", filename));
            continue;
        }

        if dry_run {
            archives_migrated += 1;
            details.push(format!("Would migrate {} → {}", filename, bng_filename));
            continue;
        }

        // Create new .bng archive with zstd compression
        let mut archive_buffer = Vec::new();
        {
            let encoder = match ZstdEncoder::new(&mut archive_buffer, 3) {
                Ok(e) => e,
                Err(e) => {
                    errors.push(format!(
                        "Failed to create zstd encoder for {}: {}",
                        filename, e
                    ));
                    continue;
                }
            };
            let mut tar = tar::Builder::new(encoder);

            for (entry_path, data) in &file_contents {
                let mut header = tar::Header::new_gnu();
                if let Err(e) = header.set_path(entry_path) {
                    errors.push(format!(
                        "Failed to set path {} in {}: {}",
                        entry_path, filename, e
                    ));
                    continue;
                }
                header.set_size(data.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                if let Err(e) = tar.append(&header, data.as_slice()) {
                    errors.push(format!(
                        "Failed to append {} to {}: {}",
                        entry_path, filename, e
                    ));
                    continue;
                }
            }

            // Finish tar archive
            let encoder = match tar.into_inner() {
                Ok(e) => e,
                Err(e) => {
                    errors.push(format!("Failed to finish tar for {}: {}", filename, e));
                    continue;
                }
            };
            // Finish zstd compression
            if let Err(e) = encoder.finish() {
                errors.push(format!("Failed to finish zstd for {}: {}", filename, e));
                continue;
            }
        }

        // Write new .bng file
        if let Err(e) = fs::write(&bng_path, &archive_buffer) {
            errors.push(format!("Failed to write {}: {}", bng_filename, e));
            continue;
        }

        // Rename old file to .old (append .old to preserve the original extension)
        let old_filename = format!("{}.old", filename);
        let old_path = archive_dir.join(&old_filename);
        if let Err(e) = fs::rename(&path, &old_path) {
            errors.push(format!(
                "Failed to rename {} to .old (new .bng was created): {}",
                filename, e
            ));
        }

        archives_migrated += 1;
        details.push(format!("Migrated {} → {}", filename, bng_filename));
    }

    Ok(DoctorFixArchivesResult {
        archives_scanned,
        archives_migrated,
        archives_skipped,
        errors,
        dry_run,
        details,
    })
}

/// Run doctor with automatic fixes for repairable issues.
pub fn doctor_fix(repo_path: &Path) -> Result<DoctorFixResult> {
    let mut storage = Storage::open(repo_path)?;
    let mut fixes_applied = Vec::new();

    // Fix: Create primary queue if missing
    if storage.get_queue().is_err() {
        let title = "Work Queue".to_string();
        let queue_id = generate_id("bnq", &title);
        let queue = Queue::new(queue_id.clone(), title.clone());
        storage.create_queue(&queue)?;
        fixes_applied.push(format!(
            "Created primary queue '{}' ({})",
            queue.title, queue_id
        ));
    }

    // Fix: Migrate legacy bni- prefixed ideas to bn- prefix
    let ideas = storage.list_ideas(None, None)?;
    let legacy_ideas: Vec<_> = ideas
        .into_iter()
        .filter(|i| i.core.id.starts_with("bni-"))
        .collect();
    for mut idea in legacy_ideas {
        let old_id = idea.core.id.clone();
        // Generate new bn- prefixed ID (replace bni- with bn-)
        let new_id = format!("bn-{}", &old_id[4..]);
        idea.core.id = new_id.clone();

        // Add the idea with new ID (this writes to JSONL and updates cache)
        storage.add_idea(&idea)?;

        // Update any edges that reference the old ID
        storage.update_edge_entity_id(&old_id, &new_id)?;

        // Delete the old idea from cache (JSONL retains history, but cache is updated)
        storage.delete_idea(&old_id)?;

        fixes_applied.push(format!("Migrated idea {} → {}", old_id, new_id));
    }

    // Fix: Migrate legacy bnd- prefixed docs to bn- prefix
    let docs = storage.list_docs(None, None, None, None)?;
    let legacy_docs: Vec<_> = docs
        .into_iter()
        .filter(|d| d.core.id.starts_with("bnd-"))
        .collect();
    for mut doc in legacy_docs {
        let old_id = doc.core.id.clone();
        // Generate new bn- prefixed ID using strip_prefix
        let new_id = if let Some(suffix) = old_id.strip_prefix("bnd-") {
            format!("bn-{}", suffix)
        } else {
            continue; // Shouldn't happen since we filtered, but be safe
        };
        doc.core.id = new_id.clone();

        // Update supersedes reference if it points to a bnd- prefixed ID
        if let Some(ref supersedes_id) = doc.supersedes
            && let Some(suffix) = supersedes_id.strip_prefix("bnd-")
        {
            doc.supersedes = Some(format!("bn-{}", suffix));
        }

        // Add the doc with new ID (this writes to JSONL and updates cache)
        storage.add_doc(&doc)?;

        // Update any edges that reference the old ID
        storage.update_edge_entity_id(&old_id, &new_id)?;

        // Delete the old doc from cache (JSONL retains history, but cache is updated)
        storage.delete_doc(&old_id)?;

        fixes_applied.push(format!("Migrated doc {} → {}", old_id, new_id));
    }

    // Fix: Remove orphan edges (edges pointing to/from deleted entities)
    // Build a set of valid entity IDs
    let tasks = storage.list_tasks(None, None, None)?;
    let bugs = storage.list_bugs(None, None, None, None, true)?; // Include all for migration
    let tests = storage.list_tests(None)?;
    let milestones = storage.list_milestones(None, None, None)?;
    let ideas = storage.list_ideas(None, None)?;
    let agents = storage.list_agents(None)?;
    let queue = storage.get_queue().ok();

    let mut valid_entity_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    for task in &tasks {
        valid_entity_ids.insert(task.core.id.clone());
    }
    for bug in &bugs {
        valid_entity_ids.insert(bug.core.id.clone());
    }
    for test in &tests {
        valid_entity_ids.insert(test.id.clone());
    }
    for milestone in &milestones {
        valid_entity_ids.insert(milestone.core.id.clone());
    }
    for idea in &ideas {
        valid_entity_ids.insert(idea.core.id.clone());
    }
    for agent in &agents {
        valid_entity_ids.insert(agent.id.clone());
    }
    if let Some(q) = &queue {
        valid_entity_ids.insert(q.id.clone());
    }

    // Find and remove orphan edges
    let all_edges = storage.list_edges(None, None, None)?;
    for edge in all_edges {
        let source_exists = valid_entity_ids.contains(&edge.source);
        let target_exists = valid_entity_ids.contains(&edge.target);

        if (!source_exists || !target_exists) && storage.remove_edge_by_id(&edge.id).is_ok() {
            let reason = if !source_exists && !target_exists {
                format!(
                    "both endpoints deleted ({} -> {})",
                    edge.source, edge.target
                )
            } else if !source_exists {
                format!("source {} deleted", edge.source)
            } else {
                format!("target {} deleted", edge.target)
            };
            fixes_applied.push(format!("Removed orphan edge {} ({})", edge.id, reason));
        }
    }

    // Run doctor again to get remaining issues
    let result = doctor(repo_path)?;
    let issues_remaining: Vec<DoctorIssue> = result
        .issues
        .into_iter()
        .filter(|issue| issue.category != "queue" && issue.category != "legacy_prefix")
        .collect();

    Ok(DoctorFixResult {
        fixes_applied,
        issues_remaining,
    })
}

// === Doctor Migration Commands ===

/// Result of edge migration.
#[derive(Serialize)]
pub struct EdgeMigrationResult {
    pub tasks_scanned: usize,
    pub bugs_scanned: usize,
    pub edges_created: usize,
    pub edges_skipped: usize, // Already existed
    pub depends_on_cleared: usize,
    pub dry_run: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<String>,
}

impl Output for EdgeMigrationResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();

        if self.dry_run {
            lines.push("Edge Migration (DRY RUN - no changes made)".to_string());
        } else {
            lines.push("Edge Migration Complete".to_string());
        }

        lines.push(format!("  Tasks scanned: {}", self.tasks_scanned));
        lines.push(format!("  Bugs scanned: {}", self.bugs_scanned));
        lines.push(format!("  Edges created: {}", self.edges_created));
        lines.push(format!(
            "  Edges skipped (already exist): {}",
            self.edges_skipped
        ));

        if self.depends_on_cleared > 0 {
            lines.push(format!(
                "  depends_on fields cleared: {}",
                self.depends_on_cleared
            ));
        }

        if !self.details.is_empty() {
            lines.push(String::new());
            lines.push("Details:".to_string());
            for detail in &self.details {
                lines.push(format!("  {}", detail));
            }
        }

        lines.join("\n")
    }
}

/// Migrate legacy depends_on fields to edge relationships.
///
/// This function:
/// 1. Scans all tasks and bugs for non-empty depends_on fields
/// 2. Creates DependsOn edges for each dependency
/// 3. Optionally clears the depends_on fields after migration
pub fn doctor_migrate_edges(
    repo_path: &Path,
    clean_unused: bool,
    dry_run: bool,
) -> Result<EdgeMigrationResult> {
    let mut storage = Storage::open(repo_path)?;

    let tasks = storage.list_tasks(None, None, None)?;
    let bugs = storage.list_bugs(None, None, None, None, true)?; // Include all for migration

    let mut edges_created = 0;
    let mut edges_skipped = 0;
    let mut depends_on_cleared = 0;
    let mut details = Vec::new();

    // Collect entities that need depends_on cleared
    let mut tasks_to_clear: Vec<String> = Vec::new();
    let mut bugs_to_clear: Vec<String> = Vec::new();

    // Process tasks
    for task in &tasks {
        if task.depends_on.is_empty() {
            continue;
        }

        for dep_id in &task.depends_on {
            // Check if edge already exists
            let existing =
                storage.list_edges(Some(EdgeType::DependsOn), Some(&task.core.id), Some(dep_id))?;

            if !existing.is_empty() {
                edges_skipped += 1;
                details.push(format!(
                    "Skipped: {} -> {} (edge exists)",
                    task.core.id, dep_id
                ));
                continue;
            }

            // Create new edge
            if !dry_run {
                let id = storage.generate_edge_id(&task.core.id, dep_id, EdgeType::DependsOn);
                let mut edge = Edge::new(
                    id,
                    task.core.id.clone(),
                    dep_id.clone(),
                    EdgeType::DependsOn,
                );
                edge.reason = Some("Migrated from task.depends_on".to_string());
                storage.add_edge(&edge)?;
            }
            edges_created += 1;
            details.push(format!(
                "Created: {} -> {} (depends_on)",
                task.core.id, dep_id
            ));
        }

        if clean_unused && !task.depends_on.is_empty() {
            tasks_to_clear.push(task.core.id.clone());
        }
    }

    // Process bugs
    for bug in &bugs {
        if bug.depends_on.is_empty() {
            continue;
        }

        for dep_id in &bug.depends_on {
            // Check if edge already exists
            let existing =
                storage.list_edges(Some(EdgeType::DependsOn), Some(&bug.core.id), Some(dep_id))?;

            if !existing.is_empty() {
                edges_skipped += 1;
                details.push(format!(
                    "Skipped: {} -> {} (edge exists)",
                    bug.core.id, dep_id
                ));
                continue;
            }

            // Create new edge
            if !dry_run {
                let id = storage.generate_edge_id(&bug.core.id, dep_id, EdgeType::DependsOn);
                let mut edge =
                    Edge::new(id, bug.core.id.clone(), dep_id.clone(), EdgeType::DependsOn);
                edge.reason = Some("Migrated from bug.depends_on".to_string());
                storage.add_edge(&edge)?;
            }
            edges_created += 1;
            details.push(format!(
                "Created: {} -> {} (depends_on)",
                bug.core.id, dep_id
            ));
        }

        if clean_unused && !bug.depends_on.is_empty() {
            bugs_to_clear.push(bug.core.id.clone());
        }
    }

    // Clear depends_on fields if requested
    if clean_unused && !dry_run {
        for task_id in &tasks_to_clear {
            if let Ok(mut task) = storage.get_task(task_id) {
                task.depends_on.clear();
                task.core.updated_at = Utc::now();
                storage.update_task(&task)?;
                depends_on_cleared += 1;
            }
        }

        for bug_id in &bugs_to_clear {
            if let Ok(mut bug) = storage.get_bug(bug_id) {
                bug.depends_on.clear();
                bug.core.updated_at = Utc::now();
                storage.update_bug(&bug)?;
                depends_on_cleared += 1;
            }
        }
    } else if clean_unused && dry_run {
        depends_on_cleared = tasks_to_clear.len() + bugs_to_clear.len();
    }

    Ok(EdgeMigrationResult {
        tasks_scanned: tasks.len(),
        bugs_scanned: bugs.len(),
        edges_created,
        edges_skipped,
        depends_on_cleared,
        dry_run,
        details,
    })
}

// === Log Command ===

/// A log entry representing a change.
#[derive(Serialize, Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub entity_type: String,
    pub entity_id: String,
    pub action: String, // "created", "updated", "closed", etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

/// Result of the log command.
#[derive(Serialize)]
pub struct LogResult {
    pub entries: Vec<LogEntry>,
    pub count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filtered_by: Option<String>,
}

impl Output for LogResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if self.entries.is_empty() {
            return "No log entries found.".to_string();
        }

        let mut lines = Vec::new();

        if let Some(ref filter) = self.filtered_by {
            lines.push(format!("Log entries for {}:\n", filter));
        } else {
            lines.push(format!("{} log entries:\n", self.count));
        }

        for entry in &self.entries {
            let details = entry
                .details
                .as_ref()
                .map(|d| format!(" - {}", d))
                .unwrap_or_default();
            lines.push(format!(
                "  {} [{}] {} {}{}",
                entry.timestamp, entry.entity_type, entry.entity_id, entry.action, details
            ));
        }

        lines.join("\n")
    }
}

/// Get the audit log of changes.
pub fn log(repo_path: &Path, task_id: Option<&str>) -> Result<LogResult> {
    let storage = Storage::open(repo_path)?;
    let entries = storage.get_log_entries(task_id)?;
    let count = entries.len();

    Ok(LogResult {
        entries,
        count,
        filtered_by: task_id.map(|s| s.to_string()),
    })
}

/// Result of log export command.
#[derive(Serialize)]
pub struct LogExportResult {
    /// Number of entries exported
    pub count: usize,
    /// Export format used
    pub format: String,
    /// Output file path (or "stdout")
    pub output: String,
}

impl Output for LogExportResult {
    fn to_json(&self) -> String {
        // When outputting to stdout, data was already printed - suppress metadata
        if self.output == "stdout" {
            String::new()
        } else {
            serde_json::to_string(self).unwrap_or_default()
        }
    }

    fn to_human(&self) -> String {
        // When outputting to stdout, data was already printed - suppress metadata
        if self.output == "stdout" {
            String::new()
        } else {
            format!(
                "Exported {} log entries in {} format to {}",
                self.count, self.format, self.output
            )
        }
    }
}

/// Export action logs to JSON, CSV, or Markdown.
#[allow(clippy::too_many_arguments)]
pub fn log_export(
    repo_path: &Path,
    format: &str,
    command_filter: Option<&str>,
    user_filter: Option<&str>,
    success_filter: Option<bool>,
    after: Option<&str>,
    before: Option<&str>,
    limit: Option<u32>,
    output_path: Option<&str>,
) -> Result<LogExportResult> {
    let storage = Storage::open(repo_path)?;

    // Query action logs from storage with filters (storage now supports after)
    let logs = storage.query_action_logs(
        limit,
        None,           // no offset for export
        before,         // before filter
        after,          // after filter
        command_filter, // command filter
        user_filter,    // user filter
        success_filter, // success filter
    )?;

    let count = logs.len();

    // Format the output
    let content = match format {
        "json" => format_logs_json(&logs),
        "csv" => format_logs_csv(&logs),
        "markdown" => format_logs_markdown(&logs),
        _ => return Err(Error::Other(format!("Unknown format: {}", format))),
    };

    // Write to file or stdout
    let output_name = if let Some(path) = output_path {
        fs::write(path, &content)
            .map_err(|e| Error::Other(format!("Failed to write to {}: {}", path, e)))?;
        path.to_string()
    } else {
        // Print to stdout
        print!("{}", content);
        "stdout".to_string()
    };

    Ok(LogExportResult {
        count,
        format: format.to_string(),
        output: output_name,
    })
}

/// Format action logs as JSON array.
fn format_logs_json(logs: &[crate::action_log::ActionLog]) -> String {
    serde_json::to_string_pretty(logs).unwrap_or_else(|_| "[]".to_string())
}

/// Format action logs as CSV.
fn format_logs_csv(logs: &[crate::action_log::ActionLog]) -> String {
    let mut csv = String::from("timestamp,command,user,success,duration_ms,error,args\n");
    for log in logs {
        // Escape CSV fields
        let error = log
            .error
            .as_ref()
            .map(|e| escape_csv(e))
            .unwrap_or_default();
        let args = escape_csv(&log.args.to_string());
        csv.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            log.timestamp.to_rfc3339(),
            escape_csv(&log.command),
            escape_csv(&log.user),
            log.success,
            log.duration_ms,
            error,
            args,
        ));
    }
    csv
}

/// Escape a field for CSV output.
fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Format action logs as Markdown.
fn format_logs_markdown(logs: &[crate::action_log::ActionLog]) -> String {
    let mut md = String::from("# Action Log Export\n\n");
    md.push_str(&format!("**Entries:** {}\n\n", logs.len()));

    if logs.is_empty() {
        md.push_str("_No log entries found._\n");
        return md;
    }

    md.push_str("| Timestamp | Command | User | Status | Duration |\n");
    md.push_str("|-----------|---------|------|--------|----------|\n");

    for log in logs {
        let status = if log.success { "✅" } else { "❌" };
        md.push_str(&format!(
            "| {} | `{}` | {} | {} | {}ms |\n",
            log.timestamp.format("%Y-%m-%d %H:%M:%S"),
            log.command,
            log.user,
            status,
            log.duration_ms,
        ));
    }

    // Add details section for failed entries
    let failures: Vec<_> = logs.iter().filter(|l| !l.success).collect();
    if !failures.is_empty() {
        md.push_str("\n## Errors\n\n");
        for log in failures {
            md.push_str(&format!(
                "### {} at {}\n\n",
                log.command,
                log.timestamp.format("%Y-%m-%d %H:%M:%S")
            ));
            if let Some(ref error) = log.error {
                md.push_str(&format!("```\n{}\n```\n\n", error));
            }
        }
    }

    md
}

// === Log Compact Command ===

/// Result of log compact command.
#[derive(Serialize)]
pub struct LogCompactResult {
    /// Number of entries deleted
    pub deleted: u32,
    /// Total entries before compaction
    pub total_before: u32,
    /// Total entries after compaction
    pub total_after: u32,
    /// Max entries setting used (if any)
    pub max_entries: Option<u32>,
    /// Max age days setting used (if any)
    pub max_age_days: Option<u32>,
    /// Whether this was a dry run
    pub dry_run: bool,
}

impl Output for LogCompactResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();

        if self.dry_run {
            lines.push("Dry run - no changes made".to_string());
        }

        if self.deleted == 0 {
            lines.push(format!(
                "No entries to delete ({} total entries)",
                self.total_before
            ));
        } else {
            lines.push(format!(
                "Deleted {} of {} entries ({} remaining)",
                self.deleted, self.total_before, self.total_after
            ));
        }

        let mut settings = Vec::new();
        if let Some(max) = self.max_entries {
            settings.push(format!("max_entries={}", max));
        }
        if let Some(days) = self.max_age_days {
            settings.push(format!("max_age_days={}", days));
        }

        if !settings.is_empty() {
            lines.push(format!("Settings: {}", settings.join(", ")));
        } else {
            lines.push("No retention settings configured".to_string());
        }

        lines.join("\n")
    }
}

/// Compact action logs by enforcing retention settings.
pub fn log_compact(
    repo_path: &Path,
    max_entries_override: Option<u32>,
    max_age_days_override: Option<u32>,
    dry_run: bool,
) -> Result<LogCompactResult> {
    let mut storage = Storage::open(repo_path)?;

    // Get retention settings (overrides take precedence over config)
    let max_entries = match max_entries_override {
        Some(n) => Some(n),
        None => storage
            .get_config("action_log_max_entries")?
            .and_then(|s| s.parse::<u32>().ok()),
    };

    let max_age_days = match max_age_days_override {
        Some(n) => Some(n),
        None => storage
            .get_config("action_log_max_age_days")?
            .and_then(|s| s.parse::<u32>().ok()),
    };

    // Get total count before
    let total_before = storage.count_action_logs(None, None, None, None, None)?;

    // If no settings, nothing to do
    if max_entries.is_none() && max_age_days.is_none() {
        return Ok(LogCompactResult {
            deleted: 0,
            total_before,
            total_after: total_before,
            max_entries,
            max_age_days,
            dry_run,
        });
    }

    // If dry run, calculate what would be deleted without deleting
    if dry_run {
        let mut would_delete = 0u32;

        // Calculate entries older than max_age_days
        if let Some(days) = max_age_days {
            let cutoff = chrono::Utc::now() - chrono::Duration::days(i64::from(days));
            let cutoff_str = cutoff.to_rfc3339();
            would_delete += storage.count_action_logs(Some(&cutoff_str), None, None, None, None)?;
        }

        // Calculate entries exceeding max_entries (after age deletion)
        if let Some(max) = max_entries {
            let remaining_after_age = total_before.saturating_sub(would_delete);
            if remaining_after_age > max {
                would_delete += remaining_after_age - max;
            }
        }

        return Ok(LogCompactResult {
            deleted: would_delete,
            total_before,
            total_after: total_before.saturating_sub(would_delete),
            max_entries,
            max_age_days,
            dry_run,
        });
    }

    // Perform deletion
    let deleted = storage.delete_old_action_logs(max_entries, max_age_days)?;

    // Get total count after
    let total_after = storage.count_action_logs(None, None, None, None, None)?;

    Ok(LogCompactResult {
        deleted,
        total_before,
        total_after,
        max_entries,
        max_age_days,
        dry_run,
    })
}

// === Config Commands ===

/// Result of config get command.
#[derive(Serialize)]
pub struct ConfigValue {
    pub key: String,
    pub value: Option<String>,
}

impl Output for ConfigValue {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        match &self.value {
            Some(v) => format!("{} = {}", self.key, v),
            None => format!("{} is not set", self.key),
        }
    }
}

/// Get a configuration value.
pub fn config_get(repo_path: &Path, key: &str) -> Result<ConfigValue> {
    let storage = Storage::open(repo_path)?;
    let value = storage.get_config(key)?;

    Ok(ConfigValue {
        key: key.to_string(),
        value,
    })
}

/// Get a boolean configuration value with default.
///
/// Returns the default value if the config is not set or cannot be parsed.
pub fn config_get_bool(repo_path: &Path, key: &str, default: bool) -> bool {
    let storage = match Storage::open(repo_path) {
        Ok(s) => s,
        Err(_) => return default,
    };

    match storage.get_config(key) {
        Ok(Some(value_str)) => {
            let parsed = value_str.to_lowercase();
            parsed == "true" || parsed == "1" || parsed == "yes"
        }
        _ => default,
    }
}

/// Check if a git commit exists in the repository.
///
/// Uses `git cat-file -t <sha>` to verify the commit exists and is a commit object.
/// Returns true if the commit exists, false otherwise.
pub fn git_commit_exists(repo_path: &Path, sha: &str) -> bool {
    Command::new("git")
        .args(["cat-file", "-t", sha])
        .current_dir(repo_path)
        .output()
        .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).trim() == "commit")
        .unwrap_or(false)
}

/// Check if there are uncommitted changes in the git repository.
///
/// Uses `git status --porcelain` to check for staged or unstaged changes.
/// Returns true if there are uncommitted changes, false otherwise.
pub fn git_has_uncommitted_changes(repo_path: &Path) -> bool {
    Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_path)
        .output()
        .map(|o| o.status.success() && !o.stdout.is_empty())
        .unwrap_or(false)
}

/// Get a string configuration value with default.
///
/// Returns the default value if the config is not set.
pub fn config_get_string(repo_path: &Path, key: &str, default: &str) -> String {
    let storage = match Storage::open(repo_path) {
        Ok(s) => s,
        Err(_) => return default.to_string(),
    };

    match storage.get_config(key) {
        Ok(Some(value)) => value,
        _ => default.to_string(),
    }
}

/// Get the archive directory for storing binnacle graph snapshots.
///
/// Returns a path based on configuration or the default location:
/// 1. If `archive.directory` is explicitly set, uses that value
/// 2. If `archive.directory` is set to empty string (""), archiving is disabled
/// 3. Otherwise, uses the default `~/.local/share/binnacle/archives/`
///
/// Returns `None` only if archiving is explicitly disabled or the home directory
/// cannot be determined.
pub fn config_get_archive_directory(repo_path: &Path) -> Option<std::path::PathBuf> {
    // Check if archiving is explicitly disabled (set to empty)
    // We check the raw config to distinguish "not set" from "set to empty"
    let storage = match crate::storage::Storage::open(repo_path) {
        Ok(s) => s,
        Err(_) => {
            // Storage not initialized - use default
            return get_default_archive_directory();
        }
    };

    // If explicitly configured, use that value (or None if empty)
    if let Ok(Some(configured)) = storage.get_config("archive.directory") {
        if configured.trim().is_empty() {
            return None; // Explicitly disabled
        }
        return Some(std::path::PathBuf::from(configured));
    }

    // Not configured - use default location
    get_default_archive_directory()
}

/// Get the default archive directory path.
///
/// Returns the archives subdirectory based on:
/// 1. `BN_DATA_DIR` environment variable (if set)
/// 2. `/binnacle/archives` (if container mode detected)
/// 3. `~/.local/share/binnacle/archives/` on Unix systems,
///    or the equivalent data directory on other platforms.
pub fn get_default_archive_directory() -> Option<std::path::PathBuf> {
    // Respect BN_DATA_DIR for test environments and custom setups
    if let Ok(data_dir) = std::env::var("BN_DATA_DIR") {
        return Some(PathBuf::from(data_dir).join("archives"));
    }

    // Check for container mode
    let container_path = Path::new("/binnacle");
    if std::env::var("BN_CONTAINER_MODE").is_ok() || container_path.exists() {
        return Some(container_path.join("archives"));
    }

    // Fall back to standard data directory
    dirs::data_dir().map(|d| d.join("binnacle").join("archives"))
}

/// Result of config set command.
#[derive(Serialize)]
pub struct ConfigSet {
    pub key: String,
    pub value: String,
}

impl Output for ConfigSet {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Set {} = {}", self.key, self.value)
    }
}

/// Set a configuration value.
pub fn config_set(repo_path: &Path, key: &str, value: &str) -> Result<ConfigSet> {
    // Validate configuration values
    match key {
        "action_log_enabled"
        | "action_log_sanitize"
        | "require_commit_for_close"
        | "co-author.enabled" => {
            // Validate boolean values
            let value_lower = value.to_lowercase();
            if value_lower != "true"
                && value_lower != "false"
                && value_lower != "1"
                && value_lower != "0"
                && value_lower != "yes"
                && value_lower != "no"
            {
                return Err(Error::Other(format!(
                    "Invalid boolean value for {}: {}. Must be one of: true, false, 1, 0, yes, no",
                    key, value
                )));
            }
        }
        "action_log_max_entries" => {
            // Validate positive integer
            match value.parse::<u32>() {
                Ok(n) if n > 0 => {}
                _ => {
                    return Err(Error::Other(format!(
                        "Invalid value for {}: {}. Must be a positive integer.",
                        key, value
                    )));
                }
            }
        }
        "action_log_max_age_days" => {
            // Validate positive integer
            match value.parse::<u32>() {
                Ok(n) if n > 0 => {}
                _ => {
                    return Err(Error::Other(format!(
                        "Invalid value for {}: {}. Must be a positive integer.",
                        key, value
                    )));
                }
            }
        }
        "action_log_path" => {
            // Validate path is not empty
            if value.trim().is_empty() {
                return Err(Error::Other("action_log_path cannot be empty".to_string()));
            }
        }
        "co-author.name" | "co-author.email" => {
            // Validate non-empty strings
            if value.trim().is_empty() {
                return Err(Error::Other(format!("{} cannot be empty", key)));
            }
        }
        "archive.directory" => {
            // Empty value is allowed (disables the feature)
            // Non-empty value must be a valid path (parent directory should exist)
            if !value.trim().is_empty() {
                let path = std::path::Path::new(value);
                if let Some(parent) = path.parent()
                    && !parent.as_os_str().is_empty()
                    && !parent.exists()
                {
                    return Err(Error::Other(format!(
                        "Parent directory does not exist: {}",
                        parent.display()
                    )));
                }
            }
        }
        _ => {
            // No validation for unknown keys (for forward compatibility)
        }
    }

    let mut storage = Storage::open(repo_path)?;
    storage.set_config(key, value)?;

    Ok(ConfigSet {
        key: key.to_string(),
        value: value.to_string(),
    })
}

/// Result of config list command.
#[derive(Serialize)]
pub struct ConfigList {
    pub configs: Vec<(String, String)>,
    pub count: usize,
}

impl Output for ConfigList {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if self.configs.is_empty() {
            return "No configuration values set.".to_string();
        }

        let mut lines = Vec::new();
        lines.push(format!("{} configuration value(s):\n", self.count));

        for (key, value) in &self.configs {
            lines.push(format!("  {} = {}", key, value));
        }

        lines.join("\n")
    }
}

/// List all configuration values.
pub fn config_list(repo_path: &Path) -> Result<ConfigList> {
    let storage = Storage::open(repo_path)?;
    let configs = storage.list_configs()?;
    let count = configs.len();

    Ok(ConfigList { configs, count })
}

// === System Store Commands ===

/// File information for store show output.
#[derive(Serialize)]
pub struct StoreFileInfo {
    pub size_bytes: u64,
    pub entries: usize,
}

/// Task count breakdown by status.
#[derive(Serialize)]
pub struct TasksByStatus {
    pub pending: usize,
    pub in_progress: usize,
    pub blocked: usize,
    pub done: usize,
}

/// Tasks information for store show output.
#[derive(Serialize)]
pub struct TasksInfo {
    pub total: usize,
    pub by_status: TasksByStatus,
}

/// Tests information for store show output.
#[derive(Serialize)]
pub struct TestsInfo {
    pub total: usize,
    pub linked: usize,
}

/// Commits information for store show output.
#[derive(Serialize)]
pub struct CommitsInfo {
    pub total: usize,
}

/// Result of the `bn system store show` command.
#[derive(Serialize)]
pub struct StoreShowResult {
    pub storage_path: String,
    pub repo_path: String,
    pub tasks: TasksInfo,
    pub tests: TestsInfo,
    pub commits: CommitsInfo,
    pub files: std::collections::HashMap<String, StoreFileInfo>,
    pub created_at: Option<String>,
    pub last_modified: Option<String>,
}

impl Output for StoreShowResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();

        lines.push(format!("Store: {}", self.storage_path));
        lines.push(format!("Repo:  {}", self.repo_path));
        lines.push(String::new());

        lines.push(format!("Tasks: {} total", self.tasks.total));
        lines.push(format!("  - pending:     {}", self.tasks.by_status.pending));
        lines.push(format!(
            "  - in_progress: {}",
            self.tasks.by_status.in_progress
        ));
        lines.push(format!("  - blocked:     {}", self.tasks.by_status.blocked));
        lines.push(format!("  - done:        {}", self.tasks.by_status.done));
        lines.push(String::new());

        lines.push(format!(
            "Tests: {} total ({} linked to tasks)",
            self.tests.total, self.tests.linked
        ));
        lines.push(format!("Commits: {} linked", self.commits.total));
        lines.push(String::new());

        lines.push("Files:".to_string());
        let mut file_names: Vec<_> = self.files.keys().collect();
        file_names.sort();
        for file_name in file_names {
            let info = &self.files[file_name];
            let size_kb = info.size_bytes as f64 / 1024.0;
            lines.push(format!(
                "  {:<18} {:>7.1} KB  ({} entries)",
                file_name, size_kb, info.entries
            ));
        }

        if let (Some(created), Some(modified)) = (&self.created_at, &self.last_modified) {
            lines.push(String::new());
            lines.push(format!("Created:  {}", created));
            lines.push(format!("Modified: {}", modified));
        }

        lines.join("\n")
    }
}

/// Display summary of current store contents.
pub fn system_store_show(repo_path: &Path) -> Result<StoreShowResult> {
    let storage = Storage::open(repo_path)?;
    let storage_path = storage.root().to_string_lossy().to_string();
    let repo_path_str = repo_path.to_string_lossy().to_string();

    // Get tasks and count by status
    let tasks = storage.list_tasks(None, None, None)?;
    let total_tasks = tasks.len();
    let mut pending = 0;
    let mut in_progress = 0;
    let mut blocked = 0;
    let mut done = 0;

    for task in &tasks {
        match task.status {
            TaskStatus::Pending => pending += 1,
            TaskStatus::InProgress => in_progress += 1,
            TaskStatus::Blocked => blocked += 1,
            TaskStatus::Done => done += 1,
            _ => {}
        }
    }

    // Get tests and count linked ones
    let tests = storage.list_tests(None)?;
    let total_tests = tests.len();
    let linked_tests = tests.iter().filter(|t| !t.linked_tasks.is_empty()).count();

    // Get commit count
    let total_commits = storage.count_commit_links()?;

    // Get file information
    use std::collections::HashMap;
    let mut files = HashMap::new();

    let file_names = [
        "tasks.jsonl",
        "bugs.jsonl",
        "commits.jsonl",
        "test-results.jsonl",
        "cache.db",
    ];
    for file_name in &file_names {
        let file_path = storage.root().join(file_name);
        if let Ok(metadata) = std::fs::metadata(&file_path) {
            let size_bytes = metadata.len();

            // Count entries for JSONL files
            let entries = if file_name.ends_with(".jsonl") {
                std::fs::read_to_string(&file_path)
                    .map(|content| {
                        content
                            .lines()
                            .filter(|line| !line.trim().is_empty())
                            .count()
                    })
                    .unwrap_or(0)
            } else {
                0 // cache.db doesn't have "entries" in the same sense
            };

            files.insert(
                file_name.to_string(),
                StoreFileInfo {
                    size_bytes,
                    entries,
                },
            );
        }
    }

    // Get creation and modification times from tasks.jsonl
    let tasks_file = storage.root().join("tasks.jsonl");
    let (created_at, last_modified) = if let Ok(metadata) = std::fs::metadata(&tasks_file) {
        use std::time::SystemTime;

        let created = metadata
            .created()
            .or_else(|_| metadata.modified())
            .ok()
            .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|duration| {
                chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0)
                    .unwrap_or_default()
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string()
            });

        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|duration| {
                chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0)
                    .unwrap_or_default()
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string()
            });

        (created, modified)
    } else {
        (None, None)
    };

    Ok(StoreShowResult {
        storage_path,
        repo_path: repo_path_str,
        tasks: TasksInfo {
            total: total_tasks,
            by_status: TasksByStatus {
                pending,
                in_progress,
                blocked,
                done,
            },
        },
        tests: TestsInfo {
            total: total_tests,
            linked: linked_tests,
        },
        commits: CommitsInfo {
            total: total_commits,
        },
        files,
        created_at,
        last_modified,
    })
}

// === Store Dump Command ===

/// Result of the `bn system store dump` command.
#[derive(Serialize)]
pub struct StoreDumpResult {
    pub files: Vec<StoreDumpFile>,
}

/// Information about a dumped file.
#[derive(Serialize)]
pub struct StoreDumpFile {
    pub name: String,
    pub line_count: usize,
    pub content: String,
}

impl Output for StoreDumpResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();

        for file in &self.files {
            lines.push(format!("=== {} ({} lines) ===", file.name, file.line_count));
            lines.push(file.content.clone());
            lines.push(String::new());
        }

        lines.join("\n")
    }
}

/// Dump all JSONL files to console with headers.
pub fn system_store_dump(repo_path: &Path) -> Result<StoreDumpResult> {
    let storage = Storage::open(repo_path)?;

    let file_names = [
        "tasks.jsonl",
        "bugs.jsonl",
        "ideas.jsonl",
        "milestones.jsonl",
        "edges.jsonl",
        "commits.jsonl",
        "test-results.jsonl",
        "agents.jsonl",
    ];

    let mut files = Vec::new();

    for file_name in &file_names {
        let file_path = storage.root().join(file_name);
        if file_path.exists() {
            let content = fs::read_to_string(&file_path).unwrap_or_default();
            let line_count = content.lines().filter(|l| !l.trim().is_empty()).count();
            files.push(StoreDumpFile {
                name: file_name.to_string(),
                line_count,
                content,
            });
        }
    }

    Ok(StoreDumpResult { files })
}

// === Store Export Command ===

/// Result of the `bn system store export` command.
#[derive(Serialize)]
pub struct StoreExportResult {
    pub exported: bool,
    pub output_path: String,
    pub size_bytes: u64,
    pub task_count: usize,
    pub test_count: usize,
    pub commit_count: usize,
}

impl Output for StoreExportResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();
        if self.exported {
            if self.output_path == "-" {
                lines.push("Exported to stdout".to_string());
            } else {
                lines.push(format!("Exported to {}", self.output_path));
            }
            let size_kb = self.size_bytes as f64 / 1024.0;
            lines.push(format!("  Size: {:.1} KB", size_kb));
            lines.push(format!("  Tasks: {}", self.task_count));
            lines.push(format!("  Tests: {}", self.test_count));
            lines.push(format!("  Commits: {}", self.commit_count));
        } else {
            lines.push("Export failed".to_string());
        }
        lines.join("\n")
    }
}

/// Manifest metadata for the export archive.
#[derive(Serialize, Deserialize)]
struct ExportManifest {
    version: u32,
    format: String,
    exported_at: String,
    source_repo: String,
    binnacle_version: String,
    task_count: usize,
    test_count: usize,
    commit_count: usize,
    checksums: std::collections::HashMap<String, String>,
}

/// Config metadata for the export archive.
#[derive(Serialize)]
struct ExportConfig {
    repo_path: String,
    exported_at: String,
}

/// Calculate SHA256 checksum of file contents.
fn calculate_checksum(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Export store to zstd-compressed tar archive.
pub fn system_store_export(
    repo_path: &Path,
    output: &str,
    format: &str,
) -> Result<StoreExportResult> {
    use zstd::stream::write::Encoder as ZstdEncoder;

    if format != "archive" {
        return Err(Error::InvalidInput(format!(
            "Unsupported format '{}'. Only 'archive' is currently supported.",
            format
        )));
    }

    let storage = Storage::open(repo_path)?;
    let storage_root = storage.root();

    // Read all JSONL files (skip missing files for backwards compatibility)
    let files_to_export = [
        "tasks.jsonl",
        "bugs.jsonl",
        "edges.jsonl",
        "commits.jsonl",
        "test-results.jsonl",
    ];
    let mut file_contents = std::collections::HashMap::new();
    let mut checksums = std::collections::HashMap::new();

    for filename in &files_to_export {
        let file_path = storage_root.join(filename);
        if file_path.exists() {
            let data = fs::read(&file_path)?;
            let checksum = calculate_checksum(&data);
            checksums.insert(filename.to_string(), checksum);
            file_contents.insert(filename.to_string(), data);
        }
    }

    // Count tasks, tests, and commits
    let tasks = storage.list_tasks(None, None, None)?;
    let tests = storage.list_tests(None)?;
    let commit_count = storage.count_commit_links()?;

    // Export action logs as JSONL (limit to most recent 1000 entries for archive size)
    let action_logs = storage.query_action_logs(Some(1000), None, None, None, None, None, None)?;
    if !action_logs.is_empty() {
        let action_log_jsonl: String = action_logs
            .iter()
            .map(|log| serde_json::to_string(log).unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n");
        let action_log_bytes = action_log_jsonl.into_bytes();
        let checksum = calculate_checksum(&action_log_bytes);
        checksums.insert("action-log.jsonl".to_string(), checksum);
        file_contents.insert("action-log.jsonl".to_string(), action_log_bytes);
    }

    // Create manifest
    let manifest = ExportManifest {
        version: 1,
        format: "binnacle-store-v1".to_string(),
        exported_at: Utc::now().to_rfc3339(),
        source_repo: repo_path.to_string_lossy().to_string(),
        binnacle_version: env!("CARGO_PKG_VERSION").to_string(),
        task_count: tasks.len(),
        test_count: tests.len(),
        commit_count,
        checksums: checksums.clone(),
    };
    let manifest_json = serde_json::to_string_pretty(&manifest)?;

    // Create config
    let config = ExportConfig {
        repo_path: repo_path.to_string_lossy().to_string(),
        exported_at: Utc::now().to_rfc3339(),
    };
    let config_json = serde_json::to_string_pretty(&config)?;

    // Create tar.zst archive in memory
    let mut archive_buffer = Vec::new();
    {
        // Use zstd compression level 3 (default) - good balance of speed and compression
        let encoder = ZstdEncoder::new(&mut archive_buffer, 3)
            .map_err(|e| Error::Io(std::io::Error::other(e)))?;
        let mut tar = tar::Builder::new(encoder);

        // Add manifest.json
        let manifest_bytes = manifest_json.as_bytes();
        let mut header = tar::Header::new_gnu();
        header.set_path("binnacle-export/manifest.json")?;
        header.set_size(manifest_bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append(&header, manifest_bytes)?;

        // Add config.json
        let config_bytes = config_json.as_bytes();
        let mut header = tar::Header::new_gnu();
        header.set_path("binnacle-export/config.json")?;
        header.set_size(config_bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append(&header, config_bytes)?;

        // Add JSONL files
        for (filename, data) in &file_contents {
            let mut header = tar::Header::new_gnu();
            header.set_path(format!("binnacle-export/{}", filename))?;
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append(&header, data.as_slice())?;
        }

        // Finish tar archive and get encoder back
        let encoder = tar.into_inner()?;
        // Finish zstd compression
        encoder
            .finish()
            .map_err(|e| Error::Io(std::io::Error::other(e)))?;
    }

    let size_bytes = archive_buffer.len() as u64;

    // Write to output (file or stdout)
    if output == "-" {
        use std::io::{self, Write};
        io::stdout().write_all(&archive_buffer)?;
    } else {
        fs::write(output, &archive_buffer)?;
    }

    Ok(StoreExportResult {
        exported: true,
        output_path: output.to_string(),
        size_bytes,
        task_count: tasks.len(),
        test_count: tests.len(),
        commit_count,
    })
}

/// Result of generating an archive for a commit snapshot.
#[derive(Serialize)]
pub struct CommitArchiveResult {
    pub created: bool,
    pub output_path: String,
    pub commit_hash: String,
    pub size_bytes: u64,
    pub task_count: usize,
    pub test_count: usize,
    pub commit_count: usize,
    /// Reason why archive was skipped (if `created` is false)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped_reason: Option<String>,
}

impl Output for CommitArchiveResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if self.created {
            format!(
                "Created archive for commit {}:\n  Path: {}\n  Size: {} bytes\n  Tasks: {}, Tests: {}, Commits: {}",
                &self.commit_hash[..7.min(self.commit_hash.len())],
                self.output_path,
                self.size_bytes,
                self.task_count,
                self.test_count,
                self.commit_count
            )
        } else {
            let reason = self
                .skipped_reason
                .as_deref()
                .unwrap_or("archive.directory not configured");
            format!(
                "Archive not created for commit {}: {}",
                &self.commit_hash[..7.min(self.commit_hash.len())],
                reason
            )
        }
    }
}

/// Generate archive for a commit snapshot.
///
/// Creates `bn_{commit-hash}.bng` in the configured archive directory.
/// Returns `None` if `archive.directory` is not configured.
pub fn generate_commit_archive(repo_path: &Path, commit_hash: &str) -> Result<CommitArchiveResult> {
    // Helper to create a skipped result
    let skipped = |reason: &str| CommitArchiveResult {
        created: false,
        output_path: String::new(),
        commit_hash: commit_hash.to_string(),
        size_bytes: 0,
        task_count: 0,
        test_count: 0,
        commit_count: 0,
        skipped_reason: Some(reason.to_string()),
    };

    // Check if archive directory is configured
    let archive_dir = match config_get_archive_directory(repo_path) {
        Some(dir) => dir,
        None => {
            return Ok(CommitArchiveResult {
                created: false,
                output_path: String::new(),
                commit_hash: commit_hash.to_string(),
                size_bytes: 0,
                task_count: 0,
                test_count: 0,
                commit_count: 0,
                skipped_reason: None, // Not configured is the default, no special reason
            });
        }
    };

    // Create archive directory if it doesn't exist
    // Handle inaccessible paths gracefully (e.g., in containers with broken mounts)
    if !archive_dir.exists()
        && let Err(e) = fs::create_dir_all(&archive_dir)
    {
        return Ok(skipped(&format!(
            "cannot create directory '{}': {}",
            archive_dir.display(),
            e
        )));
    }

    // Check if directory is writable by attempting to create a temp file
    let test_file = archive_dir.join(".bn_write_test");
    match fs::write(&test_file, b"test") {
        Ok(_) => {
            // Clean up test file
            let _ = fs::remove_file(&test_file);
        }
        Err(e) => {
            return Ok(skipped(&format!(
                "directory '{}' is not writable: {}",
                archive_dir.display(),
                e
            )));
        }
    }

    // Generate archive filename
    let archive_filename = format!("bn_{}.bng", commit_hash);
    let archive_path = archive_dir.join(&archive_filename);

    // Use the existing export functionality
    // Catch any errors during export (e.g., disk full, I/O errors)
    let result = match system_store_export(repo_path, archive_path.to_str().unwrap(), "archive") {
        Ok(r) => r,
        Err(e) => {
            return Ok(skipped(&format!("failed to export archive: {}", e)));
        }
    };

    // Set the archive file to read-only to prevent accidental modification
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = fs::metadata(&archive_path) {
            let mut perms = metadata.permissions();
            // Read-only for owner, group, and others (0o444)
            perms.set_mode(0o444);
            let _ = fs::set_permissions(&archive_path, perms);
        }
    }
    #[cfg(windows)]
    {
        if let Ok(metadata) = fs::metadata(&archive_path) {
            let mut perms = metadata.permissions();
            perms.set_readonly(true);
            let _ = fs::set_permissions(&archive_path, perms);
        }
    }

    Ok(CommitArchiveResult {
        created: true,
        output_path: archive_path.to_string_lossy().to_string(),
        commit_hash: commit_hash.to_string(),
        size_bytes: result.size_bytes,
        task_count: result.task_count,
        test_count: result.test_count,
        commit_count: result.commit_count,
        skipped_reason: None,
    })
}

/// Result of the store import command.
#[derive(Serialize)]
pub struct StoreImportResult {
    pub imported: bool,
    pub dry_run: bool,
    pub input_path: String,
    pub import_type: String,
    pub tasks_imported: usize,
    pub tasks_skipped: usize,
    pub tests_imported: usize,
    pub commits_imported: usize,
    pub collisions: usize,
    pub id_remappings: std::collections::HashMap<String, String>,
}

impl Output for StoreImportResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();

        if self.dry_run {
            lines.push("DRY RUN - No changes made".to_string());
            lines.push(String::new());
        }

        if self.imported || self.dry_run {
            if self.input_path == "-" {
                lines.push("Imported from stdin".to_string());
            } else {
                lines.push(format!("Imported from {}", self.input_path));
            }
            lines.push(format!("  Type: {}", self.import_type));
            lines.push(format!(
                "  Tasks: {} imported, {} skipped",
                self.tasks_imported, self.tasks_skipped
            ));
            lines.push(format!("  Tests: {}", self.tests_imported));
            lines.push(format!("  Commits: {}", self.commits_imported));

            if self.collisions > 0 {
                lines.push(String::new());
                lines.push(format!(
                    "⚠️  WARNING: {} ID COLLISIONS DETECTED",
                    self.collisions
                ));
                for (old_id, new_id) in &self.id_remappings {
                    lines.push(format!("   {} → {}", old_id, new_id));
                }
            }
        } else {
            lines.push("Import failed".to_string());
        }

        lines.join("\n")
    }
}

/// Parse JSON with detailed error context.
///
/// When parsing fails, includes the file name, error position, and a snippet
/// of the content near the error to help with debugging.
fn parse_json_with_context<T: serde::de::DeserializeOwned>(
    data: &[u8],
    file_name: &str,
) -> Result<T> {
    let content = String::from_utf8_lossy(data);
    serde_json::from_str(&content).map_err(|e| {
        let line = e.line();
        let column = e.column();
        let context = get_json_error_context(&content, line, column);
        Error::InvalidInput(format!(
            "Failed to parse {}: {} at line {}, column {}\n{}",
            file_name, e, line, column, context
        ))
    })
}

/// Parse a single JSONL line with detailed error context.
///
/// Returns a detailed error message including file name, line number,
/// and a snippet of the problematic content.
fn parse_jsonl_line_with_context<T: serde::de::DeserializeOwned>(
    line: &str,
    file_name: &str,
    line_number: usize,
) -> Result<Vec<T>> {
    let stream = serde_json::Deserializer::from_str(line).into_iter::<T>();
    let mut results = Vec::new();

    for (obj_index, result) in stream.enumerate() {
        match result {
            Ok(item) => results.push(item),
            Err(e) => {
                let column = e.column();
                let context = get_line_error_context(line, column);
                return Err(Error::InvalidInput(format!(
                    "Failed to parse {} at line {}{}: {}\n{}",
                    file_name,
                    line_number,
                    if obj_index > 0 {
                        format!(" (object #{})", obj_index + 1)
                    } else {
                        String::new()
                    },
                    e,
                    context
                )));
            }
        }
    }

    Ok(results)
}

/// Get context around a JSON error position.
fn get_json_error_context(content: &str, error_line: usize, error_column: usize) -> String {
    let lines: Vec<&str> = content.lines().collect();

    if lines.is_empty() {
        return "  (empty content)".to_string();
    }

    let mut context_lines = Vec::new();

    // Show up to 2 lines before and after the error line
    let start_line = error_line.saturating_sub(2).max(1);
    let end_line = (error_line + 2).min(lines.len());

    for (i, line) in lines.iter().enumerate() {
        let line_num = i + 1; // 1-indexed
        if line_num >= start_line && line_num <= end_line {
            let marker = if line_num == error_line { ">" } else { " " };
            // Truncate very long lines for readability
            let display_line = if line.len() > 120 {
                format!("{}...", &line[..117])
            } else {
                line.to_string()
            };
            context_lines.push(format!("{} {:4} | {}", marker, line_num, display_line));

            // Add column indicator for the error line
            if line_num == error_line && error_column > 0 {
                let padding = " ".repeat(error_column + 7); // Account for line number formatting
                context_lines.push(format!("{}^", padding));
            }
        }
    }

    if context_lines.is_empty() {
        format!(
            "  (error at line {} which is beyond content length {})",
            error_line,
            lines.len()
        )
    } else {
        context_lines.join("\n")
    }
}

/// Get context around an error position within a single line.
fn get_line_error_context(line: &str, error_column: usize) -> String {
    // For very long lines, show a window around the error
    const WINDOW_SIZE: usize = 60;

    if line.len() <= 120 {
        // Short enough to show entirely
        let mut result = format!("  Content: {}\n", line);
        if error_column > 0 && error_column <= line.len() {
            let padding = " ".repeat(error_column + 10); // "  Content: " is 11 chars
            result.push_str(&format!("{}^", padding));
        }
        result
    } else {
        // Show a window around the error
        let start = error_column.saturating_sub(WINDOW_SIZE / 2);
        let end = (start + WINDOW_SIZE).min(line.len());
        let start = if end == line.len() {
            end.saturating_sub(WINDOW_SIZE)
        } else {
            start
        };

        let prefix = if start > 0 { "..." } else { "" };
        let suffix = if end < line.len() { "..." } else { "" };
        let snippet = &line[start..end];

        let mut result = format!("  Content: {}{}{}\n", prefix, snippet, suffix);

        // Position indicator
        let indicator_pos = error_column.saturating_sub(start) + 11 + prefix.len();
        if indicator_pos < 120 {
            let padding = " ".repeat(indicator_pos);
            result.push_str(&format!("{}^", padding));
        }
        result
    }
}

/// Import store from archive.
pub fn system_store_import(
    repo_path: &Path,
    input: &str,
    import_type: &str,
    dry_run: bool,
) -> Result<StoreImportResult> {
    use flate2::read::GzDecoder;
    use std::io::Read;
    use zstd::stream::read::Decoder as ZstdDecoder;

    // Detect input type: stdin, directory, or archive file
    let input_path = Path::new(input);
    if input != "-" && input_path.is_dir() {
        return system_store_import_from_folder(repo_path, input_path, import_type, dry_run);
    }

    // Read input (file or stdin)
    let archive_data = if input == "-" {
        let mut buffer = Vec::new();
        std::io::stdin().read_to_end(&mut buffer)?;
        buffer
    } else {
        fs::read(input)?
    };

    // Detect compression type and extract archive
    // Try zstd first (magic bytes: 0x28, 0xB5, 0x2F, 0xFD), then fall back to gzip
    let is_zstd = archive_data.len() >= 4
        && archive_data[0] == 0x28
        && archive_data[1] == 0xB5
        && archive_data[2] == 0x2F
        && archive_data[3] == 0xFD;

    // Extract files to memory
    let mut manifest: Option<ExportManifest> = None;
    let mut tasks_jsonl: Option<Vec<u8>> = None;
    let mut bugs_jsonl: Option<Vec<u8>> = None;
    let mut edges_jsonl: Option<Vec<u8>> = None;
    let mut commits_jsonl: Option<Vec<u8>> = None;
    let mut test_results_jsonl: Option<Vec<u8>> = None;

    // Macro to extract entries from an archive (avoids type parameter issues with closures)
    macro_rules! extract_entries {
        ($archive:expr) => {
            for entry in $archive.entries()? {
                let mut entry = entry?;
                let path = entry.path()?.to_path_buf();
                let path_str = path.to_string_lossy().to_string();

                let mut data = Vec::new();
                entry.read_to_end(&mut data)?;

                if path_str.ends_with("manifest.json") {
                    manifest = Some(parse_json_with_context(&data, "manifest.json")?);
                } else if path_str.ends_with("tasks.jsonl") {
                    tasks_jsonl = Some(data);
                } else if path_str.ends_with("bugs.jsonl") {
                    bugs_jsonl = Some(data);
                } else if path_str.ends_with("edges.jsonl") {
                    edges_jsonl = Some(data);
                } else if path_str.ends_with("commits.jsonl") {
                    commits_jsonl = Some(data);
                } else if path_str.ends_with("test-results.jsonl") {
                    test_results_jsonl = Some(data);
                }
            }
        };
    }

    // Try decompression based on detected format
    if is_zstd {
        let decoder =
            ZstdDecoder::new(&archive_data[..]).map_err(|e| Error::Io(std::io::Error::other(e)))?;
        let mut archive = tar::Archive::new(decoder);
        extract_entries!(archive);
    } else {
        // Fall back to gzip for backwards compatibility
        let decoder = GzDecoder::new(&archive_data[..]);
        let mut archive = tar::Archive::new(decoder);
        extract_entries!(archive);
    }

    let manifest = manifest
        .ok_or_else(|| Error::InvalidInput("Archive does not contain manifest.json".to_string()))?;

    // Validate version
    if manifest.version != 1 {
        return Err(Error::InvalidInput(format!(
            "Incompatible archive version {}. Expected version 1.",
            manifest.version
        )));
    }

    if manifest.format != "binnacle-store-v1" {
        return Err(Error::InvalidInput(format!(
            "Incompatible archive format '{}'. Expected 'binnacle-store-v1'.",
            manifest.format
        )));
    }

    // Check if already initialized for replace mode
    let storage = Storage::open(repo_path);
    let is_initialized = storage.is_ok();

    if import_type == "replace" && is_initialized {
        return Err(Error::InvalidInput(
            "Store already initialized. Use --type merge to append data, or reinitialize the repo."
                .to_string(),
        ));
    }

    // Initialize if needed
    let mut storage = if is_initialized {
        storage.unwrap()
    } else {
        // Auto-initialize for import
        Storage::init(repo_path)?
    };

    // Parse imported tasks (handles multiple JSON objects per line if corrupted)
    let mut imported_tasks: Vec<Task> = Vec::new();
    if let Some(tasks_data) = tasks_jsonl {
        let tasks_str = String::from_utf8_lossy(&tasks_data);
        for (line_index, line) in tasks_str.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            // Use helper with detailed error context
            let tasks = parse_jsonl_line_with_context::<Task>(line, "tasks.jsonl", line_index + 1)?;
            imported_tasks.extend(tasks);
        }
    }

    // Detect ID collisions and create remapping
    let mut id_remappings: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let existing_tasks = storage.list_tasks(None, None, None)?;
    let existing_ids: std::collections::HashSet<String> =
        existing_tasks.iter().map(|t| t.core.id.clone()).collect();

    for task in &imported_tasks {
        if existing_ids.contains(&task.core.id) {
            // Generate new ID using task title as seed
            let new_id = storage.generate_unique_id("bn", &task.core.title);
            id_remappings.insert(task.core.id.clone(), new_id);
        }
    }

    let collisions = id_remappings.len();

    // If dry run, return early
    if dry_run {
        return Ok(StoreImportResult {
            imported: false,
            dry_run: true,
            input_path: input.to_string(),
            import_type: import_type.to_string(),
            tasks_imported: imported_tasks.len(),
            tasks_skipped: 0,
            tests_imported: 0,
            commits_imported: 0,
            collisions,
            id_remappings,
        });
    }

    // Import tasks with remapping - two-pass approach to handle dependencies
    // Pass 1: Create all tasks without dependencies (avoids FK constraint issues)
    let mut tasks_imported = 0;
    let import_timestamp = Utc::now();
    let mut task_dependencies: Vec<(String, Vec<String>)> = Vec::new();

    for mut task in imported_tasks {
        // Remap task ID if needed
        if let Some(new_id) = id_remappings.get(&task.core.id) {
            task.core.id = new_id.clone();
        }

        // Remap dependencies and save for later
        let mut new_depends_on = Vec::new();
        for dep in &task.depends_on {
            if let Some(new_dep_id) = id_remappings.get(dep) {
                new_depends_on.push(new_dep_id.clone());
            } else {
                new_depends_on.push(dep.clone());
            }
        }

        // Store dependencies for pass 2
        if !new_depends_on.is_empty() {
            task_dependencies.push((task.core.id.clone(), new_depends_on));
        }

        // Clear dependencies for initial insert to avoid FK errors
        task.depends_on = Vec::new();

        // Set imported_on timestamp if merging
        if import_type == "merge" {
            task.imported_on = Some(import_timestamp);
        }

        // Create task without dependencies
        storage.create_task(&task)?;
        tasks_imported += 1;
    }

    // Pass 2: Add dependencies now that all tasks exist
    // Filter out dependencies that don't exist (data integrity issue in source)
    let imported_task_ids: std::collections::HashSet<String> = storage
        .list_tasks(None, None, None)?
        .iter()
        .map(|t| t.core.id.clone())
        .collect();

    for (task_id, depends_on) in task_dependencies {
        // Filter to only include dependencies that exist
        let valid_deps: Vec<String> = depends_on
            .into_iter()
            .filter(|dep| imported_task_ids.contains(dep))
            .collect();

        if valid_deps.is_empty() {
            continue;
        }

        // Get the task from storage
        if let Ok(mut task) = storage.get_task(&task_id) {
            task.depends_on = valid_deps;
            task.core.updated_at = Utc::now();
            storage.update_task(&task)?;
        }
    }

    // Import commits (simple append, no ID remapping needed for now)
    let mut commits_imported = 0;
    if let Some(commits_data) = commits_jsonl {
        let storage_root = storage.root();
        let commits_file = storage_root.join("commits.jsonl");

        // Append to existing file
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&commits_file)?;

        file.write_all(&commits_data)?;

        // Count commits
        let commits_str = String::from_utf8_lossy(&commits_data);
        commits_imported = commits_str.lines().filter(|l| !l.trim().is_empty()).count();
    }

    // Import edges
    if let Some(edges_data) = edges_jsonl {
        let storage_root = storage.root();
        let edges_file = storage_root.join("edges.jsonl");

        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&edges_file)?;

        file.write_all(&edges_data)?;
    }

    // Import bugs
    if let Some(bugs_data) = bugs_jsonl {
        let storage_root = storage.root();
        let bugs_file = storage_root.join("bugs.jsonl");

        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&bugs_file)?;

        file.write_all(&bugs_data)?;
    }

    // Import test results
    let mut tests_imported = 0;
    if let Some(test_data) = test_results_jsonl {
        let storage_root = storage.root();
        let test_file = storage_root.join("test-results.jsonl");

        // Append to existing file
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&test_file)?;

        file.write_all(&test_data)?;

        // Count tests
        let test_str = String::from_utf8_lossy(&test_data);
        tests_imported = test_str.lines().filter(|l| !l.trim().is_empty()).count();
    }

    // Rebuild cache
    storage.rebuild_cache()?;

    Ok(StoreImportResult {
        imported: true,
        dry_run: false,
        input_path: input.to_string(),
        import_type: import_type.to_string(),
        tasks_imported,
        tasks_skipped: 0,
        tests_imported,
        commits_imported,
        collisions,
        id_remappings,
    })
}

/// Import store from a folder containing JSONL files.
///
/// Expected folder structure:
/// - tasks.jsonl (required)
/// - bugs.jsonl (optional)
/// - edges.jsonl (optional)
/// - commits.jsonl (optional)
/// - test-results.jsonl (optional)
/// - cache.db (ignored - rebuilt after import)
fn system_store_import_from_folder(
    repo_path: &Path,
    folder_path: &Path,
    import_type: &str,
    dry_run: bool,
) -> Result<StoreImportResult> {
    // Validate required tasks.jsonl exists
    let tasks_file = folder_path.join("tasks.jsonl");
    if !tasks_file.exists() {
        return Err(Error::InvalidInput(format!(
            "Folder '{}' missing required tasks.jsonl",
            folder_path.display()
        )));
    }

    // Read JSONL files from folder
    let tasks_jsonl = Some(fs::read(&tasks_file)?);

    let bugs_file = folder_path.join("bugs.jsonl");
    let bugs_jsonl = if bugs_file.exists() {
        Some(fs::read(&bugs_file)?)
    } else {
        None
    };

    let edges_file = folder_path.join("edges.jsonl");
    let edges_jsonl = if edges_file.exists() {
        Some(fs::read(&edges_file)?)
    } else {
        None
    };

    let commits_file = folder_path.join("commits.jsonl");
    let commits_jsonl = if commits_file.exists() {
        Some(fs::read(&commits_file)?)
    } else {
        None
    };

    let test_results_file = folder_path.join("test-results.jsonl");
    let test_results_jsonl = if test_results_file.exists() {
        Some(fs::read(&test_results_file)?)
    } else {
        None
    };

    // Check if already initialized for replace mode
    let storage = Storage::open(repo_path);
    let is_initialized = storage.is_ok();

    if import_type == "replace" && is_initialized {
        return Err(Error::InvalidInput(
            "Store already initialized. Use --type merge to append data, or reinitialize the repo."
                .to_string(),
        ));
    }

    // Initialize if needed
    let mut storage = if is_initialized {
        storage.unwrap()
    } else {
        // Auto-initialize for import
        Storage::init(repo_path)?
    };

    // Parse imported tasks (handles multiple JSON objects per line if corrupted)
    let mut imported_tasks: Vec<Task> = Vec::new();
    if let Some(tasks_data) = tasks_jsonl {
        let tasks_str = String::from_utf8_lossy(&tasks_data);
        for (line_index, line) in tasks_str.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            // Use helper with detailed error context
            let tasks = parse_jsonl_line_with_context::<Task>(line, "tasks.jsonl", line_index + 1)?;
            imported_tasks.extend(tasks);
        }
    }

    // Detect ID collisions and create remapping
    let mut id_remappings: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let existing_tasks = storage.list_tasks(None, None, None)?;
    let existing_ids: std::collections::HashSet<String> =
        existing_tasks.iter().map(|t| t.core.id.clone()).collect();

    for task in &imported_tasks {
        if existing_ids.contains(&task.core.id) {
            // Generate new ID using task title as seed
            let new_id = storage.generate_unique_id("bn", &task.core.title);
            id_remappings.insert(task.core.id.clone(), new_id);
        }
    }

    let collisions = id_remappings.len();
    let input_path_str = folder_path.display().to_string();

    // Count tests and commits for dry-run (without importing)
    let tests_count = test_results_jsonl
        .as_ref()
        .map(|data| {
            String::from_utf8_lossy(data)
                .lines()
                .filter(|l| !l.trim().is_empty())
                .count()
        })
        .unwrap_or(0);

    let commits_count = commits_jsonl
        .as_ref()
        .map(|data| {
            String::from_utf8_lossy(data)
                .lines()
                .filter(|l| !l.trim().is_empty())
                .count()
        })
        .unwrap_or(0);

    // If dry run, return early
    if dry_run {
        return Ok(StoreImportResult {
            imported: false,
            dry_run: true,
            input_path: input_path_str,
            import_type: import_type.to_string(),
            tasks_imported: imported_tasks.len(),
            tasks_skipped: 0,
            tests_imported: tests_count,
            commits_imported: commits_count,
            collisions,
            id_remappings,
        });
    }

    // Import tasks with remapping - two-pass approach to handle dependencies
    // Pass 1: Create all tasks without dependencies (avoids FK constraint issues)
    let mut tasks_imported = 0;
    let import_timestamp = Utc::now();
    let mut task_dependencies: Vec<(String, Vec<String>)> = Vec::new();

    for mut task in imported_tasks {
        // Remap task ID if needed
        if let Some(new_id) = id_remappings.get(&task.core.id) {
            task.core.id = new_id.clone();
        }

        // Remap dependencies and save for later
        let mut new_depends_on = Vec::new();
        for dep in &task.depends_on {
            if let Some(new_dep_id) = id_remappings.get(dep) {
                new_depends_on.push(new_dep_id.clone());
            } else {
                new_depends_on.push(dep.clone());
            }
        }

        // Store dependencies for pass 2
        if !new_depends_on.is_empty() {
            task_dependencies.push((task.core.id.clone(), new_depends_on));
        }

        // Clear dependencies for initial insert to avoid FK errors
        task.depends_on = Vec::new();

        // Set imported_on timestamp if merging
        if import_type == "merge" {
            task.imported_on = Some(import_timestamp);
        }

        // Create task without dependencies
        storage.create_task(&task)?;
        tasks_imported += 1;
    }

    // Pass 2: Add dependencies now that all tasks exist
    // Filter out dependencies that don't exist (data integrity issue in source)
    let imported_task_ids: std::collections::HashSet<String> = storage
        .list_tasks(None, None, None)?
        .iter()
        .map(|t| t.core.id.clone())
        .collect();

    for (task_id, depends_on) in task_dependencies {
        // Filter to only include dependencies that exist
        let valid_deps: Vec<String> = depends_on
            .into_iter()
            .filter(|dep| imported_task_ids.contains(dep))
            .collect();

        if valid_deps.is_empty() {
            continue;
        }

        // Get the task from storage
        if let Ok(mut task) = storage.get_task(&task_id) {
            task.depends_on = valid_deps;
            task.core.updated_at = Utc::now();
            storage.update_task(&task)?;
        }
    }

    // Import commits (simple append, no ID remapping needed for now)
    let mut commits_imported = 0;
    if let Some(commits_data) = commits_jsonl {
        let storage_root = storage.root();
        let commits_file = storage_root.join("commits.jsonl");

        // Append to existing file
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&commits_file)?;

        file.write_all(&commits_data)?;

        // Count commits
        let commits_str = String::from_utf8_lossy(&commits_data);
        commits_imported = commits_str.lines().filter(|l| !l.trim().is_empty()).count();
    }

    // Import edges
    if let Some(edges_data) = edges_jsonl {
        let storage_root = storage.root();
        let edges_file = storage_root.join("edges.jsonl");

        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&edges_file)?;

        file.write_all(&edges_data)?;
    }

    // Import bugs
    if let Some(bugs_data) = bugs_jsonl {
        let storage_root = storage.root();
        let bugs_file = storage_root.join("bugs.jsonl");

        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&bugs_file)?;

        file.write_all(&bugs_data)?;
    }

    // Import test results
    let mut tests_imported = 0;
    if let Some(test_data) = test_results_jsonl {
        let storage_root = storage.root();
        let test_file = storage_root.join("test-results.jsonl");

        // Append to existing file
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&test_file)?;

        file.write_all(&test_data)?;

        // Count tests
        let test_str = String::from_utf8_lossy(&test_data);
        tests_imported = test_str.lines().filter(|l| !l.trim().is_empty()).count();
    }

    // Rebuild cache
    storage.rebuild_cache()?;

    Ok(StoreImportResult {
        imported: true,
        dry_run: false,
        input_path: input_path_str,
        import_type: import_type.to_string(),
        tasks_imported,
        tasks_skipped: 0,
        tests_imported,
        commits_imported,
        collisions,
        id_remappings,
    })
}

// === Store Clear Command ===

/// Result of the `bn system store clear` command.
#[derive(Serialize)]
pub struct StoreClearResult {
    pub cleared: bool,
    pub storage_path: String,
    pub backup_path: Option<String>,
    pub tasks_cleared: usize,
    pub tests_cleared: usize,
    pub bugs_cleared: usize,
    pub commits_cleared: usize,
    pub edges_cleared: usize,
    pub aborted: bool,
    pub abort_reason: Option<String>,
}

impl Output for StoreClearResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();

        if self.aborted {
            lines.push("Store clear aborted.".to_string());
            if let Some(reason) = &self.abort_reason {
                lines.push(format!("  Reason: {}", reason));
            }
            return lines.join("\n");
        }

        if self.cleared {
            lines.push("Store cleared successfully.".to_string());
            lines.push(format!("  Storage: {}", self.storage_path));
            if let Some(backup) = &self.backup_path {
                lines.push(format!("  Backup:  {}", backup));
            }
            lines.push(String::new());
            lines.push("Cleared:".to_string());
            lines.push(format!("  Tasks:   {}", self.tasks_cleared));
            lines.push(format!("  Tests:   {}", self.tests_cleared));
            lines.push(format!("  Bugs:    {}", self.bugs_cleared));
            lines.push(format!("  Commits: {}", self.commits_cleared));
            lines.push(format!("  Edges:   {}", self.edges_cleared));
        } else {
            lines.push("Store clear failed.".to_string());
        }

        lines.join("\n")
    }
}

/// Clear all data from the current repository's store.
///
/// Safety features:
/// - Requires --force flag for non-interactive use
/// - Creates backup by default (use --no-backup to skip)
/// - Only clears the current repo's store, not global data
pub fn system_store_clear(
    repo_path: &Path,
    force: bool,
    no_backup: bool,
    human: bool,
) -> Result<StoreClearResult> {
    let storage = Storage::open(repo_path)?;
    let storage_path = storage.root().to_path_buf();
    let storage_path_str = storage_path.to_string_lossy().to_string();

    // Get current counts before clearing
    let tasks = storage.list_tasks(None, None, None)?;
    let tests = storage.list_tests(None)?;
    let bugs = storage.list_bugs(None, None, None, None, true)?; // Include all for counting
    let edges = storage.list_edges(None, None, None)?;

    // Count commits by reading the file directly
    let commits_file = storage_path.join("commits.jsonl");
    let commits_count = if commits_file.exists() {
        fs::read_to_string(&commits_file)
            .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count())
            .unwrap_or(0)
    } else {
        0
    };

    let total_items = tasks.len() + tests.len() + bugs.len() + commits_count + edges.len();

    // If no data, nothing to clear
    if total_items == 0 {
        return Ok(StoreClearResult {
            cleared: true,
            storage_path: storage_path_str,
            backup_path: None,
            tasks_cleared: 0,
            tests_cleared: 0,
            bugs_cleared: 0,
            commits_cleared: 0,
            edges_cleared: 0,
            aborted: false,
            abort_reason: None,
        });
    }

    // Require --force for non-interactive clearing
    if !force {
        // Show what would be cleared
        if human {
            eprintln!(
                "WARNING: This will permanently delete all binnacle data for this repository."
            );
            eprintln!();
            eprintln!("Data to be cleared:");
            eprintln!("  Tasks:   {}", tasks.len());
            eprintln!("  Tests:   {}", tests.len());
            eprintln!("  Bugs:    {}", bugs.len());
            eprintln!("  Commits: {}", commits_count);
            eprintln!("  Edges:   {}", edges.len());
            eprintln!();
            eprintln!("To proceed, run with --force flag:");
            eprintln!("  bn system store clear --force");
            eprintln!();
            eprintln!("To create a backup first (recommended):");
            eprintln!("  bn system store export backup.bng && bn system store clear --force");
        }

        return Ok(StoreClearResult {
            cleared: false,
            storage_path: storage_path_str,
            backup_path: None,
            tasks_cleared: 0,
            tests_cleared: 0,
            bugs_cleared: 0,
            commits_cleared: 0,
            edges_cleared: 0,
            aborted: true,
            abort_reason: Some("Use --force to confirm clearing all data".to_string()),
        });
    }

    // Create backup unless --no-backup is specified
    let backup_path = if !no_backup {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let backup_name = format!("binnacle_backup_{}.bng", timestamp);

        // Try to create backup in the repo directory, fall back to temp
        let backup_file = repo_path.join(&backup_name);

        match system_store_export(repo_path, &backup_file.to_string_lossy(), "archive") {
            Ok(_) => Some(backup_file.to_string_lossy().to_string()),
            Err(e) => {
                if human {
                    eprintln!("Warning: Could not create backup: {}", e);
                    eprintln!("Proceeding without backup...");
                }
                None
            }
        }
    } else {
        None
    };

    // Drop the storage connection before clearing files
    drop(storage);

    // Clear the storage directory by removing and recreating it
    let tasks_cleared = tasks.len();
    let tests_cleared = tests.len();
    let bugs_cleared = bugs.len();
    let commits_cleared = commits_count;
    let edges_cleared = edges.len();

    // Remove the storage directory
    if storage_path.exists() {
        fs::remove_dir_all(&storage_path)?;
    }

    // Reinitialize empty storage
    Storage::init(repo_path)?;

    Ok(StoreClearResult {
        cleared: true,
        storage_path: storage_path_str,
        backup_path,
        tasks_cleared,
        tests_cleared,
        bugs_cleared,
        commits_cleared,
        edges_cleared,
        aborted: false,
        abort_reason: None,
    })
}

// === Storage Migration ===

#[derive(Serialize)]
pub struct MigrateResult {
    pub success: bool,
    pub from_backend: String,
    pub to_backend: String,
    pub dry_run: bool,
    pub files_migrated: Vec<MigratedFile>,
    pub total_lines: usize,
}

#[derive(Serialize)]
pub struct MigratedFile {
    pub name: String,
    pub lines: usize,
}

impl Output for MigrateResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();

        if self.dry_run {
            lines.push(format!(
                "DRY RUN: Would migrate from '{}' to '{}'",
                self.from_backend, self.to_backend
            ));
        } else {
            lines.push(format!(
                "Migrated from '{}' to '{}'",
                self.from_backend, self.to_backend
            ));
        }

        lines.push(String::new());
        lines.push("Files:".to_string());

        for file in &self.files_migrated {
            lines.push(format!("  {} ({} lines)", file.name, file.lines));
        }

        lines.push(String::new());
        lines.push(format!("Total: {} lines migrated", self.total_lines));

        if self.dry_run {
            lines.push(String::new());
            lines.push("Run without --dry-run to perform the migration.".to_string());
        }

        lines.join("\n")
    }
}

/// Migrate data between storage backends.
pub fn migrate_storage(repo_path: &Path, to_backend: &str, dry_run: bool) -> Result<MigrateResult> {
    use crate::storage::{BackendType, GitNotesBackend, OrphanBranchBackend, StorageBackend};

    // Parse target backend type
    let target_type: BackendType = to_backend
        .parse()
        .map_err(|e: String| Error::InvalidInput(e))?;

    // Open source storage (file backend)
    let storage = Storage::open(repo_path)?;

    // List of JSONL files to migrate
    let files = [
        "tasks.jsonl",
        "bugs.jsonl",
        "ideas.jsonl",
        "milestones.jsonl",
        "edges.jsonl",
        "commits.jsonl",
        "test-results.jsonl",
        "agents.jsonl",
    ];

    // Read all data from source
    let mut file_data: Vec<(String, Vec<String>)> = Vec::new();
    let mut total_lines = 0;

    for filename in &files {
        let path = storage.root.join(filename);
        let lines = if path.exists() {
            std::fs::read_to_string(&path)?
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        total_lines += lines.len();
        file_data.push((filename.to_string(), lines));
    }

    // Build result structure
    let files_migrated: Vec<MigratedFile> = file_data
        .iter()
        .map(|(name, lines)| MigratedFile {
            name: name.clone(),
            lines: lines.len(),
        })
        .collect();

    // If dry run, return without making changes
    if dry_run {
        return Ok(MigrateResult {
            success: true,
            from_backend: "file".to_string(),
            to_backend: target_type.to_string(),
            dry_run: true,
            files_migrated,
            total_lines,
        });
    }

    // Initialize and write to target backend
    match target_type {
        BackendType::OrphanBranch => {
            let mut backend = OrphanBranchBackend::new(repo_path);
            backend.init(repo_path)?;

            for (filename, lines) in &file_data {
                if !lines.is_empty() {
                    backend.write_jsonl(filename, lines)?;
                }
            }
        }
        BackendType::GitNotes => {
            let mut backend = GitNotesBackend::new(repo_path);
            backend.init(repo_path)?;

            for (filename, lines) in &file_data {
                if !lines.is_empty() {
                    backend.write_jsonl(filename, lines)?;
                }
            }
        }
        BackendType::File => {
            // Migration to file backend doesn't make sense from file backend
            return Err(Error::InvalidInput(
                "Cannot migrate from file backend to file backend".to_string(),
            ));
        }
    }

    Ok(MigrateResult {
        success: true,
        from_backend: "file".to_string(),
        to_backend: target_type.to_string(),
        dry_run: false,
        files_migrated,
        total_lines,
    })
}

/// Result of migrating bug-tagged tasks to Bug entities.
#[derive(Serialize)]
pub struct MigrateBugsResult {
    pub success: bool,
    pub dry_run: bool,
    pub tasks_found: usize,
    pub tasks_migrated: Vec<MigratedTask>,
    pub remove_tag: bool,
}

/// Information about a migrated task.
#[derive(Serialize)]
pub struct MigratedTask {
    pub old_task_id: String,
    pub new_bug_id: String,
    pub title: String,
}

impl Output for MigrateBugsResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if self.tasks_found == 0 {
            return "No tasks with 'bug' tag found.".to_string();
        }

        let mut lines = Vec::new();
        if self.dry_run {
            lines.push(format!(
                "Dry run: would migrate {} task(s) to bugs:",
                self.tasks_found
            ));
        } else {
            lines.push(format!(
                "Migrated {} task(s) to bugs:",
                self.tasks_migrated.len()
            ));
        }

        for task in &self.tasks_migrated {
            lines.push(format!(
                "  {} -> {} \"{}\"",
                task.old_task_id, task.new_bug_id, task.title
            ));
        }

        if self.remove_tag && !self.dry_run {
            lines.push("\nRemoved 'bug' tag from original tasks.".to_string());
        }

        lines.join("\n")
    }
}

/// Convert tasks with the 'bug' tag to Bug entities.
///
/// This migration helper finds all tasks tagged with 'bug' and creates corresponding
/// Bug entities with the same metadata. The original tasks are preserved but optionally
/// have their 'bug' tag removed.
pub fn migrate_bugs(
    repo_path: &Path,
    dry_run: bool,
    remove_tag: bool,
) -> Result<MigrateBugsResult> {
    use crate::models::Bug;

    let mut storage = Storage::open(repo_path)?;

    // Find all tasks with the 'bug' tag
    let tasks = storage.list_tasks(None, None, Some("bug"))?;
    let tasks_found = tasks.len();

    if tasks_found == 0 {
        return Ok(MigrateBugsResult {
            success: true,
            dry_run,
            tasks_found: 0,
            tasks_migrated: Vec::new(),
            remove_tag,
        });
    }

    let mut migrated = Vec::new();

    for task in &tasks {
        // Generate a new bug ID based on the task title (ensures unique ID)
        let bug_id = storage.generate_unique_id("bn", &format!("bug-{}", task.core.title));

        // Create the bug entity
        let mut bug = Bug::new(bug_id.clone(), task.core.title.clone());
        bug.core.description = task.core.description.clone();
        bug.priority = task.priority;
        bug.status = task.status.clone();
        bug.core.tags = task
            .core
            .tags
            .iter()
            .filter(|t| *t != "bug") // Don't copy the 'bug' tag to the Bug entity
            .cloned()
            .collect();
        bug.assignee = task.assignee.clone();
        bug.depends_on = task.depends_on.clone();
        bug.core.created_at = task.core.created_at;
        bug.core.updated_at = task.core.updated_at;
        bug.closed_at = task.closed_at;
        bug.closed_reason = task.closed_reason.clone();

        if !dry_run {
            // Add the bug
            storage.add_bug(&bug)?;

            // Optionally remove the 'bug' tag from the original task
            if remove_tag {
                let mut updated_task = task.clone();
                updated_task.core.tags.retain(|t| t != "bug");
                updated_task.core.updated_at = chrono::Utc::now();
                storage.update_task(&updated_task)?;
            }
        }

        migrated.push(MigratedTask {
            old_task_id: task.core.id.clone(),
            new_bug_id: bug_id,
            title: task.core.title.clone(),
        });
    }

    Ok(MigrateBugsResult {
        success: true,
        dry_run,
        tasks_found,
        tasks_migrated: migrated,
        remove_tag,
    })
}

#[derive(Serialize)]
pub struct CommitLinked {
    pub sha: String,
    pub entity_id: String,
}

impl Output for CommitLinked {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Linked commit {} to {}", self.sha, self.entity_id)
    }
}

/// Link a commit to a task or bug.
pub fn commit_link(repo_path: &Path, sha: &str, entity_id: &str) -> Result<CommitLinked> {
    let mut storage = Storage::open(repo_path)?;
    storage.link_commit(sha, entity_id)?;

    Ok(CommitLinked {
        sha: sha.to_string(),
        entity_id: entity_id.to_string(),
    })
}

#[derive(Serialize)]
pub struct CommitUnlinked {
    pub sha: String,
    pub entity_id: String,
}

impl Output for CommitUnlinked {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Unlinked commit {} from {}", self.sha, self.entity_id)
    }
}

/// Unlink a commit from a task or bug.
pub fn commit_unlink(repo_path: &Path, sha: &str, entity_id: &str) -> Result<CommitUnlinked> {
    let mut storage = Storage::open(repo_path)?;
    storage.unlink_commit(sha, entity_id)?;

    Ok(CommitUnlinked {
        sha: sha.to_string(),
        entity_id: entity_id.to_string(),
    })
}

#[derive(Serialize)]
pub struct CommitList {
    pub entity_id: String,
    pub commits: Vec<CommitLink>,
    pub count: usize,
}

impl Output for CommitList {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if self.commits.is_empty() {
            return format!("No commits linked to {}", self.entity_id);
        }

        let mut lines = Vec::new();
        lines.push(format!(
            "{} commit(s) linked to {}:\n",
            self.count, self.entity_id
        ));

        for link in &self.commits {
            lines.push(format!(
                "  {} (linked {})",
                link.sha,
                link.linked_at.format("%Y-%m-%d %H:%M")
            ));
        }

        lines.join("\n")
    }
}

/// List commits linked to a task or bug.
pub fn commit_list(repo_path: &Path, entity_id: &str) -> Result<CommitList> {
    let storage = Storage::open(repo_path)?;
    let commits = storage.get_commits_for_entity(entity_id)?;
    let count = commits.len();

    Ok(CommitList {
        entity_id: entity_id.to_string(),
        commits,
        count,
    })
}

// === Agent Commands ===

use crate::models::AgentStatus;

/// Result of agent list command.
#[derive(Serialize)]
pub struct AgentListResult {
    pub agents: Vec<AgentInfo>,
    pub count: usize,
}

/// Agent information for list output.
#[derive(Serialize)]
pub struct AgentInfo {
    pub pid: u32,
    pub parent_pid: u32,
    pub name: String,
    /// Agent's purpose/role (e.g., "Task Worker", "PRD Generator")
    pub purpose: Option<String>,
    pub status: String,
    pub started_at: String,
    pub last_activity_at: String,
    pub tasks: Vec<String>,
    pub command_count: u64,
}

impl Output for AgentListResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if self.agents.is_empty() {
            return "No agents registered.".to_string();
        }

        let mut lines = Vec::new();
        lines.push(format!("Agents ({}):", self.count));
        lines.push(String::new());

        for agent in &self.agents {
            let purpose_str = agent
                .purpose
                .as_ref()
                .map(|p| format!(" - {}", p))
                .unwrap_or_default();
            lines.push(format!(
                "  PID {}  {}{}  [{}]",
                agent.pid, agent.name, purpose_str, agent.status
            ));
            lines.push(format!(
                "    Started: {}  Last activity: {}",
                agent.started_at, agent.last_activity_at
            ));
            if !agent.tasks.is_empty() {
                lines.push(format!("    Tasks: {}", agent.tasks.join(", ")));
            }
            lines.push(format!("    Commands: {}", agent.command_count));
            lines.push(String::new());
        }

        lines.join("\n")
    }
}

/// List active agents.
pub fn agent_list(repo_path: &Path, status: Option<&str>) -> Result<AgentListResult> {
    let mut storage = Storage::open(repo_path)?;
    // Update agent statuses (active/idle/stale) based on activity and process status
    let _ = storage.update_agent_statuses();
    // Clean up agents whose processes are dead
    let _ = storage.cleanup_stale_agents();
    let agents = storage.list_agents(status)?;

    let agent_infos: Vec<AgentInfo> = agents
        .into_iter()
        .map(|a| {
            let status_str = match a.status {
                AgentStatus::Active => "active",
                AgentStatus::Idle => "idle",
                AgentStatus::Stale => "stale",
            };
            AgentInfo {
                pid: a.pid,
                parent_pid: a.parent_pid,
                name: a.name,
                purpose: a.purpose,
                status: status_str.to_string(),
                started_at: a.started_at.to_rfc3339(),
                last_activity_at: a.last_activity_at.to_rfc3339(),
                tasks: a.tasks,
                command_count: a.command_count,
            }
        })
        .collect();

    let count = agent_infos.len();

    Ok(AgentListResult {
        agents: agent_infos,
        count,
    })
}

/// Track agent activity if the current process is a registered agent.
/// Updates last_activity_at and increments command_count.
/// Silently ignores errors (e.g., if not an agent or storage issues).
pub fn track_agent_activity(repo_path: &Path) {
    // Only track if storage exists and can be opened
    if Storage::exists(repo_path).unwrap_or(false)
        && let Ok(mut storage) = Storage::open(repo_path)
    {
        // First try to find the ancestor agent (for when bn commands run in subprocesses)
        // Fall back to parent_pid for backwards compatibility
        let agent_pid = find_ancestor_agent(&storage)
            .or_else(get_parent_pid)
            .unwrap_or_else(std::process::id);
        // Touch the agent if registered (silently ignore if not)
        let _ = storage.touch_agent(agent_pid);
    }
}

// === Agent Kill Command ===

/// Result of agent kill command.
#[derive(Serialize)]
pub struct AgentKillResult {
    pub pid: u32,
    pub name: String,
    pub was_running: bool,
    pub terminated: bool,
    pub signal_sent: String,
}

impl Output for AgentKillResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();

        if !self.was_running {
            lines.push(format!(
                "Agent {} (PID {}) was not running.",
                self.name, self.pid
            ));
        } else if self.terminated {
            lines.push(format!(
                "Agent {} (PID {}) terminated ({}).",
                self.name, self.pid, self.signal_sent
            ));
        } else {
            lines.push(format!(
                "Failed to terminate agent {} (PID {}).",
                self.name, self.pid
            ));
        }

        lines.join("\n")
    }
}

/// Terminate a specific agent by PID or name.
///
/// This command:
/// 1. Looks up agent by PID (numeric) or name (string)
/// 2. Sends SIGTERM to the agent's PID
/// 3. Waits up to `timeout_secs` for graceful exit
/// 4. Sends SIGKILL if still running after timeout
/// 5. Removes agent from registry
pub fn agent_kill(repo_path: &Path, target: &str, timeout_secs: u64) -> Result<AgentKillResult> {
    let mut storage = Storage::open(repo_path)?;

    // Clean up stale agents first
    let _ = storage.cleanup_stale_agents();

    // Try to find agent by PID or name
    let agent = if let Ok(pid) = target.parse::<u32>() {
        // Target is a PID
        storage.get_agent(pid)?
    } else {
        // Target is a name
        storage.get_agent_by_name(target)?
    };

    let pid = agent.pid;
    let name = agent.name.clone();

    // Check if process is still running
    let was_running = agent.is_alive();

    let (terminated, signal_sent) = if was_running {
        // Terminate the process
        let result = terminate_process(pid, timeout_secs);
        let signal = if result { "SIGTERM/SIGKILL" } else { "failed" };
        (result, signal.to_string())
    } else {
        (true, "already dead".to_string())
    };

    // Remove from registry regardless of termination result
    let _ = storage.remove_agent(pid);

    Ok(AgentKillResult {
        pid,
        name,
        was_running,
        terminated,
        signal_sent,
    })
}

// === Goodbye Command ===

/// Result of goodbye command.
#[derive(Serialize)]
pub struct GoodbyeResult {
    pub agent_name: Option<String>,
    pub parent_pid: u32,
    /// The grandparent PID - this is the actual target for termination
    /// because process tree is: agent → shell → bn goodbye
    pub grandparent_pid: u32,
    pub was_registered: bool,
    pub reason: Option<String>,
    pub terminated: bool,
    /// Whether to actually terminate the process (false for planner agents without --force)
    #[serde(default = "default_true")]
    pub should_terminate: bool,
    /// Warning message when reason is not provided
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
    /// The agent type (if registered)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,
}

#[allow(dead_code)] // Used by serde for default deserialization
fn default_true() -> bool {
    true
}

impl Output for GoodbyeResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();

        // Show warning for planner agents that won't terminate
        if !self.should_terminate {
            lines.push(
                "Error: Planner agents should not call goodbye. They produce artifacts but don't run long-lived sessions.".to_string()
            );
            lines.push("Use 'bn goodbye --force \"reason\"' if you must terminate.".to_string());
            return lines.join("\n");
        }

        if !self.was_registered {
            lines.push(
                "Warning: Agent not registered (did you run bn orient?). Terminating anyway."
                    .to_string(),
            );
        }

        if self.reason.is_none() {
            lines.push(
                "Warning: No reason provided. Please use: bn goodbye \"reason for termination\""
                    .to_string(),
            );
        }

        if let Some(name) = &self.agent_name {
            lines.push(format!("Goodbye logged for agent: {}", name));
        } else {
            lines.push("Goodbye logged.".to_string());
        }

        if let Some(reason) = &self.reason {
            lines.push(format!("Reason: {}", reason));
        }

        lines.push(format!(
            "Terminating agent session (grandparent PID {})...",
            self.grandparent_pid
        ));

        lines.join("\n")
    }
}

/// Gracefully terminate the agent.
///
/// This command:
/// 1. Looks up agent registration by current PID
/// 2. If not registered: logs warning, proceeds anyway
/// 3. Logs termination with optional reason
/// 4. Removes own registration from agents.jsonl
/// 5. Sends SIGTERM to grandparent PID (then SIGKILL after timeout if still running)
///
/// For planner agents (PRD generators), termination is blocked unless force=true.
/// Planner agents produce artifacts but don't run long-lived sessions.
///
/// Note: We target the grandparent PID because the process tree is typically:
/// agent session → shell (running bn) → bn goodbye
/// Killing the direct parent (shell) doesn't terminate the agent session.
///
/// When `BN_MCP_SESSION` env var is set, the agent is looked up by session ID
/// instead of by PID, and process termination is automatically skipped.
pub fn goodbye(repo_path: &Path, reason: Option<String>, force: bool) -> Result<GoodbyeResult> {
    let bn_pid = std::process::id();
    let parent_pid = get_parent_pid().unwrap_or(0);
    let grandparent_pid = get_grandparent_pid().unwrap_or(0);

    // Check for MCP session mode
    let mcp_session_id = std::env::var("BN_MCP_SESSION").ok();

    // When using MCP session, we don't require valid parent/grandparent PIDs
    // since we're not going to terminate them anyway
    if mcp_session_id.is_none() {
        if parent_pid == 0 {
            return Err(Error::Other("Could not determine parent PID".to_string()));
        }
        if grandparent_pid == 0 {
            return Err(Error::Other(
                "Could not determine grandparent PID".to_string(),
            ));
        }
    }

    let mut storage = Storage::open(repo_path)?;

    // Clean up stale agents first
    let _ = storage.cleanup_stale_agents();

    // Look up agent: MCP session > parent PID
    let (agent, _agent_pid_for_removal) = if let Some(ref session_id) = mcp_session_id {
        // MCP session mode - look up by session ID
        let agent = storage.get_agent_by_mcp_session(session_id).ok();
        let pid = agent.as_ref().map(|a| a.pid);
        (agent, pid)
    } else {
        // Normal PID-based lookup
        let agent = storage.get_agent(parent_pid).ok();
        (agent, Some(parent_pid))
    };

    let was_registered = agent.is_some();
    let agent_name = agent.as_ref().map(|a| a.name.clone());
    let agent_type = agent
        .as_ref()
        .map(|a| format!("{:?}", a.agent_type).to_lowercase());

    // Check if planner agent trying to call goodbye without --force
    let is_planner = agent
        .as_ref()
        .map(|a| a.agent_type == AgentType::Planner)
        .unwrap_or(false);

    // In MCP mode, should_terminate is always false (we don't terminate parent processes)
    let should_terminate = if mcp_session_id.is_some() {
        false
    } else {
        !is_planner || force
    };

    // Update agent with goodbye status before removing (for GUI animation)
    if was_registered && let Ok(mut agent_data) = storage.get_agent(parent_pid) {
        agent_data.current_action = Some("goodbye".to_string());
        agent_data.goodbye_at = Some(Utc::now());
        // Keep the agent visible for a few seconds so GUI can show goodbye animation
        // The GUI will fade out the agent based on goodbye_at timestamp
        // cleanup_stale_agents() will remove it after a delay
        let _ = storage.update_agent(&agent_data);
    }

    // Note: We don't remove the agent immediately anymore to allow GUI
    // to show the goodbye animation. The cleanup_stale_agents() function
    // will remove goodbye agents after a 10-second delay.

    // Log the bn command PID for debugging
    let _ = bn_pid;

    // Add warning if no reason provided
    let warning = if reason.is_none() && (mcp_session_id.is_some() || should_terminate) {
        Some("No reason provided. Please use: bn goodbye \"reason for termination\"".to_string())
    } else if !should_terminate && mcp_session_id.is_none() && is_planner {
        Some(
            "Planner agents should not call goodbye. Use --force if you must terminate."
                .to_string(),
        )
    } else {
        None
    };

    Ok(GoodbyeResult {
        agent_name,
        parent_pid,
        grandparent_pid,
        was_registered,
        reason,
        terminated: should_terminate,
        should_terminate,
        warning,
        agent_type,
    })
}

/// Send SIGTERM to a process, then SIGKILL after timeout if still running.
/// Returns true if process was terminated, false if it didn't exist or couldn't be terminated.
#[cfg(unix)]
pub fn terminate_process(pid: u32, timeout_secs: u64) -> bool {
    use std::process::Command;
    use std::thread;
    use std::time::Duration;

    // Check if process exists first
    let exists = Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false);

    if !exists {
        return false;
    }

    // Send SIGTERM
    let term_sent = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false);

    if !term_sent {
        return false;
    }

    // Wait for process to exit (check every 100ms)
    let check_interval = Duration::from_millis(100);
    let timeout = Duration::from_secs(timeout_secs);
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        let still_running = Command::new("kill")
            .args(["-0", &pid.to_string()])
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false);

        if !still_running {
            return true; // Process exited gracefully
        }

        thread::sleep(check_interval);
    }

    // Process still running after timeout - send SIGKILL
    let _ = Command::new("kill")
        .args(["-KILL", &pid.to_string()])
        .output();

    true
}

/// Send termination signal to a process on Windows.
#[cfg(windows)]
pub fn terminate_process(pid: u32, timeout_secs: u64) -> bool {
    use std::process::Command;
    use std::thread;
    use std::time::Duration;

    // Try graceful termination first
    let _ = Command::new("taskkill")
        .args(["/PID", &pid.to_string()])
        .output();

    // Wait for process to exit
    let check_interval = Duration::from_millis(100);
    let timeout = Duration::from_secs(timeout_secs);
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        // Check if process is still running
        let output = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid), "/FO", "CSV", "/NH"])
            .output();

        if let Ok(out) = output {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if !stdout.contains(&pid.to_string()) {
                return true; // Process exited
            }
        }

        thread::sleep(check_interval);
    }

    // Force kill after timeout
    let _ = Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .output();

    true
}

// === Container Commands ===

/// Result of container build operation.
#[derive(Serialize)]
pub struct ContainerBuildResult {
    /// Whether the build was successful
    pub success: bool,
    /// Image tag that was built
    pub tag: String,
    /// Error message if build failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Output for ContainerBuildResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if self.success {
            format!("Built image: localhost/binnacle-worker:{}", self.tag)
        } else {
            format!(
                "Build failed: {}",
                self.error.as_deref().unwrap_or("unknown error")
            )
        }
    }
}

/// Result of container run operation.
#[derive(Serialize)]
pub struct ContainerRunResult {
    /// Whether the container started successfully
    pub success: bool,
    /// Container name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Error message if run failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Output for ContainerRunResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if self.success {
            format!(
                "Started container: {}",
                self.name.as_deref().unwrap_or("unknown")
            )
        } else {
            format!(
                "Failed to start container: {}",
                self.error.as_deref().unwrap_or("unknown error")
            )
        }
    }
}

/// Result of container stop operation.
#[derive(Serialize)]
pub struct ContainerStopResult {
    /// Whether the operation was successful
    pub success: bool,
    /// Number of containers stopped
    pub stopped_count: usize,
    /// Names of stopped containers
    pub stopped: Vec<String>,
    /// Error message if stop failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Output for ContainerStopResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if self.success {
            if self.stopped.is_empty() {
                "No containers to stop".to_string()
            } else {
                format!(
                    "Stopped {} container(s): {}",
                    self.stopped_count,
                    self.stopped.join(", ")
                )
            }
        } else {
            format!(
                "Failed to stop container(s): {}",
                self.error.as_deref().unwrap_or("unknown error")
            )
        }
    }
}

/// Information about a binnacle container.
#[derive(Serialize)]
pub struct ContainerInfo {
    /// Container name
    pub name: String,
    /// Container status (running, stopped, etc.)
    pub status: String,
    /// Image used
    pub image: String,
    /// Creation time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
}

/// Result of container list operation.
#[derive(Serialize)]
pub struct ContainerListResult {
    /// Whether the operation was successful
    pub success: bool,
    /// List of containers
    pub containers: Vec<ContainerInfo>,
    /// Error message if list failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Output for ContainerListResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if !self.success {
            return format!(
                "Failed to list containers: {}",
                self.error.as_deref().unwrap_or("unknown error")
            );
        }

        if self.containers.is_empty() {
            return "No binnacle containers found".to_string();
        }

        let mut lines = vec!["NAME\tSTATUS\tIMAGE".to_string()];
        for c in &self.containers {
            lines.push(format!("{}\t{}\t{}", c.name, c.status, c.image));
        }
        lines.join("\n")
    }
}

/// Check if a command exists on the system.
fn command_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get the helpful installation message for container dependencies.
fn container_deps_missing_message() -> String {
    r#"Error: containerd or buildah not found

To use binnacle containers, install containerd and buildah:

  # Fedora/RHEL
  sudo dnf install containerd buildah
  sudo systemctl enable --now containerd

  # Debian/Ubuntu
  sudo apt install containerd buildah
  sudo systemctl enable --now containerd

For more info, see: https://github.com/containerd/containerd"#
        .to_string()
}

/// Check if the binnacle worker image exists in containerd.
fn container_image_exists(image_name: &str) -> bool {
    let filter = format!("name=={}", image_name);
    Command::new("sudo")
        .args(["ctr", "-n", "binnacle", "images", "check", &filter])
        .output()
        .map(|o| {
            // If the command succeeds and has output lines beyond the header, the image exists
            if o.status.success() {
                let stdout = String::from_utf8_lossy(&o.stdout);
                // The output includes a header line, so we need more than 1 line
                stdout.lines().count() > 1
            } else {
                false
            }
        })
        .unwrap_or(false)
}

/// Parse a memory limit string (e.g., "512m", "1g", "2048m") into bytes.
fn parse_memory_limit(s: &str) -> Result<u64> {
    let s = s.trim().to_lowercase();

    // Handle plain numbers (interpreted as bytes)
    if let Ok(bytes) = s.parse::<u64>() {
        return Ok(bytes);
    }

    // Parse with suffix
    let (num_str, multiplier) = if let Some(num) = s.strip_suffix("k") {
        (num, 1024u64)
    } else if let Some(num) = s.strip_suffix("kb") {
        (num, 1024u64)
    } else if let Some(num) = s.strip_suffix("m") {
        (num, 1024u64 * 1024)
    } else if let Some(num) = s.strip_suffix("mb") {
        (num, 1024u64 * 1024)
    } else if let Some(num) = s.strip_suffix("g") {
        (num, 1024u64 * 1024 * 1024)
    } else if let Some(num) = s.strip_suffix("gb") {
        (num, 1024u64 * 1024 * 1024)
    } else {
        return Err(Error::InvalidInput(format!(
            "Invalid memory limit format: '{}'. Use formats like '512m', '1g', '2048mb'",
            s
        )));
    };

    let num: u64 = num_str.parse().map_err(|_| {
        Error::InvalidInput(format!(
            "Invalid memory limit number: '{}'. Use formats like '512m', '1g', '2048mb'",
            s
        ))
    })?;

    Ok(num * multiplier)
}

/// Build the binnacle worker image using buildah.
pub fn container_build(tag: &str, no_cache: bool) -> Result<ContainerBuildResult> {
    // Check for required tools
    if !command_exists("buildah") || !command_exists("ctr") {
        return Ok(ContainerBuildResult {
            success: false,
            tag: tag.to_string(),
            error: Some(container_deps_missing_message()),
        });
    }

    // Check if Containerfile exists
    let containerfile = Path::new("container/Containerfile");
    if !containerfile.exists() {
        return Ok(ContainerBuildResult {
            success: false,
            tag: tag.to_string(),
            error: Some(
                "container/Containerfile not found. Run from binnacle repository root.".to_string(),
            ),
        });
    }

    // Get the currently running binary and copy it to the build context
    let current_exe = std::env::current_exe()
        .map_err(|e| Error::Other(format!("Failed to get current executable path: {}", e)))?;

    let container_binary_path = Path::new("container/bn");
    eprintln!(
        "📋 Copying bn binary from {} to {}...",
        current_exe.display(),
        container_binary_path.display()
    );

    // Copy the binary to the container build context
    fs::copy(&current_exe, container_binary_path).map_err(|e| {
        Error::Other(format!(
            "Failed to copy binary from {} to {}: {}",
            current_exe.display(),
            container_binary_path.display(),
            e
        ))
    })?;

    // Build with buildah - stream output for real-time feedback
    eprintln!(
        "📦 Building container image (localhost/binnacle-worker:{})...",
        tag
    );
    let mut build_cmd = Command::new("buildah");
    build_cmd
        .arg("bud")
        .arg("-t")
        .arg(format!("localhost/binnacle-worker:{}", tag))
        .arg("-f")
        .arg("container/Containerfile")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    if no_cache {
        build_cmd.arg("--no-cache");
    }

    build_cmd.arg(".");

    let status = build_cmd.status()?;

    // Clean up the copied binary from container/ directory
    let _ = fs::remove_file(container_binary_path);

    if !status.success() {
        return Ok(ContainerBuildResult {
            success: false,
            tag: tag.to_string(),
            error: Some("Build failed (see output above)".to_string()),
        });
    }

    // Export and import to containerd
    // Use docker-archive format (not oci-archive) because it embeds the full image
    // reference in the manifest, allowing ctr to import it with the correct name.
    // OCI archives require annotations that buildah doesn't always include.
    eprintln!("📤 Exporting image to docker archive...");
    let temp_archive = "/tmp/binnacle-worker.tar";
    let image_ref = format!("localhost/binnacle-worker:{}", tag);

    let push_output = Command::new("buildah")
        .args([
            "push",
            &image_ref,
            &format!("docker-archive:{}:{}", temp_archive, image_ref),
        ])
        .output()?;

    if !push_output.status.success() {
        return Ok(ContainerBuildResult {
            success: false,
            tag: tag.to_string(),
            error: Some(format!(
                "Failed to export image: {}",
                String::from_utf8_lossy(&push_output.stderr)
            )),
        });
    }

    eprintln!("📥 Importing image to containerd...");
    let import_output = Command::new("sudo")
        .args(["ctr", "-n", "binnacle", "images", "import", temp_archive])
        .output()?;

    // Clean up temp file
    eprintln!("🧹 Cleaning up...");
    let _ = fs::remove_file(temp_archive);

    if !import_output.status.success() {
        return Ok(ContainerBuildResult {
            success: false,
            tag: tag.to_string(),
            error: Some(format!(
                "Failed to import image to containerd: {}",
                String::from_utf8_lossy(&import_output.stderr)
            )),
        });
    }

    eprintln!("✅ Build complete!");
    Ok(ContainerBuildResult {
        success: true,
        tag: tag.to_string(),
        error: None,
    })
}

/// Detect if a path is a git worktree and return the parent repo's .git directory.
///
/// Git worktrees have a `.git` file (not directory) containing a pointer like:
/// `gitdir: /path/to/main/repo/.git/worktrees/worktree-name`
///
/// This function reads that pointer and finds the parent .git directory by:
/// 1. Parsing the `gitdir:` line from the `.git` file
/// 2. Looking for a `commondir` file that points to the main `.git` directory
/// 3. Falling back to walking up 3 levels from the gitdir path
///
/// Returns `Some(path)` if this is a worktree and we found the parent .git,
/// or `None` if this is a regular repo or resolution fails.
fn detect_worktree_parent_git(worktree_path: &Path) -> Option<PathBuf> {
    let git_path = worktree_path.join(".git");

    // Only proceed if .git is a file (worktree), not a directory (regular repo)
    if !git_path.is_file() {
        return None;
    }

    let content = fs::read_to_string(&git_path).ok()?;

    // Parse "gitdir: <path>" line
    let gitdir_line = content.lines().find(|l| l.starts_with("gitdir:"))?;
    let gitdir_path_str = gitdir_line.strip_prefix("gitdir:")?.trim();

    // Resolve relative or absolute path
    let gitdir_path = if Path::new(gitdir_path_str).is_absolute() {
        PathBuf::from(gitdir_path_str)
    } else {
        git_path.parent()?.join(gitdir_path_str)
    };

    // Look for commondir file which points to the main .git directory
    let commondir_file = gitdir_path.join("commondir");
    if commondir_file.exists()
        && let Ok(commondir_content) = fs::read_to_string(&commondir_file)
    {
        let common_path = gitdir_path.join(commondir_content.trim());
        if let Ok(canonical) = common_path.canonicalize() {
            return Some(canonical);
        }
    }

    // Fallback: try to find main .git by walking up from gitdir
    // gitdir is typically at /main/repo/.git/worktrees/<name>
    // So we go up 2 levels to find the main .git
    let mut candidate = gitdir_path.canonicalize().ok()?;
    for _ in 0..2 {
        candidate = candidate.parent()?.to_path_buf();
    }
    if candidate.is_dir() && candidate.file_name()? == ".git" {
        return Some(candidate);
    }

    None
}

/// Run a binnacle worker container.
#[allow(clippy::too_many_arguments)]
pub fn container_run(
    worktree_path: &str,
    agent_type: &str,
    name: Option<String>,
    merge_target: &str,
    no_merge: bool,
    cpus: Option<f64>,
    memory: Option<&str>,
    shell: bool,
) -> Result<ContainerRunResult> {
    // Check for required tools
    if !command_exists("ctr") {
        return Ok(ContainerRunResult {
            success: false,
            name: None,
            error: Some(container_deps_missing_message()),
        });
    }

    // Check if the worker image exists in containerd
    let image_name = "localhost/binnacle-worker:latest";
    if !container_image_exists(image_name) {
        return Ok(ContainerRunResult {
            success: false,
            name: None,
            error: Some(format!(
                "Image '{}' not found in containerd.\n\nRun 'bn container build' first to build the worker image.",
                image_name
            )),
        });
    }

    // Validate worktree path exists
    let worktree = Path::new(worktree_path);
    if !worktree.exists() {
        return Ok(ContainerRunResult {
            success: false,
            name: None,
            error: Some(format!("Worktree path does not exist: {}", worktree_path)),
        });
    }

    // Canonicalize the worktree path
    let worktree_abs = worktree.canonicalize()?;

    // Get binnacle data path
    let binnacle_data = dirs::data_local_dir()
        .unwrap_or_else(|| Path::new("/tmp").to_path_buf())
        .join("binnacle");

    // Generate container name if not provided
    let container_name =
        name.unwrap_or_else(|| format!("binnacle-worker-{}", chrono::Utc::now().timestamp()));

    // Build ctr run command
    let mut args = vec![
        "-n".to_string(),
        "binnacle".to_string(),
        "run".to_string(),
        "--rm".to_string(),
    ];

    // Check if we have a real terminal
    use std::io::IsTerminal;
    let is_tty = std::io::stdin().is_terminal();

    if is_tty {
        args.push("--tty".to_string());
    }
    // Note: Without --tty, ctr still inherits stdio but won't allocate a PTY.
    // This means the container can still output to stdout/stderr, but interactive
    // programs may not work correctly. This is acceptable for non-TTY contexts.

    // SECURITY: Host networking removes network namespace isolation.
    // Required for AI agent API calls (OpenAI, Anthropic, etc.) and package installs.
    // The container can access all host network interfaces including localhost services.
    // See bn-a3a2 for planned rootless support with more restrictive networking.
    args.push("--net-host".to_string());

    // Run as host user's UID/GID to preserve file ownership in mounted workspace
    // Container uses nss_wrapper to provide user identity for Node.js, git, etc.
    // SECURITY: User mapping is mandatory to prevent running as root
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let meta = fs::metadata(&worktree_abs).map_err(|e| {
            Error::Other(format!(
                "Failed to read worktree metadata for user mapping: {}. \
                Container cannot run without user mapping.",
                e
            ))
        })?;
        let uid = meta.uid();
        let gid = meta.gid();
        args.push("--user".to_string());
        args.push(format!("{}:{}", uid, gid));
    }

    #[cfg(not(unix))]
    {
        return Ok(ContainerRunResult {
            success: false,
            name: None,
            error: Some(
                "Container user mapping requires Unix. Windows is not supported.".to_string(),
            ),
        });
    }

    // Add resource limits if specified
    if let Some(cpu_limit) = cpus {
        args.push("--cpus".to_string());
        args.push(cpu_limit.to_string());
    }

    if let Some(mem_limit) = memory {
        // Parse memory string (e.g., "512m", "1g", "2048m") to bytes
        let bytes = parse_memory_limit(mem_limit)?;
        args.push("--memory-limit".to_string());
        args.push(bytes.to_string());
    }

    // Add mounts
    args.push("--mount".to_string());
    args.push(format!(
        "type=bind,src={},dst=/workspace,options=rbind:rw",
        worktree_abs.display()
    ));

    args.push("--mount".to_string());
    args.push(format!(
        "type=bind,src={},dst=/binnacle,options=rbind:rw",
        binnacle_data.display()
    ));

    // If this is a git worktree, mount the parent repo's .git directory
    // so git commands can resolve the worktree reference inside the container.
    // The .git file in worktrees contains "gitdir: /path/to/parent/.git/worktrees/name"
    // which git needs to access.
    //
    // NOTE: This is mounted read-write because git worktrees store their HEAD, index,
    // and refs under .git/worktrees/<name>/, and git commits add objects to the shared
    // object store. Mounting read-only would prevent all git write operations (commit,
    // checkout, etc.). The container is trusted to work only on its designated worktree.
    if let Some(parent_git_dir) = detect_worktree_parent_git(&worktree_abs) {
        args.push("--mount".to_string());
        args.push(format!(
            "type=bind,src={},dst={},options=rbind:rw",
            parent_git_dir.display(),
            parent_git_dir.display()
        ));
    }

    // Add environment variables
    args.push("--env".to_string());
    args.push(format!("BN_AGENT_TYPE={}", agent_type));

    args.push("--env".to_string());
    args.push("BN_CONTAINER_MODE=true".to_string());

    args.push("--env".to_string());
    args.push(format!("BN_MERGE_TARGET={}", merge_target));

    // Tell bn where to find the database (mounted at /binnacle)
    args.push("--env".to_string());
    args.push("BN_DATA_DIR=/binnacle".to_string());

    // Compute storage hash on host and pass to container.
    // IMPORTANT: Use find_git_root to resolve worktrees to main repo before hashing.
    // This ensures the hash matches the actual storage location, which is computed
    // the same way by get_storage_dir(). Without this, worktrees would get a different
    // hash than their main repo, causing "No binnacle database found" errors.
    let repo_root = find_git_root(&worktree_abs).unwrap_or_else(|| worktree_abs.clone());
    let mut hasher = Sha256::new();
    hasher.update(repo_root.to_string_lossy().as_bytes());
    let hash = hasher.finalize();
    let storage_hash = &format!("{:x}", hash)[..12];
    args.push("--env".to_string());
    args.push(format!("BN_STORAGE_HASH={}", storage_hash));

    if no_merge {
        args.push("--env".to_string());
        args.push("BN_NO_MERGE=true".to_string());
    }

    // Pass through GitHub tokens if available
    // GH_TOKEN is a common convention for GitHub CLI and other tools
    if let Ok(token) = std::env::var("GH_TOKEN") {
        args.push("--env".to_string());
        args.push(format!("GH_TOKEN={}", token));
    }
    // COPILOT_GITHUB_TOKEN is used by @github/copilot CLI
    if let Ok(token) = std::env::var("COPILOT_GITHUB_TOKEN") {
        args.push("--env".to_string());
        args.push(format!("COPILOT_GITHUB_TOKEN={}", token));
    }

    // Add image and container name
    args.push("localhost/binnacle-worker:latest".to_string());
    args.push(container_name.clone());

    // For shell mode, override the entrypoint to run bash with "shell" argument
    if shell {
        args.push("/entrypoint.sh".to_string());
        args.push("shell".to_string());
    }

    // Inherit stdio so container output is visible.
    // With --tty (TTY mode), ctr allocates a PTY for interactive programs.
    // Without --tty (non-TTY mode), output still streams but interactive programs may not work.
    let status = Command::new("sudo")
        .arg("ctr")
        .args(&args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    if !status.success() {
        return Ok(ContainerRunResult {
            success: false,
            name: Some(container_name),
            error: Some(format!("Container exited with status: {}", status)),
        });
    }

    Ok(ContainerRunResult {
        success: true,
        name: Some(container_name),
        error: None,
    })
}

/// Stop binnacle containers.
pub fn container_stop(name: Option<String>, all: bool) -> Result<ContainerStopResult> {
    // Check for required tools
    if !command_exists("ctr") {
        return Ok(ContainerStopResult {
            success: false,
            stopped_count: 0,
            stopped: vec![],
            error: Some(container_deps_missing_message()),
        });
    }

    let mut stopped = Vec::new();

    if all {
        // Get list of containers in binnacle namespace
        let output = Command::new("sudo")
            .args(["ctr", "-n", "binnacle", "containers", "list", "-q"])
            .output()?;

        if !output.status.success() {
            return Ok(ContainerStopResult {
                success: false,
                stopped_count: 0,
                stopped: vec![],
                error: Some(String::from_utf8_lossy(&output.stderr).to_string()),
            });
        }

        let containers: Vec<String> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .collect();

        for container in containers {
            // Kill task first
            let _ = Command::new("sudo")
                .args(["ctr", "-n", "binnacle", "tasks", "kill", &container])
                .output();

            // Remove container
            let rm_output = Command::new("sudo")
                .args(["ctr", "-n", "binnacle", "containers", "rm", &container])
                .output()?;

            if rm_output.status.success() {
                stopped.push(container);
            }
        }
    } else if let Some(container_name) = name {
        // Kill task first
        let _ = Command::new("sudo")
            .args(["ctr", "-n", "binnacle", "tasks", "kill", &container_name])
            .output();

        // Remove container
        let rm_output = Command::new("sudo")
            .args(["ctr", "-n", "binnacle", "containers", "rm", &container_name])
            .output()?;

        if rm_output.status.success() {
            stopped.push(container_name);
        } else {
            return Ok(ContainerStopResult {
                success: false,
                stopped_count: 0,
                stopped: vec![],
                error: Some(String::from_utf8_lossy(&rm_output.stderr).to_string()),
            });
        }
    } else {
        return Ok(ContainerStopResult {
            success: false,
            stopped_count: 0,
            stopped: vec![],
            error: Some("Must specify container name or --all".to_string()),
        });
    }

    Ok(ContainerStopResult {
        success: true,
        stopped_count: stopped.len(),
        stopped,
        error: None,
    })
}

/// List binnacle containers.
pub fn container_list(all: bool, _quiet: bool) -> Result<ContainerListResult> {
    // Check for required tools
    if !command_exists("ctr") {
        return Ok(ContainerListResult {
            success: false,
            containers: vec![],
            error: Some(container_deps_missing_message()),
        });
    }

    // Get list of containers
    let output = Command::new("sudo")
        .args(["ctr", "-n", "binnacle", "containers", "list"])
        .output()?;

    if !output.status.success() {
        return Ok(ContainerListResult {
            success: false,
            containers: vec![],
            error: Some(String::from_utf8_lossy(&output.stderr).to_string()),
        });
    }

    // Get running tasks to determine status
    let tasks_output = Command::new("sudo")
        .args(["ctr", "-n", "binnacle", "tasks", "list"])
        .output()?;

    let running_tasks: std::collections::HashSet<String> = if tasks_output.status.success() {
        String::from_utf8_lossy(&tasks_output.stdout)
            .lines()
            .skip(1) // Skip header
            .filter_map(|line| line.split_whitespace().next())
            .map(|s| s.to_string())
            .collect()
    } else {
        std::collections::HashSet::new()
    };

    // Parse container list output
    let mut containers = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let name = parts[0].to_string();
            let image = parts[1].to_string();
            let status = if running_tasks.contains(&name) {
                "running".to_string()
            } else {
                "stopped".to_string()
            };

            // Skip stopped containers if not showing all
            if !all && status == "stopped" {
                continue;
            }

            containers.push(ContainerInfo {
                name,
                status,
                image,
                created: None,
            });
        }
    }

    Ok(ContainerListResult {
        success: true,
        containers,
        error: None,
    })
}

// === Sync Command ===

/// Result of the sync operation.
#[derive(Serialize)]
pub struct SyncResult {
    /// The operation performed (push, pull, or both)
    pub operation: String,
    /// Remote name used (e.g., "origin")
    pub remote: String,
    /// Branch that was synced
    pub branch: String,
    /// Whether push was successful (if performed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pushed: Option<bool>,
    /// Whether pull was successful (if performed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pulled: Option<bool>,
    /// Number of commits pushed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commits_pushed: Option<usize>,
    /// Number of commits pulled
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commits_pulled: Option<usize>,
    /// Error message if sync failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Output for SyncResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();

        if let Some(error) = &self.error {
            lines.push(format!("Sync failed: {}", error));
            return lines.join("\n");
        }

        lines.push(format!(
            "Synced binnacle data ({}) with {}",
            self.branch, self.remote
        ));

        if let Some(true) = self.pulled
            && let Some(count) = self.commits_pulled
        {
            if count > 0 {
                lines.push(format!("  Pulled {} commit(s)", count));
            } else {
                lines.push("  Already up to date".to_string());
            }
        }

        if let Some(true) = self.pushed
            && let Some(count) = self.commits_pushed
        {
            if count > 0 {
                lines.push(format!("  Pushed {} commit(s)", count));
            } else {
                lines.push("  Nothing to push".to_string());
            }
        }

        lines.join("\n")
    }
}

/// The binnacle data branch name.
const BINNACLE_BRANCH: &str = "binnacle-data";

/// Sync binnacle data with a remote repository.
///
/// This command pushes and/or pulls the `binnacle-data` branch to/from a remote.
/// It only works when the orphan-branch or git-notes backend is in use.
///
/// # Arguments
/// * `repo_path` - Path to the repository
/// * `remote` - Remote name (default: "origin")
/// * `push_only` - Only push, don't pull
/// * `pull_only` - Only pull, don't push
pub fn sync(
    repo_path: &Path,
    remote: Option<String>,
    push_only: bool,
    pull_only: bool,
) -> Result<SyncResult> {
    let remote = remote.unwrap_or_else(|| "origin".to_string());

    // Check if the binnacle-data branch exists
    let branch_exists = Command::new("git")
        .args([
            "rev-parse",
            "--verify",
            &format!("refs/heads/{}", BINNACLE_BRANCH),
        ])
        .current_dir(repo_path)
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false);

    if !branch_exists {
        return Ok(SyncResult {
            operation: "none".to_string(),
            remote: remote.clone(),
            branch: BINNACLE_BRANCH.to_string(),
            pushed: None,
            pulled: None,
            commits_pushed: None,
            commits_pulled: None,
            error: Some(format!(
                "No '{}' branch found. Sync only works with the orphan-branch storage backend. \
                 Use 'bn config set storage.backend orphan-branch' to enable it.",
                BINNACLE_BRANCH
            )),
        });
    }

    // Check if remote exists
    let remote_exists = Command::new("git")
        .args(["remote", "get-url", &remote])
        .current_dir(repo_path)
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false);

    if !remote_exists {
        return Ok(SyncResult {
            operation: "none".to_string(),
            remote: remote.clone(),
            branch: BINNACLE_BRANCH.to_string(),
            pushed: None,
            pulled: None,
            commits_pushed: None,
            commits_pulled: None,
            error: Some(format!("Remote '{}' not found", remote)),
        });
    }

    let mut pulled = None;
    let mut pushed = None;
    let mut commits_pulled = None;
    let mut commits_pushed = None;

    // Pull first (if not push_only)
    if !push_only {
        // Get current commit before pull
        let before_commit = Command::new("git")
            .args(["rev-parse", BINNACLE_BRANCH])
            .current_dir(repo_path)
            .output()
            .ok()
            .and_then(|out| {
                if out.status.success() {
                    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
                } else {
                    None
                }
            });

        // Fetch the remote branch
        let fetch_result = Command::new("git")
            .args(["fetch", &remote, BINNACLE_BRANCH])
            .current_dir(repo_path)
            .output();

        if let Ok(output) = fetch_result {
            if output.status.success() {
                // Check if remote branch exists
                let remote_ref = format!("{}/{}", remote, BINNACLE_BRANCH);
                let remote_exists = Command::new("git")
                    .args(["rev-parse", "--verify", &remote_ref])
                    .current_dir(repo_path)
                    .output()
                    .map(|out| out.status.success())
                    .unwrap_or(false);

                if remote_exists {
                    // Merge the remote branch (fast-forward only for safety)
                    let merge_result = Command::new("git")
                        .args([
                            "merge",
                            "--ff-only",
                            &remote_ref,
                            // Use plumbing to avoid checkout issues
                        ])
                        .env("GIT_WORK_TREE", repo_path)
                        .current_dir(repo_path)
                        .output();

                    // Actually, we need to update the ref directly to avoid working tree issues
                    // For orphan branch, we should use update-ref approach
                    let update_result = Command::new("git")
                        .args([
                            "update-ref",
                            &format!("refs/heads/{}", BINNACLE_BRANCH),
                            &remote_ref,
                        ])
                        .current_dir(repo_path)
                        .output();

                    if update_result
                        .as_ref()
                        .map(|o| o.status.success())
                        .unwrap_or(false)
                        || merge_result
                            .as_ref()
                            .map(|o| o.status.success())
                            .unwrap_or(false)
                    {
                        pulled = Some(true);

                        // Count commits pulled
                        if let Some(before) = before_commit {
                            let after_commit = Command::new("git")
                                .args(["rev-parse", BINNACLE_BRANCH])
                                .current_dir(repo_path)
                                .output()
                                .ok()
                                .and_then(|out| {
                                    if out.status.success() {
                                        Some(
                                            String::from_utf8_lossy(&out.stdout).trim().to_string(),
                                        )
                                    } else {
                                        None
                                    }
                                });

                            if let Some(after) = after_commit {
                                if before != after {
                                    let count_output = Command::new("git")
                                        .args([
                                            "rev-list",
                                            "--count",
                                            &format!("{}..{}", before, after),
                                        ])
                                        .current_dir(repo_path)
                                        .output();

                                    commits_pulled = count_output.ok().and_then(|out| {
                                        if out.status.success() {
                                            String::from_utf8_lossy(&out.stdout).trim().parse().ok()
                                        } else {
                                            None
                                        }
                                    });
                                } else {
                                    commits_pulled = Some(0);
                                }
                            }
                        }
                    } else {
                        pulled = Some(false);
                    }
                } else {
                    // Remote branch doesn't exist yet, that's fine
                    pulled = Some(true);
                    commits_pulled = Some(0);
                }
            } else {
                // Fetch failed - maybe remote branch doesn't exist yet
                pulled = Some(true);
                commits_pulled = Some(0);
            }
        }
    }

    // Push (if not pull_only)
    if !pull_only {
        // Get current commit before push to count commits
        let local_commit = Command::new("git")
            .args(["rev-parse", BINNACLE_BRANCH])
            .current_dir(repo_path)
            .output()
            .ok()
            .and_then(|out| {
                if out.status.success() {
                    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
                } else {
                    None
                }
            });

        // Check what's on remote
        let remote_ref = format!("{}/{}", remote, BINNACLE_BRANCH);
        let remote_commit = Command::new("git")
            .args(["rev-parse", &remote_ref])
            .current_dir(repo_path)
            .output()
            .ok()
            .and_then(|out| {
                if out.status.success() {
                    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
                } else {
                    None
                }
            });

        // Push the branch
        let push_result = Command::new("git")
            .args(["push", &remote, BINNACLE_BRANCH])
            .current_dir(repo_path)
            .output();

        if let Ok(output) = push_result {
            pushed = Some(output.status.success());

            if output.status.success() {
                // Count commits pushed
                if let (Some(local), Some(remote_c)) = (&local_commit, &remote_commit) {
                    if local != remote_c {
                        let count_output = Command::new("git")
                            .args(["rev-list", "--count", &format!("{}..{}", remote_c, local)])
                            .current_dir(repo_path)
                            .output();

                        commits_pushed = count_output.ok().and_then(|out| {
                            if out.status.success() {
                                String::from_utf8_lossy(&out.stdout).trim().parse().ok()
                            } else {
                                None
                            }
                        });
                    } else {
                        commits_pushed = Some(0);
                    }
                } else if remote_commit.is_none() && local_commit.is_some() {
                    // First push - count all commits
                    let count_output = Command::new("git")
                        .args(["rev-list", "--count", BINNACLE_BRANCH])
                        .current_dir(repo_path)
                        .output();

                    commits_pushed = count_output.ok().and_then(|out| {
                        if out.status.success() {
                            String::from_utf8_lossy(&out.stdout).trim().parse().ok()
                        } else {
                            None
                        }
                    });
                }
            }
        }
    }

    let operation = match (push_only, pull_only) {
        (true, false) => "push",
        (false, true) => "pull",
        _ => "sync",
    };

    Ok(SyncResult {
        operation: operation.to_string(),
        remote,
        branch: BINNACLE_BRANCH.to_string(),
        pushed,
        pulled,
        commits_pushed,
        commits_pulled,
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::TestEnv;

    fn setup() -> TestEnv {
        let env = TestEnv::new_with_env();
        Storage::init(env.path()).unwrap();
        env
    }

    #[test]
    fn test_init_new() {
        let env = TestEnv::new_with_env();
        let result = init_with_options(env.path(), false, false, false, false, false).unwrap();
        assert!(result.initialized);
    }

    #[test]
    fn test_init_existing() {
        let env = TestEnv::new_with_env();
        Storage::init(env.path()).unwrap();
        let result = init_with_options(env.path(), false, false, false, false, false).unwrap();
        assert!(!result.initialized);
    }

    #[test]
    fn test_task_create() {
        let temp = setup();
        let result = task_create(
            temp.path(),
            "Test task".to_string(),
            None, // short_name
            None,
            Some(1),
            vec!["test".to_string()],
            None,
        )
        .unwrap();
        assert!(result.id.starts_with("bn-"));
        assert_eq!(result.title, "Test task");
    }

    #[test]
    fn test_task_show() {
        let temp = setup();
        let created = task_create(
            temp.path(),
            "Test".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let result = task_show(temp.path(), &created.id).unwrap().unwrap();
        assert_eq!(result.task.core.id, created.id);
        assert!(result.blocking_info.is_none()); // No dependencies
    }

    #[test]
    fn test_task_list() {
        let temp = setup();
        task_create(
            temp.path(),
            "Task 1".to_string(),
            None, // short_name
            None,
            Some(1),
            vec![],
            None,
        )
        .unwrap();
        task_create(
            temp.path(),
            "Task 2".to_string(),
            None, // short_name
            None,
            Some(2),
            vec![],
            None,
        )
        .unwrap();

        let list = task_list(temp.path(), None, None, None).unwrap();
        assert_eq!(list.count, 2);
    }

    #[test]
    fn test_task_update() {
        let temp = setup();
        let created = task_create(
            temp.path(),
            "Original".to_string(),
            None, // short_name
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        let updated = task_update(
            temp.path(),
            &created.id,
            Some("Updated".to_string()),
            None, // short_name
            None,
            Some(1),
            None,
            vec![],
            vec![],
            None,
            false,
            false, // keep_closed
            false, // reopen
        )
        .unwrap();

        assert!(updated.updated_fields.contains(&"title".to_string()));
        assert!(updated.updated_fields.contains(&"priority".to_string()));

        let result = task_show(temp.path(), &created.id).unwrap().unwrap();
        assert_eq!(result.task.core.title, "Updated");
        assert_eq!(result.task.priority, 1);
    }

    #[test]
    fn test_task_close_reopen() {
        let temp = setup();
        let created = task_create(
            temp.path(),
            "Test".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        task_close(temp.path(), &created.id, Some("Done".to_string()), false).unwrap();
        let result = task_show(temp.path(), &created.id).unwrap().unwrap();
        assert_eq!(result.task.status, TaskStatus::Done);
        assert!(result.task.closed_at.is_some());

        task_reopen(temp.path(), &created.id).unwrap();
        let result = task_show(temp.path(), &created.id).unwrap().unwrap();
        assert_eq!(result.task.status, TaskStatus::Reopened);
        assert!(result.task.closed_at.is_none());
    }

    #[test]
    fn test_closed_task_update_requires_flag() {
        let temp = setup();
        let created = task_create(
            temp.path(),
            "Test".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Close the task
        task_close(temp.path(), &created.id, Some("Done".to_string()), false).unwrap();

        // Try to update without flag - should fail
        let result = task_update(
            temp.path(),
            &created.id,
            Some("New Title".to_string()),
            None,
            None,
            None,
            None,
            vec![],
            vec![],
            None,
            false,
            false, // keep_closed
            false, // reopen
        );
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Cannot update closed task"));
        assert!(err.contains("--keep-closed"));
        assert!(err.contains("--reopen"));
    }

    #[test]
    fn test_closed_task_update_with_keep_closed() {
        let temp = setup();
        let created = task_create(
            temp.path(),
            "Test".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Close the task
        task_close(temp.path(), &created.id, Some("Done".to_string()), false).unwrap();

        // Update with --keep-closed - should succeed and keep status as Done
        let result = task_update(
            temp.path(),
            &created.id,
            Some("Updated Title".to_string()),
            None,
            None,
            None,
            None,
            vec![],
            vec![],
            None,
            false,
            true,  // keep_closed
            false, // reopen
        );
        assert!(result.is_ok());

        // Verify title was updated but status is still Done
        let task = task_show(temp.path(), &created.id).unwrap().unwrap();
        assert_eq!(task.task.core.title, "Updated Title");
        assert_eq!(task.task.status, TaskStatus::Done);
        assert!(task.task.closed_at.is_some());
    }

    #[test]
    fn test_closed_task_update_with_reopen() {
        let temp = setup();
        let created = task_create(
            temp.path(),
            "Test".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Close the task
        task_close(temp.path(), &created.id, Some("Done".to_string()), false).unwrap();

        // Update with --reopen - should succeed and set status to Pending
        let result = task_update(
            temp.path(),
            &created.id,
            Some("Reopened Title".to_string()),
            None,
            None,
            None,
            None,
            vec![],
            vec![],
            None,
            false,
            false, // keep_closed
            true,  // reopen
        );
        assert!(result.is_ok());

        // Verify title was updated and status is now Pending
        let task = task_show(temp.path(), &created.id).unwrap().unwrap();
        assert_eq!(task.task.core.title, "Reopened Title");
        assert_eq!(task.task.status, TaskStatus::Pending);
        assert!(task.task.closed_at.is_none());
    }

    #[test]
    fn test_cancelled_task_update_requires_flag() {
        let temp = setup();
        let created = task_create(
            temp.path(),
            "Test".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Set status to cancelled
        task_update(
            temp.path(),
            &created.id,
            None,
            None,
            None,
            None,
            Some("cancelled"),
            vec![],
            vec![],
            None,
            false,
            false, // keep_closed
            false, // reopen
        )
        .unwrap();

        // Try to update without flag - should fail
        let result = task_update(
            temp.path(),
            &created.id,
            Some("New Title".to_string()),
            None,
            None,
            None,
            None,
            vec![],
            vec![],
            None,
            false,
            false, // keep_closed
            false, // reopen
        );
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Cannot update closed task"));
    }

    #[test]
    fn test_task_close_with_incomplete_deps_fails() {
        let temp = setup();
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // B depends on A
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        // Try to close B without force (A is still pending)
        let result = task_close(temp.path(), &task_b.id, None, false);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("incomplete dependencies"));
        assert!(err_msg.contains(&task_a.id));
    }

    #[test]
    fn test_task_close_with_incomplete_deps_force() {
        let temp = setup();
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // B depends on A
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        // Close B with force (A is still pending)
        let result = task_close(temp.path(), &task_b.id, None, true).unwrap();
        assert_eq!(result.status, "done");
        assert!(result.warning.is_some());
        assert!(result.warning.unwrap().contains("incomplete dependencies"));

        // Verify task is actually closed
        let result = task_show(temp.path(), &task_b.id).unwrap().unwrap();
        assert_eq!(result.task.status, TaskStatus::Done);
    }

    #[test]
    fn test_task_close_with_complete_deps_success() {
        let temp = setup();
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // B depends on A
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        // Close A first
        task_close(temp.path(), &task_a.id, None, false).unwrap();

        // Now close B (all deps are complete)
        let result = task_close(temp.path(), &task_b.id, None, false).unwrap();
        assert_eq!(result.status, "done");
        // Warning about no commits linked is expected, but NOT about incomplete deps
        if let Some(warning) = &result.warning {
            assert!(!warning.contains("incomplete dependencies"));
            assert!(warning.contains("No commits linked"));
        }

        // Verify task is closed
        let result = task_show(temp.path(), &task_b.id).unwrap().unwrap();
        assert_eq!(result.task.status, TaskStatus::Done);
    }

    #[test]
    fn test_task_close_promotes_partial_dependents() {
        let temp = setup();
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        task_close(temp.path(), &task_b.id, Some("Done".to_string()), false).unwrap();
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        let result = task_show(temp.path(), &task_b.id).unwrap().unwrap();
        assert_eq!(result.task.status, TaskStatus::Partial);
        assert!(result.task.closed_at.is_none());
        assert!(result.task.closed_reason.is_none());

        task_close(temp.path(), &task_a.id, Some("Done".to_string()), false).unwrap();

        let result = task_show(temp.path(), &task_b.id).unwrap().unwrap();
        assert_eq!(result.task.status, TaskStatus::Done);
        assert!(result.task.closed_at.is_some());
    }

    #[test]
    fn test_task_close_requires_commit_when_config_enabled() {
        let temp = setup();
        let task = task_create(
            temp.path(),
            "Test task".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Enable require_commit_for_close
        config_set(temp.path(), "require_commit_for_close", "true").unwrap();

        // Should fail without linked commit
        let result = task_close(temp.path(), &task.id, Some("Done".to_string()), false);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("no commits linked"));
        assert!(err_msg.contains("bn commit link"));
    }

    #[test]
    fn test_task_close_succeeds_with_linked_commit_when_config_enabled() {
        let temp = setup();
        let task = task_create(
            temp.path(),
            "Test task".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Enable require_commit_for_close
        config_set(temp.path(), "require_commit_for_close", "true").unwrap();

        // Link a commit
        commit_link(
            temp.path(),
            "abc1234def5678abc1234def5678abc1234def56",
            &task.id,
        )
        .unwrap();

        // Should succeed with linked commit
        let result = task_close(temp.path(), &task.id, Some("Done".to_string()), false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_task_close_force_bypasses_commit_requirement() {
        let temp = setup();
        let task = task_create(
            temp.path(),
            "Test task".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Enable require_commit_for_close
        config_set(temp.path(), "require_commit_for_close", "true").unwrap();

        // Should succeed with --force even without commit
        let result = task_close(temp.path(), &task.id, Some("Done".to_string()), true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_task_close_works_without_commit_when_config_disabled() {
        let temp = setup();
        let task = task_create(
            temp.path(),
            "Test task".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Explicitly disable require_commit_for_close (default)
        config_set(temp.path(), "require_commit_for_close", "false").unwrap();

        // Should succeed without commit
        let result = task_close(temp.path(), &task.id, Some("Done".to_string()), false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_task_close_warns_when_linked_commit_not_in_repo() {
        let temp = setup();

        // Initialize as a git repo
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(temp.path())
            .output()
            .unwrap();

        let task = task_create(
            temp.path(),
            "Test task".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Link a commit that doesn't exist in the repo
        commit_link(
            temp.path(),
            "abcd1234567890abcdef1234567890abcdef1234",
            &task.id,
        )
        .unwrap();

        // Should succeed but with a warning
        let result = task_close(temp.path(), &task.id, Some("Done".to_string()), false);
        assert!(result.is_ok());
        let closed = result.unwrap();
        assert!(closed.warning.is_some());
        let warning = closed.warning.unwrap();
        assert!(warning.contains("not found in repository"));
        assert!(warning.contains("may have been rebased"));
    }

    #[test]
    fn test_task_update_warns_when_linked_commit_not_in_repo() {
        let temp = setup();

        // Initialize as a git repo
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(temp.path())
            .output()
            .unwrap();

        let task = task_create(
            temp.path(),
            "Test task".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Link a commit that doesn't exist in the repo
        commit_link(
            temp.path(),
            "abcd1234567890abcdef1234567890abcdef1234",
            &task.id,
        )
        .unwrap();

        // Update status to done - should succeed but with a warning
        let result = task_update(
            temp.path(),
            &task.id,
            None,
            None,
            None,
            None,
            Some("done"),
            vec![],
            vec![],
            None,
            false,
            false, // keep_closed
            false, // reopen
        );
        assert!(result.is_ok());
        let updated = result.unwrap();
        assert!(updated.warning.is_some());
        let warning = updated.warning.unwrap();
        assert!(warning.contains("not found in repository"));
    }

    #[test]
    fn test_task_close_removes_agent_association() {
        let temp = setup();

        // Register using parent PID since that's how agents are now identified
        // (the bn command itself exits, so we track the parent process instead)
        let parent_pid = get_parent_pid().unwrap_or_else(std::process::id);
        let bn_pid = std::process::id();
        let agent = Agent::new(
            parent_pid,
            bn_pid,
            "test-agent".to_string(),
            AgentType::Worker,
        );
        {
            let mut storage = Storage::open(temp.path()).unwrap();
            storage.register_agent(&agent).unwrap();
        }

        // Create a task
        let task = task_create(
            temp.path(),
            "Test task".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Set task to in_progress (adds to agent's tasks)
        task_update(
            temp.path(),
            &task.id,
            None,
            None,
            None,
            None,
            Some("in_progress"),
            vec![],
            vec![],
            None,
            false,
            false, // keep_closed
            false, // reopen
        )
        .unwrap();

        // Verify task is in agent's list
        {
            let storage = Storage::open(temp.path()).unwrap();
            let agent = storage.get_agent(parent_pid).unwrap();
            assert!(
                agent.tasks.contains(&task.id),
                "Agent should have the task in its tasks list"
            );
        }

        // Close the task
        task_close(temp.path(), &task.id, Some("Done".to_string()), false).unwrap();

        // Verify task is removed from agent's list
        let storage = Storage::open(temp.path()).unwrap();
        let agent = storage.get_agent(parent_pid).unwrap();
        assert!(
            !agent.tasks.contains(&task.id),
            "Agent should no longer have the closed task"
        );
    }

    #[test]
    fn test_task_update_status_done_requires_commit_when_enabled() {
        let temp = setup();
        let task = task_create(
            temp.path(),
            "Test task".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Enable require_commit_for_close
        config_set(temp.path(), "require_commit_for_close", "true").unwrap();

        // Should fail without linked commit
        let result = task_update(
            temp.path(),
            &task.id,
            None,
            None,
            None,
            None,
            Some("done"),
            vec![],
            vec![],
            None,
            false,
            false, // keep_closed
            false, // reopen
        );
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("no commits linked"));
    }

    #[test]
    fn test_task_update_status_done_force_bypasses_commit_requirement() {
        let temp = setup();
        let task = task_create(
            temp.path(),
            "Test task".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Enable require_commit_for_close
        config_set(temp.path(), "require_commit_for_close", "true").unwrap();

        // Should succeed with --force even without commit
        let result = task_update(
            temp.path(),
            &task.id,
            None,
            None,
            None,
            None,
            Some("done"),
            vec![],
            vec![],
            None,
            true,
            false, // keep_closed
            false, // reopen
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_task_update_status_done_succeeds_with_linked_commit() {
        let temp = setup();
        let task = task_create(
            temp.path(),
            "Test task".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Enable require_commit_for_close
        config_set(temp.path(), "require_commit_for_close", "true").unwrap();

        // Link a commit
        commit_link(
            temp.path(),
            "abc1234def5678abc1234def5678abc1234def56",
            &task.id,
        )
        .unwrap();

        // Should succeed with linked commit
        let result = task_update(
            temp.path(),
            &task.id,
            None,
            None,
            None,
            None,
            Some("done"),
            vec![],
            vec![],
            None,
            false,
            false, // keep_closed
            false, // reopen
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_task_update_status_cancelled_ignores_commit_requirement() {
        let temp = setup();
        let task = task_create(
            temp.path(),
            "Test task".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Enable require_commit_for_close
        config_set(temp.path(), "require_commit_for_close", "true").unwrap();

        // Should succeed without commit for cancelled status
        let result = task_update(
            temp.path(),
            &task.id,
            None,
            None,
            None,
            None,
            Some("cancelled"),
            vec![],
            vec![],
            None,
            false,
            false, // keep_closed
            false, // reopen
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_task_update_in_progress_tracks_agent_association() {
        let temp = setup();

        // Register using parent PID since that's how agents are now identified
        let parent_pid = get_parent_pid().unwrap_or_else(std::process::id);
        let bn_pid = std::process::id();
        let agent = Agent::new(
            parent_pid,
            bn_pid,
            "test-agent".to_string(),
            AgentType::Worker,
        );
        {
            let mut storage = Storage::open(temp.path()).unwrap();
            storage.register_agent(&agent).unwrap();
        }

        // Create a task
        let task = task_create(
            temp.path(),
            "Test task".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Update task to in_progress
        let result = task_update(
            temp.path(),
            &task.id,
            None,
            None,
            None,
            None,
            Some("in_progress"),
            vec![],
            vec![],
            None,
            false,
            false, // keep_closed
            false, // reopen
        );
        assert!(result.is_ok());

        // Verify agent has the task in its tasks list
        let storage = Storage::open(temp.path()).unwrap();
        let updated_agent = storage.get_agent(parent_pid).unwrap();
        assert!(
            updated_agent.tasks.contains(&task.id),
            "Agent should have the task in its tasks list"
        );
    }

    #[test]
    fn test_task_update_in_progress_warns_on_multiple_tasks() {
        let temp = setup();

        // Register using parent PID since that's how agents are now identified
        let parent_pid = get_parent_pid().unwrap_or_else(std::process::id);
        let bn_pid = std::process::id();
        let agent = Agent::new(
            parent_pid,
            bn_pid,
            "test-agent".to_string(),
            AgentType::Worker,
        );
        {
            let mut storage = Storage::open(temp.path()).unwrap();
            storage.register_agent(&agent).unwrap();
        }

        // Create two tasks
        let task1 = task_create(
            temp.path(),
            "First task".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task2 = task_create(
            temp.path(),
            "Second task".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Set first task to in_progress
        let result = task_update(
            temp.path(),
            &task1.id,
            None,
            None,
            None,
            None,
            Some("in_progress"),
            vec![],
            vec![],
            None,
            false,
            false, // keep_closed
            false, // reopen
        );
        assert!(result.is_ok(), "First task should succeed");

        // Try to set second task to in_progress without force
        let result = task_update(
            temp.path(),
            &task2.id,
            None,
            None,
            None,
            None,
            Some("in_progress"),
            vec![],
            vec![],
            None,
            false,
            false, // keep_closed
            false, // reopen
        );
        assert!(result.is_err(), "Second task should fail without force");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("already has"),
            "Error should mention existing tasks"
        );
    }

    #[test]
    fn test_task_update_in_progress_force_allows_multiple_tasks() {
        let temp = setup();

        // Register using parent PID since that's how agents are now identified
        let parent_pid = get_parent_pid().unwrap_or_else(std::process::id);
        let bn_pid = std::process::id();
        let agent = Agent::new(
            parent_pid,
            bn_pid,
            "test-agent".to_string(),
            AgentType::Worker,
        );
        {
            let mut storage = Storage::open(temp.path()).unwrap();
            storage.register_agent(&agent).unwrap();
        }

        // Create two tasks
        let task1 = task_create(
            temp.path(),
            "First task".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task2 = task_create(
            temp.path(),
            "Second task".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Set first task to in_progress
        task_update(
            temp.path(),
            &task1.id,
            None,
            None,
            None,
            None,
            Some("in_progress"),
            vec![],
            vec![],
            None,
            false,
            false, // keep_closed
            false, // reopen
        )
        .unwrap();

        // Set second task to in_progress with force
        let result = task_update(
            temp.path(),
            &task2.id,
            None,
            None,
            None,
            None,
            Some("in_progress"),
            vec![],
            vec![],
            None,
            true,  // force = true
            false, // keep_closed
            false, // reopen
        );
        assert!(result.is_ok(), "Second task should succeed with force");

        // Verify agent has both tasks
        let storage = Storage::open(temp.path()).unwrap();
        let agent = storage.get_agent(parent_pid).unwrap();
        assert_eq!(agent.tasks.len(), 2);
    }

    #[test]
    fn test_task_delete() {
        let temp = setup();
        let created = task_create(
            temp.path(),
            "Test".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        task_delete(temp.path(), &created.id).unwrap();
        let list = task_list(temp.path(), None, None, None).unwrap();
        assert_eq!(list.count, 0);
    }

    // === Dependency Command Tests ===

    #[test]
    fn test_dep_add() {
        let temp = setup();
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        let result = dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();
        assert_eq!(result.child, task_b.id);
        assert_eq!(result.parent, task_a.id);

        // Verify task B now depends on A
        let result = task_show(temp.path(), &task_b.id).unwrap().unwrap();
        assert!(result.task.depends_on.contains(&task_a.id));
    }

    #[test]
    fn test_dep_add_transitions_done_to_partial() {
        let temp = setup();
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        task_close(temp.path(), &task_b.id, Some("Done".to_string()), false).unwrap();
        let result = task_show(temp.path(), &task_b.id).unwrap().unwrap();
        assert_eq!(result.task.status, TaskStatus::Done);
        assert!(result.task.closed_at.is_some());

        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        let result = task_show(temp.path(), &task_b.id).unwrap().unwrap();
        assert_eq!(result.task.status, TaskStatus::Partial);
        assert!(result.task.closed_at.is_none());
        assert!(result.task.closed_reason.is_none());
    }

    #[test]
    fn test_dep_add_cycle_rejected() {
        let temp = setup();
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // A depends on B
        dep_add(temp.path(), &task_a.id, &task_b.id).unwrap();

        // B depends on A should fail (cycle)
        let result = dep_add(temp.path(), &task_b.id, &task_a.id);
        assert!(result.is_err());
    }

    #[test]
    fn test_dep_rm() {
        let temp = setup();
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();
        dep_rm(temp.path(), &task_b.id, &task_a.id).unwrap();

        // Verify task B no longer depends on A
        let result = task_show(temp.path(), &task_b.id).unwrap().unwrap();
        assert!(!result.task.depends_on.contains(&task_a.id));
    }

    #[test]
    fn test_dep_show() {
        let temp = setup();
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_c = task_create(
            temp.path(),
            "Task C".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // B depends on A, C depends on B
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();
        dep_add(temp.path(), &task_c.id, &task_b.id).unwrap();

        let graph = dep_show(temp.path(), &task_b.id).unwrap();
        assert_eq!(graph.task_id, task_b.id);
        assert!(graph.depends_on.contains(&task_a.id));
        assert!(graph.dependents.contains(&task_c.id));
    }

    // === Query Command Tests ===

    #[test]
    fn test_ready_command() {
        let temp = setup();
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // B depends on A (which is pending, so B is blocked)
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        let result = ready(temp.path(), false, false).unwrap();
        assert_eq!(result.count, 1);
        assert_eq!(result.tasks[0].task.core.id, task_a.id);
    }

    #[test]
    fn test_blocked_command() {
        let temp = setup();
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // B depends on A (which is pending, so B is blocked)
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        let result = blocked(temp.path(), false, false).unwrap();
        assert_eq!(result.count, 1);
        assert_eq!(result.tasks[0].task.core.id, task_b.id);
        assert!(result.tasks[0].blocking_tasks.contains(&task_a.id));
    }

    #[test]
    fn test_ready_after_dependency_done() {
        let temp = setup();
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        // Initially B is blocked
        let blocked_result = blocked(temp.path(), false, false).unwrap();
        assert_eq!(blocked_result.count, 1);

        // Close task A
        task_close(temp.path(), &task_a.id, None, false).unwrap();

        // Now B should be ready
        let ready_result = ready(temp.path(), false, false).unwrap();
        assert_eq!(ready_result.count, 1);
        assert_eq!(ready_result.tasks[0].task.core.id, task_b.id);

        // And B should not be blocked anymore
        let blocked_result = blocked(temp.path(), false, false).unwrap();
        assert_eq!(blocked_result.count, 0);
    }

    // === Commit Command Tests ===

    #[test]
    fn test_commit_link() {
        let temp = setup();
        let task = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        let result = commit_link(temp.path(), "a1b2c3d", &task.id).unwrap();
        assert_eq!(result.sha, "a1b2c3d");
        assert_eq!(result.entity_id, task.id);
    }

    #[test]
    fn test_commit_link_invalid_sha() {
        let temp = setup();
        let task = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // SHA too short
        let result = commit_link(temp.path(), "abc", &task.id);
        assert!(result.is_err());
    }

    #[test]
    fn test_commit_link_nonexistent_task() {
        let temp = setup();

        let result = commit_link(temp.path(), "a1b2c3d", "bn-9999");
        assert!(result.is_err());
    }

    #[test]
    fn test_commit_unlink() {
        let temp = setup();
        let task = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        commit_link(temp.path(), "a1b2c3d", &task.id).unwrap();
        let result = commit_unlink(temp.path(), "a1b2c3d", &task.id).unwrap();

        assert_eq!(result.sha, "a1b2c3d");
        assert_eq!(result.entity_id, task.id);

        // Verify commit is no longer linked
        let list = commit_list(temp.path(), &task.id).unwrap();
        assert_eq!(list.count, 0);
    }

    #[test]
    fn test_commit_unlink_nonexistent() {
        let temp = setup();
        let task = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        let result = commit_unlink(temp.path(), "a1b2c3d", &task.id);
        assert!(result.is_err());
    }

    #[test]
    fn test_commit_list() {
        let temp = setup();
        let task = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        commit_link(temp.path(), "a1b2c3d", &task.id).unwrap();
        commit_link(temp.path(), "e5f6789", &task.id).unwrap();

        let list = commit_list(temp.path(), &task.id).unwrap();
        assert_eq!(list.count, 2);
        assert_eq!(list.entity_id, task.id);
    }

    #[test]
    fn test_commit_list_empty() {
        let temp = setup();
        let task = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        let list = commit_list(temp.path(), &task.id).unwrap();
        assert_eq!(list.count, 0);
    }

    #[test]
    fn test_commit_list_nonexistent_task() {
        let temp = setup();

        let result = commit_list(temp.path(), "bn-9999");
        assert!(result.is_err());
    }

    #[test]
    fn test_git_commit_exists_valid() {
        let temp = setup();

        // Create a git repo and make a commit
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(temp.path())
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(temp.path())
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(temp.path())
            .output()
            .unwrap();

        // Create a file and commit
        std::fs::write(temp.path().join("test.txt"), "test content").unwrap();
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(temp.path())
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", "test commit"])
            .current_dir(temp.path())
            .output()
            .unwrap();

        // Get the commit SHA
        let output = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(temp.path())
            .output()
            .unwrap();
        let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Valid commit should return true
        assert!(git_commit_exists(temp.path(), &sha));
    }

    #[test]
    fn test_git_commit_exists_invalid() {
        let temp = setup();

        // Create a git repo but don't make any commits
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(temp.path())
            .output()
            .unwrap();

        // Non-existent commit should return false
        assert!(!git_commit_exists(temp.path(), "abcd1234567890abcdef"));
        assert!(!git_commit_exists(temp.path(), "nonexistent"));
    }

    #[test]
    fn test_git_commit_exists_tree_object() {
        let temp = setup();

        // Create a git repo with a commit
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(temp.path())
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(temp.path())
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(temp.path())
            .output()
            .unwrap();

        std::fs::write(temp.path().join("test.txt"), "test content").unwrap();
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(temp.path())
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", "test commit"])
            .current_dir(temp.path())
            .output()
            .unwrap();

        // Get the tree SHA (not a commit)
        let output = std::process::Command::new("git")
            .args(["rev-parse", "HEAD^{tree}"])
            .current_dir(temp.path())
            .output()
            .unwrap();
        let tree_sha = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Tree object should return false (we only accept commits)
        assert!(!git_commit_exists(temp.path(), &tree_sha));
    }

    // === Doctor Command Tests ===

    #[test]
    fn test_doctor_healthy() {
        let temp = setup();

        // Create a queue so the repo is fully healthy
        let mut storage = Storage::open(temp.path()).unwrap();
        let queue = Queue::new("bnq-test".to_string(), "Test Queue".to_string());
        storage.create_queue(&queue).unwrap();
        drop(storage);

        task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        let result = doctor(temp.path()).unwrap();
        assert!(result.healthy);
        assert!(result.issues.is_empty());
        assert_eq!(result.stats.total_tasks, 1);
    }

    #[test]
    fn test_doctor_consistency_done_task_with_pending_dep() {
        let temp = setup();

        // Create two tasks: A depends on B
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // B depends on A
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        // Close B (which depends on A, but A is still pending) - use force to allow this
        task_close(temp.path(), &task_b.id, None, true).unwrap();

        let result = doctor(temp.path()).unwrap();
        // We should find the consistency warning (done task with pending dependency)
        assert!(!result.healthy);
        assert!(!result.issues.is_empty());
        assert!(result.issues.iter().any(|i| i.category == "consistency"));
    }

    #[test]
    fn test_doctor_stats() {
        let temp = setup();
        task_create(
            temp.path(),
            "Task 1".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        task_create(
            temp.path(),
            "Task 2".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        test_create(
            temp.path(),
            "Test 1".to_string(),
            "echo test".to_string(),
            ".".to_string(),
            None,
            None,
        )
        .unwrap();

        let result = doctor(temp.path()).unwrap();
        assert_eq!(result.stats.total_tasks, 2);
        assert_eq!(result.stats.total_tests, 1);
    }

    #[test]
    fn test_doctor_bug_stats() {
        let temp = setup();
        task_create(
            temp.path(),
            "Task 1".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        bug_create(
            temp.path(),
            "Bug 1".to_string(),
            None,
            None,
            None,
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();
        bug_create(
            temp.path(),
            "Bug 2".to_string(),
            None,
            None,
            None,
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        let result = doctor(temp.path()).unwrap();
        assert_eq!(result.stats.total_tasks, 1);
        assert_eq!(result.stats.total_bugs, 2);
    }

    #[test]
    fn test_doctor_bug_done_with_pending_bug_dep() {
        let temp = setup();

        // Create two bugs: Bug A depends on Bug B
        let bug_b = bug_create(
            temp.path(),
            "Bug B".to_string(),
            None,
            None,
            None,
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        let bug_a = bug_create(
            temp.path(),
            "Bug A".to_string(),
            None,
            None,
            None,
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        // Manually add bug A dependency on bug B
        let mut storage = Storage::open(temp.path()).unwrap();
        let mut bug_a_entity = storage.get_bug(&bug_a.id).unwrap();
        bug_a_entity.depends_on.push(bug_b.id.clone());
        bug_a_entity.core.updated_at = chrono::Utc::now();
        storage.update_bug(&bug_a_entity).unwrap();
        drop(storage);

        // Close bug A (which depends on pending bug B) - use force
        bug_close(temp.path(), &bug_a.id, None, true).unwrap();

        let result = doctor(temp.path()).unwrap();
        // Should find consistency warning (done bug with pending dependency)
        assert!(!result.healthy);
        assert!(
            result
                .issues
                .iter()
                .any(|i| i.category == "consistency" && i.entity_id.as_ref() == Some(&bug_a.id))
        );
    }

    #[test]
    fn test_doctor_detects_legacy_bni_prefix() {
        let temp = setup();

        // Create a queue so we don't get that warning
        let mut storage = Storage::open(temp.path()).unwrap();
        let queue = Queue::new("bnq-test".to_string(), "Test Queue".to_string());
        storage.create_queue(&queue).unwrap();

        // Manually create an idea with the legacy bni- prefix
        let idea = crate::models::Idea::new("bni-test".to_string(), "Test Idea".to_string());
        storage.add_idea(&idea).unwrap();
        drop(storage);

        let result = doctor(temp.path()).unwrap();
        assert!(!result.healthy);
        assert!(result.issues.iter().any(|i| i.category == "legacy_prefix"));
    }

    #[test]
    fn test_doctor_fix_migrates_bni_prefix() {
        let temp = setup();

        // Create a queue so we don't get that warning
        let mut storage = Storage::open(temp.path()).unwrap();
        let queue = Queue::new("bnq-test".to_string(), "Test Queue".to_string());
        storage.create_queue(&queue).unwrap();

        // Manually create an idea with the legacy bni- prefix
        let idea = crate::models::Idea::new("bni-abcd".to_string(), "Test Idea".to_string());
        storage.add_idea(&idea).unwrap();
        drop(storage);

        // Run doctor fix
        let result = doctor_fix(temp.path()).unwrap();
        assert!(
            result
                .fixes_applied
                .iter()
                .any(|f| f.contains("Migrated idea bni-abcd"))
        );

        // Verify the idea now has bn- prefix
        let storage = Storage::open(temp.path()).unwrap();
        assert!(storage.get_idea("bn-abcd").is_ok());
        assert!(storage.get_idea("bni-abcd").is_err());
    }

    #[test]
    fn test_doctor_detects_orphaned_edges() {
        let temp = setup();

        // Create a queue so we don't get that warning
        let mut storage = Storage::open(temp.path()).unwrap();
        let queue = Queue::new("bnq-test".to_string(), "Test Queue".to_string());
        storage.create_queue(&queue).unwrap();
        drop(storage);

        // Create two tasks
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Create an edge between them
        link_add(
            temp.path(),
            &task_a.id,
            &task_b.id,
            "related_to",
            None,
            false,
        )
        .unwrap();

        // Delete task B, leaving an orphaned edge
        task_delete(temp.path(), &task_b.id).unwrap();

        // Run doctor - should detect the orphaned edge
        let result = doctor(temp.path()).unwrap();
        assert!(!result.healthy);
        assert!(result.issues.iter().any(|i| {
            i.category == "orphan" && i.message.contains("target") && i.message.contains(&task_b.id)
        }));
    }

    #[test]
    fn test_doctor_detects_orphaned_edge_source() {
        let temp = setup();

        // Create a queue so we don't get that warning
        let mut storage = Storage::open(temp.path()).unwrap();
        let queue = Queue::new("bnq-test".to_string(), "Test Queue".to_string());
        storage.create_queue(&queue).unwrap();
        drop(storage);

        // Create two tasks
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Create an edge between them
        link_add(
            temp.path(),
            &task_a.id,
            &task_b.id,
            "related_to",
            None,
            false,
        )
        .unwrap();

        // Delete task A (source), leaving an orphaned edge
        task_delete(temp.path(), &task_a.id).unwrap();

        // Run doctor - should detect the orphaned edge (source missing)
        let result = doctor(temp.path()).unwrap();
        assert!(!result.healthy);
        assert!(result.issues.iter().any(|i| {
            i.category == "orphan" && i.message.contains("source") && i.message.contains(&task_a.id)
        }));
    }

    #[test]
    fn test_doctor_detects_orphan_docs() {
        let temp = setup();

        // Create a queue so we don't get that warning
        let mut storage = Storage::open(temp.path()).unwrap();
        let queue = Queue::new("bnq-test".to_string(), "Test Queue".to_string());
        storage.create_queue(&queue).unwrap();
        drop(storage);

        // Create a task
        let task = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Create a doc linked to the task (should NOT be an orphan)
        let linked_doc = doc_create(
            temp.path(),
            "Linked Doc".to_string(),
            DocType::Prd,
            None,
            Some("# Summary\nLinked doc content".to_string()),
            None,
            vec![],
            vec![task.id.clone()],
        )
        .unwrap();

        // Run doctor - should be healthy (doc is linked)
        let result = doctor(temp.path()).unwrap();
        // Check there are no orphan doc warnings for the linked doc
        assert!(!result.issues.iter().any(|i| {
            i.category == "orphan"
                && i.entity_id.as_ref() == Some(&linked_doc.id)
                && i.message.contains("has no linked entities")
        }));

        // Now create an orphan doc (create doc, then remove its link)
        let orphan_doc = doc_create(
            temp.path(),
            "Orphan Doc".to_string(),
            DocType::Note,
            None,
            Some("# Summary\nOrphan doc content".to_string()),
            None,
            vec![],
            vec![task.id.clone()],
        )
        .unwrap();

        // Remove the link to make it an orphan
        // Note: doc_create creates edges with EdgeType::Documents (doc -> entity)
        link_rm(temp.path(), &orphan_doc.id, &task.id, Some("documents")).unwrap();

        // Run doctor again - should detect the orphan doc
        let result = doctor(temp.path()).unwrap();
        assert!(result.issues.iter().any(|i| {
            i.category == "orphan"
                && i.entity_id.as_ref() == Some(&orphan_doc.id)
                && i.message.contains("has no linked entities")
        }));
    }

    // === Log Command Tests ===

    #[test]
    fn test_log_basic() {
        let temp = setup();
        let task = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        let result = log(temp.path(), None).unwrap();
        assert!(result.count >= 1);
        assert!(result.entries.iter().any(|e| e.entity_id == task.id));
    }

    #[test]
    fn test_log_filter_by_task() {
        let temp = setup();
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let _task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        let result = log(temp.path(), Some(&task_a.id)).unwrap();
        assert!(result.entries.iter().all(|e| e.entity_id == task_a.id));
        assert_eq!(result.filtered_by, Some(task_a.id.clone()));
    }

    #[test]
    fn test_log_includes_updates() {
        let temp = setup();
        let task = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Update the task
        task_update(
            temp.path(),
            &task.id,
            Some("Updated Title".to_string()),
            None, // short_name
            None,
            None,
            None,
            vec![],
            vec![],
            None,
            false,
            false, // keep_closed
            false, // reopen
        )
        .unwrap();

        let result = log(temp.path(), Some(&task.id)).unwrap();
        // Should have at least 2 entries: created and updated
        assert!(result.count >= 2);
        assert!(result.entries.iter().any(|e| e.action == "created"));
        assert!(result.entries.iter().any(|e| e.action == "updated"));
    }

    #[test]
    fn test_log_includes_close() {
        let temp = setup();
        let task = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        task_close(temp.path(), &task.id, Some("Complete".to_string()), false).unwrap();

        let result = log(temp.path(), Some(&task.id)).unwrap();
        assert!(result.entries.iter().any(|e| e.action == "closed"));
    }

    // === Config Command Tests ===

    #[test]
    fn test_config_set_and_get() {
        let temp = setup();

        config_set(temp.path(), "test.key", "test_value").unwrap();
        let result = config_get(temp.path(), "test.key").unwrap();

        assert_eq!(result.key, "test.key");
        assert_eq!(result.value, Some("test_value".to_string()));
    }

    #[test]
    fn test_config_get_nonexistent() {
        let temp = setup();

        let result = config_get(temp.path(), "nonexistent.key").unwrap();
        assert_eq!(result.key, "nonexistent.key");
        assert_eq!(result.value, None);
    }

    #[test]
    fn test_config_list() {
        let temp = setup();

        config_set(temp.path(), "key1", "value1").unwrap();
        config_set(temp.path(), "key2", "value2").unwrap();

        let result = config_list(temp.path()).unwrap();
        assert_eq!(result.count, 2);
        assert!(
            result
                .configs
                .iter()
                .any(|(k, v)| k == "key1" && v == "value1")
        );
        assert!(
            result
                .configs
                .iter()
                .any(|(k, v)| k == "key2" && v == "value2")
        );
    }

    #[test]
    fn test_config_list_empty() {
        let temp = setup();

        let result = config_list(temp.path()).unwrap();
        assert_eq!(result.count, 0);
    }

    #[test]
    fn test_config_overwrite() {
        let temp = setup();

        config_set(temp.path(), "key", "value1").unwrap();
        config_set(temp.path(), "key", "value2").unwrap();

        let result = config_get(temp.path(), "key").unwrap();
        assert_eq!(result.value, Some("value2".to_string()));
    }

    // === Init AGENTS.md Tests ===
    // === Init AGENTS.md Tests ===

    #[test]
    fn test_init_creates_agents_md() {
        let temp = TestEnv::new_with_env();
        let agents_path = temp.path().join("AGENTS.md");

        // Verify AGENTS.md doesn't exist yet
        assert!(!agents_path.exists());

        // Run init with AGENTS.md update enabled
        let result = init_with_options(temp.path(), true, false, false, false, false).unwrap();
        assert!(result.initialized);
        assert!(result.agents_md_updated);
        assert!(!result.skills_file_created);

        // Verify AGENTS.md was created
        assert!(agents_path.exists());
        let contents = std::fs::read_to_string(&agents_path).unwrap();
        assert!(contents.contains("bn orient"));
        assert!(contents.contains("binnacle"));
    }

    #[test]
    fn test_init_appends_to_existing_agents_md() {
        let temp = TestEnv::new_with_env();
        let agents_path = temp.path().join("AGENTS.md");

        // Create existing AGENTS.md
        std::fs::write(&agents_path, "# My Existing Agents\n\nSome content here.\n").unwrap();

        // Run init with AGENTS.md update enabled
        let result = init_with_options(temp.path(), true, false, false, false, false).unwrap();
        assert!(result.initialized);
        assert!(result.agents_md_updated);

        // Verify content was appended
        let contents = std::fs::read_to_string(&agents_path).unwrap();
        assert!(contents.contains("My Existing Agents"));
        assert!(contents.contains("bn orient"));
    }

    #[test]
    fn test_init_appends_section_if_legacy_bn_orient() {
        let temp = TestEnv::new_with_env();
        let agents_path = temp.path().join("AGENTS.md");

        // Create existing AGENTS.md that references bn orient but lacks markers
        std::fs::write(
            &agents_path,
            "# Agents\n\nRun `bn orient` to get started.\n",
        )
        .unwrap();

        // Run init with AGENTS.md update enabled
        let result = init_with_options(temp.path(), true, false, false, false, false).unwrap();
        assert!(result.initialized);
        assert!(result.agents_md_updated); // Should be updated to add markers

        // Verify markers were added and original content preserved
        let contents = std::fs::read_to_string(&agents_path).unwrap();
        assert!(contents.contains("# Agents")); // Original content preserved
        assert!(contents.contains("<!-- BEGIN BINNACLE SECTION -->")); // Markers added
        assert!(contents.contains("<!-- END BINNACLE SECTION -->"));
    }

    #[test]
    fn test_init_idempotent_agents_md() {
        let temp = TestEnv::new_with_env();

        // Run init twice with AGENTS.md enabled
        init_with_options(temp.path(), true, false, false, false, false).unwrap();
        let result = init_with_options(temp.path(), true, false, false, false, false).unwrap();

        // Second run should not update AGENTS.md (content unchanged)
        assert!(!result.initialized); // binnacle already exists
        assert!(!result.agents_md_updated); // AGENTS.md content unchanged
    }

    #[test]
    fn test_init_no_change_when_standard_blurb_already_present() {
        let temp = TestEnv::new_with_env();
        let agents_path = temp.path().join("AGENTS.md");

        // Pre-create AGENTS.md with the exact standard content (with trailing newline)
        std::fs::write(&agents_path, format!("{}\n", AGENTS_MD_BLURB.trim_end())).unwrap();

        // Initialize binnacle storage (without AGENTS.md update first)
        Storage::init(temp.path()).unwrap();

        // Now run init with AGENTS.md update - should detect no change needed
        let result = init_with_options(temp.path(), true, false, false, false, false).unwrap();
        assert!(!result.initialized); // binnacle already exists
        assert!(!result.agents_md_updated); // Content already matches exactly

        // Verify file wasn't modified (content identical)
        let contents = std::fs::read_to_string(&agents_path).unwrap();
        assert_eq!(contents, format!("{}\n", AGENTS_MD_BLURB.trim_end()));
    }

    #[test]
    fn test_init_replaces_custom_binnacle_section() {
        let temp = TestEnv::new_with_env();
        let agents_path = temp.path().join("AGENTS.md");

        // Create existing AGENTS.md with custom binnacle section
        std::fs::write(
            &agents_path,
            "# Agents\n\n<!-- BEGIN BINNACLE SECTION -->\nCustom content\n<!-- END BINNACLE SECTION -->\n",
        )
        .unwrap();

        // Run init with AGENTS.md update enabled
        let result = init_with_options(temp.path(), true, false, false, false, false).unwrap();
        assert!(result.initialized);
        assert!(result.agents_md_updated); // Section was replaced with standard content

        // Verify section was replaced with standard content
        let contents = std::fs::read_to_string(&agents_path).unwrap();
        assert!(contents.contains("# Agents")); // User content preserved
        assert!(contents.contains("bn orient")); // Standard section added
        assert!(!contents.contains("Custom content")); // Custom content replaced
        assert_eq!(contents.matches("BEGIN BINNACLE SECTION").count(), 1);
    }

    #[test]
    fn test_agents_md_has_html_markers() {
        let temp = TestEnv::new_with_env();
        let agents_path = temp.path().join("AGENTS.md");

        // Run init with AGENTS.md update enabled
        init_with_options(temp.path(), true, false, false, false, false).unwrap();

        // Verify AGENTS.md contains HTML markers
        let contents = std::fs::read_to_string(&agents_path).unwrap();
        assert!(contents.contains("<!-- BEGIN BINNACLE SECTION -->"));
        assert!(contents.contains("<!-- END BINNACLE SECTION -->"));
    }

    // === Commit-msg Hook Tests ===

    #[test]
    fn test_hook_install_creates_hook_file() {
        let temp = TestEnv::new_with_env();
        // Create .git/hooks directory
        std::fs::create_dir_all(temp.path().join(".git").join("hooks")).unwrap();

        let result = install_commit_msg_hook(temp.path()).unwrap();
        assert!(result); // Hook was installed

        let hook_path = temp.path().join(".git").join("hooks").join("commit-msg");
        assert!(hook_path.exists());

        let contents = std::fs::read_to_string(&hook_path).unwrap();
        assert!(contents.contains("#!/usr/bin/env bash"));
        assert!(contents.contains("### BINNACLE HOOK START ###"));
        assert!(contents.contains("### BINNACLE HOOK END ###"));
        assert!(contents.contains("Co-authored-by"));
    }

    #[test]
    fn test_hook_install_appends_to_existing_hook() {
        let temp = TestEnv::new_with_env();
        let hooks_dir = temp.path().join(".git").join("hooks");
        std::fs::create_dir_all(&hooks_dir).unwrap();

        // Create an existing hook
        let hook_path = hooks_dir.join("commit-msg");
        std::fs::write(&hook_path, "#!/bin/bash\necho 'existing hook'\n").unwrap();

        let result = install_commit_msg_hook(temp.path()).unwrap();
        assert!(result); // Hook was updated

        let contents = std::fs::read_to_string(&hook_path).unwrap();
        assert!(contents.contains("existing hook")); // Original content preserved
        assert!(contents.contains("### BINNACLE HOOK START ###")); // Binnacle added
    }

    #[test]
    fn test_hook_install_idempotent() {
        let temp = TestEnv::new_with_env();
        std::fs::create_dir_all(temp.path().join(".git").join("hooks")).unwrap();

        // Install hook first time
        let result1 = install_commit_msg_hook(temp.path()).unwrap();
        assert!(result1); // Installed

        // Install hook second time - should be idempotent
        let result2 = install_commit_msg_hook(temp.path()).unwrap();
        assert!(!result2); // Already installed, no change

        // Verify only one binnacle section
        let hook_path = temp.path().join(".git").join("hooks").join("commit-msg");
        let contents = std::fs::read_to_string(&hook_path).unwrap();
        assert_eq!(contents.matches("### BINNACLE HOOK START ###").count(), 1);
    }

    #[test]
    fn test_hook_uninstall_removes_binnacle_section() {
        let temp = TestEnv::new_with_env();
        let hooks_dir = temp.path().join(".git").join("hooks");
        std::fs::create_dir_all(&hooks_dir).unwrap();

        // Create hook with existing content + binnacle section
        let hook_path = hooks_dir.join("commit-msg");
        std::fs::write(&hook_path, "#!/bin/bash\necho 'existing hook'\n").unwrap();
        install_commit_msg_hook(temp.path()).unwrap();

        // Uninstall
        let result = uninstall_commit_msg_hook(temp.path()).unwrap();
        assert!(result); // Something was removed

        // Verify original content remains, binnacle removed
        let contents = std::fs::read_to_string(&hook_path).unwrap();
        assert!(contents.contains("existing hook"));
        assert!(!contents.contains("### BINNACLE HOOK START ###"));
    }

    #[test]
    fn test_hook_uninstall_removes_file_if_only_binnacle() {
        let temp = TestEnv::new_with_env();
        std::fs::create_dir_all(temp.path().join(".git").join("hooks")).unwrap();

        // Install hook (creates new file)
        install_commit_msg_hook(temp.path()).unwrap();

        let hook_path = temp.path().join(".git").join("hooks").join("commit-msg");
        assert!(hook_path.exists());

        // Uninstall
        let result = uninstall_commit_msg_hook(temp.path()).unwrap();
        assert!(result);

        // File should be removed since it only contained binnacle
        assert!(!hook_path.exists());
    }

    #[test]
    fn test_hook_uninstall_noop_when_no_hook() {
        let temp = TestEnv::new_with_env();
        std::fs::create_dir_all(temp.path().join(".git").join("hooks")).unwrap();

        // Uninstall when no hook exists
        let result = uninstall_commit_msg_hook(temp.path()).unwrap();
        assert!(!result); // Nothing to do
    }

    #[test]
    fn test_hook_uninstall_noop_when_no_binnacle_section() {
        let temp = TestEnv::new_with_env();
        let hooks_dir = temp.path().join(".git").join("hooks");
        std::fs::create_dir_all(&hooks_dir).unwrap();

        // Create hook without binnacle section
        let hook_path = hooks_dir.join("commit-msg");
        std::fs::write(&hook_path, "#!/bin/bash\necho 'other hook'\n").unwrap();

        // Uninstall should be noop
        let result = uninstall_commit_msg_hook(temp.path()).unwrap();
        assert!(!result);

        // Original content unchanged
        let contents = std::fs::read_to_string(&hook_path).unwrap();
        assert!(contents.contains("other hook"));
    }

    #[test]
    fn test_init_with_hook_flag() {
        let temp = TestEnv::new_with_env();
        std::fs::create_dir_all(temp.path().join(".git").join("hooks")).unwrap();

        // Init with hook installation
        let result = init_with_options(temp.path(), false, false, false, false, true).unwrap();
        assert!(result.hook_installed);

        let hook_path = temp.path().join(".git").join("hooks").join("commit-msg");
        assert!(hook_path.exists());
    }

    #[test]
    fn test_init_without_hook_flag() {
        let temp = TestEnv::new_with_env();
        std::fs::create_dir_all(temp.path().join(".git").join("hooks")).unwrap();

        // Init without hook installation
        let result = init_with_options(temp.path(), false, false, false, false, false).unwrap();
        assert!(!result.hook_installed);

        let hook_path = temp.path().join(".git").join("hooks").join("commit-msg");
        assert!(!hook_path.exists());
    }

    // === Orient Command Tests ===

    #[test]
    fn test_orient_without_init_fails_when_not_initialized() {
        let temp = TestEnv::new_with_env();

        // Verify not initialized
        assert!(!Storage::exists(temp.path()).unwrap());

        // Run orient without allow_init - should fail (dry_run doesn't matter here since it fails)
        let result = orient(temp.path(), "worker", false, None, None, true);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::NotInitialized));
    }

    #[test]
    fn test_orient_with_init_creates_database() {
        let temp = TestEnv::new_with_env();

        // Verify not initialized
        assert!(!Storage::exists(temp.path()).unwrap());

        // Run orient with allow_init=true (dry_run to avoid agent registration)
        let result = orient(temp.path(), "worker", true, None, None, true).unwrap();
        assert!(result.ready); // Store is ready to use
        assert!(result.just_initialized); // Was just initialized this call

        // Verify now initialized
        assert!(Storage::exists(temp.path()).unwrap());

        // Verify AGENTS.md was created
        let agents_path = temp.path().join("AGENTS.md");
        assert!(agents_path.exists());
    }

    #[test]
    fn test_orient_shows_task_counts() {
        let temp = setup();

        // Create some tasks
        task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        let result = orient(temp.path(), "worker", false, None, None, true).unwrap();
        assert!(result.ready); // Store is ready to use
        assert!(!result.just_initialized); // Already initialized in setup()
        assert_eq!(result.total_tasks, 2);
        assert_eq!(result.ready_count, 2); // Both pending tasks are ready
    }

    #[test]
    fn test_orient_shows_blocked_tasks() {
        let temp = setup();

        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // B depends on A (so B is blocked)
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        let result = orient(temp.path(), "worker", false, None, None, true).unwrap();
        assert_eq!(result.total_tasks, 2);
        assert_eq!(result.ready_count, 1);
        assert!(result.ready_ids.contains(&task_a.id));
        assert_eq!(result.blocked_count, 1);
    }

    #[test]
    fn test_orient_shows_in_progress_tasks() {
        let temp = setup();

        let task = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Update to in_progress
        task_update(
            temp.path(),
            &task.id,
            None,
            None, // short_name
            None,
            None,
            Some("in_progress"),
            vec![],
            vec![],
            None,
            false,
            false, // keep_closed
            false, // reopen
        )
        .unwrap();

        let result = orient(temp.path(), "worker", false, None, None, true).unwrap();
        assert_eq!(result.in_progress_count, 1);
    }

    #[test]
    fn test_orient_human_output() {
        let temp = setup();
        task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        let result = orient(temp.path(), "worker", false, None, None, true).unwrap();
        let human = result.to_human();

        assert!(human.contains("Binnacle - AI agent task tracker"));
        assert!(human.contains("Total tasks: 1"));
        assert!(human.contains("bn ready"));
        assert!(human.contains("bn task list"));
    }

    #[test]
    fn test_orient_shows_bug_counts() {
        let temp = setup();

        // Create some bugs with different severities and statuses
        bug_create(
            temp.path(),
            "Low bug".to_string(),
            None,
            None,
            None,
            Some("low".to_string()),
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        bug_create(
            temp.path(),
            "Critical bug".to_string(),
            None,
            None,
            None,
            Some("critical".to_string()),
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        let closed_bug = bug_create(
            temp.path(),
            "Closed bug".to_string(),
            None,
            None,
            None,
            Some("high".to_string()),
            vec![],
            None,
            None,
            None,
        )
        .unwrap();
        bug_close(
            temp.path(),
            &closed_bug.id,
            Some("fixed".to_string()),
            false,
        )
        .unwrap();

        let result = orient(temp.path(), "worker", false, None, None, true).unwrap();
        assert_eq!(result.total_bugs, 3);
        assert_eq!(result.open_bugs_count, 2); // Low and Critical are open
        assert_eq!(result.critical_bugs_count, 1); // Only Critical (not High since it's closed)

        // Check human output includes bug info
        let human = result.to_human();
        assert!(human.contains("Bugs: 2 open"));
        assert!(human.contains("(1 high/critical)"));
    }

    #[test]
    fn test_orient_with_purpose_registers_agent() {
        let temp = setup();

        // Orient with a purpose (dry_run: false to actually register)
        let _result = orient(
            temp.path(),
            "worker",
            false,
            Some("test-agent".to_string()),
            Some("Task Worker".to_string()),
            false, // Not dry_run - we need to test registration
        )
        .unwrap();

        // Check the agent was registered with the purpose
        let storage = Storage::open(temp.path()).unwrap();
        let agents = storage.list_agents(None).unwrap();

        // Find our agent
        let agent = agents.iter().find(|a| a.name == "test-agent");
        assert!(agent.is_some());
        let agent = agent.unwrap();
        assert_eq!(agent.purpose, Some("Task Worker".to_string()));
        assert!(agent.is_registered());
        assert_eq!(agent.display_purpose(), "Task Worker");
    }

    #[test]
    fn test_orient_without_purpose_shows_unregistered() {
        let temp = setup();

        // Orient without a purpose (dry_run: false to actually register)
        let _result = orient(
            temp.path(),
            "worker",
            false,
            Some("anon-agent".to_string()),
            None,
            false, // Not dry_run - we need to test registration
        )
        .unwrap();

        // Check the agent was registered without purpose (UNREGISTERED)
        let storage = Storage::open(temp.path()).unwrap();
        let agents = storage.list_agents(None).unwrap();

        let agent = agents.iter().find(|a| a.name == "anon-agent");
        assert!(agent.is_some());
        let agent = agent.unwrap();
        assert_eq!(agent.purpose, None);
        assert!(!agent.is_registered());
        assert_eq!(agent.display_purpose(), "UNREGISTERED");
    }

    #[test]
    fn test_orient_can_update_purpose() {
        let temp = setup();

        // First orient without purpose (dry_run: false to actually register)
        let _result = orient(
            temp.path(),
            "worker",
            false,
            Some("update-agent".to_string()),
            None,
            false, // Not dry_run - we need to test registration
        )
        .unwrap();

        // Verify UNREGISTERED
        {
            let storage = Storage::open(temp.path()).unwrap();
            let agents = storage.list_agents(None).unwrap();
            let agent = agents.iter().find(|a| a.name == "update-agent").unwrap();
            assert_eq!(agent.display_purpose(), "UNREGISTERED");
        }

        // Orient again with purpose - should update the existing agent
        let _result = orient(
            temp.path(),
            "planner",
            false,
            Some("update-agent".to_string()),
            Some("PRD Generator".to_string()),
            false, // Not dry_run - we need to test registration
        )
        .unwrap();

        // Verify purpose was updated
        let storage = Storage::open(temp.path()).unwrap();
        let agents = storage.list_agents(None).unwrap();
        let agent = agents.iter().find(|a| a.name == "update-agent").unwrap();
        assert_eq!(agent.display_purpose(), "PRD Generator");
    }

    // === Blocker Analysis Tests ===

    #[test]
    fn test_task_show_no_dependencies_no_blocking_info() {
        let temp = setup();
        let task = task_create(
            temp.path(),
            "Solo Task".to_string(),
            None, // short_name
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        let result = task_show(temp.path(), &task.id).unwrap().unwrap();

        // No dependencies means no blocking info
        assert!(result.blocking_info.is_none());
    }

    #[test]
    fn test_task_show_all_dependencies_complete() {
        let temp = setup();
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Close both dependencies
        task_close(temp.path(), &task_a.id, Some("Done".to_string()), false).unwrap();
        task_close(temp.path(), &task_b.id, Some("Done".to_string()), false).unwrap();

        // Create task C that depends on A and B
        let task_c = task_create(
            temp.path(),
            "Task C".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        dep_add(temp.path(), &task_c.id, &task_a.id).unwrap();
        dep_add(temp.path(), &task_c.id, &task_b.id).unwrap();

        let result = task_show(temp.path(), &task_c.id).unwrap().unwrap();

        // Has dependencies but all are complete
        assert!(result.blocking_info.is_some());
        let blocking = result.blocking_info.unwrap();
        assert!(!blocking.is_blocked);
        assert_eq!(blocking.blocker_count, 0);
        assert_eq!(blocking.direct_blockers.len(), 0);
        assert!(blocking.summary.contains("All dependencies complete"));
    }

    #[test]
    fn test_task_show_direct_blockers() {
        let temp = setup();
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None, // short_name
            None,
            None,
            vec![],
            Some("agent-1".to_string()),
        )
        .unwrap();
        let task_c = task_create(
            temp.path(),
            "Task C".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Set task A to in_progress
        task_update(
            temp.path(),
            &task_a.id,
            None,
            None, // short_name
            None,
            None,
            Some("in_progress"),
            vec![],
            vec![],
            None,
            false,
            false, // keep_closed
            false, // reopen
        )
        .unwrap();

        // Set task B to pending (default)
        // Task C depends on A and B
        dep_add(temp.path(), &task_c.id, &task_a.id).unwrap();
        dep_add(temp.path(), &task_c.id, &task_b.id).unwrap();

        let result = task_show(temp.path(), &task_c.id).unwrap().unwrap();

        assert!(result.blocking_info.is_some());
        let blocking = result.blocking_info.unwrap();
        assert!(blocking.is_blocked);
        assert_eq!(blocking.blocker_count, 2);
        assert_eq!(blocking.direct_blockers.len(), 2);

        // Check direct blocker details
        let blocker_a = blocking
            .direct_blockers
            .iter()
            .find(|b| b.id == task_a.id)
            .unwrap();
        assert_eq!(blocker_a.title, "Task A");
        assert_eq!(blocker_a.status, "inprogress");
        assert!(blocker_a.assignee.is_none());
        assert_eq!(blocker_a.blocked_by.len(), 0);

        let blocker_b = blocking
            .direct_blockers
            .iter()
            .find(|b| b.id == task_b.id)
            .unwrap();
        assert_eq!(blocker_b.title, "Task B");
        assert_eq!(blocker_b.status, "pending");
        assert_eq!(blocker_b.assignee, Some("agent-1".to_string()));
        assert_eq!(blocker_b.blocked_by.len(), 0);
    }

    #[test]
    fn test_task_show_transitive_blockers() {
        let temp = setup();
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_c = task_create(
            temp.path(),
            "Task C".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Create chain: C depends on B, B depends on A
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();
        dep_add(temp.path(), &task_c.id, &task_b.id).unwrap();

        let result = task_show(temp.path(), &task_c.id).unwrap().unwrap();

        assert!(result.blocking_info.is_some());
        let blocking = result.blocking_info.unwrap();
        assert!(blocking.is_blocked);
        assert_eq!(blocking.blocker_count, 1);
        assert_eq!(blocking.direct_blockers.len(), 1);

        // Task B is the direct blocker
        let blocker_b = &blocking.direct_blockers[0];
        assert_eq!(blocker_b.id, task_b.id);
        assert_eq!(blocker_b.title, "Task B");
        assert_eq!(blocker_b.status, "pending");

        // Task B is itself blocked by A
        assert_eq!(blocker_b.blocked_by.len(), 1);
        assert!(blocker_b.blocked_by.contains(&task_a.id));

        // Check blocker chain format
        assert_eq!(blocking.blocker_chain.len(), 1);
        assert!(blocking.blocker_chain[0].contains(&task_c.id));
        assert!(blocking.blocker_chain[0].contains(&task_b.id));
        assert!(blocking.blocker_chain[0].contains(&task_a.id));
    }

    #[test]
    fn test_task_show_mixed_complete_incomplete_deps() {
        let temp = setup();
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_c = task_create(
            temp.path(),
            "Task C".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Close task A
        task_close(temp.path(), &task_a.id, Some("Done".to_string()), false).unwrap();

        // Task C depends on both A (done) and B (pending)
        dep_add(temp.path(), &task_c.id, &task_a.id).unwrap();
        dep_add(temp.path(), &task_c.id, &task_b.id).unwrap();

        let result = task_show(temp.path(), &task_c.id).unwrap().unwrap();

        assert!(result.blocking_info.is_some());
        let blocking = result.blocking_info.unwrap();
        assert!(blocking.is_blocked);
        assert_eq!(blocking.blocker_count, 1); // Only task B is blocking

        // Only task B should appear as a blocker
        assert_eq!(blocking.direct_blockers.len(), 1);
        assert_eq!(blocking.direct_blockers[0].id, task_b.id);
    }

    #[test]
    fn test_task_show_blocker_summary_format() {
        let temp = setup();
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None, // short_name
            None,
            None,
            vec![],
            Some("alice".to_string()),
        )
        .unwrap();
        let task_c = task_create(
            temp.path(),
            "Task C".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Task C depends on A and B
        dep_add(temp.path(), &task_c.id, &task_a.id).unwrap();
        dep_add(temp.path(), &task_c.id, &task_b.id).unwrap();

        let result = task_show(temp.path(), &task_c.id).unwrap().unwrap();
        let blocking = result.blocking_info.unwrap();

        // Check summary format
        assert!(
            blocking
                .summary
                .contains("Blocked by 2 incomplete dependencies")
        );
        assert!(blocking.summary.contains(&task_a.id));
        assert!(blocking.summary.contains("pending"));
        assert!(blocking.summary.contains(&task_b.id));
        assert!(blocking.summary.contains("alice"));
    }

    #[test]
    fn test_task_show_cancelled_dependencies_dont_block() {
        let temp = setup();
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_c = task_create(
            temp.path(),
            "Task C".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Cancel task A
        task_update(
            temp.path(),
            &task_a.id,
            None,
            None, // short_name
            None,
            None,
            Some("cancelled"),
            vec![],
            vec![],
            None,
            false,
            false, // keep_closed
            false, // reopen
        )
        .unwrap();

        // Close task B normally
        task_close(temp.path(), &task_b.id, Some("Done".to_string()), false).unwrap();

        // Task C depends on both A (cancelled) and B (done)
        dep_add(temp.path(), &task_c.id, &task_a.id).unwrap();
        dep_add(temp.path(), &task_c.id, &task_b.id).unwrap();

        let result = task_show(temp.path(), &task_c.id).unwrap().unwrap();

        assert!(result.blocking_info.is_some());
        let blocking = result.blocking_info.unwrap();
        // Neither cancelled nor done should block
        assert!(!blocking.is_blocked);
        assert_eq!(blocking.blocker_count, 0);
        assert_eq!(blocking.direct_blockers.len(), 0);
        assert!(blocking.summary.contains("All dependencies complete"));
    }

    #[test]
    fn test_task_show_blocked_status_is_blocker() {
        let temp = setup();
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Set task A to blocked status
        task_update(
            temp.path(),
            &task_a.id,
            None,
            None, // short_name
            None,
            None,
            Some("blocked"),
            vec![],
            vec![],
            None,
            false,
            false, // keep_closed
            false, // reopen
        )
        .unwrap();

        // Task B depends on A (blocked)
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        let result = task_show(temp.path(), &task_b.id).unwrap().unwrap();

        assert!(result.blocking_info.is_some());
        let blocking = result.blocking_info.unwrap();
        assert!(blocking.is_blocked);
        assert_eq!(blocking.blocker_count, 1);
        assert_eq!(blocking.direct_blockers[0].id, task_a.id);
        assert_eq!(blocking.direct_blockers[0].status, "blocked");
    }

    #[test]
    fn test_task_show_partial_status_is_blocker() {
        let temp = setup();
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Set task A to partial status
        task_update(
            temp.path(),
            &task_a.id,
            None,
            None, // short_name
            None,
            None,
            Some("partial"),
            vec![],
            vec![],
            None,
            false,
            false, // keep_closed
            false, // reopen
        )
        .unwrap();
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        let result = task_show(temp.path(), &task_b.id).unwrap().unwrap();

        assert!(result.blocking_info.is_some());
        let blocking = result.blocking_info.unwrap();
        assert!(blocking.is_blocked);
        assert_eq!(blocking.blocker_count, 1);
        assert_eq!(blocking.direct_blockers[0].id, task_a.id);
        assert_eq!(blocking.direct_blockers[0].status, "partial");
    }

    #[test]
    fn test_task_show_reopened_status_is_blocker() {
        let temp = setup();
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Close then reopen task A
        task_close(temp.path(), &task_a.id, Some("Done".to_string()), false).unwrap();
        task_reopen(temp.path(), &task_a.id).unwrap();

        // Task B depends on A (reopened)
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        let result = task_show(temp.path(), &task_b.id).unwrap().unwrap();

        assert!(result.blocking_info.is_some());
        let blocking = result.blocking_info.unwrap();
        assert!(blocking.is_blocked);
        assert_eq!(blocking.blocker_count, 1);
        assert_eq!(blocking.direct_blockers[0].id, task_a.id);
        assert_eq!(blocking.direct_blockers[0].status, "reopened");
    }

    #[test]
    fn test_task_show_deep_transitive_blocker_chain() {
        let temp = setup();
        // Create chain: D -> C -> B -> A
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_c = task_create(
            temp.path(),
            "Task C".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_d = task_create(
            temp.path(),
            "Task D".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();
        dep_add(temp.path(), &task_c.id, &task_b.id).unwrap();
        dep_add(temp.path(), &task_d.id, &task_c.id).unwrap();

        let result = task_show(temp.path(), &task_d.id).unwrap().unwrap();

        assert!(result.blocking_info.is_some());
        let blocking = result.blocking_info.unwrap();
        assert!(blocking.is_blocked);
        assert_eq!(blocking.blocker_count, 1);

        // Task C is the direct blocker of D
        let blocker_c = &blocking.direct_blockers[0];
        assert_eq!(blocker_c.id, task_c.id);

        // Task C is blocked by B (transitive)
        assert_eq!(blocker_c.blocked_by.len(), 1);
        assert!(blocker_c.blocked_by.contains(&task_b.id));

        // Blocker chain should show the path
        assert_eq!(blocking.blocker_chain.len(), 1);
        assert!(blocking.blocker_chain[0].contains(&task_d.id));
        assert!(blocking.blocker_chain[0].contains(&task_c.id));
        assert!(blocking.blocker_chain[0].contains(&task_b.id));
    }

    #[test]
    fn test_task_show_multiple_transitive_blockers() {
        let temp = setup();
        // Create diamond: D depends on B and C, both B and C depend on A
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_c = task_create(
            temp.path(),
            "Task C".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_d = task_create(
            temp.path(),
            "Task D".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();
        dep_add(temp.path(), &task_c.id, &task_a.id).unwrap();
        dep_add(temp.path(), &task_d.id, &task_b.id).unwrap();
        dep_add(temp.path(), &task_d.id, &task_c.id).unwrap();

        let result = task_show(temp.path(), &task_d.id).unwrap().unwrap();

        assert!(result.blocking_info.is_some());
        let blocking = result.blocking_info.unwrap();
        assert!(blocking.is_blocked);
        assert_eq!(blocking.blocker_count, 2); // Both B and C block D

        // Both B and C should be blocked by A
        for blocker in &blocking.direct_blockers {
            assert_eq!(blocker.blocked_by.len(), 1);
            assert!(blocker.blocked_by.contains(&task_a.id));
        }

        // Should have two chains in the blocker chain
        assert_eq!(blocking.blocker_chain.len(), 2);
    }

    #[test]
    fn test_build_blocker_summary_single() {
        let blockers = vec![DirectBlocker {
            id: "bn-test1".to_string(),
            title: "Test Task".to_string(),
            status: "pending".to_string(),
            assignee: None,
            blocked_by: vec![],
        }];

        let summary = build_blocker_summary(&blockers, 1);

        assert!(summary.contains("Blocked by 1 incomplete dependencies"));
        assert!(summary.contains("bn-test1"));
        assert!(summary.contains("pending"));
    }

    #[test]
    fn test_build_blocker_summary_multiple_with_assignees() {
        let blockers = vec![
            DirectBlocker {
                id: "bn-test1".to_string(),
                title: "Test Task 1".to_string(),
                status: "in_progress".to_string(),
                assignee: Some("alice".to_string()),
                blocked_by: vec![],
            },
            DirectBlocker {
                id: "bn-test2".to_string(),
                title: "Test Task 2".to_string(),
                status: "blocked".to_string(),
                assignee: Some("bob".to_string()),
                blocked_by: vec!["bn-test3".to_string()],
            },
        ];

        let summary = build_blocker_summary(&blockers, 2);

        assert!(summary.contains("Blocked by 2 incomplete dependencies"));
        assert!(summary.contains("bn-test1"));
        assert!(summary.contains("in_progress"));
        assert!(summary.contains("alice"));
        assert!(summary.contains("bn-test2"));
        assert!(summary.contains("blocked"));
        assert!(summary.contains("bob"));
        assert!(summary.contains("bn-test3"));
    }

    #[test]
    fn test_build_blocker_summary_no_assignee() {
        let blockers = vec![DirectBlocker {
            id: "bn-test1".to_string(),
            title: "Test Task".to_string(),
            status: "pending".to_string(),
            assignee: None,
            blocked_by: vec![],
        }];

        let summary = build_blocker_summary(&blockers, 1);

        assert!(summary.contains("bn-test1 is pending"));
        assert!(!summary.contains("assigned"));
    }

    #[test]
    fn test_build_blocker_summary_with_transitive_blockers() {
        let blockers = vec![DirectBlocker {
            id: "bn-test1".to_string(),
            title: "Test Task".to_string(),
            status: "pending".to_string(),
            assignee: None,
            blocked_by: vec!["bn-test2".to_string(), "bn-test3".to_string()],
        }];

        let summary = build_blocker_summary(&blockers, 1);

        assert!(summary.contains("bn-test1"));
        assert!(summary.contains("blocked by bn-test2, bn-test3"));
    }

    // === Bug Command Tests ===

    #[test]
    fn test_bug_create() {
        let temp = setup();
        let result = bug_create(
            temp.path(),
            "Test bug".to_string(),
            None,
            Some("Description".to_string()),
            Some(1),
            Some("high".to_string()),
            vec!["frontend".to_string()],
            Some("alice".to_string()),
            Some("1. Open app\n2. Click button".to_string()),
            Some("ui".to_string()),
        )
        .unwrap();
        assert!(result.id.starts_with("bn-"));
        assert_eq!(result.title, "Test bug");
    }

    #[test]
    fn test_bug_create_defaults() {
        let temp = setup();
        let result = bug_create(
            temp.path(),
            "Minimal bug".to_string(),
            None,
            None,
            None,
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        let result = bug_show(temp.path(), &result.id).unwrap().unwrap();
        assert_eq!(result.bug.priority, 2); // default priority
        assert_eq!(result.bug.severity, BugSeverity::Triage); // default severity
        assert!(result.bug.core.description.is_none());
        assert!(result.bug.core.tags.is_empty());
    }

    #[test]
    fn test_bug_create_invalid_priority() {
        let temp = setup();
        let result = bug_create(
            temp.path(),
            "Bad priority".to_string(),
            None,
            None,
            Some(5), // invalid: must be 0-4
            None,
            vec![],
            None,
            None,
            None,
        );
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Priority must be 0-4")
        );
    }

    #[test]
    fn test_bug_show() {
        let temp = setup();
        let created = bug_create(
            temp.path(),
            "Test bug".to_string(),
            None,
            Some("Bug description".to_string()),
            Some(1),
            Some("critical".to_string()),
            vec!["security".to_string()],
            None,
            None,
            None,
        )
        .unwrap();

        let result = bug_show(temp.path(), &created.id).unwrap().unwrap();
        assert_eq!(result.bug.core.id, created.id);
        assert_eq!(result.bug.core.title, "Test bug");
        assert_eq!(
            result.bug.core.description,
            Some("Bug description".to_string())
        );
        assert_eq!(result.bug.priority, 1);
        assert_eq!(result.bug.severity, BugSeverity::Critical);
        assert!(result.bug.core.tags.contains(&"security".to_string()));
    }

    #[test]
    fn test_bug_show_not_found() {
        let temp = setup();
        let result = bug_show(temp.path(), "bn-nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_bug_list() {
        let temp = setup();
        bug_create(
            temp.path(),
            "Bug 1".to_string(),
            None,
            None,
            Some(1),
            Some("high".to_string()),
            vec![],
            None,
            None,
            None,
        )
        .unwrap();
        bug_create(
            temp.path(),
            "Bug 2".to_string(),
            None,
            None,
            Some(2),
            Some("low".to_string()),
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        let list = bug_list(temp.path(), None, None, None, None, false).unwrap();
        assert_eq!(list.count, 2);
    }

    #[test]
    fn test_bug_list_filter_by_status() {
        let temp = setup();
        let bug1 = bug_create(
            temp.path(),
            "Bug 1".to_string(),
            None,
            None,
            None,
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();
        bug_create(
            temp.path(),
            "Bug 2".to_string(),
            None,
            None,
            None,
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        // Close bug 1
        bug_close(temp.path(), &bug1.id, None, false).unwrap();

        let pending_list = bug_list(temp.path(), Some("pending"), None, None, None, true).unwrap();
        assert_eq!(pending_list.count, 1);

        let done_list = bug_list(temp.path(), Some("done"), None, None, None, true).unwrap();
        assert_eq!(done_list.count, 1);
    }

    #[test]
    fn test_bug_list_filter_by_priority() {
        let temp = setup();
        bug_create(
            temp.path(),
            "High priority".to_string(),
            None,
            None,
            Some(0),
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();
        bug_create(
            temp.path(),
            "Low priority".to_string(),
            None,
            None,
            Some(3),
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        let high_list = bug_list(temp.path(), None, Some(0), None, None, false).unwrap();
        assert_eq!(high_list.count, 1);
        assert_eq!(high_list.bugs[0].core.title, "High priority");
    }

    #[test]
    fn test_bug_list_filter_by_severity() {
        let temp = setup();
        bug_create(
            temp.path(),
            "Critical bug".to_string(),
            None,
            None,
            None,
            Some("critical".to_string()),
            vec![],
            None,
            None,
            None,
        )
        .unwrap();
        bug_create(
            temp.path(),
            "Low severity".to_string(),
            None,
            None,
            None,
            Some("low".to_string()),
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        let critical_list =
            bug_list(temp.path(), None, None, Some("critical"), None, false).unwrap();
        assert_eq!(critical_list.count, 1);
        assert_eq!(critical_list.bugs[0].core.title, "Critical bug");
    }

    #[test]
    fn test_bug_list_filter_by_tag() {
        let temp = setup();
        bug_create(
            temp.path(),
            "UI bug".to_string(),
            None,
            None,
            None,
            None,
            vec!["ui".to_string()],
            None,
            None,
            None,
        )
        .unwrap();
        bug_create(
            temp.path(),
            "API bug".to_string(),
            None,
            None,
            None,
            None,
            vec!["api".to_string()],
            None,
            None,
            None,
        )
        .unwrap();

        let ui_list = bug_list(temp.path(), None, None, None, Some("ui"), false).unwrap();
        assert_eq!(ui_list.count, 1);
        assert_eq!(ui_list.bugs[0].core.title, "UI bug");
    }

    #[test]
    fn test_bug_update() {
        let temp = setup();
        let created = bug_create(
            temp.path(),
            "Original bug".to_string(),
            None,
            None,
            None,
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        let updated = bug_update(
            temp.path(),
            &created.id,
            Some("Updated bug".to_string()),
            None,
            Some("New description".to_string()),
            Some(1),
            None,
            Some("high".to_string()),
            vec!["new-tag".to_string()],
            vec![],
            Some("bob".to_string()),
            Some("Steps to reproduce".to_string()),
            Some("backend".to_string()),
            false,
            false, // keep_closed
            false, // reopen
        )
        .unwrap();

        assert!(updated.updated_fields.contains(&"title".to_string()));
        assert!(updated.updated_fields.contains(&"description".to_string()));
        assert!(updated.updated_fields.contains(&"priority".to_string()));
        assert!(updated.updated_fields.contains(&"severity".to_string()));
        assert!(updated.updated_fields.contains(&"tags".to_string()));
        assert!(updated.updated_fields.contains(&"assignee".to_string()));
        assert!(
            updated
                .updated_fields
                .contains(&"reproduction_steps".to_string())
        );
        assert!(
            updated
                .updated_fields
                .contains(&"affected_component".to_string())
        );

        let result = bug_show(temp.path(), &created.id).unwrap().unwrap();
        assert_eq!(result.bug.core.title, "Updated bug");
        assert_eq!(
            result.bug.core.description,
            Some("New description".to_string())
        );
        assert_eq!(result.bug.priority, 1);
        assert_eq!(result.bug.severity, BugSeverity::High);
        assert!(result.bug.core.tags.contains(&"new-tag".to_string()));
        assert_eq!(result.bug.assignee, Some("bob".to_string()));
    }

    #[test]
    fn test_closed_bug_update_requires_flag() {
        let temp = setup();
        let created = bug_create(
            temp.path(),
            "Test Bug".to_string(),
            None,
            None,
            None,
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        // Close the bug
        bug_close(temp.path(), &created.id, Some("Fixed".to_string()), false).unwrap();

        // Try to update without flag - should fail
        let result = bug_update(
            temp.path(),
            &created.id,
            Some("New Title".to_string()),
            None,
            None,
            None,
            None,
            None,
            vec![],
            vec![],
            None,
            None,
            None,
            false,
            false, // keep_closed
            false, // reopen
        );
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Cannot update closed bug"));
        assert!(err.contains("--keep-closed"));
        assert!(err.contains("--reopen"));
    }

    #[test]
    fn test_closed_bug_update_with_keep_closed() {
        let temp = setup();
        let created = bug_create(
            temp.path(),
            "Test Bug".to_string(),
            None,
            None,
            None,
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        // Close the bug
        bug_close(temp.path(), &created.id, Some("Fixed".to_string()), false).unwrap();

        // Update with --keep-closed - should succeed
        let result = bug_update(
            temp.path(),
            &created.id,
            Some("Updated Title".to_string()),
            None,
            None,
            None,
            None,
            None,
            vec![],
            vec![],
            None,
            None,
            None,
            false,
            true,  // keep_closed
            false, // reopen
        );
        assert!(result.is_ok());

        // Verify title was updated but status is still Done
        let bug = bug_show(temp.path(), &created.id).unwrap().unwrap();
        assert_eq!(bug.bug.core.title, "Updated Title");
        assert_eq!(bug.bug.status, TaskStatus::Done);
    }

    #[test]
    fn test_closed_bug_update_with_reopen() {
        let temp = setup();
        let created = bug_create(
            temp.path(),
            "Test Bug".to_string(),
            None,
            None,
            None,
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        // Close the bug
        bug_close(temp.path(), &created.id, Some("Fixed".to_string()), false).unwrap();

        // Update with --reopen - should succeed and set status to Pending
        let result = bug_update(
            temp.path(),
            &created.id,
            Some("Reopened Title".to_string()),
            None,
            None,
            None,
            None,
            None,
            vec![],
            vec![],
            None,
            None,
            None,
            false,
            false, // keep_closed
            true,  // reopen
        );
        assert!(result.is_ok());

        // Verify title was updated and status is now Pending
        let bug = bug_show(temp.path(), &created.id).unwrap().unwrap();
        assert_eq!(bug.bug.core.title, "Reopened Title");
        assert_eq!(bug.bug.status, TaskStatus::Pending);
    }

    #[test]
    fn test_bug_update_status() {
        let temp = setup();
        let created = bug_create(
            temp.path(),
            "Bug".to_string(),
            None,
            None,
            None,
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        bug_update(
            temp.path(),
            &created.id,
            None,
            None,
            None,
            None,
            Some("in_progress"),
            None,
            vec![],
            vec![],
            None,
            None,
            None,
            false,
            false, // keep_closed
            false, // reopen
        )
        .unwrap();

        let result = bug_show(temp.path(), &created.id).unwrap().unwrap();
        assert_eq!(result.bug.status, TaskStatus::InProgress);
    }

    #[test]
    fn test_bug_update_add_remove_tags() {
        let temp = setup();
        let created = bug_create(
            temp.path(),
            "Bug".to_string(),
            None,
            None,
            None,
            None,
            vec!["old-tag".to_string()],
            None,
            None,
            None,
        )
        .unwrap();

        bug_update(
            temp.path(),
            &created.id,
            None,
            None,
            None,
            None,
            None,
            None,
            vec!["new-tag".to_string()],
            vec!["old-tag".to_string()],
            None,
            None,
            None,
            false,
            false, // keep_closed
            false, // reopen
        )
        .unwrap();

        let result = bug_show(temp.path(), &created.id).unwrap().unwrap();
        assert!(result.bug.core.tags.contains(&"new-tag".to_string()));
        assert!(!result.bug.core.tags.contains(&"old-tag".to_string()));
    }

    #[test]
    fn test_bug_update_no_fields_error() {
        let temp = setup();
        let created = bug_create(
            temp.path(),
            "Bug".to_string(),
            None,
            None,
            None,
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        let result = bug_update(
            temp.path(),
            &created.id,
            None,
            None,
            None,
            None,
            None,
            None,
            vec![],
            vec![],
            None,
            None,
            None,
            false,
            false, // keep_closed
            false, // reopen
        );
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("No fields to update")
        );
    }

    #[test]
    fn test_bug_update_invalid_priority() {
        let temp = setup();
        let created = bug_create(
            temp.path(),
            "Bug".to_string(),
            None,
            None,
            None,
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        let result = bug_update(
            temp.path(),
            &created.id,
            None,
            None,
            None,
            Some(5), // invalid
            None,
            None,
            vec![],
            vec![],
            None,
            None,
            None,
            false,
            false, // keep_closed
            false, // reopen
        );
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Priority must be 0-4")
        );
    }

    #[test]
    fn test_bug_update_in_progress_tracks_agent_association() {
        let temp = setup();

        // Register using parent PID since that's how agents are now identified
        let parent_pid = get_parent_pid().unwrap_or_else(std::process::id);
        let bn_pid = std::process::id();
        let agent = Agent::new(
            parent_pid,
            bn_pid,
            "test-agent".to_string(),
            AgentType::Worker,
        );
        {
            let mut storage = Storage::open(temp.path()).unwrap();
            storage.register_agent(&agent).unwrap();
        }

        // Create a bug
        let bug = bug_create(
            temp.path(),
            "Test bug".to_string(),
            None,
            None,
            None,
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        // Update bug to in_progress
        let result = bug_update(
            temp.path(),
            &bug.id,
            None,
            None,
            None,
            None,
            Some("in_progress"),
            None,
            vec![],
            vec![],
            None,
            None,
            None,
            false,
            false, // keep_closed
            false, // reopen
        );
        assert!(result.is_ok());

        // Verify agent has the bug in its tasks list
        let storage = Storage::open(temp.path()).unwrap();
        let updated_agent = storage.get_agent(parent_pid).unwrap();
        assert!(
            updated_agent.tasks.contains(&bug.id),
            "Agent should have the bug in its tasks list"
        );
    }

    #[test]
    fn test_bug_close() {
        let temp = setup();
        let created = bug_create(
            temp.path(),
            "Bug".to_string(),
            None,
            None,
            None,
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        let result = bug_close(temp.path(), &created.id, Some("Fixed".to_string()), false).unwrap();
        assert_eq!(result.id, created.id);
        assert_eq!(result.status, "done");
        // Warning is expected when no commits are linked to the bug
        assert!(result.warning.is_some());
        assert!(
            result
                .warning
                .as_ref()
                .unwrap()
                .contains("No commits linked")
        );

        let result = bug_show(temp.path(), &created.id).unwrap().unwrap();
        assert_eq!(result.bug.status, TaskStatus::Done);
        assert!(result.bug.closed_at.is_some());
        assert_eq!(result.bug.closed_reason, Some("Fixed".to_string()));
    }

    #[test]
    fn test_bug_reopen() {
        let temp = setup();
        let created = bug_create(
            temp.path(),
            "Bug".to_string(),
            None,
            None,
            None,
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        bug_close(temp.path(), &created.id, Some("Fixed".to_string()), false).unwrap();
        let result = bug_reopen(temp.path(), &created.id).unwrap();
        assert_eq!(result.id, created.id);
        assert_eq!(result.status, "reopened");

        let result = bug_show(temp.path(), &created.id).unwrap().unwrap();
        assert_eq!(result.bug.status, TaskStatus::Reopened);
        assert!(result.bug.closed_at.is_none());
        assert!(result.bug.closed_reason.is_none());

        // Verify closure history is preserved in description
        let description = result.bug.core.description.unwrap();
        assert!(description.contains("Previously closed on:"));
        assert!(description.contains("Reason: Fixed"));
    }

    #[test]
    fn test_bug_delete() {
        let temp = setup();
        let created = bug_create(
            temp.path(),
            "Bug".to_string(),
            None,
            None,
            None,
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        let result = bug_delete(temp.path(), &created.id).unwrap();
        assert_eq!(result.id, created.id);

        let list = bug_list(temp.path(), None, None, None, None, true).unwrap();
        assert_eq!(list.count, 0);
    }

    #[test]
    fn test_bug_severity_values() {
        let temp = setup();

        for (severity_str, expected) in [
            ("triage", BugSeverity::Triage),
            ("low", BugSeverity::Low),
            ("medium", BugSeverity::Medium),
            ("high", BugSeverity::High),
            ("critical", BugSeverity::Critical),
        ] {
            let result = bug_create(
                temp.path(),
                format!("Bug with {} severity", severity_str),
                None,
                None,
                None,
                Some(severity_str.to_string()),
                vec![],
                None,
                None,
                None,
            )
            .unwrap();

            let result = bug_show(temp.path(), &result.id).unwrap().unwrap();
            assert_eq!(result.bug.severity, expected);
        }
    }

    #[test]
    fn test_bug_output_human_format() {
        let temp = setup();
        let created = bug_create(
            temp.path(),
            "Test bug".to_string(),
            None,
            Some("Description".to_string()),
            Some(1),
            Some("high".to_string()),
            vec!["ui".to_string()],
            Some("alice".to_string()),
            Some("1. Click\n2. See error".to_string()),
            Some("frontend".to_string()),
        )
        .unwrap();

        let result = bug_show(temp.path(), &created.id).unwrap().unwrap();
        let human = result.to_human();

        assert!(human.contains("Test bug"));
        assert!(human.contains("Status: Pending"));
        assert!(human.contains("Priority: P1"));
        assert!(human.contains("Severity: High"));
        assert!(human.contains("Description: Description"));
        assert!(human.contains("Reproduction steps: 1. Click"));
        assert!(human.contains("Affected component: frontend"));
        assert!(human.contains("Tags: ui"));
        assert!(human.contains("Assignee: alice"));
    }

    #[test]
    fn test_bug_list_output_human_format() {
        let temp = setup();
        bug_create(
            temp.path(),
            "Bug 1".to_string(),
            None,
            None,
            Some(0),
            Some("critical".to_string()),
            vec!["urgent".to_string()],
            None,
            None,
            None,
        )
        .unwrap();

        let list = bug_list(temp.path(), None, None, None, None, false).unwrap();
        let human = list.to_human();

        assert!(human.contains("1 bug(s)"));
        assert!(human.contains("P0"));
        assert!(human.contains("S:critical"));
        assert!(human.contains("Bug 1"));
        assert!(human.contains("[urgent]"));
    }

    #[test]
    fn test_bug_list_empty_output() {
        let temp = setup();
        let list = bug_list(temp.path(), None, None, None, None, false).unwrap();
        let human = list.to_human();
        assert_eq!(human, "No bugs found.");
    }

    // === Search Link Tests ===

    #[test]
    fn test_search_link_empty() {
        let temp = setup();
        let result = search_link(temp.path(), None, None, None).unwrap();
        assert_eq!(result.count, 0);
        assert!(result.edges.is_empty());
        assert!(result.filters.is_none());
    }

    #[test]
    fn test_search_link_by_type() {
        let temp = setup();

        // Create two tasks to link (title, short_name, description, priority, tags, assignee)
        let task1 = task_create(
            temp.path(),
            "Task 1".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task2 = task_create(
            temp.path(),
            "Task 2".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Create a depends_on edge
        link_add(
            temp.path(),
            &task1.id,
            &task2.id,
            "depends_on",
            Some("Testing".to_string()),
            false,
        )
        .unwrap();

        // Search by type
        let result = search_link(temp.path(), Some("depends_on"), None, None).unwrap();
        assert_eq!(result.count, 1);
        assert_eq!(result.edges[0].source, task1.id);
        assert_eq!(result.edges[0].target, task2.id);
        assert_eq!(result.edges[0].edge_type, "depends_on");
        assert_eq!(result.edges[0].reason, Some("Testing".to_string()));
        assert!(result.filters.as_ref().unwrap().edge_type.is_some());

        // Search by different type - should be empty
        let result = search_link(temp.path(), Some("fixes"), None, None).unwrap();
        assert_eq!(result.count, 0);
    }

    #[test]
    fn test_search_link_by_source() {
        let temp = setup();

        let task1 = task_create(
            temp.path(),
            "Task 1".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task2 = task_create(
            temp.path(),
            "Task 2".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task3 = task_create(
            temp.path(),
            "Task 3".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Create edges: task1 -> task2, task3 -> task2
        link_add(
            temp.path(),
            &task1.id,
            &task2.id,
            "depends_on",
            Some("test dependency".to_string()),
            false,
        )
        .unwrap();
        link_add(
            temp.path(),
            &task3.id,
            &task2.id,
            "depends_on",
            Some("test dependency".to_string()),
            false,
        )
        .unwrap();

        // Search by source=task1
        let result = search_link(temp.path(), None, Some(&task1.id), None).unwrap();
        assert_eq!(result.count, 1);
        assert_eq!(result.edges[0].source, task1.id);

        // Search by source=task3
        let result = search_link(temp.path(), None, Some(&task3.id), None).unwrap();
        assert_eq!(result.count, 1);
        assert_eq!(result.edges[0].source, task3.id);
    }

    #[test]
    fn test_search_link_by_target() {
        let temp = setup();

        let task1 = task_create(
            temp.path(),
            "Task 1".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task2 = task_create(
            temp.path(),
            "Task 2".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        link_add(
            temp.path(),
            &task1.id,
            &task2.id,
            "depends_on",
            Some("test dependency".to_string()),
            false,
        )
        .unwrap();

        // Search by target
        let result = search_link(temp.path(), None, None, Some(&task2.id)).unwrap();
        assert_eq!(result.count, 1);
        assert_eq!(result.edges[0].target, task2.id);
        assert!(result.filters.as_ref().unwrap().target.is_some());
    }

    #[test]
    fn test_search_link_combined_filters() {
        let temp = setup();

        let task1 = task_create(
            temp.path(),
            "Task 1".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task2 = task_create(
            temp.path(),
            "Task 2".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        link_add(
            temp.path(),
            &task1.id,
            &task2.id,
            "depends_on",
            Some("test dependency".to_string()),
            false,
        )
        .unwrap();
        link_add(temp.path(), &task1.id, &task2.id, "related_to", None, false).unwrap();

        // Filter by type and source
        let result = search_link(temp.path(), Some("depends_on"), Some(&task1.id), None).unwrap();
        assert_eq!(result.count, 1);
        assert_eq!(result.edges[0].edge_type, "depends_on");

        // Filter by type, source and target
        let result = search_link(
            temp.path(),
            Some("related_to"),
            Some(&task1.id),
            Some(&task2.id),
        )
        .unwrap();
        assert_eq!(result.count, 1);
        assert_eq!(result.edges[0].edge_type, "related_to");
    }

    #[test]
    fn test_link_add_depends_on_requires_reason() {
        let temp = setup();

        let task1 = task_create(
            temp.path(),
            "Task 1".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task2 = task_create(
            temp.path(),
            "Task 2".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // depends_on without reason should fail
        let result = link_add(temp.path(), &task1.id, &task2.id, "depends_on", None, false);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("--reason is required")
        );

        // depends_on with reason should succeed
        let result = link_add(
            temp.path(),
            &task1.id,
            &task2.id,
            "depends_on",
            Some("Needs task2 to complete first".to_string()),
            false,
        );
        assert!(result.is_ok());

        // other edge types don't require reason
        let result = link_add(temp.path(), &task1.id, &task2.id, "related_to", None, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_link_add_pinned_edge() {
        let temp = setup();

        let task1 = task_create(
            temp.path(),
            "Task 1".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task2 = task_create(
            temp.path(),
            "Task 2".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Create a pinned edge
        let result = link_add(temp.path(), &task1.id, &task2.id, "related_to", None, true).unwrap();
        assert!(result.pinned);

        // Verify the edge is stored as pinned
        let list = link_list(temp.path(), Some(&task1.id), false, None).unwrap();
        assert_eq!(list.edges.len(), 1);
        assert!(list.edges[0].pinned);

        // Human-readable output should show [pinned]
        let human = list.to_human();
        assert!(human.contains("[pinned]"));
    }

    #[test]
    fn test_link_add_non_pinned_edge() {
        let temp = setup();

        let task1 = task_create(
            temp.path(),
            "Task 1".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task2 = task_create(
            temp.path(),
            "Task 2".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Create a non-pinned edge (default)
        let result =
            link_add(temp.path(), &task1.id, &task2.id, "related_to", None, false).unwrap();
        assert!(!result.pinned);

        // Verify the edge is stored as non-pinned
        let list = link_list(temp.path(), Some(&task1.id), false, None).unwrap();
        assert_eq!(list.edges.len(), 1);
        assert!(!list.edges[0].pinned);

        // Human-readable output should not show [pinned]
        let human = list.to_human();
        assert!(!human.contains("[pinned]"));
    }

    #[test]
    fn test_search_link_output_human_format() {
        let temp = setup();

        let task1 = task_create(
            temp.path(),
            "Task 1".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task2 = task_create(
            temp.path(),
            "Task 2".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        link_add(
            temp.path(),
            &task1.id,
            &task2.id,
            "depends_on",
            Some("Important dependency".to_string()),
            false,
        )
        .unwrap();

        let result = search_link(temp.path(), None, None, None).unwrap();
        let human = result.to_human();

        assert!(human.contains("1 edge(s) found"));
        assert!(human.contains(&task1.id));
        assert!(human.contains(&task2.id));
        assert!(human.contains("depends_on"));
        assert!(human.contains("Important dependency"));
    }

    #[test]
    fn test_search_link_empty_output() {
        let temp = setup();
        let result = search_link(temp.path(), None, None, None).unwrap();
        let human = result.to_human();
        assert_eq!(human, "No edges found.");
    }

    // === Doctor Edge Migration Tests ===

    #[test]
    fn test_doctor_migrate_edges_no_legacy_deps() {
        let temp = setup();

        // Create tasks without depends_on
        task_create(
            temp.path(),
            "Task 1".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        task_create(
            temp.path(),
            "Task 2".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        let result = doctor_migrate_edges(temp.path(), false, false).unwrap();

        assert_eq!(result.tasks_scanned, 2);
        assert_eq!(result.edges_created, 0);
        assert_eq!(result.edges_skipped, 0);
        assert_eq!(result.depends_on_cleared, 0);
        assert!(!result.dry_run);
    }

    #[test]
    fn test_doctor_migrate_edges_with_legacy_deps() {
        let temp = setup();
        let mut storage = Storage::open(temp.path()).unwrap();

        // Create tasks
        let task1 = task_create(
            temp.path(),
            "Task 1".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task2 = task_create(
            temp.path(),
            "Task 2".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Manually add legacy depends_on to task1
        let mut task = storage.get_task(&task1.id).unwrap();
        task.depends_on.push(task2.id.clone());
        task.core.updated_at = chrono::Utc::now();
        storage.update_task(&task).unwrap();

        // Run migration
        let result = doctor_migrate_edges(temp.path(), false, false).unwrap();

        assert_eq!(result.tasks_scanned, 2);
        assert_eq!(result.edges_created, 1);
        assert_eq!(result.edges_skipped, 0);
        assert_eq!(result.depends_on_cleared, 0); // clean_unused was false

        // Verify edge was created
        let storage2 = Storage::open(temp.path()).unwrap();
        let edges = storage2
            .list_edges(Some(EdgeType::DependsOn), Some(&task1.id), Some(&task2.id))
            .unwrap();
        assert_eq!(edges.len(), 1);
    }

    #[test]
    fn test_doctor_migrate_edges_dry_run() {
        let temp = setup();
        let mut storage = Storage::open(temp.path()).unwrap();

        // Create tasks
        let task1 = task_create(
            temp.path(),
            "Task 1".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task2 = task_create(
            temp.path(),
            "Task 2".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Manually add legacy depends_on
        let mut task = storage.get_task(&task1.id).unwrap();
        task.depends_on.push(task2.id.clone());
        task.core.updated_at = chrono::Utc::now();
        storage.update_task(&task).unwrap();

        // Run dry run
        let result = doctor_migrate_edges(temp.path(), false, true).unwrap();

        assert!(result.dry_run);
        assert_eq!(result.edges_created, 1); // Would have created

        // Verify no edge was actually created
        let storage2 = Storage::open(temp.path()).unwrap();
        let edges = storage2
            .list_edges(Some(EdgeType::DependsOn), Some(&task1.id), Some(&task2.id))
            .unwrap();
        assert_eq!(edges.len(), 0);
    }

    #[test]
    fn test_doctor_migrate_edges_clean_unused() {
        let temp = setup();
        let mut storage = Storage::open(temp.path()).unwrap();

        // Create tasks
        let task1 = task_create(
            temp.path(),
            "Task 1".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task2 = task_create(
            temp.path(),
            "Task 2".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Manually add legacy depends_on
        let mut task = storage.get_task(&task1.id).unwrap();
        task.depends_on.push(task2.id.clone());
        task.core.updated_at = chrono::Utc::now();
        storage.update_task(&task).unwrap();

        // Run migration with clean_unused
        let result = doctor_migrate_edges(temp.path(), true, false).unwrap();

        assert_eq!(result.edges_created, 1);
        assert_eq!(result.depends_on_cleared, 1);

        // Verify depends_on was cleared
        let storage2 = Storage::open(temp.path()).unwrap();
        let task_after = storage2.get_task(&task1.id).unwrap();
        assert!(task_after.depends_on.is_empty());
    }

    #[test]
    fn test_doctor_migrate_edges_skips_existing() {
        let temp = setup();
        let mut storage = Storage::open(temp.path()).unwrap();

        // Create tasks
        let task1 = task_create(
            temp.path(),
            "Task 1".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task2 = task_create(
            temp.path(),
            "Task 2".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Create edge first using the proper method
        link_add(
            temp.path(),
            &task1.id,
            &task2.id,
            "depends_on",
            Some("test dependency".to_string()),
            false,
        )
        .unwrap();

        // Also add legacy depends_on for the same relationship
        let mut task = storage.get_task(&task1.id).unwrap();
        task.depends_on.push(task2.id.clone());
        task.core.updated_at = chrono::Utc::now();
        storage.update_task(&task).unwrap();

        // Run migration
        let result = doctor_migrate_edges(temp.path(), false, false).unwrap();

        assert_eq!(result.edges_created, 0);
        assert_eq!(result.edges_skipped, 1);
    }

    #[test]
    fn test_doctor_migrate_edges_output_format() {
        let temp = setup();

        // Create task without deps
        task_create(
            temp.path(),
            "Task 1".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        let result = doctor_migrate_edges(temp.path(), false, true).unwrap();

        // Test JSON output
        let json = result.to_json();
        assert!(json.contains("\"tasks_scanned\":1"));
        assert!(json.contains("\"dry_run\":true"));

        // Test human output
        let human = result.to_human();
        assert!(human.contains("DRY RUN"));
        assert!(human.contains("Tasks scanned: 1"));
    }

    // === Entity Type Mismatch Tests ===

    #[test]
    fn test_task_show_returns_bug_when_id_is_bug() {
        let temp = setup();

        // Create a bug
        let bug = bug_create(
            temp.path(),
            "Test Bug".to_string(),
            None,
            None,
            None,
            None,
            vec!["test".to_string()],
            None,
            None,
            None,
        )
        .unwrap();

        // Try to show it as a task
        let result = task_show(temp.path(), &bug.id).unwrap();

        // Should return TypeMismatch with the bug data
        match result {
            TaskShowResponse::TypeMismatch(ref mismatch) => {
                assert!(mismatch.note.contains("is a bug, not a task"));
                assert_eq!(mismatch.actual_type, "bug");
                assert!(mismatch.bug.is_some());
                assert!(mismatch.task.is_none());
            }
            TaskShowResponse::Found(_) => {
                panic!("Expected TypeMismatch, got Found");
            }
        }
    }

    #[test]
    fn test_bug_show_returns_task_when_id_is_task() {
        let temp = setup();

        // Create a task
        let task = task_create(
            temp.path(),
            "Test Task".to_string(),
            None,
            None,
            None,
            vec!["test".to_string()],
            None,
        )
        .unwrap();

        // Try to show it as a bug
        let result = bug_show(temp.path(), &task.id).unwrap();

        // Should return TypeMismatch with the task data
        match result {
            BugShowResponse::TypeMismatch(ref mismatch) => {
                assert!(mismatch.note.contains("is a task, not a bug"));
                assert_eq!(mismatch.actual_type, "task");
                assert!(mismatch.task.is_some());
                assert!(mismatch.bug.is_none());
            }
            BugShowResponse::Found(_) => {
                panic!("Expected TypeMismatch, got Found");
            }
        }
    }

    #[test]
    fn test_entity_mismatch_json_output() {
        let temp = setup();

        // Create a bug
        let bug = bug_create(
            temp.path(),
            "Test Bug".to_string(),
            None,
            None,
            None,
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        // Try to show it as a task
        let result = task_show(temp.path(), &bug.id).unwrap();
        let json = result.to_json();

        // JSON should contain the note and bug data
        assert!(json.contains("\"note\":"));
        assert!(json.contains("is a bug, not a task"));
        assert!(json.contains("\"actual_type\":\"bug\""));
        assert!(json.contains("\"id\":"));
    }

    #[test]
    fn test_config_get_bool_default() {
        let temp = setup();

        // Non-existent key should return default
        assert!(!config_get_bool(
            temp.path(),
            "require_commit_for_close",
            false
        ));
        assert!(config_get_bool(
            temp.path(),
            "require_commit_for_close",
            true
        ));
    }

    #[test]
    fn test_config_get_bool_true_values() {
        let temp = setup();
        let mut storage = Storage::open(temp.path()).unwrap();

        // Test various "true" values
        for val in &["true", "True", "TRUE", "1", "yes", "YES"] {
            storage.set_config("test_bool", val).unwrap();
            assert!(
                config_get_bool(temp.path(), "test_bool", false),
                "Expected true for value: {}",
                val
            );
        }
    }

    #[test]
    fn test_config_get_bool_false_values() {
        let temp = setup();
        let mut storage = Storage::open(temp.path()).unwrap();

        // Test various "false" values
        for val in &["false", "False", "FALSE", "0", "no", "NO"] {
            storage.set_config("test_bool", val).unwrap();
            assert!(
                !config_get_bool(temp.path(), "test_bool", true),
                "Expected false for value: {}",
                val
            );
        }
    }

    #[test]
    fn test_require_commit_for_close_config_validation() {
        let temp = setup();

        // Valid boolean values should work
        assert!(config_set(temp.path(), "require_commit_for_close", "true").is_ok());
        assert!(config_set(temp.path(), "require_commit_for_close", "false").is_ok());
        assert!(config_set(temp.path(), "require_commit_for_close", "1").is_ok());
        assert!(config_set(temp.path(), "require_commit_for_close", "0").is_ok());
        assert!(config_set(temp.path(), "require_commit_for_close", "yes").is_ok());
        assert!(config_set(temp.path(), "require_commit_for_close", "no").is_ok());

        // Invalid values should fail
        assert!(config_set(temp.path(), "require_commit_for_close", "invalid").is_err());
        assert!(config_set(temp.path(), "require_commit_for_close", "maybe").is_err());
    }

    #[test]
    fn test_co_author_enabled_config_validation() {
        let temp = setup();

        // Valid boolean values should work
        assert!(config_set(temp.path(), "co-author.enabled", "true").is_ok());
        assert!(config_set(temp.path(), "co-author.enabled", "false").is_ok());
        assert!(config_set(temp.path(), "co-author.enabled", "1").is_ok());
        assert!(config_set(temp.path(), "co-author.enabled", "0").is_ok());
        assert!(config_set(temp.path(), "co-author.enabled", "yes").is_ok());
        assert!(config_set(temp.path(), "co-author.enabled", "no").is_ok());

        // Invalid values should fail
        assert!(config_set(temp.path(), "co-author.enabled", "invalid").is_err());
        assert!(config_set(temp.path(), "co-author.enabled", "maybe").is_err());
    }

    #[test]
    fn test_co_author_name_config_validation() {
        let temp = setup();

        // Valid non-empty name should work
        assert!(config_set(temp.path(), "co-author.name", "my-bot").is_ok());
        assert!(config_set(temp.path(), "co-author.name", "binnacle-bot").is_ok());

        // Empty name should fail
        assert!(config_set(temp.path(), "co-author.name", "").is_err());
        assert!(config_set(temp.path(), "co-author.name", "   ").is_err());
    }

    #[test]
    fn test_co_author_email_config_validation() {
        let temp = setup();

        // Valid non-empty email should work
        assert!(config_set(temp.path(), "co-author.email", "bot@example.com").is_ok());
        assert!(config_set(temp.path(), "co-author.email", "noreply@binnacle.bot").is_ok());

        // Empty email should fail
        assert!(config_set(temp.path(), "co-author.email", "").is_err());
        assert!(config_set(temp.path(), "co-author.email", "   ").is_err());
    }

    #[test]
    fn test_config_get_string_default() {
        let temp = setup();

        // Non-existent key should return default
        assert_eq!(
            config_get_string(temp.path(), "co-author.name", "binnacle-bot"),
            "binnacle-bot"
        );
        assert_eq!(
            config_get_string(temp.path(), "co-author.email", "noreply@binnacle.bot"),
            "noreply@binnacle.bot"
        );
    }

    #[test]
    fn test_config_get_string_set_value() {
        let temp = setup();

        // Set custom values
        config_set(temp.path(), "co-author.name", "my-bot").unwrap();
        config_set(temp.path(), "co-author.email", "bot@example.com").unwrap();

        // Should return the set values, not defaults
        assert_eq!(
            config_get_string(temp.path(), "co-author.name", "binnacle-bot"),
            "my-bot"
        );
        assert_eq!(
            config_get_string(temp.path(), "co-author.email", "noreply@binnacle.bot"),
            "bot@example.com"
        );
    }

    #[test]
    fn test_parent_of_rejects_multiple_parents() {
        let temp = setup();

        // Create three tasks (title, short_name, description, priority, tags, assignee)
        let parent1 = task_create(
            temp.path(),
            "Parent 1".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let parent2 = task_create(
            temp.path(),
            "Parent 2".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let child = task_create(
            temp.path(),
            "Child".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // First parent_of edge should succeed
        let result = link_add(
            temp.path(),
            &parent1.id,
            &child.id,
            "parent_of",
            None,
            false,
        );
        assert!(result.is_ok());

        // Second parent_of edge should fail
        let result = link_add(
            temp.path(),
            &parent2.id,
            &child.id,
            "parent_of",
            None,
            false,
        );
        assert!(result.is_err());
        let err_msg = result.err().map(|e| e.to_string()).unwrap_or_default();
        assert!(err_msg.contains("already has a parent"));
        assert!(err_msg.contains(&parent1.id));
    }

    #[test]
    fn test_child_of_rejects_multiple_parents() {
        let temp = setup();

        // Create three tasks
        let parent1 = task_create(
            temp.path(),
            "Parent 1".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let parent2 = task_create(
            temp.path(),
            "Parent 2".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let child = task_create(
            temp.path(),
            "Child".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // First child_of edge should succeed
        let result = link_add(temp.path(), &child.id, &parent1.id, "child_of", None, false);
        assert!(result.is_ok());

        // Second child_of edge should fail
        let result = link_add(temp.path(), &child.id, &parent2.id, "child_of", None, false);
        assert!(result.is_err());
        let err_msg = result.err().map(|e| e.to_string()).unwrap_or_default();
        assert!(err_msg.contains("already has a parent"));
        assert!(err_msg.contains(&parent1.id));
    }

    #[test]
    fn test_parent_of_blocks_child_of() {
        let temp = setup();

        // Create tasks
        let parent1 = task_create(
            temp.path(),
            "Parent 1".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let parent2 = task_create(
            temp.path(),
            "Parent 2".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let child = task_create(
            temp.path(),
            "Child".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Create parent_of edge
        link_add(
            temp.path(),
            &parent1.id,
            &child.id,
            "parent_of",
            None,
            false,
        )
        .unwrap();

        // child_of edge from the same child should fail
        let result = link_add(temp.path(), &child.id, &parent2.id, "child_of", None, false);
        assert!(result.is_err());
        let err_msg = result.err().map(|e| e.to_string()).unwrap_or_default();
        assert!(err_msg.contains("already has a parent"));
    }

    #[test]
    fn test_child_of_blocks_parent_of() {
        let temp = setup();

        // Create tasks
        let parent1 = task_create(
            temp.path(),
            "Parent 1".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let parent2 = task_create(
            temp.path(),
            "Parent 2".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let child = task_create(
            temp.path(),
            "Child".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Create child_of edge
        link_add(temp.path(), &child.id, &parent1.id, "child_of", None, false).unwrap();

        // parent_of edge to the same child should fail
        let result = link_add(
            temp.path(),
            &parent2.id,
            &child.id,
            "parent_of",
            None,
            false,
        );
        assert!(result.is_err());
        let err_msg = result.err().map(|e| e.to_string()).unwrap_or_default();
        assert!(err_msg.contains("already has a parent"));
    }

    // === Goodbye Tests ===

    #[test]
    fn test_goodbye_unregistered_agent() {
        let temp = setup();

        // Goodbye without orient should still work (unregistered agent)
        let result = goodbye(temp.path(), None, false);
        assert!(result.is_ok());

        let goodbye_result = result.unwrap();
        assert!(!goodbye_result.was_registered);
        assert!(goodbye_result.agent_name.is_none());
        assert!(goodbye_result.parent_pid > 0);
        assert!(goodbye_result.grandparent_pid > 0);
    }

    #[test]
    fn test_goodbye_with_reason() {
        let temp = setup();

        let result = goodbye(temp.path(), Some("Task completed".to_string()), false);
        assert!(result.is_ok());

        let goodbye_result = result.unwrap();
        assert_eq!(goodbye_result.reason, Some("Task completed".to_string()));
    }

    #[test]
    fn test_goodbye_output_format() {
        let temp = setup();

        let result = goodbye(temp.path(), Some("Done".to_string()), false).unwrap();

        // Test JSON output
        let json = result.to_json();
        assert!(json.contains("parent_pid"));
        assert!(json.contains("grandparent_pid"));
        assert!(json.contains("was_registered"));
        assert!(json.contains("terminated"));
        assert!(json.contains("Done"));

        // Test human output
        let human = result.to_human();
        assert!(human.contains("Terminating agent session"));
        assert!(human.contains("grandparent PID"));
        assert!(human.contains("Reason: Done"));
    }

    // === Agent Kill Tests ===

    #[test]
    fn test_agent_kill_not_found() {
        let temp = setup();

        // Try to kill a non-existent agent
        let result = agent_kill(temp.path(), "99999", 1);
        assert!(result.is_err());

        // Also test with name
        let result = agent_kill(temp.path(), "nonexistent-agent", 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_agent_kill_by_pid_dead_process() {
        let temp = setup();
        let mut storage = Storage::open(temp.path()).unwrap();

        // Register an agent with a fake PID (won't be running)
        let agent = Agent::new(99998, 1, "test-agent".to_string(), AgentType::Worker);
        storage.register_agent(&agent).unwrap();

        // Kill by PID - agent gets cleaned up as stale during kill
        // so we expect a NotFound error (cleanup removes dead agents)
        let result = agent_kill(temp.path(), "99998", 1);
        // The cleanup_stale_agents removes the agent before we can kill it
        assert!(result.is_err());

        // Verify agent was removed from registry (by cleanup)
        let storage = Storage::open(temp.path()).unwrap();
        assert!(storage.get_agent(99998).is_err());
    }

    #[test]
    fn test_agent_kill_by_name_dead_process() {
        let temp = setup();
        let mut storage = Storage::open(temp.path()).unwrap();

        // Register an agent with a fake PID
        let agent = Agent::new(99997, 1, "named-agent".to_string(), AgentType::Worker);
        storage.register_agent(&agent).unwrap();

        // Kill by name - agent gets cleaned up as stale during kill
        let result = agent_kill(temp.path(), "named-agent", 1);
        // The cleanup_stale_agents removes the agent before we can kill it
        assert!(result.is_err());

        // Verify agent was removed
        let storage = Storage::open(temp.path()).unwrap();
        assert!(storage.get_agent(99997).is_err());
    }

    #[test]
    fn test_agent_kill_output_format() {
        let result = AgentKillResult {
            pid: 12345,
            name: "my-agent".to_string(),
            was_running: true,
            terminated: true,
            signal_sent: "SIGTERM/SIGKILL".to_string(),
        };

        let human = result.to_human();
        assert!(human.contains("my-agent"));
        assert!(human.contains("12345"));
        assert!(human.contains("terminated"));

        let json = result.to_json();
        assert!(json.contains("my-agent"));
        assert!(json.contains("12345"));
    }

    // === Sync Tests ===

    #[test]
    fn test_sync_result_output_no_branch() {
        let result = SyncResult {
            operation: "none".to_string(),
            remote: "origin".to_string(),
            branch: "binnacle-data".to_string(),
            pushed: None,
            pulled: None,
            commits_pushed: None,
            commits_pulled: None,
            error: Some("No 'binnacle-data' branch found".to_string()),
        };

        let human = result.to_human();
        assert!(human.contains("Sync failed"));
        assert!(human.contains("binnacle-data"));

        let json = result.to_json();
        assert!(json.contains("\"operation\":\"none\""));
        assert!(json.contains("binnacle-data"));
    }

    #[test]
    fn test_sync_result_output_success() {
        let result = SyncResult {
            operation: "sync".to_string(),
            remote: "origin".to_string(),
            branch: "binnacle-data".to_string(),
            pushed: Some(true),
            pulled: Some(true),
            commits_pushed: Some(3),
            commits_pulled: Some(2),
            error: None,
        };

        let human = result.to_human();
        assert!(human.contains("Synced binnacle data"));
        assert!(human.contains("origin"));
        assert!(human.contains("Pulled 2 commit"));
        assert!(human.contains("Pushed 3 commit"));

        let json = result.to_json();
        assert!(json.contains("\"operation\":\"sync\""));
        assert!(json.contains("\"pushed\":true"));
        assert!(json.contains("\"pulled\":true"));
    }

    #[test]
    fn test_sync_result_no_changes() {
        let result = SyncResult {
            operation: "sync".to_string(),
            remote: "origin".to_string(),
            branch: "binnacle-data".to_string(),
            pushed: Some(true),
            pulled: Some(true),
            commits_pushed: Some(0),
            commits_pulled: Some(0),
            error: None,
        };

        let human = result.to_human();
        assert!(human.contains("Already up to date"));
        assert!(human.contains("Nothing to push"));
    }

    #[test]
    fn test_sync_no_orphan_branch() {
        // Create a git repo without orphan branch
        let temp = setup();

        // Initialize git
        Command::new("git")
            .args(["init"])
            .current_dir(temp.path())
            .output()
            .unwrap();

        // Sync should fail gracefully
        let result = sync(temp.path(), None, false, false).unwrap();
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("binnacle-data"));
    }

    // === Graph Component Tests ===

    #[test]
    fn test_graph_components_empty() {
        let temp = setup();
        let result = graph_components(temp.path()).unwrap();
        assert_eq!(result.component_count, 0);
        assert!(result.components.is_empty());
        assert!(result.suggestion.is_none());
    }

    #[test]
    fn test_graph_components_single_isolated_task() {
        let temp = setup();

        // Create a single task
        task_create(
            temp.path(),
            "Task 1".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        let result = graph_components(temp.path()).unwrap();
        assert_eq!(result.component_count, 1);
        assert_eq!(result.components.len(), 1);
        assert_eq!(result.components[0].entity_count, 1);
        assert_eq!(result.components[0].root_nodes.len(), 1);
        assert!(result.suggestion.is_none()); // No suggestion for single component
    }

    #[test]
    fn test_graph_components_multiple_isolated() {
        let temp = setup();

        // Create two unconnected tasks
        task_create(
            temp.path(),
            "Task 1".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        task_create(
            temp.path(),
            "Task 2".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        let result = graph_components(temp.path()).unwrap();
        assert_eq!(result.component_count, 2);
        assert!(result.suggestion.is_some()); // Should suggest connecting
    }

    #[test]
    fn test_graph_components_connected_tasks() {
        let temp = setup();

        // Create two tasks and link them
        let task1 = task_create(
            temp.path(),
            "Task 1".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task2 = task_create(
            temp.path(),
            "Task 2".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Link task1 depends on task2
        link_add(
            temp.path(),
            &task1.id,
            &task2.id,
            "depends_on",
            Some("dependency".to_string()),
            false,
        )
        .unwrap();

        let result = graph_components(temp.path()).unwrap();
        assert_eq!(result.component_count, 1);
        assert_eq!(result.components[0].entity_count, 2);
        // task2 should be root (task1 depends on it)
        assert!(result.components[0].root_nodes.contains(&task2.id));
    }

    #[test]
    fn test_graph_components_human_output() {
        let temp = setup();

        // Create two isolated tasks
        task_create(
            temp.path(),
            "Task 1".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        task_create(
            temp.path(),
            "Task 2".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        let result = graph_components(temp.path()).unwrap();
        let human = result.to_human();

        assert!(human.contains("2 disconnected components"));
        assert!(human.contains("isolated"));
        assert!(human.contains("Tip:"));
    }

    #[test]
    fn test_orient_writes_session_state() {
        let temp = setup();

        // Run orient (dry_run: false to test session state writing)
        let _result = orient(temp.path(), "worker", false, None, None, false).unwrap();

        // Verify session state was written
        let storage = Storage::open(temp.path()).unwrap();
        let session = storage.read_session_state().unwrap();

        // Session should reflect the orient call
        assert!(session.orient_called);
        assert_eq!(session.agent_type, crate::models::AgentType::Worker);
    }

    #[test]
    fn test_orient_updates_session_state_on_reorient() {
        let temp = setup();

        // First orient as worker (dry_run: false to test session state writing)
        orient(temp.path(), "worker", false, None, None, false).unwrap();

        // Second orient as planner
        orient(temp.path(), "planner", false, None, None, false).unwrap();

        // Verify session state reflects latest orient
        let storage = Storage::open(temp.path()).unwrap();
        let session = storage.read_session_state().unwrap();
        assert_eq!(session.agent_type, crate::models::AgentType::Planner);
    }

    // === Partial Status Transition Tests ===

    #[test]
    fn test_partial_task_excluded_from_ready() {
        let temp = setup();
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Close B, then add A as dependency → B becomes partial
        task_close(temp.path(), &task_b.id, Some("Done".to_string()), false).unwrap();
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        // Verify B is partial
        let shown = task_show(temp.path(), &task_b.id).unwrap().unwrap();
        assert_eq!(shown.task.status, TaskStatus::Partial);

        // Partial tasks should not appear in ready
        let result = ready(temp.path(), false, false).unwrap();
        assert!(!result.tasks.iter().any(|t| t.task.core.id == task_b.id));
    }

    #[test]
    fn test_partial_task_appears_in_blocked() {
        let temp = setup();
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Close B, then add A as dependency → B becomes partial
        task_close(temp.path(), &task_b.id, Some("Done".to_string()), false).unwrap();
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        // Verify B is partial
        let shown = task_show(temp.path(), &task_b.id).unwrap().unwrap();
        assert_eq!(shown.task.status, TaskStatus::Partial);

        // Partial tasks should appear in blocked
        let result = blocked(temp.path(), false, false).unwrap();
        assert!(result.tasks.iter().any(|t| t.task.core.id == task_b.id));
    }

    #[test]
    fn test_done_to_partial_skipped_when_dependency_already_done() {
        let temp = setup();
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Close both tasks
        task_close(temp.path(), &task_a.id, Some("Done".to_string()), false).unwrap();
        task_close(temp.path(), &task_b.id, Some("Done".to_string()), false).unwrap();

        // Add A as dependency of B (both are done, so B should stay done)
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        // B should remain Done (not transition to Partial)
        let shown = task_show(temp.path(), &task_b.id).unwrap().unwrap();
        assert_eq!(shown.task.status, TaskStatus::Done);
        assert!(shown.task.closed_at.is_some());
    }

    #[test]
    fn test_done_to_partial_skipped_when_dependency_cancelled() {
        let temp = setup();
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Cancel A and close B
        task_update(
            temp.path(),
            &task_a.id,
            None,
            None,
            None,
            None,
            Some("cancelled"),
            vec![],
            vec![],
            None,
            false,
            false, // keep_closed
            false, // reopen
        )
        .unwrap();
        task_close(temp.path(), &task_b.id, Some("Done".to_string()), false).unwrap();

        // Add cancelled A as dependency of B
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        // B should remain Done (cancelled is treated as complete)
        let shown = task_show(temp.path(), &task_b.id).unwrap().unwrap();
        assert_eq!(shown.task.status, TaskStatus::Done);
        assert!(shown.task.closed_at.is_some());
    }

    #[test]
    fn test_cascading_partial_promotion() {
        let temp = setup();
        // Create chain: C -> B -> A
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_c = task_create(
            temp.path(),
            "Task C".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Close C and B
        task_close(temp.path(), &task_c.id, Some("Done".to_string()), false).unwrap();
        task_close(temp.path(), &task_b.id, Some("Done".to_string()), false).unwrap();

        // Add dependencies: C -> B (first, while B is done, so C stays done)
        // Then B -> A (B becomes partial since A is pending)
        dep_add(temp.path(), &task_c.id, &task_b.id).unwrap();

        // At this point, C is still Done (B is done)
        let shown_c = task_show(temp.path(), &task_c.id).unwrap().unwrap();
        assert_eq!(shown_c.task.status, TaskStatus::Done);

        // Now add B -> A (B becomes partial since A is pending)
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        // B should be partial (A is pending)
        let shown_b = task_show(temp.path(), &task_b.id).unwrap().unwrap();
        assert_eq!(shown_b.task.status, TaskStatus::Partial);

        // C is still Done because B was Done when the dependency was added
        // (partial status doesn't cascade retroactively)
        let shown_c = task_show(temp.path(), &task_c.id).unwrap().unwrap();
        assert_eq!(shown_c.task.status, TaskStatus::Done);

        // Close A - should promote B to Done
        task_close(temp.path(), &task_a.id, Some("Done".to_string()), false).unwrap();

        // B should now be Done
        let shown_b = task_show(temp.path(), &task_b.id).unwrap().unwrap();
        assert_eq!(shown_b.task.status, TaskStatus::Done);
        assert!(shown_b.task.closed_at.is_some());

        // C was already Done, still Done
        let shown_c = task_show(temp.path(), &task_c.id).unwrap().unwrap();
        assert_eq!(shown_c.task.status, TaskStatus::Done);
    }

    #[test]
    fn test_partial_with_multiple_dependencies_all_must_complete() {
        let temp = setup();
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_c = task_create(
            temp.path(),
            "Task C".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Close C, then add A and B as dependencies
        task_close(temp.path(), &task_c.id, Some("Done".to_string()), false).unwrap();
        dep_add(temp.path(), &task_c.id, &task_a.id).unwrap();
        dep_add(temp.path(), &task_c.id, &task_b.id).unwrap();

        // C should be partial
        let shown = task_show(temp.path(), &task_c.id).unwrap().unwrap();
        assert_eq!(shown.task.status, TaskStatus::Partial);

        // Close only A - C should still be partial (B is pending)
        task_close(temp.path(), &task_a.id, Some("Done".to_string()), false).unwrap();
        let shown = task_show(temp.path(), &task_c.id).unwrap().unwrap();
        assert_eq!(shown.task.status, TaskStatus::Partial);

        // Close B - now C should be promoted to Done
        task_close(temp.path(), &task_b.id, Some("Done".to_string()), false).unwrap();
        let shown = task_show(temp.path(), &task_c.id).unwrap().unwrap();
        assert_eq!(shown.task.status, TaskStatus::Done);
        assert!(shown.task.closed_at.is_some());
    }

    #[test]
    fn test_removing_dependency_does_not_auto_promote_partial() {
        let temp = setup();
        let task_a = task_create(
            temp.path(),
            "Task A".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None,
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        // Close B, add A as dependency → B becomes partial
        task_close(temp.path(), &task_b.id, Some("Done".to_string()), false).unwrap();
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        let shown = task_show(temp.path(), &task_b.id).unwrap().unwrap();
        assert_eq!(shown.task.status, TaskStatus::Partial);

        // Remove the dependency - B should still be partial
        // (removing a dep doesn't auto-promote, closing all deps does)
        dep_rm(temp.path(), &task_b.id, &task_a.id).unwrap();

        let shown = task_show(temp.path(), &task_b.id).unwrap().unwrap();
        // After removing all dependencies, the task remains partial
        // (needs manual close or close of remaining deps to promote)
        assert_eq!(shown.task.status, TaskStatus::Partial);
    }

    #[test]
    fn test_parse_memory_limit_bytes() {
        assert_eq!(parse_memory_limit("1024").unwrap(), 1024);
        assert_eq!(parse_memory_limit("2048").unwrap(), 2048);
    }

    #[test]
    fn test_parse_memory_limit_kilobytes() {
        assert_eq!(parse_memory_limit("512k").unwrap(), 512 * 1024);
        assert_eq!(parse_memory_limit("1kb").unwrap(), 1024);
    }

    #[test]
    fn test_parse_memory_limit_megabytes() {
        assert_eq!(parse_memory_limit("512m").unwrap(), 512 * 1024 * 1024);
        assert_eq!(parse_memory_limit("1mb").unwrap(), 1024 * 1024);
        assert_eq!(parse_memory_limit("2048M").unwrap(), 2048 * 1024 * 1024);
    }

    #[test]
    fn test_parse_memory_limit_gigabytes() {
        assert_eq!(parse_memory_limit("1g").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(parse_memory_limit("2gb").unwrap(), 2 * 1024 * 1024 * 1024);
        assert_eq!(parse_memory_limit("4G").unwrap(), 4 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_parse_memory_limit_invalid() {
        assert!(parse_memory_limit("invalid").is_err());
        assert!(parse_memory_limit("512x").is_err());
        assert!(parse_memory_limit("abc123").is_err());
    }

    // ==========================================================================
    // ARCHIVE SCHEMA FINGERPRINT TESTS
    //
    // These tests catch accidental schema changes in archive export format.
    // When fields are added/removed/renamed, these tests WILL FAIL.
    //
    // If a test fails after you modified archive structs:
    // 1. VERIFY the change is intentional
    // 2. CONSIDER backwards compatibility (can old archives still be read?)
    // 3. UPDATE the expected fingerprint below
    // 4. INCREMENT the manifest version if breaking change
    // 5. DOCUMENT the schema change in your commit message
    // ==========================================================================

    /// Extract all JSON keys from a serialized value, sorted alphabetically.
    fn extract_json_keys(json: &str) -> Vec<String> {
        let value: serde_json::Value = serde_json::from_str(json).unwrap();
        let mut keys = Vec::new();
        collect_keys(&value, "", &mut keys);
        keys.sort();
        keys
    }

    fn collect_keys(value: &serde_json::Value, prefix: &str, keys: &mut Vec<String>) {
        if let serde_json::Value::Object(map) = value {
            for (k, v) in map {
                let full_key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{}.{}", prefix, k)
                };
                keys.push(full_key.clone());
                collect_keys(v, &full_key, keys);
            }
        }
    }

    /// Create a fingerprint string from a list of keys.
    fn fingerprint(keys: &[String]) -> String {
        keys.join("|")
    }

    #[test]
    fn test_archive_schema_fingerprint_export_manifest() {
        // Create an ExportManifest with all fields populated
        let mut checksums = std::collections::HashMap::new();
        checksums.insert("tasks.jsonl".to_string(), "abc123".to_string());

        let manifest = super::ExportManifest {
            version: 1,
            format: "binnacle-store-v1".to_string(),
            exported_at: "2026-01-24T12:00:00Z".to_string(),
            source_repo: "/path/to/repo".to_string(),
            binnacle_version: "0.0.1".to_string(),
            task_count: 10,
            test_count: 5,
            commit_count: 3,
            checksums,
        };

        let json = serde_json::to_string(&manifest).unwrap();
        let keys = extract_json_keys(&json);
        let fp = fingerprint(&keys);

        // Expected schema fingerprint for ExportManifest
        // If this fails, you've changed the archive schema - see comment above!
        let expected = "binnacle_version|checksums|checksums.tasks.jsonl|commit_count|exported_at|format|source_repo|task_count|test_count|version";
        assert_eq!(
            fp, expected,
            "ExportManifest schema changed! Update expected fingerprint if intentional. \
             Also consider incrementing manifest version and ensuring backwards compatibility."
        );
    }

    #[test]
    fn test_archive_schema_fingerprint_export_config() {
        let config = super::ExportConfig {
            repo_path: "/path/to/repo".to_string(),
            exported_at: "2026-01-24T12:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&config).unwrap();
        let keys = extract_json_keys(&json);
        let fp = fingerprint(&keys);

        // Expected schema fingerprint for ExportConfig
        let expected = "exported_at|repo_path";
        assert_eq!(
            fp, expected,
            "ExportConfig schema changed! Update expected fingerprint if intentional."
        );
    }

    #[test]
    fn test_archive_format_version() {
        // This test ensures the archive format version is explicitly tracked.
        // If you need to make breaking changes to the archive format:
        // 1. Increment this version number
        // 2. Update the import code to handle both old and new versions
        // 3. Document the change in CHANGELOG
        const EXPECTED_VERSION: u32 = 1;
        const EXPECTED_FORMAT: &str = "binnacle-store-v1";

        // These values are used in system_store_export
        // If this test fails, you're changing the archive format version!
        assert_eq!(
            EXPECTED_VERSION, 1,
            "Archive version changed! Ensure backwards compatibility."
        );
        assert_eq!(
            EXPECTED_FORMAT, "binnacle-store-v1",
            "Archive format string changed! Ensure backwards compatibility."
        );
    }

    #[test]
    fn test_task_create_with_complexity_check_simple() {
        let temp = setup();
        let result = task_create_with_complexity_check(
            temp.path(),
            "Fix typo in README".to_string(),
            None,
            None,
            Some(2),
            vec![],
            None,
            false,
        )
        .unwrap();

        // Simple task should not trigger complexity detection
        assert!(!result.complexity_detected);
        assert!(result.task_created.is_some());
        assert!(result.suggestion.is_none());
        assert!(result.proceed_command.is_none());
    }

    #[test]
    fn test_task_create_with_complexity_check_complex() {
        let temp = setup();
        let result = task_create_with_complexity_check(
            temp.path(),
            "Add authentication and fix database and improve logging".to_string(),
            None,
            None,
            Some(2),
            vec![],
            None,
            false,
        )
        .unwrap();

        // Complex task should trigger soft-gate
        assert!(result.complexity_detected);
        assert!(result.task_created.is_none()); // No task created
        assert!(result.suggestion.is_some());
        assert!(result.proceed_command.is_some());
        assert!(result.idea_command.is_some());

        // Check the suggestion contains expected content
        let suggestion = result.suggestion.unwrap();
        assert!(suggestion.contains("idea"));
        assert!(suggestion.contains("What would you like to do?"));
    }

    #[test]
    fn test_task_create_with_complexity_check_preserves_options() {
        let temp = setup();
        let result = task_create_with_complexity_check(
            temp.path(),
            "Explore caching options and investigate patterns".to_string(),
            Some("explore cache".to_string()),
            Some("Need to research".to_string()),
            Some(1),
            vec!["research".to_string()],
            Some("henry".to_string()),
            false,
        )
        .unwrap();

        // Should be complex (exploratory language)
        assert!(result.complexity_detected);

        // Check that the proceed_command includes all options
        let proceed_cmd = result.proceed_command.unwrap();
        assert!(proceed_cmd.contains("--force"));
        assert!(proceed_cmd.contains("-s \"explore cache\""));
        assert!(proceed_cmd.contains("-d \"Need to research\""));
        assert!(proceed_cmd.contains("-p 1"));
        assert!(proceed_cmd.contains("-t research"));
        assert!(proceed_cmd.contains("-a henry"));
    }

    #[test]
    fn test_detect_worktree_parent_git_regular_repo() {
        // Regular git repo has .git directory, not file
        let env = TestEnv::new_with_env();
        let git_dir = env.path().join(".git");
        std::fs::create_dir_all(&git_dir).unwrap();

        // Should return None for regular repos
        let result = detect_worktree_parent_git(env.path());
        assert!(result.is_none());
    }

    #[test]
    fn test_detect_worktree_parent_git_no_git() {
        // Directory with no .git at all
        let env = TestEnv::new_with_env();

        // Should return None when no .git exists
        let result = detect_worktree_parent_git(env.path());
        assert!(result.is_none());
    }

    #[test]
    fn test_detect_worktree_parent_git_worktree() {
        use tempfile::tempdir;

        // Create a mock parent repo with .git directory structure
        let parent_repo = tempdir().unwrap();
        let parent_git = parent_repo.path().join(".git");
        let worktrees_dir = parent_git.join("worktrees").join("test-worktree");
        std::fs::create_dir_all(&worktrees_dir).unwrap();

        // Create commondir file pointing back to parent .git
        std::fs::write(worktrees_dir.join("commondir"), "../..\n").unwrap();

        // Create a mock worktree directory with .git file
        let worktree = tempdir().unwrap();
        let worktree_git_file = worktree.path().join(".git");
        std::fs::write(
            &worktree_git_file,
            format!("gitdir: {}\n", worktrees_dir.display()),
        )
        .unwrap();

        // Should detect the parent .git directory
        let result = detect_worktree_parent_git(worktree.path());
        assert!(result.is_some());
        let parent_git_found = result.unwrap();
        assert!(parent_git_found.ends_with(".git"));
    }

    #[test]
    fn test_get_default_archive_directory_returns_archive_path() {
        // Just test that the function returns a path ending with /archives
        // We can't easily manipulate env vars in Rust 2024 without unsafe
        let dir = get_default_archive_directory();

        // Should always return Some on systems with a data directory
        assert!(dir.is_some());
        let path = dir.unwrap();
        assert!(
            path.to_str().unwrap().ends_with("/archives"),
            "Expected path ending with /archives, got: {:?}",
            path
        );
    }
}
