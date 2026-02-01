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

/// Session server state stored in the `serve` block of state.kdl.
///
/// Tracks information about a running session server for discovery,
/// health checking, and auto-launch coordination.
///
/// # KDL Schema
///
/// ```kdl
/// serve {
///   pid 12345
///   port 3030
///   host "127.0.0.1"
///   started-at "2026-01-31T10:00:00Z"
///   repo-name "binnacle"
///   branch "main"
///   last-heartbeat "2026-01-31T10:30:00Z"
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServeState {
    /// Process ID of the running server
    pub pid: u32,

    /// Port the server is listening on
    pub port: u16,

    /// Host address the server is bound to
    pub host: String,

    /// Timestamp when the server started
    pub started_at: DateTime<Utc>,

    /// Repository name (derived from git remote)
    pub repo_name: String,

    /// Git branch name
    pub branch: String,

    /// Timestamp of last heartbeat update
    pub last_heartbeat: DateTime<Utc>,

    /// Optional upstream hub URL (e.g., wss://hub.example.com/sessions)
    pub upstream: Option<String>,
}

impl ServeState {
    /// Create a new serve state with current timestamp.
    pub fn new(pid: u32, port: u16, host: String, repo_name: String, branch: String) -> Self {
        let now = Utc::now();
        Self {
            pid,
            port,
            host,
            started_at: now,
            repo_name,
            branch,
            last_heartbeat: now,
            upstream: None,
        }
    }

