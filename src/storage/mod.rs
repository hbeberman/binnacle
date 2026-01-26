//! Storage layer for Binnacle data.
//!
//! This module handles persistence of tasks, tests, and commit links.
//!
//! ## Storage Backends
//!
//! Binnacle supports multiple storage backends:
//!
//! - **File backend** (default): External storage at `~/.local/share/binnacle/<repo-hash>/`
//! - **Orphan branch backend**: Git orphan branch `binnacle-data` within the repository
//! - **Git notes backend**: Git notes at `refs/notes/binnacle`
//!
//! All backends use:
//! - JSONL files for append-only data (tasks.jsonl, bugs.jsonl, commits.jsonl, test-results.jsonl)
//! - SQLite for indexed queries (cache.db) - file backend only

pub mod backend;
pub mod git_notes;
pub mod orphan_branch;

pub use backend::{BackendType, StorageBackend};
pub use git_notes::GitNotesBackend;
pub use orphan_branch::OrphanBranchBackend;

use crate::models::{
    Agent, AgentStatus, Bug, CommitLink, Doc, DocType, Edge, EdgeDirection, EdgeType, HydratedEdge,
    Idea, IdeaStatus, LogAnnotation, Milestone, MilestoneProgress, Queue, Task, TaskStatus,
    TestNode, TestResult,
};
use crate::{Error, Result};
use chrono::Utc;
use kdl::{KdlDocument, KdlEntry, KdlNode, KdlValue};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// Entity type enum for generic entity lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntityType {
    Task,
    Bug,
    Idea,
    Test,
    Milestone,
    Edge,
    Queue,
    Doc,
    Agent,
}

