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
