//! Orphan branch storage backend.
//!
//! Stores binnacle data in a git orphan branch named `binnacle-data`.
//! This keeps all data in the repository without polluting the main branch.
//!
//! ## Storage Structure
//!
//! The orphan branch contains:
//! - `tasks.jsonl` - Tasks and test nodes
//! - `commits.jsonl` - Commit-to-task links  
//! - `test-results.jsonl` - Test run history
//!
//! ## How It Works
//!
//! 1. Data is read from the orphan branch by checking out files via `git show`
//! 2. Data is written by creating commits on the orphan branch
//! 3. The working tree is never modified (uses git plumbing commands)

use super::backend::StorageBackend;
use crate::{Error, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Branch name for storing binnacle data.
const BINNACLE_BRANCH: &str = "binnacle-data";

/// Storage backend that uses a git orphan branch.
pub struct OrphanBranchBackend {
    /// Path to the git repository.
    repo_path: PathBuf,
    /// Whether the backend has been initialized.
    initialized: bool,
}

impl OrphanBranchBackend {
    /// Create a new orphan branch backend for the given repository.
    pub fn new(repo_path: &Path) -> Self {
        Self {
            repo_path: repo_path.to_path_buf(),
            initialized: false,
        }
    }

    /// Check if the repository is a git repository.
    fn is_git_repo(&self) -> Result<bool> {
        let output = Command::new("git")
            .args(["rev-parse", "--git-dir"])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| Error::Other(format!("Failed to run git: {}", e)))?;

        Ok(output.status.success())
    }

    /// Check if the binnacle-data branch exists.
    fn branch_exists(&self) -> Result<bool> {
        let output = Command::new("git")
            .args([
                "rev-parse",
                "--verify",
                &format!("refs/heads/{}", BINNACLE_BRANCH),
            ])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| Error::Other(format!("Failed to run git: {}", e)))?;

        Ok(output.status.success())
    }

    /// Create the orphan branch with initial empty files.
    fn create_branch(&self) -> Result<()> {
        // Create an empty tree
        let output = Command::new("git")
            .args(["hash-object", "-t", "tree", "/dev/null"])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| Error::Other(format!("Failed to create empty tree: {}", e)))?;

        if !output.status.success() {
            // Fallback: create empty tree another way
            let output = Command::new("git")
                .args(["mktree"])
                .current_dir(&self.repo_path)
                .stdin(std::process::Stdio::null())
                .output()
                .map_err(|e| Error::Other(format!("Failed to create empty tree: {}", e)))?;

            if !output.status.success() {
                return Err(Error::Other("Failed to create empty tree".to_string()));
            }
        }

        let _empty_tree = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Create initial commit with empty JSONL files
        let files = ["tasks.jsonl", "commits.jsonl", "test-results.jsonl"];

        // Create blobs for empty files
        let mut tree_entries = Vec::new();
        for file in files {
            let output = Command::new("git")
                .args(["hash-object", "-w", "--stdin"])
                .current_dir(&self.repo_path)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| Error::Other(format!("Failed to run git hash-object: {}", e)))?;

            // Write empty content
            let output = output
                .wait_with_output()
                .map_err(|e| Error::Other(format!("Failed to wait for git: {}", e)))?;

            let blob_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
            tree_entries.push(format!("100644 blob {}\t{}", blob_hash, file));
        }

        // Create tree from entries
        let tree_input = tree_entries.join("\n");
        let mut child = Command::new("git")
            .args(["mktree"])
            .current_dir(&self.repo_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| Error::Other(format!("Failed to run git mktree: {}", e)))?;

        {
            use std::io::Write;
            let stdin = child.stdin.as_mut().unwrap();
            stdin
                .write_all(tree_input.as_bytes())
                .map_err(|e| Error::Other(format!("Failed to write to git mktree: {}", e)))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| Error::Other(format!("Failed to wait for git mktree: {}", e)))?;

        if !output.status.success() {
            return Err(Error::Other("Failed to create tree".to_string()));
        }

        let tree_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Create initial commit
        let output = Command::new("git")
            .args([
                "commit-tree",
                &tree_hash,
                "-m",
                "Initialize binnacle data storage",
            ])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| Error::Other(format!("Failed to run git commit-tree: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Other(format!("Failed to create commit: {}", stderr)));
        }

        let commit_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Create the branch reference
        let output = Command::new("git")
            .args([
                "update-ref",
                &format!("refs/heads/{}", BINNACLE_BRANCH),
                &commit_hash,
            ])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| Error::Other(format!("Failed to run git update-ref: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Other(format!("Failed to create branch: {}", stderr)));
        }

        Ok(())
    }

    /// Read a file from the binnacle-data branch.
    fn read_file(&self, filename: &str) -> Result<String> {
        let output = Command::new("git")
            .args(["show", &format!("{}:{}", BINNACLE_BRANCH, filename)])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| Error::Other(format!("Failed to run git show: {}", e)))?;

        if !output.status.success() {
            // File might not exist yet
            return Ok(String::new());
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Write a file to the binnacle-data branch.
    fn write_file(&self, filename: &str, content: &str) -> Result<()> {
        // Get current tree from binnacle-data branch
        let output = Command::new("git")
            .args(["rev-parse", &format!("{}^{{tree}}", BINNACLE_BRANCH)])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| Error::Other(format!("Failed to get tree: {}", e)))?;

        if !output.status.success() {
            return Err(Error::Other("Failed to get current tree".to_string()));
        }

        let current_tree = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Create blob for new content
        let mut child = Command::new("git")
            .args(["hash-object", "-w", "--stdin"])
            .current_dir(&self.repo_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| Error::Other(format!("Failed to run git hash-object: {}", e)))?;

        {
            use std::io::Write;
            let stdin = child.stdin.as_mut().unwrap();
            stdin
                .write_all(content.as_bytes())
                .map_err(|e| Error::Other(format!("Failed to write content: {}", e)))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| Error::Other(format!("Failed to wait for git: {}", e)))?;

        if !output.status.success() {
            return Err(Error::Other("Failed to create blob".to_string()));
        }

        let blob_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Read current tree and update the specific file
        let output = Command::new("git")
            .args(["ls-tree", &current_tree])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| Error::Other(format!("Failed to list tree: {}", e)))?;

        let current_entries = String::from_utf8_lossy(&output.stdout);

        // Build new tree entries
        let mut new_entries = Vec::new();
        let mut found = false;

        for line in current_entries.lines() {
            if line.is_empty() {
                continue;
            }
            // Format: mode type hash\tfilename
            let parts: Vec<&str> = line.splitn(2, '\t').collect();
            if parts.len() != 2 {
                continue;
            }
            let entry_filename = parts[1];
            let _mode_type_hash: Vec<&str> = parts[0].split_whitespace().collect();

            if entry_filename == filename {
                // Replace this file
                new_entries.push(format!("100644 blob {}\t{}", blob_hash, filename));
                found = true;
            } else {
                new_entries.push(line.to_string());
            }
        }

        // If file didn't exist, add it
        if !found {
            new_entries.push(format!("100644 blob {}\t{}", blob_hash, filename));
        }

        // Create new tree
        let tree_input = new_entries.join("\n");
        let mut child = Command::new("git")
            .args(["mktree"])
            .current_dir(&self.repo_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| Error::Other(format!("Failed to run git mktree: {}", e)))?;

        {
            use std::io::Write;
            let stdin = child.stdin.as_mut().unwrap();
            stdin
                .write_all(tree_input.as_bytes())
                .map_err(|e| Error::Other(format!("Failed to write tree: {}", e)))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| Error::Other(format!("Failed to wait for git mktree: {}", e)))?;

        if !output.status.success() {
            return Err(Error::Other("Failed to create new tree".to_string()));
        }

        let new_tree = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Get current commit hash
        let output = Command::new("git")
            .args(["rev-parse", BINNACLE_BRANCH])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| Error::Other(format!("Failed to get commit: {}", e)))?;

        let parent_commit = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Create new commit
        let output = Command::new("git")
            .args([
                "commit-tree",
                &new_tree,
                "-p",
                &parent_commit,
                "-m",
                &format!("Update {}", filename),
            ])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| Error::Other(format!("Failed to create commit: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Other(format!("Failed to create commit: {}", stderr)));
        }

        let new_commit = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Update branch reference
        let output = Command::new("git")
            .args([
                "update-ref",
                &format!("refs/heads/{}", BINNACLE_BRANCH),
                &new_commit,
            ])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| Error::Other(format!("Failed to update ref: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Other(format!("Failed to update branch: {}", stderr)));
        }

        Ok(())
    }
}

