//! KDL parsing for agent definitions.
//!
//! This module provides KDL parsing for agent definitions from config files.
//! Agent definitions can be customized at multiple levels using the `agent` block
//! in config.kdl files.
//!
//! # KDL Schema
//!
//! ```kdl
//! agent "worker" {
//!     description "AI worker that picks tasks from bn ready"
//!     execution "container"  // "container" | "host"
//!     lifecycle "stateful"   // "stateful" | "stateless"
//!     
//!     // Copilot runtime settings (optional, defaults shown)
//!     model "claude-opus-4.6"
//!     reasoning-effort "high"
//!     show-reasoning #true
//!     render-markdown #true
//!     
//!     tools {
//!         allow "write"
//!         allow "shell(bn:*)"
//!         deny "shell(bn agent terminate:*)"
//!     }
//!     
//!     // Optional: override prompt from file
//!     prompt-file "worker/prompt.md"
//! }
//! ```
//!
//! # Resolution Paths
//!
//! Agent config files are loaded from (in order of precedence):
//! 1. **Embedded** (in bn binary) - Default agent definitions
//! 2. **System** (~/.config/binnacle/agents/config.kdl) - Global user customizations
//! 3. **Session** (~/.local/share/binnacle/<hash>/agents/config.kdl) - Per-repo customizations
//! 4. **Project** (.binnacle/agents/config.kdl) - Repo-specific customizations

use crate::Error;
use crate::agents::definitions::{AgentDefinition, ExecutionMode, LifecycleMode, ToolPermissions};
use kdl::{KdlDocument, KdlNode};
use std::path::Path;

/// Parsed agent customization from KDL.
///
/// This represents a partial agent definition that can override
/// fields from the embedded defaults. `None` values mean "use default".
#[derive(Debug, Clone, Default)]
pub struct AgentOverride {
    /// Agent type name.
    pub name: String,
    /// Override description.
    pub description: Option<String>,
    /// Override execution mode.
    pub execution: Option<ExecutionMode>,
    /// Override lifecycle mode.
    pub lifecycle: Option<LifecycleMode>,
    /// Tools to add to allow list.
    pub tools_allow: Vec<String>,
    /// Tools to add to deny list.
    pub tools_deny: Vec<String>,
    /// Path to custom prompt file (relative to config location).
    pub prompt_file: Option<String>,
    /// Inline prompt content.
    pub prompt: Option<String>,
    /// Override copilot model.
    pub model: Option<String>,
    /// Override reasoning effort.
    pub reasoning_effort: Option<String>,
    /// Override show_reasoning.
    pub show_reasoning: Option<bool>,
    /// Override render_markdown.
    pub render_markdown: Option<bool>,
}

impl AgentOverride {
    /// Create an override for a named agent.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Apply this override to a base agent definition.
    ///
    /// Returns a new AgentDefinition with overridden values.
    /// If a prompt_file is set, `prompt_base_path` is used to resolve it.
    pub fn apply_to(
        &self,
        base: &AgentDefinition,
        prompt_base_path: Option<&Path>,
    ) -> AgentDefinition {
        let mut result = base.clone();

        // Override description if set
        if let Some(ref desc) = self.description {
            result.description = desc.clone();
        }

        // Override execution mode if set
        if let Some(exec) = self.execution {
            result.execution = exec;
        }

        // Override lifecycle mode if set
        if let Some(lifecycle) = self.lifecycle {
            result.lifecycle = lifecycle;
        }

        // Merge tool permissions (additive)
        if !self.tools_allow.is_empty() || !self.tools_deny.is_empty() {
            let overlay = ToolPermissions {
                allow: self.tools_allow.clone(),
                deny: self.tools_deny.clone(),
            };
            result.tools.merge(&overlay);
        }

        // Override prompt if prompt_file is set
        if let Some(ref prompt_file) = self.prompt_file {
            if let Some(base_path) = prompt_base_path {
                let prompt_path = base_path.join(prompt_file);
                if let Ok(content) = std::fs::read_to_string(&prompt_path) {
                    result.prompt = content.trim().to_string();
                }
            }
        }

        // Override with inline prompt if set
        if let Some(ref prompt) = self.prompt {
            result.prompt = prompt.clone();
        }

        // Merge copilot config (only override fields that are explicitly set)
        if let Some(ref model) = self.model {
            result.copilot.model = model.clone();
        }
        if let Some(ref reasoning_effort) = self.reasoning_effort {
            result.copilot.reasoning_effort = reasoning_effort.clone();
        }
        if let Some(show_reasoning) = self.show_reasoning {
            result.copilot.show_reasoning = show_reasoning;
        }
        if let Some(render_markdown) = self.render_markdown {
            result.copilot.render_markdown = render_markdown;
        }

        result
    }
}

