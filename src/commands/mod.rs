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
    pub agents_md_updated: bool,
}

impl Output for InitResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();
        if self.initialized {
            lines.push(format!("Initialized binnacle at {}", self.storage_path));
        } else {
            lines.push(format!(
                "Binnacle already initialized at {}",
                self.storage_path
            ));
        }
        if self.agents_md_updated {
            lines.push("Updated AGENTS.md with binnacle reference.".to_string());
        }
        lines.join("\n")
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

    // Create or update AGENTS.md with binnacle blurb
    let agents_md_updated = update_agents_md(repo_path)?;

    Ok(InitResult {
        initialized: !already_exists,
        storage_path: storage.root().to_string_lossy().to_string(),
        agents_md_updated,
    })
}

/// The blurb to add to AGENTS.md
const AGENTS_MD_BLURB: &str = r#"# Agent Instructions
This project uses **bn** (binnacle) for long-horizon task/test status tracking. Run `bn orient` to get started!

## Task Workflow (IMPORTANT)
1. **Before starting work**: Run `bn ready` to see available tasks, then `bn task update <id> --status in_progress`
2. **After completing work**: Run `bn task close <id> --reason "brief description"` 
3. **If blocked**: Run `bn task update <id> --status blocked`

The task graph drives development priorities. Always update task status to keep it accurate.

## Before you mark task done (IMPORTANT)
1. Run `bn ready` to check if any related tasks should also be closed
2. Close ALL tasks you completed, not just the one you started with
3. Verify the task graph is accurate before finalizing your work
"#;

/// Update AGENTS.md with the binnacle blurb.
/// Returns true if the file was modified, false if it already contained the reference.
fn update_agents_md(repo_path: &Path) -> Result<bool> {
    use std::fs;
    use std::io::Write;

    let agents_path = repo_path.join("AGENTS.md");

    // Check if file exists and already contains the blurb
    if agents_path.exists() {
        let contents = fs::read_to_string(&agents_path)
            .map_err(|e| Error::Other(format!("Failed to read AGENTS.md: {}", e)))?;

        // Skip if already contains reference to bn orient
        if contents.contains("bn orient") {
            return Ok(false);
        }

        // Append the blurb
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&agents_path)
            .map_err(|e| Error::Other(format!("Failed to open AGENTS.md: {}", e)))?;

        // Add a newline before the blurb if file doesn't end with one
        let prefix = if contents.ends_with('\n') {
            "\n"
        } else {
            "\n\n"
        };
        file.write_all(prefix.as_bytes())
            .map_err(|e| Error::Other(format!("Failed to write to AGENTS.md: {}", e)))?;
        file.write_all(AGENTS_MD_BLURB.as_bytes())
            .map_err(|e| Error::Other(format!("Failed to write to AGENTS.md: {}", e)))?;

        Ok(true)
    } else {
        // Create new AGENTS.md with the blurb
        fs::write(&agents_path, AGENTS_MD_BLURB)
            .map_err(|e| Error::Other(format!("Failed to create AGENTS.md: {}", e)))?;
        Ok(true)
    }
}

// === Orient Command ===

#[derive(Serialize)]
pub struct OrientResult {
    pub initialized: bool,
    pub total_tasks: usize,
    pub ready_count: usize,
    pub ready_ids: Vec<String>,
    pub blocked_count: usize,
    pub in_progress_count: usize,
}

impl Output for OrientResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();

        lines.push("Binnacle - AI agent task tracker".to_string());
        lines.push(String::new());
        lines.push("This project uses binnacle (bn) for issue and test tracking.".to_string());
        lines.push(String::new());
        lines.push("Current State:".to_string());
        lines.push(format!("  Total tasks: {}", self.total_tasks));
        if !self.ready_ids.is_empty() {
            let ids = if self.ready_ids.len() <= 5 {
                self.ready_ids.join(", ")
            } else {
                format!(
                    "{}, ... ({} more)",
                    self.ready_ids[..5].join(", "),
                    self.ready_ids.len() - 5
                )
            };
            lines.push(format!("  Ready: {} ({})", self.ready_count, ids));
        } else {
            lines.push(format!("  Ready: {}", self.ready_count));
        }
        lines.push(format!("  Blocked: {}", self.blocked_count));
        lines.push(format!("  In progress: {}", self.in_progress_count));
        lines.push(String::new());
        lines.push("Key Commands:".to_string());
        lines
            .push("  bn              Status summary (JSON, use -H for human-readable)".to_string());
        lines.push("  bn ready        Show tasks ready to work on".to_string());
        lines.push("  bn task list    List all tasks".to_string());
        lines.push("  bn task show X  Show task details".to_string());
        lines.push("  bn test run     Run linked tests".to_string());
        lines.push(String::new());
        lines.push("Run 'bn --help' for full command reference.".to_string());

        lines.join("\n")
    }
}

