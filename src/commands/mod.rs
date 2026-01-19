//! Command implementations for Binnacle CLI.
//!
//! This module contains the business logic for each CLI command.
//! Commands are organized by entity type:
//! - `init` - Initialize binnacle for a repository
//! - `task` - Task CRUD operations
//! - `dep` - Dependency management
//! - `test` - Test node operations
//! - `commit` - Commit tracking

use crate::models::{Task, TaskStatus};
use crate::storage::{generate_id, parse_status, Storage};
use crate::{Error, Result};
use chrono::Utc;
use serde::Serialize;
use std::path::Path;

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
            format!(
                "Binnacle already initialized at {}",
                self.storage_path
            )
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
        lines.push(format!("  Created: {}", self.created_at.format("%Y-%m-%d %H:%M")));
        lines.push(format!("  Updated: {}", self.updated_at.format("%Y-%m-%d %H:%M")));
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
        let created = task_create(temp.path(), "Test".to_string(), None, None, vec![], None).unwrap();
        let task = task_show(temp.path(), &created.id).unwrap();
        assert_eq!(task.id, created.id);
    }

    #[test]
    fn test_task_list() {
        let temp = setup();
        task_create(temp.path(), "Task 1".to_string(), None, Some(1), vec![], None).unwrap();
        task_create(temp.path(), "Task 2".to_string(), None, Some(2), vec![], None).unwrap();

        let list = task_list(temp.path(), None, None, None).unwrap();
        assert_eq!(list.count, 2);
    }

    #[test]
    fn test_task_update() {
        let temp = setup();
        let created = task_create(temp.path(), "Original".to_string(), None, None, vec![], None).unwrap();

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
        let created = task_create(temp.path(), "Test".to_string(), None, None, vec![], None).unwrap();

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
        let created = task_create(temp.path(), "Test".to_string(), None, None, vec![], None).unwrap();

        task_delete(temp.path(), &created.id).unwrap();
        let list = task_list(temp.path(), None, None, None).unwrap();
        assert_eq!(list.count, 0);
    }
}
