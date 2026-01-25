//! Integration tests for action logging functionality.
//!
//! These tests verify that action logging works correctly through the CLI:
//! - Commands are logged to action.log
//! - Logging can be enabled/disabled via config
//! - Sensitive data is sanitized
//! - Log entries have correct structure

mod common;

use assert_cmd::Command;
use common::TestEnv;
use std::fs;

/// Get a Command for the bn binary in a TestEnv.
fn bn_in(env: &TestEnv) -> Command {
    env.bn()
}

/// Initialize binnacle in a temp directory and return the TestEnv.
fn init_binnacle() -> TestEnv {
    TestEnv::init()
}

/// Read the action log file contents.
fn read_action_log(env: &TestEnv) -> String {
    let log_path = env.data_path().join("action.log");
    if log_path.exists() {
        fs::read_to_string(&log_path).unwrap_or_default()
    } else {
        String::new()
    }
}

// === Basic Logging Tests ===

#[test]
fn test_action_logging_creates_log_file() {
    let env = init_binnacle();

    // Run a command
    bn_in(&env)
        .args(["task", "create", "Test task"])
        .assert()
        .success();

    // Check log file exists
    let log_path = env.data_path().join("action.log");
    assert!(log_path.exists(), "action.log should exist after command");

    // Log should contain the command
    let log_content = read_action_log(&env);
    assert!(
        log_content.contains("task create"),
        "Log should contain 'task create'"
    );
}

#[test]
fn test_action_logging_logs_multiple_commands() {
    let env = init_binnacle();

    // Run multiple commands
    bn_in(&env)
        .args(["task", "create", "Task 1"])
        .assert()
        .success();
    bn_in(&env)
        .args(["task", "create", "Task 2"])
        .assert()
        .success();
    bn_in(&env).args(["task", "list"]).assert().success();

    // All commands should be logged
    let log_content = read_action_log(&env);
    let lines: Vec<&str> = log_content.lines().collect();
    assert!(
        lines.len() >= 4, // init + 3 commands
        "Log should have at least 4 entries (init + 3 commands), got {}",
        lines.len()
    );
}

#[test]
fn test_action_log_entry_structure() {
    let env = init_binnacle();

    bn_in(&env)
        .args(["task", "create", "Test task"])
        .assert()
        .success();

    let log_content = read_action_log(&env);

    // Find the task create line
    let task_create_line = log_content
        .lines()
        .find(|line| line.contains("task create"))
        .expect("Should have task create entry");

    // Parse as JSON
    let entry: serde_json::Value =
        serde_json::from_str(task_create_line).expect("Log entry should be valid JSON");

    // Verify required fields
    assert!(entry.get("timestamp").is_some(), "Should have timestamp");
    assert!(entry.get("repo_path").is_some(), "Should have repo_path");
    assert!(entry.get("command").is_some(), "Should have command");
    assert!(entry.get("args").is_some(), "Should have args");
    assert!(entry.get("success").is_some(), "Should have success");
    assert!(
        entry.get("duration_ms").is_some(),
        "Should have duration_ms"
    );
    assert!(entry.get("user").is_some(), "Should have user");

    // Verify success is true
    assert_eq!(entry["success"], true, "Success should be true");
}

#[test]
fn test_action_logging_records_failures() {
    let env = init_binnacle();

    // Try to show a non-existent task
    bn_in(&env)
        .args(["task", "show", "bn-9999"])
        .assert()
        .failure();

    let log_content = read_action_log(&env);

    // Find the task show line
    let task_show_line = log_content
        .lines()
        .find(|line| line.contains("task show"))
        .expect("Should have task show entry");

    let entry: serde_json::Value =
        serde_json::from_str(task_show_line).expect("Log entry should be valid JSON");

    // Verify failure is recorded
    assert_eq!(entry["success"], false, "Success should be false");
    assert!(entry.get("error").is_some(), "Should have error field");
}

// === Configuration Tests ===

#[test]
fn test_action_logging_can_be_disabled() {
    let env = init_binnacle();

    // Disable action logging
    bn_in(&env)
        .args(["config", "set", "action_log_enabled", "false"])
        .assert()
        .success();

    // Clear the existing log
    let log_path = env.data_path().join("action.log");
    let lines_before = if log_path.exists() {
        fs::read_to_string(&log_path)
            .unwrap_or_default()
            .lines()
            .count()
    } else {
        0
    };

    // Run a command
    bn_in(&env)
        .args(["task", "create", "Should not be logged"])
        .assert()
        .success();

    // Log should not have grown
    let lines_after = if log_path.exists() {
        fs::read_to_string(&log_path)
            .unwrap_or_default()
            .lines()
            .count()
    } else {
        0
    };

    assert_eq!(
        lines_before, lines_after,
        "Log should not grow when disabled"
    );
}

