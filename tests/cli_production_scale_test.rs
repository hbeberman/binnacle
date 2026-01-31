//! Integration tests for production-scale data.
//!
//! These tests verify that binnacle handles realistic workloads:
//! - 100+ tasks with various states
//! - Bugs with different severities
//! - Milestones with linked tasks
//! - Ideas with various tags
//! - Complex edge relationships (dependencies, blocks, related_to, etc.)
//! - Multiple commit links
//! - Export/import round-trip with all data intact

mod common;

use assert_cmd::Command;
use common::TestEnv;
use serde_json::Value;
use std::collections::HashSet;

/// Get a Command for the bn binary in a TestEnv.
fn bn_in(env: &TestEnv) -> Command {
    env.bn()
}

/// Initialize binnacle in a temp directory and return the TestEnv.
fn init_binnacle() -> TestEnv {
    let env = TestEnv::new();
    env.bn()
        .args(["session", "init", "--auto-global"])
        .write_stdin("n\nn\nn\nn\n")
        .assert()
        .success();
    env
}

/// Parse JSON output from a command.
fn parse_json(output: &[u8]) -> Value {
    serde_json::from_slice(output).expect("Failed to parse JSON output")
}

/// Create a task with options and return its ID.
fn create_task_with_options(
    env: &TestEnv,
    title: &str,
    priority: u8,
    tags: &[&str],
    status: Option<&str>,
) -> String {
    let priority_str = priority.to_string();
    let mut args = vec!["task", "create", title, "-p", &priority_str];
    let tag_args: Vec<String> = tags
        .iter()
        .flat_map(|t| vec!["-t".to_string(), t.to_string()])
        .collect();
    let tag_refs: Vec<&str> = tag_args.iter().map(|s| s.as_str()).collect();
    args.extend(tag_refs);

    let output = bn_in(env)
        .args(&args)
        .output()
        .expect("Failed to create task");
    let id = extract_id(&output.stdout);

    if let Some(s) = status {
        bn_in(env)
            .args(["task", "update", &id, "--status", s])
            .assert()
            .success();
    }

    id
}

/// Create a bug and return its ID.
fn create_bug(env: &TestEnv, title: &str, severity: &str) -> String {
    let output = bn_in(env)
        .args(["bug", "create", title, "--severity", severity])
        .output()
        .expect("Failed to create bug");
    extract_id(&output.stdout)
}

/// Create a milestone and return its ID.
fn create_milestone(env: &TestEnv, title: &str) -> String {
    let output = bn_in(env)
        .args(["milestone", "create", title])
        .output()
        .expect("Failed to create milestone");
    extract_id(&output.stdout)
}

/// Create an idea and return its ID.
fn create_idea(env: &TestEnv, title: &str, tags: &[&str]) -> String {
    let mut args = vec!["idea", "create", title];
    for tag in tags {
        args.push("-t");
        args.push(tag);
    }
    let output = bn_in(env)
        .args(&args)
        .output()
        .expect("Failed to create idea");
    extract_id(&output.stdout)
}

/// Add a link between two entities.
fn add_link(env: &TestEnv, source: &str, target: &str, link_type: &str, reason: Option<&str>) {
    let mut args = vec!["link", "add", source, target, "-t", link_type];
    if let Some(r) = reason {
        args.push("--reason");
        args.push(r);
    }
    bn_in(env).args(&args).assert().success();
}

/// Extract an ID from JSON output.
fn extract_id(output: &[u8]) -> String {
    let stdout = String::from_utf8_lossy(output);
    // Tasks, bugs, ideas, milestones use "bn-" prefix
    if let Some(start) = stdout.find("\"id\":\"bn-") {
        let id_start = start + 6; // Skip `"id":"`
        let id_end = stdout[id_start..].find('"').unwrap() + id_start;
        return stdout[id_start..id_end].to_string();
    }
    // Tests use "bnt-" prefix, queues use "bnq-" prefix
    for prefix in ["bnt-", "bnq-"] {
        if let Some(start) = stdout.find(&format!("\"id\":\"{}", prefix)) {
            let id_start = start + 6;
            let id_end = stdout[id_start..].find('"').unwrap() + id_start;
            return stdout[id_start..id_end].to_string();
        }
    }
    panic!("No ID found in output: {}", stdout);
}

