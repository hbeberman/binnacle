//! Agent file templates for .github/agents/ format.
//!
//! These templates define agent configurations compatible with GitHub Copilot CLI's
//! `--agent` flag. Each agent includes YAML frontmatter specifying name, description,
//! tools, and the agent prompt content.
//!
//! The 6 canonical agents:
//! - `auto` - Pick task from `bn ready` and work on it
//! - `do` - Work on custom task described in argument
//! - `prd` - Render ideas into PRDs
//! - `buddy` - Quick entry for bugs/tasks/ideas
//! - `ask` - Read-only interactive Q&A
//! - `free` - General purpose with binnacle orientation

use super::prompts;

/// Metadata for an agent's YAML frontmatter.
pub struct AgentMeta {
    /// Display name for the agent
    pub name: &'static str,
    /// Brief description of what the agent does
    pub description: &'static str,
    /// Hint for the argument (shown in UI)
    pub argument_hint: &'static str,
    /// Tool permissions array (YAML format)
    pub tools: &'static str,
}

/// Metadata for the auto agent (picks from bn ready)
pub const AUTO_META: AgentMeta = AgentMeta {
    name: "Binnacle Auto",
    description: "Pick a task from bn ready and work on it immediately",
    argument_hint: "(optional) Override task selection criteria",
    tools: "['binnacle/*', 'edit', 'execute', 'agent', 'search', 'read']",
};

/// Metadata for the do agent (works on specific task)
pub const DO_META: AgentMeta = AgentMeta {
    name: "Binnacle Do",
    description: "Work on a specific task described in the argument",
    argument_hint: "Describe the task to work on",
    tools: "['binnacle/*', 'edit', 'execute', 'agent', 'search', 'read']",
};

/// Metadata for the PRD agent (renders ideas to PRDs)
pub const PRD_META: AgentMeta = AgentMeta {
    name: "Binnacle PRD",
    description: "Convert ideas into detailed product requirement documents",
    argument_hint: "(optional) Specific idea ID or topic to write PRD for",
    tools: "['binnacle/*', 'edit', 'agent', 'search', 'read', 'web/fetch']",
};

/// Metadata for the buddy agent (quick entry)
pub const BUDDY_META: AgentMeta = AgentMeta {
    name: "Binnacle Buddy",
    description: "Quick entry assistant for bugs, tasks, and ideas",
    argument_hint: "(optional) What to add - e.g., \"bug: login fails on mobile\"",
    tools: "['binnacle/*', 'agent', 'search', 'read']",
};

/// Metadata for the ask agent (read-only Q&A)
pub const ASK_META: AgentMeta = AgentMeta {
    name: "Binnacle Ask",
    description: "Read-only interactive Q&A about the repository",
    argument_hint: "Your question about the codebase",
    tools: "['binnacle/*', 'agent', 'search', 'read']",
};

/// Metadata for the free agent (general purpose)
pub const FREE_META: AgentMeta = AgentMeta {
    name: "Binnacle Free",
    description: "General purpose agent with binnacle access",
    argument_hint: "(optional) What to work on",
    tools: "['binnacle/*', 'edit', 'execute', 'agent', 'search', 'read']",
};

/// All agent template names for enumeration.
pub const AGENT_NAMES: &[&str] = &["auto", "do", "prd", "buddy", "ask", "free"];

/// Agent names written to .github/agents/ for GitHub Copilot.
/// Excludes auto/free which are designed for CLI automation, not interactive chat.
pub const COPILOT_AGENT_NAMES: &[&str] = &["do", "prd", "buddy", "ask"];

/// Generate YAML frontmatter from agent metadata.
fn format_frontmatter(meta: &AgentMeta) -> String {
    format!(
        r#"---
name: {}
description: {}
argument-hint: {}
tools: {}
---"#,
        meta.name, meta.description, meta.argument_hint, meta.tools
    )
}

/// Generate full agent file content for auto agent.
pub fn agent_auto() -> String {
    format!(
        "{}\n{}\n",
        format_frontmatter(&AUTO_META),
        prompts::WORKER_PROMPT
    )
}

/// Generate full agent file content for do agent.
/// Note: The prompt contains `{description}` placeholder which is replaced
/// with `{argument}` for the agent file format.
pub fn agent_do() -> String {
    let prompt = prompts::DO_PROMPT_TEMPLATE.replace("{description}", "{argument}");
    format!("{}\n{}\n", format_frontmatter(&DO_META), prompt)
}

/// Generate full agent file content for prd agent.
pub fn agent_prd() -> String {
    format!(
        "{}\n{}\n",
        format_frontmatter(&PRD_META),
        prompts::PRD_PROMPT
    )
}

/// Generate full agent file content for buddy agent.
pub fn agent_buddy() -> String {
    format!(
        "{}\n{}\n",
        format_frontmatter(&BUDDY_META),
        prompts::BUDDY_PROMPT
    )
}

/// Generate full agent file content for ask agent.
pub fn agent_ask() -> String {
    format!(
        "{}\n{}\n",
        format_frontmatter(&ASK_META),
        prompts::ASK_PROMPT
    )
}

/// Generate full agent file content for free agent.
pub fn agent_free() -> String {
    format!(
        "{}\n{}\n",
        format_frontmatter(&FREE_META),
        prompts::FREE_PROMPT
    )
}

/// Get agent file content by name.
pub fn get_agent_content(name: &str) -> Option<String> {
    match name {
        "auto" => Some(agent_auto()),
        "do" => Some(agent_do()),
        "prd" => Some(agent_prd()),
        "buddy" => Some(agent_buddy()),
        "ask" => Some(agent_ask()),
        "free" => Some(agent_free()),
        _ => None,
    }
}

