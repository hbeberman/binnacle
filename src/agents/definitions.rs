//! Agent definition types for unified agent configuration.
//!
//! This module defines the core types for agent definitions:
//! - `AgentDefinition`: Full agent configuration with tools and prompt
//! - `ToolPermissions`: Allowed and denied tool patterns
//! - `ExecutionMode`: Where the agent runs (Host or Container)
//! - `LifecycleMode`: How the agent manages its session (Stateful or Stateless)

use serde::{Deserialize, Serialize};

/// Execution mode for an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionMode {
    /// Agent runs directly on the host machine.
    Host,
    /// Agent runs in a container.
    Container,
}

impl std::fmt::Display for ExecutionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionMode::Host => write!(f, "host"),
            ExecutionMode::Container => write!(f, "container"),
        }
    }
}

impl std::str::FromStr for ExecutionMode {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "host" => Ok(ExecutionMode::Host),
            "container" => Ok(ExecutionMode::Container),
            _ => Err(crate::Error::InvalidInput(format!(
                "Invalid execution mode: '{}'. Expected 'host' or 'container'.",
                s
            ))),
        }
    }
}

/// Lifecycle mode for an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LifecycleMode {
    /// Agent manages binnacle session state and calls bn goodbye on completion.
    Stateful,
    /// Agent produces artifacts/answers but does not manage session state.
    Stateless,
}

impl std::fmt::Display for LifecycleMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LifecycleMode::Stateful => write!(f, "stateful"),
            LifecycleMode::Stateless => write!(f, "stateless"),
        }
    }
}

impl std::str::FromStr for LifecycleMode {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "stateful" => Ok(LifecycleMode::Stateful),
            "stateless" => Ok(LifecycleMode::Stateless),
            _ => Err(crate::Error::InvalidInput(format!(
                "Invalid lifecycle mode: '{}'. Expected 'stateful' or 'stateless'.",
                s
            ))),
        }
    }
}

/// Tool permissions for an agent.
///
/// Tool patterns support wildcards:
/// - `*` matches any characters within a single segment
/// - Exact matches like `write`, `shell(bn:*)`, `binnacle`
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolPermissions {
    /// Tools that the agent is allowed to use.
    pub allow: Vec<String>,
    /// Tools that the agent is explicitly denied from using.
    pub deny: Vec<String>,
}

impl ToolPermissions {
    /// Create empty tool permissions.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create tool permissions with allowed tools.
    pub fn with_allow(allow: Vec<String>) -> Self {
        Self {
            allow,
            deny: Vec::new(),
        }
    }

    /// Add an allowed tool pattern.
    pub fn allow(mut self, pattern: impl Into<String>) -> Self {
        self.allow.push(pattern.into());
        self
    }

    /// Add a denied tool pattern.
    pub fn deny(mut self, pattern: impl Into<String>) -> Self {
        self.deny.push(pattern.into());
        self
    }

    /// Merge another set of permissions into this one.
    ///
    /// The `other` permissions are appended (allow and deny lists are extended).
    /// This is used for layered resolution where later sources add to earlier ones.
    pub fn merge(&mut self, other: &ToolPermissions) {
        self.allow.extend(other.allow.iter().cloned());
        self.deny.extend(other.deny.iter().cloned());
    }
}

/// Agent definition with all configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentDefinition {
    /// Agent type name (e.g., "worker", "prd", "buddy").
    pub name: String,
    /// Human-readable description of the agent's purpose.
    pub description: String,
    /// Where the agent executes (host or container).
    pub execution: ExecutionMode,
    /// How the agent manages its session lifecycle.
    pub lifecycle: LifecycleMode,
    /// Tool permissions (allowed and denied patterns).
    pub tools: ToolPermissions,
    /// The agent's prompt content.
    pub prompt: String,
}

