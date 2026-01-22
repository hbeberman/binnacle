# PRD: Generic Edge Model

## Overview

Replace the current `depends_on: Vec<String>` field embedded in Task/Bug entities with a first-class `Edge` model stored separately in `edges.jsonl`. This enables richer relationship types between entities, bidirectional edge support, and cleaner graph semantics.

## Motivation

The current dependency model has limitations:
1. **Limited to dependencies** - Can only express "A depends on B"
2. **Duplicate storage** - Each entity stores its own dependency list
3. **No metadata** - Can't track when/why/who created the relationship
4. **No cross-entity support** - Bugs can't relate to tasks elegantly
5. **No bidirectional edges** - `related_to` semantics require storing twice

A generic edge model enables:
- Milestones containing tasks/bugs
- Bugs linked to causing tasks
- Tests linked to tasks/bugs they verify
- Informational relationships without blocking semantics
- Future: weighted prioritization

## Data Model

### Edge Entity

```json
{
  "id": "bne-a1b2",
  "source": "bn-1234",
  "target": "bn-5678",
  "edge_type": "depends_on",
  "bidirectional": false,
  "weight": 1.0,
  "reason": "Feature requires auth to be implemented first",
  "created_at": "2026-01-21T10:00:00Z",
  "created_by": "agent-claude"
}
```

### Edge Types

| Type | Direction | Valid Source → Target | Semantics |
|------|-----------|----------------------|-----------|
| `depends_on` | directed | Task/Bug/Milestone → Task/Bug | Source blocks until target completes |
| `blocks` | directed | Task/Bug → Task/Bug/Milestone | Source prevents target from progressing |
| `related_to` | bidirectional | Any ↔ Any | Informational link, no blocking |
| `duplicates` | directed | Task → Task, Bug → Bug | Source is duplicate of target (source gets closed) |
| `fixes` | directed | Task → Bug | Task fixes the bug |
| `caused_by` | directed | Bug → Task/Commit | Bug was caused by this work |
| `supersedes` | directed | Task → Task, Bug → Bug | Source replaces target |
| `parent_of` | directed | Task/Milestone → Task/Bug | Containment relationship |
| `child_of` | directed | Task/Bug → Task/Milestone | Inverse of parent_of |
| `tests` | directed | Test → Task/Bug | Test verifies this work |

### Entity Type Constraints

```rust
fn validate_edge(edge: &Edge) -> Result<(), EdgeError> {
    match edge.edge_type {
        EdgeType::Fixes => {
            // Only Task → Bug
            require_source_type(edge, &["task"])?;
            require_target_type(edge, &["bug"])?;
        }
        EdgeType::Duplicates | EdgeType::Supersedes => {
            // Same type only (Task→Task or Bug→Bug)
            require_same_type(edge)?;
        }
        EdgeType::Tests => {
            // Test → Task/Bug
            require_source_type(edge, &["test"])?;
            require_target_type(edge, &["task", "bug"])?;
        }
        EdgeType::ParentOf => {
            // Task/Milestone → Task/Bug
            require_source_type(edge, &["task", "milestone"])?;
            require_target_type(edge, &["task", "bug"])?;
        }
        EdgeType::ChildOf => {
            // Task/Bug → Task/Milestone
            require_source_type(edge, &["task", "bug"])?;
            require_target_type(edge, &["task", "milestone"])?;
        }
        // Other types: permissive
        _ => Ok(())
    }
}
```

### Weight Field

Weight defaults to `1.0` for all edges. Reserved for future use in prioritization algorithms (e.g., critical path analysis, weighted dependency graphs). Not exposed in CLI initially.

## Storage

### New File: `edges.jsonl`

```
~/.local/share/binnacle/<repo-hash>/
├── tasks.jsonl
├── bugs.jsonl
├── edges.jsonl       # NEW
├── commits.jsonl
├── test-results.jsonl
├── cache.db
└── config.toml
```

### Bidirectional Edge Handling

Bidirectional edges (e.g., `related_to`) are stored **once** but hydrated in both directions when querying:

```rust
// When fetching edges for node "bn-1234":
fn get_edges_for_node(node_id: &str) -> Vec<HydratedEdge> {
    let mut edges = vec![];
    
    // Outbound edges (source = this node)
    edges.extend(storage.edges_where_source(node_id));
    
    // Inbound edges (target = this node)
    for edge in storage.edges_where_target(node_id) {
        if edge.bidirectional {
            // Flip direction for display
            edges.push(edge.flip());
        } else {
            edges.push(edge.as_inbound());
        }
    }
    
    edges
}
```

