//! Integration tests for Issue CLI operations.
//!
//! These tests verify that issue commands work correctly through the CLI:
//! - `bn issue create/list/show/update/close/reopen/delete` all work
//! - JSON and human-readable output formats are correct
//! - Filtering by status, priority, and tags works
//! - Issues are excluded from bn ready and bn orient
//! - Auto-resolution of parent issues when child bugs are closed

mod common;

use assert_cmd::Command;
use common::TestEnv;
use predicates::prelude::*;

/// Get a Command for the bn binary in a TestEnv.
fn bn_in(env: &TestEnv) -> Command {
    env.bn()
}

/// Initialize binnacle in a temp directory and return the TestEnv.
fn init_binnacle() -> TestEnv {
    TestEnv::init()
}

// === Issue Create Tests ===

#[test]
fn test_issue_create_json() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["issue", "create", "My first issue"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"id\":\"bn-"))
        .stdout(predicate::str::contains("\"title\":\"My first issue\""));
}

#[test]
fn test_issue_create_human() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["-H", "issue", "create", "My first issue"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created issue bn-"))
        .stdout(predicate::str::contains("\"My first issue\""));
}

#[test]
fn test_issue_create_with_options() {
    let temp = init_binnacle();

    // Create issue with options
    let output = bn_in(&temp)
        .args([
            "issue",
            "create",
            "Priority issue",
            "-p",
            "1",
            "-t",
            "backend",
            "-t",
            "investigation",
            "-a",
            "agent-claude",
            "-d",
            "This issue needs investigation",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let issue_id = json["id"].as_str().unwrap();

    // Verify options were saved by checking the issue show output
    bn_in(&temp)
        .args(["issue", "show", issue_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"priority\":1"))
        .stdout(predicate::str::contains(
            "\"tags\":[\"backend\",\"investigation\"]",
        ))
        .stdout(predicate::str::contains("\"assignee\":\"agent-claude\""));
}

#[test]
fn test_issue_create_with_short_name() {
    let temp = init_binnacle();

    // Create issue with short name
    let output = bn_in(&temp)
        .args(["issue", "create", "Full title issue", "-s", "short-issue"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let issue_id = json["id"].as_str().unwrap();

    // Verify short name was saved
    bn_in(&temp)
        .args(["issue", "show", issue_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"short_name\":\"short-issue\""));
}

#[test]
fn test_issue_create_invalid_priority() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["issue", "create", "Bad priority", "-p", "5"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Priority must be 0-4"));
}

// === Issue List Tests ===

#[test]
fn test_issue_list_empty() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["issue", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":0"));
}

#[test]
fn test_issue_list_with_issues() {
    let temp = init_binnacle();

    // Create some issues
    bn_in(&temp)
        .args(["issue", "create", "Issue 1", "-p", "1"])
        .assert()
        .success();
    bn_in(&temp)
        .args(["issue", "create", "Issue 2", "-p", "2"])
        .assert()
        .success();

    bn_in(&temp)
        .args(["issue", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":2"));
}

#[test]
fn test_issue_list_human() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["issue", "create", "Issue 1"])
        .assert()
        .success();

    bn_in(&temp)
        .args(["-H", "issue", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 issue:"))
        .stdout(predicate::str::contains("Issue 1"));
}

#[test]
fn test_issue_list_filter_by_priority() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["issue", "create", "High priority", "-p", "1"])
        .assert()
        .success();
    bn_in(&temp)
        .args(["issue", "create", "Low priority", "-p", "3"])
        .assert()
        .success();

    bn_in(&temp)
        .args(["issue", "list", "--priority", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":1"))
        .stdout(predicate::str::contains("High priority"));
}

#[test]
fn test_issue_list_filter_by_tag() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["issue", "create", "Backend issue", "-t", "backend"])
        .assert()
        .success();
    bn_in(&temp)
        .args(["issue", "create", "Frontend issue", "-t", "frontend"])
        .assert()
        .success();

    bn_in(&temp)
        .args(["issue", "list", "--tag", "backend"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":1"))
        .stdout(predicate::str::contains("Backend issue"));
}