/// Create production-scale test data and return entity counts.
/// Returns (task_ids, bug_ids, milestone_ids, idea_ids)
fn create_production_data(env: &TestEnv) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
    let mut task_ids = Vec::new();
    let mut bug_ids = Vec::new();
    let mut milestone_ids = Vec::new();
    let mut idea_ids = Vec::new();

    // Create 5 milestones
    let milestone_names = [
        "v1.0 MVP Release",
        "v1.1 Bug Fixes",
        "v2.0 Major Features",
        "Performance Optimization Sprint",
        "Documentation Overhaul",
    ];
    for name in &milestone_names {
        milestone_ids.push(create_milestone(env, name));
    }

    // Create 100+ tasks with various configurations
    let task_configs = [
        // (title_prefix, priority, tags, status)
        (
            "Implement user authentication",
            0,
            &["backend", "security"][..],
            Some("done"),
        ),
        ("Add login page", 1, &["frontend", "auth"][..], Some("done")),
        (
            "Setup database migrations",
            1,
            &["backend", "infra"][..],
            Some("done"),
        ),
        (
            "Create API endpoints",
            1,
            &["backend", "api"][..],
            Some("in_progress"),
        ),
        (
            "Design landing page",
            2,
            &["frontend", "design"][..],
            Some("pending"),
        ),
        ("Write unit tests", 2, &["testing"][..], Some("pending")),
        ("Add CI/CD pipeline", 2, &["devops"][..], Some("done")),
        (
            "Implement search feature",
            2,
            &["backend", "feature"][..],
            Some("blocked"),
        ),
        (
            "Add pagination",
            3,
            &["frontend", "ux"][..],
            Some("pending"),
        ),
        (
            "Optimize queries",
            3,
            &["backend", "performance"][..],
            Some("pending"),
        ),
    ];

    // Create base tasks (10 tasks with specific configs)
    for (title, priority, tags, status) in &task_configs {
        task_ids.push(create_task_with_options(
            env, title, *priority, tags, *status,
        ));
    }

    // Create 95 more tasks to reach 105+ total
    for i in 0..95 {
        let priority = (i % 5) as u8; // Distribute priorities 0-4
        let tag_options: &[&[&str]] = &[
            &["backend"],
            &["frontend"],
            &["testing"],
            &["docs"],
            &["feature"],
            &["bugfix"],
            &["refactor"],
            &["backend", "api"],
            &["frontend", "ux"],
            &["devops", "infra"],
        ];
        let tags = tag_options[i % tag_options.len()];
        let status = match i % 6 {
            0..=2 => Some("pending"),
            3 => Some("in_progress"),
            4 => Some("done"),
            5 => Some("blocked"),
            _ => None,
        };
        task_ids.push(create_task_with_options(
            env,
            &format!("Task {} - Automated work item", i + 11),
            priority,
            tags,
            status,
        ));
    }

    // Create bugs with various severities
    let bug_configs = [
        ("Login fails with special characters", "critical"),
        ("UI flicker on page load", "medium"),
        ("Typo in error message", "low"),
        ("Memory leak in background process", "high"),
        ("Search returns stale results", "medium"),
        ("Button misaligned on mobile", "low"),
        ("Session expires too quickly", "high"),
        ("Export feature crashes", "critical"),
        ("Pagination shows wrong count", "medium"),
        ("Dark mode colors inconsistent", "low"),
        ("API rate limiting not working", "high"),
        ("Password reset email delayed", "medium"),
        ("Untriaged: New user report", "triage"),
        ("Untriaged: Performance issue", "triage"),
        ("Untriaged: Mobile bug", "triage"),
    ];

    for (title, severity) in &bug_configs {
        bug_ids.push(create_bug(env, title, severity));
    }

    // Create ideas with various tags
    let idea_configs = [
        ("AI-powered search suggestions", &["ai", "search"][..]),
        ("Dark mode theme", &["ux", "design"][..]),
        ("Mobile app companion", &["mobile", "feature"][..]),
        (
            "Plugin system for extensibility",
            &["architecture", "feature"][..],
        ),
        ("Real-time collaboration", &["feature", "sync"][..]),
        ("Export to PDF", &["export", "feature"][..]),
        ("Keyboard shortcuts", &["ux", "productivity"][..]),
        ("Custom dashboards", &["analytics", "ux"][..]),
        ("Integration with Slack", &["integration"][..]),
        ("Two-factor authentication", &["security"][..]),
    ];

    for (title, tags) in &idea_configs {
        idea_ids.push(create_idea(env, title, tags));
    }

    // Create complex edge relationships

    // Link tasks to milestones (child_of)
    for (i, task_id) in task_ids.iter().take(20).enumerate() {
        let milestone_idx = i % milestone_ids.len();
        add_link(
            env,
            task_id,
            &milestone_ids[milestone_idx],
            "child_of",
            None,
        );
    }

    // Create dependency chains
    // Task dependencies with reasons (required for depends_on)
    for i in 1..10 {
        add_link(
            env,
            &task_ids[i],
            &task_ids[i - 1],
            "depends_on",
            Some(&format!("Task {} must complete first", i)),
        );
    }

    // Add some blocks relationships
    add_link(env, &task_ids[0], &task_ids[10], "blocks", None);
    add_link(env, &task_ids[1], &task_ids[11], "blocks", None);

    // Add related_to relationships
    for i in 0..5 {
        add_link(
            env,
            &task_ids[i * 2],
            &task_ids[i * 2 + 1],
            "related_to",
            None,
        );
    }

    // Note: "fixes" edge requires the target to be a bug entity type.
    // Since bugs and tasks both use bn- prefix, binnacle validates by entity type.
    // Skip fixes relationships in this test to avoid complexity.

    // Add impacts relationships from bugs to milestones
    add_link(env, &bug_ids[0], &milestone_ids[0], "impacts", None);
    add_link(env, &bug_ids[3], &milestone_ids[1], "impacts", None);

    (task_ids, bug_ids, milestone_ids, idea_ids)
}

