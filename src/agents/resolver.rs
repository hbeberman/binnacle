//! Layered resolution for agent definitions.
//!
//! Agent definitions are resolved in layers, with later sources overriding earlier ones:
//!
//! 1. **Embedded** (in bn binary) - Default agent definitions from src/agents/embedded.rs
//! 2. **System** (~/.config/binnacle/agents/config.kdl) - Global user customizations
//! 3. **Session** (~/.local/share/binnacle/<hash>/agents/config.kdl) - Per-repo user customizations
//! 4. **Project** (.binnacle/agents/config.kdl) - Repo-specific customizations (committed)
//!
//! The resolver loads agent definitions from each layer and merges them.

use crate::Result;
use crate::agents::definitions::{AGENT_TYPES, AgentDefinition};
use crate::agents::embedded;
use crate::agents::kdl::{AgentOverride, load_overrides_from_file};
use crate::config::ValueSource;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Resolved agent definition with source tracking.
#[derive(Debug, Clone)]
pub struct ResolvedAgent {
    /// The resolved agent definition.
    pub agent: AgentDefinition,
    /// Where the definition came from (highest priority layer that modified it).
    pub source: ValueSource,
    /// Whether tools were merged from multiple layers.
    pub tools_merged: bool,
    /// Whether copilot config was overridden from a non-embedded layer.
    pub copilot_merged: bool,
    /// All sources that contributed to this definition.
    pub sources: Vec<ValueSource>,
}

impl ResolvedAgent {
    /// Create a new resolved agent from embedded source.
    pub fn from_embedded(agent: AgentDefinition) -> Self {
        Self {
            agent,
            source: ValueSource::Default,
            tools_merged: false,
            copilot_merged: false,
            sources: vec![ValueSource::Default],
        }
    }
}

/// Configuration paths for agent resolution.
#[derive(Debug, Clone)]
pub struct AgentPaths {
    /// System-level config: ~/.config/binnacle/agents/config.kdl
    pub system: Option<PathBuf>,
    /// Session-level config: ~/.local/share/binnacle/<hash>/agents/config.kdl
    pub session: Option<PathBuf>,
    /// Project-level config: .binnacle/agents/config.kdl (relative to repo root)
    pub project: Option<PathBuf>,
    /// Repository root (for resolving project paths)
    pub repo_root: Option<PathBuf>,
}

impl AgentPaths {
    /// Create paths for resolution in a given repository.
    pub fn for_repo(repo_root: &Path) -> Self {
        let session_path = crate::storage::compute_session_agents_path(repo_root);
        let system_path = Self::system_config_path();
        let project_path = repo_root.join(".binnacle/agents/config.kdl");

        Self {
            system: system_path,
            session: session_path,
            project: if project_path.exists() {
                Some(project_path)
            } else {
                None
            },
            repo_root: Some(repo_root.to_path_buf()),
        }
    }

    /// Create paths using only system-level config (no session or project).
    pub fn system_only() -> Self {
        Self {
            system: Self::system_config_path(),
            session: None,
            project: None,
            repo_root: None,
        }
    }

    /// Get the system config path (~/.config/binnacle/agents/config.kdl).
    fn system_config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("binnacle").join("agents").join("config.kdl"))
    }
}

impl Default for AgentPaths {
    fn default() -> Self {
        Self::system_only()
    }
}

/// Agent resolver that handles layered resolution.
#[derive(Debug, Default)]
pub struct AgentResolver {
    /// Cached resolved agents.
    cache: HashMap<String, ResolvedAgent>,
    /// Resolution paths.
    paths: AgentPaths,
}

