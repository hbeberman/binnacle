# PRD: Align AGENTS.md and SKILL.md with Current Codebase

**Status:** Draft
**Author:** GitHub Copilot
**Date:** 2025-01-23

## Overview

The embedded documentation templates (`AGENTS_MD_BLURB` and `SKILLS_FILE_CONTENT` in [src/commands/mod.rs](src/commands/mod.rs)) have drifted from the actual feature set. This PRD covers updating them to accurately reflect current `bn` capabilities.

## Motivation

Agents rely on these files for guidance on using binnacle. Missing documentation leads to:

- Agents not using `bn goodbye` for graceful termination
- Unawareness of bug tracking, idea management, milestones, and graph analysis features

## Non-Goals

- Creating a comprehensive command reference (agents should use `bn --help`)
- Documenting every flag and option
- Adding `bn agent kill` to agent-facing docs (human-operated command)
- Modifying binnacle repo-specific content in [AGENTS.md](AGENTS.md) (outside the generated section)

## Dependencies

- None

---

## Specification

### Architecture: Two-Part AGENTS.md

The [AGENTS.md](AGENTS.md) file has two distinct sections:

1. **Generated section** (between `<!-- BEGIN BINNACLE SECTION -->` and `<!-- END BINNACLE SECTION -->`):
   - Injected by `bn orient` from the `AGENTS_MD_BLURB` constant
   - **Must be generic** - applicable to ANY repo using binnacle
   - Updated automatically when `bn orient` runs

2. **Repo-specific section** (outside the tags):
   - In binnacle's case: "Build and Test", "GUI Testing Best Practices"
   - Manually maintained, not touched by `bn orient`

**This PRD only addresses the generated templates** (`AGENTS_MD_BLURB` and `SKILLS_FILE_CONTENT`). The binnacle repo's AGENTS.md will auto-update when `bn orient` is run after implementation.

### Current State Analysis

| Feature | In AGENTS_MD_BLURB | In SKILLS_FILE_CONTENT | In Code |
|---------|-------------------|------------------------|---------|
| `bn goodbye` | ✅ | ✅ | ✅ |
| Human should run init | ✅ | ❌ | ✅ |
| `bn agent list/kill` | ❌ | ❌ | ✅ |
| `bn bug` commands | ❌ | ❌ | ✅ |
| `bn idea` commands | ❌ | ❌ | ✅ |
| `bn milestone` commands | ❌ | ❌ | ✅ |
| `bn graph components` | ❌ | ❌ | ✅ |
| `bn search edges` | ❌ | ❌ | ✅ |
| `bn show` (universal viewer) | ❌ | ❌ | ✅ |
| `bn queue` commands | ❌ | ❌ | ✅ |
| Full help footer | ❌ | ✅ | N/A |

### Changes Required

#### 1. AGENTS_MD_BLURB (Generic, Auto-Injected)

The current `AGENTS_MD_BLURB` (lines 209-233) is already mostly complete. Minor addition:

**Add footer** before closing `<!-- END BINNACLE SECTION -->`:

```markdown
Run `bn --help` for the complete command reference.
```

This keeps the blurb focused on workflow while pointing agents to full docs.

#### 2. SKILLS_FILE_CONTENT (Comprehensive Reference)

