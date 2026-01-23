# PRD: PRD as First-Class Objects

## Overview

Introduce a new entity type called **PRD** (`bn-xxxx` with `type: "prd"`) that represents Product Requirement Documents as first-class objects in binnacle. PRDs store their full content within binnacle, can depend on other PRDs, support section-level task linking, and auto-complete when all linked tasks are done.

## Problem Statement

Currently, binnacle uses external markdown files (`prds/*.md`) for product requirement documents:

1. **No traceability**: Tasks cannot link back to their originating PRD
2. **Manual tracking**: PRD status must be tracked separately or not at all
3. **Scattered metadata**: PRD author, creation date, and status live in ad-hoc markdown frontmatter
4. **No aggregation**: Cannot easily query "all tasks for this PRD" or "PRD completion percentage"
5. **No history**: Changes to PRDs aren't tracked in binnacle's audit log
6. **External files**: Content lives outside binnacle, requiring manual synchronization

## Motivation

With PRDs as entities, agents and humans can:
- Create a PRD and immediately spawn linked tasks from it
- Track PRD completion based on linked task status
- Query the relationship between requirements and implementation
- Maintain a clear paper trail from idea → PRD → tasks → commits
- Link tasks to specific sections within a PRD for granular tracking
- Model PRD dependencies (e.g., "API PRD depends on Auth PRD")

---

## Proposed Solution

### Entity Type: PRD

PRDs use the unified `bn-xxxx` ID format with a `type` field, consistent with other binnacle entities.

| Property | Type | Description |
|----------|------|-------------|
| `id` | string | Format: `bn-xxxx` (hash-based, same as tasks) |
| `type` | string | Always `"prd"` |
| `title` | string | PRD title (e.g., "GUI Theming Support") |
| `description` | string | Brief summary of the PRD |
| `content` | string | Full markdown content (stored in binnacle) |
| `sections` | Section[] | Parsed sections for granular linking |
| `status` | enum | `draft` \| `approved` \| `in_progress` \| `completed` \| `abandoned` |
| `author` | string? | Who created/owns this PRD |
| `tags` | string[] | Categorization (e.g., `gui`, `backend`, `infrastructure`) |
| `priority` | int | 0-4, same as tasks |
| `depends_on` | string[] | IDs of PRDs this PRD depends on |
| `created_at` | datetime | When created |
| `updated_at` | datetime | Last modified |
| `approved_at` | datetime? | When status changed to approved |
| `completed_at` | datetime? | When marked complete |

### Section Schema

Sections are automatically parsed from markdown headings:

```json
{
  "id": "feature-1-auto-follow",
  "heading": "Feature 1: Auto-Follow Camera",
  "level": 2,
  "start_line": 45,
  "end_line": 89
}
```

### Status Flow

```
draft → approved → in_progress → completed
          ↘ abandoned    ↘ abandoned
```

- **draft**: Being written/refined, not ready for work
- **approved**: Requirements finalized, ready to spawn tasks
- **in_progress**: Tasks are being worked on (auto-transitions when first linked task starts)
- **completed**: All linked tasks closed (auto-transitions when all done)
- **abandoned**: Decided not to pursue

### Storage

PRD content stored directly in binnacle storage:

```
~/.local/share/binnacle/<repo-hash>/
├── tasks.jsonl
├── prds.jsonl         # PRDs with full content
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
bn prd show bn-xxxx
bn prd update bn-xxxx [--title|--description|--status|--priority|...]
bn prd edit bn-xxxx                    # Open content in $EDITOR
bn prd approve bn-xxxx                 # Shorthand for --status approved
bn prd complete bn-xxxx                # Shorthand for --status completed
bn prd abandon bn-xxxx [--reason "..."]
bn prd delete bn-xxxx
```

### Example Usage

```bash
$ bn prd create "GUI Theming Support" -d "Enable customizable color themes" -t gui -p 2
{"id":"bn-a1b2","type":"prd","title":"GUI Theming Support","status":"draft"}

$ bn prd list -H
2 PRD(s):

[draft]       bn-a1b2  GUI Theming Support [gui] (0/0 tasks)
[in_progress] bn-c3d4  Auto Port Selection [gui] (3/5 tasks)
```

