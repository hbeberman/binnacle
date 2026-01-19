//! Command implementations for Binnacle CLI.
//!
//! This module contains the business logic for each CLI command.
//! Commands are organized by entity type:
//! - `init` - Initialize binnacle for a repository
//! - `task` - Task CRUD operations
//! - `dep` - Dependency management
//! - `test` - Test node operations
//! - `commit` - Commit tracking

use crate::models::{Task, TaskStatus, TestNode, TestResult};
use crate::storage::{generate_id, parse_status, Storage};
use crate::{Error, Result};
use chrono::Utc;
use serde::Serialize;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

/// Output format trait for commands.
pub trait Output {
    /// Serialize to JSON string.
    fn to_json(&self) -> String;
    /// Format for human-readable output.
    fn to_human(&self) -> String;
}

/// Generic result wrapper for command outputs.
#[derive(Serialize)]
pub struct CommandOutput<T: Serialize> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T: Serialize> CommandOutput<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg.into()),
        }
    }
}

// === Init Command ===

#[derive(Serialize)]
pub struct InitResult {
    pub initialized: bool,
    pub storage_path: String,
}

impl Output for InitResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if self.initialized {
            format!("Initialized binnacle at {}", self.storage_path)
        } else {
            format!("Binnacle already initialized at {}", self.storage_path)
        }
    }
}

/// Initialize binnacle for the current repository.
pub fn init(repo_path: &Path) -> Result<InitResult> {
    let already_exists = Storage::exists(repo_path)?;
    let storage = if already_exists {
        Storage::open(repo_path)?
    } else {
        Storage::init(repo_path)?
    };

    Ok(InitResult {
        initialized: !already_exists,
        storage_path: storage.root().to_string_lossy().to_string(),
    })
}

// === Task Commands ===

#[derive(Serialize)]
pub struct TaskCreated {
    pub id: String,
    pub title: String,
}

impl Output for TaskCreated {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Created task {} \"{}\"", self.id, self.title)
    }
}

/// Create a new task.
pub fn task_create(
    repo_path: &Path,
    title: String,
    description: Option<String>,
    priority: Option<u8>,
    tags: Vec<String>,
    assignee: Option<String>,
) -> Result<TaskCreated> {
    let mut storage = Storage::open(repo_path)?;

    // Validate priority if provided
    if let Some(p) = priority {
        if p > 4 {
            return Err(Error::Other("Priority must be 0-4".to_string()));
        }
    }

    let id = generate_id("bn", &title);
    let mut task = Task::new(id.clone(), title.clone());
    task.description = description;
    task.priority = priority.unwrap_or(2);
    task.tags = tags;
    task.assignee = assignee;

    storage.create_task(&task)?;

    Ok(TaskCreated { id, title })
}

impl Output for Task {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("{} {}", self.id, self.title));
        lines.push(format!(
            "  Status: {:?}  Priority: {}",
            self.status, self.priority
        ));
        if let Some(ref desc) = self.description {
            lines.push(format!("  Description: {}", desc));
        }
        if !self.tags.is_empty() {
            lines.push(format!("  Tags: {}", self.tags.join(", ")));
        }
        if let Some(ref assignee) = self.assignee {
            lines.push(format!("  Assignee: {}", assignee));
        }
        if !self.depends_on.is_empty() {
            lines.push(format!("  Depends on: {}", self.depends_on.join(", ")));
        }
        lines.push(format!(
            "  Created: {}",
            self.created_at.format("%Y-%m-%d %H:%M")
        ));
        lines.push(format!(
            "  Updated: {}",
            self.updated_at.format("%Y-%m-%d %H:%M")
        ));
        if let Some(closed) = self.closed_at {
            lines.push(format!("  Closed: {}", closed.format("%Y-%m-%d %H:%M")));
            if let Some(ref reason) = self.closed_reason {
                lines.push(format!("  Reason: {}", reason));
            }
        }
        lines.join("\n")
    }
}

/// Show a task by ID.
pub fn task_show(repo_path: &Path, id: &str) -> Result<Task> {
    let storage = Storage::open(repo_path)?;
    storage.get_task(id)
}

