# PRD: PRD as First-Class Objects

## Overview

Introduce a new entity type called **PRD** (`bnp-xxxx`) that represents Product Requirement Documents as first-class objects in binnacle. PRDs can be programmatically linked to tasks, enabling full traceability from high-level requirements through implementation.

## Problem Statement

Currently, binnacle uses external markdown files (`prds/*.md`) for product requirement documents:

1. **No traceability**: Tasks cannot link back to their originating PRD
2. **Manual tracking**: PRD status must be tracked separately or not at all
3. **Scattered metadata**: PRD author, creation date, and status live in ad-hoc markdown frontmatter
4. **No aggregation**: Cannot easily query "all tasks for this PRD" or "PRD completion percentage"
5. **No history**: Changes to PRDs aren't tracked in binnacle's audit log

## Motivation

With PRDs as entities, agents and humans can:
- Create a PRD and immediately spawn linked tasks from it
- Track PRD completion based on linked task status
- Query the relationship between requirements and implementation
- Maintain a clear paper trail from idea → PRD → tasks → commits
- Enable the workflow: `bn idea promote bni-xxxx --as-prd` creates a trackable PRD entity

---

## Proposed Solution

### Entity Type: PRD

| Property | Type | Description |
|----------|------|-------------|
| `id` | string | Format: `bnp-xxxx` (hash-based like tasks) |
| `type` | string | Always `"prd"` |
| `title` | string | PRD title (e.g., "GUI Theming Support") |
| `description` | string | Brief summary of the PRD |
| `content_path` | string? | Optional path to full markdown content |
| `status` | enum | `draft` \| `approved` \| `in_progress` \| `completed` \| `abandoned` |
| `author` | string? | Who created/owns this PRD |
| `tags` | string[] | Categorization (e.g., `gui`, `backend`, `infrastructure`) |
| `priority` | int | 0-4, same as tasks |
| `created_at` | datetime | When created |
| `updated_at` | datetime | Last modified |
| `approved_at` | datetime? | When status changed to approved |
| `completed_at` | datetime? | When marked complete |

### Status Flow

```
draft → approved → in_progress → completed
          ↘ abandoned    ↘ abandoned
```

- **draft**: Being written/refined, not ready for work
- **approved**: Requirements finalized, ready to spawn tasks
- **in_progress**: Tasks are being worked on
- **completed**: All linked tasks closed successfully
- **abandoned**: Decided not to pursue

### Storage

PRDs stored in `prds.jsonl` alongside existing files:

```
~/.local/share/binnacle/<repo-hash>/
├── tasks.jsonl
├── ideas.jsonl
├── prds.jsonl         # NEW
├── commits.jsonl
├── test-results.jsonl
├── cache.db
└── config.toml
```

---

## Feature 1: Basic PRD CRUD

### Commands

```bash
bn prd create "Title" [-d "description"] [-p N] [-t tag] [-a author]
bn prd list [--status S] [--tag T] [--priority N]
bn prd show bnp-xxxx
bn prd update bnp-xxxx [--title|--description|--status|--priority|...]
bn prd approve bnp-xxxx                # Shorthand for --status approved
bn prd complete bnp-xxxx               # Shorthand for --status completed
bn prd abandon bnp-xxxx [--reason "..."]
bn prd delete bnp-xxxx
```

### Example Usage

```bash
$ bn prd create "GUI Theming Support" -d "Enable customizable color themes" -t gui -p 2
{"id":"bnp-a1b2","title":"GUI Theming Support","status":"draft"}

$ bn prd list -H
2 PRD(s):

[draft]       bnp-a1b2  GUI Theming Support [gui] (0/0 tasks)
[in_progress] bnp-c3d4  Auto Port Selection [gui] (3/5 tasks)
```

---

## Feature 2: PRD-Task Linking

### Link Commands

PRDs and tasks are linked using the existing `bn link` infrastructure with a new link type:

```bash
bn link add bn-xxxx bnp-yyyy --type implements  # Task implements PRD
bn link add bnp-xxxx bn-yyyy --type spawns      # PRD spawns task (inverse)
bn link list bnp-xxxx                           # Show all linked tasks
```

### Convenience Command

For common "create task for this PRD" workflow:

```bash
bn prd spawn bnp-xxxx "Task title" [-p N] [-t tag]
# Equivalent to:
#   bn task create "Task title" -p N -t tag
#   bn link add <new-task-id> bnp-xxxx --type implements
```

### Example

```bash
$ bn prd spawn bnp-a1b2 "Implement theme configuration schema" -p 1
Created task bn-e5f6, linked to bnp-a1b2

$ bn link list bnp-a1b2 -H
Links for bnp-a1b2 (GUI Theming Support):
  ← bn-e5f6 implements (Implement theme configuration schema)
  ← bn-g7h8 implements (Add CSS variable injection)
  ← bn-i9j0 implements (Create theme picker UI)
```

---

## Feature 3: PRD Progress Tracking

### Automatic Progress Calculation

When viewing a PRD, show completion progress based on linked tasks:

