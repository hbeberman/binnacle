//! Integration tests for Commit Tracking operations via CLI.
//!
//! These tests verify that commit commands work correctly through the CLI:
//! - `bn commit link <sha> <task_id>` links a commit to a task
//! - `bn commit unlink <sha> <task_id>` unlinks a commit from a task
//! - `bn commit list <task_id>` lists commits linked to a task
//! - JSON and human-readable output formats are correct

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

/// Get a Command for the bn binary, running in a temp directory.
fn bn_in(dir: &TempDir) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(dir.path());
    cmd
}

/// Initialize binnacle in a temp directory and return the temp dir.
fn init_binnacle() -> TempDir {
    let temp = TempDir::new().unwrap();
    bn_in(&temp).arg("init").assert().success();
    temp
}

/// Helper function to extract task ID from JSON output
fn extract_task_id(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .split("\"id\":\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .to_string()
}

// === Commit Link Tests ===

#[test]
fn test_commit_link() {
    let temp = init_binnacle();

    // Create a task
    let output = bn_in(&temp)
        .args(["task", "create", "Test task"])
        .output()
        .unwrap();
    let task_id = extract_task_id(&output);

    // Link a commit to the task
    bn_in(&temp)
        .args(["commit", "link", "a1b2c3d", &task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"sha\":\"a1b2c3d\""))
        .stdout(predicate::str::contains(format!(
            "\"task_id\":\"{}\"",
            task_id
        )));
}

#[test]
fn test_commit_link_human_readable() {
    let temp = init_binnacle();

    let output = bn_in(&temp)
        .args(["task", "create", "Test task"])
        .output()
        .unwrap();
    let task_id = extract_task_id(&output);

    bn_in(&temp)
        .args(["-H", "commit", "link", "a1b2c3d", &task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("Linked commit a1b2c3d to task"))
        .stdout(predicate::str::contains(&task_id));
}

#[test]
fn test_commit_link_full_sha() {
    let temp = init_binnacle();

    let output = bn_in(&temp)
        .args(["task", "create", "Test task"])
        .output()
        .unwrap();
    let task_id = extract_task_id(&output);

    // Full 40-character SHA
    bn_in(&temp)
        .args([
            "commit",
            "link",
            "a1b2c3d4e5f6789012345678901234567890abcd",
            &task_id,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "\"sha\":\"a1b2c3d4e5f6789012345678901234567890abcd\"",
        ));
}

#[test]
fn test_commit_link_invalid_sha_too_short() {
    let temp = init_binnacle();

    let output = bn_in(&temp)
        .args(["task", "create", "Test task"])
        .output()
        .unwrap();
    let task_id = extract_task_id(&output);

    // SHA too short (less than 7 chars)
    bn_in(&temp)
        .args(["commit", "link", "abc", &task_id])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "SHA must be at least 7 characters",
        ));
}

#[test]
fn test_commit_link_invalid_sha_too_long() {
    let temp = init_binnacle();

    let output = bn_in(&temp)
        .args(["task", "create", "Test task"])
        .output()
        .unwrap();
    let task_id = extract_task_id(&output);

    // SHA too long (more than 40 chars)
    bn_in(&temp)
        .args([
            "commit",
            "link",
            "a1b2c3d4e5f6789012345678901234567890abcde",
            &task_id,
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "SHA must be at most 40 characters",
        ));
}

#[test]
fn test_commit_link_invalid_sha_non_hex() {
    let temp = init_binnacle();

    let output = bn_in(&temp)
        .args(["task", "create", "Test task"])
        .output()
        .unwrap();
    let task_id = extract_task_id(&output);

    // SHA with non-hex characters
    bn_in(&temp)
        .args(["commit", "link", "ghijklm", &task_id])
        .assert()
        .failure()
        .stderr(predicate::str::contains("only hex characters"));
}

