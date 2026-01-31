//! Common test utilities for binnacle integration tests.
//!
//! Provides `TestEnv` for isolated test environments that don't pollute
//! the user's `~/.local/share/binnacle/` directory.

#![allow(dead_code)]

use assert_cmd::Command;
pub use tempfile::TempDir;

/// A test environment with isolated data storage in test mode.
///
/// Each `TestEnv` creates two temporary directories:
/// - `repo_dir`: Acts as the git repository root
/// - `data_dir`: Holds binnacle's data (via `BN_DATA_DIR` env var)
///
/// The `bn()` method returns a `Command` that automatically sets:
/// - `BN_DATA_DIR` and `BN_CONFIG_DIR` for data isolation
/// - `BN_TEST_MODE=1` to activate test mode protections
/// - `BN_TEST_ID` with a unique ID per TestEnv for parallel safety
///
/// Test mode provides additional safety guarantees:
/// - Production write protection (blocks writes to ~/.local/share/binnacle/)
/// - Blocks `bn sync --push` operations
/// - Test mode indicators in `bn orient`, `bn serve`, etc.
pub struct TestEnv {
    pub repo_dir: TempDir,
    pub data_dir: TempDir,
    /// Unique identifier for this test environment, derived from the data_dir name.
    /// Used as BN_TEST_ID for parallel test safety.
    test_id: String,
}

impl TestEnv {
    /// Create a new test environment with isolated directories.
    pub fn new() -> Self {
        let repo_dir = TempDir::new().unwrap();
        let data_dir = TempDir::new().unwrap();
        // Extract directory name as unique test ID (e.g., ".tmpXXXXXX")
        let test_id = data_dir
            .path()
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        Self {
            repo_dir,
            data_dir,
            test_id,
        }
    }

    /// Create a new test environment and initialize binnacle.
    pub fn init() -> Self {
        let env = Self::new();
        env.bn()
            .args(["session", "init", "--auto-global", "-y"])
            .assert()
            .success();
        env
    }

    /// Get a Command for the bn binary with isolated data directory.
    ///
    /// Sets `BN_DATA_DIR` and `BN_CONFIG_DIR` per-command for parallel safety,
    /// enables test mode with `BN_TEST_MODE=1` for production write protection,
    /// sets `BN_TEST_ID` for unique test identification,
    /// and clears agent-specific environment variables for test isolation.
    pub fn bn(&self) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(self.repo_dir.path());
        cmd.env("BN_DATA_DIR", self.data_dir.path());
        cmd.env("BN_CONFIG_DIR", self.data_dir.path());
        // Enable test mode for production write protection
        cmd.env("BN_TEST_MODE", "1");
        cmd.env("BN_TEST_ID", &self.test_id);
        // Clear agent-specific env vars for test isolation
        cmd.env_remove("BN_AGENT_ID");
        cmd.env_remove("BN_AGENT_NAME");
        cmd.env_remove("BN_AGENT_TYPE");
        cmd.env_remove("BN_MCP_SESSION");
        cmd.env_remove("BN_AGENT_SESSION");
        cmd.env_remove("BN_CONTAINER_MODE");
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

    /// Get the unique test ID for this environment.
    pub fn test_id(&self) -> &str {
        &self.test_id
    }
}

impl Default for TestEnv {
    fn default() -> Self {
        Self::new()
    }
}
