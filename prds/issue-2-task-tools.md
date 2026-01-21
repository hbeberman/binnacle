# PRD: Task Graph Organization Tools

**Issue:** [hbeberman/binnacle#2](https://github.com/hbeberman/binnacle/issues/2)  
**Status:** Draft  
**Author:** AI Assistant  
**Created:** 2026-01-20

## Overview

This PRD describes three interconnected features to help users better organize and understand their task graphs in Binnacle:

1. **Disconnected Component Detection** - Identify and list isolated subgraphs
2. **Close Task Warning System** - Warn when closing tasks with incomplete dependencies, with a new `Partial` status
3. **Extended Task Show with Blocker Info** - Show why a task isn't complete

These features address the common pain points of managing complex task dependency graphs, especially in multi-agent or long-running projects.

---

## Feature 1: Disconnected Component Detection

### Problem

When working on large projects, task graphs can become fragmented with multiple disconnected subgraphs. This happens when:
- Different agents work on separate features without linking them
- Tasks are created in batches without considering relationships
- Dependencies are deleted, leaving orphaned subgraphs

Currently, there's no easy way to identify these disconnected components or understand the structure of the task graph.

### Solution

Add a new `bn graph components` command that analyzes the task graph and lists all disconnected components with their root nodes.

### CLI Interface

```bash
# List all disconnected components
bn graph components

# JSON output (default)
{
  "component_count": 3,
  "components": [
    {
      "id": 1,
      "task_count": 15,
      "root_nodes": ["bn-a1b2", "bn-c3d4"],
      "task_ids": ["bn-a1b2", "bn-c3d4", "bn-e5f6", ...]
    },
    {
      "id": 2, 
      "task_count": 8,
      "root_nodes": ["bn-x9y8"],
      "task_ids": ["bn-x9y8", "bn-z7w6", ...]
    },
    {
      "id": 3,
      "task_count": 1,
      "root_nodes": ["bn-solo"],
      "task_ids": ["bn-solo"]
    }
  ],
  "suggestion": "Use 'bn dep add <child> <parent>' to connect components"
}

# Human-readable output
bn graph components -H

Task Graph Components: 3 disconnected subgraphs

Component 1 (15 tasks):
  Root nodes: bn-a1b2, bn-c3d4
  
Component 2 (8 tasks):
  Root nodes: bn-x9y8
  
Component 3 (1 task):
  Root nodes: bn-solo (isolated task)

Tip: Use 'bn dep add <child> <parent>' to connect components.
```

### Doctor Integration

Add a new check to `bn doctor` that reports disconnected components:

```rust
// In doctor() function
if component_count > 1 {
    issues.push(DoctorIssue {
        severity: "info".to_string(),
        category: "graph".to_string(),
        message: format!(
            "Task graph has {} disconnected components. Run 'bn graph components' for details.",
            component_count
        ),
        entity_id: None,
    });
}
```

### Implementation Details

#### Algorithm: Connected Components via Union-Find

```rust
/// Result of graph component analysis
#[derive(Serialize)]
pub struct GraphComponent {
    pub id: usize,
    pub task_count: usize,
    pub root_nodes: Vec<String>,  // Tasks with no dependencies in this component
    pub task_ids: Vec<String>,
}

#[derive(Serialize)]
pub struct ComponentsResult {
    pub component_count: usize,
    pub components: Vec<GraphComponent>,
    pub suggestion: Option<String>,
}

/// Find all disconnected components in the task graph.
/// Uses Union-Find algorithm for efficiency.
pub fn graph_components(repo_path: &Path) -> Result<ComponentsResult> {
    let storage = Storage::open(repo_path)?;
    let tasks = storage.list_tasks(None, None, None)?;
    
    // Build adjacency (treating edges as undirected for component finding)
    // A component is a set of tasks reachable from each other via dependencies
    
    // Union-Find implementation
    // ... (standard algorithm)
    
    // Identify root nodes within each component
    // Root = task with no dependencies OR all dependencies are in other components
}
```

#### Files to Modify

| File | Changes |
|------|---------|
| `src/cli/mod.rs` | Add `Graph` command enum with `Components` subcommand |
| `src/commands/mod.rs` | Add `graph_components()` function and output types |
| `src/storage/mod.rs` | Add `get_all_dependencies()` helper if needed |
| `src/main.rs` | Route `Graph::Components` command |
| `src/mcp/mod.rs` | Add `bn_graph_components` tool |

### Test Cases

1. **Single component** - All tasks connected -> returns 1 component
2. **Multiple components** - Separate task groups -> returns correct count
3. **Isolated tasks** - Tasks with no dependencies -> each is its own component  
4. **Empty graph** - No tasks -> returns 0 components
5. **Bidirectional detection** - A->B and C->D are separate even without reverse edges

---

## Feature 2: Close Task Warning System

### Problem

Users can close tasks (`bn task close`) even when their dependencies are still incomplete. This creates inconsistent states where a "done" task depends on pending work. While `bn doctor` catches this, it's better to warn proactively.

Additionally, when adding a new incomplete dependency to an already-done task, the task should reflect that it's no longer fully complete.

### Solution

1. **Warning on close**: `task_close()` warns if any dependencies are incomplete
2. **Force flag**: `--force` bypasses the warning
3. **Partial status**: New `TaskStatus::Partial` for tasks that were done but now have incomplete dependencies

### CLI Interface

```bash
# Attempting to close task with incomplete deps
$ bn task close bn-a1b2 --reason "done with my part"
{
  "error": "cannot_close",
  "message": "Task has 2 incomplete dependencies: bn-x1y2, bn-z3w4",
  "suggestion": "Use --force to close anyway, or complete dependencies first"
}

# Human-readable
$ bn task close bn-a1b2 --reason "done" -H
Error: Cannot close task bn-a1b2

This task depends on 2 incomplete tasks:
  - bn-x1y2: "Fix authentication" (pending)
  - bn-z3w4: "Update schema" (in_progress)

Use --force to close anyway, or complete the dependencies first.

# Force close
$ bn task close bn-a1b2 --reason "done" --force
{
  "id": "bn-a1b2",
  "status": "done",
  "warning": "Closed with 2 incomplete dependencies"
}
```

### Partial Status Behavior

```bash
# Task bn-done is already Done
$ bn task show bn-done
{ "id": "bn-done", "status": "done", ... }

# Add an incomplete dependency
$ bn dep add bn-done bn-pending
{
  "child": "bn-done",
  "parent": "bn-pending",
  "status_changed": true,
  "new_status": "partial",
  "message": "Task bn-done changed from done to partial (incomplete dependency added)"
}

# Task is now Partial
$ bn task show bn-done
{ "id": "bn-done", "status": "partial", ... }

# Complete the dependency
$ bn task close bn-pending --reason "finished"

# bn-done automatically transitions back to Done
# (checked during close/update operations)
```

### Status Transition Diagram

```
           +--------------------------------+
           |                                |
           v                                |
pending -> in_progress -> done -------------+
                          |                 |
                          | add incomplete  |
                          | dependency      |
                          v                 |
                       partial -------------+
                          |     all deps done
                          |     (auto-transition)
                          |
                          +---> done
```

### Implementation Details

#### Model Changes

```rust
// src/models/mod.rs
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    #[default]
    Pending,
    InProgress,
    Done,
    Blocked,
    Cancelled,
    Reopened,
    Partial,  // NEW: Was done, but has incomplete dependencies
}
```

#### Close Command Changes

```rust
// src/commands/mod.rs

#[derive(Serialize)]
pub struct TaskCloseResult {
    pub id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

#[derive(Serialize)]
pub struct TaskCloseError {
    pub error: String,
    pub message: String,
    pub incomplete_deps: Vec<IncompleteDep>,
    pub suggestion: String,
}

#[derive(Serialize)]
pub struct IncompleteDep {
    pub id: String,
    pub title: String,
    pub status: String,
}

pub fn task_close(
    repo_path: &Path, 
    id: &str, 
    reason: Option<String>,
    force: bool,  // NEW parameter
) -> Result<TaskCloseResult> {
    let mut storage = Storage::open(repo_path)?;
    let task = storage.get_task(id)?;
    
    // Check for incomplete dependencies
    let incomplete: Vec<_> = task.depends_on.iter()
        .filter_map(|dep_id| {
            storage.get_task(dep_id).ok()
        })
        .filter(|dep| dep.status != TaskStatus::Done && dep.status != TaskStatus::Cancelled)
        .collect();
    
    if !incomplete.is_empty() && !force {
        return Err(Error::CannotClose(TaskCloseError {
            error: "cannot_close".to_string(),
            message: format!("Task has {} incomplete dependencies", incomplete.len()),
            incomplete_deps: incomplete.iter().map(|d| IncompleteDep {
                id: d.id.clone(),
                title: d.title.clone(),
                status: format!("{:?}", d.status).to_lowercase(),
            }).collect(),
            suggestion: "Use --force to close anyway".to_string(),
        }));
    }
    
    // Proceed with close...
    let mut task = task;
    task.status = TaskStatus::Done;
    task.closed_at = Some(Utc::now());
    task.closed_reason = reason;
    task.updated_at = Utc::now();
    storage.update_task(&task)?;
    
    // Check for dependent tasks in Partial status that might transition to Done
    check_partial_transitions(&mut storage, id)?;
    
    Ok(TaskCloseResult {
        id: id.to_string(),
        status: "done".to_string(),
        warning: if !incomplete.is_empty() {
            Some(format!("Closed with {} incomplete dependencies", incomplete.len()))
        } else {
            None
        },
    })
}

/// Check if any tasks in Partial status can transition to Done
fn check_partial_transitions(storage: &mut Storage, completed_id: &str) -> Result<()> {
    let dependents = storage.get_dependents(completed_id)?;
    
    for dep_id in dependents {
        if let Ok(mut task) = storage.get_task(&dep_id) {
            if task.status == TaskStatus::Partial {
                // Check if all dependencies are now complete
                let all_done = task.depends_on.iter().all(|d| {
                    storage.get_task(d)
                        .map(|t| t.status == TaskStatus::Done || t.status == TaskStatus::Cancelled)
                        .unwrap_or(false)
                });
                
                if all_done {
                    task.status = TaskStatus::Done;
                    task.updated_at = Utc::now();
                    storage.update_task(&task)?;
                }
            }
        }
    }
    Ok(())
}
```

#### Dep Add Changes

```rust
// When adding a dependency to a Done task
pub fn dep_add(repo_path: &Path, child_id: &str, parent_id: &str) -> Result<DepAdded> {
    let mut storage = Storage::open(repo_path)?;
    
    // ... existing validation ...
    
    let mut child = storage.get_task(child_id)?;
    let parent = storage.get_task(parent_id)?;
    
    // Check if child is Done and parent is incomplete
    let status_changed = if child.status == TaskStatus::Done {
        let parent_incomplete = parent.status != TaskStatus::Done 
            && parent.status != TaskStatus::Cancelled;
        
        if parent_incomplete {
            child.status = TaskStatus::Partial;
            child.updated_at = Utc::now();
            true
        } else {
            false
        }
    } else {
        false
    };
    
    // Add dependency
    if !child.depends_on.contains(&parent_id.to_string()) {
        child.depends_on.push(parent_id.to_string());
    }
    storage.update_task(&child)?;
    
    Ok(DepAdded {
        child: child_id.to_string(),
        parent: parent_id.to_string(),
        status_changed,
        new_status: if status_changed { Some("partial".to_string()) } else { None },
        message: if status_changed {
            Some(format!("Task {} changed from done to partial", child_id))
        } else {
            None
        },
    })
}
```

#### Files to Modify

| File | Changes |
|------|---------|
| `src/models/mod.rs` | Add `Partial` variant to `TaskStatus` |
| `src/cli/mod.rs` | Add `--force` flag to `TaskCommands::Close` |
| `src/commands/mod.rs` | Update `task_close()`, `dep_add()`, add transition logic |
| `src/storage/mod.rs` | Add `get_dependents()` method, update `parse_status()` |
| `src/main.rs` | Pass `force` flag to command |
| `src/mcp/mod.rs` | Add `force` parameter to `bn_task_close` tool |

### Test Cases

1. **Close with incomplete deps (no force)** - Returns error with dep list
2. **Close with incomplete deps (force)** - Succeeds with warning
3. **Close with complete deps** - Succeeds normally  
4. **Add incomplete dep to done task** - Changes to Partial
5. **Complete dependency of partial task** - Auto-transitions to Done
6. **Add complete dep to done task** - Status unchanged
7. **Partial status serialization** - Round-trips correctly

---

## Feature 3: Extended Task Show with Blocker Info

### Problem

When a task isn't complete, users need to understand why. Currently, `bn task show` displays dependencies but doesn't indicate:
- Which dependencies are incomplete (blocking)
- What's blocking those dependencies (recursive blockers)
- A clear summary of why the task can't be worked on

### Solution

Enhance `bn task show` output to include blocker analysis when the task has incomplete dependencies.

### CLI Interface

```bash
$ bn task show bn-a1b2
{
  "id": "bn-a1b2",
  "title": "Implement user authentication",
  "status": "pending",
  "priority": 1,
  "depends_on": ["bn-x1y2", "bn-z3w4", "bn-done1"],
  "blocking_info": {
    "is_blocked": true,
    "blocker_count": 2,
    "direct_blockers": [
      {
        "id": "bn-x1y2",
        "title": "Set up database schema",
        "status": "in_progress",
        "assignee": "agent-1"
      },
      {
        "id": "bn-z3w4", 
        "title": "Define API contracts",
        "status": "pending",
        "blocked_by": ["bn-prereq1"]
      }
    ],
    "blocker_chain": [
      "bn-a1b2 <- bn-x1y2 (in_progress)",
      "bn-a1b2 <- bn-z3w4 <- bn-prereq1 (pending)"
    ],
    "summary": "Blocked by 2 incomplete dependencies. bn-x1y2 is in progress. bn-z3w4 is waiting on bn-prereq1."
  }
}

# Human-readable
$ bn task show bn-a1b2 -H

Task: bn-a1b2
Title: Implement user authentication
Status: pending (BLOCKED)
Priority: P1

Dependencies (3):
  x bn-x1y2: "Set up database schema" (in_progress) [assigned: agent-1]
  x bn-z3w4: "Define API contracts" (pending)
      +-- blocked by: bn-prereq1 (pending)
  * bn-done1: "Review requirements" (done)

Why blocked:
  This task is waiting on 2 incomplete dependencies.
  - bn-x1y2 is currently in progress
  - bn-z3w4 is pending, blocked by bn-prereq1

Next action: Complete bn-prereq1 to unblock the chain.
```

### Implementation Details

#### New Types

```rust
#[derive(Serialize)]
pub struct BlockingInfo {
    pub is_blocked: bool,
    pub blocker_count: usize,
    pub direct_blockers: Vec<DirectBlocker>,
    pub blocker_chain: Vec<String>,
    pub summary: String,
}

#[derive(Serialize)]
pub struct DirectBlocker {
    pub id: String,
    pub title: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub blocked_by: Vec<String>,
}

#[derive(Serialize)]
pub struct TaskShowResult {
    #[serde(flatten)]
    pub task: Task,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocking_info: Option<BlockingInfo>,
}
```

#### Task Show Enhancement

```rust
pub fn task_show(repo_path: &Path, id: &str) -> Result<TaskShowResult> {
    let storage = Storage::open(repo_path)?;
    let task = storage.get_task(id)?;
    
    // Analyze blocking status
    let blocking_info = if !task.depends_on.is_empty() {
        Some(analyze_blockers(&storage, &task)?)
    } else {
        None
    };
    
    Ok(TaskShowResult { task, blocking_info })
}

fn analyze_blockers(storage: &Storage, task: &Task) -> Result<BlockingInfo> {
    let mut direct_blockers = Vec::new();
    let mut blocker_chain = Vec::new();
    
    for dep_id in &task.depends_on {
        if let Ok(dep) = storage.get_task(dep_id) {
            if dep.status != TaskStatus::Done && dep.status != TaskStatus::Cancelled {
                // Find what's blocking this dependency
                let dep_blockers: Vec<String> = dep.depends_on.iter()
                    .filter(|d| {
                        storage.get_task(d)
                            .map(|t| t.status != TaskStatus::Done)
                            .unwrap_or(false)
                    })
                    .cloned()
                    .collect();
                
                direct_blockers.push(DirectBlocker {
                    id: dep.id.clone(),
                    title: dep.title.clone(),
                    status: format!("{:?}", dep.status).to_lowercase(),
                    assignee: dep.assignee.clone(),
                    blocked_by: dep_blockers.clone(),
                });
                
                // Build chain representation
                if dep_blockers.is_empty() {
                    blocker_chain.push(format!(
                        "{} <- {} ({})",
                        task.id, dep.id, format!("{:?}", dep.status).to_lowercase()
                    ));
                } else {
                    for blocker in &dep_blockers {
                        if let Ok(b) = storage.get_task(blocker) {
                            blocker_chain.push(format!(
                                "{} <- {} <- {} ({})",
                                task.id, dep.id, blocker,
                                format!("{:?}", b.status).to_lowercase()
                            ));
                        }
                    }
                }
            }
        }
    }
    
    let is_blocked = !direct_blockers.is_empty();
    let blocker_count = direct_blockers.len();
    
    let summary = if is_blocked {
        build_summary(&direct_blockers)
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
```

#### Files to Modify

| File | Changes |
|------|---------|
| `src/commands/mod.rs` | Update `task_show()`, add blocking analysis |
| `src/mcp/mod.rs` | Update `bn_task_show` response schema |

### Test Cases

1. **Task with no dependencies** - No blocking_info field
2. **Task with all deps done** - is_blocked: false
3. **Task with direct incomplete deps** - Shows blockers
4. **Task with transitive blockers** - Shows chain
5. **Mixed done/pending deps** - Only shows incomplete

---

## Implementation Plan

### Phase 1: Model Changes & Close Warning (Priority: High)

**Tasks:**
1. Add `Partial` status to `TaskStatus` enum
2. Update `parse_status()` to handle "partial"
3. Add `--force` flag to close command CLI
4. Implement warning logic in `task_close()`
5. Add tests for close with incomplete deps

**Estimated effort:** 2-3 hours

### Phase 2: Partial Status Auto-Transition (Priority: High)

**Tasks:**
1. Add `get_dependents()` to Storage
2. Implement status change in `dep_add()` for done->partial
3. Implement auto-transition check in `task_close()`
4. Add tests for partial transitions

**Estimated effort:** 2-3 hours

### Phase 3: Extended Task Show (Priority: Medium)

**Tasks:**
1. Create `BlockingInfo` and related types
2. Implement `analyze_blockers()` function
3. Update `task_show()` output
4. Update human-readable formatting
5. Add tests for blocker analysis

**Estimated effort:** 2-3 hours

### Phase 4: Graph Components (Priority: Medium)

**Tasks:**
1. Add `Graph` command enum to CLI
2. Implement Union-Find algorithm
3. Implement `graph_components()` command
4. Add component check to `bn doctor`
5. Add tests for component detection

**Estimated effort:** 3-4 hours

### Phase 5: MCP & Documentation (Priority: Low)

**Tasks:**
1. Add MCP tools for new commands
2. Update MCP schemas
3. Update PRD.md with new features
4. Update README if needed

**Estimated effort:** 1-2 hours

---

## Future Considerations (GUI - Phase 2)

These features are explicitly deferred to a follow-up:

1. **Highlight disconnected components in GUI**
   - Draw colored borders around component groups
   - Visual indicator for number of components
   
2. **Blocker visualization in graph view**
   - Red edges for blocking dependencies
   - Tooltip showing blocker chains

3. **Quick-merge UI**
   - Click two components to suggest merge points
   - Drag to create dependencies

---

## Success Criteria

1. `bn graph components` correctly identifies all disconnected subgraphs
2. `bn task close` warns appropriately and respects `--force`
3. `Partial` status auto-transitions correctly
4. `bn task show` clearly explains why a task is blocked
5. All new features have >80% test coverage
6. MCP tools are updated with new functionality
7. `bn doctor` reports disconnected components
