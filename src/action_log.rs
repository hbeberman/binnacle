//! Action logging for Binnacle commands.
//!
//! This module provides comprehensive logging of all binnacle commands and operations
//! to a structured log file in JSONL format.

use crate::storage::Storage;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Represents a single action log entry.
#[derive(Debug, Serialize, Deserialize)]
pub struct ActionLog {
    /// ISO 8601 timestamp when the action occurred
    pub timestamp: DateTime<Utc>,

    /// Repository path where the command was executed
    pub repo_path: String,

    /// Command name (e.g., "task create", "ready", "status")
    pub command: String,

    /// Command arguments as JSON
    pub args: serde_json::Value,

    /// Whether the command succeeded
    pub success: bool,

    /// Error message if the command failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// Command execution duration in milliseconds
    pub duration_ms: u64,

    /// User who executed the command
    pub user: String,
}

/// Log an action to the configured log file.
///
/// This function never fails - it will silently fall back on errors to avoid
/// breaking commands due to logging issues.
pub fn log_action(
    repo_path: &Path,
    command: &str,
    args: serde_json::Value,
    success: bool,
    error: Option<String>,
    duration_ms: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if logging is enabled
    let enabled = match get_config_bool(repo_path, "action_log_enabled") {
        Ok(Some(val)) => val,
        Ok(None) => true, // Default: enabled
        Err(_) => true,   // On error, assume enabled
    };

    if !enabled {
        return Ok(());
    }

    // Get log path
    let log_path = match get_log_path(repo_path) {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Warning: Failed to get action log path: {}", e);
            return Ok(());
        }
    };

    // Sanitize arguments if enabled
    let sanitize = match get_config_bool(repo_path, "action_log_sanitize") {
        Ok(Some(val)) => val,
        Ok(None) => true, // Default: enabled
        Err(_) => true,   // On error, assume enabled
    };

    let sanitized_args = if sanitize { sanitize_args(&args) } else { args };

    // Get current user
    let user = get_current_user();

    // Create log entry
    let entry = ActionLog {
        timestamp: Utc::now(),
        repo_path: repo_path.to_string_lossy().to_string(),
        command: command.to_string(),
        args: sanitized_args,
        success,
        error,
        duration_ms,
        user,
    };

    // Write to log file
    if let Err(e) = write_log_entry(&log_path, &entry) {
        eprintln!("Warning: Failed to write action log: {}", e);
    }

    Ok(())
}

/// Get the log file path from configuration.
fn get_log_path(repo_path: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    // Try to get custom path from config
    let custom_path = match Storage::open(repo_path) {
        Ok(storage) => storage.get_config("action_log_path").ok().flatten(),
        Err(_) => None,
    };

    if let Some(path_str) = custom_path {
        let path = PathBuf::from(path_str);
        // Expand ~ to home directory
        return Ok(expand_home(&path));
    }

    // Default path: ~/.local/share/binnacle/action.log
    let home = dirs::home_dir().ok_or("Could not determine home directory")?;
    Ok(home.join(".local/share/binnacle/action.log"))
}

/// Expand ~ in path to home directory.
fn expand_home(path: &Path) -> PathBuf {
    if let Ok(stripped) = path.strip_prefix("~") {
        if let Some(home) = dirs::home_dir() {
            return home.join(stripped);
        }
    }
    path.to_path_buf()
}

/// Write a log entry to the log file.
fn write_log_entry(path: &Path, entry: &ActionLog) -> Result<(), Box<dyn std::error::Error>> {
    // Create parent directories if needed
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Serialize to JSON
    let json = serde_json::to_string(entry)?;

    // Append to log file
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;

    writeln!(file, "{}", json)?;

    Ok(())
}

