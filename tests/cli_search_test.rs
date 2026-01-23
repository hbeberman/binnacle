//! Integration tests for Search commands via CLI.
//!
//! These tests verify that search commands work correctly through the CLI:
//! - `bn search link` queries edges with filters

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

// === Search Link Tests ===

#[test]
fn test_search_link_empty() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["search", "link"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":0"));
}

#[test]
fn test_search_link_empty_human() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["search", "link", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No edges found."));
}

#[test]
fn test_search_link_by_type() {
    let temp = init_binnacle();

    // Create two tasks
    let output = bn_in(&temp)
        .args(["task", "create", "Task 1"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let task1_id = json["id"].as_str().unwrap();

    let output = bn_in(&temp)
        .args(["task", "create", "Task 2"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let task2_id = json["id"].as_str().unwrap();

    // Create a depends_on edge
    bn_in(&temp)
        .args([
            "link",
            "add",
            task1_id,
            task2_id,
            "--type",
            "depends_on",
            "--reason",
            "Test reason",
        ])
        .assert()
        .success();

    // Search by type
    bn_in(&temp)
        .args(["search", "link", "--type", "depends_on"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":1"))
        .stdout(predicate::str::contains("\"edge_type\":\"depends_on\""))
        .stdout(predicate::str::contains("Test reason"));

    // Search by different type - should be empty
    bn_in(&temp)
        .args(["search", "link", "--type", "fixes"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":0"));
}

#[test]
fn test_search_link_by_source() {
    let temp = init_binnacle();

    // Create tasks
    let output = bn_in(&temp)
        .args(["task", "create", "Task 1"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let task1_id = json["id"].as_str().unwrap();

    let output = bn_in(&temp)
        .args(["task", "create", "Task 2"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let task2_id = json["id"].as_str().unwrap();

    let output = bn_in(&temp)
        .args(["task", "create", "Task 3"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let task3_id = json["id"].as_str().unwrap();

    // Create edges: task1 -> task2, task3 -> task2
    bn_in(&temp)
        .args(["link", "add", task1_id, task2_id, "--type", "depends_on"])
        .assert()
        .success();
    bn_in(&temp)
        .args(["link", "add", task3_id, task2_id, "--type", "depends_on"])
        .assert()
        .success();

    // Search by source=task1
    bn_in(&temp)
        .args(["search", "link", "--source", task1_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":1"))
        .stdout(predicate::str::contains(task1_id));
}

#[test]
fn test_search_link_by_target() {
    let temp = init_binnacle();

    // Create tasks
    let output = bn_in(&temp)
        .args(["task", "create", "Task 1"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let task1_id = json["id"].as_str().unwrap();

    let output = bn_in(&temp)
        .args(["task", "create", "Task 2"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let task2_id = json["id"].as_str().unwrap();

    // Create edge
    bn_in(&temp)
        .args(["link", "add", task1_id, task2_id, "--type", "depends_on"])
        .assert()
        .success();

    // Search by target=task2
    bn_in(&temp)
        .args(["search", "link", "--target", task2_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":1"))
        .stdout(predicate::str::contains(task2_id));
}

#[test]
fn test_search_link_combined_filters() {
    let temp = init_binnacle();

    // Create tasks
    let output = bn_in(&temp)
        .args(["task", "create", "Task 1"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let task1_id = json["id"].as_str().unwrap();

    let output = bn_in(&temp)
        .args(["task", "create", "Task 2"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let task2_id = json["id"].as_str().unwrap();

    // Create multiple edges
    bn_in(&temp)
        .args(["link", "add", task1_id, task2_id, "--type", "depends_on"])
        .assert()
        .success();
    bn_in(&temp)
        .args(["link", "add", task1_id, task2_id, "--type", "related_to"])
        .assert()
        .success();

    // Search with combined filters
    bn_in(&temp)
        .args([
            "search",
            "link",
            "--type",
            "depends_on",
            "--source",
            task1_id,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":1"))
        .stdout(predicate::str::contains("\"edge_type\":\"depends_on\""));

    // Search with all filters
    bn_in(&temp)
        .args([
            "search",
            "link",
            "--type",
            "related_to",
            "--source",
            task1_id,
            "--target",
            task2_id,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":1"))
        .stdout(predicate::str::contains("\"edge_type\":\"related_to\""));
}

#[test]
fn test_search_link_human_output() {
    let temp = init_binnacle();

    // Create tasks
    let output = bn_in(&temp)
        .args(["task", "create", "Task 1"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let task1_id = json["id"].as_str().unwrap();

    let output = bn_in(&temp)
        .args(["task", "create", "Task 2"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let task2_id = json["id"].as_str().unwrap();

    // Create edge with reason
    bn_in(&temp)
        .args([
            "link",
            "add",
            task1_id,
            task2_id,
            "--type",
            "depends_on",
            "--reason",
            "Important dependency",
        ])
        .assert()
        .success();

    // Search with human output
    bn_in(&temp)
        .args(["search", "link", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 edge(s) found"))
        .stdout(predicate::str::contains(task1_id))
        .stdout(predicate::str::contains("→"))
        .stdout(predicate::str::contains(task2_id))
        .stdout(predicate::str::contains("depends_on"))
        .stdout(predicate::str::contains("Important dependency"));
}

#[test]
fn test_search_link_help() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["search", "link", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--type"))
        .stdout(predicate::str::contains("--source"))
        .stdout(predicate::str::contains("--target"))
        .stdout(predicate::str::contains("depends_on"))
        .stdout(predicate::str::contains("fixes"));
}