impl AgentDefinition {
    /// Create a new agent definition.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        execution: ExecutionMode,
        lifecycle: LifecycleMode,
        prompt: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            execution,
            lifecycle,
            tools: ToolPermissions::default(),
            prompt: prompt.into(),
        }
    }

    /// Set tool permissions.
    pub fn with_tools(mut self, tools: ToolPermissions) -> Self {
        self.tools = tools;
        self
    }

    /// Check if this agent is stateful (manages session state).
    pub fn is_stateful(&self) -> bool {
        self.lifecycle == LifecycleMode::Stateful
    }

    /// Check if this agent runs in a container.
    pub fn is_container(&self) -> bool {
        self.execution == ExecutionMode::Container
    }

    /// Get a short summary for display.
    pub fn summary(&self) -> String {
        format!(
            "{}: [{}, {}] {}",
            self.name, self.execution, self.lifecycle, self.description
        )
    }

    /// Generate Copilot agent file content (.agent.md format).
    ///
    /// Generates markdown with YAML frontmatter suitable for GitHub Copilot agent files.
    /// Format:
    /// ```markdown
    /// ---
    /// name: Binnacle Worker
    /// description: AI worker that picks tasks from bn ready
    /// tools: ['binnacle/*', 'edit', 'execute', 'agent', 'search', 'read']
    /// ---
    /// [Full prompt content]
    /// ```
    pub fn generate_agent_file_content(&self) -> String {
        // Convert internal tool names to Copilot agent file format
        // The tools field uses a simplified syntax for the agent file
        let copilot_tools = self.copilot_agent_tools();
        let tools_yaml = if copilot_tools.is_empty() {
            "[]".to_string()
        } else {
            format!(
                "[{}]",
                copilot_tools
                    .iter()
                    .map(|t| format!("'{}'", t))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };

        // Generate human-friendly name: "Binnacle Worker", "Binnacle PRD", etc.
        let display_name = format!("Binnacle {}", self.display_name());

        // Include trailing newline for POSIX compliance
        format!(
            "---\nname: {}\ndescription: {}\ntools: {}\n---\n{}\n",
            display_name, self.description, tools_yaml, self.prompt
        )
    }

    /// Get human-friendly display name for agent.
    fn display_name(&self) -> String {
        match self.name.as_str() {
            "worker" => "Worker".to_string(),
            "do" => "Do".to_string(),
            "prd" => "PRD".to_string(),
            "buddy" => "Buddy".to_string(),
            "ask" => "Ask".to_string(),
            "free" => "Free".to_string(),
            other => {
                // Capitalize first letter
                let mut chars = other.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().chain(chars).collect(),
                }
            }
        }
    }

    /// Convert internal tool permissions to Copilot agent file tools format.
    ///
    /// Maps internal tool patterns like "shell(bn:*)" to Copilot's format.
    /// Returns a simplified tool list suitable for the YAML frontmatter.
    fn copilot_agent_tools(&self) -> Vec<String> {
        // Map common internal tool patterns to Copilot agent file patterns
        // The agent file format uses simpler patterns than the CLI
        let mut tools = Vec::new();

        for tool in &self.tools.allow {
            let mapped = match tool.as_str() {
                "binnacle" => "binnacle/*",
                "write" => "edit",
                "edit" => "edit",
                "create" => "edit",
                "view" => "read",
                "grep" => "search",
                "glob" => "search",
                "lsp" => "search",
                t if t.starts_with("shell(") => "execute",
                _ => tool.as_str(),
            };
            if !tools.contains(&mapped.to_string()) {
                tools.push(mapped.to_string());
            }
        }

        // Always include some standard tools if we have any permissions
        if !self.tools.allow.is_empty() {
            for standard in &["agent", "search", "read"] {
                if !tools.contains(&standard.to_string()) {
                    tools.push(standard.to_string());
                }
            }
        }

        tools
    }
}

/// Agent type names.
pub const AGENT_WORKER: &str = "worker";
pub const AGENT_DO: &str = "do";
pub const AGENT_PRD: &str = "prd";
pub const AGENT_BUDDY: &str = "buddy";
pub const AGENT_ASK: &str = "ask";
pub const AGENT_FREE: &str = "free";

