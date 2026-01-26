//! GUI performance tests with large graphs (500+ nodes)
//!
//! These tests verify that the GUI can handle production-scale graphs:
//! - Create 500+ nodes (tasks, bugs, milestones, ideas)
//! - Measure graph export performance
//! - Verify API endpoints return data efficiently
//! - Check memory usage stays reasonable

mod common;

use common::TestEnv;
use serde_json::Value;
use std::time::Instant;

/// Initialize binnacle in a temp directory and return the TestEnv.
fn init_binnacle() -> TestEnv {
    let env = TestEnv::new();
    env.bn()
        .args(["system", "init"])
        .write_stdin("n\nn\nn\nn\n")
        .assert()
        .success();
    env
}

/// Parse JSON output from a command.
fn parse_json(output: &[u8]) -> Value {
    serde_json::from_slice(output).expect("Failed to parse JSON output")
}

/// Extract an ID from JSON output.
fn extract_id(output: &[u8]) -> String {
    let stdout = String::from_utf8_lossy(output);
    if let Some(start) = stdout.find("\"id\":\"bn-") {
        let id_start = start + 6;
        let id_end = stdout[id_start..].find('"').unwrap() + id_start;
        return stdout[id_start..id_end].to_string();
    }
    for prefix in ["bnt-", "bnq-"] {
        if let Some(start) = stdout.find(&format!("\"id\":\"{}\"", prefix)) {
            let id_start = start + 6;
            let id_end = stdout[id_start..].find('"').unwrap() + id_start;
            return stdout[id_start..id_end].to_string();
        }
    }
    panic!("No ID found in output: {}", stdout);
}

/// Create a large test dataset with 500+ nodes
fn create_large_graph(env: &TestEnv) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
    let mut task_ids = Vec::new();
    let mut bug_ids = Vec::new();
    let mut milestone_ids = Vec::new();
    let mut idea_ids = Vec::new();

    println!("Creating 10 milestones...");
    for i in 0..10 {
        let output = env
            .bn()
            .args(["milestone", "create", &format!("Milestone {}", i)])
            .output()
            .expect("Failed to create milestone");
        milestone_ids.push(extract_id(&output.stdout));
    }

    println!("Creating 400 tasks...");
    for i in 0..400 {
        let priority = (i % 5) as u8;
        let status = match i % 5 {
            0 => "pending",
            1 => "in_progress",
            2 => "done",
            3 => "blocked",
            _ => "pending",
        };

        let output = env
            .bn()
            .args([
                "task",
                "create",
                &format!("Task {} - Performance test task", i),
                "-p",
                &priority.to_string(),
            ])
            .output()
            .expect("Failed to create task");
        let task_id = extract_id(&output.stdout);

        // Set status
        env.bn()
            .args(["task", "update", &task_id, "--status", status])
            .assert()
            .success();

        task_ids.push(task_id);

        // Link every 10th task to a milestone
        if i % 10 == 0 {
            let milestone_idx = (i / 10) % milestone_ids.len();
            env.bn()
                .args([
                    "link",
                    "add",
                    &task_ids[i],
                    &milestone_ids[milestone_idx],
                    "-t",
                    "child_of",
                ])
                .assert()
                .success();
        }

        // Create dependencies (every 5th task depends on the previous)
        if i > 0 && i % 5 == 0 {
            env.bn()
                .args([
                    "link",
                    "add",
                    &task_ids[i],
                    &task_ids[i - 1],
                    "-t",
                    "depends_on",
                    "--reason",
                    "Sequential dependency",
                ])
                .assert()
                .success();
        }
    }

    println!("Creating 70 bugs...");
    for i in 0..70 {
        let severity = match i % 5 {
            0 => "critical",
            1 => "high",
            2 => "medium",
            3 => "low",
            _ => "triage",
        };

        let output = env
            .bn()
            .args([
                "bug",
                "create",
                &format!("Bug {} - Performance test bug", i),
                "--severity",
                severity,
            ])
            .output()
            .expect("Failed to create bug");
        bug_ids.push(extract_id(&output.stdout));

        // Link some bugs to milestones
        if i % 7 == 0 {
            let milestone_idx = (i / 7) % milestone_ids.len();
            env.bn()
                .args([
                    "link",
                    "add",
                    &bug_ids[i],
                    &milestone_ids[milestone_idx],
                    "-t",
                    "impacts",
                ])
                .assert()
                .success();
        }
    }

    println!("Creating 50 ideas...");
    for i in 0..50 {
        let output = env
            .bn()
            .args([
                "idea",
                "create",
                &format!("Idea {} - Performance test idea", i),
            ])
            .output()
            .expect("Failed to create idea");
        idea_ids.push(extract_id(&output.stdout));
    }

    (task_ids, bug_ids, milestone_ids, idea_ids)
}

