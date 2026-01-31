//! KDL schema definitions for config.kdl and state.kdl.
//!
//! This module provides:
//! - Rust structs representing the KDL schema
//! - Serialization/deserialization to/from KDL format
//! - Validation functions
//! - Default values
//! - Legacy token detection (for migration)

use chrono::{DateTime, Utc};
use kdl::{KdlDocument, KdlEntry, KdlNode, KdlValue};
use serde::{Deserialize, Serialize};

/// Output format preference for CLI commands.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    /// JSON output (default, machine-readable)
    #[default]
    Json,
    /// Human-readable output
    Human,
}

impl OutputFormat {
    /// Parse from string, case-insensitive.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "json" => Some(OutputFormat::Json),
            "human" => Some(OutputFormat::Human),
            _ => None,
        }
    }

    /// Convert to string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            OutputFormat::Json => "json",
            OutputFormat::Human => "human",
        }
    }
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// User preferences stored in config.kdl.
///
/// These settings are safe to sync across machines (e.g., via dotfiles).
/// File permissions: 0644 (rw-r--r--)
///
/// # KDL Schema
///
/// ```kdl
/// // User preferences - safe to sync across machines
/// editor "nvim"
/// output-format "human"  // or "json"
/// default-priority 2
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BinnacleConfig {
    /// Preferred editor command (e.g., "nvim", "code", "vim")
    pub editor: Option<String>,

    /// Default output format for CLI commands
    pub output_format: Option<OutputFormat>,

    /// Default priority for new tasks (0-4, where 0 is highest)
    pub default_priority: Option<u8>,
}

impl BinnacleConfig {
    /// Create an empty config with no values set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Validate the config values.
    ///
    /// Returns an error message if any value is invalid.
    pub fn validate(&self) -> Result<(), String> {
        if let Some(priority) = self.default_priority {
            if priority > 4 {
                return Err(format!("default-priority must be 0-4, got {}", priority));
            }
        }
        Ok(())
    }

    /// Parse config from a KDL document.
    pub fn from_kdl(doc: &KdlDocument) -> Self {
        let mut config = Self::new();

        // Parse editor
        if let Some(node) = doc.get("editor") {
            if let Some(entry) = node.entries().first() {
                if let Some(s) = entry.value().as_string() {
                    config.editor = Some(s.to_string());
                }
            }
        }

        // Parse output-format
        if let Some(node) = doc.get("output-format") {
            if let Some(entry) = node.entries().first() {
                if let Some(s) = entry.value().as_string() {
                    config.output_format = OutputFormat::parse(s);
                }
            }
        }

        // Parse default-priority
        if let Some(node) = doc.get("default-priority") {
            if let Some(entry) = node.entries().first() {
                if let Some(i) = entry.value().as_integer() {
                    if (0..=4).contains(&i) {
                        config.default_priority = Some(i as u8);
                    }
                }
            }
        }

        config
    }

    /// Convert config to a KDL document.
    pub fn to_kdl(&self) -> KdlDocument {
        let mut doc = KdlDocument::new();

        if let Some(ref editor) = self.editor {
            let mut node = KdlNode::new("editor");
            node.push(KdlEntry::new(KdlValue::String(editor.clone())));
            doc.nodes_mut().push(node);
        }

        if let Some(ref format) = self.output_format {
            let mut node = KdlNode::new("output-format");
            node.push(KdlEntry::new(KdlValue::String(format.as_str().to_string())));
            doc.nodes_mut().push(node);
        }

        if let Some(priority) = self.default_priority {
            let mut node = KdlNode::new("default-priority");
            node.push(KdlEntry::new(KdlValue::Integer(priority as i128)));
            doc.nodes_mut().push(node);
        }

        doc
    }

    /// Merge another config into this one.
    /// Values from `other` override values in `self` if they are Some.
    pub fn merge(&mut self, other: &BinnacleConfig) {
        if other.editor.is_some() {
            self.editor = other.editor.clone();
        }
        if other.output_format.is_some() {
            self.output_format = other.output_format.clone();
        }
        if other.default_priority.is_some() {
            self.default_priority = other.default_priority;
        }
    }
}

/// Runtime state stored in state.kdl.
///
/// This file contains machine-specific state and secrets.
/// **MUST be created with 0600 permissions (owner read/write only)**.
///
/// # KDL Schema
///
/// ```kdl
/// // Machine-specific state - never sync
/// github-token "ghp_xxxxxxxxxxxxxxxxxxxx"
/// token-validated-at "2026-01-31T09:00:00Z"
/// last-copilot-version "1.0.0"
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BinnacleState {
    /// GitHub PAT for API access (sensitive!)
    pub github_token: Option<String>,

    /// Timestamp when the token was last validated
    pub token_validated_at: Option<DateTime<Utc>>,

    /// Last known Copilot CLI version
    pub last_copilot_version: Option<String>,
}

