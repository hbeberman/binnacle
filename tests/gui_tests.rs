//! Integration tests for GUI feature
//!
//! These tests verify the GUI command line interface and basic functionality.

mod common;

#[cfg(feature = "gui")]
mod gui_enabled {
    use super::common::TestEnv;
    use assert_cmd::Command;

    #[test]
    fn test_gui_help() {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
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
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.arg("gui")
            .arg("serve")
            .arg("--port")
            .arg("8080")
            .arg("--help");
        cmd.assert().success();
    }

    #[test]
    fn test_gui_stop_subcommand_in_help() {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.arg("gui").arg("--help");
        cmd.assert()
            .success()
            .stdout(predicates::str::contains("stop"))
            .stdout(predicates::str::contains("gracefully"));
    }

    #[test]
    fn test_gui_stop_when_not_running_json() {
        let temp = tempfile::tempdir().unwrap();

        // Initialize binnacle first
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(&temp);
        cmd.arg("system").arg("init");
        cmd.assert().success();

        // Stop should report not running (JSON)
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(&temp);
        cmd.arg("gui").arg("stop");
        cmd.assert()
            .success()
            .stdout(predicates::str::contains(r#""status":"not_running""#));
    }

    #[test]
    fn test_gui_stop_when_not_running_human() {
        let temp = tempfile::tempdir().unwrap();

        // Initialize binnacle first
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(&temp);
        cmd.arg("system").arg("init");
        cmd.assert().success();

        // Stop should report not running (human-readable)
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(&temp);
        cmd.arg("gui").arg("stop").arg("-H");
        cmd.assert()
            .success()
            .stdout(predicates::str::contains("not running"));
    }

    #[test]
    fn test_gui_serve_replace_in_help() {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
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
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(&temp);
        cmd.arg("gui").arg("serve").arg("--replace");
        cmd.assert()
            .failure()
            .stderr(predicates::str::contains("Not initialized"));
    }

    #[test]
    fn test_gui_status_subcommand_in_help() {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.arg("gui").arg("--help");
        cmd.assert()
            .success()
            .stdout(predicates::str::contains("status"))
            .stdout(predicates::str::contains("Show status"));
    }

    #[test]
    fn test_gui_status_when_not_running_json() {
        let temp = tempfile::tempdir().unwrap();

        // Initialize binnacle first
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(&temp);
        cmd.arg("system").arg("init");
        cmd.assert().success();

        // Status should report not running (JSON)
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(&temp);
        cmd.arg("gui").arg("status");
        cmd.assert()
            .success()
            .stdout(predicates::str::contains(r#""status":"not_running""#));
    }

    #[test]
    fn test_gui_status_when_not_running_human() {
        let temp = tempfile::tempdir().unwrap();

        // Initialize binnacle first
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(&temp);
        cmd.arg("system").arg("init");
        cmd.assert().success();

        // Status should report not running (human-readable)
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(&temp);
        cmd.arg("gui").arg("status").arg("-H");
        cmd.assert()
            .success()
            .stdout(predicates::str::contains("not running"));
    }

    #[test]
    fn test_gui_status_works_without_init() {
        let temp = tempfile::tempdir().unwrap();

        // status should work even without initialization
        // (it just checks for a PID file, and reports not running if none exists)
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(&temp);
        cmd.arg("gui").arg("status");
        cmd.assert()
            .success()
            .stdout(predicates::str::contains(r#""status":"not_running""#));
    }

    #[test]
    fn test_gui_status_with_stale_pid_file() {
        let temp = tempfile::tempdir().unwrap();

        // Initialize binnacle first
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(&temp);
        cmd.arg("system").arg("init");
        cmd.assert().success();

        // Create a stale PID file manually (PID that doesn't exist)
        // First we need to find the storage directory
        let storage_dir = binnacle::storage::get_storage_dir(temp.path()).unwrap();
        let pid_file_path = storage_dir.join("gui.pid");
        std::fs::create_dir_all(&storage_dir).unwrap();
        std::fs::write(&pid_file_path, "PID=999999999\nPORT=3030\nHOST=127.0.0.1\n").unwrap();

        // Status should detect the stale PID
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(&temp);
        cmd.arg("gui").arg("status");
        let output = cmd.assert().success();
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
        let temp = tempfile::tempdir().unwrap();

        // Initialize binnacle first
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(&temp);
        cmd.arg("system").arg("init");
        cmd.assert().success();

        // Create a stale PID file manually
        let storage_dir = binnacle::storage::get_storage_dir(temp.path()).unwrap();
        let pid_file_path = storage_dir.join("gui.pid");
        std::fs::create_dir_all(&storage_dir).unwrap();
        std::fs::write(&pid_file_path, "PID=999999999\nPORT=3030\nHOST=127.0.0.1\n").unwrap();

        // Stop should clean up the stale PID file
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(&temp);
        cmd.arg("gui").arg("stop");
        cmd.assert()
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
        let temp = tempfile::tempdir().unwrap();

        // Initialize binnacle first
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(&temp);
        cmd.arg("system").arg("init");
        cmd.assert().success();

        // Create a stale PID file manually
        let storage_dir = binnacle::storage::get_storage_dir(temp.path()).unwrap();
        let pid_file_path = storage_dir.join("gui.pid");
        std::fs::create_dir_all(&storage_dir).unwrap();
        std::fs::write(&pid_file_path, "PID=999999999\nPORT=3030\nHOST=127.0.0.1\n").unwrap();

        // Stop should clean up the stale PID file and report it
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(&temp);
        cmd.arg("gui").arg("stop").arg("-H");
        cmd.assert()
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
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.arg("gui").arg("serve").arg("--help");
        cmd.assert()
            .success()
            .stdout(predicates::str::contains("--host"))
            .stdout(predicates::str::contains("0.0.0.0"));
    }

    #[test]
    fn test_gui_serve_port_flag_in_help() {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.arg("gui").arg("serve").arg("--help");
        cmd.assert()
            .success()
            .stdout(predicates::str::contains("--port"))
            .stdout(predicates::str::contains("3030"));
    }

    #[test]
    fn test_gui_kill_subcommand_in_help() {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.arg("gui").arg("--help");
        cmd.assert()
            .success()
            .stdout(predicates::str::contains("kill"))
            .stdout(predicates::str::contains("immediately"));
    }

    #[test]
    fn test_gui_kill_force_flag_in_help() {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.arg("gui").arg("kill").arg("--help");
        cmd.assert()
            .success()
            .stdout(predicates::str::contains("-9"))
            .stdout(predicates::str::contains("--force"))
            .stdout(predicates::str::contains("SIGKILL"));
    }

    #[test]
    fn test_gui_kill_when_not_running() {
        let temp = tempfile::tempdir().unwrap();

        // Initialize binnacle first
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(&temp);
        cmd.arg("system").arg("init");
        cmd.assert().success();

        // Kill should report not running (JSON)
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(&temp);
        cmd.arg("gui").arg("kill");
        cmd.assert()
            .success()
            .stdout(predicates::str::contains(r#""status":"not_running""#));
    }

    #[test]
    fn test_gui_kill_force_when_not_running() {
        let temp = tempfile::tempdir().unwrap();

        // Initialize binnacle first
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(&temp);
        cmd.arg("system").arg("init");
        cmd.assert().success();

        // Kill --force should also report not running
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(&temp);
        cmd.arg("gui").arg("kill").arg("--force");
        cmd.assert()
            .success()
            .stdout(predicates::str::contains(r#""status":"not_running""#));
    }

    #[test]
    fn test_gui_kill_9_flag_when_not_running() {
        let temp = tempfile::tempdir().unwrap();

        // Initialize binnacle first
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(&temp);
        cmd.arg("system").arg("init");
        cmd.assert().success();

        // Kill -9 should also work
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(&temp);
        cmd.arg("gui").arg("kill").arg("-9");
        cmd.assert()
            .success()
            .stdout(predicates::str::contains(r#""status":"not_running""#));
    }

    #[test]
    fn test_gui_kill_cleans_stale_pid_file() {
        let temp = tempfile::tempdir().unwrap();

        // Initialize binnacle first
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(&temp);
        cmd.arg("system").arg("init");
        cmd.assert().success();

        // Create a stale PID file manually
        let storage_dir = binnacle::storage::get_storage_dir(temp.path()).unwrap();
        let pid_file_path = storage_dir.join("gui.pid");
        std::fs::create_dir_all(&storage_dir).unwrap();
        std::fs::write(&pid_file_path, "PID=999999999\nPORT=3030\nHOST=127.0.0.1\n").unwrap();

        // Kill should clean up the stale PID file
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(&temp);
        cmd.arg("gui").arg("kill");
        cmd.assert()
            .success()
            .stdout(predicates::str::contains("not_running"));

        // PID file should be cleaned up
        assert!(
            !pid_file_path.exists(),
            "Stale PID file should have been deleted"
        );
    }
}

#[cfg(not(feature = "gui"))]
mod gui_disabled {
    use assert_cmd::Command;

    #[test]
    fn test_gui_command_not_available() {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.arg("gui").arg("--help");
        // When GUI feature is disabled, the command should not exist
        cmd.assert().failure();
    }
}
