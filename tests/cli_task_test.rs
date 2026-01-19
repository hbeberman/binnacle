//! Integration tests for Task CRUD operations via CLI.
//!
//! These tests verify that task commands work correctly through the CLI:
//! - `bn init` creates directory structure
//! - `bn task create/list/show/update/close/reopen/delete` all work
//! - JSON and human-readable output formats are correct
//! - Filtering by status, priority, and tags works

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

/// Get a Command for the bn binary, running in a temp directory.
fn bn_in(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("bn").unwrap();
    cmd.current_dir(dir.path());
    cmd
}

/// Initialize binnacle in a temp directory and return the temp dir.
fn init_binnacle() -> TempDir {
    let temp = TempDir::new().unwrap();
    bn_in(&temp)
        .arg("init")
        .assert()
        .success();
    temp
}

// === Init Tests ===

#[test]
fn test_init_creates_storage() {
    let temp = TempDir::new().unwrap();
    
    bn_in(&temp)
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"initialized\":true"));
}

#[test]
fn test_init_human_readable() {
    let temp = TempDir::new().unwrap();
    
    bn_in(&temp)
        .args(["init", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialized binnacle"));
}

#[test]
fn test_init_already_initialized() {
    let temp = init_binnacle();
    
    bn_in(&temp)
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"initialized\":false"));
}

// === Task Create Tests ===

#[test]
fn test_task_create_json() {
    let temp = init_binnacle();
    
    bn_in(&temp)
        .args(["task", "create", "My first task"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"id\":\"bn-"))
        .stdout(predicate::str::contains("\"title\":\"My first task\""));
}

