//! Smoke tests for the Binnacle CLI.
//!
//! These tests verify basic CLI functionality:
//! - `bn --version` outputs version info
//! - `bn --help` outputs help text
//! - `bn` (no args) outputs valid JSON

mod common;

use assert_cmd::Command;
use predicates::prelude::*;

use common::TestEnv;

/// Get a Command for the bn binary with test isolation.
///
/// IMPORTANT: Sets BN_DATA_DIR to a temporary directory to prevent tests from
/// writing to the host's real binnacle data/archive directories.
fn bn() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    // Set isolated data directory to prevent polluting host's binnacle data
    let temp_dir = tempfile::tempdir().unwrap();
    cmd.env("BN_DATA_DIR", temp_dir.path());
    // Clear container mode to prevent tests from polluting production /binnacle
    cmd.env_remove("BN_CONTAINER_MODE");
    cmd.env_remove("BN_AGENT_ID");
    cmd.env_remove("BN_AGENT_NAME");
    cmd.env_remove("BN_AGENT_TYPE");
    cmd.env_remove("BN_MCP_SESSION");
    cmd.env_remove("BN_AGENT_SESSION");

    std::mem::forget(temp_dir); // Intentionally leak to keep path valid
    cmd
}

#[test]
fn test_version_flag() {
    bn().arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("bn "));
}

#[test]
fn test_help_flag() {
    bn().arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:"))
        .stdout(predicate::str::contains("Commands:"))
        .stdout(predicate::str::contains("Options:"));
}

#[test]
fn test_help_flag_short() {
    bn().arg("-h")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:"));
}

#[test]
fn test_no_args_outputs_json() {
    bn().assert()
        .success()
        .stdout(predicate::str::contains("{"))
        .stdout(predicate::str::contains("}"));
}

#[test]
fn test_human_readable_flag() {
    bn().arg("-H")
        .assert()
        .success()
        .stdout(predicate::str::contains("Binnacle"));
}

#[test]
fn test_task_help() {
    bn().args(["task", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("create"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("show"));
}

#[test]
fn test_invalid_command() {
    bn().arg("invalid-command")
        .assert()
        .failure()
        .stderr(predicate::str::contains("error"));
}

#[test]
fn test_repo_flag_nonexistent_path() {
    bn().args([
        "-C",
        "/nonexistent/path/that/does/not/exist",
        "task",
        "list",
    ])
    .assert()
    .failure()
    .stderr(predicate::str::contains("does not exist"));
}

#[test]
fn test_repo_flag_in_help() {
    bn().arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("-C, --repo"));
}

/// Test that explicit -C path bypasses git root detection.
///
/// When using `-C /path/to/subdir`, storage should be based on the subdir path
/// literally, NOT resolved to the git root. This verifies the PRD requirement:
/// "Bypasses git root detection - uses the path literally."
#[test]
fn test_explicit_repo_path_bypasses_git_root_detection() {
    use std::fs;

    // Create two separate test environments for root and subdir
    let root_env = TestEnv::new();
    let subdir_env = TestEnv::new();

    let root = root_env.repo_path();

    // Create .git directory to make this a "git repo"
    fs::create_dir(root.join(".git")).unwrap();

    // Create a subdirectory
    let subdir = root.join("src");
    fs::create_dir(&subdir).unwrap();

    // Initialize binnacle using explicit -C pointing to the SUBDIR
    // Use subdir_env's data_dir for isolation
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.env("BN_DATA_DIR", subdir_env.data_path());
    cmd.env_remove("BN_CONTAINER_MODE"); // Prevent container mode leaking into tests
    cmd.args([
        "-C",
        subdir.to_str().unwrap(),
        "session",
        "init",
        "--auto-global",
        "-y",
    ])
    .assert()
    .success();

    // Create a task in the subdir's binnacle
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.env("BN_DATA_DIR", subdir_env.data_path());
    cmd.env_remove("BN_CONTAINER_MODE");
    cmd.args([
        "-C",
        subdir.to_str().unwrap(),
        "task",
        "create",
        "Task in subdir",
    ])
    .assert()
    .success();

    // Now run from the git ROOT with root_env's data_dir
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.env("BN_DATA_DIR", root_env.data_path());
    cmd.env_remove("BN_CONTAINER_MODE");
    let output = cmd
        .args([
            "-C",
            root.to_str().unwrap(),
            "session",
            "init",
            "--auto-global",
            "-y",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.env("BN_DATA_DIR", root_env.data_path());
    cmd.env_remove("BN_CONTAINER_MODE");
    let list_output = cmd
        .args(["-C", root.to_str().unwrap(), "task", "list"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&list_output.stdout);

    // The root's binnacle should NOT contain "Task in subdir"
    // because explicit -C paths should create separate storage
    assert!(
        !stdout.contains("Task in subdir"),
        "Bug: explicit -C path is being resolved to git root! \
         Task created with -C <subdir> should NOT appear when listing with -C <git_root>. \
         Got output: {}",
        stdout
    );
}
