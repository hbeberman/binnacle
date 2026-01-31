//! Integration tests for anonymous commit mode.
//!
//! Tests the git identity behavior when using anonymous mode (binnacle-bot identity).

mod common;
use common::TestEnv;

use std::fs;
use std::path::Path;
use std::process::Command;

/// Helper to initialize git in a TestEnv
fn init_git(env: &TestEnv) {
    Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(env.repo_path())
        .output()
        .expect("Failed to init git");
}

/// Helper to configure git user identity
fn configure_git_identity(env: &TestEnv, name: &str, email: &str) {
    Command::new("git")
        .args(["config", "user.email", email])
        .current_dir(env.repo_path())
        .output()
        .expect("Failed to configure git email");
    Command::new("git")
        .args(["config", "user.name", name])
        .current_dir(env.repo_path())
        .output()
        .expect("Failed to configure git name");
}

/// Helper to run the commit-msg hook with environment variables
fn run_hook_with_env(
    env: &TestEnv,
    commit_msg_file: &Path,
    agent_session: bool,
    anonymous_identity: bool,
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

    // Clear agent-specific env vars for test isolation
    cmd.env_remove("BN_AGENT_ID");
    cmd.env_remove("BN_AGENT_NAME");
    cmd.env_remove("BN_AGENT_TYPE");
    cmd.env_remove("BN_MCP_SESSION");
    cmd.env_remove("BN_CONTAINER_MODE");

    // Set BN_AGENT_SESSION
    if agent_session {
        cmd.env("BN_AGENT_SESSION", "1");
    } else {
        cmd.env_remove("BN_AGENT_SESSION");
    }

    // Set BINNACLE_ANONYMOUS_IDENTITY
    if anonymous_identity {
        cmd.env("BINNACLE_ANONYMOUS_IDENTITY", "1");
    } else {
        cmd.env_remove("BINNACLE_ANONYMOUS_IDENTITY");
    }

    cmd.output().expect("Failed to run hook")
}

// ============================================================================
// Test: Commits in anonymous mode have no Co-authored-by trailer
// ============================================================================

#[test]
fn test_anonymous_mode_skips_co_author_trailer() {
    let env = TestEnv::init();
    init_git(&env);

    // Create commit message file
    let msg_file = env.repo_path().join("COMMIT_MSG");
    fs::write(&msg_file, "Test commit in anonymous mode\n").unwrap();

    // Run hook with agent session AND anonymous identity
    let output = run_hook_with_env(&env, &msg_file, true, true);
    assert!(output.status.success(), "Hook should succeed");

    // Message should NOT have Co-authored-by trailer
    let content = fs::read_to_string(&msg_file).unwrap();
    assert!(
        !content.contains("Co-authored-by:"),
        "Anonymous mode should skip Co-authored-by trailer, got: {}",
        content
    );
    assert_eq!(
        content.trim(),
        "Test commit in anonymous mode",
        "Message should be unchanged"
    );
}

#[test]
fn test_anonymous_identity_env_var_skips_trailer_even_with_session() {
    let env = TestEnv::init();
    init_git(&env);
    configure_git_identity(&env, "Test User", "test@example.com");

    // Create commit message
    let msg_file = env.repo_path().join("COMMIT_MSG");
    fs::write(&msg_file, "Commit with session but anonymous\n").unwrap();

    // Run hook with both BN_AGENT_SESSION=1 and BINNACLE_ANONYMOUS_IDENTITY=1
    let output = run_hook_with_env(&env, &msg_file, true, true);
    assert!(output.status.success());

    // Should NOT add trailer when anonymous identity is set
    let content = fs::read_to_string(&msg_file).unwrap();
    assert!(
        !content.contains("Co-authored-by:"),
        "BINNACLE_ANONYMOUS_IDENTITY=1 should skip trailer, got: {}",
        content
    );
}

// ============================================================================
// Test: Commits in normal mode have Co-authored-by when enabled
// ============================================================================

#[test]
fn test_normal_mode_adds_co_author_trailer() {
    let env = TestEnv::init();
    init_git(&env);
    configure_git_identity(&env, "Human Dev", "human@example.com");

    // Create commit message
    let msg_file = env.repo_path().join("COMMIT_MSG");
    fs::write(&msg_file, "Normal mode commit\n").unwrap();

    // Run hook with agent session but NOT anonymous identity
    let output = run_hook_with_env(&env, &msg_file, true, false);
    assert!(output.status.success());

    // Should add Co-authored-by trailer
    let content = fs::read_to_string(&msg_file).unwrap();
    assert!(
        content.contains("Co-authored-by: binnacle-bot <noreply@binnacle.bot>"),
        "Normal mode should add Co-authored-by trailer, got: {}",
        content
    );
}

#[test]
fn test_co_author_uses_custom_bot_identity() {
    let env = TestEnv::init();
    init_git(&env);

    // Set custom bot identity
    env.bn()
        .args(["config", "set", "git-bot.name", "custom-agent"])
        .assert()
        .success();
    env.bn()
        .args(["config", "set", "git-bot.email", "agent@custom.io"])
        .assert()
        .success();

    // Create commit message
    let msg_file = env.repo_path().join("COMMIT_MSG");
    fs::write(&msg_file, "Commit with custom bot\n").unwrap();

    // Run hook with agent session (not anonymous)
    let output = run_hook_with_env(&env, &msg_file, true, false);
    assert!(output.status.success());

    // Should use custom bot identity in trailer
    let content = fs::read_to_string(&msg_file).unwrap();
    assert!(
        content.contains("Co-authored-by: custom-agent <agent@custom.io>"),
        "Should use custom bot identity, got: {}",
        content
    );
}

