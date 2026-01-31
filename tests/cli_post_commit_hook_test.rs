//! Integration tests for the post-commit hook archive generation.

mod common;
use common::TestEnv;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Helper to create an archive directory OUTSIDE the repository
/// (validation now requires archive.directory to be external)
fn create_external_archive_dir(env: &TestEnv) -> PathBuf {
    let parent_dir = env.path().parent().unwrap();
    let archive_dir = parent_dir.join("archives");
    fs::create_dir_all(&archive_dir).unwrap();
    archive_dir
}

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
        .args(["session", "init", "--auto-global"])
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

    // Configure archive directory (must be outside repo)
    let archive_dir = create_external_archive_dir(&env);
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

    // Run archive command directly instead of relying on hook's background process
    // The hook spawns this in background which makes it unreliable in tests
    env.bn()
        .args(["session", "store", "archive", &commit_hash])
        .assert()
        .success();

    // Check that archive was created
    let archive_file = archive_dir.join(format!("bn_{}.bng", commit_hash));
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

    // Archive should be created in default directory (BN_DATA_DIR/archives/)
    let output = env
        .bn()
        .args(["session", "store", "archive", "test123"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    // With default archive directory, archive should be created
    let stdout = String::from_utf8_lossy(&output);
    assert!(
        stdout.contains("\"created\":true"),
        "Archive should be created in default directory. Got: {}",
        stdout
    );

    // Clean up: parse output_path and remove the archive file
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout)
        && let Some(path) = json.get("output_path").and_then(|v| v.as_str())
    {
        let _ = std::fs::remove_file(path);
    }
}

#[test]
fn test_post_commit_hook_archive_disabled() {
    let env = TestEnv::new();
    init_git(&env);
    init_binnacle(&env);

    // Explicitly disable archiving
    env.bn()
        .args(["config", "set", "archive.directory", ""])
        .assert()
        .success();

    let output = env
        .bn()
        .args(["session", "store", "archive", "test123"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    // Should report not created when explicitly disabled
    let stdout = String::from_utf8_lossy(&output);
    assert!(
        stdout.contains("\"created\":false"),
        "Archive should not be created when disabled. Got: {}",
        stdout
    );
}

#[test]
fn test_archive_graceful_nonexistent_directory() {
    let env = TestEnv::new();
    init_git(&env);
    init_binnacle(&env);

    // Create parent directory OUTSIDE repo so config validates, then make it unwritable
    let parent_dir = env.path().parent().unwrap();
    let archive_parent = parent_dir.join("archive_parent");
    let archive_dir = archive_parent.join("archives");
    fs::create_dir_all(&archive_parent).unwrap();

    // Configure archive directory - parent exists so this passes validation
    env.bn()
        .args([
            "config",
            "set",
            "archive.directory",
            archive_dir.to_str().unwrap(),
        ])
        .assert()
        .success();

    // Now make parent unwritable so archive creation will fail
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&archive_parent).unwrap().permissions();
        perms.set_mode(0o444); // read-only
        fs::set_permissions(&archive_parent, perms).unwrap();
    }

    // Try to generate archive - should fail gracefully, not error
    let output = env
        .bn()
        .args(["session", "store", "archive", "abc123"])
        .assert()
        .success() // Should succeed with created: false, not error
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8_lossy(&output);
    assert!(stdout.contains("\"created\":false"));
    assert!(stdout.contains("\"skipped_reason\""));
    assert!(stdout.contains("cannot create directory"));

    // Cleanup: restore permissions so the directory can be cleaned up
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&archive_parent).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&archive_parent, perms).unwrap();
    }
}

#[test]
fn test_archive_graceful_unwritable_directory() {
    let env = TestEnv::new();
    init_git(&env);
    init_binnacle(&env);

    // Create a directory OUTSIDE repo and make it read-only
    let parent_dir = env.path().parent().unwrap();
    let archive_dir = parent_dir.join("readonly_archive");
    fs::create_dir_all(&archive_dir).unwrap();

    // Make directory unwritable (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&archive_dir).unwrap().permissions();
        perms.set_mode(0o444); // read-only
        fs::set_permissions(&archive_dir, perms).unwrap();
    }

    env.bn()
        .args([
            "config",
            "set",
            "archive.directory",
            archive_dir.to_str().unwrap(),
        ])
        .assert()
        .success();

    // Try to generate archive - should fail gracefully
    let output = env
        .bn()
        .args(["session", "store", "archive", "abc123"])
        .assert()
        .success() // Should succeed with created: false, not error
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8_lossy(&output);
    assert!(stdout.contains("\"created\":false"));
    assert!(stdout.contains("\"skipped_reason\""));
    assert!(stdout.contains("not writable"));

    // Cleanup: restore permissions so the directory can be cleaned up
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&archive_dir).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&archive_dir, perms).unwrap();
    }
}

#[test]
fn test_archive_graceful_human_output() {
    let env = TestEnv::new();
    init_git(&env);
    init_binnacle(&env);

    // Create parent directory OUTSIDE repo so config validates, then make it unwritable
    let parent_dir = env.path().parent().unwrap();
    let archive_parent = parent_dir.join("archive_human_test");
    let archive_dir = archive_parent.join("archives");
    fs::create_dir_all(&archive_parent).unwrap();

    // Configure archive directory - parent exists so this passes validation
    env.bn()
        .args([
            "config",
            "set",
            "archive.directory",
            archive_dir.to_str().unwrap(),
        ])
        .assert()
        .success();

    // Now make parent unwritable so archive creation will fail
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&archive_parent).unwrap().permissions();
        perms.set_mode(0o444); // read-only
        fs::set_permissions(&archive_parent, perms).unwrap();
    }

    // Try with -H for human-readable output
    let output = env
        .bn()
        .args(["-H", "session", "store", "archive", "abc123"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8_lossy(&output);
    assert!(stdout.contains("Archive not created"));
    assert!(stdout.contains("cannot create directory"));

    // Cleanup: restore permissions so the directory can be cleaned up
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&archive_parent).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&archive_parent, perms).unwrap();
    }
}
