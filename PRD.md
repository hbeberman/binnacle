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
bn orient                       # Onboarding for AI agents (auto-inits)
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

### Links (Dependencies & Relationships)
```bash
bn link add <source> <target> --type depends_on  # source depends on target
bn link rm <source> <target>
bn link list bn-a1b2            # Show links for an entity
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
All CLI operations exposed as MCP tools: `bn_task_create`, `bn_task_list`, `bn_ready`, `bn_test_run`, `bn_milestone_*`, `bn_link_*`, `bn_search_link`, etc.

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

## Phase 6: MCP Server ✅

**Goal:** Expose binnacle as MCP tools for agents

### Deliverables
- [x] MCP server (`bn mcp serve`)
- [x] All operations as MCP tools (38 tools with proper JSON schemas)
- [x] Resources (`binnacle://tasks`, `binnacle://ready`, `binnacle://blocked`, `binnacle://status`)
- [x] Prompts (`start_work`, `finish_work`, `triage_regression`, `plan_feature`, `status_report`)
- [x] Milestone tools (`bn_milestone_*`)
- [x] Search tools (`bn_search_link`)

### Unit Tests
- [x] Tool handlers return correct schema
- [x] Invalid requests rejected gracefully
- [x] Resource reading works correctly
- [x] Prompt generation works correctly

### Integration Tests
- [x] Server starts, responds to `initialize`
- [x] `bn mcp manifest` outputs valid JSON with tools, resources, prompts
- [x] All tool schemas validated

### Test Summary (Phase 6)
- 114 unit tests (models, storage, commands, mcp including 20 new MCP tests)
- 132 CLI integration tests (35 task + 29 test + 18 commit + 27 maintenance + 16 MCP + 7 smoke)
- **Total: 246 tests**

---

## Phase 7: Agent Onboarding (`bn orient`) ✅

**Goal:** Self-documenting tooling that helps AI agents discover and use binnacle

### Motivation
AI agents need consistent instructions about how to use binnacle. Rather than requiring manual AGENTS.md maintenance, binnacle provides:

1. **In containers**: Agent instructions are automatically injected via the entrypoint script, combining workflow rules (`bn system emit copilot-instructions`) with MCP lifecycle guidance (`bn system emit mcp-lifecycle`)

2. **Outside containers**: Users can generate instructions manually:
   ```bash
   # Generate AGENTS.md-style content
   bn system emit copilot-instructions -H > .github/copilot-instructions.md
   ```

The `bn orient` command serves as the canonical, always-up-to-date source of truth for project state and available work.

### Deliverables
- [x] `bn orient` command that:
  - Initializes binnacle for the repo if not already initialized
  - Outputs a brief summary of current project state (tasks, ready items, blocked items)
  - Explains binnacle's purpose and key commands
  - Returns JSON by default, human-readable with `-H`
- [x] `bn system emit copilot-instructions` for generating agent instructions
- [x] `bn system emit mcp-lifecycle` for MCP tool guidance in containers
- [x] Container entrypoint injects instructions automatically

### Agent Instructions Injection (Container Mode)

In containers, `entrypoint.sh` combines multiple instruction templates:

```bash
# Get workflow rules
COPILOT_INST=$(bn system emit copilot-instructions -H)

# Get MCP lifecycle guidance (orient/goodbye must use shell)
MCP_LIFECYCLE=$(bn system emit mcp-lifecycle -H)

# Combined prompt passed to copilot CLI
FULL_PROMPT="$COPILOT_INST

$MCP_LIFECYCLE

---

$BN_INITIAL_PROMPT"
```

This ensures agents receive consistent binnacle workflow instructions without requiring AGENTS.md files in the repository.

### Non-Container Usage

For local development or non-container environments, generate instructions for your editor:

```bash
# For GitHub Copilot (VS Code custom instructions)
bn system emit copilot-instructions -H > .github/copilot-instructions.md

# For manual AGENTS.md if needed
bn system emit copilot-instructions -H > AGENTS.md
```

### Example `bn orient` Output

```bash
$ bn orient -H
Binnacle - AI agent task tracker

This project uses binnacle (bn) for issue and test tracking.

Current State:
  Total tasks: 42
  Ready: 3 (bn-a1b2, bn-c3d4, bn-e5f6)
  Blocked: 2
  In progress: 1

Key Commands:
  bn              Status summary (JSON, use -H for human-readable)
  bn ready        Show tasks ready to work on
  bn task list    List all tasks
  bn show X       Show any entity by ID (works for bn-/bnt-/bnq- IDs)
  bn test run     Run linked tests

Run 'bn --help' for full command reference.
```

### Unit Tests
- [x] Orient output includes current task counts
- [x] Orient auto-initializes repo if needed
- [x] `bn session init` does NOT create AGENTS.md
- [x] `bn system emit copilot-instructions` outputs valid content

### Integration Tests
- [x] `bn orient` works in uninitialized repo (auto-inits)
- [x] `bn orient -H` produces human-readable output
- [x] `bn session init` does not create or modify AGENTS.md
- [x] Container entrypoint injects instructions correctly

### Test Summary (Phase 7)
- 132 unit tests (models, storage, commands, mcp including orient tests)
- 138 CLI integration tests (35 task + 29 test + 18 commit + 27 maintenance + 16 MCP + 15 orient + 7 smoke)
- **Total: 270 tests**

---

## Phase 8: Alternative Storage Backends ✅

**Goal:** Orphan branch and git notes support

### Deliverables
- [x] Orphan branch backend (`binnacle-data` branch)
- [x] Git notes backend
- [x] `bn sync` for shared mode
- [x] Migration between backends

### Implemented: Orphan Branch Backend

