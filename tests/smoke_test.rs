//! Smoke tests for the Binnacle CLI.
//!
//! These tests verify basic CLI functionality:
//! - `bn --version` outputs version info
//! - `bn --help` outputs help text
//! - `bn` (no args) outputs valid JSON

use assert_cmd::Command;
use predicates::prelude::*;

/// Get a Command for the bn binary.
fn bn() -> Command {
    Command::new(env!("CARGO_BIN_EXE_bn"))
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
    use tempfile::TempDir;

    // Create a temp directory structure simulating a git repo with subdirs
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    // Create .git directory to make this a "git repo"
    fs::create_dir(root.join(".git")).unwrap();

    // Create a subdirectory
    let subdir = root.join("src");
    fs::create_dir(&subdir).unwrap();

    // Initialize binnacle using explicit -C pointing to the SUBDIR
    bn().args(["-C", subdir.to_str().unwrap(), "system", "init"])
        .assert()
        .success();

    // Create a task in the subdir's binnacle
    bn().args([
        "-C",
        subdir.to_str().unwrap(),
        "task",
        "create",
        "Task in subdir",
    ])
    .assert()
    .success();

    // Now run from the git ROOT - should NOT see the task
    // because explicit -C subdir should have created a separate storage
    let output = bn()
        .args(["-C", root.to_str().unwrap(), "system", "init"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let list_output = bn()
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
