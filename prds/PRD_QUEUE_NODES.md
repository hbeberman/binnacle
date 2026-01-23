# PRD: Queue Nodes

> Promoted from idea bni-7d71 on 2026-01-23

## Overview

Introduce a **Queue** entity type (`bnq-xxxx`) that serves as a work pool for agent task prioritization. Operators (human or agent) can add tasks to the queue to signal they should be worked on first. Agents see queued tasks prioritized in `bn ready` output.

## Problem Statement

Currently, binnacle tracks task dependencies and priorities, but there's no way to signal "work on these specific tasks next" without modifying task priorities. This creates friction:

- **Priority inflation** - Everything becomes P0 when operators want immediate attention
- **No operator control** - Human supervisors can't easily direct agent focus
- **Sprint planning gap** - No way to group work into a focused batch

## Proposed Solution

### Entity Type: Queue

A single global queue per repository that acts as a prioritized work pool.

| Property | Type | Description |
|----------|------|-------------|
| `id` | string | Format: `bnq-xxxx` (hash-based) |
| `type` | string | Always `"queue"` |
| `title` | string | Queue name (e.g., "Sprint 1", "Urgent") |
| `description` | string? | Optional description |
| `created_at` | datetime | When created |
| `updated_at` | datetime | Last modified |

### Queue Membership via Links

Tasks are added to the queue using the existing link system with a new `queued` link type:

```
task --[queued]--> queue
```

This leverages the existing link infrastructure while giving queues first-class semantics.

### Key Design Decisions

1. **Single global queue** - One queue per repo, keeping the model simple
2. **Unordered membership** - Tasks in queue are sorted by existing priority (0-4), not explicit position
3. **Link-based membership** - Uses `bn link add/rm` with `--type queued`
4. **Auto-removal on close** - Completed tasks automatically leave the queue
5. **No transitive queuing** - Queuing a parent doesn't queue children; each task must be explicitly added

## Feature 1: Queue CRUD

### Commands

```bash
bn queue create "Title" [--description "..."]   # Create queue (one per repo)
bn queue show                                   # Show queue and its tasks
bn queue delete                                 # Delete queue (removes all queue links)
```

### Constraints

- Only one queue can exist per repository
- `bn queue create` fails if a queue already exists
- `bn queue delete` removes the queue entity and all `queued` links

### Example Usage

```bash
$ bn queue create "January Sprint"
{"id":"bnq-a1b2","title":"January Sprint","type":"queue"}

$ bn queue show -H
Queue: January Sprint (bnq-a1b2)
  Created: 2026-01-23T10:00:00Z
  
  Queued tasks (3):
    [P1] bn-c3d4: Fix auth bug
    [P2] bn-e5f6: Add search feature  
    [P2] bn-g7h8: Update docs
```

## Feature 2: Adding/Removing Tasks from Queue

Uses the existing link system:

```bash
bn link add bn-a1b2 bnq-xxxx --type queued     # Add task to queue
bn link rm bn-a1b2 bnq-xxxx                     # Remove task from queue
```

### Convenience Aliases (Optional)

For discoverability, these aliases could be added:

```bash
bn queue add bn-a1b2        # Alias for: bn link add bn-a1b2 <queue-id> --type queued
bn queue rm bn-a1b2         # Alias for: bn link rm bn-a1b2 <queue-id>
```

### Auto-Removal Behavior

When a task is closed (status becomes `done`, `cancelled`, or `discarded`):
1. Check if task has any `queued` links
2. Automatically remove those links
3. Log the removal in the audit trail

## Feature 3: `bn ready` Integration

### Modified Behavior

When a queue exists with tasks:

1. **Queued tasks first** - All ready tasks that are in the queue appear first
2. **Then regular tasks** - Non-queued ready tasks appear after
3. **Within each group** - Sort by priority, then creation date

### Output Format

```bash
$ bn ready -H
Ready tasks (5):

  [QUEUED]
    [P1] bn-c3d4: Fix auth bug
    [P2] bn-e5f6: Add search feature

  [OTHER]
    [P0] bn-i9j0: Critical hotfix
    [P2] bn-k1l2: Refactor utils
    [P3] bn-m3n4: Nice-to-have cleanup
```

JSON output includes a `queued: true/false` field on each task.

### `bn orient` Integration

The orient output should mention if a queue exists:

```bash
$ bn orient -H
...
Current State:
  Total tasks: 42
  Queue: "January Sprint" (3 tasks)
  Ready: 5 (3 queued, 2 other)
...
```

