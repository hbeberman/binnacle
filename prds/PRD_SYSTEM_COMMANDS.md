# PRD: `bn system` Command Subdomain

**Status:** Implemented
**Author:** Claude
**Date:** 2026-01-21

## Overview

Add a `bn system` subdomain for human-operated administrative commands. This subdomain will contain:
- `bn system init` - Initialize binnacle (moved from `bn init`)
- `bn system store` - Data import/export/inspection commands

## Motivation

1. **Separation of concerns**: Agent-facing commands (`bn task`, `bn ready`, `bn orient`) should be distinct from human administrative commands
2. **Data portability**: Users need to move binnacle data between machines and create backups
3. **Foundation for sync**: Establishing a serialization format now enables future sync features

## Non-Goals

- Real-time synchronization between machines (future work)
- Conflict resolution for concurrent edits (future work)
- Cloud storage integration (future work)

---

## Command Specification

### `bn system`

```
Human-operated system administration commands

Usage: bn system <COMMAND>

Commands:
  init   Initialize binnacle for this repository
  store  Data store management (import/export/inspect)
  help   Print this message or help for subcommands

Note: These commands are for human operators, not AI agents.
      Agents should use 'bn orient' which auto-initializes.
```

---

### `bn system init`

Moved from `bn init` with identical behavior.

```
Initialize binnacle for this repository

Usage: bn system init [OPTIONS]

Options:
  -H, --human  Output in human-readable format instead of JSON
  -h, --help   Print help
```

**Behavior:**
- Interactive prompts for AGENTS.md update, skills file creation
- Creates storage directory at `~/.local/share/binnacle/<repo-hash>/`
- Idempotent: safe to run multiple times

**Output (JSON):**
```json
{
  "initialized": true,
  "storage_path": "/home/user/.local/share/binnacle/45daf8c4722e/",
  "agents_md_updated": true,
  "skills_file_created": true,
  "codex_skills_file_created": true
}
```

---

### `bn system store`

```
Data store management commands

Usage: bn system store <COMMAND>

Commands:
  show    Display summary of current store contents
  export  Export store to archive file
  import  Import store from archive file
  help    Print this message or help for subcommands
```

---

### `bn system store show`

```
Display summary of current store contents

Usage: bn system store show [OPTIONS]

Options:
  -H, --human  Output in human-readable format instead of JSON
  -h, --help   Print help
```

**Output (JSON):**
```json
{
  "storage_path": "/home/user/.local/share/binnacle/45daf8c4722e/",
  "repo_path": "/home/user/projects/myproject",
  "tasks": {
    "total": 42,
    "by_status": {
      "pending": 5,
      "in_progress": 2,
      "blocked": 1,
      "done": 34
    }
  },
  "tests": {
    "total": 12,
    "linked": 10
  },
  "commits": {
    "total": 8
  },
  "files": {
    "tasks.jsonl": { "size_bytes": 45230, "entries": 156 },
    "commits.jsonl": { "size_bytes": 1204, "entries": 8 },
    "test-results.jsonl": { "size_bytes": 8920, "entries": 34 },
    "cache.db": { "size_bytes": 118784 }
  },
  "created_at": "2026-01-15T10:30:00Z",
  "last_modified": "2026-01-21T14:22:00Z"
}
```

**Human-readable output:**
```
Store: /home/user/.local/share/binnacle/45daf8c4722e/
Repo:  /home/user/projects/myproject

Tasks: 42 total
  - pending:     5
  - in_progress: 2
  - blocked:     1
  - done:        34

Tests: 12 total (10 linked to tasks)
Commits: 8 linked

Files:
  tasks.jsonl        44.2 KB  (156 entries)
  commits.jsonl       1.2 KB  (8 entries)
  test-results.jsonl  8.7 KB  (34 entries)
  cache.db          116.0 KB

Created:  2026-01-15 10:30:00
Modified: 2026-01-21 14:22:00
```

---

### `bn system store export`

