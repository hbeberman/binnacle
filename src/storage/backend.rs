//! Storage backend trait and implementations.
//!
//! This module provides different storage backends for binnacle data:
//! - `FileBackend` - External file storage (default)
//! - `OrphanBranchBackend` - Git orphan branch storage
//! - `GitNotesBackend` - Git notes storage

use crate::Result;
use std::path::Path;

/// Trait for storage backends that handle raw data persistence.
///
/// Each backend must implement read/write operations for JSONL data
/// and provide initialization capabilities.
pub trait StorageBackend: Send + Sync {
    /// Initialize the storage for a repository.
    fn init(&mut self, repo_path: &Path) -> Result<()>;

    /// Check if storage exists for the repository.
    fn exists(&self, repo_path: &Path) -> Result<bool>;

    /// Read all lines from a JSONL file.
    fn read_jsonl(&self, filename: &str) -> Result<Vec<String>>;

    /// Append a line to a JSONL file.
    fn append_jsonl(&mut self, filename: &str, line: &str) -> Result<()>;

    /// Write all lines to a JSONL file (replacing existing content).
    fn write_jsonl(&mut self, filename: &str, lines: &[String]) -> Result<()>;

    /// Get the storage location description (for display purposes).
    fn location(&self) -> String;

    /// Get the backend type name.
    fn backend_type(&self) -> &'static str;
}

/// Available storage backend types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendType {
    /// External file storage (default) - ~/.local/share/binnacle/<repo-hash>/
    File,
    /// Git orphan branch storage - binnacle-data branch
    OrphanBranch,
    /// Git notes storage - refs/notes/binnacle (future)
    GitNotes,
}

impl BackendType {
    /// Parse a backend type from a string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "file" | "external" | "default" => Some(Self::File),
            "orphan" | "orphan-branch" | "branch" => Some(Self::OrphanBranch),
            "notes" | "git-notes" => Some(Self::GitNotes),
            _ => None,
        }
    }

    /// Get the string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::File => "file",
            Self::OrphanBranch => "orphan-branch",
            Self::GitNotes => "git-notes",
        }
    }
}

impl std::fmt::Display for BackendType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
