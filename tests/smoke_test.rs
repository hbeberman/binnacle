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
        .stdout(predicate::str::contains("bn"))
        .stdout(predicate::str::contains("0.1.0"));
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
