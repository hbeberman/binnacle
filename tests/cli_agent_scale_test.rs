//! Integration tests for the `bn agent scale` command.
//!
//! These tests verify that the agent scale command works correctly:
//! - Shows all agent scaling configs when no type specified
//! - Shows specific agent type config
//! - Sets min/max for a specific type
//! - Validates that min <= max
//! - Errors when trying to set without specifying type

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

// === bn agent scale Tests ===

#[test]
fn test_agent_scale_help() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["agent", "scale", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("View or set min/max"))
        .stdout(predicate::str::contains("AGENT_TYPE"))
        .stdout(predicate::str::contains("--min"))
        .stdout(predicate::str::contains("--max"));
}

#[test]
fn test_agent_scale_show_all_defaults() {
    let temp = init_binnacle();

    // Show all scaling configs (should return defaults)
    bn_in(&temp)
        .args(["agent", "scale"])
        .assert()
        .success()
        .stdout(predicate::str::contains("worker"))
        .stdout(predicate::str::contains("planner"))
        .stdout(predicate::str::contains("buddy"));
}

#[test]
fn test_agent_scale_show_all_human() {
    let temp = init_binnacle();

    // Show all scaling configs in human format
    bn_in(&temp)
        .args(["agent", "scale", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Agent Scaling Configuration:"))
        .stdout(predicate::str::contains("worker:"))
        .stdout(predicate::str::contains("planner:"))
        .stdout(predicate::str::contains("buddy:"));
}

#[test]
fn test_agent_scale_show_specific_type() {
    let temp = init_binnacle();

    // Show worker scaling config
    bn_in(&temp)
        .args(["agent", "scale", "worker"])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""agent_type":"worker""#))
        .stdout(predicate::str::contains(r#""min":"#))
        .stdout(predicate::str::contains(r#""max":"#));
}

#[test]
fn test_agent_scale_show_specific_type_human() {
    let temp = init_binnacle();

    // Show worker scaling config in human format
    bn_in(&temp)
        .args(["agent", "scale", "worker", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Agent Scaling (worker):"))
        .stdout(predicate::str::contains("min:"))
        .stdout(predicate::str::contains("max:"));
}

#[test]
fn test_agent_scale_set_min_max() {
    let temp = init_binnacle();

    // Set min and max for worker
    bn_in(&temp)
        .args(["agent", "scale", "worker", "--min", "1", "--max", "5"])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""min":1"#))
        .stdout(predicate::str::contains(r#""max":5"#));

    // Verify the change persisted
    bn_in(&temp)
        .args(["agent", "scale", "worker"])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""min":1"#))
        .stdout(predicate::str::contains(r#""max":5"#));
}

#[test]
fn test_agent_scale_set_min_only() {
    let temp = init_binnacle();

    // First set max high enough to allow min=2
    bn_in(&temp)
        .args(["agent", "scale", "planner", "--max", "5"])
        .assert()
        .success();

    // Set only min for planner
    bn_in(&temp)
        .args(["agent", "scale", "planner", "--min", "2"])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""min":2"#));

    // Verify change persisted and max is still 5
    bn_in(&temp)
        .args(["agent", "scale", "planner"])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""min":2"#))
        .stdout(predicate::str::contains(r#""max":5"#));
}

#[test]
fn test_agent_scale_set_max_only() {
    let temp = init_binnacle();

    // Set only max for buddy
    bn_in(&temp)
        .args(["agent", "scale", "buddy", "--max", "10"])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""max":10"#));

    // Verify change persisted
    bn_in(&temp)
        .args(["agent", "scale", "buddy"])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""max":10"#));
}

#[test]
fn test_agent_scale_min_greater_than_max_fails() {
    let temp = init_binnacle();

    // Try to set min > max (should fail)
    bn_in(&temp)
        .args(["agent", "scale", "worker", "--min", "10", "--max", "5"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("min").and(predicate::str::contains("max")));
}

#[test]
fn test_agent_scale_without_type_fails_when_setting() {
    let temp = init_binnacle();

    // Try to set min without specifying agent type (should fail)
    bn_in(&temp)
        .args(["agent", "scale", "--min", "1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Agent type required"));
}

#[test]
fn test_agent_scale_invalid_type() {
    let temp = init_binnacle();

    // Try to use an invalid agent type (clap validation should catch this)
    bn_in(&temp)
        .args(["agent", "scale", "invalid_type"])
        .assert()
        .failure();
}

#[test]
fn test_agent_scale_all_types() {
    let temp = init_binnacle();

    // Set scaling for all types
    bn_in(&temp)
        .args(["agent", "scale", "worker", "--min", "1", "--max", "3"])
        .assert()
        .success();

    bn_in(&temp)
        .args(["agent", "scale", "planner", "--min", "0", "--max", "2"])
        .assert()
        .success();

    bn_in(&temp)
        .args(["agent", "scale", "buddy", "--min", "0", "--max", "1"])
        .assert()
        .success();

    // Verify all changes persisted
    bn_in(&temp)
        .args(["agent", "scale", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("worker:  min=1, max=3"))
        .stdout(predicate::str::contains("planner: min=0, max=2"))
        .stdout(predicate::str::contains("buddy:   min=0, max=1"));
}
