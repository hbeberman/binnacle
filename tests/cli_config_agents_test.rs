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

// === Copilot config integration tests ===

#[test]
fn test_config_agents_show_includes_copilot_json() {
    let env = init_binnacle();

    let output = bn_in(&env)
        .args(["config", "agents", "show", "worker"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);

    // JSON output must include copilot config with default values
    assert!(
        stdout.contains("\"copilot\""),
        "JSON output should include 'copilot' object"
    );
    assert!(
        stdout.contains("\"model\": \"claude-opus-4.6\""),
        "Default model should be claude-opus-4.6"
    );
    assert!(
        stdout.contains("\"reasoning_effort\": \"high\""),
        "Default reasoning_effort should be high"
    );
    assert!(
        stdout.contains("\"show_reasoning\": true"),
        "Default show_reasoning should be true"
    );
    assert!(
        stdout.contains("\"render_markdown\": true"),
        "Default render_markdown should be true"
    );
}

#[test]
fn test_config_agents_show_includes_copilot_human() {
    let env = init_binnacle();

    bn_in(&env)
        .args(["-H", "config", "agents", "show", "worker"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Model: claude-opus-4.6"))
        .stdout(predicate::str::contains("Reasoning Effort: high"))
        .stdout(predicate::str::contains("Show Reasoning: true"))
        .stdout(predicate::str::contains("Render Markdown: true"));
}

#[test]
fn test_copilot_config_known_type_returns_valid_json() {
    let env = init_binnacle();

    let output = bn_in(&env)
        .args(["config", "agents", "copilot-config", "worker"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("copilot-config output must be valid JSON");

    assert_eq!(parsed["staff"], true, "staff must always be true");
    assert_eq!(parsed["model"], "claude-opus-4.6");
    assert_eq!(parsed["reasoning_effort"], "high");
    assert_eq!(parsed["show_reasoning"], true);
    assert_eq!(parsed["render_markdown"], true);
}

#[test]
fn test_copilot_config_unknown_type_returns_defaults() {
    let env = init_binnacle();

    let output = bn_in(&env)
        .args(["config", "agents", "copilot-config", "nonexistent"])
        .assert()
        .success(); // Must never fail

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let stderr = String::from_utf8_lossy(&output.get_output().stderr);

    // Should emit warning on stderr
    assert!(
        stderr.contains("Warning") && stderr.contains("nonexistent"),
        "stderr should contain warning about unknown type, got: {}",
        stderr
    );

    // Should still return valid JSON with defaults
    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .expect("copilot-config output must be valid JSON even for unknown types");
    assert_eq!(parsed["staff"], true);
    assert_eq!(parsed["model"], "claude-opus-4.6");
    assert_eq!(parsed["reasoning_effort"], "high");
}

#[test]
fn test_copilot_config_all_agent_types_succeed() {
    let env = init_binnacle();

    for agent_type in &["worker", "do", "prd", "buddy", "ask", "free"] {
        let output = bn_in(&env)
            .args(["config", "agents", "copilot-config", agent_type])
            .assert()
            .success();

        let stdout = String::from_utf8_lossy(&output.get_output().stdout);
        let parsed: serde_json::Value = serde_json::from_str(&stdout)
            .unwrap_or_else(|_| panic!("copilot-config for '{}' must be valid JSON", agent_type));
        assert_eq!(
            parsed["staff"], true,
            "staff must be true for '{}'",
            agent_type
        );
    }
}

#[test]
fn test_copilot_config_with_project_override() {
    let env = init_binnacle();

    // Create project-level KDL override
    let agents_dir = env.repo_path().join(".binnacle/agents");
    std::fs::create_dir_all(&agents_dir).unwrap();
    std::fs::write(
        agents_dir.join("config.kdl"),
        "agent \"worker\" {\n    model \"claude-sonnet-4\"\n    reasoning-effort \"medium\"\n    show-reasoning #false\n    render-markdown #false\n}\n",
    )
    .unwrap();

    let output = bn_in(&env)
        .args(["config", "agents", "copilot-config", "worker"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("Must be valid JSON");

    assert_eq!(parsed["staff"], true);
    assert_eq!(parsed["model"], "claude-sonnet-4");
    assert_eq!(parsed["reasoning_effort"], "medium");
    assert_eq!(parsed["show_reasoning"], false);
    assert_eq!(parsed["render_markdown"], false);
}

#[test]
fn test_config_agents_show_with_project_override_copilot() {
    let env = init_binnacle();

    // Create project-level KDL override for copilot config only
    let agents_dir = env.repo_path().join(".binnacle/agents");
    std::fs::create_dir_all(&agents_dir).unwrap();
    std::fs::write(
        agents_dir.join("config.kdl"),
        r#"
agent "worker" {
    model "gpt-4"
}
"#,
    )
    .unwrap();

    // JSON output should reflect the override
    let output = bn_in(&env)
        .args(["config", "agents", "show", "worker"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert!(
        stdout.contains("\"model\": \"gpt-4\""),
        "Model should be overridden to gpt-4"
    );
    // Other fields should remain at defaults
    assert!(
        stdout.contains("\"reasoning_effort\": \"high\""),
        "Non-overridden reasoning_effort should stay at default"
    );
}
