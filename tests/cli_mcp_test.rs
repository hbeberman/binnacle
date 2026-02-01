//! Integration tests for simplified MCP server functionality.
//!
//! The simplified MCP server provides:
//! - `binnacle-set_agent` - Initialize MCP session with path and optional session_id
//! - `binnacle-orient` - Register agent (limited via MCP)
//! - `binnacle-goodbye` - End agent session (limited via MCP)
//! - `bn_run` - Execute any bn CLI command as subprocess
//!
//! This replaces the previous 38+ individual tool approach with a simple
//! subprocess wrapper pattern.

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
        .stdout(predicate::str::contains("resources"));
}

#[test]
fn test_mcp_manifest_contains_set_agent_tool() {
    let env = setup();

    env.bn()
        .args(["mcp", "manifest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("binnacle-set_agent"))
        .stdout(predicate::str::contains("binnacle-managed repository"));
}

#[test]
fn test_mcp_manifest_contains_bn_run_tool() {
    let env = setup();

    env.bn()
        .args(["mcp", "manifest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("bn_run"))
        .stdout(predicate::str::contains("CLI command"));
}

#[test]
fn test_mcp_manifest_contains_status_resource() {
    let env = setup();

    env.bn()
        .args(["mcp", "manifest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("binnacle://status"));
}

#[test]
fn test_mcp_manifest_contains_agents_resource() {
    let env = setup();

    env.bn()
        .args(["mcp", "manifest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("binnacle://agents"));
}

#[test]
fn test_mcp_manifest_has_five_tools() {
    let env = setup();

    let output = env.bn().args(["mcp", "manifest"]).output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let manifest: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let tools = manifest["tools"].as_array().unwrap();
    assert_eq!(
        tools.len(),
        8,
        "Should have exactly 8 tools: binnacle-set_agent, binnacle-orient, binnacle-goodbye, bn_run, bn_lineage, bn_peers, bn_descendants, binnacle-debug"
    );
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
fn test_set_agent_schema_requires_path() {
    let env = setup();

    let output = env.bn().args(["mcp", "manifest"]).output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let manifest: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let tools = manifest["tools"].as_array().unwrap();
    let set_agent = tools
        .iter()
        .find(|t| t["name"] == "binnacle-set_agent")
        .expect("binnacle-set_agent tool not found");

    let schema = &set_agent["inputSchema"];
    assert_eq!(schema["type"], "object");

    let required = schema["required"].as_array().unwrap();
    assert!(required.iter().any(|r| r == "path"));

    // Verify session_id is optional (not in required)
    assert!(!required.iter().any(|r| r == "session_id"));

    // But verify it exists in properties
    let properties = schema["properties"].as_object().unwrap();
    assert!(properties.contains_key("session_id"));
}

#[test]
fn test_bn_run_schema_requires_args() {
    let env = setup();

    let output = env.bn().args(["mcp", "manifest"]).output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let manifest: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let tools = manifest["tools"].as_array().unwrap();
    let bn_run = tools
        .iter()
        .find(|t| t["name"] == "bn_run")
        .expect("bn_run tool not found");

    let schema = &bn_run["inputSchema"];
    assert_eq!(schema["type"], "object");

    let required = schema["required"].as_array().unwrap();
    assert!(required.iter().any(|r| r == "args"));

    // Verify args is an array type
    let properties = schema["properties"].as_object().unwrap();
    assert_eq!(properties["args"]["type"], "array");
}

#[test]
fn test_mcp_manifest_contains_lifecycle_tools() {
    let env = setup();

    let output = env.bn().args(["mcp", "manifest"]).output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let manifest: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let tools = manifest["tools"].as_array().unwrap();
    let tool_names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();

    assert!(
        tool_names.contains(&"binnacle-orient"),
        "Should have binnacle-orient tool"
    );
    assert!(
        tool_names.contains(&"binnacle-goodbye"),
        "Should have binnacle-goodbye tool"
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
            .env("BN_TEST_MODE", "1") // Enable test mode for production write protection
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
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
    // Set isolated data directory to prevent polluting host's binnacle data
    let temp_dir = tempfile::tempdir().unwrap();
    cmd.env("BN_DATA_DIR", temp_dir.path());
    // Enable test mode for production write protection
    cmd.env("BN_TEST_MODE", "1");
    cmd.args(["mcp", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("serve"))
        .stdout(predicate::str::contains("manifest"));
    std::mem::forget(temp_dir); // Keep temp directory alive
}

// === Timeout Tests ===

/// Test that MCP timeout works correctly with a slow-running test command.
///
/// This test:
/// 1. Creates a test node with `sleep 5` as the command
/// 2. Runs `bn test run` via MCP with a 1 second timeout
/// 3. Verifies the command times out with exit_code 124 and timed_out: true
///
/// The entire test should complete in ~2 seconds (1 second timeout + overhead).
#[test]
fn test_mcp_timeout_with_slow_command() {
    use std::io::{BufRead, BufReader};
    use std::time::Instant;

    let env = setup();

    // First, create a test node with a slow command using the CLI directly
    let output = env
        .bn()
        .args(["test", "create", "Slow test", "--cmd", "sleep 5"])
        .output()
        .expect("Failed to create test");
    assert!(output.status.success(), "Failed to create test node");

    // Extract the test ID from the output
    let stdout = String::from_utf8_lossy(&output.stdout);
    let test_id = stdout
        .split("\"id\":\"")
        .nth(1)
        .expect("No id in output")
        .split('"')
        .next()
        .expect("Malformed id")
        .to_string();

    // Now spawn MCP server with 1 second timeout
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_bn"))
        .args(["mcp", "serve"])
        .current_dir(env.repo_path())
        .env("BN_DATA_DIR", env.data_path())
        .env("BN_TEST_MODE", "1") // Enable test mode for production write protection
        .env("BN_MCP_TIMEOUT", "1") // 1 second timeout
        .env_remove("BN_CONTAINER_MODE") // Clear container mode to ensure consistent hashing
        .env_remove("BN_STORAGE_HASH") // Clear pre-computed hash
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn MCP server");

    let stdin = child.stdin.as_mut().expect("Failed to get stdin");
    let stdout_handle = child.stdout.take().expect("Failed to get stdout");
    let mut reader = BufReader::new(stdout_handle);

    // Helper to send request and read response
    fn send_recv(
        stdin: &mut impl Write,
        reader: &mut impl BufRead,
        request: &str,
    ) -> serde_json::Value {
        writeln!(stdin, "{}", request).expect("Failed to write");
        stdin.flush().expect("Failed to flush");
        let mut response = String::new();
        reader.read_line(&mut response).expect("Failed to read");
        serde_json::from_str(&response).expect("Invalid JSON")
    }

    // Initialize MCP session
    let init_resp = send_recv(
        stdin,
        &mut reader,
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}"#,
    );
    assert_eq!(init_resp["id"], 1);

    // Send initialized notification - in this implementation it sends a response
    writeln!(stdin, r#"{{"jsonrpc":"2.0","method":"initialized"}}"#).expect("Failed to write");
    stdin.flush().expect("Failed to flush");
    // Read the initialized response (this implementation responds to notifications)
    let mut _initialized_resp = String::new();
    reader
        .read_line(&mut _initialized_resp)
        .expect("Failed to read initialized response");

    // Set the agent working directory
    let set_agent_request = format!(
        r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"binnacle-set_agent","arguments":{{"path":"{}"}}}}}}"#,
        env.repo_path().display()
    );
    let set_agent_resp = send_recv(stdin, &mut reader, &set_agent_request);
    assert_eq!(set_agent_resp["id"], 2);

    // Now run the slow test via bn_run - this should timeout
    let start = Instant::now();
    let run_request = format!(
        r#"{{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{{"name":"bn_run","arguments":{{"args":["test","run","{}"]}} }} }}"#,
        test_id
    );
    let response = send_recv(stdin, &mut reader, &run_request);
    let elapsed = start.elapsed();

    assert_eq!(response["id"], 3, "Should get response for id 3");

    // Verify the response indicates timeout
    let content = response["result"]["content"][0]["text"]
        .as_str()
        .expect("Should have text content");

    let inner: serde_json::Value = serde_json::from_str(content).expect("bn output should be JSON");

    // Check for timeout indicators
    assert_eq!(
        inner["exit_code"], 124,
        "Should have exit_code 124 (timeout convention)"
    );
    assert_eq!(inner["timed_out"], true, "Should have timed_out: true");
    assert!(
        inner["stderr"].as_str().unwrap_or("").contains("timed out"),
        "stderr should mention timeout"
    );

    // Verify the test completed quickly (around 1-2 seconds, not 5 seconds)
    assert!(
        elapsed.as_secs() < 3,
        "Test should complete in ~2 seconds due to timeout, but took {} seconds",
        elapsed.as_secs()
    );

    // Clean up
    let _ = child.kill();
    let _ = child.wait();
}

// === Verify MCP manifest has proper structure ===

#[test]
fn test_mcp_manifest_has_server_info() {
    let env = setup();

    let output = env.bn().args(["mcp", "manifest"]).output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let manifest: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    // Verify manifest has name and version
    assert!(manifest["name"].is_string());
    assert!(manifest["version"].is_string());
    assert!(manifest["protocolVersion"].is_string());
}

#[test]
fn test_mcp_manifest_protocol_version() {
    let env = setup();

    let output = env.bn().args(["mcp", "manifest"]).output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let manifest: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    // Verify protocol version is present and valid
    let protocol = manifest["protocolVersion"].as_str().unwrap();
    assert!(
        protocol.starts_with("2024-"),
        "Protocol version should be in date format"
    );
}
