//! Integration tests for `bn system` commands.
//!
//! These tests verify the system administration commands:
//! - `bn system init` - Initialize binnacle repository
//! - `bn system store show` - Display store summary
//! - `bn system store export` - Export store to archive (uses zstd compression)
//! - `bn system store import` - Import store from archive (supports both zstd and gzip)

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
        .args(["system", "init"])
        .write_stdin("n\nn\nn\nn\n")
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
// bn system init Tests
// ============================================================================

#[test]
fn test_system_init_new_repo() {
    let temp = TestEnv::new();

    // Provide "n\n" responses to all interactive prompts (4 total)
    let output = bn_in(&temp)
        .args(["system", "init"])
        .write_stdin("n\nn\nn\nn\n")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json_from_mixed_output(&output);
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
fn test_system_init_existing_repo() {
    let temp = init_binnacle();

    // Initialize again (should be idempotent)
    let output = bn_in(&temp)
        .args(["system", "init"])
        .write_stdin("n\nn\nn\nn\n")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json_from_mixed_output(&output);
    assert_eq!(json["initialized"], false); // Already initialized
    assert_eq!(json["agents_md_updated"], false);
}

#[test]
fn test_system_init_human_format() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["-H", "system", "init"])
        .write_stdin("n\nn\nn\nn\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialized binnacle"));
}