#[derive(Serialize)]
pub struct TaskList {
    pub tasks: Vec<Task>,
    pub count: usize,
}

impl Output for TaskList {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if self.tasks.is_empty() {
            return "No tasks found.".to_string();
        }

        let mut lines = Vec::new();
        lines.push(format!("{} task(s):\n", self.count));

        for task in &self.tasks {
            let status_char = match task.status {
                TaskStatus::Pending => " ",
                TaskStatus::InProgress => ">",
                TaskStatus::Done => "x",
                TaskStatus::Blocked => "!",
                TaskStatus::Cancelled => "-",
                TaskStatus::Reopened => "?",
            };
            let tags = if task.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", task.tags.join(", "))
            };
            lines.push(format!(
                "[{}] {} P{} {}{}",
                status_char, task.id, task.priority, task.title, tags
            ));
        }

        lines.join("\n")
    }
}

/// List tasks with optional filters.
pub fn task_list(
    repo_path: &Path,
    status: Option<&str>,
    priority: Option<u8>,
    tag: Option<&str>,
) -> Result<TaskList> {
    let storage = Storage::open(repo_path)?;
    let tasks = storage.list_tasks(status, priority, tag)?;
    let count = tasks.len();
    Ok(TaskList { tasks, count })
}

#[derive(Serialize)]
pub struct TaskUpdated {
    pub id: String,
    pub updated_fields: Vec<String>,
}

impl Output for TaskUpdated {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!(
            "Updated task {}: {}",
            self.id,
            self.updated_fields.join(", ")
        )
    }
}

/// Update a task.
#[allow(clippy::too_many_arguments)]
pub fn task_update(
    repo_path: &Path,
    id: &str,
    title: Option<String>,
    description: Option<String>,
    priority: Option<u8>,
    status: Option<&str>,
    add_tags: Vec<String>,
    remove_tags: Vec<String>,
    assignee: Option<String>,
) -> Result<TaskUpdated> {
    let mut storage = Storage::open(repo_path)?;
    let mut task = storage.get_task(id)?;
    let mut updated_fields = Vec::new();

    if let Some(t) = title {
        task.title = t;
        updated_fields.push("title".to_string());
    }

    if let Some(d) = description {
        task.description = Some(d);
        updated_fields.push("description".to_string());
    }

    if let Some(p) = priority {
        if p > 4 {
            return Err(Error::Other("Priority must be 0-4".to_string()));
        }
        task.priority = p;
        updated_fields.push("priority".to_string());
    }

    if let Some(s) = status {
        task.status = parse_status(s)?;
        updated_fields.push("status".to_string());
    }

    if !add_tags.is_empty() {
        for tag in add_tags {
            if !task.tags.contains(&tag) {
                task.tags.push(tag);
            }
        }
        updated_fields.push("tags".to_string());
    }

    if !remove_tags.is_empty() {
        task.tags.retain(|t| !remove_tags.contains(t));
        if !updated_fields.contains(&"tags".to_string()) {
            updated_fields.push("tags".to_string());
        }
    }

    if let Some(a) = assignee {
        task.assignee = Some(a);
        updated_fields.push("assignee".to_string());
    }

    if updated_fields.is_empty() {
        return Err(Error::Other("No fields to update".to_string()));
    }

    task.updated_at = Utc::now();
    storage.update_task(&task)?;

    Ok(TaskUpdated {
        id: id.to_string(),
        updated_fields,
    })
}

#[derive(Serialize)]
pub struct TaskClosed {
    pub id: String,
    pub status: String,
}

impl Output for TaskClosed {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Closed task {}", self.id)
    }
}

/// Close a task.
pub fn task_close(repo_path: &Path, id: &str, reason: Option<String>) -> Result<TaskClosed> {
    let mut storage = Storage::open(repo_path)?;
    let mut task = storage.get_task(id)?;

    task.status = TaskStatus::Done;
    task.closed_at = Some(Utc::now());
    task.closed_reason = reason;
    task.updated_at = Utc::now();

    storage.update_task(&task)?;

    Ok(TaskClosed {
        id: id.to_string(),
        status: "done".to_string(),
    })
}

