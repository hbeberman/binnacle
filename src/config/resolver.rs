//! Unified precedence resolution for configuration and state.
//!
//! This module provides a single entry point for resolving both configuration
//! preferences and runtime state (including tokens) with proper precedence.
//!
//! ## Token Precedence (highest to lowest)
//!
//! 1. `COPILOT_GITHUB_TOKEN` environment variable
//! 2. Session state.kdl (`~/.local/share/binnacle/<repo-hash>/state.kdl`)
//! 3. System state.kdl (`~/.local/share/binnacle/state.kdl`)
//!
//! ## Config Precedence (highest to lowest)
//!
//! 1. CLI flags (passed at runtime)
//! 2. Session config.kdl (`~/.local/share/binnacle/<repo-hash>/config.kdl`)
//! 3. System config.kdl (`~/.config/binnacle/config.kdl`)
//! 4. Built-in defaults

use crate::Result;
use crate::config::OutputFormat;
use crate::storage::Storage;

/// Environment variable name for GitHub token override.
pub const COPILOT_GITHUB_TOKEN_ENV: &str = "COPILOT_GITHUB_TOKEN";

/// Tracks where a resolved value came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueSource {
    /// Value from environment variable
    EnvVar(String),
    /// Value from session-level config/state
    Session,
    /// Value from system-level config/state
    System,
    /// Value from CLI flag
    CliFlag,
    /// Built-in default value
    Default,
    /// Value from legacy location (config.kdl instead of state.kdl) - deprecated
    LegacyConfig(String),
}

impl std::fmt::Display for ValueSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValueSource::EnvVar(name) => write!(f, "env:{}", name),
            ValueSource::Session => write!(f, "session"),
            ValueSource::System => write!(f, "system"),
            ValueSource::CliFlag => write!(f, "cli"),
            ValueSource::Default => write!(f, "default"),
            ValueSource::LegacyConfig(path) => write!(f, "legacy-config:{}", path),
        }
    }
}

/// A resolved value with its source.
#[derive(Debug, Clone)]
pub struct Resolved<T> {
    /// The resolved value
    pub value: T,
    /// Where the value came from
    pub source: ValueSource,
}

impl<T> Resolved<T> {
    /// Create a new resolved value.
    pub fn new(value: T, source: ValueSource) -> Self {
        Self { value, source }
    }
}

/// Fully resolved configuration with source tracking.
#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    /// Preferred editor command
    pub editor: Option<Resolved<String>>,
    /// Output format preference
    pub output_format: Resolved<OutputFormat>,
    /// Default priority for new tasks
    pub default_priority: Resolved<u8>,
}

impl Default for ResolvedConfig {
    fn default() -> Self {
        Self {
            editor: None,
            output_format: Resolved::new(OutputFormat::Json, ValueSource::Default),
            default_priority: Resolved::new(2, ValueSource::Default),
        }
    }
}

impl ResolvedConfig {
    /// Get the editor value, if set.
    pub fn editor(&self) -> Option<&str> {
        self.editor.as_ref().map(|r| r.value.as_str())
    }

    /// Get the output format value.
    pub fn output_format(&self) -> &OutputFormat {
        &self.output_format.value
    }

    /// Get the default priority value.
    pub fn default_priority(&self) -> u8 {
        self.default_priority.value
    }
}

/// Fully resolved state with source tracking.
#[derive(Debug, Clone, Default)]
pub struct ResolvedState {
    /// GitHub token for API access
    pub github_token: Option<Resolved<String>>,
    /// Whether the token came from an environment variable
    pub token_from_env: bool,
    /// Deprecation warnings (e.g., token found in legacy location)
    pub deprecation_warnings: Vec<String>,
}

impl ResolvedState {
    /// Get the token value, if set.
    pub fn token(&self) -> Option<&str> {
        self.github_token.as_ref().map(|r| r.value.as_str())
    }

    /// Get the masked token for display purposes.
    pub fn masked_token(&self) -> Option<String> {
        self.github_token.as_ref().map(|r| {
            let token = &r.value;
            if token.len() <= 12 {
                format!("{}...", &token[..4.min(token.len())])
            } else {
                format!("{}...{}", &token[..4], &token[token.len() - 4..])
            }
        })
    }

    /// Check if a token is available.
    pub fn has_token(&self) -> bool {
        self.github_token.is_some()
    }