---

## Feature 2: Content Management

### Inline Content Storage

PRD content is stored directly in the PRD entity, not as external files:

```bash
# Create PRD with initial content
bn prd create "GUI Theming" --content "## Overview\n\nEnable themes..."

# Edit content in $EDITOR
bn prd edit bn-a1b2

# Import from existing markdown file
bn prd create "GUI Theming" --from-file prds/PRD_GUI_THEMING.md

# Export content to file
bn prd export bn-a1b2 > my-prd.md
```

### Content Display

```bash
$ bn prd show bn-a1b2 -H
PRD bn-a1b2 [in_progress]
  Title: GUI Theming Support
  Priority: 2
  Author: henry
  Tags: gui
  Progress: 60% (3/5 tasks)
  
  Sections:
    § Overview (2 tasks linked)
    § Feature 1: Theme Configuration (1 task linked)
    § Feature 2: Theme Picker UI (2 tasks linked)
  
--- Content ---
## Overview

Enable customizable color themes for the binnacle web GUI.

## Feature 1: Theme Configuration
...
```

---

## Feature 3: Section-Level Task Linking

### Automatic Section Parsing

When content is saved, binnacle parses markdown headings into sections:

```json
{
  "sections": [
    {"id": "overview", "heading": "Overview", "level": 2, "start_line": 1, "end_line": 10},
    {"id": "feature-1", "heading": "Feature 1: Theme Configuration", "level": 2, "start_line": 12, "end_line": 45},
    {"id": "feature-2", "heading": "Feature 2: Theme Picker UI", "level": 2, "start_line": 47, "end_line": 89}
  ]
}
```

### Linking Tasks to Sections

```bash
# Link task to entire PRD
bn link add bn-task1 bn-prd1 --type implements

# Link task to specific section
bn link add bn-task1 bn-prd1 --type implements --section feature-1

# Spawn task for specific section
bn prd spawn bn-a1b2 "Implement theme schema" --section feature-1
```

### Section Progress Tracking

```bash
$ bn prd show bn-a1b2 --sections -H
PRD bn-a1b2 [in_progress]
  Title: GUI Theming Support
  
  Section Progress:
    § Overview                          100% (2/2 tasks)
    § Feature 1: Theme Configuration     50% (1/2 tasks)
    § Feature 2: Theme Picker UI          0% (0/3 tasks)
  
  Overall: 43% (3/7 tasks)
```

---

## Feature 4: PRD Dependencies

PRDs can depend on other PRDs, enabling sequencing of large initiatives:

```bash
# Add dependency
bn link add bn-api-prd bn-auth-prd --type depends_on

# View PRD with dependencies
$ bn prd show bn-api-prd -H
PRD bn-api-prd [blocked]
  Title: API Endpoints
  Status: blocked (depends on bn-auth-prd)
  
  Dependencies:
    ⏳ bn-auth-prd [in_progress] Authentication System
```

### Dependency Rules

- PRD cannot transition to `in_progress` until all dependencies are `completed`
- Cycle detection prevents circular dependencies
- `bn ready --prds` shows PRDs with no blocking dependencies

---

## Feature 5: Auto-Status Transitions

### Automatic in_progress Detection

When a PRD is `approved` and any linked task transitions to `in_progress`:
- PRD status automatically changes to `in_progress`
- Log entry records the transition with triggering task

### Automatic Completion Detection

When all linked tasks for an `in_progress` PRD are closed:
- PRD status changes to `completed`
- `completed_at` timestamp is set
- Log entry records completion

This is always-on behavior, not configurable.

---

## Feature 6: PRD-Task Linking

### Link Commands

PRDs and tasks are linked using the existing `bn link` infrastructure:

```bash
bn link add bn-task bn-prd --type implements           # Task implements PRD
bn link add bn-task bn-prd --type implements --section feature-1  # Section-specific
bn link list bn-prd                                     # Show all linked tasks
```

