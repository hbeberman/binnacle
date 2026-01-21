//! Integration tests for Test Node operations via CLI.
//!
//! These tests verify that test commands work correctly through the CLI:
//! - `bn test create/list/show/link/unlink/run` all work
//! - Test execution and result capture
//! - Regression detection (auto-reopening closed tasks on test failure)

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

/// Helper function to extract ID from JSON output
fn extract_id(output: &std::process::Output) -> String {
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

// === Test Create Tests ===

#[test]
fn test_test_create_json() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["test", "create", "Unit tests", "--cmd", "echo hello"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"id\":\"bnt-"))
        .stdout(predicate::str::contains("\"name\":\"Unit tests\""));
}

#[test]
fn test_test_create_human() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["-H", "test", "create", "Unit tests", "--cmd", "echo hello"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created test bnt-"))
        .stdout(predicate::str::contains("\"Unit tests\""));
}

#[test]
fn test_test_create_with_working_dir() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args([
            "test",
            "create",
            "Subdir tests",
            "--cmd",
            "ls",
            "--dir",
            "subdir",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"id\":\"bnt-"));
}

#[test]
fn test_test_create_linked_to_task() {
    let temp = init_binnacle();

    // Create a task first
    let output = bn_in(&temp)
        .args(["task", "create", "My task"])
        .output()
        .unwrap();
    let task_id = extract_id(&output);

    // Create test linked to the task
    bn_in(&temp)
        .args([
            "test",
            "create",
            "Task tests",
            "--cmd",
            "echo test",
            "--task",
            &task_id,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"id\":\"bnt-"));

    // Verify the link exists
    bn_in(&temp)
        .args(["test", "list", "--task", &task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":1"));
}

#[test]
fn test_test_create_with_invalid_task() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args([
            "test",
            "create",
            "Invalid",
            "--cmd",
            "echo test",
            "--task",
            "bn-xxxx",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

// === Test List Tests ===

#[test]
fn test_test_list_empty() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["test", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":0"));
}

#[test]
fn test_test_list_with_tests() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["test", "create", "Test 1", "--cmd", "echo 1"])
        .assert()
        .success();
    bn_in(&temp)
        .args(["test", "create", "Test 2", "--cmd", "echo 2"])
        .assert()
        .success();

    bn_in(&temp)
        .args(["test", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":2"));
}

#[test]
fn test_test_list_human_readable() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["test", "create", "Unit tests", "--cmd", "echo test"])
        .assert()
        .success();

    bn_in(&temp)
        .args(["-H", "test", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 test(s)"))
        .stdout(predicate::str::contains("Unit tests"));
}

#[test]
fn test_test_list_filtered_by_task() {
    let temp = init_binnacle();

    // Create two tasks
    let output_a = bn_in(&temp)
        .args(["task", "create", "Task A"])
        .output()
        .unwrap();
    let task_a = extract_id(&output_a);

    let output_b = bn_in(&temp)
        .args(["task", "create", "Task B"])
        .output()
        .unwrap();
    let task_b = extract_id(&output_b);

    // Create tests linked to different tasks
    bn_in(&temp)
        .args([
            "test", "create", "Test A", "--cmd", "echo a", "--task", &task_a,
        ])
        .assert()
        .success();
    bn_in(&temp)
        .args([
            "test", "create", "Test B", "--cmd", "echo b", "--task", &task_b,
        ])
        .assert()
        .success();

    // Filter by task
    bn_in(&temp)
        .args(["test", "list", "--task", &task_a])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":1"))
        .stdout(predicate::str::contains("Test A"));
}

// === Test Show Tests ===

#[test]
fn test_test_show() {
    let temp = init_binnacle();

    let output = bn_in(&temp)
        .args(["test", "create", "My test", "--cmd", "echo hello"])
        .output()
        .unwrap();
    let test_id = extract_id(&output);

    bn_in(&temp)
        .args(["test", "show", &test_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\":\"My test\""))
        .stdout(predicate::str::contains("\"command\":\"echo hello\""));
}

