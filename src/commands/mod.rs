//! Command implementations for Binnacle CLI.
//!
//! This module contains the business logic for each CLI command.
//! Commands are organized by entity type:
//! - `init` - Initialize binnacle for a repository
//! - `task` - Task CRUD operations
//! - `dep` - Dependency management
//! - `test` - Test node operations
//! - `commit` - Commit tracking

use crate::models::{Bug, BugSeverity, Task, TaskStatus, TestNode, TestResult};
use crate::storage::{generate_id, parse_status, Storage};
use crate::{Error, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
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

/// Prompt the user for a yes/no answer.
/// Returns true for yes, false for no.
/// Default is Yes if default_yes is true, No otherwise.
fn prompt_yes_no(prompt: &str, default_yes: bool) -> bool {
    use std::io::{self, Write};

    let suffix = if default_yes { " (Y/n): " } else { " (y/N): " };
    print!("{}{}", prompt, suffix);
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();

    let trimmed = input.trim().to_lowercase();

    if trimmed.is_empty() {
        default_yes
    } else {
        trimmed == "y" || trimmed == "yes"
    }
}

#[derive(Serialize)]
pub struct InitResult {
    pub initialized: bool,
    pub storage_path: String,
    pub agents_md_updated: bool,
    pub skills_file_created: bool,
    pub codex_skills_file_created: bool,
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
        if self.skills_file_created {
            lines.push(
                "Created Claude Code skills file at ~/.claude/skills/binnacle/SKILL.md".to_string(),
            );
        }
        if self.codex_skills_file_created {
            lines
                .push("Created Codex skills file at ~/.codex/skills/binnacle/SKILL.md".to_string());
        }
        lines.join("\n")
    }
}

/// Initialize binnacle for the current repository with interactive prompts.
pub fn init(repo_path: &Path) -> Result<InitResult> {
    // Prompt for AGENTS.md update (default Yes)
    let update_agents_md = prompt_yes_no("Add binnacle reference to AGENTS.md?", true);

    // Prompt for Claude Code skills file creation (default Yes)
    let create_claude_skills = prompt_yes_no(
        "Create Claude Code skills file at ~/.claude/skills/binnacle/SKILL.md?",
        true,
    );

    // Prompt for Codex skills file creation (default Yes)
    let create_codex_skills = prompt_yes_no(
        "Create Codex skills file at ~/.codex/skills/binnacle/SKILL.md?",
        true,
    );

    init_with_options(
        repo_path,
        update_agents_md,
        create_claude_skills,
        create_codex_skills,
    )
}

/// Initialize binnacle for the current repository with explicit options.
/// Used internally and by tests.
fn init_with_options(
    repo_path: &Path,
    update_agents: bool,
    create_claude_skills: bool,
    create_codex_skills: bool,
) -> Result<InitResult> {
    let already_exists = Storage::exists(repo_path)?;
    let storage = if already_exists {
        Storage::open(repo_path)?
    } else {
        Storage::init(repo_path)?
    };

    // Update AGENTS.md if requested (idempotent: create/replace as needed)
    let agents_md_updated = if update_agents {
        update_agents_md(repo_path)?
    } else {
        false
    };

    // Create Claude Code skills file if requested
    let skills_file_created = if create_claude_skills {
        create_claude_skills_file()?
    } else {
        false
    };

    // Create Codex skills file if requested
    let codex_skills_file_created = if create_codex_skills {
        create_codex_skills_file()?
    } else {
        false
    };

    Ok(InitResult {
        initialized: !already_exists,
        storage_path: storage.root().to_string_lossy().to_string(),
        agents_md_updated,
        skills_file_created,
        codex_skills_file_created,
    })
}

/// The blurb to add to AGENTS.md
const AGENTS_MD_BLURB: &str = r#"<!-- BEGIN BINNACLE SECTION -->
# Agent Instructions

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
<!-- END BINNACLE SECTION -->
"#;

/// The skills file content for Claude Code
const SKILLS_FILE_CONTENT: &str = r#"---
name: binnacle
description: Project planning supercharged. Use to determine tasks at the start of each session. Task and test tracker for multi-session development work with AI agents
---

# Binnacle - Project Planning and Task Management

Use `bn` (binnacle) for managing tasks, tests, and project planning across multiple AI agent sessions.

## When to Use Binnacle

- Multi-session projects that span multiple conversations
- Complex tasks with dependencies and blockers
- Projects requiring persistent test tracking
- When you need to maintain context across conversation compactions

## Key Commands

### Getting Oriented
- `bn orient` - Get project overview and current state
- `bn ready` - Show tasks ready to work on (no blockers)
- `bn blocked` - Show tasks waiting on dependencies

### Task Management
- `bn task create "Title" -p 2 --tag feature` - Create a new task
- `bn task list` - List all tasks
- `bn task show <id>` - Show task details
- `bn task update <id> --status in_progress` - Update task status
- `bn task close <id> --reason "completed"` - Close a task
- `bn task update <id> --title "New title"` - Update task details

### Dependencies
- `bn dep add <child-id> <parent-id>` - Add dependency (child depends on parent)
- `bn dep show <id>` - Show dependency graph
- `bn dep rm <child-id> <parent-id>` - Remove dependency

### Test Tracking
- `bn test create "Name" --cmd "cargo test" --task <id>` - Create and link test
- `bn test run --all` - Run all tests
- `bn test run --task <id>` - Run tests for a specific task
- `bn test list` - List all tests

### Project Health
- `bn doctor` - Check for issues in task graph
- `bn log` - Show audit trail of changes
- `bn log <task-id>` - Show changes for specific task

## Task Workflow

1. **Start of session**: Run `bn orient` to understand project state
2. **Before starting work**:
   - Run `bn ready` to see available tasks
   - Select a task and mark it: `bn task update <id> --status in_progress`
3. **During work**:
   - Create new tasks as you discover them
   - Link commits: `bn commit link <sha> <task-id>`
   - If blocked: `bn task update <id> --status blocked`
4. **After completing work**:
   - Run `bn ready` to check related tasks
   - Close ALL completed tasks: `bn task close <id> --reason "description"`
   - Run tests: `bn test run --all`

## Best Practices

- **Always update task status** - Keep the task graph accurate
- **Close all related tasks** - Don't leave completed work marked as pending
- **Use dependencies** - Model blockers explicitly with `bn dep add`
- **Link tests to tasks** - Enables regression detection
- **Run `bn doctor` regularly** - Catch inconsistencies early
- **Tag tasks** - Use tags for categorization (feature, bug, refactor, etc.)

## Priority Levels

- `0` - Critical
- `1` - High
- `2` - Medium (default)
- `3` - Low
- `4` - Nice to have

## Example Workflow

```bash
# Start of session
bn orient

# See what's ready
bn ready

# Start working on a task
bn task update bn-a1b2 --status in_progress

# Discover a blocker, create it
bn task create "Fix authentication bug" -p 0 --tag bug
bn dep add bn-a1b2 bn-c3d4  # bn-a1b2 depends on bn-c3d4

# Work on the blocker instead
bn task update bn-c3d4 --status in_progress

# Complete blocker
bn task close bn-c3d4 --reason "Fixed auth validation"

# Now original task is unblocked
bn ready  # Should show bn-a1b2 is ready

# Complete original task
bn task close bn-a1b2 --reason "Implemented feature X"

# Run tests
bn test run --all
```

## Notes

- Binnacle stores data in `.bn/` directory using git's orphan branch backend
- All changes are tracked in an append-only log
- Use `bn compact` to summarize old closed tasks
- Run `bn --help` for full command reference
"#;

/// Create the Claude Code skills file for binnacle.
/// Always overwrites if the file already exists.
/// Returns true if the file was created/updated.
fn create_claude_skills_file() -> Result<bool> {
    use std::fs;

    // Get home directory
    let home_dir = dirs::home_dir()
        .ok_or_else(|| Error::Other("Could not determine home directory".to_string()))?;

    let skills_dir = home_dir.join(".claude").join("skills").join("binnacle");
    let skills_path = skills_dir.join("SKILL.md");

    // Create directory if it doesn't exist
    fs::create_dir_all(&skills_dir)
        .map_err(|e| Error::Other(format!("Failed to create Claude skills directory: {}", e)))?;

    // Write the skills file (overwrites if exists)
    fs::write(&skills_path, SKILLS_FILE_CONTENT)
        .map_err(|e| Error::Other(format!("Failed to create Claude skills file: {}", e)))?;

    Ok(true)
}

/// Create the Codex skills file for binnacle.
/// Always overwrites if the file already exists.
/// Returns true if the file was created/updated.
fn create_codex_skills_file() -> Result<bool> {
    use std::fs;

    // Get home directory
    let home_dir = dirs::home_dir()
        .ok_or_else(|| Error::Other("Could not determine home directory".to_string()))?;

    let skills_dir = home_dir.join(".codex").join("skills").join("binnacle");
    let skills_path = skills_dir.join("SKILL.md");

    // Create directory if it doesn't exist
    fs::create_dir_all(&skills_dir)
        .map_err(|e| Error::Other(format!("Failed to create Codex skills directory: {}", e)))?;

    // Write the skills file (overwrites if exists)
    fs::write(&skills_path, SKILLS_FILE_CONTENT)
        .map_err(|e| Error::Other(format!("Failed to create Codex skills file: {}", e)))?;

    Ok(true)
}

/// Marker used to detect the start of the binnacle section.
const BINNACLE_SECTION_START: &str = "<!-- BEGIN BINNACLE SECTION -->";
/// Marker used to detect the end of the binnacle section.
const BINNACLE_SECTION_END: &str = "<!-- END BINNACLE SECTION -->";

/// Replace the binnacle section in the given content with the new blurb.
/// Returns the new content with the section replaced.
/// Assumes the content contains both BEGIN and END markers.
fn replace_binnacle_section(content: &str) -> Result<String> {
    let start_idx = content
        .find(BINNACLE_SECTION_START)
        .ok_or_else(|| Error::Other("BEGIN BINNACLE SECTION marker not found".to_string()))?;

    // Find end marker - must search AFTER start to handle nested content
    let search_start = start_idx + BINNACLE_SECTION_START.len();
    let end_marker_relative = content[search_start..]
        .find(BINNACLE_SECTION_END)
        .ok_or_else(|| Error::Other("END BINNACLE SECTION marker not found".to_string()))?;
    let end_idx = search_start + end_marker_relative + BINNACLE_SECTION_END.len();

    // Build new content: before + new blurb + after
    let before = &content[..start_idx];
    let after = &content[end_idx..];

    Ok(format!("{}{}{}", before, AGENTS_MD_BLURB.trim_end(), after))
}