### Convenience Command

For common "create task for this PRD" workflow:

```bash
bn prd spawn bn-xxxx "Task title" [-p N] [-t tag] [--section S]
# Equivalent to:
#   bn task create "Task title" -p N -t tag
#   bn link add <new-task-id> bn-xxxx --type implements [--section S]
```

### Example

```bash
$ bn prd spawn bn-a1b2 "Implement theme configuration schema" -p 1 --section feature-1
Created task bn-e5f6, linked to bn-a1b2 § feature-1

$ bn link list bn-a1b2 -H
Links for bn-a1b2 (GUI Theming Support):
  § Overview:
    ← bn-d4e5 implements (Write overview docs) [done]
    ← bn-f6g7 implements (Define scope) [done]
  § Feature 1: Theme Configuration:
    ← bn-e5f6 implements (Implement theme schema) [in_progress]
  § Feature 2: Theme Picker UI:
    ← bn-g7h8 implements (Add CSS injection) [pending]
    ← bn-i9j0 implements (Create picker component) [pending]
```

---

## Feature 7: Progress Tracking

### Automatic Progress Calculation

When viewing a PRD, show completion progress based on linked tasks:

```bash
$ bn prd show bn-a1b2 -H
PRD bn-a1b2 [in_progress]
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

### JSON Output

```json
{
  "id": "bn-a1b2",
  "type": "prd",
  "title": "GUI Theming Support",
  "status": "in_progress",
  "progress": {
    "total_tasks": 5,
    "closed_tasks": 3,
    "percentage": 60,
    "by_section": {
      "overview": {"total": 2, "closed": 2, "percentage": 100},
      "feature-1": {"total": 2, "closed": 1, "percentage": 50},
      "feature-2": {"total": 1, "closed": 0, "percentage": 0}
    }
  }
}
```

---

## GUI Integration

### Graph View

PRD nodes displayed in the graph with:
- **Shape**: Rectangle with rounded corners (distinct from task circles)
- **Color**: Purple/violet gradient (distinct from task blue, bug red)
- **Size**: Larger than tasks to indicate higher-level entity
- **Label**: Title + progress percentage
- **Expandable**: Click to show/hide section sub-nodes

### Section Sub-Nodes

When a PRD is expanded in the graph:
- Section nodes appear as smaller rectangles connected to the PRD
- Each section shows its own progress
- Tasks link to sections, not directly to the PRD node

### Info Panel

When a PRD is selected in the graph, the info panel shows:
- Full PRD details and content
- Progress bar visualization (overall and per-section)
- List of linked tasks with status indicators
- Quick actions: Spawn task, Edit content, Approve/Complete

### Stats Overlay

Update the stats overlay to include PRDs:

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
| `bn_prd_edit_content` | Update PRD content |
| `bn_prd_approve` | Mark PRD as approved |
| `bn_prd_complete` | Mark PRD as completed |
| `bn_prd_abandon` | Mark PRD as abandoned |
| `bn_prd_spawn` | Create task linked to PRD/section |
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
  "id": "bn-a1b2",
  "type": "prd",
  "title": "GUI Theming Support",
  "description": "Enable customizable color themes for the web GUI",
  "content": "## Overview\n\nEnable customizable color themes...\n\n## Feature 1: Theme Configuration\n\n...",
  "sections": [
    {"id": "overview", "heading": "Overview", "level": 2, "start_line": 1, "end_line": 5},
    {"id": "feature-1", "heading": "Feature 1: Theme Configuration", "level": 2, "start_line": 7, "end_line": 45}
  ],
  "status": "in_progress",
  "author": "henry",
  "tags": ["gui", "ux"],
  "priority": 2,
  "depends_on": [],
  "created_at": "2026-01-20T10:00:00Z",
  "updated_at": "2026-01-22T14:30:00Z",
  "approved_at": "2026-01-21T09:00:00Z",
  "completed_at": null
}
```

### Link Types

