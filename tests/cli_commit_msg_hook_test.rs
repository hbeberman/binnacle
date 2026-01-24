//! Integration tests for the commit-msg hook.
//!
//! Tests the Co-authored-by trailer functionality.

mod common;
use common::TestEnv;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

/// Helper to initialize git in a TestEnv
fn init_git(env: &TestEnv) {
    Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(env.repo_path())
        .output()
        .expect("Failed to init git");

    // Configure git user (needed for commits)
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(env.repo_path())
        .output()
        .expect("Failed to configure git email");
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(env.repo_path())
        .output()
        .expect("Failed to configure git name");
}

/// Helper to run the commit-msg hook directly with test environment
fn run_hook(env: &TestEnv, commit_msg_file: &Path) -> std::process::Output {
    let hook_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("hooks/commit-msg");

    // Get the directory containing the bn binary
    let bn_binary = env!("CARGO_BIN_EXE_bn");
    let bn_dir = Path::new(bn_binary).parent().unwrap();

    // Prepend bn directory to PATH
    let path_env = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bn_dir.display(), path_env);

    let output = Command::new(&hook_path)
        .arg(commit_msg_file)
        .current_dir(env.repo_path())
        .env("BN_DATA_DIR", env.data_path())
        .env("PATH", new_path)
        .output()
        .expect("Failed to run hook");

    if !output.status.success() {
        eprintln!("Hook stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("Hook stderr: {}", String::from_utf8_lossy(&output.stderr));
    }

    output
}

/// Write a session.json file to the storage directory
fn write_session_state(env: &TestEnv, agent_pid: u32, orient_called: bool) {
    // Get storage dir for this repo
    let repo_canonical = env.repo_path().canonicalize().unwrap();
    let repo_str = repo_canonical.to_string_lossy();

    // SHA256 hash (first 12 chars)
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(repo_str.as_bytes());
    let hash = hasher.finalize();
    let hash_hex = format!("{:x}", hash);
    let short_hash = &hash_hex[..12];

    let storage_dir = env.data_path().join(short_hash);
    fs::create_dir_all(&storage_dir).unwrap();

    let session_json = format!(
        r#"{{
  "agent_pid": {},
  "agent_type": "worker",
  "started_at": "2026-01-24T00:00:00Z",
  "orient_called": {}
}}"#,
        agent_pid, orient_called
    );

    fs::write(storage_dir.join("session.json"), session_json).unwrap();
}

#[test]
fn test_hook_is_executable() {
    let hook_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("hooks/commit-msg");
    assert!(hook_path.exists(), "commit-msg hook should exist");

    let metadata = fs::metadata(&hook_path).unwrap();
    let permissions = metadata.permissions();
    assert!(permissions.mode() & 0o111 != 0, "hook should be executable");
}

#[test]
fn test_hook_no_session_does_nothing() {
    let env = TestEnv::init();

    // Create a commit message file
    let msg_file = env.repo_path().join("COMMIT_MSG");
    fs::write(&msg_file, "Initial commit\n").unwrap();

    // Run hook - should succeed but not modify anything (no session)
    let output = run_hook(&env, &msg_file);
    assert!(output.status.success(), "Hook should succeed");

    // Message should be unchanged
    let content = fs::read_to_string(&msg_file).unwrap();
    assert_eq!(content, "Initial commit\n");
}

#[test]
fn test_hook_with_orient_called_false_does_nothing() {
    let env = TestEnv::init();

    // Write session state with orient_called=false
    write_session_state(&env, std::process::id(), false);

    // Create commit message file
    let msg_file = env.repo_path().join("COMMIT_MSG");
    fs::write(&msg_file, "Test commit\n").unwrap();

    // Run hook
    let output = run_hook(&env, &msg_file);
    assert!(output.status.success());

    // Message should be unchanged
    let content = fs::read_to_string(&msg_file).unwrap();
    assert_eq!(content, "Test commit\n");
}

#[test]
fn test_hook_disabled_by_config() {
    let env = TestEnv::init();

    // Disable co-author feature
    env.bn()
        .args(["config", "set", "co-author.enabled", "false"])
        .assert()
        .success();

    // Write active session state with current process as agent
    write_session_state(&env, std::process::id(), true);

    // Create commit message file
    let msg_file = env.repo_path().join("COMMIT_MSG");
    fs::write(&msg_file, "Test commit\n").unwrap();

    // Run hook - should be disabled
    let output = run_hook(&env, &msg_file);
    assert!(output.status.success());

    // Message should be unchanged
    let content = fs::read_to_string(&msg_file).unwrap();
    assert_eq!(content, "Test commit\n");
}

#[test]
fn test_hook_with_wrong_pid_does_nothing() {
    let env = TestEnv::init();

    // Write session state with a different PID (not our ancestor)
    write_session_state(&env, 99999, true); // Very unlikely to be an ancestor

    // Create commit message file
    let msg_file = env.repo_path().join("COMMIT_MSG");
    fs::write(&msg_file, "Test commit\n").unwrap();

    // Run hook
    let output = run_hook(&env, &msg_file);
    assert!(output.status.success());

    // Message should be unchanged (wrong PID)
    let content = fs::read_to_string(&msg_file).unwrap();
    assert_eq!(content, "Test commit\n");
}

#[test]
fn test_hook_already_has_trailer_no_duplicate() {
    let env = TestEnv::init();
    init_git(&env); // Initialize git repo

    // Write active session state with PID 1 (init, always an ancestor)
    write_session_state(&env, 1, true);

    // Create commit message with existing trailer
    let msg_file = env.repo_path().join("COMMIT_MSG");
    fs::write(
        &msg_file,
        "Test commit\n\nCo-authored-by: binnacle-bot <noreply@binnacle.bot>\n",
    )
    .unwrap();

    // Run hook
    let output = run_hook(&env, &msg_file);
    assert!(output.status.success());

    // Message should not have duplicate trailer
    let content = fs::read_to_string(&msg_file).unwrap();
    let trailer_count = content.matches("Co-authored-by: binnacle-bot").count();
    assert_eq!(trailer_count, 1, "Should not duplicate trailer");
}

#[test]
fn test_hook_custom_name_email() {
    let env = TestEnv::init();
    init_git(&env); // Initialize git repo

    // Set custom name and email
    env.bn()
        .args(["config", "set", "co-author.name", "my-bot"])
        .assert()
        .success();
    env.bn()
        .args(["config", "set", "co-author.email", "bot@example.com"])
        .assert()
        .success();

    // Write active session state with PID 1
    write_session_state(&env, 1, true);

    // Create commit message
    let msg_file = env.repo_path().join("COMMIT_MSG");
    fs::write(&msg_file, "Test commit\n").unwrap();

    // Run hook
    let output = run_hook(&env, &msg_file);
    assert!(output.status.success());

    // Check for custom trailer
    let content = fs::read_to_string(&msg_file).unwrap();
    assert!(
        content.contains("Co-authored-by: my-bot <bot@example.com>"),
        "Should use custom name and email, got: {}",
        content
    );
}