#[derive(Serialize)]
pub struct TaskReopened {
    pub id: String,
    pub status: String,
}

impl Output for TaskReopened {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Reopened task {}", self.id)
    }
}

/// Reopen a closed task.
pub fn task_reopen(repo_path: &Path, id: &str) -> Result<TaskReopened> {
    let mut storage = Storage::open(repo_path)?;
    let mut task = storage.get_task(id)?;

    task.status = TaskStatus::Reopened;
    task.closed_at = None;
    task.closed_reason = None;
    task.updated_at = Utc::now();

    storage.update_task(&task)?;

    Ok(TaskReopened {
        id: id.to_string(),
        status: "reopened".to_string(),
    })
}

#[derive(Serialize)]
pub struct TaskDeleted {
    pub id: String,
}

impl Output for TaskDeleted {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Deleted task {}", self.id)
    }
}

/// Delete a task.
pub fn task_delete(repo_path: &Path, id: &str) -> Result<TaskDeleted> {
    let mut storage = Storage::open(repo_path)?;
    storage.delete_task(id)?;

    Ok(TaskDeleted { id: id.to_string() })
}

// === Status Summary ===

#[derive(Serialize)]
pub struct StatusSummary {
    pub tasks: Vec<Task>,
    pub ready: Vec<String>,
    pub blocked: Vec<String>,
    pub in_progress: Vec<String>,
}

impl Output for StatusSummary {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();

        lines.push(format!("Binnacle - {} total task(s)", self.tasks.len()));

        if !self.in_progress.is_empty() {
            lines.push(format!("  In Progress: {}", self.in_progress.join(", ")));
        }
        if !self.ready.is_empty() {
            lines.push(format!("  Ready: {}", self.ready.join(", ")));
        }
        if !self.blocked.is_empty() {
            lines.push(format!("  Blocked: {}", self.blocked.join(", ")));
        }

        if self.tasks.is_empty() {
            lines.push("Run `bn task create \"Title\"` to add a task.".to_string());
        }

        lines.join("\n")
    }
}

/// Get status summary.
pub fn status(repo_path: &Path) -> Result<StatusSummary> {
    let storage = Storage::open(repo_path)?;
    let tasks = storage.list_tasks(None, None, None)?;

    let mut ready = Vec::new();
    let mut blocked = Vec::new();
    let mut in_progress = Vec::new();

    for task in &tasks {
        match task.status {
            TaskStatus::InProgress => in_progress.push(task.id.clone()),
            TaskStatus::Blocked => blocked.push(task.id.clone()),
            TaskStatus::Pending | TaskStatus::Reopened => {
                // Check if all dependencies are done
                if task.depends_on.is_empty() {
                    ready.push(task.id.clone());
                } else {
                    let all_done = task.depends_on.iter().all(|dep_id| {
                        storage
                            .get_task(dep_id)
                            .map(|t| t.status == TaskStatus::Done)
                            .unwrap_or(false)
                    });
                    if all_done {
                        ready.push(task.id.clone());
                    } else {
                        blocked.push(task.id.clone());
                    }
                }
            }
            _ => {}
        }
    }

    Ok(StatusSummary {
        tasks,
        ready,
        blocked,
        in_progress,
    })
}

// === Dependency Commands ===

#[derive(Serialize)]
pub struct DepAdded {
    pub child: String,
    pub parent: String,
}

impl Output for DepAdded {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!(
            "Added dependency: {} depends on {}",
            self.child, self.parent
        )
    }
}

/// Add a dependency (child depends on parent).
pub fn dep_add(repo_path: &Path, child: &str, parent: &str) -> Result<DepAdded> {
    let mut storage = Storage::open(repo_path)?;
    storage.add_dependency(child, parent)?;

    Ok(DepAdded {
        child: child.to_string(),
        parent: parent.to_string(),
    })
}

#[derive(Serialize)]
pub struct DepRemoved {
    pub child: String,
    pub parent: String,
}

impl Output for DepRemoved {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!(
            "Removed dependency: {} no longer depends on {}",
            self.child, self.parent
        )
    }
}

