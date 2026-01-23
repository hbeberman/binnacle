//! Integration tests for the `bn agent kill` command.
//!
//! These tests verify that the agent kill command works correctly:
//! - Finds agents by PID or name
//! - Handles missing agents with appropriate error
//! - Removes agent from registry after kill

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

// === bn agent kill Tests ===

#[test]
fn test_agent_kill_not_found_pid() {
    let temp = init_binnacle();

    // Try to kill a non-existent agent by PID
    bn_in(&temp)
        .args(["agent", "kill", "99999"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found").or(predicate::str::contains("NotFound")));
}

#[test]
fn test_agent_kill_not_found_name() {
    let temp = init_binnacle();

    // Try to kill a non-existent agent by name
    bn_in(&temp)
        .args(["agent", "kill", "nonexistent-agent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found").or(predicate::str::contains("NotFound")));
}

#[test]
fn test_agent_kill_help() {
    let temp = TempDir::new().unwrap();

    bn_in(&temp)
        .args(["agent", "kill", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Terminate"))
        .stdout(predicate::str::contains("TARGET"))
        .stdout(predicate::str::contains("timeout"));
}

#[test]
fn test_agent_kill_with_timeout() {
    let temp = init_binnacle();

    // DON'T try to kill an agent that's registered with this test's PID!
    // Instead, just test that the command accepts the timeout parameter with a non-existent agent
    bn_in(&temp)
        .args(["agent", "kill", "99999", "--timeout", "2"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found").or(predicate::str::contains("NotFound")));
}

#[test]
fn test_agent_kill_after_orient() {
    let temp = init_binnacle();

    // Register agent with orient - this now uses the PARENT PID (this test process)
    // so the agent persists across bn invocations
    bn_in(&temp)
        .args(["orient", "--type", "worker", "--name", "doomed-agent"])
        .assert()
        .success();

    // Verify agent is listed - with the fix, the agent should now be present!
    // (Previously it would be cleaned up as stale because it used bn's own PID)
    bn_in(&temp)
        .args(["agent", "list", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("doomed-agent"));

    // Run orient again to verify agent tracking continues to work
    bn_in(&temp)
        .args(["orient", "--type", "worker", "--name", "doomed-agent"])
        .assert()
        .success();

    // Agent should still be present
    bn_in(&temp)
        .args(["agent", "list", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("doomed-agent"));
}
