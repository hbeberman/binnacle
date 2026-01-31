//! Binnacle - A project state tracking library for AI agents and humans.
//!
//! This library provides the core functionality for the `bn` CLI tool,
//! including task management, test tracking, and dependency handling.

#[cfg(not(target_arch = "wasm32"))]
pub mod action_log;
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
    /// For **command/MCP layer tests**: Use `TestEnv::new_with_env()` which sets BN_DATA_DIR.
    pub struct TestEnv {
        /// Simulated repository directory
        pub repo_dir: TempDir,
        /// Isolated data storage directory (for DI-based tests)
        pub data_dir: TempDir,
    }

    impl TestEnv {
        /// Create a new test environment with isolated directories (pure DI).
        /// Use this for storage layer tests that call Storage methods directly.
        pub fn new() -> Self {
            Self {
                repo_dir: TempDir::new().unwrap(),
                data_dir: TempDir::new().unwrap(),
            }
        }

        /// Create a new test environment that uses BN_DATA_DIR env var.
        /// Use this for command/MCP layer tests that call the public API.
        ///
        /// IMPORTANT: Tests using this MUST be marked with #[serial] to avoid
        /// environment variable races between parallel tests.
        pub fn new_with_env() -> Self {
            let env = Self::new();
            // Set BN_DATA_DIR to this test's isolated data directory
            // Clear agent-specific env vars to ensure test isolation
            // SAFETY: This is safe because tests using new_with_env() are marked #[serial]
            unsafe {
                std::env::set_var("BN_DATA_DIR", env.data_path());
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
