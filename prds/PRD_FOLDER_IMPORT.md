# PRD: Import from Binnacle Storage Folder

**Status:** Implemented  
**Author:** Claude  
**Date:** 2026-01-21

## Overview

Extend `bn system store import` to accept a directory path containing raw binnacle storage files, in addition to the existing `.tar.gz` archive support.

## Motivation

Users may have existing binnacle storage folders (`~/.local/share/binnacle/<repo-hash>/`) from before the export feature was implemented. They need a way to import this data into their current binnacle workspace without manually creating an archive first.

### Use Cases

1. **Legacy import**: User has old binnacle data and wants to import it to a new project
2. **Manual recovery**: Import from raw files without going through export step
3. **Cross-machine transfer**: Copy a folder via rsync/scp and import directly

## Non-Goals

- Importing from SQLite cache alone (requires source-of-truth JSONL files)
- Automatic discovery of storage folders
- Batch import from multiple folders

---

## Command Specification

### Updated: `bn system store import`

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

### Input Detection Logic

| Input | Behavior |
|-------|----------|
| `-` | Read archive from stdin |
| Directory path | Import from folder |
| File path | Read as `.tar.gz` archive |

### Folder Structure (expected)

```
<folder>/
├── tasks.jsonl         # Required (can be empty)
├── commits.jsonl       # Optional
├── test-results.jsonl  # Optional  
├── bugs.jsonl          # Optional
└── cache.db            # Ignored (rebuilt after import)
```

### Validation

- **Required**: `tasks.jsonl` must exist (can be empty)
- **Ignored**: `cache.db` is skipped; cache is rebuilt after import
- **Missing files**: Optional files treated as empty

### Output

Same JSON/human-readable output as archive import:

```json
{
  "imported": true,
  "dry_run": false,
  "input_path": "/home/user/.local/share/binnacle/abc123/",
  "import_type": "merge",
  "tasks_imported": 42,
  "tasks_skipped": 0,
  "tests_imported": 12,
  "commits_imported": 8,
  "collisions": 3,
  "id_remappings": {
    "bn-abc1": "bn-def2"
  }
}
```

### Errors

- `InvalidInput` - Directory missing `tasks.jsonl`
- `InvalidInput` - JSONL parsing error
- `IoError` - Cannot read directory/files

---

## Implementation Tasks

| ID | Task | Priority | Dependencies |
|----|------|----------|--------------|
| TBD | Update CLI help text for INPUT argument | P1 | - |
| TBD | Add input type detection (dir vs file vs stdin) | P1 | - |
| TBD | Implement system_store_import_from_folder() | P1 | - |
| TBD | Validate folder structure (require tasks.jsonl) | P1 | - |
| TBD | Support --dry-run for folder import | P1 | - |
| TBD | Add integration tests for folder import | P2 | - |
| TBD | Update PRD_SYSTEM_COMMANDS.md with folder support | P2 | - |

---

## Technical Notes

### Code Changes

**`src/cli/mod.rs`**: Update help text for `<INPUT>` argument.

**`src/commands/mod.rs`**: 
- Add detection logic at start of `system_store_import()`
- Extract shared import logic (ID remapping, task creation) into helper
- Add `system_store_import_from_folder()` function

### Shared Logic with Archive Import

The following logic is reused:
- ID collision detection and remapping
- Dependency remapping
- `imported_on` timestamp for merge mode
- Cache rebuild after import
- Dry-run preview

### Manifest Handling

Folders don't contain `manifest.json`. For dry-run output, generate counts by reading JSONL files without importing.

---

## Future Considerations

1. **Auto-detect storage folders**: `bn system store import --discover` to find all folders
2. **Folder export**: `bn system store export --format folder <OUTPUT_DIR>`
3. **Incremental import**: Only import entries newer than last import
