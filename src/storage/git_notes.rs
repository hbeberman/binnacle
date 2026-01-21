//! Git notes storage backend.
//!
//! Stores binnacle data in git notes at `refs/notes/binnacle`.
//! Each JSONL file is stored as a separate note attached to a
//! deterministic blob object derived from the filename.

use super::backend::StorageBackend;
use crate::{Error, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Notes reference for storing binnacle data.
const NOTES_REF: &str = "refs/notes/binnacle";
/// Prefix for note target blobs to avoid collisions.
const NOTE_PREFIX: &str = "binnacle:";

/// Storage backend that uses git notes.
pub struct GitNotesBackend {
    /// Path to the git repository.
    repo_path: PathBuf,
    /// Whether the backend has been initialized.
    initialized: bool,
}

impl GitNotesBackend {
    /// Create a new git notes backend for the given repository.
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

    /// Check if the binnacle notes ref exists.
    fn notes_ref_exists(&self) -> Result<bool> {
        let output = Command::new("git")
            .args(["show-ref", "--verify", "--quiet", NOTES_REF])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| Error::Other(format!("Failed to check notes ref: {}", e)))?;

        Ok(output.status.success())
    }

    /// Build or retrieve the deterministic blob used as a note target.
    fn note_target(&self, filename: &str) -> Result<String> {
        let key = format!("{}{}", NOTE_PREFIX, filename);
        let mut child = Command::new("git")
            .args(["hash-object", "-w", "--stdin"])
            .current_dir(&self.repo_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| Error::Other(format!("Failed to run git hash-object: {}", e)))?;

        {
            use std::io::Write;
            let stdin = child.stdin.as_mut().unwrap();
            stdin
                .write_all(key.as_bytes())
                .map_err(|e| Error::Other(format!("Failed to write note key: {}", e)))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| Error::Other(format!("Failed to wait for git: {}", e)))?;

        if !output.status.success() {
            return Err(Error::Other("Failed to create note target".to_string()));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Ensure the notes ref exists by creating a metadata note.
    fn ensure_notes_ref(&self) -> Result<()> {
        if self.notes_ref_exists()? {
            return Ok(());
        }

        let target = self.note_target("meta")?;
        let output = Command::new("git")
            .args([
                "notes",
                "--ref",
                NOTES_REF,
                "add",
                "-f",
                "-m",
                "binnacle init",
                &target,
            ])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| Error::Other(format!("Failed to create notes ref: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Other(format!(
                "Failed to initialize notes ref: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Read a note for a specific file.
    fn read_note(&self, filename: &str) -> Result<String> {
        let target = self.note_target(filename)?;
        let output = Command::new("git")
            .args(["notes", "--ref", NOTES_REF, "show", &target])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| Error::Other(format!("Failed to read note: {}", e)))?;

        if !output.status.success() {
            return Ok(String::new());
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Write a note for a specific file.
    fn write_note(&self, filename: &str, content: &str) -> Result<()> {
        let target = self.note_target(filename)?;

        if content.is_empty() {
            let output = Command::new("git")
                .args(["notes", "--ref", NOTES_REF, "remove", &target])
                .current_dir(&self.repo_path)
                .output()
                .map_err(|e| Error::Other(format!("Failed to remove note: {}", e)))?;

            if output.status.success()
                || String::from_utf8_lossy(&output.stderr).contains("no note")
            {
                return Ok(());
            }

            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Other(format!("Failed to remove note: {}", stderr)));
        }

        let mut child = Command::new("git")
            .args(["notes", "--ref", NOTES_REF, "add", "-f", "-F", "-", &target])
            .current_dir(&self.repo_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| Error::Other(format!("Failed to run git notes add: {}", e)))?;

        {
            use std::io::Write;
            let stdin = child.stdin.as_mut().unwrap();
            stdin
                .write_all(content.as_bytes())
                .map_err(|e| Error::Other(format!("Failed to write note: {}", e)))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| Error::Other(format!("Failed to wait for git notes: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Other(format!("Failed to write note: {}", stderr)));
        }

        Ok(())
    }
}

impl StorageBackend for GitNotesBackend {
    fn init(&mut self, repo_path: &Path) -> Result<()> {
        self.repo_path = repo_path.to_path_buf();

        if !self.is_git_repo()? {
            return Err(Error::Other(
                "Not a git repository. Git notes backend requires a git repository.".to_string(),
            ));
        }

        self.ensure_notes_ref()?;
        self.initialized = true;
        Ok(())
    }

    fn exists(&self, repo_path: &Path) -> Result<bool> {
        let backend = Self::new(repo_path);

        if !backend.is_git_repo()? {
            return Ok(false);
        }

        backend.notes_ref_exists()
    }

    fn read_jsonl(&self, filename: &str) -> Result<Vec<String>> {
        if !self.initialized && !self.notes_ref_exists()? {
            return Err(Error::NotInitialized);
        }

        let content = self.read_note(filename)?;
        Ok(content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|s| s.to_string())
            .collect())
    }

    fn append_jsonl(&mut self, filename: &str, line: &str) -> Result<()> {
        if !self.initialized && !self.notes_ref_exists()? {
            return Err(Error::NotInitialized);
        }

        let mut content = self.read_note(filename)?;
        if !content.is_empty() && !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str(line);
        content.push('\n');

        self.write_note(filename, &content)?;
        Ok(())
    }

    fn write_jsonl(&mut self, filename: &str, lines: &[String]) -> Result<()> {
        if !self.initialized && !self.notes_ref_exists()? {
            return Err(Error::NotInitialized);
        }

        let content = if lines.is_empty() {
            String::new()
        } else {
            lines.join("\n") + "\n"
        };

        self.write_note(filename, &content)?;
        Ok(())
    }

    fn location(&self) -> String {
        format!("git notes: {}", NOTES_REF)
    }

    fn backend_type(&self) -> &'static str {
        "git-notes"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_git_repo() -> TempDir {
        let temp = TempDir::new().unwrap();

        Command::new("git")
            .args(["init"])
            .current_dir(temp.path())
            .output()
            .expect("Failed to init git repo");

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
        let backend = GitNotesBackend::new(temp.path());
        assert!(backend.is_git_repo().unwrap());
    }

    #[test]
    fn test_not_git_repo() {
        let temp = TempDir::new().unwrap();
        let backend = GitNotesBackend::new(temp.path());
        assert!(!backend.is_git_repo().unwrap());
    }

    #[test]
    fn test_init_creates_notes_ref() {
        let temp = create_git_repo();
        let mut backend = GitNotesBackend::new(temp.path());

        assert!(!backend.notes_ref_exists().unwrap());
        backend.init(temp.path()).unwrap();
        assert!(backend.notes_ref_exists().unwrap());
    }

    #[test]
    fn test_read_write_jsonl() {
        let temp = create_git_repo();
        let mut backend = GitNotesBackend::new(temp.path());
        backend.init(temp.path()).unwrap();

        let lines = backend.read_jsonl("tasks.jsonl").unwrap();
        assert!(lines.is_empty());

        backend
            .append_jsonl("tasks.jsonl", r#"{"id":"test1"}"#)
            .unwrap();
        backend
            .append_jsonl("tasks.jsonl", r#"{"id":"test2"}"#)
            .unwrap();

        let lines = backend.read_jsonl("tasks.jsonl").unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], r#"{"id":"test1"}"#);
        assert_eq!(lines[1], r#"{"id":"test2"}"#);

        backend
            .write_jsonl("tasks.jsonl", &[r#"{"id":"test3"}"#.to_string()])
            .unwrap();

        let lines = backend.read_jsonl("tasks.jsonl").unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], r#"{"id":"test3"}"#);
    }

    #[test]
    fn test_exists() {
        let temp = create_git_repo();
        let mut backend = GitNotesBackend::new(temp.path());

        assert!(!backend.exists(temp.path()).unwrap());
        backend.init(temp.path()).unwrap();
        assert!(backend.exists(temp.path()).unwrap());
    }

    #[test]
    fn test_backend_type() {
        let temp = create_git_repo();
        let backend = GitNotesBackend::new(temp.path());
        assert_eq!(backend.backend_type(), "git-notes");
    }

    #[test]
    fn test_location() {
        let temp = create_git_repo();
        let backend = GitNotesBackend::new(temp.path());
        assert_eq!(backend.location(), "git notes: refs/notes/binnacle");
    }
}
