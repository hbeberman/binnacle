//! Simplified MCP (Model Context Protocol) server implementation.
//!
//! This module provides a subprocess wrapper approach to MCP with 5 tools:
//! - `binnacle-set_agent` - Initialize MCP session with path and optional session_id
//! - `binnacle-orient` - Register agent and get project overview (limited via MCP)
//! - `binnacle-goodbye` - End agent session (limited via MCP)
//! - `bn_run` - Execute any bn CLI command as subprocess
//! - `binnacle-debug` - Dump server state and env vars for diagnostics
//!
//! Instead of 38+ individual tool handlers, this just executes the CLI directly.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;
use uuid::Uuid;
use wait_timeout::ChildExt;

/// MCP Protocol version
const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

/// Server information
const SERVER_NAME: &str = "binnacle";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Blocked subcommands that cause issues in MCP context.
/// `orient` and `goodbye` have dedicated MCP tools and should use shell for proper lifecycle.
/// `system` is blocked to prevent MCP clients from modifying global config files.
fn blocked_subcommands() -> HashSet<&'static str> {
    ["agent", "gui", "mcp", "orient", "goodbye", "system"]
        .iter()
        .copied()
        .collect()
}

// === JSON-RPC Types ===

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<Value>, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: None,
            }),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

// === MCP Types ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDef {
    pub uri: String,
    pub name: String,
    pub description: String,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

// === MCP Server ===

pub struct McpServer {
    cwd: Option<PathBuf>,
    session_id: String,
    /// True if session_id was provided externally (caller should use shell goodbye)
    /// False if auto-generated (safe to call bn goodbye via MCP)
    session_id_is_external: bool,
    /// Cached path to bn binary (captured at startup to survive binary replacement)
    bn_path: PathBuf,
}

