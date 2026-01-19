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
    bn_in(&temp).arg("init").assert().success();
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
            "task",
            "create",
            "Priority task",
            "-p",
            "1",
            "-t",
            "backend",
            "-t",
            "urgent",
            "-a",
            "agent-claude",
            "-d",
            "This is a detailed description",
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
    bn_in(&temp)
        .args(["task", "create", "Task 1", "-p", "1"])
        .assert()
        .success();
    bn_in(&temp)
        .args(["task", "create", "Task 2", "-p", "2"])
        .assert()
        .success();

    bn_in(&temp)
        .args(["task", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":2"));
}

#[test]
fn test_task_list_human() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["task", "create", "Task 1"])
        .assert()
        .success();

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

    bn_in(&temp)
        .args(["task", "create", "High", "-p", "1"])
        .assert()
        .success();
    bn_in(&temp)
        .args(["task", "create", "Low", "-p", "3"])
        .assert()
        .success();

    bn_in(&temp)
        .args(["task", "list", "--priority", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":1"));
}

#[test]
fn test_task_list_filter_by_tag() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["task", "create", "Backend", "-t", "backend"])
        .assert()
        .success();
    bn_in(&temp)
        .args(["task", "create", "Frontend", "-t", "frontend"])
        .assert()
        .success();

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
    let id = stdout
        .split("\"id\":\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();

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
    let id = stdout
        .split("\"id\":\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();

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
    let id = stdout
        .split("\"id\":\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();

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
    let show_output = bn_in(&temp).args(["task", "show", id]).output().unwrap();

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
    let id = stdout
        .split("\"id\":\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();

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
    let id = stdout
        .split("\"id\":\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();

    // Close the task
    bn_in(&temp).args(["task", "close", id]).assert().success();

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
    let id = stdout
        .split("\"id\":\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();

    bn_in(&temp).args(["task", "delete", id]).assert().success();

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

    bn_in(&temp)
        .args(["task", "create", "Task 1"])
        .assert()
        .success();
    bn_in(&temp)
        .args(["task", "create", "Task 2"])
        .assert()
        .success();

    bn_in(&temp)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"ready\""));
}

#[test]
fn test_status_human_readable() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["task", "create", "Task 1"])
        .assert()
        .success();

    bn_in(&temp)
        .arg("-H")
        .assert()
        .success()
        .stdout(predicate::str::contains("Binnacle"))
        .stdout(predicate::str::contains("task"));
}

// === Dependency Tests ===

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

#[test]
fn test_dep_add() {
    let temp = init_binnacle();

    // Create two tasks
    let output_a = bn_in(&temp)
        .args(["task", "create", "Task A"])
        .output()
        .unwrap();
    let id_a = extract_task_id(&output_a);

    let output_b = bn_in(&temp)
        .args(["task", "create", "Task B"])
        .output()
        .unwrap();
    let id_b = extract_task_id(&output_b);

    // B depends on A
    bn_in(&temp)
        .args(["dep", "add", &id_b, &id_a])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"child\""))
        .stdout(predicate::str::contains("\"parent\""));

    // Verify B now depends on A
    bn_in(&temp)
        .args(["task", "show", &id_b])
        .assert()
        .success()
        .stdout(predicate::str::contains(&id_a));
}

#[test]
fn test_dep_add_human_readable() {
    let temp = init_binnacle();

    let output_a = bn_in(&temp)
        .args(["task", "create", "Task A"])
        .output()
        .unwrap();
    let id_a = extract_task_id(&output_a);

    let output_b = bn_in(&temp)
        .args(["task", "create", "Task B"])
        .output()
        .unwrap();
    let id_b = extract_task_id(&output_b);

    bn_in(&temp)
        .args(["-H", "dep", "add", &id_b, &id_a])
        .assert()
        .success()
        .stdout(predicate::str::contains("Added dependency"))
        .stdout(predicate::str::contains("depends on"));
}

