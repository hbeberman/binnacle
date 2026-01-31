//! Configuration and state management for Binnacle.
//!
//! This module defines KDL schemas for two distinct files:
//!
//! ## config.kdl - User preferences (safe to sync across machines)
//!
//! Located at:
//! - System: `~/.config/binnacle/config.kdl`
//! - Session: `~/.local/share/binnacle/<repo-hash>/config.kdl`
//!
//! Contains:
//! - `editor` - Preferred editor command
//! - `output-format` - "json" or "human"
//! - `default-priority` - Default task priority (0-4)
//!
//! ## state.kdl - Runtime state (machine-specific, contains secrets)
//!
//! Located at:
//! - System: `~/.local/share/binnacle/state.kdl`
//! - Session: `~/.local/share/binnacle/<repo-hash>/state.kdl`
//!
//! Contains:
//! - `github-token` - GitHub PAT for API access
//! - `token-validated-at` - ISO 8601 timestamp of last token validation
//! - `last-copilot-version` - Last known Copilot CLI version
//! - `serve` block - Session server state (pid, port, host, etc.)
//!
//! ## Security
//!
//! **CRITICAL**: `state.kdl` MUST be created with 0600 permissions (owner read/write only)
//! because it contains secrets like GitHub tokens.
//!
//! ## Precedence
//!
//! For tokens: env var > session state > system state
//! For preferences: CLI flag > session config > system config > defaults
//!
//! Use the [`resolver`] module for unified precedence resolution.

pub mod resolver;
pub mod schema;

pub use resolver::{
    COPILOT_GITHUB_TOKEN_ENV, ConfigOverrides, Resolved, ResolvedConfig, ResolvedSettings,
    ResolvedState, ValueSource, resolve_config, resolve_state, resolve_state_with_override,
};
pub use schema::{BinnacleConfig, BinnacleState, OutputFormat, ServeState};
#[cfg(unix)]
pub use schema::{CONFIG_FILE_MODE, STATE_FILE_MODE};