#[test]
fn test_issue_list_filter_by_status() {
    let temp = init_binnacle();

    // Create an issue
    let output = bn_in(&temp)
        .args(["issue", "create", "Issue to investigate"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let issue_id = json["id"].as_str().unwrap();

    // Update status to investigating
    bn_in(&temp)
        .args(["issue", "update", issue_id, "--status", "investigating"])
        .assert()
        .success();

    // Create another issue in open status
    bn_in(&temp)
        .args(["issue", "create", "Open issue"])
        .assert()
        .success();

    // Filter by investigating status
    bn_in(&temp)
        .args(["issue", "list", "--status", "investigating"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":1"))
        .stdout(predicate::str::contains("Issue to investigate"));
}

#[test]
fn test_issue_list_excludes_closed_by_default() {
    let temp = init_binnacle();

    // Create an issue and close it
    let output = bn_in(&temp)
        .args(["issue", "create", "Issue to close"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let issue_id = json["id"].as_str().unwrap();

    bn_in(&temp)
        .args(["issue", "close", issue_id, "--reason", "Resolved"])
        .assert()
        .success();

    // Create an open issue
    bn_in(&temp)
        .args(["issue", "create", "Open issue"])
        .assert()
        .success();

    // Default list should only show open issue
    bn_in(&temp)
        .args(["issue", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":1"))
        .stdout(predicate::str::contains("Open issue"));
}

#[test]
fn test_issue_list_all_includes_closed() {
    let temp = init_binnacle();

    // Create an issue and close it
    let output = bn_in(&temp)
        .args(["issue", "create", "Issue to close"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let issue_id = json["id"].as_str().unwrap();

    bn_in(&temp)
        .args(["issue", "close", issue_id, "--reason", "Resolved"])
        .assert()
        .success();

    // Create an open issue
    bn_in(&temp)
        .args(["issue", "create", "Open issue"])
        .assert()
        .success();

    // --all flag should show both
    bn_in(&temp)
        .args(["issue", "list", "--all"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":2"));
}

// === Issue Show Tests ===

#[test]
fn test_issue_show_json() {
    let temp = init_binnacle();

    // Create an issue
    let output = bn_in(&temp)
        .args(["issue", "create", "Show test issue"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let issue_id = json["id"].as_str().unwrap();

    // Show it
    bn_in(&temp)
        .args(["issue", "show", issue_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"title\":\"Show test issue\""))
        .stdout(predicate::str::contains("\"type\":\"issue\""));
}

#[test]
fn test_issue_show_human() {
    let temp = init_binnacle();

    // Create an issue
    let output = bn_in(&temp)
        .args(["issue", "create", "Human-readable issue", "-p", "1"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let issue_id = json["id"].as_str().unwrap();

    // Show in human readable format
    bn_in(&temp)
        .args(["-H", "issue", "show", issue_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("Human-readable issue"))
        .stdout(predicate::str::contains("Status:"));
}

#[test]
fn test_issue_show_not_found() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["issue", "show", "bn-nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found").or(predicate::str::contains("Not found")));
}

// === Issue Update Tests ===

#[test]
fn test_issue_update_title() {
    let temp = init_binnacle();

    // Create an issue
    let output = bn_in(&temp)
        .args(["issue", "create", "Original title"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let issue_id = json["id"].as_str().unwrap();

    // Update title
    bn_in(&temp)
        .args(["issue", "update", issue_id, "--title", "Updated title"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"updated_fields\""));

    // Verify update
    bn_in(&temp)
        .args(["issue", "show", issue_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"title\":\"Updated title\""));
}

#[test]
fn test_issue_update_priority() {
    let temp = init_binnacle();

    // Create an issue
    let output = bn_in(&temp)
        .args(["issue", "create", "Priority update test", "-p", "3"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let issue_id = json["id"].as_str().unwrap();

    // Update priority
    bn_in(&temp)
        .args(["issue", "update", issue_id, "--priority", "1"])
        .assert()
        .success();

    // Verify update
    bn_in(&temp)
        .args(["issue", "show", issue_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"priority\":1"));
}

#[test]
fn test_issue_update_status() {
    let temp = init_binnacle();

    // Create an issue
    let output = bn_in(&temp)
        .args(["issue", "create", "Status update test"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let issue_id = json["id"].as_str().unwrap();

    // Update status to investigating
    bn_in(&temp)
        .args(["issue", "update", issue_id, "--status", "investigating"])
        .assert()
        .success();

    // Verify update
    bn_in(&temp)
        .args(["issue", "show", issue_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"investigating\""));
}

#[test]
fn test_issue_update_all_statuses() {
    let temp = init_binnacle();

    // Create an issue
    let output = bn_in(&temp)
        .args(["issue", "create", "All status test"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let issue_id = json["id"].as_str().unwrap();

    // Test all valid status transitions
    let statuses = [
        "triage",
        "investigating",
        "resolved",
        "closed",
        "wont_fix",
        "by_design",
        "no_repro",
    ];

    for status in &statuses {
        bn_in(&temp)
            .args(["issue", "update", issue_id, "--status", status])
            .assert()
            .success();

        bn_in(&temp)
            .args(["issue", "show", issue_id])
            .assert()
            .success()
            .stdout(predicate::str::contains(format!(
                "\"status\":\"{}\"",
                status
            )));
    }
}

#[test]
fn test_issue_update_assignee() {
    let temp = init_binnacle();

    // Create an issue
    let output = bn_in(&temp)
        .args(["issue", "create", "Assignee test"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let issue_id = json["id"].as_str().unwrap();

    // Update assignee
    bn_in(&temp)
        .args(["issue", "update", issue_id, "--assignee", "bob"])
        .assert()
        .success();

    // Verify update
    bn_in(&temp)
        .args(["issue", "show", issue_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"assignee\":\"bob\""));
}

// === Issue Close Tests ===

#[test]
fn test_issue_close() {
    let temp = init_binnacle();

    // Create an issue
    let output = bn_in(&temp)
        .args(["issue", "create", "Issue to close"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let issue_id = json["id"].as_str().unwrap();

    // Close it
    bn_in(&temp)
        .args([
            "issue",
            "close",
            issue_id,
            "--reason",
            "Investigation complete",
        ])
        .assert()
        .success();

    // Verify closure
    bn_in(&temp)
        .args(["issue", "show", issue_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"closed\""))
        .stdout(predicate::str::contains(
            "\"closed_reason\":\"Investigation complete\"",
        ));
}

#[test]
fn test_issue_close_human() {
    let temp = init_binnacle();

    // Create an issue
    let output = bn_in(&temp)
        .args(["issue", "create", "Human close test"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let issue_id = json["id"].as_str().unwrap();

    // Close with human output
    bn_in(&temp)
        .args(["-H", "issue", "close", issue_id, "--reason", "Done"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Closed issue"));
}

// === Issue Reopen Tests ===

#[test]
fn test_issue_reopen() {
    let temp = init_binnacle();

    // Create and close an issue
    let output = bn_in(&temp)
        .args(["issue", "create", "Issue to reopen"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let issue_id = json["id"].as_str().unwrap();

    bn_in(&temp)
        .args(["issue", "close", issue_id, "--reason", "Closed too early"])
        .assert()
        .success();

    // Reopen it
    bn_in(&temp)
        .args(["issue", "reopen", issue_id])
        .assert()
        .success();

    // Verify reopened
    bn_in(&temp)
        .args(["issue", "show", issue_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"open\""));
}

#[test]
fn test_issue_reopen_human() {
    let temp = init_binnacle();

    // Create and close an issue
    let output = bn_in(&temp)
        .args(["issue", "create", "Human reopen test"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let issue_id = json["id"].as_str().unwrap();

    bn_in(&temp)
        .args(["issue", "close", issue_id, "--reason", "Test"])
        .assert()
        .success();

    // Reopen with human output
    bn_in(&temp)
        .args(["-H", "issue", "reopen", issue_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("Reopened issue"));
}

// === Issue Delete Tests ===

#[test]
fn test_issue_delete() {
    let temp = init_binnacle();

    // Create an issue
    let output = bn_in(&temp)
        .args(["issue", "create", "Issue to delete"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let issue_id = json["id"].as_str().unwrap();

    // Delete it
    bn_in(&temp)
        .args(["issue", "delete", issue_id])
        .assert()
        .success();

    // Verify it's gone
    bn_in(&temp)
        .args(["issue", "show", issue_id])
        .assert()
        .failure();
}

#[test]
fn test_issue_delete_human() {
    let temp = init_binnacle();

    // Create an issue
    let output = bn_in(&temp)
        .args(["issue", "create", "Human delete test"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let issue_id = json["id"].as_str().unwrap();

    // Delete with human output
    bn_in(&temp)
        .args(["-H", "issue", "delete", issue_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("Deleted issue"));
}

// === Ready/Orient Exclusion Tests ===

#[test]
fn test_issues_excluded_from_ready() {
    let temp = init_binnacle();

    // Create an issue (should not appear in ready)
    bn_in(&temp)
        .args(["issue", "create", "Issue for investigation"])
        .assert()
        .success();

    // Create a task (should appear in ready)
    bn_in(&temp)
        .args(["task", "create", "Task to work on"])
        .assert()
        .success();

    // bn ready should only show the task
    bn_in(&temp)
        .args(["ready"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Task to work on"))
        .stdout(predicate::str::contains("\"count\":1"));
}

#[test]
fn test_issues_excluded_from_orient() {
    let temp = init_binnacle();

    // Create an issue
    bn_in(&temp)
        .args(["issue", "create", "Issue for investigation"])
        .assert()
        .success();

    // Create a task
    bn_in(&temp)
        .args(["task", "create", "Task to work on"])
        .assert()
        .success();

    // bn orient should show task count but not issue count in ready
    // Note: orient returns JSON format
    let output = bn_in(&temp)
        .args(["orient", "--type", "worker"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    // ready_count should be 1 (just the task)
    assert_eq!(json["ready_count"].as_i64().unwrap(), 1);
}

// === Auto-Resolution Tests ===

#[test]
fn test_issue_auto_resolve_when_all_child_bugs_closed() {
    let temp = init_binnacle();

    // Create an issue
    let output = bn_in(&temp)
        .args(["issue", "create", "Issue with child bugs"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let issue_id = json["id"].as_str().unwrap();

    // Create a child bug linked to the issue
    let output = bn_in(&temp)
        .args(["bug", "create", "Child bug", "--parent", issue_id])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let bug_id = json["id"].as_str().unwrap();

    // Issue should still be open
    bn_in(&temp)
        .args(["issue", "show", issue_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"open\""));

    // Close the child bug
    bn_in(&temp)
        .args(["bug", "close", bug_id, "--reason", "Fixed"])
        .assert()
        .success();

    // Issue should now be auto-resolved
    bn_in(&temp)
        .args(["issue", "show", issue_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"resolved\""));
}

#[test]
fn test_issue_not_auto_resolve_when_some_child_bugs_open() {
    let temp = init_binnacle();

    // Create an issue
    let output = bn_in(&temp)
        .args(["issue", "create", "Issue with multiple bugs"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let issue_id = json["id"].as_str().unwrap();

    // Create two child bugs
    let output = bn_in(&temp)
        .args(["bug", "create", "Child bug 1", "--parent", issue_id])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let bug1_id = json["id"].as_str().unwrap();

    bn_in(&temp)
        .args(["bug", "create", "Child bug 2", "--parent", issue_id])
        .assert()
        .success();

    // Close only one bug
    bn_in(&temp)
        .args(["bug", "close", bug1_id, "--reason", "Fixed"])
        .assert()
        .success();

    // Issue should still be open (not auto-resolved)
    bn_in(&temp)
        .args(["issue", "show", issue_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"open\""));
}

#[test]
fn test_issue_not_auto_resolve_when_already_closed() {
    let temp = init_binnacle();

    // Create an issue and close it manually
    let output = bn_in(&temp)
        .args(["issue", "create", "Manually closed issue"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let issue_id = json["id"].as_str().unwrap();

    bn_in(&temp)
        .args(["issue", "close", issue_id, "--reason", "Manual close"])
        .assert()
        .success();

    // Create and link a child bug
    let output = bn_in(&temp)
        .args(["bug", "create", "Child bug"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let bug_id = json["id"].as_str().unwrap();

    // Link bug to issue manually
    bn_in(&temp)
        .args(["link", "add", bug_id, issue_id, "--type", "child_of"])
        .assert()
        .success();

    // Close the bug
    bn_in(&temp)
        .args(["bug", "close", bug_id, "--reason", "Fixed"])
        .assert()
        .success();

    // Issue should still be closed (not changed to resolved)
    bn_in(&temp)
        .args(["issue", "show", issue_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"closed\""));
}

// === Generic Show Command Tests ===

#[test]
fn test_bn_show_works_for_issues() {
    let temp = init_binnacle();

    // Create an issue
    let output = bn_in(&temp)
        .args(["issue", "create", "Show command test"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let issue_id = json["id"].as_str().unwrap();

    // bn show should work for issues
    bn_in(&temp)
        .args(["show", issue_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"title\":\"Show command test\""));
}

// === Queue Integration Tests ===

#[test]
fn test_issue_create_with_queue_flag() {
    let temp = init_binnacle();

    // Create a queue first
    bn_in(&temp)
        .args(["queue", "create", "Work Queue"])
        .assert()
        .success();

    // Create an issue with -q flag
    let output = bn_in(&temp)
        .args(["issue", "create", "Queued issue", "-q"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let issue_id = json["id"].as_str().unwrap();

    // Verify the issue was added to the queue (check via link list)
    bn_in(&temp)
        .args(["link", "list", issue_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("queued"));
}