impl McpServer {
    pub fn new() -> Self {
        // Cache the executable path at startup - on Linux, if the binary is replaced
        // while running, current_exe() returns a path ending in " (deleted)" which
        // won't work for spawning subprocesses.
        let bn_path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("bn"));
        Self {
            cwd: None,
            session_id: Uuid::new_v4().to_string(),
            session_id_is_external: false,
            bn_path,
        }
    }

    pub fn with_cwd(cwd: PathBuf) -> Self {
        let bn_path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("bn"));
        Self {
            cwd: Some(cwd),
            session_id: Uuid::new_v4().to_string(),
            session_id_is_external: false,
            bn_path,
        }
    }

    /// Handle a JSON-RPC request and return a response
    pub fn handle_request(&mut self, request: &JsonRpcRequest) -> JsonRpcResponse {
        match request.method.as_str() {
            "initialize" => self.handle_initialize(request),
            "initialized" => JsonRpcResponse::success(request.id.clone(), json!({})),
            "ping" => JsonRpcResponse::success(request.id.clone(), json!({})),
            "tools/list" => self.handle_tools_list(request),
            "tools/call" => self.handle_tools_call(request),
            "resources/list" => self.handle_resources_list(request),
            "resources/read" => self.handle_resources_read(request),
            _ => JsonRpcResponse::error(
                request.id.clone(),
                -32601,
                format!("Method not found: {}", request.method),
            ),
        }
    }

    fn handle_initialize(&mut self, request: &JsonRpcRequest) -> JsonRpcResponse {
        JsonRpcResponse::success(
            request.id.clone(),
            json!({
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {
                    "tools": {},
                    "resources": {
                        "subscribe": false,
                        "listChanged": false
                    }
                },
                "serverInfo": {
                    "name": SERVER_NAME,
                    "version": SERVER_VERSION
                }
            }),
        )
    }

    fn handle_tools_list(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let tools = get_tool_definitions();
        JsonRpcResponse::success(request.id.clone(), json!({ "tools": tools }))
    }

    fn handle_tools_call(&mut self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let params = match &request.params {
            Some(p) => p,
            None => {
                return JsonRpcResponse::error(
                    request.id.clone(),
                    -32602,
                    "Missing params".to_string(),
                );
            }
        };

        let tool_name = match params.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => {
                return JsonRpcResponse::error(
                    request.id.clone(),
                    -32602,
                    "Missing tool name".to_string(),
                );
            }
        };

        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));

        match self.execute_tool(tool_name, &arguments) {
            Ok(result) => JsonRpcResponse::success(
                request.id.clone(),
                json!({
                    "content": [{
                        "type": "text",
                        "text": result
                    }]
                }),
            ),
            Err(e) => JsonRpcResponse::success(
                request.id.clone(),
                json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Error: {}", e)
                    }],
                    "isError": true
                }),
            ),
        }
    }

    fn handle_resources_list(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let resources = get_resource_definitions();
        JsonRpcResponse::success(request.id.clone(), json!({ "resources": resources }))
    }

    fn handle_resources_read(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let params = match &request.params {
            Some(p) => p,
            None => {
                return JsonRpcResponse::error(
                    request.id.clone(),
                    -32602,
                    "Missing params".to_string(),
                );
            }
        };

        let uri = match params.get("uri").and_then(|v| v.as_str()) {
            Some(u) => u,
            None => {
                return JsonRpcResponse::error(
                    request.id.clone(),
                    -32602,
                    "Missing resource URI".to_string(),
                );
            }
        };

        match self.read_resource(uri) {
            Ok(content) => JsonRpcResponse::success(
                request.id.clone(),
                json!({
                    "contents": [{
                        "uri": uri,
                        "mimeType": "application/json",
                        "text": content
                    }]
                }),
            ),
            Err(e) => JsonRpcResponse::error(request.id.clone(), -32603, e),
        }
    }

    fn execute_tool(&mut self, name: &str, args: &Value) -> Result<String, String> {
        match name {
            "binnacle-set_agent" => self.tool_set_agent(args),
            "binnacle-orient" => self.tool_orient(args),
            "binnacle-goodbye" => self.tool_goodbye(args),
            "bn_run" => self.tool_bn_run(args),
            "bn_lineage" => self.tool_lineage(args),
            "bn_peers" => self.tool_peers(args),
            "bn_descendants" => self.tool_descendants(args),
            "binnacle-debug" => self.tool_debug(args),
            _ => Err(format!("Unknown tool: {}", name)),
        }
    }

    fn tool_set_agent(&mut self, args: &Value) -> Result<String, String> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'path' argument")?;

        // Expand ~ if present
        let expanded = if let Some(stripped) = path.strip_prefix("~/") {
            if let Some(home) = dirs::home_dir() {
                home.join(stripped)
            } else {
                PathBuf::from(path)
            }
        } else {
            PathBuf::from(path)
        };

        // Validate path exists and is a directory
        if !expanded.is_dir() {
            return Err(format!("Directory does not exist: {}", expanded.display()));
        }

        // Canonicalize to resolve symlinks and prevent path traversal attacks
        let canonical = expanded
            .canonicalize()
            .map_err(|e| format!("Failed to canonicalize path: {}", e))?;

        self.cwd = Some(canonical.clone());

        // Handle optional session_id - if provided externally, caller should use shell goodbye
        if let Some(session_id) = args.get("session_id").and_then(|v| v.as_str()) {
            self.session_id = session_id.to_string();
            self.session_id_is_external = true;
        }

        Ok(json!({
            "success": true,
            "message": format!("Session initialized{}",
                if self.session_id_is_external { " with external session" } else { "" }),
            "cwd": canonical.display().to_string(),
            "session_id": self.session_id,
            "session_id_is_external": self.session_id_is_external
        })
        .to_string())
    }

    fn tool_orient(&self, _args: &Value) -> Result<String, String> {
        // Check if cwd is set
        let cwd = self
            .cwd
            .as_ref()
            .ok_or("Working directory not set. Call binnacle-set_agent first.")?;

        // Execute bn orient
        let output = Command::new(&self.bn_path)
            .args(["orient", "--type", "worker"])
            .current_dir(cwd)
            .env("BN_MCP_SESSION", &self.session_id)
            .output()
            .map_err(|e| format!("Failed to execute: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        // Add MCP-specific hint to the response if using external session
        let mut result: Value = serde_json::from_str(&stdout).unwrap_or_else(|_| json!({}));
        if self.session_id_is_external
            && let Some(obj) = result.as_object_mut()
        {
            obj.insert(
                "mcp_hint".to_string(),
                json!("External session_id provided. Use shell 'bn goodbye' for proper lifecycle."),
            );
        }

        Ok(json!({
            "stdout": result.to_string(),
            "stderr": stderr,
            "exit_code": output.status.code().unwrap_or(-1)
        })
        .to_string())
    }

    fn tool_goodbye(&self, args: &Value) -> Result<String, String> {
        // If session_id was provided externally, the caller should use shell goodbye
        // for proper lifecycle management
        if self.session_id_is_external {
            return Ok(json!({
                "success": true,
                "terminated": false,
                "should_terminate": true,
                "hint": "External session_id provided. Use shell 'bn goodbye' for proper termination."
            })
            .to_string());
        }

        // Auto-generated session - safe to call bn goodbye
        // (BN_MCP_SESSION prevents actual process termination)
        let cwd = self
            .cwd
            .as_ref()
            .ok_or("Working directory not set. Call binnacle-set_agent first.")?;

        let summary = args
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("Session ended via MCP");

        let output = Command::new(&self.bn_path)
            .args(["goodbye", summary])
            .current_dir(cwd)
            .env("BN_MCP_SESSION", &self.session_id)
            .output()
            .map_err(|e| format!("Failed to execute: {}", e))?;

        Ok(json!({
            "stdout": String::from_utf8_lossy(&output.stdout),
            "stderr": String::from_utf8_lossy(&output.stderr),
            "exit_code": output.status.code().unwrap_or(-1),
            "terminated": false
        })
        .to_string())
    }

    fn tool_bn_run(&self, args: &Value) -> Result<String, String> {
        // Check if cwd is set
        let cwd = self
            .cwd
            .as_ref()
            .ok_or("Working directory not set. Call binnacle-set_agent first.")?;

        // Parse args array
        let cmd_args: Vec<String> = args
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        // Check for blocked subcommands
        let blocked = blocked_subcommands();
        if let Some(first_arg) = cmd_args.first()
            && blocked.contains(first_arg.as_str())
        {
            let hint = match first_arg.as_str() {
                "orient" | "goodbye" => format!(
                    ". Use the dedicated 'binnacle-{}' MCP tool, or shell 'bn {}' for full lifecycle support",
                    first_arg, first_arg
                ),
                _ => String::new(),
            };
            return Ok(json!({
                "stdout": "",
                "stderr": format!("Error: '{}' subcommand is blocked in bn_run{}", first_arg, hint),
                "exit_code": 1
            })
            .to_string());
        }

        // Execute with BN_MCP_SESSION to track this as an MCP call
        // Use spawn + wait_timeout to prevent hanging on blocking commands
        let mut cmd = Command::new(&self.bn_path);
        cmd.args(&cmd_args)
            .current_dir(cwd)
            .env("BN_MCP_SESSION", &self.session_id)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Pass through binnacle-specific env vars for storage resolution and agent identity
        for var in [
            "BN_DATA_DIR",
            "BN_CONTAINER_MODE",
            "BN_STORAGE_HASH",
            "BN_AGENT_ID",
            "BN_AGENT_SESSION",
            "BN_AGENT_NAME",
            "BN_AGENT_TYPE",
        ] {
            if let Ok(val) = std::env::var(var) {
                cmd.env(var, val);
            }
        }

        let mut child = cmd.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                format!("bn binary not found at '{}'", self.bn_path.display())
            } else {
                format!("Failed to execute command: {}", e)
            }
        })?;

        // Wait with configurable timeout (default 30s)
        let timeout_secs = std::env::var("BN_MCP_TIMEOUT")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(30);
        let timeout = Duration::from_secs(timeout_secs);
        match child.wait_timeout(timeout) {
            Ok(Some(status)) => {
                // Process completed within timeout
                let stdout = child
                    .stdout
                    .take()
                    .map(|mut s| {
                        let mut buf = Vec::new();
                        std::io::Read::read_to_end(&mut s, &mut buf).ok();
                        String::from_utf8_lossy(&buf).to_string()
                    })
                    .unwrap_or_default();
                let stderr = child
                    .stderr
                    .take()
                    .map(|mut s| {
                        let mut buf = Vec::new();
                        std::io::Read::read_to_end(&mut s, &mut buf).ok();
                        String::from_utf8_lossy(&buf).to_string()
                    })
                    .unwrap_or_default();

                Ok(json!({
                    "stdout": stdout,
                    "stderr": stderr,
                    "exit_code": status.code().unwrap_or(-1)
                })
                .to_string())
            }
            Ok(None) => {
                // Timeout - kill the process
                let _ = child.kill();
                let _ = child.wait(); // Clean up zombie
                Ok(json!({
                    "stdout": "",
                    "stderr": format!("Command timed out after {} seconds", timeout.as_secs()),
                    "exit_code": 124, // Unix timeout convention (same as coreutils timeout)
                    "timed_out": true
                })
                .to_string())
            }
            Err(e) => Err(format!("Failed to wait for command: {}", e)),
        }
    }

    fn tool_lineage(&self, args: &Value) -> Result<String, String> {
        let cwd = self
            .cwd
            .as_ref()
            .ok_or("Working directory not set. Call binnacle-set_agent first.")?;

        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'id' argument")?;

        let mut cmd_args = vec!["graph".to_string(), "lineage".to_string(), id.to_string()];

        if let Some(depth) = args.get("depth").and_then(|v| v.as_i64()) {
            cmd_args.push("--depth".to_string());
            cmd_args.push(depth.to_string());
        }

        if args
            .get("verbose")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            cmd_args.push("--verbose".to_string());
        }

        self.execute_bn_command(cwd, &cmd_args)
    }

    fn tool_peers(&self, args: &Value) -> Result<String, String> {
        let cwd = self
            .cwd
            .as_ref()
            .ok_or("Working directory not set. Call binnacle-set_agent first.")?;

        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'id' argument")?;

        let mut cmd_args = vec!["graph".to_string(), "peers".to_string(), id.to_string()];

        if let Some(depth) = args.get("depth").and_then(|v| v.as_i64()) {
            cmd_args.push("--depth".to_string());
            cmd_args.push(depth.to_string());
        }

        if args
            .get("include_closed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            cmd_args.push("--include-closed".to_string());
        }

        if args
            .get("verbose")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            cmd_args.push("--verbose".to_string());
        }

        self.execute_bn_command(cwd, &cmd_args)
    }

    fn tool_descendants(&self, args: &Value) -> Result<String, String> {
        let cwd = self
            .cwd
            .as_ref()
            .ok_or("Working directory not set. Call binnacle-set_agent first.")?;

        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'id' argument")?;

        let mut cmd_args = vec![
            "graph".to_string(),
            "descendants".to_string(),
            id.to_string(),
        ];

        if let Some(depth) = args.get("depth").and_then(|v| v.as_i64()) {
            cmd_args.push("--depth".to_string());
            cmd_args.push(depth.to_string());
        }

        if args.get("all").and_then(|v| v.as_bool()).unwrap_or(false) {
            cmd_args.push("--all".to_string());
        }

        if args
            .get("include_closed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            cmd_args.push("--include-closed".to_string());
        }

        if args
            .get("verbose")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            cmd_args.push("--verbose".to_string());
        }

        self.execute_bn_command(cwd, &cmd_args)
    }

    fn execute_bn_command(&self, cwd: &PathBuf, cmd_args: &[String]) -> Result<String, String> {
        let mut cmd = Command::new(&self.bn_path);
        cmd.args(cmd_args)
            .current_dir(cwd)
            .env("BN_MCP_SESSION", &self.session_id)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Pass through binnacle-specific env vars
        for var in [
            "BN_DATA_DIR",
            "BN_CONTAINER_MODE",
            "BN_STORAGE_HASH",
            "BN_AGENT_ID",
            "BN_AGENT_SESSION",
            "BN_AGENT_NAME",
            "BN_AGENT_TYPE",
        ] {
            if let Ok(val) = std::env::var(var) {
                cmd.env(var, val);
            }
        }

        let mut child = cmd.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                format!("bn binary not found at '{}'", self.bn_path.display())
            } else {
                format!("Failed to execute command: {}", e)
            }
        })?;

        // Wait with configurable timeout (default 30s)
        let timeout_secs = std::env::var("BN_MCP_TIMEOUT")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(30);
        let timeout = Duration::from_secs(timeout_secs);
        match child.wait_timeout(timeout) {
            Ok(Some(status)) => {
                let stdout = child
                    .stdout
                    .take()
                    .map(|mut s| {
                        let mut buf = Vec::new();
                        std::io::Read::read_to_end(&mut s, &mut buf).ok();
                        String::from_utf8_lossy(&buf).to_string()
                    })
                    .unwrap_or_default();
                let stderr = child
                    .stderr
                    .take()
                    .map(|mut s| {
                        let mut buf = Vec::new();
                        std::io::Read::read_to_end(&mut s, &mut buf).ok();
                        String::from_utf8_lossy(&buf).to_string()
                    })
                    .unwrap_or_default();

                Ok(json!({
                    "stdout": stdout,
                    "stderr": stderr,
                    "exit_code": status.code().unwrap_or(-1)
                })
                .to_string())
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                Ok(json!({
                    "stdout": "",
                    "stderr": format!("Command timed out after {} seconds", timeout.as_secs()),
                    "exit_code": 124,
                    "timed_out": true
                })
                .to_string())
            }
            Err(e) => Err(format!("Failed to wait for command: {}", e)),
        }
    }

    fn tool_debug(&self, _args: &Value) -> Result<String, String> {
        // Collect binnacle-specific env vars
        let bn_vars: Vec<(String, String)> = std::env::vars()
            .filter(|(k, _)| k.starts_with("BN_"))
            .collect();

        Ok(json!({
            "mcp_server_state": {
                "cwd": self.cwd.as_ref().map(|p| p.display().to_string()),
                "session_id": &self.session_id,
                "session_id_is_external": self.session_id_is_external,
                "bn_path": self.bn_path.display().to_string(),
            },
            "binnacle_env_vars": bn_vars.into_iter().collect::<std::collections::HashMap<_, _>>(),
        })
        .to_string())
    }

    fn read_resource(&self, uri: &str) -> Result<String, String> {
        let cwd = self
            .cwd
            .as_ref()
            .ok_or("Working directory not set. Call binnacle-set_agent first.")?;

        match uri {
            "binnacle://status" => {
                // Run bn status and return result
                let output = Command::new(&self.bn_path)
                    .current_dir(cwd)
                    .env("BN_MCP_SESSION", &self.session_id)
                    .output()
                    .map_err(|e| format!("Failed to get status: {}", e))?;

                if output.status.success() {
                    Ok(String::from_utf8_lossy(&output.stdout).to_string())
                } else {
                    Err(String::from_utf8_lossy(&output.stderr).to_string())
                }
            }
            "binnacle://agents" => {
                // Read AGENTS.md if present
                let agents_path = cwd.join("AGENTS.md");
                if agents_path.exists() {
                    std::fs::read_to_string(&agents_path)
                        .map_err(|e| format!("Failed to read AGENTS.md: {}", e))
                } else {
                    Ok(json!({"error": "AGENTS.md not found"}).to_string())
                }
            }
            _ => Err(format!("Unknown resource: {}", uri)),
        }
    }
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}