## CLI Commands

### New: `bn link`

```bash
# Create edge
bn link <source> <target> --type <edge_type> [--reason "..."]

# Remove edge (requires --type)
bn link rm <source> <target> --type <edge_type>

# Without --type, shows existing edges and guidance
bn link rm <source> <target>
# Output: "Found 2 edges between bn-1234 and bn-5678:
#   - depends_on (bn-1234 → bn-5678)
#   - related_to (bidirectional)
# Use --type <type> to remove a specific edge."

# List edges for a node
bn link list <id>

# Show all edges (debugging)
bn link list --all
```

### Examples

```bash
# Task depends on another task
bn link bn-1234 bn-5678 --type depends_on --reason "Needs auth first"

# Bug related to a task (bidirectional)
bn link bn-bug1 bn-task1 --type related_to

# Task fixes a bug
bn link bn-task1 bn-bug1 --type fixes

# Test covers a task
bn link bnt-001 bn-task1 --type tests

# Milestone contains tasks
bn link bn-milestone1 bn-task1 --type parent_of
bn link bn-milestone1 bn-task2 --type parent_of

# Remove a relationship
bn link rm bn-1234 bn-5678 --type depends_on

# List all edges for a node
bn link list bn-1234
```

### Deprecated: `bn dep`

The `bn dep` command is **removed** (breaking change). Use `bn link` instead:

| Old Command | New Command |
|-------------|-------------|
| `bn dep add A B` | `bn link A B --type depends_on` |
| `bn dep rm A B` | `bn link rm A B --type depends_on` |
| `bn dep show A` | `bn link list A` |

## Display Changes

### Task/Bug Show Output

```bash
$ bn task show bn-1234 -H

Task: bn-1234
Title: Implement user authentication
Status: In Progress
Priority: P1

Dependencies (depends_on):
  → bn-5678 "Set up database schema" [done]
  → bn-9abc "Configure JWT library" [pending]

Blocks:
  ← bn-def0 "User dashboard" [blocked]

Related:
  ↔ bn-bug1 "Login fails on mobile" [in_progress]

Tests:
  ← bnt-001 "Auth integration tests"
```

### Section Grouping

Edges are displayed grouped by type:
- **Dependencies** - `depends_on` edges (outbound)
- **Blocks** - `blocks` edges or inverse `depends_on` (inbound)
- **Related** - `related_to` edges (bidirectional)
- **Fixes** - `fixes` edges
- **Tests** - `tests` edges (inbound from Test nodes)
- **Children** - `parent_of` edges (outbound)
- **Parent** - `child_of` edges (outbound) or `parent_of` (inbound)

## Milestone Entity

### New Entity Type

```json
{
  "id": "bn-m001",
  "type": "milestone",
  "title": "v1.0 Release",
  "description": "First stable release",
  "target_date": "2026-03-01",
  "status": "in_progress",
  "created_at": "2026-01-21T10:00:00Z",
  "updated_at": "2026-01-21T10:00:00Z"
}
```

### CLI Commands

```bash
bn milestone create "v1.0 Release" [--target-date 2026-03-01]
bn milestone list
bn milestone show bn-m001
bn milestone update bn-m001 --title "..." --status done
bn milestone close bn-m001 --reason "Released!"
bn milestone progress bn-m001   # Show completion stats
```

### Progress Calculation

```bash
$ bn milestone progress bn-m001 -H

Milestone: bn-m001 "v1.0 Release"
Target: 2026-03-01

Progress: 7/12 items (58%)
  Tasks: 5/8 done
  Bugs: 2/4 fixed

Remaining:
  [ ] bn-task3 "Implement export" [in_progress]
  [ ] bn-task5 "Add logging" [pending]
  [ ] bn-task8 "Write docs" [pending]
  [ ] bn-bug2 "Memory leak on large files" [in_progress]
  [ ] bn-bug4 "UI glitch on resize" [pending]
```

Milestones are **informational/structural** - they don't auto-close when children complete.

## Migration

### Doctor Commands

```bash
# Migrate depends_on fields to edges
bn doctor --migrate-edges

# Clean up old depends_on fields after migration
bn doctor --clean-unused
```

### Migration Logic

