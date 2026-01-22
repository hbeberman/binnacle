//! Integration tests for Task CRUD operations via CLI.
//!
//! These tests verify that task commands work correctly through the CLI:
//! - `bn system init` creates directory structure
//! - `bn task create/list/show/update/close/reopen/delete` all work
//! - JSON and human-readable output formats are correct
//! - Filtering by status, priority, and tags works

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
    bn_in(&temp).args(["system", "init"]).assert().success();
    temp
}

// === Init Tests ===

#[test]
fn test_init_creates_storage() {
    let temp = TempDir::new().unwrap();

    bn_in(&temp)
        .args(["system", "init"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"initialized\":true"));
}

#[test]
fn test_init_human_readable() {
    let temp = TempDir::new().unwrap();

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
        .args(["link", "add", &id_b, &id_a, "--type", "depends_on"])
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
        .args(["-H", "link", "add", &id_b, &id_a, "--type", "depends_on"])
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
        .args(["link", "add", &id_a, &id_b, "--type", "depends_on"])
        .assert()
        .success();

    // B depends on A should fail (cycle)
    bn_in(&temp)
        .args(["link", "add", &id_b, &id_a, "--type", "depends_on"])
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
        .args(["link", "add", &id_b, &id_a, "--type", "depends_on"])
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
        .args(["link", "add", &id_b, &id_a, "--type", "depends_on"])
        .assert()
        .success();
    bn_in(&temp)
        .args(["link", "add", &id_c, &id_b, "--type", "depends_on"])
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
        .args(["link", "add", &id_b, &id_a, "--type", "depends_on"])
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
        .args(["link", "add", &id_b, &id_a, "--type", "depends_on"])
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
        .args(["link", "add", &id_b, &id_a, "--type", "depends_on"])
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
        .args(["link", "add", &id_b, &id_a, "--type", "depends_on"])
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
        .args(["link", "add", &id_b, &id_a, "--type", "depends_on"])
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
        .args(["link", "add", &id_b, &id_a, "--type", "depends_on"])
        .assert()
        .success();
    bn_in(&temp)
        .args(["link", "add", &id_c, &id_b, "--type", "depends_on"])
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
        .args(["link", "add", &id_b, &id_a, "--type", "depends_on"])
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
        .args(["link", "add", &id_b, &id_a, "--type", "depends_on"])
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
        .args(["link", "add", &id_b, &id_a, "--type", "depends_on"])
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
        .args(["link", "add", &id_b, &id_a, "--type", "depends_on"])
        .assert()
        .success();
    bn_in(&temp)
        .args(["link", "add", &id_c, &id_b, "--type", "depends_on"])
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
        .args(["link", "add", &id_c, &id_a, "--type", "depends_on"])
        .assert()
        .success();
    bn_in(&temp)
        .args(["link", "add", &id_c, &id_b, "--type", "depends_on"])
        .assert()
        .success();

    // Show C with both blockers
    bn_in(&temp)
        .args(["task", "show", &id_c, "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Blocked by 2 incomplete dependencies"))
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
        .args(["link", "add", &id_b, &id_a, "--type", "depends_on"])
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
        .args(["link", "add", &id_d, &id_a, "--type", "depends_on"])
        .assert()
        .success();
    bn_in(&temp)
        .args(["link", "add", &id_d, &id_b, "--type", "depends_on"])
        .assert()
        .success();
    bn_in(&temp)
        .args(["link", "add", &id_d, &id_c, "--type", "depends_on"])
        .assert()
        .success();

    // Show D, should show only A and B as blockers (not C which is done)
    let output = bn_in(&temp)
        .args(["task", "show", &id_d])
        .output()
        .unwrap();
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
        .stdout(predicate::str::contains("\"short_name\":\"VeryLongShortNameThatExceeds"));
}

#[test]
fn test_task_short_name_special_characters() {
    let temp = init_binnacle();

    // Short names with special characters should work
    bn_in(&temp)
        .args(["task", "create", "Task with special chars", "-s", "My-Task_1"])
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
