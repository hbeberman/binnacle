//! Session logging for debugging auto-launched servers.
//!
//! This module provides an append-only log file (`~/.local/state/binnacle/session.log`)
//! for tracking server lifecycle events and debugging auto-launched servers.
//!
//! The log uses a simple text format with timestamps for easy debugging:
//! ```text
//! 2026-01-31T12:00:00Z [INFO] Session server started: repo@branch on 127.0.0.1:55823 (pid: 12345)
//! 2026-01-31T12:00:05Z [INFO] WebSocket connection opened: client_id=abc123
//! 2026-01-31T12:00:10Z [WARN] Heartbeat timeout detected
//! 2026-01-31T12:01:00Z [INFO] Session server stopped: repo@branch (pid: 12345)
//! ```

use chrono::{DateTime, Utc};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

/// Log levels for session events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
        }
    }
}

/// Get the path to the session log file.
///
/// Returns `~/.local/state/binnacle/session.log` on Unix systems.
/// Creates the directory if it doesn't exist.
pub fn get_session_log_path() -> Option<PathBuf> {
    // Check for thread-local test override first (for parallel test isolation)
    if let Some(test_dir) = crate::storage::get_data_dir_override() {
        return Some(test_dir.join("session.log"));
    }

    // Check for env var test override
    if let Ok(test_dir) = std::env::var("BN_DATA_DIR") {
        let path = PathBuf::from(test_dir).join("session.log");
        return Some(path);
    }

    // Use XDG state directory: ~/.local/state/binnacle/
    let home = dirs::home_dir()?;
    let state_dir = home.join(".local").join("state").join("binnacle");
    Some(state_dir.join("session.log"))
}

/// Write a log entry to the session log file.
///
/// This function is designed to never fail or block - it silently ignores
/// any errors to avoid interfering with server operation.
pub fn log_session_event(level: LogLevel, message: &str) {
    if let Err(e) = write_session_log_entry(level, message) {
        // Only print to stderr for errors, to avoid noise
        if level == LogLevel::Error {
            eprintln!("Warning: Failed to write session log: {}", e);
        }
    }
}

/// Internal function to write a log entry.
fn write_session_log_entry(
    level: LogLevel,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let log_path = get_session_log_path().ok_or("Could not determine session log path")?;

    // Create parent directories if needed
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Format the log entry
    let timestamp: DateTime<Utc> = Utc::now();
    let entry = format!(
        "{} [{}] {}\n",
        timestamp.format("%Y-%m-%dT%H:%M:%SZ"),
        level,
        message
    );

    // Append to log file
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;

    file.write_all(entry.as_bytes())?;

    Ok(())
}

/// Log a session server start event.
pub fn log_server_start(display_name: &str, host: &str, port: u16, pid: u32) {
    log_session_event(
        LogLevel::Info,
        &format!(
            "Session server started: {} on {}:{} (pid: {})",
            display_name, host, port, pid
        ),
    );
}

/// Log a session server stop event.
pub fn log_server_stop(display_name: &str, pid: u32) {
    log_session_event(
        LogLevel::Info,
        &format!("Session server stopped: {} (pid: {})", display_name, pid),
    );
}

/// Log a GUI server start event.
pub fn log_gui_start(repo_path: &str, host: &str, port: u16, pid: u32, readonly: bool) {
    let mode = if readonly { "readonly" } else { "read-write" };
    log_session_event(
        LogLevel::Info,
        &format!(
            "GUI server started: {} on {}:{} (pid: {}, mode: {})",
            repo_path, host, port, pid, mode
        ),
    );
}

/// Log a WebSocket connection event.
pub fn log_ws_connection(client_id: &str, connected: bool) {
    let action = if connected { "opened" } else { "closed" };
    log_session_event(
        LogLevel::Info,
        &format!("WebSocket connection {}: client_id={}", action, client_id),
    );
}

/// Log a heartbeat update.
pub fn log_heartbeat(display_name: &str) {
    log_session_event(
        LogLevel::Info,
        &format!("Heartbeat updated: {}", display_name),
    );
}

/// Log a heartbeat timeout warning.
pub fn log_heartbeat_timeout(display_name: &str, last_heartbeat: &str) {
    log_session_event(
        LogLevel::Warn,
        &format!(
            "Heartbeat timeout detected: {} (last: {})",
            display_name, last_heartbeat
        ),
    );
}

/// Log an error event.
pub fn log_error(context: &str, error: &str) {
    log_session_event(LogLevel::Error, &format!("{}: {}", context, error));
}

/// Log an auto-launch attempt.
pub fn log_auto_launch_attempt(source: &str, repo_path: &str) {
    log_session_event(
        LogLevel::Info,
        &format!("Auto-launch attempt from {}: {}", source, repo_path),
    );
}

/// Log an auto-launch success.
pub fn log_auto_launch_success(source: &str, host: &str, port: u16, pid: u32) {
    log_session_event(
        LogLevel::Info,
        &format!(
            "Auto-launch success from {}: spawned server on {}:{} (pid: {})",
            source, host, port, pid
        ),
    );
}

