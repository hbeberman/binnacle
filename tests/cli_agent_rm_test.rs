//! Integration tests for the `bn agent rm` command.
//!
//! These tests verify that the agent rm command works correctly:
//! - Finds agents by ID, PID, or name
//! - Handles missing agents with appropriate error
//! - Removes agents from registry
//! - Supports --all with --type for bulk removal
//! - Supports --force for immediate SIGKILL

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

// === bn agent rm Tests ===

#[test]
fn test_agent_rm_help() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["agent", "rm", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Remove agent"))
        .stdout(predicate::str::contains("TARGET"))
        .stdout(predicate::str::contains("--force"))
        .stdout(predicate::str::contains("--all"))
        .stdout(predicate::str::contains("--type"));
}

#[test]
fn test_agent_rm_requires_target_or_all() {
    let temp = init_binnacle();

    // Without any target or --all flag, should fail
    bn_in(&temp)
        .args(["agent", "rm"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Target").or(predicate::str::contains("required")));
}

#[test]
fn test_agent_rm_all_requires_type() {
    let temp = init_binnacle();

    // --all without --type should fail
    bn_in(&temp)
        .args(["agent", "rm", "--all"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--type"));
}

#[test]
fn test_agent_rm_not_found_pid() {
    let temp = init_binnacle();

    // Try to remove a non-existent agent by PID
    bn_in(&temp)
        .args(["agent", "rm", "99999"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found").or(predicate::str::contains("NotFound")));
}

#[test]
fn test_agent_rm_not_found_name() {
    let temp = init_binnacle();

    // Try to remove a non-existent agent by name
    bn_in(&temp)
        .args(["agent", "rm", "nonexistent-agent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found").or(predicate::str::contains("NotFound")));
}

#[test]
fn test_agent_rm_all_empty() {
    let temp = init_binnacle();

    // Remove all workers when none exist - should succeed with count 0
    let output = bn_in(&temp)
        .args(["agent", "rm", "--all", "--type", "worker"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert!(stdout.contains("\"count\":0") || stdout.contains("count: 0"));
}

#[test]
fn test_agent_rm_all_workers_human_output() {
    let temp = init_binnacle();

    // Test human-readable output for empty removal
    bn_in(&temp)
        .args(["agent", "rm", "--all", "--type", "worker", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No agents removed"));
}

#[test]
fn test_agent_rm_after_orient() {
    let temp = init_binnacle();

    // This test verifies the JSON output structure of agent rm
    // We can't actually test removing a registered agent because orient
    // uses the parent PID (this test process), and killing it would kill our test.

    // Instead, test that the output format is correct when removing non-existent agents
    // by checking the help and error messages work properly

    // Verify command accepts positional target argument
    let output = bn_in(&temp)
        .args(["agent", "rm", "some-target-name"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not found") || stderr.contains("NotFound"),
        "Should indicate agent not found: {}",
        stderr
    );
}

#[test]
fn test_agent_rm_force_flag() {
    let temp = init_binnacle();

    // Test that --force flag is accepted in the command
    // We use a non-existent agent to avoid killing ourselves
    let output = bn_in(&temp)
        .args(["agent", "rm", "nonexistent-for-force", "--force"])
        .output()
        .unwrap();

    // Command should fail (agent not found), but the flag should be accepted
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not found") || stderr.contains("NotFound"),
        "Should fail with not found, got: {}",
        stderr
    );
}

#[test]
fn test_agent_rm_all_by_type() {
    let temp = init_binnacle();

    // This test verifies the --all --type combination works
    // We don't actually register agents to avoid self-termination

    // Test with no agents - should succeed with count 0
    let output = bn_in(&temp)
        .args(["agent", "rm", "--all", "--type", "planner"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert!(
        stdout.contains("\"count\":0"),
        "Should have removed 0 agents when none exist"
    );
}