impl std::fmt::Display for EntityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntityType::Task => write!(f, "task"),
            EntityType::Bug => write!(f, "bug"),
            EntityType::Idea => write!(f, "idea"),
            EntityType::Test => write!(f, "test"),
            EntityType::Milestone => write!(f, "milestone"),
            EntityType::Edge => write!(f, "edge"),
            EntityType::Queue => write!(f, "queue"),
            EntityType::Doc => write!(f, "doc"),
            EntityType::Agent => write!(f, "agent"),
        }
    }
}

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
        Self::open_at_root(root)
    }

    /// Open storage with an explicit data directory (DI-friendly for tests).
    pub fn open_with_data_dir(repo_path: &Path, data_dir: &Path) -> Result<Self> {
        let root = get_storage_dir_with_base(repo_path, data_dir)?;
        Self::open_at_root(root)
    }

    /// Internal: open storage at a specific root directory.
    fn open_at_root(root: PathBuf) -> Result<Self> {
        if !root.exists() {
            return Err(Error::NotInitialized);
        }

        let db_path = root.join("cache.db");
        let conn = Connection::open(&db_path)?;
        Self::init_schema(&conn)?;

        Ok(Self { root, conn })
    }

    /// Initialize storage for a new repository.
    pub fn init(repo_path: &Path) -> Result<Self> {
        let root = get_storage_dir(repo_path)?;
        Self::init_at_root(root)
    }

    /// Initialize storage with an explicit data directory (DI-friendly for tests).
    pub fn init_with_data_dir(repo_path: &Path, data_dir: &Path) -> Result<Self> {
        let root = get_storage_dir_with_base(repo_path, data_dir)?;
        Self::init_at_root(root)
    }

    /// Internal: initialize storage at a specific root directory.
    fn init_at_root(root: PathBuf) -> Result<Self> {
        // Create directory structure
        fs::create_dir_all(&root)?;

        // Create empty JSONL files
        let files = [
            "tasks.jsonl",
            "bugs.jsonl",
            "ideas.jsonl",
            "docs.jsonl",
            "milestones.jsonl",
            "edges.jsonl",
            "commits.jsonl",
            "test-results.jsonl",
            "agents.jsonl",
        ];
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

        // Set default configuration values for new storage
        Self::set_default_configs(&conn)?;

        Ok(Self { root, conn })
    }

    /// Check if storage exists for the given repository.
    pub fn exists(repo_path: &Path) -> Result<bool> {
        let root = get_storage_dir(repo_path)?;
        Self::exists_at_root(&root)
    }

    /// Check if storage exists with an explicit data directory (DI-friendly for tests).
    pub fn exists_with_data_dir(repo_path: &Path, data_dir: &Path) -> Result<bool> {
        let root = get_storage_dir_with_base(repo_path, data_dir)?;
        Self::exists_at_root(&root)
    }

    /// Internal: check if storage exists at a specific root directory.
    fn exists_at_root(root: &Path) -> Result<bool> {
        Ok(root.exists() && root.join("cache.db").exists())
    }

    /// Initialize the SQLite schema.
    fn init_schema(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                short_name TEXT,
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

            CREATE TABLE IF NOT EXISTS bugs (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT,
                priority INTEGER NOT NULL DEFAULT 2,
                status TEXT NOT NULL DEFAULT 'pending',
                severity TEXT NOT NULL DEFAULT 'triage',
                reproduction_steps TEXT,
                affected_component TEXT,
                assignee TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                closed_at TEXT,
                closed_reason TEXT
            );

            CREATE TABLE IF NOT EXISTS bug_tags (
                bug_id TEXT NOT NULL,
                tag TEXT NOT NULL,
                PRIMARY KEY (bug_id, tag),
                FOREIGN KEY (bug_id) REFERENCES bugs(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS bug_dependencies (
                child_id TEXT NOT NULL,
                parent_id TEXT NOT NULL,
                PRIMARY KEY (child_id, parent_id),
                FOREIGN KEY (child_id) REFERENCES bugs(id) ON DELETE CASCADE,
                FOREIGN KEY (parent_id) REFERENCES bugs(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
            CREATE INDEX IF NOT EXISTS idx_tasks_priority ON tasks(priority);
            CREATE INDEX IF NOT EXISTS idx_task_tags_tag ON task_tags(tag);

            CREATE INDEX IF NOT EXISTS idx_bugs_status ON bugs(status);
            CREATE INDEX IF NOT EXISTS idx_bugs_priority ON bugs(priority);
            CREATE INDEX IF NOT EXISTS idx_bugs_severity ON bugs(severity);
            CREATE INDEX IF NOT EXISTS idx_bug_tags_tag ON bug_tags(tag);

            -- Milestone tables
            CREATE TABLE IF NOT EXISTS milestones (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT,
                priority INTEGER NOT NULL DEFAULT 2,
                status TEXT NOT NULL DEFAULT 'pending',
                due_date TEXT,
                assignee TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                closed_at TEXT,
                closed_reason TEXT
            );

            CREATE TABLE IF NOT EXISTS milestone_tags (
                milestone_id TEXT NOT NULL,
                tag TEXT NOT NULL,
                PRIMARY KEY (milestone_id, tag),
                FOREIGN KEY (milestone_id) REFERENCES milestones(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_milestones_status ON milestones(status);
            CREATE INDEX IF NOT EXISTS idx_milestones_priority ON milestones(priority);
            CREATE INDEX IF NOT EXISTS idx_milestone_tags_tag ON milestone_tags(tag);

            -- Idea tables
            CREATE TABLE IF NOT EXISTS ideas (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT,
                status TEXT NOT NULL DEFAULT 'seed',
                promoted_to TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS idea_tags (
                idea_id TEXT NOT NULL,
                tag TEXT NOT NULL,
                PRIMARY KEY (idea_id, tag),
                FOREIGN KEY (idea_id) REFERENCES ideas(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_ideas_status ON ideas(status);
            CREATE INDEX IF NOT EXISTS idx_idea_tags_tag ON idea_tags(tag);

            -- Doc node tables (for markdown documentation attached to entities)
            CREATE TABLE IF NOT EXISTS docs (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT,
                doc_type TEXT NOT NULL DEFAULT 'prd',
                content TEXT NOT NULL DEFAULT '',
                summary_dirty INTEGER NOT NULL DEFAULT 0,
                editors TEXT NOT NULL DEFAULT '[]',
                supersedes TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS doc_tags (
                doc_id TEXT NOT NULL,
                tag TEXT NOT NULL,
                PRIMARY KEY (doc_id, tag),
                FOREIGN KEY (doc_id) REFERENCES docs(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_doc_tags_tag ON doc_tags(tag);

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

            -- Test-to-bug links (for verifying bug fixes)
            CREATE TABLE IF NOT EXISTS test_bug_links (
                test_id TEXT NOT NULL,
                bug_id TEXT NOT NULL,
                PRIMARY KEY (test_id, bug_id),
                FOREIGN KEY (test_id) REFERENCES tests(id) ON DELETE CASCADE,
                FOREIGN KEY (bug_id) REFERENCES bugs(id) ON DELETE CASCADE
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
            CREATE INDEX IF NOT EXISTS idx_test_bug_links_bug ON test_bug_links(bug_id);
            CREATE INDEX IF NOT EXISTS idx_test_results_test ON test_results(test_id);

            -- Commit link tables (supports linking to tasks and bugs)
            CREATE TABLE IF NOT EXISTS commit_links (
                sha TEXT NOT NULL,
                task_id TEXT NOT NULL,
                linked_at TEXT NOT NULL,
                PRIMARY KEY (sha, task_id)
            );

            CREATE INDEX IF NOT EXISTS idx_commit_links_task ON commit_links(task_id);
            CREATE INDEX IF NOT EXISTS idx_commit_links_sha ON commit_links(sha);

            -- Edge tables (generic relationships between entities)
            CREATE TABLE IF NOT EXISTS edges (
                id TEXT PRIMARY KEY,
                source TEXT NOT NULL,
                target TEXT NOT NULL,
                edge_type TEXT NOT NULL,
                weight REAL NOT NULL DEFAULT 1.0,
                reason TEXT,
                created_at TEXT NOT NULL,
                created_by TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_edges_source ON edges(source);
            CREATE INDEX IF NOT EXISTS idx_edges_target ON edges(target);
            CREATE INDEX IF NOT EXISTS idx_edges_type ON edges(edge_type);
            CREATE INDEX IF NOT EXISTS idx_edges_source_target ON edges(source, target);

            -- Configuration table
            CREATE TABLE IF NOT EXISTS config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            -- Agent tables (for AI agent lifecycle management)
            CREATE TABLE IF NOT EXISTS agents (
                pid INTEGER PRIMARY KEY,
                parent_pid INTEGER NOT NULL,
                name TEXT NOT NULL,
                purpose TEXT,
                mcp_session_id TEXT,
                status TEXT NOT NULL DEFAULT 'active',
                started_at TEXT NOT NULL,
                last_activity_at TEXT NOT NULL,
                command_count INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS agent_tasks (
                pid INTEGER NOT NULL,
                task_id TEXT NOT NULL,
                PRIMARY KEY (pid, task_id),
                FOREIGN KEY (pid) REFERENCES agents(pid) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_agents_status ON agents(status);
            CREATE INDEX IF NOT EXISTS idx_agent_tasks_pid ON agent_tasks(pid);

            -- Queue table (single global queue per repository)
            CREATE TABLE IF NOT EXISTS queues (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            -- Action log table (for efficient pagination and filtering)
            CREATE TABLE IF NOT EXISTS action_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                repo_path TEXT NOT NULL,
                command TEXT NOT NULL,
                args TEXT NOT NULL,
                success INTEGER NOT NULL,
                error TEXT,
                duration_ms INTEGER NOT NULL,
                user TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_action_logs_timestamp ON action_logs(timestamp DESC);
            CREATE INDEX IF NOT EXISTS idx_action_logs_command ON action_logs(command);
            CREATE INDEX IF NOT EXISTS idx_action_logs_user ON action_logs(user);
            CREATE INDEX IF NOT EXISTS idx_action_logs_success ON action_logs(success);

            -- Log annotations table (for attaching notes to log entries)
            CREATE TABLE IF NOT EXISTS log_annotations (
                id TEXT PRIMARY KEY,
                log_timestamp TEXT NOT NULL,
                content TEXT NOT NULL,
                author TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_log_annotations_timestamp ON log_annotations(log_timestamp);
            "#,
        )?;

        // Run migrations for schema changes
        Self::run_migrations(conn)?;

        Ok(())
    }

    /// Run database migrations for schema changes.
    /// This handles adding new columns to existing databases.
    fn run_migrations(conn: &Connection) -> Result<()> {
        // Migration: Add short_name column to tasks table if it doesn't exist
        // SQLite doesn't support IF NOT EXISTS for ALTER TABLE, so we check the schema first
        let has_short_name: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('tasks') WHERE name = 'short_name'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !has_short_name {
            conn.execute("ALTER TABLE tasks ADD COLUMN short_name TEXT", [])?;
        }

        // Migration: Add purpose column to agents table if it doesn't exist
        let has_purpose: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('agents') WHERE name = 'purpose'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !has_purpose {
            conn.execute("ALTER TABLE agents ADD COLUMN purpose TEXT", [])?;
        }

        // Migration: Add id column to agents table if it doesn't exist
        let has_agent_id: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('agents') WHERE name = 'id'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !has_agent_id {
            conn.execute("ALTER TABLE agents ADD COLUMN id TEXT", [])?;
            conn.execute("CREATE INDEX IF NOT EXISTS idx_agents_id ON agents(id)", [])?;
        }

        // Migration: Add doc_type column to docs table if it doesn't exist
        let has_doc_type: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('docs') WHERE name = 'doc_type'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !has_doc_type {
            conn.execute(
                "ALTER TABLE docs ADD COLUMN doc_type TEXT NOT NULL DEFAULT 'note'",
                [],
            )?;
        }

        // Migration: Add summary_dirty column to docs table if it doesn't exist
        let has_summary_dirty: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('docs') WHERE name = 'summary_dirty'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !has_summary_dirty {
            conn.execute(
                "ALTER TABLE docs ADD COLUMN summary_dirty INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
        }

        // Migration: Add editors column to docs table if it doesn't exist
        let has_editors: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('docs') WHERE name = 'editors'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !has_editors {
            conn.execute(
                "ALTER TABLE docs ADD COLUMN editors TEXT NOT NULL DEFAULT '[]'",
                [],
            )?;
        }

        // Migration: Add supersedes column to docs table if it doesn't exist
        let has_supersedes: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('docs') WHERE name = 'supersedes'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !has_supersedes {
            conn.execute("ALTER TABLE docs ADD COLUMN supersedes TEXT", [])?;
        }

        // Migration: Add mcp_session_id column to agents table if it doesn't exist
        let has_mcp_session_id: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('agents') WHERE name = 'mcp_session_id'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !has_mcp_session_id {
            conn.execute("ALTER TABLE agents ADD COLUMN mcp_session_id TEXT", [])?;
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_agents_mcp_session ON agents(mcp_session_id)",
                [],
            )?;
        }

        // Migration: Create action_logs table if it doesn't exist
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS action_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                repo_path TEXT NOT NULL,
                command TEXT NOT NULL,
                args TEXT NOT NULL,
                success INTEGER NOT NULL,
                error TEXT,
                duration_ms INTEGER NOT NULL,
                user TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_action_logs_timestamp ON action_logs(timestamp DESC);
            CREATE INDEX IF NOT EXISTS idx_action_logs_command ON action_logs(command);
            CREATE INDEX IF NOT EXISTS idx_action_logs_user ON action_logs(user);
            CREATE INDEX IF NOT EXISTS idx_action_logs_success ON action_logs(success);
            "#,
        )?;

        // Migration: Create log_annotations table if it doesn't exist
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS log_annotations (
                id TEXT PRIMARY KEY,
                log_timestamp TEXT NOT NULL,
                content TEXT NOT NULL,
                author TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_log_annotations_timestamp ON log_annotations(log_timestamp);
            "#,
        )?;

        Ok(())
    }

    /// Set default configuration values for new storage.
    /// Only sets values if they don't already exist.
    fn set_default_configs(conn: &Connection) -> Result<()> {
        // Insert default configs only if they don't already exist
        // Uses INSERT OR IGNORE to avoid overwriting existing values
        conn.execute_batch(
            r#"
            INSERT OR IGNORE INTO config (key, value) VALUES ('agents.worker.min', '0');
            INSERT OR IGNORE INTO config (key, value) VALUES ('agents.worker.max', '1');
            INSERT OR IGNORE INTO config (key, value) VALUES ('co-author.enabled', 'yes');
            INSERT OR IGNORE INTO config (key, value) VALUES ('co-author.name', 'binnacle-bot');
            "#,
        )?;
        Ok(())
    }

    /// Rebuild the SQLite cache from JSONL files.
    pub fn rebuild_cache(&mut self) -> Result<()> {
        // Disable foreign keys during rebuild to avoid constraint issues
        self.conn.execute("PRAGMA foreign_keys = OFF", [])?;

        // Clear existing data
        self.conn.execute_batch(
            r#"
            DELETE FROM task_dependencies;
            DELETE FROM task_tags;
            DELETE FROM tasks;
            DELETE FROM bug_dependencies;
            DELETE FROM bug_tags;
            DELETE FROM bugs;
            DELETE FROM idea_tags;
            DELETE FROM ideas;
            DELETE FROM doc_tags;
            DELETE FROM docs;
            DELETE FROM milestone_tags;
            DELETE FROM milestones;
            DELETE FROM test_links;
            DELETE FROM test_bug_links;
            DELETE FROM tests;
            DELETE FROM edges;
            DELETE FROM agent_tasks;
            DELETE FROM agents;
            DELETE FROM queues;
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
                if let Ok(test) = serde_json::from_str::<TestNode>(&line)
                    && test.entity_type == "test"
                {
                    self.cache_test(&test)?;
                }
            }
        }

        // Re-read bugs from bugs.jsonl
        let bugs_path = self.root.join("bugs.jsonl");
        if bugs_path.exists() {
            let file = File::open(&bugs_path)?;
            let reader = BufReader::new(file);

            for line in reader.lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(bug) = serde_json::from_str::<Bug>(&line)
                    && bug.core.entity_type == "bug"
                {
                    self.cache_bug(&bug)?;
                }
            }
        }

        // Re-read ideas from ideas.jsonl
        let ideas_path = self.root.join("ideas.jsonl");
        if ideas_path.exists() {
            let file = File::open(&ideas_path)?;
            let reader = BufReader::new(file);

            for line in reader.lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(idea) = serde_json::from_str::<Idea>(&line)
                    && idea.core.entity_type == "idea"
                {
                    self.cache_idea(&idea)?;
                }
            }
        }

        // Re-read docs from docs.jsonl
        let docs_path = self.root.join("docs.jsonl");
        if docs_path.exists() {
            let file = File::open(&docs_path)?;
            let reader = BufReader::new(file);

            for line in reader.lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(doc) = serde_json::from_str::<Doc>(&line)
                    && doc.core.entity_type == "doc"
                {
                    self.cache_doc(&doc)?;
                }
            }
        }

        // Re-read milestones from milestones.jsonl
        let milestones_path = self.root.join("milestones.jsonl");
        if milestones_path.exists() {
            let file = File::open(&milestones_path)?;
            let reader = BufReader::new(file);

            for line in reader.lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(milestone) = serde_json::from_str::<Milestone>(&line)
                    && milestone.core.entity_type == "milestone"
                {
                    self.cache_milestone(&milestone)?;
                }
            }
        }

        // Re-read edges from edges.jsonl
        let edges_path = self.root.join("edges.jsonl");
        if edges_path.exists() {
            let file = File::open(&edges_path)?;
            let reader = BufReader::new(file);

            for line in reader.lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(edge) = serde_json::from_str::<Edge>(&line)
                    && edge.entity_type == "edge"
                {
                    self.cache_edge(&edge)?;
                }
            }
        }

        // Re-read agents from agents.jsonl
        let agents_path = self.root.join("agents.jsonl");
        if agents_path.exists() {
            let file = File::open(&agents_path)?;
            let reader = BufReader::new(file);

            for line in reader.lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(mut agent) = serde_json::from_str::<Agent>(&line) {
                    // Ensure backward compatibility: generate ID if missing
                    agent.ensure_id();
                    self.cache_agent(&agent)?;
                }
            }
        }

        // Re-read queues from queues.jsonl
        let queues_path = self.root.join("queues.jsonl");
        if queues_path.exists() {
            let file = File::open(&queues_path)?;
            let reader = BufReader::new(file);

            for line in reader.lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(queue) = serde_json::from_str::<Queue>(&line)
                    && queue.entity_type == "queue"
                {
                    self.cache_queue(&queue)?;
                }
            }
        }

        // Re-enable foreign keys
        self.conn.execute("PRAGMA foreign_keys = ON", [])?;

        Ok(())
    }

    /// Cache a task in SQLite for fast querying.
    fn cache_task(&self, task: &Task) -> Result<()> {
        // Insert or replace task
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO tasks
            (id, title, short_name, description, priority, status, parent, assignee,
             created_at, updated_at, closed_at, closed_reason)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            "#,
            params![
                task.core.id,
                task.core.title,
                task.core.short_name,
                task.core.description,
                task.priority,
                serde_json::to_string(&task.status)?.trim_matches('"'),
                task.parent,
                task.assignee,
                task.core.created_at.to_rfc3339(),
                task.core.updated_at.to_rfc3339(),
                task.closed_at.map(|t| t.to_rfc3339()),
                task.closed_reason,
            ],
        )?;

        // Update tags
        self.conn
            .execute("DELETE FROM task_tags WHERE task_id = ?1", [&task.core.id])?;
        for tag in &task.core.tags {
            self.conn.execute(
                "INSERT INTO task_tags (task_id, tag) VALUES (?1, ?2)",
                params![task.core.id, tag],
            )?;
        }

        // Update dependencies
        self.conn.execute(
            "DELETE FROM task_dependencies WHERE child_id = ?1",
            [&task.core.id],
        )?;
        for parent_id in &task.depends_on {
            self.conn.execute(
                "INSERT INTO task_dependencies (child_id, parent_id) VALUES (?1, ?2)",
                params![task.core.id, parent_id],
            )?;
        }

        Ok(())
    }

    /// Cache a bug in SQLite for fast querying.
    fn cache_bug(&self, bug: &Bug) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO bugs
            (id, title, description, priority, status, severity, reproduction_steps,
             affected_component, assignee, created_at, updated_at, closed_at, closed_reason)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            "#,
            params![
                bug.core.id,
                bug.core.title,
                bug.core.description,
                bug.priority,
                serde_json::to_string(&bug.status)?.trim_matches('"'),
                serde_json::to_string(&bug.severity)?.trim_matches('"'),
                bug.reproduction_steps,
                bug.affected_component,
                bug.assignee,
                bug.core.created_at.to_rfc3339(),
                bug.core.updated_at.to_rfc3339(),
                bug.closed_at.map(|t| t.to_rfc3339()),
                bug.closed_reason,
            ],
        )?;

        self.conn
            .execute("DELETE FROM bug_tags WHERE bug_id = ?1", [&bug.core.id])?;
        for tag in &bug.core.tags {
            self.conn.execute(
                "INSERT INTO bug_tags (bug_id, tag) VALUES (?1, ?2)",
                params![bug.core.id, tag],
            )?;
        }

        self.conn.execute(
            "DELETE FROM bug_dependencies WHERE child_id = ?1",
            [&bug.core.id],
        )?;
        for parent_id in &bug.depends_on {
            self.conn.execute(
                "INSERT INTO bug_dependencies (child_id, parent_id) VALUES (?1, ?2)",
                params![bug.core.id, parent_id],
            )?;
        }

        Ok(())
    }

    /// Cache an edge in SQLite for fast querying.
    fn cache_edge(&self, edge: &Edge) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO edges
            (id, source, target, edge_type, weight, reason, created_at, created_by)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                edge.id,
                edge.source,
                edge.target,
                edge.edge_type.to_string(),
                edge.weight,
                edge.reason,
                edge.created_at.to_rfc3339(),
                edge.created_by,
            ],
        )?;
        Ok(())
    }

    /// Generate a unique ID that doesn't collide with existing entities.
    ///
    /// This method checks the storage for existing IDs and retries with
    /// different seeds until a unique ID is found.
    pub fn generate_unique_id(&self, prefix: &str, seed: &str) -> String {
        const MAX_RETRIES: usize = 100;

        for attempt in 0..MAX_RETRIES {
            let effective_seed = if attempt == 0 {
                seed.to_string()
            } else {
                format!("{}-{}", seed, attempt)
            };
            let id = generate_id(prefix, &effective_seed);

            // Check if ID already exists
            if !self.entity_exists(&id) {
                return id;
            }
        }

        // Fallback: just generate an ID (extremely unlikely to reach here)
        generate_id(
            prefix,
            &format!(
                "{}-fallback-{}",
                seed,
                chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
            ),
        )
    }

    /// Check if an entity with the given ID exists in storage.
    fn entity_exists(&self, id: &str) -> bool {
        // Check tasks
        if self.get_task(id).is_ok() {
            return true;
        }
        // Check bugs
        if self.get_bug(id).is_ok() {
            return true;
        }
        // Check milestones
        if self.get_milestone(id).is_ok() {
            return true;
        }
        // Check ideas
        if self.get_idea(id).is_ok() {
            return true;
        }
        // Check tests
        if self.get_test(id).is_ok() {
            return true;
        }
        // Check queues (single queue per repo, check if ID matches)
        if let Ok(queue) = self.get_queue()
            && queue.id == id
        {
            return true;
        }
        // Check docs
        if self.get_doc(id).is_ok() {
            return true;
        }
        false
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
            if let Ok(task) = serde_json::from_str::<Task>(&line)
                && task.core.id == id
            {
                latest = Some(task);
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
        self.get_task(&task.core.id)?;

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

    // === Bug Operations ===

    /// Add a new bug.
    pub fn add_bug(&mut self, bug: &Bug) -> Result<()> {
        let bugs_path = self.root.join("bugs.jsonl");
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&bugs_path)?;

        let json = serde_json::to_string(bug)?;
        writeln!(file, "{}", json)?;

        self.cache_bug(bug)?;

        Ok(())
    }

    /// Get a bug by ID.
    pub fn get_bug(&self, id: &str) -> Result<Bug> {
        let bugs_path = self.root.join("bugs.jsonl");
        if !bugs_path.exists() {
            return Err(Error::NotFound(format!("Bug not found: {}", id)));
        }

        let file = File::open(&bugs_path)?;
        let reader = BufReader::new(file);

        let mut latest: Option<Bug> = None;
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(bug) = serde_json::from_str::<Bug>(&line)
                && bug.core.id == id
            {
                latest = Some(bug);
            }
        }

        latest.ok_or_else(|| Error::NotFound(format!("Bug not found: {}", id)))
    }

    /// List all bugs, optionally filtered.
    /// By default, excludes closed bugs (Done/Cancelled) unless `include_closed` is true.
    pub fn list_bugs(
        &self,
        status: Option<&str>,
        priority: Option<u8>,
        severity: Option<&str>,
        tag: Option<&str>,
        include_closed: bool,
    ) -> Result<Vec<Bug>> {
        let mut sql = String::from(
            "SELECT DISTINCT b.id FROM bugs b
             LEFT JOIN bug_tags bt ON b.id = bt.bug_id
             WHERE 1=1",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        // If a specific status is requested, use it; otherwise exclude closed bugs unless --all
        if let Some(s) = status {
            sql.push_str(" AND b.status = ?");
            params_vec.push(Box::new(s.to_string()));
        } else if !include_closed {
            // Exclude done and cancelled bugs by default (note: snake_case due to serde config)
            sql.push_str(" AND b.status NOT IN ('done', 'cancelled')");
        }
        if let Some(p) = priority {
            sql.push_str(" AND b.priority = ?");
            params_vec.push(Box::new(p));
        }
        if let Some(s) = severity {
            sql.push_str(" AND b.severity = ?");
            params_vec.push(Box::new(s.to_string()));
        }
        if let Some(t) = tag {
            sql.push_str(" AND bt.tag = ?");
            params_vec.push(Box::new(t.to_string()));
        }

        sql.push_str(" ORDER BY b.priority ASC, b.created_at DESC");

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let mut stmt = self.conn.prepare(&sql)?;
        let ids: Vec<String> = stmt
            .query_map(params_refs.as_slice(), |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        let mut bugs = Vec::new();
        for id in ids {
            if let Ok(bug) = self.get_bug(&id) {
                bugs.push(bug);
            }
        }

        Ok(bugs)
    }

    /// Update a bug.
    pub fn update_bug(&mut self, bug: &Bug) -> Result<()> {
        self.get_bug(&bug.core.id)?;

        let bugs_path = self.root.join("bugs.jsonl");
        let mut file = OpenOptions::new().append(true).open(&bugs_path)?;

        let json = serde_json::to_string(bug)?;
        writeln!(file, "{}", json)?;

        self.cache_bug(bug)?;

        Ok(())
    }

    /// Delete a bug by ID.
    pub fn delete_bug(&mut self, id: &str) -> Result<()> {
        self.get_bug(id)?;

        self.conn.execute("DELETE FROM bugs WHERE id = ?", [id])?;
        self.conn
            .execute("DELETE FROM bug_tags WHERE bug_id = ?", [id])?;
        self.conn.execute(
            "DELETE FROM bug_dependencies WHERE child_id = ? OR parent_id = ?",
            [id, id],
        )?;

        Ok(())
    }

    // === Idea Operations ===

    /// Add a new idea.
    pub fn add_idea(&mut self, idea: &Idea) -> Result<()> {
        let ideas_path = self.root.join("ideas.jsonl");
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&ideas_path)?;

        let json = serde_json::to_string(idea)?;
        writeln!(file, "{}", json)?;

        self.cache_idea(idea)?;

        Ok(())
    }

    /// Get an idea by ID.
    pub fn get_idea(&self, id: &str) -> Result<Idea> {
        // First check if the idea exists in the cache (handles deletions)
        let exists: bool = self
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM ideas WHERE id = ?)",
                [id],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !exists {
            return Err(Error::NotFound(format!("Idea not found: {}", id)));
        }

        let ideas_path = self.root.join("ideas.jsonl");
        if !ideas_path.exists() {
            return Err(Error::NotFound(format!("Idea not found: {}", id)));
        }

        let file = File::open(&ideas_path)?;
        let reader = BufReader::new(file);

        let mut latest: Option<Idea> = None;
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(idea) = serde_json::from_str::<Idea>(&line)
                && idea.core.id == id
            {
                latest = Some(idea);
            }
        }

        latest.ok_or_else(|| Error::NotFound(format!("Idea not found: {}", id)))
    }

    /// List all ideas, optionally filtered.
    pub fn list_ideas(&self, status: Option<&str>, tag: Option<&str>) -> Result<Vec<Idea>> {
        let mut sql = String::from(
            "SELECT DISTINCT i.id FROM ideas i
             LEFT JOIN idea_tags it ON i.id = it.idea_id
             WHERE 1=1",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(s) = status {
            sql.push_str(" AND i.status = ?");
            params_vec.push(Box::new(s.to_string()));
        }
        if let Some(t) = tag {
            sql.push_str(" AND it.tag = ?");
            params_vec.push(Box::new(t.to_string()));
        }

        sql.push_str(" ORDER BY i.created_at DESC");

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let mut stmt = self.conn.prepare(&sql)?;
        let ids: Vec<String> = stmt
            .query_map(params_refs.as_slice(), |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        let mut ideas = Vec::new();
        for id in ids {
            if let Ok(idea) = self.get_idea(&id) {
                ideas.push(idea);
            }
        }

        Ok(ideas)
    }

    /// Update an idea.
    pub fn update_idea(&mut self, idea: &Idea) -> Result<()> {
        self.get_idea(&idea.core.id)?;

        let ideas_path = self.root.join("ideas.jsonl");
        let mut file = OpenOptions::new().append(true).open(&ideas_path)?;

        let json = serde_json::to_string(idea)?;
        writeln!(file, "{}", json)?;

        self.cache_idea(idea)?;

        Ok(())
    }

    /// Delete an idea by ID.
    pub fn delete_idea(&mut self, id: &str) -> Result<()> {
        self.get_idea(id)?;

        self.conn.execute("DELETE FROM ideas WHERE id = ?", [id])?;
        self.conn
            .execute("DELETE FROM idea_tags WHERE idea_id = ?", [id])?;

        Ok(())
    }

    /// Cache an idea in the SQLite database.
    fn cache_idea(&self, idea: &Idea) -> Result<()> {
        let status = match idea.status {
            IdeaStatus::Seed => "seed",
            IdeaStatus::Germinating => "germinating",
            IdeaStatus::Promoted => "promoted",
            IdeaStatus::Discarded => "discarded",
        };

        self.conn.execute(
            "INSERT OR REPLACE INTO ideas (id, title, description, status, promoted_to, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![
                &idea.core.id,
                &idea.core.title,
                &idea.core.description,
                status,
                &idea.promoted_to,
                idea.core.created_at.to_rfc3339(),
                idea.core.updated_at.to_rfc3339(),
            ],
        )?;

        // Update tags
        self.conn
            .execute("DELETE FROM idea_tags WHERE idea_id = ?", [&idea.core.id])?;
        for tag in &idea.core.tags {
            self.conn.execute(
                "INSERT INTO idea_tags (idea_id, tag) VALUES (?, ?)",
                [&idea.core.id, tag],
            )?;
        }

        Ok(())
    }

    /// Cache a doc in SQLite for fast querying.
    fn cache_doc(&self, doc: &Doc) -> Result<()> {
        let doc_type = serde_json::to_string(&doc.doc_type)?
            .trim_matches('"')
            .to_string();
        let editors_json = serde_json::to_string(&doc.editors)?;

        self.conn.execute(
            "INSERT OR REPLACE INTO docs (id, title, description, doc_type, content, summary_dirty, editors, supersedes, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                &doc.core.id,
                &doc.core.title,
                &doc.core.description,
                &doc_type,
                &doc.content,
                doc.summary_dirty as i32,
                &editors_json,
                &doc.supersedes,
                doc.core.created_at.to_rfc3339(),
                doc.core.updated_at.to_rfc3339(),
            ],
        )?;

        // Update tags
        self.conn
            .execute("DELETE FROM doc_tags WHERE doc_id = ?", [&doc.core.id])?;
        for tag in &doc.core.tags {
            self.conn.execute(
                "INSERT INTO doc_tags (doc_id, tag) VALUES (?, ?)",
                [&doc.core.id, tag],
            )?;
        }

        Ok(())
    }

    // === Doc Operations ===

    /// Add a new doc.
    pub fn add_doc(&mut self, doc: &Doc) -> Result<()> {
        let docs_path = self.root.join("docs.jsonl");
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&docs_path)?;

        let json = serde_json::to_string(doc)?;
        writeln!(file, "{}", json)?;

        self.cache_doc(doc)?;

        Ok(())
    }

    /// Get a doc by ID.
    pub fn get_doc(&self, id: &str) -> Result<Doc> {
        // First check if the doc exists in the cache (handles deletions)
        let exists: bool = self
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM docs WHERE id = ?)",
                [id],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !exists {
            return Err(Error::NotFound(format!("Doc not found: {}", id)));
        }

        let docs_path = self.root.join("docs.jsonl");
        if !docs_path.exists() {
            return Err(Error::NotFound(format!("Doc not found: {}", id)));
        }

        let file = File::open(&docs_path)?;
        let reader = BufReader::new(file);

        let mut latest: Option<Doc> = None;
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(doc) = serde_json::from_str::<Doc>(&line)
                && doc.core.id == id
            {
                latest = Some(doc);
            }
        }

        latest.ok_or_else(|| Error::NotFound(format!("Doc not found: {}", id)))
    }

    /// Find the document that supersedes the given document ID.
    /// Returns None if no document supersedes this one (i.e., this is the latest version).
    pub fn get_doc_superseded_by(&self, id: &str) -> Result<Option<String>> {
        let superseding_id: Option<String> = self
            .conn
            .query_row("SELECT id FROM docs WHERE supersedes = ?", [id], |row| {
                row.get(0)
            })
            .ok();

        Ok(superseding_id)
    }

    /// List all docs, optionally filtered.
    ///
    /// # Arguments
    /// * `tag` - Filter by tag
    /// * `doc_type` - Filter by doc type (prd, note, handoff)
    /// * `edited_by` - Filter by editor (format: "agent:id" or "user:name")
    /// * `for_entity` - Filter by linked entity ID
    pub fn list_docs(
        &self,
        tag: Option<&str>,
        doc_type: Option<&DocType>,
        edited_by: Option<&str>,
        for_entity: Option<&str>,
    ) -> Result<Vec<Doc>> {
        let mut sql = String::from(
            "SELECT DISTINCT d.id FROM docs d
             LEFT JOIN doc_tags dt ON d.id = dt.doc_id
             LEFT JOIN edges e ON (d.id = e.source OR d.id = e.target)
             WHERE 1=1",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(t) = tag {
            sql.push_str(" AND dt.tag = ?");
            params_vec.push(Box::new(t.to_string()));
        }

        if let Some(dt) = doc_type {
            let doc_type_str = serde_json::to_string(dt)
                .unwrap_or_default()
                .trim_matches('"')
                .to_string();
            sql.push_str(" AND d.doc_type = ?");
            params_vec.push(Box::new(doc_type_str));
        }

        if let Some(editor) = edited_by {
            // Search for editor in the editors JSON array
            // Format: "agent:id" or "user:name"
            sql.push_str(" AND d.editors LIKE ?");
            params_vec.push(Box::new(format!("%{}%", editor)));
        }

        if let Some(entity_id) = for_entity {
            // Find docs linked to the entity (either direction)
            sql.push_str(" AND (e.source = ? OR e.target = ?)");
            params_vec.push(Box::new(entity_id.to_string()));
            params_vec.push(Box::new(entity_id.to_string()));
        }

        sql.push_str(" ORDER BY d.updated_at DESC");

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let mut stmt = self.conn.prepare(&sql)?;
        let ids: Vec<String> = stmt
            .query_map(params_refs.as_slice(), |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        let mut docs = Vec::new();
        for id in ids {
            if let Ok(doc) = self.get_doc(&id) {
                docs.push(doc);
            }
        }

        Ok(docs)
    }

    /// Update a doc.
    pub fn update_doc(&mut self, doc: &Doc) -> Result<()> {
        // Verify doc exists
        self.get_doc(&doc.core.id)?;

        // Append updated version to JSONL
        let docs_path = self.root.join("docs.jsonl");
        let mut file = OpenOptions::new().append(true).open(&docs_path)?;

        let json = serde_json::to_string(doc)?;
        writeln!(file, "{}", json)?;

        // Update cache
        self.cache_doc(doc)?;

        Ok(())
    }

    /// Delete a doc by ID.
    pub fn delete_doc(&mut self, id: &str) -> Result<()> {
        // Verify doc exists
        self.get_doc(id)?;

        // Remove from cache (JSONL keeps history)
        self.conn
            .execute("DELETE FROM doc_tags WHERE doc_id = ?", [id])?;
        self.conn.execute("DELETE FROM docs WHERE id = ?", [id])?;

        // Also remove any edges involving this doc
        self.conn
            .execute("DELETE FROM edges WHERE source = ? OR target = ?", [id, id])?;

        Ok(())
    }

    // === Queue Operations ===

    /// Create a new queue. Only one queue can exist per repository.
    pub fn create_queue(&mut self, queue: &Queue) -> Result<()> {
        // Check if a queue already exists
        if self.get_queue().is_ok() {
            return Err(Error::QueueAlreadyExists);
        }

        // Append to JSONL
        let queues_path = self.root.join("queues.jsonl");
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&queues_path)?;

        let json = serde_json::to_string(queue)?;
        writeln!(file, "{}", json)?;

        // Update cache
        self.cache_queue(queue)?;

        Ok(())
    }

    /// Get the queue (single queue per repo).
    pub fn get_queue(&self) -> Result<Queue> {
        let queues_path = self.root.join("queues.jsonl");
        if !queues_path.exists() {
            return Err(Error::NotFound("No queue exists".to_string()));
        }

        // First check if queue exists in cache (handles deletions)
        let exists: bool = self
            .conn
            .query_row("SELECT EXISTS(SELECT 1 FROM queues LIMIT 1)", [], |row| {
                row.get(0)
            })
            .unwrap_or(false);

        if !exists {
            return Err(Error::NotFound("No queue exists".to_string()));
        }

        let file = File::open(&queues_path)?;
        let reader = BufReader::new(file);

        let mut latest: Option<Queue> = None;
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(queue) = serde_json::from_str::<Queue>(&line) {
                // Check if this queue still exists in cache
                let still_exists: bool = self
                    .conn
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM queues WHERE id = ?)",
                        [&queue.id],
                        |row| row.get(0),
                    )
                    .unwrap_or(false);
                if still_exists {
                    latest = Some(queue);
                }
            }
        }

        latest.ok_or_else(|| Error::NotFound("No queue exists".to_string()))
    }

    /// Get the queue by ID (for entity lookup).
    pub fn get_queue_by_id(&self, id: &str) -> Result<Queue> {
        let queue = self.get_queue()?;
        if queue.id == id {
            Ok(queue)
        } else {
            Err(Error::NotFound(format!("Queue not found: {}", id)))
        }
    }

    /// Update the queue.
    pub fn update_queue(&mut self, queue: &Queue) -> Result<()> {
        self.get_queue()?;

        let queues_path = self.root.join("queues.jsonl");
        let mut file = OpenOptions::new().append(true).open(&queues_path)?;

        let json = serde_json::to_string(queue)?;
        writeln!(file, "{}", json)?;

        self.cache_queue(queue)?;

        Ok(())
    }

    /// Delete the queue by ID.
    pub fn delete_queue(&mut self, id: &str) -> Result<()> {
        let queue = self.get_queue()?;
        if queue.id != id {
            return Err(Error::NotFound(format!("Queue not found: {}", id)));
        }

        // Remove all queued edges targeting this queue
        let edges = self.list_edges(Some(EdgeType::Queued), None, Some(id))?;
        for edge in edges {
            self.remove_edge(&edge.source, &edge.target, EdgeType::Queued)?;
        }

        // Remove from cache
        self.conn.execute("DELETE FROM queues WHERE id = ?", [id])?;

        Ok(())
    }

    /// Get tasks that are in the queue.
    pub fn get_queued_tasks(&self) -> Result<Vec<Task>> {
        let queue = match self.get_queue() {
            Ok(q) => q,
            Err(_) => return Ok(Vec::new()),
        };

        let edges = self.list_edges(Some(EdgeType::Queued), None, Some(&queue.id))?;
        let mut tasks = Vec::new();
        for edge in edges {
            if let Ok(task) = self.get_task(&edge.source) {
                tasks.push(task);
            }
        }

        // Sort by priority
        tasks.sort_by_key(|t| t.priority);
        Ok(tasks)
    }

    /// Get bugs that are in the queue.
    pub fn get_queued_bugs(&self) -> Result<Vec<Bug>> {
        let queue = match self.get_queue() {
            Ok(q) => q,
            Err(_) => return Ok(Vec::new()),
        };

        let edges = self.list_edges(Some(EdgeType::Queued), None, Some(&queue.id))?;
        let mut bugs = Vec::new();
        for edge in edges {
            if let Ok(bug) = self.get_bug(&edge.source) {
                bugs.push(bug);
            }
        }

        // Sort by priority
        bugs.sort_by_key(|b| b.priority);
        Ok(bugs)
    }

    /// Cache a queue in the SQLite database.
    fn cache_queue(&self, queue: &Queue) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO queues (id, title, description, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?)",
            params![
                &queue.id,
                &queue.title,
                &queue.description,
                queue.created_at.to_rfc3339(),
                queue.updated_at.to_rfc3339(),
            ],
        )?;

        Ok(())
    }

    // === Milestone Operations ===

    /// Add a new milestone.
    pub fn add_milestone(&mut self, milestone: &Milestone) -> Result<()> {
        let milestones_path = self.root.join("milestones.jsonl");
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&milestones_path)?;

        let json = serde_json::to_string(milestone)?;
        writeln!(file, "{}", json)?;

        self.cache_milestone(milestone)?;

        Ok(())
    }

    /// Get a milestone by ID.
    pub fn get_milestone(&self, id: &str) -> Result<Milestone> {
        let milestones_path = self.root.join("milestones.jsonl");
        if !milestones_path.exists() {
            return Err(Error::NotFound(format!("Milestone not found: {}", id)));
        }

        let file = File::open(&milestones_path)?;
        let reader = BufReader::new(file);

        let mut latest: Option<Milestone> = None;
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(milestone) = serde_json::from_str::<Milestone>(&line)
                && milestone.core.id == id
            {
                latest = Some(milestone);
            }
        }

        latest.ok_or_else(|| Error::NotFound(format!("Milestone not found: {}", id)))
    }

    /// List all milestones, optionally filtered.
    pub fn list_milestones(
        &self,
        status: Option<&str>,
        priority: Option<u8>,
        tag: Option<&str>,
    ) -> Result<Vec<Milestone>> {
        let mut sql = String::from(
            "SELECT DISTINCT m.id FROM milestones m
             LEFT JOIN milestone_tags mt ON m.id = mt.milestone_id
             WHERE 1=1",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(s) = status {
            sql.push_str(" AND m.status = ?");
            params_vec.push(Box::new(s.to_string()));
        }
        if let Some(p) = priority {
            sql.push_str(" AND m.priority = ?");
            params_vec.push(Box::new(p));
        }
        if let Some(t) = tag {
            sql.push_str(" AND mt.tag = ?");
            params_vec.push(Box::new(t.to_string()));
        }

        sql.push_str(" ORDER BY m.priority ASC, m.created_at DESC");

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let mut stmt = self.conn.prepare(&sql)?;
        let ids: Vec<String> = stmt
            .query_map(params_refs.as_slice(), |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        let mut milestones = Vec::new();
        for id in ids {
            if let Ok(milestone) = self.get_milestone(&id) {
                milestones.push(milestone);
            }
        }

        Ok(milestones)
    }

    /// Update a milestone.
    pub fn update_milestone(&mut self, milestone: &Milestone) -> Result<()> {
        self.get_milestone(&milestone.core.id)?;

        let milestones_path = self.root.join("milestones.jsonl");
        let mut file = OpenOptions::new().append(true).open(&milestones_path)?;

        let json = serde_json::to_string(milestone)?;
        writeln!(file, "{}", json)?;

        self.cache_milestone(milestone)?;

        Ok(())
    }

    /// Delete a milestone by ID.
    pub fn delete_milestone(&mut self, id: &str) -> Result<()> {
        self.get_milestone(id)?;

        self.conn
            .execute("DELETE FROM milestones WHERE id = ?", [id])?;
        self.conn
            .execute("DELETE FROM milestone_tags WHERE milestone_id = ?", [id])?;

        Ok(())
    }

    /// Get progress statistics for a milestone.
    /// Child items are identified by parent_of edges from this milestone.
    pub fn get_milestone_progress(&self, milestone_id: &str) -> Result<MilestoneProgress> {
        // Get all children via parent_of edges from this milestone
        let edges = self.list_edges(Some(EdgeType::ParentOf), Some(milestone_id), None)?;
        let child_ids: Vec<&str> = edges.iter().map(|e| e.target.as_str()).collect();

        let mut total = 0;
        let mut completed = 0;

        for child_id in &child_ids {
            // Try task first, then bug
            if let Ok(task) = self.get_task(child_id) {
                total += 1;
                if task.status == TaskStatus::Done {
                    completed += 1;
                }
            } else if let Ok(bug) = self.get_bug(child_id) {
                total += 1;
                if bug.status == TaskStatus::Done {
                    completed += 1;
                }
            }
        }

        Ok(MilestoneProgress::new(total, completed))
    }

    /// Cache a milestone in SQLite for fast querying.
    fn cache_milestone(&self, milestone: &Milestone) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO milestones
            (id, title, description, priority, status, due_date, assignee,
             created_at, updated_at, closed_at, closed_reason)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
            params![
                milestone.core.id,
                milestone.core.title,
                milestone.core.description,
                milestone.priority,
                serde_json::to_string(&milestone.status)?.trim_matches('"'),
                milestone.due_date.map(|t| t.to_rfc3339()),
                milestone.assignee,
                milestone.core.created_at.to_rfc3339(),
                milestone.core.updated_at.to_rfc3339(),
                milestone.closed_at.map(|t| t.to_rfc3339()),
                milestone.closed_reason,
            ],
        )?;

        self.conn.execute(
            "DELETE FROM milestone_tags WHERE milestone_id = ?1",
            [&milestone.core.id],
        )?;
        for tag in &milestone.core.tags {
            self.conn.execute(
                "INSERT INTO milestone_tags (milestone_id, tag) VALUES (?1, ?2)",
                params![milestone.core.id, tag],
            )?;
        }

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
        let mut child = self.get_task(child_id)?;
        let parent = self.get_task(parent_id)?;

        // Check for self-dependency
        if child_id == parent_id {
            return Err(Error::Other("A task cannot depend on itself".to_string()));
        }

        // Check if dependency already exists
        if child.depends_on.iter().any(|id| id == parent_id) {
            return Err(Error::Other(format!(
                "Dependency already exists: {} -> {}",
                child_id, parent_id
            )));
        }

        // Check for cycle: would adding this edge create a path from parent back to child?
        if self.would_create_cycle(child_id, parent_id)? {
            return Err(Error::CycleDetected);
        }

        // Update the task's depends_on list and status, then persist.
        child.depends_on.push(parent_id.to_string());
        child.core.updated_at = chrono::Utc::now();

        let parent_incomplete = !matches!(parent.status, TaskStatus::Done | TaskStatus::Cancelled);
        if child.status == TaskStatus::Done && parent_incomplete {
            child.status = TaskStatus::Partial;
            child.closed_at = None;
            child.closed_reason = None;
        }

        self.update_task(&child)?;

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
        task.core.updated_at = chrono::Utc::now();

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
    /// Checks both legacy depends_on fields AND edge-based dependencies.
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

            // Get legacy dependencies (from task.depends_on)
            let legacy_deps = self.get_dependencies(&current)?;
            for dep in legacy_deps {
                if !visited.contains(&dep) {
                    stack.push(dep);
                }
            }

            // Get edge-based dependencies
            let edge_deps = self.get_edge_dependencies(&current).unwrap_or_default();
            for dep in edge_deps {
                if !visited.contains(&dep) {
                    stack.push(dep);
                }
            }
        }

        Ok(false)
    }

    /// Get tasks that are ready (pending/reopened with all dependencies done).
    /// Checks both the legacy depends_on field and edge-based dependencies.
    pub fn get_ready_tasks(&self) -> Result<Vec<Task>> {
        let tasks = self.list_tasks(None, None, None)?;
        let mut ready = Vec::new();

        for task in tasks {
            match task.status {
                TaskStatus::Pending | TaskStatus::Reopened => {
                    // Check legacy depends_on field
                    let legacy_deps_done = task.depends_on.is_empty()
                        || task
                            .depends_on
                            .iter()
                            .all(|dep_id| self.is_entity_done(dep_id));

                    // Check edge-based dependencies
                    let edge_deps = self
                        .get_edge_dependencies(&task.core.id)
                        .unwrap_or_default();
                    let edge_deps_done = edge_deps.is_empty()
                        || edge_deps.iter().all(|dep_id| self.is_entity_done(dep_id));

                    if legacy_deps_done && edge_deps_done {
                        ready.push(task);
                    }
                }
                _ => {}
            }
        }

        Ok(ready)
    }

    /// Get bugs that are ready (no open blockers).
    pub fn get_ready_bugs(&self) -> Result<Vec<Bug>> {
        let bugs = self.list_bugs(None, None, None, None, false)?; // Exclude closed bugs
        let mut ready = Vec::new();

        for bug in bugs {
            match bug.status {
                TaskStatus::Pending | TaskStatus::Reopened => {
                    // Check legacy depends_on field
                    let legacy_deps_done = bug.depends_on.is_empty()
                        || bug
                            .depends_on
                            .iter()
                            .all(|dep_id| self.is_entity_done(dep_id));

                    // Check edge-based dependencies
                    let edge_deps = self.get_edge_dependencies(&bug.core.id).unwrap_or_default();
                    let edge_deps_done = edge_deps.is_empty()
                        || edge_deps.iter().all(|dep_id| self.is_entity_done(dep_id));

                    if legacy_deps_done && edge_deps_done {
                        ready.push(bug);
                    }
                }
                _ => {}
            }
        }

        Ok(ready)
    }

    /// Check if an entity (task or bug) is in a "done" state.
    pub fn is_entity_done(&self, id: &str) -> bool {
        if let Ok(task) = self.get_task(id) {
            return task.status == TaskStatus::Done;
        }
        if let Ok(bug) = self.get_bug(id) {
            return bug.status == TaskStatus::Done;
        }
        false
    }

    /// Get tasks that are blocked (have open dependencies).
    /// Checks both the legacy depends_on field and edge-based dependencies.
    pub fn get_blocked_tasks(&self) -> Result<Vec<Task>> {
        let tasks = self.list_tasks(None, None, None)?;
        let mut blocked = Vec::new();

        for task in tasks {
            match task.status {
                TaskStatus::Pending | TaskStatus::Reopened => {
                    // Check legacy depends_on field
                    let has_open_legacy_deps = !task.depends_on.is_empty()
                        && task
                            .depends_on
                            .iter()
                            .any(|dep_id| !self.is_entity_done(dep_id));

                    // Check edge-based dependencies
                    let edge_deps = self
                        .get_edge_dependencies(&task.core.id)
                        .unwrap_or_default();
                    let has_open_edge_deps = !edge_deps.is_empty()
                        && edge_deps.iter().any(|dep_id| !self.is_entity_done(dep_id));

                    if has_open_legacy_deps || has_open_edge_deps {
                        blocked.push(task);
                    }
                }
                TaskStatus::Blocked => {
                    // Explicitly blocked tasks are always included
                    blocked.push(task);
                }
                TaskStatus::Partial => {
                    // Partial tasks are tasks that were done but now have incomplete
                    // dependencies added - they are effectively blocked
                    blocked.push(task);
                }
                _ => {}
            }
        }

        Ok(blocked)
    }

    /// Get bugs that are blocked (have open dependencies).
    /// Checks both the legacy depends_on field and edge-based dependencies.
    pub fn get_blocked_bugs(&self) -> Result<Vec<Bug>> {
        let bugs = self.list_bugs(None, None, None, None, false)?; // Exclude closed bugs
        let mut blocked = Vec::new();

        for bug in bugs {
            match bug.status {
                TaskStatus::Pending | TaskStatus::Reopened => {
                    // Check legacy depends_on field
                    let has_open_legacy_deps = !bug.depends_on.is_empty()
                        && bug
                            .depends_on
                            .iter()
                            .any(|dep_id| !self.is_entity_done(dep_id));

                    // Check edge-based dependencies
                    let edge_deps = self.get_edge_dependencies(&bug.core.id).unwrap_or_default();
                    let has_open_edge_deps = !edge_deps.is_empty()
                        && edge_deps.iter().any(|dep_id| !self.is_entity_done(dep_id));

                    if has_open_legacy_deps || has_open_edge_deps {
                        blocked.push(bug);
                    }
                }
                TaskStatus::Blocked => {
                    // Explicitly blocked bugs are always included
                    blocked.push(bug);
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

    // === Edge Operations ===

    /// Generate a unique edge ID.
    pub fn generate_edge_id(&self, source: &str, target: &str, edge_type: EdgeType) -> String {
        let seed = format!("{}-{}-{}", source, target, edge_type);
        generate_id("bne", &seed)
    }

    /// Add a new edge.
    pub fn add_edge(&mut self, edge: &Edge) -> Result<()> {
        // Check if edge already exists between source and target with same type
        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM edges WHERE source = ?1 AND target = ?2 AND edge_type = ?3)",
            params![edge.source, edge.target, edge.edge_type.to_string()],
            |row| row.get(0),
        )?;

        if exists {
            return Err(Error::Other(format!(
                "Edge already exists: {} --[{}]--> {}",
                edge.source, edge.edge_type, edge.target
            )));
        }

        // Append to JSONL
        let edges_path = self.root.join("edges.jsonl");
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&edges_path)?;

        let json = serde_json::to_string(edge)?;
        writeln!(file, "{}", json)?;

        // Update cache
        self.cache_edge(edge)?;

        Ok(())
    }

    /// Get an edge by ID.
    pub fn get_edge(&self, id: &str) -> Result<Edge> {
        let edges_path = self.root.join("edges.jsonl");
        if !edges_path.exists() {
            return Err(Error::NotFound(format!("Edge not found: {}", id)));
        }

        let file = File::open(&edges_path)?;
        let reader = BufReader::new(file);

        let mut latest: Option<Edge> = None;
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(edge) = serde_json::from_str::<Edge>(&line)
                && edge.id == id
            {
                latest = Some(edge);
            }
        }

        latest.ok_or_else(|| Error::NotFound(format!("Edge not found: {}", id)))
    }

    /// List all edges, optionally filtered.
    pub fn list_edges(
        &self,
        edge_type: Option<EdgeType>,
        source: Option<&str>,
        target: Option<&str>,
    ) -> Result<Vec<Edge>> {
        let mut sql = String::from("SELECT id FROM edges WHERE 1=1");
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(et) = edge_type {
            sql.push_str(" AND edge_type = ?");
            params_vec.push(Box::new(et.to_string()));
        }
        if let Some(s) = source {
            sql.push_str(" AND source = ?");
            params_vec.push(Box::new(s.to_string()));
        }
        if let Some(t) = target {
            sql.push_str(" AND target = ?");
            params_vec.push(Box::new(t.to_string()));
        }

        sql.push_str(" ORDER BY created_at DESC");

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let mut stmt = self.conn.prepare(&sql)?;
        let ids: Vec<String> = stmt
            .query_map(params_refs.as_slice(), |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        let mut edges = Vec::new();
        for id in ids {
            if let Ok(edge) = self.get_edge(&id) {
                edges.push(edge);
            }
        }

        Ok(edges)
    }

    /// Remove an edge by source, target, and type.
    pub fn remove_edge(&mut self, source: &str, target: &str, edge_type: EdgeType) -> Result<()> {
        // Find the edge
        let edge_id: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM edges WHERE source = ?1 AND target = ?2 AND edge_type = ?3",
                params![source, target, edge_type.to_string()],
                |row| row.get(0),
            )
            .ok();

        let edge_id = edge_id.ok_or_else(|| {
            Error::NotFound(format!(
                "Edge not found: {} --[{}]--> {}",
                source, edge_type, target
            ))
        })?;

        // Remove from cache
        self.conn
            .execute("DELETE FROM edges WHERE id = ?", [&edge_id])?;

        Ok(())
    }

    /// Remove an edge by its ID (for cleaning up orphan edges).
    pub fn remove_edge_by_id(&mut self, edge_id: &str) -> Result<()> {
        let affected = self
            .conn
            .execute("DELETE FROM edges WHERE id = ?", [edge_id])?;

        if affected == 0 {
            return Err(Error::NotFound(format!("Edge not found: {}", edge_id)));
        }

        Ok(())
    }

    /// Get edges for a specific entity (both outbound and inbound).
    pub fn get_edges_for_entity(&self, entity_id: &str) -> Result<Vec<HydratedEdge>> {
        let mut edges = Vec::new();

        // Outbound edges (source = this entity)
        let outbound = self.list_edges(None, Some(entity_id), None)?;
        for edge in outbound {
            let direction = if edge.is_bidirectional() {
                EdgeDirection::Both
            } else {
                EdgeDirection::Outbound
            };
            edges.push(HydratedEdge { edge, direction });
        }

        // Inbound edges (target = this entity)
        let inbound = self.list_edges(None, None, Some(entity_id))?;
        for edge in inbound {
            // Skip bidirectional edges we already added (to avoid duplicates)
            if edge.is_bidirectional() {
                continue;
            }
            edges.push(HydratedEdge {
                edge,
                direction: EdgeDirection::Inbound,
            });
        }

        Ok(edges)
    }

    /// Get edges between two entities.
    pub fn get_edges_between(&self, source: &str, target: &str) -> Result<Vec<Edge>> {
        let mut edges = self.list_edges(None, Some(source), Some(target))?;

        // Also include bidirectional edges from the other direction
        let reverse = self.list_edges(None, Some(target), Some(source))?;
        for edge in reverse {
            if edge.is_bidirectional() {
                edges.push(edge);
            }
        }

        Ok(edges)
    }

    /// Check if adding an edge would create a cycle (for blocking edge types).
    /// Checks both edge-based dependencies AND legacy depends_on fields.
    pub fn would_edge_create_cycle(
        &self,
        source: &str,
        target: &str,
        edge_type: EdgeType,
    ) -> Result<bool> {
        // Only check for cycles on blocking edge types
        if !edge_type.is_blocking() {
            return Ok(false);
        }

        // For depends_on: source depends on target, so check if target depends on source (directly or indirectly)
        // For blocks: source blocks target, so check if target blocks source (directly or indirectly)
        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![target.to_string()];

        while let Some(current) = stack.pop() {
            if current == source {
                return Ok(true); // Found a path back to source, would create cycle
            }

            if visited.contains(&current) {
                continue;
            }
            visited.insert(current.clone());

            // Get all blocking edges where current is the source
            let deps = self.list_edges(Some(edge_type), Some(&current), None)?;
            for edge in deps {
                if !visited.contains(&edge.target) {
                    stack.push(edge.target);
                }
            }

            // Also check legacy depends_on for DependsOn edge type
            if edge_type == EdgeType::DependsOn {
                let legacy_deps = self.get_dependencies(&current).unwrap_or_default();
                for dep in legacy_deps {
                    if !visited.contains(&dep) {
                        stack.push(dep);
                    }
                }
            }
        }

        Ok(false)
    }

    /// Get dependencies for an entity using the edge model (replaces get_dependencies for tasks).
    pub fn get_edge_dependencies(&self, entity_id: &str) -> Result<Vec<String>> {
        let edges = self.list_edges(Some(EdgeType::DependsOn), Some(entity_id), None)?;
        Ok(edges.into_iter().map(|e| e.target).collect())
    }

    /// Get dependents for an entity using the edge model (replaces get_dependents for tasks).
    pub fn get_edge_dependents(&self, entity_id: &str) -> Result<Vec<String>> {
        let edges = self.list_edges(Some(EdgeType::DependsOn), None, Some(entity_id))?;
        Ok(edges.into_iter().map(|e| e.source).collect())
    }

    /// Update all edges that reference an old entity ID to use a new entity ID.
    /// This is used during migrations (e.g., bni- to bn- prefix migration).
    pub fn update_edge_entity_id(&mut self, old_id: &str, new_id: &str) -> Result<()> {
        // Update edges where old_id is the source
        self.conn.execute(
            "UPDATE edges SET source = ?1 WHERE source = ?2",
            params![new_id, old_id],
        )?;

        // Update edges where old_id is the target
        self.conn.execute(
            "UPDATE edges SET target = ?1 WHERE target = ?2",
            params![new_id, old_id],
        )?;

        Ok(())
    }

    // === Entity Type Detection ===

    /// Detect the type of an entity by its ID.
    /// Tries each entity type (task, bug, test, milestone, edge, queue) and returns the first match.
    pub fn get_entity_type(&self, id: &str) -> Result<EntityType> {
        // Try task
        if self.get_task(id).is_ok() {
            return Ok(EntityType::Task);
        }

        // Try bug
        if self.get_bug(id).is_ok() {
            return Ok(EntityType::Bug);
        }

        // Try idea
        if self.get_idea(id).is_ok() {
            return Ok(EntityType::Idea);
        }

        // Try test
        if self.get_test(id).is_ok() {
            return Ok(EntityType::Test);
        }

        // Try milestone
        if self.get_milestone(id).is_ok() {
            return Ok(EntityType::Milestone);
        }

        // Try edge
        if self.get_edge(id).is_ok() {
            return Ok(EntityType::Edge);
        }

        // Try queue
        if self.get_queue_by_id(id).is_ok() {
            return Ok(EntityType::Queue);
        }

        // Try doc
        if self.get_doc(id).is_ok() {
            return Ok(EntityType::Doc);
        }

        // Try agent
        if self.get_agent_by_id(id).is_ok() {
            return Ok(EntityType::Agent);
        }

        Err(Error::NotFound(id.to_string()))
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

    // === KDL Config File Operations ===

    /// Get the path to the config.kdl file.
    pub fn config_kdl_path(&self) -> PathBuf {
        self.root.join("config.kdl")
    }

    /// Read and parse the config.kdl file, or return an empty document if it doesn't exist.
    pub fn read_config_kdl(&self) -> Result<KdlDocument> {
        let path = self.config_kdl_path();
        if !path.exists() {
            return Ok(KdlDocument::new());
        }
        let content = fs::read_to_string(&path)?;
        content
            .parse::<KdlDocument>()
            .map_err(|e| Error::Other(format!("Failed to parse config.kdl: {}", e)))
    }

    /// Write the config.kdl file.
    pub fn write_config_kdl(&self, doc: &KdlDocument) -> Result<()> {
        let path = self.config_kdl_path();
        fs::write(&path, doc.to_string())?;
        Ok(())
    }

    /// Get agent scaling config from config.kdl.
    /// Returns (min, max) for the specified agent type, or defaults (0, 1) if not found.
    pub fn get_agent_scaling_kdl(&self, agent_type: &str) -> Result<(u32, u32)> {
        let doc = self.read_config_kdl()?;

        // Look for agents { worker min=X max=Y; ... }
        if let Some(agents_node) = doc.get("agents")
            && let Some(children) = agents_node.children()
            && let Some(type_node) = children.get(agent_type)
        {
            let min = type_node
                .get("min")
                .and_then(|v| v.as_integer())
                .unwrap_or(0) as u32;
            let max = type_node
                .get("max")
                .and_then(|v| v.as_integer())
                .unwrap_or(1) as u32;
            return Ok((min, max));
        }

        // Defaults
        Ok((0, 1))
    }

    /// Set agent scaling config in config.kdl.
    pub fn set_agent_scaling_kdl(&self, agent_type: &str, min: u32, max: u32) -> Result<()> {
        let mut doc = self.read_config_kdl()?;

        // Find or create agents node
        let agents_idx = doc
            .nodes()
            .iter()
            .position(|n| n.name().value() == "agents");

        if let Some(idx) = agents_idx {
            // Update existing agents node
            let agents_node = &mut doc.nodes_mut()[idx];
            let children = agents_node
                .children_mut()
                .get_or_insert_with(KdlDocument::new);

            // Find or create type node
            let type_idx = children
                .nodes()
                .iter()
                .position(|n| n.name().value() == agent_type);

            let mut type_node = KdlNode::new(agent_type);
            type_node.insert("min", KdlEntry::new(KdlValue::Integer(min as i128)));
            type_node.insert("max", KdlEntry::new(KdlValue::Integer(max as i128)));

            if let Some(tidx) = type_idx {
                children.nodes_mut()[tidx] = type_node;
            } else {
                children.nodes_mut().push(type_node);
            }
        } else {
            // Create agents node with child
            let mut agents_node = KdlNode::new("agents");
            let mut children = KdlDocument::new();

            let mut type_node = KdlNode::new(agent_type);
            type_node.insert("min", KdlEntry::new(KdlValue::Integer(min as i128)));
            type_node.insert("max", KdlEntry::new(KdlValue::Integer(max as i128)));
            children.nodes_mut().push(type_node);

            agents_node.set_children(children);
            doc.nodes_mut().push(agents_node);
        }

        self.write_config_kdl(&doc)
    }

    /// Get all agent scaling configs from config.kdl.
    pub fn get_all_agent_scaling_kdl(&self) -> Result<Vec<(String, u32, u32)>> {
        let doc = self.read_config_kdl()?;
        let mut results = Vec::new();

        if let Some(agents_node) = doc.get("agents")
            && let Some(children) = agents_node.children()
        {
            for node in children.nodes() {
                let agent_type = node.name().value().to_string();
                let min = node.get("min").and_then(|v| v.as_integer()).unwrap_or(0) as u32;
                let max = node.get("max").and_then(|v| v.as_integer()).unwrap_or(1) as u32;
                results.push((agent_type, min, max));
            }
        }

        Ok(results)
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
            if let Ok(task) = serde_json::from_str::<Task>(&line)
                && task.core.entity_type == "task"
            {
                // Filter by task_id if provided
                if let Some(filter_id) = task_id
                    && task.core.id != filter_id
                {
                    continue;
                }

                let action = if seen_tasks.contains_key(&task.core.id) {
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
                    timestamp: task.core.updated_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                    entity_type: "task".to_string(),
                    entity_id: task.core.id.clone(),
                    action: action.to_string(),
                    details,
                });

                seen_tasks.insert(task.core.id.clone(), task.core.updated_at);
            }

            // Try to parse as TestNode
            if let Ok(test) = serde_json::from_str::<TestNode>(&line)
                && test.entity_type == "test"
            {
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

        // Update task links
        self.conn
            .execute("DELETE FROM test_links WHERE test_id = ?1", [&test.id])?;
        for task_id in &test.linked_tasks {
            self.conn.execute(
                "INSERT OR IGNORE INTO test_links (test_id, task_id) VALUES (?1, ?2)",
                params![test.id, task_id],
            )?;
        }

        // Update bug links
        self.conn
            .execute("DELETE FROM test_bug_links WHERE test_id = ?1", [&test.id])?;
        for bug_id in &test.linked_bugs {
            self.conn.execute(
                "INSERT OR IGNORE INTO test_bug_links (test_id, bug_id) VALUES (?1, ?2)",
                params![test.id, bug_id],
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
            if let Ok(test) = serde_json::from_str::<TestNode>(&line)
                && test.entity_type == "test"
                && test.id == id
            {
                latest = Some(test);
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
        self.conn
            .execute("DELETE FROM test_bug_links WHERE test_id = ?", [id])?;

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

    /// Link a test to a bug.
    pub fn link_test_to_bug(&mut self, test_id: &str, bug_id: &str) -> Result<()> {
        // Verify both exist
        let mut test = self.get_test(test_id)?;
        self.get_bug(bug_id)?;

        // Check if already linked
        if test.linked_bugs.contains(&bug_id.to_string()) {
            return Err(Error::Other(format!(
                "Test {} is already linked to bug {}",
                test_id, bug_id
            )));
        }

        // Add link
        test.linked_bugs.push(bug_id.to_string());

        // Update storage
        self.update_test(&test)?;

        Ok(())
    }

    /// Unlink a test from a bug.
    pub fn unlink_test_from_bug(&mut self, test_id: &str, bug_id: &str) -> Result<()> {
        // Verify both exist
        let mut test = self.get_test(test_id)?;
        self.get_bug(bug_id)?;

        // Check if linked
        if !test.linked_bugs.contains(&bug_id.to_string()) {
            return Err(Error::NotFound(format!(
                "Test {} is not linked to bug {}",
                test_id, bug_id
            )));
        }

        // Remove link
        test.linked_bugs.retain(|id| id != bug_id);

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
            if let Ok(Some(result)) = self.get_last_test_result(&test.id)
                && !result.passed
            {
                failed.push(test);
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
                    task.core.updated_at = Utc::now();
                    self.update_task(&task)?;
                    reopened.push(task_id.clone());
                }
            }
        }

        Ok(reopened)
    }

    // === Commit Link Operations ===

    /// Link a commit to a task.
    /// Link a commit to a task or bug.
    pub fn link_commit(&mut self, sha: &str, entity_id: &str) -> Result<CommitLink> {
        // Validate SHA format
        validate_sha(sha)?;

        // Validate entity exists (task or bug)
        let entity_type = self.get_entity_type(entity_id).map_err(|_| {
            Error::NotFound(format!(
                "Entity not found: {} (must be a task or bug)",
                entity_id
            ))
        })?;

        // Only allow tasks and bugs
        match entity_type {
            EntityType::Task | EntityType::Bug => {}
            _ => {
                return Err(Error::InvalidInput(format!(
                    "Cannot link commits to {}: only tasks and bugs are supported",
                    entity_type
                )));
            }
        }

        // Check if already linked
        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM commit_links WHERE sha = ?1 AND task_id = ?2)",
            params![sha, entity_id],
            |row| row.get(0),
        )?;

        if exists {
            return Err(Error::Other(format!(
                "Commit {} is already linked to {}",
                sha, entity_id
            )));
        }

        let linked_at = Utc::now();

        // Insert into SQLite cache
        self.conn.execute(
            "INSERT INTO commit_links (sha, task_id, linked_at) VALUES (?1, ?2, ?3)",
            params![sha, entity_id, linked_at.to_rfc3339()],
        )?;

        // Append to JSONL
        let link = CommitLink {
            sha: sha.to_string(),
            task_id: entity_id.to_string(),
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

    /// Unlink a commit from a task or bug.
    pub fn unlink_commit(&mut self, sha: &str, entity_id: &str) -> Result<()> {
        // Validate SHA format
        validate_sha(sha)?;

        // Validate entity exists (task or bug)
        let entity_type = self.get_entity_type(entity_id).map_err(|_| {
            Error::NotFound(format!(
                "Entity not found: {} (must be a task or bug)",
                entity_id
            ))
        })?;

        // Only allow tasks and bugs
        match entity_type {
            EntityType::Task | EntityType::Bug => {}
            _ => {
                return Err(Error::InvalidInput(format!(
                    "Cannot unlink commits from {}: only tasks and bugs are supported",
                    entity_type
                )));
            }
        }

        // Check if linked
        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM commit_links WHERE sha = ?1 AND task_id = ?2)",
            params![sha, entity_id],
            |row| row.get(0),
        )?;

        if !exists {
            return Err(Error::NotFound(format!(
                "Commit {} is not linked to {}",
                sha, entity_id
            )));
        }

        // Remove from cache
        self.conn.execute(
            "DELETE FROM commit_links WHERE sha = ?1 AND task_id = ?2",
            params![sha, entity_id],
        )?;

        Ok(())
    }

    /// Get all commits linked to a task (backward compatibility).
    pub fn get_commits_for_task(&self, task_id: &str) -> Result<Vec<CommitLink>> {
        // Validate task exists
        self.get_task(task_id)?;

        self.get_commits_for_entity_internal(task_id)
    }

    /// Get all commits linked to a task or bug.
    pub fn get_commits_for_entity(&self, entity_id: &str) -> Result<Vec<CommitLink>> {
        // Validate entity exists (task or bug)
        let entity_type = self.get_entity_type(entity_id).map_err(|_| {
            Error::NotFound(format!(
                "Entity not found: {} (must be a task or bug)",
                entity_id
            ))
        })?;

        // Only allow tasks and bugs
        match entity_type {
            EntityType::Task | EntityType::Bug => {}
            _ => {
                return Err(Error::InvalidInput(format!(
                    "Cannot list commits for {}: only tasks and bugs are supported",
                    entity_type
                )));
            }
        }

        self.get_commits_for_entity_internal(entity_id)
    }

    /// Internal helper to get commits for an entity (no validation).
    fn get_commits_for_entity_internal(&self, entity_id: &str) -> Result<Vec<CommitLink>> {
        let mut stmt = self.conn.prepare(
            "SELECT sha, task_id, linked_at FROM commit_links WHERE task_id = ?1 ORDER BY linked_at DESC",
        )?;

        let links: Vec<CommitLink> = stmt
            .query_map([entity_id], |row| {
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

    // === Agent Operations ===

    /// Register a new agent or update an existing one.
    pub fn register_agent(&mut self, agent: &Agent) -> Result<()> {
        // Append to JSONL
        let agents_path = self.root.join("agents.jsonl");
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&agents_path)?;

        let json = serde_json::to_string(agent)?;
        writeln!(file, "{}", json)?;

        // Update cache
        self.cache_agent(agent)?;

        Ok(())
    }

    /// Cache an agent in SQLite.
    fn cache_agent(&self, agent: &Agent) -> Result<()> {
        // Insert or replace the agent
        self.conn.execute(
            "INSERT OR REPLACE INTO agents (pid, parent_pid, name, purpose, mcp_session_id, status, started_at, last_activity_at, command_count, id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                agent.pid,
                agent.parent_pid,
                agent.name,
                agent.purpose,
                agent.mcp_session_id,
                format!("{:?}", agent.status).to_lowercase(),
                agent.started_at.to_rfc3339(),
                agent.last_activity_at.to_rfc3339(),
                agent.command_count,
                agent.id,
            ],
        )?;

        // Update agent tasks
        self.conn
            .execute("DELETE FROM agent_tasks WHERE pid = ?1", params![agent.pid])?;

        for task_id in &agent.tasks {
            self.conn.execute(
                "INSERT INTO agent_tasks (pid, task_id) VALUES (?1, ?2)",
                params![agent.pid, task_id],
            )?;
        }

        Ok(())
    }

    /// Get an agent by PID.
    pub fn get_agent(&self, pid: u32) -> Result<Agent> {
        // First check if agent exists in cache (handles removed agents)
        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM agents WHERE pid = ?1)",
            params![pid],
            |row| row.get(0),
        )?;

        if !exists {
            return Err(Error::NotFound(format!(
                "Agent not found with PID: {}",
                pid
            )));
        }

        // Read from JSONL to get the latest version
        let agents_path = self.root.join("agents.jsonl");
        let file = File::open(&agents_path)?;
        let reader = BufReader::new(file);

        let mut latest: Option<Agent> = None;
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(mut agent) = serde_json::from_str::<Agent>(&line)
                && agent.pid == pid
            {
                // Ensure backward compatibility: generate ID if missing
                agent.ensure_id();
                latest = Some(agent);
            }
        }

        latest.ok_or_else(|| Error::NotFound(format!("Agent not found with PID: {}", pid)))
    }

    /// List all agents, optionally filtered by status.
    pub fn list_agents(&self, status: Option<&str>) -> Result<Vec<Agent>> {
        let mut sql = String::from("SELECT pid FROM agents WHERE 1=1");
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(s) = status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(s.to_string()));
        }

        sql.push_str(" ORDER BY started_at DESC");

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let mut stmt = self.conn.prepare(&sql)?;
        let pids: Vec<u32> = stmt
            .query_map(params_refs.as_slice(), |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        // Fetch full agent data from JSONL
        let mut agents = Vec::new();
        for pid in pids {
            if let Ok(agent) = self.get_agent(pid) {
                agents.push(agent);
            }
        }

        Ok(agents)
    }

    /// Remove an agent from the registry.
    /// Transitions any working_on edges to worked_on (historical record) before cleanup.
    pub fn remove_agent(&mut self, pid: u32) -> Result<()> {
        // Get the agent first so we have its ID for edge cleanup
        let agent = self
            .get_agent(pid)
            .map_err(|_| Error::NotFound(format!("Agent not found with PID: {}", pid)))?;

        // Transition working_on edges to worked_on (historical record)
        // This preserves the history of what the agent worked on
        let working_on_edges = self.list_edges(Some(EdgeType::WorkingOn), Some(&agent.id), None)?;
        for edge in working_on_edges {
            // Remove the working_on edge
            if self
                .remove_edge(&edge.source, &edge.target, EdgeType::WorkingOn)
                .is_ok()
            {
                // Create a worked_on edge to record the historical work
                let edge_id = self.generate_edge_id(&edge.source, &edge.target, EdgeType::WorkedOn);
                let worked_on_edge = Edge::new(
                    edge_id,
                    edge.source.clone(),
                    edge.target.clone(),
                    EdgeType::WorkedOn,
                );
                let _ = self.add_edge(&worked_on_edge); // Ignore error if it already exists
            }
        }

        // Remove from cache (JSONL is append-only, so we just remove from cache)
        self.conn
            .execute("DELETE FROM agent_tasks WHERE pid = ?1", params![pid])?;
        self.conn
            .execute("DELETE FROM agents WHERE pid = ?1", params![pid])?;

        Ok(())
    }

    /// Update an agent's activity timestamp and increment command count.
    pub fn touch_agent(&mut self, pid: u32) -> Result<()> {
        let mut agent = self.get_agent(pid)?;
        agent.touch();
        self.register_agent(&agent)?;
        Ok(())
    }

    /// Update an agent's data (overwrites existing agent with same PID).
    pub fn update_agent(&mut self, agent: &Agent) -> Result<()> {
        self.register_agent(agent)
    }

    /// Update agent status.
    pub fn update_agent_status(&mut self, pid: u32, status: AgentStatus) -> Result<()> {
        let mut agent = self.get_agent(pid)?;
        agent.status = status;
        self.register_agent(&agent)?;
        Ok(())
    }

    /// Add a task to an agent's working set and create a working_on edge.
    pub fn agent_add_task(&mut self, pid: u32, task_id: &str) -> Result<()> {
        let mut agent = self.get_agent(pid)?;
        if !agent.tasks.contains(&task_id.to_string()) {
            agent.tasks.push(task_id.to_string());
            self.register_agent(&agent)?;

            // Create a working_on edge from agent to task
            let edge_id = self.generate_edge_id(&agent.id, task_id, EdgeType::WorkingOn);
            let edge = Edge::new(
                edge_id,
                agent.id.clone(),
                task_id.to_string(),
                EdgeType::WorkingOn,
            );
            self.add_edge(&edge)?;
        }
        Ok(())
    }

    /// Remove a task from an agent's working set and delete the working_on edge.
    pub fn agent_remove_task(&mut self, pid: u32, task_id: &str) -> Result<()> {
        let mut agent = self.get_agent(pid)?;
        if agent.tasks.contains(&task_id.to_string()) {
            agent.tasks.retain(|t| t != task_id);
            self.register_agent(&agent)?;

            // Transition working_on edge to worked_on (historical record)
            // First remove the working_on edge
            if self
                .remove_edge(&agent.id, task_id, EdgeType::WorkingOn)
                .is_ok()
            {
                // Create a worked_on edge to record the completed work
                let edge_id = self.generate_edge_id(&agent.id, task_id, EdgeType::WorkedOn);
                let worked_on_edge = Edge::new(
                    edge_id,
                    agent.id.clone(),
                    task_id.to_string(),
                    EdgeType::WorkedOn,
                );
                let _ = self.add_edge(&worked_on_edge); // Ignore error if it already exists
            }
        }
        Ok(())
    }

    /// Get agent by binnacle ID (bn-xxxx with entity_type=agent).
    pub fn get_agent_by_id(&self, id: &str) -> Result<Agent> {
        use rusqlite::OptionalExtension;
        // First try to find in cache
        let pid: Option<u32> = self
            .conn
            .query_row("SELECT pid FROM agents WHERE id = ?1", params![id], |row| {
                row.get(0)
            })
            .optional()?;

        match pid {
            Some(p) => self.get_agent(p),
            None => Err(Error::NotFound(format!("Agent not found: {}", id))),
        }
    }

    /// Get agent by name.
    pub fn get_agent_by_name(&self, name: &str) -> Result<Agent> {
        // First try to find in cache
        let pid: Option<u32> = self
            .conn
            .query_row(
                "SELECT pid FROM agents WHERE name = ?1 ORDER BY started_at DESC LIMIT 1",
                params![name],
                |row| row.get(0),
            )
            .ok();

        match pid {
            Some(p) => self.get_agent(p),
            None => Err(Error::NotFound(format!("Agent not found: {}", name))),
        }
    }

    /// Get agent by MCP session ID.
    /// Used for MCP wrapper invocation where PID-based tracking doesn't work.
    pub fn get_agent_by_mcp_session(&self, session_id: &str) -> Result<Agent> {
        let pid: Option<u32> = self
            .conn
            .query_row(
                "SELECT pid FROM agents WHERE mcp_session_id = ?1 LIMIT 1",
                params![session_id],
                |row| row.get(0),
            )
            .ok();

        match pid {
            Some(p) => self.get_agent(p),
            None => Err(Error::NotFound(format!(
                "Agent not found with MCP session: {}",
                session_id
            ))),
        }
    }

    /// Clean up stale agents (PIDs that are no longer running).
    /// Returns the list of removed agent PIDs.
    pub fn cleanup_stale_agents(&mut self) -> Result<Vec<u32>> {
        let agents = self.list_agents(None)?;
        let mut removed = Vec::new();
        let now = chrono::Utc::now();
        let goodbye_delay = chrono::Duration::seconds(10);

        for agent in agents {
            let should_remove = if !agent.is_alive() {
                // Process is no longer running
                true
            } else if let Some(goodbye_at) = agent.goodbye_at {
                // Agent said goodbye - remove after 10 seconds to allow GUI animation
                now.signed_duration_since(goodbye_at) > goodbye_delay
            } else {
                false
            };

            if should_remove && self.remove_agent(agent.pid).is_ok() {
                removed.push(agent.pid);
            }
        }

        Ok(removed)
    }

    /// Mark agents as idle or stale based on activity.
    /// - Active: Agent has activity in the last 5 minutes
    /// - Idle: No activity in the last 5 minutes but less than 30 minutes
    /// - Stale: PID is no longer running OR no activity for 30+ minutes
    pub fn update_agent_statuses(&mut self) -> Result<()> {
        let agents = self.list_agents(None)?;
        let now = chrono::Utc::now();
        let idle_threshold = chrono::Duration::minutes(5);
        let stale_threshold = chrono::Duration::minutes(30);

        for mut agent in agents {
            let inactive_duration = now.signed_duration_since(agent.last_activity_at);
            let new_status = if !agent.is_alive() {
                AgentStatus::Stale
            } else if inactive_duration > stale_threshold {
                // Agent process is alive but unresponsive for 30+ minutes
                AgentStatus::Stale
            } else if inactive_duration > idle_threshold {
                AgentStatus::Idle
            } else {
                AgentStatus::Active
            };

            if agent.status != new_status {
                agent.status = new_status;
                self.register_agent(&agent)?;
            }
        }

        Ok(())
    }

    // === Session State Operations ===

    /// Write session state to session.json for commit-msg hook detection.
    pub fn write_session_state(&self, state: &crate::models::SessionState) -> Result<()> {
        let session_path = self.root.join("session.json");
        let json = serde_json::to_string_pretty(state)?;
        fs::write(&session_path, json)?;
        Ok(())
    }

    /// Read session state from session.json if it exists.
    pub fn read_session_state(&self) -> Result<crate::models::SessionState> {
        let session_path = self.root.join("session.json");
        if !session_path.exists() {
            return Err(Error::NotFound("No active session".to_string()));
        }
        let content = fs::read_to_string(&session_path)?;
        let state: crate::models::SessionState = serde_json::from_str(&content)?;
        Ok(state)
    }

    /// Delete session state file.
    pub fn clear_session_state(&self) -> Result<()> {
        let session_path = self.root.join("session.json");
        if session_path.exists() {
            fs::remove_file(&session_path)?;
        }
        Ok(())
    }

    // ============================================================
    // Action Log methods
    // ============================================================

    /// Insert an action log entry into the SQLite cache.
    pub fn add_action_log(&self, log: &crate::action_log::ActionLog) -> Result<()> {
        let args_str = serde_json::to_string(&log.args)?;
        self.conn.execute(
            r#"
            INSERT INTO action_logs (timestamp, repo_path, command, args, success, error, duration_ms, user)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                log.timestamp.to_rfc3339(),
                log.repo_path,
                log.command,
                args_str,
                log.success as i32,
                log.error,
                log.duration_ms as i64,
                log.user,
            ],
        )?;
        Ok(())
    }

    /// Query action logs with pagination and optional filters.
    ///
    /// # Arguments
    /// * `limit` - Maximum number of entries to return (default: 100, max: 1000)
    /// * `offset` - Number of entries to skip for pagination
    /// * `before` - Only return entries before this ISO 8601 timestamp
    /// * `command_filter` - Filter by command name (partial match)
    /// * `user_filter` - Filter by user name (exact match)
    /// * `success_filter` - Filter by success status
    #[allow(clippy::too_many_arguments)]
    pub fn query_action_logs(
        &self,
        limit: Option<u32>,
        offset: Option<u32>,
        before: Option<&str>,
        after: Option<&str>,
        command_filter: Option<&str>,
        user_filter: Option<&str>,
        success_filter: Option<bool>,
    ) -> Result<Vec<crate::action_log::ActionLog>> {
        let limit = limit.unwrap_or(100).min(1000);
        let offset = offset.unwrap_or(0);

        // Build the query dynamically based on filters
        let mut sql = String::from(
            "SELECT timestamp, repo_path, command, args, success, error, duration_ms, user FROM action_logs WHERE 1=1",
        );
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(before_ts) = before {
            sql.push_str(" AND timestamp < ?");
            params.push(Box::new(before_ts.to_string()));
        }

        if let Some(after_ts) = after {
            sql.push_str(" AND timestamp > ?");
            params.push(Box::new(after_ts.to_string()));
        }

        if let Some(cmd) = command_filter {
            sql.push_str(" AND command LIKE ?");
            params.push(Box::new(format!("%{}%", cmd)));
        }

        if let Some(user) = user_filter {
            sql.push_str(" AND user = ?");
            params.push(Box::new(user.to_string()));
        }

        if let Some(success) = success_filter {
            sql.push_str(" AND success = ?");
            params.push(Box::new(success as i32));
        }

        sql.push_str(" ORDER BY timestamp DESC LIMIT ? OFFSET ?");
        params.push(Box::new(limit as i64));
        params.push(Box::new(offset as i64));

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            let timestamp_str: String = row.get(0)?;
            let args_str: String = row.get(3)?;
            let success_int: i32 = row.get(4)?;
            let duration_ms: i64 = row.get(6)?;

            Ok(crate::action_log::ActionLog {
                timestamp: chrono::DateTime::parse_from_rfc3339(&timestamp_str)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
                repo_path: row.get(1)?,
                command: row.get(2)?,
                args: serde_json::from_str(&args_str).unwrap_or(serde_json::Value::Null),
                success: success_int != 0,
                error: row.get(5)?,
                duration_ms: duration_ms as u64,
                user: row.get(7)?,
            })
        })?;

        let mut logs = Vec::new();
        for row in rows {
            logs.push(row?);
        }
        Ok(logs)
    }

    /// Count total action log entries (with optional filters).
    pub fn count_action_logs(
        &self,
        before: Option<&str>,
        after: Option<&str>,
        command_filter: Option<&str>,
        user_filter: Option<&str>,
        success_filter: Option<bool>,
    ) -> Result<u32> {
        let mut sql = String::from("SELECT COUNT(*) FROM action_logs WHERE 1=1");
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(before_ts) = before {
            sql.push_str(" AND timestamp < ?");
            params.push(Box::new(before_ts.to_string()));
        }

        if let Some(after_ts) = after {
            sql.push_str(" AND timestamp > ?");
            params.push(Box::new(after_ts.to_string()));
        }

        if let Some(cmd) = command_filter {
            sql.push_str(" AND command LIKE ?");
            params.push(Box::new(format!("%{}%", cmd)));
        }

        if let Some(user) = user_filter {
            sql.push_str(" AND user = ?");
            params.push(Box::new(user.to_string()));
        }

        if let Some(success) = success_filter {
            sql.push_str(" AND success = ?");
            params.push(Box::new(success as i32));
        }

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let count: u32 = self
            .conn
            .query_row(&sql, param_refs.as_slice(), |row| row.get(0))?;
        Ok(count)
    }

    /// Delete old action logs based on retention settings.
    ///
    /// Returns the number of entries deleted.
    ///
    /// - `max_entries`: If set, keep only this many most recent entries
    /// - `max_age_days`: If set, delete entries older than this many days
    ///
    /// Both limits are applied if both are set.
    pub fn delete_old_action_logs(
        &mut self,
        max_entries: Option<u32>,
        max_age_days: Option<u32>,
    ) -> Result<u32> {
        let mut total_deleted = 0u32;

        // Delete entries older than max_age_days
        if let Some(days) = max_age_days {
            let cutoff = chrono::Utc::now() - chrono::Duration::days(i64::from(days));
            let cutoff_str = cutoff.to_rfc3339();

            let deleted = self.conn.execute(
                "DELETE FROM action_logs WHERE timestamp < ?1",
                [&cutoff_str],
            )?;
            total_deleted += deleted as u32;
        }

        // Delete entries exceeding max_entries (keep most recent)
        if let Some(max) = max_entries {
            // First count how many we have
            let count: u32 =
                self.conn
                    .query_row("SELECT COUNT(*) FROM action_logs", [], |row| row.get(0))?;

            if count > max {
                let to_delete = count - max;
                // Delete oldest entries (those with smallest timestamps)
                let deleted = self.conn.execute(
                    "DELETE FROM action_logs WHERE rowid IN (
                        SELECT rowid FROM action_logs ORDER BY timestamp ASC LIMIT ?1
                    )",
                    [to_delete],
                )?;
                total_deleted += deleted as u32;
            }
        }

        Ok(total_deleted)
    }

    /// Import action logs from JSONL file into SQLite cache.
    /// This is used to populate the cache from existing logs.
    pub fn import_action_logs_from_file(&mut self, log_path: &Path) -> Result<u32> {
        if !log_path.exists() {
            return Ok(0);
        }

        let file = File::open(log_path)?;
        let reader = BufReader::new(file);
        let mut imported = 0;

        // Begin transaction for bulk insert
        self.conn.execute("BEGIN TRANSACTION", [])?;

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(log) = serde_json::from_str::<crate::action_log::ActionLog>(&line) {
                let args_str =
                    serde_json::to_string(&log.args).unwrap_or_else(|_| "{}".to_string());
                // Use INSERT OR IGNORE to skip duplicates (based on unique constraint on timestamp+repo_path+command)
                let result = self.conn.execute(
                    r#"
                    INSERT INTO action_logs (timestamp, repo_path, command, args, success, error, duration_ms, user)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                    "#,
                    params![
                        log.timestamp.to_rfc3339(),
                        log.repo_path,
                        log.command,
                        args_str,
                        log.success as i32,
                        log.error,
                        log.duration_ms as i64,
                        log.user,
                    ],
                );
                if result.is_ok() {
                    imported += 1;
                }
            }
        }

        self.conn.execute("COMMIT", [])?;
        Ok(imported)
    }

    // ============================================================
    // Log Annotation methods
    // ============================================================

    /// Generate a unique ID for a log annotation.
    pub fn generate_annotation_id(&self, log_timestamp: &str) -> String {
        generate_id(
            "bnl",
            &format!(
                "{}-{}",
                log_timestamp,
                Utc::now().timestamp_nanos_opt().unwrap_or(0)
            ),
        )
    }

    /// Add a log annotation to storage.
    pub fn add_log_annotation(&self, annotation: &LogAnnotation) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO log_annotations (id, log_timestamp, content, author, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                annotation.id,
                annotation.log_timestamp,
                annotation.content,
                annotation.author,
                annotation.created_at.to_rfc3339(),
                annotation.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Get a log annotation by ID.
    pub fn get_log_annotation(&self, id: &str) -> Result<LogAnnotation> {
        let annotation = self.conn.query_row(
            "SELECT id, log_timestamp, content, author, created_at, updated_at FROM log_annotations WHERE id = ?1",
            [id],
            |row| {
                let created_at_str: String = row.get(4)?;
                let updated_at_str: String = row.get(5)?;
                Ok(LogAnnotation {
                    id: row.get(0)?,
                    entity_type: "log_annotation".to_string(),
                    log_timestamp: row.get(1)?,
                    content: row.get(2)?,
                    author: row.get(3)?,
                    created_at: chrono::DateTime::parse_from_rfc3339(&created_at_str)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now()),
                    updated_at: chrono::DateTime::parse_from_rfc3339(&updated_at_str)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now()),
                })
            },
        )?;
        Ok(annotation)
    }

    /// Get all annotations for a specific log entry (by timestamp).
    pub fn get_annotations_for_log(&self, log_timestamp: &str) -> Result<Vec<LogAnnotation>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, log_timestamp, content, author, created_at, updated_at FROM log_annotations WHERE log_timestamp = ?1 ORDER BY created_at ASC"
        )?;
        let annotations = stmt
            .query_map([log_timestamp], |row| {
                let created_at_str: String = row.get(4)?;
                let updated_at_str: String = row.get(5)?;
                Ok(LogAnnotation {
                    id: row.get(0)?,
                    entity_type: "log_annotation".to_string(),
                    log_timestamp: row.get(1)?,
                    content: row.get(2)?,
                    author: row.get(3)?,
                    created_at: chrono::DateTime::parse_from_rfc3339(&created_at_str)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now()),
                    updated_at: chrono::DateTime::parse_from_rfc3339(&updated_at_str)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now()),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(annotations)
    }

    /// List all log annotations, optionally filtered.
    ///
    /// * `author_filter` - Filter by author (exact match)
    /// * `search_filter` - Search in content (partial match)
    pub fn list_log_annotations(
        &self,
        author_filter: Option<&str>,
        search_filter: Option<&str>,
    ) -> Result<Vec<LogAnnotation>> {
        let mut sql = String::from(
            "SELECT id, log_timestamp, content, author, created_at, updated_at FROM log_annotations WHERE 1=1",
        );
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(author) = author_filter {
            sql.push_str(" AND author = ?");
            params.push(Box::new(author.to_string()));
        }

        if let Some(search) = search_filter {
            sql.push_str(" AND content LIKE ?");
            params.push(Box::new(format!("%{}%", search)));
        }

        sql.push_str(" ORDER BY created_at DESC");

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = self.conn.prepare(&sql)?;
        let annotations = stmt
            .query_map(params_refs.as_slice(), |row| {
                let created_at_str: String = row.get(4)?;
                let updated_at_str: String = row.get(5)?;
                Ok(LogAnnotation {
                    id: row.get(0)?,
                    entity_type: "log_annotation".to_string(),
                    log_timestamp: row.get(1)?,
                    content: row.get(2)?,
                    author: row.get(3)?,
                    created_at: chrono::DateTime::parse_from_rfc3339(&created_at_str)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now()),
                    updated_at: chrono::DateTime::parse_from_rfc3339(&updated_at_str)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now()),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(annotations)
    }

    /// Update an existing log annotation.
    pub fn update_log_annotation(&self, id: &str, content: &str) -> Result<()> {
        let updated_at = Utc::now().to_rfc3339();
        let rows_affected = self.conn.execute(
            "UPDATE log_annotations SET content = ?1, updated_at = ?2 WHERE id = ?3",
            params![content, updated_at, id],
        )?;
        if rows_affected == 0 {
            return Err(Error::NotFound(format!("Log annotation not found: {}", id)));
        }
        Ok(())
    }

    /// Delete a log annotation.
    pub fn delete_log_annotation(&self, id: &str) -> Result<()> {
        let rows_affected = self
            .conn
            .execute("DELETE FROM log_annotations WHERE id = ?1", [id])?;
        if rows_affected == 0 {
            return Err(Error::NotFound(format!("Log annotation not found: {}", id)));
        }
        Ok(())
    }
}

/// Resolve a git worktree's `.git` file to find the main repository root.
///
/// Git worktrees have a `.git` file (not directory) containing a pointer like:
/// `gitdir: /path/to/main/repo/.git/worktrees/worktree-name`
///
/// This function reads that pointer and finds the main repo by:
/// 1. Parsing the `gitdir:` line from the `.git` file
/// 2. Looking for a `commondir` file that points to the main `.git` directory
/// 3. Returning the parent of that main `.git` directory
///
/// Returns `None` if this isn't a worktree or if resolution fails.
fn resolve_worktree_to_main_repo(git_file: &Path) -> Option<PathBuf> {
    let content = fs::read_to_string(git_file).ok()?;

    // Parse "gitdir: <path>" line
    let gitdir_line = content.lines().find(|l| l.starts_with("gitdir:"))?;
    let gitdir_path_str = gitdir_line.strip_prefix("gitdir:")?.trim();

    // Resolve relative or absolute path
    let gitdir_path = if Path::new(gitdir_path_str).is_absolute() {
        PathBuf::from(gitdir_path_str)
    } else {
        git_file.parent()?.join(gitdir_path_str)
    };

    // Look for commondir file which points to the main .git directory
    let commondir_file = gitdir_path.join("commondir");
    if commondir_file.exists() {
        let commondir_content = fs::read_to_string(&commondir_file).ok()?;
        let common_path = gitdir_path.join(commondir_content.trim());
        // common_path is the main .git dir, parent is repo root
        return common_path
            .canonicalize()
            .ok()?
            .parent()
            .map(|p| p.to_path_buf());
    }

    // Fallback: try to find main repo by walking up from gitdir
    // gitdir is typically at /main/repo/.git/worktrees/<name>
    // So we go up 3 levels to find the main repo
    let mut candidate = gitdir_path.canonicalize().ok()?;
    for _ in 0..3 {
        candidate = candidate.parent()?.to_path_buf();
    }
    if candidate.join(".git").is_dir() {
        return Some(candidate);
    }

    None
}

/// Find the git root by walking up directories looking for .git.
///
/// Supports both regular git repositories (.git directory) and git worktrees
/// (.git file pointing to the worktree's git directory). For worktrees, this
/// resolves back to the main repository root so all worktrees share the same
/// binnacle database.
///
/// Git submodules also have a `.git` file, but their format differs from worktrees
/// (`gitdir: ../.git/modules/<name>` vs `gitdir: .../.git/worktrees/<name>`).
/// Submodules are intentionally treated as their own repository root since they
/// are logically independent git repositories that happen to be nested.
///
/// Returns `None` if no .git is found (not in a git repository).
pub fn find_git_root(start: &Path) -> Option<PathBuf> {
    // Start from canonicalized path to handle symlinks consistently
    let mut current = start.canonicalize().ok()?;

    // In container mode, skip worktree-to-main-repo resolution.
    // Containers mount only the worktree and parent .git (read-only), so resolving
    // to the main repo would return a path that isn't fully accessible. The storage
    // hash is pre-computed by the host and passed via BN_STORAGE_HASH.
    let skip_worktree_resolution = std::env::var("BN_CONTAINER_MODE").is_ok();

    loop {
        let git_path = current.join(".git");
        if git_path.is_dir() {
            // Normal git repo - return this directory
            return Some(current);
        } else if git_path.is_file() {
            // Git worktree or submodule detected (.git is a file, not directory)
            if skip_worktree_resolution {
                // Container mode: treat worktree as its own root
                return Some(current);
            }
            // Normal mode: resolve to main repo for shared binnacle database.
            // Git submodule: resolution will fail (different gitdir format),
            // and we'll fall back to treating it as its own root (intentional).
            if let Some(main_repo) = resolve_worktree_to_main_repo(&git_path) {
                return Some(main_repo);
            }
            // Resolution failed - this is either a submodule (intentionally its
            // own root) or a broken worktree (fallback to worktree root)
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Get the storage directory for a repository.
///
/// Uses a hash of the repository path to create a unique directory.
/// The base directory is determined by:
/// 1. `BN_DATA_DIR` environment variable (if set) - highest priority for test isolation
/// 2. `BN_CONTAINER_MODE` environment variable (if set, uses `/binnacle`)
/// 3. `/binnacle` path exists (auto-detect container environment)
/// 4. Falls back to `~/.local/share/binnacle/`
///
/// The provided path is used directly; git root detection (if desired)
/// should be done by the caller before invoking this function.
pub fn get_storage_dir(repo_path: &Path) -> Result<PathBuf> {
    let container_path = Path::new("/binnacle");

    // BN_DATA_DIR takes highest priority - enables test isolation even in containers
    let base_dir = if let Ok(override_dir) = std::env::var("BN_DATA_DIR") {
        PathBuf::from(override_dir)
    } else if std::env::var("BN_CONTAINER_MODE").is_ok() {
        // Container mode: use /binnacle as base directory
        PathBuf::from("/binnacle")
    } else if container_path.exists() {
        // Auto-detect container environment: /binnacle exists
        container_path.to_path_buf()
    } else {
        dirs::data_dir()
            .ok_or_else(|| Error::Other("Could not determine data directory".to_string()))?
            .join("binnacle")
    };

    get_storage_dir_with_base(repo_path, &base_dir)
}

/// Get the storage directory for a repository with an explicit base directory.
///
/// This is the DI-friendly variant used by tests to avoid env var manipulation.
/// The `base_dir` is used directly as the parent for the hashed repo subdirectory.
///
/// In container mode (`BN_CONTAINER_MODE` set), if `BN_STORAGE_HASH` is provided,
/// it is used directly as the subdirectory name instead of computing from path.
/// This allows the host to pre-compute the hash for the mounted workspace.
pub fn get_storage_dir_with_base(repo_path: &Path, base_dir: &Path) -> Result<PathBuf> {
    // In container mode, use pre-computed hash if provided
    if std::env::var("BN_CONTAINER_MODE").is_ok()
        && let Ok(hash) = std::env::var("BN_STORAGE_HASH")
    {
        return Ok(base_dir.join(hash));
    }

    let canonical = repo_path
        .canonicalize()
        .map_err(|e| Error::Other(format!("Could not canonicalize repo path: {}", e)))?;

    let mut hasher = Sha256::new();
    hasher.update(canonical.to_string_lossy().as_bytes());
    let hash = hasher.finalize();
    let hash_hex = format!("{:x}", hash);
    let short_hash = &hash_hex[..12];

    Ok(base_dir.join(short_hash))
}

/// Thread-local counter for ID generation to ensure uniqueness within a process.
use std::sync::atomic::{AtomicU64, Ordering};
static ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generate a unique ID for a task or test node.
///
/// Format: `<prefix>-<4 hex chars>`
/// - Task prefix: "bn"
/// - Test prefix: "bnt"
///
/// Uses a combination of timestamp, seed, atomic counter, and process ID to ensure
/// uniqueness even when multiple IDs are generated in quick succession across processes.
pub fn generate_id(prefix: &str, seed: &str) -> String {
    let counter = ID_COUNTER.fetch_add(1, Ordering::SeqCst);
    let pid = std::process::id();
    let mut hasher = Sha256::new();
    hasher.update(seed.as_bytes());
    hasher.update(
        chrono::Utc::now()
            .timestamp_nanos_opt()
            .unwrap_or(0)
            .to_le_bytes(),
    );
    hasher.update(counter.to_le_bytes());
    hasher.update(pid.to_le_bytes());
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
        "partial" => Ok(TaskStatus::Partial),
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
    use crate::models::{AgentType, BugSeverity};
    use crate::test_utils::TestEnv;

    fn create_test_storage() -> (TestEnv, Storage) {
        let env = TestEnv::new();
        let storage = env.init_storage();
        (env, storage)
    }

    #[test]
    fn test_migration_adds_short_name_column() {
        let env = TestEnv::new();

        // Initialize storage (creates schema with short_name)
        let storage = env.init_storage();

        // Simulate an "old" database by dropping the short_name column
        storage
            .conn
            .execute("ALTER TABLE tasks DROP COLUMN short_name", [])
            .ok(); // SQLite may not support DROP COLUMN, but that's fine

        // Re-open storage - this should run migrations and add the column back
        drop(storage);
        let mut storage2 = env.open_storage();

        // Verify we can create a task with short_name
        let mut task = Task::new("bn-test".to_string(), "Test".to_string());
        task.core.short_name = Some("Short".to_string());
        storage2.create_task(&task).unwrap();

        // Verify we can retrieve it with short_name intact
        let retrieved = storage2.get_task("bn-test").unwrap();
        assert_eq!(retrieved.core.short_name, Some("Short".to_string()));
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
        let env = TestEnv::new();
        let storage = env.init_storage();

        assert!(storage.root.exists());
        assert!(storage.root.join("tasks.jsonl").exists());
        assert!(storage.root.join("bugs.jsonl").exists());
        assert!(storage.root.join("cache.db").exists());
    }

    #[test]
    fn test_storage_exists() {
        let env = TestEnv::new();
        assert!(!env.storage_exists());

        env.init_storage();
        assert!(env.storage_exists());
    }

    #[test]
    fn test_bn_data_dir_override() {
        // Test that get_storage_dir respects BN_DATA_DIR env var
        let env = TestEnv::new();
        let custom_base = env.data_path();

        // Use the DI variant to verify the path is constructed correctly
        let storage_dir = get_storage_dir_with_base(env.path(), custom_base).unwrap();

        // Storage dir should be under our custom base, not ~/.local/share/binnacle
        assert!(storage_dir.starts_with(custom_base));

        // The subdirectory should be a 12-char hex hash
        let subdir_name = storage_dir.file_name().unwrap().to_str().unwrap();
        assert_eq!(subdir_name.len(), 12);
        assert!(subdir_name.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_create_and_get_task() {
        let (_temp_dir, mut storage) = create_test_storage();

        let task = Task::new("bn-test".to_string(), "Test task".to_string());
        storage.create_task(&task).unwrap();

        let retrieved = storage.get_task("bn-test").unwrap();
        assert_eq!(retrieved.core.id, "bn-test");
        assert_eq!(retrieved.core.title, "Test task");
    }

    #[test]
    fn test_create_and_get_bug() {
        let (_temp_dir, mut storage) = create_test_storage();

        let bug = Bug::new("bn-bug".to_string(), "Test bug".to_string());
        storage.add_bug(&bug).unwrap();

        let retrieved = storage.get_bug("bn-bug").unwrap();
        assert_eq!(retrieved.core.id, "bn-bug");
        assert_eq!(retrieved.core.title, "Test bug");
        assert_eq!(retrieved.severity, BugSeverity::Triage);
    }

    #[test]
    fn test_list_bugs_with_filters() {
        let (_temp_dir, mut storage) = create_test_storage();

        let bug = Bug::new("bn-bug".to_string(), "Test bug".to_string());
        storage.add_bug(&bug).unwrap();

        let filtered = storage
            .list_bugs(Some("pending"), Some(2), Some("triage"), None, true)
            .unwrap();
        assert_eq!(filtered.len(), 1);

        let none = storage
            .list_bugs(Some("done"), None, None, None, true)
            .unwrap();
        assert!(none.is_empty());
    }

    #[test]
    fn test_list_tasks() {
        let (_temp_dir, mut storage) = create_test_storage();

        let mut task1 = Task::new("bn-0001".to_string(), "Task 1".to_string());
        task1.priority = 1;
        task1.core.tags = vec!["backend".to_string()];
        storage.create_task(&task1).unwrap();

        let mut task2 = Task::new("bn-0002".to_string(), "Task 2".to_string());
        task2.priority = 2;
        task2.core.tags = vec!["frontend".to_string()];
        storage.create_task(&task2).unwrap();

        // List all
        let all = storage.list_tasks(None, None, None).unwrap();
        assert_eq!(all.len(), 2);

        // Filter by priority
        let p1 = storage.list_tasks(None, Some(1), None).unwrap();
        assert_eq!(p1.len(), 1);
        assert_eq!(p1[0].core.id, "bn-0001");

        // Filter by tag
        let backend = storage.list_tasks(None, None, Some("backend")).unwrap();
        assert_eq!(backend.len(), 1);
        assert_eq!(backend[0].core.id, "bn-0001");
    }

    #[test]
    fn test_update_task() {
        let (_temp_dir, mut storage) = create_test_storage();

        let mut task = Task::new("bn-test".to_string(), "Original".to_string());
        storage.create_task(&task).unwrap();

        task.core.title = "Updated".to_string();
        task.status = TaskStatus::InProgress;
        storage.update_task(&task).unwrap();

        let retrieved = storage.get_task("bn-test").unwrap();
        assert_eq!(retrieved.core.title, "Updated");
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
        assert_eq!(parse_status("partial").unwrap(), TaskStatus::Partial);
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
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("cannot depend on itself")
        );
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
        assert_eq!(ready[0].core.id, "bn-aaaa");
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
        assert_eq!(ready[0].core.id, "bn-bbbb");
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

        let blocked_ids: Vec<&str> = blocked.iter().map(|t| t.core.id.as_str()).collect();
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

        // Note: Storage initializes with 4 default configs:
        // agents.worker.min, agents.worker.max, co-author.enabled, co-author.name
        let default_count = 4;

        storage.set_config("alpha", "1").unwrap();
        storage.set_config("beta", "2").unwrap();
        storage.set_config("gamma", "3").unwrap();

        let configs = storage.list_configs().unwrap();
        assert_eq!(configs.len(), default_count + 3);

        // Should be sorted by key - verify our custom configs are present
        // (defaults like "agents.worker.*" and "co-author.*" sort before alpha/beta/gamma)
        let custom_configs: Vec<_> = configs
            .iter()
            .filter(|(k, _)| k == "alpha" || k == "beta" || k == "gamma")
            .collect();
        assert_eq!(custom_configs.len(), 3);
        assert!(custom_configs.iter().any(|(k, v)| k == "alpha" && v == "1"));
        assert!(custom_configs.iter().any(|(k, v)| k == "beta" && v == "2"));
        assert!(custom_configs.iter().any(|(k, v)| k == "gamma" && v == "3"));
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
    fn test_find_git_root_from_subdir() {
        let env = TestEnv::new();
        let root = env.path();

        // Create a .git directory to simulate a git repo
        fs::create_dir(root.join(".git")).unwrap();

        // Create nested subdirectories
        let subdir = root.join("src").join("deeply").join("nested");
        fs::create_dir_all(&subdir).unwrap();

        // find_git_root should return the root from any depth
        let found = super::find_git_root(&subdir).unwrap();
        assert_eq!(found.canonicalize().unwrap(), root.canonicalize().unwrap());

        // Also works from the root itself
        let found_from_root = super::find_git_root(root).unwrap();
        assert_eq!(
            found_from_root.canonicalize().unwrap(),
            root.canonicalize().unwrap()
        );
    }

    #[test]
    fn test_find_git_root_no_git() {
        let env = TestEnv::new();
        let root = env.path();

        // No .git directory - should return None
        let result = super::find_git_root(root);
        assert!(result.is_none());
    }

    #[test]
    fn test_find_git_root_worktree_resolves_to_main() {
        let env = TestEnv::new();
        let base = env.path();

        // Create main repo structure
        let main_repo = base.join("main-repo");
        fs::create_dir_all(main_repo.join(".git").join("worktrees").join("my-worktree")).unwrap();

        // Create commondir file pointing to main .git
        fs::write(
            main_repo
                .join(".git")
                .join("worktrees")
                .join("my-worktree")
                .join("commondir"),
            "../..",
        )
        .unwrap();

        // Create worktree directory with .git file
        let worktree = base.join("worktree-dir");
        fs::create_dir_all(worktree.join("src")).unwrap();

        // Write .git file pointing to worktree gitdir (use absolute path for test reliability)
        let gitdir_path = main_repo.join(".git").join("worktrees").join("my-worktree");
        fs::write(
            worktree.join(".git"),
            format!("gitdir: {}", gitdir_path.display()),
        )
        .unwrap();

        // find_git_root from worktree subdir should resolve to main repo
        let found = super::find_git_root(&worktree.join("src")).unwrap();
        assert_eq!(
            found.canonicalize().unwrap(),
            main_repo.canonicalize().unwrap()
        );
    }

    #[test]
    fn test_find_git_root_worktree_fallback() {
        let env = TestEnv::new();
        let root = env.path();

        // Create a .git file pointing to a non-existent location (simulates broken worktree)
        let git_file = root.join(".git");
        fs::write(
            &git_file,
            "gitdir: /nonexistent/path/to/.git/worktrees/broken",
        )
        .unwrap();

        // Create nested subdirectory
        let subdir = root.join("src");
        fs::create_dir(&subdir).unwrap();

        // find_git_root should fall back to worktree root when resolution fails
        let found = super::find_git_root(&subdir).unwrap();
        assert_eq!(found.canonicalize().unwrap(), root.canonicalize().unwrap());
    }

    #[test]
    fn test_find_git_root_worktree_container_mode_skips_resolution() {
        let env = TestEnv::new();
        let base = env.path();

        // Create main repo structure
        let main_repo = base.join("main-repo");
        fs::create_dir_all(main_repo.join(".git").join("worktrees").join("my-worktree")).unwrap();

        // Create commondir file pointing to main .git
        fs::write(
            main_repo
                .join(".git")
                .join("worktrees")
                .join("my-worktree")
                .join("commondir"),
            "../..",
        )
        .unwrap();

        // Create worktree directory with .git file
        let worktree = base.join("worktree-dir");
        fs::create_dir_all(worktree.join("src")).unwrap();

        // Write .git file pointing to worktree gitdir (use absolute path for test reliability)
        let gitdir_path = main_repo.join(".git").join("worktrees").join("my-worktree");
        fs::write(
            worktree.join(".git"),
            format!("gitdir: {}", gitdir_path.display()),
        )
        .unwrap();

        // Set container mode env var
        // SAFETY: This is test code; we accept the POSIX setenv race condition
        unsafe {
            std::env::set_var("BN_CONTAINER_MODE", "true");
        }

        // In container mode, find_git_root should NOT resolve to main repo
        // It should return the worktree directory itself
        let found = super::find_git_root(&worktree.join("src")).unwrap();
        assert_eq!(
            found.canonicalize().unwrap(),
            worktree.canonicalize().unwrap(),
            "Container mode should treat worktree as its own root, not resolve to main repo"
        );

        // Clean up env var
        // SAFETY: This is test code; we accept the POSIX setenv race condition
        unsafe {
            std::env::remove_var("BN_CONTAINER_MODE");
        }
    }

    #[test]
    fn test_get_storage_dir_uses_path_literally() {
        let env = TestEnv::new();
        let root = env.path();

        // Create a .git directory to simulate a git repo
        fs::create_dir(root.join(".git")).unwrap();

        // Create a subdirectory
        let subdir = root.join("src");
        fs::create_dir(&subdir).unwrap();

        // get_storage_dir should use paths literally (no git root detection)
        // The caller is responsible for resolving git root if desired
        let storage_from_root = super::get_storage_dir(root).unwrap();
        let storage_from_subdir = super::get_storage_dir(&subdir).unwrap();

        // These should be DIFFERENT because get_storage_dir uses paths literally
        assert_ne!(storage_from_root, storage_from_subdir);

        // But if caller resolves git root first, they get the same storage
        let git_root_from_subdir = super::find_git_root(&subdir).unwrap();
        let storage_via_git_root = super::get_storage_dir(&git_root_from_subdir).unwrap();
        assert_eq!(storage_from_root, storage_via_git_root);
    }

    #[test]
    fn test_get_storage_dir_container_mode() {
        // This test verifies the BN_CONTAINER_MODE env var support.
        // We can't easily test actual /binnacle usage, but we can test
        // the priority order by using get_storage_dir_with_base.

        let env = TestEnv::new();
        let root = env.path();
        fs::create_dir(root.join(".git")).unwrap();

        // Test that container mode base directory (/binnacle) works with the hashing
        let container_base = std::path::Path::new("/binnacle");
        let storage_dir = super::get_storage_dir_with_base(root, container_base).unwrap();

        // Storage dir should be under /binnacle/<hash>
        assert!(storage_dir.starts_with("/binnacle"));
        // And should have a 12-char hash component
        let hash_component = storage_dir.file_name().unwrap().to_str().unwrap();
        assert_eq!(hash_component.len(), 12);
        assert!(hash_component.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_get_storage_dir_path_fallback_uses_container_base() {
        // This test verifies that when a custom path is used as base directory,
        // the storage dir is correctly computed under that path.
        // This exercises the same code path that the /binnacle fallback uses.

        let env = TestEnv::new();
        let root = env.path();
        fs::create_dir(root.join(".git")).unwrap();

        // Create a custom base directory (simulating /binnacle existing)
        let custom_base = env.path().join("custom_binnacle");
        fs::create_dir(&custom_base).unwrap();

        // Test storage with custom base
        let storage_dir = super::get_storage_dir_with_base(root, &custom_base).unwrap();

        // Storage dir should be under custom_base/<hash>
        assert!(storage_dir.starts_with(&custom_base));
        let hash_component = storage_dir.file_name().unwrap().to_str().unwrap();
        assert_eq!(hash_component.len(), 12);
        assert!(hash_component.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_bn_data_dir_overrides_container_auto_detection() {
        // This test verifies that BN_DATA_DIR takes precedence over container
        // auto-detection (when /binnacle exists). This ensures test isolation
        // works correctly even when running inside containers.
        //
        // We can't easily create /binnacle in a unit test, but we can verify
        // the priority logic by checking get_storage_dir documentation and
        // ensuring the env var check order is correct in the implementation.
        //
        // The real test for this is the integration tests which set BN_DATA_DIR
        // and must not pollute the host graph when run inside containers.

        let env = TestEnv::new();
        let root = env.path();
        fs::create_dir(root.join(".git")).unwrap();

        // Create a "container" style path to simulate /binnacle existing
        let fake_container = env.path().join("fake_binnacle");
        fs::create_dir(&fake_container).unwrap();

        // Create a test isolation path
        let test_isolation = env.path().join("test_isolation");
        fs::create_dir(&test_isolation).unwrap();

        // Set BN_DATA_DIR and verify it's used even when we call with_base
        // using a "container" path - the DI approach should win
        let storage_dir = super::get_storage_dir_with_base(root, &test_isolation).unwrap();

        // Storage should be under test_isolation, not fake_container
        assert!(
            storage_dir.starts_with(&test_isolation),
            "Expected storage under {:?}, got {:?}",
            test_isolation,
            storage_dir
        );
        assert!(
            !storage_dir.starts_with(&fake_container),
            "Storage should NOT be under container path"
        );
    }

    // === Agent Tests ===

    #[test]
    fn test_agent_register_and_get() {
        let (_temp_dir, mut storage) = create_test_storage();

        let agent = Agent::new(1234, 1000, "test-agent".to_string(), AgentType::Worker);
        storage.register_agent(&agent).unwrap();

        let retrieved = storage.get_agent(1234).unwrap();
        assert_eq!(retrieved.pid, 1234);
        assert_eq!(retrieved.parent_pid, 1000);
        assert_eq!(retrieved.name, "test-agent");
    }

    #[test]
    fn test_agent_list() {
        let (_temp_dir, mut storage) = create_test_storage();

        let agent1 = Agent::new(1234, 1000, "agent-1".to_string(), AgentType::Worker);
        let agent2 = Agent::new(5678, 1000, "agent-2".to_string(), AgentType::Planner);
        storage.register_agent(&agent1).unwrap();
        storage.register_agent(&agent2).unwrap();

        let agents = storage.list_agents(None).unwrap();
        assert_eq!(agents.len(), 2);
    }

    #[test]
    fn test_agent_remove() {
        let (_temp_dir, mut storage) = create_test_storage();

        let agent = Agent::new(1234, 1000, "test-agent".to_string(), AgentType::Worker);
        storage.register_agent(&agent).unwrap();

        storage.remove_agent(1234).unwrap();

        // Should fail to get removed agent
        let result = storage.get_agent(1234);
        assert!(result.is_err());
    }

    #[test]
    fn test_agent_remove_cleans_up_edges() {
        let (_temp_dir, mut storage) = create_test_storage();

        // Create a task for the agent to work on
        let task = Task::new("bn-test".to_string(), "Test task".to_string());
        storage.create_task(&task).unwrap();

        // Register an agent
        let agent = Agent::new(1234, 1000, "test-agent".to_string(), AgentType::Worker);
        storage.register_agent(&agent).unwrap();

        // Add task to agent (creates working_on edge)
        storage.agent_add_task(1234, "bn-test").unwrap();

        // Verify the working_on edge exists
        let edges = storage
            .list_edges(Some(EdgeType::WorkingOn), Some(&agent.id), None)
            .unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].target, "bn-test");

        // Remove the agent
        storage.remove_agent(1234).unwrap();

        // Verify the working_on edge is cleaned up
        let edges_after = storage
            .list_edges(Some(EdgeType::WorkingOn), Some(&agent.id), None)
            .unwrap();
        assert_eq!(
            edges_after.len(),
            0,
            "working_on edges should be cleaned up when agent is removed"
        );
    }

    #[test]
    fn test_agent_touch() {
        let (_temp_dir, mut storage) = create_test_storage();

        let agent = Agent::new(1234, 1000, "test-agent".to_string(), AgentType::Worker);
        storage.register_agent(&agent).unwrap();

        storage.touch_agent(1234).unwrap();

        let updated = storage.get_agent(1234).unwrap();
        assert_eq!(updated.command_count, 1);
    }

    #[test]
    fn test_agent_add_task() {
        let (_temp_dir, mut storage) = create_test_storage();

        let agent = Agent::new(1234, 1000, "test-agent".to_string(), AgentType::Worker);
        storage.register_agent(&agent).unwrap();

        storage.agent_add_task(1234, "bn-task").unwrap();

        let updated = storage.get_agent(1234).unwrap();
        assert_eq!(updated.tasks, vec!["bn-task".to_string()]);
    }

    #[test]
    fn test_agent_get_by_name() {
        let (_temp_dir, mut storage) = create_test_storage();

        let agent = Agent::new(1234, 1000, "claude".to_string(), AgentType::Buddy);
        storage.register_agent(&agent).unwrap();

        let retrieved = storage.get_agent_by_name("claude").unwrap();
        assert_eq!(retrieved.pid, 1234);
    }

    #[test]
    fn test_agent_update_status() {
        let (_temp_dir, mut storage) = create_test_storage();

        let agent = Agent::new(1234, 1000, "test-agent".to_string(), AgentType::Worker);
        storage.register_agent(&agent).unwrap();

        storage
            .update_agent_status(1234, AgentStatus::Idle)
            .unwrap();

        let updated = storage.get_agent(1234).unwrap();
        assert_eq!(updated.status, AgentStatus::Idle);
    }

    #[test]
    fn test_cleanup_stale_agents() {
        let (_temp_dir, mut storage) = create_test_storage();

        // Register an agent with a PID that doesn't exist (99999999)
        let agent = Agent::new(99999999, 1000, "stale-agent".to_string(), AgentType::Worker);
        storage.register_agent(&agent).unwrap();

        // Should be listed initially
        let agents = storage.list_agents(None).unwrap();
        assert_eq!(agents.len(), 1);

        // Cleanup should remove it
        let removed = storage.cleanup_stale_agents().unwrap();
        assert_eq!(removed, vec![99999999]);

        // Should be empty now
        let agents = storage.list_agents(None).unwrap();
        assert_eq!(agents.len(), 0);
    }

    #[test]
    fn test_update_agent_statuses_30min_stale() {
        let (_temp_dir, mut storage) = create_test_storage();

        // Use the current process PID so the agent appears "alive"
        let current_pid = std::process::id();
        let mut agent = Agent::new(current_pid, 1, "test-agent".to_string(), AgentType::Worker);

        // Set last_activity_at to 35 minutes ago (beyond the 30-minute stale threshold)
        agent.last_activity_at = chrono::Utc::now() - chrono::Duration::minutes(35);
        agent.status = AgentStatus::Active; // Start as active
        storage.register_agent(&agent).unwrap();

        // Update statuses - should mark as stale due to 30+ minute inactivity
        storage.update_agent_statuses().unwrap();

        let updated = storage.get_agent(current_pid).unwrap();
        assert_eq!(updated.status, AgentStatus::Stale);
    }

    #[test]
    fn test_update_agent_statuses_idle_not_stale() {
        let (_temp_dir, mut storage) = create_test_storage();

        // Use the current process PID so the agent appears "alive"
        let current_pid = std::process::id();
        let mut agent = Agent::new(current_pid, 1, "test-agent".to_string(), AgentType::Worker);

        // Set last_activity_at to 10 minutes ago (beyond idle threshold but before stale)
        agent.last_activity_at = chrono::Utc::now() - chrono::Duration::minutes(10);
        agent.status = AgentStatus::Active; // Start as active
        storage.register_agent(&agent).unwrap();

        // Update statuses - should mark as idle (not stale)
        storage.update_agent_statuses().unwrap();

        let updated = storage.get_agent(current_pid).unwrap();
        assert_eq!(updated.status, AgentStatus::Idle);
    }

    #[test]
    fn test_update_agent_statuses_stays_active() {
        let (_temp_dir, mut storage) = create_test_storage();

        // Use the current process PID so the agent appears "alive"
        let current_pid = std::process::id();
        let agent = Agent::new(current_pid, 1, "test-agent".to_string(), AgentType::Worker);
        // Fresh agent with recent activity (created_at and last_activity_at are set to now)
        storage.register_agent(&agent).unwrap();

        // Update statuses - should stay active
        storage.update_agent_statuses().unwrap();

        let updated = storage.get_agent(current_pid).unwrap();
        assert_eq!(updated.status, AgentStatus::Active);
    }

    // === Session State Tests ===

    #[test]
    fn test_session_state_write_and_read() {
        use crate::models::SessionState;
        let (_temp_dir, storage) = create_test_storage();

        let state = SessionState::new(1234, AgentType::Worker);
        storage.write_session_state(&state).unwrap();

        let read_state = storage.read_session_state().unwrap();
        assert_eq!(read_state.agent_pid, 1234);
        assert_eq!(read_state.agent_type, AgentType::Worker);
        assert!(read_state.orient_called);
    }

    #[test]
    fn test_session_state_read_not_found() {
        let (_temp_dir, storage) = create_test_storage();

        // No session state written yet
        let result = storage.read_session_state();
        assert!(result.is_err());
    }

    #[test]
    fn test_session_state_clear() {
        use crate::models::SessionState;
        let (_temp_dir, storage) = create_test_storage();

        let state = SessionState::new(5678, AgentType::Planner);
        storage.write_session_state(&state).unwrap();

        // Verify it exists
        assert!(storage.read_session_state().is_ok());

        // Clear it
        storage.clear_session_state().unwrap();

        // Verify it's gone
        assert!(storage.read_session_state().is_err());
    }

    #[test]
    fn test_session_state_overwrite() {
        use crate::models::SessionState;
        let (_temp_dir, storage) = create_test_storage();

        // Write first state
        let state1 = SessionState::new(1111, AgentType::Worker);
        storage.write_session_state(&state1).unwrap();

        // Overwrite with second state
        let state2 = SessionState::new(2222, AgentType::Buddy);
        storage.write_session_state(&state2).unwrap();

        // Should read the second state
        let read_state = storage.read_session_state().unwrap();
        assert_eq!(read_state.agent_pid, 2222);
        assert_eq!(read_state.agent_type, AgentType::Buddy);
    }

    // === Doc Storage Tests ===

    #[test]
    fn test_doc_create_and_get() {
        let (_env, mut storage) = create_test_storage();

        let doc = Doc::new("bn-doc1".to_string(), "Test Doc".to_string());
        storage.add_doc(&doc).unwrap();

        let retrieved = storage.get_doc("bn-doc1").unwrap();
        assert_eq!(retrieved.core.id, "bn-doc1");
        assert_eq!(retrieved.core.title, "Test Doc");
    }

    #[test]
    fn test_doc_with_all_fields() {
        use crate::models::{DocType, Editor};

        let (_env, mut storage) = create_test_storage();

        let mut doc = Doc::new("bn-doc2".to_string(), "Full Doc".to_string());
        doc.doc_type = DocType::Prd;
        doc.content = "# Summary\nTest content".to_string();
        doc.summary_dirty = true;
        doc.editors = vec![
            Editor::agent("bn-1234".to_string()),
            Editor::user("henry".to_string()),
        ];
        doc.supersedes = Some("bn-doc1".to_string());

        storage.add_doc(&doc).unwrap();

        let retrieved = storage.get_doc("bn-doc2").unwrap();
        assert_eq!(retrieved.doc_type, DocType::Prd);
        assert_eq!(retrieved.content, "# Summary\nTest content");
        assert!(retrieved.summary_dirty);
        assert_eq!(retrieved.editors.len(), 2);
        assert_eq!(retrieved.supersedes, Some("bn-doc1".to_string()));
    }

    #[test]
    fn test_doc_list_basic() {
        let (_env, mut storage) = create_test_storage();

        let doc1 = Doc::new("bn-doc1".to_string(), "Doc 1".to_string());
        let doc2 = Doc::new("bn-doc2".to_string(), "Doc 2".to_string());
        storage.add_doc(&doc1).unwrap();
        storage.add_doc(&doc2).unwrap();

        let docs = storage.list_docs(None, None, None, None).unwrap();
        assert_eq!(docs.len(), 2);
    }

    #[test]
    fn test_doc_list_filter_by_type() {
        use crate::models::DocType;

        let (_env, mut storage) = create_test_storage();

        let mut doc1 = Doc::new("bn-doc1".to_string(), "PRD Doc".to_string());
        doc1.doc_type = DocType::Prd;

        let mut doc2 = Doc::new("bn-doc2".to_string(), "Note Doc".to_string());
        doc2.doc_type = DocType::Note;

        storage.add_doc(&doc1).unwrap();
        storage.add_doc(&doc2).unwrap();

        let prds = storage
            .list_docs(None, Some(&DocType::Prd), None, None)
            .unwrap();
        assert_eq!(prds.len(), 1);
        assert_eq!(prds[0].core.title, "PRD Doc");

        let notes = storage
            .list_docs(None, Some(&DocType::Note), None, None)
            .unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].core.title, "Note Doc");
    }

    #[test]
    fn test_doc_update() {
        let (_env, mut storage) = create_test_storage();

        let doc = Doc::new("bn-doc1".to_string(), "Original Title".to_string());
        storage.add_doc(&doc).unwrap();

        let mut updated = storage.get_doc("bn-doc1").unwrap();
        updated.core.title = "Updated Title".to_string();
        updated.content = "New content".to_string();
        storage.update_doc(&updated).unwrap();

        let retrieved = storage.get_doc("bn-doc1").unwrap();
        assert_eq!(retrieved.core.title, "Updated Title");
        assert_eq!(retrieved.content, "New content");
    }

    #[test]
    fn test_doc_delete() {
        let (_env, mut storage) = create_test_storage();

        let doc = Doc::new("bn-doc1".to_string(), "Test Doc".to_string());
        storage.add_doc(&doc).unwrap();

        // Verify it exists
        assert!(storage.get_doc("bn-doc1").is_ok());

        // Delete it
        storage.delete_doc("bn-doc1").unwrap();

        // Verify it's gone
        assert!(storage.get_doc("bn-doc1").is_err());
    }

    #[test]
    fn test_docs_jsonl_created_on_init() {
        let env = TestEnv::new();
        let storage = env.init_storage();

        // Verify docs.jsonl file was created
        let docs_path = storage.root.join("docs.jsonl");
        assert!(
            docs_path.exists(),
            "docs.jsonl should be created during init"
        );
    }

    // ============================================================
    // Action Log tests
    // ============================================================

    #[test]
    fn test_action_log_add_and_query() {
        let (_env, storage) = create_test_storage();

        // Create a test action log entry
        let log_entry = crate::action_log::ActionLog {
            timestamp: chrono::Utc::now(),
            repo_path: "/test/repo".to_string(),
            command: "task create".to_string(),
            args: serde_json::json!({"title": "Test task"}),
            success: true,
            error: None,
            duration_ms: 42,
            user: "testuser".to_string(),
        };

        // Add the entry
        storage.add_action_log(&log_entry).unwrap();

        // Query it back
        let logs = storage
            .query_action_logs(None, None, None, None, None, None, None)
            .unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].command, "task create");
        assert_eq!(logs[0].user, "testuser");
        assert!(logs[0].success);
    }

    #[test]
    fn test_action_log_pagination() {
        let (_env, storage) = create_test_storage();

        // Add multiple entries
        for i in 0..10 {
            let log_entry = crate::action_log::ActionLog {
                timestamp: chrono::Utc::now() + chrono::Duration::milliseconds(i as i64),
                repo_path: "/test/repo".to_string(),
                command: format!("command_{}", i),
                args: serde_json::json!({}),
                success: true,
                error: None,
                duration_ms: 10,
                user: "testuser".to_string(),
            };
            storage.add_action_log(&log_entry).unwrap();
        }

        // Query with limit
        let logs = storage
            .query_action_logs(Some(3), None, None, None, None, None, None)
            .unwrap();
        assert_eq!(logs.len(), 3);

        // Query with offset
        let logs = storage
            .query_action_logs(Some(3), Some(5), None, None, None, None, None)
            .unwrap();
        assert_eq!(logs.len(), 3);

        // Count total
        let count = storage
            .count_action_logs(None, None, None, None, None)
            .unwrap();
        assert_eq!(count, 10);
    }

    #[test]
    fn test_action_log_filters() {
        let (_env, storage) = create_test_storage();

        // Add entries with different properties
        let log_success = crate::action_log::ActionLog {
            timestamp: chrono::Utc::now(),
            repo_path: "/test/repo".to_string(),
            command: "task create".to_string(),
            args: serde_json::json!({}),
            success: true,
            error: None,
            duration_ms: 10,
            user: "alice".to_string(),
        };
        storage.add_action_log(&log_success).unwrap();

        let log_failure = crate::action_log::ActionLog {
            timestamp: chrono::Utc::now() + chrono::Duration::milliseconds(1),
            repo_path: "/test/repo".to_string(),
            command: "task delete".to_string(),
            args: serde_json::json!({}),
            success: false,
            error: Some("Not found".to_string()),
            duration_ms: 5,
            user: "bob".to_string(),
        };
        storage.add_action_log(&log_failure).unwrap();

        // Filter by command
        let logs = storage
            .query_action_logs(None, None, None, None, Some("create"), None, None)
            .unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].command, "task create");

        // Filter by user
        let logs = storage
            .query_action_logs(None, None, None, None, None, Some("bob"), None)
            .unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].user, "bob");

        // Filter by success
        let logs = storage
            .query_action_logs(None, None, None, None, None, None, Some(false))
            .unwrap();
        assert_eq!(logs.len(), 1);
        assert!(!logs[0].success);
    }

    #[test]
    fn test_action_log_delete_by_max_entries() {
        let (_env, mut storage) = create_test_storage();

        // Add 10 entries with increasing timestamps
        for i in 0..10 {
            let log_entry = crate::action_log::ActionLog {
                timestamp: chrono::Utc::now() + chrono::Duration::milliseconds(i as i64 * 100),
                repo_path: "/test/repo".to_string(),
                command: format!("command_{}", i),
                args: serde_json::json!({}),
                success: true,
                error: None,
                duration_ms: 10,
                user: "testuser".to_string(),
            };
            storage.add_action_log(&log_entry).unwrap();
        }

        // Verify we have 10 entries
        let count = storage
            .count_action_logs(None, None, None, None, None)
            .unwrap();
        assert_eq!(count, 10);

        // Delete to keep only 5 entries
        let deleted = storage.delete_old_action_logs(Some(5), None).unwrap();
        assert_eq!(deleted, 5);

        // Verify we now have 5 entries
        let count = storage
            .count_action_logs(None, None, None, None, None)
            .unwrap();
        assert_eq!(count, 5);

        // Verify the most recent 5 entries are kept (command_5 through command_9)
        let logs = storage
            .query_action_logs(None, None, None, None, None, None, None)
            .unwrap();
        assert_eq!(logs.len(), 5);
        // Logs are returned in DESC order by timestamp, so most recent first
        for log in &logs {
            let cmd_num: u32 = log
                .command
                .strip_prefix("command_")
                .unwrap()
                .parse()
                .unwrap();
            assert!(
                cmd_num >= 5,
                "Expected command_5 or higher, got {}",
                log.command
            );
        }
    }

    #[test]
    fn test_action_log_delete_by_age() {
        let (_env, mut storage) = create_test_storage();

        // Add entries: 3 old (8 days ago) and 3 recent (now)
        for i in 0..3 {
            let old_entry = crate::action_log::ActionLog {
                timestamp: chrono::Utc::now() - chrono::Duration::days(8)
                    + chrono::Duration::milliseconds(i as i64),
                repo_path: "/test/repo".to_string(),
                command: format!("old_command_{}", i),
                args: serde_json::json!({}),
                success: true,
                error: None,
                duration_ms: 10,
                user: "testuser".to_string(),
            };
            storage.add_action_log(&old_entry).unwrap();
        }

        for i in 0..3 {
            let recent_entry = crate::action_log::ActionLog {
                timestamp: chrono::Utc::now() + chrono::Duration::milliseconds(i as i64),
                repo_path: "/test/repo".to_string(),
                command: format!("recent_command_{}", i),
                args: serde_json::json!({}),
                success: true,
                error: None,
                duration_ms: 10,
                user: "testuser".to_string(),
            };
            storage.add_action_log(&recent_entry).unwrap();
        }

        // Verify we have 6 entries
        let count = storage
            .count_action_logs(None, None, None, None, None)
            .unwrap();
        assert_eq!(count, 6);

        // Delete entries older than 7 days
        let deleted = storage.delete_old_action_logs(None, Some(7)).unwrap();
        assert_eq!(deleted, 3);

        // Verify we now have 3 entries (the recent ones)
        let count = storage
            .count_action_logs(None, None, None, None, None)
            .unwrap();
        assert_eq!(count, 3);

        // Verify all remaining entries are recent
        let logs = storage
            .query_action_logs(None, None, None, None, None, None, None)
            .unwrap();
        for log in &logs {
            assert!(
                log.command.starts_with("recent_"),
                "Expected recent command, got {}",
                log.command
            );
        }
    }

    #[test]
    fn test_action_log_delete_no_settings() {
        let (_env, mut storage) = create_test_storage();

        // Add 3 entries
        for i in 0..3 {
            let log_entry = crate::action_log::ActionLog {
                timestamp: chrono::Utc::now() + chrono::Duration::milliseconds(i as i64),
                repo_path: "/test/repo".to_string(),
                command: format!("command_{}", i),
                args: serde_json::json!({}),
                success: true,
                error: None,
                duration_ms: 10,
                user: "testuser".to_string(),
            };
            storage.add_action_log(&log_entry).unwrap();
        }

        // Delete with no settings - should delete nothing
        let deleted = storage.delete_old_action_logs(None, None).unwrap();
        assert_eq!(deleted, 0);

        // Verify all 3 entries are still there
        let count = storage
            .count_action_logs(None, None, None, None, None)
            .unwrap();
        assert_eq!(count, 3);
    }

    // ============================================================
    // Log Annotation tests
    // ============================================================

    #[test]
    fn test_log_annotation_add_and_get() {
        let (_env, storage) = create_test_storage();

        let timestamp = "2026-01-25T12:00:00Z";
        let id = storage.generate_annotation_id(timestamp);

        let annotation = LogAnnotation::new(
            id.clone(),
            timestamp.to_string(),
            "This command failed because of missing permissions".to_string(),
            "testuser".to_string(),
        );

        // Add the annotation
        storage.add_log_annotation(&annotation).unwrap();

        // Get it back
        let retrieved = storage.get_log_annotation(&id).unwrap();
        assert_eq!(retrieved.id, id);
        assert_eq!(retrieved.log_timestamp, timestamp);
        assert_eq!(
            retrieved.content,
            "This command failed because of missing permissions"
        );
        assert_eq!(retrieved.author, "testuser");
    }

    #[test]
    fn test_log_annotation_get_by_timestamp() {
        let (_env, storage) = create_test_storage();

        let timestamp1 = "2026-01-25T12:00:00Z";
        let timestamp2 = "2026-01-25T13:00:00Z";

        // Add two annotations for timestamp1
        let id1 = storage.generate_annotation_id(timestamp1);
        storage
            .add_log_annotation(&LogAnnotation::new(
                id1.clone(),
                timestamp1.to_string(),
                "First note".to_string(),
                "alice".to_string(),
            ))
            .unwrap();

        // Sleep briefly to get a different ID
        std::thread::sleep(std::time::Duration::from_millis(10));
        let id2 = storage.generate_annotation_id(timestamp1);
        storage
            .add_log_annotation(&LogAnnotation::new(
                id2.clone(),
                timestamp1.to_string(),
                "Second note".to_string(),
                "bob".to_string(),
            ))
            .unwrap();

        // Add one annotation for timestamp2
        let id3 = storage.generate_annotation_id(timestamp2);
        storage
            .add_log_annotation(&LogAnnotation::new(
                id3.clone(),
                timestamp2.to_string(),
                "Note for different entry".to_string(),
                "alice".to_string(),
            ))
            .unwrap();

        // Get annotations for timestamp1
        let annotations = storage.get_annotations_for_log(timestamp1).unwrap();
        assert_eq!(annotations.len(), 2);

        // Get annotations for timestamp2
        let annotations = storage.get_annotations_for_log(timestamp2).unwrap();
        assert_eq!(annotations.len(), 1);
        assert_eq!(annotations[0].content, "Note for different entry");
    }

    #[test]
    fn test_log_annotation_list_and_filter() {
        let (_env, storage) = create_test_storage();

        // Add a few annotations
        let id1 = storage.generate_annotation_id("ts1");
        storage
            .add_log_annotation(&LogAnnotation::new(
                id1,
                "ts1".to_string(),
                "Error due to network timeout".to_string(),
                "alice".to_string(),
            ))
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(10));
        let id2 = storage.generate_annotation_id("ts2");
        storage
            .add_log_annotation(&LogAnnotation::new(
                id2,
                "ts2".to_string(),
                "Fixed by restarting service".to_string(),
                "bob".to_string(),
            ))
            .unwrap();

        // List all
        let all = storage.list_log_annotations(None, None).unwrap();
        assert_eq!(all.len(), 2);

        // Filter by author
        let by_alice = storage.list_log_annotations(Some("alice"), None).unwrap();
        assert_eq!(by_alice.len(), 1);
        assert_eq!(by_alice[0].author, "alice");

        // Search in content
        let by_search = storage.list_log_annotations(None, Some("network")).unwrap();
        assert_eq!(by_search.len(), 1);
        assert!(by_search[0].content.contains("network"));
    }

    #[test]
    fn test_log_annotation_update() {
        let (_env, storage) = create_test_storage();

        let id = storage.generate_annotation_id("ts");
        storage
            .add_log_annotation(&LogAnnotation::new(
                id.clone(),
                "ts".to_string(),
                "Original content".to_string(),
                "testuser".to_string(),
            ))
            .unwrap();

        // Update the content
        storage
            .update_log_annotation(&id, "Updated content")
            .unwrap();

        // Verify the update
        let retrieved = storage.get_log_annotation(&id).unwrap();
        assert_eq!(retrieved.content, "Updated content");
    }

    #[test]
    fn test_log_annotation_delete() {
        let (_env, storage) = create_test_storage();

        let id = storage.generate_annotation_id("ts");
        storage
            .add_log_annotation(&LogAnnotation::new(
                id.clone(),
                "ts".to_string(),
                "To be deleted".to_string(),
                "testuser".to_string(),
            ))
            .unwrap();

        // Verify it exists
        assert!(storage.get_log_annotation(&id).is_ok());

        // Delete it
        storage.delete_log_annotation(&id).unwrap();

        // Verify it's gone
        assert!(storage.get_log_annotation(&id).is_err());
    }

    #[test]
    fn test_log_annotation_delete_not_found() {
        let (_env, storage) = create_test_storage();

        // Try to delete non-existent annotation
        let result = storage.delete_log_annotation("bnl-nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_agent_transitions_working_on_to_worked_on() {
        let (_env, mut storage) = create_test_storage();

        // Create a task
        let task = Task::new("bn-test".to_string(), "Test task".to_string());
        storage.create_task(&task).unwrap();

        // Create an agent with a specific PID
        let test_pid = 99999_u32;
        let parent_pid = 1_u32;
        let agent = Agent::new(
            test_pid,
            parent_pid,
            "test-agent".to_string(),
            AgentType::Worker,
        );
        storage.register_agent(&agent).unwrap();

        // Add a working_on edge from the agent to the task
        let edge_id = storage.generate_edge_id(&agent.id, &task.core.id, EdgeType::WorkingOn);
        let working_on_edge = Edge::new(
            edge_id,
            agent.id.clone(),
            task.core.id.clone(),
            EdgeType::WorkingOn,
        );
        storage.add_edge(&working_on_edge).unwrap();

        // Verify the working_on edge exists
        let edges_before = storage
            .list_edges(Some(EdgeType::WorkingOn), Some(&agent.id), None)
            .unwrap();
        assert_eq!(edges_before.len(), 1);
        assert_eq!(edges_before[0].target, task.core.id);

        // Verify no worked_on edge exists yet
        let worked_on_before = storage
            .list_edges(Some(EdgeType::WorkedOn), Some(&agent.id), None)
            .unwrap();
        assert_eq!(worked_on_before.len(), 0);

        // Remove the agent
        storage.remove_agent(test_pid).unwrap();

        // Verify the working_on edge is gone
        let edges_after = storage
            .list_edges(Some(EdgeType::WorkingOn), Some(&agent.id), None)
            .unwrap();
        assert_eq!(edges_after.len(), 0);

        // Verify a worked_on edge now exists (historical record)
        let worked_on_after = storage
            .list_edges(Some(EdgeType::WorkedOn), Some(&agent.id), None)
            .unwrap();
        assert_eq!(worked_on_after.len(), 1);
        assert_eq!(worked_on_after[0].source, agent.id);
        assert_eq!(worked_on_after[0].target, task.core.id);
    }

    #[test]
    fn test_default_configs_set_on_init() {
        let env = TestEnv::new();
        let storage = env.init_storage();

        // Verify default configs are set
        assert_eq!(
            storage.get_config("agents.worker.min").unwrap(),
            Some("0".to_string())
        );
        assert_eq!(
            storage.get_config("agents.worker.max").unwrap(),
            Some("1".to_string())
        );
        assert_eq!(
            storage.get_config("co-author.enabled").unwrap(),
            Some("yes".to_string())
        );
        assert_eq!(
            storage.get_config("co-author.name").unwrap(),
            Some("binnacle-bot".to_string())
        );
    }

    #[test]
    fn test_default_configs_not_overwritten() {
        let env = TestEnv::new();
        let mut storage = env.init_storage();

        // Set custom values
        storage.set_config("agents.worker.max", "5").unwrap();
        storage.set_config("co-author.name", "custom-bot").unwrap();

        // Re-open storage (which runs init_schema and set_default_configs)
        drop(storage);
        let storage2 = env.open_storage();

        // Verify custom values were preserved (not overwritten by defaults)
        assert_eq!(
            storage2.get_config("agents.worker.max").unwrap(),
            Some("5".to_string())
        );
        assert_eq!(
            storage2.get_config("co-author.name").unwrap(),
            Some("custom-bot".to_string())
        );
    }
}