/// Orient an AI agent to this project.
/// Auto-initializes binnacle if not already initialized.
pub fn orient(repo_path: &Path) -> Result<OrientResult> {
    // Auto-initialize if needed
    let initialized = if !Storage::exists(repo_path)? {
        Storage::init(repo_path)?;
        update_agents_md(repo_path)?;
        true
    } else {
        false
    };

    // Get current state
    let storage = Storage::open(repo_path)?;
    let tasks = storage.list_tasks(None, None, None)?;

    let mut ready_ids = Vec::new();
    let mut blocked_count = 0;
    let mut in_progress_count = 0;

    for task in &tasks {
        match task.status {
            TaskStatus::InProgress => in_progress_count += 1,
            TaskStatus::Blocked => blocked_count += 1,
            TaskStatus::Pending | TaskStatus::Reopened => {
                // Check if all dependencies are done
                if task.depends_on.is_empty() {
                    ready_ids.push(task.id.clone());
                } else {
                    let all_done = task.depends_on.iter().all(|dep_id| {
                        storage
                            .get_task(dep_id)
                            .map(|t| t.status == TaskStatus::Done)
                            .unwrap_or(false)
                    });
                    if all_done {
                        ready_ids.push(task.id.clone());
                    } else {
                        blocked_count += 1;
                    }
                }
            }
            _ => {}
        }
    }

    let ready_count = ready_ids.len();

    Ok(OrientResult {
        initialized,
        total_tasks: tasks.len(),
        ready_count,
        ready_ids,
        blocked_count,
        in_progress_count,
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

// === Commit Tracking Commands ===

use crate::models::CommitLink;

// === Doctor Command ===

/// A single issue detected by the doctor command.
#[derive(Serialize, Clone)]
pub struct DoctorIssue {
    pub severity: String, // "error", "warning", "info"
    pub category: String, // "orphan", "cycle", "consistency", "storage"
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<String>,
}

/// Result of the doctor command.
#[derive(Serialize)]
pub struct DoctorResult {
    pub healthy: bool,
    pub issues: Vec<DoctorIssue>,
    pub stats: DoctorStats,
}

/// Statistics about the binnacle data.
#[derive(Serialize)]
pub struct DoctorStats {
    pub total_tasks: usize,
    pub total_tests: usize,
    pub total_commits: usize,
    pub storage_path: String,
}

impl Output for DoctorResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();

        if self.healthy {
            lines.push("Health check: OK".to_string());
        } else {
            lines.push(format!(
                "Health check: {} issue(s) found",
                self.issues.len()
            ));
        }

        lines.push(String::new());
        lines.push("Statistics:".to_string());
        lines.push(format!("  Tasks: {}", self.stats.total_tasks));
        lines.push(format!("  Tests: {}", self.stats.total_tests));
        lines.push(format!("  Commit links: {}", self.stats.total_commits));
        lines.push(format!("  Storage: {}", self.stats.storage_path));

        if !self.issues.is_empty() {
            lines.push(String::new());
            lines.push("Issues:".to_string());
            for issue in &self.issues {
                let severity_marker = match issue.severity.as_str() {
                    "error" => "[ERROR]",
                    "warning" => "[WARN]",
                    _ => "[INFO]",
                };
                let entity = issue
                    .entity_id
                    .as_ref()
                    .map(|id| format!(" ({})", id))
                    .unwrap_or_default();
                lines.push(format!(
                    "  {} {}: {}{}",
                    severity_marker, issue.category, issue.message, entity
                ));
            }
        }

        lines.join("\n")
    }
}

/// Run health checks on the binnacle data.
pub fn doctor(repo_path: &Path) -> Result<DoctorResult> {
    let storage = Storage::open(repo_path)?;
    let mut issues = Vec::new();

    // Get all tasks and tests for analysis
    let tasks = storage.list_tasks(None, None, None)?;
    let tests = storage.list_tests(None)?;

    // Check for orphan dependencies (tasks that reference non-existent tasks)
    for task in &tasks {
        for dep_id in &task.depends_on {
            if storage.get_task(dep_id).is_err() {
                issues.push(DoctorIssue {
                    severity: "error".to_string(),
                    category: "orphan".to_string(),
                    message: format!("Task depends on non-existent task {}", dep_id),
                    entity_id: Some(task.id.clone()),
                });
            }
        }
    }

    // Check for orphan test links (tests that reference non-existent tasks)
    for test in &tests {
        for task_id in &test.linked_tasks {
            if storage.get_task(task_id).is_err() {
                issues.push(DoctorIssue {
                    severity: "error".to_string(),
                    category: "orphan".to_string(),
                    message: format!("Test linked to non-existent task {}", task_id),
                    entity_id: Some(test.id.clone()),
                });
            }
        }
    }

    // Check for inconsistent task states
    for task in &tasks {
        // Check for done tasks with pending dependencies
        if task.status == TaskStatus::Done {
            for dep_id in &task.depends_on {
                if let Ok(dep_task) = storage.get_task(dep_id) {
                    if dep_task.status != TaskStatus::Done
                        && dep_task.status != TaskStatus::Cancelled
                    {
                        issues.push(DoctorIssue {
                            severity: "warning".to_string(),
                            category: "consistency".to_string(),
                            message: format!(
                                "Task is done but depends on incomplete task {}",
                                dep_id
                            ),
                            entity_id: Some(task.id.clone()),
                        });
                    }
                }
            }
        }

        // Check for closed tasks without closed_at timestamp
        if task.status == TaskStatus::Done && task.closed_at.is_none() {
            issues.push(DoctorIssue {
                severity: "info".to_string(),
                category: "consistency".to_string(),
                message: "Task is done but has no closed_at timestamp".to_string(),
                entity_id: Some(task.id.clone()),
            });
        }
    }

    // Get commit count
    let commit_count = storage.count_commit_links()?;

    let stats = DoctorStats {
        total_tasks: tasks.len(),
        total_tests: tests.len(),
        total_commits: commit_count,
        storage_path: storage.root().to_string_lossy().to_string(),
    };

    Ok(DoctorResult {
        healthy: issues.is_empty(),
        issues,
        stats,
    })
}

// === Log Command ===

/// A log entry representing a change.
#[derive(Serialize, Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub entity_type: String,
    pub entity_id: String,
    pub action: String, // "created", "updated", "closed", etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

/// Result of the log command.
#[derive(Serialize)]
pub struct LogResult {
    pub entries: Vec<LogEntry>,
    pub count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filtered_by: Option<String>,
}

impl Output for LogResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if self.entries.is_empty() {
            return "No log entries found.".to_string();
        }

        let mut lines = Vec::new();

        if let Some(ref filter) = self.filtered_by {
            lines.push(format!("Log entries for {}:\n", filter));
        } else {
            lines.push(format!("{} log entries:\n", self.count));
        }

        for entry in &self.entries {
            let details = entry
                .details
                .as_ref()
                .map(|d| format!(" - {}", d))
                .unwrap_or_default();
            lines.push(format!(
                "  {} [{}] {} {}{}",
                entry.timestamp, entry.entity_type, entry.entity_id, entry.action, details
            ));
        }

        lines.join("\n")
    }
}

