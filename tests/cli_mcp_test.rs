//! Integration tests for MCP server functionality.
//!
//! These tests verify:
//! - `bn mcp manifest` outputs valid JSON with tools, resources, prompts
//! - MCP protocol message handling

#![allow(dead_code)] // McpServerHandle::send_request is for future use

use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;
use std::process::{Child, Stdio};
use tempfile::TempDir;

/// Get a Command for the bn binary.
fn bn() -> Command {
    Command::new(env!("CARGO_BIN_EXE_bn"))
}

/// Setup a temp directory and initialize binnacle.
fn setup() -> TempDir {
    let temp = TempDir::new().unwrap();
    bn().arg("init").current_dir(temp.path()).assert().success();
    temp
}

// === Manifest Tests ===

#[test]
fn test_mcp_manifest_outputs_json() {
    let temp = setup();

    bn().args(["mcp", "manifest"])
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("tools"))
        .stdout(predicate::str::contains("resources"))
        .stdout(predicate::str::contains("prompts"));
}

#[test]
fn test_mcp_manifest_contains_task_tools() {
    let temp = setup();

    bn().args(["mcp", "manifest"])
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("bn_task_create"))
        .stdout(predicate::str::contains("bn_task_list"))
        .stdout(predicate::str::contains("bn_task_show"))
        .stdout(predicate::str::contains("bn_task_update"))
        .stdout(predicate::str::contains("bn_task_close"));
}

#[test]
fn test_mcp_manifest_contains_dep_tools() {
    let temp = setup();

    bn().args(["mcp", "manifest"])
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("bn_dep_add"))
        .stdout(predicate::str::contains("bn_dep_rm"))
        .stdout(predicate::str::contains("bn_dep_show"));
}

#[test]
fn test_mcp_manifest_contains_test_tools() {
    let temp = setup();

    bn().args(["mcp", "manifest"])
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("bn_test_create"))
        .stdout(predicate::str::contains("bn_test_list"))
        .stdout(predicate::str::contains("bn_test_run"));
}

#[test]
fn test_mcp_manifest_contains_query_tools() {
    let temp = setup();

    bn().args(["mcp", "manifest"])
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("bn_ready"))
        .stdout(predicate::str::contains("bn_blocked"));
}

#[test]
fn test_mcp_manifest_contains_maintenance_tools() {
    let temp = setup();

    bn().args(["mcp", "manifest"])
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("bn_doctor"))
        .stdout(predicate::str::contains("bn_log"))
        .stdout(predicate::str::contains("bn_compact"));
}

#[test]
fn test_mcp_manifest_contains_resources() {
    let temp = setup();

    bn().args(["mcp", "manifest"])
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("binnacle://tasks"))
        .stdout(predicate::str::contains("binnacle://ready"))
        .stdout(predicate::str::contains("binnacle://blocked"))
        .stdout(predicate::str::contains("binnacle://status"));
}

#[test]
fn test_mcp_manifest_contains_prompts() {
    let temp = setup();

    bn().args(["mcp", "manifest"])
        .current_dir(temp.path())
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
    let temp = setup();

    let output = bn()
        .args(["mcp", "manifest"])
        .current_dir(temp.path())
        .output()
        .unwrap();

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
    let temp = setup();

    let output = bn()
        .args(["mcp", "manifest"])
        .current_dir(temp.path())
        .output()
        .unwrap();

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
    let temp = setup();

    let output = bn()
        .args(["mcp", "manifest"])
        .current_dir(temp.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let manifest: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    // Verify prompts have proper structure
    let prompts = manifest["prompts"].as_array().unwrap();
    assert!(!prompts.is_empty());

    for prompt in prompts {
        assert!(prompt["name"].is_string());
        assert!(prompt["description"].is_string());
        // arguments is optional but if present should be an array
        if let Some(args) = prompt.get("arguments") {
            if !args.is_null() {
                assert!(args.is_array());
            }
        }
    }
}

// === MCP Server Protocol Tests ===

/// Helper to spawn MCP server and interact with it
struct McpServerHandle {
    child: Child,
}

impl McpServerHandle {
    fn spawn(temp_dir: &TempDir) -> Self {
        let child = std::process::Command::new(env!("CARGO_BIN_EXE_bn"))
            .args(["mcp", "serve"])
            .current_dir(temp_dir.path())
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
    let temp = setup();

    // Just verify the server can start (it will block waiting for input)
    // We use a short timeout to verify it doesn't immediately crash
    let mut handle = McpServerHandle::spawn(&temp);

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
    bn().args(["mcp", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("serve"))
        .stdout(predicate::str::contains("manifest"));
}

// === Tool Schema Validation ===

#[test]
fn test_task_create_schema_has_required_title() {
    let temp = setup();

    let output = bn()
        .args(["mcp", "manifest"])
        .current_dir(temp.path())
        .output()
        .unwrap();

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
fn test_dep_add_schema_requires_child_and_parent() {
    let temp = setup();

    let output = bn()
        .args(["mcp", "manifest"])
        .current_dir(temp.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let manifest: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let tools = manifest["tools"].as_array().unwrap();
    let dep_add = tools.iter().find(|t| t["name"] == "bn_dep_add").unwrap();

    let required = dep_add["inputSchema"]["required"].as_array().unwrap();
    assert!(required.iter().any(|r| r == "child"));
    assert!(required.iter().any(|r| r == "parent"));
}

#[test]
fn test_status_tool_no_required_args() {
    let temp = setup();

    let output = bn()
        .args(["mcp", "manifest"])
        .current_dir(temp.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let manifest: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let tools = manifest["tools"].as_array().unwrap();
    let status = tools.iter().find(|t| t["name"] == "bn_status").unwrap();

    let required = status["inputSchema"]["required"].as_array().unwrap();
    assert!(required.is_empty());
}
