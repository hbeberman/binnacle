//! Binnacle - A project state tracking library for AI agents and humans.
//!
//! This library provides the core functionality for the `bn` CLI tool,
//! including task management, test tracking, and dependency handling.

#[cfg(not(target_arch = "wasm32"))]
pub mod action_log;
#[cfg(not(target_arch = "wasm32"))]
pub mod agents;
#[cfg(not(target_arch = "wasm32"))]
pub mod cli;
#[cfg(not(target_arch = "wasm32"))]
pub mod commands;
pub mod config;
#[cfg(not(target_arch = "wasm32"))]
pub mod container;
#[cfg(not(target_arch = "wasm32"))]
pub mod github;
pub mod gui;
#[cfg(not(target_arch = "wasm32"))]
pub mod mcp;
pub mod models;
#[cfg(not(target_arch = "wasm32"))]
pub mod storage;
#[cfg(not(target_arch = "wasm32"))]
pub mod sys;
#[cfg(all(feature = "tmux", not(target_arch = "wasm32")))]
pub mod tmux;
#[cfg(all(feature = "tui", not(target_arch = "wasm32")))]
pub mod tui;
#[cfg(feature = "wasm")]
pub mod wasm;

/// Test utilities for isolated test environments.
#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) mod test_utils {
    use std::path::Path;
    use tempfile::TempDir;

    use crate::storage::Storage;

    /// Test environment with isolated storage using dependency injection.
    ///
    /// For **storage layer tests**: Use `TestEnv::new()` + `init_storage()` for pure DI.
    /// For **command/MCP layer tests**: Use `TestEnv::new_isolated()` (preferred) or `TestEnv::new_with_env()`.
    pub struct TestEnv {
        /// Simulated repository directory
        pub repo_dir: TempDir,
        /// Isolated data storage directory (for DI-based tests)
        pub data_dir: TempDir,
        /// Whether env vars were set by this instance (for cleanup)
        env_vars_set: bool,
        /// Whether thread-local override was set by this instance (for cleanup)
        thread_local_set: bool,
    }

    impl TestEnv {
        /// Create a new test environment with isolated directories (pure DI).
        /// Use this for storage layer tests that call Storage methods directly.
        pub fn new() -> Self {
            Self {
                repo_dir: TempDir::new().unwrap(),
                data_dir: TempDir::new().unwrap(),
                env_vars_set: false,
                thread_local_set: false,
            }
        }

        /// Create a new test environment using thread-local isolation.
        /// Use this for command/MCP layer tests that call the public API.
        ///
        /// This sets a thread-local data directory override, allowing tests to run
        /// in parallel without `#[serial]`. The override is cleared on drop.
        ///
        /// Prefer this over `new_with_env()` for new tests.
        pub fn new_isolated() -> Self {
            let env = Self {
                repo_dir: TempDir::new().unwrap(),
                data_dir: TempDir::new().unwrap(),
                env_vars_set: false,
                thread_local_set: true,
            };
            // Set thread-local override for this test's data directory
            crate::storage::set_data_dir_override(env.data_path().to_path_buf());
            env
        }

        /// Create a new test environment that uses BN_DATA_DIR env var.
        /// Use this for command/MCP layer tests that call the public API.
        ///
        /// IMPORTANT: Tests using this MUST be marked with #[serial] to avoid
        /// environment variable races between parallel tests.
        ///
        /// DEPRECATED: Prefer `new_isolated()` for new tests, which uses thread-local
        /// storage and doesn't require `#[serial]`.
        ///
        /// This also sets `BN_TEST_MODE=1` and `BN_TEST_ID` for test mode isolation.
        #[allow(dead_code)]
        pub fn new_with_env() -> Self {
            let env = Self {
                repo_dir: TempDir::new().unwrap(),
                data_dir: TempDir::new().unwrap(),
                env_vars_set: true,
                thread_local_set: false,
            };
            // Extract a unique test ID from the data directory name
            let test_id = env
                .data_path()
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            // Set environment variables for test isolation
            // Clear agent-specific env vars to ensure test isolation
            // SAFETY: This is safe because tests using new_with_env() are marked #[serial]
            unsafe {
                std::env::set_var("BN_DATA_DIR", env.data_path());
                std::env::set_var("BN_TEST_MODE", "1");
                std::env::set_var("BN_TEST_ID", &test_id);
                std::env::remove_var("BN_AGENT_ID");
                std::env::remove_var("BN_AGENT_NAME");
                std::env::remove_var("BN_AGENT_TYPE");
                std::env::remove_var("BN_MCP_SESSION");
                std::env::remove_var("BN_AGENT_SESSION");
                std::env::remove_var("BN_CONTAINER_MODE");
            }
            env
        }

        /// Get the path to the simulated repository.
        pub fn path(&self) -> &Path {
            self.repo_dir.path()
        }

        /// Get the path to the isolated data directory.
        pub fn data_path(&self) -> &Path {
            self.data_dir.path()
        }

        /// Initialize storage for this test environment (DI-based).
        pub fn init_storage(&self) -> Storage {
            Storage::init_with_data_dir(self.path(), self.data_path()).unwrap()
        }

        /// Open storage for this test environment (DI-based).
        pub fn open_storage(&self) -> Storage {
            Storage::open_with_data_dir(self.path(), self.data_path()).unwrap()
        }

        /// Check if storage exists for this test environment (DI-based).
        pub fn storage_exists(&self) -> bool {
            Storage::exists_with_data_dir(self.path(), self.data_path()).unwrap()
        }
    }

    impl Drop for TestEnv {
        fn drop(&mut self) {
            // Clean up thread-local override if this instance set it
            if self.thread_local_set {
                crate::storage::clear_data_dir_override();
            }
            // Clean up environment variables if this instance set them
            if self.env_vars_set {
                // SAFETY: This runs when the test environment is being torn down
                // and only for instances that set env vars (which must be #[serial])
                unsafe {
                    std::env::remove_var("BN_DATA_DIR");
                    std::env::remove_var("BN_TEST_MODE");
                    std::env::remove_var("BN_TEST_ID");
                }
            }
        }
    }

    impl Default for TestEnv {
        fn default() -> Self {
            Self::new()
        }
    }
}

/// Library-level error type for Binnacle operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[cfg(not(target_arch = "wasm32"))]
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Not initialized: run `bn session init` first")]
    NotInitialized,

    #[error("Entity not found: {0}")]
    NotFound(String),

    #[error("Invalid ID format: {0}")]
    InvalidId(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Cycle detected in dependencies")]
    CycleDetected,

    #[error("A queue already exists for this repository")]
    QueueAlreadyExists,

    #[error("{0}")]
    Other(String),
}

/// Result type alias for Binnacle operations.
pub type Result<T> = std::result::Result<T, Error>;
