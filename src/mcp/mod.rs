//! MCP (Model Context Protocol) server implementation.
//!
//! This module provides:
//! - `bn mcp serve` - Start stdio MCP server
//! - `bn mcp manifest` - Output tool definitions
//!
//! All CLI operations are exposed as MCP tools for AI agent integration.

use crate::commands::{self, Output};
use crate::Error;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::Path;

/// MCP Protocol version
const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

/// Server information
const SERVER_NAME: &str = "binnacle";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

// === JSON-RPC Types ===

/// JSON-RPC request structure
#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

/// JSON-RPC response structure
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

/// Tool definition for MCP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

/// Resource definition for MCP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDef {
    pub uri: String,
    pub name: String,
    pub description: String,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Prompt definition for MCP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptDef {
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Vec<PromptArgument>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptArgument {
    pub name: String,
    pub description: String,
    pub required: bool,
}

/// MCP Server state
pub struct McpServer {
    repo_path: std::path::PathBuf,
    initialized: bool,
}

impl McpServer {
    pub fn new(repo_path: &Path) -> Self {
        Self {
            repo_path: repo_path.to_path_buf(),
            initialized: false,
        }
    }

    /// Handle a JSON-RPC request and return a response
    pub fn handle_request(&mut self, request: &JsonRpcRequest) -> JsonRpcResponse {
        match request.method.as_str() {
            "initialize" => self.handle_initialize(request),
            "initialized" => {
                // Notification, no response needed but we'll mark as ready
                self.initialized = true;
                JsonRpcResponse::success(request.id.clone(), json!({}))
            }
            "ping" => JsonRpcResponse::success(request.id.clone(), json!({})),
            "tools/list" => self.handle_tools_list(request),
            "tools/call" => self.handle_tools_call(request),
            "resources/list" => self.handle_resources_list(request),
            "resources/read" => self.handle_resources_read(request),
            "prompts/list" => self.handle_prompts_list(request),
            "prompts/get" => self.handle_prompts_get(request),
            _ => JsonRpcResponse::error(
                request.id.clone(),
                -32601,
                format!("Method not found: {}", request.method),
            ),
        }
    }

    fn handle_initialize(&mut self, request: &JsonRpcRequest) -> JsonRpcResponse {
        self.initialized = true;
        JsonRpcResponse::success(
            request.id.clone(),
            json!({
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {
                    "tools": {},
                    "resources": {
                        "subscribe": false,
                        "listChanged": false
                    },
                    "prompts": {
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

    fn handle_tools_call(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let params = match &request.params {
            Some(p) => p,
            None => {
                return JsonRpcResponse::error(
                    request.id.clone(),
                    -32602,
                    "Missing params".to_string(),
                )
            }
        };

        let tool_name = match params.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => {
                return JsonRpcResponse::error(
                    request.id.clone(),
                    -32602,
                    "Missing tool name".to_string(),
                )
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
                )
            }
        };

        let uri = match params.get("uri").and_then(|v| v.as_str()) {
            Some(u) => u,
            None => {
                return JsonRpcResponse::error(
                    request.id.clone(),
                    -32602,
                    "Missing uri".to_string(),
                )
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
            Err(e) => JsonRpcResponse::error(request.id.clone(), -32000, e.to_string()),
        }
    }

    fn handle_prompts_list(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let prompts = get_prompt_definitions();
        JsonRpcResponse::success(request.id.clone(), json!({ "prompts": prompts }))
    }

    fn handle_prompts_get(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let params = match &request.params {
            Some(p) => p,
            None => {
                return JsonRpcResponse::error(
                    request.id.clone(),
                    -32602,
                    "Missing params".to_string(),
                )
            }
        };

        let prompt_name = match params.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => {
                return JsonRpcResponse::error(
                    request.id.clone(),
                    -32602,
                    "Missing prompt name".to_string(),
                )
            }
        };

        let arguments: HashMap<String, String> = params
            .get("arguments")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        match self.get_prompt(prompt_name, &arguments) {
            Ok((description, messages)) => JsonRpcResponse::success(
                request.id.clone(),
                json!({
                    "description": description,
                    "messages": messages
                }),
            ),
            Err(e) => JsonRpcResponse::error(request.id.clone(), -32000, e.to_string()),
        }
    }

    /// Execute a tool and return the result as JSON string
    fn execute_tool(&self, name: &str, args: &Value) -> Result<String, Error> {
        let repo = &self.repo_path;

        match name {
            "bn_init" => {
                let result = commands::init(repo)?;
                Ok(result.to_json())
            }
            "bn_status" => {
                let result = commands::status(repo)?;
                Ok(result.to_json())
            }
            "bn_task_create" => {
                let title = get_string_arg(args, "title")?;
                let short_name = get_optional_string(args, "short_name");
                let description = get_optional_string(args, "description");
                let priority = get_optional_u8(args, "priority");
                let tags = get_string_array(args, "tags");
                let assignee = get_optional_string(args, "assignee");
                let result = commands::task_create(
                    repo,
                    title,
                    short_name,
                    description,
                    priority,
                    tags,
                    assignee,
                )?;
                Ok(result.to_json())
            }
            "bn_task_list" => {
                let status = get_optional_string(args, "status");
                let priority = get_optional_u8(args, "priority");
                let tag = get_optional_string(args, "tag");
                let result =
                    commands::task_list(repo, status.as_deref(), priority, tag.as_deref())?;
                Ok(result.to_json())
            }
            "bn_task_show" => {
                let id = get_string_arg(args, "id")?;
                let result = commands::task_show(repo, &id)?;
                Ok(result.to_json())
            }
            "bn_task_update" => {
                let id = get_string_arg(args, "id")?;
                let title = get_optional_string(args, "title");
                let short_name = get_optional_string(args, "short_name");
                let description = get_optional_string(args, "description");
                let priority = get_optional_u8(args, "priority");
                let status = get_optional_string(args, "status");
                let add_tags = get_string_array(args, "add_tags");
                let remove_tags = get_string_array(args, "remove_tags");
                let assignee = get_optional_string(args, "assignee");
                let force = get_optional_bool(args, "force").unwrap_or(false);
                let result = commands::task_update(
                    repo,
                    &id,
                    title,
                    short_name,
                    description,
                    priority,
                    status.as_deref(),
                    add_tags,
                    remove_tags,
                    assignee,
                    force,
                )?;
                Ok(result.to_json())
            }
            "bn_task_close" => {
                let id = get_string_arg(args, "id")?;
                let reason = get_optional_string(args, "reason");
                let force = get_optional_bool(args, "force").unwrap_or(false);
                let result = commands::task_close(repo, &id, reason, force)?;
                Ok(result.to_json())
            }
            "bn_task_reopen" => {
                let id = get_string_arg(args, "id")?;
                let result = commands::task_reopen(repo, &id)?;
                Ok(result.to_json())
            }
            "bn_task_delete" => {
                let id = get_string_arg(args, "id")?;
                let result = commands::task_delete(repo, &id)?;
                Ok(result.to_json())
            }
            "bn_link_add" => {
                let source = get_string_arg(args, "source")?;
                let target = get_string_arg(args, "target")?;
                let edge_type = get_string_arg(args, "edge_type")?;
                let reason = get_optional_string(args, "reason");
                let result = commands::link_add(repo, &source, &target, &edge_type, reason)?;
                Ok(result.to_json())
            }
            "bn_link_rm" => {
                let source = get_string_arg(args, "source")?;
                let target = get_string_arg(args, "target")?;
                let edge_type = get_optional_string(args, "edge_type");
                let result = commands::link_rm(repo, &source, &target, edge_type.as_deref())?;
                Ok(result.to_json())
            }
            "bn_link_list" => {
                let entity_id = get_optional_string(args, "entity_id");
                let all = args.get("all").and_then(|v| v.as_bool()).unwrap_or(false);
                let edge_type = get_optional_string(args, "edge_type");
                let result =
                    commands::link_list(repo, entity_id.as_deref(), all, edge_type.as_deref())?;
                Ok(result.to_json())
            }
            "bn_ready" => {
                let result = commands::ready(repo)?;
                Ok(result.to_json())
            }
            "bn_blocked" => {
                let result = commands::blocked(repo)?;
                Ok(result.to_json())
            }
            "bn_test_create" => {
                let name = get_string_arg(args, "name")?;
                let command = get_string_arg(args, "command")?;
                let working_dir =
                    get_optional_string(args, "working_dir").unwrap_or_else(|| ".".to_string());
                let task_id = get_optional_string(args, "task_id");
                let result = commands::test_create(repo, name, command, working_dir, task_id)?;
                Ok(result.to_json())
            }
            "bn_test_list" => {
                let task_id = get_optional_string(args, "task_id");
                let result = commands::test_list(repo, task_id.as_deref())?;
                Ok(result.to_json())
            }
            "bn_test_show" => {
                let id = get_string_arg(args, "id")?;
                let result = commands::test_show(repo, &id)?;
                Ok(result.to_json())
            }
            "bn_test_link" => {
                let test_id = get_string_arg(args, "test_id")?;
                let task_id = get_string_arg(args, "task_id")?;
                let result = commands::test_link(repo, &test_id, &task_id)?;
                Ok(result.to_json())
            }
            "bn_test_unlink" => {
                let test_id = get_string_arg(args, "test_id")?;
                let task_id = get_string_arg(args, "task_id")?;
                let result = commands::test_unlink(repo, &test_id, &task_id)?;
                Ok(result.to_json())
            }
            "bn_test_run" => {
                let test_id = get_optional_string(args, "test_id");
                let task_id = get_optional_string(args, "task_id");
                let all = args.get("all").and_then(|v| v.as_bool()).unwrap_or(false);
                let failed = args
                    .get("failed")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let result =
                    commands::test_run(repo, test_id.as_deref(), task_id.as_deref(), all, failed)?;
                Ok(result.to_json())
            }
            "bn_commit_link" => {
                let sha = get_string_arg(args, "sha")?;
                let task_id = get_string_arg(args, "task_id")?;
                let result = commands::commit_link(repo, &sha, &task_id)?;
                Ok(result.to_json())
            }
            "bn_commit_unlink" => {
                let sha = get_string_arg(args, "sha")?;
                let task_id = get_string_arg(args, "task_id")?;
                let result = commands::commit_unlink(repo, &sha, &task_id)?;
                Ok(result.to_json())
            }
            "bn_commit_list" => {
                let task_id = get_string_arg(args, "task_id")?;
                let result = commands::commit_list(repo, &task_id)?;
                Ok(result.to_json())
            }
            "bn_doctor" => {
                let result = commands::doctor(repo)?;
                Ok(result.to_json())
            }
            "bn_log" => {
                let task_id = get_optional_string(args, "task_id");
                let result = commands::log(repo, task_id.as_deref())?;
                Ok(result.to_json())
            }
            "bn_compact" => {
                let result = commands::compact(repo)?;
                Ok(result.to_json())
            }
            "bn_config_get" => {
                let key = get_string_arg(args, "key")?;
                let result = commands::config_get(repo, &key)?;
                Ok(result.to_json())
            }
            "bn_config_set" => {
                let key = get_string_arg(args, "key")?;
                let value = get_string_arg(args, "value")?;
                let result = commands::config_set(repo, &key, &value)?;
                Ok(result.to_json())
            }
            "bn_config_list" => {
                let result = commands::config_list(repo)?;
                Ok(result.to_json())
            }
            // Milestone tools
            "bn_milestone_create" => {
                let title = get_string_arg(args, "title")?;
                let description = get_optional_string(args, "description");
                let priority = args
                    .get("priority")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u8);
                let tags = args
                    .get("tags")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                let assignee = get_optional_string(args, "assignee");
                let due_date = get_optional_string(args, "due_date");
                let result = commands::milestone_create(
                    repo,
                    title,
                    description,
                    priority,
                    tags,
                    assignee,
                    due_date,
                )?;
                Ok(result.to_json())
            }
            "bn_milestone_list" => {
                let status = get_optional_string(args, "status");
                let priority = args
                    .get("priority")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u8);
                let tag = get_optional_string(args, "tag");
                let result =
                    commands::milestone_list(repo, status.as_deref(), priority, tag.as_deref())?;
                Ok(result.to_json())
            }
            "bn_milestone_show" => {
                let id = get_string_arg(args, "id")?;
                let result = commands::milestone_show(repo, &id)?;
                Ok(result.to_json())
            }
            "bn_milestone_update" => {
                let id = get_string_arg(args, "id")?;
                let title = get_optional_string(args, "title");
                let description = get_optional_string(args, "description");
                let priority = args
                    .get("priority")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u8);
                let status = get_optional_string(args, "status");
                let add_tags = args
                    .get("add_tags")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                let remove_tags = args
                    .get("remove_tags")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                let assignee = get_optional_string(args, "assignee");
                let due_date = get_optional_string(args, "due_date");
                let result = commands::milestone_update(
                    repo,
                    &id,
                    title,
                    description,
                    priority,
                    status.as_deref(),
                    add_tags,
                    remove_tags,
                    assignee,
                    due_date,
                )?;
                Ok(result.to_json())
            }
            "bn_milestone_close" => {
                let id = get_string_arg(args, "id")?;
                let reason = get_optional_string(args, "reason");
                let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
                let result = commands::milestone_close(repo, &id, reason, force)?;
                Ok(result.to_json())
            }
            "bn_milestone_reopen" => {
                let id = get_string_arg(args, "id")?;
                let result = commands::milestone_reopen(repo, &id)?;
                Ok(result.to_json())
            }
            "bn_milestone_delete" => {
                let id = get_string_arg(args, "id")?;
                let result = commands::milestone_delete(repo, &id)?;
                Ok(result.to_json())
            }
            "bn_milestone_progress" => {
                let id = get_string_arg(args, "id")?;
                let result = commands::milestone_progress(repo, &id)?;
                Ok(result.to_json())
            }
            // Search tools
            "bn_search_link" => {
                let edge_type = get_optional_string(args, "edge_type");
                let source = get_optional_string(args, "source");
                let target = get_optional_string(args, "target");
                let result = commands::search_link(
                    repo,
                    edge_type.as_deref(),
                    source.as_deref(),
                    target.as_deref(),
                )?;
                Ok(result.to_json())
            }
            _ => Err(Error::Other(format!("Unknown tool: {}", name))),
        }
    }

    /// Read a resource by URI
    fn read_resource(&self, uri: &str) -> Result<String, Error> {
        let repo = &self.repo_path;

        match uri {
            "binnacle://tasks" => {
                let result = commands::task_list(repo, None, None, None)?;
                Ok(result.to_json())
            }
            "binnacle://ready" => {
                let result = commands::ready(repo)?;
                Ok(result.to_json())
            }
            "binnacle://blocked" => {
                let result = commands::blocked(repo)?;
                Ok(result.to_json())
            }
            "binnacle://status" => {
                let result = commands::status(repo)?;
                Ok(result.to_json())
            }
            _ => Err(Error::Other(format!("Unknown resource: {}", uri))),
        }
    }

    /// Get a prompt with arguments
    fn get_prompt(
        &self,
        name: &str,
        args: &HashMap<String, String>,
    ) -> Result<(String, Vec<Value>), Error> {
        match name {
            "start_work" => {
                let task_id = args.get("task_id").cloned().unwrap_or_default();
                let description =
                    "Begin working on a task - sets status to in_progress and provides context";
                let messages = vec![json!({
                    "role": "user",
                    "content": {
                        "type": "text",
                        "text": format!(
                            "I'm starting work on task {}. Please:\n\
                            1. Show me the task details with `bn task show {}`\n\
                            2. Update the task status to in_progress with `bn task update {} --status in_progress`\n\
                            3. Show any dependencies with `bn dep show {}`\n\
                            4. List any linked tests with `bn test list --task {}`\n\
                            Then provide a summary of what needs to be done.",
                            task_id, task_id, task_id, task_id, task_id
                        )
                    }
                })];
                Ok((description.to_string(), messages))
            }
            "finish_work" => {
                let task_id = args.get("task_id").cloned().unwrap_or_default();
                let description =
                    "Complete work on a task - runs tests, links commits, and closes the task";
                let messages = vec![json!({
                    "role": "user",
                    "content": {
                        "type": "text",
                        "text": format!(
                            "I'm finishing work on task {}. Please:\n\
                            1. Run any linked tests with `bn test run --task {}`\n\
                            2. If tests pass, close the task with `bn task close {}`\n\
                            3. Show the final task state with `bn task show {}`\n\
                            Provide a summary of the completed work.",
                            task_id, task_id, task_id, task_id
                        )
                    }
                })];
                Ok((description.to_string(), messages))
            }
            "triage_regression" => {
                let test_id = args.get("test_id").cloned().unwrap_or_default();
                let description = "Investigate a test failure and its linked tasks";
                let messages = vec![json!({
                    "role": "user",
                    "content": {
                        "type": "text",
                        "text": format!(
                            "Test {} has failed. Please:\n\
                            1. Show the test details with `bn test show {}`\n\
                            2. Check which tasks are linked to this test\n\
                            3. Review recent commits linked to those tasks\n\
                            4. Analyze the test failure and suggest fixes\n\
                            Provide a triage report with recommended next steps.",
                            test_id, test_id
                        )
                    }
                })];
                Ok((description.to_string(), messages))
            }
            "plan_feature" => {
                let feature = args.get("feature").cloned().unwrap_or_default();
                let description = "Break down a feature into tasks with dependencies";
                let messages = vec![json!({
                    "role": "user",
                    "content": {
                        "type": "text",
                        "text": format!(
                            "I want to implement: {}\n\n\
                            Please help me plan this feature by:\n\
                            1. Breaking it down into discrete tasks\n\
                            2. Creating tasks with `bn task create \"title\" -p <priority>`\n\
                            3. Setting up dependencies with `bn dep add <child> <parent>`\n\
                            4. Creating test nodes for key functionality\n\
                            Show me the final task graph when done.",
                            feature
                        )
                    }
                })];
                Ok((description.to_string(), messages))
            }
            "status_report" => {
                let description = "Generate a summary of current project state";
                let messages = vec![json!({
                    "role": "user",
                    "content": {
                        "type": "text",
                        "text": "Please generate a status report by:\n\
                            1. Running `bn` to get overall status\n\
                            2. Listing ready tasks with `bn ready`\n\
                            3. Listing blocked tasks with `bn blocked`\n\
                            4. Running `bn doctor` to check for issues\n\
                            Provide a human-readable summary of the project state."
                    }
                })];
                Ok((description.to_string(), messages))
            }
            _ => Err(Error::Other(format!("Unknown prompt: {}", name))),
        }
    }
}

// === Helper functions for argument extraction ===

fn get_string_arg(args: &Value, key: &str) -> Result<String, Error> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| Error::Other(format!("Missing required argument: {}", key)))
}