#[test]
fn test_dep_add_cycle_rejected() {
    let temp = init_binnacle();

    let output_a = bn_in(&temp)
        .args(["task", "create", "Task A"])
        .output()
        .unwrap();
    let id_a = extract_task_id(&output_a);

    let output_b = bn_in(&temp)
        .args(["task", "create", "Task B"])
        .output()
        .unwrap();
    let id_b = extract_task_id(&output_b);

    // A depends on B
    bn_in(&temp)
        .args(["dep", "add", &id_a, &id_b])
        .assert()
        .success();

    // B depends on A should fail (cycle)
    bn_in(&temp)
        .args(["dep", "add", &id_b, &id_a])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Cycle"));
}

#[test]
fn test_dep_rm() {
    let temp = init_binnacle();

    let output_a = bn_in(&temp)
        .args(["task", "create", "Task A"])
        .output()
        .unwrap();
    let id_a = extract_task_id(&output_a);

    let output_b = bn_in(&temp)
        .args(["task", "create", "Task B"])
        .output()
        .unwrap();
    let id_b = extract_task_id(&output_b);

    // Add then remove dependency
    bn_in(&temp)
        .args(["dep", "add", &id_b, &id_a])
        .assert()
        .success();

    bn_in(&temp)
        .args(["dep", "rm", &id_b, &id_a])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"child\""));

    // Verify dependency is removed
    let show_output = bn_in(&temp).args(["task", "show", &id_b]).output().unwrap();
    let stdout = String::from_utf8_lossy(&show_output.stdout);
    assert!(!stdout.contains(&id_a));
}

#[test]
fn test_dep_show() {
    let temp = init_binnacle();

    let output_a = bn_in(&temp)
        .args(["task", "create", "Task A"])
        .output()
        .unwrap();
    let id_a = extract_task_id(&output_a);

    let output_b = bn_in(&temp)
        .args(["task", "create", "Task B"])
        .output()
        .unwrap();
    let id_b = extract_task_id(&output_b);

    let output_c = bn_in(&temp)
        .args(["task", "create", "Task C"])
        .output()
        .unwrap();
    let id_c = extract_task_id(&output_c);

    // B depends on A, C depends on B
    bn_in(&temp)
        .args(["dep", "add", &id_b, &id_a])
        .assert()
        .success();
    bn_in(&temp)
        .args(["dep", "add", &id_c, &id_b])
        .assert()
        .success();

    // Show B's dependencies
    bn_in(&temp)
        .args(["dep", "show", &id_b])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"depends_on\""))
        .stdout(predicate::str::contains("\"dependents\""))
        .stdout(predicate::str::contains(&id_a))
        .stdout(predicate::str::contains(&id_c));
}

#[test]
fn test_dep_show_human_readable() {
    let temp = init_binnacle();

    let output_a = bn_in(&temp)
        .args(["task", "create", "Task A"])
        .output()
        .unwrap();
    let id_a = extract_task_id(&output_a);

    bn_in(&temp)
        .args(["-H", "dep", "show", &id_a])
        .assert()
        .success()
        .stdout(predicate::str::contains("Dependency graph"))
        .stdout(predicate::str::contains("Depends on"))
        .stdout(predicate::str::contains("Dependents"));
}

// === Ready/Blocked Query Tests ===

#[test]
fn test_ready_command() {
    let temp = init_binnacle();

    let output_a = bn_in(&temp)
        .args(["task", "create", "Task A"])
        .output()
        .unwrap();
    let id_a = extract_task_id(&output_a);

    let output_b = bn_in(&temp)
        .args(["task", "create", "Task B"])
        .output()
        .unwrap();
    let id_b = extract_task_id(&output_b);

    // B depends on A (which is pending, so B is blocked)
    bn_in(&temp)
        .args(["dep", "add", &id_b, &id_a])
        .assert()
        .success();

    // Only A should be ready
    bn_in(&temp)
        .args(["ready"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":1"))
        .stdout(predicate::str::contains(&id_a));
}