impl AgentResolver {
    /// Create a new agent resolver with default paths (system only).
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            paths: AgentPaths::default(),
        }
    }

    /// Create a resolver for a specific repository with full layered resolution.
    pub fn for_repo(repo_root: &Path) -> Self {
        Self {
            cache: HashMap::new(),
            paths: AgentPaths::for_repo(repo_root),
        }
    }

    /// Create a resolver with custom paths.
    pub fn with_paths(paths: AgentPaths) -> Self {
        Self {
            cache: HashMap::new(),
            paths,
        }
    }

    /// Resolve an agent definition by name.
    ///
    /// Applies layered resolution: embedded -> system -> session -> project.
    /// Each layer can override or extend the previous layers.
    pub fn resolve(&mut self, name: &str) -> Result<Option<ResolvedAgent>> {
        // Check cache first
        if let Some(cached) = self.cache.get(name) {
            return Ok(Some(cached.clone()));
        }

        // Start with embedded agent (base layer)
        let Some(mut agent) = embedded::get_embedded_agent(name) else {
            return Ok(None);
        };

        let mut resolved = ResolvedAgent::from_embedded(agent.clone());

        // Load and apply system overrides
        if let Some(ref system_path) = self.paths.system {
            if let Some(override_def) = self.load_override_for_agent(system_path, name)? {
                let base_path = system_path.parent();
                agent = override_def.apply_to(&agent, base_path);
                resolved.source = ValueSource::System;
                resolved.sources.push(ValueSource::System);
                if !override_def.tools_allow.is_empty() || !override_def.tools_deny.is_empty() {
                    resolved.tools_merged = true;
                }
                if override_def.model.is_some()
                    || override_def.reasoning_effort.is_some()
                    || override_def.show_reasoning.is_some()
                    || override_def.render_markdown.is_some()
                {
                    resolved.copilot_merged = true;
                }
            }
        }

        // Load and apply session overrides
        if let Some(ref session_path) = self.paths.session {
            if let Some(override_def) = self.load_override_for_agent(session_path, name)? {
                let base_path = session_path.parent();
                agent = override_def.apply_to(&agent, base_path);
                resolved.source = ValueSource::Session;
                resolved.sources.push(ValueSource::Session);
                if !override_def.tools_allow.is_empty() || !override_def.tools_deny.is_empty() {
                    resolved.tools_merged = true;
                }
                if override_def.model.is_some()
                    || override_def.reasoning_effort.is_some()
                    || override_def.show_reasoning.is_some()
                    || override_def.render_markdown.is_some()
                {
                    resolved.copilot_merged = true;
                }
            }
        }

        // Load and apply project overrides
        if let Some(ref project_path) = self.paths.project {
            if let Some(override_def) = self.load_override_for_agent(project_path, name)? {
                let base_path = project_path.parent();
                agent = override_def.apply_to(&agent, base_path);
                resolved.source = ValueSource::Project;
                resolved.sources.push(ValueSource::Project);
                if !override_def.tools_allow.is_empty() || !override_def.tools_deny.is_empty() {
                    resolved.tools_merged = true;
                }
                if override_def.model.is_some()
                    || override_def.reasoning_effort.is_some()
                    || override_def.show_reasoning.is_some()
                    || override_def.render_markdown.is_some()
                {
                    resolved.copilot_merged = true;
                }
            }
        }

        resolved.agent = agent;

        // Cache and return
        self.cache.insert(name.to_string(), resolved.clone());
        Ok(Some(resolved))
    }

    /// Load an override for a specific agent from a KDL file.
    fn load_override_for_agent(&self, path: &Path, name: &str) -> Result<Option<AgentOverride>> {
        let overrides = load_overrides_from_file(path)?;
        Ok(overrides.into_iter().find(|o| o.name == name))
    }

    /// Resolve all known agent types.
    pub fn resolve_all(&mut self) -> Result<Vec<ResolvedAgent>> {
        let mut agents = Vec::new();
        for name in AGENT_TYPES {
            if let Some(resolved) = self.resolve(name)? {
                agents.push(resolved);
            }
        }
        Ok(agents)
    }

    /// Clear the resolution cache.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Get the resolved agent for a name without caching.
    pub fn get_uncached(&self, name: &str) -> Option<AgentDefinition> {
        embedded::get_embedded_agent(name)
    }

    /// Check if an agent type is valid.
    pub fn is_valid_agent_type(name: &str) -> bool {
        AGENT_TYPES.contains(&name)
    }

    /// Get the resolution paths being used.
    pub fn paths(&self) -> &AgentPaths {
        &self.paths
    }
}

/// Get a resolved agent definition by name.
///
/// This is a convenience function that creates a resolver and resolves a single agent.
/// Note: This uses system-only resolution. Use AgentResolver::for_repo() for full layered resolution.
pub fn resolve_agent(name: &str) -> Result<Option<ResolvedAgent>> {
    let mut resolver = AgentResolver::new();
    resolver.resolve(name)
}

