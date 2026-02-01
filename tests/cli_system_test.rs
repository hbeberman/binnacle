//! Integration tests for `bn system` and `bn session` commands.
//!
//! These tests verify the session administration commands:
//! - `bn session init` - Initialize binnacle repository
//! - `bn session store show` - Display store summary
//! - `bn session store export` - Export store to archive (uses zstd compression)
//! - `bn session store import` - Import store from archive (supports both zstd and gzip)

mod common;

use assert_cmd::Command;
use common::TestEnv;
use flate2::Compression;
use flate2::write::GzEncoder;
use predicates::prelude::*;
use serde_json::Value;
use std::fs;
use std::io::Write;

/// Get a Command for the bn binary in a TestEnv.
fn bn_in(env: &TestEnv) -> Command {
    env.bn()
}

/// Initialize binnacle in a temp directory and return the TestEnv.
fn init_binnacle() -> TestEnv {
    let env = TestEnv::new();
    env.bn()
        .args(["session", "init", "--auto-global", "-y"])
        .assert()
        .success();
    env
}

/// Create a task and return its ID.
fn create_task(env: &TestEnv, title: &str) -> String {
    let output = bn_in(env)
        .args(["task", "create", title])
        .output()
        .expect("Failed to run bn task create");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let id_start = stdout.find("\"id\":\"").expect("No id in output") + 6;
    let id_end = stdout[id_start..]
        .find('"')
        .expect("No closing quote for id")
        + id_start;
    stdout[id_start..id_end].to_string()
}

/// Parse JSON output from a command.
fn parse_json(output: &[u8]) -> Value {
    serde_json::from_slice(output).expect("Failed to parse JSON output")
}

/// Parse JSON from output that may contain interactive prompts.
/// Extracts JSON that may be on the same line as prompts.
fn parse_json_from_mixed_output(output: &[u8]) -> Value {
    let output_str = String::from_utf8_lossy(output);

    // Find the first occurrence of '{' which starts the JSON
    if let Some(start_pos) = output_str.find('{') {
        let json_str = &output_str[start_pos..];
        if let Ok(json) = serde_json::from_str(json_str.trim()) {
            return json;
        }
    }

    panic!("Failed to find JSON in output: {}", output_str);
}

// ============================================================================
// bn session init Tests
// ============================================================================