/// Get the filename for an agent template.
pub fn get_agent_filename(name: &str) -> Option<&'static str> {
    match name {
        "auto" => Some("binnacle-auto.agent.md"),
        "do" => Some("binnacle-do.agent.md"),
        "prd" => Some("binnacle-prd.agent.md"),
        "buddy" => Some("binnacle-buddy.agent.md"),
        "ask" => Some("binnacle-ask.agent.md"),
        "free" => Some("binnacle-free.agent.md"),
        _ => None,
    }
}

/// Get agent metadata by name.
pub fn get_agent_meta(name: &str) -> Option<&'static AgentMeta> {
    match name {
        "auto" => Some(&AUTO_META),
        "do" => Some(&DO_META),
        "prd" => Some(&PRD_META),
        "buddy" => Some(&BUDDY_META),
        "ask" => Some(&ASK_META),
        "free" => Some(&FREE_META),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_auto_has_required_fields() {
        let content = agent_auto();
        assert!(content.contains("name: Binnacle Auto"));
        assert!(content.contains("description:"));
        assert!(content.contains("tools:"));
        assert!(content.contains("bn orient"));
        assert!(content.contains("bn ready"));
        assert!(content.contains("bn goodbye"));
        assert!(content.contains("queued"));
    }

    #[test]
    fn test_agent_do_has_required_fields() {
        let content = agent_do();
        assert!(content.contains("name: Binnacle Do"));
        assert!(content.contains("description:"));
        assert!(content.contains("tools:"));
        assert!(content.contains("bn orient"));
        assert!(content.contains("bn goodbye"));
        assert!(content.contains("{argument}"));
    }

    #[test]
    fn test_agent_prd_has_required_fields() {
        let content = agent_prd();
        assert!(content.contains("name: Binnacle PRD"));
        assert!(content.contains("description:"));
        assert!(content.contains("tools:"));
        assert!(content.contains("bn orient --type planner"));
        assert!(content.contains("bn idea list"));
        assert!(content.contains("bn doc create"));
        assert!(content.contains("Do NOT run `bn goodbye`"));
    }

    #[test]
    fn test_agent_buddy_has_required_fields() {
        let content = agent_buddy();
        assert!(content.contains("name: Binnacle Buddy"));
        assert!(content.contains("description:"));
        assert!(content.contains("tools:"));
        assert!(content.contains("bn orient --type buddy"));
        assert!(content.contains("bn idea create"));
        assert!(content.contains("bn task create"));
        assert!(content.contains("bn bug create"));
        assert!(content.contains("-s"));
    }

    #[test]
    fn test_agent_ask_has_required_fields() {
        let content = agent_ask();
        assert!(content.contains("name: Binnacle Ask"));
        assert!(content.contains("description:"));
        assert!(content.contains("tools:"));
        assert!(content.contains("bn orient --type ask"));
        assert!(content.contains("READ-ONLY"));
        assert!(content.contains("Do NOT run `bn goodbye`"));
    }

    #[test]
    fn test_agent_free_has_required_fields() {
        let content = agent_free();
        assert!(content.contains("name: Binnacle Free"));
        assert!(content.contains("description:"));
        assert!(content.contains("tools:"));
        assert!(content.contains("bn orient"));
        assert!(content.contains("bn ready"));
        assert!(content.contains("bn goodbye"));
    }

    #[test]
    fn test_all_agents_have_valid_yaml_frontmatter() {
        for name in AGENT_NAMES {
            let content = get_agent_content(name).expect("Agent should exist");
            assert!(
                content.starts_with("---\n"),
                "Agent {} should start with YAML frontmatter",
                name
            );
            assert!(
                content.contains("\n---\n"),
                "Agent {} should have closing frontmatter delimiter",
                name
            );
        }
    }

    #[test]
    fn test_get_agent_content_returns_correct_content() {
        assert!(get_agent_content("auto").is_some());
        assert!(get_agent_content("do").is_some());
        assert!(get_agent_content("prd").is_some());
        assert!(get_agent_content("buddy").is_some());
        assert!(get_agent_content("ask").is_some());
        assert!(get_agent_content("free").is_some());
        assert!(get_agent_content("invalid").is_none());
    }

    #[test]
    fn test_get_agent_filename_returns_correct_names() {
        assert_eq!(get_agent_filename("auto"), Some("binnacle-auto.agent.md"));
        assert_eq!(get_agent_filename("do"), Some("binnacle-do.agent.md"));
        assert_eq!(get_agent_filename("prd"), Some("binnacle-prd.agent.md"));
        assert_eq!(get_agent_filename("buddy"), Some("binnacle-buddy.agent.md"));
        assert_eq!(get_agent_filename("ask"), Some("binnacle-ask.agent.md"));
        assert_eq!(get_agent_filename("free"), Some("binnacle-free.agent.md"));
        assert_eq!(get_agent_filename("invalid"), None);
    }

    #[test]
    fn test_worker_agents_have_goodbye() {
        // auto, do, free are worker agents that should have goodbye
        assert!(agent_auto().contains("bn goodbye"));
        assert!(agent_do().contains("bn goodbye"));
        assert!(agent_free().contains("bn goodbye"));
    }

    #[test]
    fn test_non_worker_agents_skip_goodbye() {
        // prd and ask should NOT run goodbye
        assert!(agent_prd().contains("Do NOT run `bn goodbye`"));
        assert!(agent_ask().contains("Do NOT run `bn goodbye`"));
    }

    #[test]
    fn test_get_agent_meta_returns_metadata() {
        let auto = get_agent_meta("auto").unwrap();
        assert_eq!(auto.name, "Binnacle Auto");

        let prd = get_agent_meta("prd").unwrap();
        assert_eq!(prd.name, "Binnacle PRD");

        assert!(get_agent_meta("invalid").is_none());
    }
}
