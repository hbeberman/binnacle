# PRD: Explicit Initialization with `--init` Flag

**Status:** Implemented
**Author:** Claude
**Date:** 2026-01-22
**Implemented:** 2026-01-23

## Overview

Require `--init` flag on `bn orient` to create new databases. When run from a subdirectory of a git repo, automatically use the git root (no prompt needed since nested stores are not supported).

## Motivation

`bn orient` silently auto-initializes a new database if none exists. This leads to accidental database creation, especially when combined with git root detection—user might not realize they're in a different context.

## Non-Goals

- **Nested binnacle stores within a single git repo** (one repo = one store; use tags or git submodules instead)
- Requiring `--init` on other commands (they just error if not initialized)
- Parent directory scanning for non-git nested stores
- Interactive prompts (git root is always used automatically)

## Dependencies

- Requires: PRD_GIT_ROOT_DETECTION (git root detection function) ✅ Already implemented

---

## Specification

### Behavior Matrix

| Scenario | Without `--init` | With `--init` |
|----------|------------------|---------------|
| DB exists at git root | ✅ Use it | ✅ Use it (no-op) |
| DB exists, CWD != git root | ✅ Use git root DB | ✅ Use git root DB (no-op) |
| No DB, CWD == git root | ❌ Error: "Run with --init" | ✅ Create at git root |
| No DB, CWD != git root | ❌ Error: "Run with --init" | ✅ Create at git root (auto) |
| No DB, non-git directory | ❌ Error: "Run with --init" | ✅ Create at CWD |

### Subdirectory Behavior

When `--init` is passed from a subdirectory of a git repo, binnacle automatically uses the git root:

```
$ cd /home/user/repos/myproject/src/lib
$ bn orient --init
Initializing binnacle at git root: /home/user/repos/myproject
```

No prompt is shown—nested stores within a git repo are not supported.

### CLI Change

```
bn orient [--init] [-H|--human]

Options:
  --init   Initialize a new binnacle database (non-interactive, for AI agents)
  -H       Output in human-readable format instead of JSON
```

### Error Message (Implemented)

Human-readable (`-H`):

```
Error: No binnacle database found.

To initialize a new database:
    bn system init        # Interactive, recommended for humans
    bn orient --init      # Non-interactive, for AI agents

Database location: /home/user/repos/myproject
```

JSON (default):

```json
{"error": "No binnacle database found", "hint": "Human should run 'bn system init' (interactive). AI agents: use 'bn orient --init' (non-interactive, conservative defaults).", "path": "/home/user/repos/myproject"}
```

---

## Implementation

**Files modified:**

| File | Change |
|------|--------|
| `src/cli/mod.rs` | Add `--init` flag to Orient command (struct variant) |
| `src/main.rs` | Handle NotInitialized error with custom message, wire `--init` flag |
| `src/commands/mod.rs` | Modify `orient()` to accept `allow_init: bool` parameter |
| `tests/cli_orient_test.rs` | Update tests for new behavior |
| `AGENTS.md` | Document `bn system init` for humans, `bn orient --init` for AI |
| `README.md` | Update quickstart and command reference |

### Deviation from Original Proposal

1. **Error message recommends two options**: Instead of just suggesting `bn orient --init`, the error message now recommends:
   - `bn system init` for humans (interactive, with helpful prompts)
   - `bn orient --init` for AI agents (non-interactive, conservative defaults)

2. **Documentation emphasizes human vs AI paths**: AGENTS.md explicitly tells AI agents that humans should run `bn system init`, and `--init` is a fallback for when human intervention is unavailable.

---

## Testing

Tests implemented in `tests/cli_orient_test.rs`:

- `test_orient_without_init_fails_when_not_initialized` - Verifies error with hint
- `test_orient_with_init_creates_database` - Verifies DB creation
- `test_orient_works_when_already_initialized` - Verifies existing DB works without `--init`
- `test_orient_init_is_idempotent` - Verifies `--init` on existing DB is no-op
- `test_orient_error_json_is_valid` - Verifies JSON error output is valid and contains expected fields

Unit tests in `src/commands/mod.rs`:

- `test_orient_without_init_fails_when_not_initialized`
- `test_orient_with_init_creates_database`

## Migration

- Existing databases: No change, `bn orient` works without `--init`
- New users: Must use `bn orient --init` (AI) or `bn system init` (human) once per repository
- Scripts/MCP: Add `--init` to initialization commands

## Open Questions

1. Should other commands (like `bn task create`) also require explicit init?
   - **Resolution**: No, they error with "not initialized" message pointing to `bn orient --init`