```
Export store to archive file

Usage: bn system store export [OPTIONS] <OUTPUT>

Arguments:
  <OUTPUT>  Output path (use '-' for stdout)

Options:
      --format <FORMAT>  Export format [default: archive] [possible values: archive]
  -H, --human            Output in human-readable format instead of JSON
  -h, --help             Print help
```

**Formats:**

| Format | Description |
|--------|-------------|
| `archive` | gzip-compressed tar archive (`.tar.gz`) - default |

Additional formats may be added in future versions (e.g., `json`, `sqlite`).

**Archive Format:**

The export produces a gzip-compressed tar archive (`.tar.gz`) containing:
```
binnacle-export/
├── manifest.json      # Export metadata
├── tasks.jsonl        # Full task history
├── commits.jsonl      # Commit links
├── test-results.jsonl # Test execution history
└── config.json        # Store configuration
```

**manifest.json:**
```json
{
  "version": 1,
  "format": "binnacle-store-v1",
  "exported_at": "2026-01-21T14:30:00Z",
  "source_repo": "/home/user/projects/myproject",
  "binnacle_version": "0.1.0",
  "task_count": 42,
  "test_count": 12,
  "commit_count": 8
}
```

**Behavior:**
- Exports append-only log files (source of truth), not SQLite cache
- SQLite cache is rebuilt on import from log files
- Stdout support (`-`) enables piping: `bn system store export - | ssh remote "bn system store import -"`

**Output (JSON):**
```json
{
  "exported": true,
  "output_path": "/home/user/backup.tar.gz",
  "size_bytes": 12450,
  "task_count": 42,
  "test_count": 12,
  "commit_count": 8
}
```

**Errors:**
- `NotInitialized` - Binnacle not initialized for this repo
- `IoError` - Cannot write to output path

---

### `bn system store import`

```
Import store from archive file or storage folder

Usage: bn system store import [OPTIONS] <INPUT>

Arguments:
  <INPUT>  Input path: archive file (.tar.gz), storage folder, or '-' for stdin

Options:
      --type <TYPE>  Import type [default: replace] [possible values: replace, merge]
      --dry-run      Preview import without making changes (shows ID remappings)
  -H, --human        Output in human-readable format instead of JSON
  -h, --help         Print help
```

**Input Detection:**

| Input | Behavior |
|-------|----------|
| `-` | Read archive from stdin |
| Directory path | Import from folder (see Folder Import below) |
| File path | Read as `.tar.gz` archive |

**Import Types:**

| Type | Behavior |
|------|----------|
| `replace` | Error if binnacle already initialized. Clean import only. |
| `merge` | Append imported tasks/tests/commits to existing store. Handles ID conflicts. |

**Merge Semantics:**

When `--type merge` is used:
1. **Task ID conflicts**: Imported tasks with conflicting IDs are assigned new IDs (with loud warning)
2. **Dependency remapping**: Dependencies are updated to point to new IDs
3. **Test links remapping**: Test-task links updated accordingly
4. **Deduplication**: Exact duplicate entries (same content hash) are skipped
5. **Imported marker**: All imported tasks receive an `imported_on` timestamp

**Collision Warnings:**

When ID collisions are detected during merge, the CLI outputs prominent warnings:
```
⚠️  WARNING: 3 ID COLLISIONS DETECTED
   bn-abc1 → bn-def2 (existing: "Fix login bug", imported: "Add dashboard")
   bn-abc2 → bn-def3 (existing: "Update docs", imported: "Refactor API")
   bn-xyz9 → bn-ghi4 (existing: "Add tests", imported: "Deploy script")

These tasks will be assigned new IDs. Use --dry-run to preview without changes.
```

**Dry Run Mode:**

With `--dry-run`, the import is simulated without writing any data:
- Shows all ID remappings that would occur
- Shows collision warnings
- Reports counts of what would be imported
- Useful for previewing merge operations before committing

**Output (JSON):**
```json
{
  "imported": true,
  "dry_run": false,
  "input_path": "/home/user/backup.tar.gz",
  "import_type": "merge",
  "tasks_imported": 42,
  "tasks_skipped": 3,
  "tests_imported": 12,
  "commits_imported": 8,
  "collisions": 3,
  "id_remappings": {
    "bn-abc1": "bn-def2",
    "bn-abc2": "bn-def3",
    "bn-xyz9": "bn-ghi4"
  }
}
```