## Feature 4: GUI Integration

### Visual Representation

- **Queue node** - Displayed as a distinct node shape (e.g., rounded rectangle or hexagon)
- **Queue color** - Unique color to distinguish from tasks/tests/ideas (suggested: teal/cyan)
- **Queued edges** - Links from tasks to queue shown with distinct style (dashed or different color)
- **Queued task highlight** - Tasks in queue could have a subtle glow or badge

### Interaction

- Drag tasks onto queue node to add them
- Right-click queue to see options (show members, delete)
- Queued tasks could be visually grouped near the queue node

## Feature 5: MCP Integration

### New Tools

| Tool | Description |
|------|-------------|
| `bn_queue_create` | Create the queue |
| `bn_queue_show` | Show queue and its tasks |
| `bn_queue_delete` | Delete the queue |

The existing `bn_link_add` and `bn_link_rm` tools support the `queued` type.

### Updated Resources

- `binnacle://ready` - Now includes `queued` field on tasks
- `binnacle://queue` - New resource for queue state (subscribable)

### New Prompt

- `prioritize_work` - Help operator decide what to add to queue based on project state

## Data Model

### Queue Schema (JSON)

```json
{
  "id": "bnq-a1b2",
  "type": "queue",
  "title": "January Sprint",
  "description": "Focus on auth and search features",
  "created_at": "2026-01-23T10:00:00Z",
  "updated_at": "2026-01-23T10:00:00Z"
}
```

### Link Schema (queued type)

```json
{
  "source": "bn-c3d4",
  "target": "bnq-a1b2",
  "link_type": "queued",
  "created_at": "2026-01-23T10:05:00Z"
}
```

### Storage

Queue stored in existing `tasks.jsonl` (it's just another entity type):

```
~/.local/share/binnacle/<repo-hash>/
├── tasks.jsonl       # Tasks, tests, ideas, AND queues
├── links.jsonl       # Includes queued links
├── ...
```

## Implementation Plan

### Phase 1: Core Queue Entity
- [ ] Add Queue model to `src/models/`
- [ ] Add `queue` type to entity enum
- [ ] Implement `bn queue create/show/delete` commands
- [ ] Add `queued` to link type enum
- [ ] Unit tests for queue model

### Phase 2: Ready Integration
- [ ] Modify `bn ready` to check for queue membership
- [ ] Sort queued tasks first
- [ ] Add `queued` field to ready output
- [ ] Update `bn orient` to show queue info
- [ ] Integration tests for ready ordering

### Phase 3: Auto-Removal
- [ ] Hook task close to check for queue links
- [ ] Remove `queued` links on close
- [ ] Add audit log entry for auto-removal
- [ ] Tests for auto-removal behavior

### Phase 4: GUI Support
- [ ] Add queue node rendering
- [ ] Distinct visual style for queue
- [ ] Show queued edges
- [ ] Drag-to-queue interaction

### Phase 5: MCP Tools
- [ ] Add `bn_queue_create`, `bn_queue_show`, `bn_queue_delete` tools
- [ ] Add `binnacle://queue` resource
- [ ] Add `prioritize_work` prompt
- [ ] MCP integration tests

## Testing Strategy

### Unit Tests
- Queue model serialization round-trip
- Link type validation for `queued`
- Auto-removal logic
- Ready sorting with queue

### Integration Tests
- Full queue lifecycle: create → add tasks → show → close task → verify removal → delete
- `bn ready` ordering with mix of queued and non-queued tasks
- Single queue constraint (create fails if exists)
- Link commands work with queue entities

### Edge Cases
- Queue with no tasks
- All queued tasks are blocked (none ready)
- Delete queue with tasks still linked
- Task in queue becomes blocked

## Success Criteria

1. Operators can create a queue and add tasks in under 10 seconds
2. `bn ready` clearly shows queued tasks first
3. Agents following `bn ready` naturally work on queued items
4. Completing queued work doesn't require manual cleanup
5. GUI shows queue state at a glance

## Open Questions

1. **Multiple queues later?** - Start with one, but design for extensibility
2. **Queue history?** - Track what was in queue over time for retrospectives?
3. **Queue notifications?** - Alert when queue is empty or all items blocked?

## Future Considerations

- Named queues (v2) - Multiple queues with different purposes
- Queue templates - Pre-defined queue configurations
- Queue metrics - Velocity, throughput tracking
- Queue sharing - Sync queue state across team members