/// Remove a dependency.
pub fn dep_rm(repo_path: &Path, child: &str, parent: &str) -> Result<DepRemoved> {
    let mut storage = Storage::open(repo_path)?;
    storage.remove_dependency(child, parent)?;

    Ok(DepRemoved {
        child: child.to_string(),
        parent: parent.to_string(),
    })
}

#[derive(Serialize)]
pub struct DepGraph {
    pub task_id: String,
    pub depends_on: Vec<String>,
    pub dependents: Vec<String>,
    pub transitive_deps: Vec<String>,
}

impl Output for DepGraph {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("Dependency graph for {}:", self.task_id));

        if self.depends_on.is_empty() {
            lines.push("  Depends on: (none)".to_string());
        } else {
            lines.push(format!("  Depends on: {}", self.depends_on.join(", ")));
        }

        if self.dependents.is_empty() {
            lines.push("  Dependents: (none)".to_string());
        } else {
            lines.push(format!("  Dependents: {}", self.dependents.join(", ")));
        }

        if !self.transitive_deps.is_empty() {
            lines.push(format!(
                "  All blockers: {}",
                self.transitive_deps.join(", ")
            ));
        }

        lines.join("\n")
    }
}

/// Show dependency graph for a task.
pub fn dep_show(repo_path: &Path, id: &str) -> Result<DepGraph> {
    let storage = Storage::open(repo_path)?;

    // Verify task exists
    storage.get_task(id)?;

    let depends_on = storage.get_dependencies(id)?;
    let dependents = storage.get_dependents(id)?;

    // Calculate transitive dependencies (all blockers)
    let mut transitive_deps = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut stack: Vec<String> = depends_on.clone();

    while let Some(current) = stack.pop() {
        if visited.contains(&current) {
            continue;
        }
        visited.insert(current.clone());
        transitive_deps.push(current.clone());

        if let Ok(deps) = storage.get_dependencies(&current) {
            for dep in deps {
                if !visited.contains(&dep) {
                    stack.push(dep);
                }
            }
        }
    }

    Ok(DepGraph {
        task_id: id.to_string(),
        depends_on,
        dependents,
        transitive_deps,
    })
}

// === Query Commands ===

#[derive(Serialize)]
pub struct ReadyTasks {
    pub tasks: Vec<Task>,
    pub count: usize,
}

impl Output for ReadyTasks {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if self.tasks.is_empty() {
            return "No ready tasks.".to_string();
        }

        let mut lines = Vec::new();
        lines.push(format!("{} ready task(s):\n", self.count));

        for task in &self.tasks {
            let tags = if task.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", task.tags.join(", "))
            };
            lines.push(format!(
                "  {} P{} {}{}",
                task.id, task.priority, task.title, tags
            ));
        }

        lines.join("\n")
    }
}

/// Get tasks that are ready (no open blockers).
pub fn ready(repo_path: &Path) -> Result<ReadyTasks> {
    let storage = Storage::open(repo_path)?;
    let tasks = storage.get_ready_tasks()?;
    let count = tasks.len();
    Ok(ReadyTasks { tasks, count })
}

#[derive(Serialize)]
pub struct BlockedTasks {
    pub tasks: Vec<BlockedTask>,
    pub count: usize,
}

#[derive(Serialize)]
pub struct BlockedTask {
    #[serde(flatten)]
    pub task: Task,
    pub blocking_tasks: Vec<String>,
}

impl Output for BlockedTasks {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if self.tasks.is_empty() {
            return "No blocked tasks.".to_string();
        }

        let mut lines = Vec::new();
        lines.push(format!("{} blocked task(s):\n", self.count));

        for bt in &self.tasks {
            let task = &bt.task;
            let blockers = if bt.blocking_tasks.is_empty() {
                String::new()
            } else {
                format!(" (blocked by: {})", bt.blocking_tasks.join(", "))
            };
            lines.push(format!(
                "  {} P{} {}{}",
                task.id, task.priority, task.title, blockers
            ));
        }

        lines.join("\n")
    }
}

