# PRD: Test Isolation with `BN_DATA_DIR`

**Status:** Implemented
**Author:** Claude
**Date:** 2026-01-22
**Implemented:** 2026-01-23

## Overview

Add a `BN_DATA_DIR` environment variable to override the default binnacle data directory (`~/.local/share/binnacle/`). This enables test isolation and prevents test pollution of the user's data directory.

## Motivation

The test suite creates ~6000+ orphan binnacle databases in `~/.local/share/binnacle/` because each test's temp directory gets a unique hash. These accumulate over time and:

1. Pollute the user's data directory
2. Waste disk space
3. Make `bn system store show` output noisy
4. Cannot be easily cleaned up

## Non-Goals

- Cleaning up existing orphan databases
- Changing how storage hashing works

---

## Specification

### Environment Variable

```
BN_DATA_DIR=/path/to/data  # Overrides ~/.local/share/binnacle
```

When set, all binnacle data (SQLite databases) will be stored under this directory instead of the default location.

### Behavior

| `BN_DATA_DIR` | Result |
|---------------|--------|
| Not set | Use `~/.local/share/binnacle/` (default) |
| Set to valid path | Use that path, create if needed |
| Set to invalid path | Error with clear message |

---

## Implementation (Completed)

We implemented a **two-tier approach** combining DI for the storage layer with env var for the command layer:

### Tier 1: DI-Based Storage APIs

Added explicit data directory parameters to the storage layer for direct use in unit tests:

```rust
// In src/storage/mod.rs

/// Get storage dir with explicit base directory (DI-friendly)
pub fn get_storage_dir_with_base(repo_path: &Path, base_dir: &Path) -> Result<PathBuf>

impl Storage {
    /// Initialize with explicit data directory
    pub fn init_with_data_dir(repo_path: &Path, data_dir: &Path) -> Result<Self>

    /// Open with explicit data directory
    pub fn open_with_data_dir(repo_path: &Path, data_dir: &Path) -> Result<Self>

    /// Check existence with explicit data directory
    pub fn exists_with_data_dir(repo_path: &Path, data_dir: &Path) -> Result<bool>
}
```

### Tier 2: Env Var for Command Layer

The command/MCP layer calls `Storage::open(repo_path)` internally without access to a data_dir parameter. For these tests, we use `BN_DATA_DIR` with proper synchronization:

```rust
// In src/lib.rs test_utils module

static INIT_ENV: Once = Once::new();

/// Set BN_DATA_DIR exactly once per process (before parallel tests run)
pub fn init_test_env_var() {
    INIT_ENV.call_once(|| {
        let dir = TempDir::new().unwrap();
        unsafe {
            std::env::set_var("BN_DATA_DIR", dir.path());
            TEST_DATA_DIR = Some(dir);
        }
    });
}
```

### Unified TestEnv

A single `TestEnv` struct in `src/lib.rs` provides both approaches:

```rust
pub struct TestEnv {
    pub repo_dir: TempDir,
    pub data_dir: TempDir,
}

impl TestEnv {
    /// Pure DI - for storage layer tests
    pub fn new() -> Self

    /// Env var based - for command/MCP layer tests
    pub fn new_with_env() -> Self

    /// DI methods for storage tests
    pub fn init_storage(&self) -> Storage
    pub fn open_storage(&self) -> Storage
    pub fn storage_exists(&self) -> bool
}
```

### Integration Tests

Integration tests use per-subprocess env vars (completely safe):

```rust
// In tests/common/mod.rs

impl TestEnv {
    pub fn bn(&self) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(self.repo_dir.path());
        cmd.env("BN_DATA_DIR", self.data_dir.path()); // Per-process, parallel-safe
        cmd
    }
}
```

---

## Files Modified

| File | Changes |
|------|---------|
| `src/storage/mod.rs` | Added `*_with_data_dir()` DI methods, `get_storage_dir_with_base()` |
| `src/lib.rs` | Added `test_utils` module with `TestEnv` and `init_test_env_var()` |
| `src/commands/mod.rs` | Updated tests to use shared `TestEnv::new_with_env()` |
| `src/mcp/mod.rs` | Updated tests to use shared `TestEnv::new_with_env()` |
| `tests/common/mod.rs` | Simplified to per-subprocess env var approach |
| `README.md` | Documented `BN_DATA_DIR` in Environment Variables section |

---

## Test Usage Patterns

### Storage Layer Tests (Pure DI)

```rust
#[test]
fn test_storage_init() {
    let env = TestEnv::new();  // No env var manipulation
    let storage = env.init_storage();
    assert!(storage.root.exists());
}
```

### Command Layer Tests (Env Var)

```rust
#[test]
fn test_task_create() {
    let temp = setup();  // Calls TestEnv::new_with_env()
    let result = task_create(temp.path(), "Test".into(), ...).unwrap();
    assert!(result.id.starts_with("bn-"));
}
```

### Integration Tests (Per-Subprocess)

```rust
#[test]
fn test_cli_task_create() {
    let env = TestEnv::init();  // Creates dirs, runs bn system init
    env.bn()
        .args(["task", "create", "Test"])
        .assert()
        .success();
}
```

---

## Thread Safety Notes

The `Once` guard ensures the env var is set exactly once before parallel tests run. While `set_var` is technically unsafe in multi-threaded contexts on POSIX systems, the synchronization minimizes risk:

1. `Once::call_once()` provides a barrier - only one thread enters
2. All subsequent tests see the same value
3. Integration tests use per-subprocess env vars (completely safe)

---

## Migration

- **All tests migrated**: Unit and integration tests now use isolated storage
- **No user impact**: `BN_DATA_DIR` is opt-in for advanced users
- **Documented**: README.md includes Environment Variables section
