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
    run_hook_with_session(env, commit_msg_file, false)
}

/// Helper to run the commit-msg hook with or without an active agent session
fn run_hook_with_session(
    env: &TestEnv,
    commit_msg_file: &Path,
    agent_session: bool,
) -> std::process::Output {
    let hook_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("hooks/commit-msg");

    // Get the directory containing the bn binary
    let bn_binary = env!("CARGO_BIN_EXE_bn");
    let bn_dir = Path::new(bn_binary).parent().unwrap();

    // Prepend bn directory to PATH
    let path_env = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bn_dir.display(), path_env);

    let mut cmd = Command::new(&hook_path);
    cmd.arg(commit_msg_file)
        .current_dir(env.repo_path())
        .env("BN_DATA_DIR", env.data_path())
        .env("PATH", new_path);

    // Set BN_AGENT_SESSION if running under agent session
    if agent_session {
        cmd.env("BN_AGENT_SESSION", "1");
    }

    let output = cmd.output().expect("Failed to run hook");

    if !output.status.success() {
        eprintln!("Hook stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("Hook stderr: {}", String::from_utf8_lossy(&output.stderr));
    }

    output
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
fn test_hook_without_agent_session_does_nothing() {
    let env = TestEnv::init();

    // Create commit message file
    let msg_file = env.repo_path().join("COMMIT_MSG");
    fs::write(&msg_file, "Test commit\n").unwrap();

    // Run hook without BN_AGENT_SESSION
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

    // Create commit message file
    let msg_file = env.repo_path().join("COMMIT_MSG");
    fs::write(&msg_file, "Test commit\n").unwrap();

    // Run hook with agent session - should be disabled by config
    let output = run_hook_with_session(&env, &msg_file, true);
    assert!(output.status.success());

    // Message should be unchanged
    let content = fs::read_to_string(&msg_file).unwrap();
    assert_eq!(content, "Test commit\n");
}

#[test]
fn test_hook_with_agent_session_adds_trailer() {
    let env = TestEnv::init();
    init_git(&env); // Initialize git repo

    // Create commit message file
    let msg_file = env.repo_path().join("COMMIT_MSG");
    fs::write(&msg_file, "Test commit\n").unwrap();

    // Run hook with BN_AGENT_SESSION=1
    let output = run_hook_with_session(&env, &msg_file, true);
    assert!(output.status.success());

    // Message should have trailer added
    let content = fs::read_to_string(&msg_file).unwrap();
    assert!(
        content.contains("Co-authored-by: binnacle-bot <noreply@binnacle.bot>"),
        "Should add trailer, got: {}",
        content
    );
}

#[test]
fn test_hook_already_has_trailer_no_duplicate() {
    let env = TestEnv::init();
    init_git(&env); // Initialize git repo

    // Create commit message with existing trailer
    let msg_file = env.repo_path().join("COMMIT_MSG");
    fs::write(
        &msg_file,
        "Test commit\n\nCo-authored-by: binnacle-bot <noreply@binnacle.bot>\n",
    )
    .unwrap();

    // Run hook with agent session
    let output = run_hook_with_session(&env, &msg_file, true);
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

    // Create commit message
    let msg_file = env.repo_path().join("COMMIT_MSG");
    fs::write(&msg_file, "Test commit\n").unwrap();

    // Run hook with agent session
    let output = run_hook_with_session(&env, &msg_file, true);
    assert!(output.status.success());

    // Check for custom trailer
    let content = fs::read_to_string(&msg_file).unwrap();
    assert!(
        content.contains("Co-authored-by: my-bot <bot@example.com>"),
        "Should use custom name and email, got: {}",
        content
    );
}
