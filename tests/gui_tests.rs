//! Integration tests for GUI feature
//!
//! These tests verify the GUI command line interface and basic functionality.

#[cfg(feature = "gui")]
mod gui_enabled {
    use assert_cmd::Command;

    #[test]
    fn test_gui_help() {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.arg("gui").arg("--help");
        cmd.assert()
            .success()
            .stdout(predicates::str::contains("Start the web GUI"));
    }

    #[test]
    fn test_gui_requires_init() {
        let temp = tempfile::tempdir().unwrap();
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(&temp);
        cmd.arg("gui");
        cmd.assert()
            .failure()
            .stderr(predicates::str::contains("Not initialized"));
    }

    #[test]
    fn test_gui_custom_port_parsing() {
        // Just test that the CLI accepts a custom port argument
        // (actual server won't start in test, but parsing should work)
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.arg("gui").arg("--port").arg("8080").arg("--help");
        cmd.assert().success();
    }

    #[test]
    fn test_gui_stop_flag_in_help() {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.arg("gui").arg("--help");
        cmd.assert()
            .success()
            .stdout(predicates::str::contains("--stop"))
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
        cmd.arg("gui").arg("--stop");
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
        cmd.arg("gui").arg("--stop").arg("-H");
        cmd.assert()
            .success()
            .stdout(predicates::str::contains("not running"));
    }

    #[test]
    fn test_gui_replace_flag_in_help() {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.arg("gui").arg("--help");
        cmd.assert()
            .success()
            .stdout(predicates::str::contains("--replace"))
            .stdout(predicates::str::contains(
                "Stop any running GUI server and start a new one",
            ));
    }

    #[test]
    fn test_gui_replace_requires_init() {
        let temp = tempfile::tempdir().unwrap();

        // --replace should also require initialization
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bn"));
        cmd.current_dir(&temp);
        cmd.arg("gui").arg("--replace");
        cmd.assert()
            .failure()
            .stderr(predicates::str::contains("Not initialized"));
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