/// Get tool definitions for MCP
fn get_tool_definitions() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "binnacle-set_agent".to_string(),
            description: "Initialize binnacle MCP session. Must be called before using other binnacle tools.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute path to a binnacle-managed repository"
                    },
                    "session_id": {
                        "type": "string",
                        "description": "Optional MCP session ID. If provided, binnacle-goodbye will hint to use shell goodbye instead (for external lifecycle management)."
                    }
                },
                "required": ["path"]
            }),
        },
        ToolDef {
            name: "binnacle-orient".to_string(),
            description: "Register agent with binnacle and get project overview.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDef {
            name: "binnacle-goodbye".to_string(),
            description: "End agent session with binnacle. Cleans up agent registration. Does NOT terminate the agent process.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "summary": {
                        "type": "string",
                        "description": "Summary of what was accomplished in the session"
                    }
                },
                "required": []
            }),
        },
        ToolDef {
            name: "bn_run".to_string(),
            description: "Run a binnacle (bn) CLI command. Returns stdout, stderr, and exit code. Common commands: 'ready -H' (available work), 'task list -H', 'task create \"Title\"'".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "args": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Arguments to pass to bn (e.g., [\"ready\", \"-H\"])"
                    }
                },
                "required": ["args"]
            }),
        },
        ToolDef {
            name: "bn_lineage".to_string(),
            description: "Walk ancestry chain from a task up to its PRD document".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Entity ID to find lineage for"
                    },
                    "depth": {
                        "type": "integer",
                        "description": "Maximum hops (default: 10)"
                    },
                    "verbose": {
                        "type": "boolean",
                        "description": "Include descriptions"
                    }
                },
                "required": ["id"]
            }),
        },
        ToolDef {
            name: "bn_peers".to_string(),
            description: "Find sibling and cousin tasks through shared parents".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Entity ID to find peers for"
                    },
                    "depth": {
                        "type": "integer",
                        "description": "1=siblings, 2=siblings+cousins (default: 1)"
                    },
                    "include_closed": {
                        "type": "boolean",
                        "description": "Include closed tasks"
                    },
                    "verbose": {
                        "type": "boolean",
                        "description": "Include descriptions"
                    }
                },
                "required": ["id"]
            }),
        },
        ToolDef {
            name: "bn_descendants".to_string(),
            description: "Explore subtree below a node".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Entity ID to explore"
                    },
                    "depth": {
                        "type": "integer",
                        "description": "Max depth (default: 3)"
                    },
                    "all": {
                        "type": "boolean",
                        "description": "Show all descendants"
                    },
                    "include_closed": {
                        "type": "boolean",
                        "description": "Include closed tasks"
                    },
                    "verbose": {
                        "type": "boolean",
                        "description": "Include descriptions"
                    }
                },
                "required": ["id"]
            }),
        },
        ToolDef {
            name: "binnacle-debug".to_string(),
            description: "Debug tool: dumps MCP server state and environment variables. Use this to diagnose storage/path issues.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
    ]
}

