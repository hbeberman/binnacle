//! Integration tests for Phase 5 Maintenance Commands.
//!
//! These tests verify that maintenance commands work correctly through the CLI:
//! - `bn doctor` - health check and issue detection
//! - `bn log` - audit trail of changes
//! - `bn config get/set/list` - configuration management

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

/// Create a task and return its ID.
fn create_task(env: &TestEnv, title: &str) -> String {
    let output = bn_in(env)
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

/// Create a queue to make the repo fully healthy
fn create_queue(env: &TestEnv) {
    bn_in(env)
        .args(["queue", "create", "Work Queue"])
        .assert()
        .success();
}

// === Doctor Tests ===

#[test]
fn test_doctor_healthy_json() {
    let temp = init_binnacle();

    // Create a queue to make repo fully healthy
    create_queue(&temp);

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

    // Create a queue to make repo fully healthy
    create_queue(&temp);

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

// === Doctor Fix Tests ===

#[test]
fn test_doctor_fix_creates_queue() {
    let temp = init_binnacle();

    // Without a queue, doctor should report an issue
    bn_in(&temp)
        .args(["-H", "doctor"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No primary queue exists"));

    // Run doctor --fix to create the queue
    bn_in(&temp)
        .args(["-H", "doctor", "--fix"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created primary queue"));

    // Now doctor should report healthy
    bn_in(&temp)
        .args(["-H", "doctor"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Health check: OK"));
}

// === Doctor Edge Migration Tests ===

#[test]
fn test_doctor_migrate_edges_dry_run() {
    let temp = init_binnacle();

    // Create two tasks
    let task1 = create_task(&temp, "Task 1");
    let task2 = create_task(&temp, "Task 2");

    // Add a link the proper way (to create baseline)
    bn_in(&temp)
        .args([
            "link",
            "add",
            &task1,
            &task2,
            "--type",
            "depends_on",
            "--reason",
            "test dep",
        ])
        .assert()
        .success();

    // Run migration in dry-run mode
    bn_in(&temp)
        .args(["doctor", "--migrate-edges", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"dry_run\":true"))
        .stdout(predicate::str::contains("\"tasks_scanned\":2"));
}

#[test]
fn test_doctor_migrate_edges_human_output() {
    let temp = init_binnacle();

    create_task(&temp, "Task 1");

    bn_in(&temp)
        .args(["-H", "doctor", "--migrate-edges", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Edge Migration (DRY RUN"))
        .stdout(predicate::str::contains("Tasks scanned:"));
}

#[test]
fn test_doctor_migrate_edges_json_output() {
    let temp = init_binnacle();

    create_task(&temp, "Task 1");

    bn_in(&temp)
        .args(["doctor", "--migrate-edges"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"tasks_scanned\":1"))
        .stdout(predicate::str::contains("\"edges_created\":0"))
        .stdout(predicate::str::contains("\"dry_run\":false"));
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

// === Not Initialized Tests ===

#[test]
fn test_doctor_not_initialized() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .arg("doctor")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not initialized"));
}

#[test]
fn test_log_not_initialized() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .arg("log")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not initialized"));
}

#[test]
fn test_config_not_initialized() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["config", "list"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not initialized"));
}
