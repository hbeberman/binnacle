# PRD: Idea Nodes

## Overview

Introduce a new entity type called **Idea** (`bn-xxxx`) that is distinct from Tasks. Ideas represent low-stakes seeds—rough concepts, shower thoughts, or early-stage notions—that can be captured quickly and potentially grown into full PRDs or task trees in future sessions.

## Problem Statement

Currently, binnacle only has Tasks and Tests. When an agent or user has a half-formed thought or exploratory notion:
- Creating a Task feels too heavy (implies commitment, priority, status tracking)
- The thought gets lost if not captured somewhere
- There's no natural path from "vague idea" to "concrete work item"

## Proposed Solution

### Entity Type: Idea

| Property | Type | Description |
|----------|------|-------------|
| `id` | string | Format: `bn-xxxx` (hash-based like tasks) |
| `type` | string | Always `"idea"` |
| `title` | string | Brief title |
| `description` | string | Optional longer description |
| `tags` | string[] | Categorization |
| `status` | enum | `seed` \| `germinating` \| `promoted` \| `discarded` |
| `promoted_to` | string? | Task ID if promoted (e.g., `bn-a1b2`) |
| `created_at` | datetime | When captured |
| `updated_at` | datetime | Last modified |

### Status Flow

```
seed → germinating → promoted (becomes task/PRD)
              ↘ discarded
```

- **seed**: Just captured, raw thought
- **germinating**: Being fleshed out, gaining detail
- **promoted**: Has graduated to a task or PRD
- **discarded**: Decided not to pursue

## Feature 1: Basic Idea CRUD

### Commands

```bash
bn idea create "Title" [--description "..."] [--tag T]  # Create idea
bn idea list [--status S] [--tag T]                     # List ideas
bn idea show bn-xxxx                                   # View details
bn idea update bn-xxxx [--title|--description|--status]  # Modify
bn idea close bn-xxxx [--reason "..."]                 # Mark as discarded
bn idea delete bn-xxxx                                 # Permanent removal
```

### Key Design Decisions

1. **Consistent with task/test**: Uses `create`/`close` verbs like other entity types
2. **No priority**: Ideas don't compete with tasks for attention
3. **No dependencies**: Ideas are standalone until promoted
4. **Minimal fields**: Just enough to jog memory later

### Example Usage

```bash
$ bn idea create "Use SQLite FTS for search" --tag search
{"id":"bn-a1b2","title":"Use SQLite FTS for search"}

$ bn idea list -H
3 idea(s):

[ ] bn-a1b2 [seed] Use SQLite FTS for search [search]
[ ] bn-c3d4 [seed] Add visualization of task graph
[ ] bn-e5f6 [germinating] Time-based task aging
```

## Feature 2: Idea Promotion to Tasks/PRDs

### Commands

```bash
bn idea promote bn-xxxx                           # Convert to task
bn idea promote bn-xxxx --as-prd                  # Generate PRD file
bn idea germinate bn-xxxx                         # Mark as being developed
```

### Promotion Workflow

When an idea is promoted to a task:
1. Creates new task with idea's title and description
2. Copies tags to new task
3. Sets idea status to `promoted`
4. Links idea to new task via `promoted_to` field

When promoted as PRD:
1. Creates `prds/PRD_<TITLE_SLUG>.md` with template
2. Template includes idea description as starting point
3. Sets idea status to `promoted`
4. Stores PRD path in idea metadata

### PRD Template (generated)

```markdown
# PRD: {Title}

> Promoted from idea bn-xxxx on {date}

## Origin
{Original idea description}

## Problem Statement
[TODO: Define the problem]

## Proposed Solution
[TODO: Describe the solution]

## Implementation Plan
[TODO: Break into tasks]
```

### Example

```bash
$ bn idea promote bn-a1b2 --as-prd
Created PRD: prds/PRD_SQLITE_FTS_SEARCH.md
Idea bn-a1b2 marked as promoted

$ bn idea show bn-a1b2 -H
Idea bn-a1b2 [promoted]
  Title: Use SQLite FTS for search
  Promoted to: prds/PRD_SQLITE_FTS_SEARCH.md
  Created: 2026-01-22T09:00:00Z
```

## Data Model

### Idea Schema (JSON)

```json
{
  "id": "bn-a1b2",
  "type": "idea",
  "title": "Use SQLite FTS for search",
  "description": "Could use FTS5 extension for full-text search across task titles and descriptions",
  "tags": ["search", "performance"],
  "status": "seed",
  "promoted_to": null,
  "created_at": "2026-01-22T09:00:00Z",
  "updated_at": "2026-01-22T09:00:00Z"
}
```

### Storage

Ideas stored in `ideas.jsonl` alongside existing files:

```
~/.local/share/binnacle/<repo-hash>/
├── tasks.jsonl
├── ideas.jsonl        # NEW
├── commits.jsonl
├── test-results.jsonl
├── cache.db
└── config.toml
```

## MCP Integration

### New Tools
- `bn_idea_create`
- `bn_idea_list`
- `bn_idea_show`
- `bn_idea_update`
- `bn_idea_close`
- `bn_idea_promote`
- `bn_idea_germinate`

### New Resource
- `binnacle://ideas` - All ideas (subscribable)

### New Prompt
- `grow_idea` - Help develop a seed idea into a fuller concept

## Testing Strategy

### Feature 1: Basic CRUD
- Unit tests: Idea model serialization, ID generation
- Integration tests: Full CRUD round-trip, filtering by status/tag

### Feature 2: Promotion
- Unit tests: Promotion logic, PRD template generation
- Integration tests: Promote to task, promote to PRD, verify linking

## Success Criteria

1. Ideas can be captured in under 2 seconds (low friction)
2. Ideas persist across sessions without cluttering task lists
3. Clear promotion path from idea → task or idea → PRD
4. MCP tools enable AI agents to capture ideas during work

## Open Questions

1. Should ideas support attachments (links, code snippets)?
2. Should there be a "related to task" soft link for context?
3. Auto-suggest promotion when an idea has been germinating for N days?