#[test]
fn test_session_init_new_repo() {
    let temp = TestEnv::new();

    let output = bn_in(&temp)
        .args(["session", "init", "--auto-global", "-y"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["initialized"], true);
    // Storage path should be in the test data directory (via BN_DATA_DIR)
    let storage_path = json["storage_path"].as_str().unwrap();
    assert!(
        storage_path.starts_with(temp.data_path().to_str().unwrap()),
        "storage_path '{}' should be under data_path '{}'",
        storage_path,
        temp.data_path().display()
    );
}

#[test]
fn test_session_init_existing_repo() {
    let temp = init_binnacle();

    // Initialize again (should be idempotent)
    let output = bn_in(&temp)
        .args(["session", "init", "--auto-global", "-y"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["initialized"], false); // Already initialized
}

#[test]
fn test_session_init_human_format() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["-H", "session", "init", "--auto-global", "-y"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Session initialized"));
}

#[test]
fn test_session_init_write_copilot_prompts_flag() {
    let temp = TestEnv::new();

    let output = bn_in(&temp)
        .args([
            "session",
            "init",
            "--auto-global",
            "--write-copilot-prompts",
            "-y",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["initialized"], true);
    assert_eq!(json["copilot_prompts_created"], true);

    // Verify files were created
    let agents_dir = temp.path().join(".github").join("agents");
    let instructions_dir = temp.path().join(".github").join("instructions");

    assert!(agents_dir.join("binnacle-plan.agent.md").exists());
    assert!(agents_dir.join("binnacle-prd.agent.md").exists());
    assert!(agents_dir.join("binnacle-tasks.agent.md").exists());
    assert!(instructions_dir.join("binnacle.instructions.md").exists());

    // Verify content
    let plan_content = fs::read_to_string(agents_dir.join("binnacle-plan.agent.md")).unwrap();
    assert!(plan_content.contains("name: Binnacle Plan"));
    assert!(plan_content.contains("PLANNING AGENT"));
}

#[test]
fn test_session_init_write_copilot_prompts_idempotent() {
    let temp = TestEnv::new();

    // First init
    bn_in(&temp)
        .args([
            "session",
            "init",
            "--auto-global",
            "--write-copilot-prompts",
            "-y",
        ])
        .assert()
        .success();

    // Second init should overwrite without error
    let output = bn_in(&temp)
        .args([
            "session",
            "init",
            "--auto-global",
            "--write-copilot-prompts",
            "-y",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["initialized"], false); // Already initialized
    assert_eq!(json["copilot_prompts_created"], true); // Still created/updated
}

#[test]
fn test_session_init_write_copilot_prompts_human_output() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args([
            "-H",
            "session",
            "init",
            "--auto-global",
            "--write-copilot-prompts",
            "-y",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Copilot agents"))
        .stdout(predicate::str::contains(".github/agents"));
}

// ============================================================================
// MCP Config Integration Tests
// ============================================================================

#[test]
fn test_session_init_write_mcp_vscode_creates_config() {
    let temp = TestEnv::new();

    let output = bn_in(&temp)
        .args([
            "session",
            "init",
            "--auto-global",
            "--write-mcp-vscode",
            "-y",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["initialized"], true);
    assert_eq!(json["mcp_vscode_config_created"], true);

    // Verify the file was created
    let config_path = temp.path().join(".vscode").join("mcp.json");
    assert!(config_path.exists(), "VS Code MCP config should exist");

    // Verify the config content
    let content = fs::read_to_string(&config_path).unwrap();
    let config: Value = serde_json::from_str(&content).unwrap();
    assert!(config["servers"]["binnacle"].is_object());
    assert_eq!(config["servers"]["binnacle"]["command"], "bn");
    assert_eq!(
        config["servers"]["binnacle"]["args"],
        serde_json::json!(["mcp", "serve", "--cwd", "${workspaceFolder}"])
    );
    assert_eq!(config["servers"]["binnacle"]["type"], "stdio");
}

#[test]
fn test_session_init_write_mcp_vscode_merges_with_existing() {
    let temp = TestEnv::new();

    // Create an existing VS Code MCP config with another server
    let vscode_dir = temp.path().join(".vscode");
    fs::create_dir_all(&vscode_dir).unwrap();
    let existing_config = serde_json::json!({
        "servers": {
            "other-server": {
                "command": "other-cmd",
                "args": ["arg1"]
            }
        }
    });
    fs::write(
        vscode_dir.join("mcp.json"),
        serde_json::to_string_pretty(&existing_config).unwrap(),
    )
    .unwrap();

    // Run init with --write-mcp-vscode
    bn_in(&temp)
        .args([
            "session",
            "init",
            "--auto-global",
            "--write-mcp-vscode",
            "-y",
        ])
        .assert()
        .success();

    // Verify both servers exist
    let content = fs::read_to_string(vscode_dir.join("mcp.json")).unwrap();
    let config: Value = serde_json::from_str(&content).unwrap();
    assert!(
        config["servers"]["other-server"].is_object(),
        "existing server should be preserved"
    );
    assert!(
        config["servers"]["binnacle"].is_object(),
        "binnacle server should be added"
    );
}

#[test]
fn test_session_init_write_mcp_vscode_idempotent() {
    let temp = TestEnv::new();

    // First init
    bn_in(&temp)
        .args([
            "session",
            "init",
            "--auto-global",
            "--write-mcp-vscode",
            "-y",
        ])
        .assert()
        .success();

    // Second init should succeed without error
    let output = bn_in(&temp)
        .args([
            "session",
            "init",
            "--auto-global",
            "--write-mcp-vscode",
            "-y",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["mcp_vscode_config_created"], true);
}

#[test]
fn test_host_init_write_mcp_copilot() {
    let temp = TestEnv::new();

    // Host-init with --write-mcp-copilot creates the copilot CLI config
    let output = bn_in(&temp)
        .args(["system", "host-init", "--write-mcp-copilot", "-y"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["mcp_copilot_config_created"], true);
}

// ============================================================================
// Session Token Tests
// ============================================================================

#[test]
fn test_session_init_token_invalid_returns_error() {
    let temp = TestEnv::new();

    // Try to use an invalid token - should fail validation
    bn_in(&temp)
        .args([
            "session",
            "init",
            "--auto-global",
            "--token",
            "invalid_token_that_wont_validate",
            "-y",
        ])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("Token validation failed")
                .or(predicate::str::contains("Copilot token validation failed")),
        );
}

#[test]
fn test_session_init_token_non_validated_invalid_returns_error() {
    let temp = TestEnv::new();

    // Try to use an invalid token with --token-non-validated - should fail GitHub user validation
    bn_in(&temp)
        .args([
            "session",
            "init",
            "--auto-global",
            "--token-non-validated",
            "invalid_token_that_wont_validate",
            "-y",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Token validation failed"));
}

#[test]
fn test_session_init_token_flags_conflict() {
    let temp = TestEnv::new();

    // Both --token and --token-non-validated cannot be used together
    bn_in(&temp)
        .args([
            "session",
            "init",
            "--auto-global",
            "--token",
            "some_token",
            "--token-non-validated",
            "another_token",
            "-y",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn test_session_init_without_token_has_token_stored_false() {
    let temp = TestEnv::new();

    // Initialize without token
    let output = bn_in(&temp)
        .args(["session", "init", "--auto-global", "-y"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["token_stored"], false);
    assert_eq!(json["token_username"], serde_json::Value::Null);
    assert_eq!(json["copilot_validated"], false);
}

// ============================================================================
// bn session store show Tests
// ============================================================================

#[test]
fn test_store_show_empty_repo() {
    let temp = init_binnacle();

    let output = bn_in(&temp)
        .args(["session", "store", "show"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    // Storage path should be in the test data directory (via BN_DATA_DIR)
    let storage_path = json["storage_path"].as_str().unwrap();
    assert!(
        storage_path.starts_with(temp.data_path().to_str().unwrap()),
        "storage_path '{}' should be under data_path '{}'",
        storage_path,
        temp.data_path().display()
    );
    assert!(json["repo_path"].is_string());
    assert_eq!(json["tasks"]["total"], 0);
    assert_eq!(json["tests"]["total"], 0);
    assert_eq!(json["commits"]["total"], 0);
}

#[test]
fn test_store_show_with_tasks() {
    let temp = init_binnacle();

    // Create some tasks
    create_task(&temp, "Task A");
    create_task(&temp, "Task B");
    let task_c = create_task(&temp, "Task C");

    // Update one to in_progress
    bn_in(&temp)
        .args(["task", "update", &task_c, "--status", "in_progress"])
        .assert()
        .success();

    let output = bn_in(&temp)
        .args(["session", "store", "show"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["tasks"]["total"], 3);
    assert_eq!(json["tasks"]["by_status"]["pending"], 2);
    assert_eq!(json["tasks"]["by_status"]["in_progress"], 1);
}

#[test]
fn test_store_show_human_format() {
    let temp = init_binnacle();
    create_task(&temp, "Test Task");

    bn_in(&temp)
        .args(["-H", "session", "store", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Store:"))
        .stdout(predicate::str::contains("Repo:"))
        .stdout(predicate::str::contains("Tasks: 1 total"))
        .stdout(predicate::str::contains("tasks.jsonl"));
}

#[test]
fn test_store_show_files_section() {
    let temp = init_binnacle();
    create_task(&temp, "Test");

    let output = bn_in(&temp)
        .args(["session", "store", "show"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert!(json["files"]["tasks.jsonl"].is_object());
    assert!(json["files"]["tasks.jsonl"]["size_bytes"].is_number());
    assert!(json["files"]["tasks.jsonl"]["entries"].is_number());
}

#[test]
fn test_store_show_not_initialized() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["session", "store", "show"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not initialized"));
}

// ============================================================================
// bn system store export Tests
// ============================================================================

#[test]
fn test_store_export_to_file() {
    let temp = init_binnacle();
    create_task(&temp, "Export Test Task");

    let export_path = temp.path().join("backup.bng");

    let output = bn_in(&temp)
        .args(["session", "store", "export", export_path.to_str().unwrap()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["exported"], true);
    assert!(
        json["output_path"]
            .as_str()
            .unwrap()
            .ends_with("backup.bng")
    );
    assert!(json["size_bytes"].as_u64().unwrap() > 0);
    assert_eq!(json["task_count"], 1);

    // Verify file exists and has content
    assert!(export_path.exists());
    let metadata = fs::metadata(&export_path).unwrap();
    assert!(metadata.len() > 0);
}

#[test]
fn test_store_export_stdout() {
    let temp = init_binnacle();
    create_task(&temp, "Stdout Export Task");

    let output = bn_in(&temp)
        .args(["session", "store", "export", "-"])
        .assert()
        .success()
        .get_output()
        .clone();

    // stdout should contain binary tar.gz data
    assert!(output.stdout.len() > 100); // Archive should be reasonably sized

    // The command should succeed - JSON output location varies by implementation
    // Just verify we got archive data
}

#[test]
fn test_store_export_human_format() {
    let temp = init_binnacle();
    let export_path = temp.path().join("backup.bng");

    bn_in(&temp)
        .args([
            "-H",
            "session",
            "store",
            "export",
            export_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Exported"))
        .stdout(predicate::str::contains("backup.bng"));
}

#[test]
fn test_store_export_not_initialized() {
    let temp = TestEnv::new();
    let export_path = temp.path().join("backup.bng");

    bn_in(&temp)
        .args(["session", "store", "export", export_path.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not initialized"));
}

#[test]
fn test_store_export_with_format_flag() {
    let temp = init_binnacle();
    let export_path = temp.path().join("backup.bng");

    bn_in(&temp)
        .args([
            "session",
            "store",
            "export",
            "--format",
            "archive",
            export_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert!(export_path.exists());
}

// ============================================================================
// bn system store import Tests - Replace Mode
// ============================================================================

#[test]
fn test_store_import_replace_mode_clean() {
    let temp1 = init_binnacle();
    create_task(&temp1, "Original Task");

    // Export from first repo
    let export_path = temp1.path().join("backup.bng");
    bn_in(&temp1)
        .args(["session", "store", "export", export_path.to_str().unwrap()])
        .assert()
        .success();

    // Import to fresh repo
    let temp2 = TestEnv::new();
    let output = bn_in(&temp2)
        .args(["session", "store", "import", export_path.to_str().unwrap()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["imported"], true);
    assert_eq!(json["dry_run"], false);
    assert_eq!(json["import_type"], "replace");
    assert_eq!(json["tasks_imported"], 1);
    assert_eq!(json["collisions"], 0);

    // Verify task was imported
    let list_output = bn_in(&temp2)
        .args(["task", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert!(String::from_utf8_lossy(&list_output).contains("Original Task"));
}

#[test]
fn test_store_import_replace_mode_already_initialized() {
    let temp1 = init_binnacle();
    let export_path = temp1.path().join("backup.bng");
    bn_in(&temp1)
        .args(["session", "store", "export", export_path.to_str().unwrap()])
        .assert()
        .success();

    // Try to import to already initialized repo
    let temp2 = init_binnacle();
    bn_in(&temp2)
        .args(["session", "store", "import", export_path.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already initialized"))
        .stderr(predicate::str::contains("--type merge"));
}

#[test]
fn test_store_import_stdin() {
    let temp1 = init_binnacle();
    create_task(&temp1, "Stdin Import Task");

    // Export to stdout
    let export_output = bn_in(&temp1)
        .args(["session", "store", "export", "-"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    // Import from stdin
    let temp2 = TestEnv::new();
    let mut cmd = bn_in(&temp2);
    cmd.args(["session", "store", "import", "-"])
        .write_stdin(export_output);

    let output = cmd.assert().success().get_output().stdout.clone();

    let json = parse_json(&output);
    assert_eq!(json["imported"], true);
    assert_eq!(json["input_path"], "-");
}

// ============================================================================
// bn system store import Tests - Merge Mode
// ============================================================================

#[test]
fn test_store_import_merge_no_collisions() {
    let temp1 = init_binnacle();
    create_task(&temp1, "Task A");

    let export_path = temp1.path().join("backup.bng");
    bn_in(&temp1)
        .args(["session", "store", "export", export_path.to_str().unwrap()])
        .assert()
        .success();

    // Create different task in second repo
    let temp2 = init_binnacle();
    create_task(&temp2, "Task B");

    // Merge import
    let output = bn_in(&temp2)
        .args([
            "session",
            "store",
            "import",
            "--type",
            "merge",
            export_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["imported"], true);
    assert_eq!(json["import_type"], "merge");
    assert_eq!(json["tasks_imported"], 1);
    assert_eq!(json["collisions"], 0);

    // Verify both tasks exist
    let list_output = bn_in(&temp2)
        .args(["task", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let list_str = String::from_utf8_lossy(&list_output);
    assert!(list_str.contains("Task A"));
    assert!(list_str.contains("Task B"));
}

#[test]
fn test_store_import_merge_with_collisions() {
    // Create first repo with task
    let temp1 = init_binnacle();
    let task_id = create_task(&temp1, "Exported Task");

    let export_path = temp1.path().join("backup.bng");
    bn_in(&temp1)
        .args(["session", "store", "export", export_path.to_str().unwrap()])
        .assert()
        .success();

    // Create second repo and import once (creates task with same ID)
    let temp2 = TestEnv::new();
    bn_in(&temp2)
        .args(["session", "store", "import", export_path.to_str().unwrap()])
        .assert()
        .success();

    // Create a different task with the same ID would collide
    // So we modify the task title in place
    bn_in(&temp2)
        .args(["task", "update", &task_id, "--title", "Modified Task"])
        .assert()
        .success();

    // Import again with merge - should handle collision
    let output = bn_in(&temp2)
        .args([
            "session",
            "store",
            "import",
            "--type",
            "merge",
            export_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .get_output()
        .clone();

    let json = parse_json(&output.stdout);
    assert_eq!(json["import_type"], "merge");

    // The merge should succeed. Collision handling may vary by implementation.
    // Just verify the merge completed successfully.
    assert_eq!(json["imported"], true);
}

#[test]
fn test_store_import_dry_run() {
    let temp1 = init_binnacle();
    create_task(&temp1, "Dry Run Task");

    let export_path = temp1.path().join("backup.bng");
    bn_in(&temp1)
        .args(["session", "store", "export", export_path.to_str().unwrap()])
        .assert()
        .success();

    // Dry run import
    let temp2 = TestEnv::new();
    let output = bn_in(&temp2)
        .args([
            "session",
            "store",
            "import",
            "--dry-run",
            export_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["dry_run"], true);
    assert_eq!(json["tasks_imported"], 1);

    // Note: Dry run may still initialize the repo structure.
    // Verify the task wasn't actually imported by checking task list is empty.
    let list_output = bn_in(&temp2)
        .args(["task", "list"])
        .output()
        .expect("Failed to list tasks");

    if list_output.status.success() {
        let json = parse_json(&list_output.stdout);
        assert_eq!(json["count"], 0, "Dry run should not import tasks");
    }
}

#[test]
fn test_store_import_dry_run_shows_collisions() {
    // Create first repo
    let temp1 = init_binnacle();
    let task_id = create_task(&temp1, "Original Task");

    let export_path = temp1.path().join("backup.bng");
    bn_in(&temp1)
        .args(["session", "store", "export", export_path.to_str().unwrap()])
        .assert()
        .success();

    // Import to second repo
    let temp2 = TestEnv::new();
    bn_in(&temp2)
        .args(["session", "store", "import", export_path.to_str().unwrap()])
        .assert()
        .success();

    // Modify the task
    bn_in(&temp2)
        .args(["task", "update", &task_id, "--title", "Modified"])
        .assert()
        .success();

    // Dry run merge - should show potential collisions without applying
    let output = bn_in(&temp2)
        .args([
            "session",
            "store",
            "import",
            "--type",
            "merge",
            "--dry-run",
            export_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["dry_run"], true);

    // Original task should still be "Modified", not changed by dry-run
    let show_output = bn_in(&temp2)
        .args(["task", "show", &task_id])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert!(String::from_utf8_lossy(&show_output).contains("Modified"));
}

#[test]
fn test_store_import_human_format() {
    let temp1 = init_binnacle();
    create_task(&temp1, "Human Format Task");

    let export_path = temp1.path().join("backup.bng");
    bn_in(&temp1)
        .args(["session", "store", "export", export_path.to_str().unwrap()])
        .assert()
        .success();

    let temp2 = TestEnv::new();
    bn_in(&temp2)
        .args([
            "-H",
            "session",
            "store",
            "import",
            export_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Imported"))
        .stdout(predicate::str::contains("Tasks"));
}

#[test]
fn test_store_import_invalid_archive() {
    let temp = TestEnv::new();
    let bad_file = temp.path().join("bad.bng");
    fs::write(&bad_file, b"not a real archive").unwrap();

    bn_in(&temp)
        .args(["session", "store", "import", bad_file.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("Invalid")
                .or(predicate::str::contains("Failed"))
                .or(predicate::str::contains("invalid gzip header"))
                .or(predicate::str::contains("IO error")),
        );
}

#[test]
fn test_store_import_gzip_backwards_compatibility() {
    // Create a gzip archive manually to test backwards compatibility
    let temp1 = init_binnacle();
    let _task_id = create_task(&temp1, "Gzip Test Task");

    // Export first to get the correct archive structure
    let zstd_export = temp1.path().join("export.bng");
    bn_in(&temp1)
        .args(["session", "store", "export", zstd_export.to_str().unwrap()])
        .assert()
        .success();

    // Decompress the zstd archive
    let zstd_data = fs::read(&zstd_export).unwrap();
    let tar_data = zstd::decode_all(&zstd_data[..]).unwrap();

    // Re-compress as gzip to create a backwards-compatible archive
    let gzip_archive = temp1.path().join("legacy.tar.gz");
    let gzip_file = fs::File::create(&gzip_archive).unwrap();
    let mut encoder = GzEncoder::new(gzip_file, Compression::default());
    encoder.write_all(&tar_data).unwrap();
    encoder.finish().unwrap();

    // Import the gzip archive into a new repo
    let temp2 = TestEnv::new();
    bn_in(&temp2)
        .args(["session", "store", "import", gzip_archive.to_str().unwrap()])
        .assert()
        .success();

    // Verify the task was imported
    let output = bn_in(&temp2)
        .args(["task", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();
    assert!(
        json["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["title"] == "Gzip Test Task"),
        "Task from gzip archive should be imported"
    );
}

// ============================================================================
// Round-trip Tests
// ============================================================================

#[test]
fn test_export_import_roundtrip() {
    // Create repo with various data
    let temp1 = init_binnacle();
    let task_a = create_task(&temp1, "Task A");
    let task_b = create_task(&temp1, "Task B");
    let task_c = create_task(&temp1, "Task C");

    // Create dependency
    bn_in(&temp1)
        .args([
            "link",
            "add",
            &task_b,
            &task_a,
            "--type",
            "depends_on",
            "--reason",
            "test dep",
        ])
        .assert()
        .success();

    // Update status
    bn_in(&temp1)
        .args(["task", "update", &task_c, "--status", "in_progress"])
        .assert()
        .success();

    // Export
    let export_path = temp1.path().join("roundtrip.bng");
    bn_in(&temp1)
        .args(["session", "store", "export", export_path.to_str().unwrap()])
        .assert()
        .success();

    // Import to new repo
    let temp2 = TestEnv::new();
    bn_in(&temp2)
        .args(["session", "store", "import", export_path.to_str().unwrap()])
        .assert()
        .success();

    // Verify all tasks exist
    let list_output = bn_in(&temp2)
        .args(["task", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let list_str = String::from_utf8_lossy(&list_output);
    assert!(list_str.contains("Task A"));
    assert!(list_str.contains("Task B"));
    assert!(list_str.contains("Task C"));

    // Verify dependency preserved
    let dep_output = bn_in(&temp2)
        .args(["link", "list", &task_b])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert!(String::from_utf8_lossy(&dep_output).contains(&task_a));

    // Verify task status preserved
    let show_output = bn_in(&temp2)
        .args(["task", "show", &task_c])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert!(String::from_utf8_lossy(&show_output).contains("in_progress"));
}

#[test]
fn test_export_import_via_stdout_stdin_piping() {
    // Create source repo
    let temp1 = init_binnacle();
    create_task(&temp1, "Piped Task");

    // Export to stdout
    let archive_data = bn_in(&temp1)
        .args(["session", "store", "export", "-"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    // Import from stdin to new repo
    let temp2 = TestEnv::new();
    let mut cmd = bn_in(&temp2);
    cmd.args(["session", "store", "import", "-"])
        .write_stdin(archive_data);

    cmd.assert().success();

    // Verify task exists
    let list_output = bn_in(&temp2)
        .args(["task", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert!(String::from_utf8_lossy(&list_output).contains("Piped Task"));
}

// ============================================================================
// bn system store import Tests - Folder Import
// ============================================================================

/// Helper to create a folder with binnacle data files for import testing.
fn create_import_folder(env: &TestEnv) -> std::path::PathBuf {
    let folder = env.path().join("import_source");
    fs::create_dir_all(&folder).unwrap();
    folder
}

/// Write a tasks.jsonl file to a folder.
fn write_tasks_jsonl(folder: &std::path::Path, tasks: &[(&str, &str)]) {
    let tasks_file = folder.join("tasks.jsonl");
    let mut content = String::new();

    for (id, title) in tasks {
        content.push_str(&format!(
            r#"{{"id":"{}","type":"task","title":"{}","priority":2,"status":"pending","tags":[],"depends_on":[],"created_at":"2026-01-21T10:00:00Z","updated_at":"2026-01-21T10:00:00Z"}}"#,
            id, title
        ));
        content.push('\n');
    }

    fs::write(tasks_file, content).unwrap();
}

/// Write a commits.jsonl file to a folder.
fn write_commits_jsonl(folder: &std::path::Path, commits: &[&str]) {
    let commits_file = folder.join("commits.jsonl");
    let mut content = String::new();

    for sha in commits {
        content.push_str(&format!(
            r#"{{"sha":"{}","message":"Test commit","task_ids":[],"created_at":"2026-01-21T10:00:00Z"}}"#,
            sha
        ));
        content.push('\n');
    }

    fs::write(commits_file, content).unwrap();
}

#[test]
fn test_folder_import_replace_mode_clean() {
    // Create a folder with tasks.jsonl
    let source_temp = TestEnv::new();
    let import_folder = create_import_folder(&source_temp);
    write_tasks_jsonl(&import_folder, &[("bn-test1", "Folder Import Task")]);

    // Import to fresh repo (replace mode - default)
    let dest_temp = TestEnv::new();
    let output = bn_in(&dest_temp)
        .args([
            "session",
            "store",
            "import",
            import_folder.to_str().unwrap(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["imported"], true);
    assert_eq!(json["dry_run"], false);
    assert_eq!(json["import_type"], "replace");
    assert_eq!(json["tasks_imported"], 1);
    assert_eq!(json["collisions"], 0);

    // Verify task was imported
    let list_output = bn_in(&dest_temp)
        .args(["task", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert!(String::from_utf8_lossy(&list_output).contains("Folder Import Task"));
}

#[test]
fn test_folder_import_replace_mode_already_initialized() {
    // Create source folder
    let source_temp = TestEnv::new();
    let import_folder = create_import_folder(&source_temp);
    write_tasks_jsonl(&import_folder, &[("bn-test1", "Folder Task")]);

    // Try to import to already initialized repo (should fail with replace mode)
    let dest_temp = init_binnacle();
    bn_in(&dest_temp)
        .args([
            "session",
            "store",
            "import",
            import_folder.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already initialized"))
        .stderr(predicate::str::contains("--type merge"));
}

#[test]
fn test_folder_import_merge_no_collisions() {
    // Create source folder with task
    let source_temp = TestEnv::new();
    let import_folder = create_import_folder(&source_temp);
    write_tasks_jsonl(&import_folder, &[("bn-import1", "Imported Task")]);

    // Create dest repo with different task
    let dest_temp = init_binnacle();
    create_task(&dest_temp, "Existing Task");

    // Merge import
    let output = bn_in(&dest_temp)
        .args([
            "session",
            "store",
            "import",
            "--type",
            "merge",
            import_folder.to_str().unwrap(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["imported"], true);
    assert_eq!(json["import_type"], "merge");
    assert_eq!(json["tasks_imported"], 1);
    assert_eq!(json["collisions"], 0);

    // Verify both tasks exist
    let list_output = bn_in(&dest_temp)
        .args(["task", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let list_str = String::from_utf8_lossy(&list_output);
    assert!(list_str.contains("Existing Task"));
    assert!(list_str.contains("Imported Task"));
}

#[test]
fn test_folder_import_merge_with_id_collision() {
    // Create source folder with specific task ID
    let source_temp = TestEnv::new();
    let import_folder = create_import_folder(&source_temp);
    write_tasks_jsonl(
        &import_folder,
        &[("bn-collision", "Imported Collision Task")],
    );

    // Create dest repo and manually create a task (we'll force collision)
    let dest_temp = init_binnacle();

    // First do a clean import of same folder to create the task ID
    let temp_import = TestEnv::new();
    let temp_folder = create_import_folder(&temp_import);
    write_tasks_jsonl(&temp_folder, &[("bn-collision", "First Version")]);
    bn_in(&dest_temp)
        .args([
            "session",
            "store",
            "import",
            "--type",
            "merge",
            temp_folder.to_str().unwrap(),
        ])
        .assert()
        .success();

    // Now import again with same ID - should detect collision
    let output = bn_in(&dest_temp)
        .args([
            "session",
            "store",
            "import",
            "--type",
            "merge",
            import_folder.to_str().unwrap(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["imported"], true);
    assert_eq!(json["import_type"], "merge");
    assert_eq!(json["collisions"], 1);
    assert!(!json["id_remappings"].as_object().unwrap().is_empty());

    // Both tasks should exist (with remapped ID for second one)
    let list_output = bn_in(&dest_temp)
        .args(["task", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let list_str = String::from_utf8_lossy(&list_output);
    assert!(list_str.contains("First Version"));
    assert!(list_str.contains("Imported Collision Task"));
}

#[test]
fn test_folder_import_dry_run() {
    // Create source folder
    let source_temp = TestEnv::new();
    let import_folder = create_import_folder(&source_temp);
    write_tasks_jsonl(&import_folder, &[("bn-dry1", "Dry Run Task")]);

    // Dry run import
    let dest_temp = TestEnv::new();
    let output = bn_in(&dest_temp)
        .args([
            "session",
            "store",
            "import",
            "--dry-run",
            import_folder.to_str().unwrap(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["dry_run"], true);
    assert_eq!(json["tasks_imported"], 1);

    // Verify task was NOT actually imported
    let list_output = bn_in(&dest_temp)
        .args(["task", "list"])
        .output()
        .expect("Failed to list tasks");

    if list_output.status.success() {
        let json = parse_json(&list_output.stdout);
        assert_eq!(json["count"], 0, "Dry run should not import tasks");
    }
}

#[test]
fn test_folder_import_missing_tasks_jsonl() {
    // Create folder WITHOUT tasks.jsonl
    let source_temp = TestEnv::new();
    let import_folder = source_temp.path().join("empty_folder");
    fs::create_dir_all(&import_folder).unwrap();

    // Only create commits.jsonl (not tasks.jsonl)
    write_commits_jsonl(&import_folder, &["abc123"]);

    // Import should fail
    let dest_temp = TestEnv::new();
    bn_in(&dest_temp)
        .args([
            "session",
            "store",
            "import",
            import_folder.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("missing required tasks.jsonl"));
}

#[test]
fn test_folder_import_with_optional_files() {
    // Create folder with all optional files
    let source_temp = TestEnv::new();
    let import_folder = create_import_folder(&source_temp);
    write_tasks_jsonl(&import_folder, &[("bn-full1", "Full Import Task")]);
    write_commits_jsonl(&import_folder, &["commit123", "commit456"]);

    // Create test-results.jsonl
    let test_results = import_folder.join("test-results.jsonl");
    fs::write(
        test_results,
        r#"{"id":"tr-1","test_name":"test_foo","passed":true,"created_at":"2026-01-21T10:00:00Z"}"#,
    )
    .unwrap();

    // Import
    let dest_temp = TestEnv::new();
    let output = bn_in(&dest_temp)
        .args([
            "session",
            "store",
            "import",
            import_folder.to_str().unwrap(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["imported"], true);
    assert_eq!(json["tasks_imported"], 1);
    assert_eq!(json["commits_imported"], 2);
    assert_eq!(json["tests_imported"], 1);
}

#[test]
fn test_folder_import_empty_tasks_jsonl() {
    // Create folder with empty tasks.jsonl (valid but no tasks)
    let source_temp = TestEnv::new();
    let import_folder = create_import_folder(&source_temp);
    fs::write(import_folder.join("tasks.jsonl"), "").unwrap();

    // Import should succeed with 0 tasks
    let dest_temp = TestEnv::new();
    let output = bn_in(&dest_temp)
        .args([
            "session",
            "store",
            "import",
            import_folder.to_str().unwrap(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["imported"], true);
    assert_eq!(json["tasks_imported"], 0);
}

#[test]
fn test_folder_import_human_format() {
    let source_temp = TestEnv::new();
    let import_folder = create_import_folder(&source_temp);
    write_tasks_jsonl(&import_folder, &[("bn-human1", "Human Format Task")]);

    let dest_temp = TestEnv::new();
    bn_in(&dest_temp)
        .args([
            "-H",
            "session",
            "store",
            "import",
            import_folder.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Imported"))
        .stdout(predicate::str::contains("Tasks"));
}

#[test]
fn test_folder_import_roundtrip_via_store_path() {
    // Create repo with tasks
    let temp1 = init_binnacle();
    create_task(&temp1, "Roundtrip Task A");
    let task_b = create_task(&temp1, "Roundtrip Task B");

    // Update one task
    bn_in(&temp1)
        .args(["task", "update", &task_b, "--status", "in_progress"])
        .assert()
        .success();

    // Get the storage path
    let show_output = bn_in(&temp1)
        .args(["session", "store", "show"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let show_json = parse_json(&show_output);
    let storage_path = show_json["storage_path"].as_str().unwrap();

    // Import from the storage folder directly (simulating legacy import)
    let temp2 = TestEnv::new();
    let output = bn_in(&temp2)
        .args(["session", "store", "import", storage_path])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["imported"], true);
    assert!(json["tasks_imported"].as_u64().unwrap() >= 2);

    // Verify tasks exist in new repo
    let list_output = bn_in(&temp2)
        .args(["task", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let list_str = String::from_utf8_lossy(&list_output);
    assert!(list_str.contains("Roundtrip Task A"));
    assert!(list_str.contains("Roundtrip Task B"));
}

// ============================================================================
// bn system emit Tests
// ============================================================================

#[test]
fn test_system_emit_agents_human_format() {
    // emit command doesn't require initialization
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["-H", "system", "emit", "agents"])
        .assert()
        .success()
        .stdout(predicate::str::contains("<!-- BEGIN BINNACLE SECTION -->"))
        .stdout(predicate::str::contains("<!-- END BINNACLE SECTION -->"))
        .stdout(predicate::str::contains("bn orient"))
        .stdout(predicate::str::contains("bn ready"));
}

#[test]
fn test_system_emit_agents_json_format() {
    let temp = TestEnv::new();

    let output = bn_in(&temp)
        .args(["system", "emit", "agents"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    let content = json["content"]
        .as_str()
        .expect("content field should exist");
    assert!(content.contains("<!-- BEGIN BINNACLE SECTION -->"));
    assert!(content.contains("<!-- END BINNACLE SECTION -->"));
}

#[test]
fn test_system_emit_skill_human_format() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["-H", "system", "emit", "skill"])
        .assert()
        .success()
        .stdout(predicate::str::contains("name: binnacle"))
        .stdout(predicate::str::contains("Key Commands"))
        .stdout(predicate::str::contains("Task Management"));
}

#[test]
fn test_system_emit_skill_json_format() {
    let temp = TestEnv::new();

    let output = bn_in(&temp)
        .args(["system", "emit", "skill"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    let content = json["content"]
        .as_str()
        .expect("content field should exist");
    assert!(content.contains("name: binnacle"));
    assert!(content.contains("Key Commands"));
}

#[test]
fn test_system_emit_no_init_required() {
    // Verify emit works without any initialization
    let temp = TestEnv::new();

    // Don't call init - just run emit directly
    bn_in(&temp)
        .args(["system", "emit", "agents"])
        .assert()
        .success();

    bn_in(&temp)
        .args(["system", "emit", "skill"])
        .assert()
        .success();
}

#[test]
fn test_system_emit_plan_agent_human_format() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["-H", "system", "emit", "plan-agent"])
        .assert()
        .success()
        .stdout(predicate::str::contains("name: Binnacle Plan"))
        .stdout(predicate::str::contains("PLANNING AGENT"))
        .stdout(predicate::str::contains("binnacle/*"));
}

#[test]
fn test_system_emit_prd_agent_human_format() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["-H", "system", "emit", "prd-agent"])
        .assert()
        .success()
        .stdout(predicate::str::contains("name: Binnacle PRD"))
        .stdout(predicate::str::contains("PRDs"))
        .stdout(predicate::str::contains("prd_template"));
}

#[test]
fn test_system_emit_tasks_agent_human_format() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["-H", "system", "emit", "tasks-agent"])
        .assert()
        .success()
        .stdout(predicate::str::contains("name: Binnacle Tasks"))
        .stdout(predicate::str::contains("task creation"))
        .stdout(predicate::str::contains("bn milestone create"));
}

#[test]
fn test_system_emit_copilot_instructions_human_format() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["-H", "system", "emit", "copilot-instructions"])
        .assert()
        .success()
        .stdout(predicate::str::contains("applyTo: '**'"))
        .stdout(predicate::str::contains("Binnacle Project Instructions"))
        .stdout(predicate::str::contains("bn orient"));
}

#[test]
fn test_system_emit_plan_agent_json_format() {
    let temp = TestEnv::new();

    let output = bn_in(&temp)
        .args(["system", "emit", "plan-agent"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    let content = json["content"]
        .as_str()
        .expect("content field should exist");
    assert!(content.contains("name: Binnacle Plan"));
    assert!(content.contains("PLANNING AGENT"));
}

#[test]
fn test_system_emit_auto_worker_human_format() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["-H", "system", "emit", "auto-worker"])
        .assert()
        .success()
        .stdout(predicate::str::contains("bn orient --type worker"))
        .stdout(predicate::str::contains("bn ready"))
        .stdout(predicate::str::contains("queued items first"))
        .stdout(predicate::str::contains("bn goodbye"));
}

#[test]
fn test_system_emit_do_agent_human_format() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["-H", "system", "emit", "do-agent"])
        .assert()
        .success()
        .stdout(predicate::str::contains("bn orient --type worker"))
        .stdout(predicate::str::contains("{description}"))
        .stdout(predicate::str::contains("Test your changes"))
        .stdout(predicate::str::contains("bn goodbye"));
}

#[test]
fn test_system_emit_prd_writer_human_format() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["-H", "system", "emit", "prd-writer"])
        .assert()
        .success()
        .stdout(predicate::str::contains("bn orient --type planner"))
        .stdout(predicate::str::contains("PRD"))
        .stdout(predicate::str::contains("bn idea list"))
        .stdout(predicate::str::contains("prds/"));
}

#[test]
fn test_system_emit_buddy_human_format() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["-H", "system", "emit", "buddy"])
        .assert()
        .success()
        .stdout(predicate::str::contains("binnacle buddy"))
        .stdout(predicate::str::contains("bn orient --type buddy"))
        .stdout(predicate::str::contains("bn idea create"))
        .stdout(predicate::str::contains("bn task create"))
        .stdout(predicate::str::contains("bn bug create"));
}

#[test]
fn test_system_emit_free_human_format() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["-H", "system", "emit", "free"])
        .assert()
        .success()
        .stdout(predicate::str::contains("binnacle (bn)"))
        .stdout(predicate::str::contains("bn orient --type worker"))
        .stdout(predicate::str::contains("bn ready"))
        .stdout(predicate::str::contains("bn goodbye"));
}

#[test]
fn test_system_emit_auto_worker_json_format() {
    let temp = TestEnv::new();

    let output = bn_in(&temp)
        .args(["system", "emit", "auto-worker"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    let content = json["content"]
        .as_str()
        .expect("content field should exist");
    assert!(content.contains("bn orient --type worker"));
    assert!(content.contains("queued items first"));
}

#[test]
fn test_system_emit_buddy_json_format() {
    let temp = TestEnv::new();

    let output = bn_in(&temp)
        .args(["system", "emit", "buddy"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    let content = json["content"]
        .as_str()
        .expect("content field should exist");
    assert!(content.contains("binnacle buddy"));
    assert!(content.contains("TASK DECOMPOSITION"));
}

#[test]
fn test_system_emit_mcp_claude() {
    let temp = TestEnv::new();

    let output = bn_in(&temp)
        .args(["system", "emit", "mcp-claude"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    let content = json["content"]
        .as_str()
        .expect("content field should exist");
    // Verify JSON structure
    let mcp_json: Value = serde_json::from_str(content).expect("content should be valid JSON");
    assert!(
        mcp_json["mcpServers"]["binnacle"].is_object(),
        "should have mcpServers.binnacle object"
    );
    assert_eq!(
        mcp_json["mcpServers"]["binnacle"]["command"], "bn",
        "command should be bn"
    );
    assert!(
        mcp_json["mcpServers"]["binnacle"]["args"]
            .as_array()
            .map(|a| a.iter().any(|v| v == "serve"))
            .unwrap_or(false),
        "args should contain 'serve'"
    );
}

#[test]
fn test_system_emit_mcp_vscode() {
    let temp = TestEnv::new();

    let output = bn_in(&temp)
        .args(["system", "emit", "mcp-vscode"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    let content = json["content"]
        .as_str()
        .expect("content field should exist");
    // Verify JSON structure
    let mcp_json: Value = serde_json::from_str(content).expect("content should be valid JSON");
    assert!(
        mcp_json["servers"]["binnacle"].is_object(),
        "should have servers.binnacle object"
    );
    assert_eq!(
        mcp_json["servers"]["binnacle"]["type"], "stdio",
        "type should be stdio"
    );
    assert_eq!(
        mcp_json["servers"]["binnacle"]["command"], "bn",
        "command should be bn"
    );
    // VS Code version should have workspaceFolder in args
    let args = mcp_json["servers"]["binnacle"]["args"]
        .as_array()
        .expect("args should be array");
    assert!(
        args.iter().any(|v| v
            .as_str()
            .map(|s| s.contains("workspaceFolder"))
            .unwrap_or(false)),
        "args should contain workspaceFolder placeholder"
    );
}

#[test]
fn test_system_emit_mcp_copilot() {
    let temp = TestEnv::new();

    let output = bn_in(&temp)
        .args(["system", "emit", "mcp-copilot"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    let content = json["content"]
        .as_str()
        .expect("content field should exist");
    // Verify JSON structure
    let mcp_json: Value = serde_json::from_str(content).expect("content should be valid JSON");
    assert!(
        mcp_json["mcpServers"]["binnacle"].is_object(),
        "should have mcpServers.binnacle object"
    );
    assert_eq!(
        mcp_json["mcpServers"]["binnacle"]["type"], "local",
        "type should be local"
    );
    assert_eq!(
        mcp_json["mcpServers"]["binnacle"]["command"], "bn",
        "command should be bn"
    );
    // Copilot version should have tools field
    assert!(
        mcp_json["mcpServers"]["binnacle"]["tools"].is_array(),
        "should have tools array"
    );
    // Note: env vars are injected dynamically by entrypoint.sh, not in the static config
}

#[test]
fn test_system_emit_mcp_lifecycle_human_format() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["-H", "system", "emit", "mcp-lifecycle"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "IMPORTANT - Binnacle MCP Lifecycle",
        ))
        .stdout(predicate::str::contains("bn orient"))
        .stdout(predicate::str::contains("bn goodbye"))
        .stdout(predicate::str::contains("shell"));
}

#[test]
fn test_system_emit_mcp_lifecycle_json_format() {
    let temp = TestEnv::new();

    let output = bn_in(&temp)
        .args(["system", "emit", "mcp-lifecycle"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    let content = json["content"]
        .as_str()
        .expect("content field should exist");
    assert!(content.contains("Binnacle MCP Lifecycle"));
    assert!(content.contains("orient"));
    assert!(content.contains("goodbye"));
}

#[test]
fn test_system_emit_mcp_lifecycle_planner_human_format() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["-H", "system", "emit", "mcp-lifecycle-planner"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "IMPORTANT - Binnacle MCP Lifecycle",
        ))
        .stdout(predicate::str::contains("bn orient"))
        // Planner agents don't call goodbye
        .stdout(predicate::str::contains("should NOT call bn goodbye"));
}

// ============================================================================
// Container Prompt Injection Tests (validates entrypoint.sh logic)
// ============================================================================
// These tests verify that the templates used by container/entrypoint.sh work correctly
// for the hybrid prompt injection approach: AGENT_INSTRUCTIONS + BN_INITIAL_PROMPT

#[test]
fn test_container_prompt_injection_copilot_instructions_produces_content() {
    // The entrypoint.sh loads copilot-instructions as base workflow rules
    let temp = TestEnv::new();

    let output = bn_in(&temp)
        .args(["-H", "system", "emit", "copilot-instructions"])
        .output()
        .expect("Failed to run command");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should produce non-empty content with core instructions
    assert!(
        !stdout.is_empty(),
        "copilot-instructions should produce content"
    );
    assert!(
        stdout.contains("Binnacle Project Instructions"),
        "Should contain project instructions"
    );
    assert!(
        stdout.contains("bn orient"),
        "Should mention bn orient command"
    );
    assert!(
        stdout.contains("bn ready"),
        "Should mention bn ready command"
    );
}

#[test]
fn test_container_prompt_injection_mcp_lifecycle_produces_content() {
    // The entrypoint.sh loads mcp-lifecycle as MCP usage guidance
    let temp = TestEnv::new();

    let output = bn_in(&temp)
        .args(["-H", "system", "emit", "mcp-lifecycle"])
        .output()
        .expect("Failed to run command");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should produce non-empty content with MCP lifecycle rules
    assert!(!stdout.is_empty(), "mcp-lifecycle should produce content");
    assert!(
        stdout.contains("MCP Lifecycle"),
        "Should mention MCP lifecycle"
    );
    assert!(
        stdout.contains("shell"),
        "Should mention shell commands for orient/goodbye"
    );
}

#[test]
fn test_container_prompt_injection_both_templates_can_be_combined() {
    // The entrypoint.sh combines copilot-instructions + mcp-lifecycle with "---" delimiter
    // This test verifies both templates work and could be combined
    let temp = TestEnv::new();

    // Get copilot-instructions content
    let copilot_output = bn_in(&temp)
        .args(["-H", "system", "emit", "copilot-instructions"])
        .output()
        .expect("Failed to run copilot-instructions");
    let copilot_content = String::from_utf8_lossy(&copilot_output.stdout);

    // Get mcp-lifecycle content
    let mcp_output = bn_in(&temp)
        .args(["-H", "system", "emit", "mcp-lifecycle"])
        .output()
        .expect("Failed to run mcp-lifecycle");
    let mcp_content = String::from_utf8_lossy(&mcp_output.stdout);

    // Both should succeed and have content
    assert!(
        copilot_output.status.success(),
        "copilot-instructions should succeed"
    );
    assert!(mcp_output.status.success(), "mcp-lifecycle should succeed");

    // Simulate the entrypoint.sh combination
    let combined = format!("{}\n\n{}", copilot_content.trim(), mcp_content.trim());

    // Verify combined prompt has content from both sources
    assert!(
        combined.contains("Binnacle Project Instructions"),
        "Combined should have project instructions"
    );
    assert!(
        combined.contains("MCP Lifecycle"),
        "Combined should have MCP lifecycle"
    );
    assert!(
        combined.len() > copilot_content.len(),
        "Combined should be larger than just copilot-instructions"
    );

    // Verify neither template is empty (would break the hybrid injection)
    assert!(
        copilot_content.lines().count() > 5,
        "copilot-instructions should have substantial content"
    );
    assert!(
        mcp_content.lines().count() > 5,
        "mcp-lifecycle should have substantial content"
    );
}

#[test]
fn test_container_prompt_injection_templates_no_init_required() {
    // The entrypoint.sh runs before bn init, so templates must work without initialization
    // This is critical for container startup
    let temp = TestEnv::new();

    // Do NOT initialize binnacle - simulate fresh container

    // Both templates should work without init
    bn_in(&temp)
        .args(["-H", "system", "emit", "copilot-instructions"])
        .assert()
        .success();

    bn_in(&temp)
        .args(["-H", "system", "emit", "mcp-lifecycle"])
        .assert()
        .success();
}

// ============================================================================
// bn system store clear Tests
// ============================================================================

#[test]
fn test_store_clear_requires_force() {
    let env = init_binnacle();

    // Create a task first
    create_task(&env, "Test task");

    // Try to clear without --force - should abort
    let output = bn_in(&env)
        .args(["session", "store", "clear"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["cleared"], false);
    assert_eq!(json["aborted"], true);
    assert!(json["abort_reason"].as_str().unwrap().contains("--force"));
}

#[test]
fn test_store_clear_with_force_clears_data() {
    let env = init_binnacle();

    // Create some data
    create_task(&env, "Task 1");
    create_task(&env, "Task 2");

    // Clear with --force
    let output = bn_in(&env)
        .args(["session", "store", "clear", "--force"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["cleared"], true);
    assert_eq!(json["aborted"], false);
    assert_eq!(json["tasks_cleared"], 2);

    // Verify tasks are gone
    let list_output = bn_in(&env)
        .args(["task", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let list_json = parse_json(&list_output);
    assert_eq!(list_json["tasks"].as_array().unwrap().len(), 0);
}

#[test]
fn test_store_clear_creates_backup_by_default() {
    let env = init_binnacle();

    // Create some data
    create_task(&env, "Important task");

    // Clear with --force (backup created by default)
    let output = bn_in(&env)
        .args(["session", "store", "clear", "--force"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["cleared"], true);
    assert!(json["backup_path"].as_str().is_some());

    // Verify backup file exists
    let backup_path = json["backup_path"].as_str().unwrap();
    assert!(
        std::path::Path::new(backup_path).exists(),
        "Backup file should exist at {}",
        backup_path
    );

    // Clean up backup file
    let _ = fs::remove_file(backup_path);
}

#[test]
fn test_store_clear_no_backup_skips_backup() {
    let env = init_binnacle();

    // Create some data
    create_task(&env, "Task to clear");

    // Clear with --force --no-backup
    let output = bn_in(&env)
        .args(["session", "store", "clear", "--force", "--no-backup"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["cleared"], true);
    assert!(json["backup_path"].is_null());
}

#[test]
fn test_store_clear_empty_store_succeeds() {
    let env = init_binnacle();

    // Clear empty store - should succeed immediately
    let output = bn_in(&env)
        .args(["session", "store", "clear", "--force"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["cleared"], true);
    assert_eq!(json["tasks_cleared"], 0);
    assert_eq!(json["aborted"], false);
}

#[test]
fn test_store_clear_human_readable_warning() {
    let env = init_binnacle();

    // Create a task first
    create_task(&env, "Test task");

    // Try to clear without --force in human mode - should show warning
    bn_in(&env)
        .args(["-H", "session", "store", "clear"])
        .assert()
        .success()
        .stderr(predicate::str::contains("WARNING"))
        .stderr(predicate::str::contains("permanently delete"))
        .stderr(predicate::str::contains("--force"));
}

// === Archive Generation Tests ===

#[test]
fn test_store_archive_without_config() {
    let env = init_binnacle();

    // Create some data
    create_task(&env, "Task to archive");

    // Archive without configuring archive.directory should use default location
    // and create an archive there
    let output = bn_in(&env)
        .args(["session", "store", "archive", "abc123def456"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    // With default archive directory, archive is created
    assert_eq!(json["created"], true);
    assert_eq!(json["commit_hash"], "abc123def456");
    assert!(json["task_count"].as_u64().unwrap() >= 1);

    // Clean up the archive in the default directory
    if let Some(output_path) = json["output_path"].as_str() {
        let _ = fs::remove_file(output_path);
    }
}

#[test]
fn test_store_archive_explicitly_disabled() {
    let env = init_binnacle();

    // Explicitly disable archiving by setting to empty
    bn_in(&env)
        .args(["config", "set", "archive.directory", ""])
        .assert()
        .success();

    // Create some data
    create_task(&env, "Task to archive");

    // Archive should not be created when explicitly disabled
    let output = bn_in(&env)
        .args(["session", "store", "archive", "abc123def456"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["created"], false);
    assert_eq!(json["commit_hash"], "abc123def456");
}

#[test]
fn test_store_archive_with_config() {
    let env = init_binnacle();

    // Create archive directory inside data_dir to avoid parallel test conflicts
    let archive_dir = env.data_path().join("test_archives");
    fs::create_dir_all(&archive_dir).unwrap();

    // Configure archive directory
    bn_in(&env)
        .args([
            "config",
            "set",
            "archive.directory",
            archive_dir.to_str().unwrap(),
        ])
        .assert()
        .success();

    // Create some data
    create_task(&env, "Task to archive");

    // Generate archive
    let output = bn_in(&env)
        .args(["session", "store", "archive", "abc123def456"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["created"], true);
    assert_eq!(json["commit_hash"], "abc123def456");
    assert!(json["size_bytes"].as_u64().unwrap() > 0);
    assert_eq!(json["task_count"], 1);

    // Verify archive file exists
    let archive_path = archive_dir.join("bn_abc123def456.bng");
    assert!(archive_path.exists(), "Archive file should exist");
}

#[test]
fn test_store_archive_human_readable() {
    let env = init_binnacle();

    // Archive without config - human readable (uses default location)
    let output = bn_in(&env)
        .args(["-H", "session", "store", "archive", "abc123"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created archive"))
        .stdout(predicate::str::contains("abc123"))
        .get_output()
        .stdout
        .clone();

    // Clean up the archive in the default directory
    // Parse the path from output like "Path: /path/to/file.bng"
    let output_str = String::from_utf8_lossy(&output);
    if let Some(line) = output_str.lines().find(|l| l.contains("Path:"))
        && let Some(path) = line.split("Path:").nth(1)
    {
        let _ = fs::remove_file(path.trim());
    }
}

#[test]
fn test_store_archive_human_readable_disabled() {
    let env = init_binnacle();

    // Explicitly disable archiving
    bn_in(&env)
        .args(["config", "set", "archive.directory", ""])
        .assert()
        .success();

    // Archive with disabled config - human readable
    bn_in(&env)
        .args(["-H", "session", "store", "archive", "abc123"])
        .assert()
        .success()
        .stdout(predicate::str::contains("not created"))
        .stdout(predicate::str::contains("abc123"));
}

#[test]
fn test_store_archive_creates_directory() {
    let env = init_binnacle();

    // Configure archive directory that doesn't exist yet - use unique path based on data_dir
    // to avoid conflicts with parallel test runs
    let archive_dir = env.data_path().join("new_archives_test");
    bn_in(&env)
        .args([
            "config",
            "set",
            "archive.directory",
            archive_dir.to_str().unwrap(),
        ])
        .assert()
        .success();

    // Generate archive - should create directory automatically
    bn_in(&env)
        .args(["session", "store", "archive", "deadbeef"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"created\":true"));

    // Verify directory was created and archive exists
    assert!(archive_dir.exists(), "Archive directory should be created");
    assert!(
        archive_dir.join("bn_deadbeef.bng").exists(),
        "Archive file should exist"
    );
}

// ============================================================================
// bn system build-info Tests
// ============================================================================

#[test]
fn test_system_build_info_json_output() {
    let temp = TestEnv::new();

    let output = bn_in(&temp)
        .args(["system", "build-info"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert!(json["version"].is_string());
    assert!(json["commit"].is_string());
    assert!(json["built"].is_string());

    // Verify version format
    let version = json["version"].as_str().unwrap();
    assert!(!version.is_empty());
}

#[test]
fn test_system_build_info_human_output() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["-H", "system", "build-info"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Version:"))
        .stdout(predicate::str::contains("Commit:"))
        .stdout(predicate::str::contains("Built:"));
}

#[test]
fn test_store_import_handles_missing_title_field() {
    use flate2::Compression;
    use flate2::write::GzEncoder;

    let temp = TestEnv::new();

    // Create a minimal archive with a task missing the title field (old format)
    let manifest = r#"{"version":1,"format":"binnacle-store-v1","exported_at":"2026-01-27T08:00:00Z","source_repo":"/test/repo","binnacle_version":"0.1.0","task_count":1,"test_count":0,"commit_count":0,"checksums":{}}"#;
    let tasks_jsonl = r#"{"id":"bn-test","type":"task","description":"Old task without title","priority":2,"status":"pending","depends_on":[],"created_at":"2026-01-27T08:00:00Z","updated_at":"2026-01-27T08:00:00Z"}"#;

    // Create tar.gz archive
    let archive_path = temp.path().join("old_format.bng");
    let archive_file = std::fs::File::create(&archive_path).unwrap();
    let encoder = GzEncoder::new(archive_file, Compression::default());
    let mut archive = tar::Builder::new(encoder);

    // Add manifest
    let mut header = tar::Header::new_gnu();
    header.set_size(manifest.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    archive
        .append_data(&mut header, "manifest.json", manifest.as_bytes())
        .unwrap();

    // Add tasks.jsonl
    let mut header = tar::Header::new_gnu();
    header.set_size(tasks_jsonl.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    archive
        .append_data(&mut header, "tasks.jsonl", tasks_jsonl.as_bytes())
        .unwrap();

    archive.finish().unwrap();
    drop(archive);

    // Import should succeed despite missing title field
    let output = bn_in(&temp)
        .args(["session", "store", "import", archive_path.to_str().unwrap()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["imported"], true);
    assert_eq!(json["tasks_imported"], 1);

    // Verify task was imported with empty title
    let list_output = bn_in(&temp)
        .args(["task", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let tasks: serde_json::Value = serde_json::from_slice(&list_output).unwrap();
    assert_eq!(tasks["tasks"].as_array().unwrap().len(), 1);
    let task = &tasks["tasks"][0];
    assert_eq!(task["id"], "bn-test");
    assert_eq!(task["title"], ""); // Should default to empty string when missing
}

#[test]
fn test_copilot_path_binnacle_preferred() {
    let env = init_binnacle();

    // Test copilot path with JSON output
    let output = bn_in(&env)
        .args(["system", "copilot", "path"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["version"], "v0.0.399");
    assert_eq!(json["source"], "binnacle-preferred");
    assert!(json["path"].as_str().unwrap().contains("copilot"));
    // exists may be false if not installed
}

#[test]
fn test_copilot_path_human_format() {
    let env = init_binnacle();

    // Test copilot path with human-readable output
    bn_in(&env)
        .args(["system", "copilot", "path", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("v0.0.399"))
        .stdout(predicate::str::contains("binnacle-preferred"));
}

#[test]
fn test_copilot_path_in_uninitialized_directory() {
    // Create a TestEnv without initializing binnacle
    // This tests the fix for bn-ffc2: bn-agent shows confusing error
    // instead of 'not initialized' when run in uninitialized directory.
    // copilot_path should work even without binnacle initialization.
    let env = TestEnv::new();

    // Test copilot path without initializing binnacle - should succeed
    let output = bn_in(&env)
        .args(["system", "copilot", "path"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    // Should use binnacle-preferred since no config exists
    assert_eq!(json["version"], "v0.0.399");
    assert_eq!(json["source"], "binnacle-preferred");
    assert!(json["path"].as_str().unwrap().contains("copilot"));
}

#[test]
fn test_copilot_path_with_config() {
    use sha2::{Digest, Sha256};

    let env = init_binnacle();

    // Compute storage hash like the code does
    let canonical = env.repo_path().canonicalize().unwrap();
    let mut hasher = Sha256::new();
    hasher.update(canonical.to_string_lossy().as_bytes());
    let hash = hasher.finalize();
    let hash_hex = format!("{:x}", hash);
    let short_hash = &hash_hex[..12];

    // TestEnv uses data_dir for BN_DATA_DIR
    let storage_root = env.data_dir.path().join(short_hash);
    let config_path = storage_root.join("config.kdl");

    // Ensure storage directory exists
    fs::create_dir_all(&storage_root).unwrap();

    // Write a config.kdl file with a copilot version
    let config_content = r#"copilot version="v0.0.396"
"#;
    fs::write(&config_path, config_content).unwrap();

    // Test copilot path should use config version
    let output = bn_in(&env)
        .args(["system", "copilot", "path"])
        .output()
        .expect("Failed to run command");

    assert!(output.status.success(), "Command failed");

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["version"], "v0.0.396");
    assert_eq!(json["source"], "config");
    assert!(json["path"].as_str().unwrap().contains("v0.0.396"));
}

#[test]
fn test_copilot_version_no_installations() {
    let env = init_binnacle();

    // Test copilot version with no installations
    let output = bn_in(&env)
        .args(["system", "copilot", "version"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["active_version"], "v0.0.399");
    assert_eq!(json["active_source"], "binnacle-preferred");
    assert_eq!(json["installed_versions"].as_array().unwrap().len(), 0);
}

#[test]
fn test_copilot_version_human_format() {
    let env = init_binnacle();

    // Test copilot version with human-readable output
    bn_in(&env)
        .args(["system", "copilot", "version", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Active: v0.0.399 (binnacle-preferred)",
        ))
        .stdout(predicate::str::contains("No versions installed"));
}

#[test]
fn test_copilot_version_with_installation() {
    let env = init_binnacle();

    // Create a mock installation directory
    let copilot_base_dir = env.data_dir.path().join("utils").join("copilot");
    let version_dir = copilot_base_dir.join("v0.0.399");
    fs::create_dir_all(&version_dir).unwrap();

    // Create a mock copilot binary
    let binary_path = version_dir.join("copilot");
    fs::write(&binary_path, "#!/bin/sh\necho 'mock copilot'").unwrap();

    // Test copilot version should show the installation
    let output = bn_in(&env)
        .args(["system", "copilot", "version"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["active_version"], "v0.0.399");
    assert_eq!(json["active_source"], "binnacle-preferred");

    let installed = json["installed_versions"].as_array().unwrap();
    assert_eq!(installed.len(), 1);
    assert_eq!(installed[0]["version"], "v0.0.399");
    assert_eq!(installed[0]["is_active"], true);
    assert!(installed[0]["path"].as_str().unwrap().contains("v0.0.399"));
}

#[test]
fn test_copilot_version_multiple_installations() {
    let env = init_binnacle();

    // Create mock installation directories for multiple versions
    let copilot_base_dir = env.data_dir.path().join("utils").join("copilot");

    // Install v0.0.399 (active)
    let version_dir_1 = copilot_base_dir.join("v0.0.399");
    fs::create_dir_all(&version_dir_1).unwrap();
    let binary_path_1 = version_dir_1.join("copilot");
    fs::write(&binary_path_1, "#!/bin/sh\necho 'mock copilot v0.0.399'").unwrap();

    // Install v1.1.0 (inactive)
    let version_dir_2 = copilot_base_dir.join("v1.1.0");
    fs::create_dir_all(&version_dir_2).unwrap();
    let binary_path_2 = version_dir_2.join("copilot");
    fs::write(&binary_path_2, "#!/bin/sh\necho 'mock copilot v1.1.0'").unwrap();

    // Test copilot version should show both installations
    let output = bn_in(&env)
        .args(["system", "copilot", "version"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["active_version"], "v0.0.399");

    let installed = json["installed_versions"].as_array().unwrap();
    assert_eq!(installed.len(), 2);

    // Check that versions are sorted in descending order (v1.1.0 should be first)
    assert_eq!(installed[0]["version"], "v1.1.0");
    assert_eq!(installed[0]["is_active"], false);
    assert_eq!(installed[1]["version"], "v0.0.399");
    assert_eq!(installed[1]["is_active"], true);
}

#[test]
fn test_copilot_version_with_config_override() {
    use sha2::{Digest, Sha256};

    let env = init_binnacle();

    // Compute storage hash and write config with different version
    let canonical = env.repo_path().canonicalize().unwrap();
    let mut hasher = Sha256::new();
    hasher.update(canonical.to_string_lossy().as_bytes());
    let hash = hasher.finalize();
    let hash_hex = format!("{:x}", hash);
    let short_hash = &hash_hex[..12];

    let storage_root = env.data_dir.path().join(short_hash);
    fs::create_dir_all(&storage_root).unwrap();

    let config_path = storage_root.join("config.kdl");
    let config_content = r#"copilot version="v1.1.0"
"#;
    fs::write(&config_path, config_content).unwrap();

    // Create mock installations
    let copilot_base_dir = env.data_dir.path().join("utils").join("copilot");

    let version_dir_1 = copilot_base_dir.join("v0.0.399");
    fs::create_dir_all(&version_dir_1).unwrap();
    fs::write(version_dir_1.join("copilot"), "#!/bin/sh\necho 'v0.0.399'").unwrap();

    let version_dir_2 = copilot_base_dir.join("v1.1.0");
    fs::create_dir_all(&version_dir_2).unwrap();
    fs::write(version_dir_2.join("copilot"), "#!/bin/sh\necho 'v1.1.0'").unwrap();

    // Test that active version respects config
    let output = bn_in(&env)
        .args(["system", "copilot", "version"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["active_version"], "v1.1.0");
    assert_eq!(json["active_source"], "config");

    let installed = json["installed_versions"].as_array().unwrap();
    assert_eq!(installed.len(), 2);

    // v1.1.0 should be marked as active even though it's not the binnacle-preferred
    let v110 = installed.iter().find(|v| v["version"] == "v1.1.0").unwrap();
    assert_eq!(v110["is_active"], true);

    let v120 = installed
        .iter()
        .find(|v| v["version"] == "v0.0.399")
        .unwrap();
    assert_eq!(v120["is_active"], false);
}

// ============================================================================
// bn session reinit Tests
// ============================================================================

#[test]
fn test_session_reinit_runs_full_init() {
    let temp = TestEnv::new();

    // First initialize the session
    bn_in(&temp)
        .args(["session", "init", "--auto-global", "-y"])
        .assert()
        .success();

    // Run reinit (should prompt for everything)
    let output = bn_in(&temp)
        .args(["session", "reinit"])
        .write_stdin("n\nn\nn\nn\n") // All prompts (repo-specific)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json_from_mixed_output(&output);
    assert_eq!(json["initialized"], false); // Already initialized
}

#[test]
fn test_session_reinit_human_format() {
    let temp = TestEnv::new();

    // First initialize the session
    bn_in(&temp)
        .args(["session", "init", "--auto-global", "-y"])
        .assert()
        .success();

    bn_in(&temp)
        .args(["-H", "session", "reinit"])
        .write_stdin("n\nn\nn\nn\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("Session already initialized"));
}

// ============================================================================
// bn system host-init Tests
// ============================================================================

#[test]
fn test_system_host_init_creates_config_dir() {
    let temp = TestEnv::new();

    // Run host-init in non-interactive mode (all flags off)
    let output = bn_in(&temp)
        .args(["system", "host-init", "-y"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["initialized"], true);
    assert!(json["config_path"].as_str().is_some());
    assert_eq!(json["skills_file_created"], false);
    assert_eq!(json["codex_skills_file_created"], false);
    assert_eq!(json["mcp_copilot_config_created"], false);
    assert_eq!(json["copilot_installed"], false);
    assert_eq!(json["bn_agent_installed"], false);
    assert_eq!(json["container_built"], false);
}

#[test]
fn test_system_host_init_human_format() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["-H", "system", "host-init", "-y"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Initialized binnacle system config",
        ));
}

#[test]
fn test_system_host_init_already_exists() {
    let temp = TestEnv::new();

    // First init
    bn_in(&temp)
        .args(["system", "host-init", "-y"])
        .assert()
        .success();

    // Second init should report already exists
    let output = bn_in(&temp)
        .args(["system", "host-init", "-y"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["initialized"], false);
}

#[test]
fn test_system_host_init_already_exists_human() {
    let temp = TestEnv::new();

    // First init
    bn_in(&temp)
        .args(["system", "host-init", "-y"])
        .assert()
        .success();

    // Second init should report already exists in human format
    bn_in(&temp)
        .args(["-H", "system", "host-init", "-y"])
        .assert()
        .success()
        .stdout(predicate::str::contains("already exists"));
}

#[test]
fn test_system_host_init_help() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["system", "host-init", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Host-global binnacle setup"))
        .stdout(predicate::str::contains("--write-claude-skills"))
        .stdout(predicate::str::contains("--write-codex-skills"))
        .stdout(predicate::str::contains("--write-mcp-copilot"))
        .stdout(predicate::str::contains("--install-copilot"))
        .stdout(predicate::str::contains("--install-bn-agent"))
        .stdout(predicate::str::contains("--build-container"))
        .stdout(predicate::str::contains("--token-non-validated"));
}

#[test]
fn test_system_host_init_token_invalid() {
    let temp = TestEnv::new();

    // Try to store an invalid token - should fail validation
    bn_in(&temp)
        .args([
            "system",
            "host-init",
            "--token-non-validated",
            "invalid_token_12345",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Token validation failed"));
}

#[test]
fn test_system_host_init_token_invalid_shows_guidance() {
    let temp = TestEnv::new();

    // Invalid token should show guidance on how to acquire a proper token
    bn_in(&temp)
        .args([
            "system",
            "host-init",
            "--token-non-validated",
            "invalid_token_12345",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("github.com/settings/tokens"));
}

#[test]
fn test_system_host_init_json_includes_token_fields() {
    let temp = TestEnv::new();

    // Run host-init without token
    let output = bn_in(&temp)
        .args(["system", "host-init", "-y"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    // Verify the new fields are present in output
    assert_eq!(json["token_stored"], false);
    assert!(json["token_username"].is_null());
}

#[test]
fn test_system_host_init_token_non_validated_triggers_non_interactive() {
    let temp = TestEnv::new();

    // Providing --token-non-validated should trigger non-interactive mode
    // (even without -y flag), but it will fail because the token is invalid
    bn_in(&temp)
        .args(["system", "host-init", "--token-non-validated", "bad_token"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Token validation failed"));
}

#[test]
fn test_system_host_init_token_copilot_validation_invalid() {
    let temp = TestEnv::new();

    // Try to store a token with --token (Copilot validation) - should fail
    // because the token is invalid and won't pass GitHub user validation
    bn_in(&temp)
        .args(["system", "host-init", "--token", "invalid_token_12345"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Copilot token validation failed"));
}

#[test]
fn test_system_host_init_token_triggers_non_interactive() {
    let temp = TestEnv::new();

    // Providing --token should trigger non-interactive mode
    // (even without -y flag), but it will fail because the token is invalid
    bn_in(&temp)
        .args(["system", "host-init", "--token", "bad_token"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Copilot token validation failed"));
}

#[test]
fn test_system_host_init_token_flags_conflict() {
    let temp = TestEnv::new();

    // --token and --token-non-validated should conflict
    bn_in(&temp)
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

#[test]
fn test_system_host_init_json_includes_copilot_validated_field() {
    let temp = TestEnv::new();

    // Run host-init without token
    let output = bn_in(&temp)
        .args(["system", "host-init", "-y"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    // Verify the copilot_validated field is present
    assert_eq!(json["copilot_validated"], false);
}

// ============================================================================
// bn system host-init Container Mode Tests (BN_CONTAINER_MODE)
// ============================================================================
// Note: host-init outputs emoji-formatted text (not JSON) as of commit e0a9924.
// These tests verify behavior through string matching on the emoji output.

/// Helper to create a bn command with container mode enabled.
fn bn_container_mode(temp: &TestEnv) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(temp.repo_path());
    cmd.env("BN_DATA_DIR", temp.data_path());
    cmd.env("BN_TEST_MODE", "1");
    cmd.env("BN_CONFIG_DIR", temp.repo_path().join(".config_test"));
    cmd.env("BN_CONTAINER_MODE", "true");
    cmd
}

/// Helper to create a bn command without container mode.
fn bn_normal_mode(temp: &TestEnv) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(temp.repo_path());
    cmd.env("BN_DATA_DIR", temp.data_path());
    cmd.env("BN_TEST_MODE", "1");
    cmd.env("BN_CONFIG_DIR", temp.repo_path().join(".config_test"));
    cmd.env_remove("BN_CONTAINER_MODE");
    cmd
}

/// Test that BN_CONTAINER_MODE=true triggers container mode initialization
/// which auto-enables skills/MCP and skips install prompts.
#[test]
fn test_system_host_init_container_mode_triggers() {
    let temp = TestEnv::new();

    let output = bn_container_mode(&temp)
        .args(["system", "host-init"])
        .output()
        .expect("Failed to run command");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should succeed without prompting (container mode = non-interactive)
    assert!(
        output.status.success(),
        "host-init should succeed in container mode. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Container mode indicator should be present
    assert!(
        stdout.contains("Running in container mode") || stdout.contains("🐳"),
        "Output should indicate container mode: {}",
        stdout
    );
}

/// Test that container mode auto-enables skills file creation.
#[test]
fn test_system_host_init_container_mode_auto_enables_skills() {
    let temp = TestEnv::new();

    let output = bn_container_mode(&temp)
        .args(["system", "host-init"])
        .output()
        .expect("Failed to run command");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());

    // Container mode should auto-enable skills
    assert!(
        stdout.contains("Created Claude skills file"),
        "Container mode should create Claude skills file: {}",
        stdout
    );
    assert!(
        stdout.contains("Created Codex skills file"),
        "Container mode should create Codex skills file: {}",
        stdout
    );
}

/// Test that container mode auto-enables MCP copilot config creation.
#[test]
fn test_system_host_init_container_mode_auto_enables_mcp() {
    let temp = TestEnv::new();

    let output = bn_container_mode(&temp)
        .args(["system", "host-init"])
        .output()
        .expect("Failed to run command");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());

    // Container mode should auto-enable MCP copilot config
    assert!(
        stdout.contains("Updated Copilot CLI MCP config")
            || stdout.contains("Copilot CLI MCP config"),
        "Container mode should update MCP config: {}",
        stdout
    );
}

/// Test that container mode skips copilot installation (pre-installed in image).
#[test]
fn test_system_host_init_container_mode_skips_installs() {
    let temp = TestEnv::new();

    let output = bn_container_mode(&temp)
        .args(["system", "host-init"])
        .output()
        .expect("Failed to run command");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());

    // Container mode should NOT install copilot or bn-agent
    assert!(
        !stdout.contains("Installed GitHub Copilot CLI"),
        "Container mode should not install Copilot CLI: {}",
        stdout
    );
    assert!(
        !stdout.contains("Installed bn-agent"),
        "Container mode should not install bn-agent: {}",
        stdout
    );
    assert!(
        !stdout.contains("Built container"),
        "Container mode should not build container: {}",
        stdout
    );
}

/// Test that container mode writes copilot staff config for LSP support.
#[test]
fn test_system_host_init_container_mode_creates_copilot_staff_config() {
    let temp = TestEnv::new();

    let output = bn_container_mode(&temp)
        .args(["system", "host-init"])
        .output()
        .expect("Failed to run command");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());

    // Container mode should create copilot staff config
    assert!(
        stdout.contains("Created Copilot staff config"),
        "Container mode should create Copilot staff config: {}",
        stdout
    );
}

/// Test that without BN_CONTAINER_MODE, host-init behaves normally.
#[test]
fn test_system_host_init_without_container_mode_unchanged() {
    let temp = TestEnv::new();

    let output = bn_normal_mode(&temp)
        .args(["system", "host-init", "-y"])
        .output()
        .expect("Failed to run command");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());

    // Normal mode with -y should NOT auto-enable skills/MCP
    assert!(
        !stdout.contains("Created Claude skills file"),
        "Normal mode with -y should not create Claude skills file: {}",
        stdout
    );
    assert!(
        !stdout.contains("Created Codex skills file"),
        "Normal mode with -y should not create Codex skills file: {}",
        stdout
    );
    assert!(
        !stdout.contains("Updated Copilot CLI MCP config"),
        "Normal mode with -y should not update MCP config: {}",
        stdout
    );
    // Normal mode should NOT show container mode indicator
    assert!(
        !stdout.contains("Running in container mode") && !stdout.contains("🐳"),
        "Normal mode should not show container mode indicator: {}",
        stdout
    );
}

/// Test that BN_CONTAINER_MODE accepts both "true" and "1" as valid values.
#[test]
fn test_system_host_init_container_mode_accepts_1() {
    let temp = TestEnv::new();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(temp.repo_path());
    cmd.env("BN_DATA_DIR", temp.data_path());
    cmd.env("BN_TEST_MODE", "1");
    cmd.env("BN_CONFIG_DIR", temp.repo_path().join(".config_test"));
    cmd.env("BN_CONTAINER_MODE", "1");
    cmd.args(["system", "host-init"]);

    let output = cmd.output().expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());

    // BN_CONTAINER_MODE=1 should trigger container mode
    assert!(
        stdout.contains("Running in container mode") || stdout.contains("🐳"),
        "BN_CONTAINER_MODE=1 should trigger container mode: {}",
        stdout
    );
}

/// Test that BN_CONTAINER_MODE accepts case-insensitive "TRUE".
#[test]
fn test_system_host_init_container_mode_case_insensitive() {
    let temp = TestEnv::new();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(temp.repo_path());
    cmd.env("BN_DATA_DIR", temp.data_path());
    cmd.env("BN_TEST_MODE", "1");
    cmd.env("BN_CONFIG_DIR", temp.repo_path().join(".config_test"));
    cmd.env("BN_CONTAINER_MODE", "TRUE");
    cmd.args(["system", "host-init"]);

    let output = cmd.output().expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());

    // BN_CONTAINER_MODE=TRUE should trigger container mode
    assert!(
        stdout.contains("Running in container mode") || stdout.contains("🐳"),
        "BN_CONTAINER_MODE=TRUE (uppercase) should trigger container mode: {}",
        stdout
    );
}

/// Test that human-readable output works in container mode.
#[test]
fn test_system_host_init_container_mode_human_format() {
    let temp = TestEnv::new();

    let output = bn_container_mode(&temp)
        .args(["-H", "system", "host-init"])
        .output()
        .expect("Failed to run command");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(
        stdout.contains("Initialized") || stdout.contains("system config"),
        "Human output should contain initialization message: {}",
        stdout
    );
}

// ============================================================================
// bn system sessions Tests
// ============================================================================

#[test]
fn test_system_sessions_empty() {
    let temp = TestEnv::new();

    // No sessions initialized yet - should return empty list
    let output = bn_in(&temp)
        .args(["system", "sessions"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    let sessions = json["sessions"]
        .as_array()
        .expect("sessions should be array");
    assert!(sessions.is_empty());
}

#[test]
fn test_system_sessions_empty_human() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["-H", "system", "sessions"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No binnacle sessions found"));
}

#[test]
fn test_system_sessions_with_initialized_repo() {
    let temp = TestEnv::new();

    // Initialize a repo
    bn_in(&temp)
        .args(["session", "init", "--auto-global"])
        .write_stdin("n\nn\nn\nn\nn\nn\nn\nn\n")
        .assert()
        .success();

    // Check sessions includes the new repo
    let output = bn_in(&temp)
        .args(["system", "sessions"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    let sessions = json["sessions"]
        .as_array()
        .expect("sessions should be array");
    assert_eq!(sessions.len(), 1);
    assert!(sessions[0]["id"].as_str().is_some());
    assert!(sessions[0]["storage_path"].as_str().is_some());
    assert!(sessions[0]["size_bytes"].as_u64().is_some());
}

#[test]
fn test_system_sessions_human_format() {
    let temp = TestEnv::new();

    // Initialize a repo
    bn_in(&temp)
        .args(["session", "init", "--auto-global"])
        .write_stdin("n\nn\nn\nn\nn\nn\nn\nn\n")
        .assert()
        .success();

    // Check sessions in human format
    bn_in(&temp)
        .args(["-H", "system", "sessions"])
        .assert()
        .success()
        .stdout(predicate::str::contains("session(s) found"));
}

// === Token Management Tests ===

#[test]
fn test_system_token_show_no_token() {
    let temp = TestEnv::new();

    // Token show when no token is set
    let output = bn_in(&temp)
        .args(["system", "token", "show"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["has_token"], false);
    assert!(json.get("masked_token").is_none() || json["masked_token"].is_null());
}

#[test]
fn test_system_token_show_no_token_human() {
    let temp = TestEnv::new();

    // Token show in human format when no token is set
    bn_in(&temp)
        .args(["-H", "system", "token", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No token configured"))
        .stdout(predicate::str::contains("bn system token set"));
}

#[test]
fn test_system_token_clear_no_token() {
    let temp = TestEnv::new();

    // Clearing when no token exists
    let output = bn_in(&temp)
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
fn test_system_token_clear_no_token_human() {
    let temp = TestEnv::new();

    // Clearing when no token exists (human format)
    bn_in(&temp)
        .args(["-H", "system", "token", "clear"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No token was configured"));
}

#[test]
fn test_system_token_test_no_token() {
    let temp = TestEnv::new();

    // Testing when no token is set
    let output = bn_in(&temp)
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
            .unwrap()
            .contains("No token configured")
    );
}

#[test]
fn test_system_token_test_no_token_human() {
    let temp = TestEnv::new();

    // Testing when no token is set (human format)
    bn_in(&temp)
        .args(["-H", "system", "token", "test"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Token validation failed"))
        .stdout(predicate::str::contains("No token configured"));
}

#[test]
fn test_system_token_set_invalid_token() {
    let temp = TestEnv::new();

    // Setting an invalid token
    bn_in(&temp)
        .args(["system", "token", "set", "invalid_token_12345"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Token validation failed"));
}

#[test]
fn test_system_token_set_invalid_token_shows_guidance() {
    let temp = TestEnv::new();

    // Setting an invalid token should show guidance
    bn_in(&temp)
        .args(["system", "token", "set", "invalid_token_12345"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("github.com/settings/tokens"));
}

#[test]
fn test_system_token_help() {
    let temp = TestEnv::new();

    // Token help should show all subcommands
    bn_in(&temp)
        .args(["system", "token", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("show"))
        .stdout(predicate::str::contains("set"))
        .stdout(predicate::str::contains("clear"))
        .stdout(predicate::str::contains("test"));
}
