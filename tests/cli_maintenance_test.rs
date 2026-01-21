//! Integration tests for Phase 5 Maintenance Commands.
//!
//! These tests verify that maintenance commands work correctly through the CLI:
//! - `bn doctor` - health check and issue detection
//! - `bn log` - audit trail of changes
//! - `bn config get/set/list` - configuration management
//! - `bn compact` - storage compaction

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

/// Create a task and return its ID.
fn create_task(dir: &TempDir, title: &str) -> String {
    let output = bn_in(dir)
        .args(["task", "create", title])
        .output()
        .expect("Failed to run bn task create");

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Extract ID from JSON output like {"id":"bn-xxxx","title":"..."}
    let id_start = stdout.find("\"id\":\"").expect("No id in output") + 6;
    let id_end = stdout[id_start..]
        .find('"')
        .expect("No closing quote for id")
        + id_start;
    stdout[id_start..id_end].to_string()
}

// === Doctor Tests ===

#[test]
fn test_doctor_healthy_json() {
    let temp = init_binnacle();

    // Create a task to have some data
    create_task(&temp, "Test task");

    bn_in(&temp)
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"healthy\":true"))
        .stdout(predicate::str::contains("\"total_tasks\":1"));
}

#[test]
fn test_doctor_healthy_human() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["-H", "doctor"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Health check: OK"))
        .stdout(predicate::str::contains("Statistics:"));
}

#[test]
fn test_doctor_stats() {
    let temp = init_binnacle();

    // Create some tasks and tests
    create_task(&temp, "Task 1");
    create_task(&temp, "Task 2");

    bn_in(&temp)
        .args(["test", "create", "Test node", "--cmd", "echo test"])
        .assert()
        .success();

    bn_in(&temp)
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total_tasks\":2"))
        .stdout(predicate::str::contains("\"total_tests\":1"));
}

#[test]
fn test_doctor_human_stats() {
    let temp = init_binnacle();

    create_task(&temp, "Task 1");

    bn_in(&temp)
        .args(["-H", "doctor"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Tasks: 1"))
        .stdout(predicate::str::contains("Storage:"));
}

// === Log Tests ===

#[test]
fn test_log_empty() {
    let temp = init_binnacle();

    bn_in(&temp)
        .arg("log")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"entries\":[]"));
}

#[test]
fn test_log_shows_created_tasks() {
    let temp = init_binnacle();

    let task_id = create_task(&temp, "My logged task");

    bn_in(&temp)
        .arg("log")
        .assert()
        .success()
        .stdout(predicate::str::contains(&task_id))
        .stdout(predicate::str::contains("created"));
}

#[test]
fn test_log_human_format() {
    let temp = init_binnacle();

    create_task(&temp, "Test task");

    bn_in(&temp)
        .args(["-H", "log"])
        .assert()
        .success()
        .stdout(predicate::str::contains("log entries"))
        .stdout(predicate::str::contains("[task]"))
        .stdout(predicate::str::contains("created"));
}

#[test]
fn test_log_filter_by_task() {
    let temp = init_binnacle();

    let task_a = create_task(&temp, "Task A");
    let _task_b = create_task(&temp, "Task B");

    bn_in(&temp)
        .args(["log", &task_a])
        .assert()
        .success()
        .stdout(predicate::str::contains(&task_a))
        .stdout(predicate::str::contains("\"filtered_by\""));
}

#[test]
fn test_log_shows_updates() {
    let temp = init_binnacle();

    let task_id = create_task(&temp, "Original title");

    // Update the task
    bn_in(&temp)
        .args(["task", "update", &task_id, "--title", "Updated title"])
        .assert()
        .success();

    bn_in(&temp)
        .args(["log", &task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("created"))
        .stdout(predicate::str::contains("updated"));
}

#[test]
fn test_log_shows_closed() {
    let temp = init_binnacle();

    let task_id = create_task(&temp, "Task to close");

    // Close the task
    bn_in(&temp)
        .args(["task", "close", &task_id])
        .assert()
        .success();

    bn_in(&temp)
        .args(["log", &task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("closed"));
}

// === Config Tests ===

#[test]
fn test_config_set_and_get() {
    let temp = init_binnacle();

    // Set a config value
    bn_in(&temp)
        .args(["config", "set", "test.key", "test_value"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"key\":\"test.key\""))
        .stdout(predicate::str::contains("\"value\":\"test_value\""));

    // Get the config value
    bn_in(&temp)
        .args(["config", "get", "test.key"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"value\":\"test_value\""));
}

#[test]
fn test_config_get_nonexistent() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["config", "get", "nonexistent.key"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"value\":null"));
}