#[test]
fn test_co_author_disabled_by_config() {
    let env = TestEnv::init();
    init_git(&env);

    // Disable co-author feature
    env.bn()
        .args(["config", "set", "git.co-author.enabled", "false"])
        .assert()
        .success();

    // Create commit message
    let msg_file = env.repo_path().join("COMMIT_MSG");
    fs::write(&msg_file, "Commit with co-author disabled\n").unwrap();

    // Run hook with agent session (not anonymous)
    let output = run_hook_with_env(&env, &msg_file, true, false);
    assert!(output.status.success());

    // Should NOT add trailer when disabled
    let content = fs::read_to_string(&msg_file).unwrap();
    assert!(
        !content.contains("Co-authored-by:"),
        "Disabled config should skip trailer, got: {}",
        content
    );
}

// ============================================================================
// Test: git.anonymous.allow config validation and behavior
// ============================================================================

#[test]
fn test_anonymous_allow_config_accepts_valid_values() {
    let env = TestEnv::init();

    // All valid boolean values should be accepted
    for value in &["true", "false", "yes", "no", "1", "0"] {
        env.bn()
            .args(["config", "set", "git.anonymous.allow", value])
            .assert()
            .success();
    }
}

#[test]
fn test_anonymous_allow_config_rejects_invalid_values() {
    let env = TestEnv::init();

    // Invalid values should be rejected
    for value in &["invalid", "maybe", "always", "never", "enabled"] {
        env.bn()
            .args(["config", "set", "git.anonymous.allow", value])
            .assert()
            .failure();
    }
}

#[test]
fn test_anonymous_allow_default_is_true() {
    let env = TestEnv::init();

    // Check default value is true
    env.bn()
        .args(["config", "get", "git.anonymous.allow"])
        .assert()
        .success()
        .stdout(predicates::str::contains("\"value\":\"true\""));
}

// ============================================================================
// Test: Config migration preserves user customizations end-to-end
// ============================================================================

#[test]
fn test_config_customizations_persist_after_reinit() {
    let env = TestEnv::init();

    // Set custom values
    env.bn()
        .args(["config", "set", "git-bot.name", "my-custom-bot"])
        .assert()
        .success();
    env.bn()
        .args(["config", "set", "git-bot.email", "custom@bot.dev"])
        .assert()
        .success();
    env.bn()
        .args(["config", "set", "git.co-author.enabled", "false"])
        .assert()
        .success();
    env.bn()
        .args(["config", "set", "git.anonymous.allow", "false"])
        .assert()
        .success();

    // Verify values are set
    env.bn()
        .args(["config", "get", "git-bot.name"])
        .assert()
        .success()
        .stdout(predicates::str::contains("my-custom-bot"));
    env.bn()
        .args(["config", "get", "git-bot.email"])
        .assert()
        .success()
        .stdout(predicates::str::contains("custom@bot.dev"));
    env.bn()
        .args(["config", "get", "git.co-author.enabled"])
        .assert()
        .success()
        .stdout(predicates::str::contains("\"value\":\"false\""));
    env.bn()
        .args(["config", "get", "git.anonymous.allow"])
        .assert()
        .success()
        .stdout(predicates::str::contains("\"value\":\"false\""));

    // Running init again should NOT overwrite customizations (idempotent)
    env.bn()
        .args(["session", "init", "--auto-global", "-y"])
        .assert()
        .success();

    // Verify customizations are preserved
    env.bn()
        .args(["config", "get", "git-bot.name"])
        .assert()
        .success()
        .stdout(predicates::str::contains("my-custom-bot"));
    env.bn()
        .args(["config", "get", "git-bot.email"])
        .assert()
        .success()
        .stdout(predicates::str::contains("custom@bot.dev"));
}

#[test]
fn test_hook_behavior_with_customized_config() {
    let env = TestEnv::init();
    init_git(&env);

    // Customize bot identity
    env.bn()
        .args(["config", "set", "git-bot.name", "team-bot"])
        .assert()
        .success();
    env.bn()
        .args(["config", "set", "git-bot.email", "bot@team.io"])
        .assert()
        .success();

    // Create commit message
    let msg_file = env.repo_path().join("COMMIT_MSG");
    fs::write(&msg_file, "Commit with team bot\n").unwrap();

    // Run hook with agent session (not anonymous)
    let output = run_hook_with_env(&env, &msg_file, true, false);
    assert!(output.status.success());

    // Verify custom identity is used in trailer
    let content = fs::read_to_string(&msg_file).unwrap();
    assert!(
        content.contains("Co-authored-by: team-bot <bot@team.io>"),
        "Hook should use customized bot identity, got: {}",
        content
    );
}

// ============================================================================
// Test: No session means no modification (baseline)
// ============================================================================

#[test]
fn test_no_agent_session_leaves_message_unchanged() {
    let env = TestEnv::init();
    init_git(&env);

    // Create commit message
    let msg_file = env.repo_path().join("COMMIT_MSG");
    fs::write(&msg_file, "Regular commit\n").unwrap();

    // Run hook without agent session (anonymous flag doesn't matter)
    let output = run_hook_with_env(&env, &msg_file, false, false);
    assert!(output.status.success());

    let content = fs::read_to_string(&msg_file).unwrap();
    assert_eq!(content.trim(), "Regular commit");
}

#[test]
fn test_no_agent_session_with_anonymous_flag_leaves_unchanged() {
    let env = TestEnv::init();
    init_git(&env);

    // Create commit message
    let msg_file = env.repo_path().join("COMMIT_MSG");
    fs::write(&msg_file, "Another commit\n").unwrap();

    // Run hook without agent session but with anonymous flag (edge case)
    let output = run_hook_with_env(&env, &msg_file, false, true);
    assert!(output.status.success());

    let content = fs::read_to_string(&msg_file).unwrap();
    assert_eq!(content.trim(), "Another commit");
}
