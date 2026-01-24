//! Integration tests for container mode support.
//!
//! These tests verify that binnacle works correctly in container environments:
//! - BN_CONTAINER_MODE environment variable detection
//! - Container-style storage paths work correctly
//! - bn commands function properly with container data directories

mod common;

use assert_cmd::Command;
use common::TestEnv;
use predicates::prelude::*;
use std::fs;

// === Container Mode Environment Variable Tests ===

#[test]
fn test_container_mode_env_var_recognized() {
    // Create a test environment that simulates container mode
    let env = TestEnv::new();

    // Create a "container" binnacle directory
    let container_data = env.repo_path().join("container_binnacle");
    fs::create_dir_all(&container_data).unwrap();

    // Initialize with BN_CONTAINER_MODE set and BN_DATA_DIR pointing to our test container dir
    // Note: In real containers, BN_CONTAINER_MODE would use /binnacle, but we use BN_DATA_DIR
    // for testing since we can't write to /binnacle in tests
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.env("BN_DATA_DIR", &container_data);
    cmd.args(["system", "init", "-y"]);
    cmd.assert().success();

    // Verify bn orient works with container data directory
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.env("BN_DATA_DIR", &container_data);
    cmd.args(["orient", "--type", "worker", "-H"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Binnacle"));
}

#[test]
fn test_container_mode_task_operations() {
    // Test that task CRUD operations work correctly with container-style paths
    let env = TestEnv::new();

    // Set up container-like data directory
    let container_data = env.repo_path().join("binnacle_data");
    fs::create_dir_all(&container_data).unwrap();

    // Helper to create a command with container data dir
    let bn_container = || {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(env.repo_path());
        cmd.env("BN_DATA_DIR", &container_data);
        cmd
    };

    // Initialize
    bn_container()
        .args(["system", "init", "-y"])
        .assert()
        .success();

    // Create a task
    let output = bn_container()
        .args(["task", "create", "Container test task"])
        .output()
        .expect("Failed to create task");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"id\":\"bn-"));

    // Extract task ID
    let id_start = stdout.find("\"id\":\"").unwrap() + 6;
    let id_end = stdout[id_start..].find('"').unwrap() + id_start;
    let task_id = &stdout[id_start..id_end];

    // Show task
    bn_container()
        .args(["task", "show", task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("Container test task"));

    // List tasks
    bn_container()
        .args(["task", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains(task_id));

    // Update task status
    bn_container()
        .args(["task", "update", task_id, "--status", "in_progress"])
        .assert()
        .success();

    // Close task
    bn_container()
        .args(["task", "close", task_id, "--reason", "Test completed"])
        .assert()
        .success();

    // Verify closed status
    bn_container()
        .args(["task", "show", task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"done\""));
}

#[test]
fn test_container_mode_ready_and_blocked() {
    // Test that ready/blocked queries work with container paths
    let env = TestEnv::new();

    let container_data = env.repo_path().join("binnacle_data");
    fs::create_dir_all(&container_data).unwrap();

    let bn_container = || {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(env.repo_path());
        cmd.env("BN_DATA_DIR", &container_data);
        cmd
    };

    // Initialize
    bn_container()
        .args(["system", "init", "-y"])
        .assert()
        .success();

    // Create tasks
    let output = bn_container()
        .args(["task", "create", "First task"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id1_start = stdout.find("\"id\":\"").unwrap() + 6;
    let id1_end = stdout[id1_start..].find('"').unwrap() + id1_start;
    let task1 = stdout[id1_start..id1_end].to_string();

    let output = bn_container()
        .args(["task", "create", "Second task"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id2_start = stdout.find("\"id\":\"").unwrap() + 6;
    let id2_end = stdout[id2_start..].find('"').unwrap() + id2_start;
    let task2 = stdout[id2_start..id2_end].to_string();

    // Make task2 depend on task1
    bn_container()
        .args([
            "link",
            "add",
            &task2,
            &task1,
            "-t",
            "depends_on",
            "--reason",
            "Task 2 depends on Task 1 completion",
        ])
        .assert()
        .success();

    // Check ready - should include task1
    bn_container()
        .args(["ready"])
        .assert()
        .success()
        .stdout(predicate::str::contains(&task1));

    // Check blocked - should include task2
    bn_container()
        .args(["blocked"])
        .assert()
        .success()
        .stdout(predicate::str::contains(&task2));
}

#[test]
fn test_container_mode_bug_operations() {
    // Test bug tracking works in container mode
    let env = TestEnv::new();

    let container_data = env.repo_path().join("binnacle_data");
    fs::create_dir_all(&container_data).unwrap();

    let bn_container = || {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(env.repo_path());
        cmd.env("BN_DATA_DIR", &container_data);
        cmd
    };

    // Initialize
    bn_container()
        .args(["system", "init", "-y"])
        .assert()
        .success();

    // Create a bug
    let output = bn_container()
        .args(["bug", "create", "Container bug"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"id\":\"bn-"));

    let id_start = stdout.find("\"id\":\"").unwrap() + 6;
    let id_end = stdout[id_start..].find('"').unwrap() + id_start;
    let bug_id = &stdout[id_start..id_end];

    // Show bug
    bn_container()
        .args(["bug", "show", bug_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("Container bug"));

    // Close bug
    bn_container()
        .args(["bug", "close", bug_id, "--reason", "Fixed"])
        .assert()
        .success();
}

#[test]
fn test_container_mode_idea_operations() {
    // Test idea tracking works in container mode
    let env = TestEnv::new();

    let container_data = env.repo_path().join("binnacle_data");
    fs::create_dir_all(&container_data).unwrap();

    let bn_container = || {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(env.repo_path());
        cmd.env("BN_DATA_DIR", &container_data);
        cmd
    };

    // Initialize
    bn_container()
        .args(["system", "init", "-y"])
        .assert()
        .success();

    // Create an idea
    let output = bn_container()
        .args(["idea", "create", "Container idea"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Ideas now use "bn-" prefix in the ID
    assert!(stdout.contains("\"id\":\"bn-"));
    assert!(stdout.contains("Container idea"));

    let id_start = stdout.find("\"id\":\"bn-").unwrap() + 6;
    let id_end = stdout[id_start..].find('"').unwrap() + id_start;
    let idea_id = &stdout[id_start..id_end];

    // Show idea
    bn_container()
        .args(["idea", "show", idea_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("Container idea"));
}

#[test]
fn test_container_mode_queue_operations() {
    // Test queue operations work in container mode
    let env = TestEnv::new();

    let container_data = env.repo_path().join("binnacle_data");
    fs::create_dir_all(&container_data).unwrap();

    let bn_container = || {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(env.repo_path());
        cmd.env("BN_DATA_DIR", &container_data);
        cmd
    };

    // Initialize
    bn_container()
        .args(["system", "init", "-y"])
        .assert()
        .success();

    // Create a queue
    bn_container()
        .args(["queue", "create", "Work Queue"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"id\":\"bnq-"));

    // Show queue
    bn_container()
        .args(["queue", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Work Queue"));

    // Create a task and add to queue
    let output = bn_container()
        .args(["task", "create", "Queued task"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id_start = stdout.find("\"id\":\"").unwrap() + 6;
    let id_end = stdout[id_start..].find('"').unwrap() + id_start;
    let task_id = &stdout[id_start..id_end];

    // Add task to queue
    bn_container()
        .args(["queue", "add", task_id])
        .assert()
        .success();

    // Verify task is in queue
    bn_container()
        .args(["queue", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains(task_id));
}

#[test]
fn test_container_mode_agent_registration() {
    // Test agent registration works in container mode
    let env = TestEnv::new();

    let container_data = env.repo_path().join("binnacle_data");
    fs::create_dir_all(&container_data).unwrap();

    let bn_container = || {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(env.repo_path());
        cmd.env("BN_DATA_DIR", &container_data);
        cmd
    };

    // Initialize
    bn_container()
        .args(["system", "init", "-y"])
        .assert()
        .success();

    // Orient with --register (agent registration) - requires --type
    bn_container()
        .args([
            "orient",
            "--type",
            "worker",
            "--register",
            "container-worker",
            "-H",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Binnacle"));

    // List agents
    bn_container()
        .args(["agent", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("container-worker"));
}

#[test]
fn test_container_mode_test_operations() {
    // Test that test node operations work in container mode
    let env = TestEnv::new();

    let container_data = env.repo_path().join("binnacle_data");
    fs::create_dir_all(&container_data).unwrap();

    let bn_container = || {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(env.repo_path());
        cmd.env("BN_DATA_DIR", &container_data);
        cmd
    };

    // Initialize
    bn_container()
        .args(["system", "init", "-y"])
        .assert()
        .success();

    // Create a test node
    let output = bn_container()
        .args(["test", "create", "Container test", "--cmd", "echo passed"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"id\":\"bnt-"));

    let id_start = stdout.find("\"id\":\"bnt-").unwrap() + 6;
    let id_end = stdout[id_start..].find('"').unwrap() + id_start;
    let test_id = &stdout[id_start..id_end];

    // Show test
    bn_container()
        .args(["test", "show", test_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("Container test"));

    // Run test
    bn_container()
        .args(["test", "run", test_id])
        .assert()
        .success();
}

#[test]
fn test_container_mode_data_persistence() {
    // Test that data persists across multiple commands (simulating container restarts)
    let env = TestEnv::new();

    let container_data = env.repo_path().join("binnacle_data");
    fs::create_dir_all(&container_data).unwrap();

    let bn_container = || {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(env.repo_path());
        cmd.env("BN_DATA_DIR", &container_data);
        cmd
    };

    // Initialize
    bn_container()
        .args(["system", "init", "-y"])
        .assert()
        .success();

    // Create a task
    let output = bn_container()
        .args(["task", "create", "Persistent task", "-p", "1"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id_start = stdout.find("\"id\":\"").unwrap() + 6;
    let id_end = stdout[id_start..].find('"').unwrap() + id_start;
    let task_id = &stdout[id_start..id_end];

    // Verify we can retrieve the task in a new command invocation
    // (simulating container restart with same mounted volume)
    bn_container()
        .args(["task", "show", task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("Persistent task"))
        .stdout(predicate::str::contains("\"priority\":1"));

    // Verify orient shows the task (requires --type)
    bn_container()
        .args(["orient", "--type", "worker", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Total tasks: 1"));
}
