//! Integration tests for Idea CRUD operations via CLI.
//!
//! These tests verify that idea commands work correctly through the CLI:
//! - `bn idea create/list/show/update/close/delete` all work
//! - JSON and human-readable output formats are correct
//! - Filtering by status and tags works

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

// === Idea Create Tests ===

#[test]
fn test_idea_create_json() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["idea", "create", "Use SQLite FTS for search"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"id\":\"bn-"))
        .stdout(predicate::str::contains(
            "\"title\":\"Use SQLite FTS for search\"",
        ));
}

#[test]
fn test_idea_create_human() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["idea", "create", "Test idea", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created idea bn-"))
        .stdout(predicate::str::contains("\"Test idea\""));
}

#[test]
fn test_idea_create_with_tags() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args([
            "idea",
            "create",
            "Tagged idea",
            "--tag",
            "search",
            "--tag",
            "perf",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"id\":\"bn-"));
}

#[test]
fn test_idea_create_with_description() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args([
            "idea",
            "create",
            "Idea with desc",
            "--description",
            "This is a detailed description",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"id\":\"bn-"));
}

// === Idea List Tests ===

#[test]
fn test_idea_list_empty() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["idea", "list", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No ideas found."));
}

#[test]
fn test_idea_list_with_ideas() {
    let temp = init_binnacle();

    // Create an idea
    bn_in(&temp)
        .args(["idea", "create", "First idea"])
        .assert()
        .success();

    // List ideas
    bn_in(&temp)
        .args(["idea", "list", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 idea(s):"))
        .stdout(predicate::str::contains("[seed]"))
        .stdout(predicate::str::contains("First idea"));
}

#[test]
fn test_idea_list_filter_by_status() {
    let temp = init_binnacle();

    // Create two ideas
    let output = bn_in(&temp)
        .args(["idea", "create", "Seed idea"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let seed_id = stdout
        .split("\"id\":\"")
        .nth(1)
        .unwrap()
        .split("\"")
        .next()
        .unwrap();

    bn_in(&temp)
        .args(["idea", "create", "Another idea"])
        .assert()
        .success();

    // Update first to germinating
    bn_in(&temp)
        .args(["idea", "update", seed_id, "--status", "germinating"])
        .assert()
        .success();

    // Filter by germinating status
    bn_in(&temp)
        .args(["idea", "list", "--status", "germinating", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 idea(s):"))
        .stdout(predicate::str::contains("Seed idea"));

    // Filter by seed status
    bn_in(&temp)
        .args(["idea", "list", "--status", "seed", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 idea(s):"))
        .stdout(predicate::str::contains("Another idea"));
}

#[test]
fn test_idea_list_filter_by_tag() {
    let temp = init_binnacle();

    // Create ideas with different tags
    bn_in(&temp)
        .args(["idea", "create", "Search idea", "--tag", "search"])
        .assert()
        .success();

    bn_in(&temp)
        .args(["idea", "create", "GUI idea", "--tag", "gui"])
        .assert()
        .success();

    // Filter by tag
    bn_in(&temp)
        .args(["idea", "list", "--tag", "search", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 idea(s):"))
        .stdout(predicate::str::contains("Search idea"));
}

// === Idea Show Tests ===

#[test]
fn test_idea_show_json() {
    let temp = init_binnacle();

    // Create an idea
    let output = bn_in(&temp)
        .args(["idea", "create", "Show me"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id = stdout
        .split("\"id\":\"")
        .nth(1)
        .unwrap()
        .split("\"")
        .next()
        .unwrap();

    // Show the idea
    bn_in(&temp)
        .args(["idea", "show", id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"id\":\""))
        .stdout(predicate::str::contains("\"title\":\"Show me\""))
        .stdout(predicate::str::contains("\"status\":\"seed\""));
}

#[test]
fn test_idea_show_human() {
    let temp = init_binnacle();

    // Create an idea
    let output = bn_in(&temp)
        .args([
            "idea",
            "create",
            "Human show",
            "--description",
            "Detailed desc",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id = stdout
        .split("\"id\":\"")
        .nth(1)
        .unwrap()
        .split("\"")
        .next()
        .unwrap();

    // Show the idea
    bn_in(&temp)
        .args(["idea", "show", id, "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("[seed]"))
        .stdout(predicate::str::contains("Human show"))
        .stdout(predicate::str::contains("Description: Detailed desc"));
}

#[test]
fn test_idea_show_not_found() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["idea", "show", "bni-9999"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

// === Idea Update Tests ===

#[test]
fn test_idea_update_title() {
    let temp = init_binnacle();

    // Create an idea
    let output = bn_in(&temp)
        .args(["idea", "create", "Original title"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id = stdout
        .split("\"id\":\"")
        .nth(1)
        .unwrap()
        .split("\"")
        .next()
        .unwrap();

    // Update title
    bn_in(&temp)
        .args(["idea", "update", id, "--title", "New title"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"updated_fields\":[\"title\"]"));

    // Verify
    bn_in(&temp)
        .args(["idea", "show", id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"title\":\"New title\""));
}

#[test]
fn test_idea_update_status() {
    let temp = init_binnacle();

    // Create an idea
    let output = bn_in(&temp)
        .args(["idea", "create", "Status test"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id = stdout
        .split("\"id\":\"")
        .nth(1)
        .unwrap()
        .split("\"")
        .next()
        .unwrap();

    // Update status to germinating
    bn_in(&temp)
        .args(["idea", "update", id, "--status", "germinating"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"updated_fields\":[\"status\"]"));

    // Verify
    bn_in(&temp)
        .args(["idea", "show", id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"germinating\""));
}

#[test]
fn test_idea_update_tags() {
    let temp = init_binnacle();

    // Create an idea with a tag
    let output = bn_in(&temp)
        .args(["idea", "create", "Tag test", "--tag", "original"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id = stdout
        .split("\"id\":\"")
        .nth(1)
        .unwrap()
        .split("\"")
        .next()
        .unwrap();

    // Add a tag
    bn_in(&temp)
        .args(["idea", "update", id, "--add-tag", "new"])
        .assert()
        .success();

    // Remove original tag
    bn_in(&temp)
        .args(["idea", "update", id, "--remove-tag", "original"])
        .assert()
        .success();

    // Verify
    bn_in(&temp)
        .args(["idea", "show", id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"tags\":[\"new\"]"));
}

// === Idea Close Tests ===

#[test]
fn test_idea_close() {
    let temp = init_binnacle();

    // Create an idea
    let output = bn_in(&temp)
        .args(["idea", "create", "To close"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id = stdout
        .split("\"id\":\"")
        .nth(1)
        .unwrap()
        .split("\"")
        .next()
        .unwrap();

    // Close (discard) the idea
    bn_in(&temp)
        .args(["idea", "close", id, "--reason", "Not useful"])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!("\"id\":\"{}\"", id)));

    // Verify it's discarded
    bn_in(&temp)
        .args(["idea", "show", id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"discarded\""));
}

#[test]
fn test_idea_close_human() {
    let temp = init_binnacle();

    // Create an idea
    let output = bn_in(&temp)
        .args(["idea", "create", "Close me"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id = stdout
        .split("\"id\":\"")
        .nth(1)
        .unwrap()
        .split("\"")
        .next()
        .unwrap();

    // Close with human output
    bn_in(&temp)
        .args(["idea", "close", id, "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Discarded idea"));
}

// === Idea Delete Tests ===

#[test]
fn test_idea_delete() {
    let temp = init_binnacle();

    // Create an idea
    let output = bn_in(&temp)
        .args(["idea", "create", "To delete"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id = stdout
        .split("\"id\":\"")
        .nth(1)
        .unwrap()
        .split("\"")
        .next()
        .unwrap();

    // Delete it
    bn_in(&temp)
        .args(["idea", "delete", id])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!("\"id\":\"{}\"", id)));

    // Verify it's gone
    bn_in(&temp).args(["idea", "show", id]).assert().failure();
}

#[test]
fn test_idea_delete_not_found() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["idea", "delete", "bni-9999"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

// === Generic Show with Idea ===

#[test]
fn test_generic_show_idea() {
    let temp = init_binnacle();

    // Create an idea
    let output = bn_in(&temp)
        .args(["idea", "create", "Generic show test"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id = stdout
        .split("\"id\":\"")
        .nth(1)
        .unwrap()
        .split("\"")
        .next()
        .unwrap();

    // Use generic show command
    bn_in(&temp)
        .args(["show", id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"type\":\"idea\""))
        .stdout(predicate::str::contains("Generic show test"));
}

// === Idea Germinate Tests ===

#[test]
fn test_idea_germinate_json() {
    let temp = init_binnacle();

    // Create an idea
    let output = bn_in(&temp)
        .args(["idea", "create", "To germinate"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id = stdout
        .split("\"id\":\"")
        .nth(1)
        .unwrap()
        .split("\"")
        .next()
        .unwrap();

    // Germinate it
    bn_in(&temp)
        .args(["idea", "germinate", id])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!("\"id\":\"{}\"", id)));

    // Verify status changed
    bn_in(&temp)
        .args(["idea", "show", id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"germinating\""));
}

#[test]
fn test_idea_germinate_human() {
    let temp = init_binnacle();

    // Create an idea
    let output = bn_in(&temp)
        .args(["idea", "create", "Germinate test"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id = stdout
        .split("\"id\":\"")
        .nth(1)
        .unwrap()
        .split("\"")
        .next()
        .unwrap();

    // Germinate with human output
    bn_in(&temp)
        .args(["idea", "germinate", id, "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("is now germinating"));
}

#[test]
fn test_idea_germinate_already_promoted_fails() {
    let temp = init_binnacle();

    // Create an idea
    let output = bn_in(&temp)
        .args(["idea", "create", "Already promoted"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id = stdout
        .split("\"id\":\"")
        .nth(1)
        .unwrap()
        .split("\"")
        .next()
        .unwrap();

    // Promote first
    bn_in(&temp)
        .args(["idea", "promote", id])
        .assert()
        .success();

    // Try to germinate - should fail
    bn_in(&temp)
        .args(["idea", "germinate", id])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already promoted"));
}

#[test]
fn test_idea_germinate_discarded_fails() {
    let temp = init_binnacle();

    // Create an idea
    let output = bn_in(&temp)
        .args(["idea", "create", "Discarded idea"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id = stdout
        .split("\"id\":\"")
        .nth(1)
        .unwrap()
        .split("\"")
        .next()
        .unwrap();

    // Discard first
    bn_in(&temp).args(["idea", "close", id]).assert().success();

    // Try to germinate - should fail
    bn_in(&temp)
        .args(["idea", "germinate", id])
        .assert()
        .failure()
        .stderr(predicate::str::contains("discarded"));
}

// === Idea Promote Tests ===

#[test]
fn test_idea_promote_to_task_json() {
    let temp = init_binnacle();

    // Create an idea
    let output = bn_in(&temp)
        .args([
            "idea",
            "create",
            "Promote to task",
            "--tag",
            "feature",
            "--description",
            "A detailed description",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let idea_id = stdout
        .split("\"id\":\"")
        .nth(1)
        .unwrap()
        .split("\"")
        .next()
        .unwrap();

    // Promote to task
    let output = bn_in(&temp)
        .args(["idea", "promote", idea_id])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"promoted_to\":\"bn-"));
    assert!(!stdout.contains("\"prd_path\""));

    // Get the task ID from the output
    let task_id = stdout
        .split("\"promoted_to\":\"")
        .nth(1)
        .unwrap()
        .split("\"")
        .next()
        .unwrap();

    // Verify idea is marked as promoted
    bn_in(&temp)
        .args(["idea", "show", idea_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"promoted\""))
        .stdout(predicate::str::contains(format!(
            "\"promoted_to\":\"{}\"",
            task_id
        )));

    // Verify task was created with correct fields
    bn_in(&temp)
        .args(["task", "show", task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"title\":\"Promote to task\""))
        .stdout(predicate::str::contains(
            "\"description\":\"A detailed description\"",
        ))
        .stdout(predicate::str::contains("\"feature\""));
}

#[test]
fn test_idea_promote_to_task_human() {
    let temp = init_binnacle();

    // Create an idea
    let output = bn_in(&temp)
        .args(["idea", "create", "Human promote test"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id = stdout
        .split("\"id\":\"")
        .nth(1)
        .unwrap()
        .split("\"")
        .next()
        .unwrap();

    // Promote with human output
    bn_in(&temp)
        .args(["idea", "promote", id, "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Promoted idea"))
        .stdout(predicate::str::contains("to task: bn-"))
        .stdout(predicate::str::contains("marked as promoted"));
}

#[test]
fn test_idea_promote_with_priority() {
    let temp = init_binnacle();

    // Create an idea
    let output = bn_in(&temp)
        .args(["idea", "create", "Priority test"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let idea_id = stdout
        .split("\"id\":\"")
        .nth(1)
        .unwrap()
        .split("\"")
        .next()
        .unwrap();

    // Promote with priority 1
    let output = bn_in(&temp)
        .args(["idea", "promote", idea_id, "--priority", "1"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let task_id = stdout
        .split("\"promoted_to\":\"")
        .nth(1)
        .unwrap()
        .split("\"")
        .next()
        .unwrap();

    // Verify task has priority 1
    bn_in(&temp)
        .args(["task", "show", task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"priority\":1"));
}

#[test]
fn test_idea_promote_to_prd() {
    let temp = init_binnacle();

    // Create an idea
    let output = bn_in(&temp)
        .args([
            "idea",
            "create",
            "Create PRD Feature",
            "--description",
            "This is the origin story",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let idea_id = stdout
        .split("\"id\":\"")
        .nth(1)
        .unwrap()
        .split("\"")
        .next()
        .unwrap();

    // Promote as PRD
    let output = bn_in(&temp)
        .args(["idea", "promote", idea_id, "--as-prd"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"prd_path\":"));
    assert!(stdout.contains("PRD_CREATE_PRD_FEATURE.md"));

    // Verify the PRD file was created
    let prd_path = temp.path().join("prds/PRD_CREATE_PRD_FEATURE.md");
    assert!(prd_path.exists(), "PRD file should exist");

    // Verify PRD content
    let prd_content = std::fs::read_to_string(&prd_path).unwrap();
    assert!(prd_content.contains("# PRD: Create PRD Feature"));
    assert!(prd_content.contains(&format!("Promoted from idea {}", idea_id)));
    assert!(prd_content.contains("This is the origin story"));
    assert!(prd_content.contains("## Problem Statement"));
    assert!(prd_content.contains("## Proposed Solution"));
    assert!(prd_content.contains("## Implementation Plan"));

    // Verify idea is marked as promoted
    bn_in(&temp)
        .args(["idea", "show", idea_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"promoted\""));
}

#[test]
fn test_idea_promote_to_prd_human() {
    let temp = init_binnacle();

    // Create an idea
    let output = bn_in(&temp)
        .args(["idea", "create", "PRD Human Test"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id = stdout
        .split("\"id\":\"")
        .nth(1)
        .unwrap()
        .split("\"")
        .next()
        .unwrap();

    // Promote as PRD with human output
    bn_in(&temp)
        .args(["idea", "promote", id, "--as-prd", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Promoted idea"))
        .stdout(predicate::str::contains("to PRD:"))
        .stdout(predicate::str::contains("PRD_PRD_HUMAN_TEST.md"));
}

#[test]
fn test_idea_promote_already_promoted_fails() {
    let temp = init_binnacle();

    // Create an idea
    let output = bn_in(&temp)
        .args(["idea", "create", "Double promote"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id = stdout
        .split("\"id\":\"")
        .nth(1)
        .unwrap()
        .split("\"")
        .next()
        .unwrap();

    // Promote first time
    bn_in(&temp)
        .args(["idea", "promote", id])
        .assert()
        .success();

    // Try to promote again - should fail
    bn_in(&temp)
        .args(["idea", "promote", id])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already promoted"));
}

#[test]
fn test_idea_promote_discarded_fails() {
    let temp = init_binnacle();

    // Create an idea
    let output = bn_in(&temp)
        .args(["idea", "create", "Discarded promote"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id = stdout
        .split("\"id\":\"")
        .nth(1)
        .unwrap()
        .split("\"")
        .next()
        .unwrap();

    // Discard first
    bn_in(&temp).args(["idea", "close", id]).assert().success();

    // Try to promote - should fail
    bn_in(&temp)
        .args(["idea", "promote", id])
        .assert()
        .failure()
        .stderr(predicate::str::contains("discarded"));
}

#[test]
fn test_idea_promote_creates_prds_directory() {
    let temp = init_binnacle();

    // Remove prds directory if it exists
    let prds_dir = temp.path().join("prds");
    if prds_dir.exists() {
        std::fs::remove_dir_all(&prds_dir).unwrap();
    }

    // Create an idea
    let output = bn_in(&temp)
        .args(["idea", "create", "New PRD"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id = stdout
        .split("\"id\":\"")
        .nth(1)
        .unwrap()
        .split("\"")
        .next()
        .unwrap();

    // Promote as PRD - should create prds directory
    bn_in(&temp)
        .args(["idea", "promote", id, "--as-prd"])
        .assert()
        .success();

    assert!(prds_dir.exists(), "prds directory should be created");
}

#[test]
fn test_idea_promote_prd_already_exists_fails() {
    let temp = init_binnacle();

    // Create an idea
    let output = bn_in(&temp)
        .args(["idea", "create", "Existing PRD"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id = stdout
        .split("\"id\":\"")
        .nth(1)
        .unwrap()
        .split("\"")
        .next()
        .unwrap();

    // Create prds directory and a conflicting PRD file
    let prds_dir = temp.path().join("prds");
    std::fs::create_dir_all(&prds_dir).unwrap();
    std::fs::write(prds_dir.join("PRD_EXISTING_PRD.md"), "existing content").unwrap();

    // Try to promote as PRD - should fail due to existing file
    bn_in(&temp)
        .args(["idea", "promote", id, "--as-prd"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}