/// Update AGENTS.md with the binnacle blurb.
/// - If file doesn't exist: create with binnacle section
/// - If file exists with markers: replace section between markers (only if different)
/// - If file exists without markers: append binnacle section
/// Returns true if the file was actually modified.
fn update_agents_md(repo_path: &Path) -> Result<bool> {
    use std::fs;
    use std::io::Write;

    let agents_path = repo_path.join("AGENTS.md");

    if agents_path.exists() {
        let contents = fs::read_to_string(&agents_path)
            .map_err(|e| Error::Other(format!("Failed to read AGENTS.md: {}", e)))?;

        // Check if file has binnacle section markers
        let has_start = contents.contains(BINNACLE_SECTION_START);
        let has_end = contents.contains(BINNACLE_SECTION_END);

        // Warn about malformed markers (one without the other)
        if has_start != has_end {
            eprintln!(
                "Warning: AGENTS.md has {} but not {}. Appending fresh section.",
                if has_start { "BEGIN marker" } else { "END marker" },
                if has_start { "END marker" } else { "BEGIN marker" }
            );
        }

        if has_start && has_end {
            // Replace existing section
            let new_contents = replace_binnacle_section(&contents)?;
            // Only write if content actually changed
            if new_contents != contents {
                fs::write(&agents_path, new_contents)
                    .map_err(|e| Error::Other(format!("Failed to write AGENTS.md: {}", e)))?;
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            // Append new section
            let mut file = fs::OpenOptions::new()
                .append(true)
                .open(&agents_path)
                .map_err(|e| Error::Other(format!("Failed to open AGENTS.md: {}", e)))?;

            // Add a newline before the blurb if file doesn't end with one
            let prefix = if contents.ends_with('\n') { "\n" } else { "\n\n" };
            file.write_all(prefix.as_bytes())
                .map_err(|e| Error::Other(format!("Failed to write to AGENTS.md: {}", e)))?;
            file.write_all(AGENTS_MD_BLURB.trim_end().as_bytes())
                .map_err(|e| Error::Other(format!("Failed to write to AGENTS.md: {}", e)))?;
            file.write_all(b"\n")
                .map_err(|e| Error::Other(format!("Failed to write to AGENTS.md: {}", e)))?;
            Ok(true)
        }
    } else {
        // Create new AGENTS.md with the blurb
        fs::write(&agents_path, format!("{}\n", AGENTS_MD_BLURB.trim_end()))
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
    // Auto-initialize if needed (without prompts)
    let initialized = if !Storage::exists(repo_path)? {
        Storage::init(repo_path)?;
        // Auto-update AGENTS.md (idempotent)
        let _ = update_agents_md(repo_path);
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_name: Option<String>,
}

impl Output for TaskCreated {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        match &self.short_name {
            Some(sn) => format!("Created task {} [{}] \"{}\"", self.id, sn, self.title),
            None => format!("Created task {} \"{}\"", self.id, self.title),
        }
    }
}

/// Create a new task.
pub fn task_create(
    repo_path: &Path,
    title: String,
    short_name: Option<String>,
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

    // Auto-truncate very long short_name (2x display limit = 30 chars)
    let short_name = short_name.map(|sn| {
        if sn.chars().count() > 30 {
            eprintln!(
                "Note: short_name truncated from {} to 30 chars for GUI display.",
                sn.chars().count()
            );
            sn.chars().take(30).collect::<String>()
        } else {
            sn
        }
    });

    let id = generate_id("bn", &title);
    let mut task = Task::new(id.clone(), title.clone());
    task.short_name = short_name.clone();
    task.description = description;
    task.priority = priority.unwrap_or(2);
    task.tags = tags;
    task.assignee = assignee;

    storage.create_task(&task)?;

    Ok(TaskCreated {
        id,
        title,
        short_name,
    })
}

impl Output for Task {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("{} {}", self.id, self.title));
        if let Some(ref sn) = self.short_name {
            lines.push(format!("  Short name: {}", sn));
        }
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

/// Information about what is blocking a task.
#[derive(Serialize)]
pub struct BlockingInfo {
    pub is_blocked: bool,
    pub blocker_count: usize,
    pub direct_blockers: Vec<DirectBlocker>,
    pub blocker_chain: Vec<String>,
    pub summary: String,
}

/// A task that is directly blocking another task.
#[derive(Serialize)]
pub struct DirectBlocker {
    pub id: String,
    pub title: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub blocked_by: Vec<String>,
}

/// Result of task_show with optional blocking analysis.
#[derive(Serialize)]
pub struct TaskShowResult {
    #[serde(flatten)]
    pub task: Task,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocking_info: Option<BlockingInfo>,
}

impl Output for TaskShowResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = vec![
            format!("Task: {}", self.task.id),
            format!("Title: {}", self.task.title),
        ];

        if let Some(ref sn) = self.task.short_name {
            lines.push(format!("Short Name: {}", sn));
        }

        lines.push(format!("Status: {:?}", self.task.status));
        lines.push(format!("Priority: P{}", self.task.priority));

        if !self.task.tags.is_empty() {
            lines.push(format!("Tags: {}", self.task.tags.join(", ")));
        }

        if let Some(ref desc) = self.task.description {
            lines.push(format!("Description: {}", desc));
        }

        if let Some(ref assignee) = self.task.assignee {
            lines.push(format!("Assignee: {}", assignee));
        }

        if !self.task.depends_on.is_empty() {
            lines.push(format!(
                "\nDependencies ({}): {}",
                self.task.depends_on.len(),
                self.task.depends_on.join(", ")
            ));
        }

        if let Some(ref blocking) = self.blocking_info {
            lines.push(format!("\n{}", blocking.summary));
        }

        if let Some(ref closed_at) = self.task.closed_at {
            lines.push(format!("\nClosed at: {}", closed_at));
            if let Some(ref reason) = self.task.closed_reason {
                lines.push(format!("Reason: {}", reason));
            }
        }

        lines.join("\n")
    }
}

/// Analyze what is blocking a task from completion.
fn analyze_blockers(storage: &Storage, task: &Task) -> Result<BlockingInfo> {
    let mut direct_blockers = Vec::new();
    let mut blocker_chain = Vec::new();

    for dep_id in &task.depends_on {
        if let Ok(dep) = storage.get_task(dep_id) {
            // Only consider incomplete dependencies as blockers
            if dep.status != TaskStatus::Done && dep.status != TaskStatus::Cancelled {
                // Find what's blocking this dependency (transitive blockers)
                let dep_blockers: Vec<String> = dep
                    .depends_on
                    .iter()
                    .filter(|d| {
                        storage
                            .get_task(d)
                            .map(|t| {
                                t.status != TaskStatus::Done && t.status != TaskStatus::Cancelled
                            })
                            .unwrap_or(false)
                    })
                    .cloned()
                    .collect();

                direct_blockers.push(DirectBlocker {
                    id: dep.id.clone(),
                    title: dep.title.clone(),
                    status: format!("{:?}", dep.status).to_lowercase(),
                    assignee: dep.assignee.clone(),
                    blocked_by: dep_blockers.clone(),
                });

                // Build chain representation
                if dep_blockers.is_empty() {
                    blocker_chain.push(format!(
                        "{} <- {} ({})",
                        task.id,
                        dep.id,
                        format!("{:?}", dep.status).to_lowercase()
                    ));
                } else {
                    for blocker in &dep_blockers {
                        if let Ok(b) = storage.get_task(blocker) {
                            blocker_chain.push(format!(
                                "{} <- {} <- {} ({})",
                                task.id,
                                dep.id,
                                blocker,
                                format!("{:?}", b.status).to_lowercase()
                            ));
                        }
                    }
                }
            }
        }
    }

    let is_blocked = !direct_blockers.is_empty();
    let blocker_count = direct_blockers.len();

    let summary = if is_blocked {
        build_blocker_summary(&direct_blockers, blocker_count)
    } else {
        "All dependencies complete.".to_string()
    };

    Ok(BlockingInfo {
        is_blocked,
        blocker_count,
        direct_blockers,
        blocker_chain,
        summary,
    })
}

/// Build a human-readable summary of blocking dependencies.
fn build_blocker_summary(blockers: &[DirectBlocker], count: usize) -> String {
    let mut parts = Vec::new();
    parts.push(format!("Blocked by {} incomplete dependencies.", count));

    for blocker in blockers {
        let mut desc = format!("- {} is {}", blocker.id, blocker.status);
        if let Some(ref assignee) = blocker.assignee {
            desc.push_str(&format!(" (assigned: {})", assignee));
        }
        if !blocker.blocked_by.is_empty() {
            desc.push_str(&format!(", blocked by {}", blocker.blocked_by.join(", ")));
        }
        parts.push(desc);
    }

    parts.join("\n")
}

