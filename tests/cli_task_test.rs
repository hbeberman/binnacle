//! Integration tests for Task CRUD operations via CLI.
//!
//! These tests verify that task commands work correctly through the CLI:
//! - `bn system init` creates directory structure
//! - `bn task create/list/show/update/close/reopen/delete` all work
//! - JSON and human-readable output formats are correct
//! - Filtering by status, priority, and tags works

mod common;

use assert_cmd::Command;
use common::TestEnv;
use predicates::prelude::*;

/// Get a Command for the bn binary in a TestEnv.
fn bn_in(env: &TestEnv) -> Command {
    env.bn()
}

/// Initialize binnacle in a temp directory and return the TestEnv.
fn init_binnacle() -> TestEnv {
    TestEnv::init()
}

// === Init Tests ===

#[test]
fn test_init_creates_storage() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["system", "init"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"initialized\":true"));
}

#[test]
fn test_init_human_readable() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["system", "init", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialized binnacle"));
}

#[test]
fn test_init_already_initialized() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["system", "init"])
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
        .stdout(predicate::str::contains("1 task:"))
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
    let temp = TestEnv::new();

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
fn test_link_add() {
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
        .args([
            "link",
            "add",
            &id_b,
            &id_a,
            "--type",
            "depends_on",
            "--reason",
            "test dep",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"source\""))
        .stdout(predicate::str::contains("\"target\""));

    // Verify B now depends on A
    bn_in(&temp)
        .args(["task", "show", &id_b])
        .assert()
        .success()
        .stdout(predicate::str::contains(&id_a));
}

