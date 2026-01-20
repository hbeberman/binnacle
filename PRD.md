# Binnacle

Binnacle is a Rust-based CLI application for AI agents and humans alike to track the state of a project, inspired by [steveyegge/beads](https://github.com/steveyegge/beads).

## Design Philosophy

- **JSON-first output** with `-H` flag for human-readable format
- **Minimal by default**, configurable for power users  
- **No repo pollution** - external storage by default (configurable)
- **Test-aware** - first-class test nodes linked to tasks
- **No commit message requirements** - explicit linking instead

## Entity Types

| Type | ID Format | Description |
|------|-----------|-------------|
| Task | `bn-xxxx` | Work item with status, priority, dependencies |
| Test | `bnt-xxxx` | Test node with command, linked to tasks |

## Storage

Default: External storage (doesn't touch repo)

```
~/.local/share/binnacle/<repo-hash>/
├── tasks.jsonl       # Tasks and test nodes (append-only)
├── commits.jsonl     # Commit-to-task links
├── test-results.jsonl # Test run history
├── cache.db          # SQLite index
└── config.toml       # Local settings
```

Configurable alternatives:
- Orphan branch (`binnacle-data`)
- Git notes (`refs/notes/binnacle`)

## CLI Commands

### Core
```bash
bn                              # Status summary (JSON)
bn -H                           # Human-readable status
bn init                         # Initialize for this repo
```

### Tasks (noun-verb)
```bash
bn task create "Title" [-p N] [-t tag] [-a user]
bn task list [--status X] [--priority N] [--tag T]
bn task show bn-a1b2
bn task update bn-a1b2 [--title|--desc|--priority|--status|...]
bn task close bn-a1b2 [--reason "..."]
bn task reopen bn-a1b2
bn task delete bn-a1b2
```

### Dependencies
```bash
bn dep add <child> <parent>     # child depends on parent
bn dep rm <child> <parent>
bn dep show bn-a1b2             # Show dependency graph
```

### Queries
```bash
bn ready                        # Tasks with no open blockers
bn blocked                      # Tasks waiting on dependencies
```

### Tests
```bash
bn test create "Name" --cmd "cargo test foo" [--dir "."] [--task bn-a1b2]
bn test list [--task bn-a1b2]
bn test show bnt-xxxx
bn test link bnt-xxxx bn-a1b2
bn test unlink bnt-xxxx bn-a1b2
bn test run [bnt-xxxx | --task bn-a1b2 | --all | --failed]
```

### Commit Tracking
```bash
bn commit link <sha> bn-a1b2    # Associate commit with task
bn commit list bn-a1b2          # Show commits linked to task
```

### Maintenance
```bash
bn doctor                       # Health check, detect issues
bn log [bn-a1b2]               # Audit trail of changes
bn compact                      # Summarize old closed tasks
bn sync                         # Push/pull (when sharing enabled)
```

### Configuration
```bash
bn config get <key>
bn config set <key> <value>
bn config list
```

### MCP Server
```bash
bn mcp serve                    # Start stdio MCP server
bn mcp manifest                 # Output tool definitions
```

## Data Schemas

### Task
```json
{
  "id": "bn-a1b2",
  "type": "task",
  "title": "Implement auth middleware",
  "description": "Add JWT validation to all API endpoints",
  "priority": 1,
  "status": "in_progress",
  "parent": null,
  "tags": ["backend", "security"],
  "assignee": "agent-claude",
  "depends_on": ["bn-c3d4"],
  "created_at": "2026-01-19T10:00:00Z",
  "updated_at": "2026-01-19T14:30:00Z",
  "closed_at": null,
  "closed_reason": null
}
```

### Test Node
```json
{
  "id": "bnt-0001",
  "type": "test",
  "name": "JWT validation tests",
  "command": "cargo test auth::jwt_validation",
  "working_dir": ".",
  "pattern": "tests/auth_test.rs::test_jwt_*",
  "linked_tasks": ["bn-a1b2"],
  "created_at": "2026-01-19T10:00:00Z"
}
```

## Status Flow

```
pending → in_progress → done
                     ↘ blocked
                     ↘ cancelled
done → reopened → in_progress → done
```

## Regression Detection

1. `bn test run` executes tests
2. On failure: look up linked tasks
3. If linked task is closed → auto-reopen with status `reopened`
4. Add note with failure context and commits since close

## MCP Integration

### Tools
All CLI operations exposed as MCP tools: `bn_task_create`, `bn_task_list`, `bn_ready`, `bn_test_run`, etc.

### Resources
- `binnacle://tasks` - All tasks (subscribable)
- `binnacle://ready` - Currently actionable tasks

### Prompts
- `start_work` - Begin working on a task
- `finish_work` - Complete current task properly
- `triage_regression` - Investigate a test failure
- `plan_feature` - Break down a feature into tasks
- `status_report` - Generate summary of current state

---

# Implementation Plan

## Testing Strategy

**CRITICAL: Each phase must have comprehensive unit and integration tests before moving to the next phase.**

### Testing Philosophy
1. **Test-first where possible** - Write tests before implementation for core logic
2. **Unit tests for each phase** - No phase is complete until tests pass
3. **Integration tests** - CLI behavior tests using temp directories
4. **Property-based tests** - For ID generation, serialization round-trips
5. **Dogfooding** - Use binnacle to track binnacle development once Phase 3 is ready

### Test Organization
```
tests/
├── unit/
│   ├── models_test.rs        # Task, Test, Commit serialization
│   ├── storage_test.rs       # JSONL, SQLite operations
│   ├── id_generation_test.rs # Hash ID properties
│   ├── dependency_test.rs    # Graph, cycle detection
│   └── regression_test.rs    # Reopen logic
├── integration/
│   ├── cli_task_test.rs      # Task CRUD via CLI
│   ├── cli_test_test.rs      # Test node operations
│   ├── cli_dep_test.rs       # Dependency commands
│   └── cli_mcp_test.rs       # MCP server tests
└── fixtures/
    ├── test_pass.sh          # Always exits 0
    ├── test_fail.sh          # Always exits 1
    └── sample_tasks.jsonl    # Pre-populated test data
```

---

## Phase 0: Project Setup ✅

**Goal:** Rust project scaffolding, basic structure

### Deliverables
- [x] `Cargo.toml` with dependencies (clap, serde, rusqlite, chrono, sha2, dirs, thiserror)
- [x] Project structure:
  ```
  src/
  ├── main.rs
  ├── lib.rs
  ├── cli/
  ├── models/
  ├── storage/
  ├── commands/
  └── mcp/
  ```
- [x] Test utilities module (assert_cmd, predicates, tempfile in dev-dependencies)

### Tests
- [x] Smoke test: `bn --version`, `bn --help` (7 integration tests + 9 unit tests)

---

## Phase 1: Core Task Management ✅

**Goal:** Basic task CRUD with JSON output

### Deliverables
- [x] Task model with all fields
- [x] Hash-based ID generation (`bn-xxxx`)
- [x] Storage layer (JSONL + SQLite cache)
- [x] Commands: `init`, `task create/list/show/update/close/delete`
- [x] Output: JSON default, `-H` for human-readable

### Unit Tests
- [x] ID generation: uniqueness, format validation
- [x] Task model: serialization round-trip
- [x] Storage: JSONL append/read, SQLite rebuild
- [x] Field validation: priority 0-4, status enum

### Integration Tests
- [x] `bn init` creates directory structure
- [x] Full CRUD round-trip
- [x] Filtering by status, priority, tags
- [x] `-H` flag output difference

### Test Summary (Phase 1)
- 24 unit tests (models, storage, commands)
- 23 CLI integration tests (task CRUD, filtering, output formats)
- 7 smoke tests (version, help, basic output)

---

## Phase 2: Dependencies & Queries ✅

**Goal:** Task relationships, smart queries

### Deliverables
- [x] Dependency graph storage
- [x] Cycle detection
- [x] Commands: `dep add/rm/show`, `ready`, `blocked`

### Unit Tests
- [x] Cycle detection (A→B→C→A fails)
- [x] Transitive blocking calculation
- [x] Self-dependency rejection

### Integration Tests
- [x] `bn dep add` creates relationship, rejects cycles
- [x] `bn ready` and `bn blocked` correctness

### Test Summary
- 41 unit tests (models, storage, commands including 12 new dependency tests)
- 35 CLI integration tests (task CRUD, deps, queries, output formats)
- 7 smoke tests (version, help, basic output)

---

## Phase 3: Test Nodes ✅

**Goal:** First-class test tracking with regression detection

### Deliverables
- [x] Test model (`bnt-xxxx`)
- [x] Test results storage
- [x] Commands: `test create/list/show/link/unlink/run`
- [x] Regression detection: auto-reopen closed tasks on failure

### Unit Tests
- [x] Test model serialization
- [x] Command execution, exit code capture
- [x] Regression detection logic

### Integration Tests
- [x] Test node CRUD
- [x] `bn test run` execution
- [x] Regression: closed task reopens on failure

### Test Summary (Phase 3)
- 51 unit tests (models, storage, commands including 11 new test node tests)
- 64 CLI integration tests (35 task + 29 test node tests)
- 7 smoke tests (version, help, basic output)
- **Total: 122 tests**

---

## Phase 4: Commit Tracking ✅

**Goal:** Explicit commit-to-task links

### Deliverables
- [x] Commit link model
- [x] Commands: `commit link/unlink/list`
- [x] Regression context includes commits since close

### Unit Tests
- [x] Commit link serialization, SHA validation
- [x] Lookup by task and by commit

### Integration Tests
- [x] Link/unlink operations
- [x] `bn commit list` output
- [x] Regression context includes commits

### Test Summary (Phase 4)
- 72 unit tests (models, storage, commands including 18 new commit tracking tests)
- 82 CLI integration tests (35 task + 29 test + 18 commit tests)
- 7 smoke tests (version, help, basic output)
- **Total: 161 tests**

---

## Phase 5: Maintenance Commands ✅

**Goal:** Health, history, compaction

### Deliverables
- [x] Commands: `doctor`, `log`, `compact`, `config`

### Unit Tests
- [x] Doctor checks: orphans, consistency
- [x] Compact logic: summarization preserves key info
- [x] Config get/set/list operations
- [x] Log filtering and entry retrieval

### Integration Tests
- [x] `bn doctor` detects known issues
- [x] `bn log` chronological output
- [x] `bn config` get/set/list
- [x] `bn compact` preserves data integrity

### Test Summary (Phase 5)
- 94 unit tests (models, storage, commands including 26 new maintenance tests)
- 109 CLI integration tests (35 task + 29 test + 18 commit + 27 maintenance tests)
- 7 smoke tests (version, help, basic output)
- **Total: 210 tests**

---

## Phase 6: MCP Server

**Goal:** Expose binnacle as MCP tools for agents

### Deliverables
- [ ] MCP server (`bn mcp serve`)
- [ ] All operations as MCP tools
- [ ] Resources and prompts

### Unit Tests
- [ ] Tool handlers return correct schema
- [ ] Invalid requests rejected gracefully

### Integration Tests
- [ ] Server starts, responds to `initialize`
- [ ] Each tool callable via MCP protocol

---

## Phase 7: Alternative Storage Backends (v1.1+)

**Goal:** Orphan branch and git notes support

### Deliverables
- [ ] Orphan branch backend
- [ ] Git notes backend
- [ ] `bn sync` for shared mode
- [ ] Migration between backends

---

## Phase 8: CI/CD Pipeline

**Goal:** Automated testing and quality checks

### Deliverables
- [ ] GitHub Actions workflow (`.github/workflows/ci.yml`)
- [ ] `cargo test` on push/PR
- [ ] `cargo clippy` linting
- [ ] `cargo fmt --check` formatting verification
- [ ] Release workflow for tagged versions

---

## Future Considerations (v2+)

- Agent sessions (multi-agent coordination)
- WIP limits per priority
- Focus mode with auto-commit-linking
- Test result trends and flakiness detection
- Natural language query interface