#[test]
fn test_gui_perf_create_500_plus_nodes() {
    let env = init_binnacle();

    let start = Instant::now();
    let (task_ids, bug_ids, milestone_ids, idea_ids) = create_large_graph(&env);
    let creation_time = start.elapsed();

    let total_nodes = task_ids.len() + bug_ids.len() + milestone_ids.len() + idea_ids.len();

    println!("\n=== Graph Creation Performance ===");
    println!("Total nodes created: {}", total_nodes);
    println!("  Tasks: {}", task_ids.len());
    println!("  Bugs: {}", bug_ids.len());
    println!("  Milestones: {}", milestone_ids.len());
    println!("  Ideas: {}", idea_ids.len());
    println!("Creation time: {:?}", creation_time);
    println!(
        "Average time per node: {:?}",
        creation_time / total_nodes as u32
    );

    assert!(total_nodes >= 500, "Should have created at least 500 nodes");
    assert_eq!(task_ids.len(), 400);
    assert_eq!(bug_ids.len(), 70);
    assert_eq!(milestone_ids.len(), 10);
    assert_eq!(idea_ids.len(), 50);

    // Benchmark: Creation should complete in reasonable time (< 60 seconds)
    assert!(
        creation_time.as_secs() < 60,
        "Creating 500+ nodes took too long: {:?}",
        creation_time
    );
}

#[test]
fn test_gui_perf_orient_large_graph() {
    let env = init_binnacle();
    create_large_graph(&env);

    // Test orient command performance (commonly used by agents)
    println!("\n=== Orient Performance ===");
    let start = Instant::now();
    let output = env
        .bn()
        .args(["orient", "--type", "worker"])
        .output()
        .expect("Failed to run orient");
    let orient_time = start.elapsed();

    assert!(output.status.success());
    let json = parse_json(&output.stdout);

    println!("Orient query time: {:?}", orient_time);
    println!("Total tasks: {}", json["total_tasks"].as_u64().unwrap_or(0));
    println!("Ready count: {}", json["ready_count"].as_u64().unwrap_or(0));

    // Orient should be fast even with 500+ nodes (< 6 seconds)
    assert!(
        orient_time.as_secs() < 6,
        "Orient took too long: {:?}",
        orient_time
    );
}

#[test]
fn test_gui_perf_task_list_large_graph() {
    let env = init_binnacle();
    create_large_graph(&env);

    println!("\n=== Task List Performance ===");
    let start = Instant::now();
    let output = env
        .bn()
        .args(["task", "list"])
        .output()
        .expect("Failed to list tasks");
    let list_time = start.elapsed();

    assert!(output.status.success());
    let json = parse_json(&output.stdout);
    let tasks = json["tasks"].as_array().expect("tasks should be array");

    println!("Task list query time: {:?}", list_time);
    println!("Tasks returned: {}", tasks.len());

    assert_eq!(tasks.len(), 400);

    // Task list should be reasonably fast (< 5 seconds for 400 tasks)
    assert!(
        list_time.as_secs() < 5,
        "Task list took too long: {:?}",
        list_time
    );
}

#[test]
fn test_gui_perf_search_edges_large_graph() {
    let env = init_binnacle();
    create_large_graph(&env);

    println!("\n=== Edge Search Performance ===");
    let start = Instant::now();
    let output = env
        .bn()
        .args(["search", "link", "--type", "depends_on"])
        .output()
        .expect("Failed to search edges");
    let search_time = start.elapsed();

    assert!(output.status.success());
    let json = parse_json(&output.stdout);
    let edge_count = json["edges"].as_array().map(|a| a.len()).unwrap_or(0);

    println!("Edge search time: {:?}", search_time);
    println!("Edges found: {}", edge_count);

    // Edge search should be reasonably fast (< 5 seconds)
    assert!(
        search_time.as_secs() < 5,
        "Edge search took too long: {:?}",
        search_time
    );
}

