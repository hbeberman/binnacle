//! Embedded default agent definitions.
//!
//! This module contains the built-in agent definitions that are compiled into
//! the bn binary. These serve as the base layer for agent resolution.
//!
//! The 6 canonical agent types are:
//! - **worker**: Container-based autonomous worker (picks from bn ready)
//! - **do**: Host-based directed task executor
//! - **prd**: Host-based PRD writer/planner (stateless)
//! - **buddy**: Host-based quick entry helper
//! - **ask**: Host-based read-only Q&A assistant (stateless)
//! - **free**: Host-based general purpose agent

use crate::agents::definitions::{
    AGENT_ASK, AGENT_BUDDY, AGENT_DO, AGENT_FREE, AGENT_PRD, AGENT_WORKER, AgentDefinition,
    CopilotConfig, ExecutionMode, LifecycleMode, ToolPermissions,
};

// Embedded prompt content (included at compile time)
const WORKER_PROMPT: &str = include_str!("embedded/worker.md");
const DO_PROMPT_TEMPLATE: &str = include_str!("embedded/do.md");
const PRD_PROMPT: &str = include_str!("embedded/prd.md");
const BUDDY_PROMPT: &str = include_str!("embedded/buddy.md");
const ASK_PROMPT: &str = include_str!("embedded/ask.md");
const FREE_PROMPT: &str = include_str!("embedded/free.md");

/// Tool permission sets for different agent types.
///
/// These define the default allowed and denied tools for each agent category.
pub mod tool_sets {
    use super::*;

    /// Full tool access for worker agents (most permissive).
    pub fn worker_tools() -> ToolPermissions {
        ToolPermissions {
            allow: vec![
                "write".to_string(),
                "shell(bn:*)".to_string(),
                "shell(cargo:*)".to_string(),
                "shell(git:*)".to_string(),
                "shell(npm:*)".to_string(),
                "shell(just:*)".to_string(),
                "binnacle".to_string(),
                "edit".to_string(),
                "create".to_string(),
                "view".to_string(),
                "grep".to_string(),
                "glob".to_string(),
                "lsp".to_string(),
            ],
            deny: vec![
                "shell(bn agent terminate:*)".to_string(),
                "binnacle(binnacle-orient)".to_string(),
                "binnacle(binnacle-goodbye)".to_string(),
            ],
        }
    }

    /// Tool access for PRD/planner agents (read-heavy, limited write).
    pub fn prd_tools() -> ToolPermissions {
        ToolPermissions {
            allow: vec![
                "write".to_string(),
                "shell(bn:*)".to_string(),
                "shell(git:*)".to_string(),
                "binnacle".to_string(),
                "view".to_string(),
                "grep".to_string(),
                "glob".to_string(),
                "lsp".to_string(),
            ],
            deny: vec![
                "shell(cargo:*)".to_string(),
                "shell(npm:*)".to_string(),
                "binnacle(binnacle-orient)".to_string(),
                "binnacle(binnacle-goodbye)".to_string(),
            ],
        }
    }

    /// Tool access for buddy agents (bn operations focused).
    pub fn buddy_tools() -> ToolPermissions {
        ToolPermissions {
            allow: vec![
                "write".to_string(),
                "shell(bn:*)".to_string(),
                "shell(git:*)".to_string(),
                "binnacle".to_string(),
                "view".to_string(),
                "grep".to_string(),
                "glob".to_string(),
                "lsp".to_string(),
            ],
            deny: vec![
                "shell(cargo:*)".to_string(),
                "shell(npm:*)".to_string(),
                "binnacle(binnacle-orient)".to_string(),
                "binnacle(binnacle-goodbye)".to_string(),
            ],
        }
    }

    /// Tool access for ask agents (read-only).
    pub fn ask_tools() -> ToolPermissions {
        ToolPermissions {
            allow: vec![
                "shell(bn:*)".to_string(),
                "shell(git:log)".to_string(),
                "shell(git:show)".to_string(),
                "shell(git:diff)".to_string(),
                "binnacle".to_string(),
                "view".to_string(),
                "grep".to_string(),
                "glob".to_string(),
                "lsp".to_string(),
            ],
            deny: vec![
                "write".to_string(),
                "edit".to_string(),
                "create".to_string(),
                "shell(cargo:*)".to_string(),
                "shell(npm:*)".to_string(),
                "shell(git:push)".to_string(),
                "shell(git:commit)".to_string(),
                "binnacle(binnacle-orient)".to_string(),
                "binnacle(binnacle-goodbye)".to_string(),
            ],
        }
    }

    /// Tool access for free agents (full access, user-directed).
    pub fn free_tools() -> ToolPermissions {
        ToolPermissions {
            allow: vec![
                "write".to_string(),
                "shell(bn:*)".to_string(),
                "shell(cargo:*)".to_string(),
                "shell(git:*)".to_string(),
                "shell(npm:*)".to_string(),
                "shell(just:*)".to_string(),
                "binnacle".to_string(),
                "edit".to_string(),
                "create".to_string(),
                "view".to_string(),
                "grep".to_string(),
                "glob".to_string(),
                "lsp".to_string(),
            ],
            deny: vec![
                "shell(bn agent terminate:*)".to_string(),
                "binnacle(binnacle-orient)".to_string(),
                "binnacle(binnacle-goodbye)".to_string(),
            ],
        }
    }
}