/// All known agent type names.
pub const AGENT_TYPES: &[&str] = &[
    AGENT_WORKER,
    AGENT_DO,
    AGENT_PRD,
    AGENT_BUDDY,
    AGENT_ASK,
    AGENT_FREE,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_mode_display() {
        assert_eq!(ExecutionMode::Host.to_string(), "host");
        assert_eq!(ExecutionMode::Container.to_string(), "container");
    }

    #[test]
    fn test_execution_mode_from_str() {
        assert_eq!(
            "host".parse::<ExecutionMode>().unwrap(),
            ExecutionMode::Host
        );
        assert_eq!(
            "container".parse::<ExecutionMode>().unwrap(),
            ExecutionMode::Container
        );
        assert_eq!(
            "HOST".parse::<ExecutionMode>().unwrap(),
            ExecutionMode::Host
        );
        assert!("invalid".parse::<ExecutionMode>().is_err());
    }

    #[test]
    fn test_lifecycle_mode_display() {
        assert_eq!(LifecycleMode::Stateful.to_string(), "stateful");
        assert_eq!(LifecycleMode::Stateless.to_string(), "stateless");
    }

    #[test]
    fn test_lifecycle_mode_from_str() {
        assert_eq!(
            "stateful".parse::<LifecycleMode>().unwrap(),
            LifecycleMode::Stateful
        );
        assert_eq!(
            "stateless".parse::<LifecycleMode>().unwrap(),
            LifecycleMode::Stateless
        );
        assert_eq!(
            "STATEFUL".parse::<LifecycleMode>().unwrap(),
            LifecycleMode::Stateful
        );
        assert!("invalid".parse::<LifecycleMode>().is_err());
    }

    #[test]
    fn test_tool_permissions_builder() {
        let perms = ToolPermissions::new()
            .allow("write")
            .allow("shell(bn:*)")
            .deny("shell(rm:*)");

        assert_eq!(perms.allow, vec!["write", "shell(bn:*)"]);
        assert_eq!(perms.deny, vec!["shell(rm:*)"]);
    }

    #[test]
    fn test_tool_permissions_merge() {
        let mut base = ToolPermissions::new().allow("write");
        let overlay = ToolPermissions::new().allow("read").deny("execute");

        base.merge(&overlay);

        assert_eq!(base.allow, vec!["write", "read"]);
        assert_eq!(base.deny, vec!["execute"]);
    }

    #[test]
    fn test_agent_definition_new() {
        let agent = AgentDefinition::new(
            "worker",
            "Test worker agent",
            ExecutionMode::Container,
            LifecycleMode::Stateful,
            "Test prompt",
        );

        assert_eq!(agent.name, "worker");
        assert_eq!(agent.description, "Test worker agent");
        assert_eq!(agent.execution, ExecutionMode::Container);
        assert_eq!(agent.lifecycle, LifecycleMode::Stateful);
        assert_eq!(agent.prompt, "Test prompt");
        assert!(agent.is_stateful());
        assert!(agent.is_container());
    }

    #[test]
    fn test_agent_definition_with_tools() {
        let tools = ToolPermissions::new().allow("write").deny("execute");
        let agent = AgentDefinition::new(
            "test",
            "Test agent",
            ExecutionMode::Host,
            LifecycleMode::Stateless,
            "Prompt",
        )
        .with_tools(tools);

        assert_eq!(agent.tools.allow, vec!["write"]);
        assert_eq!(agent.tools.deny, vec!["execute"]);
        assert!(!agent.is_stateful());
        assert!(!agent.is_container());
    }

    #[test]
    fn test_agent_definition_summary() {
        let agent = AgentDefinition::new(
            "worker",
            "AI worker that picks tasks",
            ExecutionMode::Container,
            LifecycleMode::Stateful,
            "Prompt",
        );

        let summary = agent.summary();
        assert!(summary.contains("worker"));
        assert!(summary.contains("container"));
        assert!(summary.contains("stateful"));
        assert!(summary.contains("AI worker that picks tasks"));
    }

    #[test]
    fn test_agent_types_contains_all() {
        assert!(AGENT_TYPES.contains(&AGENT_WORKER));
        assert!(AGENT_TYPES.contains(&AGENT_DO));
        assert!(AGENT_TYPES.contains(&AGENT_PRD));
        assert!(AGENT_TYPES.contains(&AGENT_BUDDY));
        assert!(AGENT_TYPES.contains(&AGENT_ASK));
        assert!(AGENT_TYPES.contains(&AGENT_FREE));
        assert_eq!(AGENT_TYPES.len(), 6);
    }

    #[test]
    fn test_agent_definition_serialization() {
        let agent = AgentDefinition::new(
            "worker",
            "Test worker",
            ExecutionMode::Container,
            LifecycleMode::Stateful,
            "Test prompt",
        )
        .with_tools(ToolPermissions::new().allow("write"));

        let json = serde_json::to_string(&agent).unwrap();
        let parsed: AgentDefinition = serde_json::from_str(&json).unwrap();

        assert_eq!(agent, parsed);
    }

    #[test]
    fn test_generate_agent_file_content() {
        let agent = AgentDefinition::new(
            "worker",
            "AI worker that picks tasks from bn ready",
            ExecutionMode::Container,
            LifecycleMode::Stateful,
            "Run bn orient to get started.",
        )
        .with_tools(ToolPermissions::new().allow("write").allow("binnacle"));

        let content = agent.generate_agent_file_content();

        // Check YAML frontmatter structure
        assert!(content.starts_with("---\n"));
        assert!(content.contains("name: Binnacle Worker"));
        assert!(content.contains("description: AI worker that picks tasks from bn ready"));
        assert!(content.contains("tools:"));
        assert!(content.contains("---\nRun bn orient"));

        // Check tools are mapped correctly
        assert!(content.contains("edit")); // write -> edit
        assert!(content.contains("binnacle/*")); // binnacle -> binnacle/*
    }

    #[test]
    fn test_display_name() {
        let worker = AgentDefinition::new(
            "worker",
            "",
            ExecutionMode::Host,
            LifecycleMode::Stateful,
            "",
        );
        assert_eq!(worker.display_name(), "Worker");

        let prd =
            AgentDefinition::new("prd", "", ExecutionMode::Host, LifecycleMode::Stateless, "");
        assert_eq!(prd.display_name(), "PRD");

        let custom = AgentDefinition::new(
            "custom",
            "",
            ExecutionMode::Host,
            LifecycleMode::Stateful,
            "",
        );
        assert_eq!(custom.display_name(), "Custom");
    }
}