```rust
fn migrate_edges(storage: &mut Storage) -> Result<MigrationReport> {
    let mut edges_created = 0;
    
    // Migrate Task.depends_on
    for task in storage.list_tasks()? {
        for dep_id in &task.depends_on {
            let edge = Edge::new(
                task.id.clone(),
                dep_id.clone(),
                EdgeType::DependsOn,
            );
            storage.add_edge(edge)?;
            edges_created += 1;
        }
    }
    
    // Migrate Bug.depends_on
    for bug in storage.list_bugs()? {
        for dep_id in &bug.depends_on {
            let edge = Edge::new(
                bug.id.clone(),
                dep_id.clone(),
                EdgeType::DependsOn,
            );
            storage.add_edge(edge)?;
            edges_created += 1;
        }
    }
    
    Ok(MigrationReport { edges_created })
}
```

### Clean Unused

After migration is verified, `--clean-unused` removes the `depends_on` field from all Task/Bug entities. This is a separate step to allow verification.

## Ready/Blocked Queries

`bn ready` and `bn blocked` continue to work based on `depends_on` edges only:

- **Ready**: No incomplete `depends_on` edges where this node is the source
- **Blocked**: At least one incomplete `depends_on` edge where this node is the source

Other edge types (e.g., `related_to`, `caused_by`) do not affect readiness.

## Implementation Phases

### Phase 1: Edge Infrastructure
- [ ] Add `Edge` model to `models/mod.rs`
- [ ] Add `EdgeType` enum with all types
- [ ] Add `edges.jsonl` to storage layer
- [ ] Add edge validation logic
- [ ] Add bidirectional edge hydration

### Phase 2: CLI Commands
- [ ] Implement `bn link <source> <target> --type <type>`
- [ ] Implement `bn link rm`
- [ ] Implement `bn link list`
- [ ] Remove `bn dep` commands
- [ ] Update `bn task show` / `bn bug show` to display edges

### Phase 3: Migration
- [ ] Implement `bn doctor --migrate-edges`
- [ ] Implement `bn doctor --clean-unused`
- [ ] Update storage to prefer edges over `depends_on` field

### Phase 4: Milestone Entity
- [ ] Add `Milestone` model
- [ ] Add `milestones.jsonl` storage
- [ ] Implement `bn milestone` commands
- [ ] Implement progress calculation

### Phase 5: Ready/Blocked Updates
- [ ] Update `bn ready` to use edge-based queries
- [ ] Update `bn blocked` to use edge-based queries
- [ ] Update cycle detection to use edges

### Phase 6: MCP & Documentation
- [ ] Update MCP tools for edges
- [ ] Add `bn_link_*` tools
- [ ] Add `bn_milestone_*` tools
- [ ] Update AGENTS.md
- [ ] Update PRD.md

## Test Plan

### Unit Tests
- Edge model serialization/deserialization
- Edge type validation (source/target constraints)
- Bidirectional edge hydration
- Migration logic

### Integration Tests
- `bn link` CRUD operations
- Edge display in `bn task show`
- `bn ready` / `bn blocked` with edges
- Cycle detection with edges
- Milestone progress calculation

### Migration Tests
- Migrate tasks with `depends_on` to edges
- Verify edge equivalence
- Clean unused fields
- Rollback scenarios

## Open Questions

1. ~~Should we support edge queries?~~ Yes - see `bn search link` below
2. **Should edges have their own IDs visible in CLI?** Currently `bne-xxxx` but not exposed.
3. ~~Should `bn link rm` require `--type`?~~ Yes - shows guidance without it

## Edge Search

### `bn search link`

Query edges by type, source, or target:

```bash
# Find all edges of a type
bn search link --type fixes

# Find edges targeting a specific bug
bn search link --type fixes --target bn-bug1

# Find edges from a specific source
bn search link --source bn-task1

# Combine filters
bn search link --type depends_on --source bn-milestone1

# Output format
$ bn search link --type fixes -H
2 edge(s) found:

bn-task3 → bn-bug1 (fixes)
  Reason: "Patched null pointer dereference"
  Created: 2026-01-20 by agent-claude

bn-task7 → bn-bug2 (fixes)
  Created: 2026-01-21 by henry
```

---

## Related Tasks

- `bn-9a0b`: Add cross-entity dependencies (task<->bug) - **superseded by this PRD**
- Bug tracking feature tasks - will use generic edges for bug→task relationships
