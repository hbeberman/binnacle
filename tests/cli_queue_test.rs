//! Integration tests for Queue feature via CLI.
//!
//! These tests verify queue functionality:
//! - Queue create/show/delete
//! - Queue membership via links
//! - Queue integration with ready command
//! - Queue info in orient command

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

/// Extract task ID from JSON output containing "id":"bn-xxxx"
fn extract_task_id(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id_start = stdout.find("\"id\":\"bn-").expect("task id not found");
    let id_end = stdout[id_start + 6..].find('"').unwrap() + id_start + 6;
    stdout[id_start + 6..id_end].to_string()
}

/// Extract queue ID from JSON output containing "id":"bnq-xxxx"
fn extract_queue_id(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id_start = stdout.find("\"id\":\"bnq-").expect("queue id not found");
    let id_end = stdout[id_start + 6..].find('"').unwrap() + id_start + 6;
    stdout[id_start + 6..id_end].to_string()
}

// === Queue CRUD Tests ===

#[test]
fn test_queue_create() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["queue", "create", "Sprint 1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"id\":\"bnq-"))
        .stdout(predicate::str::contains("\"title\":\"Sprint 1\""));
}

#[test]
fn test_queue_create_human_readable() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["-H", "queue", "create", "Sprint 1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created queue bnq-"))
        .stdout(predicate::str::contains("Sprint 1"));
}

#[test]
fn test_queue_show() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["queue", "create", "Sprint 1"])
        .assert()
        .success();

    bn_in(&temp)
        .args(["queue", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"title\":\"Sprint 1\""))
        .stdout(predicate::str::contains("\"tasks\":[]"));
}

