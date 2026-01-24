//! Integration tests for the post-commit hook archive generation.

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

/// Helper to initialize binnacle
fn init_binnacle(env: &TestEnv) {
    env.bn()
        .args(["system", "init"])
        .write_stdin("n\nn\nn\nn\n")
        .assert()
        .success();
}

/// Helper to run the post-commit hook directly
fn run_hook(env: &TestEnv) -> std::process::Output {
    let hook_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("hooks/post-commit");

    // Get the directory containing the bn binary
    let bn_binary = env!("CARGO_BIN_EXE_bn");
    let bn_dir = Path::new(bn_binary).parent().unwrap();

    // Prepend bn directory to PATH so jq not being present doesn't affect results
    let path_env = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bn_dir.display(), path_env);

    Command::new(&hook_path)
        .current_dir(env.repo_path())
        .env("BN_DATA_DIR", env.data_path())
        .env("PATH", new_path)
        .output()
        .expect("Failed to run hook")
}

#[test]
fn test_post_commit_hook_no_archive_config() {
    let env = TestEnv::new();
    init_git(&env);
    init_binnacle(&env);

    // Make a commit first (so HEAD is valid)
    let test_file = env.path().join("test.txt");
    fs::write(&test_file, "test content").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(env.repo_path())
        .output()
        .expect("Failed to add file");
    Command::new("git")
        .args(["commit", "-m", "test commit"])
        .current_dir(env.repo_path())
        .output()
        .expect("Failed to commit");

    // Run hook without archive.directory configured - should succeed quietly
    let output = run_hook(&env);
    assert!(output.status.success(), "Hook should succeed");
}

#[test]
fn test_post_commit_hook_with_archive_config() {
    let env = TestEnv::new();
    init_git(&env);
    init_binnacle(&env);

    // Configure archive directory
    let archive_dir = env.path().join("archives");
    fs::create_dir_all(&archive_dir).unwrap();
    env.bn()
        .args([
            "config",
            "set",
            "archive.directory",
            archive_dir.to_str().unwrap(),
        ])
        .assert()
        .success();

    // Make a commit
    let test_file = env.path().join("test.txt");
    fs::write(&test_file, "test content").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(env.repo_path())
        .output()
        .expect("Failed to add file");
    Command::new("git")
        .args(["commit", "-m", "test commit"])
        .current_dir(env.repo_path())
        .output()
        .expect("Failed to commit");

    // Get the commit hash
    let commit_output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(env.repo_path())
        .output()
        .expect("Failed to get HEAD");
    let commit_hash = String::from_utf8_lossy(&commit_output.stdout)
        .trim()
        .to_string();

    // Run hook - should create archive
    let output = run_hook(&env);
    assert!(output.status.success(), "Hook should succeed");

    // Give the background process time to complete (hook runs archive in background)
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Check that archive was created
    let archive_file = archive_dir.join(format!("bn_{}.tar.gz", commit_hash));
    assert!(
        archive_file.exists(),
        "Archive file should exist at {}",
        archive_file.display()
    );
}

#[test]
fn test_post_commit_hook_install_function() {
    let env = TestEnv::new();
    init_git(&env);
    init_binnacle(&env);

    // Manually install the post-commit hook
    let output = env
        .bn()
        .args(["system", "store", "archive", "test123"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    // Should report not created (no config)
    let stdout = String::from_utf8_lossy(&output);
    assert!(stdout.contains("\"created\":false"));
}