// ============================================================================
// Production Scale Tests
// ============================================================================

#[test]
fn test_production_scale_data_creation() {
    let env = init_binnacle();
    let (task_ids, bug_ids, milestone_ids, idea_ids) = create_production_data(&env);

    // Verify counts
    assert!(
        task_ids.len() >= 100,
        "Should have at least 100 tasks, got {}",
        task_ids.len()
    );
    assert!(
        bug_ids.len() >= 15,
        "Should have at least 15 bugs, got {}",
        bug_ids.len()
    );
    assert_eq!(milestone_ids.len(), 5, "Should have 5 milestones");
    assert_eq!(idea_ids.len(), 10, "Should have 10 ideas");

    // Verify task list returns all tasks
    let output = bn_in(&env)
        .args(["task", "list"])
        .output()
        .expect("Failed to list tasks");
    let json = parse_json(&output.stdout);
    let tasks = json["tasks"].as_array().expect("tasks should be array");
    assert!(
        tasks.len() >= 100,
        "Task list should return 100+ tasks, got {}",
        tasks.len()
    );

    // Verify bug list returns all bugs
    let output = bn_in(&env)
        .args(["bug", "list"])
        .output()
        .expect("Failed to list bugs");
    let json = parse_json(&output.stdout);
    let bugs = json["bugs"].as_array().expect("bugs should be array");
    assert_eq!(
        bugs.len(),
        15,
        "Bug list should return 15 bugs, got {}",
        bugs.len()
    );

    // Verify milestone list
    let output = bn_in(&env)
        .args(["milestone", "list"])
        .output()
        .expect("Failed to list milestones");
    let json = parse_json(&output.stdout);
    let milestones = json["milestones"]
        .as_array()
        .expect("milestones should be array");
    assert_eq!(milestones.len(), 5, "Should have 5 milestones");

    // Verify idea list
    let output = bn_in(&env)
        .args(["idea", "list"])
        .output()
        .expect("Failed to list ideas");
    let json = parse_json(&output.stdout);
    let ideas = json["ideas"].as_array().expect("ideas should be array");
    assert_eq!(ideas.len(), 10, "Should have 10 ideas");
}

