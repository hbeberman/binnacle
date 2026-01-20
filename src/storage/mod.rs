//! Storage layer for Binnacle data.
//!
//! This module handles persistence of tasks, tests, and commit links.
//! Default storage uses:
//! - JSONL files for append-only data (tasks.jsonl, commits.jsonl, test-results.jsonl)
//! - SQLite for indexed queries (cache.db)
//!
//! Storage location: `~/.local/share/binnacle/<repo-hash>/`

use crate::models::{CommitLink, Task, TaskStatus, TestNode, TestResult};
use crate::{Error, Result};
use chrono::Utc;
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// Storage manager for a single repository.
pub struct Storage {
    /// Root directory for this repository's data
    pub root: PathBuf,
    /// SQLite connection for indexed queries
    conn: Connection,
}

impl Storage {
    /// Open or create storage for the given repository path.
    pub fn open(repo_path: &Path) -> Result<Self> {
        let root = get_storage_dir(repo_path)?;

        if !root.exists() {
            return Err(Error::NotInitialized);
        }

        let db_path = root.join("cache.db");
        let conn = Connection::open(&db_path)?;

        Ok(Self { root, conn })
    }

    /// Initialize storage for a new repository.
    pub fn init(repo_path: &Path) -> Result<Self> {
        let root = get_storage_dir(repo_path)?;

        // Create directory structure
        fs::create_dir_all(&root)?;

        // Create empty JSONL files
        let files = ["tasks.jsonl", "commits.jsonl", "test-results.jsonl"];
        for file in files {
            let path = root.join(file);
            if !path.exists() {
                File::create(&path)?;
            }
        }

        // Initialize SQLite cache
        let db_path = root.join("cache.db");
        let conn = Connection::open(&db_path)?;
        Self::init_schema(&conn)?;

        Ok(Self { root, conn })
    }

    /// Check if storage exists for the given repository.
    pub fn exists(repo_path: &Path) -> Result<bool> {
        let root = get_storage_dir(repo_path)?;
        Ok(root.exists() && root.join("cache.db").exists())
    }

    /// Initialize the SQLite schema.
    fn init_schema(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT,
                priority INTEGER NOT NULL DEFAULT 2,
                status TEXT NOT NULL DEFAULT 'pending',
                parent TEXT,
                assignee TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                closed_at TEXT,
                closed_reason TEXT
            );

