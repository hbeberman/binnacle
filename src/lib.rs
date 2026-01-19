//! Binnacle - A project state tracking library for AI agents and humans.
//!
//! This library provides the core functionality for the `bn` CLI tool,
//! including task management, test tracking, and dependency handling.

pub mod cli;
pub mod commands;
pub mod mcp;
pub mod models;
pub mod storage;

/// Library-level error type for Binnacle operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Not initialized: run `bn init` first")]
    NotInitialized,

    #[error("Entity not found: {0}")]
    NotFound(String),

    #[error("Invalid ID format: {0}")]
    InvalidId(String),

    #[error("Cycle detected in dependencies")]
    CycleDetected,

    #[error("{0}")]
    Other(String),
}

/// Result type alias for Binnacle operations.
pub type Result<T> = std::result::Result<T, Error>;