```bash
$ bn prd show bnp-a1b2 -H
PRD bnp-a1b2 [in_progress]
  Title: GUI Theming Support
  Description: Enable customizable color themes
  Priority: 2
  Author: henry
  Tags: gui
  Created: 2026-01-20T10:00:00Z
  
  Progress: 60% (3/5 tasks closed)
  
  Linked Tasks:
    [done]        bn-e5f6  Implement theme configuration schema
    [done]        bn-g7h8  Add CSS variable injection
    [done]        bn-k1l2  Write theme documentation
    [in_progress] bn-i9j0  Create theme picker UI
    [pending]     bn-m3n4  Add default theme presets
```

### Progress in List View

```bash
$ bn prd list -H
3 PRD(s):

[draft]       bnp-x1y2  New Feature      [backend]  (0/0 tasks)
[in_progress] bnp-a1b2  GUI Theming      [gui]      (3/5 tasks, 60%)
[completed]   bnp-c3d4  Auto Port        [gui]      (5/5 tasks, 100%)
```

### JSON Output

```json
{
  "id": "bnp-a1b2",
  "title": "GUI Theming Support",
  "status": "in_progress",
  "progress": {
    "total_tasks": 5,
    "closed_tasks": 3,
    "percentage": 60
  },
  "linked_tasks": ["bn-e5f6", "bn-g7h8", "bn-i9j0", "bn-k1l2", "bn-m3n4"]
}
```

---

## Feature 4: Auto-Status Transitions

### Automatic in_progress Detection

When a PRD is `approved` and any linked task transitions to `in_progress`:
- PRD status automatically changes to `in_progress`
- Log entry records the transition with triggering task

### Automatic Completion Detection

When all linked tasks for an `in_progress` PRD are closed:
- PRD status changes to `completed`
- `completed_at` timestamp is set
- Log entry records completion

### Configurable Behavior

```bash
bn config set prd.auto_progress true   # Enable auto-transitions (default)
bn config set prd.auto_progress false  # Manual status management only
```

---

## Feature 5: Content File Association

PRDs can optionally link to a full markdown document:

```bash
bn prd create "GUI Theming" --content prds/PRD_GUI_THEMING.md
bn prd update bnp-xxxx --content prds/updated_prd.md
```

### Behavior

- `content_path` is stored in the PRD entity
- Content file is NOT managed by binnacle (user creates/edits it)
- `bn prd show --full` displays the linked file content
- Validates file exists when setting path

### Example

```bash
$ bn prd show bnp-a1b2 --full -H
PRD bnp-a1b2 [in_progress]
  Title: GUI Theming Support
  Content: prds/PRD_GUI_THEMING.md
  Progress: 60% (3/5 tasks)
  
--- Content ---
# PRD: GUI Theming Support

## Overview
Enable customizable color themes for the binnacle web GUI...
[rest of file content]
```

---

## Feature 6: Idea → PRD Promotion Integration

Enhance the existing idea promotion (from PRD_IDEA_NODES) to create PRD entities:

```bash
bn idea promote bni-xxxx --as-prd
```

### Workflow

1. Creates PRD entity with idea's title, description, tags
2. Optionally generates `prds/PRD_<SLUG>.md` template
3. Links PRD to content file via `content_path`
4. Sets idea status to `promoted`
5. Records `promoted_to: bnp-xxxx` on idea

### Generated PRD Template

```markdown
# PRD: {Title}

> Promoted from idea bni-xxxx on {date}

## Origin
{Original idea description}

## Problem Statement
[TODO: Define the problem]

## Proposed Solution
[TODO: Describe the solution]

## Implementation Plan
[TODO: Break into tasks]

## Success Criteria
[TODO: Define done]
```

---

## GUI Integration

### Graph View

PRD nodes displayed in the graph with:
- **Shape**: Rectangle with rounded corners (distinct from task circles)
- **Color**: Purple/violet gradient (distinct from task blue, bug red)
- **Size**: Larger than tasks to indicate higher-level entity
- **Label**: Title + progress percentage

### Info Panel

When a PRD is selected in the graph, the info panel shows:
- Full PRD details
- Progress bar visualization
- List of linked tasks with status indicators
- Quick actions: Spawn task, View content

### Stats Overlay

Update the stats overlay (from PRD_GUI_CAMERA_FOLLOW) to include PRDs:

```
🐛 5 bugs | 📋 12 tasks | 📄 3 PRDs (1 in progress)
```

---

## MCP Integration

### New Tools

| Tool | Description |
|------|-------------|
| `bn_prd_create` | Create a new PRD |
| `bn_prd_list` | List PRDs with filtering |
| `bn_prd_show` | Show PRD details with progress |
| `bn_prd_update` | Update PRD fields |
| `bn_prd_approve` | Mark PRD as approved |
| `bn_prd_complete` | Mark PRD as completed |
| `bn_prd_abandon` | Mark PRD as abandoned |
| `bn_prd_spawn` | Create task linked to PRD |
| `bn_prd_delete` | Delete a PRD |

### New Resource

