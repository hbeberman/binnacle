//! Integration tests for Graph commands via CLI.
//!
//! These tests verify that graph analysis commands work correctly:
//! - `bn graph components` finds disconnected components
//! - Output formats (JSON and human-readable) work correctly

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

// === Graph Components Tests ===

#[test]
fn test_graph_components_empty() {
    let env = init_binnacle();

    bn_in(&env)
        .args(["graph", "components"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"component_count\":0"));
}

#[test]
fn test_graph_components_single_task() {
    let env = init_binnacle();

    // Create a single task
    bn_in(&env)
        .args(["task", "create", "Single task"])
        .assert()
        .success();

    // Should have one component with one task
    bn_in(&env)
        .args(["graph", "components"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"component_count\":1"))
        .stdout(predicate::str::contains("\"entity_count\":1"));
}

#[test]
fn test_graph_components_multiple_isolated() {
    let env = init_binnacle();

    // Create three isolated tasks
    bn_in(&env)
        .args(["task", "create", "Task 1"])
        .assert()
        .success();
    bn_in(&env)
        .args(["task", "create", "Task 2"])
        .assert()
        .success();
    bn_in(&env)
        .args(["task", "create", "Task 3"])
        .assert()
        .success();

    // Should have three disconnected components
    bn_in(&env)
        .args(["graph", "components"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"component_count\":3"));
}

#[test]
fn test_graph_components_connected_via_dependency() {
    let env = init_binnacle();

    // Create parent task
    let output = bn_in(&env)
        .args(["task", "create", "Parent task"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parent_id: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let parent_id = parent_id["id"].as_str().unwrap();

    // Create child task
    let output = bn_in(&env)
        .args(["task", "create", "Child task"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let child_id: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let child_id = child_id["id"].as_str().unwrap();

    // Link them
    bn_in(&env)
        .args([
            "link",
            "add",
            child_id,
            parent_id,
            "-t",
            "depends_on",
            "--reason",
            "test",
        ])
        .assert()
        .success();

    // Should have one component with two tasks
    bn_in(&env)
        .args(["graph", "components"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"component_count\":1"))
        .stdout(predicate::str::contains("\"entity_count\":2"));
}

#[test]
fn test_graph_components_human_readable() {
    let env = init_binnacle();

    // Create a task
    bn_in(&env)
        .args(["task", "create", "Test task"])
        .assert()
        .success();

    bn_in(&env)
        .args(["graph", "components", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Task Graph:"))
        .stdout(predicate::str::contains("Component 1"));
}

#[test]
fn test_graph_components_with_bugs() {
    let env = init_binnacle();

    // Create a task and a bug
    bn_in(&env)
        .args(["task", "create", "A task"])
        .assert()
        .success();
    bn_in(&env)
        .args(["bug", "create", "A bug"])
        .assert()
        .success();

    // Both should be counted as separate components
    bn_in(&env)
        .args(["graph", "components"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"component_count\":2"));
}

#[test]
fn test_graph_components_shows_root_nodes() {
    let env = init_binnacle();

    // Create a task
    let output = bn_in(&env)
        .args(["task", "create", "Root task"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let task: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let task_id = task["id"].as_str().unwrap();

    bn_in(&env)
        .args(["graph", "components"])
        .assert()
        .success()
        .stdout(predicate::str::contains("root_nodes"))
        .stdout(predicate::str::contains(task_id));
}
