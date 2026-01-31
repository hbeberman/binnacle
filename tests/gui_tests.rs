//! Integration tests for GUI feature
//!
//! These tests verify the GUI command line interface and basic functionality.

mod common;

#[cfg(feature = "gui")]
mod gui_enabled {
    use super::common::TestEnv;
    use assert_cmd::Command;

    /// Helper to create an isolated bn command (without TestEnv).
    /// Sets a temporary data directory to prevent test data leaking into production.
    fn bn_isolated() -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        // Set isolated data directory to prevent polluting host's binnacle data
        // Note: TempDir is dropped immediately, but the path is captured in the env var
        // For simple commands like --help this is fine. For commands that need the dir,
        // use TestEnv instead.
        let temp_dir = tempfile::tempdir().unwrap();
        cmd.env("BN_DATA_DIR", temp_dir.path());
        cmd.env_remove("BN_CONTAINER_MODE");
        cmd.env_remove("BN_AGENT_ID");
        cmd.env_remove("BN_AGENT_NAME");
        cmd.env_remove("BN_AGENT_TYPE");
        cmd.env_remove("BN_MCP_SESSION");
        cmd.env_remove("BN_AGENT_SESSION");

        // Keep temp_dir alive until the end of this function
        // so the path exists when the command runs
        std::mem::forget(temp_dir); // Intentionally leak to keep path valid
        cmd
    }

    #[test]
    fn test_gui_help() {
        let mut cmd = bn_isolated();
        cmd.arg("gui").arg("--help");
        cmd.assert()
            .success()
            .stdout(predicates::str::contains("Web GUI management"));
    }

    #[test]
    fn test_gui_requires_init() {
        let env = TestEnv::new();
        env.bn()
            .arg("gui")
            .assert()
            .failure()
            .stderr(predicates::str::contains("Not initialized"));
    }

    #[test]
    fn test_gui_serve_custom_port_parsing() {
        // Just test that the CLI accepts a custom port argument
        let mut cmd = bn_isolated();
        cmd.arg("gui")
            .arg("serve")
            .arg("--port")
            .arg("8080")
            .arg("--help");
        cmd.assert().success();
    }

    #[test]
    fn test_gui_stop_subcommand_in_help() {
        let mut cmd = bn_isolated();
        cmd.arg("gui").arg("--help");
        cmd.assert()
            .success()
            .stdout(predicates::str::contains("stop"))
            .stdout(predicates::str::contains("gracefully"));
    }

    #[test]
    fn test_gui_stop_when_not_running_json() {
        let env = TestEnv::init();

        // Stop should report not running (JSON)
        env.bn()
            .args(["gui", "stop"])
            .assert()
            .success()
            .stdout(predicates::str::contains(r#""status":"not_running""#));
    }

    #[test]
    fn test_gui_stop_when_not_running_human() {
        let env = TestEnv::init();

        // Stop should report not running (human-readable)
        env.bn()
            .args(["gui", "stop", "-H"])
            .assert()
            .success()
            .stdout(predicates::str::contains("not running"));
    }

    #[test]
    fn test_gui_serve_replace_in_help() {
        let mut cmd = bn_isolated();
        cmd.arg("gui").arg("serve").arg("--help");
        cmd.assert()
            .success()
            .stdout(predicates::str::contains("--replace"))
            .stdout(predicates::str::contains(
                "Stop any running GUI server first",
            ));
    }

    #[test]
    fn test_gui_serve_replace_requires_init() {
        let temp = tempfile::tempdir().unwrap();

        // serve --replace should also require initialization
        let mut cmd = bn_isolated();
        cmd.current_dir(&temp);
        cmd.arg("gui").arg("serve").arg("--replace");
        cmd.assert()
            .failure()
            .stderr(predicates::str::contains("Not initialized"));
    }

    #[test]
    fn test_gui_status_subcommand_in_help() {
        let mut cmd = bn_isolated();
        cmd.arg("gui").arg("--help");
        cmd.assert()
            .success()
            .stdout(predicates::str::contains("status"))
            .stdout(predicates::str::contains("Show status"));
    }

    #[test]
    fn test_gui_status_when_not_running_json() {
        let env = TestEnv::init();

        // Status should report not running (JSON)
        env.bn()
            .args(["gui", "status"])
            .assert()
            .success()
            .stdout(predicates::str::contains(r#""status":"not_running""#));
    }

    #[test]
    fn test_gui_status_when_not_running_human() {
        let env = TestEnv::init();

        // Status should report not running (human-readable)
        env.bn()
            .args(["gui", "status", "-H"])
            .assert()
            .success()
            .stdout(predicates::str::contains("not running"));
    }

    #[test]
    fn test_gui_status_works_without_init() {
        let temp = tempfile::tempdir().unwrap();

        // status should work even without initialization
        // (it just checks for a PID file, and reports not running if none exists)
        let mut cmd = bn_isolated();
        cmd.current_dir(&temp);
        cmd.arg("gui").arg("status");
        cmd.assert()
            .success()
            .stdout(predicates::str::contains(r#""status":"not_running""#));
    }

    /// Helper to find the storage directory created by bn (finds the hash directory in data_path).
    /// This is needed because the command may compute a different hash than expected due to
    /// path differences (e.g., canonicalization, git root detection).
    fn find_storage_dir_in_data_path(data_path: &std::path::Path) -> std::path::PathBuf {
        let entries = std::fs::read_dir(data_path).expect("Should be able to read data_path");
        for entry in entries.flatten() {
            let path = entry.path();
            // Hash directories are 12 characters long
            if path.is_dir() && path.file_name().is_some_and(|n| n.len() == 12) {
                return path;
            }
        }
        panic!("No hash directory found in data_path: {:?}", data_path);
    }

    #[test]
    fn test_gui_status_with_stale_pid_file() {
        let env = TestEnv::new();

        // Initialize binnacle first
        env.bn()
            .args(["session", "init", "--auto-global", "-y"])
            .assert()
            .success();

        // Find the storage dir that was created and add a stale PID file
        let storage_dir = find_storage_dir_in_data_path(env.data_path());
        let pid_file_path = storage_dir.join("gui.pid");
        std::fs::write(&pid_file_path, "PID=999999999\nPORT=3030\nHOST=127.0.0.1\n").unwrap();

        // Status should detect the stale PID
        let output = env.bn().args(["gui", "status"]).assert().success();
        let stdout = String::from_utf8_lossy(&output.get_output().stdout);
        // Should show not_running or stale status
        assert!(
            stdout.contains("not_running") || stdout.contains("stale"),
            "Expected not_running or stale status, got: {}",
            stdout
        );
    }

    #[test]
    fn test_gui_stop_cleans_stale_pid_file() {
        let env = TestEnv::new();

        // Initialize binnacle first
        env.bn()
            .args(["session", "init", "--auto-global", "-y"])
            .assert()
            .success();

        // Find the storage dir that was created
        let storage_dir = find_storage_dir_in_data_path(env.data_path());
        let pid_file_path = storage_dir.join("gui.pid");
        std::fs::write(&pid_file_path, "PID=999999999\nPORT=3030\nHOST=127.0.0.1\n").unwrap();

        // Stop should clean up the stale PID file
        env.bn()
            .args(["gui", "stop"])
            .assert()
            .success()
            .stdout(predicates::str::contains("not_running"));

        // PID file should be cleaned up
        assert!(
            !pid_file_path.exists(),
            "Stale PID file should have been deleted"
        );
    }

    #[test]
    fn test_gui_stop_cleans_stale_pid_file_human() {
        let env = TestEnv::new();

        // Initialize binnacle first
        env.bn()
            .args(["session", "init", "--auto-global", "-y"])
            .assert()
            .success();

        // Find the storage dir and create a stale PID file
        let storage_dir = find_storage_dir_in_data_path(env.data_path());
        let pid_file_path = storage_dir.join("gui.pid");
        std::fs::write(&pid_file_path, "PID=999999999\nPORT=3030\nHOST=127.0.0.1\n").unwrap();

        // Stop should clean up the stale PID file and report it
        env.bn()
            .args(["gui", "stop", "-H"])
            .assert()
            .success()
            .stdout(predicates::str::contains("not running"));

        // PID file should be cleaned up
        assert!(
            !pid_file_path.exists(),
            "Stale PID file should have been deleted"
        );
    }

    #[test]
    fn test_gui_serve_host_flag_in_help() {
        let mut cmd = bn_isolated();
        cmd.arg("gui").arg("serve").arg("--help");
        cmd.assert()
            .success()
            .stdout(predicates::str::contains("--host"))
            .stdout(predicates::str::contains("0.0.0.0"));
    }

    #[test]
    fn test_gui_export_includes_state_js() {
        let env = TestEnv::new();

        // Initialize binnacle
        env.bn()
            .args(["session", "init", "--auto-global", "-y"])
            .assert()
            .success();

        // Export GUI to temporary output directory
        let output_dir = tempfile::tempdir().unwrap();
        let output = env
            .bn()
            .args(["gui", "export", "-o"])
            .arg(output_dir.path())
            .assert()
            .success();

        let stdout = String::from_utf8_lossy(&output.get_output().stdout);
        assert!(
            stdout.contains(r#""status":"success""#),
            "Export should succeed"
        );

        // Verify state.js exists in the exported bundle
        let state_js_path = output_dir.path().join("js").join("state.js");
        assert!(
            state_js_path.exists(),
            "state.js should be included in exported bundle at {:?}",
            state_js_path
        );

        // Verify it's a valid JavaScript file with exports
        let state_js_content =
            std::fs::read_to_string(&state_js_path).expect("Should be able to read state.js");
        assert!(
            state_js_content.contains("export"),
            "state.js should contain exports (valid ES module)"
        );

        // Verify it's not empty and has reasonable size
        let metadata =
            std::fs::metadata(&state_js_path).expect("Should be able to get state.js metadata");
        assert!(
            metadata.len() > 1000,
            "state.js should be at least 1KB (was {} bytes)",
            metadata.len()
        );
    }

    #[test]
    fn test_gui_state_js_no_self_import() {
        let env = TestEnv::new();

        // Initialize binnacle
        env.bn()
            .args(["session", "init", "--auto-global", "-y"])
            .assert()
            .success();

        // Export GUI to temporary output directory
        let output_dir = tempfile::tempdir().unwrap();
        env.bn()
            .args(["gui", "export", "-o"])
            .arg(output_dir.path())
            .assert()
            .success();

        // Read state.js content
        let state_js_path = output_dir.path().join("js").join("state.js");
        let state_js_content =
            std::fs::read_to_string(&state_js_path).expect("Should be able to read state.js");

        // Regression test for bn-021e: state.js should NOT import itself
        // The bug was that esbuild was adding `import*as _ from"../state.js";` to bundled state.js
        // We fixed this by copying state.js as-is instead of bundling it
        assert!(
            !state_js_content.contains("import*as"),
            "state.js should not contain minified import statements (should be unbundled)"
        );
        assert!(
            !state_js_content.contains(r#"from"../state.js""#),
            "state.js should not import itself via '../state.js'"
        );
        assert!(
            !state_js_content.contains(r#"from"./state.js""#),
            "state.js should not import itself via './state.js'"
        );

        // Verify it's the original source (not minified/bundled)
        assert!(
            state_js_content.contains("/**"),
            "state.js should contain doc comments (not minified)"
        );
        assert!(
            state_js_content.contains("Connection modes"),
            "state.js should contain original comments"
        );
    }

    #[test]
    fn test_gui_serve_port_flag_in_help() {
        let mut cmd = bn_isolated();
        cmd.arg("gui").arg("serve").arg("--help");
        cmd.assert()
            .success()
            .stdout(predicates::str::contains("--port"))
            .stdout(predicates::str::contains("3030"));
    }

    #[test]
    fn test_gui_kill_subcommand_in_help() {
        let mut cmd = bn_isolated();
        cmd.arg("gui").arg("--help");
        cmd.assert()
            .success()
            .stdout(predicates::str::contains("kill"))
            .stdout(predicates::str::contains("immediately"));
    }

    #[test]
    fn test_gui_kill_force_flag_in_help() {
        let mut cmd = bn_isolated();
        cmd.arg("gui").arg("kill").arg("--help");
        cmd.assert()
            .success()
            .stdout(predicates::str::contains("-9"))
            .stdout(predicates::str::contains("--force"))
            .stdout(predicates::str::contains("SIGKILL"));
    }

    #[test]
    fn test_gui_kill_when_not_running() {
        let env = TestEnv::init();

        // Kill should report not running (JSON)
        env.bn()
            .args(["gui", "kill"])
            .assert()
            .success()
            .stdout(predicates::str::contains(r#""status":"not_running""#));
    }

    #[test]
    fn test_gui_kill_force_when_not_running() {
        let env = TestEnv::init();

        // Kill --force should also report not running
        env.bn()
            .args(["gui", "kill", "--force"])
            .assert()
            .success()
            .stdout(predicates::str::contains(r#""status":"not_running""#));
    }

    #[test]
    fn test_gui_kill_9_flag_when_not_running() {
        let env = TestEnv::init();

        // Kill -9 should also work
        env.bn()
            .args(["gui", "kill", "-9"])
            .assert()
            .success()
            .stdout(predicates::str::contains(r#""status":"not_running""#));
    }

    #[test]
    fn test_gui_kill_cleans_stale_pid_file() {
        let env = TestEnv::new();

        // Initialize binnacle first
        env.bn()
            .args(["session", "init", "--auto-global", "-y"])
            .assert()
            .success();

        // Find the storage dir and create a stale PID file
        let storage_dir = find_storage_dir_in_data_path(env.data_path());
        let pid_file_path = storage_dir.join("gui.pid");
        std::fs::write(&pid_file_path, "PID=999999999\nPORT=3030\nHOST=127.0.0.1\n").unwrap();

        // Kill should clean up the stale PID file
        env.bn()
            .args(["gui", "kill"])
            .assert()
            .success()
            .stdout(predicates::str::contains("not_running"));

        // PID file should be cleaned up
        assert!(
            !pid_file_path.exists(),
            "Stale PID file should have been deleted"
        );
    }

    #[test]
    fn test_gui_kill_when_not_running_human() {
        let env = TestEnv::init();

        // Kill should report not running (human-readable)
        env.bn()
            .args(["gui", "kill", "-H"])
            .assert()
            .success()
            .stdout(predicates::str::contains("not running"));
    }

    #[test]
    fn test_gui_kill_force_when_not_running_human() {
        let env = TestEnv::init();

        // Kill --force should also report not running (human-readable)
        env.bn()
            .args(["gui", "kill", "--force", "-H"])
            .assert()
            .success()
            .stdout(predicates::str::contains("not running"));
    }

    #[test]
    fn test_gui_kill_9_flag_when_not_running_human() {
        let env = TestEnv::init();

        // Kill -9 -H should report not running (human-readable)
        env.bn()
            .args(["gui", "kill", "-9", "-H"])
            .assert()
            .success()
            .stdout(predicates::str::contains("not running"));
    }

    #[test]
    fn test_gui_kill_cleans_stale_pid_file_human() {
        let env = TestEnv::new();

        // Initialize binnacle first
        env.bn()
            .args(["session", "init", "--auto-global", "-y"])
            .assert()
            .success();

        // Find the storage dir and create a stale PID file
        let storage_dir = find_storage_dir_in_data_path(env.data_path());
        let pid_file_path = storage_dir.join("gui.pid");
        std::fs::write(&pid_file_path, "PID=999999999\nPORT=3030\nHOST=127.0.0.1\n").unwrap();

        // Kill should clean up the stale PID file (human-readable)
        env.bn()
            .args(["gui", "kill", "-H"])
            .assert()
            .success()
            .stdout(predicates::str::contains("not running"))
            .stdout(predicates::str::contains("cleaned up stale"));

        // PID file should be cleaned up
        assert!(
            !pid_file_path.exists(),
            "Stale PID file should have been deleted"
        );
    }

    #[test]
    fn test_gui_kill_json_output_structure_not_running() {
        let env = TestEnv::init();

        // Verify JSON structure when not running (no PID file)
        let output = env.bn().args(["gui", "kill"]).assert().success();
        let stdout = String::from_utf8_lossy(&output.get_output().stdout);
        let json: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();

        assert_eq!(json["status"], "not_running");
        // When no PID file exists, cleaned_stale should not be present
        assert!(json.get("cleaned_stale").is_none());
    }

    #[test]
    fn test_gui_kill_json_output_structure_stale_pid() {
        let env = TestEnv::new();

        // Initialize binnacle first
        env.bn()
            .args(["session", "init", "--auto-global", "-y"])
            .assert()
            .success();

        // Find the storage dir and create a stale PID file
        let storage_dir = find_storage_dir_in_data_path(env.data_path());
        let pid_file_path = storage_dir.join("gui.pid");
        std::fs::write(&pid_file_path, "PID=999999999\nPORT=3030\nHOST=127.0.0.1\n").unwrap();

        // Verify JSON structure when stale PID file exists
        let output = env.bn().args(["gui", "kill"]).assert().success();
        let stdout = String::from_utf8_lossy(&output.get_output().stdout);
        let json: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();

        assert_eq!(json["status"], "not_running");
        assert_eq!(json["cleaned_stale"], true);
    }

    #[test]
    fn test_gui_kill_force_cleans_stale_pid_file() {
        let env = TestEnv::new();

        // Initialize binnacle first
        env.bn()
            .args(["session", "init", "--auto-global", "-y"])
            .assert()
            .success();

        // Find the storage dir and create a stale PID file
        let storage_dir = find_storage_dir_in_data_path(env.data_path());
        let pid_file_path = storage_dir.join("gui.pid");
        std::fs::write(&pid_file_path, "PID=999999999\nPORT=3030\nHOST=127.0.0.1\n").unwrap();

        // Kill --force should also clean up the stale PID file
        env.bn()
            .args(["gui", "kill", "--force"])
            .assert()
            .success()
            .stdout(predicates::str::contains("not_running"));

        // PID file should be cleaned up
        assert!(
            !pid_file_path.exists(),
            "Stale PID file should have been deleted"
        );
    }

    #[test]
    fn test_gui_kill_works_without_init() {
        let temp = tempfile::tempdir().unwrap();

        // Kill should work even without initialization
        // (it just checks for a PID file, and reports not running if none exists)
        let mut cmd = bn_isolated();
        cmd.current_dir(&temp);
        cmd.arg("gui").arg("kill");
        cmd.assert()
            .success()
            .stdout(predicates::str::contains(r#""status":"not_running""#));
    }
}

#[cfg(not(feature = "gui"))]
mod gui_disabled {
    use assert_cmd::Command;

    /// Helper to create an isolated bn command (without TestEnv).
    /// Sets a temporary data directory to prevent test data leaking into production.
    fn bn_isolated() -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        // Set isolated data directory to prevent polluting host's binnacle data
        let temp_dir = tempfile::tempdir().unwrap();
        cmd.env("BN_DATA_DIR", temp_dir.path());
        cmd.env_remove("BN_CONTAINER_MODE");
        cmd.env_remove("BN_AGENT_ID");
        cmd.env_remove("BN_AGENT_NAME");
        cmd.env_remove("BN_AGENT_TYPE");
        cmd.env_remove("BN_MCP_SESSION");
        cmd.env_remove("BN_AGENT_SESSION");

        std::mem::forget(temp_dir); // Intentionally leak to keep path valid
        cmd
    }

    #[test]
    fn test_gui_command_not_available() {
        let mut cmd = bn_isolated();
        cmd.arg("gui").arg("--help");
        // When GUI feature is disabled, the command should not exist
        cmd.assert().failure();
    }
}