/// Parse agent overrides from a KDL document.
///
/// Looks for `agent "name" { ... }` blocks in the document.
pub fn parse_agent_overrides(doc: &KdlDocument) -> Result<Vec<AgentOverride>, Error> {
    let mut overrides = Vec::new();

    for node in doc.nodes() {
        if node.name().value() == "agent" {
            let override_def = parse_agent_node(node)?;
            overrides.push(override_def);
        }
    }

    Ok(overrides)
}

/// Parse a single agent node.
fn parse_agent_node(node: &KdlNode) -> Result<AgentOverride, Error> {
    // Get agent name from first argument
    let name = node
        .entries()
        .first()
        .and_then(|e| e.value().as_string())
        .ok_or_else(|| Error::InvalidInput("agent node must have a name argument".to_string()))?;

    let mut override_def = AgentOverride::new(name);

    // Parse children if present
    if let Some(children) = node.children() {
        for child in children.nodes() {
            match child.name().value() {
                "description" => {
                    if let Some(desc) = get_string_arg(child) {
                        override_def.description = Some(desc);
                    }
                }
                "execution" => {
                    if let Some(exec_str) = get_string_arg(child) {
                        override_def.execution = Some(exec_str.parse()?);
                    }
                }
                "lifecycle" => {
                    if let Some(lifecycle_str) = get_string_arg(child) {
                        override_def.lifecycle = Some(lifecycle_str.parse()?);
                    }
                }
                "prompt-file" => {
                    if let Some(path) = get_string_arg(child) {
                        override_def.prompt_file = Some(path);
                    }
                }
                "prompt" => {
                    if let Some(content) = get_string_arg(child) {
                        override_def.prompt = Some(content);
                    }
                }
                "model" => {
                    if let Some(model) = get_string_arg(child) {
                        override_def.model = Some(model);
                    }
                }
                "reasoning-effort" => {
                    if let Some(effort) = get_string_arg(child) {
                        override_def.reasoning_effort = Some(effort);
                    }
                }
                "show-reasoning" => {
                    if let Some(val) = get_bool_arg(child) {
                        override_def.show_reasoning = Some(val);
                    }
                }
                "render-markdown" => {
                    if let Some(val) = get_bool_arg(child) {
                        override_def.render_markdown = Some(val);
                    }
                }
                "tools" => {
                    parse_tools_node(child, &mut override_def)?;
                }
                _ => {
                    // Ignore unknown fields for forward compatibility
                }
            }
        }
    }

    Ok(override_def)
}

/// Parse a tools block.
fn parse_tools_node(node: &KdlNode, override_def: &mut AgentOverride) -> Result<(), Error> {
    if let Some(children) = node.children() {
        for child in children.nodes() {
            match child.name().value() {
                "allow" => {
                    if let Some(pattern) = get_string_arg(child) {
                        override_def.tools_allow.push(pattern);
                    }
                }
                "deny" => {
                    if let Some(pattern) = get_string_arg(child) {
                        override_def.tools_deny.push(pattern);
                    }
                }
                _ => {
                    // Ignore unknown fields
                }
            }
        }
    }
    Ok(())
}

/// Get a string argument from a node's first entry.
fn get_string_arg(node: &KdlNode) -> Option<String> {
    node.entries()
        .first()
        .and_then(|e| e.value().as_string())
        .map(|s| s.to_string())
}

/// Get a boolean argument from a node's first entry.
fn get_bool_arg(node: &KdlNode) -> Option<bool> {
    node.entries().first().and_then(|e| e.value().as_bool())
}

