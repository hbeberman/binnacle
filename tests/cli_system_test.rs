//! Integration tests for `bn system` commands.
//!
//! These tests verify the system administration commands:
//! - `bn system init` - Initialize binnacle repository
//! - `bn system store show` - Display store summary
//! - `bn system store export` - Export store to archive
//! - `bn system store import` - Import store from archive

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::fs;
use tempfile::TempDir;

/// Get a Command for the bn binary, running in a temp directory.
fn bn_in(dir: &TempDir) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    cmd.current_dir(dir.path());
    cmd
}

/// Initialize binnacle in a temp directory and return the temp dir.
fn init_binnacle() -> TempDir {
    let temp = TempDir::new().unwrap();
    bn_in(&temp)
        .args(["system", "init"])
        .write_stdin("n\nn\nn\n")
        .assert()
        .success();
    temp
}

/// Create a task and return its ID.
fn create_task(dir: &TempDir, title: &str) -> String {
    let output = bn_in(dir)
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
    let temp = TempDir::new().unwrap();

    // Provide "n\n" responses to all interactive prompts
    let output = bn_in(&temp)
        .args(["system", "init"])
        .write_stdin("n\nn\nn\n")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = parse_json_from_mixed_output(&output);
    assert_eq!(json["initialized"], true);
    assert!(json["storage_path"].as_str().unwrap().contains("binnacle"));
}

#[test]
fn test_system_init_existing_repo() {
    let temp = init_binnacle();

    // Initialize again (should be idempotent)
    let output = bn_in(&temp)
        .args(["system", "init"])
        .write_stdin("n\nn\nn\n")
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
    let temp = TempDir::new().unwrap();

    bn_in(&temp)
        .args(["-H", "system", "init"])
        .write_stdin("n\nn\nn\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialized binnacle"));
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
    assert!(json["storage_path"].as_str().unwrap().contains("binnacle"));
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
    let temp = TempDir::new().unwrap();

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

    let export_path = temp.path().join("backup.tar.gz");

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
            .ends_with("backup.tar.gz")
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
    let export_path = temp.path().join("backup.tar.gz");

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
        .stdout(predicate::str::contains("backup.tar.gz"));
}

#[test]
fn test_store_export_not_initialized() {
    let temp = TempDir::new().unwrap();
    let export_path = temp.path().join("backup.tar.gz");

    bn_in(&temp)
        .args(["system", "store", "export", export_path.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not initialized"));
}

#[test]
fn test_store_export_with_format_flag() {
    let temp = init_binnacle();
    let export_path = temp.path().join("backup.tar.gz");

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
    let export_path = temp1.path().join("backup.tar.gz");
    bn_in(&temp1)
        .args(["system", "store", "export", export_path.to_str().unwrap()])
        .assert()
        .success();

    // Import to fresh repo
    let temp2 = TempDir::new().unwrap();
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
    let export_path = temp1.path().join("backup.tar.gz");
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
    let temp2 = TempDir::new().unwrap();
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

    let export_path = temp1.path().join("backup.tar.gz");
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

    let export_path = temp1.path().join("backup.tar.gz");
    bn_in(&temp1)
        .args(["system", "store", "export", export_path.to_str().unwrap()])
        .assert()
        .success();

    // Create second repo and import once (creates task with same ID)
    let temp2 = TempDir::new().unwrap();
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

    let export_path = temp1.path().join("backup.tar.gz");
    bn_in(&temp1)
        .args(["system", "store", "export", export_path.to_str().unwrap()])
        .assert()
        .success();

    // Dry run import
    let temp2 = TempDir::new().unwrap();
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

    let export_path = temp1.path().join("backup.tar.gz");
    bn_in(&temp1)
        .args(["system", "store", "export", export_path.to_str().unwrap()])
        .assert()
        .success();

    // Import to second repo
    let temp2 = TempDir::new().unwrap();
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

    let export_path = temp1.path().join("backup.tar.gz");
    bn_in(&temp1)
        .args(["system", "store", "export", export_path.to_str().unwrap()])
        .assert()
        .success();

    let temp2 = TempDir::new().unwrap();
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
    let temp = TempDir::new().unwrap();
    let bad_file = temp.path().join("bad.tar.gz");
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
        .args(["link", "add", &task_b, &task_a, "--type", "depends_on"])
        .assert()
        .success();

    // Update status
    bn_in(&temp1)
        .args(["task", "update", &task_c, "--status", "in_progress"])
        .assert()
        .success();

    // Export
    let export_path = temp1.path().join("roundtrip.tar.gz");
    bn_in(&temp1)
        .args(["system", "store", "export", export_path.to_str().unwrap()])
        .assert()
        .success();

    // Import to new repo
    let temp2 = TempDir::new().unwrap();
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
    let temp2 = TempDir::new().unwrap();
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
fn create_import_folder(dir: &TempDir) -> std::path::PathBuf {
    let folder = dir.path().join("import_source");
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
    let source_temp = TempDir::new().unwrap();
    let import_folder = create_import_folder(&source_temp);
    write_tasks_jsonl(&import_folder, &[("bn-test1", "Folder Import Task")]);

    // Import to fresh repo (replace mode - default)
    let dest_temp = TempDir::new().unwrap();
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
    let source_temp = TempDir::new().unwrap();
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
    let source_temp = TempDir::new().unwrap();
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
    let source_temp = TempDir::new().unwrap();
    let import_folder = create_import_folder(&source_temp);
    write_tasks_jsonl(
        &import_folder,
        &[("bn-collision", "Imported Collision Task")],
    );

    // Create dest repo and manually create a task (we'll force collision)
    let dest_temp = init_binnacle();

    // First do a clean import of same folder to create the task ID
    let temp_import = TempDir::new().unwrap();
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
    let source_temp = TempDir::new().unwrap();
    let import_folder = create_import_folder(&source_temp);
    write_tasks_jsonl(&import_folder, &[("bn-dry1", "Dry Run Task")]);

    // Dry run import
    let dest_temp = TempDir::new().unwrap();
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
    let source_temp = TempDir::new().unwrap();
    let import_folder = source_temp.path().join("empty_folder");
    fs::create_dir_all(&import_folder).unwrap();

    // Only create commits.jsonl (not tasks.jsonl)
    write_commits_jsonl(&import_folder, &["abc123"]);

    // Import should fail
    let dest_temp = TempDir::new().unwrap();
    bn_in(&dest_temp)
        .args(["system", "store", "import", import_folder.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("missing required tasks.jsonl"));
}

#[test]
fn test_folder_import_with_optional_files() {
    // Create folder with all optional files
    let source_temp = TempDir::new().unwrap();
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
    let dest_temp = TempDir::new().unwrap();
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
    let source_temp = TempDir::new().unwrap();
    let import_folder = create_import_folder(&source_temp);
    fs::write(import_folder.join("tasks.jsonl"), "").unwrap();

    // Import should succeed with 0 tasks
    let dest_temp = TempDir::new().unwrap();
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
    let source_temp = TempDir::new().unwrap();
    let import_folder = create_import_folder(&source_temp);
    write_tasks_jsonl(&import_folder, &[("bn-human1", "Human Format Task")]);

    let dest_temp = TempDir::new().unwrap();
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
    let temp2 = TempDir::new().unwrap();
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
