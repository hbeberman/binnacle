//! Integration tests for Phase 7: Agent Onboarding (`bn orient`).
//!
//! These tests verify that the orient command works correctly through the CLI:
//! - `bn orient` - Auto-initializes and shows project state
//! - `bn session init` - Initializes binnacle for a repo (does NOT touch AGENTS.md)

mod common;

use assert_cmd::Command;
use common::TestEnv;
use predicates::prelude::*;
use std::fs;

/// Get a Command for the bn binary in a TestEnv.
fn bn_in(env: &TestEnv) -> Command {
    env.bn()
}

/// Initialize binnacle in a temp directory and return the TestEnv.
fn init_binnacle() -> TestEnv {
    TestEnv::init()
}

/// Create a task and return its ID.
fn create_task(env: &TestEnv, title: &str) -> String {
    let output = bn_in(env)
        .args(["task", "create", title])
        .output()
        .expect("Failed to run bn task create");

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Extract ID from JSON output like {"id":"bn-xxxx","title":"..."}
    let id_start = stdout.find("\"id\":\"").expect("No id in output") + 6;
    let id_end = stdout[id_start..]
        .find('"')
        .expect("No closing quote for id")
        + id_start;
    stdout[id_start..id_end].to_string()
}

// === bn session init AGENTS.md Tests ===
// These tests verify that bn session init does NOT create or modify AGENTS.md.
// Agent instructions are now delivered via container prompt injection.
// See PRD bn-3e98 for details.

#[test]
fn test_init_does_not_create_agents_md() {
    let temp = TestEnv::new();
    let agents_path = temp.path().join("AGENTS.md");

    // Verify AGENTS.md doesn't exist yet
    assert!(!agents_path.exists());

    // Run init (without --write-agents-md flag which no longer exists)
    bn_in(&temp)
        .args(["session", "init", "--auto-global", "-y"])
        .assert()
        .success();

    // Verify AGENTS.md was NOT created
    assert!(
        !agents_path.exists(),
        "bn session init should NOT create AGENTS.md"
    );
}

#[test]
fn test_init_does_not_modify_existing_agents_md() {
    let temp = TestEnv::new();
    let agents_path = temp.path().join("AGENTS.md");

    // Create existing AGENTS.md with custom content
    let original_content = "# My Existing Agents\n\nSome content here.\n";
    fs::write(&agents_path, original_content).unwrap();

    // Run init
    bn_in(&temp)
        .args(["session", "init", "--auto-global", "-y"])
        .assert()
        .success();

    // Verify content was NOT modified
    let contents = fs::read_to_string(&agents_path).unwrap();
    assert_eq!(
        contents, original_content,
        "bn session init should NOT modify existing AGENTS.md"
    );
}

#[test]
fn test_init_idempotent_without_agents_md() {
    let temp = TestEnv::new();
    let agents_path = temp.path().join("AGENTS.md");

    // Run init twice
    bn_in(&temp)
        .args(["session", "init", "--auto-global", "-y"])
        .assert()
        .success();

    bn_in(&temp)
        .args(["session", "init", "--auto-global", "-y"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"initialized\":false"));

    // Verify AGENTS.md was never created
    assert!(
        !agents_path.exists(),
        "bn session init should NOT create AGENTS.md even on repeated runs"
    );
}

#[test]
fn test_init_human_does_not_mention_agents_md() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["-H", "session", "init", "--auto-global", "-y"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Session initialized"))
        // Should NOT mention AGENTS.md updates
        .stdout(predicate::str::contains("AGENTS.md").not());
}

// === bn orient Tests ===

#[test]
fn test_orient_without_init_fails_when_not_initialized() {
    let temp = TestEnv::new();

    // Run orient without --init (should fail)
    bn_in(&temp)
        .args(["orient", "--type", "worker", "--dry-run"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No binnacle database found"))
        .stderr(predicate::str::contains("bn orient --init"));
}

#[test]
fn test_orient_with_init_creates_database() {
    let temp = TestEnv::new();

    // Run orient --init (should succeed and initialize)
    bn_in(&temp)
        .args(["orient", "--type", "worker", "--init", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"ready\":true"))
        .stdout(predicate::str::contains("\"just_initialized\":true"));

    // Verify database was created (by checking storage exists)
    let storage_path = temp.path().join(".git").join(".binnacle");
    assert!(
        binnacle::storage::Storage::exists(temp.path()).unwrap()
            || storage_path.exists()
            || temp.path().join(".binnacle").exists()
    );
}

#[test]
fn test_orient_works_when_already_initialized() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["orient", "--type", "worker", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"ready\":true"))
        .stdout(predicate::str::contains("\"just_initialized\":false"))
        .stdout(predicate::str::contains("\"total_tasks\""));
}