/// Load agent overrides from a KDL file path.
///
/// Returns an empty vector if the file doesn't exist.
pub fn load_overrides_from_file(path: &Path) -> Result<Vec<AgentOverride>, Error> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(path)
        .map_err(|e| Error::Other(format!("Failed to read {}: {}", path.display(), e)))?;

    let doc: KdlDocument = content
        .parse()
        .map_err(|e| Error::Other(format!("Failed to parse KDL in {}: {}", path.display(), e)))?;

    parse_agent_overrides(&doc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::definitions::{CopilotConfig, ExecutionMode, LifecycleMode};

    #[test]
    fn test_parse_agent_override_basic() {
        let kdl = r#"
            agent "worker" {
                description "Custom worker description"
                execution "host"
                lifecycle "stateless"
            }
        "#;

        let doc: KdlDocument = kdl.parse().unwrap();
        let overrides = parse_agent_overrides(&doc).unwrap();

        assert_eq!(overrides.len(), 1);
        let worker = &overrides[0];
        assert_eq!(worker.name, "worker");
        assert_eq!(
            worker.description,
            Some("Custom worker description".to_string())
        );
        assert_eq!(worker.execution, Some(ExecutionMode::Host));
        assert_eq!(worker.lifecycle, Some(LifecycleMode::Stateless));
    }

    #[test]
    fn test_parse_agent_override_with_tools() {
        let kdl = r#"
            agent "prd" {
                tools {
                    allow "shell(npm:*)"
                    allow "shell(yarn:*)"
                    deny "shell(rm:*)"
                }
            }
        "#;

        let doc: KdlDocument = kdl.parse().unwrap();
        let overrides = parse_agent_overrides(&doc).unwrap();

        assert_eq!(overrides.len(), 1);
        let prd = &overrides[0];
        assert_eq!(prd.name, "prd");
        assert_eq!(prd.tools_allow, vec!["shell(npm:*)", "shell(yarn:*)"]);
        assert_eq!(prd.tools_deny, vec!["shell(rm:*)"]);
    }

    #[test]
    fn test_parse_agent_override_with_prompt_file() {
        let kdl = r#"
            agent "buddy" {
                prompt-file "buddy/custom-prompt.md"
            }
        "#;

        let doc: KdlDocument = kdl.parse().unwrap();
        let overrides = parse_agent_overrides(&doc).unwrap();

        assert_eq!(overrides.len(), 1);
        let buddy = &overrides[0];
        assert_eq!(
            buddy.prompt_file,
            Some("buddy/custom-prompt.md".to_string())
        );
    }

    #[test]
    fn test_parse_multiple_agents() {
        let kdl = r#"
            agent "worker" {
                description "Worker 1"
            }
            
            agent "prd" {
                description "PRD 1"
            }
        "#;

        let doc: KdlDocument = kdl.parse().unwrap();
        let overrides = parse_agent_overrides(&doc).unwrap();

        assert_eq!(overrides.len(), 2);
        assert_eq!(overrides[0].name, "worker");
        assert_eq!(overrides[1].name, "prd");
    }

    #[test]
    fn test_apply_override_to_base() {
        let base = AgentDefinition::new(
            "worker",
            "Original description",
            ExecutionMode::Container,
            LifecycleMode::Stateful,
            "Original prompt",
        );

        let override_def = AgentOverride {
            name: "worker".to_string(),
            description: Some("Custom description".to_string()),
            execution: Some(ExecutionMode::Host),
            lifecycle: None, // Keep original
            tools_allow: vec!["custom-tool".to_string()],
            tools_deny: vec![],
            prompt_file: None,
            prompt: Some("Custom prompt".to_string()),
            model: None,
            reasoning_effort: None,
            show_reasoning: None,
            render_markdown: None,
        };

        let result = override_def.apply_to(&base, None);

        assert_eq!(result.name, "worker");
        assert_eq!(result.description, "Custom description");
        assert_eq!(result.execution, ExecutionMode::Host);
        assert_eq!(result.lifecycle, LifecycleMode::Stateful); // Unchanged
        assert!(result.tools.allow.contains(&"custom-tool".to_string()));
        assert_eq!(result.prompt, "Custom prompt");
    }

    #[test]
    fn test_parse_empty_document() {
        let kdl = "// Just a comment\n";

        let doc: KdlDocument = kdl.parse().unwrap();
        let overrides = parse_agent_overrides(&doc).unwrap();

        assert!(overrides.is_empty());
    }

    #[test]
    fn test_parse_agent_without_children() {
        // Agent with just a name, no customizations
        let kdl = r#"agent "worker""#;

        let doc: KdlDocument = kdl.parse().unwrap();
        let overrides = parse_agent_overrides(&doc).unwrap();

        assert_eq!(overrides.len(), 1);
        assert_eq!(overrides[0].name, "worker");
        assert!(overrides[0].description.is_none());
    }

    #[test]
    fn test_parse_ignores_unknown_fields() {
        let kdl = r#"
            agent "worker" {
                description "Test"
                unknown-field "should be ignored"
                future-feature 1
            }
        "#;

        let doc: KdlDocument = kdl.parse().unwrap();
        let overrides = parse_agent_overrides(&doc).unwrap();

        assert_eq!(overrides.len(), 1);
        assert_eq!(overrides[0].description, Some("Test".to_string()));
    }

    #[test]
    fn test_parse_agent_missing_name_errors() {
        let kdl = r#"
            agent {
                description "No name"
            }
        "#;

        let doc: KdlDocument = kdl.parse().unwrap();
        let result = parse_agent_overrides(&doc);

        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_execution_mode_errors() {
        let kdl = r#"
            agent "worker" {
                execution "invalid"
            }
        "#;

        let doc: KdlDocument = kdl.parse().unwrap();
        let result = parse_agent_overrides(&doc);

        assert!(result.is_err());
    }

    #[test]
    fn test_apply_tools_merge() {
        let base_tools = ToolPermissions::new().allow("write").deny("shell(rm:*)");

        let base = AgentDefinition::new(
            "worker",
            "Test",
            ExecutionMode::Container,
            LifecycleMode::Stateful,
            "Prompt",
        )
        .with_tools(base_tools);

        let override_def = AgentOverride {
            name: "worker".to_string(),
            tools_allow: vec!["read".to_string()],
            tools_deny: vec!["shell(sudo:*)".to_string()],
            ..Default::default()
        };

        let result = override_def.apply_to(&base, None);

        // Should have both original and new tools
        assert!(result.tools.allow.contains(&"write".to_string()));
        assert!(result.tools.allow.contains(&"read".to_string()));
        assert!(result.tools.deny.contains(&"shell(rm:*)".to_string()));
        assert!(result.tools.deny.contains(&"shell(sudo:*)".to_string()));
    }

    #[test]
    fn test_parse_copilot_config_all_fields() {
        let kdl = r#"
            agent "worker" {
                model "claude-sonnet-4"
                reasoning-effort "medium"
                show-reasoning #false
                render-markdown #false
            }
        "#;

        let doc: KdlDocument = kdl.parse().unwrap();
        let overrides = parse_agent_overrides(&doc).unwrap();

        assert_eq!(overrides.len(), 1);
        let worker = &overrides[0];
        assert_eq!(worker.model, Some("claude-sonnet-4".to_string()));
        assert_eq!(worker.reasoning_effort, Some("medium".to_string()));
        assert_eq!(worker.show_reasoning, Some(false));
        assert_eq!(worker.render_markdown, Some(false));
    }

    #[test]
    fn test_parse_copilot_config_partial() {
        let kdl = r#"
            agent "worker" {
                model "gpt-4"
            }
        "#;

        let doc: KdlDocument = kdl.parse().unwrap();
        let overrides = parse_agent_overrides(&doc).unwrap();

        assert_eq!(overrides.len(), 1);
        let worker = &overrides[0];
        assert_eq!(worker.model, Some("gpt-4".to_string()));
        assert!(worker.reasoning_effort.is_none());
        assert!(worker.show_reasoning.is_none());
        assert!(worker.render_markdown.is_none());
    }

    #[test]
    fn test_apply_copilot_config_override() {
        let base = AgentDefinition::new(
            "worker",
            "Test worker",
            ExecutionMode::Container,
            LifecycleMode::Stateful,
            "Prompt",
        );
        // base.copilot is CopilotConfig::default()

        let override_def = AgentOverride {
            name: "worker".to_string(),
            model: Some("claude-sonnet-4".to_string()),
            reasoning_effort: Some("low".to_string()),
            ..Default::default()
        };

        let result = override_def.apply_to(&base, None);

        assert_eq!(result.copilot.model, "claude-sonnet-4");
        assert_eq!(result.copilot.reasoning_effort, "low");
        // Unset fields preserve defaults
        assert!(result.copilot.show_reasoning);
        assert!(result.copilot.render_markdown);
    }

    #[test]
    fn test_apply_copilot_config_no_override() {
        let base = AgentDefinition::new(
            "worker",
            "Test worker",
            ExecutionMode::Container,
            LifecycleMode::Stateful,
            "Prompt",
        );

        let override_def = AgentOverride {
            name: "worker".to_string(),
            ..Default::default()
        };

        let result = override_def.apply_to(&base, None);

        // All copilot fields should remain at defaults
        assert_eq!(result.copilot, CopilotConfig::default());
    }

    #[test]
    fn test_parse_copilot_config_with_other_fields() {
        let kdl = r#"
            agent "worker" {
                description "Custom worker"
                execution "host"
                model "claude-opus-4.6"
                reasoning-effort "high"
                show-reasoning #true
                render-markdown #true
                tools {
                    allow "write"
                }
            }
        "#;

        let doc: KdlDocument = kdl.parse().unwrap();
        let overrides = parse_agent_overrides(&doc).unwrap();

        assert_eq!(overrides.len(), 1);
        let worker = &overrides[0];
        assert_eq!(worker.description, Some("Custom worker".to_string()));
        assert_eq!(worker.execution, Some(ExecutionMode::Host));
        assert_eq!(worker.model, Some("claude-opus-4.6".to_string()));
        assert_eq!(worker.reasoning_effort, Some("high".to_string()));
        assert_eq!(worker.show_reasoning, Some(true));
        assert_eq!(worker.render_markdown, Some(true));
        assert_eq!(worker.tools_allow, vec!["write"]);
    }
}