            CREATE TABLE IF NOT EXISTS task_tags (
                task_id TEXT NOT NULL,
                tag TEXT NOT NULL,
                PRIMARY KEY (task_id, tag),
                FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS task_dependencies (
                child_id TEXT NOT NULL,
                parent_id TEXT NOT NULL,
                PRIMARY KEY (child_id, parent_id),
                FOREIGN KEY (child_id) REFERENCES tasks(id) ON DELETE CASCADE,
                FOREIGN KEY (parent_id) REFERENCES tasks(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
            CREATE INDEX IF NOT EXISTS idx_tasks_priority ON tasks(priority);
            CREATE INDEX IF NOT EXISTS idx_task_tags_tag ON task_tags(tag);

            -- Test node tables
            CREATE TABLE IF NOT EXISTS tests (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                command TEXT NOT NULL,
                working_dir TEXT NOT NULL DEFAULT '.',
                pattern TEXT,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS test_links (
                test_id TEXT NOT NULL,
                task_id TEXT NOT NULL,
                PRIMARY KEY (test_id, task_id),
                FOREIGN KEY (test_id) REFERENCES tests(id) ON DELETE CASCADE,
                FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS test_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                test_id TEXT NOT NULL,
                passed INTEGER NOT NULL,
                exit_code INTEGER NOT NULL,
                stdout TEXT,
                stderr TEXT,
                duration_ms INTEGER NOT NULL,
                executed_at TEXT NOT NULL,
                FOREIGN KEY (test_id) REFERENCES tests(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_test_links_task ON test_links(task_id);
            CREATE INDEX IF NOT EXISTS idx_test_results_test ON test_results(test_id);

            -- Commit link tables
            CREATE TABLE IF NOT EXISTS commit_links (
                sha TEXT NOT NULL,
                task_id TEXT NOT NULL,
                linked_at TEXT NOT NULL,
                PRIMARY KEY (sha, task_id),
                FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_commit_links_task ON commit_links(task_id);
            CREATE INDEX IF NOT EXISTS idx_commit_links_sha ON commit_links(sha);

            -- Configuration table
            CREATE TABLE IF NOT EXISTS config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            "#,
        )?;
        Ok(())
    }

    /// Rebuild the SQLite cache from JSONL files.
    pub fn rebuild_cache(&mut self) -> Result<()> {
        // Clear existing data
        self.conn.execute_batch(
            r#"
            DELETE FROM task_dependencies;
            DELETE FROM task_tags;
            DELETE FROM tasks;
            DELETE FROM test_links;
            DELETE FROM tests;
            "#,
        )?;

        // Re-read tasks from JSONL
        let tasks_path = self.root.join("tasks.jsonl");
        if tasks_path.exists() {
            let file = File::open(&tasks_path)?;
            let reader = BufReader::new(file);

            for line in reader.lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(task) = serde_json::from_str::<Task>(&line) {
                    self.cache_task(&task)?;
                }
            }
        }

        // Re-read tests from JSONL (tests.jsonl contains both tasks and tests)
        // Tests have entity_type = "test"
        let tasks_path = self.root.join("tasks.jsonl");
        if tasks_path.exists() {
            let file = File::open(&tasks_path)?;
            let reader = BufReader::new(file);

            for line in reader.lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(test) = serde_json::from_str::<TestNode>(&line) {
                    if test.entity_type == "test" {
                        self.cache_test(&test)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Cache a task in SQLite for fast querying.
    fn cache_task(&self, task: &Task) -> Result<()> {
        // Insert or replace task
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO tasks 
            (id, title, description, priority, status, parent, assignee, 
             created_at, updated_at, closed_at, closed_reason)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
            params![
                task.id,
                task.title,
                task.description,
                task.priority,
                serde_json::to_string(&task.status)?.trim_matches('"'),
                task.parent,
                task.assignee,
                task.created_at.to_rfc3339(),
                task.updated_at.to_rfc3339(),
                task.closed_at.map(|t| t.to_rfc3339()),
                task.closed_reason,
            ],
        )?;

        // Update tags
        self.conn
            .execute("DELETE FROM task_tags WHERE task_id = ?1", [&task.id])?;
        for tag in &task.tags {
            self.conn.execute(
                "INSERT INTO task_tags (task_id, tag) VALUES (?1, ?2)",
                params![task.id, tag],
            )?;
        }

        // Update dependencies
        self.conn.execute(
            "DELETE FROM task_dependencies WHERE child_id = ?1",
            [&task.id],
        )?;
        for parent_id in &task.depends_on {
            self.conn.execute(
                "INSERT INTO task_dependencies (child_id, parent_id) VALUES (?1, ?2)",
                params![task.id, parent_id],
            )?;
        }

        Ok(())
    }

    // === Task Operations ===

    /// Create a new task.
    pub fn create_task(&mut self, task: &Task) -> Result<()> {
        // Append to JSONL
        let tasks_path = self.root.join("tasks.jsonl");
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&tasks_path)?;

        let json = serde_json::to_string(task)?;
        writeln!(file, "{}", json)?;

        // Update cache
        self.cache_task(task)?;

        Ok(())
    }

    /// Get a task by ID.
    pub fn get_task(&self, id: &str) -> Result<Task> {
        // Read from JSONL to get the latest version
        let tasks_path = self.root.join("tasks.jsonl");
        let file = File::open(&tasks_path)?;
        let reader = BufReader::new(file);

        let mut latest: Option<Task> = None;
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(task) = serde_json::from_str::<Task>(&line) {
                if task.id == id {
                    latest = Some(task);
                }
            }
        }

        latest.ok_or_else(|| Error::NotFound(format!("Task not found: {}", id)))
    }

    /// List all tasks, optionally filtered.
    pub fn list_tasks(
        &self,
        status: Option<&str>,
        priority: Option<u8>,
        tag: Option<&str>,
    ) -> Result<Vec<Task>> {
        // Build query
        let mut sql = String::from(
            "SELECT DISTINCT t.id FROM tasks t 
             LEFT JOIN task_tags tt ON t.id = tt.task_id 
             WHERE 1=1",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(s) = status {
            sql.push_str(" AND t.status = ?");
            params_vec.push(Box::new(s.to_string()));
        }
        if let Some(p) = priority {
            sql.push_str(" AND t.priority = ?");
            params_vec.push(Box::new(p));
        }
        if let Some(t) = tag {
            sql.push_str(" AND tt.tag = ?");
            params_vec.push(Box::new(t.to_string()));
        }

        sql.push_str(" ORDER BY t.priority ASC, t.created_at DESC");

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let mut stmt = self.conn.prepare(&sql)?;
        let ids: Vec<String> = stmt
            .query_map(params_refs.as_slice(), |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        // Fetch full task objects
        let mut tasks = Vec::new();
        for id in ids {
            if let Ok(task) = self.get_task(&id) {
                tasks.push(task);
            }
        }

        Ok(tasks)
    }

    /// Update a task.
    pub fn update_task(&mut self, task: &Task) -> Result<()> {
        // Verify task exists
        self.get_task(&task.id)?;

        // Append updated version to JSONL
        let tasks_path = self.root.join("tasks.jsonl");
        let mut file = OpenOptions::new().append(true).open(&tasks_path)?;

        let json = serde_json::to_string(task)?;
        writeln!(file, "{}", json)?;

        // Update cache
        self.cache_task(task)?;

        Ok(())
    }

    /// Delete a task by ID.
    pub fn delete_task(&mut self, id: &str) -> Result<()> {
        // Verify task exists
        self.get_task(id)?;

        // We don't actually remove from JSONL (append-only log),
        // but we mark it as deleted in cache
        self.conn.execute("DELETE FROM tasks WHERE id = ?", [id])?;
        self.conn
            .execute("DELETE FROM task_tags WHERE task_id = ?", [id])?;
        self.conn.execute(
            "DELETE FROM task_dependencies WHERE child_id = ? OR parent_id = ?",
            [id, id],
        )?;

        Ok(())
    }

    // === Dependency Operations ===

    /// Add a dependency (child depends on parent).
    ///
    /// Returns an error if:
    /// - Either task doesn't exist
    /// - Adding the dependency would create a cycle
    /// - The dependency already exists
    pub fn add_dependency(&mut self, child_id: &str, parent_id: &str) -> Result<()> {
        // Validate both tasks exist
        self.get_task(child_id)?;
        self.get_task(parent_id)?;

        // Check for self-dependency
        if child_id == parent_id {
            return Err(Error::Other("A task cannot depend on itself".to_string()));
        }

        // Check if dependency already exists
        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM task_dependencies WHERE child_id = ?1 AND parent_id = ?2)",
            params![child_id, parent_id],
            |row| row.get(0),
        )?;

        if exists {
            return Err(Error::Other(format!(
                "Dependency already exists: {} -> {}",
                child_id, parent_id
            )));
        }

        // Check for cycle: would adding this edge create a path from parent back to child?
        if self.would_create_cycle(child_id, parent_id)? {
            return Err(Error::CycleDetected);
        }

        // Add the dependency to the cache
        self.conn.execute(
            "INSERT INTO task_dependencies (child_id, parent_id) VALUES (?1, ?2)",
            params![child_id, parent_id],
        )?;

        // Update the task's depends_on list and append to JSONL
        let mut task = self.get_task(child_id)?;
        if !task.depends_on.contains(&parent_id.to_string()) {
            task.depends_on.push(parent_id.to_string());
            task.updated_at = chrono::Utc::now();

            // Append updated task to JSONL
            let tasks_path = self.root.join("tasks.jsonl");
            let mut file = OpenOptions::new().append(true).open(&tasks_path)?;
            let json = serde_json::to_string(&task)?;
            writeln!(file, "{}", json)?;
        }

        Ok(())
    }

    /// Remove a dependency.
    pub fn remove_dependency(&mut self, child_id: &str, parent_id: &str) -> Result<()> {
        // Validate both tasks exist
        self.get_task(child_id)?;
        self.get_task(parent_id)?;

        // Check if dependency exists
        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM task_dependencies WHERE child_id = ?1 AND parent_id = ?2)",
            params![child_id, parent_id],
            |row| row.get(0),
        )?;

        if !exists {
            return Err(Error::NotFound(format!(
                "Dependency not found: {} -> {}",
                child_id, parent_id
            )));
        }

        // Remove from cache
        self.conn.execute(
            "DELETE FROM task_dependencies WHERE child_id = ?1 AND parent_id = ?2",
            params![child_id, parent_id],
        )?;

        // Update the task's depends_on list and append to JSONL
        let mut task = self.get_task(child_id)?;
        task.depends_on.retain(|id| id != parent_id);
        task.updated_at = chrono::Utc::now();

        let tasks_path = self.root.join("tasks.jsonl");
        let mut file = OpenOptions::new().append(true).open(&tasks_path)?;
        let json = serde_json::to_string(&task)?;
        writeln!(file, "{}", json)?;

        Ok(())
    }

    /// Get all dependencies of a task (what it depends on).
    pub fn get_dependencies(&self, task_id: &str) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT parent_id FROM task_dependencies WHERE child_id = ?1")?;
        let ids: Vec<String> = stmt
            .query_map([task_id], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(ids)
    }

    /// Get all dependents of a task (what depends on it).
    pub fn get_dependents(&self, task_id: &str) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT child_id FROM task_dependencies WHERE parent_id = ?1")?;
        let ids: Vec<String> = stmt
            .query_map([task_id], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(ids)
    }

    /// Check if adding an edge from child to parent would create a cycle.
    ///
    /// This uses DFS to check if there's a path from parent to child.
    /// If there is, adding child->parent would create a cycle.
    fn would_create_cycle(&self, child_id: &str, parent_id: &str) -> Result<bool> {
        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![parent_id.to_string()];

        while let Some(current) = stack.pop() {
            if current == child_id {
                return Ok(true); // Found a path back to child, would create cycle
            }

            if visited.contains(&current) {
                continue;
            }
            visited.insert(current.clone());

            // Get all tasks that the current task depends on
            let deps = self.get_dependencies(&current)?;
            for dep in deps {
                if !visited.contains(&dep) {
                    stack.push(dep);
                }
            }
        }

        Ok(false)
    }

    /// Get tasks that are ready (pending/reopened with all dependencies done).
    pub fn get_ready_tasks(&self) -> Result<Vec<Task>> {
        let tasks = self.list_tasks(None, None, None)?;
        let mut ready = Vec::new();

        for task in tasks {
            match task.status {
                TaskStatus::Pending | TaskStatus::Reopened => {
                    if task.depends_on.is_empty() {
                        ready.push(task);
                    } else {
                        let all_done = task.depends_on.iter().all(|dep_id| {
                            self.get_task(dep_id)
                                .map(|t| t.status == TaskStatus::Done)
                                .unwrap_or(false)
                        });
                        if all_done {
                            ready.push(task);
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(ready)
    }

    /// Get tasks that are blocked (have open dependencies).
    pub fn get_blocked_tasks(&self) -> Result<Vec<Task>> {
        let tasks = self.list_tasks(None, None, None)?;
        let mut blocked = Vec::new();

        for task in tasks {
            match task.status {
                TaskStatus::Pending | TaskStatus::Reopened => {
                    if !task.depends_on.is_empty() {
                        let has_open_deps = task.depends_on.iter().any(|dep_id| {
                            self.get_task(dep_id)
                                .map(|t| t.status != TaskStatus::Done)
                                .unwrap_or(true)
                        });
                        if has_open_deps {
                            blocked.push(task);
                        }
                    }
                }
                TaskStatus::Blocked => {
                    // Explicitly blocked tasks are always included
                    blocked.push(task);
                }
                _ => {}
            }
        }

        Ok(blocked)
    }

    /// Get the storage root path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    // === Config Operations ===

    /// Get a configuration value.
    pub fn get_config(&self, key: &str) -> Result<Option<String>> {
        let value: Option<String> = self
            .conn
            .query_row("SELECT value FROM config WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .ok();
        Ok(value)
    }

    /// Set a configuration value.
    pub fn set_config(&mut self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO config (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    /// List all configuration values.
    pub fn list_configs(&self) -> Result<Vec<(String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT key, value FROM config ORDER BY key")?;
        let configs: Vec<(String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(configs)
    }

    // === Log Operations ===

    /// Get log entries from the JSONL file.
    ///
    /// Reads the tasks.jsonl file and reconstructs the history of changes.
    /// If task_id is provided, filters to entries for that task only.
    pub fn get_log_entries(&self, task_id: Option<&str>) -> Result<Vec<crate::commands::LogEntry>> {
        use std::collections::HashMap;

        let tasks_path = self.root.join("tasks.jsonl");
        let file = File::open(&tasks_path)?;
        let reader = BufReader::new(file);

        let mut entries = Vec::new();
        let mut seen_tasks: HashMap<String, chrono::DateTime<Utc>> = HashMap::new();
        let mut seen_tests: HashMap<String, chrono::DateTime<Utc>> = HashMap::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            // Try to parse as Task
            if let Ok(task) = serde_json::from_str::<Task>(&line) {
                if task.entity_type == "task" {
                    // Filter by task_id if provided
                    if let Some(filter_id) = task_id {
                        if task.id != filter_id {
                            continue;
                        }
                    }

                    let action = if seen_tasks.contains_key(&task.id) {
                        // Determine what kind of update
                        if task.status == TaskStatus::Done && task.closed_at.is_some() {
                            "closed"
                        } else if task.status == TaskStatus::Reopened {
                            "reopened"
                        } else {
                            "updated"
                        }
                    } else {
                        "created"
                    };

                    let details = match action {
                        "closed" => task.closed_reason.clone(),
                        "updated" => Some(format!("status: {:?}", task.status)),
                        _ => None,
                    };

                    entries.push(crate::commands::LogEntry {
                        timestamp: task.updated_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                        entity_type: "task".to_string(),
                        entity_id: task.id.clone(),
                        action: action.to_string(),
                        details,
                    });

                    seen_tasks.insert(task.id.clone(), task.updated_at);
                }
            }

            // Try to parse as TestNode
            if let Ok(test) = serde_json::from_str::<TestNode>(&line) {
                if test.entity_type == "test" {
                    // Only include if not filtered or if it's linked to the task
                    let include = match task_id {
                        Some(filter_id) => test.linked_tasks.contains(&filter_id.to_string()),
                        None => true,
                    };

                    if include {
                        let action = if seen_tests.contains_key(&test.id) {
                            "updated"
                        } else {
                            "created"
                        };

                        entries.push(crate::commands::LogEntry {
                            timestamp: test.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                            entity_type: "test".to_string(),
                            entity_id: test.id.clone(),
                            action: action.to_string(),
                            details: None,
                        });

                        seen_tests.insert(test.id.clone(), test.created_at);
                    }
                }
            }
        }

        // Also include commit links if not filtered or if linked to the task
        let commits_path = self.root.join("commits.jsonl");
        if commits_path.exists() {
            let file = File::open(&commits_path)?;
            let reader = BufReader::new(file);

            for line in reader.lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }

                if let Ok(link) = serde_json::from_str::<CommitLink>(&line) {
                    let include = match task_id {
                        Some(filter_id) => link.task_id == filter_id,
                        None => true,
                    };

                    if include {
                        entries.push(crate::commands::LogEntry {
                            timestamp: link.linked_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                            entity_type: "commit".to_string(),
                            entity_id: link.sha.clone(),
                            action: "linked".to_string(),
                            details: Some(format!("to task {}", link.task_id)),
                        });
                    }
                }
            }
        }

        // Sort by timestamp descending (newest first)
        entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(entries)
    }

    // === Compact Operation ===

    /// Compact the storage by keeping only the latest version of each entity.
    ///
    /// Returns statistics about the compaction.
    pub fn compact(&mut self) -> Result<crate::commands::CompactResult> {
        use std::collections::HashMap;

        let tasks_path = self.root.join("tasks.jsonl");
        let backup_path = self.root.join("tasks.jsonl.bak");

        // Read original file size
        let original_size = fs::metadata(&tasks_path)
            .map(|m| m.len() as usize)
            .unwrap_or(0);

        // Read all entries and keep only the latest version of each entity
        let file = File::open(&tasks_path)?;
        let reader = BufReader::new(file);

        let mut latest_tasks: HashMap<String, Task> = HashMap::new();
        let mut latest_tests: HashMap<String, TestNode> = HashMap::new();
        let mut original_entries = 0;

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            original_entries += 1;

            // Try to parse as Task
            if let Ok(task) = serde_json::from_str::<Task>(&line) {
                if task.entity_type == "task" {
                    latest_tasks.insert(task.id.clone(), task);
                    continue;
                }
            }

            // Try to parse as TestNode
            if let Ok(test) = serde_json::from_str::<TestNode>(&line) {
                if test.entity_type == "test" {
                    latest_tests.insert(test.id.clone(), test);
                }
            }
        }

        // Create backup
        fs::copy(&tasks_path, &backup_path)?;

        // Write compacted file
        let mut file = File::create(&tasks_path)?;

        // Write tasks sorted by created_at
        let mut tasks: Vec<_> = latest_tasks.values().collect();
        tasks.sort_by(|a, b| a.created_at.cmp(&b.created_at));

        for task in &tasks {
            let json = serde_json::to_string(task)?;
            writeln!(file, "{}", json)?;
        }

        // Write tests sorted by created_at
        let mut tests: Vec<_> = latest_tests.values().collect();
        tests.sort_by(|a, b| a.created_at.cmp(&b.created_at));

        for test in &tests {
            let json = serde_json::to_string(test)?;
            writeln!(file, "{}", json)?;
        }

        let final_entries = tasks.len() + tests.len();

        // Calculate space saved
        let new_size = fs::metadata(&tasks_path)
            .map(|m| m.len() as usize)
            .unwrap_or(0);
        let space_saved = original_size.saturating_sub(new_size);

        // Rebuild cache
        self.rebuild_cache()?;

        // Remove backup on success
        let _ = fs::remove_file(&backup_path);

        Ok(crate::commands::CompactResult {
            tasks_compacted: latest_tasks.len(),
            original_entries,
            final_entries,
            space_saved_bytes: space_saved,
        })
    }

    /// Count total commit links.
    pub fn count_commit_links(&self) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM commit_links", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    // === Test Node Operations ===

    /// Cache a test node in SQLite for fast querying.
    fn cache_test(&self, test: &TestNode) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO tests
            (id, name, command, working_dir, pattern, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                test.id,
                test.name,
                test.command,
                test.working_dir,
                test.pattern,
                test.created_at.to_rfc3339(),
            ],
        )?;

        // Update links
        self.conn
            .execute("DELETE FROM test_links WHERE test_id = ?1", [&test.id])?;
        for task_id in &test.linked_tasks {
            self.conn.execute(
                "INSERT OR IGNORE INTO test_links (test_id, task_id) VALUES (?1, ?2)",
                params![test.id, task_id],
            )?;
        }

        Ok(())
    }

    /// Create a new test node.
    pub fn create_test(&mut self, test: &TestNode) -> Result<()> {
        // Append to JSONL (same file as tasks for simplicity)
        let tasks_path = self.root.join("tasks.jsonl");
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&tasks_path)?;

        let json = serde_json::to_string(test)?;
        writeln!(file, "{}", json)?;

        // Update cache
        self.cache_test(test)?;

        Ok(())
    }

    /// Get a test node by ID.
    pub fn get_test(&self, id: &str) -> Result<TestNode> {
        let tasks_path = self.root.join("tasks.jsonl");
        let file = File::open(&tasks_path)?;
        let reader = BufReader::new(file);

        let mut latest: Option<TestNode> = None;
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(test) = serde_json::from_str::<TestNode>(&line) {
                if test.entity_type == "test" && test.id == id {
                    latest = Some(test);
                }
            }
        }

        latest.ok_or_else(|| Error::NotFound(format!("Test not found: {}", id)))
    }

    /// List all test nodes, optionally filtered by linked task.
    pub fn list_tests(&self, task_id: Option<&str>) -> Result<Vec<TestNode>> {
        let mut sql = String::from("SELECT DISTINCT t.id FROM tests t");

        if task_id.is_some() {
            sql.push_str(" INNER JOIN test_links tl ON t.id = tl.test_id WHERE tl.task_id = ?1");
        }

        sql.push_str(" ORDER BY t.created_at DESC");

        let mut stmt = self.conn.prepare(&sql)?;
        let ids: Vec<String> = if let Some(tid) = task_id {
            stmt.query_map([tid], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect()
        } else {
            stmt.query_map([], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect()
        };

        let mut tests = Vec::new();
        for id in ids {
            if let Ok(test) = self.get_test(&id) {
                tests.push(test);
            }
        }

        Ok(tests)
    }

    /// Update a test node.
    pub fn update_test(&mut self, test: &TestNode) -> Result<()> {
        // Verify test exists
        self.get_test(&test.id)?;

        // Append updated version to JSONL
        let tasks_path = self.root.join("tasks.jsonl");
        let mut file = OpenOptions::new().append(true).open(&tasks_path)?;

        let json = serde_json::to_string(test)?;
        writeln!(file, "{}", json)?;

        // Update cache
        self.cache_test(test)?;

        Ok(())
    }

    /// Delete a test node by ID.
    pub fn delete_test(&mut self, id: &str) -> Result<()> {
        // Verify test exists
        self.get_test(id)?;

        // Remove from cache
        self.conn.execute("DELETE FROM tests WHERE id = ?", [id])?;
        self.conn
            .execute("DELETE FROM test_links WHERE test_id = ?", [id])?;

        Ok(())
    }

    /// Link a test to a task.
    pub fn link_test_to_task(&mut self, test_id: &str, task_id: &str) -> Result<()> {
        // Verify both exist
        let mut test = self.get_test(test_id)?;
        self.get_task(task_id)?;

        // Check if already linked
        if test.linked_tasks.contains(&task_id.to_string()) {
            return Err(Error::Other(format!(
                "Test {} is already linked to task {}",
                test_id, task_id
            )));
        }

        // Add link
        test.linked_tasks.push(task_id.to_string());

        // Update storage
        self.update_test(&test)?;

        Ok(())
    }

    /// Unlink a test from a task.
    pub fn unlink_test_from_task(&mut self, test_id: &str, task_id: &str) -> Result<()> {
        // Verify both exist
        let mut test = self.get_test(test_id)?;
        self.get_task(task_id)?;

        // Check if linked
        if !test.linked_tasks.contains(&task_id.to_string()) {
            return Err(Error::NotFound(format!(
                "Test {} is not linked to task {}",
                test_id, task_id
            )));
        }

        // Remove link
        test.linked_tasks.retain(|id| id != task_id);

        // Update storage
        self.update_test(&test)?;

        Ok(())
    }

    /// Get all tests linked to a task.
    pub fn get_tests_for_task(&self, task_id: &str) -> Result<Vec<TestNode>> {
        self.list_tests(Some(task_id))
    }

    // === Test Result Operations ===

    /// Save a test result.
    pub fn save_test_result(&mut self, result: &TestResult) -> Result<()> {
        // Append to test-results.jsonl
        let results_path = self.root.join("test-results.jsonl");
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&results_path)?;

        let json = serde_json::to_string(result)?;
        writeln!(file, "{}", json)?;

        // Also cache in SQLite
        self.conn.execute(
            r#"
            INSERT INTO test_results
            (test_id, passed, exit_code, stdout, stderr, duration_ms, executed_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                result.test_id,
                result.passed as i32,
                result.exit_code,
                result.stdout,
                result.stderr,
                result.duration_ms as i64,
                result.executed_at.to_rfc3339(),
            ],
        )?;

        Ok(())
    }

    /// Get the last test result for a test node.
    pub fn get_last_test_result(&self, test_id: &str) -> Result<Option<TestResult>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT test_id, passed, exit_code, stdout, stderr, duration_ms, executed_at
            FROM test_results
            WHERE test_id = ?1
            ORDER BY executed_at DESC
            LIMIT 1
            "#,
        )?;

        let result = stmt
            .query_row([test_id], |row| {
                let passed: i32 = row.get(1)?;
                let duration_ms: i64 = row.get(5)?;
                let executed_at_str: String = row.get(6)?;
                Ok(TestResult {
                    test_id: row.get(0)?,
                    passed: passed != 0,
                    exit_code: row.get(2)?,
                    stdout: row.get(3)?,
                    stderr: row.get(4)?,
                    duration_ms: duration_ms as u64,
                    executed_at: chrono::DateTime::parse_from_rfc3339(&executed_at_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                })
            })
            .ok();

        Ok(result)
    }

    /// Get all failed tests (tests whose last result was a failure).
    pub fn get_failed_tests(&self) -> Result<Vec<TestNode>> {
        let tests = self.list_tests(None)?;
        let mut failed = Vec::new();

        for test in tests {
            if let Ok(Some(result)) = self.get_last_test_result(&test.id) {
                if !result.passed {
                    failed.push(test);
                }
            }
        }

        Ok(failed)
    }

    /// Reopen tasks linked to a failed test (regression detection).
    ///
    /// Returns the list of task IDs that were reopened.
    pub fn reopen_linked_tasks_on_failure(&mut self, test_id: &str) -> Result<Vec<String>> {
        let test = self.get_test(test_id)?;
        let mut reopened = Vec::new();

        for task_id in &test.linked_tasks {
            if let Ok(mut task) = self.get_task(task_id) {
                // Only reopen if the task was closed (done)
                if task.status == TaskStatus::Done {
                    task.status = TaskStatus::Reopened;
                    task.closed_at = None;
                    task.updated_at = Utc::now();
                    self.update_task(&task)?;
                    reopened.push(task_id.clone());
                }
            }
        }

        Ok(reopened)
    }

    // === Commit Link Operations ===

    /// Link a commit to a task.
    pub fn link_commit(&mut self, sha: &str, task_id: &str) -> Result<CommitLink> {
        // Validate SHA format
        validate_sha(sha)?;

        // Validate task exists
        self.get_task(task_id)?;

        // Check if already linked
        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM commit_links WHERE sha = ?1 AND task_id = ?2)",
            params![sha, task_id],
            |row| row.get(0),
        )?;

        if exists {
            return Err(Error::Other(format!(
                "Commit {} is already linked to task {}",
                sha, task_id
            )));
        }

        let linked_at = Utc::now();

        // Insert into SQLite cache
        self.conn.execute(
            "INSERT INTO commit_links (sha, task_id, linked_at) VALUES (?1, ?2, ?3)",
            params![sha, task_id, linked_at.to_rfc3339()],
        )?;

        // Append to JSONL
        let link = CommitLink {
            sha: sha.to_string(),
            task_id: task_id.to_string(),
            linked_at,
        };
        let commits_path = self.root.join("commits.jsonl");
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&commits_path)?;
        let json = serde_json::to_string(&link)?;
        writeln!(file, "{}", json)?;

        Ok(link)
    }