fn get_optional_string(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn get_optional_u8(args: &Value, key: &str) -> Option<u8> {
    args.get(key).and_then(|v| v.as_u64()).map(|n| n as u8)
}

fn get_optional_bool(args: &Value, key: &str) -> Option<bool> {
    args.get(key).and_then(|v| v.as_bool())
}

fn get_string_array(args: &Value, key: &str) -> Vec<String> {
    args.get(key)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

// === Tool Definitions ===

/// Get all available MCP tool definitions
pub fn get_tool_definitions() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "bn_init".to_string(),
            description: "Initialize binnacle for this repository".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDef {
            name: "bn_status".to_string(),
            description: "Get status summary of all tasks".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDef {
            name: "bn_task_create".to_string(),
            description: "Create a new task".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "Task title"
                    },
                    "short_name": {
                        "type": "string",
                        "description": "Short display name for GUI (recommended: 1-2 words, ~12 chars max)"
                    },
                    "description": {
                        "type": "string",
                        "description": "Task description"
                    },
                    "priority": {
                        "type": "integer",
                        "description": "Priority (0-4, lower is higher priority)",
                        "minimum": 0,
                        "maximum": 4
                    },
                    "tags": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Tags for categorization"
                    },
                    "assignee": {
                        "type": "string",
                        "description": "Assigned user or agent"
                    }
                },
                "required": ["title"]
            }),
        },
        ToolDef {
            name: "bn_task_list".to_string(),
            description: "List tasks with optional filters".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "status": {
                        "type": "string",
                        "description": "Filter by status (pending, in_progress, done, blocked, cancelled, reopened)"
                    },
                    "priority": {
                        "type": "integer",
                        "description": "Filter by priority"
                    },
                    "tag": {
                        "type": "string",
                        "description": "Filter by tag"
                    }
                },
                "required": []
            }),
        },
        ToolDef {
            name: "bn_task_show".to_string(),
            description: "Show details of a specific task".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Task ID (e.g., bn-a1b2)"
                    }
                },
                "required": ["id"]
            }),
        },
        ToolDef {
            name: "bn_task_update".to_string(),
            description: "Update a task's properties".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Task ID"
                    },
                    "title": {
                        "type": "string",
                        "description": "New title"
                    },
                    "short_name": {
                        "type": "string",
                        "description": "New short display name for GUI (recommended: 1-2 words, ~12 chars max)"
                    },
                    "description": {
                        "type": "string",
                        "description": "New description"
                    },
                    "priority": {
                        "type": "integer",
                        "description": "New priority (0-4)"
                    },
                    "status": {
                        "type": "string",
                        "description": "New status (pending, in_progress, partial, blocked, done)"
                    },
                    "add_tags": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Tags to add"
                    },
                    "remove_tags": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Tags to remove"
                    },
                    "assignee": {
                        "type": "string",
                        "description": "New assignee"
                    },
                    "force": {
                        "type": "boolean",
                        "description": "When setting status to done, bypass commit requirement"
                    }
                },
                "required": ["id"]
            }),
        },
        ToolDef {
            name: "bn_task_close".to_string(),
            description: "Close a task".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Task ID"
                    },
                    "reason": {
                        "type": "string",
                        "description": "Reason for closing"
                    }
                },
                "required": ["id"]
            }),
        },
        ToolDef {
            name: "bn_task_reopen".to_string(),
            description: "Reopen a closed task".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Task ID"
                    }
                },
                "required": ["id"]
            }),
        },
        ToolDef {
            name: "bn_task_delete".to_string(),
            description: "Delete a task".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Task ID"
                    }
                },
                "required": ["id"]
            }),
        },
        ToolDef {
            name: "bn_link_add".to_string(),
            description: "Create a link (edge) between two entities".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "source": {
                        "type": "string",
                        "description": "Source entity ID (e.g., bn-1234)"
                    },
                    "target": {
                        "type": "string",
                        "description": "Target entity ID (e.g., bn-5678)"
                    },
                    "edge_type": {
                        "type": "string",
                        "description": "Type of relationship",
                        "enum": ["depends_on", "blocks", "related_to", "duplicates", "fixes", "caused_by", "supersedes", "parent_of", "child_of", "tests"]
                    },
                    "reason": {
                        "type": "string",
                        "description": "Optional reason for creating this link"
                    }
                },
                "required": ["source", "target", "edge_type"]
            }),
        },
        ToolDef {
            name: "bn_link_rm".to_string(),
            description: "Remove a link between two entities".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "source": {
                        "type": "string",
                        "description": "Source entity ID"
                    },
                    "target": {
                        "type": "string",
                        "description": "Target entity ID"
                    },
                    "edge_type": {
                        "type": "string",
                        "description": "Type of relationship (required to identify which edge to remove)",
                        "enum": ["depends_on", "blocks", "related_to", "duplicates", "fixes", "caused_by", "supersedes", "parent_of", "child_of", "tests"]
                    }
                },
                "required": ["source", "target", "edge_type"]
            }),
        },
        ToolDef {
            name: "bn_link_list".to_string(),
            description: "List links for an entity or all links".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "entity_id": {
                        "type": "string",
                        "description": "Entity ID to list links for (omit for --all)"
                    },
                    "all": {
                        "type": "boolean",
                        "description": "List all links in the system"
                    },
                    "edge_type": {
                        "type": "string",
                        "description": "Filter by edge type",
                        "enum": ["depends_on", "blocks", "related_to", "duplicates", "fixes", "caused_by", "supersedes", "parent_of", "child_of", "tests"]
                    }
                },
                "required": []
            }),
        },
        ToolDef {
            name: "bn_ready".to_string(),
            description: "List tasks with no open blockers (ready to work on)".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDef {
            name: "bn_blocked".to_string(),
            description: "List tasks waiting on dependencies".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDef {
            name: "bn_test_create".to_string(),
            description: "Create a new test node".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Test name"
                    },
                    "command": {
                        "type": "string",
                        "description": "Command to run"
                    },
                    "working_dir": {
                        "type": "string",
                        "description": "Working directory (default: '.')"
                    },
                    "task_id": {
                        "type": "string",
                        "description": "Task ID to link to"
                    }
                },
                "required": ["name", "command"]
            }),
        },
        ToolDef {
            name: "bn_test_list".to_string(),
            description: "List test nodes".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "task_id": {
                        "type": "string",
                        "description": "Filter by linked task"
                    }
                },
                "required": []
            }),
        },
        ToolDef {
            name: "bn_test_show".to_string(),
            description: "Show test node details".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Test ID (e.g., bnt-0001)"
                    }
                },
                "required": ["id"]
            }),
        },
        ToolDef {
            name: "bn_test_link".to_string(),
            description: "Link a test to a task".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "test_id": {
                        "type": "string",
                        "description": "Test ID"
                    },
                    "task_id": {
                        "type": "string",
                        "description": "Task ID"
                    }
                },
                "required": ["test_id", "task_id"]
            }),
        },
        ToolDef {
            name: "bn_test_unlink".to_string(),
            description: "Unlink a test from a task".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "test_id": {
                        "type": "string",
                        "description": "Test ID"
                    },
                    "task_id": {
                        "type": "string",
                        "description": "Task ID"
                    }
                },
                "required": ["test_id", "task_id"]
            }),
        },
        ToolDef {
            name: "bn_test_run".to_string(),
            description: "Run tests and detect regressions".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "test_id": {
                        "type": "string",
                        "description": "Specific test ID to run"
                    },
                    "task_id": {
                        "type": "string",
                        "description": "Run tests linked to this task"
                    },
                    "all": {
                        "type": "boolean",
                        "description": "Run all tests"
                    },
                    "failed": {
                        "type": "boolean",
                        "description": "Run only previously failed tests"
                    }
                },
                "required": []
            }),
        },
        ToolDef {
            name: "bn_commit_link".to_string(),
            description: "Link a commit to a task".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "sha": {
                        "type": "string",
                        "description": "Commit SHA"
                    },
                    "task_id": {
                        "type": "string",
                        "description": "Task ID"
                    }
                },
                "required": ["sha", "task_id"]
            }),
        },
        ToolDef {
            name: "bn_commit_unlink".to_string(),
            description: "Unlink a commit from a task".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "sha": {
                        "type": "string",
                        "description": "Commit SHA"
                    },
                    "task_id": {
                        "type": "string",
                        "description": "Task ID"
                    }
                },
                "required": ["sha", "task_id"]
            }),
        },
        ToolDef {
            name: "bn_commit_list".to_string(),
            description: "List commits linked to a task".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "task_id": {
                        "type": "string",
                        "description": "Task ID"
                    }
                },
                "required": ["task_id"]
            }),
        },
        ToolDef {
            name: "bn_doctor".to_string(),
            description: "Run health checks and detect issues".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDef {
            name: "bn_log".to_string(),
            description: "Show audit trail of changes".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "task_id": {
                        "type": "string",
                        "description": "Filter by task ID"
                    }
                },
                "required": []
            }),
        },
        ToolDef {
            name: "bn_compact".to_string(),
            description: "Compact storage by summarizing old closed tasks".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDef {
            name: "bn_config_get".to_string(),
            description: "Get a configuration value".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "key": {
                        "type": "string",
                        "description": "Configuration key"
                    }
                },
                "required": ["key"]
            }),
        },
        ToolDef {
            name: "bn_config_set".to_string(),
            description: "Set a configuration value".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "key": {
                        "type": "string",
                        "description": "Configuration key"
                    },
                    "value": {
                        "type": "string",
                        "description": "Configuration value"
                    }
                },
                "required": ["key", "value"]
            }),
        },
        ToolDef {
            name: "bn_config_list".to_string(),
            description: "List all configuration values".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        // Milestone tools
        ToolDef {
            name: "bn_milestone_create".to_string(),
            description: "Create a new milestone to group related tasks".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "Milestone title"
                    },
                    "description": {
                        "type": "string",
                        "description": "Detailed description"
                    },
                    "priority": {
                        "type": "integer",
                        "description": "Priority level (0=critical, 1=high, 2=medium, 3=low, 4=nice-to-have)",
                        "minimum": 0,
                        "maximum": 4
                    },
                    "tags": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Tags to categorize the milestone"
                    },
                    "assignee": {
                        "type": "string",
                        "description": "User or agent assigned to this milestone"
                    },
                    "due_date": {
                        "type": "string",
                        "description": "Due date in ISO 8601 format (e.g., 2026-01-31T00:00:00Z)"
                    }
                },
                "required": ["title"]
            }),
        },
        ToolDef {
            name: "bn_milestone_list".to_string(),
            description: "List milestones with optional filtering".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "status": {
                        "type": "string",
                        "description": "Filter by status",
                        "enum": ["pending", "in_progress", "done", "blocked", "cancelled", "reopened", "partial"]
                    },
                    "priority": {
                        "type": "integer",
                        "description": "Filter by priority (0-4)",
                        "minimum": 0,
                        "maximum": 4
                    },
                    "tag": {
                        "type": "string",
                        "description": "Filter by tag"
                    }
                },
                "required": []
            }),
        },
        ToolDef {
            name: "bn_milestone_show".to_string(),
            description: "Show milestone details including progress and linked tasks".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Milestone ID (e.g., bn-1234)"
                    }
                },
                "required": ["id"]
            }),
        },
        ToolDef {
            name: "bn_milestone_update".to_string(),
            description: "Update milestone properties".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Milestone ID"
                    },
                    "title": {
                        "type": "string",
                        "description": "New title"
                    },
                    "description": {
                        "type": "string",
                        "description": "New description"
                    },
                    "priority": {
                        "type": "integer",
                        "description": "New priority (0-4)",
                        "minimum": 0,
                        "maximum": 4
                    },
                    "status": {
                        "type": "string",
                        "description": "New status",
                        "enum": ["pending", "in_progress", "blocked", "partial"]
                    },
                    "add_tags": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Tags to add"
                    },
                    "remove_tags": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Tags to remove"
                    },
                    "assignee": {
                        "type": "string",
                        "description": "New assignee"
                    },
                    "due_date": {
                        "type": "string",
                        "description": "New due date in ISO 8601 format"
                    }
                },
                "required": ["id"]
            }),
        },
        ToolDef {
            name: "bn_milestone_close".to_string(),
            description: "Close a milestone (marks as done)".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Milestone ID"
                    },
                    "reason": {
                        "type": "string",
                        "description": "Reason for closing"
                    },
                    "force": {
                        "type": "boolean",
                        "description": "Force close even if tasks are incomplete"
                    }
                },
                "required": ["id"]
            }),
        },
        ToolDef {
            name: "bn_milestone_reopen".to_string(),
            description: "Reopen a closed milestone".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Milestone ID"
                    }
                },
                "required": ["id"]
            }),
        },
        ToolDef {
            name: "bn_milestone_delete".to_string(),
            description: "Delete a milestone".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Milestone ID"
                    }
                },
                "required": ["id"]
            }),
        },
        ToolDef {
            name: "bn_milestone_progress".to_string(),
            description: "Get progress for a milestone (completed/total tasks)".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Milestone ID"
                    }
                },
                "required": ["id"]
            }),
        },
        // Search tools
        ToolDef {
            name: "bn_search_link".to_string(),
            description: "Search for links/edges by type, source, or target".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "edge_type": {
                        "type": "string",
                        "description": "Filter by edge type",
                        "enum": ["depends_on", "blocks", "related_to", "duplicates", "fixes", "caused_by", "supersedes", "parent_of", "child_of", "tests"]
                    },
                    "source": {
                        "type": "string",
                        "description": "Filter by source entity ID"
                    },
                    "target": {
                        "type": "string",
                        "description": "Filter by target entity ID"
                    }
                },
                "required": []
            }),
        },
    ]
}

