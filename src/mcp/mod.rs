//! MCP (Model Context Protocol) server implementation.
//!
//! This module provides:
//! - `bn mcp serve` - Start stdio MCP server
//! - `bn mcp manifest` - Output tool definitions
//!
//! All CLI operations are exposed as MCP tools for AI agent integration.

/// MCP tool definitions for binnacle operations.
pub mod tools {
    /// Tool definition for MCP manifest.
    pub struct ToolDef {
        pub name: &'static str,
        pub description: &'static str,
    }

    /// Get all available MCP tools.
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

/// Start the MCP stdio server.
pub fn serve() {
    // TODO: Implement in Phase 6
    // 1. Read JSON-RPC requests from stdin
    // 2. Dispatch to appropriate handler
    // 3. Write JSON-RPC responses to stdout
    eprintln!("MCP server not yet implemented");
}

/// Output the MCP tool manifest.
pub fn manifest() {
    // TODO: Implement in Phase 6
    let tools = tools::get_tools();
    println!("{{");
    println!("  \"tools\": [");
    for (i, tool) in tools.iter().enumerate() {
        let comma = if i < tools.len() - 1 { "," } else { "" };
        println!(
            "    {{\"name\": \"{}\", \"description\": \"{}\"}}{}",
            tool.name, tool.description, comma
        );
    }
    println!("  ]");
    println!("}}");
}
