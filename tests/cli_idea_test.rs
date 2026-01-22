//! Integration tests for Idea CRUD operations via CLI.
//!
//! These tests verify that idea commands work correctly through the CLI:
//! - `bn idea create/list/show/update/close/delete` all work
//! - JSON and human-readable output formats are correct
//! - Filtering by status and tags works

use assert_cmd::Command;
use predicates::prelude::*;
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
    bn_in(&temp).args(["system", "init"]).assert().success();
    temp
}

// === Idea Create Tests ===

#[test]
fn test_idea_create_json() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["idea", "create", "Use SQLite FTS for search"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"id\":\"bni-"))
        .stdout(predicate::str::contains("\"title\":\"Use SQLite FTS for search\""));
}

#[test]
fn test_idea_create_human() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["idea", "create", "Test idea", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created idea bni-"))
        .stdout(predicate::str::contains("\"Test idea\""));
}

#[test]
fn test_idea_create_with_tags() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["idea", "create", "Tagged idea", "--tag", "search", "--tag", "perf"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"id\":\"bni-"));
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
        .stdout(predicate::str::contains("\"id\":\"bni-"));
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
        .args(["idea", "create", "Human show", "--description", "Detailed desc"])
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
        .stdout(predicate::str::contains(&format!("\"id\":\"{}\"", id)));

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
        .stdout(predicate::str::contains(&format!("\"id\":\"{}\"", id)));

    // Verify it's gone
    bn_in(&temp)
        .args(["idea", "show", id])
        .assert()
        .failure();
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
