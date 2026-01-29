// Tests for summarize session API endpoints

use std::process::Command;
use std::time::Duration;
use tempfile::TempDir;

// Helper to setup a test repository with isolated data directory
fn setup_test_repo() -> (TempDir, TempDir) {
    let repo_temp = TempDir::new().unwrap();
    let data_temp = TempDir::new().unwrap();
    let repo_path = repo_temp.path();

    // Initialize git repo
    Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to init git repo");

    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to set git user.name");

    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to set git user.email");

    // Initialize binnacle with isolated data directory
    let bn = env!("CARGO_BIN_EXE_bn");
    Command::new(bn)
        .args(["system", "init"])
        .current_dir(repo_path)
        .env("BN_DATA_DIR", data_temp.path())
        .output()
        .expect("Failed to init binnacle");

    (repo_temp, data_temp)
}

#[test]
fn test_summarize_endpoints_exist() {
    let (temp, data_temp) = setup_test_repo();
    let repo_path = temp.path();

    // Start GUI server in readonly mode on a high port
    let bn = env!("CARGO_BIN_EXE_bn");
    let port = 33333; // Use a high port to avoid conflicts
    let mut child = Command::new(bn)
        .args(["gui", "serve", "--port", &port.to_string(), "--readonly"])
        .current_dir(repo_path)
        .env("BN_DATA_DIR", data_temp.path())
        .spawn()
        .expect("Failed to start GUI server");

    // Wait for server to start
    std::thread::sleep(Duration::from_secs(2));

    // Test that endpoints respond (using curl)
    let base_url = format!("http://127.0.0.1:{}", port);

    // Test /api/summarize/start (should reject in readonly mode)
    let output = Command::new("curl")
        .args([
            "-X",
            "POST",
            "-H",
            "Content-Type: application/json",
            "-d",
            r#"{"context": {}}"#,
            &format!("{}/api/summarize/start", base_url),
            "-s",
        ])
        .output();

    if let Ok(output) = output {
        let response = String::from_utf8_lossy(&output.stdout);
        // Should get forbidden response in readonly mode
        assert!(
            response.contains("readonly") || response.contains("forbidden"),
            "Expected readonly error, got: {}",
            response
        );
    }

    // Kill the server
    child.kill().ok();
    child.wait().ok();
}
