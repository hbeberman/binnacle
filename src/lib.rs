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
pub mod gui;
#[cfg(not(target_arch = "wasm32"))]
pub mod mcp;
pub mod models;
#[cfg(not(target_arch = "wasm32"))]
pub mod storage;
#[cfg(not(target_arch = "wasm32"))]
pub mod sys;
#[cfg(feature = "wasm")]
pub mod wasm;

/// Test utilities for isolated test environments.
#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) mod test_utils {
    use std::path::Path;
    use std::sync::OnceLock;
    use tempfile::TempDir;

    use crate::storage::Storage;

    /// Global test data directory for tests that need env var isolation.
    /// This is set once per process and shared by all tests.
    ///
    /// Using `OnceLock` ensures the `TempDir` stays alive for the process lifetime
    /// without requiring `static mut`.
    static TEST_DATA_DIR: OnceLock<TempDir> = OnceLock::new();

    /// Initialize the shared test data directory via BN_DATA_DIR env var.
    ///
    /// This is for tests that call the command/MCP layer which doesn't support DI.
    /// Uses `OnceLock` to ensure the env var is set exactly once per process.
    ///
    /// # Thread Safety Note (Test Code Only)
    ///
    /// The `set_var` call is inherently unsafe in multi-threaded POSIX contexts
    /// because `setenv(3)` is not thread-safe. However, this is acceptable here:
    ///
    /// 1. This code only runs in `#[cfg(test)]` builds
    /// 2. `OnceLock::get_or_init` ensures initialization happens exactly once
    /// 3. Tests calling `new_with_env()` early in setup will see the same value
    /// 4. Integration tests use per-subprocess env vars (completely safe)
    ///
    /// For production code, use the `*_with_data_dir()` DI methods instead.
    pub fn init_test_env_var() {
        TEST_DATA_DIR.get_or_init(|| {
            let dir = TempDir::new().unwrap();
            // SAFETY: set_var is technically unsafe on POSIX due to setenv(3) not being
            // thread-safe. This is acceptable in test code because:
            // 1. OnceLock ensures this runs exactly once
            // 2. Tests call new_with_env() early in setup
            // 3. Integration tests use per-subprocess env vars (completely safe)
            unsafe {
                std::env::set_var("BN_DATA_DIR", dir.path());
            }
            dir
        });
    }

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
        pub fn new_with_env() -> Self {
            init_test_env_var();
            Self::new()
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

    #[error("Not initialized: run `bn system init` first")]
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