#[test]
fn test_config_human_format() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["-H", "config", "set", "my.setting", "hello"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Set my.setting = hello"));

    bn_in(&temp)
        .args(["-H", "config", "get", "my.setting"])
        .assert()
        .success()
        .stdout(predicate::str::contains("my.setting = hello"));
}

#[test]
fn test_config_get_nonexistent_human() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["-H", "config", "get", "nonexistent"])
        .assert()
        .success()
        .stdout(predicate::str::contains("nonexistent is not set"));
}

#[test]
fn test_config_list_empty() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["config", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":0"));
}

#[test]
fn test_config_list() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["config", "set", "key1", "value1"])
        .assert()
        .success();
    bn_in(&temp)
        .args(["config", "set", "key2", "value2"])
        .assert()
        .success();

    bn_in(&temp)
        .args(["config", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":2"))
        .stdout(predicate::str::contains("key1"))
        .stdout(predicate::str::contains("key2"));
}

#[test]
fn test_config_list_human() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["config", "set", "alpha", "1"])
        .assert()
        .success();
    bn_in(&temp)
        .args(["config", "set", "beta", "2"])
        .assert()
        .success();

    bn_in(&temp)
        .args(["-H", "config", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("2 configuration value(s)"))
        .stdout(predicate::str::contains("alpha = 1"))
        .stdout(predicate::str::contains("beta = 2"));
}

#[test]
fn test_config_list_empty_human() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["-H", "config", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No configuration values set"));
}

#[test]
fn test_config_overwrite() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["config", "set", "key", "old_value"])
        .assert()
        .success();
    bn_in(&temp)
        .args(["config", "set", "key", "new_value"])
        .assert()
        .success();

    bn_in(&temp)
        .args(["config", "get", "key"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"value\":\"new_value\""));
}

// === Compact Tests ===

#[test]
fn test_compact_basic() {
    let temp = init_binnacle();

    // Create a task and update it multiple times
    let task_id = create_task(&temp, "Original");

    bn_in(&temp)
        .args(["task", "update", &task_id, "--title", "Update 1"])
        .assert()
        .success();
    bn_in(&temp)
        .args(["task", "update", &task_id, "--title", "Update 2"])
        .assert()
        .success();

    // Run compact
    bn_in(&temp)
        .arg("compact")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"tasks_compacted\":1"))
        .stdout(predicate::str::contains("\"final_entries\":1"));
}

#[test]
fn test_compact_human_format() {
    let temp = init_binnacle();

    create_task(&temp, "Task 1");
    create_task(&temp, "Task 2");

    bn_in(&temp)
        .args(["-H", "compact"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Compact complete"))
        .stdout(predicate::str::contains("Tasks compacted: 2"));
}

#[test]
fn test_compact_preserves_data() {
    let temp = init_binnacle();

    // Create multiple tasks
    let task1 = create_task(&temp, "Task 1");
    let task2 = create_task(&temp, "Task 2");

    // Update one task
    bn_in(&temp)
        .args(["task", "update", &task1, "--title", "Task 1 Updated"])
        .assert()
        .success();

    // Compact
    bn_in(&temp).arg("compact").assert().success();

    // Verify both tasks still exist with correct data
    bn_in(&temp)
        .args(["task", "show", &task1])
        .assert()
        .success()
        .stdout(predicate::str::contains("Task 1 Updated"));

    bn_in(&temp)
        .args(["task", "show", &task2])
        .assert()
        .success()
        .stdout(predicate::str::contains("Task 2"));
}

#[test]
fn test_compact_with_tests() {
    let temp = init_binnacle();

    // Create a task and a test
    create_task(&temp, "Task 1");

    bn_in(&temp)
        .args(["test", "create", "Test 1", "--cmd", "echo test"])
        .assert()
        .success();

    // Compact
    bn_in(&temp)
        .arg("compact")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"final_entries\":2")); // 1 task + 1 test

    // Verify both still exist
    bn_in(&temp)
        .args(["task", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":1"));

    bn_in(&temp)
        .args(["test", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":1"));
}

// === Not Initialized Tests ===

#[test]
fn test_doctor_not_initialized() {
    let temp = TempDir::new().unwrap();

    bn_in(&temp)
        .arg("doctor")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not initialized"));
}

#[test]
fn test_log_not_initialized() {
    let temp = TempDir::new().unwrap();

    bn_in(&temp)
        .arg("log")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not initialized"));
}

#[test]
fn test_config_not_initialized() {
    let temp = TempDir::new().unwrap();

    bn_in(&temp)
        .args(["config", "list"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not initialized"));
}

#[test]
fn test_compact_not_initialized() {
    let temp = TempDir::new().unwrap();

    bn_in(&temp)
        .arg("compact")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not initialized"));
}