/// Get all resource definitions
pub fn get_resource_definitions() -> Vec<ResourceDef> {
    vec![
        ResourceDef {
            uri: "binnacle://tasks".to_string(),
            name: "All Tasks".to_string(),
            description: "List of all tasks in the project".to_string(),
            mime_type: Some("application/json".to_string()),
        },
        ResourceDef {
            uri: "binnacle://ready".to_string(),
            name: "Ready Tasks".to_string(),
            description: "Tasks that are ready to work on (no open blockers)".to_string(),
            mime_type: Some("application/json".to_string()),
        },
        ResourceDef {
            uri: "binnacle://blocked".to_string(),
            name: "Blocked Tasks".to_string(),
            description: "Tasks waiting on dependencies".to_string(),
            mime_type: Some("application/json".to_string()),
        },
        ResourceDef {
            uri: "binnacle://status".to_string(),
            name: "Project Status".to_string(),
            description: "Overall project status summary".to_string(),
            mime_type: Some("application/json".to_string()),
        },
    ]
}

/// Get all prompt definitions
pub fn get_prompt_definitions() -> Vec<PromptDef> {
    vec![
        PromptDef {
            name: "start_work".to_string(),
            description: "Begin working on a task".to_string(),
            arguments: Some(vec![PromptArgument {
                name: "task_id".to_string(),
                description: "Task ID to start working on".to_string(),
                required: true,
            }]),
        },
        PromptDef {
            name: "finish_work".to_string(),
            description: "Complete current task properly".to_string(),
            arguments: Some(vec![PromptArgument {
                name: "task_id".to_string(),
                description: "Task ID to finish".to_string(),
                required: true,
            }]),
        },
        PromptDef {
            name: "triage_regression".to_string(),
            description: "Investigate a test failure".to_string(),
            arguments: Some(vec![PromptArgument {
                name: "test_id".to_string(),
                description: "Test ID that failed".to_string(),
                required: true,
            }]),
        },
        PromptDef {
            name: "plan_feature".to_string(),
            description: "Break down a feature into tasks".to_string(),
            arguments: Some(vec![PromptArgument {
                name: "feature".to_string(),
                description: "Feature description to plan".to_string(),
                required: true,
            }]),
        },
        PromptDef {
            name: "status_report".to_string(),
            description: "Generate summary of current state".to_string(),
            arguments: None,
        },
    ]
}