    /// Get the source of the token, if set.
    pub fn token_source(&self) -> Option<&ValueSource> {
        self.github_token.as_ref().map(|r| &r.source)
    }

    /// Check if the token is from a deprecated location.
    pub fn is_token_from_legacy_location(&self) -> bool {
        matches!(self.token_source(), Some(ValueSource::LegacyConfig(_)))
    }

    /// Get deprecation warnings, if any.
    pub fn get_deprecation_warnings(&self) -> &[String] {
        &self.deprecation_warnings
    }
}

/// CLI overrides for configuration resolution.
#[derive(Debug, Clone, Default)]
pub struct ConfigOverrides {
    /// Editor override from CLI flag
    pub editor: Option<String>,
    /// Output format override from CLI flag
    pub output_format: Option<OutputFormat>,
    /// Default priority override from CLI flag
    pub default_priority: Option<u8>,
}

impl ConfigOverrides {
    /// Create empty overrides.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set editor override.
    pub fn with_editor(mut self, editor: impl Into<String>) -> Self {
        self.editor = Some(editor.into());
        self
    }

    /// Set output format override.
    pub fn with_output_format(mut self, format: OutputFormat) -> Self {
        self.output_format = Some(format);
        self
    }

    /// Set default priority override.
    pub fn with_default_priority(mut self, priority: u8) -> Self {
        self.default_priority = Some(priority);
        self
    }
}

/// Resolve configuration with full precedence chain.
///
/// Precedence (highest to lowest):
/// 1. CLI flags (from `overrides`)
/// 2. Session config.kdl
/// 3. System config.kdl
/// 4. Built-in defaults
pub fn resolve_config(storage: &Storage, overrides: &ConfigOverrides) -> Result<ResolvedConfig> {
    let mut result = ResolvedConfig::default();

    // Load system config (lowest precedence among file-based)
    let system_config = Storage::read_system_binnacle_config()?;

    // Load session config (higher precedence than system)
    let session_config = storage.read_binnacle_config()?;

    // Resolve editor
    if let Some(ref editor) = overrides.editor {
        result.editor = Some(Resolved::new(editor.clone(), ValueSource::CliFlag));
    } else if let Some(ref editor) = session_config.editor {
        result.editor = Some(Resolved::new(editor.clone(), ValueSource::Session));
    } else if let Some(ref editor) = system_config.editor {
        result.editor = Some(Resolved::new(editor.clone(), ValueSource::System));
    }
    // else: remains None (no default for editor)

    // Resolve output_format
    if let Some(ref format) = overrides.output_format {
        result.output_format = Resolved::new(format.clone(), ValueSource::CliFlag);
    } else if let Some(ref format) = session_config.output_format {
        result.output_format = Resolved::new(format.clone(), ValueSource::Session);
    } else if let Some(ref format) = system_config.output_format {
        result.output_format = Resolved::new(format.clone(), ValueSource::System);
    }
    // else: remains Default (Json)

    // Resolve default_priority
    if let Some(priority) = overrides.default_priority {
        result.default_priority = Resolved::new(priority, ValueSource::CliFlag);
    } else if let Some(priority) = session_config.default_priority {
        result.default_priority = Resolved::new(priority, ValueSource::Session);
    } else if let Some(priority) = system_config.default_priority {
        result.default_priority = Resolved::new(priority, ValueSource::System);
    }
    // else: remains Default (2)

    Ok(result)
}