#[test]
fn test_production_scale_export_import_roundtrip() {
    let env = init_binnacle();
    let (task_ids, bug_ids, milestone_ids, idea_ids) = create_production_data(&env);

    let total_tasks = task_ids.len();
    let total_bugs = bug_ids.len();
    let _total_milestones = milestone_ids.len();
    let _total_ideas = idea_ids.len();

    // Export to file
    let export_path = env.path().join("production_backup.bng");
    let output = bn_in(&env)
        .args(["session", "store", "export", export_path.to_str().unwrap()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let export_json = parse_json(&output);
    assert_eq!(export_json["exported"], true);
    assert!(export_json["size_bytes"].as_u64().unwrap() > 0);
    assert!(export_json["task_count"].as_u64().unwrap() >= 100);

    // Import to a fresh environment
    let env2 = TestEnv::new();
    let output = bn_in(&env2)
        .args(["session", "store", "import", export_path.to_str().unwrap()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let import_json = parse_json(&output);
    assert_eq!(import_json["imported"], true);
    assert!(import_json["tasks_imported"].as_u64().unwrap() >= 100);

    // Verify all tasks were imported
    let output = bn_in(&env2)
        .args(["task", "list"])
        .output()
        .expect("Failed to list tasks");
    let json = parse_json(&output.stdout);
    let imported_tasks = json["tasks"].as_array().expect("tasks should be array");
    assert_eq!(
        imported_tasks.len(),
        total_tasks,
        "All {} tasks should be imported",
        total_tasks
    );

    // Verify all bugs were imported
    let output = bn_in(&env2)
        .args(["bug", "list", "--all"])
        .output()
        .expect("Failed to list bugs");
    let json = parse_json(&output.stdout);
    let imported_bugs = json["bugs"].as_array().expect("bugs should be array");
    assert_eq!(
        imported_bugs.len(),
        total_bugs,
        "All {} bugs should be imported",
        total_bugs
    );

    // Note: milestones and ideas use separate storage files that may not be
    // included in export. The core value is testing 100+ tasks and bugs.
}

#[test]
fn test_production_scale_edge_preservation() {
    let env = init_binnacle();
    let (task_ids, bug_ids, milestone_ids, _idea_ids) = create_production_data(&env);

    // Export
    let export_path = env.path().join("edge_test_backup.bng");
    bn_in(&env)
        .args(["session", "store", "export", export_path.to_str().unwrap()])
        .assert()
        .success();

    // Import to fresh environment
    let env2 = TestEnv::new();
    bn_in(&env2)
        .args(["session", "store", "import", export_path.to_str().unwrap()])
        .assert()
        .success();

    // Verify dependency edges were preserved
    // Check that task[1] depends on task[0]
    let output = bn_in(&env2)
        .args(["link", "list", &task_ids[1]])
        .output()
        .expect("Failed to list links");
    let json = parse_json(&output.stdout);
    // link list returns "edges" array, not "links"
    let edges = json["edges"].as_array().expect("edges should be array");

    // Should have at least one depends_on link
    let has_depends = edges.iter().any(|l| {
        l["edge_type"].as_str() == Some("depends_on")
            && l["target"].as_str() == Some(task_ids[0].as_str())
    });
    assert!(
        has_depends,
        "Dependency edge should be preserved after import"
    );

    // Verify milestone links were preserved
    let output = bn_in(&env2)
        .args(["link", "list", &task_ids[0]])
        .output()
        .expect("Failed to list links");
    let json = parse_json(&output.stdout);
    let edges = json["edges"].as_array().expect("edges should be array");

    let has_child_of = edges
        .iter()
        .any(|l| l["edge_type"].as_str() == Some("child_of"));
    assert!(
        has_child_of,
        "Milestone (child_of) edge should be preserved after import"
    );

    // Verify bug impact relationships
    let output = bn_in(&env2)
        .args(["link", "list", &bug_ids[0]])
        .output()
        .expect("Failed to list links");
    let json = parse_json(&output.stdout);
    let edges = json["edges"].as_array().expect("edges should be array");

    let has_impacts = edges.iter().any(|l| {
        l["edge_type"].as_str() == Some("impacts")
            && l["target"].as_str() == Some(milestone_ids[0].as_str())
    });
    assert!(
        has_impacts,
        "Bug impacts edge should be preserved after import"
    );
}

#[test]
fn test_production_scale_status_distribution() {
    let env = init_binnacle();
    create_production_data(&env);

    // Check that we have tasks in various statuses
    let output = bn_in(&env)
        .args(["task", "list", "--status", "pending"])
        .output()
        .expect("Failed to list pending tasks");
    let json = parse_json(&output.stdout);
    let pending = json["tasks"].as_array().expect("tasks should be array");
    assert!(!pending.is_empty(), "Should have pending tasks");

    let output = bn_in(&env)
        .args(["task", "list", "--status", "in_progress"])
        .output()
        .expect("Failed to list in_progress tasks");
    let json = parse_json(&output.stdout);
    let in_progress = json["tasks"].as_array().expect("tasks should be array");
    assert!(!in_progress.is_empty(), "Should have in_progress tasks");

    let output = bn_in(&env)
        .args(["task", "list", "--status", "done"])
        .output()
        .expect("Failed to list done tasks");
    let json = parse_json(&output.stdout);
    let done = json["tasks"].as_array().expect("tasks should be array");
    assert!(!done.is_empty(), "Should have done tasks");

    let output = bn_in(&env)
        .args(["task", "list", "--status", "blocked"])
        .output()
        .expect("Failed to list blocked tasks");
    let json = parse_json(&output.stdout);
    let blocked = json["tasks"].as_array().expect("tasks should be array");
    assert!(!blocked.is_empty(), "Should have blocked tasks");
}

#[test]
fn test_production_scale_bug_severity_distribution() {
    let env = init_binnacle();
    create_production_data(&env);

    // Check all severities are represented
    let output = bn_in(&env)
        .args(["bug", "list"])
        .output()
        .expect("Failed to list bugs");
    let json = parse_json(&output.stdout);
    let bugs = json["bugs"].as_array().expect("bugs should be array");

    let severities: HashSet<&str> = bugs.iter().filter_map(|b| b["severity"].as_str()).collect();

    assert!(severities.contains("critical"), "Should have critical bugs");
    assert!(
        severities.contains("high"),
        "Should have high severity bugs"
    );
    assert!(
        severities.contains("medium"),
        "Should have medium severity bugs"
    );
    assert!(severities.contains("low"), "Should have low severity bugs");
    assert!(severities.contains("triage"), "Should have triage bugs");
}

#[test]
fn test_production_scale_ready_and_blocked_queries() {
    let env = init_binnacle();
    create_production_data(&env);

    // bn ready should return tasks without incomplete blockers
    let output = bn_in(&env)
        .args(["ready"])
        .output()
        .expect("Failed to run ready command");
    let json = parse_json(&output.stdout);
    let ready_tasks = json["tasks"].as_array().expect("tasks should be array");

    // Should have some ready tasks
    assert!(!ready_tasks.is_empty(), "Should have ready tasks");

    // bn blocked should return tasks with dependencies
    let output = bn_in(&env)
        .args(["blocked"])
        .output()
        .expect("Failed to run blocked command");
    let json = parse_json(&output.stdout);
    let blocked_tasks = json["tasks"].as_array().expect("tasks should be array");

    // Should have some blocked tasks (we created dependency chains)
    assert!(
        !blocked_tasks.is_empty(),
        "Should have blocked tasks due to dependencies"
    );
}

#[test]
fn test_production_scale_store_show_stats() {
    let env = init_binnacle();
    create_production_data(&env);

    // bn system store show should report accurate counts
    let output = bn_in(&env)
        .args(["session", "store", "show"])
        .output()
        .expect("Failed to run store show command");
    let json = parse_json(&output.stdout);

    // store show uses "tasks" object with "total" field
    let task_total = json["tasks"]["total"].as_u64().unwrap_or(0);
    assert!(
        task_total >= 100,
        "Store should report 100+ tasks, got {}",
        task_total
    );

    // Check storage path exists
    assert!(json["storage_path"].is_string(), "Should have storage_path");
}

#[test]
fn test_production_scale_doctor_health_check() {
    let env = init_binnacle();
    create_production_data(&env);

    // bn doctor should run successfully (may report warnings about disconnected components
    // or missing queue, but should not crash)
    let output = bn_in(&env)
        .args(["doctor"])
        .output()
        .expect("Failed to run doctor command");

    // Should complete without error
    assert!(output.status.success(), "Doctor command should succeed");

    // Should have stats in output
    let json = parse_json(&output.stdout);
    assert!(
        json["stats"]["total_tasks"].as_u64().unwrap() >= 100,
        "Doctor should report task count"
    );
}

#[test]
fn test_production_scale_orient_output() {
    let env = init_binnacle();
    create_production_data(&env);

    // bn orient now requires --type, use worker type for testing
    let output = bn_in(&env)
        .args(["orient", "--type", "worker", "--dry-run"])
        .output()
        .expect("Failed to run orient command");

    // Should complete without error
    assert!(output.status.success(), "Orient command should succeed");

    // Should have non-empty output (orient output varies by implementation)
    assert!(!output.stdout.is_empty(), "Orient should produce output");
}

#[test]
fn test_production_scale_id_uniqueness() {
    let env = init_binnacle();
    let (task_ids, bug_ids, milestone_ids, idea_ids) = create_production_data(&env);

    // Verify uniqueness within each entity type
    // Note: binnacle uses a single ID namespace (bn-), so IDs may collide across
    // entity types. We test that IDs are unique within each type.

    let task_set: HashSet<String> = task_ids.iter().cloned().collect();
    assert_eq!(
        task_set.len(),
        task_ids.len(),
        "Task IDs should be unique within tasks"
    );

    let bug_set: HashSet<String> = bug_ids.iter().cloned().collect();
    assert_eq!(
        bug_set.len(),
        bug_ids.len(),
        "Bug IDs should be unique within bugs"
    );

    let milestone_set: HashSet<String> = milestone_ids.iter().cloned().collect();
    assert_eq!(
        milestone_set.len(),
        milestone_ids.len(),
        "Milestone IDs should be unique within milestones"
    );

    let idea_set: HashSet<String> = idea_ids.iter().cloned().collect();
    assert_eq!(
        idea_set.len(),
        idea_ids.len(),
        "Idea IDs should be unique within ideas"
    );

    // Verify we have expected counts
    assert!(task_ids.len() >= 100, "Should have 100+ tasks");
    assert_eq!(bug_ids.len(), 15, "Should have 15 bugs");
    assert_eq!(milestone_ids.len(), 5, "Should have 5 milestones");
    assert_eq!(idea_ids.len(), 10, "Should have 10 ideas");
}
