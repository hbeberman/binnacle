//! Integration tests for MCP server functionality.
//!
//! These tests verify:
//! - `bn mcp manifest` outputs valid JSON with tools, resources, prompts
//! - MCP protocol message handling

#![allow(dead_code)] // McpServerHandle::send_request is for future use

mod common;

use assert_cmd::Command;
use common::TestEnv;
use predicates::prelude::*;
use std::io::Write;
use std::process::{Child, Stdio};

/// Setup a TestEnv and initialize binnacle.
fn setup() -> TestEnv {
    TestEnv::init()
}

// === Manifest Tests ===

#[test]
fn test_mcp_manifest_outputs_json() {
    let env = setup();

    env.bn()
        .args(["mcp", "manifest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("tools"))
        .stdout(predicate::str::contains("resources"))
        .stdout(predicate::str::contains("prompts"));
}

#[test]
fn test_mcp_manifest_contains_task_tools() {
    let env = setup();

    env.bn()
        .args(["mcp", "manifest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("bn_task_create"))
        .stdout(predicate::str::contains("bn_task_list"))
        .stdout(predicate::str::contains("bn_task_show"))
        .stdout(predicate::str::contains("bn_task_update"))
        .stdout(predicate::str::contains("bn_task_close"));
}

#[test]
fn test_mcp_manifest_contains_link_tools() {
    let env = setup();

    env.bn()
        .args(["mcp", "manifest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("bn_link_add"))
        .stdout(predicate::str::contains("bn_link_rm"))
        .stdout(predicate::str::contains("bn_link_list"));
}

#[test]
fn test_mcp_manifest_contains_test_tools() {
    let env = setup();

    env.bn()
        .args(["mcp", "manifest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("bn_test_create"))
        .stdout(predicate::str::contains("bn_test_list"))
        .stdout(predicate::str::contains("bn_test_run"));
}

#[test]
fn test_mcp_manifest_contains_query_tools() {
    let env = setup();

    env.bn()
        .args(["mcp", "manifest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("bn_ready"))
        .stdout(predicate::str::contains("bn_blocked"));
}

#[test]
fn test_mcp_manifest_contains_milestone_tools() {
    let env = setup();

    env.bn()
        .args(["mcp", "manifest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("bn_milestone_create"))
        .stdout(predicate::str::contains("bn_milestone_list"))
        .stdout(predicate::str::contains("bn_milestone_show"))
        .stdout(predicate::str::contains("bn_milestone_update"))
        .stdout(predicate::str::contains("bn_milestone_close"))
        .stdout(predicate::str::contains("bn_milestone_reopen"))
        .stdout(predicate::str::contains("bn_milestone_delete"))
        .stdout(predicate::str::contains("bn_milestone_progress"));
}

#[test]
fn test_mcp_manifest_contains_search_tools() {
    let env = setup();

    env.bn()
        .args(["mcp", "manifest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("bn_search_link"));
}

#[test]
fn test_mcp_manifest_contains_maintenance_tools() {
    let env = setup();

    env.bn()
        .args(["mcp", "manifest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("bn_doctor"))
        .stdout(predicate::str::contains("bn_log"));
}

#[test]
fn test_mcp_manifest_contains_resources() {
    let env = setup();

    env.bn()
        .args(["mcp", "manifest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("binnacle://tasks"))
        .stdout(predicate::str::contains("binnacle://ready"))
        .stdout(predicate::str::contains("binnacle://blocked"))
        .stdout(predicate::str::contains("binnacle://status"));
}

#[test]
fn test_mcp_manifest_contains_prompts() {
    let env = setup();

    env.bn()
        .args(["mcp", "manifest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("start_work"))
        .stdout(predicate::str::contains("finish_work"))
        .stdout(predicate::str::contains("triage_regression"))
        .stdout(predicate::str::contains("plan_feature"))
        .stdout(predicate::str::contains("status_report"));
}

#[test]
fn test_mcp_manifest_tool_has_schema() {
    let env = setup();

    let output = env.bn().args(["mcp", "manifest"]).output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let manifest: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    // Verify tools have inputSchema
    let tools = manifest["tools"].as_array().unwrap();
    assert!(!tools.is_empty());

    for tool in tools {
        assert!(tool["name"].is_string());
        assert!(tool["description"].is_string());
        assert!(tool["inputSchema"].is_object());
    }
}

#[test]
fn test_mcp_manifest_resource_has_uri() {
    let env = setup();

    let output = env.bn().args(["mcp", "manifest"]).output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let manifest: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    // Verify resources have uri
    let resources = manifest["resources"].as_array().unwrap();
    assert!(!resources.is_empty());

    for resource in resources {
        assert!(resource["uri"].is_string());
        assert!(resource["name"].is_string());
        assert!(resource["description"].is_string());
    }
}

#[test]
fn test_mcp_manifest_prompt_has_arguments() {
    let env = setup();

    let output = env.bn().args(["mcp", "manifest"]).output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let manifest: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    // Verify prompts have proper structure
    let prompts = manifest["prompts"].as_array().unwrap();
    assert!(!prompts.is_empty());

    for prompt in prompts {
        assert!(prompt["name"].is_string());
        assert!(prompt["description"].is_string());
        // arguments is optional but if present should be an array
        if let Some(args) = prompt.get("arguments")
            && !args.is_null()
        {
            assert!(args.is_array());
        }
    }
}

#[test]
fn test_mcp_manifest_task_tools_have_short_name() {
    let env = setup();

    let output = env.bn().args(["mcp", "manifest"]).output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let manifest: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let tools = manifest["tools"].as_array().unwrap();

    // Find bn_task_create and verify it has short_name in schema
    let task_create = tools
        .iter()
        .find(|t| t["name"] == "bn_task_create")
        .expect("bn_task_create tool not found");
    let create_props = &task_create["inputSchema"]["properties"];
    assert!(
        create_props.get("short_name").is_some(),
        "bn_task_create should have short_name property"
    );
    assert_eq!(
        create_props["short_name"]["type"], "string",
        "short_name should be a string"
    );

    // Find bn_task_update and verify it has short_name in schema
    let task_update = tools
        .iter()
        .find(|t| t["name"] == "bn_task_update")
        .expect("bn_task_update tool not found");
    let update_props = &task_update["inputSchema"]["properties"];
    assert!(
        update_props.get("short_name").is_some(),
        "bn_task_update should have short_name property"
    );
    assert_eq!(
        update_props["short_name"]["type"], "string",
        "short_name should be a string"
    );
}

// === MCP Server Protocol Tests ===

/// Helper to spawn MCP server and interact with it
struct McpServerHandle {
    child: Child,
}

impl McpServerHandle {
    fn spawn(env: &TestEnv) -> Self {
        let child = std::process::Command::new(env!("CARGO_BIN_EXE_bn"))
            .args(["mcp", "serve"])
            .current_dir(env.repo_path())
            .env("BN_DATA_DIR", env.data_path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to spawn MCP server");

        Self { child }
    }

    fn send_request(&mut self, request: &str) -> String {
        let stdin = self.child.stdin.as_mut().expect("Failed to get stdin");
        writeln!(stdin, "{}", request).expect("Failed to write to stdin");
        stdin.flush().expect("Failed to flush stdin");

        // Give the server a moment to process
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Read response (this is tricky with blocking I/O)
        // For a proper test we'd need async or timeout reads
        // For now we just verify the server can be started
        String::new()
    }
}

impl Drop for McpServerHandle {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

#[test]
fn test_mcp_serve_starts() {
    let env = setup();

    // Just verify the server can start (it will block waiting for input)
    // We use a short timeout to verify it doesn't immediately crash
    let mut handle = McpServerHandle::spawn(&env);

    // Give it a moment to initialize
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Check it's still running (didn't crash)
    match handle.child.try_wait() {
        Ok(None) => {
            // Still running - good!
        }
        Ok(Some(status)) => {
            panic!("Server exited prematurely with status: {:?}", status);
        }
        Err(e) => {
            panic!("Error checking server status: {:?}", e);
        }
    }
}

#[test]
fn test_mcp_help() {
    Command::new(env!("CARGO_BIN_EXE_bn"))
        .args(["mcp", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("serve"))
        .stdout(predicate::str::contains("manifest"));
}

// === Tool Schema Validation ===

#[test]
fn test_task_create_schema_has_required_title() {
    let env = setup();

    let output = env.bn().args(["mcp", "manifest"]).output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let manifest: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let tools = manifest["tools"].as_array().unwrap();
    let task_create = tools
        .iter()
        .find(|t| t["name"] == "bn_task_create")
        .unwrap();

    let schema = &task_create["inputSchema"];
    assert_eq!(schema["type"], "object");

    let required = schema["required"].as_array().unwrap();
    assert!(required.iter().any(|r| r == "title"));
}

#[test]
fn test_link_add_schema_requires_source_and_target() {
    let env = setup();

    let output = env.bn().args(["mcp", "manifest"]).output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let manifest: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let tools = manifest["tools"].as_array().unwrap();
    let link_add = tools.iter().find(|t| t["name"] == "bn_link_add").unwrap();

    let required = link_add["inputSchema"]["required"].as_array().unwrap();
    assert!(required.iter().any(|r| r == "source"));
    assert!(required.iter().any(|r| r == "target"));
}

#[test]
fn test_status_tool_no_required_args() {
    let env = setup();

    let output = env.bn().args(["mcp", "manifest"]).output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let manifest: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let tools = manifest["tools"].as_array().unwrap();
    let status = tools.iter().find(|t| t["name"] == "bn_status").unwrap();

    let required = status["inputSchema"]["required"].as_array().unwrap();
    assert!(required.is_empty());
}

// === Queue Tool Tests ===

#[test]
fn test_mcp_manifest_contains_queue_tools() {
    let env = setup();

    env.bn()
        .args(["mcp", "manifest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("bn_queue_create"))
        .stdout(predicate::str::contains("bn_queue_show"))
        .stdout(predicate::str::contains("bn_queue_delete"));
}

#[test]
fn test_mcp_manifest_contains_queue_resource() {
    let env = setup();

    env.bn()
        .args(["mcp", "manifest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("binnacle://queue"));
}

#[test]
fn test_mcp_manifest_contains_prioritize_work_prompt() {
    let env = setup();

    env.bn()
        .args(["mcp", "manifest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("prioritize_work"));
}

#[test]
fn test_queue_create_schema_requires_title() {
    let env = setup();

    let output = env.bn().args(["mcp", "manifest"]).output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let manifest: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let tools = manifest["tools"].as_array().unwrap();
    let queue_create = tools
        .iter()
        .find(|t| t["name"] == "bn_queue_create")
        .expect("bn_queue_create tool not found");

    let schema = &queue_create["inputSchema"];
    assert_eq!(schema["type"], "object");

    let required = schema["required"].as_array().unwrap();
    assert!(required.iter().any(|r| r == "title"));

    // Verify description is optional
    let properties = schema["properties"].as_object().unwrap();
    assert!(properties.contains_key("description"));
}

#[test]
fn test_queue_show_schema_no_required_args() {
    let env = setup();

    let output = env.bn().args(["mcp", "manifest"]).output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let manifest: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let tools = manifest["tools"].as_array().unwrap();
    let queue_show = tools
        .iter()
        .find(|t| t["name"] == "bn_queue_show")
        .expect("bn_queue_show tool not found");

    let required = queue_show["inputSchema"]["required"].as_array().unwrap();
    assert!(required.is_empty());
}

#[test]
fn test_queue_delete_schema_no_required_args() {
    let env = setup();

    let output = env.bn().args(["mcp", "manifest"]).output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let manifest: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let tools = manifest["tools"].as_array().unwrap();
    let queue_delete = tools
        .iter()
        .find(|t| t["name"] == "bn_queue_delete")
        .expect("bn_queue_delete tool not found");

    let required = queue_delete["inputSchema"]["required"].as_array().unwrap();
    assert!(required.is_empty());
}