/// Show a task by ID with optional blocking analysis.
pub fn task_show(repo_path: &Path, id: &str) -> Result<TaskShowResult> {
    let storage = Storage::open(repo_path)?;
    let task = storage.get_task(id)?;

    // Analyze blocking status if task has dependencies
    let blocking_info = if !task.depends_on.is_empty() {
        Some(analyze_blockers(&storage, &task)?)
    } else {
        None
    };

    Ok(TaskShowResult {
        task,
        blocking_info,
    })
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
                TaskStatus::Partial => "~",
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
    short_name: Option<String>,
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

    if let Some(s) = short_name {
        // Empty or whitespace-only clears the short_name
        if s.trim().is_empty() {
            task.short_name = None;
        } else {
            // Auto-truncate very long short_name (2x display limit = 30 chars)
            let truncated = if s.chars().count() > 30 {
                eprintln!(
                    "Note: short_name truncated from {} to 30 chars for GUI display.",
                    s.chars().count()
                );
                s.chars().take(30).collect::<String>()
            } else {
                s
            };
            task.short_name = Some(truncated);
        }
        updated_fields.push("short_name".to_string());
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

#[derive(Debug, Serialize)]
pub struct TaskClosed {
    pub id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

impl Output for TaskClosed {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut output = format!("Closed task {}", self.id);
        if let Some(warning) = &self.warning {
            output.push_str(&format!("\nWarning: {}", warning));
        }
        output
    }
}

fn promote_partial_tasks(storage: &mut Storage) -> Result<Vec<String>> {
    let mut promoted = Vec::new();

    loop {
        let tasks = storage.list_tasks(None, None, None)?;
        let mut did_promote = false;

        for mut task in tasks {
            if task.status != TaskStatus::Partial {
                continue;
            }

            let all_done = task.depends_on.iter().all(|dep_id| {
                storage
                    .get_task(dep_id)
                    .map(|dep| matches!(dep.status, TaskStatus::Done | TaskStatus::Cancelled))
                    .unwrap_or(false)
            });

            if all_done {
                task.status = TaskStatus::Done;
                task.closed_at = Some(Utc::now());
                task.updated_at = Utc::now();
                storage.update_task(&task)?;
                promoted.push(task.id.clone());
                did_promote = true;
            }
        }

        if !did_promote {
            break;
        }
    }

    Ok(promoted)
}

/// Close a task.
pub fn task_close(
    repo_path: &Path,
    id: &str,
    reason: Option<String>,
    force: bool,
) -> Result<TaskClosed> {
    let mut storage = Storage::open(repo_path)?;
    let task = storage.get_task(id)?;

    // Check for incomplete dependencies
    let incomplete_deps: Vec<Task> = task
        .depends_on
        .iter()
        .filter_map(|dep_id| storage.get_task(dep_id).ok())
        .filter(|dep| dep.status != TaskStatus::Done && dep.status != TaskStatus::Cancelled)
        .collect();

    // If there are incomplete dependencies and force is false, return error
    if !incomplete_deps.is_empty() && !force {
        let dep_list: Vec<String> = incomplete_deps
            .iter()
            .map(|d| {
                format!(
                    "{}: \"{}\" ({})",
                    d.id,
                    d.title,
                    format!("{:?}", d.status).to_lowercase()
                )
            })
            .collect();

        return Err(Error::Other(format!(
            "Cannot close task {}. It has {} incomplete dependencies:\n  - {}\n\nUse --force to close anyway, or complete the dependencies first.",
            id,
            incomplete_deps.len(),
            dep_list.join("\n  - ")
        )));
    }

    // Proceed with close
    let mut task = task;
    task.status = TaskStatus::Done;
    task.closed_at = Some(Utc::now());
    task.closed_reason = reason;
    task.updated_at = Utc::now();

    storage.update_task(&task)?;
    promote_partial_tasks(&mut storage)?;

    // Generate warning if force was used with incomplete deps
    let warning = if !incomplete_deps.is_empty() {
        Some(format!(
            "Closed with {} incomplete dependencies",
            incomplete_deps.len()
        ))
    } else {
        None
    };

    Ok(TaskClosed {
        id: id.to_string(),
        status: "done".to_string(),
        warning,
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

// === Bug Commands ===

fn parse_severity(s: &str) -> Result<BugSeverity> {
    match s.to_lowercase().as_str() {
        "triage" => Ok(BugSeverity::Triage),
        "low" => Ok(BugSeverity::Low),
        "medium" => Ok(BugSeverity::Medium),
        "high" => Ok(BugSeverity::High),
        "critical" => Ok(BugSeverity::Critical),
        _ => Err(Error::Other(format!("Invalid severity: {}", s))),
    }
}

#[derive(Debug, Serialize)]
pub struct BugCreated {
    pub id: String,
    pub title: String,
}

impl Output for BugCreated {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Created bug {} \"{}\"", self.id, self.title)
    }
}

/// Create a new bug.
#[allow(clippy::too_many_arguments)]
pub fn bug_create(
    repo_path: &Path,
    title: String,
    description: Option<String>,
    priority: Option<u8>,
    severity: Option<String>,
    tags: Vec<String>,
    assignee: Option<String>,
    reproduction_steps: Option<String>,
    affected_component: Option<String>,
) -> Result<BugCreated> {
    let mut storage = Storage::open(repo_path)?;

    if let Some(p) = priority {
        if p > 4 {
            return Err(Error::Other("Priority must be 0-4".to_string()));
        }
    }

    let id = generate_id("bn", &title);
    let mut bug = Bug::new(id.clone(), title.clone());
    bug.description = description;
    bug.priority = priority.unwrap_or(2);
    bug.severity = severity
        .as_deref()
        .map(parse_severity)
        .transpose()?
        .unwrap_or_default();
    bug.tags = tags;
    bug.assignee = assignee;
    bug.reproduction_steps = reproduction_steps;
    bug.affected_component = affected_component;

    storage.add_bug(&bug)?;

    Ok(BugCreated { id, title })
}

impl Output for Bug {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("{} {}", self.id, self.title));
        lines.push(format!(
            "  Status: {:?}  Priority: {}  Severity: {:?}",
            self.status, self.priority, self.severity
        ));
        if let Some(ref desc) = self.description {
            lines.push(format!("  Description: {}", desc));
        }
        if let Some(ref steps) = self.reproduction_steps {
            lines.push(format!("  Reproduction steps: {}", steps));
        }
        if let Some(ref component) = self.affected_component {
            lines.push(format!("  Affected component: {}", component));
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

#[derive(Serialize)]
pub struct BugList {
    pub bugs: Vec<Bug>,
    pub count: usize,
}

impl Output for BugList {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        if self.bugs.is_empty() {
            return "No bugs found.".to_string();
        }

        let mut lines = Vec::new();
        lines.push(format!("{} bug(s):\n", self.count));

        for bug in &self.bugs {
            let status_char = match bug.status {
                TaskStatus::Pending => " ",
                TaskStatus::InProgress => ">",
                TaskStatus::Done => "x",
                TaskStatus::Blocked => "!",
                TaskStatus::Cancelled => "-",
                TaskStatus::Reopened => "?",
                TaskStatus::Partial => "~",
            };
            let tags = if bug.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", bug.tags.join(", "))
            };
            lines.push(format!(
                "[{}] {} P{} S:{} {}{}",
                status_char,
                bug.id,
                bug.priority,
                format!("{:?}", bug.severity).to_lowercase(),
                bug.title,
                tags
            ));
        }

        lines.join("\n")
    }
}

/// List bugs with optional filters.
pub fn bug_list(
    repo_path: &Path,
    status: Option<&str>,
    priority: Option<u8>,
    severity: Option<&str>,
    tag: Option<&str>,
) -> Result<BugList> {
    let storage = Storage::open(repo_path)?;
    let bugs = storage.list_bugs(status, priority, severity, tag)?;
    let count = bugs.len();
    Ok(BugList { bugs, count })
}

/// Show a bug by ID.
pub fn bug_show(repo_path: &Path, id: &str) -> Result<Bug> {
    let storage = Storage::open(repo_path)?;
    storage.get_bug(id)
}

#[derive(Debug, Serialize)]
pub struct BugUpdated {
    pub id: String,
    pub updated_fields: Vec<String>,
}

impl Output for BugUpdated {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!(
            "Updated bug {}: {}",
            self.id,
            self.updated_fields.join(", ")
        )
    }
}

/// Update a bug.
#[allow(clippy::too_many_arguments)]
pub fn bug_update(
    repo_path: &Path,
    id: &str,
    title: Option<String>,
    description: Option<String>,
    priority: Option<u8>,
    status: Option<&str>,
    severity: Option<String>,
    add_tags: Vec<String>,
    remove_tags: Vec<String>,
    assignee: Option<String>,
    reproduction_steps: Option<String>,
    affected_component: Option<String>,
) -> Result<BugUpdated> {
    let mut storage = Storage::open(repo_path)?;
    let mut bug = storage.get_bug(id)?;
    let mut updated_fields = Vec::new();

    if let Some(t) = title {
        bug.title = t;
        updated_fields.push("title".to_string());
    }

    if let Some(d) = description {
        bug.description = Some(d);
        updated_fields.push("description".to_string());
    }

    if let Some(p) = priority {
        if p > 4 {
            return Err(Error::Other("Priority must be 0-4".to_string()));
        }
        bug.priority = p;
        updated_fields.push("priority".to_string());
    }

    if let Some(s) = status {
        bug.status = parse_status(s)?;
        updated_fields.push("status".to_string());
    }

    if let Some(s) = severity {
        bug.severity = parse_severity(&s)?;
        updated_fields.push("severity".to_string());
    }

    if !add_tags.is_empty() {
        for tag in add_tags {
            if !bug.tags.contains(&tag) {
                bug.tags.push(tag);
            }
        }
        updated_fields.push("tags".to_string());
    }

    if !remove_tags.is_empty() {
        bug.tags.retain(|t| !remove_tags.contains(t));
        if !updated_fields.contains(&"tags".to_string()) {
            updated_fields.push("tags".to_string());
        }
    }

    if let Some(a) = assignee {
        bug.assignee = Some(a);
        updated_fields.push("assignee".to_string());
    }

    if let Some(steps) = reproduction_steps {
        bug.reproduction_steps = Some(steps);
        updated_fields.push("reproduction_steps".to_string());
    }

    if let Some(component) = affected_component {
        bug.affected_component = Some(component);
        updated_fields.push("affected_component".to_string());
    }

    if updated_fields.is_empty() {
        return Err(Error::Other("No fields to update".to_string()));
    }

    bug.updated_at = Utc::now();
    storage.update_bug(&bug)?;

    Ok(BugUpdated {
        id: id.to_string(),
        updated_fields,
    })
}

#[derive(Debug, Serialize)]
pub struct BugClosed {
    pub id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

impl Output for BugClosed {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut output = format!("Closed bug {}", self.id);
        if let Some(warning) = &self.warning {
            output.push_str(&format!("\nWarning: {}", warning));
        }
        output
    }
}

/// Close a bug.
pub fn bug_close(
    repo_path: &Path,
    id: &str,
    reason: Option<String>,
    force: bool,
) -> Result<BugClosed> {
    let mut storage = Storage::open(repo_path)?;
    let bug = storage.get_bug(id)?;

    let incomplete_deps: Vec<Bug> = bug
        .depends_on
        .iter()
        .filter_map(|dep_id| storage.get_bug(dep_id).ok())
        .filter(|dep| dep.status != TaskStatus::Done && dep.status != TaskStatus::Cancelled)
        .collect();

    if !incomplete_deps.is_empty() && !force {
        let dep_list: Vec<String> = incomplete_deps
            .iter()
            .map(|d| {
                format!(
                    "{}: \"{}\" ({})",
                    d.id,
                    d.title,
                    format!("{:?}", d.status).to_lowercase()
                )
            })
            .collect();

        return Err(Error::Other(format!(
            "Cannot close bug {}. It has {} incomplete dependencies:\n  - {}\n\nUse --force to close anyway, or complete the dependencies first.",
            id,
            incomplete_deps.len(),
            dep_list.join("\n  - ")
        )));
    }

    let mut bug = bug;
    bug.status = TaskStatus::Done;
    bug.closed_at = Some(Utc::now());
    bug.closed_reason = reason;
    bug.updated_at = Utc::now();

    storage.update_bug(&bug)?;

    let warning = if !incomplete_deps.is_empty() {
        Some(format!(
            "Closed with {} incomplete dependencies",
            incomplete_deps.len()
        ))
    } else {
        None
    };

    Ok(BugClosed {
        id: id.to_string(),
        status: "done".to_string(),
        warning,
    })
}

#[derive(Serialize)]
pub struct BugReopened {
    pub id: String,
    pub status: String,
}

impl Output for BugReopened {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Reopened bug {}", self.id)
    }
}

/// Reopen a closed bug.
pub fn bug_reopen(repo_path: &Path, id: &str) -> Result<BugReopened> {
    let mut storage = Storage::open(repo_path)?;
    let mut bug = storage.get_bug(id)?;

    bug.status = TaskStatus::Reopened;
    bug.closed_at = None;
    bug.closed_reason = None;
    bug.updated_at = Utc::now();

    storage.update_bug(&bug)?;

    Ok(BugReopened {
        id: id.to_string(),
        status: "reopened".to_string(),
    })
}

#[derive(Serialize)]
pub struct BugDeleted {
    pub id: String,
}

impl Output for BugDeleted {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        format!("Deleted bug {}", self.id)
    }
}

/// Delete a bug.
pub fn bug_delete(repo_path: &Path, id: &str) -> Result<BugDeleted> {
    let mut storage = Storage::open(repo_path)?;
    storage.delete_bug(id)?;

    Ok(BugDeleted { id: id.to_string() })
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
    // Validate configuration values
    match key {
        "action_log_enabled" | "action_log_sanitize" => {
            // Validate boolean values
            let value_lower = value.to_lowercase();
            if value_lower != "true"
                && value_lower != "false"
                && value_lower != "1"
                && value_lower != "0"
                && value_lower != "yes"
                && value_lower != "no"
            {
                return Err(Error::Other(format!(
                    "Invalid boolean value for {}: {}. Must be one of: true, false, 1, 0, yes, no",
                    key, value
                )));
            }
        }
        "action_log_path" => {
            // Validate path is not empty
            if value.trim().is_empty() {
                return Err(Error::Other("action_log_path cannot be empty".to_string()));
            }
        }
        _ => {
            // No validation for unknown keys (for forward compatibility)
        }
    }

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

// === System Store Commands ===

/// File information for store show output.
#[derive(Serialize)]
pub struct StoreFileInfo {
    pub size_bytes: u64,
    pub entries: usize,
}

/// Task count breakdown by status.
#[derive(Serialize)]
pub struct TasksByStatus {
    pub pending: usize,
    pub in_progress: usize,
    pub blocked: usize,
    pub done: usize,
}

/// Tasks information for store show output.
#[derive(Serialize)]
pub struct TasksInfo {
    pub total: usize,
    pub by_status: TasksByStatus,
}

/// Tests information for store show output.
#[derive(Serialize)]
pub struct TestsInfo {
    pub total: usize,
    pub linked: usize,
}

/// Commits information for store show output.
#[derive(Serialize)]
pub struct CommitsInfo {
    pub total: usize,
}

/// Result of the `bn system store show` command.
#[derive(Serialize)]
pub struct StoreShowResult {
    pub storage_path: String,
    pub repo_path: String,
    pub tasks: TasksInfo,
    pub tests: TestsInfo,
    pub commits: CommitsInfo,
    pub files: std::collections::HashMap<String, StoreFileInfo>,
    pub created_at: Option<String>,
    pub last_modified: Option<String>,
}

impl Output for StoreShowResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();

        lines.push(format!("Store: {}", self.storage_path));
        lines.push(format!("Repo:  {}", self.repo_path));
        lines.push(String::new());

        lines.push(format!("Tasks: {} total", self.tasks.total));
        lines.push(format!("  - pending:     {}", self.tasks.by_status.pending));
        lines.push(format!("  - in_progress: {}", self.tasks.by_status.in_progress));
        lines.push(format!("  - blocked:     {}", self.tasks.by_status.blocked));
        lines.push(format!("  - done:        {}", self.tasks.by_status.done));
        lines.push(String::new());

        lines.push(format!(
            "Tests: {} total ({} linked to tasks)",
            self.tests.total, self.tests.linked
        ));
        lines.push(format!("Commits: {} linked", self.commits.total));
        lines.push(String::new());

        lines.push("Files:".to_string());
        let mut file_names: Vec<_> = self.files.keys().collect();
        file_names.sort();
        for file_name in file_names {
            let info = &self.files[file_name];
            let size_kb = info.size_bytes as f64 / 1024.0;
            lines.push(format!(
                "  {:<18} {:>7.1} KB  ({} entries)",
                file_name, size_kb, info.entries
            ));
        }

        if let (Some(created), Some(modified)) = (&self.created_at, &self.last_modified) {
            lines.push(String::new());
            lines.push(format!("Created:  {}", created));
            lines.push(format!("Modified: {}", modified));
        }

        lines.join("\n")
    }
}

/// Display summary of current store contents.
pub fn system_store_show(repo_path: &Path) -> Result<StoreShowResult> {
    let storage = Storage::open(repo_path)?;
    let storage_path = storage.root().to_string_lossy().to_string();
    let repo_path_str = repo_path.to_string_lossy().to_string();

    // Get tasks and count by status
    let tasks = storage.list_tasks(None, None, None)?;
    let total_tasks = tasks.len();
    let mut pending = 0;
    let mut in_progress = 0;
    let mut blocked = 0;
    let mut done = 0;

    for task in &tasks {
        match task.status {
            TaskStatus::Pending => pending += 1,
            TaskStatus::InProgress => in_progress += 1,
            TaskStatus::Blocked => blocked += 1,
            TaskStatus::Done => done += 1,
            _ => {}
        }
    }

    // Get tests and count linked ones
    let tests = storage.list_tests(None)?;
    let total_tests = tests.len();
    let linked_tests = tests.iter().filter(|t| !t.linked_tasks.is_empty()).count();

    // Get commit count
    let total_commits = storage.count_commit_links()?;

    // Get file information
    use std::collections::HashMap;
    let mut files = HashMap::new();

    let file_names = ["tasks.jsonl", "bugs.jsonl", "commits.jsonl", "test-results.jsonl", "cache.db"];
    for file_name in &file_names {
        let file_path = storage.root().join(file_name);
        if let Ok(metadata) = std::fs::metadata(&file_path) {
            let size_bytes = metadata.len();

            // Count entries for JSONL files
            let entries = if file_name.ends_with(".jsonl") {
                std::fs::read_to_string(&file_path)
                    .map(|content| content.lines().filter(|line| !line.trim().is_empty()).count())
                    .unwrap_or(0)
            } else {
                0 // cache.db doesn't have "entries" in the same sense
            };

            files.insert(
                file_name.to_string(),
                StoreFileInfo {
                    size_bytes,
                    entries,
                },
            );
        }
    }

    // Get creation and modification times from tasks.jsonl
    let tasks_file = storage.root().join("tasks.jsonl");
    let (created_at, last_modified) = if let Ok(metadata) = std::fs::metadata(&tasks_file) {
        use std::time::SystemTime;

        let created = metadata.created()
            .or_else(|_| metadata.modified())
            .ok()
            .and_then(|time| {
                time.duration_since(SystemTime::UNIX_EPOCH).ok()
            })
            .map(|duration| {
                chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0)
                    .unwrap_or_default()
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string()
            });

        let modified = metadata.modified()
            .ok()
            .and_then(|time| {
                time.duration_since(SystemTime::UNIX_EPOCH).ok()
            })
            .map(|duration| {
                chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0)
                    .unwrap_or_default()
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string()
            });

        (created, modified)
    } else {
        (None, None)
    };

    Ok(StoreShowResult {
        storage_path,
        repo_path: repo_path_str,
        tasks: TasksInfo {
            total: total_tasks,
            by_status: TasksByStatus {
                pending,
                in_progress,
                blocked,
                done,
            },
        },
        tests: TestsInfo {
            total: total_tests,
            linked: linked_tests,
        },
        commits: CommitsInfo {
            total: total_commits,
        },
        files,
        created_at,
        last_modified,
    })
}

