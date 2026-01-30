//! Integration tests for tmux commands that actually invoke tmux.
//!
//! These tests are marked with `#[ignore]` and should be run with `--ignored`:
//! ```
//! cargo test --features tmux --test cli_tmux_integration_test -- --ignored
//! ```
//!
//! Requirements:
//! - tmux must be installed and available in PATH
//! - Tests must not be run inside an existing tmux session
//!
//! These tests complement the unit tests for command generation by verifying
//! real session creation/cleanup behavior.

#![cfg(feature = "tmux")]

mod common;

use common::TestEnv;
use predicates::prelude::*;
use std::fs;
use std::process::Command;

/// Generate a unique session name for testing to avoid conflicts.
fn unique_session_name(prefix: &str) -> String {
    format!(
        "bn-test-{}-{}",
        prefix,
        std::process::id()
            ^ (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u32)
    )
}

/// Check if tmux is available.
fn tmux_available() -> bool {
    Command::new("which")
        .arg("tmux")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Check if currently inside a tmux session.
fn in_tmux_session() -> bool {
    std::env::var("TMUX").is_ok()
}

/// Check if a tmux session exists.
fn session_exists(name: &str) -> bool {
    Command::new("tmux")
        .args(["has-session", "-t", name])
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Kill a tmux session if it exists.
fn kill_session(name: &str) {
    let _ = Command::new("tmux")
        .args(["kill-session", "-t", name])
        .output();
}

// ============================================================================
// Basic tmux availability tests
// ============================================================================

#[test]
#[ignore]
fn test_tmux_binary_check() {
    if !tmux_available() {
        panic!("tmux is not available - these tests require tmux to be installed");
    }

    // Verify version output works
    let output = Command::new("tmux")
        .arg("-V")
        .output()
        .expect("Failed to run tmux -V");
    assert!(output.status.success());

    let version = String::from_utf8_lossy(&output.stdout);
    assert!(
        version.contains("tmux"),
        "Expected 'tmux' in version output, got: {}",
        version
    );
}

// ============================================================================
// Session creation/cleanup tests
// ============================================================================

#[test]
#[ignore]
fn test_create_and_cleanup_session() {
    if !tmux_available() {
        return;
    }
    if in_tmux_session() {
        println!("Skipping: cannot run inside existing tmux session");
        return;
    }

    let session_name = unique_session_name("basic");

    // Ensure clean state
    kill_session(&session_name);
    assert!(!session_exists(&session_name));

    // Create a detached session
    let output = Command::new("tmux")
        .args(["new-session", "-d", "-s", &session_name])
        .output()
        .expect("Failed to create tmux session");
    assert!(output.status.success(), "Failed to create session");

    // Verify session exists
    assert!(session_exists(&session_name));

    // Cleanup
    kill_session(&session_name);
    assert!(!session_exists(&session_name));
}

#[test]
#[ignore]
fn test_session_with_directory() {
    if !tmux_available() {
        return;
    }
    if in_tmux_session() {
        println!("Skipping: cannot run inside existing tmux session");
        return;
    }

    let session_name = unique_session_name("dir");
    let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");

    // Ensure clean state
    kill_session(&session_name);

    // Create session with specific directory
    let output = Command::new("tmux")
        .args([
            "new-session",
            "-d",
            "-s",
            &session_name,
            "-c",
            temp_dir.path().to_str().unwrap(),
        ])
        .output()
        .expect("Failed to create tmux session");
    assert!(output.status.success());
    assert!(session_exists(&session_name));

    // Verify the pane's working directory
    let pwd_output = Command::new("tmux")
        .args([
            "display-message",
            "-t",
            &session_name,
            "-p",
            "#{pane_current_path}",
        ])
        .output()
        .expect("Failed to get pane path");

    let pane_path = String::from_utf8_lossy(&pwd_output.stdout)
        .trim()
        .to_string();

    // The paths should match (may need canonicalization on some systems)
    assert!(
        pane_path.contains(temp_dir.path().file_name().unwrap().to_str().unwrap())
            || std::fs::canonicalize(&pane_path)
                .ok()
                .as_ref()
                .and_then(|p| std::fs::canonicalize(temp_dir.path()).ok().map(|t| p == &t))
                .unwrap_or(false),
        "Expected pane path to match temp dir, got: {}",
        pane_path
    );

    // Cleanup
    kill_session(&session_name);
}

#[test]
#[ignore]
fn test_session_with_window_and_panes() {
    if !tmux_available() {
        return;
    }
    if in_tmux_session() {
        println!("Skipping: cannot run inside existing tmux session");
        return;
    }

    let session_name = unique_session_name("panes");

    // Ensure clean state
    kill_session(&session_name);

    // Create session
    let output = Command::new("tmux")
        .args(["new-session", "-d", "-s", &session_name])
        .output()
        .expect("Failed to create session");
    assert!(output.status.success());

    // Split window horizontally
    let output = Command::new("tmux")
        .args(["split-window", "-t", &session_name, "-h"])
        .output()
        .expect("Failed to split window");
    assert!(output.status.success());

    // Split first pane vertically
    let output = Command::new("tmux")
        .args(["split-window", "-t", &format!("{}:0.0", session_name), "-v"])
        .output()
        .expect("Failed to split pane");
    assert!(output.status.success());

    // Count panes - should be 3
    let output = Command::new("tmux")
        .args(["list-panes", "-t", &session_name, "-F", "#{pane_index}"])
        .output()
        .expect("Failed to list panes");

    let pane_count = String::from_utf8_lossy(&output.stdout).lines().count();
    assert_eq!(pane_count, 3, "Expected 3 panes, got {}", pane_count);

    // Cleanup
    kill_session(&session_name);
}

// ============================================================================
// Environment variable tests
// ============================================================================

#[test]
#[ignore]
fn test_session_environment_variable() {
    if !tmux_available() {
        return;
    }
    if in_tmux_session() {
        println!("Skipping: cannot run inside existing tmux session");
        return;
    }

    let session_name = unique_session_name("env");

    // Ensure clean state
    kill_session(&session_name);

    // Create session
    let output = Command::new("tmux")
        .args(["new-session", "-d", "-s", &session_name])
        .output()
        .expect("Failed to create session");
    assert!(output.status.success());

    // Set environment variable
    let test_value = "test-hash-12345";
    let output = Command::new("tmux")
        .args([
            "set-environment",
            "-t",
            &session_name,
            "BINNACLE_REPO_HASH",
            test_value,
        ])
        .output()
        .expect("Failed to set environment");
    assert!(output.status.success());

    // Get environment variable
    let output = Command::new("tmux")
        .args([
            "show-environment",
            "-t",
            &session_name,
            "BINNACLE_REPO_HASH",
        ])
        .output()
        .expect("Failed to get environment");

    let env_output = String::from_utf8_lossy(&output.stdout);
    assert!(
        env_output.contains(test_value),
        "Expected environment variable to contain '{}', got: {}",
        test_value,
        env_output
    );

    // Cleanup
    kill_session(&session_name);
}

// ============================================================================
// Window management tests
// ============================================================================

#[test]
#[ignore]
fn test_multiple_windows() {
    if !tmux_available() {
        return;
    }
    if in_tmux_session() {
        println!("Skipping: cannot run inside existing tmux session");
        return;
    }

    let session_name = unique_session_name("windows");

    // Ensure clean state
    kill_session(&session_name);

    // Create session with first window named "main"
    Command::new("tmux")
        .args(["new-session", "-d", "-s", &session_name, "-n", "main"])
        .output()
        .expect("Failed to create session");

    // Create second window
    Command::new("tmux")
        .args(["new-window", "-t", &session_name, "-n", "second"])
        .output()
        .expect("Failed to create window");

    // Create third window
    Command::new("tmux")
        .args(["new-window", "-t", &session_name, "-n", "third"])
        .output()
        .expect("Failed to create window");

    // List windows
    let output = Command::new("tmux")
        .args(["list-windows", "-t", &session_name, "-F", "#{window_name}"])
        .output()
        .expect("Failed to list windows");

    let windows: Vec<_> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.to_string())
        .collect();

    assert_eq!(windows.len(), 3);
    assert!(windows.contains(&"main".to_string()));
    assert!(windows.contains(&"second".to_string()));
    assert!(windows.contains(&"third".to_string()));

    // Cleanup
    kill_session(&session_name);
}

#[test]
#[ignore]
fn test_rename_window() {
    if !tmux_available() {
        return;
    }
    if in_tmux_session() {
        println!("Skipping: cannot run inside existing tmux session");
        return;
    }

    let session_name = unique_session_name("rename");

    // Ensure clean state
    kill_session(&session_name);

    // Create session
    Command::new("tmux")
        .args(["new-session", "-d", "-s", &session_name])
        .output()
        .expect("Failed to create session");

    // Rename the window
    Command::new("tmux")
        .args([
            "rename-window",
            "-t",
            &format!("{}:0", session_name),
            "editor",
        ])
        .output()
        .expect("Failed to rename window");

    // Get window name
    let output = Command::new("tmux")
        .args([
            "display-message",
            "-t",
            &format!("{}:0", session_name),
            "-p",
            "#{window_name}",
        ])
        .output()
        .expect("Failed to get window name");

    let window_name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(window_name, "editor");

    // Cleanup
    kill_session(&session_name);
}

// ============================================================================
// Send keys tests
// ============================================================================

#[test]
#[ignore]
fn test_send_keys() {
    if !tmux_available() {
        return;
    }
    if in_tmux_session() {
        println!("Skipping: cannot run inside existing tmux session");
        return;
    }

    let session_name = unique_session_name("keys");

    // Ensure clean state
    kill_session(&session_name);

    // Create session
    Command::new("tmux")
        .args(["new-session", "-d", "-s", &session_name])
        .output()
        .expect("Failed to create session");

    // Send a command (don't execute, just type it)
    Command::new("tmux")
        .args(["send-keys", "-t", &session_name, "-l", "echo hello world"])
        .output()
        .expect("Failed to send keys");

    // Give tmux a moment to process
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Capture pane content
    let output = Command::new("tmux")
        .args(["capture-pane", "-t", &session_name, "-p"])
        .output()
        .expect("Failed to capture pane");

    let content = String::from_utf8_lossy(&output.stdout);
    assert!(
        content.contains("echo hello world"),
        "Expected pane to contain 'echo hello world', got: {}",
        content
    );

    // Cleanup
    kill_session(&session_name);
}

// ============================================================================
// bn tmux CLI integration tests
// ============================================================================

#[test]
#[ignore]
fn test_bn_tmux_list_empty() {
    if !tmux_available() {
        return;
    }

    let env = TestEnv::init();

    // List layouts should work even with no layouts
    env.bn().args(["tmux", "list"]).assert().success();
}

#[test]
#[ignore]
fn test_bn_tmux_save_requires_tmux_session() {
    if !tmux_available() {
        return;
    }
    // Only run this test if NOT inside tmux
    if in_tmux_session() {
        println!("Skipping: test requires being outside tmux session");
        return;
    }

    let env = TestEnv::init();

    // Save should fail when not in a tmux session
    env.bn()
        .args(["tmux", "save"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("tmux session"));
}

#[test]
#[ignore]
fn test_bn_tmux_show_nonexistent() {
    if !tmux_available() {
        return;
    }

    let env = TestEnv::init();

    // Show should fail for non-existent layout
    env.bn()
        .args(["tmux", "show", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found").or(predicate::str::contains("No layout")));
}

#[test]
#[ignore]
fn test_bn_tmux_load_with_kdl_file() {
    if !tmux_available() {
        return;
    }
    if in_tmux_session() {
        println!("Skipping: cannot run inside existing tmux session");
        return;
    }

    let env = TestEnv::init();
    let session_name = unique_session_name("load");

    // Create a layout file
    let tmux_dir = env.repo_path().join(".binnacle").join("tmux");
    fs::create_dir_all(&tmux_dir).expect("Failed to create tmux dir");

    let layout_kdl = format!(
        r#"layout "{}" {{
    window "main" {{
        pane dir="."
    }}
}}"#,
        session_name
    );
    fs::write(tmux_dir.join(format!("{}.kdl", session_name)), layout_kdl)
        .expect("Failed to write layout file");

    // Ensure clean state
    kill_session(&session_name);

    // Load the layout
    env.bn()
        .args(["tmux", "load", &session_name])
        .assert()
        .success();

    // Verify session was created
    assert!(
        session_exists(&session_name),
        "Session '{}' should exist after load",
        session_name
    );

    // Cleanup
    kill_session(&session_name);
}

#[test]
#[ignore]
fn test_bn_tmux_list_shows_project_layouts() {
    if !tmux_available() {
        return;
    }

    let env = TestEnv::init();

    // Create a layout file
    let tmux_dir = env.repo_path().join(".binnacle").join("tmux");
    fs::create_dir_all(&tmux_dir).expect("Failed to create tmux dir");

    fs::write(
        tmux_dir.join("dev.kdl"),
        r#"layout "dev" { window "main" { pane dir="." } }"#,
    )
    .expect("Failed to write layout file");

    // List should show the layout
    env.bn()
        .args(["tmux", "list", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("dev"));
}

#[test]
#[ignore]
fn test_bn_tmux_show_displays_layout_info() {
    if !tmux_available() {
        return;
    }

    let env = TestEnv::init();

    // Create a layout file with multiple windows
    let tmux_dir = env.repo_path().join(".binnacle").join("tmux");
    fs::create_dir_all(&tmux_dir).expect("Failed to create tmux dir");

    let layout_kdl = r#"layout "mydev" {
    window "editor" {
        pane dir="./src" {
            cmd "nvim"
        }
    }
    window "terminal" {
        pane dir="."
    }
}"#;
    fs::write(tmux_dir.join("mydev.kdl"), layout_kdl).expect("Failed to write layout file");

    // Show should display layout information
    env.bn()
        .args(["tmux", "show", "mydev", "-H"])
        .assert()
        .success()
        .stdout(predicate::str::contains("editor"))
        .stdout(predicate::str::contains("terminal"));
}
