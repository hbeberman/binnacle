//! Integration tests for Phase 7: Agent Onboarding (`bn orient`).
//!
//! These tests verify that the orient command and AGENTS.md functionality
//! work correctly through the CLI:
//! - `bn orient` - Auto-initializes and shows project state
//! - `bn init` - Creates/updates AGENTS.md with binnacle reference

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
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
    bn_in(&temp).arg("init").assert().success();
    temp
}

/// Create a task and return its ID.
fn create_task(dir: &TempDir, title: &str) -> String {
    let output = bn_in(dir)
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

// === bn init AGENTS.md Tests ===

#[test]
fn test_init_creates_agents_md() {
    let temp = TempDir::new().unwrap();
    let agents_path = temp.path().join("AGENTS.md");

    // Verify AGENTS.md doesn't exist yet
    assert!(!agents_path.exists());

    // Run init
    bn_in(&temp)
        .arg("init")
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
    let temp = TempDir::new().unwrap();
    let agents_path = temp.path().join("AGENTS.md");

    // Create existing AGENTS.md
    fs::write(&agents_path, "# My Existing Agents\n\nSome content here.\n").unwrap();

    // Run init
    bn_in(&temp)
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"agents_md_updated\":true"));

    // Verify content was appended, not replaced
    let contents = fs::read_to_string(&agents_path).unwrap();
    assert!(contents.contains("My Existing Agents"));
    assert!(contents.contains("bn orient"));
}

#[test]
fn test_init_skips_agents_md_if_already_has_bn_orient() {
    let temp = TempDir::new().unwrap();
    let agents_path = temp.path().join("AGENTS.md");

    // Create existing AGENTS.md that already references bn orient
    fs::write(
        &agents_path,
        "# Agents\n\nRun `bn orient` to get started.\n",
    )
    .unwrap();

    // Run init
    bn_in(&temp)
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"agents_md_updated\":false"));

    // Verify content wasn't duplicated
    let contents = fs::read_to_string(&agents_path).unwrap();
    assert_eq!(contents.matches("bn orient").count(), 1);
}

#[test]
fn test_init_idempotent_agents_md() {
    let temp = TempDir::new().unwrap();

    // Run init twice
    bn_in(&temp).arg("init").assert().success();

    bn_in(&temp)
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"initialized\":false"))
        .stdout(predicate::str::contains("\"agents_md_updated\":false"));
}

#[test]
fn test_init_human_shows_agents_md_update() {
    let temp = TempDir::new().unwrap();

    bn_in(&temp)
        .args(["-H", "init"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialized binnacle"))
        .stdout(predicate::str::contains("Updated AGENTS.md"));
}

// === bn orient Tests ===

#[test]
fn test_orient_auto_initializes() {
    let temp = TempDir::new().unwrap();
    let agents_path = temp.path().join("AGENTS.md");

    // Verify not initialized
    assert!(!agents_path.exists());

    // Run orient (should auto-init)
    bn_in(&temp)
        .arg("orient")
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
        .args(["dep", "add", &task_b, &task_a])
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
    let temp = TempDir::new().unwrap();

    bn_in(&temp)
        .arg("orient")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total_tasks\":0"))
        .stdout(predicate::str::contains("\"ready_count\":0"));
}

#[test]
fn test_orient_human_empty_project() {
    let temp = TempDir::new().unwrap();

    bn_in(&temp)
        .args(["-H", "orient"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Total tasks: 0"))
        .stdout(predicate::str::contains("Ready: 0"));
}