- `binnacle://prds` - All PRDs (subscribable)

### New Prompts

| Prompt | Description |
|--------|-------------|
| `write_prd` | Help structure a PRD from rough requirements |
| `break_down_prd` | Generate task breakdown from PRD content |
| `prd_status_report` | Summarize progress across all active PRDs |

---

## Data Model

### PRD Schema (JSON)

```json
{
  "id": "bnp-a1b2",
  "type": "prd",
  "title": "GUI Theming Support",
  "description": "Enable customizable color themes for the web GUI",
  "content_path": "prds/PRD_GUI_THEMING.md",
  "status": "in_progress",
  "author": "henry",
  "tags": ["gui", "ux"],
  "priority": 2,
  "created_at": "2026-01-20T10:00:00Z",
  "updated_at": "2026-01-22T14:30:00Z",
  "approved_at": "2026-01-21T09:00:00Z",
  "completed_at": null
}
```

### Link Types

| Link Type | Source | Target | Meaning |
|-----------|--------|--------|---------|
| `implements` | Task | PRD | Task implements requirement from PRD |
| `spawns` | PRD | Task | PRD spawned this task (inverse view) |

---

## Testing Strategy

### Unit Tests

- PRD model serialization round-trip
- ID generation (`bnp-xxxx` format)
- Status transition validation
- Progress calculation from linked tasks
- Auto-status transition logic

### Integration Tests

- Full CRUD round-trip via CLI
- `bn prd spawn` creates and links correctly
- Auto-status transitions work
- `bn link list` shows PRD relationships
- Content file validation
- Idea → PRD promotion creates both entity and file

### Test Count Estimate

- 12 unit tests (model, status, progress)
- 18 integration tests (CRUD, linking, promotion)
- **Total new tests: ~30**

---

## Implementation Plan

### Phase 1: Core PRD Entity
- [ ] Add PRD model to `src/models/`
- [ ] Add PRD storage to JSONL backend
- [ ] Implement basic CRUD commands
- [ ] Add `prds.jsonl` to orphan branch backend

### Phase 2: Linking Infrastructure
- [ ] Add `implements` link type
- [ ] Implement `bn prd spawn` convenience command
- [ ] Update `bn link list` for PRD relationships

### Phase 3: Progress Tracking
- [ ] Calculate progress from linked tasks
- [ ] Add progress to `bn prd show` and `bn prd list`
- [ ] Implement auto-status transitions

### Phase 4: Content Association
- [ ] Add `--content` flag to create/update
- [ ] Implement `--full` flag for show
- [ ] Validate content file exists

### Phase 5: Idea Integration
- [ ] Update `bn idea promote --as-prd` to create PRD entity
- [ ] Generate markdown template with PRD link

### Phase 6: MCP Tools
- [ ] Add all `bn_prd_*` tools
- [ ] Add `binnacle://prds` resource
- [ ] Add prompts: `write_prd`, `break_down_prd`, `prd_status_report`

### Phase 7: GUI Integration
- [ ] Add PRD node rendering in graph view
- [ ] Update info panel for PRD selection
- [ ] Add PRDs to stats overlay

---

## Migration Path

For existing `prds/*.md` files:

```bash
# Import existing PRD file as entity
bn prd import prds/PRD_GUI_THEMING.md --title "GUI Theming Support" --status in_progress

# Or bulk import
bn prd import-all prds/ --status draft
```

The import command:
1. Creates PRD entity with extracted/provided title
2. Sets `content_path` to the file
3. Parses markdown for description if possible
4. Does NOT move or modify the original file

---

## Success Criteria

1. PRDs can be created, listed, and queried as first-class entities
2. Tasks can be spawned from PRDs with automatic linking
3. PRD progress is automatically calculated from linked tasks
4. Status transitions reflect actual work state
5. GUI displays PRDs as distinct nodes with progress visualization
6. Existing markdown PRDs can be imported without disruption
7. Full traceability: Idea → PRD → Tasks → Commits

---

## Open Questions

1. Should PRDs support dependencies on other PRDs?
2. Should there be a "milestone" that groups multiple PRDs?
3. Should `bn prd spawn` support creating multiple tasks from a list?
4. Auto-complete threshold: 100% of tasks, or allow exceptions?
5. Should deleted tasks affect PRD progress calculation?

---

## Appendix: Command Reference

```bash
# CRUD
bn prd create "Title" [-d "desc"] [-p N] [-t tag] [-a author] [--content path]
bn prd list [--status S] [--tag T] [--priority N] [--author A]
bn prd show <id> [--full]
bn prd update <id> [--title|--description|--status|--priority|--content|...]
bn prd delete <id>

# Status shortcuts
bn prd approve <id>
bn prd complete <id>
bn prd abandon <id> [--reason "..."]

# Linking
bn prd spawn <id> "Task title" [-p N] [-t tag]
bn link add <task-id> <prd-id> --type implements
bn link list <prd-id>

# Migration
bn prd import <file> [--title "..."] [--status S]
bn prd import-all <dir> [--status S]
```