    /// Create a new serve state with upstream hub URL.
    pub fn with_upstream(
        pid: u32,
        port: u16,
        host: String,
        repo_name: String,
        branch: String,
        upstream: Option<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            pid,
            port,
            host,
            started_at: now,
            repo_name,
            branch,
            last_heartbeat: now,
            upstream,
        }
    }

    /// Update the heartbeat timestamp to now.
    pub fn touch_heartbeat(&mut self) {
        self.last_heartbeat = Utc::now();
    }

    /// Check if the heartbeat is stale (older than threshold).
    pub fn is_heartbeat_stale(&self, threshold_secs: i64) -> bool {
        let elapsed = Utc::now()
            .signed_duration_since(self.last_heartbeat)
            .num_seconds();
        elapsed > threshold_secs
    }

    /// Parse serve state from a KDL node (the `serve` block).
    pub fn from_kdl_node(node: &KdlNode) -> Option<Self> {
        let children = node.children()?;

        let pid = children
            .get("pid")?
            .entries()
            .first()?
            .value()
            .as_integer()? as u32;

        let port = children
            .get("port")?
            .entries()
            .first()?
            .value()
            .as_integer()? as u16;

        let host = children
            .get("host")?
            .entries()
            .first()?
            .value()
            .as_string()?
            .to_string();

        let started_at_str = children
            .get("started-at")?
            .entries()
            .first()?
            .value()
            .as_string()?;
        let started_at = started_at_str.parse::<DateTime<Utc>>().ok()?;

        let repo_name = children
            .get("repo-name")?
            .entries()
            .first()?
            .value()
            .as_string()?
            .to_string();

        let branch = children
            .get("branch")?
            .entries()
            .first()?
            .value()
            .as_string()?
            .to_string();

        let last_heartbeat_str = children
            .get("last-heartbeat")?
            .entries()
            .first()?
            .value()
            .as_string()?;
        let last_heartbeat = last_heartbeat_str.parse::<DateTime<Utc>>().ok()?;

        // Optional upstream field
        let upstream = children.get("upstream").and_then(|n| {
            n.entries()
                .first()
                .and_then(|e| e.value().as_string().map(|s| s.to_string()))
        });

        Some(Self {
            pid,
            port,
            host,
            started_at,
            repo_name,
            branch,
            last_heartbeat,
            upstream,
        })
    }

    /// Convert serve state to a KDL node (the `serve` block).
    pub fn to_kdl_node(&self) -> KdlNode {
        let mut node = KdlNode::new("serve");
        let mut children = KdlDocument::new();

        // pid
        let mut pid_node = KdlNode::new("pid");
        pid_node.push(KdlEntry::new(KdlValue::Integer(self.pid as i128)));
        children.nodes_mut().push(pid_node);

        // port
        let mut port_node = KdlNode::new("port");
        port_node.push(KdlEntry::new(KdlValue::Integer(self.port as i128)));
        children.nodes_mut().push(port_node);

        // host
        let mut host_node = KdlNode::new("host");
        host_node.push(KdlEntry::new(KdlValue::String(self.host.clone())));
        children.nodes_mut().push(host_node);

        // started-at
        let mut started_at_node = KdlNode::new("started-at");
        started_at_node.push(KdlEntry::new(KdlValue::String(
            self.started_at.to_rfc3339(),
        )));
        children.nodes_mut().push(started_at_node);

        // repo-name
        let mut repo_name_node = KdlNode::new("repo-name");
        repo_name_node.push(KdlEntry::new(KdlValue::String(self.repo_name.clone())));
        children.nodes_mut().push(repo_name_node);

        // branch
        let mut branch_node = KdlNode::new("branch");
        branch_node.push(KdlEntry::new(KdlValue::String(self.branch.clone())));
        children.nodes_mut().push(branch_node);

        // last-heartbeat
        let mut heartbeat_node = KdlNode::new("last-heartbeat");
        heartbeat_node.push(KdlEntry::new(KdlValue::String(
            self.last_heartbeat.to_rfc3339(),
        )));
        children.nodes_mut().push(heartbeat_node);

        // upstream (optional)
        if let Some(ref upstream_url) = self.upstream {
            let mut upstream_node = KdlNode::new("upstream");
            upstream_node.push(KdlEntry::new(KdlValue::String(upstream_url.clone())));
            children.nodes_mut().push(upstream_node);
        }

        node.set_children(children);
        node
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
///
/// serve {
///   pid 12345
///   port 3030
///   host "127.0.0.1"
///   started-at "2026-01-31T10:00:00Z"
///   repo-name "binnacle"
///   branch "main"
///   last-heartbeat "2026-01-31T10:30:00Z"
/// }
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BinnacleState {
    /// GitHub PAT for API access (sensitive!)
    pub github_token: Option<String>,

    /// Timestamp when the token was last validated
    pub token_validated_at: Option<DateTime<Utc>>,

    /// Last known Copilot CLI version
    pub last_copilot_version: Option<String>,

    /// Session server state (if a server is running)
    pub serve: Option<ServeState>,
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

        // Parse serve block
        if let Some(node) = doc.get("serve") {
            state.serve = ServeState::from_kdl_node(node);
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

        if let Some(ref serve) = self.serve {
            doc.nodes_mut().push(serve.to_kdl_node());
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
        if other.serve.is_some() {
            self.serve = other.serve.clone();
        }
    }

    /// Set the serve state (for session server startup).
    pub fn set_serve(&mut self, serve: ServeState) {
        self.serve = Some(serve);
    }

    /// Clear the serve state (for session server shutdown).
    pub fn clear_serve(&mut self) {
        self.serve = None;
    }

    /// Update the heartbeat timestamp in serve state.
    /// Returns false if no serve state exists.
    pub fn touch_heartbeat(&mut self) -> bool {
        if let Some(ref mut serve) = self.serve {
            serve.touch_heartbeat();
            true
        } else {
            false
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
        assert_eq!(state.serve, None);
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
            serve: None,
        };

        let doc = state.to_kdl();
        let parsed = BinnacleState::from_kdl(&doc);

        assert_eq!(state.github_token, parsed.github_token);
        // Note: DateTime comparison may have precision differences due to formatting
        assert!(parsed.token_validated_at.is_some());
        assert_eq!(state.last_copilot_version, parsed.last_copilot_version);
        assert_eq!(state.serve, parsed.serve);
    }

    #[test]
    fn test_state_merge() {
        let mut base = BinnacleState {
            github_token: Some("old_token".to_string()),
            token_validated_at: None,
            last_copilot_version: Some("1.0.0".to_string()),
            serve: None,
        };

        let override_state = BinnacleState {
            github_token: Some("new_token".to_string()),
            token_validated_at: Some(Utc::now()),
            last_copilot_version: None,
            serve: Some(ServeState::new(
                123,
                3030,
                "127.0.0.1".to_string(),
                "test-repo".to_string(),
                "main".to_string(),
            )),
        };

        base.merge(&override_state);

        assert_eq!(base.github_token, Some("new_token".to_string())); // Overridden
        assert!(base.token_validated_at.is_some()); // Overridden
        assert_eq!(base.last_copilot_version, Some("1.0.0".to_string())); // Not overridden
        assert!(base.serve.is_some()); // Overridden
    }

    // ==================== ServeState Tests ====================

    #[test]
    fn test_serve_state_new() {
        let serve = ServeState::new(
            1234,
            3030,
            "127.0.0.1".to_string(),
            "binnacle".to_string(),
            "main".to_string(),
        );

        assert_eq!(serve.pid, 1234);
        assert_eq!(serve.port, 3030);
        assert_eq!(serve.host, "127.0.0.1");
        assert_eq!(serve.repo_name, "binnacle");
        assert_eq!(serve.branch, "main");
        // started_at and last_heartbeat should be set to now
        assert!(serve.started_at <= Utc::now());
        assert_eq!(serve.started_at, serve.last_heartbeat);
    }

    #[test]
    fn test_serve_state_touch_heartbeat() {
        let mut serve = ServeState::new(
            1234,
            3030,
            "127.0.0.1".to_string(),
            "binnacle".to_string(),
            "main".to_string(),
        );

        let original_heartbeat = serve.last_heartbeat;
        std::thread::sleep(std::time::Duration::from_millis(10));
        serve.touch_heartbeat();

        assert!(serve.last_heartbeat >= original_heartbeat);
        assert_eq!(serve.started_at, original_heartbeat); // started_at unchanged
    }

    #[test]
    fn test_serve_state_is_heartbeat_stale() {
        let serve = ServeState {
            pid: 1234,
            port: 3030,
            host: "127.0.0.1".to_string(),
            started_at: Utc::now(),
            repo_name: "binnacle".to_string(),
            branch: "main".to_string(),
            last_heartbeat: Utc::now() - chrono::Duration::seconds(60),
            upstream: None,
        };

        assert!(serve.is_heartbeat_stale(30)); // Stale if threshold is 30s
        assert!(!serve.is_heartbeat_stale(120)); // Not stale if threshold is 120s
    }

    #[test]
    fn test_serve_state_kdl_roundtrip() {
        let serve = ServeState::new(
            12345,
            3030,
            "0.0.0.0".to_string(),
            "test-repo".to_string(),
            "feature/test".to_string(),
        );

        let node = serve.to_kdl_node();
        let parsed = ServeState::from_kdl_node(&node).unwrap();

        assert_eq!(serve.pid, parsed.pid);
        assert_eq!(serve.port, parsed.port);
        assert_eq!(serve.host, parsed.host);
        assert_eq!(serve.repo_name, parsed.repo_name);
        assert_eq!(serve.branch, parsed.branch);
        // DateTime precision may differ due to RFC3339 formatting
    }

    #[test]
    fn test_serve_state_from_kdl_full_document() {
        let kdl = r#"
            github-token "ghp_test123"
            serve {
                pid 9999
                port 8080
                host "localhost"
                started-at "2026-01-31T10:00:00Z"
                repo-name "my-project"
                branch "develop"
                last-heartbeat "2026-01-31T10:30:00Z"
            }
        "#;
        let doc: KdlDocument = kdl.parse().unwrap();
        let state = BinnacleState::from_kdl(&doc);

        assert!(state.serve.is_some());
        let serve = state.serve.unwrap();
        assert_eq!(serve.pid, 9999);
        assert_eq!(serve.port, 8080);
        assert_eq!(serve.host, "localhost");
        assert_eq!(serve.repo_name, "my-project");
        assert_eq!(serve.branch, "develop");
    }

    #[test]
    fn test_serve_state_to_kdl_in_full_document() {
        let state = BinnacleState {
            github_token: Some("ghp_test".to_string()),
            token_validated_at: None,
            last_copilot_version: None,
            serve: Some(ServeState::new(
                4567,
                3030,
                "127.0.0.1".to_string(),
                "binnacle".to_string(),
                "main".to_string(),
            )),
        };

        let doc = state.to_kdl();
        let kdl_str = doc.to_string();

        assert!(kdl_str.contains("serve"));
        assert!(kdl_str.contains("pid 4567"));
        assert!(kdl_str.contains("port 3030"));
        assert!(kdl_str.contains("host \"127.0.0.1\""));

        // Roundtrip test
        let parsed = BinnacleState::from_kdl(&doc);
        assert!(parsed.serve.is_some());
        assert_eq!(parsed.serve.unwrap().pid, 4567);
    }

    #[test]
    fn test_binnacle_state_set_serve() {
        let mut state = BinnacleState::default();
        assert!(state.serve.is_none());

        let serve = ServeState::new(
            123,
            3030,
            "127.0.0.1".to_string(),
            "test".to_string(),
            "main".to_string(),
        );
        state.set_serve(serve);

        assert!(state.serve.is_some());
        assert_eq!(state.serve.as_ref().unwrap().pid, 123);
    }

    #[test]
    fn test_binnacle_state_clear_serve() {
        let mut state = BinnacleState {
            serve: Some(ServeState::new(
                123,
                3030,
                "127.0.0.1".to_string(),
                "test".to_string(),
                "main".to_string(),
            )),
            ..Default::default()
        };
        assert!(state.serve.is_some());

        state.clear_serve();
        assert!(state.serve.is_none());
    }

    #[test]
    fn test_binnacle_state_touch_heartbeat() {
        let mut state = BinnacleState {
            serve: Some(ServeState::new(
                123,
                3030,
                "127.0.0.1".to_string(),
                "test".to_string(),
                "main".to_string(),
            )),
            ..Default::default()
        };

        assert!(state.touch_heartbeat());

        // Test with no serve state
        let mut empty_state = BinnacleState::default();
        assert!(!empty_state.touch_heartbeat());
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
