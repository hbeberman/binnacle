//! Unified agent definitions module.
//!
//! This module provides a declarative model for agent definitions that enables
//! consistent agent behavior across all execution contexts (CLI, containers, VSCode MCP).
//!
//! ## Agent Types
//!
//! | Type   | Execution | Lifecycle  | Description                           |
//! |--------|-----------|------------|---------------------------------------|
//! | worker | container | stateful   | Picks from bn ready, works autonomously |
//! | do     | host      | stateful   | Works on user-specified task          |
//! | prd    | host      | stateless  | Converts ideas to PRDs (planner)      |
//! | buddy  | host      | stateful   | Creates bugs/tasks/ideas              |
//! | ask    | host      | stateless  | Read-only codebase exploration        |
//! | free   | host      | stateful   | Full access, user-directed            |
//!
//! ## Resolution Order
//!
//! Agent definitions are resolved in layers (later sources override earlier):
//!
//! 1. **Embedded** (in bn binary) - Default agent definitions
//! 2. **System** (~/.config/binnacle/agents/config.kdl) - Global user customizations
//! 3. **Session** (~/.local/share/binnacle/<hash>/agents/config.kdl) - Per-repo customizations
//! 4. **Project** (.binnacle/agents/config.kdl) - Repo-specific customizations (committed)
//!
//! ## KDL Configuration
//!
//! Agent definitions can be customized using KDL config files:
//!
//! ```kdl
//! agent "worker" {
//!     description "Custom worker description"
//!     execution "container"  // "container" | "host"
//!     lifecycle "stateful"   // "stateful" | "stateless"
//!     
//!     tools {
//!         allow "shell(npm:*)"
//!         deny "shell(rm:*)"
//!     }
//!     
//!     prompt-file "worker/custom-prompt.md"  // Optional custom prompt
//! }
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use binnacle::agents::{resolve_agent, resolve_all_agents, AgentDefinition};
//!
//! // Get a single agent
//! if let Some(resolved) = resolve_agent("worker")? {
//!     println!("Worker: {}", resolved.agent.description);
//!     println!("Source: {}", resolved.source);
//! }
//!
//! // Get all agents
//! for resolved in resolve_all_agents()? {
//!     println!("{}", resolved.agent.summary());
//! }
//! ```

pub mod definitions;
pub mod embedded;
pub mod kdl;
pub mod resolver;

// Re-export commonly used types
pub use definitions::{
    AGENT_ASK, AGENT_BUDDY, AGENT_DO, AGENT_FREE, AGENT_PRD, AGENT_TYPES, AGENT_WORKER,
    AgentDefinition, CopilotConfig, ExecutionMode, LifecycleMode, ToolPermissions,
};
pub use embedded::{do_prompt, get_all_embedded_agents, get_embedded_agent};
pub use kdl::{AgentOverride, load_overrides_from_file, parse_agent_overrides};
pub use resolver::{
    AgentPaths, AgentResolver, ResolvedAgent, resolve_agent, resolve_agent_for_repo,
    resolve_all_agents, resolve_all_agents_for_repo,
};
