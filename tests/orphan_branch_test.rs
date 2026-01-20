//! Integration tests for the orphan branch storage backend.
//!
//! These tests verify that the orphan branch backend:
//! - Creates and manages the binnacle-data branch correctly
//! - Stores and retrieves tasks via git plumbing commands
//! - Works without modifying the working tree

use std::process::Command as StdCommand;
use tempfile::TempDir;

/// Create a git repository in a temp directory.
fn create_git_repo() -> TempDir {
    let temp = TempDir::new().unwrap();

    // Initialize git repo
    StdCommand::new("git")
        .args(["init"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to init git repo");

    // Configure git user for commits
    StdCommand::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to configure git");

    StdCommand::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to configure git");

    temp
}

/// Check if a branch exists in the git repo.
fn branch_exists(dir: &TempDir, branch: &str) -> bool {
    let output = StdCommand::new("git")
        .args(["rev-parse", "--verify", &format!("refs/heads/{}", branch)])
        .current_dir(dir.path())
        .output()
        .expect("Failed to check branch");

    output.status.success()
}

/// Read a file from a git branch.
fn read_from_branch(dir: &TempDir, branch: &str, file: &str) -> String {
    let output = StdCommand::new("git")
        .args(["show", &format!("{}:{}", branch, file)])
        .current_dir(dir.path())
        .output()
        .expect("Failed to read from branch");

    String::from_utf8_lossy(&output.stdout).to_string()
}

// === Basic Backend Tests ===

#[test]
fn test_orphan_branch_backend_init() {
    let temp = create_git_repo();

    // The binnacle-data branch should not exist yet
    assert!(!branch_exists(&temp, "binnacle-data"));

    // Initialize binnacle (currently uses file backend by default)
    // For now, we test the backend module directly via library tests
    // This test verifies the branch creation works

    // Use the library directly to test the orphan branch backend
    use binnacle::storage::OrphanBranchBackend;
    use binnacle::storage::StorageBackend;

    let mut backend = OrphanBranchBackend::new(temp.path());
    backend.init(temp.path()).unwrap();

    // The binnacle-data branch should now exist
    assert!(branch_exists(&temp, "binnacle-data"));
}

#[test]
fn test_orphan_branch_stores_data() {
    let temp = create_git_repo();

    use binnacle::storage::OrphanBranchBackend;
    use binnacle::storage::StorageBackend;

    let mut backend = OrphanBranchBackend::new(temp.path());
    backend.init(temp.path()).unwrap();

    // Write some data
    backend
        .append_jsonl("tasks.jsonl", r#"{"id":"bn-test","title":"Test"}"#)
        .unwrap();

    // Read it back via the backend
    let lines = backend.read_jsonl("tasks.jsonl").unwrap();
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("bn-test"));

    // Verify it's actually in the git branch
    let content = read_from_branch(&temp, "binnacle-data", "tasks.jsonl");
    assert!(content.contains("bn-test"));
}

#[test]
fn test_orphan_branch_no_working_tree_pollution() {
    let temp = create_git_repo();

    use binnacle::storage::OrphanBranchBackend;
    use binnacle::storage::StorageBackend;

    let mut backend = OrphanBranchBackend::new(temp.path());
    backend.init(temp.path()).unwrap();

    // Write data
    backend
        .append_jsonl("tasks.jsonl", r#"{"id":"bn-test"}"#)
        .unwrap();

    // The working tree should not contain tasks.jsonl
    assert!(!temp.path().join("tasks.jsonl").exists());

    // The binnacle-data branch files should not appear in working directory
    let output = StdCommand::new("git")
        .args(["status", "--porcelain"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to get git status");

    let status = String::from_utf8_lossy(&output.stdout);
    assert!(
        !status.contains("tasks.jsonl"),
        "tasks.jsonl should not appear in working tree"
    );
}

#[test]
fn test_orphan_branch_multiple_files() {
    let temp = create_git_repo();

    use binnacle::storage::OrphanBranchBackend;
    use binnacle::storage::StorageBackend;

    let mut backend = OrphanBranchBackend::new(temp.path());
    backend.init(temp.path()).unwrap();

    // Write to multiple files
    backend
        .append_jsonl("tasks.jsonl", r#"{"id":"bn-task1"}"#)
        .unwrap();
    backend
        .append_jsonl("commits.jsonl", r#"{"sha":"abc1234"}"#)
        .unwrap();
    backend
        .append_jsonl("test-results.jsonl", r#"{"test_id":"bnt-0001"}"#)
        .unwrap();

    // Read back each file
    let tasks = backend.read_jsonl("tasks.jsonl").unwrap();
    let commits = backend.read_jsonl("commits.jsonl").unwrap();
    let results = backend.read_jsonl("test-results.jsonl").unwrap();

    assert_eq!(tasks.len(), 1);
    assert_eq!(commits.len(), 1);
    assert_eq!(results.len(), 1);
}

#[test]
fn test_orphan_branch_commit_history() {
    let temp = create_git_repo();

    use binnacle::storage::OrphanBranchBackend;
    use binnacle::storage::StorageBackend;

    let mut backend = OrphanBranchBackend::new(temp.path());
    backend.init(temp.path()).unwrap();

    // Make multiple writes
    backend
        .append_jsonl("tasks.jsonl", r#"{"id":"bn-task1"}"#)
        .unwrap();
    backend
        .append_jsonl("tasks.jsonl", r#"{"id":"bn-task2"}"#)
        .unwrap();

    // Check that the branch has commits
    let output = StdCommand::new("git")
        .args(["log", "--oneline", "binnacle-data"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to get git log");

    let log = String::from_utf8_lossy(&output.stdout);
    let commit_count = log.lines().count();

    // Should have at least 3 commits: init + 2 updates
    assert!(
        commit_count >= 3,
        "Expected at least 3 commits, got {}",
        commit_count
    );
}

#[test]
fn test_orphan_branch_persistence() {
    let temp = create_git_repo();

    use binnacle::storage::OrphanBranchBackend;
    use binnacle::storage::StorageBackend;

    // First backend instance - write data
    {
        let mut backend = OrphanBranchBackend::new(temp.path());
        backend.init(temp.path()).unwrap();
        backend
            .append_jsonl("tasks.jsonl", r#"{"id":"bn-persist"}"#)
            .unwrap();
    }

    // Second backend instance - read data
    {
        let backend = OrphanBranchBackend::new(temp.path());
        let lines = backend.read_jsonl("tasks.jsonl").unwrap();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("bn-persist"));
    }
}

#[test]
fn test_orphan_branch_write_overwrite() {
    let temp = create_git_repo();

    use binnacle::storage::OrphanBranchBackend;
    use binnacle::storage::StorageBackend;

    let mut backend = OrphanBranchBackend::new(temp.path());
    backend.init(temp.path()).unwrap();

    // Write initial data
    backend
        .append_jsonl("tasks.jsonl", r#"{"id":"bn-old"}"#)
        .unwrap();

    // Overwrite with new data
    backend
        .write_jsonl(
            "tasks.jsonl",
            &[
                r#"{"id":"bn-new1"}"#.to_string(),
                r#"{"id":"bn-new2"}"#.to_string(),
            ],
        )
        .unwrap();

    // Read back - should only have new data
    let lines = backend.read_jsonl("tasks.jsonl").unwrap();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("bn-new1"));
    assert!(lines[1].contains("bn-new2"));
    assert!(!lines.iter().any(|l| l.contains("bn-old")));
}

#[test]
fn test_orphan_branch_backend_type() {
    let temp = create_git_repo();

    use binnacle::storage::OrphanBranchBackend;
    use binnacle::storage::StorageBackend;

    let backend = OrphanBranchBackend::new(temp.path());
    assert_eq!(backend.backend_type(), "orphan-branch");
    assert_eq!(backend.location(), "git branch: binnacle-data");
}

#[test]
fn test_orphan_branch_fails_without_git() {
    let temp = TempDir::new().unwrap();
    // This is NOT a git repo

    use binnacle::storage::OrphanBranchBackend;
    use binnacle::storage::StorageBackend;

    let mut backend = OrphanBranchBackend::new(temp.path());
    let result = backend.init(temp.path());

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("Not a git repository"),
        "Expected 'Not a git repository' error, got: {}",
        err
    );
}