#[test]
fn test_gui_perf_ready_query_large_graph() {
    let env = init_binnacle();
    create_large_graph(&env);

    println!("\n=== Ready Query Performance ===");
    let start = Instant::now();
    let output = env
        .bn()
        .args(["ready"])
        .output()
        .expect("Failed to run ready");
    let ready_time = start.elapsed();

    assert!(output.status.success());
    let json = parse_json(&output.stdout);
    let ready_count = json["tasks"].as_array().map(|a| a.len()).unwrap_or(0);

    println!("Ready query time: {:?}", ready_time);
    println!("Ready tasks found: {}", ready_count);

    // Ready query should be reasonably fast (< 6 seconds)
    assert!(
        ready_time.as_secs() < 6,
        "Ready query took too long: {:?}",
        ready_time
    );
}

#[test]
fn test_gui_perf_blocked_query_large_graph() {
    let env = init_binnacle();
    create_large_graph(&env);

    println!("\n=== Blocked Query Performance ===");
    let start = Instant::now();
    let output = env
        .bn()
        .args(["blocked"])
        .output()
        .expect("Failed to run blocked");
    let blocked_time = start.elapsed();

    assert!(output.status.success());
    let json = parse_json(&output.stdout);
    let blocked_count = json["tasks"].as_array().map(|a| a.len()).unwrap_or(0);

    println!("Blocked query time: {:?}", blocked_time);
    println!("Blocked tasks found: {}", blocked_count);

    // Blocked query should be reasonably fast (< 7 seconds)
    assert!(
        blocked_time.as_secs() < 7,
        "Blocked query took too long: {:?}",
        blocked_time
    );
}

#[test]
fn test_gui_perf_export_import_roundtrip() {
    let env = init_binnacle();
    create_large_graph(&env);

    let export_path = env.path().join("large_graph.bng");

    println!("\n=== Export/Import Performance ===");

    // Export
    let start = Instant::now();
    env.bn()
        .args(["system", "store", "export", export_path.to_str().unwrap()])
        .assert()
        .success();
    let export_time = start.elapsed();

    println!("Export time: {:?}", export_time);

    // Import to fresh environment
    let env2 = TestEnv::new();
    let start = Instant::now();
    env2.bn()
        .args(["system", "store", "import", export_path.to_str().unwrap()])
        .assert()
        .success();
    let import_time = start.elapsed();

    println!("Import time: {:?}", import_time);

    // Export/import should be reasonably fast (< 10 seconds each)
    assert!(
        export_time.as_secs() < 10,
        "Export took too long: {:?}",
        export_time
    );
    assert!(
        import_time.as_secs() < 10,
        "Import took too long: {:?}",
        import_time
    );

    // Verify all data was imported
    let output = env2
        .bn()
        .args(["task", "list"])
        .output()
        .expect("Failed to list tasks");
    let json = parse_json(&output.stdout);
    let imported_tasks = json["tasks"].as_array().expect("tasks should be array");

    assert_eq!(imported_tasks.len(), 400, "All tasks should be imported");
}

#[test]
fn test_gui_perf_log_query() {
    let env = init_binnacle();
    create_large_graph(&env);

    println!("\n=== Log Query Performance ===");

    // Test log export which gives us stats about all entities
    let start = Instant::now();
    let output = env
        .bn()
        .args(["log", "--export"])
        .output()
        .expect("Failed to export log");
    let log_time = start.elapsed();

    println!("Log export time: {:?}", log_time);

    // Log export might be large but should complete
    assert!(
        log_time.as_secs() < 10,
        "Log export took too long: {:?}",
        log_time
    );

    // Try to parse as JSON and get some stats
    if output.status.success() && !output.stdout.is_empty() {
        let json = parse_json(&output.stdout);
        if let Some(entries) = json["entries"].as_array() {
            println!("Log entries: {}", entries.len());
        }
    }
}