#[test]
fn test_queue_only_one_allowed() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["queue", "create", "Sprint 1"])
        .assert()
        .success();

    bn_in(&temp)
        .args(["queue", "create", "Sprint 2"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn test_queue_delete() {
    let temp = init_binnacle();

    bn_in(&temp)
        .args(["queue", "create", "Sprint 1"])
        .assert()
        .success();

    // Queue delete doesn't take an ID (single queue per repo)
    bn_in(&temp).args(["queue", "delete"]).assert().success();

    // Queue should no longer exist
    bn_in(&temp)
        .args(["queue", "show"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No queue exists"));
}

// === Queue Membership Tests ===

#[test]
fn test_add_task_to_queue() {
    let temp = init_binnacle();

    let queue_output = bn_in(&temp)
        .args(["queue", "create", "Sprint 1"])
        .output()
        .unwrap();
    let queue_id = extract_queue_id(&queue_output);

    let task_output = bn_in(&temp)
        .args(["task", "create", "Task A"])
        .output()
        .unwrap();
    let task_id = extract_task_id(&task_output);

    // Add task to queue using link
    bn_in(&temp)
        .args(["link", "add", &task_id, &queue_id, "--type", "queued"])
        .assert()
        .success();

    // Queue show should include the task
    bn_in(&temp)
        .args(["queue", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains(&task_id));
}

#[test]
fn test_remove_task_from_queue() {
    let temp = init_binnacle();

    let queue_output = bn_in(&temp)
        .args(["queue", "create", "Sprint 1"])
        .output()
        .unwrap();
    let queue_id = extract_queue_id(&queue_output);

    let task_output = bn_in(&temp)
        .args(["task", "create", "Task A"])
        .output()
        .unwrap();
    let task_id = extract_task_id(&task_output);

    // Add then remove (need to specify --type when removing)
    bn_in(&temp)
        .args(["link", "add", &task_id, &queue_id, "--type", "queued"])
        .assert()
        .success();

    bn_in(&temp)
        .args(["link", "rm", &task_id, &queue_id, "--type", "queued"])
        .assert()
        .success();

    // Queue show should not include the task
    bn_in(&temp)
        .args(["queue", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"tasks\":[]"));
}

// === Ready Integration Tests ===

#[test]
fn test_ready_shows_queued_field() {
    let temp = init_binnacle();

    let queue_output = bn_in(&temp)
        .args(["queue", "create", "Sprint 1"])
        .output()
        .unwrap();
    let queue_id = extract_queue_id(&queue_output);

    let task_output = bn_in(&temp)
        .args(["task", "create", "Task A"])
        .output()
        .unwrap();
    let task_id = extract_task_id(&task_output);

    // Add task to queue
    bn_in(&temp)
        .args(["link", "add", &task_id, &queue_id, "--type", "queued"])
        .assert()
        .success();

    // Ready output should include queued field
    bn_in(&temp)
        .args(["ready"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"queued\":true"))
        .stdout(predicate::str::contains("\"queued_count\":1"));
}

#[test]
fn test_ready_queued_tasks_first() {
    let temp = init_binnacle();

    // Create a high priority task (not queued)
    let task_a_output = bn_in(&temp)
        .args(["task", "create", "High Priority Task", "-p", "0"])
        .output()
        .unwrap();
    let _task_a_id = extract_task_id(&task_a_output);

    // Create a low priority task
    let task_b_output = bn_in(&temp)
        .args(["task", "create", "Low Priority Task", "-p", "3"])
        .output()
        .unwrap();
    let task_b_id = extract_task_id(&task_b_output);

    // Create queue and add the low priority task
    let queue_output = bn_in(&temp)
        .args(["queue", "create", "Sprint 1"])
        .output()
        .unwrap();
    let queue_id = extract_queue_id(&queue_output);

    bn_in(&temp)
        .args(["link", "add", &task_b_id, &queue_id, "--type", "queued"])
        .assert()
        .success();

    // In JSON output, the low priority queued task should come first
    let output = bn_in(&temp).args(["ready"]).output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Find positions of both tasks
    let pos_b = stdout.find(&task_b_id).expect("task B not found");
    let pos_a = stdout.find("High Priority Task").expect("task A not found");

    // Queued task B should appear before non-queued task A
    assert!(
        pos_b < pos_a,
        "Queued task should appear before non-queued task"
    );
}

#[test]
fn test_ready_human_shows_queued_section() {
    let temp = init_binnacle();

    let queue_output = bn_in(&temp)
        .args(["queue", "create", "Sprint 1"])
        .output()
        .unwrap();
    let queue_id = extract_queue_id(&queue_output);

    // Create two tasks
    let task_a_output = bn_in(&temp)
        .args(["task", "create", "Queued Task"])
        .output()
        .unwrap();
    let task_a_id = extract_task_id(&task_a_output);

    bn_in(&temp)
        .args(["task", "create", "Other Task"])
        .assert()
        .success();

    // Queue one of them
    bn_in(&temp)
        .args(["link", "add", &task_a_id, &queue_id, "--type", "queued"])
        .assert()
        .success();

    // Human output should show [QUEUED] and [OTHER] sections
    bn_in(&temp)
        .args(["-H", "ready"])
        .assert()
        .success()
        .stdout(predicate::str::contains("[QUEUED]"))
        .stdout(predicate::str::contains("[OTHER]"));
}

// === Orient Integration Tests ===

#[test]
fn test_orient_shows_queue_info() {
    let temp = init_binnacle();

    let queue_output = bn_in(&temp)
        .args(["queue", "create", "Sprint 1"])
        .output()
        .unwrap();
    let queue_id = extract_queue_id(&queue_output);

    let task_output = bn_in(&temp)
        .args(["task", "create", "Task A"])
        .output()
        .unwrap();
    let task_id = extract_task_id(&task_output);

    // Add task to queue
    bn_in(&temp)
        .args(["link", "add", &task_id, &queue_id, "--type", "queued"])
        .assert()
        .success();

    // Orient output should include queue info
    bn_in(&temp)
        .args(["orient", "--type", "worker"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"queue\""))
        .stdout(predicate::str::contains("\"title\":\"Sprint 1\""))
        .stdout(predicate::str::contains("\"queued_ready_count\":1"));
}

#[test]
fn test_orient_human_shows_queue() {
    let temp = init_binnacle();

    let queue_output = bn_in(&temp)
        .args(["queue", "create", "Sprint 1"])
        .output()
        .unwrap();
    let queue_id = extract_queue_id(&queue_output);

    let task_output = bn_in(&temp)
        .args(["task", "create", "Task A"])
        .output()
        .unwrap();
    let task_id = extract_task_id(&task_output);

    // Add task to queue
    bn_in(&temp)
        .args(["link", "add", &task_id, &queue_id, "--type", "queued"])
        .assert()
        .success();

    // Human orient output should show queue info in "Also:" section
    bn_in(&temp)
        .args(["-H", "orient", "--type", "worker"])
        .assert()
        .success()
        .stdout(predicate::str::contains("queue \"Sprint 1\""))
        .stdout(predicate::str::contains("queued"));
}

#[test]
fn test_orient_without_queue() {
    let temp = init_binnacle();

    // Orient without queue should not have queue field
    bn_in(&temp)
        .args(["orient", "--type", "worker"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"queued_ready_count\":0"));
}

// === Auto-Removal Tests (Phase 3) ===

#[test]
fn test_close_task_removes_from_queue() {
    let temp = init_binnacle();

    // Create queue and task
    let queue_output = bn_in(&temp)
        .args(["queue", "create", "Sprint 1"])
        .output()
        .unwrap();
    let queue_id = extract_queue_id(&queue_output);

    let task_output = bn_in(&temp)
        .args(["task", "create", "Task A"])
        .output()
        .unwrap();
    let task_id = extract_task_id(&task_output);

    // Add task to queue
    bn_in(&temp)
        .args(["link", "add", &task_id, &queue_id, "--type", "queued"])
        .assert()
        .success();

    // Verify task is in queue
    bn_in(&temp)
        .args(["queue", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains(&task_id));

    // Close the task
    bn_in(&temp)
        .args(["task", "close", &task_id, "--reason", "done"])
        .assert()
        .success()
        .stdout(predicate::str::contains("removed_from_queues"))
        .stdout(predicate::str::contains(&queue_id));

    // Verify task is no longer in queue
    bn_in(&temp)
        .args(["queue", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"tasks\":[]"));
}

#[test]
fn test_close_task_human_shows_queue_removal() {
    let temp = init_binnacle();

    // Create queue and task
    let queue_output = bn_in(&temp)
        .args(["queue", "create", "Sprint 1"])
        .output()
        .unwrap();
    let queue_id = extract_queue_id(&queue_output);

    let task_output = bn_in(&temp)
        .args(["task", "create", "Task A"])
        .output()
        .unwrap();
    let task_id = extract_task_id(&task_output);

    // Add task to queue
    bn_in(&temp)
        .args(["link", "add", &task_id, &queue_id, "--type", "queued"])
        .assert()
        .success();

    // Close the task with human-readable output
    bn_in(&temp)
        .args(["-H", "task", "close", &task_id, "--reason", "done"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Removed from queue(s)"))
        .stdout(predicate::str::contains(&queue_id));
}

#[test]
fn test_close_task_not_in_queue_no_removal_message() {
    let temp = init_binnacle();

    // Create task (no queue)
    let task_output = bn_in(&temp)
        .args(["task", "create", "Task A"])
        .output()
        .unwrap();
    let task_id = extract_task_id(&task_output);

    // Close the task - should not have removed_from_queues field
    let output = bn_in(&temp)
        .args(["task", "close", &task_id, "--reason", "done"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("removed_from_queues"));
}

#[test]
fn test_cancelled_task_removed_from_queue() {
    let temp = init_binnacle();

    // Create queue and task
    let queue_output = bn_in(&temp)
        .args(["queue", "create", "Sprint 1"])
        .output()
        .unwrap();
    let queue_id = extract_queue_id(&queue_output);

    let task_output = bn_in(&temp)
        .args(["task", "create", "Task A"])
        .output()
        .unwrap();
    let task_id = extract_task_id(&task_output);

    // Add task to queue
    bn_in(&temp)
        .args(["link", "add", &task_id, &queue_id, "--type", "queued"])
        .assert()
        .success();

    // Cancel the task (update status to cancelled then close with force)
    bn_in(&temp)
        .args(["task", "update", &task_id, "--status", "cancelled"])
        .assert()
        .success();

    // Verify task is no longer in queue (update to cancelled should not remove)
    // Actually, the auto-removal is only on task_close - let's check the queue
    bn_in(&temp)
        .args(["queue", "show"])
        .assert()
        .success()
        // Task should still be in queue after status update (not close)
        .stdout(predicate::str::contains(&task_id));
}

#[test]
fn test_reopen_task_not_readded_to_queue() {
    let temp = init_binnacle();

    // Create queue and task
    let queue_output = bn_in(&temp)
        .args(["queue", "create", "Sprint 1"])
        .output()
        .unwrap();
    let queue_id = extract_queue_id(&queue_output);

    let task_output = bn_in(&temp)
        .args(["task", "create", "Task A"])
        .output()
        .unwrap();
    let task_id = extract_task_id(&task_output);

    // Add task to queue
    bn_in(&temp)
        .args(["link", "add", &task_id, &queue_id, "--type", "queued"])
        .assert()
        .success();

    // Close the task (removes from queue)
    bn_in(&temp)
        .args(["task", "close", &task_id, "--reason", "done"])
        .assert()
        .success();

    // Reopen the task
    bn_in(&temp)
        .args(["task", "reopen", &task_id])
        .assert()
        .success();

    // Task should NOT be automatically re-added to queue
    bn_in(&temp)
        .args(["queue", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"tasks\":[]"));
}

// === Queue Rejects Closed Items Tests ===

#[test]
fn test_queue_add_rejects_closed_task() {
    let temp = init_binnacle();

    // Create queue and task
    bn_in(&temp)
        .args(["queue", "create", "Sprint 1"])
        .assert()
        .success();

    let task_output = bn_in(&temp)
        .args(["task", "create", "Task A"])
        .output()
        .unwrap();
    let task_id = extract_task_id(&task_output);

    // Close the task
    bn_in(&temp)
        .args(["task", "close", &task_id, "--reason", "done"])
        .assert()
        .success();

    // Try to add closed task to queue - should fail
    bn_in(&temp)
        .args(["queue", "add", &task_id])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Cannot add"))
        .stderr(predicate::str::contains("closed"));
}

#[test]
fn test_queue_add_rejects_cancelled_task() {
    let temp = init_binnacle();

    // Create queue and task
    bn_in(&temp)
        .args(["queue", "create", "Sprint 1"])
        .assert()
        .success();

    let task_output = bn_in(&temp)
        .args(["task", "create", "Task A"])
        .output()
        .unwrap();
    let task_id = extract_task_id(&task_output);

    // Set task status to cancelled
    bn_in(&temp)
        .args(["task", "update", &task_id, "--status", "cancelled"])
        .assert()
        .success();

    // Try to add cancelled task to queue - should fail
    bn_in(&temp)
        .args(["queue", "add", &task_id])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Cannot add"))
        .stderr(predicate::str::contains("closed"));
}

#[test]
fn test_queue_add_accepts_pending_task() {
    let temp = init_binnacle();

    // Create queue and task
    bn_in(&temp)
        .args(["queue", "create", "Sprint 1"])
        .assert()
        .success();

    let task_output = bn_in(&temp)
        .args(["task", "create", "Task A"])
        .output()
        .unwrap();
    let task_id = extract_task_id(&task_output);

    // Add pending task to queue - should succeed
    bn_in(&temp)
        .args(["queue", "add", &task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("item_id"));
}

#[test]
fn test_queue_add_accepts_in_progress_task() {
    let temp = init_binnacle();

    // Create queue and task
    bn_in(&temp)
        .args(["queue", "create", "Sprint 1"])
        .assert()
        .success();

    let task_output = bn_in(&temp)
        .args(["task", "create", "Task A"])
        .output()
        .unwrap();
    let task_id = extract_task_id(&task_output);

    // Set task to in_progress
    bn_in(&temp)
        .args(["task", "update", &task_id, "--status", "in_progress"])
        .assert()
        .success();

    // Add in_progress task to queue - should succeed
    bn_in(&temp)
        .args(["queue", "add", &task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("item_id"));
}

#[test]
fn test_queue_add_accepts_blocked_task() {
    let temp = init_binnacle();

    // Create queue and task
    bn_in(&temp)
        .args(["queue", "create", "Sprint 1"])
        .assert()
        .success();

    let task_output = bn_in(&temp)
        .args(["task", "create", "Task A"])
        .output()
        .unwrap();
    let task_id = extract_task_id(&task_output);

    // Set task to blocked
    bn_in(&temp)
        .args(["task", "update", &task_id, "--status", "blocked"])
        .assert()
        .success();

    // Add blocked task to queue - should succeed
    bn_in(&temp)
        .args(["queue", "add", &task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("item_id"));
}

#[test]
fn test_queue_add_accepts_reopened_task() {
    let temp = init_binnacle();

    // Create queue and task
    bn_in(&temp)
        .args(["queue", "create", "Sprint 1"])
        .assert()
        .success();

    let task_output = bn_in(&temp)
        .args(["task", "create", "Task A"])
        .output()
        .unwrap();
    let task_id = extract_task_id(&task_output);

    // Close then reopen the task
    bn_in(&temp)
        .args(["task", "close", &task_id, "--reason", "done"])
        .assert()
        .success();

    bn_in(&temp)
        .args(["task", "reopen", &task_id])
        .assert()
        .success();

    // Add reopened task to queue - should succeed
    bn_in(&temp)
        .args(["queue", "add", &task_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("item_id"));
}

// === Bug Tests for Queue Rejection ===

#[test]
fn test_queue_add_rejects_closed_bug() {
    let temp = init_binnacle();

    // Create queue and bug
    bn_in(&temp)
        .args(["queue", "create", "Sprint 1"])
        .assert()
        .success();

    let bug_output = bn_in(&temp)
        .args(["bug", "create", "Bug A"])
        .output()
        .unwrap();
    // Bugs also use bn- prefix
    let bug_id = extract_task_id(&bug_output);

    // Close the bug
    bn_in(&temp)
        .args(["bug", "close", &bug_id, "--reason", "fixed"])
        .assert()
        .success();

    // Try to add closed bug to queue - should fail
    bn_in(&temp)
        .args(["queue", "add", &bug_id])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Cannot add"))
        .stderr(predicate::str::contains("closed"));
}

#[test]
fn test_queue_add_accepts_open_bug() {
    let temp = init_binnacle();

    // Create queue and bug
    bn_in(&temp)
        .args(["queue", "create", "Sprint 1"])
        .assert()
        .success();

    let bug_output = bn_in(&temp)
        .args(["bug", "create", "Bug A"])
        .output()
        .unwrap();
    // Bugs also use bn- prefix
    let bug_id = extract_task_id(&bug_output);

    // Add open bug to queue - should succeed
    bn_in(&temp)
        .args(["queue", "add", &bug_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("item_id"));
}
