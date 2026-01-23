# PRD: GUI Process Management

**Idea:** bni-bbf8  
**Status:** Implemented ✅  
**Priority:** P2  
**Tags:** gui, dx, process-management

## Problem Statement

Currently, iterating on the GUI during development is friction-heavy:
1. If a GUI is already running, `bn gui` errors with "port in use"
2. Developers must manually find and kill the process
3. No way to query if a GUI is running or on which port
4. No graceful shutdown mechanism

This slows down the dev workflow where the browser stays connected and just needs a refresh after rebuild.

## Solution

Add PID file tracking and process management flags to `bn gui`:
- `--replace` - Kill existing GUI and take over its port
- `--stop` - Gracefully stop running GUI without starting a new one
- `--status` - Check if GUI is running (port, PID)

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| PID file location | `~/.local/share/binnacle/<repo-hash>/gui.pid` | Per-repo tracking, findable from CLI |
| Shutdown signal | SIGTERM with timeout, then SIGKILL | Graceful shutdown allowing WebSocket cleanup |
| Default behavior (port in use) | Error with `--replace` suggestion | Explicit is better than implicit |
| Stale PID handling | Auto-cleanup | Don't burden user with crashed process remnants |
| Process verification | Check if PID is `bn` process | Safety: don't kill unrelated processes |

## Detailed Behavior

### PID File Format

```json
{
  "pid": 12345,
  "port": 3030,
  "started_at": "2026-01-23T04:00:00Z"
}
```

Stored at: `~/.local/share/binnacle/<repo-hash>/gui.pid`

### `bn gui` (no flags, port in use)

```
$ bn gui
Error: Port 3030 is already in use.

A binnacle GUI is running (PID 12345, started 2h ago).
  • To replace it: bn gui --replace
  • To stop it: bn gui --stop
  • To use a different port: BN_GUI_PORT=3031 bn gui
```

### `bn gui --replace`

1. Check for existing PID file
2. If PID exists and process is running:
   - Verify process is a `bn` binary (check `/proc/<pid>/exe` or `ps`)
   - Send SIGTERM
   - Wait up to 5 seconds for graceful shutdown
   - If still running, send SIGKILL
3. Remove old PID file
4. Start new GUI server
5. Write new PID file
6. Output: `Replaced GUI (killed PID 12345). New GUI at http://127.0.0.1:3030 (PID 12346)`

### `bn gui --stop`

1. Check for existing PID file
2. If no PID file or process not running: `No GUI running for this repository.`
3. If running:
   - Verify process is a `bn` binary
   - Send SIGTERM
   - Wait up to 5 seconds
   - If still running, send SIGKILL
   - Remove PID file
4. Output: `Stopped GUI (PID 12345)`

### `bn gui --status`

```
$ bn gui --status
GUI running: yes
  PID: 12345
  Port: 3030
  URL: http://127.0.0.1:3030
  Started: 2026-01-23T04:00:00Z (2h ago)
```

Or if not running:
```
$ bn gui --status
GUI running: no
```

JSON output with default (no `-H`):
```json
{
  "running": true,
  "pid": 12345,
  "port": 3030,
  "url": "http://127.0.0.1:3030",
  "started_at": "2026-01-23T04:00:00Z"
}
```

### Stale PID Handling

If PID file exists but process is not running (crashed, rebooted):
- Auto-remove stale PID file
- Proceed with requested operation
- No user warning needed

### Process Verification

Before killing, verify the PID is actually a `bn` process:

**Linux:**
```rust
// Check /proc/<pid>/exe symlink
let exe = std::fs::read_link(format!("/proc/{}/exe", pid))?;
let is_bn = exe.file_name() == Some("bn") || exe.file_name() == Some("bn-gui");
```

**macOS:**
```rust
// Use ps command
let output = Command::new("ps").args(["-p", &pid.to_string(), "-o", "comm="]).output()?;
let comm = String::from_utf8_lossy(&output.stdout);
let is_bn = comm.trim() == "bn" || comm.trim() == "bn-gui";
```

If verification fails: `Error: PID 12345 is not a binnacle process. Refusing to kill.`

## Implementation Tasks

- [x] Add `GuiPidFile` struct with read/write/delete methods
- [x] Add `--replace` flag to `bn gui` command
- [x] Add `--stop` flag to `bn gui` command  
- [x] Add `--status` flag to `bn gui` command
- [x] Implement graceful shutdown (SIGTERM + timeout + SIGKILL)
- [x] Implement process verification (cross-platform)
- [x] Write PID file on GUI startup
- [x] Clean up PID file on graceful shutdown
- [x] Update `just gui` to use `--replace` for smoother workflow
- [x] Add tests for PID file management
- [x] Add tests for process verification
- [x] Update documentation

## Testing Strategy

### Unit Tests
- PID file serialization/deserialization
- Stale PID detection logic
- Process verification logic (mocked)

### Integration Tests
- `bn gui --status` when no GUI running
- `bn gui` → `bn gui --status` → `bn gui --stop` → `bn gui --status`
- `bn gui` → `bn gui --replace` works
- Stale PID cleanup on startup

## Open Questions

None - all clarifications resolved.

## Out of Scope

- True hot-reload (code changes without restart) - future enhancement
- Multiple simultaneous GUIs per repo - not needed
- Remote GUI management - local only