/// Start the MCP stdio server.
pub fn serve(repo_path: &Path) {
    let mut server = McpServer::new(repo_path);
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.trim().is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let error_response =
                    JsonRpcResponse::error(None, -32700, format!("Parse error: {}", e));
                let _ = writeln!(
                    stdout,
                    "{}",
                    serde_json::to_string(&error_response).unwrap()
                );
                let _ = stdout.flush();
                continue;
            }
        };

        // Handle notifications (no id) - don't send response
        if request.id.is_none() && request.method == "notifications/initialized" {
            server.initialized = true;
            continue;
        }

        let response = server.handle_request(&request);

        // Only send response if there was an id (not a notification)
        if request.id.is_some() || response.error.is_some() {
            let _ = writeln!(stdout, "{}", serde_json::to_string(&response).unwrap());
            let _ = stdout.flush();
        }
    }
}

/// Output the MCP tool manifest.
pub fn manifest() {
    let tools = get_tool_definitions();
    let resources = get_resource_definitions();
    let prompts = get_prompt_definitions();

    let manifest = json!({
        "tools": tools,
        "resources": resources,
        "prompts": prompts
    });

    println!("{}", serde_json::to_string_pretty(&manifest).unwrap());
}

// Legacy compatibility - export tools module
pub mod tools {
    /// Tool definition for MCP manifest (legacy).
    pub struct ToolDef {
        pub name: &'static str,
        pub description: &'static str,
    }

