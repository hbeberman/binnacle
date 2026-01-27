//! Integration tests for the `bn system migrate` command.
//!
//! These tests verify migration between storage backends:
//! - file -> orphan-branch
//! - file -> git-notes
//! - Dry run mode

mod common;

use assert_cmd::Command;
use common::TempDir;
use predicates::prelude::*;
use std::path::Path;
use std::process::Command as StdCommand;

/// A test environment with a git repository and isolated data storage.
struct GitTestEnv {
    repo_dir: TempDir,
    data_dir: TempDir,
}

impl GitTestEnv {
    /// Create a new test environment with a git repository.
    fn new() -> Self {
        let repo_dir = TempDir::new().unwrap();
        let data_dir = TempDir::new().unwrap();

        // Initialize git repo
        StdCommand::new("git")
            .args(["init"])
            .current_dir(repo_dir.path())
            .output()
            .expect("Failed to init git repo");

        // Configure git user for commits
        StdCommand::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(repo_dir.path())
            .output()
            .expect("Failed to configure git");

        StdCommand::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(repo_dir.path())
            .output()
            .expect("Failed to configure git");

        Self { repo_dir, data_dir }
    }

    /// Get a Command for the bn binary with isolated data directory.
    fn bn(&self) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(self.repo_dir.path());
        cmd.env("BN_DATA_DIR", self.data_dir.path());
        cmd
    }

    /// Get the path to the repo directory.
    fn path(&self) -> &Path {
        self.repo_dir.path()
    }
}

/// Initialize binnacle and create some test data
fn setup_with_data(env: &GitTestEnv) {
    // Initialize binnacle
    env.bn().args(["system", "init", "-y"]).assert().success();

    // Create a task
    env.bn()
        .args(["task", "create", "Test task"])
        .assert()
        .success();

    // Create a bug
    env.bn()
        .args(["bug", "create", "Test bug"])
        .assert()
        .success();

    // Create an idea
    env.bn()
        .args(["idea", "create", "Test idea"])
        .assert()
        .success();
}

/// Check if a branch exists in the git repo.
fn branch_exists(dir: &Path, branch: &str) -> bool {
    let output = StdCommand::new("git")
        .args(["rev-parse", "--verify", &format!("refs/heads/{}", branch)])
        .current_dir(dir)
        .output()
        .expect("Failed to check branch");

    output.status.success()
}

/// Check if git notes ref exists.
fn notes_ref_exists(dir: &Path) -> bool {
    let output = StdCommand::new("git")
        .args(["show-ref", "--verify", "--quiet", "refs/notes/binnacle"])
        .current_dir(dir)
        .output()
        .expect("Failed to check notes ref");

    output.status.success()
}

/// Read a file from a git branch.
fn read_from_branch(dir: &Path, branch: &str, file: &str) -> String {
    let output = StdCommand::new("git")
        .args(["show", &format!("{}:{}", branch, file)])
        .current_dir(dir)
        .output()
        .expect("Failed to read from branch");

    String::from_utf8_lossy(&output.stdout).to_string()
}

// === Dry Run Tests ===