/// Get resource definitions for MCP
fn get_resource_definitions() -> Vec<ResourceDef> {
    vec![
        ResourceDef {
            uri: "binnacle://status".to_string(),
            name: "Project Status".to_string(),
            description: "Current project status (task counts, queue state)".to_string(),
            mime_type: Some("application/json".to_string()),
        },
        ResourceDef {
            uri: "binnacle://agents".to_string(),
            name: "Agent Instructions".to_string(),
            description: "Content of AGENTS.md if present in the repository".to_string(),
            mime_type: Some("text/markdown".to_string()),
        },
    ]
}

/// Start the MCP server (stdio mode)
pub fn serve(_repo_path: &std::path::Path, cwd: Option<PathBuf>) {
    let stdin = io::stdin();
    let stdout = io::stdout();

    let mut server = match cwd {
        Some(path) => McpServer::with_cwd(path),
        None => McpServer::new(),
    };

    eprintln!(
        "Binnacle MCP server {} started (session: {})",
        SERVER_VERSION, server.session_id
    );

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("Error reading stdin: {}", e);
                continue;
            }
        };

        if line.is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let response = JsonRpcResponse::error(None, -32700, format!("Parse error: {}", e));
                let output = serde_json::to_string(&response).unwrap();
                println!("{}", output);
                let _ = stdout.lock().flush();
                continue;
            }
        };

        let response = server.handle_request(&request);
        let output = serde_json::to_string(&response).unwrap();
        println!("{}", output);
        let _ = stdout.lock().flush();
    }
}

