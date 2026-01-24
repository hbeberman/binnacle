//! Integration tests for Doc CRUD operations via CLI.
//!
//! These tests verify that doc commands work correctly through the CLI:
//! - `bn doc create` creates a doc linked to entities
//! - Content can be provided via -c, --file, or --stdin
//! - Doc types (prd, note, handoff) work correctly
//! - Summary section is prepended when --short is used

mod common;

use assert_cmd::Command;
use common::TestEnv;
use predicates::prelude::*;
use std::io::Write;

/// Get a Command for the bn binary in a TestEnv.
fn bn_in(env: &TestEnv) -> Command {
    env.bn()
}

/// Initialize binnacle and create a task to link docs to.
fn init_with_task() -> (TestEnv, String) {
    let temp = TestEnv::init();

    let output = bn_in(&temp)
        .args(["task", "create", "Test task"])
        .output()
        .expect("Failed to create task");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("Invalid JSON");
    let task_id = json["id"].as_str().expect("No task ID").to_string();

    (temp, task_id)
}

// === Doc Create Tests ===

#[test]
fn test_doc_create_requires_entity_id() {
    let temp = TestEnv::init();

    bn_in(&temp)
        .args(["doc", "create", "-T", "Orphan doc"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("required arguments"));
}

#[test]
fn test_doc_create_validates_entity_exists() {
    let temp = TestEnv::init();

    bn_in(&temp)
        .args(["doc", "create", "bn-nonexistent", "-T", "Bad doc"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not exist"));
}

#[test]
fn test_doc_create_with_inline_content() {
    let (temp, task_id) = init_with_task();

    bn_in(&temp)
        .args([
            "doc",
            "create",
            &task_id,
            "-T",
            "My doc",
            "--type",
            "prd",
            "-c",
            "# Hello\n\nWorld",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"id\":\"bnd-"))
        .stdout(predicate::str::contains("\"doc_type\":\"prd\""))
        .stdout(predicate::str::contains(&task_id));
}

#[test]
fn test_doc_create_with_summary() {
    let (temp, task_id) = init_with_task();

    // Create doc with summary
    let output = bn_in(&temp)
        .args([
            "doc",
            "create",
            &task_id,
            "-T",
            "Doc with summary",
            "--short",
            "This is the summary",
            "-c",
            "# Main content",
        ])
        .output()
        .expect("Failed to create doc");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("Invalid JSON");
    let doc_id = json["id"].as_str().expect("No doc ID");

    // Verify summary was prepended (need --full to see full content including # Summary header)
    bn_in(&temp)
        .args(["doc", "show", doc_id, "-H", "--full"])
        .assert()
        .success()
        .stdout(predicate::str::contains("# Summary"))
        .stdout(predicate::str::contains("This is the summary"))
        .stdout(predicate::str::contains("# Main content"));
}

#[test]
fn test_doc_show_default_shows_summary_only() {
    let (temp, task_id) = init_with_task();

    // Create doc with summary and content
    let output = bn_in(&temp)
        .args([
            "doc",
            "create",
            &task_id,
            "-T",
            "Doc for show test",
            "--short",
            "The summary text",
            "-c",
            "# Main content\n\nThis is the main body.",
        ])
        .output()
        .expect("Failed to create doc");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("Invalid JSON");
    let doc_id = json["id"].as_str().expect("No doc ID");

    // Default show (without --full) should show summary but NOT full content
    bn_in(&temp)
        .args(["doc", "show", doc_id, "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Summary:"))
        .stdout(predicate::str::contains("The summary text"))
        .stdout(predicate::str::contains(
            "(Use --full to see complete content)",
        ))
        // Should NOT contain full markdown headers
        .stdout(predicate::str::contains("# Main content").not());
}

#[test]
fn test_doc_show_full_shows_all_content() {
    let (temp, task_id) = init_with_task();

    // Create doc with summary and content
    let output = bn_in(&temp)
        .args([
            "doc",
            "create",
            &task_id,
            "-T",
            "Doc for full show test",
            "--short",
            "Summary here",
            "-c",
            "# Body\n\nFull body text here.",
        ])
        .output()
        .expect("Failed to create doc");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("Invalid JSON");
    let doc_id = json["id"].as_str().expect("No doc ID");

    // With --full flag, should show all content
    bn_in(&temp)
        .args(["doc", "show", doc_id, "-H", "--full"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Content:"))
        .stdout(predicate::str::contains("# Summary"))
        .stdout(predicate::str::contains("Summary here"))
        .stdout(predicate::str::contains("# Body"))
        .stdout(predicate::str::contains("Full body text here."))
        // Should NOT contain the hint since we're showing full content
        .stdout(predicate::str::contains("(Use --full").not());
}

#[test]
fn test_doc_show_displays_linked_entities() {
    let (temp, task_id) = init_with_task();

    // Create doc linked to task
    let output = bn_in(&temp)
        .args([
            "doc",
            "create",
            &task_id,
            "-T",
            "Linked doc",
            "-c",
            "content",
        ])
        .output()
        .expect("Failed to create doc");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("Invalid JSON");
    let doc_id = json["id"].as_str().expect("No doc ID");

    // Show should display linked entities
    bn_in(&temp)
        .args(["doc", "show", doc_id, "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Linked Entities"))
        .stdout(predicate::str::contains(&task_id))
        .stdout(predicate::str::contains("Test task"));
}

#[test]
fn test_doc_create_links_to_entity() {
    let (temp, task_id) = init_with_task();

    // Create doc
    let output = bn_in(&temp)
        .args([
            "doc",
            "create",
            &task_id,
            "-T",
            "Linked doc",
            "-c",
            "content",
        ])
        .output()
        .expect("Failed to create doc");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("Invalid JSON");
    let _doc_id = json["id"].as_str().expect("No doc ID");

    // Verify link was created
    bn_in(&temp)
        .args(["link", "list", &task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("documents"));
}

#[test]
fn test_doc_create_with_multiple_entities() {
    let temp = TestEnv::init();

    // Create two tasks
    let output1 = bn_in(&temp)
        .args(["task", "create", "Task 1"])
        .output()
        .expect("Failed to create task");
    let json1: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output1.stdout)).expect("Invalid JSON");
    let task1_id = json1["id"].as_str().expect("No task ID").to_string();

    let output2 = bn_in(&temp)
        .args(["task", "create", "Task 2"])
        .output()
        .expect("Failed to create task");
    let json2: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output2.stdout)).expect("Invalid JSON");
    let task2_id = json2["id"].as_str().expect("No task ID").to_string();

    // Create doc linked to both
    bn_in(&temp)
        .args([
            "doc",
            "create",
            &task1_id,
            &task2_id,
            "-T",
            "Multi-linked doc",
            "-c",
            "content",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(&task1_id))
        .stdout(predicate::str::contains(&task2_id));
}

#[test]
fn test_doc_create_with_file() {
    let (temp, task_id) = init_with_task();

    // Create a temp file with content
    let file_path = temp.path().join("test_doc.md");
    let mut file = std::fs::File::create(&file_path).expect("Failed to create temp file");
    file.write_all(b"# From File\n\nThis content came from a file.")
        .expect("Failed to write file");

    // Create doc from file
    let output = bn_in(&temp)
        .args([
            "doc",
            "create",
            &task_id,
            "-T",
            "File doc",
            "--file",
            file_path.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to create doc");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("Invalid JSON");
    let doc_id = json["id"].as_str().expect("No doc ID");

    // Verify content (need --full to see full content)
    bn_in(&temp)
        .args(["doc", "show", doc_id, "-H", "--full"])
        .assert()
        .success()
        .stdout(predicate::str::contains("# From File"))
        .stdout(predicate::str::contains("from a file"));
}

#[test]
fn test_doc_create_with_stdin() {
    let (temp, task_id) = init_with_task();

    bn_in(&temp)
        .args([
            "doc",
            "create",
            &task_id,
            "-T",
            "Stdin doc",
            "--type",
            "handoff",
            "--stdin",
        ])
        .write_stdin("# From Stdin\n\nThis came from stdin.")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"doc_type\":\"handoff\""));
}

#[test]
fn test_doc_create_invalid_type() {
    let (temp, task_id) = init_with_task();

    bn_in(&temp)
        .args([
            "doc",
            "create",
            &task_id,
            "-T",
            "Bad type doc",
            "--type",
            "invalid",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid doc type"));
}

#[test]
fn test_doc_create_all_types() {
    let (temp, task_id) = init_with_task();

    for doc_type in &["prd", "note", "handoff"] {
        bn_in(&temp)
            .args([
                "doc",
                "create",
                &task_id,
                "-T",
                &format!("{} doc", doc_type),
                "--type",
                doc_type,
                "-c",
                "content",
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains(format!(
                "\"doc_type\":\"{}\"",
                doc_type
            )));
    }
}

#[test]
fn test_doc_create_human_readable() {
    let (temp, task_id) = init_with_task();

    bn_in(&temp)
        .args([
            "-H",
            "doc",
            "create",
            &task_id,
            "-T",
            "Human readable doc",
            "--type",
            "prd",
            "-c",
            "content",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created prd doc"))
        .stdout(predicate::str::contains("Human readable doc"))
        .stdout(predicate::str::contains(&task_id));
}

#[test]
fn test_doc_create_with_short_name_and_tags() {
    let (temp, task_id) = init_with_task();

    let output = bn_in(&temp)
        .args([
            "doc",
            "create",
            &task_id,
            "-T",
            "Tagged doc",
            "-s",
            "shortie",
            "-t",
            "tag1",
            "-t",
            "tag2",
            "-c",
            "content",
        ])
        .output()
        .expect("Failed to create doc");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("Invalid JSON");
    let doc_id = json["id"].as_str().expect("No doc ID");

    // Show doc and verify short name and tags
    bn_in(&temp)
        .args(["doc", "show", doc_id, "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Short Name: shortie"))
        .stdout(predicate::str::contains("tag1"))
        .stdout(predicate::str::contains("tag2"));
}

#[test]
fn test_doc_create_only_summary() {
    let (temp, task_id) = init_with_task();

    // Create doc with only summary, no content
    let output = bn_in(&temp)
        .args([
            "doc",
            "create",
            &task_id,
            "-T",
            "Summary only doc",
            "--short",
            "Just a summary",
        ])
        .output()
        .expect("Failed to create doc");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("Invalid JSON");
    let doc_id = json["id"].as_str().expect("No doc ID");

    // Verify summary is shown (need --full to see the # Summary header)
    bn_in(&temp)
        .args(["doc", "show", doc_id, "-H", "--full"])
        .assert()
        .success()
        .stdout(predicate::str::contains("# Summary"))
        .stdout(predicate::str::contains("Just a summary"));
}