Add documentation for missing commands to the embedded `SKILLS_FILE_CONTENT` constant (starting at line 239 of [src/commands/mod.rs](src/commands/mod.rs#L239)).

**Add to "Key Commands" section:**

```markdown
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
- `bn search edges --type depends_on` - Search edges by type
```

**Add to "Agent Lifecycle" or new section:**

```markdown
### Agent Lifecycle
- `bn orient --name "MyAgent" --register "Implementing feature X"` - Register agent with purpose
- `bn goodbye` - Gracefully terminate session (signals parent process)
- `bn agent list` - List registered agents (human use)
```

---

## Implementation

### Files to Modify

1. **[src/commands/mod.rs](src/commands/mod.rs)** - Two constants to update:
   - `AGENTS_MD_BLURB` (lines 209-233): Add `bn --help` footer
   - `SKILLS_FILE_CONTENT` (lines 239-358): Add missing command subsections

### Code Changes

#### AGENTS_MD_BLURB Changes

Add before the closing `<!-- END BINNACLE SECTION -->` tag:

```markdown
Run `bn --help` for the complete command reference.
```

#### SKILLS_FILE_CONTENT Changes

Insert new subsections into the "Key Commands" section. The exact insertion point is after the "Project Health" subsection (approximately line 315).

---

## New Command: `bn system emit`

### Motivation

Need a way to programmatically access the embedded templates (`AGENTS_MD_BLURB`, `SKILLS_FILE_CONTENT`) without initializing a repo or modifying files. Useful for:

- Pre-commit hooks (compare output vs current file)
- CI/CD pipelines
- External tooling
- Debugging template content

### CLI Design

```
bn system emit <template>

Arguments:
  <template>  Which template to emit: 'agents' or 'skill'

Examples:
  bn system emit agents    # Outputs AGENTS_MD_BLURB to stdout
  bn system emit skill     # Outputs SKILLS_FILE_CONTENT to stdout
```

### Behavior

- **No side effects**: Does not create/modify any files
- **No initialization required**: Works without a binnacle database
- **Raw output**: Just the template content, no JSON wrapper (unless `-H` is omitted, then JSON with `{"content": "..."}`)
- **Exit 0**: Always succeeds

### Implementation

Add to `src/cli/mod.rs` under `SystemCommands`:

```rust
/// Emit embedded templates to stdout (no side effects)
Emit {
    /// Which template to emit
    #[arg(value_enum)]
    template: EmitTemplate,
},
```

```rust
#[derive(Clone, ValueEnum)]
pub enum EmitTemplate {
    Agents,
    Skill,
}
```

Add handler in `src/main.rs`:

```rust
Some(SystemCommands::Emit { template }) => {
    let content = match template {
        EmitTemplate::Agents => commands::AGENTS_MD_BLURB,
        EmitTemplate::Skill => commands::SKILLS_FILE_CONTENT,
    };
    if human {
        println!("{}", content.trim());
    } else {
        println!("{}", serde_json::json!({"content": content.trim()}));
    }
}
```

Make constants public in `src/commands/mod.rs`:

```rust
pub const AGENTS_MD_BLURB: &str = ...
pub const SKILLS_FILE_CONTENT: &str = ...
```

---

## Testing

1. **Build and run**: After changes, run `cargo build` and verify no compile errors
2. **SKILL.md generation**: Run `bn orient --init` in a test directory and verify the generated SKILL.md contains all new sections
3. **AGENTS.md injection**: Verify `bn orient` updates existing AGENTS.md with the new blurb (including footer)
4. **Idempotency**: Run `bn orient` twice, verify AGENTS.md doesn't change on second run
5. **Help consistency**: Verify documented commands match `bn --help` output
6. **Emit command**: Verify `bn system emit agents` and `bn system emit skill` output correct content
7. **Pre-commit hook**: Verify hook detects stale AGENTS.md when `AGENTS_MD_BLURB` is modified

### Post-Implementation

After implementation, run `bn orient` in the binnacle repo itself to update the generated section of [AGENTS.md](AGENTS.md).

---

## Pre-Commit Hook: AGENTS.md Sync Check

### Motivation

When `AGENTS_MD_BLURB` in [src/commands/mod.rs](src/commands/mod.rs) is modified, the binnacle repo's own [AGENTS.md](AGENTS.md) becomes stale. Developers may forget to run `bn orient` to regenerate it.

### Implementation

Add a check to [hooks/pre-commit](hooks/pre-commit) that uses `bn system emit agents` to compare against the current AGENTS.md binnacle section.

### Hook Addition

Add after the clippy check, before the security audit:

```bash
# Check if AGENTS.md is in sync with embedded blurb
# Uses cargo run to check against the code being committed, not the installed bn
# NOTE: This runs unconditionally (not just when src/commands/mod.rs changes)
# to catch drift caused by refactors moving the constant to a different file.
echo "  → Checking AGENTS.md sync..."

# Extract current binnacle section from AGENTS.md
current_section=$(sed -n '/<!-- BEGIN BINNACLE SECTION -->/,/<!-- END BINNACLE SECTION -->/p' AGENTS.md 2>/dev/null || true)

# Get expected content from the code being committed
expected_section=$(cargo run --quiet -- -H system emit agents 2>/dev/null || true)

if [ -n "$expected_section" ] && [ "$current_section" != "$expected_section" ]; then
    echo ""
    echo "❌ AGENTS.md binnacle section is out of sync!"
    echo "   Run 'cargo run -- system init --write-agents-md -y' to update, then stage AGENTS.md"
    exit 1
fi
```

### Behavior

| Scenario | Result |
|----------|--------|
| Binnacle section matches emit output | Pass |
| Binnacle section differs | Fail, prompt to run `system init --write-agents-md` |
| `cargo run` fails | Skip check (graceful degradation) |

### Design Rationale

The check runs unconditionally on every commit rather than only when `src/commands/mod.rs` is staged. This ensures the check catches drift even if the constant is refactored to a different file. The trade-off is a `cargo run` on each commit, but binnacle builds quickly (~0.1s on warm cache).


---

## Open Questions

1. **Should SKILL.md include `bn system store export/import`?** These are administrative commands that agents rarely need. *Recommendation: Omit for now, agents can discover via `bn --help`.*

2. **Should AGENTS_MD_BLURB mention init guidance?** It currently does ("For new projects, the human should run `bn system init`..."). This seems appropriate for generic use. *Recommendation: Keep as-is.*