#[test]
fn test_link_add_human_readable() {
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
        .args([
            "-H",
            "link",
            "add",
            &id_b,
            &id_a,
            "--type",
            "depends_on",
            "--reason",
            "test dep",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created link"));
}

#[test]
fn test_link_add_cycle_rejected() {
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
        .args([
            "link",
            "add",
            &id_a,
            &id_b,
            "--type",
            "depends_on",
            "--reason",
            "test dep",
        ])
        .assert()
        .success();

    // B depends on A should fail (cycle)
    bn_in(&temp)
        .args([
            "link",
            "add",
            &id_b,
            &id_a,
            "--type",
            "depends_on",
            "--reason",
            "test dep",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Cycle"));
}

#[test]
fn test_link_rm() {
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
        .args([
            "link",
            "add",
            &id_b,
            &id_a,
            "--type",
            "depends_on",
            "--reason",
            "test dep",
        ])
        .assert()
        .success();

    bn_in(&temp)
        .args(["link", "rm", &id_b, &id_a, "--type", "depends_on"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"source\""));

    // Verify dependency is removed
    let show_output = bn_in(&temp).args(["task", "show", &id_b]).output().unwrap();
    let stdout = String::from_utf8_lossy(&show_output.stdout);
    assert!(!stdout.contains(&id_a));
}

#[test]
fn test_link_list() {
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
        .args([
            "link",
            "add",
            &id_b,
            &id_a,
            "--type",
            "depends_on",
            "--reason",
            "test dep",
        ])
        .assert()
        .success();
    bn_in(&temp)
        .args([
            "link",
            "add",
            &id_c,
            &id_b,
            "--type",
            "depends_on",
            "--reason",
            "test dep",
        ])
        .assert()
        .success();

    // Show B's links (should show outbound to A and inbound from C)
    bn_in(&temp)
        .args(["link", "list", &id_b])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"edge_type\""))
        .stdout(predicate::str::contains(&id_a))
        .stdout(predicate::str::contains(&id_c));
}

#[test]
fn test_link_list_human_readable() {
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
        .args([
            "link",
            "add",
            &id_b,
            &id_a,
            "--type",
            "depends_on",
            "--reason",
            "test dep",
        ])
        .assert()
        .success();

    bn_in(&temp)
        .args(["-H", "link", "list", &id_b])
        .assert()
        .success()
        .stdout(predicate::str::contains("link(s)"))
        .stdout(predicate::str::contains(&id_a));
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
        .args([
            "link",
            "add",
            &id_b,
            &id_a,
            "--type",
            "depends_on",
            "--reason",
            "test dep",
        ])
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
        .stdout(predicate::str::contains("task ready"));
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
        .args([
            "link",
            "add",
            &id_b,
            &id_a,
            "--type",
            "depends_on",
            "--reason",
            "test dep",
        ])
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
        .args([
            "link",
            "add",
            &id_b,
            &id_a,
            "--type",
            "depends_on",
            "--reason",
            "test dep",
        ])
        .assert()
        .success();

    bn_in(&temp)
        .args(["-H", "blocked"])
        .assert()
        .success()
        .stdout(predicate::str::contains("blocked task"))
        .stdout(predicate::str::contains("Task B"));
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
        .args([
            "link",
            "add",
            &id_b,
            &id_a,
            "--type",
            "depends_on",
            "--reason",
            "test dep",
        ])
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
        .args([
            "link",
            "add",
            &id_b,
            &id_a,
            "--type",
            "depends_on",
            "--reason",
            "test dep",
        ])
        .assert()
        .success();
    bn_in(&temp)
        .args([
            "link",
            "add",
            &id_c,
            &id_b,
            "--type",
            "depends_on",
            "--reason",
            "test dep",
        ])
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

    // link list for C should show its direct link to B
    bn_in(&temp)
        .args(["link", "list", &id_c])
        .assert()
        .success()
        .stdout(predicate::str::contains(&id_b));
}

// === Blocker Analysis Tests (task show with blocking info) ===

#[test]
fn test_task_show_no_blocking_info_when_no_deps() {
    let temp = init_binnacle();

    let output = bn_in(&temp)
        .args(["task", "create", "Solo Task"])
        .output()
        .unwrap();
    let id = extract_task_id(&output);

    // JSON output should not have blocking_info field
    bn_in(&temp)
        .args(["task", "show", &id])
        .assert()
        .success()
        .stdout(predicate::str::contains(&id))
        .stdout(predicate::str::contains("blocking_info").not());
}

#[test]
fn test_task_show_blocking_info_json_format() {
    let temp = init_binnacle();

    // Create A and B
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

    // B depends on A (pending)
    bn_in(&temp)
        .args([
            "link",
            "add",
            &id_b,
            &id_a,
            "--type",
            "depends_on",
            "--reason",
            "test dep",
        ])
        .assert()
        .success();

    // Show B, should have blocking_info
    bn_in(&temp)
        .args(["task", "show", &id_b])
        .assert()
        .success()
        .stdout(predicate::str::contains("blocking_info"))
        .stdout(predicate::str::contains("is_blocked"))
        .stdout(predicate::str::contains("blocker_count"))
        .stdout(predicate::str::contains("direct_blockers"))
        .stdout(predicate::str::contains(&id_a));
}

#[test]
fn test_task_show_blocking_info_human_readable() {
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
        .args([
            "link",
            "add",
            &id_b,
            &id_a,
            "--type",
            "depends_on",
            "--reason",
            "test dep",
        ])
        .assert()
        .success();

    // Show B with -H, should show blocker summary
    bn_in(&temp)
        .args(["task", "show", &id_b, "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Blocked by"))
        .stdout(predicate::str::contains("incomplete dependencies"))
        .stdout(predicate::str::contains(&id_a));
}

#[test]
fn test_task_show_all_deps_complete() {
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

    // Close A
    bn_in(&temp)
        .args(["task", "close", &id_a, "--reason", "Done"])
        .assert()
        .success();

    // B depends on A (done)
    bn_in(&temp)
        .args([
            "link",
            "add",
            &id_b,
            &id_a,
            "--type",
            "depends_on",
            "--reason",
            "test dep",
        ])
        .assert()
        .success();

    // Show B, should show not blocked
    bn_in(&temp)
        .args(["task", "show", &id_b, "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("All dependencies complete"));
}

#[test]
fn test_task_show_transitive_blockers_json() {
    let temp = init_binnacle();

    // Create chain: C -> B -> A
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
        .args([
            "link",
            "add",
            &id_b,
            &id_a,
            "--type",
            "depends_on",
            "--reason",
            "test dep",
        ])
        .assert()
        .success();
    bn_in(&temp)
        .args([
            "link",
            "add",
            &id_c,
            &id_b,
            "--type",
            "depends_on",
            "--reason",
            "test dep",
        ])
        .assert()
        .success();

    // Show C, should show B as blocker with A in blocked_by
    bn_in(&temp)
        .args(["task", "show", &id_c])
        .assert()
        .success()
        .stdout(predicate::str::contains("blocking_info"))
        .stdout(predicate::str::contains(&id_b))
        .stdout(predicate::str::contains("blocked_by"))
        .stdout(predicate::str::contains(&id_a))
        .stdout(predicate::str::contains("blocker_chain"));
}

#[test]
fn test_task_show_multiple_blockers() {
    let temp = init_binnacle();

    let output_a = bn_in(&temp)
        .args(["task", "create", "Task A"])
        .output()
        .unwrap();
    let id_a = extract_task_id(&output_a);

    let output_b = bn_in(&temp)
        .args(["task", "create", "Task B", "--assignee", "alice"])
        .output()
        .unwrap();
    let id_b = extract_task_id(&output_b);

    let output_c = bn_in(&temp)
        .args(["task", "create", "Task C"])
        .output()
        .unwrap();
    let id_c = extract_task_id(&output_c);

    // C depends on both A and B
    bn_in(&temp)
        .args([
            "link",
            "add",
            &id_c,
            &id_a,
            "--type",
            "depends_on",
            "--reason",
            "test dep",
        ])
        .assert()
        .success();
    bn_in(&temp)
        .args([
            "link",
            "add",
            &id_c,
            &id_b,
            "--type",
            "depends_on",
            "--reason",
            "test dep",
        ])
        .assert()
        .success();

    // Show C with both blockers
    bn_in(&temp)
        .args(["task", "show", &id_c, "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Blocked by 2 incomplete dependencies",
        ))
        .stdout(predicate::str::contains(&id_a))
        .stdout(predicate::str::contains(&id_b))
        .stdout(predicate::str::contains("alice"));
}

#[test]
fn test_task_show_cancelled_deps_dont_block() {
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

    // Cancel A
    bn_in(&temp)
        .args(["task", "update", &id_a, "--status", "cancelled"])
        .assert()
        .success();

    // B depends on A (cancelled)
    bn_in(&temp)
        .args([
            "link",
            "add",
            &id_b,
            &id_a,
            "--type",
            "depends_on",
            "--reason",
            "test dep",
        ])
        .assert()
        .success();

    // Show B, should not be blocked
    bn_in(&temp)
        .args(["task", "show", &id_b, "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("All dependencies complete"));
}

#[test]
fn test_task_show_mixed_status_blockers() {
    let temp = init_binnacle();

    // Create A (pending), B (in_progress), C (done)
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

    let output_d = bn_in(&temp)
        .args(["task", "create", "Task D"])
        .output()
        .unwrap();
    let id_d = extract_task_id(&output_d);

    // Set statuses
    bn_in(&temp)
        .args(["task", "update", &id_b, "--status", "in_progress"])
        .assert()
        .success();
    bn_in(&temp)
        .args(["task", "close", &id_c, "--reason", "Done"])
        .assert()
        .success();

    // D depends on all three
    bn_in(&temp)
        .args([
            "link",
            "add",
            &id_d,
            &id_a,
            "--type",
            "depends_on",
            "--reason",
            "test dep",
        ])
        .assert()
        .success();
    bn_in(&temp)
        .args([
            "link",
            "add",
            &id_d,
            &id_b,
            "--type",
            "depends_on",
            "--reason",
            "test dep",
        ])
        .assert()
        .success();
    bn_in(&temp)
        .args([
            "link",
            "add",
            &id_d,
            &id_c,
            "--type",
            "depends_on",
            "--reason",
            "test dep",
        ])
        .assert()
        .success();

    // Show D, should show only A and B as blockers (not C which is done)
    let output = bn_in(&temp).args(["task", "show", &id_d]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(stdout.contains("\"blocker_count\":2"));
    assert!(stdout.contains(&id_a));
    assert!(stdout.contains(&id_b));
    assert!(stdout.contains("pending"));
    assert!(stdout.contains("inprogress"));
}

// === Short Name Tests ===

#[test]
fn test_task_create_with_short_name() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["task", "create", "My task with short name", "-s", "MyTask"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"short_name\":\"MyTask\""));
}

#[test]
fn test_task_create_with_short_name_human() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args([
            "-H",
            "task",
            "create",
            "My task with short name",
            "-s",
            "MyTask",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("[MyTask]"));
}

#[test]
fn test_task_update_short_name() {
    let temp = init_binnacle();

    // Create task without short_name
    let output = bn_in(&temp)
        .args(["task", "create", "Task without short name"])
        .output()
        .unwrap();
    let id = extract_task_id(&output);

    // Update to add short_name
    bn_in(&temp)
        .args(["task", "update", &id, "-s", "Added"])
        .assert()
        .success();

    // Verify short_name was added
    bn_in(&temp)
        .args(["task", "show", &id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"short_name\":\"Added\""));
}

#[test]
fn test_task_update_short_name_human() {
    let temp = init_binnacle();

    let output = bn_in(&temp)
        .args(["task", "create", "Task without short name"])
        .output()
        .unwrap();
    let id = extract_task_id(&output);

    bn_in(&temp)
        .args(["-H", "task", "update", &id, "-s", "NewName"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated task"));
}

#[test]
fn test_task_short_name_empty_string() {
    let temp = init_binnacle();

    // Empty short_name should be treated as None (not included in output)
    bn_in(&temp)
        .args(["task", "create", "Task with empty short name", "-s", ""])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"short_name\"").not());
}

#[test]
fn test_task_short_name_long_truncation() {
    let temp = init_binnacle();

    // A very long short_name (>30 chars) should be auto-truncated with a note
    bn_in(&temp)
        .args([
            "task",
            "create",
            "Task with long short name",
            "-s",
            "VeryLongShortNameThatExceedsThirtyChars",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("truncated"));

    // Verify it was actually truncated to 30 chars
    bn_in(&temp)
        .args(["task", "list"])
        .assert()
        .success()
        // The truncated name should be exactly 30 chars
        .stdout(predicate::str::contains(
            "\"short_name\":\"VeryLongShortNameThatExceeds",
        ));
}

#[test]
fn test_task_short_name_special_characters() {
    let temp = init_binnacle();

    // Short names with special characters should work
    bn_in(&temp)
        .args([
            "task",
            "create",
            "Task with special chars",
            "-s",
            "My-Task_1",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"short_name\":\"My-Task_1\""));
}

#[test]
fn test_task_short_name_persists() {
    let temp = init_binnacle();

    // Create task with short_name
    let output = bn_in(&temp)
        .args(["task", "create", "Persistent task", "-s", "Persist"])
        .output()
        .unwrap();
    let id = extract_task_id(&output);

    // Verify it persists through list
    bn_in(&temp)
        .args(["task", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"short_name\":\"Persist\""));

    // Verify it persists through show
    bn_in(&temp)
        .args(["task", "show", &id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"short_name\":\"Persist\""));
}

#[test]
fn test_task_show_short_name_human() {
    let temp = init_binnacle();

    let output = bn_in(&temp)
        .args(["task", "create", "Task with short name", "-s", "ShortNm"])
        .output()
        .unwrap();
    let id = extract_task_id(&output);

    bn_in(&temp)
        .args(["-H", "task", "show", &id])
        .assert()
        .success()
        .stdout(predicate::str::contains("Short Name: ShortNm"));
}

// === Short Name Edge Case Tests ===

#[test]
fn test_task_short_name_unicode() {
    let temp = init_binnacle();

    // Unicode characters in short_name should work
    bn_in(&temp)
        .args(["task", "create", "Task with unicode", "-s", "任务α"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"short_name\":\"任务α\""));
}

#[test]
fn test_task_short_name_emoji() {
    let temp = init_binnacle();

    // Emoji in short_name should work
    bn_in(&temp)
        .args(["task", "create", "Task with emoji", "-s", "🚀Ship"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"short_name\":\"🚀Ship\""));
}

#[test]
fn test_task_short_name_single_char() {
    let temp = init_binnacle();

    // Single character short_name should work
    bn_in(&temp)
        .args(["task", "create", "Task with single char", "-s", "X"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"short_name\":\"X\""));
}

#[test]
fn test_task_short_name_whitespace_only() {
    let temp = init_binnacle();

    // Whitespace-only short_name should be treated as None (like empty string)
    bn_in(&temp)
        .args(["task", "create", "Task with whitespace", "-s", "   "])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"short_name\"").not());
}

#[test]
fn test_task_short_name_with_spaces() {
    let temp = init_binnacle();

    // Short name with internal spaces should work
    bn_in(&temp)
        .args(["task", "create", "Task with spaces", "-s", "My Task"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"short_name\":\"My Task\""));
}

#[test]
fn test_task_update_clear_short_name() {
    let temp = init_binnacle();

    // Create task with short_name
    let output = bn_in(&temp)
        .args(["task", "create", "Task to clear", "-s", "HasName"])
        .output()
        .unwrap();
    let id = extract_task_id(&output);

    // Update with empty string to clear short_name
    bn_in(&temp)
        .args(["task", "update", &id, "-s", ""])
        .assert()
        .success();

    // Verify short_name was cleared
    bn_in(&temp)
        .args(["task", "show", &id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"short_name\"").not());
}

#[test]
fn test_blocked_shows_edge_based_blockers() {
    let temp = init_binnacle();

    // Create two tasks
    let output_a = bn_in(&temp)
        .args(["task", "create", "Blocker Task"])
        .output()
        .unwrap();
    let id_a = extract_task_id(&output_a);

    let output_b = bn_in(&temp)
        .args(["task", "create", "Blocked Task"])
        .output()
        .unwrap();
    let id_b = extract_task_id(&output_b);

    // B depends on A via edge (not legacy depends_on)
    bn_in(&temp)
        .args([
            "link",
            "add",
            &id_b,
            &id_a,
            "--type",
            "depends_on",
            "--reason",
            "test dep",
        ])
        .assert()
        .success();

    // Blocked command should show A as the blocker for B
    bn_in(&temp)
        .args(["blocked"])
        .assert()
        .success()
        .stdout(predicate::str::contains(&id_b))
        .stdout(predicate::str::contains(&id_a)); // Blocker should be shown
}

#[test]
fn test_blocked_human_shows_edge_blocker_id() {
    let temp = init_binnacle();

    let output_a = bn_in(&temp)
        .args(["task", "create", "Blocker"])
        .output()
        .unwrap();
    let id_a = extract_task_id(&output_a);

    let output_b = bn_in(&temp)
        .args(["task", "create", "Blocked"])
        .output()
        .unwrap();
    let id_b = extract_task_id(&output_b);

    bn_in(&temp)
        .args([
            "link",
            "add",
            &id_b,
            &id_a,
            "--type",
            "depends_on",
            "--reason",
            "test dep",
        ])
        .assert()
        .success();

    // Human-readable blocked should show blocker ID
    bn_in(&temp)
        .args(["-H", "blocked"])
        .assert()
        .success()
        .stdout(predicate::str::contains("blocked by"))
        .stdout(predicate::str::contains(&id_a));
}

// === Generic Show Command Tests ===

#[test]
fn test_show_task_json() {
    let temp = init_binnacle();

    let output = bn_in(&temp)
        .args(["task", "create", "Test task"])
        .output()
        .unwrap();
    let id = extract_task_id(&output);

    // Generic show should auto-detect task type
    bn_in(&temp)
        .args(["show", &id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"type\":\"task\""))
        .stdout(predicate::str::contains("\"task\":"))
        .stdout(predicate::str::contains(&id));
}

#[test]
fn test_show_task_human() {
    let temp = init_binnacle();

    let output = bn_in(&temp)
        .args(["task", "create", "Test task"])
        .output()
        .unwrap();
    let id = extract_task_id(&output);

    bn_in(&temp)
        .args(["-H", "show", &id])
        .assert()
        .success()
        .stdout(predicate::str::contains("Task:"))
        .stdout(predicate::str::contains(&id));
}

#[test]
fn test_show_not_found() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["show", "bn-nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Entity not found"));
}

#[test]
fn test_show_milestone() {
    let temp = init_binnacle();

    let output = bn_in(&temp)
        .args(["milestone", "create", "Test milestone"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let id = json["id"].as_str().unwrap();

    // Generic show should auto-detect milestone type
    bn_in(&temp)
        .args(["show", id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"type\":\"milestone\""))
        .stdout(predicate::str::contains("\"milestone\":"));
}

// === Commit Required Closure Tests ===

#[test]
fn test_task_close_fails_without_commit_when_required() {
    let temp = init_binnacle();

    // Enable require_commit_for_close
    bn_in(&temp)
        .args(["config", "set", "require_commit_for_close", "true"])
        .assert()
        .success();

    // Create a task
    let output = bn_in(&temp)
        .args(["task", "create", "Task needing commit"])
        .output()
        .unwrap();
    let id = extract_task_id(&output);

    // Closing should fail without linked commit
    bn_in(&temp)
        .args(["task", "close", &id])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no commits linked"))
        .stderr(predicate::str::contains("bn commit link"));
}

#[test]
fn test_task_close_force_bypasses_commit_requirement() {
    let temp = init_binnacle();

    // Enable require_commit_for_close
    bn_in(&temp)
        .args(["config", "set", "require_commit_for_close", "true"])
        .assert()
        .success();

    // Create a task
    let output = bn_in(&temp)
        .args(["task", "create", "Task using force"])
        .output()
        .unwrap();
    let id = extract_task_id(&output);

    // Closing with --force should succeed
    bn_in(&temp)
        .args(["task", "close", &id, "--force"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"done\""));
}

#[test]
fn test_task_close_succeeds_with_linked_commit() {
    let temp = init_binnacle();

    // Enable require_commit_for_close
    bn_in(&temp)
        .args(["config", "set", "require_commit_for_close", "true"])
        .assert()
        .success();

    // Create a task
    let output = bn_in(&temp)
        .args(["task", "create", "Task with commit"])
        .output()
        .unwrap();
    let id = extract_task_id(&output);

    // Link a commit
    bn_in(&temp)
        .args([
            "commit",
            "link",
            "abc1234def5678abc1234def5678abc1234def56",
            &id,
        ])
        .assert()
        .success();

    // Closing should succeed with linked commit
    bn_in(&temp)
        .args(["task", "close", &id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"done\""));
}

#[test]
fn test_task_close_works_without_commit_when_disabled() {
    let temp = init_binnacle();

    // Explicitly disable require_commit_for_close (default)
    bn_in(&temp)
        .args(["config", "set", "require_commit_for_close", "false"])
        .assert()
        .success();

    // Create a task
    let output = bn_in(&temp)
        .args(["task", "create", "Task without requirement"])
        .output()
        .unwrap();
    let id = extract_task_id(&output);

    // Closing should succeed without linked commit
    bn_in(&temp)
        .args(["task", "close", &id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"done\""));
}

#[test]
fn test_task_update_status_done_fails_without_commit_when_required() {
    let temp = init_binnacle();

    // Enable require_commit_for_close
    bn_in(&temp)
        .args(["config", "set", "require_commit_for_close", "true"])
        .assert()
        .success();

    // Create a task
    let output = bn_in(&temp)
        .args(["task", "create", "Task for update"])
        .output()
        .unwrap();
    let id = extract_task_id(&output);

    // Updating to done should fail without linked commit
    bn_in(&temp)
        .args(["task", "update", &id, "--status", "done"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no commits linked"))
        .stderr(predicate::str::contains("bn task update"))
        .stderr(predicate::str::contains("--force"));
}

#[test]
fn test_task_update_status_done_force_bypasses_requirement() {
    let temp = init_binnacle();

    // Enable require_commit_for_close
    bn_in(&temp)
        .args(["config", "set", "require_commit_for_close", "true"])
        .assert()
        .success();

    // Create a task
    let output = bn_in(&temp)
        .args(["task", "create", "Task for update force"])
        .output()
        .unwrap();
    let id = extract_task_id(&output);

    // Updating to done with --force should succeed
    bn_in(&temp)
        .args(["task", "update", &id, "--status", "done", "--force"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"updated_fields\""));
}

#[test]
fn test_task_update_status_cancelled_ignores_commit_requirement() {
    let temp = init_binnacle();

    // Enable require_commit_for_close
    bn_in(&temp)
        .args(["config", "set", "require_commit_for_close", "true"])
        .assert()
        .success();

    // Create a task
    let output = bn_in(&temp)
        .args(["task", "create", "Task to cancel"])
        .output()
        .unwrap();
    let id = extract_task_id(&output);

    // Updating to cancelled should succeed without linked commit
    bn_in(&temp)
        .args(["task", "update", &id, "--status", "cancelled"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"updated_fields\""));
}

// === Closed Task Update Protection Tests ===

#[test]
fn test_closed_task_update_without_flag_fails() {
    let temp = init_binnacle();

    // Create and close a task
    let output = bn_in(&temp)
        .args(["task", "create", "Task to close"])
        .output()
        .unwrap();
    let id = extract_task_id(&output);

    bn_in(&temp)
        .args(["task", "close", &id, "--reason", "Done"])
        .assert()
        .success();

    // Try to update without flag - should fail
    bn_in(&temp)
        .args(["task", "update", &id, "--title", "New Title"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Cannot update closed task"))
        .stderr(predicate::str::contains("--keep-closed"))
        .stderr(predicate::str::contains("--reopen"));
}

#[test]
fn test_closed_task_update_with_keep_closed_succeeds() {
    let temp = init_binnacle();

    // Create and close a task
    let output = bn_in(&temp)
        .args(["task", "create", "Task to close"])
        .output()
        .unwrap();
    let id = extract_task_id(&output);

    bn_in(&temp)
        .args(["task", "close", &id, "--reason", "Done"])
        .assert()
        .success();

    // Update with --keep-closed - should succeed
    bn_in(&temp)
        .args([
            "task",
            "update",
            &id,
            "--title",
            "Updated Title",
            "--keep-closed",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"updated_fields\""));

    // Verify task is still done
    bn_in(&temp)
        .args(["task", "show", &id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"done\""))
        .stdout(predicate::str::contains("Updated Title"));
}

#[test]
fn test_closed_task_update_with_reopen_succeeds() {
    let temp = init_binnacle();

    // Create and close a task
    let output = bn_in(&temp)
        .args(["task", "create", "Task to close"])
        .output()
        .unwrap();
    let id = extract_task_id(&output);

    bn_in(&temp)
        .args(["task", "close", &id, "--reason", "Done"])
        .assert()
        .success();

    // Update with --reopen - should succeed and reopen
    bn_in(&temp)
        .args([
            "task",
            "update",
            &id,
            "--title",
            "Reopened Title",
            "--reopen",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"updated_fields\""));

    // Verify task is now pending
    bn_in(&temp)
        .args(["task", "show", &id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"pending\""))
        .stdout(predicate::str::contains("Reopened Title"));
}

#[test]
fn test_cancelled_task_update_without_flag_fails() {
    let temp = init_binnacle();

    // Create and cancel a task
    let output = bn_in(&temp)
        .args(["task", "create", "Task to cancel"])
        .output()
        .unwrap();
    let id = extract_task_id(&output);

    bn_in(&temp)
        .args(["task", "update", &id, "--status", "cancelled"])
        .assert()
        .success();

    // Try to update without flag - should fail
    bn_in(&temp)
        .args(["task", "update", &id, "--title", "New Title"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Cannot update closed task"));
}

#[test]
fn test_task_close_json_includes_goodbye_hint() {
    let temp = init_binnacle();

    // Create a task
    let output = bn_in(&temp)
        .args(["task", "create", "Task with hint"])
        .output()
        .unwrap();
    let id = extract_task_id(&output);

    // Close the task and check JSON output includes hint
    let close_output = bn_in(&temp)
        .args(["task", "close", &id, "--reason", "Done"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&close_output.stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Output should be valid JSON");

    // Should have hint field reminding about goodbye
    assert!(json.get("hint").is_some(), "JSON should include hint field");
    assert!(
        json["hint"].as_str().unwrap().contains("goodbye"),
        "Hint should mention goodbye command"
    );
}

#[test]
fn test_bug_close_json_includes_goodbye_hint() {
    let temp = init_binnacle();

    // Create a bug
    let output = bn_in(&temp)
        .args(["bug", "create", "Bug with hint"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("Bug create should be JSON");
    let id = json["id"].as_str().unwrap();

    // Close the bug and check JSON output includes hint
    let close_output = bn_in(&temp)
        .args(["bug", "close", id, "--reason", "Fixed"])
        .output()
        .unwrap();

    let close_stdout = String::from_utf8_lossy(&close_output.stdout);
    let close_json: serde_json::Value =
        serde_json::from_str(&close_stdout).expect("Bug close output should be valid JSON");

    // Should have hint field reminding about goodbye
    assert!(
        close_json.get("hint").is_some(),
        "JSON should include hint field"
    );
    assert!(
        close_json["hint"].as_str().unwrap().contains("goodbye"),
        "Hint should mention goodbye command"
    );
}

// === Close with Incomplete Dependencies Tests ===

#[test]
fn test_task_close_fails_with_incomplete_deps() {
    let temp = init_binnacle();

    // Create task A (will be the dependency)
    let output_a = bn_in(&temp)
        .args(["task", "create", "Dependency task A"])
        .output()
        .unwrap();
    let id_a = extract_task_id(&output_a);

    // Create task B
    let output_b = bn_in(&temp)
        .args(["task", "create", "Dependent task B"])
        .output()
        .unwrap();
    let id_b = extract_task_id(&output_b);

    // B depends on A
    bn_in(&temp)
        .args([
            "link",
            "add",
            &id_b,
            &id_a,
            "-t",
            "depends_on",
            "--reason",
            "B needs A",
        ])
        .assert()
        .success();

    // Try to close B without force (A is still pending)
    bn_in(&temp)
        .args(["task", "close", &id_b])
        .assert()
        .failure()
        .stderr(predicate::str::contains("incomplete dependencies"))
        .stderr(predicate::str::contains(&id_a));
}

#[test]
fn test_task_close_with_force_succeeds_despite_incomplete_deps() {
    let temp = init_binnacle();

    // Create task A (will be the dependency)
    let output_a = bn_in(&temp)
        .args(["task", "create", "Dependency task A"])
        .output()
        .unwrap();
    let id_a = extract_task_id(&output_a);

    // Create task B
    let output_b = bn_in(&temp)
        .args(["task", "create", "Dependent task B"])
        .output()
        .unwrap();
    let id_b = extract_task_id(&output_b);

    // B depends on A
    bn_in(&temp)
        .args([
            "link",
            "add",
            &id_b,
            &id_a,
            "-t",
            "depends_on",
            "--reason",
            "B needs A",
        ])
        .assert()
        .success();

    // Close B with --force (A is still pending)
    bn_in(&temp)
        .args(["task", "close", &id_b, "--force"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"done\""))
        .stdout(predicate::str::contains("warning"))
        .stdout(predicate::str::contains("incomplete dependencies"));

    // Verify task is actually closed
    bn_in(&temp)
        .args(["task", "show", &id_b])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"done\""));
}

#[test]
fn test_task_close_succeeds_when_deps_are_complete() {
    let temp = init_binnacle();

    // Create task A (will be the dependency)
    let output_a = bn_in(&temp)
        .args(["task", "create", "Dependency task A"])
        .output()
        .unwrap();
    let id_a = extract_task_id(&output_a);

    // Create task B
    let output_b = bn_in(&temp)
        .args(["task", "create", "Dependent task B"])
        .output()
        .unwrap();
    let id_b = extract_task_id(&output_b);

    // B depends on A
    bn_in(&temp)
        .args([
            "link",
            "add",
            &id_b,
            &id_a,
            "-t",
            "depends_on",
            "--reason",
            "B needs A",
        ])
        .assert()
        .success();

    // Close A first
    bn_in(&temp)
        .args(["task", "close", &id_a, "--reason", "Done"])
        .assert()
        .success();

    // Now close B (A is complete, should succeed without --force)
    bn_in(&temp)
        .args(["task", "close", &id_b, "--reason", "Done"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"done\""));
}

#[test]
fn test_task_close_fails_with_multiple_incomplete_deps() {
    let temp = init_binnacle();

    // Create task A
    let output_a = bn_in(&temp)
        .args(["task", "create", "Dependency A"])
        .output()
        .unwrap();
    let id_a = extract_task_id(&output_a);

    // Create task B
    let output_b = bn_in(&temp)
        .args(["task", "create", "Dependency B"])
        .output()
        .unwrap();
    let id_b = extract_task_id(&output_b);

    // Create task C (depends on both A and B)
    let output_c = bn_in(&temp)
        .args(["task", "create", "Dependent task C"])
        .output()
        .unwrap();
    let id_c = extract_task_id(&output_c);

    // C depends on A and B
    bn_in(&temp)
        .args([
            "link",
            "add",
            &id_c,
            &id_a,
            "-t",
            "depends_on",
            "--reason",
            "C needs A",
        ])
        .assert()
        .success();
    bn_in(&temp)
        .args([
            "link",
            "add",
            &id_c,
            &id_b,
            "-t",
            "depends_on",
            "--reason",
            "C needs B",
        ])
        .assert()
        .success();

    // Try to close C without force
    let output = bn_in(&temp)
        .args(["task", "close", &id_c])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(stderr.contains("incomplete dependencies"));
    // Both A and B should be mentioned
    assert!(stderr.contains(&id_a));
    assert!(stderr.contains(&id_b));
}

#[test]
fn test_task_close_succeeds_when_dep_is_cancelled() {
    let temp = init_binnacle();

    // Create task A (will be cancelled)
    let output_a = bn_in(&temp)
        .args(["task", "create", "Dependency to cancel"])
        .output()
        .unwrap();
    let id_a = extract_task_id(&output_a);

    // Create task B
    let output_b = bn_in(&temp)
        .args(["task", "create", "Dependent task B"])
        .output()
        .unwrap();
    let id_b = extract_task_id(&output_b);

    // B depends on A
    bn_in(&temp)
        .args([
            "link",
            "add",
            &id_b,
            &id_a,
            "-t",
            "depends_on",
            "--reason",
            "B needs A",
        ])
        .assert()
        .success();

    // Cancel A (cancelled tasks are considered "complete" for dependency purposes)
    bn_in(&temp)
        .args(["task", "update", &id_a, "--status", "cancelled"])
        .assert()
        .success();

    // Close B (A is cancelled, should succeed without --force)
    bn_in(&temp)
        .args(["task", "close", &id_b, "--reason", "Done"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"done\""));
}

#[test]
fn test_task_close_human_readable_shows_incomplete_deps_error() {
    let temp = init_binnacle();

    // Create task A
    let output_a = bn_in(&temp)
        .args(["task", "create", "Blocker task"])
        .output()
        .unwrap();
    let id_a = extract_task_id(&output_a);

    // Create task B
    let output_b = bn_in(&temp)
        .args(["task", "create", "Blocked task"])
        .output()
        .unwrap();
    let id_b = extract_task_id(&output_b);

    // B depends on A
    bn_in(&temp)
        .args([
            "link",
            "add",
            &id_b,
            &id_a,
            "-t",
            "depends_on",
            "--reason",
            "B needs A",
        ])
        .assert()
        .success();

    // Try to close B with -H flag
    bn_in(&temp)
        .args(["task", "close", &id_b, "-H"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("incomplete dependencies"))
        .stderr(predicate::str::contains("--force"));
}

// === Complexity Check Tests ===

#[test]
fn test_task_create_with_check_complexity_simple_task() {
    let temp = init_binnacle();

    // Simple task should not trigger complexity detection
    bn_in(&temp)
        .args(["task", "create", "--check-complexity", "Fix typo in README"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"complexity_detected\":false"))
        .stdout(predicate::str::contains("\"task_created\":{"));
}

#[test]
fn test_task_create_with_check_complexity_complex_task() {
    let temp = init_binnacle();

    // Complex task should trigger soft-gate suggestion
    bn_in(&temp)
        .args([
            "task",
            "create",
            "--check-complexity",
            "Add authentication and fix database and improve logging",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"complexity_detected\":true"))
        .stdout(predicate::str::contains("\"suggestion\":"))
        .stdout(predicate::str::contains("\"proceed_command\":"))
        .stdout(predicate::str::contains("\"idea_command\":"))
        // Should NOT contain task_created since it was blocked
        .stdout(predicate::str::contains("\"task_created\":null").not());
}

#[test]
fn test_task_create_with_check_complexity_human_readable() {
    let temp = init_binnacle();

    // Complex task with human-readable output
    bn_in(&temp)
        .args([
            "-H",
            "task",
            "create",
            "--check-complexity",
            "Explore caching and investigate patterns",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("better as an **idea**"))
        .stdout(predicate::str::contains("What would you like to do?"))
        .stdout(predicate::str::contains("bn idea create"))
        .stdout(predicate::str::contains("--force"));
}

#[test]
fn test_task_create_with_force_bypasses_complexity() {
    let temp = init_binnacle();

    // Force flag should bypass complexity check and create task
    bn_in(&temp)
        .args([
            "task",
            "create",
            "--force",
            "Add authentication and fix database and improve logging",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"id\":\"bn-"))
        .stdout(predicate::str::contains("\"title\":"));
}

#[test]
fn test_task_create_check_complexity_with_all_options() {
    let temp = init_binnacle();

    // Complex task with all options - verify they appear in proceed_command
    bn_in(&temp)
        .args([
            "task",
            "create",
            "--check-complexity",
            "Explore caching options and investigate patterns",
            "-s",
            "explore cache",
            "-d",
            "Need to research",
            "-p",
            "1",
            "-t",
            "research",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"complexity_detected\":true"))
        .stdout(predicate::str::contains("-s \\\"explore cache\\\""))
        .stdout(predicate::str::contains("-d \\\"Need to research\\\""))
        .stdout(predicate::str::contains("-p 1"))
        .stdout(predicate::str::contains("-t research"));
}
