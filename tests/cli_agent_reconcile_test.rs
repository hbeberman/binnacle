//! Integration tests for the `bn agent reconcile` command.
//!
//! These tests verify that the agent reconciliation logic works correctly:
//! - Shows correct current vs desired agent counts
//! - Respects scaling configuration
//! - Work-aware scaling for workers (scale to 0 when no work)
//! - Dry-run mode doesn't make changes

mod common;

use assert_cmd::Command;
use common::TestEnv;
use predicates::prelude::*;

/// Get a Command for the bn binary in a TestEnv.
fn bn_in(env: &TestEnv) -> Command {
    env.bn()
}

/// Initialize binnacle and return the TestEnv.
fn init_binnacle() -> TestEnv {
    TestEnv::init()
}

// === bn agent reconcile Tests ===

#[test]
fn test_agent_reconcile_help() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["agent", "reconcile", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("reconciliation"))
        .stdout(predicate::str::contains("--dry-run"));
}

#[test]
fn test_agent_reconcile_no_work_no_agents() {
    let temp = init_binnacle();

    // With no tasks and default scaling (min=0 for all types), reconcile should do nothing
    bn_in(&temp)
        .args(["agent", "reconcile", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("dry_run"))
        .stdout(predicate::str::contains("work_count"));
}

#[test]
fn test_agent_reconcile_human_output() {
    let temp = init_binnacle();

    // Human-readable output
    bn_in(&temp)
        .args(["agent", "reconcile", "--dry-run", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Reconciliation (dry run):"))
        .stdout(predicate::str::contains("Work available:"))
        .stdout(predicate::str::contains("Workers:"))
        .stdout(predicate::str::contains("current"))
        .stdout(predicate::str::contains("desired"));
}

#[test]
fn test_agent_reconcile_work_aware_scaling_no_work() {
    let temp = init_binnacle();

    // Set worker min=1, but with no tasks, desired should still be 0
    bn_in(&temp)
        .args(["agent", "scale", "worker", "--min", "1", "--max", "3"])
        .assert()
        .success();

    // Reconcile should show desired=0 because no work is available
    bn_in(&temp)
        .args(["agent", "reconcile", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""work_count":0"#));
}

#[test]
fn test_agent_reconcile_work_aware_scaling_with_tasks() {
    let temp = init_binnacle();

    // Set worker scaling
    bn_in(&temp)
        .args(["agent", "scale", "worker", "--min", "1", "--max", "3"])
        .assert()
        .success();

    // Create a task to generate work
    bn_in(&temp)
        .args(["task", "create", "Test task"])
        .assert()
        .success();

    // Reconcile should show work_count >= 1 and desired worker count >= 1
    // Note: We use dry-run because we can't actually spawn containers in tests
    let output = bn_in(&temp)
        .args(["agent", "reconcile", "--dry-run"])
        .assert()
        .success();

    // Check that work is recognized
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.contains(r#""work_count":1"#) || stdout.contains(r#""work_count": 1"#),
        "Expected work_count of 1, got: {}",
        stdout
    );
}

#[test]
fn test_agent_reconcile_non_worker_maintains_min() {
    let temp = init_binnacle();

    // Set planner min=1 (planners are not work-aware)
    bn_in(&temp)
        .args(["agent", "scale", "planner", "--min", "1", "--max", "1"])
        .assert()
        .success();

    // Reconcile should want to spawn a planner even without work
    bn_in(&temp)
        .args(["agent", "reconcile", "--dry-run", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Planners: 0 current, 1 desired"))
        .stdout(predicate::str::contains("spawn"))
        .stdout(predicate::str::contains("planner"));
}

#[test]
fn test_agent_reconcile_reports_actions() {
    let temp = init_binnacle();

    // Set buddy min=1
    bn_in(&temp)
        .args(["agent", "scale", "buddy", "--min", "1", "--max", "2"])
        .assert()
        .success();

    // Reconcile should report a spawn action
    bn_in(&temp)
        .args(["agent", "reconcile", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""action":"spawn""#))
        .stdout(predicate::str::contains(r#""agent_type":"buddy""#))
        .stdout(predicate::str::contains(r#""executed":false"#)); // dry-run doesn't execute
}

#[test]
fn test_agent_reconcile_success_flag() {
    let temp = init_binnacle();

    // Should return success=true
    bn_in(&temp)
        .args(["agent", "reconcile", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""success":true"#));
}

#[test]
fn test_agent_reconcile_multiple_work_items() {
    let temp = init_binnacle();

    // Set worker scaling to allow multiple workers
    bn_in(&temp)
        .args(["agent", "scale", "worker", "--min", "1", "--max", "5"])
        .assert()
        .success();

    // Create multiple tasks
    for i in 1..=3 {
        bn_in(&temp)
            .args(["task", "create", &format!("Task {}", i)])
            .assert()
            .success();
    }

    // Reconcile should recognize multiple work items
    let output = bn_in(&temp)
        .args(["agent", "reconcile", "--dry-run"])
        .assert()
        .success();

    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    // work_count should be 3
    assert!(
        stdout.contains(r#""work_count":3"#) || stdout.contains(r#""work_count": 3"#),
        "Expected work_count of 3, got: {}",
        stdout
    );
}

#[test]
fn test_agent_reconcile_with_bugs_as_work() {
    let temp = init_binnacle();

    // Set worker scaling
    bn_in(&temp)
        .args(["agent", "scale", "worker", "--min", "1", "--max", "3"])
        .assert()
        .success();

    // Create a bug (bugs count as work for workers)
    bn_in(&temp)
        .args(["bug", "create", "Test bug"])
        .assert()
        .success();

    // Reconcile should recognize the bug as work
    let output = bn_in(&temp)
        .args(["agent", "reconcile", "--dry-run"])
        .assert()
        .success();

    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.contains(r#""work_count":1"#) || stdout.contains(r#""work_count": 1"#),
        "Expected work_count of 1 (from bug), got: {}",
        stdout
    );
}