| Link Type | Source | Target | Metadata | Meaning |
|-----------|--------|--------|----------|---------|
| `implements` | Task | PRD | `section?` | Task implements requirement from PRD |
| `depends_on` | PRD | PRD | - | PRD depends on another PRD |

---

## Testing Strategy

### Unit Tests

- PRD model serialization round-trip
- Section parsing from markdown content
- Status transition validation
- Progress calculation from linked tasks
- Auto-status transition logic
- Dependency cycle detection

### Integration Tests

- Full CRUD round-trip via CLI
- `bn prd spawn` creates and links correctly
- Section-level linking works
- Auto-status transitions fire correctly
- PRD dependency blocking works
- `bn prd edit` opens editor and saves content
- Import from existing markdown files

### Test Count Estimate

- 15 unit tests (model, sections, status, progress)
- 22 integration tests (CRUD, linking, sections, dependencies)
- **Total new tests: ~37**

---

## Implementation Plan

### Phase 1: Core PRD Entity
- [ ] Add PRD model to `src/models/`
- [ ] Add PRD storage to JSONL backend
- [ ] Implement basic CRUD commands
- [ ] Add content storage and `bn prd edit`

### Phase 2: Section Parsing
- [ ] Implement markdown heading parser
- [ ] Store sections in PRD entity
- [ ] Add section display to `bn prd show`

### Phase 3: Linking Infrastructure
- [ ] Add `implements` link type with optional section
- [ ] Implement `bn prd spawn` with `--section` flag
- [ ] Update `bn link list` for PRD/section relationships

### Phase 4: Progress Tracking
- [ ] Calculate progress from linked tasks
- [ ] Add section-level progress
- [ ] Implement auto-status transitions

### Phase 5: PRD Dependencies
- [ ] Add `depends_on` field and linking
- [ ] Implement dependency blocking
- [ ] Add cycle detection

### Phase 6: MCP Tools
- [ ] Add all `bn_prd_*` tools
- [ ] Add `binnacle://prds` resource
- [ ] Add prompts: `write_prd`, `break_down_prd`, `prd_status_report`

### Phase 7: GUI Integration
- [ ] Add PRD node rendering in graph view
- [ ] Implement expandable section sub-nodes
- [ ] Update info panel for PRD selection
- [ ] Add PRDs to stats overlay

---

## Migration Path

For existing `prds/*.md` files:

```bash
# Import existing PRD file as entity
bn prd create "GUI Theming Support" --from-file prds/PRD_GUI_THEMING.md --status in_progress

# Or bulk import
for f in prds/*.md; do
  bn prd create "$(basename "$f" .md)" --from-file "$f" --status draft
done
```

The import:
1. Creates PRD entity with content from file
2. Parses sections from markdown headings
3. Original file can be deleted or kept as backup

---

## Success Criteria

1. PRDs can be created, listed, and queried as first-class entities
2. PRD content is stored within binnacle, not external files
3. Tasks can be linked to specific PRD sections
4. PRDs auto-transition to `completed` when all tasks are done
5. PRD dependencies block progress correctly
6. GUI displays PRDs with expandable sections
7. Full traceability: Idea → PRD → Sections → Tasks → Commits

---

## Appendix: Command Reference

```bash
# CRUD
bn prd create "Title" [-d "desc"] [-p N] [-t tag] [-a author] [--content "..."] [--from-file path]
bn prd list [--status S] [--tag T] [--priority N] [--author A]
bn prd show <id> [--sections]
bn prd update <id> [--title|--description|--status|--priority|...]
bn prd edit <id>                       # Open in $EDITOR
bn prd export <id>                     # Output content to stdout
bn prd delete <id>

# Status shortcuts
bn prd approve <id>
bn prd complete <id>
bn prd abandon <id> [--reason "..."]

# Linking
bn prd spawn <id> "Task title" [-p N] [-t tag] [--section S]
bn link add <task-id> <prd-id> --type implements [--section S]
bn link add <prd-id> <prd-id> --type depends_on
bn link list <prd-id>
```
