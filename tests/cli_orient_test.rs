//! Integration tests for Phase 7: Agent Onboarding (`bn orient`).
//!
//! These tests verify that the orient command and AGENTS.md functionality
//! work correctly through the CLI:
//! - `bn orient` - Auto-initializes and shows project state
//! - `bn system init` - Creates/updates AGENTS.md with binnacle reference

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

// === bn system init AGENTS.md Tests ===

#[test]
fn test_init_creates_agents_md() {
    let temp = TestEnv::new();
    let agents_path = temp.path().join("AGENTS.md");

    // Verify AGENTS.md doesn't exist yet
    assert!(!agents_path.exists());

    // Run init
    bn_in(&temp)
        .args(["system", "init"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"agents_md_updated\":true"));

    // Verify AGENTS.md was created
    assert!(agents_path.exists());
    let contents = fs::read_to_string(&agents_path).unwrap();
    assert!(contents.contains("bn orient"));
    assert!(contents.contains("binnacle"));
}

#[test]
fn test_init_appends_to_existing_agents_md() {
    let temp = TestEnv::new();
    let agents_path = temp.path().join("AGENTS.md");

    // Create existing AGENTS.md
    fs::write(&agents_path, "# My Existing Agents\n\nSome content here.\n").unwrap();

    // Run init
    bn_in(&temp)
        .args(["system", "init"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"agents_md_updated\":true"));

    // Verify content was appended, not replaced
    let contents = fs::read_to_string(&agents_path).unwrap();
    assert!(contents.contains("My Existing Agents"));
    assert!(contents.contains("bn orient"));
}

#[test]
fn test_init_appends_markers_if_legacy_bn_orient() {
    let temp = TestEnv::new();
    let agents_path = temp.path().join("AGENTS.md");

    // Create existing AGENTS.md that references bn orient but lacks markers
    fs::write(
        &agents_path,
        "# Agents\n\nRun `bn orient` to get started.\n",
    )
    .unwrap();

    // Run init - should append section with markers
    bn_in(&temp)
        .args(["system", "init"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"agents_md_updated\":true"));

    // Verify markers were added and original content preserved
    let contents = fs::read_to_string(&agents_path).unwrap();
    assert!(contents.contains("# Agents")); // Original content preserved
    assert!(contents.contains("<!-- BEGIN BINNACLE SECTION -->")); // Markers added
    assert!(contents.contains("<!-- END BINNACLE SECTION -->"));
}

#[test]
fn test_init_idempotent_agents_md() {
    let temp = TestEnv::new();

    // Run init twice
    bn_in(&temp).args(["system", "init"]).assert().success();

    bn_in(&temp)
        .args(["system", "init"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"initialized\":false"))
        .stdout(predicate::str::contains("\"agents_md_updated\":false"));
}

#[test]
fn test_init_human_shows_agents_md_update() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["-H", "system", "init"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialized binnacle"))
        .stdout(predicate::str::contains("Updated AGENTS.md"));
}

// === bn orient Tests ===

#[test]
fn test_orient_without_init_fails_when_not_initialized() {
    let temp = TestEnv::new();

    // Run orient without --init (should fail)
    bn_in(&temp)
        .arg("orient")
        .assert()
        .failure()
        .stderr(predicate::str::contains("No binnacle database found"))
        .stderr(predicate::str::contains("bn orient --init"));
}

#[test]
fn test_orient_with_init_creates_database() {
    let temp = TestEnv::new();
    let agents_path = temp.path().join("AGENTS.md");

    // Verify not initialized
    assert!(!agents_path.exists());

    // Run orient --init (should succeed and initialize)
    bn_in(&temp)
        .args(["orient", "--init"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"initialized\":true"));

    // Verify AGENTS.md was created
    assert!(agents_path.exists());
}

#[test]
fn test_orient_works_when_already_initialized() {
    let temp = init_binnacle();

    bn_in(&temp)
        .arg("orient")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"initialized\":false"))
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
        .arg("orient")
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
        .arg("orient")
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
        .arg("orient")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"in_progress_count\":1"));
}

#[test]
fn test_orient_human_format() {
    let temp = init_binnacle();
    create_task(&temp, "My Task");

    bn_in(&temp)
        .args(["-H", "orient"])
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
        .args(["-H", "orient"])
        .assert()
        .success()
        .stdout(predicate::str::contains(&task_id));
}

#[test]
fn test_orient_json_includes_ready_ids() {
    let temp = init_binnacle();
    let task_id = create_task(&temp, "My Task");

    bn_in(&temp)
        .arg("orient")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"ready_ids\""))
        .stdout(predicate::str::contains(&task_id));
}

#[test]
fn test_orient_empty_project() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["orient", "--init"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total_tasks\":0"))
        .stdout(predicate::str::contains("\"ready_count\":0"));
}

#[test]
fn test_orient_human_empty_project() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["-H", "orient", "--init"])
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
        .args(["orient", "--init"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"initialized\":true"));

    // Second --init should also succeed (no-op for already initialized)
    bn_in(&temp)
        .args(["orient", "--init"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"initialized\":false"));
}

#[test]
fn test_orient_error_json_is_valid() {
    let temp = TestEnv::new();

    // Run orient without --init and capture JSON error
    let output = bn_in(&temp).arg("orient").assert().failure();

    // Get stderr and verify it's valid JSON with expected fields
    let stderr = String::from_utf8_lossy(&output.get_output().stderr);
    let json: serde_json::Value =
        serde_json::from_str(stderr.trim()).expect("Error output should be valid JSON");

    assert_eq!(json["error"], "No binnacle database found");
    assert!(json["hint"].as_str().unwrap().contains("bn system init"));
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
        .stdout(predicate::str::contains("purpose"));
}