// === Store Export Command ===

/// Result of the `bn system store export` command.
#[derive(Serialize)]
pub struct StoreExportResult {
    pub exported: bool,
    pub output_path: String,
    pub size_bytes: u64,
    pub task_count: usize,
    pub test_count: usize,
    pub commit_count: usize,
}

impl Output for StoreExportResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();
        if self.exported {
            if self.output_path == "-" {
                lines.push("Exported to stdout".to_string());
            } else {
                lines.push(format!("Exported to {}", self.output_path));
            }
            let size_kb = self.size_bytes as f64 / 1024.0;
            lines.push(format!("  Size: {:.1} KB", size_kb));
            lines.push(format!("  Tasks: {}", self.task_count));
            lines.push(format!("  Tests: {}", self.test_count));
            lines.push(format!("  Commits: {}", self.commit_count));
        } else {
            lines.push("Export failed".to_string());
        }
        lines.join("\n")
    }
}

/// Manifest metadata for the export archive.
#[derive(Serialize, Deserialize)]
struct ExportManifest {
    version: u32,
    format: String,
    exported_at: String,
    source_repo: String,
    binnacle_version: String,
    task_count: usize,
    test_count: usize,
    commit_count: usize,
    checksums: std::collections::HashMap<String, String>,
}

/// Config metadata for the export archive.
#[derive(Serialize)]
struct ExportConfig {
    repo_path: String,
    exported_at: String,
}

/// Calculate SHA256 checksum of file contents.
fn calculate_checksum(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Export store to gzip tar archive.
pub fn system_store_export(repo_path: &Path, output: &str, format: &str) -> Result<StoreExportResult> {
    use flate2::write::GzEncoder;
    use flate2::Compression;

    if format != "archive" {
        return Err(Error::InvalidInput(format!(
            "Unsupported format '{}'. Only 'archive' is currently supported.",
            format
        )));
    }

    let storage = Storage::open(repo_path)?;
    let storage_root = storage.root();

    // Read all JSONL files (skip missing files for backwards compatibility)
    let files_to_export = ["tasks.jsonl", "bugs.jsonl", "commits.jsonl", "test-results.jsonl"];
    let mut file_contents = std::collections::HashMap::new();
    let mut checksums = std::collections::HashMap::new();

    for filename in &files_to_export {
        let file_path = storage_root.join(filename);
        if file_path.exists() {
            let data = fs::read(&file_path)?;
            let checksum = calculate_checksum(&data);
            checksums.insert(filename.to_string(), checksum);
            file_contents.insert(filename.to_string(), data);
        }
    }

    // Count tasks, tests, and commits
    let tasks = storage.list_tasks(None, None, None)?;
    let tests = storage.list_tests(None)?;
    let commit_count = storage.count_commit_links()?;

    // Create manifest
    let manifest = ExportManifest {
        version: 1,
        format: "binnacle-store-v1".to_string(),
        exported_at: Utc::now().to_rfc3339(),
        source_repo: repo_path.to_string_lossy().to_string(),
        binnacle_version: env!("CARGO_PKG_VERSION").to_string(),
        task_count: tasks.len(),
        test_count: tests.len(),
        commit_count,
        checksums: checksums.clone(),
    };
    let manifest_json = serde_json::to_string_pretty(&manifest)?;

    // Create config
    let config = ExportConfig {
        repo_path: repo_path.to_string_lossy().to_string(),
        exported_at: Utc::now().to_rfc3339(),
    };
    let config_json = serde_json::to_string_pretty(&config)?;

    // Create tar.gz archive in memory
    let mut archive_buffer = Vec::new();
    {
        let encoder = GzEncoder::new(&mut archive_buffer, Compression::default());
        let mut tar = tar::Builder::new(encoder);

        // Add manifest.json
        let manifest_bytes = manifest_json.as_bytes();
        let mut header = tar::Header::new_gnu();
        header.set_path("binnacle-export/manifest.json")?;
        header.set_size(manifest_bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append(&header, manifest_bytes)?;

        // Add config.json
        let config_bytes = config_json.as_bytes();
        let mut header = tar::Header::new_gnu();
        header.set_path("binnacle-export/config.json")?;
        header.set_size(config_bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append(&header, config_bytes)?;

        // Add JSONL files
        for (filename, data) in &file_contents {
            let mut header = tar::Header::new_gnu();
            header.set_path(format!("binnacle-export/{}", filename))?;
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append(&header, data.as_slice())?;
        }

        tar.finish()?;
    }

    let size_bytes = archive_buffer.len() as u64;

    // Write to output (file or stdout)
    if output == "-" {
        use std::io::{self, Write};
        io::stdout().write_all(&archive_buffer)?;
    } else {
        fs::write(output, &archive_buffer)?;
    }

    Ok(StoreExportResult {
        exported: true,
        output_path: output.to_string(),
        size_bytes,
        task_count: tasks.len(),
        test_count: tests.len(),
        commit_count,
    })
}

/// Result of the store import command.
#[derive(Serialize)]
pub struct StoreImportResult {
    pub imported: bool,
    pub dry_run: bool,
    pub input_path: String,
    pub import_type: String,
    pub tasks_imported: usize,
    pub tasks_skipped: usize,
    pub tests_imported: usize,
    pub commits_imported: usize,
    pub collisions: usize,
    pub id_remappings: std::collections::HashMap<String, String>,
}

impl Output for StoreImportResult {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn to_human(&self) -> String {
        let mut lines = Vec::new();

        if self.dry_run {
            lines.push("DRY RUN - No changes made".to_string());
            lines.push(String::new());
        }

        if self.imported || self.dry_run {
            if self.input_path == "-" {
                lines.push("Imported from stdin".to_string());
            } else {
                lines.push(format!("Imported from {}", self.input_path));
            }
            lines.push(format!("  Type: {}", self.import_type));
            lines.push(format!("  Tasks: {} imported, {} skipped", self.tasks_imported, self.tasks_skipped));
            lines.push(format!("  Tests: {}", self.tests_imported));
            lines.push(format!("  Commits: {}", self.commits_imported));

            if self.collisions > 0 {
                lines.push(String::new());
                lines.push(format!("⚠️  WARNING: {} ID COLLISIONS DETECTED", self.collisions));
                for (old_id, new_id) in &self.id_remappings {
                    lines.push(format!("   {} → {}", old_id, new_id));
                }
            }
        } else {
            lines.push("Import failed".to_string());
        }

        lines.join("\n")
    }
}

/// Import store from archive.
pub fn system_store_import(
    repo_path: &Path,
    input: &str,
    import_type: &str,
    dry_run: bool,
) -> Result<StoreImportResult> {
    use flate2::read::GzDecoder;
    use std::io::Read;

    // Detect input type: stdin, directory, or archive file
    let input_path = Path::new(input);
    if input != "-" && input_path.is_dir() {
        return system_store_import_from_folder(repo_path, input_path, import_type, dry_run);
    }

    // Read input (file or stdin)
    let archive_data = if input == "-" {
        let mut buffer = Vec::new();
        std::io::stdin().read_to_end(&mut buffer)?;
        buffer
    } else {
        fs::read(input)?
    };

    // Extract archive
    let decoder = GzDecoder::new(&archive_data[..]);
    let mut archive = tar::Archive::new(decoder);

    // Extract files to memory
    let mut manifest: Option<ExportManifest> = None;
    let mut tasks_jsonl: Option<Vec<u8>> = None;
    let mut commits_jsonl: Option<Vec<u8>> = None;
    let mut test_results_jsonl: Option<Vec<u8>> = None;

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_path_buf();
        let path_str = path.to_string_lossy().to_string();

        let mut data = Vec::new();
        entry.read_to_end(&mut data)?;

        if path_str.ends_with("manifest.json") {
            manifest = Some(serde_json::from_slice(&data)?);
        } else if path_str.ends_with("tasks.jsonl") {
            tasks_jsonl = Some(data);
        } else if path_str.ends_with("commits.jsonl") {
            commits_jsonl = Some(data);
        } else if path_str.ends_with("test-results.jsonl") {
            test_results_jsonl = Some(data);
        }
    }

    let manifest = manifest.ok_or_else(|| {
        Error::InvalidInput("Archive does not contain manifest.json".to_string())
    })?;

    // Validate version
    if manifest.version != 1 {
        return Err(Error::InvalidInput(format!(
            "Incompatible archive version {}. Expected version 1.",
            manifest.version
        )));
    }

    if manifest.format != "binnacle-store-v1" {
        return Err(Error::InvalidInput(format!(
            "Incompatible archive format '{}'. Expected 'binnacle-store-v1'.",
            manifest.format
        )));
    }

    // Check if already initialized for replace mode
    let storage = Storage::open(repo_path);
    let is_initialized = storage.is_ok();

    if import_type == "replace" && is_initialized {
        return Err(Error::InvalidInput(
            "Store already initialized. Use --type merge to append data, or reinitialize the repo.".to_string()
        ));
    }

    // Initialize if needed
    let mut storage = if is_initialized {
        storage.unwrap()
    } else {
        // Auto-initialize for import
        Storage::init(repo_path)?
    };

    // Parse imported tasks
    let mut imported_tasks: Vec<Task> = Vec::new();
    if let Some(tasks_data) = tasks_jsonl {
        let tasks_str = String::from_utf8_lossy(&tasks_data);
        for line in tasks_str.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let task: Task = serde_json::from_str(line)?;
            imported_tasks.push(task);
        }
    }

    // Detect ID collisions and create remapping
    let mut id_remappings: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let existing_tasks = storage.list_tasks(None, None, None)?;
    let existing_ids: std::collections::HashSet<String> =
        existing_tasks.iter().map(|t| t.id.clone()).collect();

    for task in &imported_tasks {
        if existing_ids.contains(&task.id) {
            // Generate new ID using task title as seed
            let new_id = crate::storage::generate_id("bn", &task.title);
            id_remappings.insert(task.id.clone(), new_id);
        }
    }

    let collisions = id_remappings.len();

    // If dry run, return early
    if dry_run {
        return Ok(StoreImportResult {
            imported: false,
            dry_run: true,
            input_path: input.to_string(),
            import_type: import_type.to_string(),
            tasks_imported: imported_tasks.len(),
            tasks_skipped: 0,
            tests_imported: 0,
            commits_imported: 0,
            collisions,
            id_remappings,
        });
    }

    // Import tasks with remapping
    let mut tasks_imported = 0;
    let import_timestamp = Utc::now();

    for mut task in imported_tasks {
        // Remap task ID if needed
        if let Some(new_id) = id_remappings.get(&task.id) {
            task.id = new_id.clone();
        }

        // Remap dependencies
        let mut new_depends_on = Vec::new();
        for dep in &task.depends_on {
            if let Some(new_dep_id) = id_remappings.get(dep) {
                new_depends_on.push(new_dep_id.clone());
            } else {
                new_depends_on.push(dep.clone());
            }
        }
        task.depends_on = new_depends_on;

        // Set imported_on timestamp if merging
        if import_type == "merge" {
            task.imported_on = Some(import_timestamp);
        }

        // Create task
        storage.create_task(&task)?;
        tasks_imported += 1;
    }

    // Import commits (simple append, no ID remapping needed for now)
    let mut commits_imported = 0;
    if let Some(commits_data) = commits_jsonl {
        let storage_root = storage.root();
        let commits_file = storage_root.join("commits.jsonl");

        // Append to existing file
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&commits_file)?;

        file.write_all(&commits_data)?;

        // Count commits
        let commits_str = String::from_utf8_lossy(&commits_data);
        commits_imported = commits_str.lines().filter(|l| !l.trim().is_empty()).count();
    }

    // Import test results
    let mut tests_imported = 0;
    if let Some(test_data) = test_results_jsonl {
        let storage_root = storage.root();
        let test_file = storage_root.join("test-results.jsonl");

        // Append to existing file
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&test_file)?;

        file.write_all(&test_data)?;

        // Count tests
        let test_str = String::from_utf8_lossy(&test_data);
        tests_imported = test_str.lines().filter(|l| !l.trim().is_empty()).count();
    }

    // Rebuild cache
    storage.rebuild_cache()?;

    Ok(StoreImportResult {
        imported: true,
        dry_run: false,
        input_path: input.to_string(),
        import_type: import_type.to_string(),
        tasks_imported,
        tasks_skipped: 0,
        tests_imported,
        commits_imported,
        collisions,
        id_remappings,
    })
}