impl BinnacleState {
    /// Create an empty state with no values set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if this state contains any secrets.
    pub fn has_secrets(&self) -> bool {
        self.github_token.is_some()
    }

    /// Mask the GitHub token for display purposes.
    ///
    /// Returns the token with middle characters replaced by asterisks,
    /// showing only the first 4 and last 4 characters.
    pub fn masked_token(&self) -> Option<String> {
        self.github_token.as_ref().map(|token| {
            if token.len() <= 12 {
                // Too short to mask meaningfully, hide entire middle
                format!("{}...{}", &token[..4.min(token.len())], "")
            } else {
                format!("{}...{}", &token[..4], &token[token.len() - 4..])
            }
        })
    }

    /// Parse state from a KDL document.
    pub fn from_kdl(doc: &KdlDocument) -> Self {
        let mut state = Self::new();

        // Parse github-token
        if let Some(node) = doc.get("github-token") {
            if let Some(entry) = node.entries().first() {
                if let Some(s) = entry.value().as_string() {
                    state.github_token = Some(s.to_string());
                }
            }
        }

        // Parse token-validated-at
        if let Some(node) = doc.get("token-validated-at") {
            if let Some(entry) = node.entries().first() {
                if let Some(s) = entry.value().as_string() {
                    if let Ok(dt) = s.parse::<DateTime<Utc>>() {
                        state.token_validated_at = Some(dt);
                    }
                }
            }
        }

        // Parse last-copilot-version
        if let Some(node) = doc.get("last-copilot-version") {
            if let Some(entry) = node.entries().first() {
                if let Some(s) = entry.value().as_string() {
                    state.last_copilot_version = Some(s.to_string());
                }
            }
        }

        state
    }

    /// Convert state to a KDL document.
    pub fn to_kdl(&self) -> KdlDocument {
        let mut doc = KdlDocument::new();

        if let Some(ref token) = self.github_token {
            let mut node = KdlNode::new("github-token");
            node.push(KdlEntry::new(KdlValue::String(token.clone())));
            doc.nodes_mut().push(node);
        }

        if let Some(ref validated_at) = self.token_validated_at {
            let mut node = KdlNode::new("token-validated-at");
            node.push(KdlEntry::new(KdlValue::String(validated_at.to_rfc3339())));
            doc.nodes_mut().push(node);
        }

        if let Some(ref version) = self.last_copilot_version {
            let mut node = KdlNode::new("last-copilot-version");
            node.push(KdlEntry::new(KdlValue::String(version.clone())));
            doc.nodes_mut().push(node);
        }

        doc
    }

    /// Merge another state into this one.
    /// Values from `other` override values in `self` if they are Some.
    pub fn merge(&mut self, other: &BinnacleState) {
        if other.github_token.is_some() {
            self.github_token = other.github_token.clone();
        }
        if other.token_validated_at.is_some() {
            self.token_validated_at = other.token_validated_at;
        }
        if other.last_copilot_version.is_some() {
            self.last_copilot_version = other.last_copilot_version.clone();
        }
    }
}

/// Required permissions for state.kdl (Unix: 0600, owner read/write only).
#[cfg(unix)]
pub const STATE_FILE_MODE: u32 = 0o600;

/// Required permissions for config.kdl (Unix: 0644, readable by all).
#[cfg(unix)]
pub const CONFIG_FILE_MODE: u32 = 0o644;

/// Check if a KDL document contains a legacy github-token entry.
///
/// Tokens should be stored in state.kdl, not config.kdl. This function
/// detects legacy/accidental token placements for migration purposes.
pub fn has_legacy_token_in_config(doc: &KdlDocument) -> bool {
    doc.get("github-token").is_some()
}

/// Extract a legacy github-token from a KDL document (config.kdl).
///
/// Returns the token value if found. Tokens found in config.kdl are
/// considered legacy and should be migrated to state.kdl.
pub fn get_legacy_token_from_config(doc: &KdlDocument) -> Option<String> {
    if let Some(node) = doc.get("github-token") {
        if let Some(entry) = node.entries().first() {
            if let Some(s) = entry.value().as_string() {
                return Some(s.to_string());
            }
        }
    }
    None
}

