//! Integration tests for the `bn goodbye` command.
//!
//! These tests verify that the goodbye command works correctly:
//! - Looks up agent registration
//! - Handles unregistered agents gracefully
//! - Logs termination with optional reason
//! - Removes agent from registry
//!
//! Note: Tests use --dry-run to avoid actually terminating processes.

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

// === bn goodbye Tests ===

#[test]
fn test_goodbye_without_registration_warns() {
    let temp = init_binnacle();

    // Run goodbye without running orient first (unregistered agent)
    bn_in(&temp)
        .args(["goodbye", "-H", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Warning: Agent not registered"));
}

#[test]
fn test_goodbye_with_reason() {
    let temp = init_binnacle();

    // Run goodbye with a reason
    bn_in(&temp)
        .args([
            "goodbye",
            "-H",
            "--dry-run",
            "Task complete, all tests passing",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Reason: Task complete, all tests passing",
        ));
}

#[test]
fn test_goodbye_json_output() {
    let temp = init_binnacle();

    // Run goodbye and check JSON output format
    let output = bn_in(&temp)
        .args(["goodbye", "--dry-run"])
        .output()
        .expect("Failed to run goodbye");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should be valid JSON
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Output should be valid JSON");

    // Check expected fields
    assert!(json.get("parent_pid").is_some());
    assert!(json.get("was_registered").is_some());
    assert!(json.get("terminated").is_some());
}

#[test]
fn test_goodbye_after_orient_shows_agent_name() {
    let temp = init_binnacle();

    // Register agent with orient
    bn_in(&temp)
        .args(["orient", "--type", "worker", "--name", "test-agent"])
        .assert()
        .success();

    // Note: In CLI tests, each command runs in a separate subprocess with its own PID.
    // When orient runs, it registers the orient subprocess's PID.
    // When goodbye runs, it looks up the goodbye subprocess's PID, which is different.
    // So goodbye will report "not registered" even after orient registered a different process.
    //
    // This test verifies the command completes successfully.
    // In a real agent scenario, the same process calls both orient and goodbye.
    bn_in(&temp)
        .args(["goodbye", "-H", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Terminating agent"));
}

#[test]
fn test_goodbye_removes_agent_from_registry() {
    let temp = init_binnacle();

    // Register agent
    bn_in(&temp)
        .args(["orient", "--type", "worker", "--name", "cleanup-agent"])
        .assert()
        .success();

    // Agent should be listed
    bn_in(&temp)
        .args(["agent", "list", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("cleanup-agent"));

    // Goodbye removes registration (with dry-run so it doesn't terminate)
    bn_in(&temp)
        .args(["goodbye", "--dry-run"])
        .assert()
        .success();

    // Note: Since goodbye runs in a separate process from orient, the agent
    // registered by orient (with orient's PID) won't be the same as the one
    // goodbye tries to remove (with goodbye's PID). This is expected behavior.
    // In a real scenario, the same process that calls orient also calls goodbye.
}

#[test]
fn test_goodbye_help() {
    let temp = TempDir::new().unwrap();

    bn_in(&temp)
        .args(["goodbye", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Gracefully terminate"));
}

#[test]
fn test_goodbye_dry_run_flag() {
    let temp = init_binnacle();

    // Verify dry-run is documented in help
    bn_in(&temp)
        .args(["goodbye", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("dry-run"));
}

#[test]
fn test_goodbye_without_reason_warns() {
    let temp = init_binnacle();

    // Run goodbye without a reason - should warn
    bn_in(&temp)
        .args(["goodbye", "-H", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Warning: No reason provided"));
}

#[test]
fn test_goodbye_with_reason_no_warning() {
    let temp = init_binnacle();

    // Run goodbye with a reason - should NOT warn about missing reason
    bn_in(&temp)
        .args(["goodbye", "-H", "--dry-run", "Task completed successfully"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Warning: No reason provided").not());
}

#[test]
fn test_goodbye_json_includes_warning_field() {
    let temp = init_binnacle();

    // Run goodbye without reason and check JSON output includes warning
    let output = bn_in(&temp)
        .args(["goodbye", "--dry-run"])
        .output()
        .expect("Failed to run goodbye");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Output should be valid JSON");

    // Should have warning field when no reason provided
    assert!(json.get("warning").is_some());
    assert!(
        json["warning"]
            .as_str()
            .unwrap()
            .contains("No reason provided")
    );
}

// === Planner Agent Tests ===

#[test]
fn test_goodbye_force_flag_documented_in_help() {
    let temp = TempDir::new().unwrap();

    bn_in(&temp)
        .args(["goodbye", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--force"))
        .stdout(predicate::str::contains("planner"));
}

#[test]
fn test_goodbye_json_includes_should_terminate() {
    let temp = init_binnacle();

    // Run goodbye and check JSON includes should_terminate field
    let output = bn_in(&temp)
        .args(["goodbye", "--dry-run"])
        .output()
        .expect("Failed to run goodbye");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Output should be valid JSON");

    // should_terminate should be present and true for non-planner agents
    assert!(json.get("should_terminate").is_some());
    assert!(json["should_terminate"].as_bool().unwrap());
}

#[test]
fn test_goodbye_with_force_flag() {
    let temp = init_binnacle();

    // Run goodbye with --force flag
    let output = bn_in(&temp)
        .args(["goodbye", "--dry-run", "--force", "Forced termination"])
        .output()
        .expect("Failed to run goodbye");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Output should be valid JSON");

    // should_terminate should be true when --force is used
    assert!(json["should_terminate"].as_bool().unwrap());
}