/// Output the MCP manifest (tool definitions)
pub fn manifest() {
    let manifest = json!({
        "name": SERVER_NAME,
        "version": SERVER_VERSION,
        "protocolVersion": MCP_PROTOCOL_VERSION,
        "tools": get_tool_definitions(),
        "resources": get_resource_definitions(),
    });
    println!("{}", serde_json::to_string_pretty(&manifest).unwrap());
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_tool_definitions() {
        let tools = get_tool_definitions();
        assert_eq!(tools.len(), 8);
        assert!(tools.iter().any(|t| t.name == "binnacle-set_agent"));
        assert!(tools.iter().any(|t| t.name == "binnacle-orient"));
        assert!(tools.iter().any(|t| t.name == "binnacle-goodbye"));
        assert!(tools.iter().any(|t| t.name == "bn_run"));
        assert!(tools.iter().any(|t| t.name == "bn_lineage"));
        assert!(tools.iter().any(|t| t.name == "bn_peers"));
        assert!(tools.iter().any(|t| t.name == "bn_descendants"));
        assert!(tools.iter().any(|t| t.name == "binnacle-debug"));
    }

    #[test]
    fn test_resource_definitions() {
        let resources = get_resource_definitions();
        assert_eq!(resources.len(), 2);
        assert!(resources.iter().any(|r| r.uri == "binnacle://status"));
        assert!(resources.iter().any(|r| r.uri == "binnacle://agents"));
    }

    #[test]
    fn test_set_agent_missing_path() {
        let mut server = McpServer::new();
        let result = server.tool_set_agent(&json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing 'path'"));
    }

    #[test]
    fn test_set_agent_nonexistent_path() {
        let mut server = McpServer::new();
        let result = server.tool_set_agent(&json!({"path": "/nonexistent/path/xyz"}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }

    #[test]
    fn test_set_agent_valid_path() {
        let temp = TempDir::new().unwrap();
        let mut server = McpServer::new();
        let result = server.tool_set_agent(&json!({"path": temp.path().to_str().unwrap()}));
        assert!(result.is_ok());
        let json: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(json["success"], true);
        assert_eq!(json["session_id_is_external"], false);
    }

    #[test]
    fn test_set_agent_with_session_id() {
        let temp = TempDir::new().unwrap();
        let mut server = McpServer::new();
        let result = server.tool_set_agent(&json!({
            "path": temp.path().to_str().unwrap(),
            "session_id": "external-session-123"
        }));
        assert!(result.is_ok());
        let json: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(json["success"], true);
        assert_eq!(json["session_id"], "external-session-123");
        assert_eq!(json["session_id_is_external"], true);
    }

    #[test]
    fn test_set_agent_canonicalizes_path() {
        let temp = TempDir::new().unwrap();
        // Create a subdirectory and a symlink to it
        let subdir = temp.path().join("real_dir");
        std::fs::create_dir(&subdir).unwrap();
        let symlink = temp.path().join("link_dir");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&subdir, &symlink).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&subdir, &symlink).unwrap();

        let mut server = McpServer::new();
        let result = server.tool_set_agent(&json!({"path": symlink.to_str().unwrap()}));
        assert!(result.is_ok());
        let json: Value = serde_json::from_str(&result.unwrap()).unwrap();
        // The cwd should be the canonical (resolved) path, not the symlink
        let cwd = json["cwd"].as_str().unwrap();
        assert!(
            !cwd.contains("link_dir"),
            "Path should be canonicalized, but got: {}",
            cwd
        );
        assert!(
            cwd.contains("real_dir"),
            "Path should resolve to real_dir, but got: {}",
            cwd
        );
    }

    #[test]
    fn test_bn_run_without_cwd() {
        let server = McpServer::new();
        let result = server.tool_bn_run(&json!({"args": ["--version"]}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Working directory not set"));
    }

    #[test]
    fn test_bn_run_blocked_mcp() {
        let temp = TempDir::new().unwrap();
        let mut server = McpServer::new();
        server.cwd = Some(temp.path().to_path_buf());

        let result = server.tool_bn_run(&json!({"args": ["mcp", "serve"]}));
        assert!(result.is_ok());
        let json: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(json["exit_code"], 1);
        assert!(json["stderr"].as_str().unwrap().contains("blocked"));
    }

    #[test]
    fn test_bn_run_blocked_orient() {
        let temp = TempDir::new().unwrap();
        let mut server = McpServer::new();
        server.cwd = Some(temp.path().to_path_buf());

        let result = server.tool_bn_run(&json!({"args": ["orient"]}));
        assert!(result.is_ok());
        let json: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(json["exit_code"], 1);
        assert!(json["stderr"].as_str().unwrap().contains("blocked"));
        assert!(json["stderr"].as_str().unwrap().contains("binnacle-orient"));
    }

    #[test]
    fn test_bn_run_blocked_goodbye() {
        let temp = TempDir::new().unwrap();
        let mut server = McpServer::new();
        server.cwd = Some(temp.path().to_path_buf());

        let result = server.tool_bn_run(&json!({"args": ["goodbye", "test"]}));
        assert!(result.is_ok());
        let json: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(json["exit_code"], 1);
        assert!(json["stderr"].as_str().unwrap().contains("blocked"));
        assert!(
            json["stderr"]
                .as_str()
                .unwrap()
                .contains("binnacle-goodbye")
        );
    }

    #[test]
    fn test_bn_run_blocked_agent() {
        let temp = TempDir::new().unwrap();
        let mut server = McpServer::new();
        server.cwd = Some(temp.path().to_path_buf());

        let result = server.tool_bn_run(&json!({"args": ["agent", "kill", "bna-1234"]}));
        assert!(result.is_ok());
        let json: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(json["exit_code"], 1);
        assert!(json["stderr"].as_str().unwrap().contains("blocked"));
    }

    #[test]
    fn test_bn_run_blocked_system() {
        let temp = TempDir::new().unwrap();
        let mut server = McpServer::new();
        server.cwd = Some(temp.path().to_path_buf());

        let result = server.tool_bn_run(&json!({"args": ["system", "init"]}));
        assert!(result.is_ok());
        let json: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(json["exit_code"], 1);
        assert!(json["stderr"].as_str().unwrap().contains("blocked"));
    }

    #[test]
    fn test_bn_run_help() {
        let temp = TempDir::new().unwrap();
        let mut server = McpServer::new();
        server.cwd = Some(temp.path().to_path_buf());

        let result = server.tool_bn_run(&json!({"args": ["--help"]}));
        assert!(result.is_ok());
        let json: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(json["exit_code"], 0);
        assert!(json["stdout"].as_str().unwrap().contains("binnacle"));
    }

    #[test]
    fn test_initialize_response() {
        let mut server = McpServer::new();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "initialize".to_string(),
            params: None,
        };
        let response = server.handle_request(&request);
        assert!(response.result.is_some());
        let result = response.result.unwrap();
        assert_eq!(result["protocolVersion"], MCP_PROTOCOL_VERSION);
        assert_eq!(result["serverInfo"]["name"], SERVER_NAME);
    }

    #[test]
    fn test_tools_list() {
        let mut server = McpServer::new();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "tools/list".to_string(),
            params: None,
        };
        let response = server.handle_request(&request);
        assert!(response.result.is_some());
        let tools = &response.result.unwrap()["tools"];
        assert_eq!(tools.as_array().unwrap().len(), 8);
    }

    #[test]
    fn test_unknown_method() {
        let mut server = McpServer::new();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "unknown/method".to_string(),
            params: None,
        };
        let response = server.handle_request(&request);
        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, -32601);
    }
}
