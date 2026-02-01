//! Integration tests for Milestone Auto-Close functionality.
//!
//! These tests verify that milestone auto-close behavior works correctly through the CLI:
//! - Auto-close when last child task completes
//! - Cascade auto-close to grandparent milestones
//! - Auto-reopen when new child added to auto-closed milestone
//! - Manual close protection (no auto-reopen)
//! - Doctor checks for empty and completable milestones
//! - --no-cascade flag to prevent auto-close

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

/// Helper to extract an ID from JSON output
fn extract_id(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("Failed to parse JSON");
    json["id"].as_str().expect("Missing id field").to_string()
}

// === Auto-Close on Last Child Tests ===

#[test]
fn test_milestone_auto_close_on_last_child_task() {
    let temp = init_binnacle();

    // Create a milestone
    let output = bn_in(&temp)
        .args(["milestone", "create", "Test Milestone"])
        .output()
        .unwrap();
    let milestone_id = extract_id(&output);

    // Create two child tasks
    let output = bn_in(&temp)
        .args(["task", "create", "Task 1"])
        .output()
        .unwrap();
    let task1_id = extract_id(&output);

    let output = bn_in(&temp)
        .args(["task", "create", "Task 2"])
        .output()
        .unwrap();
    let task2_id = extract_id(&output);

    // Link tasks to milestone as children
    bn_in(&temp)
        .args([
            "link",
            "add",
            &task1_id,
            &milestone_id,
            "--type",
            "child_of",
        ])
        .assert()
        .success();

    bn_in(&temp)
        .args([
            "link",
            "add",
            &task2_id,
            &milestone_id,
            "--type",
            "child_of",
        ])
        .assert()
        .success();

    // Close first task - milestone should still be pending
    bn_in(&temp)
        .args(["task", "close", &task1_id, "--reason", "Done"])
        .assert()
        .success();

    // Verify milestone is still pending
    bn_in(&temp)
        .args(["milestone", "show", &milestone_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"pending\""));

    // Close second task - milestone should now auto-close
    let output = bn_in(&temp)
        .args(["task", "close", &task2_id, "--reason", "Done"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify the close response mentions auto-completed milestone
    assert!(
        stdout.contains("auto_completed_milestones") && stdout.contains(&milestone_id),
        "Expected auto_completed_milestones to contain milestone_id. Got: {}",
        stdout
    );

    // Verify milestone is now done
    bn_in(&temp)
        .args(["milestone", "show", &milestone_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"done\""));
}

#[test]
fn test_milestone_auto_close_on_single_child() {
    let temp = init_binnacle();

    // Create a milestone with a single task
    let output = bn_in(&temp)
        .args(["milestone", "create", "Single Child Milestone"])
        .output()
        .unwrap();
    let milestone_id = extract_id(&output);

    let output = bn_in(&temp)
        .args(["task", "create", "Only Task"])
        .output()
        .unwrap();
    let task_id = extract_id(&output);

    bn_in(&temp)
        .args(["link", "add", &task_id, &milestone_id, "--type", "child_of"])
        .assert()
        .success();

    // Close the only task
    let output = bn_in(&temp)
        .args(["task", "close", &task_id, "--reason", "Done"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify the milestone was auto-closed
    assert!(
        stdout.contains(&milestone_id),
        "Expected milestone to be auto-closed"
    );

    // Verify milestone has correct closed_reason (singular form)
    bn_in(&temp)
        .args(["milestone", "show", &milestone_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("Only child completed"));
}

#[test]
fn test_milestone_auto_close_on_cancelled_children() {
    let temp = init_binnacle();

    // Create milestone with two tasks
    let output = bn_in(&temp)
        .args(["milestone", "create", "Cancelled Children Milestone"])
        .output()
        .unwrap();
    let milestone_id = extract_id(&output);

    let output = bn_in(&temp)
        .args(["task", "create", "Task to cancel 1"])
        .output()
        .unwrap();
    let task1_id = extract_id(&output);

    let output = bn_in(&temp)
        .args(["task", "create", "Task to cancel 2"])
        .output()
        .unwrap();
    let task2_id = extract_id(&output);

    bn_in(&temp)
        .args([
            "link",
            "add",
            &task1_id,
            &milestone_id,
            "--type",
            "child_of",
        ])
        .assert()
        .success();

    bn_in(&temp)
        .args([
            "link",
            "add",
            &task2_id,
            &milestone_id,
            "--type",
            "child_of",
        ])
        .assert()
        .success();

    // Cancel first task
    bn_in(&temp)
        .args(["task", "update", &task1_id, "--status", "cancelled"])
        .assert()
        .success();

    // Verify milestone still pending
    bn_in(&temp)
        .args(["milestone", "show", &milestone_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"pending\""));

    // Cancel second task - milestone should auto-close
    bn_in(&temp)
        .args(["task", "update", &task2_id, "--status", "cancelled"])
        .assert()
        .success();

    // Verify milestone is now done
    bn_in(&temp)
        .args(["milestone", "show", &milestone_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"done\""));
}

// === Cascade to Grandparent Tests ===

#[test]
fn test_milestone_cascade_to_grandparent() {
    let temp = init_binnacle();

    // Create grandparent milestone
    let output = bn_in(&temp)
        .args(["milestone", "create", "Grandparent"])
        .output()
        .unwrap();
    let grandparent_id = extract_id(&output);

    // Create parent milestone as child of grandparent
    let output = bn_in(&temp)
        .args(["milestone", "create", "Parent"])
        .output()
        .unwrap();
    let parent_id = extract_id(&output);

    bn_in(&temp)
        .args([
            "link",
            "add",
            &parent_id,
            &grandparent_id,
            "--type",
            "child_of",
        ])
        .assert()
        .success();

    // Create task as child of parent
    let output = bn_in(&temp)
        .args(["task", "create", "Child Task"])
        .output()
        .unwrap();
    let task_id = extract_id(&output);

    bn_in(&temp)
        .args(["link", "add", &task_id, &parent_id, "--type", "child_of"])
        .assert()
        .success();

    // Close the task - should cascade: parent auto-closes, then grandparent auto-closes
    let output = bn_in(&temp)
        .args(["task", "close", &task_id, "--reason", "Done"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Both parent and grandparent should be in auto_completed_milestones
    assert!(
        stdout.contains(&parent_id),
        "Expected parent milestone to be auto-closed"
    );
    assert!(
        stdout.contains(&grandparent_id),
        "Expected grandparent milestone to be auto-closed"
    );

    // Verify both milestones are done
    bn_in(&temp)
        .args(["milestone", "show", &parent_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"done\""));

    bn_in(&temp)
        .args(["milestone", "show", &grandparent_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"done\""));
}

#[test]
fn test_milestone_close_cascades_to_grandparent() {
    let temp = init_binnacle();

    // Create grandparent and parent milestones (no tasks)
    let output = bn_in(&temp)
        .args(["milestone", "create", "Grandparent"])
        .output()
        .unwrap();
    let grandparent_id = extract_id(&output);

    let output = bn_in(&temp)
        .args(["milestone", "create", "Parent"])
        .output()
        .unwrap();
    let parent_id = extract_id(&output);

    bn_in(&temp)
        .args([
            "link",
            "add",
            &parent_id,
            &grandparent_id,
            "--type",
            "child_of",
        ])
        .assert()
        .success();

    // Close parent with --force (no children) - should cascade to grandparent
    bn_in(&temp)
        .args([
            "milestone",
            "close",
            &parent_id,
            "--force",
            "--reason",
            "Done manually",
        ])
        .assert()
        .success();

    // Verify grandparent is also closed
    bn_in(&temp)
        .args(["milestone", "show", &grandparent_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"done\""));
}

// === Auto-Reopen Tests ===

#[test]
fn test_milestone_auto_reopen_on_new_child() {
    let temp = init_binnacle();

    // Create milestone with one task
    let output = bn_in(&temp)
        .args(["milestone", "create", "Reopen Test Milestone"])
        .output()
        .unwrap();
    let milestone_id = extract_id(&output);

    let output = bn_in(&temp)
        .args(["task", "create", "Original Task"])
        .output()
        .unwrap();
    let task1_id = extract_id(&output);

    bn_in(&temp)
        .args([
            "link",
            "add",
            &task1_id,
            &milestone_id,
            "--type",
            "child_of",
        ])
        .assert()
        .success();

    // Close task - milestone auto-closes
    bn_in(&temp)
        .args(["task", "close", &task1_id, "--reason", "Done"])
        .assert()
        .success();

    // Verify milestone is done
    bn_in(&temp)
        .args(["milestone", "show", &milestone_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"done\""));

    // Create and link a new task to the closed milestone
    let output = bn_in(&temp)
        .args(["task", "create", "New Task"])
        .output()
        .unwrap();
    let task2_id = extract_id(&output);

    let output = bn_in(&temp)
        .args([
            "link",
            "add",
            &task2_id,
            &milestone_id,
            "--type",
            "child_of",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify auto-reopen happened
    assert!(
        stdout.contains("auto_reopened_milestones") || stdout.contains(&milestone_id),
        "Expected auto-reopen indication in output. Got: {}",
        stdout
    );

    // Verify milestone is now pending again
    bn_in(&temp)
        .args(["milestone", "show", &milestone_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"pending\""));
}

#[test]
fn test_milestone_no_reopen_for_manual_close() {
    let temp = init_binnacle();

    // Create milestone with one task
    let output = bn_in(&temp)
        .args(["milestone", "create", "Manual Close Milestone"])
        .output()
        .unwrap();
    let milestone_id = extract_id(&output);

    let output = bn_in(&temp)
        .args(["task", "create", "Original Task"])
        .output()
        .unwrap();
    let task1_id = extract_id(&output);

    bn_in(&temp)
        .args([
            "link",
            "add",
            &task1_id,
            &milestone_id,
            "--type",
            "child_of",
        ])
        .assert()
        .success();

    // Close task to auto-close milestone
    bn_in(&temp)
        .args(["task", "close", &task1_id, "--reason", "Done"])
        .assert()
        .success();

    // Reopen milestone
    bn_in(&temp)
        .args(["milestone", "reopen", &milestone_id])
        .assert()
        .success();

    // Manually close the milestone with a custom reason
    bn_in(&temp)
        .args([
            "milestone",
            "close",
            &milestone_id,
            "--force",
            "--reason",
            "Descoped - not needed this quarter",
        ])
        .assert()
        .success();

    // Create and link a new task
    let output = bn_in(&temp)
        .args(["task", "create", "New Task After Manual Close"])
        .output()
        .unwrap();
    let task2_id = extract_id(&output);

    bn_in(&temp)
        .args([
            "link",
            "add",
            &task2_id,
            &milestone_id,
            "--type",
            "child_of",
        ])
        .assert()
        .success();

    // Milestone should remain closed (not auto-reopened)
    bn_in(&temp)
        .args(["milestone", "show", &milestone_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"done\""));
}

// === Doctor Check Tests ===

#[test]
fn test_doctor_flags_completable_milestones() {
    let temp = init_binnacle();

    // Create milestone with one task
    let output = bn_in(&temp)
        .args(["milestone", "create", "Completable Milestone"])
        .output()
        .unwrap();
    let milestone_id = extract_id(&output);

    let output = bn_in(&temp)
        .args(["task", "create", "Task"])
        .output()
        .unwrap();
    let task_id = extract_id(&output);

    bn_in(&temp)
        .args(["link", "add", &task_id, &milestone_id, "--type", "child_of"])
        .assert()
        .success();

    // Close task with --no-cascade to prevent auto-close
    bn_in(&temp)
        .args([
            "task",
            "close",
            &task_id,
            "--reason",
            "Done",
            "--no-cascade",
        ])
        .assert()
        .success();

    // Verify milestone is still pending (not auto-closed due to --no-cascade)
    bn_in(&temp)
        .args(["milestone", "show", &milestone_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"pending\""));

    // Doctor should flag this as completable
    bn_in(&temp)
        .args(["doctor", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("completable"))
        .stdout(predicate::str::contains(&milestone_id));
}

#[test]
fn test_doctor_does_not_flag_new_empty_milestones() {
    let temp = init_binnacle();

    // Create an empty milestone (just created, should not be flagged)
    let output = bn_in(&temp)
        .args(["milestone", "create", "New Empty Milestone"])
        .output()
        .unwrap();
    let milestone_id = extract_id(&output);

    // Doctor should NOT flag this new empty milestone (less than 24 hours old)
    let output = bn_in(&temp).args(["doctor"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // The milestone should not appear in empty warnings
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let issues = json["issues"].as_array().unwrap();

    let empty_warnings: Vec<_> = issues
        .iter()
        .filter(|i| {
            i["category"].as_str() == Some("empty")
                && i["entity_id"].as_str() == Some(&milestone_id)
        })
        .collect();

    assert!(
        empty_warnings.is_empty(),
        "New empty milestone should not be flagged as stale"
    );
}

// === --no-cascade Flag Tests ===

#[test]
fn test_task_close_no_cascade_flag() {
    let temp = init_binnacle();

    // Create milestone with one task
    let output = bn_in(&temp)
        .args(["milestone", "create", "No Cascade Milestone"])
        .output()
        .unwrap();
    let milestone_id = extract_id(&output);

    let output = bn_in(&temp)
        .args(["task", "create", "Task"])
        .output()
        .unwrap();
    let task_id = extract_id(&output);

    bn_in(&temp)
        .args(["link", "add", &task_id, &milestone_id, "--type", "child_of"])
        .assert()
        .success();

    // Close task with --no-cascade
    let output = bn_in(&temp)
        .args([
            "task",
            "close",
            &task_id,
            "--reason",
            "Done",
            "--no-cascade",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // auto_completed_milestones should be empty or not contain milestone_id
    if stdout.contains("auto_completed_milestones") {
        let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        let empty_vec = vec![];
        let auto_completed = json["auto_completed_milestones"]
            .as_array()
            .unwrap_or(&empty_vec);
        assert!(
            auto_completed.is_empty(),
            "Expected no auto-completed milestones with --no-cascade"
        );
    }

    // Verify milestone is still pending
    bn_in(&temp)
        .args(["milestone", "show", &milestone_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"pending\""));
}

#[test]
fn test_bug_close_no_cascade_flag() {
    let temp = init_binnacle();

    // Create milestone with one bug
    let output = bn_in(&temp)
        .args(["milestone", "create", "Bug No Cascade Milestone"])
        .output()
        .unwrap();
    let milestone_id = extract_id(&output);

    let output = bn_in(&temp)
        .args(["bug", "create", "Bug"])
        .output()
        .unwrap();
    let bug_id = extract_id(&output);

    bn_in(&temp)
        .args(["link", "add", &bug_id, &milestone_id, "--type", "child_of"])
        .assert()
        .success();

    // Close bug with --no-cascade
    bn_in(&temp)
        .args(["bug", "close", &bug_id, "--reason", "Fixed", "--no-cascade"])
        .assert()
        .success();

    // Verify milestone is still pending
    bn_in(&temp)
        .args(["milestone", "show", &milestone_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"pending\""));
}

#[test]
fn test_milestone_close_no_cascade_flag() {
    let temp = init_binnacle();

    // Create grandparent and parent milestones
    let output = bn_in(&temp)
        .args(["milestone", "create", "Grandparent No Cascade"])
        .output()
        .unwrap();
    let grandparent_id = extract_id(&output);

    let output = bn_in(&temp)
        .args(["milestone", "create", "Parent No Cascade"])
        .output()
        .unwrap();
    let parent_id = extract_id(&output);

    bn_in(&temp)
        .args([
            "link",
            "add",
            &parent_id,
            &grandparent_id,
            "--type",
            "child_of",
        ])
        .assert()
        .success();

    // Close parent with --force and --no-cascade
    bn_in(&temp)
        .args([
            "milestone",
            "close",
            &parent_id,
            "--force",
            "--no-cascade",
            "--reason",
            "Done",
        ])
        .assert()
        .success();

    // Grandparent should still be pending (no cascade)
    bn_in(&temp)
        .args(["milestone", "show", &grandparent_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"pending\""));
}

// === Mixed Children Tests ===

#[test]
fn test_milestone_auto_close_mixed_task_and_bug() {
    let temp = init_binnacle();

    // Create milestone with one task and one bug
    let output = bn_in(&temp)
        .args(["milestone", "create", "Mixed Children Milestone"])
        .output()
        .unwrap();
    let milestone_id = extract_id(&output);

    let output = bn_in(&temp)
        .args(["task", "create", "Task"])
        .output()
        .unwrap();
    let task_id = extract_id(&output);

    let output = bn_in(&temp)
        .args(["bug", "create", "Bug"])
        .output()
        .unwrap();
    let bug_id = extract_id(&output);

    bn_in(&temp)
        .args(["link", "add", &task_id, &milestone_id, "--type", "child_of"])
        .assert()
        .success();

    bn_in(&temp)
        .args(["link", "add", &bug_id, &milestone_id, "--type", "child_of"])
        .assert()
        .success();

    // Close task - milestone should still be pending
    bn_in(&temp)
        .args(["task", "close", &task_id, "--reason", "Done"])
        .assert()
        .success();

    bn_in(&temp)
        .args(["milestone", "show", &milestone_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"pending\""));

    // Close bug - milestone should now auto-close
    bn_in(&temp)
        .args(["bug", "close", &bug_id, "--reason", "Fixed"])
        .assert()
        .success();

    bn_in(&temp)
        .args(["milestone", "show", &milestone_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"done\""))
        .stdout(predicate::str::contains("All 2 children completed"));
}

#[test]
fn test_milestone_ignores_doc_children() {
    let temp = init_binnacle();

    // Create milestone with a doc (docs shouldn't count as children for completion)
    let output = bn_in(&temp)
        .args(["milestone", "create", "Doc Child Milestone"])
        .output()
        .unwrap();
    let milestone_id = extract_id(&output);

    // Create a doc linked to the milestone
    let output = bn_in(&temp)
        .args([
            "doc",
            "create",
            &milestone_id,
            "-T",
            "Test Doc",
            "-c",
            "Content",
        ])
        .output()
        .unwrap();
    let _doc_id = extract_id(&output);

    // Create a task as the only "real" child
    let output = bn_in(&temp)
        .args(["task", "create", "Task"])
        .output()
        .unwrap();
    let task_id = extract_id(&output);

    bn_in(&temp)
        .args(["link", "add", &task_id, &milestone_id, "--type", "child_of"])
        .assert()
        .success();

    // Close the task - milestone should auto-close (doc doesn't count)
    bn_in(&temp)
        .args(["task", "close", &task_id, "--reason", "Done"])
        .assert()
        .success();

    // Verify milestone closed with "Only child completed" (doc doesn't count)
    bn_in(&temp)
        .args(["milestone", "show", &milestone_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"done\""))
        .stdout(predicate::str::contains("Only child completed"));
}

#[test]
fn test_milestone_ignores_idea_children() {
    let temp = init_binnacle();

    // Create milestone with an idea (ideas shouldn't count as children for completion)
    let output = bn_in(&temp)
        .args(["milestone", "create", "Idea Child Milestone"])
        .output()
        .unwrap();
    let milestone_id = extract_id(&output);

    // Create an idea and link to milestone
    let output = bn_in(&temp)
        .args(["idea", "create", "Test Idea"])
        .output()
        .unwrap();
    let idea_id = extract_id(&output);

    bn_in(&temp)
        .args(["link", "add", &idea_id, &milestone_id, "--type", "child_of"])
        .assert()
        .success();

    // Create a task as the only "real" child
    let output = bn_in(&temp)
        .args(["task", "create", "Task"])
        .output()
        .unwrap();
    let task_id = extract_id(&output);

    bn_in(&temp)
        .args(["link", "add", &task_id, &milestone_id, "--type", "child_of"])
        .assert()
        .success();

    // Close the task - milestone should auto-close (idea doesn't count)
    bn_in(&temp)
        .args(["task", "close", &task_id, "--reason", "Done"])
        .assert()
        .success();

    // Verify milestone closed with "Only child completed" (idea doesn't count)
    bn_in(&temp)
        .args(["milestone", "show", &milestone_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"done\""))
        .stdout(predicate::str::contains("Only child completed"));
}

// === Idempotency Tests ===

#[test]
fn test_milestone_auto_close_idempotent() {
    let temp = init_binnacle();

    // Create milestone with two tasks
    let output = bn_in(&temp)
        .args(["milestone", "create", "Idempotent Milestone"])
        .output()
        .unwrap();
    let milestone_id = extract_id(&output);

    let output = bn_in(&temp)
        .args(["task", "create", "Task 1"])
        .output()
        .unwrap();
    let task1_id = extract_id(&output);

    let output = bn_in(&temp)
        .args(["task", "create", "Task 2"])
        .output()
        .unwrap();
    let task2_id = extract_id(&output);

    bn_in(&temp)
        .args([
            "link",
            "add",
            &task1_id,
            &milestone_id,
            "--type",
            "child_of",
        ])
        .assert()
        .success();

    bn_in(&temp)
        .args([
            "link",
            "add",
            &task2_id,
            &milestone_id,
            "--type",
            "child_of",
        ])
        .assert()
        .success();

    // Close both tasks (simulating concurrent agent scenario)
    bn_in(&temp)
        .args(["task", "close", &task1_id, "--reason", "Done"])
        .assert()
        .success();

    // This should be idempotent - milestone already closed by first task close
    bn_in(&temp)
        .args(["task", "close", &task2_id, "--reason", "Done"])
        .assert()
        .success();

    // Milestone should be done
    bn_in(&temp)
        .args(["milestone", "show", &milestone_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"done\""));
}

#[test]
fn test_milestone_reopen_idempotent() {
    let temp = init_binnacle();

    // Create and auto-close milestone
    let output = bn_in(&temp)
        .args(["milestone", "create", "Reopen Idempotent Milestone"])
        .output()
        .unwrap();
    let milestone_id = extract_id(&output);

    let output = bn_in(&temp)
        .args(["task", "create", "Task"])
        .output()
        .unwrap();
    let task1_id = extract_id(&output);

    bn_in(&temp)
        .args([
            "link",
            "add",
            &task1_id,
            &milestone_id,
            "--type",
            "child_of",
        ])
        .assert()
        .success();

    bn_in(&temp)
        .args(["task", "close", &task1_id, "--reason", "Done"])
        .assert()
        .success();

    // Add new child to reopen
    let output = bn_in(&temp)
        .args(["task", "create", "New Task 1"])
        .output()
        .unwrap();
    let task2_id = extract_id(&output);

    bn_in(&temp)
        .args([
            "link",
            "add",
            &task2_id,
            &milestone_id,
            "--type",
            "child_of",
        ])
        .assert()
        .success();

    // Milestone should be pending now
    bn_in(&temp)
        .args(["milestone", "show", &milestone_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"pending\""));

    // Adding another child should be idempotent (already open)
    let output = bn_in(&temp)
        .args(["task", "create", "New Task 2"])
        .output()
        .unwrap();
    let task3_id = extract_id(&output);

    // This should succeed without issues
    bn_in(&temp)
        .args([
            "link",
            "add",
            &task3_id,
            &milestone_id,
            "--type",
            "child_of",
        ])
        .assert()
        .success();

    // Milestone should still be pending
    bn_in(&temp)
        .args(["milestone", "show", &milestone_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"pending\""));
}