/// Get all resolved agent definitions.
///
/// This is a convenience function that creates a resolver and resolves all agents.
/// Note: This uses system-only resolution. Use AgentResolver::for_repo() for full layered resolution.
pub fn resolve_all_agents() -> Result<Vec<ResolvedAgent>> {
    let mut resolver = AgentResolver::new();
    resolver.resolve_all()
}

/// Get a resolved agent definition by name for a specific repository.
///
/// This uses full layered resolution: embedded -> system -> session -> project.
pub fn resolve_agent_for_repo(name: &str, repo_root: &Path) -> Result<Option<ResolvedAgent>> {
    let mut resolver = AgentResolver::for_repo(repo_root);
    resolver.resolve(name)
}

/// Get all resolved agent definitions for a specific repository.
///
/// This uses full layered resolution: embedded -> system -> session -> project.
pub fn resolve_all_agents_for_repo(repo_root: &Path) -> Result<Vec<ResolvedAgent>> {
    let mut resolver = AgentResolver::for_repo(repo_root);
    resolver.resolve_all()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::definitions::{
        AGENT_ASK, AGENT_BUDDY, AGENT_DO, AGENT_FREE, AGENT_PRD, AGENT_WORKER, ExecutionMode,
        LifecycleMode,
    };
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_resolve_worker() {
        let resolved = resolve_agent(AGENT_WORKER).unwrap().unwrap();
        assert_eq!(resolved.agent.name, AGENT_WORKER);
        assert_eq!(resolved.agent.execution, ExecutionMode::Container);
        assert_eq!(resolved.agent.lifecycle, LifecycleMode::Stateful);
        assert_eq!(resolved.source, ValueSource::Default);
    }

    #[test]
    fn test_resolve_prd() {
        let resolved = resolve_agent(AGENT_PRD).unwrap().unwrap();
        assert_eq!(resolved.agent.name, AGENT_PRD);
        assert_eq!(resolved.agent.execution, ExecutionMode::Host);
        assert_eq!(resolved.agent.lifecycle, LifecycleMode::Stateless);
    }

    #[test]
    fn test_resolve_unknown() {
        let result = resolve_agent("unknown").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_all() {
        let agents = resolve_all_agents().unwrap();
        assert_eq!(agents.len(), 6);

        let names: Vec<&str> = agents.iter().map(|r| r.agent.name.as_str()).collect();
        assert!(names.contains(&AGENT_WORKER));
        assert!(names.contains(&AGENT_DO));
        assert!(names.contains(&AGENT_PRD));
        assert!(names.contains(&AGENT_BUDDY));
        assert!(names.contains(&AGENT_ASK));
        assert!(names.contains(&AGENT_FREE));
    }

    #[test]
    fn test_resolver_caching() {
        let mut resolver = AgentResolver::new();

        // First resolve - should populate cache
        let first = resolver.resolve(AGENT_WORKER).unwrap().unwrap();
        assert!(resolver.cache.contains_key(AGENT_WORKER));

        // Second resolve - should return cached
        let second = resolver.resolve(AGENT_WORKER).unwrap().unwrap();
        assert_eq!(first.agent.name, second.agent.name);

        // Clear cache
        resolver.clear_cache();
        assert!(!resolver.cache.contains_key(AGENT_WORKER));
    }

    #[test]
    fn test_is_valid_agent_type() {
        assert!(AgentResolver::is_valid_agent_type(AGENT_WORKER));
        assert!(AgentResolver::is_valid_agent_type(AGENT_PRD));
        assert!(AgentResolver::is_valid_agent_type(AGENT_BUDDY));
        assert!(AgentResolver::is_valid_agent_type(AGENT_ASK));
        assert!(AgentResolver::is_valid_agent_type(AGENT_FREE));
        assert!(AgentResolver::is_valid_agent_type(AGENT_DO));
        assert!(!AgentResolver::is_valid_agent_type("unknown"));
        assert!(!AgentResolver::is_valid_agent_type(""));
    }

    #[test]
    fn test_get_uncached() {
        let resolver = AgentResolver::new();
        let agent = resolver.get_uncached(AGENT_WORKER).unwrap();
        assert_eq!(agent.name, AGENT_WORKER);

        // Cache should not be populated
        assert!(resolver.cache.is_empty());
    }

    #[test]
    fn test_resolved_agent_source() {
        // All embedded agents should have Default source
        for resolved in resolve_all_agents().unwrap() {
            assert_eq!(
                resolved.source,
                ValueSource::Default,
                "Agent '{}' should have Default source",
                resolved.agent.name
            );
        }
    }

    #[test]
    fn test_resolve_with_project_override() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        // Create .binnacle/agents/config.kdl with an override
        let agents_dir = repo_root.join(".binnacle/agents");
        std::fs::create_dir_all(&agents_dir).unwrap();

        let config_path = agents_dir.join("config.kdl");
        let mut file = std::fs::File::create(&config_path).unwrap();
        writeln!(
            file,
            r#"
agent "worker" {{
    description "Custom worker from project"
    execution "host"
    tools {{
        allow "custom-tool"
    }}
}}
"#
        )
        .unwrap();

        let mut resolver = AgentResolver::for_repo(repo_root);
        let resolved = resolver.resolve(AGENT_WORKER).unwrap().unwrap();

        // Should have project overrides applied
        assert_eq!(resolved.agent.description, "Custom worker from project");
        assert_eq!(resolved.agent.execution, ExecutionMode::Host);
        assert!(
            resolved
                .agent
                .tools
                .allow
                .contains(&"custom-tool".to_string())
        );
        assert!(resolved.tools_merged);
        assert_eq!(resolved.source, ValueSource::Project);
    }

    #[test]
    fn test_resolve_without_config_files() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        // No config files exist
        let mut resolver = AgentResolver::for_repo(repo_root);
        let resolved = resolver.resolve(AGENT_WORKER).unwrap().unwrap();

        // Should fall back to embedded
        assert_eq!(resolved.agent.name, AGENT_WORKER);
        assert_eq!(resolved.source, ValueSource::Default);
        assert!(!resolved.tools_merged);
    }

    #[test]
    fn test_agent_paths_for_repo() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        let paths = AgentPaths::for_repo(repo_root);

        // System path should be set
        assert!(paths.system.is_some());

        // Session path should be set
        assert!(paths.session.is_some());

        // Project path should not be set (no .binnacle/agents exists)
        assert!(paths.project.is_none());

        // Repo root should be set
        assert!(paths.repo_root.is_some());
    }

    #[test]
    fn test_agent_paths_system_only() {
        let paths = AgentPaths::system_only();

        assert!(paths.system.is_some());
        assert!(paths.session.is_none());
        assert!(paths.project.is_none());
        assert!(paths.repo_root.is_none());
    }

    #[test]
    fn test_resolve_all_for_repo() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        let agents = resolve_all_agents_for_repo(repo_root).unwrap();

        // Should have all 6 agents
        assert_eq!(agents.len(), 6);
    }

    #[test]
    fn test_layered_override_merging() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        // Create .binnacle/agents/config.kdl
        let agents_dir = repo_root.join(".binnacle/agents");
        std::fs::create_dir_all(&agents_dir).unwrap();

        let config_path = agents_dir.join("config.kdl");
        let mut file = std::fs::File::create(&config_path).unwrap();
        // Only override description, keep other fields from embedded
        writeln!(
            file,
            r#"
agent "worker" {{
    description "Partially overridden worker"
}}
"#
        )
        .unwrap();

        let mut resolver = AgentResolver::for_repo(repo_root);
        let resolved = resolver.resolve(AGENT_WORKER).unwrap().unwrap();

        // Description should be overridden
        assert_eq!(resolved.agent.description, "Partially overridden worker");

        // Other fields should remain from embedded
        assert_eq!(resolved.agent.execution, ExecutionMode::Container);
        assert_eq!(resolved.agent.lifecycle, LifecycleMode::Stateful);

        // Tools should be from embedded (no override specified)
        assert!(!resolved.tools_merged);

        // Copilot config should be from embedded (no override specified)
        assert!(!resolved.copilot_merged);
    }

    #[test]
    fn test_copilot_config_merge_from_project() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        // Create .binnacle/agents/config.kdl with copilot overrides
        let agents_dir = repo_root.join(".binnacle/agents");
        std::fs::create_dir_all(&agents_dir).unwrap();

        let config_path = agents_dir.join("config.kdl");
        let mut file = std::fs::File::create(&config_path).unwrap();
        writeln!(
            file,
            r#"
agent "worker" {{
    model "claude-sonnet-4"
    reasoning-effort "medium"
}}
"#
        )
        .unwrap();

        let mut resolver = AgentResolver::for_repo(repo_root);
        let resolved = resolver.resolve(AGENT_WORKER).unwrap().unwrap();

        // Copilot config should be overridden
        assert_eq!(resolved.agent.copilot.model, "claude-sonnet-4");
        assert_eq!(resolved.agent.copilot.reasoning_effort, "medium");
        // Unset fields preserve defaults
        assert!(resolved.agent.copilot.show_reasoning);
        assert!(resolved.agent.copilot.render_markdown);
        assert!(resolved.copilot_merged);
        assert_eq!(resolved.source, ValueSource::Project);
    }

    #[test]
    fn test_copilot_config_partial_override() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        // Override only render_markdown
        let agents_dir = repo_root.join(".binnacle/agents");
        std::fs::create_dir_all(&agents_dir).unwrap();

        let config_path = agents_dir.join("config.kdl");
        let mut file = std::fs::File::create(&config_path).unwrap();
        writeln!(
            file,
            r#"
agent "worker" {{
    render-markdown #false
}}
"#
        )
        .unwrap();

        let mut resolver = AgentResolver::for_repo(repo_root);
        let resolved = resolver.resolve(AGENT_WORKER).unwrap().unwrap();

        // Only render_markdown should be overridden
        assert_eq!(resolved.agent.copilot.model, "claude-opus-4.6");
        assert_eq!(resolved.agent.copilot.reasoning_effort, "high");
        assert!(resolved.agent.copilot.show_reasoning);
        assert!(!resolved.agent.copilot.render_markdown);
        assert!(resolved.copilot_merged);
    }

    #[test]
    fn test_copilot_not_merged_without_override() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        // No config files
        let mut resolver = AgentResolver::for_repo(repo_root);
        let resolved = resolver.resolve(AGENT_WORKER).unwrap().unwrap();

        assert!(!resolved.copilot_merged);
        // Should have defaults
        assert_eq!(resolved.agent.copilot.model, "claude-opus-4.6");
        assert_eq!(resolved.agent.copilot.reasoning_effort, "high");
        assert!(resolved.agent.copilot.show_reasoning);
        assert!(resolved.agent.copilot.render_markdown);
    }

    #[test]
    fn test_copilot_config_multi_layer_override() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        // System layer: override model only
        let system_dir = temp_dir.path().join("system_config");
        std::fs::create_dir_all(&system_dir).unwrap();
        let system_config = system_dir.join("config.kdl");
        let mut f = std::fs::File::create(&system_config).unwrap();
        writeln!(
            f,
            r#"
agent "worker" {{
    model "claude-sonnet-4"
}}
"#
        )
        .unwrap();

        // Session layer: override reasoning_effort only
        let session_dir = temp_dir.path().join("session_config");
        std::fs::create_dir_all(&session_dir).unwrap();
        let session_config = session_dir.join("config.kdl");
        let mut f = std::fs::File::create(&session_config).unwrap();
        writeln!(
            f,
            r#"
agent "worker" {{
    reasoning-effort "low"
}}
"#
        )
        .unwrap();

        // Project layer: override render-markdown only
        let project_dir = repo_root.join(".binnacle/agents");
        std::fs::create_dir_all(&project_dir).unwrap();
        let project_config = project_dir.join("config.kdl");
        let mut f = std::fs::File::create(&project_config).unwrap();
        writeln!(
            f,
            r#"
agent "worker" {{
    render-markdown #false
}}
"#
        )
        .unwrap();

        let paths = AgentPaths {
            system: Some(system_config),
            session: Some(session_config),
            project: Some(project_config),
            repo_root: Some(repo_root.to_path_buf()),
        };

        let mut resolver = AgentResolver::with_paths(paths);
        let resolved = resolver.resolve(AGENT_WORKER).unwrap().unwrap();

        // System set model
        assert_eq!(resolved.agent.copilot.model, "claude-sonnet-4");
        // Session set reasoning_effort (overriding embedded "high")
        assert_eq!(resolved.agent.copilot.reasoning_effort, "low");
        // Embedded show_reasoning preserved (no layer overrode it)
        assert!(resolved.agent.copilot.show_reasoning);
        // Project set render_markdown to false
        assert!(!resolved.agent.copilot.render_markdown);
        // Source should be Project (last layer that modified it)
        assert_eq!(resolved.source, ValueSource::Project);
        assert!(resolved.copilot_merged);
    }
}
