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

// === Graph Lineage Tests ===

#[test]
fn test_graph_lineage_single_task_no_parents() {
    let env = init_binnacle();

    // Create a single task with no parents
    let output = bn_in(&env)
        .args(["task", "create", "Isolated task"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let task: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let task_id = task["id"].as_str().unwrap();

    // Lineage should only contain the task itself
    bn_in(&env)
        .args(["graph", "lineage", task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains(task_id))
        .stdout(predicate::str::contains("\"hops\""));
}

#[test]
fn test_graph_lineage_simple_parent_child() {
    let env = init_binnacle();

    // Create parent
    let output = bn_in(&env)
        .args(["task", "create", "Parent task"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parent: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let parent_id = parent["id"].as_str().unwrap();

    // Create child
    let output = bn_in(&env)
        .args(["task", "create", "Child task"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let child: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let child_id = child["id"].as_str().unwrap();

    // Link child to parent
    bn_in(&env)
        .args([
            "link", "add", child_id, parent_id, "-t", "child_of", "--reason", "test",
        ])
        .assert()
        .success();

    // Lineage should show child -> parent
    let output = bn_in(&env)
        .args(["graph", "lineage", child_id])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(child_id));
    assert!(stdout.contains(parent_id));
}

#[test]
fn test_graph_lineage_multi_level_ancestry() {
    let env = init_binnacle();

    // Create grandparent
    let output = bn_in(&env)
        .args(["task", "create", "Grandparent"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let grandparent: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let grandparent_id = grandparent["id"].as_str().unwrap();

    // Create parent
    let output = bn_in(&env)
        .args(["task", "create", "Parent"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parent: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let parent_id = parent["id"].as_str().unwrap();

    // Create child
    let output = bn_in(&env)
        .args(["task", "create", "Child"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let child: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let child_id = child["id"].as_str().unwrap();

    // Link parent to grandparent
    bn_in(&env)
        .args([
            "link",
            "add",
            parent_id,
            grandparent_id,
            "-t",
            "child_of",
            "--reason",
            "test",
        ])
        .assert()
        .success();

    // Link child to parent
    bn_in(&env)
        .args([
            "link", "add", child_id, parent_id, "-t", "child_of", "--reason", "test",
        ])
        .assert()
        .success();

    // Lineage should show full chain: child -> parent -> grandparent
    let output = bn_in(&env)
        .args(["graph", "lineage", child_id])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(child_id));
    assert!(stdout.contains(parent_id));
    assert!(stdout.contains(grandparent_id));
}

#[test]
fn test_graph_lineage_depth_limit() {
    let env = init_binnacle();

    // Create a 3-level chain
    let output = bn_in(&env)
        .args(["task", "create", "Level 3"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let l3: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let l3_id = l3["id"].as_str().unwrap();

    let output = bn_in(&env)
        .args(["task", "create", "Level 2"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let l2: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let l2_id = l2["id"].as_str().unwrap();

    let output = bn_in(&env)
        .args(["task", "create", "Level 1"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let l1: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let l1_id = l1["id"].as_str().unwrap();

    bn_in(&env)
        .args([
            "link", "add", l2_id, l3_id, "-t", "child_of", "--reason", "test",
        ])
        .assert()
        .success();
    bn_in(&env)
        .args([
            "link", "add", l1_id, l2_id, "-t", "child_of", "--reason", "test",
        ])
        .assert()
        .success();

    // Request only 2 levels deep - should not see l3
    let output = bn_in(&env)
        .args(["graph", "lineage", l1_id, "--depth", "2"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(l1_id));
    assert!(stdout.contains(l2_id));
}

#[test]
fn test_graph_lineage_human_readable() {
    let env = init_binnacle();

    let output = bn_in(&env)
        .args(["task", "create", "Test task"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let task: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let task_id = task["id"].as_str().unwrap();

    bn_in(&env)
        .args(["graph", "lineage", task_id, "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Lineage"));
}

// === Graph Peers Tests ===

#[test]
fn test_graph_peers_no_siblings() {
    let env = init_binnacle();

    // Create a task with no siblings
    let output = bn_in(&env)
        .args(["task", "create", "Only child"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let task: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let task_id = task["id"].as_str().unwrap();

    bn_in(&env)
        .args(["graph", "peers", task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"peers\""));
}

#[test]
fn test_graph_peers_siblings_via_shared_parent() {
    let env = init_binnacle();

    // Create parent
    let output = bn_in(&env)
        .args(["task", "create", "Parent"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parent: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let parent_id = parent["id"].as_str().unwrap();

    // Create two children
    let output = bn_in(&env)
        .args(["task", "create", "Child 1"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let child1: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let child1_id = child1["id"].as_str().unwrap();

    let output = bn_in(&env)
        .args(["task", "create", "Child 2"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let child2: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let child2_id = child2["id"].as_str().unwrap();

    // Link both children to same parent
    bn_in(&env)
        .args([
            "link", "add", child1_id, parent_id, "-t", "child_of", "--reason", "test",
        ])
        .assert()
        .success();
    bn_in(&env)
        .args([
            "link", "add", child2_id, parent_id, "-t", "child_of", "--reason", "test",
        ])
        .assert()
        .success();

    // Child1's peers should include Child2
    let output = bn_in(&env)
        .args(["graph", "peers", child1_id])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(child2_id));
}

#[test]
fn test_graph_peers_exclude_closed_by_default() {
    let env = init_binnacle();

    // Create parent
    let output = bn_in(&env)
        .args(["task", "create", "Parent"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parent: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let parent_id = parent["id"].as_str().unwrap();

    // Create two children
    let output = bn_in(&env)
        .args(["task", "create", "Open child"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let open_child: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let open_id = open_child["id"].as_str().unwrap();

    let output = bn_in(&env)
        .args(["task", "create", "Closed child"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let closed_child: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let closed_id = closed_child["id"].as_str().unwrap();

    // Link both to parent
    bn_in(&env)
        .args([
            "link", "add", open_id, parent_id, "-t", "child_of", "--reason", "test",
        ])
        .assert()
        .success();
    bn_in(&env)
        .args([
            "link", "add", closed_id, parent_id, "-t", "child_of", "--reason", "test",
        ])
        .assert()
        .success();

    // Close one child
    bn_in(&env)
        .args(["task", "close", closed_id, "--reason", "test"])
        .assert()
        .success();

    // Peers should not include closed task by default
    let output = bn_in(&env)
        .args(["graph", "peers", open_id])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains(closed_id));
}

#[test]
fn test_graph_peers_include_closed_flag() {
    let env = init_binnacle();

    // Create parent
    let output = bn_in(&env)
        .args(["task", "create", "Parent"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parent: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let parent_id = parent["id"].as_str().unwrap();

    // Create two children
    let output = bn_in(&env)
        .args(["task", "create", "Open child"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let open_child: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let open_id = open_child["id"].as_str().unwrap();

    let output = bn_in(&env)
        .args(["task", "create", "Closed child"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let closed_child: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let closed_id = closed_child["id"].as_str().unwrap();

    // Link both to parent
    bn_in(&env)
        .args([
            "link", "add", open_id, parent_id, "-t", "child_of", "--reason", "test",
        ])
        .assert()
        .success();
    bn_in(&env)
        .args([
            "link", "add", closed_id, parent_id, "-t", "child_of", "--reason", "test",
        ])
        .assert()
        .success();

    // Close one child
    bn_in(&env)
        .args(["task", "close", closed_id, "--reason", "test"])
        .assert()
        .success();

    // With --include-closed, should see both
    let output = bn_in(&env)
        .args(["graph", "peers", open_id, "--include-closed"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(closed_id));
}

#[test]
fn test_graph_peers_human_readable() {
    let env = init_binnacle();

    let output = bn_in(&env)
        .args(["task", "create", "Test task"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let task: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let task_id = task["id"].as_str().unwrap();

    bn_in(&env)
        .args(["graph", "peers", task_id, "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Peers"));
}

// === Graph Descendants Tests ===

#[test]
fn test_graph_descendants_no_children() {
    let env = init_binnacle();

    // Create a task with no children
    let output = bn_in(&env)
        .args(["task", "create", "Leaf task"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let task: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let task_id = task["id"].as_str().unwrap();

    bn_in(&env)
        .args(["graph", "descendants", task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"descendants\""));
}

#[test]
fn test_graph_descendants_single_level() {
    let env = init_binnacle();

    // Create parent
    let output = bn_in(&env)
        .args(["task", "create", "Parent"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parent: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let parent_id = parent["id"].as_str().unwrap();

    // Create child
    let output = bn_in(&env)
        .args(["task", "create", "Child"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let child: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let child_id = child["id"].as_str().unwrap();

    // Link child to parent
    bn_in(&env)
        .args([
            "link", "add", child_id, parent_id, "-t", "child_of", "--reason", "test",
        ])
        .assert()
        .success();

    // Descendants of parent should include child
    let output = bn_in(&env)
        .args(["graph", "descendants", parent_id])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(child_id));
}

#[test]
fn test_graph_descendants_multi_level() {
    let env = init_binnacle();

    // Create 3-level tree
    let output = bn_in(&env)
        .args(["task", "create", "Grandparent"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let gp: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let gp_id = gp["id"].as_str().unwrap();

    let output = bn_in(&env)
        .args(["task", "create", "Parent"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let p: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let p_id = p["id"].as_str().unwrap();

    let output = bn_in(&env)
        .args(["task", "create", "Child"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let c: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let c_id = c["id"].as_str().unwrap();

    bn_in(&env)
        .args([
            "link", "add", p_id, gp_id, "-t", "child_of", "--reason", "test",
        ])
        .assert()
        .success();
    bn_in(&env)
        .args([
            "link", "add", c_id, p_id, "-t", "child_of", "--reason", "test",
        ])
        .assert()
        .success();

    // Descendants of grandparent should include both parent and child
    let output = bn_in(&env)
        .args(["graph", "descendants", gp_id])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(p_id));
    assert!(stdout.contains(c_id));
}

#[test]
fn test_graph_descendants_depth_limit() {
    let env = init_binnacle();

    // Create 3-level tree
    let output = bn_in(&env)
        .args(["task", "create", "Level 1"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let l1: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let l1_id = l1["id"].as_str().unwrap();

    let output = bn_in(&env)
        .args(["task", "create", "Level 2"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let l2: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let l2_id = l2["id"].as_str().unwrap();

    let output = bn_in(&env)
        .args(["task", "create", "Level 3"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let l3: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let l3_id = l3["id"].as_str().unwrap();

    bn_in(&env)
        .args([
            "link", "add", l2_id, l1_id, "-t", "child_of", "--reason", "test",
        ])
        .assert()
        .success();
    bn_in(&env)
        .args([
            "link", "add", l3_id, l2_id, "-t", "child_of", "--reason", "test",
        ])
        .assert()
        .success();

    // With depth=1, should only see l2, not l3
    let output = bn_in(&env)
        .args(["graph", "descendants", l1_id, "--depth", "1"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(l2_id));
    assert!(!stdout.contains(l3_id));
}

#[test]
fn test_graph_descendants_exclude_closed_by_default() {
    let env = init_binnacle();

    // Create parent and two children
    let output = bn_in(&env)
        .args(["task", "create", "Parent"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parent: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let parent_id = parent["id"].as_str().unwrap();

    let output = bn_in(&env)
        .args(["task", "create", "Open child"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let open: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let open_id = open["id"].as_str().unwrap();

    let output = bn_in(&env)
        .args(["task", "create", "Closed child"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let closed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let closed_id = closed["id"].as_str().unwrap();

    bn_in(&env)
        .args([
            "link", "add", open_id, parent_id, "-t", "child_of", "--reason", "test",
        ])
        .assert()
        .success();
    bn_in(&env)
        .args([
            "link", "add", closed_id, parent_id, "-t", "child_of", "--reason", "test",
        ])
        .assert()
        .success();

    // Close one child
    bn_in(&env)
        .args(["task", "close", closed_id, "--reason", "test"])
        .assert()
        .success();

    // Descendants should not include closed by default
    let output = bn_in(&env)
        .args(["graph", "descendants", parent_id])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(open_id));
    assert!(!stdout.contains(closed_id));
}

#[test]
fn test_graph_descendants_include_closed_flag() {
    let env = init_binnacle();

    // Create parent and two children
    let output = bn_in(&env)
        .args(["task", "create", "Parent"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parent: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let parent_id = parent["id"].as_str().unwrap();

    let output = bn_in(&env)
        .args(["task", "create", "Closed child"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let closed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let closed_id = closed["id"].as_str().unwrap();

    bn_in(&env)
        .args([
            "link", "add", closed_id, parent_id, "-t", "child_of", "--reason", "test",
        ])
        .assert()
        .success();

    // Close the child
    bn_in(&env)
        .args(["task", "close", closed_id, "--reason", "test"])
        .assert()
        .success();

    // With --include-closed, should see closed task
    let output = bn_in(&env)
        .args(["graph", "descendants", parent_id, "--include-closed"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(closed_id));
}

#[test]
fn test_graph_descendants_human_readable() {
    let env = init_binnacle();

    let output = bn_in(&env)
        .args(["task", "create", "Test task"])
        .output()
        .expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let task: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let task_id = task["id"].as_str().unwrap();

    bn_in(&env)
        .args(["graph", "descendants", task_id, "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Descendants"));
}