#[test]
fn test_ready_command_human_readable() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["task", "create", "Task A"])
        .assert()
        .success();

    bn_in(&temp)
        .args(["-H", "ready"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ready task"));
}

#[test]
fn test_blocked_command() {
    let temp = init_binnacle();

    let output_a = bn_in(&temp)
        .args(["task", "create", "Task A"])
        .output()
        .unwrap();
    let id_a = extract_task_id(&output_a);

    let output_b = bn_in(&temp)
        .args(["task", "create", "Task B"])
        .output()
        .unwrap();
    let id_b = extract_task_id(&output_b);

    // B depends on A
    bn_in(&temp)
        .args(["dep", "add", &id_b, &id_a])
        .assert()
        .success();

    // B should be blocked
    bn_in(&temp)
        .args(["blocked"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":1"))
        .stdout(predicate::str::contains(&id_b))
        .stdout(predicate::str::contains("blocking_tasks"));
}

#[test]
fn test_blocked_command_human_readable() {
    let temp = init_binnacle();

    let output_a = bn_in(&temp)
        .args(["task", "create", "Task A"])
        .output()
        .unwrap();
    let id_a = extract_task_id(&output_a);

    let output_b = bn_in(&temp)
        .args(["task", "create", "Task B"])
        .output()
        .unwrap();
    let id_b = extract_task_id(&output_b);

    bn_in(&temp)
        .args(["dep", "add", &id_b, &id_a])
        .assert()
        .success();

    bn_in(&temp)
        .args(["-H", "blocked"])
        .assert()
        .success()
        .stdout(predicate::str::contains("blocked task"))
        .stdout(predicate::str::contains("blocked by"));
}

#[test]
fn test_ready_after_closing_dependency() {
    let temp = init_binnacle();

    let output_a = bn_in(&temp)
        .args(["task", "create", "Task A"])
        .output()
        .unwrap();
    let id_a = extract_task_id(&output_a);

    let output_b = bn_in(&temp)
        .args(["task", "create", "Task B"])
        .output()
        .unwrap();
    let id_b = extract_task_id(&output_b);

    // B depends on A
    bn_in(&temp)
        .args(["dep", "add", &id_b, &id_a])
        .assert()
        .success();

    // Initially only A is ready
    bn_in(&temp)
        .args(["ready"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":1"));

    // Close A
    bn_in(&temp)
        .args(["task", "close", &id_a])
        .assert()
        .success();

    // Now B should be ready
    bn_in(&temp)
        .args(["ready"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":1"))
        .stdout(predicate::str::contains(&id_b));

    // And nothing should be blocked
    bn_in(&temp)
        .args(["blocked"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":0"));
}

#[test]
fn test_transitive_dependencies() {
    let temp = init_binnacle();

    // Create A -> B -> C chain (C depends on B, B depends on A)
    let output_a = bn_in(&temp)
        .args(["task", "create", "Task A"])
        .output()
        .unwrap();
    let id_a = extract_task_id(&output_a);

    let output_b = bn_in(&temp)
        .args(["task", "create", "Task B"])
        .output()
        .unwrap();
    let id_b = extract_task_id(&output_b);

    let output_c = bn_in(&temp)
        .args(["task", "create", "Task C"])
        .output()
        .unwrap();
    let id_c = extract_task_id(&output_c);

    bn_in(&temp)
        .args(["dep", "add", &id_b, &id_a])
        .assert()
        .success();
    bn_in(&temp)
        .args(["dep", "add", &id_c, &id_b])
        .assert()
        .success();

    // Only A should be ready
    bn_in(&temp)
        .args(["ready"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":1"))
        .stdout(predicate::str::contains(&id_a));

    // B and C should be blocked
    bn_in(&temp)
        .args(["blocked"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":2"));

    // dep show for C should show transitive deps
    bn_in(&temp)
        .args(["dep", "show", &id_c])
        .assert()
        .success()
        .stdout(predicate::str::contains("transitive_deps"))
        .stdout(predicate::str::contains(&id_b));
}