/// Get the audit log of changes.
pub fn log(repo_path: &Path, task_id: Option<&str>) -> Result<LogResult> {
    let storage = Storage::open(repo_path)?;
    let entries = storage.get_log_entries(task_id)?;
    let count = entries.len();

    Ok(LogResult {
        entries,
        count,
        filtered_by: task_id.map(|s| s.to_string()),
    })
}

// === Config Commands ===

/// Result of config get command.
#[derive(Serialize)]
pub struct ConfigValue {
    pub key: String,
    pub value: Option<String>,
}

impl Output for ConfigValue {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        match &self.value {
            Some(v) => format!("{} = {}", self.key, v),
            None => format!("{} is not set", self.key),
        }
    }
}

/// Get a configuration value.
pub fn config_get(repo_path: &Path, key: &str) -> Result<ConfigValue> {
    let storage = Storage::open(repo_path)?;
    let value = storage.get_config(key)?;

    Ok(ConfigValue {
        key: key.to_string(),
        value,
    })
}

/// Result of config set command.
#[derive(Serialize)]
pub struct ConfigSet {
    pub key: String,
    pub value: String,
}

impl Output for ConfigSet {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Set {} = {}", self.key, self.value)
    }
}

/// Set a configuration value.
pub fn config_set(repo_path: &Path, key: &str, value: &str) -> Result<ConfigSet> {
    let mut storage = Storage::open(repo_path)?;
    storage.set_config(key, value)?;

    Ok(ConfigSet {
        key: key.to_string(),
        value: value.to_string(),
    })
}