/// Sanitize arguments to remove sensitive data.
fn sanitize_args(args: &serde_json::Value) -> serde_json::Value {
    match args {
        serde_json::Value::Object(map) => {
            let mut sanitized = serde_json::Map::new();
            for (key, value) in map {
                // Check if key contains sensitive keywords
                let key_lower = key.to_lowercase();
                if key_lower.contains("password")
                    || key_lower.contains("token")
                    || key_lower.contains("key")
                    || key_lower.contains("secret")
                {
                    sanitized.insert(
                        key.clone(),
                        serde_json::Value::String("[REDACTED]".to_string()),
                    );
                } else {
                    sanitized.insert(key.clone(), sanitize_args(value));
                }
            }
            serde_json::Value::Object(sanitized)
        }
        serde_json::Value::Array(arr) => {
            if arr.len() > 10 {
                // Summarize large arrays
                serde_json::Value::String(format!("[Array with {} items]", arr.len()))
            } else {
                serde_json::Value::Array(arr.iter().map(sanitize_args).collect())
            }
        }
        serde_json::Value::String(s) => {
            // Sanitize file paths (convert to basename)
            let sanitized = if s.contains('/') || s.contains('\\') {
                // Extract basename by splitting on both / and \
                s.rsplit(['/', '\\']).next().unwrap_or(s).to_string()
            } else {
                s.clone()
            };

            // Truncate long strings
            if sanitized.len() > 100 {
                serde_json::Value::String(format!(
                    "{}... ({} chars)",
                    &sanitized[..97],
                    sanitized.len()
                ))
            } else {
                serde_json::Value::String(sanitized)
            }
        }
        _ => args.clone(),
    }
}

/// Get a boolean configuration value.
fn get_config_bool(
    repo_path: &Path,
    key: &str,
) -> Result<Option<bool>, Box<dyn std::error::Error>> {
    let storage = Storage::open(repo_path)?;
    if let Some(value_str) = storage.get_config(key)? {
        let parsed = value_str.to_lowercase();
        let bool_val = parsed == "true" || parsed == "1" || parsed == "yes";
        Ok(Some(bool_val))
    } else {
        Ok(None)
    }
}

/// Get the current user's username.
fn get_current_user() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_simple_string() {
        let value = serde_json::json!("hello");
        let sanitized = sanitize_args(&value);
        assert_eq!(sanitized, serde_json::json!("hello"));
    }

    #[test]
    fn test_sanitize_file_path() {
        let value = serde_json::json!("/very/long/path/to/file.txt");
        let sanitized = sanitize_args(&value);
        assert_eq!(sanitized, serde_json::json!("file.txt"));
    }

    #[test]
    fn test_sanitize_windows_path() {
        let value = serde_json::json!("C:\\Users\\test\\file.txt");
        let sanitized = sanitize_args(&value);
        assert_eq!(sanitized, serde_json::json!("file.txt"));
    }

    #[test]
    fn test_sanitize_long_string() {
        let long_str = "a".repeat(150);
        let value = serde_json::json!(long_str);
        let sanitized = sanitize_args(&value);
        if let serde_json::Value::String(s) = sanitized {
            assert!(s.contains("... (150 chars)"));
        } else {
            panic!("Expected string value");
        }
    }

    #[test]
    fn test_sanitize_sensitive_keys() {
        let value = serde_json::json!({
            "username": "alice",
            "password": "secret123",
            "api_token": "abc123",
            "title": "My task"
        });
        let sanitized = sanitize_args(&value);

        assert_eq!(sanitized["username"], "alice");
        assert_eq!(sanitized["password"], "[REDACTED]");
        assert_eq!(sanitized["api_token"], "[REDACTED]");
        assert_eq!(sanitized["title"], "My task");
    }

    #[test]
    fn test_sanitize_large_array() {
        let arr: Vec<i32> = (0..15).collect();
        let value = serde_json::json!(arr);
        let sanitized = sanitize_args(&value);

        if let serde_json::Value::String(s) = sanitized {
            assert_eq!(s, "[Array with 15 items]");
        } else {
            panic!("Expected string value for large array");
        }
    }

    #[test]
    fn test_sanitize_small_array() {
        let value = serde_json::json!([1, 2, 3]);
        let sanitized = sanitize_args(&value);
        assert_eq!(sanitized, serde_json::json!([1, 2, 3]));
    }

    #[test]
    fn test_sanitize_nested_object() {
        let value = serde_json::json!({
            "user": {
                "name": "alice",
                "password": "secret"
            },
            "file": "/home/user/data.txt"
        });
        let sanitized = sanitize_args(&value);

        assert_eq!(sanitized["user"]["name"], "alice");
        assert_eq!(sanitized["user"]["password"], "[REDACTED]");
        assert_eq!(sanitized["file"], "data.txt");
    }
}