/// Get tasks that are blocked (waiting on dependencies).
pub fn blocked(repo_path: &Path) -> Result<BlockedTasks> {
    let storage = Storage::open(repo_path)?;
    let tasks = storage.get_blocked_tasks()?;

    let mut blocked_tasks = Vec::new();
    for task in tasks {
        // Find which dependencies are blocking
        let blocking: Vec<String> = task
            .depends_on
            .iter()
            .filter(|dep_id| {
                storage
                    .get_task(dep_id)
                    .map(|t| t.status != TaskStatus::Done)
                    .unwrap_or(true)
            })
            .cloned()
            .collect();

        blocked_tasks.push(BlockedTask {
            task,
            blocking_tasks: blocking,
        });
    }

    let count = blocked_tasks.len();
    Ok(BlockedTasks {
        tasks: blocked_tasks,
        count,
    })
}

// === Test Node Commands ===

#[derive(Serialize)]
pub struct TestCreated {
    pub id: String,
    pub name: String,
}

impl Output for TestCreated {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Created test {} \"{}\"", self.id, self.name)
    }
}

/// Create a new test node.
pub fn test_create(
    repo_path: &Path,
    name: String,
    command: String,
    working_dir: String,
    task_id: Option<String>,
) -> Result<TestCreated> {
    let mut storage = Storage::open(repo_path)?;

    let id = generate_id("bnt", &name);
    let mut test = TestNode::new(id.clone(), name.clone(), command);
    test.working_dir = working_dir;

    // If task_id provided, link immediately
    if let Some(tid) = task_id {
        // Verify task exists
        storage.get_task(&tid)?;
        test.linked_tasks.push(tid);
    }

    storage.create_test(&test)?;

    Ok(TestCreated { id, name })
}

impl Output for TestNode {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("{} {}", self.id, self.name));
        lines.push(format!("  Command: {}", self.command));
        lines.push(format!("  Working dir: {}", self.working_dir));
        if let Some(ref pattern) = self.pattern {
            lines.push(format!("  Pattern: {}", pattern));
        }
        if self.linked_tasks.is_empty() {
            lines.push("  Linked tasks: (none)".to_string());
        } else {
            lines.push(format!("  Linked tasks: {}", self.linked_tasks.join(", ")));
        }
        lines.push(format!(
            "  Created: {}",
            self.created_at.format("%Y-%m-%d %H:%M")
        ));
        lines.join("\n")
    }
}

/// Show a test node by ID.
pub fn test_show(repo_path: &Path, id: &str) -> Result<TestNode> {
    let storage = Storage::open(repo_path)?;
    storage.get_test(id)
}

#[derive(Serialize)]
pub struct TestList {
    pub tests: Vec<TestNode>,
    pub count: usize,
}

impl Output for TestList {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if self.tests.is_empty() {
            return "No tests found.".to_string();
        }

        let mut lines = Vec::new();
        lines.push(format!("{} test(s):\n", self.count));

        for test in &self.tests {
            let links = if test.linked_tasks.is_empty() {
                String::new()
            } else {
                format!(" -> {}", test.linked_tasks.join(", "))
            };
            lines.push(format!("  {} {}{}", test.id, test.name, links));
        }

        lines.join("\n")
    }
}

/// List test nodes with optional filters.
pub fn test_list(repo_path: &Path, task_id: Option<&str>) -> Result<TestList> {
    let storage = Storage::open(repo_path)?;
    let tests = storage.list_tests(task_id)?;
    let count = tests.len();
    Ok(TestList { tests, count })
}

#[derive(Serialize)]
pub struct TestLinked {
    pub test_id: String,
    pub task_id: String,
}

impl Output for TestLinked {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Linked test {} to task {}", self.test_id, self.task_id)
    }
}

/// Link a test to a task.
pub fn test_link(repo_path: &Path, test_id: &str, task_id: &str) -> Result<TestLinked> {
    let mut storage = Storage::open(repo_path)?;
    storage.link_test_to_task(test_id, task_id)?;

    Ok(TestLinked {
        test_id: test_id.to_string(),
        task_id: task_id.to_string(),
    })
}

#[derive(Serialize)]
pub struct TestUnlinked {
    pub test_id: String,
    pub task_id: String,
}

