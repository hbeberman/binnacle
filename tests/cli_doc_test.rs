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
        .stdout(predicate::str::contains("\"id\":\"bn-"))
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
fn test_doc_show_json_returns_decompressed_content() {
    let (temp, task_id) = init_with_task();

    // Create doc with recognizable content
    let content = "# Summary\n\nThis is the summary.\n\n# Body\n\nThis is readable body text.";
    let output = bn_in(&temp)
        .args([
            "doc",
            "create",
            &task_id,
            "-T",
            "Doc for JSON test",
            "-c",
            content,
        ])
        .output()
        .expect("Failed to create doc");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("Invalid JSON");
    let doc_id = json["id"].as_str().expect("No doc ID");

    // Get JSON output (no -H flag)
    let show_output = bn_in(&temp)
        .args(["doc", "show", doc_id])
        .output()
        .expect("Failed to show doc");

    let show_stdout = String::from_utf8_lossy(&show_output.stdout);
    let show_json: serde_json::Value = serde_json::from_str(&show_stdout).expect("Invalid JSON");

    // The content field in JSON should be decompressed, readable text
    // Note: doc show returns { "doc": { ... }, "linked_entities": [...] }
    let returned_content = show_json["doc"]["content"]
        .as_str()
        .expect("No doc.content field");
    assert!(
        returned_content.contains("# Summary"),
        "Content should contain '# Summary', got: {}",
        returned_content
    );
    assert!(
        returned_content.contains("This is the summary"),
        "Content should contain 'This is the summary', got: {}",
        returned_content
    );
    assert!(
        returned_content.contains("This is readable body text"),
        "Content should contain 'This is readable body text', got: {}",
        returned_content
    );

    // The content should NOT look like base64 or compressed data
    // Base64 typically has lots of + / = characters and no spaces
    assert!(
        !returned_content.starts_with("KL"),
        "Content should not be compressed (starts with zstd magic), got: {}",
        returned_content
    );
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

// === Doc Update (Versioning) Tests ===

#[test]
fn test_doc_update_creates_new_version() {
    let (temp, task_id) = init_with_task();

    // Create initial doc
    let output = bn_in(&temp)
        .args([
            "doc",
            "create",
            &task_id,
            "-T",
            "Versioned doc",
            "-c",
            "Version 1 content",
        ])
        .output()
        .expect("Failed to create doc");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("Invalid JSON");
    let original_id = json["id"].as_str().expect("No doc ID").to_string();

    // Update the doc (creates new version)
    let output = bn_in(&temp)
        .args([
            "doc",
            "update",
            &original_id,
            "-c",
            "Version 2 content",
            "--editor",
            "user:tester",
        ])
        .output()
        .expect("Failed to update doc");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("Invalid JSON");

    // Should have a new ID
    let new_id = json["new_id"].as_str().expect("No new_id");
    assert_ne!(new_id, original_id);

    // Should reference the previous version
    assert_eq!(
        json["previous_id"].as_str().expect("No previous_id"),
        original_id
    );
}

#[test]
fn test_doc_update_transfers_edges() {
    let temp = TestEnv::init();

    // Create two tasks to link doc to
    let output = bn_in(&temp)
        .args(["task", "create", "Task A"])
        .output()
        .expect("Failed to create task A");
    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("Invalid JSON");
    let task_a = json["id"].as_str().expect("No task A ID").to_string();

    let output = bn_in(&temp)
        .args(["task", "create", "Task B"])
        .output()
        .expect("Failed to create task B");
    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("Invalid JSON");
    let task_b = json["id"].as_str().expect("No task B ID").to_string();

    // Create doc linked to both tasks
    let output = bn_in(&temp)
        .args([
            "doc",
            "create",
            &task_a,
            &task_b,
            "-T",
            "Multi-linked doc",
            "-c",
            "Initial content",
        ])
        .output()
        .expect("Failed to create doc");

    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("Invalid JSON");
    let original_id = json["id"].as_str().expect("No doc ID").to_string();

    // Update the doc
    let output = bn_in(&temp)
        .args(["doc", "update", &original_id, "-c", "Updated content"])
        .output()
        .expect("Failed to update doc");

    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("Invalid JSON");
    let new_id = json["new_id"].as_str().expect("No new_id").to_string();

    // Should have transferred edges (2 edges from the two tasks)
    let edges_transferred = json["edges_transferred"].as_u64().expect("No edges count");
    assert!(
        edges_transferred >= 2,
        "Expected at least 2 edges transferred, got {}",
        edges_transferred
    );

    // Verify new doc is linked to tasks
    bn_in(&temp)
        .args(["doc", "show", &new_id])
        .assert()
        .success()
        .stdout(predicate::str::contains(&task_a))
        .stdout(predicate::str::contains(&task_b));
}

#[test]
fn test_doc_update_with_title_change() {
    let (temp, task_id) = init_with_task();

    // Create doc
    let output = bn_in(&temp)
        .args([
            "doc",
            "create",
            &task_id,
            "-T",
            "Original Title",
            "-c",
            "Content",
        ])
        .output()
        .expect("Failed to create doc");

    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("Invalid JSON");
    let original_id = json["id"].as_str().expect("No doc ID").to_string();

    // Update with new title
    let output = bn_in(&temp)
        .args(["doc", "update", &original_id, "-T", "New Title"])
        .output()
        .expect("Failed to update doc");

    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("Invalid JSON");
    let new_id = json["new_id"].as_str().expect("No new_id");

    // Verify new doc has new title
    bn_in(&temp)
        .args(["doc", "show", new_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("New Title"));
}

#[test]
fn test_doc_update_with_editor_attribution() {
    let (temp, task_id) = init_with_task();

    // Create doc
    let output = bn_in(&temp)
        .args([
            "doc",
            "create",
            &task_id,
            "-T",
            "Editor test doc",
            "-c",
            "Content",
        ])
        .output()
        .expect("Failed to create doc");

    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("Invalid JSON");
    let original_id = json["id"].as_str().expect("No doc ID").to_string();

    // Update with agent editor
    let output = bn_in(&temp)
        .args([
            "doc",
            "update",
            &original_id,
            "-c",
            "Updated by agent",
            "--editor",
            "agent:bn-test",
        ])
        .output()
        .expect("Failed to update doc");

    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("Invalid JSON");
    let new_id = json["new_id"].as_str().expect("No new_id");

    // Verify editor is recorded
    bn_in(&temp)
        .args(["doc", "show", new_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("bn-test"));
}

#[test]
fn test_doc_update_human_readable() {
    let (temp, task_id) = init_with_task();

    // Create doc
    let output = bn_in(&temp)
        .args(["doc", "create", &task_id, "-T", "HR doc", "-c", "Content"])
        .output()
        .expect("Failed to create doc");

    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("Invalid JSON");
    let original_id = json["id"].as_str().expect("No doc ID").to_string();

    // Update with human-readable output
    bn_in(&temp)
        .args(["doc", "update", &original_id, "-c", "New content", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created new version"))
        .stdout(predicate::str::contains("supersedes"))
        .stdout(predicate::str::contains(&original_id));
}

// === Doc History Tests ===

#[test]
fn test_doc_history_single_version() {
    let (temp, task_id) = init_with_task();

    // Create doc
    let output = bn_in(&temp)
        .args([
            "doc",
            "create",
            &task_id,
            "-T",
            "History test doc",
            "-c",
            "Content",
        ])
        .output()
        .expect("Failed to create doc");

    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("Invalid JSON");
    let doc_id = json["id"].as_str().expect("No doc ID");

    // Get history
    let output = bn_in(&temp)
        .args(["doc", "history", doc_id])
        .output()
        .expect("Failed to get history");

    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("Invalid JSON");

    // Should have exactly 1 version
    let versions = json["versions"].as_array().expect("No versions array");
    assert_eq!(versions.len(), 1);
    assert!(versions[0]["is_current"].as_bool().unwrap());
}

#[test]
fn test_doc_history_multiple_versions() {
    let (temp, task_id) = init_with_task();

    // Create initial doc
    let output = bn_in(&temp)
        .args([
            "doc",
            "create",
            &task_id,
            "-T",
            "Multi-version doc",
            "-c",
            "Version 1",
        ])
        .output()
        .expect("Failed to create doc");

    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("Invalid JSON");
    let v1_id = json["id"].as_str().expect("No doc ID").to_string();

    // Update to version 2
    let output = bn_in(&temp)
        .args(["doc", "update", &v1_id, "-c", "Version 2"])
        .output()
        .expect("Failed to update doc");

    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("Invalid JSON");
    let v2_id = json["new_id"].as_str().expect("No new_id").to_string();

    // Update to version 3
    let output = bn_in(&temp)
        .args(["doc", "update", &v2_id, "-c", "Version 3"])
        .output()
        .expect("Failed to update doc");

    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("Invalid JSON");
    let v3_id = json["new_id"].as_str().expect("No new_id");

    // Get history from the latest version
    let output = bn_in(&temp)
        .args(["doc", "history", v3_id])
        .output()
        .expect("Failed to get history");

    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("Invalid JSON");

    // Should have 3 versions
    let versions = json["versions"].as_array().expect("No versions array");
    assert_eq!(versions.len(), 3);

    // First should be current
    assert!(versions[0]["is_current"].as_bool().unwrap());
    assert!(!versions[1]["is_current"].as_bool().unwrap());
    assert!(!versions[2]["is_current"].as_bool().unwrap());
}

#[test]
fn test_doc_history_human_readable() {
    let (temp, task_id) = init_with_task();

    // Create doc
    let output = bn_in(&temp)
        .args([
            "doc",
            "create",
            &task_id,
            "-T",
            "HR history doc",
            "-c",
            "Content",
        ])
        .output()
        .expect("Failed to create doc");

    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("Invalid JSON");
    let doc_id = json["id"].as_str().expect("No doc ID");

    // Update once
    let output = bn_in(&temp)
        .args([
            "doc",
            "update",
            doc_id,
            "-c",
            "V2",
            "--editor",
            "user:alice",
        ])
        .output()
        .expect("Failed to update doc");

    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("Invalid JSON");
    let new_id = json["new_id"].as_str().expect("No new_id");

    // Get history in human-readable format
    bn_in(&temp)
        .args(["doc", "history", new_id, "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("history"))
        .stdout(predicate::str::contains("2 versions"))
        .stdout(predicate::str::contains("(current)"))
        .stdout(predicate::str::contains("user:alice"));
}

#[test]
fn test_doc_update_invalid_editor_format() {
    let (temp, task_id) = init_with_task();

    // Create doc
    let output = bn_in(&temp)
        .args(["doc", "create", &task_id, "-T", "Test", "-c", "Content"])
        .output()
        .expect("Failed to create doc");

    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("Invalid JSON");
    let doc_id = json["id"].as_str().expect("No doc ID");

    // Try to update with invalid editor format
    bn_in(&temp)
        .args([
            "doc",
            "update",
            doc_id,
            "-c",
            "New content",
            "--editor",
            "invalid_format",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid editor format"));
}

#[test]
fn test_doc_update_invalid_editor_type() {
    let (temp, task_id) = init_with_task();

    // Create doc
    let output = bn_in(&temp)
        .args(["doc", "create", &task_id, "-T", "Test", "-c", "Content"])
        .output()
        .expect("Failed to create doc");

    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("Invalid JSON");
    let doc_id = json["id"].as_str().expect("No doc ID");

    // Try to update with invalid editor type
    bn_in(&temp)
        .args([
            "doc",
            "update",
            doc_id,
            "-c",
            "New content",
            "--editor",
            "robot:r2d2",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid editor type"));
}

// === Summary Dirty Detection Tests ===

#[test]
fn test_doc_update_sets_summary_dirty_when_content_changes_but_summary_doesnt() {
    let (temp, task_id) = init_with_task();

    // Create doc with summary section using stdin for proper newlines
    let output = bn_in(&temp)
        .args(["doc", "create", &task_id, "-T", "Dirty test doc", "--stdin"])
        .write_stdin("# Summary\n\nOriginal summary\n\n# Details\n\nOriginal details")
        .output()
        .expect("Failed to create doc");

    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("Invalid JSON");
    let original_id = json["id"].as_str().expect("No doc ID").to_string();

    // Verify initial doc is not dirty
    let output = bn_in(&temp)
        .args(["doc", "show", &original_id])
        .output()
        .expect("Failed to show doc");
    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("Invalid JSON");
    assert!(
        !json["doc"]["summary_dirty"].as_bool().unwrap_or(true),
        "Initial doc should not be dirty"
    );

    // Update content but NOT the summary section
    let output = bn_in(&temp)
        .args(["doc", "update", &original_id, "--stdin"])
        .write_stdin(
            "# Summary\n\nOriginal summary\n\n# Details\n\nUPDATED details - summary unchanged",
        )
        .output()
        .expect("Failed to update doc");

    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("Invalid JSON");
    let new_id = json["new_id"].as_str().expect("No new_id").to_string();

    // Verify new version has summary_dirty = true
    let output = bn_in(&temp)
        .args(["doc", "show", &new_id])
        .output()
        .expect("Failed to show updated doc");
    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("Invalid JSON");
    assert!(
        json["doc"]["summary_dirty"].as_bool().unwrap_or(false),
        "summary_dirty should be true when content changes but summary doesn't"
    );
}

#[test]
fn test_doc_update_clears_summary_dirty_when_summary_changes() {
    let (temp, task_id) = init_with_task();

    // Create doc with summary section using stdin for proper newlines
    let output = bn_in(&temp)
        .args(["doc", "create", &task_id, "-T", "Clean test doc", "--stdin"])
        .write_stdin("# Summary\n\nOriginal summary\n\n# Details\n\nOriginal details")
        .output()
        .expect("Failed to create doc");

    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("Invalid JSON");
    let original_id = json["id"].as_str().expect("No doc ID").to_string();

    // Update both content AND summary using stdin
    let output = bn_in(&temp)
        .args(["doc", "update", &original_id, "--stdin"])
        .write_stdin("# Summary\n\nUPDATED summary\n\n# Details\n\nUPDATED details")
        .output()
        .expect("Failed to update doc");

    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("Invalid JSON");
    let new_id = json["new_id"].as_str().expect("No new_id").to_string();

    // Verify new version has summary_dirty = false
    let output = bn_in(&temp)
        .args(["doc", "show", &new_id])
        .output()
        .expect("Failed to show updated doc");
    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("Invalid JSON");
    assert!(
        !json["doc"]["summary_dirty"].as_bool().unwrap_or(true),
        "summary_dirty should be false when summary is also updated"
    );
}

#[test]
fn test_doc_update_clear_dirty_flag_overrides_detection() {
    let (temp, task_id) = init_with_task();

    // Create doc with summary section using stdin for proper newlines
    let output = bn_in(&temp)
        .args([
            "doc",
            "create",
            &task_id,
            "-T",
            "Override test doc",
            "--stdin",
        ])
        .write_stdin("# Summary\n\nOriginal summary\n\n# Details\n\nOriginal details")
        .output()
        .expect("Failed to create doc");

    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("Invalid JSON");
    let original_id = json["id"].as_str().expect("No doc ID").to_string();

    // Update content without changing summary, but use --clear-dirty flag
    let output = bn_in(&temp)
        .args(["doc", "update", &original_id, "--stdin", "--clear-dirty"])
        .write_stdin(
            "# Summary\n\nOriginal summary\n\n# Details\n\nUPDATED details but flag clears dirty",
        )
        .output()
        .expect("Failed to update doc");

    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("Invalid JSON");
    let new_id = json["new_id"].as_str().expect("No new_id").to_string();

    // Verify new version has summary_dirty = false (clear-dirty takes precedence)
    let output = bn_in(&temp)
        .args(["doc", "show", &new_id])
        .output()
        .expect("Failed to show updated doc");
    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("Invalid JSON");
    assert!(
        !json["doc"]["summary_dirty"].as_bool().unwrap_or(true),
        "summary_dirty should be false when --clear-dirty flag is used"
    );
}