/// Get the embedded agent definition for a given agent type.
///
/// Returns `None` if the agent type is not recognized.
pub fn get_embedded_agent(name: &str) -> Option<AgentDefinition> {
    match name {
        AGENT_WORKER => Some(worker_agent()),
        AGENT_DO => Some(do_agent()),
        AGENT_PRD => Some(prd_agent()),
        AGENT_BUDDY => Some(buddy_agent()),
        AGENT_ASK => Some(ask_agent()),
        AGENT_FREE => Some(free_agent()),
        _ => None,
    }
}

/// Get all embedded agent definitions.
pub fn get_all_embedded_agents() -> Vec<AgentDefinition> {
    vec![
        worker_agent(),
        do_agent(),
        prd_agent(),
        buddy_agent(),
        ask_agent(),
        free_agent(),
    ]
}

/// Worker agent: Container-based autonomous worker.
fn worker_agent() -> AgentDefinition {
    AgentDefinition::new(
        AGENT_WORKER,
        "AI worker that picks tasks from bn ready",
        ExecutionMode::Container,
        LifecycleMode::Stateful,
        WORKER_PROMPT.trim(),
    )
    .with_tools(tool_sets::worker_tools())
    .with_copilot(CopilotConfig::default())
}

/// Do agent: Host-based directed task executor.
fn do_agent() -> AgentDefinition {
    AgentDefinition::new(
        AGENT_DO,
        "Works on a user-specified task",
        ExecutionMode::Host,
        LifecycleMode::Stateful,
        DO_PROMPT_TEMPLATE.trim(),
    )
    .with_tools(tool_sets::worker_tools())
    .with_copilot(CopilotConfig::default())
}

/// PRD agent: Host-based PRD writer/planner.
fn prd_agent() -> AgentDefinition {
    AgentDefinition::new(
        AGENT_PRD,
        "Converts ideas into detailed PRDs",
        ExecutionMode::Host,
        LifecycleMode::Stateless,
        PRD_PROMPT.trim(),
    )
    .with_tools(tool_sets::prd_tools())
    .with_copilot(CopilotConfig::default())
}

/// Buddy agent: Host-based quick entry helper.
fn buddy_agent() -> AgentDefinition {
    AgentDefinition::new(
        AGENT_BUDDY,
        "Creates bugs/tasks/ideas quickly",
        ExecutionMode::Host,
        LifecycleMode::Stateful,
        BUDDY_PROMPT.trim(),
    )
    .with_tools(tool_sets::buddy_tools())
    .with_copilot(CopilotConfig::default())
}

/// Ask agent: Host-based read-only Q&A assistant.
fn ask_agent() -> AgentDefinition {
    AgentDefinition::new(
        AGENT_ASK,
        "Read-only codebase exploration",
        ExecutionMode::Host,
        LifecycleMode::Stateless,
        ASK_PROMPT.trim(),
    )
    .with_tools(tool_sets::ask_tools())
    .with_copilot(CopilotConfig::default())
}

/// Free agent: Host-based general purpose agent.
fn free_agent() -> AgentDefinition {
    AgentDefinition::new(
        AGENT_FREE,
        "Full access, user-directed",
        ExecutionMode::Host,
        LifecycleMode::Stateful,
        FREE_PROMPT.trim(),
    )
    .with_tools(tool_sets::free_tools())
    .with_copilot(CopilotConfig::default())
}