/// Result of config list command.
#[derive(Serialize)]
pub struct ConfigList {
    pub configs: Vec<(String, String)>,
    pub count: usize,
}

impl Output for ConfigList {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if self.configs.is_empty() {
            return "No configuration values set.".to_string();
        }

        let mut lines = Vec::new();
        lines.push(format!("{} configuration value(s):\n", self.count));

        for (key, value) in &self.configs {
            lines.push(format!("  {} = {}", key, value));
        }

        lines.join("\n")
    }
}

/// List all configuration values.
pub fn config_list(repo_path: &Path) -> Result<ConfigList> {
    let storage = Storage::open(repo_path)?;
    let configs = storage.list_configs()?;
    let count = configs.len();

    Ok(ConfigList { configs, count })
}

// === Compact Command ===

/// Result of the compact command.
#[derive(Serialize)]
pub struct CompactResult {
    pub tasks_compacted: usize,
    pub original_entries: usize,
    pub final_entries: usize,
    pub space_saved_bytes: usize,
}

impl Output for CompactResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();
        lines.push("Compact complete.".to_string());
        lines.push(format!("  Tasks compacted: {}", self.tasks_compacted));
        lines.push(format!(
            "  Entries: {} -> {}",
            self.original_entries, self.final_entries
        ));
        if self.space_saved_bytes > 0 {
            lines.push(format!("  Space saved: {} bytes", self.space_saved_bytes));
        }
        lines.join("\n")
    }
}

/// Compact the storage by removing duplicate entries and summarizing old closed tasks.
pub fn compact(repo_path: &Path) -> Result<CompactResult> {
    let mut storage = Storage::open(repo_path)?;
    let result = storage.compact()?;

    Ok(result)
}

#[derive(Serialize)]
pub struct CommitLinked {
    pub sha: String,
    pub task_id: String,
}

impl Output for CommitLinked {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Linked commit {} to task {}", self.sha, self.task_id)
    }
}

/// Link a commit to a task.
pub fn commit_link(repo_path: &Path, sha: &str, task_id: &str) -> Result<CommitLinked> {
    let mut storage = Storage::open(repo_path)?;
    storage.link_commit(sha, task_id)?;

    Ok(CommitLinked {
        sha: sha.to_string(),
        task_id: task_id.to_string(),
    })
}

#[derive(Serialize)]
pub struct CommitUnlinked {
    pub sha: String,
    pub task_id: String,
}

impl Output for CommitUnlinked {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Unlinked commit {} from task {}", self.sha, self.task_id)
    }
}

/// Unlink a commit from a task.
pub fn commit_unlink(repo_path: &Path, sha: &str, task_id: &str) -> Result<CommitUnlinked> {
    let mut storage = Storage::open(repo_path)?;
    storage.unlink_commit(sha, task_id)?;

    Ok(CommitUnlinked {
        sha: sha.to_string(),
        task_id: task_id.to_string(),
    })
}