#[test]
fn test_test_show_human_readable() {
    let temp = init_binnacle();

    let output = bn_in(&temp)
        .args(["test", "create", "My test", "--cmd", "echo hello"])
        .output()
        .unwrap();
    let test_id = extract_id(&output);

    bn_in(&temp)
        .args(["-H", "test", "show", &test_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("My test"))
        .stdout(predicate::str::contains("Command: echo hello"));
}

#[test]
fn test_test_show_not_found() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["test", "show", "bnt-xxxx"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

// === Test Link/Unlink Tests ===

#[test]
fn test_test_link() {
    let temp = init_binnacle();

    // Create task and test
    let task_output = bn_in(&temp)
        .args(["task", "create", "My task"])
        .output()
        .unwrap();
    let task_id = extract_id(&task_output);

    let test_output = bn_in(&temp)
        .args(["test", "create", "My test", "--cmd", "echo test"])
        .output()
        .unwrap();
    let test_id = extract_id(&test_output);

    // Link them
    bn_in(&temp)
        .args(["test", "link", &test_id, &task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"test_id\""))
        .stdout(predicate::str::contains("\"task_id\""));

    // Verify link in test show
    bn_in(&temp)
        .args(["test", "show", &test_id])
        .assert()
        .success()
        .stdout(predicate::str::contains(&task_id));
}

#[test]
fn test_test_link_human_readable() {
    let temp = init_binnacle();

    let task_output = bn_in(&temp)
        .args(["task", "create", "My task"])
        .output()
        .unwrap();
    let task_id = extract_id(&task_output);

    let test_output = bn_in(&temp)
        .args(["test", "create", "My test", "--cmd", "echo test"])
        .output()
        .unwrap();
    let test_id = extract_id(&test_output);

    bn_in(&temp)
        .args(["-H", "test", "link", &test_id, &task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("Linked test"))
        .stdout(predicate::str::contains("to task"));
}

#[test]
fn test_test_link_duplicate_fails() {
    let temp = init_binnacle();

    let task_output = bn_in(&temp)
        .args(["task", "create", "My task"])
        .output()
        .unwrap();
    let task_id = extract_id(&task_output);

    let test_output = bn_in(&temp)
        .args(["test", "create", "My test", "--cmd", "echo test"])
        .output()
        .unwrap();
    let test_id = extract_id(&test_output);

    // First link succeeds
    bn_in(&temp)
        .args(["test", "link", &test_id, &task_id])
        .assert()
        .success();

    // Second link fails
    bn_in(&temp)
        .args(["test", "link", &test_id, &task_id])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already linked"));
}

#[test]
fn test_test_unlink() {
    let temp = init_binnacle();

    let task_output = bn_in(&temp)
        .args(["task", "create", "My task"])
        .output()
        .unwrap();
    let task_id = extract_id(&task_output);

    let test_output = bn_in(&temp)
        .args([
            "test",
            "create",
            "My test",
            "--cmd",
            "echo test",
            "--task",
            &task_id,
        ])
        .output()
        .unwrap();
    let test_id = extract_id(&test_output);

    // Unlink
    bn_in(&temp)
        .args(["test", "unlink", &test_id, &task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"test_id\""))
        .stdout(predicate::str::contains("\"task_id\""));

    // Verify no longer linked
    let show_output = bn_in(&temp)
        .args(["test", "show", &test_id])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&show_output.stdout);
    assert!(!stdout.contains(&task_id));
}

#[test]
fn test_test_unlink_not_linked_fails() {
    let temp = init_binnacle();

    let task_output = bn_in(&temp)
        .args(["task", "create", "My task"])
        .output()
        .unwrap();
    let task_id = extract_id(&task_output);

    let test_output = bn_in(&temp)
        .args(["test", "create", "My test", "--cmd", "echo test"])
        .output()
        .unwrap();
    let test_id = extract_id(&test_output);

    // Unlink without linking first fails
    bn_in(&temp)
        .args(["test", "unlink", &test_id, &task_id])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not linked"));
}

// === Test Run Tests ===

#[test]
fn test_test_run_single_passing() {
    let temp = init_binnacle();

    let test_output = bn_in(&temp)
        .args(["test", "create", "Passing test", "--cmd", "true"])
        .output()
        .unwrap();
    let test_id = extract_id(&test_output);

    bn_in(&temp)
        .args(["test", "run", &test_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"passed\":true"))
        .stdout(predicate::str::contains("\"exit_code\":0"));
}

#[test]
fn test_test_run_single_failing() {
    let temp = init_binnacle();

    let test_output = bn_in(&temp)
        .args(["test", "create", "Failing test", "--cmd", "false"])
        .output()
        .unwrap();
    let test_id = extract_id(&test_output);

    bn_in(&temp)
        .args(["test", "run", &test_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"passed\":false"))
        .stdout(predicate::str::contains("\"failed\":1"));
}

#[test]
fn test_test_run_human_readable() {
    let temp = init_binnacle();

    let test_output = bn_in(&temp)
        .args(["test", "create", "My test", "--cmd", "echo hello"])
        .output()
        .unwrap();
    let test_id = extract_id(&test_output);

    bn_in(&temp)
        .args(["-H", "test", "run", &test_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("PASSED"))
        .stdout(predicate::str::contains("My test"));
}

#[test]
fn test_test_run_all() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["test", "create", "Test 1", "--cmd", "true"])
        .assert()
        .success();
    bn_in(&temp)
        .args(["test", "create", "Test 2", "--cmd", "true"])
        .assert()
        .success();

    bn_in(&temp)
        .args(["test", "run", "--all"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total\":2"))
        .stdout(predicate::str::contains("\"passed\":2"));
}

#[test]
fn test_test_run_by_task() {
    let temp = init_binnacle();

    // Create task
    let task_output = bn_in(&temp)
        .args(["task", "create", "My task"])
        .output()
        .unwrap();
    let task_id = extract_id(&task_output);

    // Create tests, one linked to task
    bn_in(&temp)
        .args([
            "test",
            "create",
            "Linked test",
            "--cmd",
            "true",
            "--task",
            &task_id,
        ])
        .assert()
        .success();
    bn_in(&temp)
        .args(["test", "create", "Unlinked test", "--cmd", "true"])
        .assert()
        .success();

    // Run by task should only run the linked test
    bn_in(&temp)
        .args(["test", "run", "--task", &task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total\":1"))
        .stdout(predicate::str::contains("Linked test"));
}

#[test]
fn test_test_run_no_option_fails() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["test", "run"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Specify --all"));
}

#[test]
fn test_test_run_captures_output() {
    let temp = init_binnacle();

    let test_output = bn_in(&temp)
        .args(["test", "create", "Echo test", "--cmd", "echo captured"])
        .output()
        .unwrap();
    let test_id = extract_id(&test_output);

    bn_in(&temp)
        .args(["test", "run", &test_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("captured"));
}

// === Regression Detection Tests ===

#[test]
fn test_regression_reopens_closed_task() {
    let temp = init_binnacle();

    // Create a task and close it
    let task_output = bn_in(&temp)
        .args(["task", "create", "Feature task"])
        .output()
        .unwrap();
    let task_id = extract_id(&task_output);

    bn_in(&temp)
        .args(["task", "close", &task_id])
        .assert()
        .success();

    // Verify it's closed
    bn_in(&temp)
        .args(["task", "show", &task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"done\""));

    // Create a failing test linked to the task
    let test_output = bn_in(&temp)
        .args([
            "test",
            "create",
            "Regression test",
            "--cmd",
            "false",
            "--task",
            &task_id,
        ])
        .output()
        .unwrap();
    let test_id = extract_id(&test_output);

    // Run the test - should reopen the task
    bn_in(&temp)
        .args(["test", "run", &test_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"reopened_tasks\""))
        .stdout(predicate::str::contains(&task_id));

    // Verify the task was reopened
    bn_in(&temp)
        .args(["task", "show", &task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"reopened\""));
}

#[test]
fn test_regression_human_readable() {
    let temp = init_binnacle();

    // Create and close a task
    let task_output = bn_in(&temp)
        .args(["task", "create", "Feature task"])
        .output()
        .unwrap();
    let task_id = extract_id(&task_output);

    bn_in(&temp)
        .args(["task", "close", &task_id])
        .assert()
        .success();

    // Create a failing test linked to the task
    let test_output = bn_in(&temp)
        .args([
            "test",
            "create",
            "Regression test",
            "--cmd",
            "false",
            "--task",
            &task_id,
        ])
        .output()
        .unwrap();
    let test_id = extract_id(&test_output);

    // Run the test with human-readable output
    bn_in(&temp)
        .args(["-H", "test", "run", &test_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("FAILED"))
        .stdout(predicate::str::contains("Regression detected"))
        .stdout(predicate::str::contains("Reopened tasks"));
}

#[test]
fn test_no_regression_when_task_not_closed() {
    let temp = init_binnacle();

    // Create a task but don't close it
    let task_output = bn_in(&temp)
        .args(["task", "create", "Open task"])
        .output()
        .unwrap();
    let task_id = extract_id(&task_output);

    // Create a failing test linked to the task
    let test_output = bn_in(&temp)
        .args([
            "test",
            "create",
            "Failing test",
            "--cmd",
            "false",
            "--task",
            &task_id,
        ])
        .output()
        .unwrap();
    let test_id = extract_id(&test_output);

    // Run the test - should NOT reopen (it's not closed)
    // reopened_tasks is omitted when empty, so just check the test failed
    let output = bn_in(&temp)
        .args(["test", "run", &test_id])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should not contain reopened_tasks with task_id
    assert!(!stdout.contains(&task_id) || !stdout.contains("reopened_tasks"));

    // Task should still be pending
    bn_in(&temp)
        .args(["task", "show", &task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"pending\""));
}

#[test]
fn test_passing_test_does_not_reopen_task() {
    let temp = init_binnacle();

    // Create and close a task
    let task_output = bn_in(&temp)
        .args(["task", "create", "Feature task"])
        .output()
        .unwrap();
    let task_id = extract_id(&task_output);

    bn_in(&temp)
        .args(["task", "close", &task_id])
        .assert()
        .success();

    // Create a passing test linked to the task
    let test_output = bn_in(&temp)
        .args([
            "test",
            "create",
            "Passing test",
            "--cmd",
            "true",
            "--task",
            &task_id,
        ])
        .output()
        .unwrap();
    let test_id = extract_id(&test_output);

    // Run the test - should NOT reopen
    bn_in(&temp)
        .args(["test", "run", &test_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"passed\":true"));

    // Task should still be done
    bn_in(&temp)
        .args(["task", "show", &task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"done\""));
}

// === Run Failed Tests ===

#[test]
fn test_test_run_failed_only() {
    let temp = init_binnacle();

    // Create passing and failing tests
    let pass_output = bn_in(&temp)
        .args(["test", "create", "Passing", "--cmd", "true"])
        .output()
        .unwrap();
    let pass_id = extract_id(&pass_output);

    let fail_output = bn_in(&temp)
        .args(["test", "create", "Failing", "--cmd", "false"])
        .output()
        .unwrap();
    let fail_id = extract_id(&fail_output);

    // Run all tests to record results
    bn_in(&temp)
        .args(["test", "run", "--all"])
        .assert()
        .success();

    // Now run --failed only
    bn_in(&temp)
        .args(["test", "run", "--failed"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total\":1"))
        .stdout(predicate::str::contains(&fail_id));

    // Should not contain the passing test
    let output = bn_in(&temp)
        .args(["test", "run", "--failed"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains(&pass_id));
}