    /// Get all available MCP tools (legacy).
    pub fn get_tools() -> Vec<ToolDef> {
        vec![
            ToolDef {
                name: "bn_task_create",
                description: "Create a new task",
            },
            ToolDef {
                name: "bn_task_list",
                description: "List tasks with optional filters",
            },
            ToolDef {
                name: "bn_task_show",
                description: "Show details of a specific task",
            },
            ToolDef {
                name: "bn_task_update",
                description: "Update a task's properties",
            },
            ToolDef {
                name: "bn_task_close",
                description: "Close a task",
            },
            ToolDef {
                name: "bn_ready",
                description: "List tasks with no open blockers",
            },
            ToolDef {
                name: "bn_blocked",
                description: "List tasks waiting on dependencies",
            },
            ToolDef {
                name: "bn_test_run",
                description: "Run tests and detect regressions",
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Storage;
    use tempfile::TempDir;

    fn setup() -> TempDir {
        let temp = TempDir::new().unwrap();
        Storage::init(temp.path()).unwrap();
        temp
    }

    #[test]
    fn test_tool_definitions_valid() {
        let tools = get_tool_definitions();
        assert!(!tools.is_empty());

        for tool in &tools {
            assert!(!tool.name.is_empty());
            assert!(!tool.description.is_empty());
            assert!(tool.input_schema.is_object());
        }
    }

    #[test]
    fn test_resource_definitions_valid() {
        let resources = get_resource_definitions();
        assert!(!resources.is_empty());

        for resource in &resources {
            assert!(resource.uri.starts_with("binnacle://"));
            assert!(!resource.name.is_empty());
        }
    }

    #[test]
    fn test_prompt_definitions_valid() {
        let prompts = get_prompt_definitions();
        assert!(!prompts.is_empty());

        for prompt in &prompts {
            assert!(!prompt.name.is_empty());
            assert!(!prompt.description.is_empty());
        }
    }

    #[test]
    fn test_server_initialize() {
        let temp = setup();
        let mut server = McpServer::new(temp.path());

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "initialize".to_string(),
            params: Some(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "test-client",
                    "version": "1.0.0"
                }
            })),
        };