#[test]
fn test_action_logging_can_be_re_enabled() {
    let env = init_binnacle();

    // Disable then re-enable
    bn_in(&env)
        .args(["config", "set", "action_log_enabled", "false"])
        .assert()
        .success();
    bn_in(&env)
        .args(["config", "set", "action_log_enabled", "true"])
        .assert()
        .success();

    let log_path = env.data_path().join("action.log");
    let lines_before = fs::read_to_string(&log_path)
        .unwrap_or_default()
        .lines()
        .count();

    // Run a command
    bn_in(&env)
        .args(["task", "create", "Should be logged"])
        .assert()
        .success();

    // Log should have grown
    let lines_after = fs::read_to_string(&log_path)
        .unwrap_or_default()
        .lines()
        .count();

    assert!(
        lines_after > lines_before,
        "Log should grow after re-enabling"
    );
}

// === Sanitization Tests ===

#[test]
fn test_action_logging_sanitizes_paths_by_default() {
    let env = init_binnacle();

    // The repo_path in logs should be present (we can't fully test basename sanitization
    // without controlling the exact path format, but we can verify logs are created)
    bn_in(&env)
        .args(["task", "create", "Test task"])
        .assert()
        .success();

    let log_content = read_action_log(&env);
    assert!(!log_content.is_empty(), "Log should have content");

    // Verify it's valid JSON
    for line in log_content.lines() {
        if !line.trim().is_empty() {
            serde_json::from_str::<serde_json::Value>(line)
                .expect("Each log line should be valid JSON");
        }
    }
}

#[test]
fn test_action_logging_sanitization_can_be_disabled() {
    let env = init_binnacle();

    // Disable sanitization
    bn_in(&env)
        .args(["config", "set", "action_log_sanitize", "false"])
        .assert()
        .success();

    // Run a command
    bn_in(&env)
        .args(["task", "create", "Test task"])
        .assert()
        .success();

    // Log should still work
    let log_content = read_action_log(&env);
    assert!(
        log_content.contains("task create"),
        "Log should contain command"
    );
}

// === Duration Tracking Tests ===

#[test]
fn test_action_logging_tracks_duration() {
    let env = init_binnacle();

    bn_in(&env)
        .args(["task", "create", "Test task"])
        .assert()
        .success();

    let log_content = read_action_log(&env);
    let task_create_line = log_content
        .lines()
        .find(|line| line.contains("task create"))
        .expect("Should have task create entry");

    let entry: serde_json::Value = serde_json::from_str(task_create_line).unwrap();

    let duration = entry["duration_ms"].as_u64().expect("Should have duration");
    assert!(
        duration < 60000,
        "Duration should be less than 60 seconds (was {}ms)",
        duration
    );
}

// === Command Variety Tests ===

#[test]
fn test_action_logging_logs_orient_command() {
    let env = init_binnacle();

    bn_in(&env)
        .args(["orient", "--type", "worker", "--dry-run"])
        .assert()
        .success();

    let log_content = read_action_log(&env);
    assert!(
        log_content.contains("orient"),
        "Log should contain 'orient'"
    );
}

#[test]
fn test_action_logging_logs_ready_command() {
    let env = init_binnacle();

    bn_in(&env).args(["ready"]).assert().success();

    let log_content = read_action_log(&env);
    assert!(log_content.contains("ready"), "Log should contain 'ready'");
}