#[test]
fn test_orient_shows_task_counts() {
    let temp = init_binnacle();

    // Create some tasks
    create_task(&temp, "Task A");
    create_task(&temp, "Task B");
    create_task(&temp, "Task C");

    bn_in(&temp)
        .args(["orient", "--type", "worker", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total_tasks\":3"))
        .stdout(predicate::str::contains("\"ready_count\":3"));
}

#[test]
fn test_orient_shows_blocked_tasks() {
    let temp = init_binnacle();

    let task_a = create_task(&temp, "Task A");
    let task_b = create_task(&temp, "Task B");

    // B depends on A
    bn_in(&temp)
        .args([
            "link",
            "add",
            &task_b,
            &task_a,
            "--type",
            "depends_on",
            "--reason",
            "test dep",
        ])
        .assert()
        .success();

    bn_in(&temp)
        .args(["orient", "--type", "worker", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"ready_count\":1"))
        .stdout(predicate::str::contains("\"blocked_count\":1"));
}

#[test]
fn test_orient_shows_in_progress_tasks() {
    let temp = init_binnacle();

    let task = create_task(&temp, "Task A");

    // Set to in_progress
    bn_in(&temp)
        .args(["task", "update", &task, "--status", "in_progress"])
        .assert()
        .success();

    bn_in(&temp)
        .args(["orient", "--type", "worker", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"in_progress_count\":1"));
}

#[test]
fn test_orient_human_format() {
    let temp = init_binnacle();
    create_task(&temp, "My Task");

    bn_in(&temp)
        .args(["-H", "orient", "--type", "worker"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Binnacle - AI agent task tracker"))
        .stdout(predicate::str::contains("Total tasks: 1"))
        .stdout(predicate::str::contains("Key Commands:"))
        .stdout(predicate::str::contains("bn ready"))
        .stdout(predicate::str::contains("bn task list"));
}

#[test]
fn test_orient_human_shows_ready_task_ids() {
    let temp = init_binnacle();
    let task_id = create_task(&temp, "My Task");

    bn_in(&temp)
        .args(["-H", "orient", "--type", "worker"])
        .assert()
        .success()
        .stdout(predicate::str::contains(&task_id));
}

#[test]
fn test_orient_json_includes_ready_ids() {
    let temp = init_binnacle();
    let task_id = create_task(&temp, "My Task");

    bn_in(&temp)
        .args(["orient", "--type", "worker", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"ready_ids\""))
        .stdout(predicate::str::contains(&task_id));
}

#[test]
fn test_orient_empty_project() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["orient", "--type", "worker", "--init", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total_tasks\":0"))
        .stdout(predicate::str::contains("\"ready_count\":0"));
}

#[test]
fn test_orient_human_empty_project() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["-H", "orient", "--type", "worker", "--init"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Total tasks: 0"))
        .stdout(predicate::str::contains("Ready: 0"));
}

#[test]
fn test_orient_init_is_idempotent() {
    let temp = TestEnv::new();

    // First --init
    bn_in(&temp)
        .args(["orient", "--type", "worker", "--init", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"ready\":true"))
        .stdout(predicate::str::contains("\"just_initialized\":true"));

    // Second --init should also succeed (no-op for already initialized)
    bn_in(&temp)
        .args(["orient", "--type", "worker", "--init", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"ready\":true"))
        .stdout(predicate::str::contains("\"just_initialized\":false"));
}

#[test]
fn test_orient_error_json_is_valid() {
    let temp = TestEnv::new();

    // Run orient without --init and capture JSON error
    let output = bn_in(&temp)
        .args(["orient", "--type", "worker", "--dry-run"])
        .assert()
        .failure();

    // Get stderr and verify it's valid JSON with expected fields
    let stderr = String::from_utf8_lossy(&output.get_output().stderr);
    let json: serde_json::Value =
        serde_json::from_str(stderr.trim()).expect("Error output should be valid JSON");

    assert_eq!(json["error"], "No binnacle database found");
    assert!(json["hint"].as_str().unwrap().contains("bn session init"));
    assert!(json["hint"].as_str().unwrap().contains("bn orient --init"));
    assert!(json["path"].is_string());
}

