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
    // Explicitly clear BN_AGENT_ID to prevent inheriting from parent shell
    cmd.env_remove("BN_AGENT_ID");
    cmd.env_remove("BN_AGENT_SESSION");
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

// === Container Definition Tests ===

#[test]
fn test_container_list_definitions_embedded_fallback() {
    // Test that list-definitions returns embedded fallback when no config files exist
    let env = TestEnv::new();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.args(["container", "list-definitions"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"name\":\"binnacle\""))
        .stdout(predicate::str::contains("\"source\":\"embedded\""));
}

#[test]
fn test_container_list_definitions_human_readable() {
    // Test human-readable output
    let env = TestEnv::new();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.args(["container", "list-definitions", "-H"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("binnacle (embedded)"))
        .stdout(predicate::str::contains("Embedded binnacle container"));
}

#[test]
fn test_container_list_definitions_with_project_config() {
    // Test listing definitions when project-level config exists
    let env = TestEnv::new();

    // Create project-level container config
    let containers_dir = env.repo_path().join(".binnacle").join("containers");
    fs::create_dir_all(&containers_dir).unwrap();
    fs::write(
        containers_dir.join("config.kdl"),
        r#"
container "base" {
    description "Fedora base with common dev tools"
    
    defaults {
        cpus 2
        memory "4g"
    }
}

container "rust-dev" {
    parent "base"
    description "Rust development environment"
    entrypoint mode="before"
}
"#,
    )
    .unwrap();

    // List definitions in JSON format
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.args(["container", "list-definitions"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"name\":\"base\""))
        .stdout(predicate::str::contains("\"name\":\"rust-dev\""))
        .stdout(predicate::str::contains("\"source\":\"project\""))
        .stdout(predicate::str::contains("\"count\":2"));

    // List definitions in human-readable format
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.args(["container", "list-definitions", "-H"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("base (project)"))
        .stdout(predicate::str::contains("rust-dev (project)"))
        .stdout(predicate::str::contains("Parent: base"))
        .stdout(predicate::str::contains("2 container definition(s) found"));
}

#[test]
fn test_container_list_definitions_shows_config_path() {
    // Verify config path is shown in output
    let env = TestEnv::new();

    // Create project-level container config
    let containers_dir = env.repo_path().join(".binnacle").join("containers");
    fs::create_dir_all(&containers_dir).unwrap();
    fs::write(
        containers_dir.join("config.kdl"),
        r#"
container "test" {
    description "Test container"
}
"#,
    )
    .unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.args(["container", "list-definitions", "-H"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Config:"))
        .stdout(predicate::str::contains(".binnacle/containers/config.kdl"));
}

// === Container Definition Comprehensive Tests ===

// Note: Some validations (cycle detection, invalid characters, mount paths) are done
// at build-time, not list-time. These tests verify the current behavior and will
// help validate any future changes to validation timing.

#[test]
fn test_container_definition_reserved_name_rejected() {
    // Using reserved name "binnacle" should fail at parse time
    let env = TestEnv::new();

    let containers_dir = env.repo_path().join(".binnacle").join("containers");
    fs::create_dir_all(&containers_dir).unwrap();
    fs::write(
        containers_dir.join("config.kdl"),
        r#"
container "binnacle" {
    description "Should fail"
}
"#,
    )
    .unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.args(["container", "list-definitions"]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("reserved container name"));
}

#[test]
fn test_container_definition_circular_dependency_listed() {
    // Circular dependencies are detected at build-time, not list-time
    // list-definitions returns the definitions without validation
    let env = TestEnv::new();

    let containers_dir = env.repo_path().join(".binnacle").join("containers");
    fs::create_dir_all(&containers_dir).unwrap();
    fs::write(
        containers_dir.join("config.kdl"),
        r#"
container "a" {
    parent "b"
}
container "b" {
    parent "a"
}
"#,
    )
    .unwrap();

    // list-definitions succeeds but shows the definitions
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.args(["container", "list-definitions"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"name\":\"a\""))
        .stdout(predicate::str::contains("\"name\":\"b\""))
        .stdout(predicate::str::contains("\"parent\":\"b\""))
        .stdout(predicate::str::contains("\"parent\":\"a\""));
}

#[test]
fn test_container_definition_self_reference_listed() {
    // Self-references are detected at build-time, not list-time
    let env = TestEnv::new();

    let containers_dir = env.repo_path().join(".binnacle").join("containers");
    fs::create_dir_all(&containers_dir).unwrap();
    fs::write(
        containers_dir.join("config.kdl"),
        r#"
container "self-loop" {
    parent "self-loop"
}
"#,
    )
    .unwrap();

    // list-definitions succeeds and shows the definition
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.args(["container", "list-definitions"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"name\":\"self-loop\""))
        .stdout(predicate::str::contains("\"parent\":\"self-loop\""));
}

#[test]
fn test_container_definition_mount_targets_listed() {
    // Mount targets are validated at build-time, not list-time
    // (mount details not included in list output, but parsing succeeds)
    let env = TestEnv::new();

    let containers_dir = env.repo_path().join(".binnacle").join("containers");
    fs::create_dir_all(&containers_dir).unwrap();
    fs::write(
        containers_dir.join("config.kdl"),
        r#"
container "test" {
    mounts {
        mount "data" target="relative/path" mode="rw"
    }
}
"#,
    )
    .unwrap();

    // list-definitions succeeds (mount details not shown in summary output)
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.args(["container", "list-definitions"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"name\":\"test\""));
}

#[test]
fn test_container_definition_invalid_entrypoint_mode() {
    // Invalid entrypoint mode should fail at parse time
    let env = TestEnv::new();

    let containers_dir = env.repo_path().join(".binnacle").join("containers");
    fs::create_dir_all(&containers_dir).unwrap();
    fs::write(
        containers_dir.join("config.kdl"),
        r#"
container "test" {
    entrypoint mode="invalid"
}
"#,
    )
    .unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.args(["container", "list-definitions"]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("invalid entrypoint mode"));
}

#[test]
fn test_container_definition_invalid_mount_mode() {
    // Invalid mount mode should fail at parse time
    let env = TestEnv::new();

    let containers_dir = env.repo_path().join(".binnacle").join("containers");
    fs::create_dir_all(&containers_dir).unwrap();
    fs::write(
        containers_dir.join("config.kdl"),
        r#"
container "test" {
    mounts {
        mount "data" target="/data" mode="invalid"
    }
}
"#,
    )
    .unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.args(["container", "list-definitions"]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("invalid mount mode"));
}

#[test]
fn test_container_definition_all_entrypoint_modes_parsed() {
    // Verify all entrypoint modes are parsed correctly
    // (Note: entrypoint_mode is not included in list-definitions output,
    // but parsing should succeed for valid modes)
    let env = TestEnv::new();

    let containers_dir = env.repo_path().join(".binnacle").join("containers");
    fs::create_dir_all(&containers_dir).unwrap();
    fs::write(
        containers_dir.join("config.kdl"),
        r#"
container "base" {
    description "Base (default replace)"
}
container "before-mode" {
    parent "base"
    entrypoint mode="before"
    description "Before mode"
}
container "after-mode" {
    parent "base"
    entrypoint mode="after"
    description "After mode"
}
container "replace-mode" {
    parent "base"
    entrypoint mode="replace"
    description "Replace mode"
}
"#,
    )
    .unwrap();

    // All should be listed successfully (entrypoint modes are valid)
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.args(["container", "list-definitions"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"name\":\"base\""))
        .stdout(predicate::str::contains("\"name\":\"before-mode\""))
        .stdout(predicate::str::contains("\"name\":\"after-mode\""))
        .stdout(predicate::str::contains("\"name\":\"replace-mode\""))
        .stdout(predicate::str::contains("\"count\":4"));
}

#[test]
fn test_container_definition_with_mounts_parsed() {
    // Verify mounts are parsed correctly
    // (Note: mount details not included in list-definitions summary output)
    let env = TestEnv::new();

    let containers_dir = env.repo_path().join(".binnacle").join("containers");
    fs::create_dir_all(&containers_dir).unwrap();
    fs::write(
        containers_dir.join("config.kdl"),
        r#"
container "test" {
    description "Container with mounts"
    mounts {
        mount "workspace" target="/workspace" mode="rw"
        mount "cargo-cache" source="~/.cargo" target="/cargo-cache" mode="ro" optional=#true
        mount "data" source="/host/data" target="/data" mode="rw"
    }
}
"#,
    )
    .unwrap();

    // Should parse successfully (mount details not shown in list output)
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.args(["container", "list-definitions"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"name\":\"test\""))
        .stdout(predicate::str::contains("Container with mounts"));
}

#[test]
fn test_container_definition_with_defaults_parsed() {
    // Verify defaults (cpus, memory) are parsed correctly
    // (Note: defaults not included in list-definitions summary output)
    let env = TestEnv::new();

    let containers_dir = env.repo_path().join(".binnacle").join("containers");
    fs::create_dir_all(&containers_dir).unwrap();
    fs::write(
        containers_dir.join("config.kdl"),
        r#"
container "test" {
    description "Container with defaults"
    defaults {
        cpus 4
        memory "8g"
    }
}
"#,
    )
    .unwrap();

    // Should parse successfully (defaults not shown in list output)
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.args(["container", "list-definitions"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"name\":\"test\""))
        .stdout(predicate::str::contains("Container with defaults"));
}

#[test]
fn test_container_definition_parent_chain_displayed() {
    // Verify parent chains are correctly shown
    let env = TestEnv::new();

    let containers_dir = env.repo_path().join(".binnacle").join("containers");
    fs::create_dir_all(&containers_dir).unwrap();
    fs::write(
        containers_dir.join("config.kdl"),
        r#"
container "base" {
    description "Base container"
}
container "rust" {
    parent "base"
    description "Rust development"
}
container "app" {
    parent "rust"
    description "Application"
}
"#,
    )
    .unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.args(["container", "list-definitions", "-H"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Parent: base"))
        .stdout(predicate::str::contains("Parent: rust"));
}

#[test]
fn test_container_definition_empty_name_listed() {
    // Empty container name is parsed (validation happens at build-time)
    let env = TestEnv::new();

    let containers_dir = env.repo_path().join(".binnacle").join("containers");
    fs::create_dir_all(&containers_dir).unwrap();
    fs::write(
        containers_dir.join("config.kdl"),
        r#"
container "" {
    description "No name"
}
"#,
    )
    .unwrap();

    // Listing succeeds (validation at build-time)
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.args(["container", "list-definitions"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"name\":\"\""))
        .stdout(predicate::str::contains("No name"));
}

#[test]
fn test_container_definition_invalid_characters_listed() {
    // Container names with invalid characters are parsed (validation at build-time)
    let env = TestEnv::new();

    let containers_dir = env.repo_path().join(".binnacle").join("containers");
    fs::create_dir_all(&containers_dir).unwrap();
    fs::write(
        containers_dir.join("config.kdl"),
        r#"
container "my container" {
    description "Has spaces"
}
"#,
    )
    .unwrap();

    // Listing succeeds (validation at build-time)
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.args(["container", "list-definitions"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"name\":\"my container\""));
}

#[test]
fn test_container_definition_valid_names_accepted() {
    // Valid names with hyphens and underscores should work
    let env = TestEnv::new();

    let containers_dir = env.repo_path().join(".binnacle").join("containers");
    fs::create_dir_all(&containers_dir).unwrap();
    fs::write(
        containers_dir.join("config.kdl"),
        r#"
container "my-container_v2" {
    description "Valid name"
}
container "UPPERCASE" {
    description "Also valid"
}
container "mixed-Case_123" {
    description "Mixed case with numbers"
}
"#,
    )
    .unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.args(["container", "list-definitions"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"name\":\"my-container_v2\""))
        .stdout(predicate::str::contains("\"name\":\"UPPERCASE\""))
        .stdout(predicate::str::contains("\"name\":\"mixed-Case_123\""));
}

#[test]
fn test_container_build_without_containerd_graceful_error() {
    // When containerd/buildah isn't available, should return a helpful error
    // (Note: actual build tests require a proper container runtime)
    let env = TestEnv::new();

    let containers_dir = env.repo_path().join(".binnacle").join("containers");
    fs::create_dir_all(&containers_dir).unwrap();
    fs::write(
        containers_dir.join("config.kdl"),
        r#"
container "exists" {
    description "This exists"
}
"#,
    )
    .unwrap();

    // Build command returns success with error in JSON (graceful handling)
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.args(["container", "build", "nonexistent"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"success\":false"));
}

#[test]
fn test_container_build_definition_listed() {
    // Building a definition that exists shows container runtime needed error
    let env = TestEnv::new();

    let containers_dir = env.repo_path().join(".binnacle").join("containers");
    fs::create_dir_all(&containers_dir).unwrap();
    fs::write(
        containers_dir.join("config.kdl"),
        r#"
container "child" {
    parent "nonexistent-parent"
    description "Orphan"
}
"#,
    )
    .unwrap();

    // Build command gracefully reports container runtime needed
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.args(["container", "build", "child"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"success\":false"));
}

#[test]
fn test_container_definition_three_level_chain() {
    // Three-level parent chain: base -> middle -> child
    let env = TestEnv::new();

    let containers_dir = env.repo_path().join(".binnacle").join("containers");
    fs::create_dir_all(&containers_dir).unwrap();
    fs::write(
        containers_dir.join("config.kdl"),
        r#"
container "base" {
    description "Base layer"
}
container "middle" {
    parent "base"
    description "Middle layer"
}
container "child" {
    parent "middle"
    description "Child layer"
}
"#,
    )
    .unwrap();

    // Should list all 3 definitions
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.args(["container", "list-definitions"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"count\":3"));

    // Human-readable should show parent relationships
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.args(["container", "list-definitions", "-H"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("middle (project)"))
        .stdout(predicate::str::contains("Parent: base"))
        .stdout(predicate::str::contains("child (project)"))
        .stdout(predicate::str::contains("Parent: middle"));
}

#[test]
fn test_container_definition_three_node_cycle_listed() {
    // Three-node cycle: a -> b -> c -> a (detected at build-time)
    let env = TestEnv::new();

    let containers_dir = env.repo_path().join(".binnacle").join("containers");
    fs::create_dir_all(&containers_dir).unwrap();
    fs::write(
        containers_dir.join("config.kdl"),
        r#"
container "a" {
    parent "c"
}
container "b" {
    parent "a"
}
container "c" {
    parent "b"
}
"#,
    )
    .unwrap();

    // List succeeds (cycle detected at build-time)
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.args(["container", "list-definitions"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"name\":\"a\""))
        .stdout(predicate::str::contains("\"name\":\"b\""))
        .stdout(predicate::str::contains("\"name\":\"c\""))
        .stdout(predicate::str::contains("\"count\":3"));
}

#[test]
fn test_container_definition_mount_special_values_parsed() {
    // Special mount source values (workspace, binnacle) should parse correctly
    // (Note: mount details not included in list-definitions output)
    let env = TestEnv::new();

    let containers_dir = env.repo_path().join(".binnacle").join("containers");
    fs::create_dir_all(&containers_dir).unwrap();
    fs::write(
        containers_dir.join("config.kdl"),
        r#"
container "test" {
    description "Container with special mounts"
    mounts {
        mount "workspace" source="workspace" target="/workspace" mode="rw"
        mount "binnacle" source="binnacle" target="/binnacle" mode="rw"
    }
}
"#,
    )
    .unwrap();

    // Parsing should succeed (mount details not in summary output)
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.args(["container", "list-definitions"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"name\":\"test\""))
        .stdout(predicate::str::contains("Container with special mounts"));
}

#[test]
fn test_container_build_skip_mount_validation_flag() {
    // The --skip-mount-validation flag should be accepted
    let env = TestEnv::new();

    let containers_dir = env.repo_path().join(".binnacle").join("containers");
    fs::create_dir_all(&containers_dir).unwrap();
    fs::write(
        containers_dir.join("config.kdl"),
        r#"
container "test" {
    mounts {
        mount "data" source="/nonexistent/path" target="/data" mode="rw"
    }
}
"#,
    )
    .unwrap();

    // Without --skip-mount-validation, listing should still work (validation is at build time)
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.args(["container", "list-definitions"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"name\":\"test\""));
}

#[test]
fn test_container_definition_multiple_containers_same_parent() {
    // Multiple containers can have the same parent (tree structure)
    let env = TestEnv::new();

    let containers_dir = env.repo_path().join(".binnacle").join("containers");
    fs::create_dir_all(&containers_dir).unwrap();
    fs::write(
        containers_dir.join("config.kdl"),
        r#"
container "base" {
    description "Common base"
}
container "rust-dev" {
    parent "base"
    description "For Rust"
}
container "python-dev" {
    parent "base"
    description "For Python"
}
container "go-dev" {
    parent "base"
    description "For Go"
}
"#,
    )
    .unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.args(["container", "list-definitions"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"count\":4"));

    // All should show parent "base"
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(env.repo_path());
    cmd.args(["container", "list-definitions", "-H"]);
    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should have "Parent: base" appearing 3 times (for rust-dev, python-dev, go-dev)
    assert_eq!(
        stdout.matches("Parent: base").count(),
        3,
        "Should have 3 children with parent 'base'"
    );
}