impl StorageBackend for OrphanBranchBackend {
    fn init(&mut self, repo_path: &Path) -> Result<()> {
        self.repo_path = repo_path.to_path_buf();

        // Verify this is a git repository
        if !self.is_git_repo()? {
            return Err(Error::Other(
                "Not a git repository. Orphan branch backend requires a git repository."
                    .to_string(),
            ));
        }

        // Create the branch if it doesn't exist
        if !self.branch_exists()? {
            self.create_branch()?;
        }

        self.initialized = true;
        Ok(())
    }

    fn exists(&self, repo_path: &Path) -> Result<bool> {
        let backend = Self::new(repo_path);

        if !backend.is_git_repo()? {
            return Ok(false);
        }

        backend.branch_exists()
    }

    fn read_jsonl(&self, filename: &str) -> Result<Vec<String>> {
        if !self.initialized && !self.branch_exists()? {
            return Err(Error::NotInitialized);
        }

        let content = self.read_file(filename)?;
        Ok(content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|s| s.to_string())
            .collect())
    }

    fn append_jsonl(&mut self, filename: &str, line: &str) -> Result<()> {
        if !self.initialized && !self.branch_exists()? {
            return Err(Error::NotInitialized);
        }

        // Read existing content
        let mut content = self.read_file(filename)?;

        // Append new line
        if !content.is_empty() && !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str(line);
        content.push('\n');

        // Write back
        self.write_file(filename, &content)?;

        Ok(())
    }

    fn write_jsonl(&mut self, filename: &str, lines: &[String]) -> Result<()> {
        if !self.initialized && !self.branch_exists()? {
            return Err(Error::NotInitialized);
        }

        let content = if lines.is_empty() {
            String::new()
        } else {
            lines.join("\n") + "\n"
        };

        self.write_file(filename, &content)?;

        Ok(())
    }

    fn location(&self) -> String {
        format!("git branch: {}", BINNACLE_BRANCH)
    }

    fn backend_type(&self) -> &'static str {
        "orphan-branch"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_git_repo() -> TempDir {
        let temp = TempDir::new().unwrap();

        // Initialize git repo
        Command::new("git")
            .args(["init"])
            .current_dir(temp.path())
            .output()
            .expect("Failed to init git repo");

        // Configure git user for commits
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(temp.path())
            .output()
            .expect("Failed to configure git");

        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(temp.path())
            .output()
            .expect("Failed to configure git");

        temp
    }

    #[test]
    fn test_is_git_repo() {
        let temp = create_git_repo();
        let backend = OrphanBranchBackend::new(temp.path());
        assert!(backend.is_git_repo().unwrap());
    }

    #[test]
    fn test_not_git_repo() {
        let temp = TempDir::new().unwrap();
        let backend = OrphanBranchBackend::new(temp.path());
        assert!(!backend.is_git_repo().unwrap());
    }

    #[test]
    fn test_init_creates_branch() {
        let temp = create_git_repo();
        let mut backend = OrphanBranchBackend::new(temp.path());

        assert!(!backend.branch_exists().unwrap());
        backend.init(temp.path()).unwrap();
        assert!(backend.branch_exists().unwrap());
    }

    #[test]
    fn test_read_write_jsonl() {
        let temp = create_git_repo();
        let mut backend = OrphanBranchBackend::new(temp.path());
        backend.init(temp.path()).unwrap();

        // Initially empty
        let lines = backend.read_jsonl("tasks.jsonl").unwrap();
        assert!(lines.is_empty());

        // Append some lines
        backend
            .append_jsonl("tasks.jsonl", r#"{"id":"test1"}"#)
            .unwrap();
        backend
            .append_jsonl("tasks.jsonl", r#"{"id":"test2"}"#)
            .unwrap();

        // Read back
        let lines = backend.read_jsonl("tasks.jsonl").unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], r#"{"id":"test1"}"#);
        assert_eq!(lines[1], r#"{"id":"test2"}"#);
    }

    #[test]
    fn test_write_jsonl_overwrites() {
        let temp = create_git_repo();
        let mut backend = OrphanBranchBackend::new(temp.path());
        backend.init(temp.path()).unwrap();

        // Write some lines
        backend
            .append_jsonl("tasks.jsonl", r#"{"id":"old"}"#)
            .unwrap();

        // Overwrite with new content
        backend
            .write_jsonl(
                "tasks.jsonl",
                &[
                    r#"{"id":"new1"}"#.to_string(),
                    r#"{"id":"new2"}"#.to_string(),
                ],
            )
            .unwrap();

        // Read back
        let lines = backend.read_jsonl("tasks.jsonl").unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], r#"{"id":"new1"}"#);
        assert_eq!(lines[1], r#"{"id":"new2"}"#);
    }

    #[test]
    fn test_exists() {
        let temp = create_git_repo();
        let mut backend = OrphanBranchBackend::new(temp.path());

        assert!(!backend.exists(temp.path()).unwrap());
        backend.init(temp.path()).unwrap();
        assert!(backend.exists(temp.path()).unwrap());
    }

    #[test]
    fn test_backend_type() {
        let temp = create_git_repo();
        let backend = OrphanBranchBackend::new(temp.path());
        assert_eq!(backend.backend_type(), "orphan-branch");
    }

    #[test]
    fn test_location() {
        let temp = create_git_repo();
        let backend = OrphanBranchBackend::new(temp.path());
        assert_eq!(backend.location(), "git branch: binnacle-data");
    }
}