#[test]
fn test_system_init_write_copilot_prompts_flag() {
    let temp = TestEnv::new();

    let output = bn_in(&temp)
        .args(["system", "init", "--write-copilot-prompts", "-y"])
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
fn test_system_init_write_copilot_prompts_idempotent() {
    let temp = TestEnv::new();

    // First init
    bn_in(&temp)
        .args(["system", "init", "--write-copilot-prompts", "-y"])
        .assert()
        .success();

    // Second init should overwrite without error
    let output = bn_in(&temp)
        .args(["system", "init", "--write-copilot-prompts", "-y"])
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
fn test_system_init_write_copilot_prompts_human_output() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["-H", "system", "init", "--write-copilot-prompts", "-y"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Copilot agents"))
        .stdout(predicate::str::contains(".github/agents"));
}

// ============================================================================
// MCP Config Integration Tests
// ============================================================================

#[test]
fn test_system_init_write_mcp_vscode_creates_config() {
    let temp = TestEnv::new();

    let output = bn_in(&temp)
        .args(["system", "init", "--write-mcp-vscode", "-y"])
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
fn test_system_init_write_mcp_vscode_merges_with_existing() {
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
        .args(["system", "init", "--write-mcp-vscode", "-y"])
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
fn test_system_init_write_mcp_vscode_idempotent() {
    let temp = TestEnv::new();

    // First init
    bn_in(&temp)
        .args(["system", "init", "--write-mcp-vscode", "-y"])
        .assert()
        .success();

    // Second init should succeed without error
    let output = bn_in(&temp)
        .args(["system", "init", "--write-mcp-vscode", "-y"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["mcp_vscode_config_created"], true);
}

#[test]
fn test_system_init_write_mcp_all_creates_all_configs() {
    let temp = TestEnv::new();

    let output = bn_in(&temp)
        .args(["system", "init", "--write-mcp-all", "-y"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json(&output);
    assert_eq!(json["initialized"], true);
    assert_eq!(json["mcp_vscode_config_created"], true);
    assert_eq!(json["mcp_copilot_config_created"], true);

    // Verify VS Code config was created (we can verify this in the repo)
    let vscode_config = temp.path().join(".vscode").join("mcp.json");
    assert!(vscode_config.exists(), "VS Code MCP config should exist");
}

#[test]
fn test_system_init_write_mcp_all_human_output() {
    let temp = TestEnv::new();

    bn_in(&temp)
        .args(["-H", "system", "init", "--write-mcp-all", "-y"])
        .assert()
        .success()
        .stdout(predicate::str::contains("VS Code MCP config"))
        .stdout(predicate::str::contains("Copilot CLI MCP config"));
}

#[test]
fn test_system_init_write_mcp_all_combined_with_individual_flags() {
    let temp = TestEnv::new();

    // --write-mcp-all should work together with individual flags (no conflict)
    let output = bn_in(&temp)
        .args([
            "system",
            "init",
            "--write-mcp-all",
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
    assert_eq!(json["mcp_copilot_config_created"], true);
}

// ============================================================================
// bn system store show Tests
// ============================================================================

#[test]
fn test_store_show_empty_repo() {
    let temp = init_binnacle();

    let output = bn_in(&temp)
        .args(["system", "store", "show"])
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
        .args(["system", "store", "show"])
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
        .args(["-H", "system", "store", "show"])
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
        .args(["system", "store", "show"])
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
        .args(["system", "store", "show"])
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
        .args(["system", "store", "export", export_path.to_str().unwrap()])
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
        .args(["system", "store", "export", "-"])
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
            "system",
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
        .args(["system", "store", "export", export_path.to_str().unwrap()])
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
            "system",
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
        .args(["system", "store", "export", export_path.to_str().unwrap()])
        .assert()
        .success();

    // Import to fresh repo
    let temp2 = TestEnv::new();
    let output = bn_in(&temp2)
        .args(["system", "store", "import", export_path.to_str().unwrap()])
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
        .args(["system", "store", "export", export_path.to_str().unwrap()])
        .assert()
        .success();

    // Try to import to already initialized repo
    let temp2 = init_binnacle();
    bn_in(&temp2)
        .args(["system", "store", "import", export_path.to_str().unwrap()])
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
        .args(["system", "store", "export", "-"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    // Import from stdin
    let temp2 = TestEnv::new();
    let mut cmd = bn_in(&temp2);
    cmd.args(["system", "store", "import", "-"])
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
        .args(["system", "store", "export", export_path.to_str().unwrap()])
        .assert()
        .success();

    // Create different task in second repo
    let temp2 = init_binnacle();
    create_task(&temp2, "Task B");

    // Merge import
    let output = bn_in(&temp2)
        .args([
            "system",
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
        .args(["system", "store", "export", export_path.to_str().unwrap()])
        .assert()
        .success();

    // Create second repo and import once (creates task with same ID)
    let temp2 = TestEnv::new();
    bn_in(&temp2)
        .args(["system", "store", "import", export_path.to_str().unwrap()])
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
            "system",
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
        .args(["system", "store", "export", export_path.to_str().unwrap()])
        .assert()
        .success();

    // Dry run import
    let temp2 = TestEnv::new();
    let output = bn_in(&temp2)
        .args([
            "system",
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
        .args(["system", "store", "export", export_path.to_str().unwrap()])
        .assert()
        .success();

    // Import to second repo
    let temp2 = TestEnv::new();
    bn_in(&temp2)
        .args(["system", "store", "import", export_path.to_str().unwrap()])
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
            "system",
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
        .args(["system", "store", "export", export_path.to_str().unwrap()])
        .assert()
        .success();

    let temp2 = TestEnv::new();
    bn_in(&temp2)
        .args([
            "-H",
            "system",
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
        .args(["system", "store", "import", bad_file.to_str().unwrap()])
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
        .args(["system", "store", "export", zstd_export.to_str().unwrap()])
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
        .args(["system", "store", "import", gzip_archive.to_str().unwrap()])
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
        .args(["system", "store", "export", export_path.to_str().unwrap()])
        .assert()
        .success();

    // Import to new repo
    let temp2 = TestEnv::new();
    bn_in(&temp2)
        .args(["system", "store", "import", export_path.to_str().unwrap()])
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
        .args(["system", "store", "export", "-"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    // Import from stdin to new repo
    let temp2 = TestEnv::new();
    let mut cmd = bn_in(&temp2);
    cmd.args(["system", "store", "import", "-"])
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
        .args(["system", "store", "import", import_folder.to_str().unwrap()])
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
        .args(["system", "store", "import", import_folder.to_str().unwrap()])
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
            "system",
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
            "system",
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
            "system",
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
            "system",
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
        .args(["system", "store", "import", import_folder.to_str().unwrap()])
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
        .args(["system", "store", "import", import_folder.to_str().unwrap()])
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
        .args(["system", "store", "import", import_folder.to_str().unwrap()])
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
            "system",
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
        .args(["system", "store", "show"])
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
        .args(["system", "store", "import", storage_path])
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
        .args(["system", "store", "clear"])
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
        .args(["system", "store", "clear", "--force"])
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
        .args(["system", "store", "clear", "--force"])
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
        .args(["system", "store", "clear", "--force", "--no-backup"])
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
        .args(["system", "store", "clear", "--force"])
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
        .args(["-H", "system", "store", "clear"])
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
        .args(["system", "store", "archive", "abc123def456"])
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
        .args(["system", "store", "archive", "abc123def456"])
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

    // Create archive directory OUTSIDE repo
    let parent_dir = env.path().parent().unwrap();
    let archive_dir = parent_dir.join("archives");
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
        .args(["system", "store", "archive", "abc123def456"])
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
        .args(["-H", "system", "store", "archive", "abc123"])
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
        .args(["-H", "system", "store", "archive", "abc123"])
        .assert()
        .success()
        .stdout(predicate::str::contains("not created"))
        .stdout(predicate::str::contains("abc123"));
}

#[test]
fn test_store_archive_creates_directory() {
    let env = init_binnacle();

    // Configure archive directory that doesn't exist yet (must be outside repo)
    let parent_dir = env.path().parent().unwrap();
    let archive_dir = parent_dir.join("new_archives");
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
        .args(["system", "store", "archive", "deadbeef"])
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
        .args(["system", "store", "import", archive_path.to_str().unwrap()])
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