impl Output for TestUnlinked {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Unlinked test {} from task {}", self.test_id, self.task_id)
    }
}

/// Unlink a test from a task.
pub fn test_unlink(repo_path: &Path, test_id: &str, task_id: &str) -> Result<TestUnlinked> {
    let mut storage = Storage::open(repo_path)?;
    storage.unlink_test_from_task(test_id, task_id)?;

    Ok(TestUnlinked {
        test_id: test_id.to_string(),
        task_id: task_id.to_string(),
    })
}

#[derive(Serialize)]
pub struct TestRunResult {
    pub test_id: String,
    pub test_name: String,
    pub passed: bool,
    pub exit_code: i32,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub reopened_tasks: Vec<String>,
}

impl Output for TestRunResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let status = if self.passed { "PASSED" } else { "FAILED" };
        let mut lines = Vec::new();
        lines.push(format!(
            "{} {} ({}) - {}ms",
            status, self.test_name, self.test_id, self.duration_ms
        ));

        if !self.passed {
            if let Some(ref stderr) = self.stderr {
                if !stderr.is_empty() {
                    lines.push(format!(
                        "  stderr: {}",
                        stderr
                            .lines()
                            .take(5)
                            .collect::<Vec<_>>()
                            .join("\n         ")
                    ));
                }
            }
        }

        if !self.reopened_tasks.is_empty() {
            lines.push(format!(
                "  Regression detected! Reopened tasks: {}",
                self.reopened_tasks.join(", ")
            ));
        }

        lines.join("\n")
    }
}

#[derive(Serialize)]
pub struct TestRunResults {
    pub results: Vec<TestRunResult>,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
}

impl Output for TestRunResults {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if self.results.is_empty() {
            return "No tests to run.".to_string();
        }

        let mut lines = Vec::new();

        for result in &self.results {
            lines.push(result.to_human());
        }

        lines.push(String::new());
        lines.push(format!(
            "Results: {} passed, {} failed, {} total",
            self.passed, self.failed, self.total
        ));

        lines.join("\n")
    }
}

/// Run a single test and return the result.
fn run_single_test(
    storage: &mut Storage,
    test: &TestNode,
    repo_path: &Path,
) -> Result<TestRunResult> {
    let start = Instant::now();

    // Execute the command
    let working_dir = if test.working_dir == "." {
        repo_path.to_path_buf()
    } else {
        repo_path.join(&test.working_dir)
    };

    let output = Command::new("sh")
        .arg("-c")
        .arg(&test.command)
        .current_dir(&working_dir)
        .output()
        .map_err(|e| Error::Other(format!("Failed to execute command: {}", e)))?;

    let duration_ms = start.elapsed().as_millis() as u64;
    let exit_code = output.status.code().unwrap_or(-1);
    let passed = output.status.success();

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    // Save test result
    let result = TestResult {
        test_id: test.id.clone(),
        passed,
        exit_code,
        stdout: if stdout.is_empty() {
            None
        } else {
            Some(stdout.clone())
        },
        stderr: if stderr.is_empty() {
            None
        } else {
            Some(stderr.clone())
        },
        duration_ms,
        executed_at: Utc::now(),
    };
    storage.save_test_result(&result)?;

    // Handle regression detection
    let mut reopened_tasks = Vec::new();
    if !passed {
        reopened_tasks = storage.reopen_linked_tasks_on_failure(&test.id)?;
    }

    Ok(TestRunResult {
        test_id: test.id.clone(),
        test_name: test.name.clone(),
        passed,
        exit_code,
        duration_ms,
        stdout: if stdout.is_empty() {
            None
        } else {
            Some(stdout)
        },
        stderr: if stderr.is_empty() {
            None
        } else {
            Some(stderr)
        },
        reopened_tasks,
    })
}