/// Log an auto-launch failure.
pub fn log_auto_launch_failure(source: &str, error: &str) {
    log_session_event(
        LogLevel::Error,
        &format!("Auto-launch failed from {}: {}", source, error),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Thread-local test setup that doesn't require #[serial].
    /// Uses thread-local data dir override instead of env vars.
    fn setup_test_env_isolated() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        crate::storage::set_data_dir_override(temp_dir.path().to_path_buf());
        temp_dir
    }

    fn cleanup_test_env_isolated() {
        crate::storage::clear_data_dir_override();
    }

    #[test]
    fn test_log_level_display() {
        assert_eq!(format!("{}", LogLevel::Info), "INFO");
        assert_eq!(format!("{}", LogLevel::Warn), "WARN");
        assert_eq!(format!("{}", LogLevel::Error), "ERROR");
    }

    #[test]

    fn test_get_session_log_path_with_override() {
        let temp_dir = setup_test_env_isolated();
        let path = get_session_log_path().unwrap();
        assert_eq!(path, temp_dir.path().join("session.log"));
        cleanup_test_env_isolated();
    }

    #[test]

    fn test_log_session_event_creates_file() {
        let temp_dir = setup_test_env_isolated();
        let log_path = temp_dir.path().join("session.log");

        log_session_event(LogLevel::Info, "Test message");

        assert!(log_path.exists());
        let content = fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("[INFO] Test message"));
        cleanup_test_env_isolated();
    }

    #[test]

    fn test_log_session_event_appends() {
        let temp_dir = setup_test_env_isolated();
        let log_path = temp_dir.path().join("session.log");

        log_session_event(LogLevel::Info, "First message");
        log_session_event(LogLevel::Warn, "Second message");
        log_session_event(LogLevel::Error, "Third message");

        let content = fs::read_to_string(&log_path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].contains("[INFO] First message"));
        assert!(lines[1].contains("[WARN] Second message"));
        assert!(lines[2].contains("[ERROR] Third message"));
        cleanup_test_env_isolated();
    }

    #[test]

    fn test_log_server_start() {
        let temp_dir = setup_test_env_isolated();
        let log_path = temp_dir.path().join("session.log");

        log_server_start("myrepo@main", "127.0.0.1", 55823, 12345);

        let content = fs::read_to_string(&log_path).unwrap();
        assert!(
            content.contains("Session server started: myrepo@main on 127.0.0.1:55823 (pid: 12345)")
        );
        cleanup_test_env_isolated();
    }

    #[test]

    fn test_log_server_stop() {
        let temp_dir = setup_test_env_isolated();
        let log_path = temp_dir.path().join("session.log");

        log_server_stop("myrepo@main", 12345);

        let content = fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("Session server stopped: myrepo@main (pid: 12345)"));
        cleanup_test_env_isolated();
    }

    #[test]

    fn test_log_gui_start() {
        let temp_dir = setup_test_env_isolated();
        let log_path = temp_dir.path().join("session.log");

        log_gui_start("/path/to/repo", "0.0.0.0", 55823, 54321, false);

        let content = fs::read_to_string(&log_path).unwrap();
        assert!(content.contains(
            "GUI server started: /path/to/repo on 0.0.0.0:55823 (pid: 54321, mode: read-write)"
        ));
        cleanup_test_env_isolated();
    }

    #[test]

    fn test_log_gui_start_readonly() {
        let temp_dir = setup_test_env_isolated();
        let log_path = temp_dir.path().join("session.log");

        log_gui_start("/path/to/repo", "0.0.0.0", 55823, 54321, true);

        let content = fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("mode: readonly"));
        cleanup_test_env_isolated();
    }

    #[test]

    fn test_log_auto_launch_attempt() {
        let temp_dir = setup_test_env_isolated();
        let log_path = temp_dir.path().join("session.log");

        log_auto_launch_attempt("GUI", "/workspace/myrepo");

        let content = fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("Auto-launch attempt from GUI: /workspace/myrepo"));
        cleanup_test_env_isolated();
    }

    #[test]

    fn test_log_auto_launch_success() {
        let temp_dir = setup_test_env_isolated();
        let log_path = temp_dir.path().join("session.log");

        log_auto_launch_success("TUI", "127.0.0.1", 3031, 99999);

        let content = fs::read_to_string(&log_path).unwrap();
        assert!(content.contains(
            "Auto-launch success from TUI: spawned server on 127.0.0.1:3031 (pid: 99999)"
        ));
        cleanup_test_env_isolated();
    }

    #[test]

    fn test_log_auto_launch_failure() {
        let temp_dir = setup_test_env_isolated();
        let log_path = temp_dir.path().join("session.log");

        log_auto_launch_failure("GUI", "Port already in use");

        let content = fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("Auto-launch failed from GUI: Port already in use"));
        cleanup_test_env_isolated();
    }
}