#[test]
fn test_commit_link_nonexistent_task() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["commit", "link", "a1b2c3d", "bn-xxxx"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn test_commit_link_duplicate() {
    let temp = init_binnacle();

    let output = bn_in(&temp)
        .args(["task", "create", "Test task"])
        .output()
        .unwrap();
    let task_id = extract_task_id(&output);

    // First link succeeds
    bn_in(&temp)
        .args(["commit", "link", "a1b2c3d", &task_id])
        .assert()
        .success();

    // Second link fails
    bn_in(&temp)
        .args(["commit", "link", "a1b2c3d", &task_id])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already linked"));
}

// === Commit Unlink Tests ===

#[test]
fn test_commit_unlink() {
    let temp = init_binnacle();

    let output = bn_in(&temp)
        .args(["task", "create", "Test task"])
        .output()
        .unwrap();
    let task_id = extract_task_id(&output);

    // Link then unlink
    bn_in(&temp)
        .args(["commit", "link", "a1b2c3d", &task_id])
        .assert()
        .success();

    bn_in(&temp)
        .args(["commit", "unlink", "a1b2c3d", &task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"sha\":\"a1b2c3d\""))
        .stdout(predicate::str::contains(format!(
            "\"task_id\":\"{}\"",
            task_id
        )));
}

#[test]
fn test_commit_unlink_human_readable() {
    let temp = init_binnacle();

    let output = bn_in(&temp)
        .args(["task", "create", "Test task"])
        .output()
        .unwrap();
    let task_id = extract_task_id(&output);

    bn_in(&temp)
        .args(["commit", "link", "a1b2c3d", &task_id])
        .assert()
        .success();

    bn_in(&temp)
        .args(["-H", "commit", "unlink", "a1b2c3d", &task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Unlinked commit a1b2c3d from task",
        ))
        .stdout(predicate::str::contains(&task_id));
}

#[test]
fn test_commit_unlink_not_linked() {
    let temp = init_binnacle();

    let output = bn_in(&temp)
        .args(["task", "create", "Test task"])
        .output()
        .unwrap();
    let task_id = extract_task_id(&output);

    bn_in(&temp)
        .args(["commit", "unlink", "a1b2c3d", &task_id])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not linked"));
}

// === Commit List Tests ===

#[test]
fn test_commit_list_empty() {
    let temp = init_binnacle();

    let output = bn_in(&temp)
        .args(["task", "create", "Test task"])
        .output()
        .unwrap();
    let task_id = extract_task_id(&output);

    bn_in(&temp)
        .args(["commit", "list", &task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":0"));
}

#[test]
fn test_commit_list_empty_human_readable() {
    let temp = init_binnacle();

    let output = bn_in(&temp)
        .args(["task", "create", "Test task"])
        .output()
        .unwrap();
    let task_id = extract_task_id(&output);

    bn_in(&temp)
        .args(["-H", "commit", "list", &task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("No commits linked to task"));
}

#[test]
fn test_commit_list_with_commits() {
    let temp = init_binnacle();

    let output = bn_in(&temp)
        .args(["task", "create", "Test task"])
        .output()
        .unwrap();
    let task_id = extract_task_id(&output);

    // Link multiple commits
    bn_in(&temp)
        .args(["commit", "link", "a1b2c3d", &task_id])
        .assert()
        .success();
    bn_in(&temp)
        .args(["commit", "link", "e5f6789", &task_id])
        .assert()
        .success();

    bn_in(&temp)
        .args(["commit", "list", &task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":2"))
        .stdout(predicate::str::contains("a1b2c3d"))
        .stdout(predicate::str::contains("e5f6789"));
}

#[test]
fn test_commit_list_with_commits_human_readable() {
    let temp = init_binnacle();

    let output = bn_in(&temp)
        .args(["task", "create", "Test task"])
        .output()
        .unwrap();
    let task_id = extract_task_id(&output);

    bn_in(&temp)
        .args(["commit", "link", "a1b2c3d", &task_id])
        .assert()
        .success();
    bn_in(&temp)
        .args(["commit", "link", "e5f6789", &task_id])
        .assert()
        .success();

    bn_in(&temp)
        .args(["-H", "commit", "list", &task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("2 commit(s) linked to task"))
        .stdout(predicate::str::contains("a1b2c3d"))
        .stdout(predicate::str::contains("e5f6789"));
}

#[test]
fn test_commit_list_nonexistent_task() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["commit", "list", "bn-xxxx"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

// === Same Commit Multiple Tasks ===

#[test]
fn test_same_commit_multiple_tasks() {
    let temp = init_binnacle();

    // Create two tasks
    let output_a = bn_in(&temp)
        .args(["task", "create", "Task A"])
        .output()
        .unwrap();
    let task_a = extract_task_id(&output_a);

    let output_b = bn_in(&temp)
        .args(["task", "create", "Task B"])
        .output()
        .unwrap();
    let task_b = extract_task_id(&output_b);

    // Link same commit to both tasks
    bn_in(&temp)
        .args(["commit", "link", "a1b2c3d", &task_a])
        .assert()
        .success();
    bn_in(&temp)
        .args(["commit", "link", "a1b2c3d", &task_b])
        .assert()
        .success();

    // Both tasks should have the commit
    bn_in(&temp)
        .args(["commit", "list", &task_a])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":1"));
    bn_in(&temp)
        .args(["commit", "list", &task_b])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":1"));
}

// === Link then Unlink then Relist ===

#[test]
fn test_commit_link_unlink_list_roundtrip() {
    let temp = init_binnacle();

    let output = bn_in(&temp)
        .args(["task", "create", "Test task"])
        .output()
        .unwrap();
    let task_id = extract_task_id(&output);

    // Link
    bn_in(&temp)
        .args(["commit", "link", "a1b2c3d", &task_id])
        .assert()
        .success();

    // List shows 1 commit
    bn_in(&temp)
        .args(["commit", "list", &task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":1"));

    // Unlink
    bn_in(&temp)
        .args(["commit", "unlink", "a1b2c3d", &task_id])
        .assert()
        .success();

    // List shows 0 commits
    bn_in(&temp)
        .args(["commit", "list", &task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":0"));
}