The orphan branch backend stores binnacle data in a git orphan branch named `binnacle-data`. 
This keeps all data within the repository without polluting the main branch or working tree.

**Key Features:**
- Uses git plumbing commands (hash-object, mktree, commit-tree, update-ref)
- Never modifies the working tree
- Full commit history for data changes
- Data persists across clones when branch is pushed

**Architecture:**
- `StorageBackend` trait abstracts storage operations
- `OrphanBranchBackend` implements the trait using git commands
- Files stored: `tasks.jsonl`, `commits.jsonl`, `test-results.jsonl`

**Test Summary (Phase 8 - Orphan Branch):**
- 8 unit tests (backend initialization, read/write, branch management)
- 9 integration tests (data persistence, commit history, no working tree pollution)

### Implemented: Git Notes Backend

The git notes backend stores binnacle data in git notes at `refs/notes/binnacle`.
Each JSONL file is stored as a separate note attached to a deterministic blob object.

**Key Features:**
- Uses `git notes` commands for storage
- Each file stored as a separate note with `binnacle:` prefix
- Data persists across clones when notes are pushed
- Supports the same operations as other backends

### Implemented: Sync Command

The `bn sync` command enables pushing/pulling binnacle data with remotes.

**Key Features:**
- Works with orphan-branch backend
- Supports `--push` only or `--pull` only modes
- Configurable remote name (default: origin)

### Implemented: Storage Migration

The `bn system migrate` command enables switching between backends.

**Key Features:**
- Migrates data from current (file) backend to orphan-branch or git-notes
- Supports `--dry-run` to preview changes
- Validates target backend type

**Test Summary (Phase 8 - Complete):**
- 8 unit tests for orphan branch backend
- 9 integration tests for orphan branch
- 5 integration tests for migration (file→orphan, file→git-notes)
- Sync command tested via manual verification

---

## Phase 9: CI/CD Pipeline ✅

**Goal:** Automated testing and quality checks

### Deliverables
- [x] GitHub Actions workflow (`.github/workflows/ci.yml`)
- [x] `cargo test` on push/PR
- [x] `cargo clippy` linting
- [x] `cargo fmt --check` formatting verification
- [x] Release workflow for tagged versions (`.github/workflows/release.yml`)

### Workflows Created

**CI Workflow (`.github/workflows/ci.yml`):**
- Runs on push/PR to main/master branches
- Three parallel jobs: test, clippy, fmt
- Uses cargo caching for faster builds
- Fails on any clippy warnings (`-D warnings`)

**Release Workflow (`.github/workflows/release.yml`):**
- Triggered by version tags (e.g., `v0.1.0`)
- Builds binaries for multiple platforms:
  - Linux x86_64
  - macOS x86_64 and ARM64
  - Windows x86_64
- Creates GitHub Release with auto-generated notes
- Uploads platform-specific archives

---

## Phase 10: Crates.io Publish Workflow (Alpha) ✅

**Goal:** Publish alpha releases to crates.io via GitHub Actions with safety checks.

### Deliverables
- [x] GitHub Actions workflow (`.github/workflows/cd.yml`) triggered by releases (version tags)
- [x] Preflight validation job: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features`
- [x] Publish job guarded by preflight success and `CARGO_REGISTRY_TOKEN` secret
- [x] Dry-run step (`cargo publish --dry-run`) before actual publish
- [x] Tag format documented in README (alpha versioning policy)

### Workflow Outline
1. On tag push matching `v*`.
2. Run preflight checks and `cargo publish --dry-run`.
3. If all checks pass, run `cargo publish` with the registry token.
4. Fail fast on any step; no retries on publish.

---

## Phase 11: Queue Nodes ✅

**Goal:** Agent task prioritization through a work queue

### Motivation

Introduce a **Queue** entity type (`bnq-xxxx`) that serves as a work pool for agent task prioritization. Operators can add tasks to the queue to signal they should be worked on first.

### Deliverables
- [x] Queue model (`bnq-xxxx` IDs)
- [x] Commands: `bn queue create/show/delete`
- [x] `queued` link type for task-to-queue membership
- [x] `bn ready` integration (queued tasks sorted first)
- [x] `bn orient` shows queue info
- [x] Auto-removal of `queued` links when tasks are closed
- [x] MCP tools: `bn_queue_create`, `bn_queue_show`, `bn_queue_delete`
- [x] MCP resource: `binnacle://queue`
- [x] MCP prompt: `prioritize_work`
- [x] GUI support (queue node rendering, visual styles)
- [x] Documentation updates

### Key Design Decisions

1. **Single global queue** - One queue per repo, keeping the model simple
2. **Link-based membership** - Uses `bn link add/rm` with `--type queued`
3. **Unordered membership** - Tasks sorted by priority (0-4), not explicit position
4. **Auto-removal on close** - Completed tasks automatically leave the queue

### Commands

```bash
bn queue create "Title" [--description "..."]   # Create queue (one per repo)
bn queue show                                   # Show queue and its tasks
bn queue delete                                 # Delete queue (removes all queue links)
bn link add bn-xxxx bnq-xxxx --type queued      # Add task to queue
bn link rm bn-xxxx bnq-xxxx                     # Remove task from queue
```

### Test Summary (Phase 11)
- Unit tests for Queue model serialization
- Integration tests for queue CRUD, ready ordering, auto-removal
- MCP integration tests for queue tools and resources

See `prds/PRD_QUEUE_NODES.md` for full specification.

---

## Future Considerations (v2+)

- Agent sessions (multi-agent coordination)
- WIP limits per priority
- Focus mode with auto-commit-linking
- Test result trends and flakiness detection
- Natural language query interface
