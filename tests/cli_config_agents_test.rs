//! Tests for `bn config agents` commands.
//!
//! These tests verify the agent configuration listing, showing, and emitting functionality.

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

#[test]
fn test_config_agents_list_json() {
    let env = init_binnacle();

    bn_in(&env)
        .args(["config", "agents", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\": 6"))
        .stdout(predicate::str::contains("\"name\": \"worker\""))
        .stdout(predicate::str::contains("\"name\": \"prd\""))
        .stdout(predicate::str::contains("\"name\": \"buddy\""))
        .stdout(predicate::str::contains("\"name\": \"ask\""))
        .stdout(predicate::str::contains("\"name\": \"free\""))
        .stdout(predicate::str::contains("\"name\": \"do\""));
}

#[test]
fn test_config_agents_list_human() {
    let env = init_binnacle();

    bn_in(&env)
        .args(["-H", "config", "agents", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("6 agent definitions"))
        .stdout(predicate::str::contains("worker"))
        .stdout(predicate::str::contains("container, stateful"))
        .stdout(predicate::str::contains("Source: embedded"));
}

#[test]
fn test_config_agents_show_worker_json() {
    let env = init_binnacle();

    bn_in(&env)
        .args(["config", "agents", "show", "worker"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\": \"worker\""))
        .stdout(predicate::str::contains("\"execution\": \"container\""))
        .stdout(predicate::str::contains("\"lifecycle\": \"stateful\""))
        .stdout(predicate::str::contains("\"tools_allow\""))
        .stdout(predicate::str::contains("\"tools_deny\""));
}

#[test]
fn test_config_agents_show_prd_human() {
    let env = init_binnacle();

    bn_in(&env)
        .args(["-H", "config", "agents", "show", "prd"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Agent: prd"))
        .stdout(predicate::str::contains("Execution: host"))
        .stdout(predicate::str::contains(
            "Lifecycle: stateless (no goodbye)",
        ))
        .stdout(predicate::str::contains("Tools Allowed:"))
        .stdout(predicate::str::contains("Tools Denied:"));
}

#[test]
fn test_config_agents_show_invalid_agent() {
    let env = init_binnacle();

    bn_in(&env)
        .args(["config", "agents", "show", "invalid"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unknown agent type 'invalid'"))
        .stderr(predicate::str::contains(
            "worker, do, prd, buddy, ask, free",
        ));
}

#[test]
fn test_config_agents_emit_worker_json() {
    let env = init_binnacle();

    bn_in(&env)
        .args(["config", "agents", "emit", "worker"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\": \"worker\""))
        .stdout(predicate::str::contains("\"prompt\""))
        .stdout(predicate::str::contains("bn orient"));
}

#[test]
fn test_config_agents_emit_prd_human() {
    let env = init_binnacle();

    // Human mode emits raw prompt for piping
    bn_in(&env)
        .args(["-H", "config", "agents", "emit", "prd"])
        .assert()
        .success()
        .stdout(predicate::str::contains("bn orient --type planner"))
        .stdout(predicate::str::contains("PRD"));
}

#[test]
fn test_config_agents_emit_invalid_agent() {
    let env = init_binnacle();

    bn_in(&env)
        .args(["config", "agents", "emit", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unknown agent type 'nonexistent'"));
}

#[test]
fn test_config_agents_show_tools_content() {
    let env = init_binnacle();

    // Worker should have shell(bn:*) allowed and binnacle-orient denied
    let output = bn_in(&env)
        .args(["config", "agents", "show", "worker"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert!(
        stdout.contains("shell(bn:*)"),
        "Worker should allow shell(bn:*)"
    );
    assert!(
        stdout.contains("binnacle(binnacle-orient)") || stdout.contains("binnacle-orient"),
        "Worker should deny binnacle-orient"
    );
}