/// Import store from a folder containing JSONL files.
///
/// Expected folder structure:
/// - tasks.jsonl (required)
/// - commits.jsonl (optional)
/// - test-results.jsonl (optional)
/// - cache.db (ignored - rebuilt after import)
fn system_store_import_from_folder(
    repo_path: &Path,
    folder_path: &Path,
    import_type: &str,
    dry_run: bool,
) -> Result<StoreImportResult> {
    // Validate required tasks.jsonl exists
    let tasks_file = folder_path.join("tasks.jsonl");
    if !tasks_file.exists() {
        return Err(Error::InvalidInput(format!(
            "Folder '{}' missing required tasks.jsonl",
            folder_path.display()
        )));
    }

    // Read JSONL files from folder
    let tasks_jsonl = Some(fs::read(&tasks_file)?);

    let commits_file = folder_path.join("commits.jsonl");
    let commits_jsonl = if commits_file.exists() {
        Some(fs::read(&commits_file)?)
    } else {
        None
    };

    let test_results_file = folder_path.join("test-results.jsonl");
    let test_results_jsonl = if test_results_file.exists() {
        Some(fs::read(&test_results_file)?)
    } else {
        None
    };

    // Check if already initialized for replace mode
    let storage = Storage::open(repo_path);
    let is_initialized = storage.is_ok();

    if import_type == "replace" && is_initialized {
        return Err(Error::InvalidInput(
            "Store already initialized. Use --type merge to append data, or reinitialize the repo."
                .to_string(),
        ));
    }

    // Initialize if needed
    let mut storage = if is_initialized {
        storage.unwrap()
    } else {
        // Auto-initialize for import
        Storage::init(repo_path)?
    };

    // Parse imported tasks
    let mut imported_tasks: Vec<Task> = Vec::new();
    if let Some(tasks_data) = tasks_jsonl {
        let tasks_str = String::from_utf8_lossy(&tasks_data);
        for line in tasks_str.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let task: Task = serde_json::from_str(line)?;
            imported_tasks.push(task);
        }
    }

    // Detect ID collisions and create remapping
    let mut id_remappings: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let existing_tasks = storage.list_tasks(None, None, None)?;
    let existing_ids: std::collections::HashSet<String> =
        existing_tasks.iter().map(|t| t.id.clone()).collect();

    for task in &imported_tasks {
        if existing_ids.contains(&task.id) {
            // Generate new ID using task title as seed
            let new_id = crate::storage::generate_id("bn", &task.title);
            id_remappings.insert(task.id.clone(), new_id);
        }
    }

    let collisions = id_remappings.len();
    let input_path_str = folder_path.display().to_string();

    // Count tests and commits for dry-run (without importing)
    let tests_count = test_results_jsonl
        .as_ref()
        .map(|data| {
            String::from_utf8_lossy(data)
                .lines()
                .filter(|l| !l.trim().is_empty())
                .count()
        })
        .unwrap_or(0);

    let commits_count = commits_jsonl
        .as_ref()
        .map(|data| {
            String::from_utf8_lossy(data)
                .lines()
                .filter(|l| !l.trim().is_empty())
                .count()
        })
        .unwrap_or(0);

    // If dry run, return early
    if dry_run {
        return Ok(StoreImportResult {
            imported: false,
            dry_run: true,
            input_path: input_path_str,
            import_type: import_type.to_string(),
            tasks_imported: imported_tasks.len(),
            tasks_skipped: 0,
            tests_imported: tests_count,
            commits_imported: commits_count,
            collisions,
            id_remappings,
        });
    }

    // Import tasks with remapping
    let mut tasks_imported = 0;
    let import_timestamp = Utc::now();

    for mut task in imported_tasks {
        // Remap task ID if needed
        if let Some(new_id) = id_remappings.get(&task.id) {
            task.id = new_id.clone();
        }

        // Remap dependencies
        let mut new_depends_on = Vec::new();
        for dep in &task.depends_on {
            if let Some(new_dep_id) = id_remappings.get(dep) {
                new_depends_on.push(new_dep_id.clone());
            } else {
                new_depends_on.push(dep.clone());
            }
        }
        task.depends_on = new_depends_on;

        // Set imported_on timestamp if merging
        if import_type == "merge" {
            task.imported_on = Some(import_timestamp);
        }

        // Create task
        storage.create_task(&task)?;
        tasks_imported += 1;
    }

    // Import commits (simple append, no ID remapping needed for now)
    let mut commits_imported = 0;
    if let Some(commits_data) = commits_jsonl {
        let storage_root = storage.root();
        let commits_file = storage_root.join("commits.jsonl");

        // Append to existing file
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&commits_file)?;

        file.write_all(&commits_data)?;

        // Count commits
        let commits_str = String::from_utf8_lossy(&commits_data);
        commits_imported = commits_str.lines().filter(|l| !l.trim().is_empty()).count();
    }

    // Import test results
    let mut tests_imported = 0;
    if let Some(test_data) = test_results_jsonl {
        let storage_root = storage.root();
        let test_file = storage_root.join("test-results.jsonl");

        // Append to existing file
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&test_file)?;

        file.write_all(&test_data)?;

        // Count tests
        let test_str = String::from_utf8_lossy(&test_data);
        tests_imported = test_str.lines().filter(|l| !l.trim().is_empty()).count();
    }

    // Rebuild cache
    storage.rebuild_cache()?;

    Ok(StoreImportResult {
        imported: true,
        dry_run: false,
        input_path: input_path_str,
        import_type: import_type.to_string(),
        tasks_imported,
        tasks_skipped: 0,
        tests_imported,
        commits_imported,
        collisions,
        id_remappings,
    })
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
        let result = init_with_options(temp.path(), false, false, false).unwrap();
        assert!(result.initialized);
    }

    #[test]
    fn test_init_existing() {
        let temp = TempDir::new().unwrap();
        Storage::init(temp.path()).unwrap();
        let result = init_with_options(temp.path(), false, false, false).unwrap();
        assert!(!result.initialized);
    }

    #[test]
    fn test_task_create() {
        let temp = setup();
        let result = task_create(
            temp.path(),
            "Test task".to_string(),
            None, // short_name
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
            task_create(temp.path(), "Test".to_string(), None, None, None, vec![], None).unwrap();
        let result = task_show(temp.path(), &created.id).unwrap();
        assert_eq!(result.task.id, created.id);
        assert!(result.blocking_info.is_none()); // No dependencies
    }

    #[test]
    fn test_task_list() {
        let temp = setup();
        task_create(
            temp.path(),
            "Task 1".to_string(),
            None, // short_name
            None,
            Some(1),
            vec![],
            None,
        )
        .unwrap();
        task_create(
            temp.path(),
            "Task 2".to_string(),
            None, // short_name
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
            None, // short_name
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
            None, // short_name
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

        let result = task_show(temp.path(), &created.id).unwrap();
        assert_eq!(result.task.title, "Updated");
        assert_eq!(result.task.priority, 1);
    }

    #[test]
    fn test_task_close_reopen() {
        let temp = setup();
        let created =
            task_create(temp.path(), "Test".to_string(), None, None, None, vec![], None).unwrap();

        task_close(temp.path(), &created.id, Some("Done".to_string()), false).unwrap();
        let result = task_show(temp.path(), &created.id).unwrap();
        assert_eq!(result.task.status, TaskStatus::Done);
        assert!(result.task.closed_at.is_some());

        task_reopen(temp.path(), &created.id).unwrap();
        let result = task_show(temp.path(), &created.id).unwrap();
        assert_eq!(result.task.status, TaskStatus::Reopened);
        assert!(result.task.closed_at.is_none());
    }

    #[test]
    fn test_task_close_with_incomplete_deps_fails() {
        let temp = setup();
        let task_a =
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();
        let task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, None, vec![], None).unwrap();

        // B depends on A
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        // Try to close B without force (A is still pending)
        let result = task_close(temp.path(), &task_b.id, None, false);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("incomplete dependencies"));
        assert!(err_msg.contains(&task_a.id));
    }

    #[test]
    fn test_task_close_with_incomplete_deps_force() {
        let temp = setup();
        let task_a =
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();
        let task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, None, vec![], None).unwrap();

        // B depends on A
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        // Close B with force (A is still pending)
        let result = task_close(temp.path(), &task_b.id, None, true).unwrap();
        assert_eq!(result.status, "done");
        assert!(result.warning.is_some());
        assert!(result.warning.unwrap().contains("incomplete dependencies"));

        // Verify task is actually closed
        let result = task_show(temp.path(), &task_b.id).unwrap();
        assert_eq!(result.task.status, TaskStatus::Done);
    }

    #[test]
    fn test_task_close_with_complete_deps_success() {
        let temp = setup();
        let task_a =
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();
        let task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, None, vec![], None).unwrap();

        // B depends on A
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        // Close A first
        task_close(temp.path(), &task_a.id, None, false).unwrap();

        // Now close B (all deps are complete)
        let result = task_close(temp.path(), &task_b.id, None, false).unwrap();
        assert_eq!(result.status, "done");
        assert!(result.warning.is_none());

        // Verify task is closed
        let result = task_show(temp.path(), &task_b.id).unwrap();
        assert_eq!(result.task.status, TaskStatus::Done);
    }

    #[test]
    fn test_task_close_promotes_partial_dependents() {
        let temp = setup();
        let task_a =
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();
        let task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, None, vec![], None).unwrap();

        task_close(temp.path(), &task_b.id, Some("Done".to_string()), false).unwrap();
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        let result = task_show(temp.path(), &task_b.id).unwrap();
        assert_eq!(result.task.status, TaskStatus::Partial);
        assert!(result.task.closed_at.is_none());
        assert!(result.task.closed_reason.is_none());

        task_close(temp.path(), &task_a.id, Some("Done".to_string()), false).unwrap();

        let result = task_show(temp.path(), &task_b.id).unwrap();
        assert_eq!(result.task.status, TaskStatus::Done);
        assert!(result.task.closed_at.is_some());
    }

    #[test]
    fn test_task_delete() {
        let temp = setup();
        let created =
            task_create(temp.path(), "Test".to_string(), None, None, None, vec![], None).unwrap();

        task_delete(temp.path(), &created.id).unwrap();
        let list = task_list(temp.path(), None, None, None).unwrap();
        assert_eq!(list.count, 0);
    }

    // === Dependency Command Tests ===

    #[test]
    fn test_dep_add() {
        let temp = setup();
        let task_a =
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();
        let task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, None, vec![], None).unwrap();

        let result = dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();
        assert_eq!(result.child, task_b.id);
        assert_eq!(result.parent, task_a.id);

        // Verify task B now depends on A
        let result = task_show(temp.path(), &task_b.id).unwrap();
        assert!(result.task.depends_on.contains(&task_a.id));
    }

    #[test]
    fn test_dep_add_transitions_done_to_partial() {
        let temp = setup();
        let task_a =
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();
        let task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, None, vec![], None).unwrap();

        task_close(temp.path(), &task_b.id, Some("Done".to_string()), false).unwrap();
        let result = task_show(temp.path(), &task_b.id).unwrap();
        assert_eq!(result.task.status, TaskStatus::Done);
        assert!(result.task.closed_at.is_some());

        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        let result = task_show(temp.path(), &task_b.id).unwrap();
        assert_eq!(result.task.status, TaskStatus::Partial);
        assert!(result.task.closed_at.is_none());
        assert!(result.task.closed_reason.is_none());
    }

    #[test]
    fn test_dep_add_cycle_rejected() {
        let temp = setup();
        let task_a =
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();
        let task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, None, vec![], None).unwrap();

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
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();
        let task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, None, vec![], None).unwrap();

        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();
        dep_rm(temp.path(), &task_b.id, &task_a.id).unwrap();

        // Verify task B no longer depends on A
        let result = task_show(temp.path(), &task_b.id).unwrap();
        assert!(!result.task.depends_on.contains(&task_a.id));
    }

    #[test]
    fn test_dep_show() {
        let temp = setup();
        let task_a =
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();
        let task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, None, vec![], None).unwrap();
        let task_c =
            task_create(temp.path(), "Task C".to_string(), None, None, None, vec![], None).unwrap();

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
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();
        let task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, None, vec![], None).unwrap();

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
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();
        let task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, None, vec![], None).unwrap();

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
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();
        let task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, None, vec![], None).unwrap();

        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        // Initially B is blocked
        let blocked_result = blocked(temp.path()).unwrap();
        assert_eq!(blocked_result.count, 1);

        // Close task A
        task_close(temp.path(), &task_a.id, None, false).unwrap();

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
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();

        let result = commit_link(temp.path(), "a1b2c3d", &task.id).unwrap();
        assert_eq!(result.sha, "a1b2c3d");
        assert_eq!(result.task_id, task.id);
    }

    #[test]
    fn test_commit_link_invalid_sha() {
        let temp = setup();
        let task =
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();

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
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();

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
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();

        let result = commit_unlink(temp.path(), "a1b2c3d", &task.id);
        assert!(result.is_err());
    }

    #[test]
    fn test_commit_list() {
        let temp = setup();
        let task =
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();

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
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();

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
        task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();

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
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();
        let task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, None, vec![], None).unwrap();

        // B depends on A
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        // Close B (which depends on A, but A is still pending) - use force to allow this
        task_close(temp.path(), &task_b.id, None, true).unwrap();

        let result = doctor(temp.path()).unwrap();
        // We should find the consistency warning (done task with pending dependency)
        assert!(!result.healthy);
        assert!(!result.issues.is_empty());
        assert!(result.issues.iter().any(|i| i.category == "consistency"));
    }

    #[test]
    fn test_doctor_stats() {
        let temp = setup();
        task_create(temp.path(), "Task 1".to_string(), None, None, None, vec![], None).unwrap();
        task_create(temp.path(), "Task 2".to_string(), None, None, None, vec![], None).unwrap();
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
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();

        let result = log(temp.path(), None).unwrap();
        assert!(result.count >= 1);
        assert!(result.entries.iter().any(|e| e.entity_id == task.id));
    }

    #[test]
    fn test_log_filter_by_task() {
        let temp = setup();
        let task_a =
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();
        let _task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, None, vec![], None).unwrap();

        let result = log(temp.path(), Some(&task_a.id)).unwrap();
        assert!(result.entries.iter().all(|e| e.entity_id == task_a.id));
        assert_eq!(result.filtered_by, Some(task_a.id.clone()));
    }

    #[test]
    fn test_log_includes_updates() {
        let temp = setup();
        let task =
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();

        // Update the task
        task_update(
            temp.path(),
            &task.id,
            Some("Updated Title".to_string()),
            None, // short_name
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
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();

        task_close(temp.path(), &task.id, Some("Complete".to_string()), false).unwrap();

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
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();
        task_update(
            temp.path(),
            &task.id,
            Some("Updated 1".to_string()),
            None, // short_name
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
            None, // short_name
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
        let result = task_show(temp.path(), &task.id).unwrap();
        assert_eq!(result.task.title, "Updated 2");
    }

    #[test]
    fn test_compact_preserves_all_tasks() {
        let temp = setup();

        task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();
        task_create(temp.path(), "Task B".to_string(), None, None, None, vec![], None).unwrap();
        task_create(temp.path(), "Task C".to_string(), None, None, None, vec![], None).unwrap();

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
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();
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

        // Run init with AGENTS.md update enabled
        let result = init_with_options(temp.path(), true, false, false).unwrap();
        assert!(result.initialized);
        assert!(result.agents_md_updated);
        assert!(!result.skills_file_created);

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

        // Run init with AGENTS.md update enabled
        let result = init_with_options(temp.path(), true, false, false).unwrap();
        assert!(result.initialized);
        assert!(result.agents_md_updated);

        // Verify content was appended
        let contents = std::fs::read_to_string(&agents_path).unwrap();
        assert!(contents.contains("My Existing Agents"));
        assert!(contents.contains("bn orient"));
    }

    #[test]
    fn test_init_appends_section_if_legacy_bn_orient() {
        let temp = TempDir::new().unwrap();
        let agents_path = temp.path().join("AGENTS.md");

        // Create existing AGENTS.md that references bn orient but lacks markers
        std::fs::write(
            &agents_path,
            "# Agents\n\nRun `bn orient` to get started.\n",
        )
        .unwrap();

        // Run init with AGENTS.md update enabled
        let result = init_with_options(temp.path(), true, false, false).unwrap();
        assert!(result.initialized);
        assert!(result.agents_md_updated); // Should be updated to add markers

        // Verify markers were added and original content preserved
        let contents = std::fs::read_to_string(&agents_path).unwrap();
        assert!(contents.contains("# Agents")); // Original content preserved
        assert!(contents.contains("<!-- BEGIN BINNACLE SECTION -->")); // Markers added
        assert!(contents.contains("<!-- END BINNACLE SECTION -->"));
    }

    #[test]
    fn test_init_idempotent_agents_md() {
        let temp = TempDir::new().unwrap();

        // Run init twice with AGENTS.md enabled
        init_with_options(temp.path(), true, false, false).unwrap();
        let result = init_with_options(temp.path(), true, false, false).unwrap();

        // Second run should not update AGENTS.md (content unchanged)
        assert!(!result.initialized); // binnacle already exists
        assert!(!result.agents_md_updated); // AGENTS.md content unchanged
    }

    #[test]
    fn test_init_no_change_when_standard_blurb_already_present() {
        let temp = TempDir::new().unwrap();
        let agents_path = temp.path().join("AGENTS.md");

        // Pre-create AGENTS.md with the exact standard content (with trailing newline)
        std::fs::write(&agents_path, format!("{}\n", AGENTS_MD_BLURB.trim_end())).unwrap();

        // Initialize binnacle storage (without AGENTS.md update first)
        Storage::init(temp.path()).unwrap();

        // Now run init with AGENTS.md update - should detect no change needed
        let result = init_with_options(temp.path(), true, false, false).unwrap();
        assert!(!result.initialized); // binnacle already exists
        assert!(!result.agents_md_updated); // Content already matches exactly

        // Verify file wasn't modified (content identical)
        let contents = std::fs::read_to_string(&agents_path).unwrap();
        assert_eq!(contents, format!("{}\n", AGENTS_MD_BLURB.trim_end()));
    }

    #[test]
    fn test_init_replaces_custom_binnacle_section() {
        let temp = TempDir::new().unwrap();
        let agents_path = temp.path().join("AGENTS.md");

        // Create existing AGENTS.md with custom binnacle section
        std::fs::write(
            &agents_path,
            "# Agents\n\n<!-- BEGIN BINNACLE SECTION -->\nCustom content\n<!-- END BINNACLE SECTION -->\n",
        )
        .unwrap();

        // Run init with AGENTS.md update enabled
        let result = init_with_options(temp.path(), true, false, false).unwrap();
        assert!(result.initialized);
        assert!(result.agents_md_updated); // Section was replaced with standard content

        // Verify section was replaced with standard content
        let contents = std::fs::read_to_string(&agents_path).unwrap();
        assert!(contents.contains("# Agents")); // User content preserved
        assert!(contents.contains("bn orient")); // Standard section added
        assert!(!contents.contains("Custom content")); // Custom content replaced
        assert_eq!(contents.matches("BEGIN BINNACLE SECTION").count(), 1);
    }

    #[test]
    fn test_agents_md_has_html_markers() {
        let temp = TempDir::new().unwrap();
        let agents_path = temp.path().join("AGENTS.md");

        // Run init with AGENTS.md update enabled
        init_with_options(temp.path(), true, false, false).unwrap();

        // Verify AGENTS.md contains HTML markers
        let contents = std::fs::read_to_string(&agents_path).unwrap();
        assert!(contents.contains("<!-- BEGIN BINNACLE SECTION -->"));
        assert!(contents.contains("<!-- END BINNACLE SECTION -->"));
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
        task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();
        task_create(temp.path(), "Task B".to_string(), None, None, None, vec![], None).unwrap();

        let result = orient(temp.path()).unwrap();
        assert!(!result.initialized); // Already initialized in setup()
        assert_eq!(result.total_tasks, 2);
        assert_eq!(result.ready_count, 2); // Both pending tasks are ready
    }

    #[test]
    fn test_orient_shows_blocked_tasks() {
        let temp = setup();

        let task_a =
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();
        let task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, None, vec![], None).unwrap();

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
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();

        // Update to in_progress
        task_update(
            temp.path(),
            &task.id,
            None,
            None, // short_name
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
        task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();

        let result = orient(temp.path()).unwrap();
        let human = result.to_human();

        assert!(human.contains("Binnacle - AI agent task tracker"));
        assert!(human.contains("Total tasks: 1"));
        assert!(human.contains("bn ready"));
        assert!(human.contains("bn task list"));
    }

    // === Blocker Analysis Tests ===

    #[test]
    fn test_task_show_no_dependencies_no_blocking_info() {
        let temp = setup();
        let task = task_create(
            temp.path(),
            "Solo Task".to_string(),
            None, // short_name
            None,
            None,
            vec![],
            None,
        )
        .unwrap();

        let result = task_show(temp.path(), &task.id).unwrap();

        // No dependencies means no blocking info
        assert!(result.blocking_info.is_none());
    }

    #[test]
    fn test_task_show_all_dependencies_complete() {
        let temp = setup();
        let task_a =
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();
        let task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, None, vec![], None).unwrap();

        // Close both dependencies
        task_close(temp.path(), &task_a.id, Some("Done".to_string()), false).unwrap();
        task_close(temp.path(), &task_b.id, Some("Done".to_string()), false).unwrap();

        // Create task C that depends on A and B
        let task_c =
            task_create(temp.path(), "Task C".to_string(), None, None, None, vec![], None).unwrap();
        dep_add(temp.path(), &task_c.id, &task_a.id).unwrap();
        dep_add(temp.path(), &task_c.id, &task_b.id).unwrap();

        let result = task_show(temp.path(), &task_c.id).unwrap();

        // Has dependencies but all are complete
        assert!(result.blocking_info.is_some());
        let blocking = result.blocking_info.unwrap();
        assert!(!blocking.is_blocked);
        assert_eq!(blocking.blocker_count, 0);
        assert_eq!(blocking.direct_blockers.len(), 0);
        assert!(blocking.summary.contains("All dependencies complete"));
    }

    #[test]
    fn test_task_show_direct_blockers() {
        let temp = setup();
        let task_a =
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None, // short_name
            None,
            None,
            vec![],
            Some("agent-1".to_string()),
        )
        .unwrap();
        let task_c =
            task_create(temp.path(), "Task C".to_string(), None, None, None, vec![], None).unwrap();

        // Set task A to in_progress
        task_update(
            temp.path(),
            &task_a.id,
            None,
            None, // short_name
            None,
            None,
            Some("in_progress"),
            vec![],
            vec![],
            None,
        )
        .unwrap();

        // Set task B to pending (default)
        // Task C depends on A and B
        dep_add(temp.path(), &task_c.id, &task_a.id).unwrap();
        dep_add(temp.path(), &task_c.id, &task_b.id).unwrap();

        let result = task_show(temp.path(), &task_c.id).unwrap();

        assert!(result.blocking_info.is_some());
        let blocking = result.blocking_info.unwrap();
        assert!(blocking.is_blocked);
        assert_eq!(blocking.blocker_count, 2);
        assert_eq!(blocking.direct_blockers.len(), 2);

        // Check direct blocker details
        let blocker_a = blocking
            .direct_blockers
            .iter()
            .find(|b| b.id == task_a.id)
            .unwrap();
        assert_eq!(blocker_a.title, "Task A");
        assert_eq!(blocker_a.status, "inprogress");
        assert!(blocker_a.assignee.is_none());
        assert_eq!(blocker_a.blocked_by.len(), 0);

        let blocker_b = blocking
            .direct_blockers
            .iter()
            .find(|b| b.id == task_b.id)
            .unwrap();
        assert_eq!(blocker_b.title, "Task B");
        assert_eq!(blocker_b.status, "pending");
        assert_eq!(blocker_b.assignee, Some("agent-1".to_string()));
        assert_eq!(blocker_b.blocked_by.len(), 0);
    }

    #[test]
    fn test_task_show_transitive_blockers() {
        let temp = setup();
        let task_a =
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();
        let task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, None, vec![], None).unwrap();
        let task_c =
            task_create(temp.path(), "Task C".to_string(), None, None, None, vec![], None).unwrap();

        // Create chain: C depends on B, B depends on A
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();
        dep_add(temp.path(), &task_c.id, &task_b.id).unwrap();

        let result = task_show(temp.path(), &task_c.id).unwrap();

        assert!(result.blocking_info.is_some());
        let blocking = result.blocking_info.unwrap();
        assert!(blocking.is_blocked);
        assert_eq!(blocking.blocker_count, 1);
        assert_eq!(blocking.direct_blockers.len(), 1);

        // Task B is the direct blocker
        let blocker_b = &blocking.direct_blockers[0];
        assert_eq!(blocker_b.id, task_b.id);
        assert_eq!(blocker_b.title, "Task B");
        assert_eq!(blocker_b.status, "pending");

        // Task B is itself blocked by A
        assert_eq!(blocker_b.blocked_by.len(), 1);
        assert!(blocker_b.blocked_by.contains(&task_a.id));

        // Check blocker chain format
        assert_eq!(blocking.blocker_chain.len(), 1);
        assert!(blocking.blocker_chain[0].contains(&task_c.id));
        assert!(blocking.blocker_chain[0].contains(&task_b.id));
        assert!(blocking.blocker_chain[0].contains(&task_a.id));
    }

    #[test]
    fn test_task_show_mixed_complete_incomplete_deps() {
        let temp = setup();
        let task_a =
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();
        let task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, None, vec![], None).unwrap();
        let task_c =
            task_create(temp.path(), "Task C".to_string(), None, None, None, vec![], None).unwrap();

        // Close task A
        task_close(temp.path(), &task_a.id, Some("Done".to_string()), false).unwrap();

        // Task C depends on both A (done) and B (pending)
        dep_add(temp.path(), &task_c.id, &task_a.id).unwrap();
        dep_add(temp.path(), &task_c.id, &task_b.id).unwrap();

        let result = task_show(temp.path(), &task_c.id).unwrap();

        assert!(result.blocking_info.is_some());
        let blocking = result.blocking_info.unwrap();
        assert!(blocking.is_blocked);
        assert_eq!(blocking.blocker_count, 1); // Only task B is blocking

        // Only task B should appear as a blocker
        assert_eq!(blocking.direct_blockers.len(), 1);
        assert_eq!(blocking.direct_blockers[0].id, task_b.id);
    }

    #[test]
    fn test_task_show_blocker_summary_format() {
        let temp = setup();
        let task_a =
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();
        let task_b = task_create(
            temp.path(),
            "Task B".to_string(),
            None, // short_name
            None,
            None,
            vec![],
            Some("alice".to_string()),
        )
        .unwrap();
        let task_c =
            task_create(temp.path(), "Task C".to_string(), None, None, None, vec![], None).unwrap();

        // Task C depends on A and B
        dep_add(temp.path(), &task_c.id, &task_a.id).unwrap();
        dep_add(temp.path(), &task_c.id, &task_b.id).unwrap();

        let result = task_show(temp.path(), &task_c.id).unwrap();
        let blocking = result.blocking_info.unwrap();

        // Check summary format
        assert!(blocking
            .summary
            .contains("Blocked by 2 incomplete dependencies"));
        assert!(blocking.summary.contains(&task_a.id));
        assert!(blocking.summary.contains("pending"));
        assert!(blocking.summary.contains(&task_b.id));
        assert!(blocking.summary.contains("alice"));
    }

    #[test]
    fn test_task_show_cancelled_dependencies_dont_block() {
        let temp = setup();
        let task_a =
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();
        let task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, None, vec![], None).unwrap();
        let task_c =
            task_create(temp.path(), "Task C".to_string(), None, None, None, vec![], None).unwrap();

        // Cancel task A
        task_update(
            temp.path(),
            &task_a.id,
            None,
            None, // short_name
            None,
            None,
            Some("cancelled"),
            vec![],
            vec![],
            None,
        )
        .unwrap();

        // Close task B normally
        task_close(temp.path(), &task_b.id, Some("Done".to_string()), false).unwrap();

        // Task C depends on both A (cancelled) and B (done)
        dep_add(temp.path(), &task_c.id, &task_a.id).unwrap();
        dep_add(temp.path(), &task_c.id, &task_b.id).unwrap();

        let result = task_show(temp.path(), &task_c.id).unwrap();

        assert!(result.blocking_info.is_some());
        let blocking = result.blocking_info.unwrap();
        // Neither cancelled nor done should block
        assert!(!blocking.is_blocked);
        assert_eq!(blocking.blocker_count, 0);
        assert_eq!(blocking.direct_blockers.len(), 0);
        assert!(blocking.summary.contains("All dependencies complete"));
    }

    #[test]
    fn test_task_show_blocked_status_is_blocker() {
        let temp = setup();
        let task_a =
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();
        let task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, None, vec![], None).unwrap();

        // Set task A to blocked status
        task_update(
            temp.path(),
            &task_a.id,
            None,
            None, // short_name
            None,
            None,
            Some("blocked"),
            vec![],
            vec![],
            None,
        )
        .unwrap();

        // Task B depends on A (blocked)
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        let result = task_show(temp.path(), &task_b.id).unwrap();

        assert!(result.blocking_info.is_some());
        let blocking = result.blocking_info.unwrap();
        assert!(blocking.is_blocked);
        assert_eq!(blocking.blocker_count, 1);
        assert_eq!(blocking.direct_blockers[0].id, task_a.id);
        assert_eq!(blocking.direct_blockers[0].status, "blocked");
    }

    #[test]
    fn test_task_show_partial_status_is_blocker() {
        let temp = setup();
        let task_a =
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();
        let task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, None, vec![], None).unwrap();

        // Set task A to partial status
        task_update(
            temp.path(),
            &task_a.id,
            None,
            None, // short_name
            None,
            None,
            Some("partial"),
            vec![],
            vec![],
            None,
        )
        .unwrap();

        // Task B depends on A (partial)
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        let result = task_show(temp.path(), &task_b.id).unwrap();

        assert!(result.blocking_info.is_some());
        let blocking = result.blocking_info.unwrap();
        assert!(blocking.is_blocked);
        assert_eq!(blocking.blocker_count, 1);
        assert_eq!(blocking.direct_blockers[0].id, task_a.id);
        assert_eq!(blocking.direct_blockers[0].status, "partial");
    }

    #[test]
    fn test_task_show_reopened_status_is_blocker() {
        let temp = setup();
        let task_a =
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();
        let task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, None, vec![], None).unwrap();

        // Close then reopen task A
        task_close(temp.path(), &task_a.id, Some("Done".to_string()), false).unwrap();
        task_reopen(temp.path(), &task_a.id).unwrap();

        // Task B depends on A (reopened)
        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();

        let result = task_show(temp.path(), &task_b.id).unwrap();

        assert!(result.blocking_info.is_some());
        let blocking = result.blocking_info.unwrap();
        assert!(blocking.is_blocked);
        assert_eq!(blocking.blocker_count, 1);
        assert_eq!(blocking.direct_blockers[0].id, task_a.id);
        assert_eq!(blocking.direct_blockers[0].status, "reopened");
    }

    #[test]
    fn test_task_show_deep_transitive_blocker_chain() {
        let temp = setup();
        // Create chain: D -> C -> B -> A
        let task_a =
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();
        let task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, None, vec![], None).unwrap();
        let task_c =
            task_create(temp.path(), "Task C".to_string(), None, None, None, vec![], None).unwrap();
        let task_d =
            task_create(temp.path(), "Task D".to_string(), None, None, None, vec![], None).unwrap();

        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();
        dep_add(temp.path(), &task_c.id, &task_b.id).unwrap();
        dep_add(temp.path(), &task_d.id, &task_c.id).unwrap();

        let result = task_show(temp.path(), &task_d.id).unwrap();

        assert!(result.blocking_info.is_some());
        let blocking = result.blocking_info.unwrap();
        assert!(blocking.is_blocked);
        assert_eq!(blocking.blocker_count, 1);

        // Task C is the direct blocker of D
        let blocker_c = &blocking.direct_blockers[0];
        assert_eq!(blocker_c.id, task_c.id);

        // Task C is blocked by B (transitive)
        assert_eq!(blocker_c.blocked_by.len(), 1);
        assert!(blocker_c.blocked_by.contains(&task_b.id));

        // Blocker chain should show the path
        assert_eq!(blocking.blocker_chain.len(), 1);
        assert!(blocking.blocker_chain[0].contains(&task_d.id));
        assert!(blocking.blocker_chain[0].contains(&task_c.id));
        assert!(blocking.blocker_chain[0].contains(&task_b.id));
    }

    #[test]
    fn test_task_show_multiple_transitive_blockers() {
        let temp = setup();
        // Create diamond: D depends on B and C, both B and C depend on A
        let task_a =
            task_create(temp.path(), "Task A".to_string(), None, None, None, vec![], None).unwrap();
        let task_b =
            task_create(temp.path(), "Task B".to_string(), None, None, None, vec![], None).unwrap();
        let task_c =
            task_create(temp.path(), "Task C".to_string(), None, None, None, vec![], None).unwrap();
        let task_d =
            task_create(temp.path(), "Task D".to_string(), None, None, None, vec![], None).unwrap();

        dep_add(temp.path(), &task_b.id, &task_a.id).unwrap();
        dep_add(temp.path(), &task_c.id, &task_a.id).unwrap();
        dep_add(temp.path(), &task_d.id, &task_b.id).unwrap();
        dep_add(temp.path(), &task_d.id, &task_c.id).unwrap();

        let result = task_show(temp.path(), &task_d.id).unwrap();

        assert!(result.blocking_info.is_some());
        let blocking = result.blocking_info.unwrap();
        assert!(blocking.is_blocked);
        assert_eq!(blocking.blocker_count, 2); // Both B and C block D

        // Both B and C should be blocked by A
        for blocker in &blocking.direct_blockers {
            assert_eq!(blocker.blocked_by.len(), 1);
            assert!(blocker.blocked_by.contains(&task_a.id));
        }

        // Should have two chains in the blocker chain
        assert_eq!(blocking.blocker_chain.len(), 2);
    }

    #[test]
    fn test_build_blocker_summary_single() {
        let blockers = vec![DirectBlocker {
            id: "bn-test1".to_string(),
            title: "Test Task".to_string(),
            status: "pending".to_string(),
            assignee: None,
            blocked_by: vec![],
        }];

        let summary = build_blocker_summary(&blockers, 1);

        assert!(summary.contains("Blocked by 1 incomplete dependencies"));
        assert!(summary.contains("bn-test1"));
        assert!(summary.contains("pending"));
    }

    #[test]
    fn test_build_blocker_summary_multiple_with_assignees() {
        let blockers = vec![
            DirectBlocker {
                id: "bn-test1".to_string(),
                title: "Test Task 1".to_string(),
                status: "in_progress".to_string(),
                assignee: Some("alice".to_string()),
                blocked_by: vec![],
            },
            DirectBlocker {
                id: "bn-test2".to_string(),
                title: "Test Task 2".to_string(),
                status: "blocked".to_string(),
                assignee: Some("bob".to_string()),
                blocked_by: vec!["bn-test3".to_string()],
            },
        ];

        let summary = build_blocker_summary(&blockers, 2);

        assert!(summary.contains("Blocked by 2 incomplete dependencies"));
        assert!(summary.contains("bn-test1"));
        assert!(summary.contains("in_progress"));
        assert!(summary.contains("alice"));
        assert!(summary.contains("bn-test2"));
        assert!(summary.contains("blocked"));
        assert!(summary.contains("bob"));
        assert!(summary.contains("bn-test3"));
    }

    #[test]
    fn test_build_blocker_summary_no_assignee() {
        let blockers = vec![DirectBlocker {
            id: "bn-test1".to_string(),
            title: "Test Task".to_string(),
            status: "pending".to_string(),
            assignee: None,
            blocked_by: vec![],
        }];

        let summary = build_blocker_summary(&blockers, 1);

        assert!(summary.contains("bn-test1 is pending"));
        assert!(!summary.contains("assigned"));
    }

    #[test]
    fn test_build_blocker_summary_with_transitive_blockers() {
        let blockers = vec![DirectBlocker {
            id: "bn-test1".to_string(),
            title: "Test Task".to_string(),
            status: "pending".to_string(),
            assignee: None,
            blocked_by: vec!["bn-test2".to_string(), "bn-test3".to_string()],
        }];

        let summary = build_blocker_summary(&blockers, 1);

        assert!(summary.contains("bn-test1"));
        assert!(summary.contains("blocked by bn-test2, bn-test3"));
    }

    // === Bug Command Tests ===

    #[test]
    fn test_bug_create() {
        let temp = setup();
        let result = bug_create(
            temp.path(),
            "Test bug".to_string(),
            Some("Description".to_string()),
            Some(1),
            Some("high".to_string()),
            vec!["frontend".to_string()],
            Some("alice".to_string()),
            Some("1. Open app\n2. Click button".to_string()),
            Some("ui".to_string()),
        )
        .unwrap();
        assert!(result.id.starts_with("bn-"));
        assert_eq!(result.title, "Test bug");
    }

    #[test]
    fn test_bug_create_defaults() {
        let temp = setup();
        let result = bug_create(
            temp.path(),
            "Minimal bug".to_string(),
            None,
            None,
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        let bug = bug_show(temp.path(), &result.id).unwrap();
        assert_eq!(bug.priority, 2); // default priority
        assert_eq!(bug.severity, BugSeverity::Triage); // default severity
        assert!(bug.description.is_none());
        assert!(bug.tags.is_empty());
    }

    #[test]
    fn test_bug_create_invalid_priority() {
        let temp = setup();
        let result = bug_create(
            temp.path(),
            "Bad priority".to_string(),
            None,
            Some(5), // invalid: must be 0-4
            None,
            vec![],
            None,
            None,
            None,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Priority must be 0-4"));
    }

    #[test]
    fn test_bug_show() {
        let temp = setup();
        let created = bug_create(
            temp.path(),
            "Test bug".to_string(),
            Some("Bug description".to_string()),
            Some(1),
            Some("critical".to_string()),
            vec!["security".to_string()],
            None,
            None,
            None,
        )
        .unwrap();

        let bug = bug_show(temp.path(), &created.id).unwrap();
        assert_eq!(bug.id, created.id);
        assert_eq!(bug.title, "Test bug");
        assert_eq!(bug.description, Some("Bug description".to_string()));
        assert_eq!(bug.priority, 1);
        assert_eq!(bug.severity, BugSeverity::Critical);
        assert!(bug.tags.contains(&"security".to_string()));
    }

    #[test]
    fn test_bug_show_not_found() {
        let temp = setup();
        let result = bug_show(temp.path(), "bn-nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_bug_list() {
        let temp = setup();
        bug_create(
            temp.path(),
            "Bug 1".to_string(),
            None,
            Some(1),
            Some("high".to_string()),
            vec![],
            None,
            None,
            None,
        )
        .unwrap();
        bug_create(
            temp.path(),
            "Bug 2".to_string(),
            None,
            Some(2),
            Some("low".to_string()),
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        let list = bug_list(temp.path(), None, None, None, None).unwrap();
        assert_eq!(list.count, 2);
    }

    #[test]
    fn test_bug_list_filter_by_status() {
        let temp = setup();
        let bug1 = bug_create(
            temp.path(),
            "Bug 1".to_string(),
            None,
            None,
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();
        bug_create(
            temp.path(),
            "Bug 2".to_string(),
            None,
            None,
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        // Close bug 1
        bug_close(temp.path(), &bug1.id, None, false).unwrap();

        let pending_list = bug_list(temp.path(), Some("pending"), None, None, None).unwrap();
        assert_eq!(pending_list.count, 1);

        let done_list = bug_list(temp.path(), Some("done"), None, None, None).unwrap();
        assert_eq!(done_list.count, 1);
    }

    #[test]
    fn test_bug_list_filter_by_priority() {
        let temp = setup();
        bug_create(
            temp.path(),
            "High priority".to_string(),
            None,
            Some(0),
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();
        bug_create(
            temp.path(),
            "Low priority".to_string(),
            None,
            Some(3),
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        let high_list = bug_list(temp.path(), None, Some(0), None, None).unwrap();
        assert_eq!(high_list.count, 1);
        assert_eq!(high_list.bugs[0].title, "High priority");
    }

    #[test]
    fn test_bug_list_filter_by_severity() {
        let temp = setup();
        bug_create(
            temp.path(),
            "Critical bug".to_string(),
            None,
            None,
            Some("critical".to_string()),
            vec![],
            None,
            None,
            None,
        )
        .unwrap();
        bug_create(
            temp.path(),
            "Low severity".to_string(),
            None,
            None,
            Some("low".to_string()),
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        let critical_list = bug_list(temp.path(), None, None, Some("critical"), None).unwrap();
        assert_eq!(critical_list.count, 1);
        assert_eq!(critical_list.bugs[0].title, "Critical bug");
    }

    #[test]
    fn test_bug_list_filter_by_tag() {
        let temp = setup();
        bug_create(
            temp.path(),
            "UI bug".to_string(),
            None,
            None,
            None,
            vec!["ui".to_string()],
            None,
            None,
            None,
        )
        .unwrap();
        bug_create(
            temp.path(),
            "API bug".to_string(),
            None,
            None,
            None,
            vec!["api".to_string()],
            None,
            None,
            None,
        )
        .unwrap();

        let ui_list = bug_list(temp.path(), None, None, None, Some("ui")).unwrap();
        assert_eq!(ui_list.count, 1);
        assert_eq!(ui_list.bugs[0].title, "UI bug");
    }

    #[test]
    fn test_bug_update() {
        let temp = setup();
        let created = bug_create(
            temp.path(),
            "Original bug".to_string(),
            None,
            None,
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        let updated = bug_update(
            temp.path(),
            &created.id,
            Some("Updated bug".to_string()),
            Some("New description".to_string()),
            Some(1),
            None,
            Some("high".to_string()),
            vec!["new-tag".to_string()],
            vec![],
            Some("bob".to_string()),
            Some("Steps to reproduce".to_string()),
            Some("backend".to_string()),
        )
        .unwrap();

        assert!(updated.updated_fields.contains(&"title".to_string()));
        assert!(updated.updated_fields.contains(&"description".to_string()));
        assert!(updated.updated_fields.contains(&"priority".to_string()));
        assert!(updated.updated_fields.contains(&"severity".to_string()));
        assert!(updated.updated_fields.contains(&"tags".to_string()));
        assert!(updated.updated_fields.contains(&"assignee".to_string()));
        assert!(updated.updated_fields.contains(&"reproduction_steps".to_string()));
        assert!(updated.updated_fields.contains(&"affected_component".to_string()));

        let bug = bug_show(temp.path(), &created.id).unwrap();
        assert_eq!(bug.title, "Updated bug");
        assert_eq!(bug.description, Some("New description".to_string()));
        assert_eq!(bug.priority, 1);
        assert_eq!(bug.severity, BugSeverity::High);
        assert!(bug.tags.contains(&"new-tag".to_string()));
        assert_eq!(bug.assignee, Some("bob".to_string()));
    }

    #[test]
    fn test_bug_update_status() {
        let temp = setup();
        let created = bug_create(
            temp.path(),
            "Bug".to_string(),
            None,
            None,
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        bug_update(
            temp.path(),
            &created.id,
            None,
            None,
            None,
            Some("in_progress"),
            None,
            vec![],
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        let bug = bug_show(temp.path(), &created.id).unwrap();
        assert_eq!(bug.status, TaskStatus::InProgress);
    }

    #[test]
    fn test_bug_update_add_remove_tags() {
        let temp = setup();
        let created = bug_create(
            temp.path(),
            "Bug".to_string(),
            None,
            None,
            None,
            vec!["old-tag".to_string()],
            None,
            None,
            None,
        )
        .unwrap();

        bug_update(
            temp.path(),
            &created.id,
            None,
            None,
            None,
            None,
            None,
            vec!["new-tag".to_string()],
            vec!["old-tag".to_string()],
            None,
            None,
            None,
        )
        .unwrap();

        let bug = bug_show(temp.path(), &created.id).unwrap();
        assert!(bug.tags.contains(&"new-tag".to_string()));
        assert!(!bug.tags.contains(&"old-tag".to_string()));
    }

    #[test]
    fn test_bug_update_no_fields_error() {
        let temp = setup();
        let created = bug_create(
            temp.path(),
            "Bug".to_string(),
            None,
            None,
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        let result = bug_update(
            temp.path(),
            &created.id,
            None,
            None,
            None,
            None,
            None,
            vec![],
            vec![],
            None,
            None,
            None,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No fields to update"));
    }

    #[test]
    fn test_bug_update_invalid_priority() {
        let temp = setup();
        let created = bug_create(
            temp.path(),
            "Bug".to_string(),
            None,
            None,
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        let result = bug_update(
            temp.path(),
            &created.id,
            None,
            None,
            Some(5), // invalid
            None,
            None,
            vec![],
            vec![],
            None,
            None,
            None,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Priority must be 0-4"));
    }

    #[test]
    fn test_bug_close() {
        let temp = setup();
        let created = bug_create(
            temp.path(),
            "Bug".to_string(),
            None,
            None,
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        let result = bug_close(temp.path(), &created.id, Some("Fixed".to_string()), false).unwrap();
        assert_eq!(result.id, created.id);
        assert_eq!(result.status, "done");
        assert!(result.warning.is_none());

        let bug = bug_show(temp.path(), &created.id).unwrap();
        assert_eq!(bug.status, TaskStatus::Done);
        assert!(bug.closed_at.is_some());
        assert_eq!(bug.closed_reason, Some("Fixed".to_string()));
    }

    #[test]
    fn test_bug_reopen() {
        let temp = setup();
        let created = bug_create(
            temp.path(),
            "Bug".to_string(),
            None,
            None,
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        bug_close(temp.path(), &created.id, Some("Fixed".to_string()), false).unwrap();
        let result = bug_reopen(temp.path(), &created.id).unwrap();
        assert_eq!(result.id, created.id);
        assert_eq!(result.status, "reopened");

        let bug = bug_show(temp.path(), &created.id).unwrap();
        assert_eq!(bug.status, TaskStatus::Reopened);
        assert!(bug.closed_at.is_none());
        assert!(bug.closed_reason.is_none());
    }

    #[test]
    fn test_bug_delete() {
        let temp = setup();
        let created = bug_create(
            temp.path(),
            "Bug".to_string(),
            None,
            None,
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap();

        let result = bug_delete(temp.path(), &created.id).unwrap();
        assert_eq!(result.id, created.id);

        let list = bug_list(temp.path(), None, None, None, None).unwrap();
        assert_eq!(list.count, 0);
    }

    #[test]
    fn test_bug_severity_values() {
        let temp = setup();

        for (severity_str, expected) in [
            ("triage", BugSeverity::Triage),
            ("low", BugSeverity::Low),
            ("medium", BugSeverity::Medium),
            ("high", BugSeverity::High),
            ("critical", BugSeverity::Critical),
        ] {
            let result = bug_create(
                temp.path(),
                format!("Bug with {} severity", severity_str),
                None,
                None,
                Some(severity_str.to_string()),
                vec![],
                None,
                None,
                None,
            )
            .unwrap();

            let bug = bug_show(temp.path(), &result.id).unwrap();
            assert_eq!(bug.severity, expected);
        }
    }

    #[test]
    fn test_bug_output_human_format() {
        let temp = setup();
        let created = bug_create(
            temp.path(),
            "Test bug".to_string(),
            Some("Description".to_string()),
            Some(1),
            Some("high".to_string()),
            vec!["ui".to_string()],
            Some("alice".to_string()),
            Some("1. Click\n2. See error".to_string()),
            Some("frontend".to_string()),
        )
        .unwrap();

        let bug = bug_show(temp.path(), &created.id).unwrap();
        let human = bug.to_human();

        assert!(human.contains("Test bug"));
        assert!(human.contains("Status: Pending"));
        assert!(human.contains("Priority: 1"));
        assert!(human.contains("Severity: High"));
        assert!(human.contains("Description: Description"));
        assert!(human.contains("Reproduction steps: 1. Click"));
        assert!(human.contains("Affected component: frontend"));
        assert!(human.contains("Tags: ui"));
        assert!(human.contains("Assignee: alice"));
    }

    #[test]
    fn test_bug_list_output_human_format() {
        let temp = setup();
        bug_create(
            temp.path(),
            "Bug 1".to_string(),
            None,
            Some(0),
            Some("critical".to_string()),
            vec!["urgent".to_string()],
            None,
            None,
            None,
        )
        .unwrap();

        let list = bug_list(temp.path(), None, None, None, None).unwrap();
        let human = list.to_human();

        assert!(human.contains("1 bug(s)"));
        assert!(human.contains("P0"));
        assert!(human.contains("S:critical"));
        assert!(human.contains("Bug 1"));
        assert!(human.contains("[urgent]"));
    }

    #[test]
    fn test_bug_list_empty_output() {
        let temp = setup();
        let list = bug_list(temp.path(), None, None, None, None).unwrap();
        let human = list.to_human();
        assert_eq!(human, "No bugs found.");
    }
}
