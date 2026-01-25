# PRD: GUI Kill Subcommand

**Related Ideas:** bn-c9ea (seamless GUI hot-reload - out of scope, future work)  
**Status:** Draft  
**Priority:** P0  
**Tags:** gui, dx, process-management

## Problem Statement

While `bn gui --stop` exists for gracefully terminating the GUI server, the workflow for stopping a GUI during development or CI is suboptimal:

1. **Discoverability**: `--stop` is buried in `bn gui --help`. A dedicated subcommand `bn gui kill` is more discoverable and follows verb-noun patterns used elsewhere (e.g., `docker kill`).

2. **Force kill**: The current `--stop` always attempts graceful shutdown (SIGTERM + 5s timeout + SIGKILL). In development iteration and CI, users often want immediate termination to replace the binary without waiting.

3. **Script-friendliness**: A dedicated subcommand with clear exit codes is easier to use in build scripts and CI pipelines than flag combinations.

## Target Users

- **Developers** iterating on the binnacle codebase (rebuild → kill → restart cycle)
- **CI/automation** needing reliable process termination before binary replacement

## Solution

Add `bn gui kill` subcommand with a `--force` flag for immediate SIGKILL.

### Commands

```bash
# Graceful shutdown (equivalent to current --stop)
bn gui kill

# Immediate termination (SIGKILL, no waiting)
bn gui kill --force
bn gui kill -9    # Unix-style shorthand
```

### Behavior

| Command | Signal | Timeout | Use Case |
|---------|--------|---------|----------|
| `bn gui kill` | SIGTERM → SIGKILL | 5s | Normal shutdown, WebSocket cleanup |
| `bn gui kill --force` | SIGKILL | None | Dev iteration, CI, stuck processes |

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | GUI stopped successfully (or wasn't running) |
| 1 | Error (e.g., permission denied, PID file corruption) |

### Output

```bash
$ bn gui kill -H
Stopping GUI server (PID: 12345)...
GUI server stopped gracefully

$ bn gui kill --force -H
GUI server terminated (PID: 12345)

$ bn gui kill -H  # when not running
GUI server is not running

# JSON output (default)
$ bn gui kill
{"status":"stopped","pid":12345,"method":"sigterm"}

$ bn gui kill --force
{"status":"stopped","pid":12345,"method":"sigkill"}

$ bn gui kill  # when not running
{"status":"not_running"}
```

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Subcommand vs flag | Subcommand `kill` | More discoverable, matches `docker kill` pattern |
| `-9` shorthand | Alias for `--force` | Unix convention, muscle memory |
| Default behavior | Graceful (SIGTERM) | Allows WebSocket cleanup, matches `--stop` |
| Not running = success | Exit 0 | Idempotent, script-friendly |
| Keep `--stop` flag | Yes, as alias | Backward compatibility |

## Implementation Tasks

### CLI Changes

1. Add `GuiCommands` enum with `Kill` subcommand
2. Convert `bn gui` from flat struct to subcommand pattern:
   - `bn gui` (no subcommand) → start server (current behavior)
   - `bn gui kill` → stop server
   - `bn gui status` → show status
3. Add `--force` / `-9` flag to `kill` subcommand
4. Deprecate `--stop` and `--status` flags (keep working, add deprecation notice)

### Core Logic

5. Extract `stop_gui()` logic into reusable function with `force: bool` parameter
6. For `--force`: Skip SIGTERM, send SIGKILL immediately, minimal wait (500ms)

### Testing

7. Unit tests for force-kill logic
8. Integration tests:
   - `bn gui kill` when running → graceful stop
   - `bn gui kill --force` → immediate stop
   - `bn gui kill` when not running → success (idempotent)
   - Exit codes verified

### Documentation

9. Update `bn gui --help`
10. Update AGENTS.md GUI section if present
11. Update justfile `gui` target comments

## CLI Structure (After)

```
bn gui                    # Start server (current behavior)
bn gui kill              # Stop server gracefully
bn gui kill --force/-9   # Stop server immediately
bn gui status            # Show server status

# Deprecated (still work, but show notice):
bn gui --stop            → suggests `bn gui kill`
bn gui --status          → suggests `bn gui status`
bn gui --replace         → still works (no good subcommand equivalent)
```

## Migration Path

1. **Phase 1**: Add subcommands, keep flags working
2. **Phase 2** (future): Add deprecation warnings to flags
3. **Phase 3** (future): Remove deprecated flags

For P0 priority, only Phase 1 is in scope.

## Out of Scope

- Hot-reload / automatic reconnection (see bn-c9ea)
- Remote GUI management
- Multiple simultaneous GUIs
- Windows support for `-9` flag (Windows has no SIGKILL equivalent, `--force` uses `taskkill /F`)

## Success Criteria

1. `bn gui kill` stops running GUI with same behavior as `--stop`
2. `bn gui kill --force` terminates immediately without waiting
3. All existing functionality (`--stop`, `--status`, `--replace`) continues working
4. Exit codes are script-friendly (0 = success, including "not running")
5. Works on Linux and macOS (Windows: graceful degradation)

## Test Cases

```bash
# Test 1: Basic kill
bn gui &
sleep 2
bn gui kill
# Expect: GUI stops, exit 0

# Test 2: Force kill
bn gui &
sleep 2
bn gui kill --force
# Expect: GUI stops immediately, exit 0

# Test 3: Kill when not running
bn gui kill
# Expect: exit 0, output indicates not running

# Test 4: -9 shorthand
bn gui &
sleep 2
bn gui kill -9
# Expect: Same as --force

# Test 5: JSON output
bn gui kill
# Expect: valid JSON with status field
```