#[derive(Serialize)]
pub struct CommitList {
    pub task_id: String,
    pub commits: Vec<CommitLink>,
    pub count: usize,
}

impl Output for CommitList {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if self.commits.is_empty() {
            return format!("No commits linked to task {}", self.task_id);
        }

        let mut lines = Vec::new();
        lines.push(format!(
            "{} commit(s) linked to task {}:\n",
            self.count, self.task_id
        ));

        for link in &self.commits {
            lines.push(format!(
                "  {} (linked {})",
                link.sha,
                link.linked_at.format("%Y-%m-%d %H:%M")
            ));
        }

        lines.join("\n")
    }
}

/// List commits linked to a task.
pub fn commit_list(repo_path: &Path, task_id: &str) -> Result<CommitList> {
    let storage = Storage::open(repo_path)?;
    let commits = storage.get_commits_for_task(task_id)?;
    let count = commits.len();

    Ok(CommitList {
        task_id: task_id.to_string(),
        commits,
        count,
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

    // === Commit Command Tests ===

    #[test]
    fn test_commit_link() {
        let temp = setup();
        let task =
            task_create(temp.path(), "Task A".to_string(), None, None, vec![], None).unwrap();

        let result = commit_link(temp.path(), "a1b2c3d", &task.id).unwrap();
        assert_eq!(result.sha, "a1b2c3d");
        assert_eq!(result.task_id, task.id);
    }

    #[test]
    fn test_commit_link_invalid_sha() {
        let temp = setup();
        let task =
            task_create(temp.path(), "Task A".to_string(), None, None, vec![], None).unwrap();

        // SHA too short
        let result = commit_link(temp.path(), "abc", &task.id);
        assert!(result.is_err());
    }

    #[test]
    fn test_commit_link_nonexistent_task() {
        let temp = setup();

        let result = commit_link(temp.path(), "a1b2c3d", "bn-9999");
        assert!(result.is_err());
    }

    #[test]
    fn test_commit_unlink() {
        let temp = setup();
        let task =
            task_create(temp.path(), "Task A".to_string(), None, None, vec![], None).unwrap();

        commit_link(temp.path(), "a1b2c3d", &task.id).unwrap();
        let result = commit_unlink(temp.path(), "a1b2c3d", &task.id).unwrap();

        assert_eq!(result.sha, "a1b2c3d");
        assert_eq!(result.task_id, task.id);

        // Verify commit is no longer linked
        let list = commit_list(temp.path(), &task.id).unwrap();
        assert_eq!(list.count, 0);
    }

    #[test]
    fn test_commit_unlink_nonexistent() {
        let temp = setup();
        let task =
            task_create(temp.path(), "Task A".to_string(), None, None, vec![], None).unwrap();

        let result = commit_unlink(temp.path(), "a1b2c3d", &task.id);
        assert!(result.is_err());
    }

    #[test]
    fn test_commit_list() {
        let temp = setup();
        let task =
            task_create(temp.path(), "Task A".to_string(), None, None, vec![], None).unwrap();

        commit_link(temp.path(), "a1b2c3d", &task.id).unwrap();
        commit_link(temp.path(), "e5f6789", &task.id).unwrap();

        let list = commit_list(temp.path(), &task.id).unwrap();
        assert_eq!(list.count, 2);
        assert_eq!(list.task_id, task.id);
    }

    #[test]
    fn test_commit_list_empty() {
        let temp = setup();
        let task =
            task_create(temp.path(), "Task A".to_string(), None, None, vec![], None).unwrap();

        let list = commit_list(temp.path(), &task.id).unwrap();
        assert_eq!(list.count, 0);
    }

    #[test]
    fn test_commit_list_nonexistent_task() {
        let temp = setup();

        let result = commit_list(temp.path(), "bn-9999");
        assert!(result.is_err());
    }

    // === Doctor Command Tests ===

    #[test]
    fn test_doctor_healthy() {
        let temp = setup();
        task_create(temp.path(), "Task A".to_string(), None, None, vec![], None).unwrap();

        let result = doctor(temp.path()).unwrap();
        assert!(result.healthy);
        assert!(result.issues.is_empty());
        assert_eq!(result.stats.total_tasks, 1);
    }

    #[test]
    fn test_doctor_consistency_done_task_with_pending_dep() {
        let temp = setup();

        // Create two tasks: A depends on B
        let task_a =
            task_create(temp.path(), "Task A".to_string(), None, None, vec![], None).unwrap();
        let task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, vec![], None).unwrap();

        // B depends on A
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        // Close B (which depends on A, but A is still pending) - this creates inconsistency
        task_close(temp.path(), &task_b.id, None).unwrap();

        let result = doctor(temp.path()).unwrap();
        // We should find the consistency warning (done task with pending dependency)
        assert!(!result.healthy);
        assert!(!result.issues.is_empty());
        assert!(result.issues.iter().any(|i| i.category == "consistency"));
    }

    #[test]
    fn test_doctor_stats() {
        let temp = setup();
        task_create(temp.path(), "Task 1".to_string(), None, None, vec![], None).unwrap();
        task_create(temp.path(), "Task 2".to_string(), None, None, vec![], None).unwrap();
        test_create(
            temp.path(),
            "Test 1".to_string(),
            "echo test".to_string(),
            ".".to_string(),
            None,
        )
        .unwrap();

        let result = doctor(temp.path()).unwrap();
        assert_eq!(result.stats.total_tasks, 2);
        assert_eq!(result.stats.total_tests, 1);
    }

    // === Log Command Tests ===

    #[test]
    fn test_log_basic() {
        let temp = setup();
        let task =
            task_create(temp.path(), "Task A".to_string(), None, None, vec![], None).unwrap();

        let result = log(temp.path(), None).unwrap();
        assert!(result.count >= 1);
        assert!(result.entries.iter().any(|e| e.entity_id == task.id));
    }

    #[test]
    fn test_log_filter_by_task() {
        let temp = setup();
        let task_a =
            task_create(temp.path(), "Task A".to_string(), None, None, vec![], None).unwrap();
        let _task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, vec![], None).unwrap();

        let result = log(temp.path(), Some(&task_a.id)).unwrap();
        assert!(result.entries.iter().all(|e| e.entity_id == task_a.id));
        assert_eq!(result.filtered_by, Some(task_a.id.clone()));
    }

    #[test]
    fn test_log_includes_updates() {
        let temp = setup();
        let task =
            task_create(temp.path(), "Task A".to_string(), None, None, vec![], None).unwrap();

        // Update the task
        task_update(
            temp.path(),
            &task.id,
            Some("Updated Title".to_string()),
            None,
            None,
            None,
            vec![],
            vec![],
            None,
        )
        .unwrap();

        let result = log(temp.path(), Some(&task.id)).unwrap();
        // Should have at least 2 entries: created and updated
        assert!(result.count >= 2);
        assert!(result.entries.iter().any(|e| e.action == "created"));
        assert!(result.entries.iter().any(|e| e.action == "updated"));
    }

    #[test]
    fn test_log_includes_close() {
        let temp = setup();
        let task =
            task_create(temp.path(), "Task A".to_string(), None, None, vec![], None).unwrap();

        task_close(temp.path(), &task.id, Some("Complete".to_string())).unwrap();

        let result = log(temp.path(), Some(&task.id)).unwrap();
        assert!(result.entries.iter().any(|e| e.action == "closed"));
    }

    // === Config Command Tests ===

    #[test]
    fn test_config_set_and_get() {
        let temp = setup();

        config_set(temp.path(), "test.key", "test_value").unwrap();
        let result = config_get(temp.path(), "test.key").unwrap();

        assert_eq!(result.key, "test.key");
        assert_eq!(result.value, Some("test_value".to_string()));
    }

    #[test]
    fn test_config_get_nonexistent() {
        let temp = setup();

        let result = config_get(temp.path(), "nonexistent.key").unwrap();
        assert_eq!(result.key, "nonexistent.key");
        assert_eq!(result.value, None);
    }

    #[test]
    fn test_config_list() {
        let temp = setup();

        config_set(temp.path(), "key1", "value1").unwrap();
        config_set(temp.path(), "key2", "value2").unwrap();

        let result = config_list(temp.path()).unwrap();
        assert_eq!(result.count, 2);
        assert!(result
            .configs
            .iter()
            .any(|(k, v)| k == "key1" && v == "value1"));
        assert!(result
            .configs
            .iter()
            .any(|(k, v)| k == "key2" && v == "value2"));
    }

    #[test]
    fn test_config_list_empty() {
        let temp = setup();

        let result = config_list(temp.path()).unwrap();
        assert_eq!(result.count, 0);
    }

    #[test]
    fn test_config_overwrite() {
        let temp = setup();

        config_set(temp.path(), "key", "value1").unwrap();
        config_set(temp.path(), "key", "value2").unwrap();

        let result = config_get(temp.path(), "key").unwrap();
        assert_eq!(result.value, Some("value2".to_string()));
    }

    // === Compact Command Tests ===

    #[test]
    fn test_compact_basic() {
        let temp = setup();

        // Create a task and update it multiple times
        let task =
            task_create(temp.path(), "Task A".to_string(), None, None, vec![], None).unwrap();
        task_update(
            temp.path(),
            &task.id,
            Some("Updated 1".to_string()),
            None,
            None,
            None,
            vec![],
            vec![],
            None,
        )
        .unwrap();
        task_update(
            temp.path(),
            &task.id,
            Some("Updated 2".to_string()),
            None,
            None,
            None,
            vec![],
            vec![],
            None,
        )
        .unwrap();

        let result = compact(temp.path()).unwrap();

        // Should have compacted 3 entries (create + 2 updates) to 1
        assert!(result.original_entries >= 3);
        assert_eq!(result.final_entries, 1);
        assert_eq!(result.tasks_compacted, 1);

        // Verify the task still exists with the final title
        let task = task_show(temp.path(), &task.id).unwrap();
        assert_eq!(task.title, "Updated 2");
    }

    #[test]
    fn test_compact_preserves_all_tasks() {
        let temp = setup();

        task_create(temp.path(), "Task A".to_string(), None, None, vec![], None).unwrap();
        task_create(temp.path(), "Task B".to_string(), None, None, vec![], None).unwrap();
        task_create(temp.path(), "Task C".to_string(), None, None, vec![], None).unwrap();

        let result = compact(temp.path()).unwrap();
        assert_eq!(result.tasks_compacted, 3);
        assert_eq!(result.final_entries, 3);

        // Verify all tasks still exist
        let tasks = task_list(temp.path(), None, None, None).unwrap();
        assert_eq!(tasks.count, 3);
    }

    #[test]
    fn test_compact_with_tests() {
        let temp = setup();

        let task =
            task_create(temp.path(), "Task A".to_string(), None, None, vec![], None).unwrap();
        test_create(
            temp.path(),
            "Test 1".to_string(),
            "echo test".to_string(),
            ".".to_string(),
            Some(task.id.clone()),
        )
        .unwrap();

        let result = compact(temp.path()).unwrap();
        assert_eq!(result.final_entries, 2); // 1 task + 1 test

        // Verify both still exist
        let tasks = task_list(temp.path(), None, None, None).unwrap();
        assert_eq!(tasks.count, 1);
        let tests = test_list(temp.path(), None).unwrap();
        assert_eq!(tests.count, 1);
    }

    // === Init AGENTS.md Tests ===

    #[test]
    fn test_init_creates_agents_md() {
        let temp = TempDir::new().unwrap();
        let agents_path = temp.path().join("AGENTS.md");

        // Verify AGENTS.md doesn't exist yet
        assert!(!agents_path.exists());

        // Run init
        let result = init(temp.path()).unwrap();
        assert!(result.initialized);
        assert!(result.agents_md_updated);

        // Verify AGENTS.md was created
        assert!(agents_path.exists());
        let contents = std::fs::read_to_string(&agents_path).unwrap();
        assert!(contents.contains("bn orient"));
        assert!(contents.contains("binnacle"));
    }

    #[test]
    fn test_init_appends_to_existing_agents_md() {
        let temp = TempDir::new().unwrap();
        let agents_path = temp.path().join("AGENTS.md");

        // Create existing AGENTS.md
        std::fs::write(&agents_path, "# My Existing Agents\n\nSome content here.\n").unwrap();

        // Run init
        let result = init(temp.path()).unwrap();
        assert!(result.initialized);
        assert!(result.agents_md_updated);

        // Verify content was appended
        let contents = std::fs::read_to_string(&agents_path).unwrap();
        assert!(contents.contains("My Existing Agents"));
        assert!(contents.contains("bn orient"));
    }

    #[test]
    fn test_init_skips_agents_md_if_already_has_bn_orient() {
        let temp = TempDir::new().unwrap();
        let agents_path = temp.path().join("AGENTS.md");

        // Create existing AGENTS.md that already references bn orient
        std::fs::write(
            &agents_path,
            "# Agents\n\nRun `bn orient` to get started.\n",
        )
        .unwrap();

        // Run init
        let result = init(temp.path()).unwrap();
        assert!(result.initialized);
        assert!(!result.agents_md_updated); // Should NOT be updated

        // Verify content wasn't duplicated
        let contents = std::fs::read_to_string(&agents_path).unwrap();
        assert_eq!(contents.matches("bn orient").count(), 1);
    }

    #[test]
    fn test_init_idempotent_agents_md() {
        let temp = TempDir::new().unwrap();

        // Run init twice
        init(temp.path()).unwrap();
        let result = init(temp.path()).unwrap();

        // Second run should not update AGENTS.md (already has bn orient)
        assert!(!result.initialized); // binnacle already exists
        assert!(!result.agents_md_updated); // AGENTS.md already has bn orient
    }

    // === Orient Command Tests ===

    #[test]
    fn test_orient_auto_initializes() {
        let temp = TempDir::new().unwrap();

        // Verify not initialized
        assert!(!Storage::exists(temp.path()).unwrap());

        // Run orient
        let result = orient(temp.path()).unwrap();
        assert!(result.initialized);

        // Verify now initialized
        assert!(Storage::exists(temp.path()).unwrap());

        // Verify AGENTS.md was created
        let agents_path = temp.path().join("AGENTS.md");
        assert!(agents_path.exists());
    }

    #[test]
    fn test_orient_shows_task_counts() {
        let temp = setup();

        // Create some tasks
        task_create(temp.path(), "Task A".to_string(), None, None, vec![], None).unwrap();
        task_create(temp.path(), "Task B".to_string(), None, None, vec![], None).unwrap();

        let result = orient(temp.path()).unwrap();
        assert!(!result.initialized); // Already initialized in setup()
        assert_eq!(result.total_tasks, 2);
        assert_eq!(result.ready_count, 2); // Both pending tasks are ready
    }

    #[test]
    fn test_orient_shows_blocked_tasks() {
        let temp = setup();

        let task_a =
            task_create(temp.path(), "Task A".to_string(), None, None, vec![], None).unwrap();
        let task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, vec![], None).unwrap();

        // B depends on A (so B is blocked)
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        let result = orient(temp.path()).unwrap();
        assert_eq!(result.total_tasks, 2);
        assert_eq!(result.ready_count, 1);
        assert!(result.ready_ids.contains(&task_a.id));
        assert_eq!(result.blocked_count, 1);
    }

    #[test]
    fn test_orient_shows_in_progress_tasks() {
        let temp = setup();

        let task =
            task_create(temp.path(), "Task A".to_string(), None, None, vec![], None).unwrap();

        // Update to in_progress
        task_update(
            temp.path(),
            &task.id,
            None,
            None,
            None,
            Some("in_progress"),
            vec![],
            vec![],
            None,
        )
        .unwrap();

        let result = orient(temp.path()).unwrap();
        assert_eq!(result.in_progress_count, 1);
    }

    #[test]
    fn test_orient_human_output() {
        let temp = setup();
        task_create(temp.path(), "Task A".to_string(), None, None, vec![], None).unwrap();

        let result = orient(temp.path()).unwrap();
        let human = result.to_human();

        assert!(human.contains("Binnacle - AI agent task tracker"));
        assert!(human.contains("Total tasks: 1"));
        assert!(human.contains("bn ready"));
        assert!(human.contains("bn task list"));
    }
}