#[test]
fn test_task_create_human() {
    let temp = init_binnacle();
    
    bn_in(&temp)
        .args(["-H", "task", "create", "My first task"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created task bn-"))
        .stdout(predicate::str::contains("\"My first task\""));
}

#[test]
fn test_task_create_with_options() {
    let temp = init_binnacle();
    
    bn_in(&temp)
        .args([
            "task", "create", "Priority task",
            "-p", "1",
            "-t", "backend",
            "-t", "urgent",
            "-a", "agent-claude",
            "-d", "This is a detailed description",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"id\":\"bn-"));
}

#[test]
fn test_task_create_invalid_priority() {
    let temp = init_binnacle();
    
    bn_in(&temp)
        .args(["task", "create", "Bad priority", "-p", "5"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Priority must be 0-4"));
}

// === Task List Tests ===

#[test]
fn test_task_list_empty() {
    let temp = init_binnacle();
    
    bn_in(&temp)
        .args(["task", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":0"));
}

#[test]
fn test_task_list_with_tasks() {
    let temp = init_binnacle();
    
    // Create some tasks
    bn_in(&temp).args(["task", "create", "Task 1", "-p", "1"]).assert().success();
    bn_in(&temp).args(["task", "create", "Task 2", "-p", "2"]).assert().success();
    
    bn_in(&temp)
        .args(["task", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":2"));
}

#[test]
fn test_task_list_human() {
    let temp = init_binnacle();
    
    bn_in(&temp).args(["task", "create", "Task 1"]).assert().success();
    
    bn_in(&temp)
        .args(["-H", "task", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 task(s)"))
        .stdout(predicate::str::contains("Task 1"));
}

#[test]
fn test_task_list_filter_by_priority() {
    let temp = init_binnacle();
    
    bn_in(&temp).args(["task", "create", "High", "-p", "1"]).assert().success();
    bn_in(&temp).args(["task", "create", "Low", "-p", "3"]).assert().success();
    
    bn_in(&temp)
        .args(["task", "list", "--priority", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":1"));
}

#[test]
fn test_task_list_filter_by_tag() {
    let temp = init_binnacle();
    
    bn_in(&temp).args(["task", "create", "Backend", "-t", "backend"]).assert().success();
    bn_in(&temp).args(["task", "create", "Frontend", "-t", "frontend"]).assert().success();
    
    bn_in(&temp)
        .args(["task", "list", "--tag", "backend"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":1"))
        .stdout(predicate::str::contains("Backend"));
}

// === Task Show Tests ===

#[test]
fn test_task_show() {
    let temp = init_binnacle();
    
    // Create a task and capture its ID
    let output = bn_in(&temp)
        .args(["task", "create", "Test show"])
        .output()
        .unwrap();
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Extract ID from JSON output like {"id":"bn-xxxx","title":"Test show"}
    let id = stdout
        .split("\"id\":\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    
    bn_in(&temp)
        .args(["task", "show", id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"title\":\"Test show\""))
        .stdout(predicate::str::contains("\"status\":\"pending\""));
}

#[test]
fn test_task_show_not_found() {
    let temp = init_binnacle();
    
    bn_in(&temp)
        .args(["task", "show", "bn-xxxx"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

// === Task Update Tests ===

#[test]
fn test_task_update_title() {
    let temp = init_binnacle();
    
    // Create a task
    let output = bn_in(&temp)
        .args(["task", "create", "Original"])
        .output()
        .unwrap();
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id = stdout.split("\"id\":\"").nth(1).unwrap().split('"').next().unwrap();
    
    // Update the title
    bn_in(&temp)
        .args(["task", "update", id, "--title", "Updated"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"updated_fields\""))
        .stdout(predicate::str::contains("title"));
    
    // Verify the update
    bn_in(&temp)
        .args(["task", "show", id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"title\":\"Updated\""));
}

#[test]
fn test_task_update_status() {
    let temp = init_binnacle();
    
    let output = bn_in(&temp)
        .args(["task", "create", "Test"])
        .output()
        .unwrap();
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id = stdout.split("\"id\":\"").nth(1).unwrap().split('"').next().unwrap();
    
    bn_in(&temp)
        .args(["task", "update", id, "--status", "in_progress"])
        .assert()
        .success();
    
    bn_in(&temp)
        .args(["task", "show", id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"in_progress\""));
}

#[test]
fn test_task_update_add_remove_tags() {
    let temp = init_binnacle();
    
    let output = bn_in(&temp)
        .args(["task", "create", "Test", "-t", "initial"])
        .output()
        .unwrap();
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id = stdout.split("\"id\":\"").nth(1).unwrap().split('"').next().unwrap();
    
    // Add a tag
    bn_in(&temp)
        .args(["task", "update", id, "--add-tag", "new"])
        .assert()
        .success();
    
    // Verify both tags exist
    bn_in(&temp)
        .args(["task", "show", id])
        .assert()
        .success()
        .stdout(predicate::str::contains("initial"))
        .stdout(predicate::str::contains("new"));
    
    // Remove initial tag
    bn_in(&temp)
        .args(["task", "update", id, "--remove-tag", "initial"])
        .assert()
        .success();
    
    // Verify only new tag remains
    let show_output = bn_in(&temp)
        .args(["task", "show", id])
        .output()
        .unwrap();
    
    let show_stdout = String::from_utf8_lossy(&show_output.stdout);
    assert!(show_stdout.contains("new"));
    assert!(!show_stdout.contains("initial"));
}

// === Task Close/Reopen Tests ===

#[test]
fn test_task_close() {
    let temp = init_binnacle();
    
    let output = bn_in(&temp)
        .args(["task", "create", "To close"])
        .output()
        .unwrap();
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id = stdout.split("\"id\":\"").nth(1).unwrap().split('"').next().unwrap();
    
    bn_in(&temp)
        .args(["task", "close", id, "--reason", "Completed"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"done\""));
    
    bn_in(&temp)
        .args(["task", "show", id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"done\""))
        .stdout(predicate::str::contains("\"closed_reason\":\"Completed\""));
}

#[test]
fn test_task_reopen() {
    let temp = init_binnacle();
    
    let output = bn_in(&temp)
        .args(["task", "create", "To reopen"])
        .output()
        .unwrap();
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id = stdout.split("\"id\":\"").nth(1).unwrap().split('"').next().unwrap();
    
    // Close the task
    bn_in(&temp)
        .args(["task", "close", id])
        .assert()
        .success();
    
    // Reopen the task
    bn_in(&temp)
        .args(["task", "reopen", id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"reopened\""));
    
    bn_in(&temp)
        .args(["task", "show", id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"reopened\""));
}

// === Task Delete Tests ===

#[test]
fn test_task_delete() {
    let temp = init_binnacle();
    
    let output = bn_in(&temp)
        .args(["task", "create", "To delete"])
        .output()
        .unwrap();
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id = stdout.split("\"id\":\"").nth(1).unwrap().split('"').next().unwrap();
    
    bn_in(&temp)
        .args(["task", "delete", id])
        .assert()
        .success();
    
    // Task should no longer exist in list
    bn_in(&temp)
        .args(["task", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":0"));
}

// === Status Summary Tests ===

#[test]
fn test_status_not_initialized() {
    let temp = TempDir::new().unwrap();
    
    bn_in(&temp)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"initialized\": false"));
}

#[test]
fn test_status_with_tasks() {
    let temp = init_binnacle();
    
    bn_in(&temp).args(["task", "create", "Task 1"]).assert().success();
    bn_in(&temp).args(["task", "create", "Task 2"]).assert().success();
    
    bn_in(&temp)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"ready\""));
}

#[test]
fn test_status_human_readable() {
    let temp = init_binnacle();
    
    bn_in(&temp).args(["task", "create", "Task 1"]).assert().success();
    
    bn_in(&temp)
        .arg("-H")
        .assert()
        .success()
        .stdout(predicate::str::contains("Binnacle"))
        .stdout(predicate::str::contains("task"));
}