/// Resolve state with full precedence chain.
///
/// Token precedence (highest to lowest):
/// 1. `COPILOT_GITHUB_TOKEN` environment variable
/// 2. Session state.kdl
/// 3. System state.kdl
/// 4. (Legacy fallback) Session config.kdl - DEPRECATED
/// 5. (Legacy fallback) System config.kdl - DEPRECATED
///
/// Note: If a token is found in config.kdl, a deprecation warning is emitted.
/// Run `bn system migrate-config` to move tokens to state.kdl.
pub fn resolve_state(storage: &Storage) -> Result<ResolvedState> {
    use crate::config::schema::get_legacy_token_from_config;

    let mut result = ResolvedState::default();

    // Check environment variable first (highest precedence)
    if let Ok(token) = std::env::var(COPILOT_GITHUB_TOKEN_ENV) {
        if !token.is_empty() {
            result.github_token = Some(Resolved::new(
                token,
                ValueSource::EnvVar(COPILOT_GITHUB_TOKEN_ENV.to_string()),
            ));
            result.token_from_env = true;
            return Ok(result);
        }
    }

    // Load system state (lowest file-based precedence)
    let system_state = Storage::read_system_binnacle_state()?;

    // Load session state (higher precedence than system)
    let session_state = storage.read_binnacle_state()?;

    // Resolve token from state.kdl files (correct location)
    if let Some(ref token) = session_state.github_token {
        result.github_token = Some(Resolved::new(token.clone(), ValueSource::Session));
        return Ok(result);
    }
    if let Some(ref token) = system_state.github_token {
        result.github_token = Some(Resolved::new(token.clone(), ValueSource::System));
        return Ok(result);
    }

    // Legacy fallback: check config.kdl files (deprecated location)
    // Session config.kdl
    let session_config_doc = storage.read_config_kdl()?;
    if let Some(token) = get_legacy_token_from_config(&session_config_doc) {
        let config_path = storage.config_kdl_path();
        result.github_token = Some(Resolved::new(
            token,
            ValueSource::LegacyConfig(config_path.display().to_string()),
        ));
        result.deprecation_warnings.push(format!(
            "Token found in deprecated location: {}. Run 'bn system migrate-config' to move it to state.kdl.",
            config_path.display()
        ));
        return Ok(result);
    }

    // System config.kdl
    let system_config_doc = Storage::read_system_config_kdl()?;
    if let Some(token) = get_legacy_token_from_config(&system_config_doc) {
        let config_path = Storage::system_config_kdl_path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "~/.config/binnacle/config.kdl".to_string());
        result.github_token = Some(Resolved::new(
            token,
            ValueSource::LegacyConfig(config_path.clone()),
        ));
        result.deprecation_warnings.push(format!(
            "Token found in deprecated location: {}. Run 'bn system migrate-config' to move it to state.kdl.",
            config_path
        ));
        return Ok(result);
    }

    // No token found anywhere
    Ok(result)
}

/// Resolve state with a specific token value to check (for testing or explicit override).
///
/// If `token_override` is provided, it takes highest precedence.
pub fn resolve_state_with_override(
    storage: &Storage,
    token_override: Option<&str>,
) -> Result<ResolvedState> {
    if let Some(token) = token_override {
        return Ok(ResolvedState {
            github_token: Some(Resolved::new(token.to_string(), ValueSource::CliFlag)),
            token_from_env: false,
            deprecation_warnings: Vec::new(),
        });
    }
    resolve_state(storage)
}

/// Combined resolver for both config and state.
#[derive(Debug, Clone)]
pub struct ResolvedSettings {
    /// Resolved configuration
    pub config: ResolvedConfig,
    /// Resolved state
    pub state: ResolvedState,
}

impl ResolvedSettings {
    /// Resolve all settings with the given overrides.
    pub fn resolve(storage: &Storage, config_overrides: &ConfigOverrides) -> Result<Self> {
        Ok(Self {
            config: resolve_config(storage, config_overrides)?,
            state: resolve_state(storage)?,
        })
    }

