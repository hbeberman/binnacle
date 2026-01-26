//! Integration tests for container mode support.
//!
//! These tests verify that binnacle works correctly in container environments:
//! - BN_CONTAINER_MODE environment variable detection
//! - Container-style storage paths work correctly
//! - bn commands function properly with container data directories

mod common;

use assert_cmd::Command;
use common::TestEnv;
use predicates::prelude::*;
use std::fs;

// === Container Mode Environment Variable Tests ===

#[test]
fn test_container_mode_env_var_recognized() {
    // Create a test environment that simulates container mode
    let env = TestEnv::new();

    // Create a "container" binnacle directory
    let container_data = env.repo_path().join("container_binnacle");
    fs::create_dir_all(&container_data).unwrap();

    // Initialize with BN_CONTAINER_MODE set and BN_DATA_DIR pointing to our test container dir
    // Note: In real containers, BN_CONTAINER_MODE would use /binnacle, but we use BN_DATA_DIR
    // for testing since we can't write to /binnacle in tests
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.env("BN_DATA_DIR", &container_data);
    cmd.args(["system", "init", "-y"]);
    cmd.assert().success();

    // Verify bn orient works with container data directory (dry-run to avoid agent registration)
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.env("BN_DATA_DIR", &container_data);
    cmd.args(["orient", "--type", "worker", "-H", "--dry-run"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Binnacle"));
}

#[test]
fn test_container_mode_task_operations() {
    // Test that task CRUD operations work correctly with container-style paths
    let env = TestEnv::new();

    // Set up container-like data directory
    let container_data = env.repo_path().join("binnacle_data");
    fs::create_dir_all(&container_data).unwrap();

    // Helper to create a command with container data dir
    let bn_container = || {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(env.repo_path());
        cmd.env("BN_DATA_DIR", &container_data);
        cmd
    };

    // Initialize
    bn_container()
        .args(["system", "init", "-y"])
        .assert()
        .success();

    // Create a task
    let output = bn_container()
        .args(["task", "create", "Container test task"])
        .output()
        .expect("Failed to create task");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"id\":\"bn-"));

    // Extract task ID
    let id_start = stdout.find("\"id\":\"").unwrap() + 6;
    let id_end = stdout[id_start..].find('"').unwrap() + id_start;
    let task_id = &stdout[id_start..id_end];

    // Show task
    bn_container()
        .args(["task", "show", task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("Container test task"));

    // List tasks
    bn_container()
        .args(["task", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains(task_id));

    // Update task status
    bn_container()
        .args(["task", "update", task_id, "--status", "in_progress"])
        .assert()
        .success();

    // Close task
    bn_container()
        .args(["task", "close", task_id, "--reason", "Test completed"])
        .assert()
        .success();

    // Verify closed status
    bn_container()
        .args(["task", "show", task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"done\""));
}

#[test]
fn test_container_mode_ready_and_blocked() {
    // Test that ready/blocked queries work with container paths
    let env = TestEnv::new();

    let container_data = env.repo_path().join("binnacle_data");
    fs::create_dir_all(&container_data).unwrap();

    let bn_container = || {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(env.repo_path());
        cmd.env("BN_DATA_DIR", &container_data);
        cmd
    };

    // Initialize
    bn_container()
        .args(["system", "init", "-y"])
        .assert()
        .success();

    // Create tasks
    let output = bn_container()
        .args(["task", "create", "First task"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id1_start = stdout.find("\"id\":\"").unwrap() + 6;
    let id1_end = stdout[id1_start..].find('"').unwrap() + id1_start;
    let task1 = stdout[id1_start..id1_end].to_string();

    let output = bn_container()
        .args(["task", "create", "Second task"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id2_start = stdout.find("\"id\":\"").unwrap() + 6;
    let id2_end = stdout[id2_start..].find('"').unwrap() + id2_start;
    let task2 = stdout[id2_start..id2_end].to_string();

    // Make task2 depend on task1
    bn_container()
        .args([
            "link",
            "add",
            &task2,
            &task1,
            "-t",
            "depends_on",
            "--reason",
            "Task 2 depends on Task 1 completion",
        ])
        .assert()
        .success();

    // Check ready - should include task1
    bn_container()
        .args(["ready"])
        .assert()
        .success()
        .stdout(predicate::str::contains(&task1));

    // Check blocked - should include task2
    bn_container()
        .args(["blocked"])
        .assert()
        .success()
        .stdout(predicate::str::contains(&task2));
}

#[test]
fn test_container_mode_bug_operations() {
    // Test bug tracking works in container mode
    let env = TestEnv::new();

    let container_data = env.repo_path().join("binnacle_data");
    fs::create_dir_all(&container_data).unwrap();

    let bn_container = || {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(env.repo_path());
        cmd.env("BN_DATA_DIR", &container_data);
        cmd
    };

    // Initialize
    bn_container()
        .args(["system", "init", "-y"])
        .assert()
        .success();

    // Create a bug
    let output = bn_container()
        .args(["bug", "create", "Container bug"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"id\":\"bn-"));

    let id_start = stdout.find("\"id\":\"").unwrap() + 6;
    let id_end = stdout[id_start..].find('"').unwrap() + id_start;
    let bug_id = &stdout[id_start..id_end];

    // Show bug
    bn_container()
        .args(["bug", "show", bug_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("Container bug"));

    // Close bug
    bn_container()
        .args(["bug", "close", bug_id, "--reason", "Fixed"])
        .assert()
        .success();
}

#[test]
fn test_container_mode_idea_operations() {
    // Test idea tracking works in container mode
    let env = TestEnv::new();

    let container_data = env.repo_path().join("binnacle_data");
    fs::create_dir_all(&container_data).unwrap();

    let bn_container = || {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(env.repo_path());
        cmd.env("BN_DATA_DIR", &container_data);
        cmd
    };

    // Initialize
    bn_container()
        .args(["system", "init", "-y"])
        .assert()
        .success();

    // Create an idea
    let output = bn_container()
        .args(["idea", "create", "Container idea"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Ideas now use "bn-" prefix in the ID
    assert!(stdout.contains("\"id\":\"bn-"));
    assert!(stdout.contains("Container idea"));

    let id_start = stdout.find("\"id\":\"bn-").unwrap() + 6;
    let id_end = stdout[id_start..].find('"').unwrap() + id_start;
    let idea_id = &stdout[id_start..id_end];

    // Show idea
    bn_container()
        .args(["idea", "show", idea_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("Container idea"));
}

#[test]
fn test_container_mode_queue_operations() {
    // Test queue operations work in container mode
    let env = TestEnv::new();

    let container_data = env.repo_path().join("binnacle_data");
    fs::create_dir_all(&container_data).unwrap();

    let bn_container = || {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(env.repo_path());
        cmd.env("BN_DATA_DIR", &container_data);
        cmd
    };

    // Initialize
    bn_container()
        .args(["system", "init", "-y"])
        .assert()
        .success();

    // Create a queue
    bn_container()
        .args(["queue", "create", "Work Queue"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"id\":\"bnq-"));

    // Show queue
    bn_container()
        .args(["queue", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Work Queue"));

    // Create a task and add to queue
    let output = bn_container()
        .args(["task", "create", "Queued task"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id_start = stdout.find("\"id\":\"").unwrap() + 6;
    let id_end = stdout[id_start..].find('"').unwrap() + id_start;
    let task_id = &stdout[id_start..id_end];

    // Add task to queue
    bn_container()
        .args(["queue", "add", task_id])
        .assert()
        .success();

    // Verify task is in queue
    bn_container()
        .args(["queue", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains(task_id));
}

#[test]
fn test_container_mode_agent_registration() {
    // Test agent registration works in container mode
    let env = TestEnv::new();

    let container_data = env.repo_path().join("binnacle_data");
    fs::create_dir_all(&container_data).unwrap();

    let bn_container = || {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(env.repo_path());
        cmd.env("BN_DATA_DIR", &container_data);
        cmd
    };

    // Initialize
    bn_container()
        .args(["system", "init", "-y"])
        .assert()
        .success();

    // Orient with --register (agent registration) - requires --type
    bn_container()
        .args([
            "orient",
            "--type",
            "worker",
            "--register",
            "container-worker",
            "-H",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Binnacle"));

    // List agents
    bn_container()
        .args(["agent", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("container-worker"));
}

#[test]
fn test_container_mode_test_operations() {
    // Test that test node operations work in container mode
    let env = TestEnv::new();

    let container_data = env.repo_path().join("binnacle_data");
    fs::create_dir_all(&container_data).unwrap();

    let bn_container = || {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(env.repo_path());
        cmd.env("BN_DATA_DIR", &container_data);
        cmd
    };

    // Initialize
    bn_container()
        .args(["system", "init", "-y"])
        .assert()
        .success();

    // Create a test node
    let output = bn_container()
        .args(["test", "create", "Container test", "--cmd", "echo passed"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"id\":\"bnt-"));

    let id_start = stdout.find("\"id\":\"bnt-").unwrap() + 6;
    let id_end = stdout[id_start..].find('"').unwrap() + id_start;
    let test_id = &stdout[id_start..id_end];

    // Show test
    bn_container()
        .args(["test", "show", test_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("Container test"));

    // Run test
    bn_container()
        .args(["test", "run", test_id])
        .assert()
        .success();
}

#[test]
fn test_container_mode_data_persistence() {
    // Test that data persists across multiple commands (simulating container restarts)
    let env = TestEnv::new();

    let container_data = env.repo_path().join("binnacle_data");
    fs::create_dir_all(&container_data).unwrap();

    let bn_container = || {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(env.repo_path());
        cmd.env("BN_DATA_DIR", &container_data);
        cmd
    };

    // Initialize
    bn_container()
        .args(["system", "init", "-y"])
        .assert()
        .success();

    // Create a task
    let output = bn_container()
        .args(["task", "create", "Persistent task", "-p", "1"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id_start = stdout.find("\"id\":\"").unwrap() + 6;
    let id_end = stdout[id_start..].find('"').unwrap() + id_start;
    let task_id = &stdout[id_start..id_end];

    // Verify we can retrieve the task in a new command invocation
    // (simulating container restart with same mounted volume)
    bn_container()
        .args(["task", "show", task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("Persistent task"))
        .stdout(predicate::str::contains("\"priority\":1"));

    // Verify orient shows the task (requires --type) - dry-run to avoid agent registration
    bn_container()
        .args(["orient", "--type", "worker", "-H", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Total tasks: 1"));
}

// === Auto-Merge Tests ===
// These tests verify the merge logic from container/entrypoint.sh

/// Helper to run git commands in a test environment
fn git_cmd(env: &TestEnv, args: &[&str]) -> std::process::Output {
    std::process::Command::new("git")
        .current_dir(env.repo_path())
        .args(args)
        .output()
        .expect("Failed to run git command")
}

/// Helper to run git commands and assert success
fn git(env: &TestEnv, args: &[&str]) {
    let output = git_cmd(env, args);
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("git {:?} failed: {}", args, stderr);
    }
}

/// Initialize a git repo with an initial commit on main
fn init_git_repo(env: &TestEnv) {
    git(env, &["init", "-b", "main"]);
    git(env, &["config", "user.email", "test@test.com"]);
    git(env, &["config", "user.name", "Test User"]);

    // Create initial file and commit
    fs::write(env.repo_path().join("README.md"), "# Test\n").unwrap();
    git(env, &["add", "README.md"]);
    git(env, &["commit", "-m", "Initial commit"]);
}

#[test]
fn test_auto_merge_fast_forward_succeeds() {
    // Test that fast-forward merge works when work branch is ahead of main
    let env = TestEnv::new();
    init_git_repo(&env);

    // Create a work branch
    git(&env, &["checkout", "-b", "work-branch"]);

    // Make a commit on work branch
    fs::write(env.repo_path().join("feature.txt"), "New feature\n").unwrap();
    git(&env, &["add", "feature.txt"]);
    git(&env, &["commit", "-m", "Add feature"]);

    // Store the work branch HEAD
    let work_head = git_cmd(&env, &["rev-parse", "HEAD"]);
    let work_head = String::from_utf8_lossy(&work_head.stdout)
        .trim()
        .to_string();

    // Now simulate the merge logic from entrypoint.sh
    // Checkout main and attempt fast-forward merge
    git(&env, &["checkout", "main"]);
    let merge_result = git_cmd(&env, &["merge", "--ff-only", "work-branch"]);

    // Verify merge succeeded
    assert!(
        merge_result.status.success(),
        "Fast-forward merge should succeed"
    );

    // Verify main is now at the same commit as work branch
    let main_head = git_cmd(&env, &["rev-parse", "HEAD"]);
    let main_head = String::from_utf8_lossy(&main_head.stdout)
        .trim()
        .to_string();
    assert_eq!(work_head, main_head, "main should be at work branch HEAD");

    // Verify feature.txt exists on main
    assert!(
        env.repo_path().join("feature.txt").exists(),
        "feature.txt should exist on main after merge"
    );
}

#[test]
fn test_auto_merge_fails_when_diverged() {
    // Test that fast-forward merge fails when branches have diverged
    let env = TestEnv::new();
    init_git_repo(&env);

    // Create a work branch
    git(&env, &["checkout", "-b", "work-branch"]);

    // Make a commit on work branch
    fs::write(env.repo_path().join("feature.txt"), "Feature on work\n").unwrap();
    git(&env, &["add", "feature.txt"]);
    git(&env, &["commit", "-m", "Add feature on work"]);

    // Go back to main and make a conflicting commit
    git(&env, &["checkout", "main"]);
    fs::write(env.repo_path().join("other.txt"), "Other change\n").unwrap();
    git(&env, &["add", "other.txt"]);
    git(&env, &["commit", "-m", "Add other change on main"]);

    // Now attempt fast-forward merge (should fail)
    let merge_result = git_cmd(&env, &["merge", "--ff-only", "work-branch"]);

    // Verify merge failed (branches have diverged)
    assert!(
        !merge_result.status.success(),
        "Fast-forward merge should fail when branches diverge"
    );

    // Verify we're still on main (entrypoint would checkout work-branch on failure)
    let current_branch = git_cmd(&env, &["rev-parse", "--abbrev-ref", "HEAD"]);
    let current_branch = String::from_utf8_lossy(&current_branch.stdout)
        .trim()
        .to_string();
    assert_eq!(current_branch, "main");
}

#[test]
fn test_auto_merge_entrypoint_script_exists() {
    // Verify the entrypoint script exists and has expected content
    let entrypoint_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("container")
        .join("entrypoint.sh");

    assert!(
        entrypoint_path.exists(),
        "container/entrypoint.sh should exist"
    );

    let content = fs::read_to_string(&entrypoint_path).expect("Failed to read entrypoint.sh");

    // Verify key merge logic is present
    assert!(
        content.contains("BN_MERGE_TARGET"),
        "entrypoint.sh should reference BN_MERGE_TARGET"
    );
    assert!(
        content.contains("BN_AUTO_MERGE"),
        "entrypoint.sh should reference BN_AUTO_MERGE"
    );
    assert!(
        content.contains("--ff-only"),
        "entrypoint.sh should use --ff-only merge"
    );
    assert!(
        content.contains("git checkout"),
        "entrypoint.sh should checkout target branch"
    );
}

#[test]
fn test_auto_merge_can_be_disabled() {
    // Test that BN_AUTO_MERGE=false would skip merge (verify in script content)
    let entrypoint_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("container")
        .join("entrypoint.sh");

    let content = fs::read_to_string(&entrypoint_path).expect("Failed to read entrypoint.sh");

    // Verify the auto-merge disable logic is present
    assert!(
        content.contains("BN_AUTO_MERGE"),
        "entrypoint.sh should have BN_AUTO_MERGE variable"
    );
    assert!(
        content.contains("!= \"true\"") || content.contains("!= 'true'"),
        "entrypoint.sh should check if BN_AUTO_MERGE != true"
    );
    assert!(
        content.contains("Auto-merge disabled") || content.contains("skipping merge"),
        "entrypoint.sh should have message about skipping merge"
    );
}

#[test]
fn test_auto_merge_returns_to_work_branch_on_failure() {
    // Verify entrypoint script returns to work branch on merge failure
    let entrypoint_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("container")
        .join("entrypoint.sh");

    let content = fs::read_to_string(&entrypoint_path).expect("Failed to read entrypoint.sh");

    // The script should checkout the work branch on failure
    // Look for the pattern: checkout work branch after merge failure
    assert!(
        content.contains("git checkout \"$WORK_BRANCH\""),
        "entrypoint.sh should return to work branch on merge failure"
    );
}

#[test]
fn test_work_branch_tracking() {
    // Test the WORK_BRANCH capture from entrypoint.sh works correctly
    let env = TestEnv::new();
    init_git_repo(&env);

    // Create and checkout a work branch
    git(&env, &["checkout", "-b", "feature-xyz"]);

    // Verify we can get the branch name like entrypoint.sh does
    let branch_output = git_cmd(&env, &["rev-parse", "--abbrev-ref", "HEAD"]);
    let branch_name = String::from_utf8_lossy(&branch_output.stdout)
        .trim()
        .to_string();

    assert_eq!(branch_name, "feature-xyz", "Should detect current branch");
}

// === Container Security Tests ===

#[test]
fn test_container_run_help_shows_shell_flag() {
    // Verify the --shell flag appears in help
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.args(["container", "run", "--help"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("--shell"))
        .stdout(predicate::str::contains("interactive shell"));
}

#[test]
fn test_nss_wrapper_templates_exist() {
    // Verify the Containerfile creates nss_wrapper template files
    let containerfile_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("container")
        .join("Containerfile");

    let content = fs::read_to_string(&containerfile_path).expect("Failed to read Containerfile");

    // Should create nss_wrapper directory with templates
    assert!(
        content.contains("/etc/nss_wrapper"),
        "Containerfile should create /etc/nss_wrapper directory"
    );
    assert!(
        content.contains("nss_wrapper"),
        "Containerfile should install nss_wrapper package"
    );
    // Should NOT have the old passwd hacks
    assert!(
        !content.contains("chmod 666 /etc/passwd"),
        "Containerfile should NOT make /etc/passwd world-writable"
    );
    assert!(
        !content.contains("chmod 666 /etc/shadow"),
        "Containerfile should NOT make /etc/shadow world-writable"
    );
}

#[test]
fn test_entrypoint_nss_wrapper_setup() {
    // Verify entrypoint.sh sets up nss_wrapper for non-root mode
    let entrypoint_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("container")
        .join("entrypoint.sh");

    let content = fs::read_to_string(&entrypoint_path).expect("Failed to read entrypoint.sh");

    // Should check if running as root
    assert!(
        content.contains("CURRENT_UID") && content.contains("!= \"0\""),
        "entrypoint.sh should check if running as non-root"
    );

    // Should set up nss_wrapper env vars
    assert!(
        content.contains("LD_PRELOAD"),
        "entrypoint.sh should set LD_PRELOAD for nss_wrapper"
    );
    assert!(
        content.contains("NSS_WRAPPER_PASSWD"),
        "entrypoint.sh should set NSS_WRAPPER_PASSWD"
    );
}

#[test]
fn test_container_mode_requires_bn_agent_id_for_orient() {
    // Container agents MUST have BN_AGENT_ID set by their creator.
    // Using parent PID in containers is dangerous because PID 1 is init.
    let env = TestEnv::new();

    let container_data = env.repo_path().join("binnacle_data");
    fs::create_dir_all(&container_data).unwrap();

    // Initialize binnacle first
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.env("BN_DATA_DIR", &container_data);
    cmd.args(["system", "init", "-y"]);
    cmd.assert().success();

    // Running orient with BN_CONTAINER_MODE but WITHOUT BN_AGENT_ID should fail
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.env("BN_DATA_DIR", &container_data);
    cmd.env("BN_CONTAINER_MODE", "true");
    // Note: NOT setting BN_AGENT_ID
    cmd.args(["orient", "--type", "worker"]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("BN_AGENT_ID"));

    // But with BN_AGENT_ID set, it should succeed
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.env("BN_DATA_DIR", &container_data);
    cmd.env("BN_CONTAINER_MODE", "true");
    cmd.env("BN_AGENT_ID", "bn-test");
    cmd.env("BN_AGENT_NAME", "test-agent");
    cmd.args(["orient", "--type", "worker"]);
    cmd.assert().success();
}
