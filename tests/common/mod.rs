//! Common test utilities for binnacle integration tests.
//!
//! Provides `TestEnv` for isolated test environments that don't pollute
//! the user's `~/.local/share/binnacle/` directory.

#![allow(dead_code)]

use assert_cmd::Command;
pub use tempfile::TempDir;

/// A test environment with isolated data storage.
///
/// Each `TestEnv` creates two temporary directories:
/// - `repo_dir`: Acts as the git repository root
/// - `data_dir`: Holds binnacle's data (via `BN_DATA_DIR` env var)
///
/// The `bn()` method returns a `Command` that automatically sets `BN_DATA_DIR`
/// per-invocation, making tests parallel-safe.
pub struct TestEnv {
    pub repo_dir: TempDir,
    pub data_dir: TempDir,
}

impl TestEnv {
    /// Create a new test environment with isolated directories.
    pub fn new() -> Self {
        Self {
            repo_dir: TempDir::new().unwrap(),
            data_dir: TempDir::new().unwrap(),
        }
    }

    /// Create a new test environment and initialize binnacle.
    pub fn init() -> Self {
        let env = Self::new();
        env.bn().args(["system", "init"]).assert().success();
        env
    }

    /// Get a Command for the bn binary with isolated data directory.
    ///
    /// Sets `BN_DATA_DIR` per-command for parallel safety.
    pub fn bn(&self) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(self.repo_dir.path());
        cmd.env("BN_DATA_DIR", self.data_dir.path());
        cmd
    }

    /// Get the path to the repo directory.
    pub fn repo_path(&self) -> &std::path::Path {
        self.repo_dir.path()
    }

    /// Get the path to the repo directory (alias for backward compatibility).
    pub fn path(&self) -> &std::path::Path {
        self.repo_dir.path()
    }

    /// Get the path to the data directory.
    pub fn data_path(&self) -> &std::path::Path {
        self.data_dir.path()
    }
}

impl Default for TestEnv {
    fn default() -> Self {
        Self::new()
    }
}