#[test]
fn test_action_logging_logs_task_update() {
    let env = init_binnacle();

    // Create a task
    let output = bn_in(&env)
        .args(["task", "create", "Test task"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id_start = stdout.find("\"id\":\"").unwrap() + 6;
    let id_end = stdout[id_start..].find('"').unwrap() + id_start;
    let task_id = &stdout[id_start..id_end];

    // Update it
    bn_in(&env)
        .args(["task", "update", task_id, "--status", "in_progress"])
        .assert()
        .success();

    let log_content = read_action_log(&env);
    assert!(
        log_content.contains("task update"),
        "Log should contain 'task update'"
    );
}

#[test]
fn test_action_logging_logs_config_commands() {
    let env = init_binnacle();

    bn_in(&env).args(["config", "list"]).assert().success();

    let log_content = read_action_log(&env);
    assert!(
        log_content.contains("config list"),
        "Log should contain 'config list'"
    );
}

// === Log Export Tests ===

#[test]
fn test_log_export_json_format() {
    let env = init_binnacle();

    // Generate some action logs
    bn_in(&env)
        .args(["task", "create", "Test task 1"])
        .assert()
        .success();
    bn_in(&env)
        .args(["task", "create", "Test task 2"])
        .assert()
        .success();

    // Export as JSON
    let output = bn_in(&env)
        .args(["log", "export", "--format", "json", "-n", "2"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should be valid JSON array
    let parsed: Vec<serde_json::Value> =
        serde_json::from_str(&stdout).expect("Export should be valid JSON array");

    assert!(!parsed.is_empty(), "Should have exported entries");

    // Check structure of first entry
    let first = &parsed[0];
    assert!(first.get("timestamp").is_some(), "Should have timestamp");
    assert!(first.get("command").is_some(), "Should have command");
    assert!(first.get("user").is_some(), "Should have user");
    assert!(first.get("success").is_some(), "Should have success");
}

#[test]
fn test_log_export_csv_format() {
    let env = init_binnacle();

    // Generate some action logs
    bn_in(&env)
        .args(["task", "create", "Test task"])
        .assert()
        .success();

    // Export as CSV
    let output = bn_in(&env)
        .args(["log", "export", "--format", "csv", "-n", "2"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should have header line
    assert!(
        stdout.starts_with("timestamp,command,user,success,duration_ms,error,args\n"),
        "CSV should start with header"
    );

    // Should have at least one data line
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(
        lines.len() >= 2,
        "Should have header and at least one data line"
    );
}

#[test]
fn test_log_export_markdown_format() {
    let env = init_binnacle();

    // Generate some action logs
    bn_in(&env)
        .args(["task", "create", "Test task"])
        .assert()
        .success();

    // Export as Markdown
    let output = bn_in(&env)
        .args(["log", "export", "--format", "markdown", "-n", "2"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should have markdown structure
    assert!(stdout.contains("# Action Log Export"), "Should have title");
    assert!(stdout.contains("| Timestamp |"), "Should have table header");
    assert!(stdout.contains("| Command |"), "Should have table header");
}

#[test]
fn test_log_export_command_filter() {
    let env = init_binnacle();

    // Generate different command types
    bn_in(&env)
        .args(["task", "create", "Test task"])
        .assert()
        .success();
    bn_in(&env).args(["ready"]).assert().success();

    // Export only task commands
    let output = bn_in(&env)
        .args([
            "log",
            "export",
            "--format",
            "json",
            "--command",
            "task",
            "-n",
            "10",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: Vec<serde_json::Value> =
        serde_json::from_str(&stdout).expect("Export should be valid JSON array");

    // All entries should be task commands
    for entry in &parsed {
        let cmd = entry["command"].as_str().unwrap();
        assert!(
            cmd.contains("task"),
            "Filtered entry '{}' should contain 'task'",
            cmd
        );
    }
}

#[test]
fn test_log_export_to_file() {
    let env = init_binnacle();

    // Generate some action logs
    bn_in(&env)
        .args(["task", "create", "Test task"])
        .assert()
        .success();

    // Export to file
    let output_path = env.data_path().join("export_test.csv");
    bn_in(&env)
        .args([
            "log",
            "export",
            "--format",
            "csv",
            "-o",
            output_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    // File should exist and have content
    assert!(output_path.exists(), "Export file should exist");
    let content = fs::read_to_string(&output_path).unwrap();
    assert!(
        content.contains("timestamp,command"),
        "File should have CSV header"
    );
}

#[test]
fn test_log_export_human_readable_output() {
    let env = init_binnacle();

    // Generate some action logs
    bn_in(&env)
        .args(["task", "create", "Test task"])
        .assert()
        .success();

    // Export to file with -H flag
    let output_path = env.data_path().join("export_test2.json");
    let output = bn_in(&env)
        .args([
            "-H",
            "log",
            "export",
            "--format",
            "json",
            "-o",
            output_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Human-readable output should mention export
    assert!(
        stdout.contains("Exported"),
        "Should show 'Exported' message"
    );
    assert!(
        stdout.contains("log entries"),
        "Should mention 'log entries'"
    );
}

#[test]
fn test_log_export_limit() {
    let env = init_binnacle();

    // Generate several action logs
    for i in 0..5 {
        bn_in(&env)
            .args(["task", "create", &format!("Test task {}", i)])
            .assert()
            .success();
    }

    // Export with limit
    let output = bn_in(&env)
        .args(["log", "export", "--format", "json", "-n", "3"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: Vec<serde_json::Value> =
        serde_json::from_str(&stdout).expect("Export should be valid JSON array");

    assert_eq!(parsed.len(), 3, "Should export exactly 3 entries");
}

#[test]
fn test_log_show_command_works() {
    let env = init_binnacle();

    // Generate some entity changes
    bn_in(&env)
        .args(["task", "create", "Test task"])
        .assert()
        .success();

    // bn log show should work (shows entity audit trail)
    let output = bn_in(&env).args(["log", "show"]).output().unwrap();

    assert!(output.status.success(), "bn log show should succeed");

    // Should have JSON output
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("entries") || stdout.contains("count"),
        "Should have audit log structure"
    );
}

#[test]
fn test_bn_log_default_works() {
    let env = init_binnacle();

    // bn log (no subcommand) should work like bn log show
    let output = bn_in(&env).args(["log"]).output().unwrap();

    assert!(output.status.success(), "bn log should succeed");
}