    /// Unlink a commit from a task.
    pub fn unlink_commit(&mut self, sha: &str, task_id: &str) -> Result<()> {
        // Validate SHA format
        validate_sha(sha)?;

        // Validate task exists
        self.get_task(task_id)?;

        // Check if linked
        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM commit_links WHERE sha = ?1 AND task_id = ?2)",
            params![sha, task_id],
            |row| row.get(0),
        )?;

        if !exists {
            return Err(Error::NotFound(format!(
                "Commit {} is not linked to task {}",
                sha, task_id
            )));
        }

        // Remove from cache
        self.conn.execute(
            "DELETE FROM commit_links WHERE sha = ?1 AND task_id = ?2",
            params![sha, task_id],
        )?;

        Ok(())
    }

    /// Get all commits linked to a task.
    pub fn get_commits_for_task(&self, task_id: &str) -> Result<Vec<CommitLink>> {
        // Validate task exists
        self.get_task(task_id)?;

        let mut stmt = self.conn.prepare(
            "SELECT sha, task_id, linked_at FROM commit_links WHERE task_id = ?1 ORDER BY linked_at DESC",
        )?;

        let links: Vec<CommitLink> = stmt
            .query_map([task_id], |row| {
                let linked_at_str: String = row.get(2)?;
                Ok(CommitLink {
                    sha: row.get(0)?,
                    task_id: row.get(1)?,
                    linked_at: chrono::DateTime::parse_from_rfc3339(&linked_at_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(links)
    }

    /// Get all tasks linked to a commit.
    pub fn get_tasks_for_commit(&self, sha: &str) -> Result<Vec<String>> {
        validate_sha(sha)?;

        let mut stmt = self
            .conn
            .prepare("SELECT task_id FROM commit_links WHERE sha = ?1")?;

        let task_ids: Vec<String> = stmt
            .query_map([sha], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(task_ids)
    }

    /// Get commits since a task was closed (for regression context).
    /// Returns commits linked to the task that were made after the task was closed.
    pub fn get_commits_since_close(&self, task_id: &str) -> Result<Vec<CommitLink>> {
        let task = self.get_task(task_id)?;

        // If task was never closed, return empty
        let closed_at = match task.closed_at {
            Some(dt) => dt,
            None => return Ok(Vec::new()),
        };

        let mut stmt = self.conn.prepare(
            "SELECT sha, task_id, linked_at FROM commit_links WHERE task_id = ?1 AND linked_at > ?2 ORDER BY linked_at DESC",
        )?;

        let links: Vec<CommitLink> = stmt
            .query_map(params![task_id, closed_at.to_rfc3339()], |row| {
                let linked_at_str: String = row.get(2)?;
                Ok(CommitLink {
                    sha: row.get(0)?,
                    task_id: row.get(1)?,
                    linked_at: chrono::DateTime::parse_from_rfc3339(&linked_at_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(links)
    }
}

/// Get the storage directory for a repository.
///
/// Uses a hash of the repository path to create a unique directory
/// under `~/.local/share/binnacle/`.
pub fn get_storage_dir(repo_path: &Path) -> Result<PathBuf> {
    let data_dir = dirs::data_dir()
        .ok_or_else(|| Error::Other("Could not determine data directory".to_string()))?;

    let repo_canonical = repo_path
        .canonicalize()
        .map_err(|e| Error::Other(format!("Could not canonicalize repo path: {}", e)))?;

    let mut hasher = Sha256::new();
    hasher.update(repo_canonical.to_string_lossy().as_bytes());
    let hash = hasher.finalize();
    let hash_hex = format!("{:x}", hash);
    let short_hash = &hash_hex[..12];

    Ok(data_dir.join("binnacle").join(short_hash))
}

/// Generate a unique ID for a task or test node.
///
/// Format: `<prefix>-<4 hex chars>`
/// - Task prefix: "bn"
/// - Test prefix: "bnt"
pub fn generate_id(prefix: &str, seed: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(seed.as_bytes());
    hasher.update(
        chrono::Utc::now()
            .timestamp_nanos_opt()
            .unwrap_or(0)
            .to_le_bytes(),
    );
    let hash = hasher.finalize();
    let hash_hex = format!("{:x}", hash);
    format!("{}-{}", prefix, &hash_hex[..4])
}

/// Validate that an ID matches the expected format.
pub fn validate_id(id: &str, prefix: &str) -> Result<()> {
    if !id.starts_with(&format!("{}-", prefix)) {
        return Err(Error::InvalidId(format!(
            "ID must start with '{}-', got: {}",
            prefix, id
        )));
    }

    let suffix = &id[prefix.len() + 1..];
    if suffix.len() != 4 || !suffix.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(Error::InvalidId(format!(
            "ID suffix must be 4 hex characters, got: {}",
            suffix
        )));
    }

    Ok(())
}

/// Validate a test ID (bnt-xxxx format).
pub fn validate_test_id(id: &str) -> Result<()> {
    validate_id(id, "bnt")
}

/// Validate a task ID (bn-xxxx format).
pub fn validate_task_id(id: &str) -> Result<()> {
    validate_id(id, "bn")
}

/// Parse a status string into TaskStatus.
pub fn parse_status(s: &str) -> Result<TaskStatus> {
    match s.to_lowercase().as_str() {
        "pending" => Ok(TaskStatus::Pending),
        "in_progress" | "in-progress" | "inprogress" => Ok(TaskStatus::InProgress),
        "done" => Ok(TaskStatus::Done),
        "blocked" => Ok(TaskStatus::Blocked),
        "cancelled" | "canceled" => Ok(TaskStatus::Cancelled),
        "reopened" => Ok(TaskStatus::Reopened),
        _ => Err(Error::Other(format!("Invalid status: {}", s))),
    }
}

/// Validate a git commit SHA.
///
/// Accepts both short (7+ chars) and full (40 chars) SHA formats.
/// SHAs must consist only of hexadecimal characters.
pub fn validate_sha(sha: &str) -> Result<()> {
    if sha.len() < 7 {
        return Err(Error::InvalidId(format!(
            "SHA must be at least 7 characters, got {} characters: {}",
            sha.len(),
            sha
        )));
    }
    if sha.len() > 40 {
        return Err(Error::InvalidId(format!(
            "SHA must be at most 40 characters, got {} characters: {}",
            sha.len(),
            sha
        )));
    }
    if !sha.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(Error::InvalidId(format!(
            "SHA must contain only hex characters: {}",
            sha
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_storage() -> (TempDir, Storage) {
        let temp_dir = TempDir::new().unwrap();
        let storage = Storage::init(temp_dir.path()).unwrap();
        (temp_dir, storage)
    }

    #[test]
    fn test_generate_id_format() {
        let id = generate_id("bn", "test seed");
        assert!(id.starts_with("bn-"));
        assert_eq!(id.len(), 7); // "bn-" + 4 hex chars
    }

    #[test]
    fn test_generate_id_uniqueness() {
        let id1 = generate_id("bn", "seed1");
        let id2 = generate_id("bn", "seed2");
        // IDs should be different (with high probability)
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_validate_id_valid() {
        assert!(validate_id("bn-a1b2", "bn").is_ok());
        assert!(validate_id("bnt-ffff", "bnt").is_ok());
    }

    #[test]
    fn test_validate_id_invalid_prefix() {
        assert!(validate_id("task-a1b2", "bn").is_err());
        assert!(validate_id("a1b2", "bn").is_err());
    }

    #[test]
    fn test_validate_id_invalid_suffix() {
        assert!(validate_id("bn-a1b", "bn").is_err()); // Too short
        assert!(validate_id("bn-a1b2c", "bn").is_err()); // Too long
        assert!(validate_id("bn-ghij", "bn").is_err()); // Non-hex chars
    }

    #[test]
    fn test_storage_init() {
        let temp_dir = TempDir::new().unwrap();
        let storage = Storage::init(temp_dir.path()).unwrap();

        assert!(storage.root.exists());
        assert!(storage.root.join("tasks.jsonl").exists());
        assert!(storage.root.join("cache.db").exists());
    }

    #[test]
    fn test_storage_exists() {
        let temp_dir = TempDir::new().unwrap();
        assert!(!Storage::exists(temp_dir.path()).unwrap());

        Storage::init(temp_dir.path()).unwrap();
        assert!(Storage::exists(temp_dir.path()).unwrap());
    }

    #[test]
    fn test_create_and_get_task() {
        let (_temp_dir, mut storage) = create_test_storage();

        let task = Task::new("bn-test".to_string(), "Test task".to_string());
        storage.create_task(&task).unwrap();

        let retrieved = storage.get_task("bn-test").unwrap();
        assert_eq!(retrieved.id, "bn-test");
        assert_eq!(retrieved.title, "Test task");
    }

    #[test]
    fn test_list_tasks() {
        let (_temp_dir, mut storage) = create_test_storage();

        let mut task1 = Task::new("bn-0001".to_string(), "Task 1".to_string());
        task1.priority = 1;
        task1.tags = vec!["backend".to_string()];
        storage.create_task(&task1).unwrap();

        let mut task2 = Task::new("bn-0002".to_string(), "Task 2".to_string());
        task2.priority = 2;
        task2.tags = vec!["frontend".to_string()];
        storage.create_task(&task2).unwrap();

        // List all
        let all = storage.list_tasks(None, None, None).unwrap();
        assert_eq!(all.len(), 2);

        // Filter by priority
        let p1 = storage.list_tasks(None, Some(1), None).unwrap();
        assert_eq!(p1.len(), 1);
        assert_eq!(p1[0].id, "bn-0001");

        // Filter by tag
        let backend = storage.list_tasks(None, None, Some("backend")).unwrap();
        assert_eq!(backend.len(), 1);
        assert_eq!(backend[0].id, "bn-0001");
    }

    #[test]
    fn test_update_task() {
        let (_temp_dir, mut storage) = create_test_storage();

        let mut task = Task::new("bn-test".to_string(), "Original".to_string());
        storage.create_task(&task).unwrap();

        task.title = "Updated".to_string();
        task.status = TaskStatus::InProgress;
        storage.update_task(&task).unwrap();

        let retrieved = storage.get_task("bn-test").unwrap();
        assert_eq!(retrieved.title, "Updated");
        assert_eq!(retrieved.status, TaskStatus::InProgress);
    }

    #[test]
    fn test_delete_task() {
        let (_temp_dir, mut storage) = create_test_storage();

        let task = Task::new("bn-test".to_string(), "Test".to_string());
        storage.create_task(&task).unwrap();

        storage.delete_task("bn-test").unwrap();

        // Task should not be in cache
        let all = storage.list_tasks(None, None, None).unwrap();
        assert_eq!(all.len(), 0);
    }

    #[test]
    fn test_parse_status() {
        assert_eq!(parse_status("pending").unwrap(), TaskStatus::Pending);
        assert_eq!(parse_status("in_progress").unwrap(), TaskStatus::InProgress);
        assert_eq!(parse_status("in-progress").unwrap(), TaskStatus::InProgress);
        assert_eq!(parse_status("done").unwrap(), TaskStatus::Done);
        assert!(parse_status("invalid").is_err());
    }

    // === Dependency Tests ===

    #[test]
    fn test_add_dependency() {
        let (_temp_dir, mut storage) = create_test_storage();

        let task_a = Task::new("bn-aaaa".to_string(), "Task A".to_string());
        let task_b = Task::new("bn-bbbb".to_string(), "Task B".to_string());
        storage.create_task(&task_a).unwrap();
        storage.create_task(&task_b).unwrap();

        // B depends on A
        storage.add_dependency("bn-bbbb", "bn-aaaa").unwrap();

        let deps = storage.get_dependencies("bn-bbbb").unwrap();
        assert_eq!(deps, vec!["bn-aaaa"]);

        let dependents = storage.get_dependents("bn-aaaa").unwrap();
        assert_eq!(dependents, vec!["bn-bbbb"]);
    }

    #[test]
    fn test_add_dependency_self_reference() {
        let (_temp_dir, mut storage) = create_test_storage();

        let task = Task::new("bn-aaaa".to_string(), "Task A".to_string());
        storage.create_task(&task).unwrap();

        // A cannot depend on itself
        let result = storage.add_dependency("bn-aaaa", "bn-aaaa");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cannot depend on itself"));
    }

    #[test]
    fn test_add_dependency_duplicate() {
        let (_temp_dir, mut storage) = create_test_storage();

        let task_a = Task::new("bn-aaaa".to_string(), "Task A".to_string());
        let task_b = Task::new("bn-bbbb".to_string(), "Task B".to_string());
        storage.create_task(&task_a).unwrap();
        storage.create_task(&task_b).unwrap();

        storage.add_dependency("bn-bbbb", "bn-aaaa").unwrap();

        // Adding same dependency again should fail
        let result = storage.add_dependency("bn-bbbb", "bn-aaaa");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[test]
    fn test_cycle_detection_direct() {
        let (_temp_dir, mut storage) = create_test_storage();

        let task_a = Task::new("bn-aaaa".to_string(), "Task A".to_string());
        let task_b = Task::new("bn-bbbb".to_string(), "Task B".to_string());
        storage.create_task(&task_a).unwrap();
        storage.create_task(&task_b).unwrap();

        // A depends on B
        storage.add_dependency("bn-aaaa", "bn-bbbb").unwrap();

        // B depends on A would create a cycle
        let result = storage.add_dependency("bn-bbbb", "bn-aaaa");
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::Error::CycleDetected => {}
            e => panic!("Expected CycleDetected, got {:?}", e),
        }
    }

    #[test]
    fn test_cycle_detection_transitive() {
        let (_temp_dir, mut storage) = create_test_storage();

        // Create A -> B -> C chain
        let task_a = Task::new("bn-aaaa".to_string(), "Task A".to_string());
        let task_b = Task::new("bn-bbbb".to_string(), "Task B".to_string());
        let task_c = Task::new("bn-cccc".to_string(), "Task C".to_string());
        storage.create_task(&task_a).unwrap();
        storage.create_task(&task_b).unwrap();
        storage.create_task(&task_c).unwrap();

        // B depends on A
        storage.add_dependency("bn-bbbb", "bn-aaaa").unwrap();
        // C depends on B
        storage.add_dependency("bn-cccc", "bn-bbbb").unwrap();

        // A depends on C would create a cycle (A -> B -> C -> A)
        let result = storage.add_dependency("bn-aaaa", "bn-cccc");
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::Error::CycleDetected => {}
            e => panic!("Expected CycleDetected, got {:?}", e),
        }
    }

    #[test]
    fn test_remove_dependency() {
        let (_temp_dir, mut storage) = create_test_storage();

        let task_a = Task::new("bn-aaaa".to_string(), "Task A".to_string());
        let task_b = Task::new("bn-bbbb".to_string(), "Task B".to_string());
        storage.create_task(&task_a).unwrap();
        storage.create_task(&task_b).unwrap();

        storage.add_dependency("bn-bbbb", "bn-aaaa").unwrap();
        storage.remove_dependency("bn-bbbb", "bn-aaaa").unwrap();

        let deps = storage.get_dependencies("bn-bbbb").unwrap();
        assert!(deps.is_empty());
    }

    #[test]
    fn test_remove_nonexistent_dependency() {
        let (_temp_dir, mut storage) = create_test_storage();

        let task_a = Task::new("bn-aaaa".to_string(), "Task A".to_string());
        let task_b = Task::new("bn-bbbb".to_string(), "Task B".to_string());
        storage.create_task(&task_a).unwrap();
        storage.create_task(&task_b).unwrap();

        let result = storage.remove_dependency("bn-bbbb", "bn-aaaa");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_get_ready_tasks() {
        let (_temp_dir, mut storage) = create_test_storage();

        // Task A: no dependencies, pending -> ready
        let task_a = Task::new("bn-aaaa".to_string(), "Task A".to_string());
        storage.create_task(&task_a).unwrap();

        // Task B: depends on A (pending) -> blocked
        let task_b = Task::new("bn-bbbb".to_string(), "Task B".to_string());
        storage.create_task(&task_b).unwrap();
        storage.add_dependency("bn-bbbb", "bn-aaaa").unwrap();

        let ready = storage.get_ready_tasks().unwrap();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, "bn-aaaa");
    }

    #[test]
    fn test_get_ready_tasks_with_done_dependency() {
        let (_temp_dir, mut storage) = create_test_storage();

        // Task A: done
        let mut task_a = Task::new("bn-aaaa".to_string(), "Task A".to_string());
        task_a.status = TaskStatus::Done;
        storage.create_task(&task_a).unwrap();

        // Task B: depends on A (done) -> ready
        let task_b = Task::new("bn-bbbb".to_string(), "Task B".to_string());
        storage.create_task(&task_b).unwrap();
        storage.add_dependency("bn-bbbb", "bn-aaaa").unwrap();

        let ready = storage.get_ready_tasks().unwrap();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, "bn-bbbb");
    }

    #[test]
    fn test_get_blocked_tasks() {
        let (_temp_dir, mut storage) = create_test_storage();

        // Task A: pending
        let task_a = Task::new("bn-aaaa".to_string(), "Task A".to_string());
        storage.create_task(&task_a).unwrap();

        // Task B: depends on A (pending) -> blocked
        let task_b = Task::new("bn-bbbb".to_string(), "Task B".to_string());
        storage.create_task(&task_b).unwrap();
        storage.add_dependency("bn-bbbb", "bn-aaaa").unwrap();

        // Task C: explicitly blocked status
        let mut task_c = Task::new("bn-cccc".to_string(), "Task C".to_string());
        task_c.status = TaskStatus::Blocked;
        storage.create_task(&task_c).unwrap();

        let blocked = storage.get_blocked_tasks().unwrap();
        assert_eq!(blocked.len(), 2);

        let blocked_ids: Vec<&str> = blocked.iter().map(|t| t.id.as_str()).collect();
        assert!(blocked_ids.contains(&"bn-bbbb"));
        assert!(blocked_ids.contains(&"bn-cccc"));
    }

    // === Test Node Tests ===

    #[test]
    fn test_create_and_get_test() {
        let (_temp_dir, mut storage) = create_test_storage();

        let test = crate::models::TestNode::new(
            "bnt-0001".to_string(),
            "Unit tests".to_string(),
            "cargo test".to_string(),
        );
        storage.create_test(&test).unwrap();

        let retrieved = storage.get_test("bnt-0001").unwrap();
        assert_eq!(retrieved.id, "bnt-0001");
        assert_eq!(retrieved.name, "Unit tests");
        assert_eq!(retrieved.command, "cargo test");
    }

    #[test]
    fn test_list_tests() {
        let (_temp_dir, mut storage) = create_test_storage();

        let test1 = crate::models::TestNode::new(
            "bnt-0001".to_string(),
            "Test 1".to_string(),
            "echo test1".to_string(),
        );
        let test2 = crate::models::TestNode::new(
            "bnt-0002".to_string(),
            "Test 2".to_string(),
            "echo test2".to_string(),
        );
        storage.create_test(&test1).unwrap();
        storage.create_test(&test2).unwrap();

        let all = storage.list_tests(None).unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_link_test_to_task() {
        let (_temp_dir, mut storage) = create_test_storage();

        // Create task and test
        let task = Task::new("bn-aaaa".to_string(), "Task A".to_string());
        storage.create_task(&task).unwrap();

        let test = crate::models::TestNode::new(
            "bnt-0001".to_string(),
            "Test 1".to_string(),
            "echo test".to_string(),
        );
        storage.create_test(&test).unwrap();

        // Link them
        storage.link_test_to_task("bnt-0001", "bn-aaaa").unwrap();

        // Verify link
        let retrieved = storage.get_test("bnt-0001").unwrap();
        assert!(retrieved.linked_tasks.contains(&"bn-aaaa".to_string()));

        // List tests for task
        let task_tests = storage.get_tests_for_task("bn-aaaa").unwrap();
        assert_eq!(task_tests.len(), 1);
        assert_eq!(task_tests[0].id, "bnt-0001");
    }

    #[test]
    fn test_link_duplicate_rejected() {
        let (_temp_dir, mut storage) = create_test_storage();

        let task = Task::new("bn-aaaa".to_string(), "Task A".to_string());
        storage.create_task(&task).unwrap();

        let test = crate::models::TestNode::new(
            "bnt-0001".to_string(),
            "Test 1".to_string(),
            "echo test".to_string(),
        );
        storage.create_test(&test).unwrap();

        storage.link_test_to_task("bnt-0001", "bn-aaaa").unwrap();

        // Second link should fail
        let result = storage.link_test_to_task("bnt-0001", "bn-aaaa");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already linked"));
    }

    #[test]
    fn test_unlink_test_from_task() {
        let (_temp_dir, mut storage) = create_test_storage();

        let task = Task::new("bn-aaaa".to_string(), "Task A".to_string());
        storage.create_task(&task).unwrap();

        let test = crate::models::TestNode::new(
            "bnt-0001".to_string(),
            "Test 1".to_string(),
            "echo test".to_string(),
        );
        storage.create_test(&test).unwrap();

        storage.link_test_to_task("bnt-0001", "bn-aaaa").unwrap();
        storage
            .unlink_test_from_task("bnt-0001", "bn-aaaa")
            .unwrap();

        let retrieved = storage.get_test("bnt-0001").unwrap();
        assert!(!retrieved.linked_tasks.contains(&"bn-aaaa".to_string()));
    }

    #[test]
    fn test_unlink_nonexistent_fails() {
        let (_temp_dir, mut storage) = create_test_storage();

        let task = Task::new("bn-aaaa".to_string(), "Task A".to_string());
        storage.create_task(&task).unwrap();

        let test = crate::models::TestNode::new(
            "bnt-0001".to_string(),
            "Test 1".to_string(),
            "echo test".to_string(),
        );
        storage.create_test(&test).unwrap();

        let result = storage.unlink_test_from_task("bnt-0001", "bn-aaaa");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not linked"));
    }

    #[test]
    fn test_save_and_get_test_result() {
        let (_temp_dir, mut storage) = create_test_storage();

        let test = crate::models::TestNode::new(
            "bnt-0001".to_string(),
            "Test 1".to_string(),
            "echo test".to_string(),
        );
        storage.create_test(&test).unwrap();

        let result = crate::models::TestResult {
            test_id: "bnt-0001".to_string(),
            passed: true,
            exit_code: 0,
            stdout: Some("output".to_string()),
            stderr: None,
            duration_ms: 100,
            executed_at: chrono::Utc::now(),
        };
        storage.save_test_result(&result).unwrap();

        let retrieved = storage.get_last_test_result("bnt-0001").unwrap();
        assert!(retrieved.is_some());
        let r = retrieved.unwrap();
        assert!(r.passed);
        assert_eq!(r.exit_code, 0);
    }

    #[test]
    fn test_get_failed_tests() {
        let (_temp_dir, mut storage) = create_test_storage();

        // Create two tests
        let test1 = crate::models::TestNode::new(
            "bnt-0001".to_string(),
            "Passing".to_string(),
            "true".to_string(),
        );
        let test2 = crate::models::TestNode::new(
            "bnt-0002".to_string(),
            "Failing".to_string(),
            "false".to_string(),
        );
        storage.create_test(&test1).unwrap();
        storage.create_test(&test2).unwrap();

        // Record results
        let pass_result = crate::models::TestResult {
            test_id: "bnt-0001".to_string(),
            passed: true,
            exit_code: 0,
            stdout: None,
            stderr: None,
            duration_ms: 10,
            executed_at: chrono::Utc::now(),
        };
        let fail_result = crate::models::TestResult {
            test_id: "bnt-0002".to_string(),
            passed: false,
            exit_code: 1,
            stdout: None,
            stderr: None,
            duration_ms: 10,
            executed_at: chrono::Utc::now(),
        };
        storage.save_test_result(&pass_result).unwrap();
        storage.save_test_result(&fail_result).unwrap();

        let failed = storage.get_failed_tests().unwrap();
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].id, "bnt-0002");
    }

    #[test]
    fn test_reopen_linked_tasks_on_failure() {
        let (_temp_dir, mut storage) = create_test_storage();

        // Create a done task
        let mut task = Task::new("bn-aaaa".to_string(), "Task A".to_string());
        task.status = TaskStatus::Done;
        task.closed_at = Some(chrono::Utc::now());
        storage.create_task(&task).unwrap();

        // Create a test linked to the task
        let mut test = crate::models::TestNode::new(
            "bnt-0001".to_string(),
            "Test 1".to_string(),
            "false".to_string(),
        );
        test.linked_tasks.push("bn-aaaa".to_string());
        storage.create_test(&test).unwrap();

        // Trigger regression detection
        let reopened = storage.reopen_linked_tasks_on_failure("bnt-0001").unwrap();
        assert_eq!(reopened, vec!["bn-aaaa".to_string()]);

        // Verify task was reopened
        let task = storage.get_task("bn-aaaa").unwrap();
        assert_eq!(task.status, TaskStatus::Reopened);
        assert!(task.closed_at.is_none());
    }

    #[test]
    fn test_validate_test_id() {
        assert!(validate_test_id("bnt-a1b2").is_ok());
        assert!(validate_test_id("bnt-ffff").is_ok());
        assert!(validate_test_id("bn-a1b2").is_err()); // Wrong prefix
        assert!(validate_test_id("bnt-abc").is_err()); // Too short
    }

    // === SHA Validation Tests ===

    #[test]
    fn test_validate_sha_valid() {
        // Short SHA (7 chars)
        assert!(validate_sha("a1b2c3d").is_ok());
        // Full SHA (40 chars)
        assert!(validate_sha("a1b2c3d4e5f6789012345678901234567890abcd").is_ok());
        // Medium length
        assert!(validate_sha("a1b2c3d4e5f6").is_ok());
    }

    #[test]
    fn test_validate_sha_too_short() {
        assert!(validate_sha("a1b2c3").is_err()); // 6 chars
        assert!(validate_sha("abc").is_err()); // 3 chars
        assert!(validate_sha("").is_err()); // Empty
    }

    #[test]
    fn test_validate_sha_too_long() {
        // 41 chars - too long
        assert!(validate_sha("a1b2c3d4e5f6789012345678901234567890abcde").is_err());
    }

    #[test]
    fn test_validate_sha_invalid_chars() {
        assert!(validate_sha("g1b2c3d").is_err()); // 'g' is not hex
        assert!(validate_sha("a1b2c3!").is_err()); // '!' is not hex
        assert!(validate_sha("GHIJKLM").is_err()); // Non-hex uppercase
    }

    // === Commit Link Tests ===

    #[test]
    fn test_link_commit() {
        let (_temp_dir, mut storage) = create_test_storage();

        // Create a task
        let task = Task::new("bn-aaaa".to_string(), "Task A".to_string());
        storage.create_task(&task).unwrap();

        // Link a commit
        let link = storage.link_commit("a1b2c3d", "bn-aaaa").unwrap();
        assert_eq!(link.sha, "a1b2c3d");
        assert_eq!(link.task_id, "bn-aaaa");
    }

    #[test]
    fn test_link_commit_invalid_sha() {
        let (_temp_dir, mut storage) = create_test_storage();

        let task = Task::new("bn-aaaa".to_string(), "Task A".to_string());
        storage.create_task(&task).unwrap();

        // Too short
        let result = storage.link_commit("abc", "bn-aaaa");
        assert!(result.is_err());
    }

    #[test]
    fn test_link_commit_nonexistent_task() {
        let (_temp_dir, mut storage) = create_test_storage();

        let result = storage.link_commit("a1b2c3d", "bn-9999");
        assert!(result.is_err());
    }

    #[test]
    fn test_link_commit_duplicate() {
        let (_temp_dir, mut storage) = create_test_storage();

        let task = Task::new("bn-aaaa".to_string(), "Task A".to_string());
        storage.create_task(&task).unwrap();

        storage.link_commit("a1b2c3d", "bn-aaaa").unwrap();

        // Second link should fail
        let result = storage.link_commit("a1b2c3d", "bn-aaaa");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already linked"));
    }

    #[test]
    fn test_unlink_commit() {
        let (_temp_dir, mut storage) = create_test_storage();

        let task = Task::new("bn-aaaa".to_string(), "Task A".to_string());
        storage.create_task(&task).unwrap();

        storage.link_commit("a1b2c3d", "bn-aaaa").unwrap();
        storage.unlink_commit("a1b2c3d", "bn-aaaa").unwrap();

        // Should have no commits now
        let commits = storage.get_commits_for_task("bn-aaaa").unwrap();
        assert!(commits.is_empty());
    }

    #[test]
    fn test_unlink_nonexistent_commit() {
        let (_temp_dir, mut storage) = create_test_storage();

        let task = Task::new("bn-aaaa".to_string(), "Task A".to_string());
        storage.create_task(&task).unwrap();

        let result = storage.unlink_commit("a1b2c3d", "bn-aaaa");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not linked"));
    }

    #[test]
    fn test_get_commits_for_task() {
        let (_temp_dir, mut storage) = create_test_storage();

        let task = Task::new("bn-aaaa".to_string(), "Task A".to_string());
        storage.create_task(&task).unwrap();

        storage.link_commit("a1b2c3d", "bn-aaaa").unwrap();
        storage.link_commit("e5f6789", "bn-aaaa").unwrap();

        let commits = storage.get_commits_for_task("bn-aaaa").unwrap();
        assert_eq!(commits.len(), 2);
    }

    #[test]
    fn test_get_tasks_for_commit() {
        let (_temp_dir, mut storage) = create_test_storage();

        let task_a = Task::new("bn-aaaa".to_string(), "Task A".to_string());
        let task_b = Task::new("bn-bbbb".to_string(), "Task B".to_string());
        storage.create_task(&task_a).unwrap();
        storage.create_task(&task_b).unwrap();

        // Link same commit to both tasks
        storage.link_commit("a1b2c3d", "bn-aaaa").unwrap();
        storage.link_commit("a1b2c3d", "bn-bbbb").unwrap();

        let tasks = storage.get_tasks_for_commit("a1b2c3d").unwrap();
        assert_eq!(tasks.len(), 2);
        assert!(tasks.contains(&"bn-aaaa".to_string()));
        assert!(tasks.contains(&"bn-bbbb".to_string()));
    }

    #[test]
    fn test_commit_link_multiple_tasks() {
        let (_temp_dir, mut storage) = create_test_storage();

        let task_a = Task::new("bn-aaaa".to_string(), "Task A".to_string());
        let task_b = Task::new("bn-bbbb".to_string(), "Task B".to_string());
        storage.create_task(&task_a).unwrap();
        storage.create_task(&task_b).unwrap();

        // Link different commits to different tasks
        storage.link_commit("a1b2c3d", "bn-aaaa").unwrap();
        storage.link_commit("e5f6789", "bn-bbbb").unwrap();
        storage.link_commit("1234567", "bn-aaaa").unwrap();

        let commits_a = storage.get_commits_for_task("bn-aaaa").unwrap();
        let commits_b = storage.get_commits_for_task("bn-bbbb").unwrap();

        assert_eq!(commits_a.len(), 2);
        assert_eq!(commits_b.len(), 1);
    }

    // === Config Tests ===

    #[test]
    fn test_config_set_get() {
        let (_temp_dir, mut storage) = create_test_storage();

        storage.set_config("test.key", "test_value").unwrap();
        let value = storage.get_config("test.key").unwrap();

        assert_eq!(value, Some("test_value".to_string()));
    }

    #[test]
    fn test_config_get_nonexistent() {
        let (_temp_dir, storage) = create_test_storage();

        let value = storage.get_config("nonexistent").unwrap();
        assert_eq!(value, None);
    }

    #[test]
    fn test_config_overwrite() {
        let (_temp_dir, mut storage) = create_test_storage();

        storage.set_config("key", "value1").unwrap();
        storage.set_config("key", "value2").unwrap();

        let value = storage.get_config("key").unwrap();
        assert_eq!(value, Some("value2".to_string()));
    }

    #[test]
    fn test_config_list() {
        let (_temp_dir, mut storage) = create_test_storage();

        storage.set_config("alpha", "1").unwrap();
        storage.set_config("beta", "2").unwrap();
        storage.set_config("gamma", "3").unwrap();

        let configs = storage.list_configs().unwrap();
        assert_eq!(configs.len(), 3);

        // Should be sorted by key
        assert_eq!(configs[0], ("alpha".to_string(), "1".to_string()));
        assert_eq!(configs[1], ("beta".to_string(), "2".to_string()));
        assert_eq!(configs[2], ("gamma".to_string(), "3".to_string()));
    }

    #[test]
    fn test_count_commit_links() {
        let (_temp_dir, mut storage) = create_test_storage();

        let task = Task::new("bn-aaaa".to_string(), "Task A".to_string());
        storage.create_task(&task).unwrap();

        assert_eq!(storage.count_commit_links().unwrap(), 0);

        storage.link_commit("a1b2c3d", "bn-aaaa").unwrap();
        storage.link_commit("e5f6789", "bn-aaaa").unwrap();

        assert_eq!(storage.count_commit_links().unwrap(), 2);
    }

    #[test]
    fn test_compact_basic() {
        let (_temp_dir, mut storage) = create_test_storage();

        // Create a task
        let task = Task::new("bn-aaaa".to_string(), "Task A".to_string());
        storage.create_task(&task).unwrap();

        // Update the task multiple times
        let mut updated = task.clone();
        updated.title = "Updated 1".to_string();
        storage.update_task(&updated).unwrap();

        updated.title = "Updated 2".to_string();
        storage.update_task(&updated).unwrap();

        // Compact
        let result = storage.compact().unwrap();

        assert!(result.original_entries >= 3);
        assert_eq!(result.final_entries, 1);
        assert_eq!(result.tasks_compacted, 1);

        // Verify task still exists with final title
        let task = storage.get_task("bn-aaaa").unwrap();
        assert_eq!(task.title, "Updated 2");
    }

    #[test]
    fn test_compact_preserves_all_entities() {
        let (_temp_dir, mut storage) = create_test_storage();

        // Create multiple tasks and tests
        let task1 = Task::new("bn-aaaa".to_string(), "Task A".to_string());
        let task2 = Task::new("bn-bbbb".to_string(), "Task B".to_string());
        storage.create_task(&task1).unwrap();
        storage.create_task(&task2).unwrap();

        let test = crate::models::TestNode::new(
            "bnt-0001".to_string(),
            "Test 1".to_string(),
            "echo test".to_string(),
        );
        storage.create_test(&test).unwrap();

        let result = storage.compact().unwrap();

        assert_eq!(result.final_entries, 3); // 2 tasks + 1 test

        // Verify all entities still exist
        assert!(storage.get_task("bn-aaaa").is_ok());
        assert!(storage.get_task("bn-bbbb").is_ok());
        assert!(storage.get_test("bnt-0001").is_ok());
    }
}
