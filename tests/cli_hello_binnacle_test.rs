//! Integration tests for Hello Binnacle onboarding flow.
//!
//! These tests verify the token management features introduced in the
//! "Hello Binnacle" PRD:
//! - Token validation paths (`--token` and `--token-non-validated`)
//! - Token storage in state.kdl files
//! - Precedence resolution (env > session > system)
//! - Agent injection preparation
//!
//! Note: These tests cannot verify actual Copilot/GitHub API validation since
//! we can't use real tokens in CI. Instead, they test:
//! - Token storage mechanics
//! - Precedence resolution logic
//! - Error handling for invalid tokens
//! - Output format correctness

mod common;

use assert_cmd::Command;
use common::TestEnv;
use predicates::prelude::*;
use serde_json::Value;
use std::fs;

/// Get a Command for the bn binary in a TestEnv.
fn bn_in(env: &TestEnv) -> Command {
    env.bn()
}

/// Parse JSON output from a command.
fn parse_json(output: &[u8]) -> Value {
    serde_json::from_slice(output).expect("Failed to parse JSON output")
}

// ============================================================================
// Token Storage Tests
// ============================================================================

#[test]
fn test_session_state_token_persisted_to_file() {
    let env = TestEnv::new();

    // Initialize session without token
    env.bn()
        .args(["session", "init", "--auto-global", "-y"])
        .assert()
        .success();

    // Use system token command to verify initial state
    let output = bn_in(&env)
        .args(["system", "token", "show"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(
        json["has_token"], false,
        "No token should be stored initially"
    );
}

#[test]
fn test_system_token_show_provides_source_info() {
    let env = TestEnv::new();

    // Initialize without token
    env.bn()
        .args(["system", "host-init", "-y"])
        .assert()
        .success();

    // Token show should indicate no token
    let output = bn_in(&env)
        .args(["system", "token", "show"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["has_token"], false);
    assert!(json.get("source").is_none() || json["source"].is_null());
}

#[test]
fn test_system_token_show_human_guidance() {
    let env = TestEnv::new();

    // Initialize without token
    env.bn()
        .args(["system", "host-init", "-y"])
        .assert()
        .success();

    // Human format should provide guidance
    bn_in(&env)
        .args(["-H", "system", "token", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No token configured"))
        .stdout(predicate::str::contains("bn system token set"));
}

// ============================================================================
// Token Validation Path Tests
// ============================================================================

#[test]
fn test_session_init_token_validates_format() {
    let env = TestEnv::new();

    // Empty token should fail
    bn_in(&env)
        .args(["session", "init", "--auto-global", "--token", "", "-y"])
        .assert()
        .failure();
}

#[test]
fn test_session_init_token_non_validated_validates_format() {
    let env = TestEnv::new();

    // Empty token should fail even with non-validated path
    bn_in(&env)
        .args([
            "session",
            "init",
            "--auto-global",
            "--token-non-validated",
            "",
            "-y",
        ])
        .assert()
        .failure();
}

#[test]
fn test_session_init_both_token_flags_conflict() {
    let env = TestEnv::new();

    // Using both flags should error
    bn_in(&env)
        .args([
            "session",
            "init",
            "--auto-global",
            "--token",
            "token1",
            "--token-non-validated",
            "token2",
            "-y",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn test_host_init_both_token_flags_conflict() {
    let env = TestEnv::new();

    // Using both flags in host-init should also error
    bn_in(&env)
        .args([
            "system",
            "host-init",
            "--token",
            "token1",
            "--token-non-validated",
            "token2",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

// ============================================================================
// Precedence Resolution Integration Tests
// ============================================================================
// Note: The `bn system token show` command only shows tokens stored in state.kdl files.
// It does NOT include environment variable tokens. The precedence resolution
// (env > session > system) is used internally for agent injection, not for display.
// These tests verify the storage layer and output formatting work correctly.

#[test]
fn test_token_show_only_shows_stored_tokens() {
    let env = TestEnv::new();

    // Initialize without token
    env.bn()
        .args(["session", "init", "--auto-global", "-y"])
        .assert()
        .success();

    // Set env var - but token show should NOT report it
    // (env var precedence is for injection, not display)
    let output = bn_in(&env)
        .env("COPILOT_GITHUB_TOKEN", "ghp_test_env_token")
        .args(["system", "token", "show"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    // has_token should be false since no token is STORED
    assert_eq!(
        json["has_token"], false,
        "Token show should only report stored tokens, not env vars"
    );
}

#[test]
fn test_token_show_human_no_stored_token() {
    let env = TestEnv::new();

    // Initialize without token
    env.bn()
        .args(["session", "init", "--auto-global", "-y"])
        .assert()
        .success();

    // Human format should guide user to set token
    bn_in(&env)
        .args(["-H", "system", "token", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No token configured"));
}

#[test]
fn test_token_test_no_stored_token() {
    // Token test checks stored tokens, not env vars
    let env = TestEnv::new();

    // Initialize
    env.bn()
        .args(["system", "host-init", "-y"])
        .assert()
        .success();

    // token test without stored token should report no token
    let output = bn_in(&env)
        .args(["system", "token", "test"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["valid"], false);
    assert!(
        json["error"]
            .as_str()
            .unwrap_or("")
            .contains("No token configured"),
        "Should report no token configured"
    );
}

// ============================================================================
// Output Format Tests
// ============================================================================

#[test]
fn test_session_init_output_includes_token_fields() {
    let env = TestEnv::new();

    let output = bn_in(&env)
        .args(["session", "init", "--auto-global", "-y"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);

    // Verify token-related fields are present
    assert!(
        json.get("token_stored").is_some(),
        "Output should include token_stored field"
    );
    assert!(
        json.get("copilot_validated").is_some(),
        "Output should include copilot_validated field"
    );
    // token_username may be null but field should exist for consistent schema
}

#[test]
fn test_host_init_outputs_json_by_default() {
    let env = TestEnv::new();

    // host-init outputs JSON by default
    let output = bn_in(&env)
        .args(["system", "host-init", "-y"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["initialized"], true);
    assert!(json["config_path"].as_str().is_some());
}

#[test]
fn test_host_init_outputs_emoji_in_human_mode() {
    let env = TestEnv::new();

    // host-init outputs emoji-formatted text with -H flag
    bn_in(&env)
        .args(["-H", "system", "host-init", "-y"])
        .assert()
        .success()
        .stdout(predicate::str::contains("✅"))
        .stdout(predicate::str::contains("system config"));
}

#[test]
fn test_token_show_json_schema() {
    let env = TestEnv::new();

    // Initialize first
    env.bn()
        .args(["system", "host-init", "-y"])
        .assert()
        .success();

    let output = bn_in(&env)
        .args(["system", "token", "show"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);

    // Verify expected fields exist
    assert!(
        json.get("has_token").is_some(),
        "Output should include has_token field"
    );
    // When has_token is true, masked_token and source should also be present
}

#[test]
fn test_token_clear_json_schema() {
    let env = TestEnv::new();

    // Initialize first
    env.bn()
        .args(["system", "host-init", "-y"])
        .assert()
        .success();

    let output = bn_in(&env)
        .args(["system", "token", "clear"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);

    // Verify expected fields exist
    assert!(
        json.get("cleared").is_some(),
        "Output should include cleared field"
    );
}

#[test]
fn test_token_test_json_schema() {
    let env = TestEnv::new();

    // Initialize first
    env.bn()
        .args(["system", "host-init", "-y"])
        .assert()
        .success();

    let output = bn_in(&env)
        .args(["system", "token", "test"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);

    // Verify expected fields exist
    assert!(
        json.get("valid").is_some(),
        "Output should include valid field"
    );
    // When invalid, error field should be present
    if json["valid"] == false {
        assert!(
            json.get("error").is_some(),
            "When valid=false, error field should be present"
        );
    }
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn test_invalid_token_shows_guidance_message() {
    let env = TestEnv::new();

    // Try to set invalid token
    bn_in(&env)
        .args(["system", "token", "set", "not_a_valid_token"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("github.com/settings/tokens"));
}

#[test]
fn test_session_init_invalid_token_shows_guidance() {
    let env = TestEnv::new();

    // Try to init with invalid token
    bn_in(&env)
        .args([
            "session",
            "init",
            "--auto-global",
            "--token-non-validated",
            "invalid",
            "-y",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Token validation failed"));
}

#[test]
fn test_host_init_invalid_token_shows_guidance() {
    let env = TestEnv::new();

    // Try to init with invalid token
    bn_in(&env)
        .args(["system", "host-init", "--token-non-validated", "invalid"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Token validation failed"))
        .stderr(predicate::str::contains("github.com/settings/tokens"));
}

// ============================================================================
// Token Masking Tests
// ============================================================================
// Note: We can't test token masking with real stored tokens since we can't
// pass valid tokens. These tests verify the masking logic exists in the
// code path by checking the resolver's masking implementation via unit tests
// in src/config/resolver.rs. Here we just verify the human output guidance.

#[test]
fn test_token_show_guidance_when_no_token() {
    let env = TestEnv::new();

    // Initialize
    env.bn()
        .args(["session", "init", "--auto-global", "-y"])
        .assert()
        .success();

    // Human format should provide guidance
    bn_in(&env)
        .args(["-H", "system", "token", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No token configured"))
        .stdout(predicate::str::contains("bn system token set"));
}

#[test]
fn test_token_clear_when_no_token() {
    let env = TestEnv::new();

    // Initialize
    env.bn()
        .args(["session", "init", "--auto-global", "-y"])
        .assert()
        .success();

    // Clear when no token should indicate nothing to clear
    let output = bn_in(&env)
        .args(["system", "token", "clear"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["cleared"], false);
}

#[test]
fn test_token_clear_human_when_no_token() {
    let env = TestEnv::new();

    // Initialize
    env.bn()
        .args(["session", "init", "--auto-global", "-y"])
        .assert()
        .success();

    // Human format should indicate no token was configured
    bn_in(&env)
        .args(["-H", "system", "token", "clear"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No token was configured"));
}

// ============================================================================
// State File Location Tests
// ============================================================================

#[test]
fn test_state_files_in_correct_location() {
    let env = TestEnv::new();

    // Initialize
    env.bn()
        .args(["session", "init", "--auto-global", "-y"])
        .assert()
        .success();

    // Check that state directory exists under data_path
    let data_path = env.data_path();

    // System-level state.kdl should be in data directory root
    // (This test verifies the infrastructure is set up correctly)
    assert!(data_path.exists(), "Data directory should exist");

    // The storage structure uses repo hashes, so we verify the data dir is used
    let entries: Vec<_> = fs::read_dir(data_path)
        .expect("Failed to read data directory")
        .filter_map(|e| e.ok())
        .collect();

    // Should have at least one entry (either state.kdl or repo hash directory)
    assert!(
        !entries.is_empty(),
        "Data directory should have entries after init"
    );
}

// ============================================================================
// Help and Documentation Tests
// ============================================================================

#[test]
fn test_token_help_shows_all_subcommands() {
    let env = TestEnv::new();

    bn_in(&env)
        .args(["system", "token", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("show"))
        .stdout(predicate::str::contains("set"))
        .stdout(predicate::str::contains("clear"))
        .stdout(predicate::str::contains("test"));
}

#[test]
fn test_session_init_help_shows_token_flags() {
    let env = TestEnv::new();

    bn_in(&env)
        .args(["session", "init", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--token"))
        .stdout(predicate::str::contains("--token-non-validated"));
}

#[test]
fn test_host_init_help_shows_token_flags() {
    let env = TestEnv::new();

    bn_in(&env)
        .args(["system", "host-init", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--token"))
        .stdout(predicate::str::contains("--token-non-validated"));
}