/// Run tests based on the provided options.
pub fn test_run(
    repo_path: &Path,
    test_id: Option<&str>,
    task_id: Option<&str>,
    all: bool,
    failed_only: bool,
) -> Result<TestRunResults> {
    let mut storage = Storage::open(repo_path)?;

    // Determine which tests to run
    let tests: Vec<TestNode> = if let Some(id) = test_id {
        // Run specific test
        vec![storage.get_test(id)?]
    } else if let Some(tid) = task_id {
        // Run tests linked to a task
        storage.get_tests_for_task(tid)?
    } else if failed_only {
        // Run only previously failed tests
        storage.get_failed_tests()?
    } else if all {
        // Run all tests
        storage.list_tests(None)?
    } else {
        return Err(Error::Other(
            "Specify --all, --failed, --task, or a test ID".to_string(),
        ));
    };

    if tests.is_empty() {
        return Ok(TestRunResults {
            results: vec![],
            total: 0,
            passed: 0,
            failed: 0,
        });
    }

    let mut results = Vec::new();
    for test in &tests {
        let result = run_single_test(&mut storage, test, repo_path)?;
        results.push(result);
    }

    let total = results.len();
    let passed = results.iter().filter(|r| r.passed).count();
    let failed = total - passed;

    Ok(TestRunResults {
        results,
        total,
        passed,
        failed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> TempDir {
        let temp = TempDir::new().unwrap();
        Storage::init(temp.path()).unwrap();
        temp
    }

    #[test]
    fn test_init_new() {
        let temp = TempDir::new().unwrap();
        let result = init(temp.path()).unwrap();
        assert!(result.initialized);
    }

    #[test]
    fn test_init_existing() {
        let temp = TempDir::new().unwrap();
        Storage::init(temp.path()).unwrap();
        let result = init(temp.path()).unwrap();
        assert!(!result.initialized);
    }

    #[test]
    fn test_task_create() {
        let temp = setup();
        let result = task_create(
            temp.path(),
            "Test task".to_string(),
            None,
            Some(1),
            vec!["test".to_string()],
            None,
        )
        .unwrap();
        assert!(result.id.starts_with("bn-"));
        assert_eq!(result.title, "Test task");
    }

    #[test]
    fn test_task_show() {
        let temp = setup();
        let created =
            task_create(temp.path(), "Test".to_string(), None, None, vec![], None).unwrap();
        let task = task_show(temp.path(), &created.id).unwrap();
        assert_eq!(task.id, created.id);
    }

    #[test]
    fn test_task_list() {
        let temp = setup();
        task_create(
            temp.path(),
            "Task 1".to_string(),
            None,
            Some(1),
            vec![],
            None,
        )
        .unwrap();
        task_create(
            temp.path(),
            "Task 2".to_string(),
            None,
            Some(2),
            vec![],
            None,
        )
        .unwrap();

        let list = task_list(temp.path(), None, None, None).unwrap();
        assert_eq!(list.count, 2);
    }

    #[test]
    fn test_task_update() {
        let temp = setup();
        let created = task_create(
            temp.path(),
            "Original".to_string(),
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        let updated = task_update(
            temp.path(),
            &created.id,
            Some("Updated".to_string()),
            None,
            Some(1),
            None,
            vec![],
            vec![],
            None,
        )
        .unwrap();

        assert!(updated.updated_fields.contains(&"title".to_string()));
        assert!(updated.updated_fields.contains(&"priority".to_string()));

        let task = task_show(temp.path(), &created.id).unwrap();
        assert_eq!(task.title, "Updated");
        assert_eq!(task.priority, 1);
    }

    #[test]
    fn test_task_close_reopen() {
        let temp = setup();
        let created =
            task_create(temp.path(), "Test".to_string(), None, None, vec![], None).unwrap();

        task_close(temp.path(), &created.id, Some("Done".to_string())).unwrap();
        let task = task_show(temp.path(), &created.id).unwrap();
        assert_eq!(task.status, TaskStatus::Done);
        assert!(task.closed_at.is_some());

        task_reopen(temp.path(), &created.id).unwrap();
        let task = task_show(temp.path(), &created.id).unwrap();
        assert_eq!(task.status, TaskStatus::Reopened);
        assert!(task.closed_at.is_none());
    }

    #[test]
    fn test_task_delete() {
        let temp = setup();
        let created =
            task_create(temp.path(), "Test".to_string(), None, None, vec![], None).unwrap();

        task_delete(temp.path(), &created.id).unwrap();
        let list = task_list(temp.path(), None, None, None).unwrap();
        assert_eq!(list.count, 0);
    }

    // === Dependency Command Tests ===

    #[test]
    fn test_dep_add() {
        let temp = setup();
        let task_a =
            task_create(temp.path(), "Task A".to_string(), None, None, vec![], None).unwrap();
        let task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, vec![], None).unwrap();

        let result = dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();
        assert_eq!(result.child, task_b.id);
        assert_eq!(result.parent, task_a.id);

        // Verify task B now depends on A
        let task = task_show(temp.path(), &task_b.id).unwrap();
        assert!(task.depends_on.contains(&task_a.id));
    }

    #[test]
    fn test_dep_add_cycle_rejected() {
        let temp = setup();
        let task_a =
            task_create(temp.path(), "Task A".to_string(), None, None, vec![], None).unwrap();
        let task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, vec![], None).unwrap();

        // A depends on B
        dep_add(temp.path(), &task_a.id, &task_b.id).unwrap();

        // B depends on A should fail (cycle)
        let result = dep_add(temp.path(), &task_b.id, &task_a.id);
        assert!(result.is_err());
    }

    #[test]
    fn test_dep_rm() {
        let temp = setup();
        let task_a =
            task_create(temp.path(), "Task A".to_string(), None, None, vec![], None).unwrap();
        let task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, vec![], None).unwrap();

        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();
        dep_rm(temp.path(), &task_b.id, &task_a.id).unwrap();

        // Verify task B no longer depends on A
        let task = task_show(temp.path(), &task_b.id).unwrap();
        assert!(!task.depends_on.contains(&task_a.id));
    }

    #[test]
    fn test_dep_show() {
        let temp = setup();
        let task_a =
            task_create(temp.path(), "Task A".to_string(), None, None, vec![], None).unwrap();
        let task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, vec![], None).unwrap();
        let task_c =
            task_create(temp.path(), "Task C".to_string(), None, None, vec![], None).unwrap();

        // B depends on A, C depends on B
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();
        dep_add(temp.path(), &task_c.id, &task_b.id).unwrap();

        let graph = dep_show(temp.path(), &task_b.id).unwrap();
        assert_eq!(graph.task_id, task_b.id);
        assert!(graph.depends_on.contains(&task_a.id));
        assert!(graph.dependents.contains(&task_c.id));
    }

    // === Query Command Tests ===

    #[test]
    fn test_ready_command() {
        let temp = setup();
        let task_a =
            task_create(temp.path(), "Task A".to_string(), None, None, vec![], None).unwrap();
        let task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, vec![], None).unwrap();

        // B depends on A (which is pending, so B is blocked)
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        let result = ready(temp.path()).unwrap();
        assert_eq!(result.count, 1);
        assert_eq!(result.tasks[0].id, task_a.id);
    }

    #[test]
    fn test_blocked_command() {
        let temp = setup();
        let task_a =
            task_create(temp.path(), "Task A".to_string(), None, None, vec![], None).unwrap();
        let task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, vec![], None).unwrap();

        // B depends on A (which is pending, so B is blocked)
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        let result = blocked(temp.path()).unwrap();
        assert_eq!(result.count, 1);
        assert_eq!(result.tasks[0].task.id, task_b.id);
        assert!(result.tasks[0].blocking_tasks.contains(&task_a.id));
    }

    #[test]
    fn test_ready_after_dependency_done() {
        let temp = setup();
        let task_a =
            task_create(temp.path(), "Task A".to_string(), None, None, vec![], None).unwrap();
        let task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, vec![], None).unwrap();

        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        // Initially B is blocked
        let blocked_result = blocked(temp.path()).unwrap();
        assert_eq!(blocked_result.count, 1);

        // Close task A
        task_close(temp.path(), &task_a.id, None).unwrap();

        // Now B should be ready
        let ready_result = ready(temp.path()).unwrap();
        assert_eq!(ready_result.count, 1);
        assert_eq!(ready_result.tasks[0].id, task_b.id);

        // And B should not be blocked anymore
        let blocked_result = blocked(temp.path()).unwrap();
        assert_eq!(blocked_result.count, 0);
    }
}