    /// Resolve with no CLI overrides.
    pub fn resolve_defaults(storage: &Storage) -> Result<Self> {
        Self::resolve(storage, &ConfigOverrides::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BinnacleConfig, BinnacleState};
    use crate::test_utils::TestEnv;
    use tempfile::TempDir;

    fn create_test_storage() -> (TestEnv, Storage) {
        let env = TestEnv::new();
        let storage = env.init_storage();
        (env, storage)
    }

    // ==================== ValueSource Tests ====================

    #[test]
    fn test_value_source_display() {
        assert_eq!(
            format!("{}", ValueSource::EnvVar("FOO".to_string())),
            "env:FOO"
        );
        assert_eq!(format!("{}", ValueSource::Session), "session");
        assert_eq!(format!("{}", ValueSource::System), "system");
        assert_eq!(format!("{}", ValueSource::CliFlag), "cli");
        assert_eq!(format!("{}", ValueSource::Default), "default");
        assert_eq!(
            format!(
                "{}",
                ValueSource::LegacyConfig("/path/to/config.kdl".to_string())
            ),
            "legacy-config:/path/to/config.kdl"
        );
    }

    // ==================== Config Resolution Tests ====================

    #[test]
    fn test_resolve_config_defaults() {
        let (_temp_dir, storage) = create_test_storage();

        let config = resolve_config(&storage, &ConfigOverrides::default()).unwrap();

        assert!(config.editor.is_none());
        assert_eq!(*config.output_format(), OutputFormat::Json);
        assert_eq!(config.output_format.source, ValueSource::Default);
        assert_eq!(config.default_priority(), 2);
        assert_eq!(config.default_priority.source, ValueSource::Default);
    }

    #[test]
    fn test_resolve_config_from_session() {
        let (_temp_dir, storage) = create_test_storage();

        // Write session config
        let session_config = BinnacleConfig {
            editor: Some("nvim".to_string()),
            output_format: Some(OutputFormat::Human),
            default_priority: Some(1),
        };
        storage.write_binnacle_config(&session_config).unwrap();

        let config = resolve_config(&storage, &ConfigOverrides::default()).unwrap();

        assert_eq!(config.editor().unwrap(), "nvim");
        assert_eq!(config.editor.as_ref().unwrap().source, ValueSource::Session);
        assert_eq!(*config.output_format(), OutputFormat::Human);
        assert_eq!(config.output_format.source, ValueSource::Session);
        assert_eq!(config.default_priority(), 1);
        assert_eq!(config.default_priority.source, ValueSource::Session);
    }

    #[test]
    fn test_resolve_config_cli_overrides_session() {
        let (_temp_dir, storage) = create_test_storage();

        // Write session config
        let session_config = BinnacleConfig {
            editor: Some("vim".to_string()),
            output_format: Some(OutputFormat::Json),
            default_priority: Some(3),
        };
        storage.write_binnacle_config(&session_config).unwrap();

        // Apply CLI overrides
        let overrides = ConfigOverrides::new()
            .with_editor("code")
            .with_output_format(OutputFormat::Human)
            .with_default_priority(0);

        let config = resolve_config(&storage, &overrides).unwrap();

        // CLI should win
        assert_eq!(config.editor().unwrap(), "code");
        assert_eq!(config.editor.as_ref().unwrap().source, ValueSource::CliFlag);
        assert_eq!(*config.output_format(), OutputFormat::Human);
        assert_eq!(config.output_format.source, ValueSource::CliFlag);
        assert_eq!(config.default_priority(), 0);
        assert_eq!(config.default_priority.source, ValueSource::CliFlag);
    }

    #[test]
    fn test_resolve_config_session_overrides_system() {
        let (_temp_dir, storage) = create_test_storage();

        // Create a separate "system" config directory
        let system_config_dir = TempDir::new().unwrap();

        // Set BN_CONFIG_DIR to point to our test system config
        // SAFETY: We're in a test environment and this test should run serially
        unsafe { std::env::set_var("BN_CONFIG_DIR", system_config_dir.path()) };

        // Write system config
        let system_config = BinnacleConfig {
            editor: Some("vim".to_string()),
            output_format: Some(OutputFormat::Json),
            default_priority: Some(3),
        };
        Storage::write_system_binnacle_config(&system_config).unwrap();

        // Write session config (partial override)
        let session_config = BinnacleConfig {
            editor: Some("nvim".to_string()), // Override
            output_format: None,              // Don't override
            default_priority: None,           // Don't override
        };
        storage.write_binnacle_config(&session_config).unwrap();

        let config = resolve_config(&storage, &ConfigOverrides::default()).unwrap();

        // Session wins for editor
        assert_eq!(config.editor().unwrap(), "nvim");
        assert_eq!(config.editor.as_ref().unwrap().source, ValueSource::Session);

        // System wins for output_format and default_priority
        assert_eq!(*config.output_format(), OutputFormat::Json);
        assert_eq!(config.output_format.source, ValueSource::System);
        assert_eq!(config.default_priority(), 3);
        assert_eq!(config.default_priority.source, ValueSource::System);

        // Clean up
        unsafe { std::env::remove_var("BN_CONFIG_DIR") };
    }

    // ==================== State Resolution Tests ====================

    #[test]
    fn test_resolve_state_no_token() {
        let (_temp_dir, storage) = create_test_storage();

        // Ensure env var is not set
        unsafe { std::env::remove_var(COPILOT_GITHUB_TOKEN_ENV) };

        let state = resolve_state(&storage).unwrap();

        assert!(!state.has_token());
        assert!(state.token().is_none());
        assert!(!state.token_from_env);
    }

    #[test]
    fn test_resolve_state_from_session() {
        let (_temp_dir, storage) = create_test_storage();

        // Ensure env var is not set
        unsafe { std::env::remove_var(COPILOT_GITHUB_TOKEN_ENV) };

        // Write session state
        let session_state = BinnacleState {
            github_token: Some("ghp_session_token_1234".to_string()),
            ..Default::default()
        };
        storage.write_binnacle_state(&session_state).unwrap();

        let state = resolve_state(&storage).unwrap();

        assert!(state.has_token());
        assert_eq!(state.token().unwrap(), "ghp_session_token_1234");
        assert_eq!(state.token_source().unwrap(), &ValueSource::Session);
        assert!(!state.token_from_env);
    }

    #[test]
    fn test_resolve_state_env_overrides_session() {
        let (_temp_dir, storage) = create_test_storage();

        // Write session state with a token
        let session_state = BinnacleState {
            github_token: Some("ghp_session_token".to_string()),
            ..Default::default()
        };
        storage.write_binnacle_state(&session_state).unwrap();

        // Set env var (should override)
        unsafe { std::env::set_var(COPILOT_GITHUB_TOKEN_ENV, "ghp_env_token_override") };

        let state = resolve_state(&storage).unwrap();

        // Env var should win
        assert!(state.has_token());
        assert_eq!(state.token().unwrap(), "ghp_env_token_override");
        assert!(state.token_from_env);
        assert_eq!(
            state.token_source().unwrap(),
            &ValueSource::EnvVar(COPILOT_GITHUB_TOKEN_ENV.to_string())
        );

        // Clean up
        unsafe { std::env::remove_var(COPILOT_GITHUB_TOKEN_ENV) };
    }

    #[test]
    fn test_resolve_state_session_overrides_system() {
        let (_temp_dir, storage) = create_test_storage();

        // Ensure env var is not set
        unsafe { std::env::remove_var(COPILOT_GITHUB_TOKEN_ENV) };

        // Create a separate "system" data directory
        let system_data_dir = TempDir::new().unwrap();

        // Set BN_DATA_DIR to point to our test system state
        unsafe { std::env::set_var("BN_DATA_DIR", system_data_dir.path()) };

        // Write system state
        let system_state = BinnacleState {
            github_token: Some("ghp_system_token".to_string()),
            ..Default::default()
        };
        Storage::write_system_binnacle_state(&system_state).unwrap();

        // Write session state (override)
        let session_state = BinnacleState {
            github_token: Some("ghp_session_token".to_string()),
            ..Default::default()
        };
        storage.write_binnacle_state(&session_state).unwrap();

        let state = resolve_state(&storage).unwrap();

        // Session should win
        assert!(state.has_token());
        assert_eq!(state.token().unwrap(), "ghp_session_token");
        assert_eq!(state.token_source().unwrap(), &ValueSource::Session);
        assert!(!state.token_from_env);

        // Clean up
        unsafe { std::env::remove_var("BN_DATA_DIR") };
    }

    #[test]
    fn test_resolve_state_with_override() {
        let (_temp_dir, storage) = create_test_storage();

        // Write session state
        let session_state = BinnacleState {
            github_token: Some("ghp_session_token".to_string()),
            ..Default::default()
        };
        storage.write_binnacle_state(&session_state).unwrap();

        // Resolve with explicit override
        let state = resolve_state_with_override(&storage, Some("ghp_explicit_override")).unwrap();

        // Override should win
        assert!(state.has_token());
        assert_eq!(state.token().unwrap(), "ghp_explicit_override");
        assert_eq!(state.token_source().unwrap(), &ValueSource::CliFlag);
        assert!(!state.token_from_env);
    }

    #[test]
    fn test_resolve_state_masked_token() {
        let (_temp_dir, storage) = create_test_storage();

        // Ensure env var is not set
        unsafe { std::env::remove_var(COPILOT_GITHUB_TOKEN_ENV) };

        let session_state = BinnacleState {
            github_token: Some("ghp_xxxxxxxxxxxxxxxxxxxx".to_string()),
            ..Default::default()
        };
        storage.write_binnacle_state(&session_state).unwrap();

        let state = resolve_state(&storage).unwrap();
        let masked = state.masked_token().unwrap();

        assert_eq!(masked, "ghp_...xxxx");
    }

    // ==================== Combined Resolution Tests ====================

    #[test]
    fn test_resolved_settings() {
        let (_temp_dir, storage) = create_test_storage();

        // Ensure env var is not set
        unsafe { std::env::remove_var(COPILOT_GITHUB_TOKEN_ENV) };

        // Write session config and state
        let config = BinnacleConfig {
            editor: Some("nvim".to_string()),
            output_format: Some(OutputFormat::Human),
            default_priority: Some(1),
        };
        storage.write_binnacle_config(&config).unwrap();

        let state = BinnacleState {
            github_token: Some("ghp_test_token".to_string()),
            ..Default::default()
        };
        storage.write_binnacle_state(&state).unwrap();

        let settings = ResolvedSettings::resolve_defaults(&storage).unwrap();

        assert_eq!(settings.config.editor().unwrap(), "nvim");
        assert_eq!(*settings.config.output_format(), OutputFormat::Human);
        assert_eq!(settings.config.default_priority(), 1);
        assert_eq!(settings.state.token().unwrap(), "ghp_test_token");
    }

    // ==================== Legacy Token Detection Tests ====================

    #[test]
    fn test_resolve_state_legacy_token_in_session_config() {
        use kdl::{KdlDocument, KdlEntry, KdlNode, KdlValue};

        let (_temp_dir, storage) = create_test_storage();

        // Ensure env var is not set
        unsafe { std::env::remove_var(COPILOT_GITHUB_TOKEN_ENV) };

        // Write a token to config.kdl (legacy/wrong location)
        let mut doc = KdlDocument::new();
        let mut node = KdlNode::new("github-token");
        node.push(KdlEntry::new(KdlValue::String(
            "ghp_legacy_session_token".to_string(),
        )));
        doc.nodes_mut().push(node);
        storage.write_config_kdl(&doc).unwrap();

        let state = resolve_state(&storage).unwrap();

        // Token should be found
        assert!(state.has_token());
        assert_eq!(state.token().unwrap(), "ghp_legacy_session_token");

        // Should be marked as from legacy location
        assert!(state.is_token_from_legacy_location());
        assert!(matches!(
            state.token_source().unwrap(),
            ValueSource::LegacyConfig(_)
        ));

        // Should have deprecation warning
        assert!(!state.deprecation_warnings.is_empty());
        assert!(state.deprecation_warnings[0].contains("deprecated"));
    }

    #[test]
    fn test_resolve_state_prefers_state_kdl_over_legacy_config() {
        use kdl::{KdlDocument, KdlEntry, KdlNode, KdlValue};

        let (_temp_dir, storage) = create_test_storage();

        // Ensure env var is not set
        unsafe { std::env::remove_var(COPILOT_GITHUB_TOKEN_ENV) };

        // Write a token to config.kdl (legacy/wrong location)
        let mut doc = KdlDocument::new();
        let mut node = KdlNode::new("github-token");
        node.push(KdlEntry::new(KdlValue::String(
            "ghp_legacy_token".to_string(),
        )));
        doc.nodes_mut().push(node);
        storage.write_config_kdl(&doc).unwrap();

        // Also write a token to state.kdl (correct location)
        let state_data = BinnacleState {
            github_token: Some("ghp_correct_token".to_string()),
            ..Default::default()
        };
        storage.write_binnacle_state(&state_data).unwrap();

        let state = resolve_state(&storage).unwrap();

        // Should prefer the token from state.kdl
        assert!(state.has_token());
        assert_eq!(state.token().unwrap(), "ghp_correct_token");

        // Should NOT be marked as legacy
        assert!(!state.is_token_from_legacy_location());
        assert_eq!(state.token_source().unwrap(), &ValueSource::Session);

        // Should NOT have deprecation warning
        assert!(state.deprecation_warnings.is_empty());
    }

    #[test]
    fn test_resolve_state_no_legacy_warning_when_token_in_correct_location() {
        let (_temp_dir, storage) = create_test_storage();

        // Ensure env var is not set
        unsafe { std::env::remove_var(COPILOT_GITHUB_TOKEN_ENV) };

        // Write token to state.kdl (correct location)
        let state_data = BinnacleState {
            github_token: Some("ghp_correct_token".to_string()),
            ..Default::default()
        };
        storage.write_binnacle_state(&state_data).unwrap();

        let state = resolve_state(&storage).unwrap();

        assert!(state.has_token());
        assert!(!state.is_token_from_legacy_location());
        assert!(state.deprecation_warnings.is_empty());
    }
}