/// Remove a github-token entry from a KDL document.
///
/// Returns true if a token was removed, false if none was found.
pub fn remove_token_from_kdl_doc(doc: &mut KdlDocument) -> bool {
    let nodes = doc.nodes_mut();
    let original_len = nodes.len();
    nodes.retain(|node| node.name().value() != "github-token");
    nodes.len() < original_len
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== OutputFormat Tests ====================

    #[test]
    fn test_output_format_from_str() {
        assert_eq!(OutputFormat::parse("json"), Some(OutputFormat::Json));
        assert_eq!(OutputFormat::parse("JSON"), Some(OutputFormat::Json));
        assert_eq!(OutputFormat::parse("human"), Some(OutputFormat::Human));
        assert_eq!(OutputFormat::parse("HUMAN"), Some(OutputFormat::Human));
        assert_eq!(OutputFormat::parse("invalid"), None);
    }

    #[test]
    fn test_output_format_as_str() {
        assert_eq!(OutputFormat::Json.as_str(), "json");
        assert_eq!(OutputFormat::Human.as_str(), "human");
    }

    #[test]
    fn test_output_format_display() {
        assert_eq!(format!("{}", OutputFormat::Json), "json");
        assert_eq!(format!("{}", OutputFormat::Human), "human");
    }

    // ==================== BinnacleConfig Tests ====================

    #[test]
    fn test_config_default() {
        let config = BinnacleConfig::default();
        assert_eq!(config.editor, None);
        assert_eq!(config.output_format, None);
        assert_eq!(config.default_priority, None);
    }

    #[test]
    fn test_config_validate_valid() {
        let config = BinnacleConfig {
            editor: Some("nvim".to_string()),
            output_format: Some(OutputFormat::Human),
            default_priority: Some(2),
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validate_invalid_priority() {
        let config = BinnacleConfig {
            default_priority: Some(5),
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("default-priority must be 0-4"));
    }

    #[test]
    fn test_config_from_kdl_empty() {
        let doc = KdlDocument::new();
        let config = BinnacleConfig::from_kdl(&doc);
        assert_eq!(config, BinnacleConfig::default());
    }

    #[test]
    fn test_config_from_kdl_full() {
        let kdl = r#"
            editor "nvim"
            output-format "human"
            default-priority 2
        "#;
        let doc: KdlDocument = kdl.parse().unwrap();
        let config = BinnacleConfig::from_kdl(&doc);

        assert_eq!(config.editor, Some("nvim".to_string()));
        assert_eq!(config.output_format, Some(OutputFormat::Human));
        assert_eq!(config.default_priority, Some(2));
    }

    #[test]
    fn test_config_to_kdl_roundtrip() {
        let config = BinnacleConfig {
            editor: Some("code".to_string()),
            output_format: Some(OutputFormat::Json),
            default_priority: Some(1),
        };

        let doc = config.to_kdl();
        let parsed = BinnacleConfig::from_kdl(&doc);

        assert_eq!(config, parsed);
    }

    #[test]
    fn test_config_merge() {
        let mut base = BinnacleConfig {
            editor: Some("vim".to_string()),
            output_format: Some(OutputFormat::Json),
            default_priority: Some(3),
        };

        let override_config = BinnacleConfig {
            editor: None,
            output_format: Some(OutputFormat::Human),
            default_priority: None,
        };

        base.merge(&override_config);

        assert_eq!(base.editor, Some("vim".to_string())); // Not overridden
        assert_eq!(base.output_format, Some(OutputFormat::Human)); // Overridden
        assert_eq!(base.default_priority, Some(3)); // Not overridden
    }

    // ==================== BinnacleState Tests ====================

    #[test]
    fn test_state_default() {
        let state = BinnacleState::default();
        assert_eq!(state.github_token, None);
        assert_eq!(state.token_validated_at, None);
        assert_eq!(state.last_copilot_version, None);
    }

    #[test]
    fn test_state_has_secrets() {
        let no_secrets = BinnacleState::default();
        assert!(!no_secrets.has_secrets());

        let with_secrets = BinnacleState {
            github_token: Some("ghp_test".to_string()),
            ..Default::default()
        };
        assert!(with_secrets.has_secrets());
    }

    #[test]
    fn test_state_masked_token() {
        let state = BinnacleState {
            github_token: Some("ghp_xxxxxxxxxxxxxxxxxxxx".to_string()),
            ..Default::default()
        };
        let masked = state.masked_token().unwrap();
        assert_eq!(masked, "ghp_...xxxx");
    }

    #[test]
    fn test_state_masked_token_short() {
        let state = BinnacleState {
            github_token: Some("short".to_string()),
            ..Default::default()
        };
        let masked = state.masked_token().unwrap();
        // Short tokens: show first 4 chars + "..."
        assert!(masked.starts_with("shor"));
    }

    #[test]
    fn test_state_from_kdl_empty() {
        let doc = KdlDocument::new();
        let state = BinnacleState::from_kdl(&doc);
        assert_eq!(state, BinnacleState::default());
    }

    #[test]
    fn test_state_from_kdl_full() {
        let kdl = r#"
            github-token "ghp_test123"
            token-validated-at "2026-01-31T09:00:00Z"
            last-copilot-version "1.0.0"
        "#;
        let doc: KdlDocument = kdl.parse().unwrap();
        let state = BinnacleState::from_kdl(&doc);

        assert_eq!(state.github_token, Some("ghp_test123".to_string()));
        assert!(state.token_validated_at.is_some());
        assert_eq!(state.last_copilot_version, Some("1.0.0".to_string()));
    }

    #[test]
    fn test_state_to_kdl_roundtrip() {
        let state = BinnacleState {
            github_token: Some("ghp_test".to_string()),
            token_validated_at: Some(Utc::now()),
            last_copilot_version: Some("2.0.0".to_string()),
        };

        let doc = state.to_kdl();
        let parsed = BinnacleState::from_kdl(&doc);

        assert_eq!(state.github_token, parsed.github_token);
        // Note: DateTime comparison may have precision differences due to formatting
        assert!(parsed.token_validated_at.is_some());
        assert_eq!(state.last_copilot_version, parsed.last_copilot_version);
    }

    #[test]
    fn test_state_merge() {
        let mut base = BinnacleState {
            github_token: Some("old_token".to_string()),
            token_validated_at: None,
            last_copilot_version: Some("1.0.0".to_string()),
        };

        let override_state = BinnacleState {
            github_token: Some("new_token".to_string()),
            token_validated_at: Some(Utc::now()),
            last_copilot_version: None,
        };

        base.merge(&override_state);

        assert_eq!(base.github_token, Some("new_token".to_string())); // Overridden
        assert!(base.token_validated_at.is_some()); // Overridden
        assert_eq!(base.last_copilot_version, Some("1.0.0".to_string())); // Not overridden
    }

    // ==================== Permission Constant Tests ====================

    #[cfg(unix)]
    #[test]
    fn test_file_mode_constants() {
        assert_eq!(STATE_FILE_MODE, 0o600);
        assert_eq!(CONFIG_FILE_MODE, 0o644);
    }

    // ==================== Legacy Token Detection Tests ====================

    #[test]
    fn test_has_legacy_token_in_config_false() {
        let kdl = r#"
            editor "nvim"
            output-format "human"
        "#;
        let doc: KdlDocument = kdl.parse().unwrap();
        assert!(!has_legacy_token_in_config(&doc));
    }

    #[test]
    fn test_has_legacy_token_in_config_true() {
        let kdl = r#"
            editor "nvim"
            github-token "ghp_legacy_token_abc123"
        "#;
        let doc: KdlDocument = kdl.parse().unwrap();
        assert!(has_legacy_token_in_config(&doc));
    }

    #[test]
    fn test_get_legacy_token_from_config() {
        let kdl = r#"
            editor "nvim"
            github-token "ghp_legacy_token_xyz"
        "#;
        let doc: KdlDocument = kdl.parse().unwrap();
        assert_eq!(
            get_legacy_token_from_config(&doc),
            Some("ghp_legacy_token_xyz".to_string())
        );
    }

    #[test]
    fn test_get_legacy_token_from_config_none() {
        let kdl = r#"
            editor "nvim"
        "#;
        let doc: KdlDocument = kdl.parse().unwrap();
        assert_eq!(get_legacy_token_from_config(&doc), None);
    }

    #[test]
    fn test_remove_token_from_kdl_doc() {
        let kdl = r#"
            editor "nvim"
            github-token "ghp_to_remove"
            output-format "human"
        "#;
        let mut doc: KdlDocument = kdl.parse().unwrap();

        assert!(has_legacy_token_in_config(&doc));
        let removed = remove_token_from_kdl_doc(&mut doc);
        assert!(removed);
        assert!(!has_legacy_token_in_config(&doc));

        // Other nodes should remain
        assert!(doc.get("editor").is_some());
        assert!(doc.get("output-format").is_some());
    }

    #[test]
    fn test_remove_token_from_kdl_doc_not_found() {
        let kdl = r#"
            editor "nvim"
        "#;
        let mut doc: KdlDocument = kdl.parse().unwrap();

        let removed = remove_token_from_kdl_doc(&mut doc);
        assert!(!removed);
    }
}