/// Generate a "do" prompt with the given task description.
///
/// This replaces the `{description}` placeholder in the do agent's prompt template.
pub fn do_prompt(description: &str) -> String {
    DO_PROMPT_TEMPLATE.replace("{description}", description)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_embedded_agent_worker() {
        let agent = get_embedded_agent(AGENT_WORKER).unwrap();
        assert_eq!(agent.name, AGENT_WORKER);
        assert_eq!(agent.execution, ExecutionMode::Container);
        assert_eq!(agent.lifecycle, LifecycleMode::Stateful);
        assert!(agent.prompt.contains("bn orient"));
        assert!(agent.prompt.contains("bn ready"));
    }

    #[test]
    fn test_get_embedded_agent_prd() {
        let agent = get_embedded_agent(AGENT_PRD).unwrap();
        assert_eq!(agent.name, AGENT_PRD);
        assert_eq!(agent.execution, ExecutionMode::Host);
        assert_eq!(agent.lifecycle, LifecycleMode::Stateless);
        assert!(agent.prompt.contains("bn doc create"));
    }

    #[test]
    fn test_get_embedded_agent_ask() {
        let agent = get_embedded_agent(AGENT_ASK).unwrap();
        assert_eq!(agent.name, AGENT_ASK);
        assert_eq!(agent.execution, ExecutionMode::Host);
        assert_eq!(agent.lifecycle, LifecycleMode::Stateless);
        // Ask agent should have read-only tools
        assert!(!agent.tools.allow.contains(&"edit".to_string()));
        assert!(agent.tools.deny.contains(&"write".to_string()));
    }

    #[test]
    fn test_get_embedded_agent_buddy() {
        let agent = get_embedded_agent(AGENT_BUDDY).unwrap();
        assert_eq!(agent.name, AGENT_BUDDY);
        assert_eq!(agent.execution, ExecutionMode::Host);
        assert_eq!(agent.lifecycle, LifecycleMode::Stateful);
        assert!(agent.prompt.contains("bn idea create"));
        assert!(agent.prompt.contains("bn task create"));
        assert!(agent.prompt.contains("bn bug create"));
    }

    #[test]
    fn test_get_embedded_agent_free() {
        let agent = get_embedded_agent(AGENT_FREE).unwrap();
        assert_eq!(agent.name, AGENT_FREE);
        assert_eq!(agent.execution, ExecutionMode::Host);
        assert_eq!(agent.lifecycle, LifecycleMode::Stateful);
    }

    #[test]
    fn test_get_embedded_agent_do() {
        let agent = get_embedded_agent(AGENT_DO).unwrap();
        assert_eq!(agent.name, AGENT_DO);
        assert_eq!(agent.execution, ExecutionMode::Host);
        assert_eq!(agent.lifecycle, LifecycleMode::Stateful);
        // Do agent template has placeholder
        assert!(agent.prompt.contains("{description}"));
    }

    #[test]
    fn test_get_embedded_agent_unknown() {
        assert!(get_embedded_agent("unknown").is_none());
        assert!(get_embedded_agent("").is_none());
    }

    #[test]
    fn test_get_all_embedded_agents() {
        let agents = get_all_embedded_agents();
        assert_eq!(agents.len(), 6);

        let names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
        assert!(names.contains(&AGENT_WORKER));
        assert!(names.contains(&AGENT_DO));
        assert!(names.contains(&AGENT_PRD));
        assert!(names.contains(&AGENT_BUDDY));
        assert!(names.contains(&AGENT_ASK));
        assert!(names.contains(&AGENT_FREE));
    }

    #[test]
    fn test_do_prompt_replaces_description() {
        let prompt = do_prompt("implement the foo feature");
        assert!(prompt.contains("implement the foo feature"));
        assert!(!prompt.contains("{description}"));
    }

    #[test]
    fn test_all_prompts_contain_lsp_guidance() {
        for agent in get_all_embedded_agents() {
            assert!(
                agent.prompt.contains("LSP"),
                "Agent '{}' prompt should contain LSP guidance",
                agent.name
            );
        }
    }

    #[test]
    fn test_stateful_agents_mention_goodbye() {
        for agent in get_all_embedded_agents() {
            if agent.lifecycle == LifecycleMode::Stateful {
                assert!(
                    agent.prompt.contains("goodbye"),
                    "Stateful agent '{}' should mention goodbye in prompt",
                    agent.name
                );
            }
        }
    }

    #[test]
    fn test_stateless_agents_no_goodbye() {
        for agent in get_all_embedded_agents() {
            if agent.lifecycle == LifecycleMode::Stateless {
                // Stateless agents should either not mention goodbye or explicitly say not to use it
                let mentions_no_goodbye = agent.prompt.contains("Do NOT run `bn goodbye`")
                    || agent.prompt.contains("Do NOT run `bn goodbye`");
                let doesnt_mention = !agent.prompt.contains("bn goodbye");

                assert!(
                    mentions_no_goodbye || doesnt_mention,
                    "Stateless agent '{}' should either not mention goodbye or explicitly say not to use it",
                    agent.name
                );
            }
        }
    }

    #[test]
    fn test_worker_tools_deny_mcp_lifecycle() {
        let tools = tool_sets::worker_tools();
        assert!(
            tools
                .deny
                .contains(&"binnacle(binnacle-orient)".to_string())
        );
        assert!(
            tools
                .deny
                .contains(&"binnacle(binnacle-goodbye)".to_string())
        );
    }

    #[test]
    fn test_ask_tools_are_read_only() {
        let tools = tool_sets::ask_tools();

        // Should deny write operations
        assert!(tools.deny.contains(&"write".to_string()));
        assert!(tools.deny.contains(&"edit".to_string()));
        assert!(tools.deny.contains(&"create".to_string()));

        // Should allow read operations
        assert!(tools.allow.contains(&"view".to_string()));
        assert!(tools.allow.contains(&"grep".to_string()));
        assert!(tools.allow.contains(&"glob".to_string()));
    }

    #[test]
    fn test_all_embedded_agents_have_copilot_defaults() {
        let expected = CopilotConfig::default();
        for agent in get_all_embedded_agents() {
            assert_eq!(
                agent.copilot, expected,
                "Agent '{}' should have CopilotConfig defaults",
                agent.name
            );
            assert_eq!(agent.copilot.model, "claude-opus-4.6");
            assert_eq!(agent.copilot.reasoning_effort, "high");
            assert!(agent.copilot.show_reasoning);
            assert!(agent.copilot.render_markdown);
        }
    }
}