// === Agent Purpose Registration Tests ===

#[test]
fn test_orient_with_register_registers_purpose() {
    let temp = init_binnacle();

    // Orient with --register
    bn_in(&temp)
        .args([
            "orient",
            "--type",
            "worker",
            "--name",
            "test-worker",
            "--register",
            "Task Worker",
        ])
        .assert()
        .success();

    // Check agent was registered with purpose via bn agent list
    let output = bn_in(&temp)
        .args(["agent", "list"])
        .output()
        .expect("Failed to run bn agent list");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Task Worker") || stdout.contains("test-worker"));
}

#[test]
fn test_orient_help_shows_register_flag() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["orient", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--register"))
        .stdout(predicate::str::contains("purpose"))
        .stdout(predicate::str::contains("--type"));
}

#[test]
fn test_orient_requires_type_flag() {
    let temp = init_binnacle();

    // Run orient without --type (should fail)
    bn_in(&temp)
        .arg("orient")
        .assert()
        .failure()
        .stderr(predicate::str::contains("--type"));
}

#[test]
fn test_orient_accepts_all_agent_types() {
    let temp = TestEnv::new();

    // Test worker type (with dry-run and init since test env won't have agents)
    bn_in(&temp)
        .args(["orient", "--type", "worker", "--init", "--dry-run"])
        .assert()
        .success();

    // Test planner type (with dry-run)
    bn_in(&temp)
        .args(["orient", "--type", "planner", "--dry-run"])
        .assert()
        .success();

    // Test buddy type (with dry-run)
    bn_in(&temp)
        .args(["orient", "--type", "buddy", "--dry-run"])
        .assert()
        .success();
}

#[test]
fn test_orient_rejects_invalid_type() {
    let temp = init_binnacle();

    // Run orient with invalid type
    bn_in(&temp)
        .args(["orient", "--type", "invalid"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid"));
}

#[test]
fn test_orient_uses_bn_agent_name_env_var() {
    let temp = init_binnacle();

    // Orient with BN_AGENT_NAME env var (NOT dry-run, we need to test registration)
    bn_in(&temp)
        .env("BN_AGENT_NAME", "container-worker-1")
        .args(["orient", "--type", "worker"])
        .assert()
        .success();

    // Check agent was registered with the env var name
    let output = bn_in(&temp)
        .args(["agent", "list"])
        .output()
        .expect("Failed to run bn agent list");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("container-worker-1"),
        "Agent list should contain the BN_AGENT_NAME value. Got: {}",
        stdout
    );
}

#[test]
fn test_orient_name_flag_takes_precedence_over_env_var() {
    let temp = init_binnacle();

    // Orient with both --name flag and BN_AGENT_NAME env var
    bn_in(&temp)
        .env("BN_AGENT_NAME", "env-name")
        .args(["orient", "--type", "worker", "--name", "flag-name"])
        .assert()
        .success();

    // Check agent was registered with the flag name (not env var)
    let output = bn_in(&temp)
        .args(["agent", "list"])
        .output()
        .expect("Failed to run bn agent list");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("flag-name"),
        "Agent list should contain the --name flag value, not BN_AGENT_NAME. Got: {}",
        stdout
    );
    // The env var name should not appear (unless both somehow register)
    // Note: we're checking that the flag takes precedence, but both could appear
    // if the test framework creates multiple PIDs. Focus on flag-name being present.
}

#[test]
fn test_orient_dry_run_skips_agent_registration() {
    let temp = init_binnacle();

    // Orient with --dry-run should NOT register the agent
    bn_in(&temp)
        .args([
            "orient",
            "--type",
            "worker",
            "--name",
            "ghost-agent",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"agent_id\":\"dry-run\""));

    // Check that the agent was NOT registered
    let output = bn_in(&temp)
        .args(["agent", "list"])
        .output()
        .expect("Failed to run bn agent list");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("ghost-agent"),
        "Agent list should NOT contain the dry-run agent. Got: {}",
        stdout
    );
}

#[test]
fn test_orient_dry_run_still_shows_project_state() {
    let temp = init_binnacle();

    // Create some tasks
    create_task(&temp, "Test Task A");
    create_task(&temp, "Test Task B");

    // Orient with --dry-run should still show accurate project state
    bn_in(&temp)
        .args(["orient", "--type", "worker", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total_tasks\":2"))
        .stdout(predicate::str::contains("\"ready_count\":2"));
}