**Errors:**
- `AlreadyInitialized` - Store exists and `--type replace` (default)
- `InvalidArchive` - Archive format not recognized
- `VersionMismatch` - Archive from incompatible binnacle version
- `IoError` - Cannot read input path

**Folder Import:**

When importing from a storage folder (instead of an archive), the folder must have this structure:

```
<folder>/
├── tasks.jsonl         # Required (can be empty)
├── commits.jsonl       # Optional
├── test-results.jsonl  # Optional  
├── bugs.jsonl          # Optional
└── cache.db            # Ignored (rebuilt after import)
```

Use cases for folder import:
- **Legacy import**: Import old binnacle data from before export was implemented
- **Manual recovery**: Import from raw files without creating an archive first
- **Cross-machine transfer**: Copy a folder via rsync/scp and import directly

The same import types (`replace`, `merge`) and options (`--dry-run`) work for both archive and folder imports.

---

## Breaking Changes

### Removed: `bn init`

The top-level `bn init` command is removed entirely.

**Migration:**
- Humans: Use `bn system init`
- Agents: Should never have called `bn init` directly; use `bn orient` which auto-initializes

### Documentation Updates Required

1. **README.md**: Update quickstart to use `bn system init`
2. **AGENTS.md**: Verify no `bn init` references (agents use `bn orient`)
3. **Skills files**: Remove any `bn init` references from `~/.claude/skills/binnacle/SKILL.md`

---

## Implementation Tasks

| ID | Task | Priority | Dependencies |
|----|------|----------|--------------|
| bn-fc29 | Create bn system CLI subdomain structure | P1 | - |
| bn-d461 | Move init command to bn system init | P1 | bn-fc29 |
| bn-9312 | Implement bn system store show | P1 | bn-fc29 |
| bn-c84b | Implement bn system store export (with --format flag) | P1 | bn-fc29 |
| bn-df1d | Implement bn system store import (with --dry-run, collision warnings, imported_on) | P1 | bn-fc29 |
| bn-bdc1 | Update README and remove bn init references | P1 | - |
| bn-ab14 | Update AGENTS.md and skills files | P1 | - |
| bn-a6f5 | Add integration tests for bn system commands | P2 | bn-d461, bn-9312, bn-c84b, bn-df1d |
| bn-0c2a | Design safe bn system store clear with proper safeguards | P3 | - |

---

## Schema Changes

### Task: `imported_on` Attribute

A new optional field is added to the Task schema:

```rust
pub struct Task {
    // ... existing fields ...

    /// Timestamp when this task was imported from another store.
    /// None for tasks created locally.
    pub imported_on: Option<DateTime<Utc>>,
}
```

**Behavior:**
- Set automatically during `bn system store import --type merge`
- Not set for tasks created locally via `bn task create`
- Preserved through subsequent exports (tracks original import time)
- Queryable: future enhancement could add `bn task list --imported` filter

**JSON representation:**
```json
{
  "id": "bn-def2",
  "title": "Imported task example",
  "imported_on": "2026-01-21T15:30:00Z",
  ...
}
```

---

## Future Considerations

1. **Sync protocol**: Build on this serialization format for real-time sync
2. **Partial export**: Export specific tasks or date ranges
3. **Archive encryption**: Optional encryption for sensitive project data
4. **Remote storage**: Direct export to S3/GCS/etc.
5. **Safe clear command**: Design `bn system store clear` with proper safeguards (see bn-0c2a)

---

## Decisions Made

1. **`--format` flag**: Yes, added to export with `archive` as the only current option
2. **`--dry-run` flag**: Yes, added to import for previewing ID remappings
3. **Collision warnings**: Loud warnings displayed when IDs collide during merge
4. **`bn system store clear`**: Deferred - requires careful safeguard design (task bn-0c2a)
5. **`imported_on` attribute**: Added to Task schema for tracking imported tasks