        let response = server.handle_request(&request);
        assert!(response.error.is_none());
        assert!(response.result.is_some());

        let result = response.result.unwrap();
        assert_eq!(result["protocolVersion"], MCP_PROTOCOL_VERSION);
        assert!(result["capabilities"]["tools"].is_object());
    }

    #[test]
    fn test_server_tools_list() {
        let temp = setup();
        let mut server = McpServer::new(temp.path());

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "tools/list".to_string(),
            params: None,
        };

        let response = server.handle_request(&request);
        assert!(response.error.is_none());

        let result = response.result.unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert!(!tools.is_empty());
    }

    #[test]
    fn test_server_tool_call_task_create() {
        let temp = setup();
        let mut server = McpServer::new(temp.path());

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "bn_task_create",
                "arguments": {
                    "title": "Test task"
                }
            })),
        };

        let response = server.handle_request(&request);
        assert!(response.error.is_none());

        let result = response.result.unwrap();
        let content = &result["content"][0]["text"];
        assert!(content.as_str().unwrap().contains("bn-"));
    }

    #[test]
    fn test_server_tool_call_task_list() {
        let temp = setup();
        let mut server = McpServer::new(temp.path());

        // Create a task first
        server
            .execute_tool("bn_task_create", &json!({"title": "Test task"}))
            .unwrap();

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "bn_task_list",
                "arguments": {}
            })),
        };

        let response = server.handle_request(&request);
        assert!(response.error.is_none());

        let result = response.result.unwrap();
        let content = result["content"][0]["text"].as_str().unwrap();
        assert!(content.contains("Test task"));
    }

    #[test]
    fn test_server_resources_list() {
        let temp = setup();
        let mut server = McpServer::new(temp.path());

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "resources/list".to_string(),
            params: None,
        };

        let response = server.handle_request(&request);
        assert!(response.error.is_none());

        let result = response.result.unwrap();
        let resources = result["resources"].as_array().unwrap();
        assert!(!resources.is_empty());
    }

    #[test]
    fn test_server_resource_read_tasks() {
        let temp = setup();
        let mut server = McpServer::new(temp.path());

        // Create a task first
        server
            .execute_tool("bn_task_create", &json!({"title": "Resource test"}))
            .unwrap();

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "resources/read".to_string(),
            params: Some(json!({
                "uri": "binnacle://tasks"
            })),
        };

        let response = server.handle_request(&request);
        assert!(response.error.is_none());

        let result = response.result.unwrap();
        let content = result["contents"][0]["text"].as_str().unwrap();
        assert!(content.contains("Resource test"));
    }

    #[test]
    fn test_server_prompts_list() {
        let temp = setup();
        let mut server = McpServer::new(temp.path());

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "prompts/list".to_string(),
            params: None,
        };

        let response = server.handle_request(&request);
        assert!(response.error.is_none());

        let result = response.result.unwrap();
        let prompts = result["prompts"].as_array().unwrap();
        assert!(!prompts.is_empty());
    }

    #[test]
    fn test_server_prompt_get_start_work() {
        let temp = setup();
        let mut server = McpServer::new(temp.path());

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "prompts/get".to_string(),
            params: Some(json!({
                "name": "start_work",
                "arguments": {
                    "task_id": "bn-1234"
                }
            })),
        };

        let response = server.handle_request(&request);
        assert!(response.error.is_none());

        let result = response.result.unwrap();
        assert!(result["description"].as_str().is_some());
        assert!(result["messages"].as_array().is_some());
    }

    #[test]
    fn test_server_unknown_method() {
        let temp = setup();
        let mut server = McpServer::new(temp.path());

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

    #[test]
    fn test_server_unknown_tool() {
        let temp = setup();
        let mut server = McpServer::new(temp.path());

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "unknown_tool",
                "arguments": {}
            })),
        };

        let response = server.handle_request(&request);
        // Tool errors are returned as content with isError flag
        let result = response.result.unwrap();
        assert!(result["isError"].as_bool().unwrap_or(false));
    }

    #[test]
    fn test_execute_tool_dep_operations() {
        let temp = setup();
        let server = McpServer::new(temp.path());

        // Create two tasks
        let result1 = server
            .execute_tool("bn_task_create", &json!({"title": "Parent task"}))
            .unwrap();
        let task1: serde_json::Value = serde_json::from_str(&result1).unwrap();
        let parent_id = task1["id"].as_str().unwrap();

        let result2 = server
            .execute_tool("bn_task_create", &json!({"title": "Child task"}))
            .unwrap();
        let task2: serde_json::Value = serde_json::from_str(&result2).unwrap();
        let child_id = task2["id"].as_str().unwrap();

        // Add link (dependency)
        let link_result = server
            .execute_tool(
                "bn_link_add",
                &json!({
                    "source": child_id,
                    "target": parent_id,
                    "edge_type": "depends_on"
                }),
            )
            .unwrap();
        assert!(link_result.contains(child_id));

        // Check blocked
        let blocked = server.execute_tool("bn_blocked", &json!({})).unwrap();
        assert!(blocked.contains(child_id));

        // Check ready
        let ready = server.execute_tool("bn_ready", &json!({})).unwrap();
        assert!(ready.contains(parent_id));
    }

    #[test]
    fn test_execute_tool_test_operations() {
        let temp = setup();
        let server = McpServer::new(temp.path());

        // Create test
        let result = server
            .execute_tool(
                "bn_test_create",
                &json!({
                    "name": "Unit tests",
                    "command": "echo hello"
                }),
            )
            .unwrap();
        assert!(result.contains("bnt-"));

        // List tests
        let list = server.execute_tool("bn_test_list", &json!({})).unwrap();
        assert!(list.contains("Unit tests"));
    }

    #[test]
    fn test_execute_tool_config_operations() {
        let temp = setup();
        let server = McpServer::new(temp.path());

        // Set config
        let set_result = server
            .execute_tool(
                "bn_config_set",
                &json!({
                    "key": "test.key",
                    "value": "test.value"
                }),
            )
            .unwrap();
        assert!(set_result.contains("test.key"));

        // Get config
        let get_result = server
            .execute_tool(
                "bn_config_get",
                &json!({
                    "key": "test.key"
                }),
            )
            .unwrap();
        assert!(get_result.contains("test.value"));

        // List configs
        let list_result = server.execute_tool("bn_config_list", &json!({})).unwrap();
        assert!(list_result.contains("test.key"));
    }

    #[test]
    fn test_read_resource_ready() {
        let temp = setup();
        let server = McpServer::new(temp.path());

        // Create a task
        server
            .execute_tool("bn_task_create", &json!({"title": "Ready task"}))
            .unwrap();

        let result = server.read_resource("binnacle://ready").unwrap();
        assert!(result.contains("Ready task"));
    }

    #[test]
    fn test_read_resource_status() {
        let temp = setup();
        let server = McpServer::new(temp.path());

        let result = server.read_resource("binnacle://status").unwrap();
        assert!(result.contains("tasks"));
    }

    #[test]
    fn test_get_prompt_status_report() {
        let temp = setup();
        let server = McpServer::new(temp.path());

        let (description, messages) = server.get_prompt("status_report", &HashMap::new()).unwrap();
        assert!(!description.is_empty());
        assert!(!messages.is_empty());
    }

    #[test]
    fn test_manifest_output() {
        // Just verify it doesn't panic
        let tools = get_tool_definitions();
        let resources = get_resource_definitions();
        let prompts = get_prompt_definitions();

        let manifest = json!({
            "tools": tools,
            "resources": resources,
            "prompts": prompts
        });

        let output = serde_json::to_string_pretty(&manifest).unwrap();
        assert!(output.contains("bn_task_create"));
        assert!(output.contains("binnacle://tasks"));
        assert!(output.contains("start_work"));
    }
}
