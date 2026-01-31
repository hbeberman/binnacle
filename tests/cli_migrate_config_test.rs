//! Integration tests for `bn system migrate-config` command.
//!
//! These tests verify token migration from config.kdl to state.kdl:
//! - Detecting legacy tokens in config.kdl
//! - Dry run mode
//! - Actual migration
//! - Handling when no legacy tokens exist

mod common;

use common::TestEnv;
use predicates::prelude::*;
use std::fs;
use std::path::Path;

/// Write a KDL file with a token
fn write_config_with_token(path: &Path, token: &str) {
    let content = format!(
        r#"// Config file
editor "vim"
github-token "{}"
output-format "json"
"#,
        token
    );
    fs::write(path, content).expect("Failed to write config file");
}

/// Write a KDL state file with a token
fn write_state_with_token(path: &Path, token: &str) {
    let content = format!(
        r#"// State file
github-token "{}"
"#,
        token
    );
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("Failed to create parent directory");
    }
    fs::write(path, content).expect("Failed to write state file");
}

/// Read file content as string
fn read_file(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

// ============================================================================
// bn system migrate-config Tests
// ============================================================================

#[test]
fn test_migrate_config_no_legacy_tokens() {
    let env = TestEnv::new();

    // Initialize binnacle (no legacy tokens)
    env.bn()
        .args(["session", "init", "--auto-global", "-y"])
        .assert()
        .success();

    // Run migrate-config - should find nothing
    env.bn()
        .args(["system", "migrate-config", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No legacy tokens found"));
}

#[test]
fn test_migrate_config_dry_run_detects_token() {
    let env = TestEnv::new();

    // Initialize binnacle
    env.bn()
        .args(["session", "init", "--auto-global", "-y"])
        .assert()
        .success();

    // Get the system config path and add a legacy token
    let config_dir = env.data_path().join("binnacle");
    fs::create_dir_all(&config_dir).expect("Failed to create config dir");
    let config_path = config_dir.join("config.kdl");
    write_config_with_token(&config_path, "ghp_legacy_test_token_12345");

    // Run dry run - should detect the token (last 4 chars of token are "2345")
    env.bn()
        .args(["system", "migrate-config", "--dry-run", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("DRY RUN"))
        .stdout(predicate::str::contains("ghp_...2345"))
        .stdout(predicate::str::contains("Run without --dry-run"));

    // Verify token is still in config.kdl (not migrated)
    let content = read_file(&config_path);
    assert!(
        content.contains("github-token"),
        "Token should still be in config.kdl after dry run"
    );
}

#[test]
fn test_migrate_config_moves_token() {
    let env = TestEnv::new();

    // Initialize binnacle
    env.bn()
        .args(["session", "init", "--auto-global", "-y"])
        .assert()
        .success();

    // Get the system config path and add a legacy token
    // Note: system_config_kdl_path uses BN_CONFIG_DIR/binnacle/config.kdl
    let config_dir = env.data_path().join("binnacle");
    fs::create_dir_all(&config_dir).expect("Failed to create config dir");
    let config_path = config_dir.join("config.kdl");
    write_config_with_token(&config_path, "ghp_migrate_me_token123");

    // Verify token is in config.kdl
    let content = read_file(&config_path);
    assert!(content.contains("github-token"));

    // Run actual migration (last 4 chars of token are "n123")
    env.bn()
        .args(["system", "migrate-config", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Migrated"))
        .stdout(predicate::str::contains("ghp_...n123"));

    // Verify token is removed from config.kdl
    let content = read_file(&config_path);
    assert!(
        !content.contains("github-token"),
        "Token should be removed from config.kdl after migration"
    );

    // Verify other config values are preserved
    assert!(content.contains("editor"));
    assert!(content.contains("output-format"));

    // Verify token is in state.kdl
    // Note: system_state_kdl_path uses BN_DATA_DIR/state.kdl (not in binnacle subdir)
    let state_path = env.data_path().join("state.kdl");
    let state_content = read_file(&state_path);
    assert!(
        state_content.contains("github-token"),
        "Token should be in state.kdl after migration"
    );
}

#[test]
fn test_migrate_config_skips_if_state_has_token() {
    let env = TestEnv::new();

    // Initialize binnacle
    env.bn()
        .args(["session", "init", "--auto-global", "-y"])
        .assert()
        .success();

    // Get paths
    // Note: system_config_kdl_path uses BN_CONFIG_DIR/binnacle/config.kdl
    // Note: system_state_kdl_path uses BN_DATA_DIR/state.kdl
    let config_dir = env.data_path().join("binnacle");
    fs::create_dir_all(&config_dir).expect("Failed to create config dir");
    let config_path = config_dir.join("config.kdl");
    let state_path = env.data_path().join("state.kdl");

    // Add legacy token to config.kdl (last 4 chars are "d123")
    write_config_with_token(&config_path, "ghp_legacy_token_old123");

    // Add existing token to state.kdl
    write_state_with_token(&state_path, "ghp_existing_token_new");

    // Run migration - should warn about existing token and show masked token
    env.bn()
        .args(["system", "migrate-config", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("already has a token"))
        .stdout(predicate::str::contains("ghp_...d123"));

    // Verify state.kdl still has the original token (not overwritten)
    let state_content = read_file(&state_path);
    assert!(
        state_content.contains("ghp_existing_token_new"),
        "Original token in state.kdl should be preserved"
    );
    assert!(
        !state_content.contains("ghp_legacy_token_old123"),
        "Legacy token should not overwrite existing token"
    );

    // Verify legacy token is still removed from config.kdl
    let config_content = read_file(&config_path);
    assert!(
        !config_content.contains("github-token"),
        "Token should be removed from config.kdl even when not migrated"
    );
}

#[test]
fn test_migrate_config_json_output() {
    let env = TestEnv::new();

    // Initialize binnacle
    env.bn()
        .args(["session", "init", "--auto-global", "-y"])
        .assert()
        .success();

    // Get the system config path and add a legacy token
    let config_dir = env.data_path().join("binnacle");
    fs::create_dir_all(&config_dir).expect("Failed to create config dir");
    let config_path = config_dir.join("config.kdl");
    write_config_with_token(&config_path, "ghp_json_test_token567");

    // Run migration with JSON output
    let output = env
        .bn()
        .args(["system", "migrate-config", "--dry-run"])
        .output()
        .expect("Failed to run command");

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Invalid JSON output");

    assert_eq!(json["success"], true);
    assert_eq!(json["dry_run"], true);
    assert!(json["legacy_tokens_found"].is_array());
    assert!(!json["legacy_tokens_found"].as_array().unwrap().is_empty());
}