#[test]
fn test_migrate_dry_run_shows_preview() {
    let env = GitTestEnv::new();
    setup_with_data(&env);

    env.bn()
        .args([
            "system",
            "migrate",
            "--to",
            "orphan-branch",
            "--dry-run",
            "-H",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("DRY RUN"))
        .stdout(predicate::str::contains("tasks.jsonl"))
        .stdout(predicate::str::contains("bugs.jsonl"))
        .stdout(predicate::str::contains("ideas.jsonl"))
        .stdout(predicate::str::contains("Run without --dry-run"));

    // The branch should not exist after dry run
    assert!(
        !branch_exists(env.path(), "binnacle-data"),
        "Branch should not be created during dry run"
    );
}

#[test]
fn test_migrate_dry_run_json_output() {
    let env = GitTestEnv::new();
    setup_with_data(&env);

    let output = env
        .bn()
        .args(["system", "migrate", "--to", "orphan-branch", "--dry-run"])
        .output()
        .expect("Failed to run command");

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Invalid JSON output");

    assert_eq!(json["success"], true);
    assert_eq!(json["dry_run"], true);
    assert_eq!(json["from_backend"], "file");
    assert_eq!(json["to_backend"], "orphan-branch");
    assert!(json["files_migrated"].is_array());
}

// === Actual Migration Tests ===

#[test]
fn test_migrate_to_orphan_branch() {
    let env = GitTestEnv::new();
    setup_with_data(&env);

    // Verify branch doesn't exist yet
    assert!(!branch_exists(env.path(), "binnacle-data"));

    // Perform migration
    env.bn()
        .args(["system", "migrate", "--to", "orphan-branch", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Migrated from 'file' to 'orphan-branch'",
        ));

    // Verify branch now exists
    assert!(
        branch_exists(env.path(), "binnacle-data"),
        "binnacle-data branch should exist after migration"
    );

    // Verify data was migrated
    let tasks_content = read_from_branch(env.path(), "binnacle-data", "tasks.jsonl");
    assert!(
        tasks_content.contains("Test task"),
        "Tasks should be migrated"
    );

    let bugs_content = read_from_branch(env.path(), "binnacle-data", "bugs.jsonl");
    assert!(bugs_content.contains("Test bug"), "Bugs should be migrated");

    let ideas_content = read_from_branch(env.path(), "binnacle-data", "ideas.jsonl");
    assert!(
        ideas_content.contains("Test idea"),
        "Ideas should be migrated"
    );
}

#[test]
fn test_migrate_to_git_notes() {
    let env = GitTestEnv::new();
    setup_with_data(&env);

    // Verify notes ref doesn't exist yet
    assert!(!notes_ref_exists(env.path()));

    // Perform migration
    env.bn()
        .args(["system", "migrate", "--to", "git-notes", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Migrated from 'file' to 'git-notes'",
        ));

    // Verify notes ref now exists
    assert!(
        notes_ref_exists(env.path()),
        "refs/notes/binnacle should exist after migration"
    );
}

// === Error Cases ===

#[test]
fn test_migrate_to_file_fails() {
    let env = GitTestEnv::new();
    setup_with_data(&env);

    // Migration from file to file should fail
    env.bn()
        .args(["system", "migrate", "--to", "file"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Cannot migrate from file backend to file backend",
        ));
}

#[test]
fn test_migrate_unknown_backend_fails() {
    let env = GitTestEnv::new();
    setup_with_data(&env);

    env.bn()
        .args(["system", "migrate", "--to", "unknown-backend"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown backend type"));
}

#[test]
fn test_migrate_uninit_repo_fails() {
    let env = GitTestEnv::new();
    // Don't initialize binnacle

    env.bn()
        .args(["system", "migrate", "--to", "orphan-branch"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not initialized"));
}

// === Backend Alias Tests ===

#[test]
fn test_migrate_backend_aliases() {
    let env = GitTestEnv::new();
    setup_with_data(&env);

    // "orphan" should work as an alias for "orphan-branch"
    env.bn()
        .args(["system", "migrate", "--to", "orphan", "--dry-run"])
        .assert()
        .success();

    // "branch" should also work
    env.bn()
        .args(["system", "migrate", "--to", "branch", "--dry-run"])
        .assert()
        .success();

    // "notes" should work as an alias for "git-notes"
    env.bn()
        .args(["system", "migrate", "--to", "notes", "--dry-run"])
        .assert()
        .success();
}

// ============================================================================
// Tests for `bn system migrate-bugs` command
// ============================================================================

/// Initialize binnacle only (no test data)
fn init_only(env: &GitTestEnv) {
    env.bn().args(["system", "init", "-y"]).assert().success();
}

#[test]
fn migrate_bugs_no_tasks_with_bug_tag() {
    let env = GitTestEnv::new();
    init_only(&env);

    // Create a regular task without 'bug' tag
    env.bn()
        .args(["task", "create", "Regular task", "-t", "feature"])
        .assert()
        .success();

    // Run migrate-bugs - should find nothing
    env.bn()
        .args(["system", "migrate-bugs", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No tasks with 'bug' tag found"));
}

#[test]
fn migrate_bugs_converts_tagged_tasks() {
    let env = GitTestEnv::new();
    init_only(&env);

    // Create tasks with 'bug' tag
    env.bn()
        .args(["task", "create", "Bug task one", "-t", "bug", "-p", "1"])
        .assert()
        .success();

    env.bn()
        .args([
            "task",
            "create",
            "Bug task two",
            "-t",
            "bug",
            "-t",
            "backend",
        ])
        .assert()
        .success();

    // Run migrate-bugs
    env.bn()
        .args(["system", "migrate-bugs", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Migrated 2 tasks to bugs"))
        .stdout(predicate::str::contains("Bug task one"))
        .stdout(predicate::str::contains("Bug task two"));

    // Verify bugs were created
    env.bn()
        .args(["bug", "list", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Bug task one"))
        .stdout(predicate::str::contains("Bug task two"));
}

#[test]
fn migrate_bugs_dry_run_mode() {
    let env = GitTestEnv::new();
    init_only(&env);

    // Create a task with 'bug' tag
    env.bn()
        .args(["task", "create", "Bug to migrate", "-t", "bug"])
        .assert()
        .success();

    // Run migrate-bugs with --dry-run
    env.bn()
        .args(["system", "migrate-bugs", "--dry-run", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Dry run: would migrate 1 task to bugs",
        ))
        .stdout(predicate::str::contains("Bug to migrate"));

    // Verify no bugs were actually created
    env.bn().args(["bug", "list"]).assert().success().stdout(
        predicate::str::contains("\"count\":0").or(predicate::str::contains("\"bugs\":[]")),
    );
}

#[test]
fn migrate_bugs_removes_tag_when_requested() {
    let env = GitTestEnv::new();
    init_only(&env);

    // Create a task with 'bug' tag and another tag
    env.bn()
        .args([
            "task",
            "create",
            "Bug with multiple tags",
            "-t",
            "bug",
            "-t",
            "frontend",
        ])
        .assert()
        .success();

    // Run migrate-bugs with --remove-tag
    env.bn()
        .args(["system", "migrate-bugs", "--remove-tag", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Removed 'bug' tag from original tasks",
        ));

    // Verify the original task no longer has 'bug' tag but still has 'frontend'
    let output = env.bn().args(["task", "list"]).assert().success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    // The task should still exist with 'frontend' tag
    assert!(stdout.contains("frontend"));
    // Note: task is still there (we don't delete tasks, just remove the tag)
}

#[test]
fn migrate_bugs_preserves_task_metadata() {
    let env = GitTestEnv::new();
    init_only(&env);

    // Create a task with bug tag and various metadata
    env.bn()
        .args([
            "task",
            "create",
            "Bug with metadata",
            "-t",
            "bug",
            "-p",
            "0",
            "-a",
            "alice",
            "-d",
            "This is a description",
        ])
        .assert()
        .success();

    // Run migrate-bugs
    env.bn()
        .args(["system", "migrate-bugs", "-H"])
        .assert()
        .success();

    // List bugs and verify metadata was preserved
    let output = env.bn().args(["bug", "list"]).assert().success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    // Verify priority was preserved (0 is highest)
    assert!(stdout.contains("\"priority\":0"));
    // Verify assignee was preserved
    assert!(stdout.contains("\"assignee\":\"alice\""));
}

#[test]
fn migrate_bugs_json_output() {
    let env = GitTestEnv::new();
    init_only(&env);

    // Create a task with 'bug' tag
    env.bn()
        .args(["task", "create", "JSON bug task", "-t", "bug"])
        .assert()
        .success();

    // Run migrate-bugs (JSON output by default)
    env.bn()
        .args(["system", "migrate-bugs"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"success\":true"))
        .stdout(predicate::str::contains("\"tasks_found\":1"))
        .stdout(predicate::str::contains("\"tasks_migrated\":["))
        .stdout(predicate::str::contains("\"old_task_id\":"))
        .stdout(predicate::str::contains("\"new_bug_id\":"));
}
