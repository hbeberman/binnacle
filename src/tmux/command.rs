//! Tmux command generation.
//!
//! This module provides a builder for generating tmux CLI command strings.
//! It does not execute commands, only generates the string representations.

use crate::tmux::schema::{Size, Split};

/// Builder for generating tmux command strings.
#[derive(Debug, Clone)]
pub struct TmuxCommand {
    args: Vec<String>,
}

impl TmuxCommand {
    /// Create a new tmux command builder.
    fn new(command: &str) -> Self {
        Self {
            args: vec!["tmux".to_string(), command.to_string()],
        }
    }

    /// Add a flag to the command.
    fn flag(mut self, flag: &str) -> Self {
        self.args.push(flag.to_string());
        self
    }

    /// Add a flag with a value to the command.
    fn flag_with_value(mut self, flag: &str, value: &str) -> Self {
        self.args.push(flag.to_string());
        self.args.push(value.to_string());
        self
    }

    /// Add an argument to the command.
    fn arg(mut self, arg: &str) -> Self {
        self.args.push(arg.to_string());
        self
    }

    /// Build the final command string.
    pub fn build(self) -> String {
        self.args.join(" ")
    }

    /// Create a new tmux session.
    ///
    /// # Arguments
    /// * `session_name` - Name of the session to create
    /// * `detached` - If true, don't attach to the session
    /// * `start_directory` - Optional starting directory
    ///
    /// # Example
    /// ```
    /// use binnacle::tmux::command::TmuxCommand;
    /// let cmd = TmuxCommand::new_session("my-session", true, Some("/workspace"));
    /// assert_eq!(cmd.build(), "tmux new-session -d -s my-session -c /workspace");
    /// ```
    pub fn new_session(session_name: &str, detached: bool, start_directory: Option<&str>) -> Self {
        let mut cmd = Self::new("new-session");
        if detached {
            cmd = cmd.flag("-d");
        }
        cmd = cmd.flag_with_value("-s", session_name);
        if let Some(dir) = start_directory {
            cmd = cmd.flag_with_value("-c", dir);
        }
        cmd
    }

    /// Create a new window in the current session.
    ///
    /// # Arguments
    /// * `window_name` - Name of the window to create
    /// * `target_session` - Optional target session (defaults to current)
    /// * `start_directory` - Optional starting directory
    ///
    /// # Example
    /// ```
    /// use binnacle::tmux::command::TmuxCommand;
    /// let cmd = TmuxCommand::new_window("editor", Some("my-session"), Some("/workspace"));
    /// assert_eq!(cmd.build(), "tmux new-window -t my-session -n editor -c /workspace");
    /// ```
    pub fn new_window(
        window_name: &str,
        target_session: Option<&str>,
        start_directory: Option<&str>,
    ) -> Self {
        let mut cmd = Self::new("new-window");
        if let Some(target) = target_session {
            cmd = cmd.flag_with_value("-t", target);
        }
        cmd = cmd.flag_with_value("-n", window_name);
        if let Some(dir) = start_directory {
            cmd = cmd.flag_with_value("-c", dir);
        }
        cmd
    }

    /// Split a window pane.
    ///
    /// # Arguments
    /// * `target` - Target pane to split
    /// * `split` - Horizontal or vertical split
    /// * `size` - Optional size specification
    /// * `start_directory` - Optional starting directory
    ///
    /// # Example
    /// ```
    /// use binnacle::tmux::command::TmuxCommand;
    /// use binnacle::tmux::schema::{Split, Size};
    /// let cmd = TmuxCommand::split_window(
    ///     Some("my-session:1.0"),
    ///     Split::Horizontal,
    ///     Some(Size::Percentage(70)),
    ///     Some("/workspace")
    /// );
    /// assert_eq!(cmd.build(), "tmux split-window -t my-session:1.0 -h -p 70 -c /workspace");
    /// ```
    pub fn split_window(
        target: Option<&str>,
        split: Split,
        size: Option<Size>,
        start_directory: Option<&str>,
    ) -> Self {
        let mut cmd = Self::new("split-window");
        if let Some(t) = target {
            cmd = cmd.flag_with_value("-t", t);
        }
        cmd = match split {
            Split::Horizontal => cmd.flag("-h"),
            Split::Vertical => cmd.flag("-v"),
        };
        if let Some(sz) = size {
            match sz {
                Size::Percentage(p) => cmd = cmd.flag_with_value("-p", &p.to_string()),
                Size::Lines(l) => cmd = cmd.flag_with_value("-l", &l.to_string()),
            }
        }
        if let Some(dir) = start_directory {
            cmd = cmd.flag_with_value("-c", dir);
        }
        cmd
    }

    /// Send keys to a pane.
    ///
    /// # Arguments
    /// * `target` - Target pane to send keys to
    /// * `keys` - Keys to send
    /// * `literal` - If true, send keys literally without interpretation
    ///
    /// # Example
    /// ```
    /// use binnacle::tmux::command::TmuxCommand;
    /// let cmd = TmuxCommand::send_keys(Some("my-session:1.0"), "echo hello", true);
    /// assert_eq!(cmd.build(), "tmux send-keys -t my-session:1.0 -l echo hello");
    /// ```
    pub fn send_keys(target: Option<&str>, keys: &str, literal: bool) -> Self {
        let mut cmd = Self::new("send-keys");
        if let Some(t) = target {
            cmd = cmd.flag_with_value("-t", t);
        }
        if literal {
            cmd = cmd.flag("-l");
        }
        cmd = cmd.arg(keys);
        cmd
    }

    /// Set an environment variable.
    ///
    /// # Arguments
    /// * `target` - Target session or global (-g)
    /// * `name` - Environment variable name
    /// * `value` - Environment variable value
    /// * `global` - If true, set globally
    ///
    /// # Example
    /// ```
    /// use binnacle::tmux::command::TmuxCommand;
    /// let cmd = TmuxCommand::set_environment(Some("my-session"), "PATH", "/usr/bin", false);
    /// assert_eq!(cmd.build(), "tmux set-environment -t my-session PATH /usr/bin");
    /// ```
    pub fn set_environment(target: Option<&str>, name: &str, value: &str, global: bool) -> Self {
        let mut cmd = Self::new("set-environment");
        if global {
            cmd = cmd.flag("-g");
        }
        if let Some(t) = target {
            cmd = cmd.flag_with_value("-t", t);
        }
        cmd = cmd.arg(name);
        cmd = cmd.arg(value);
        cmd
    }

    /// Select a window.
    ///
    /// # Arguments
    /// * `target` - Target window to select
    ///
    /// # Example
    /// ```
    /// use binnacle::tmux::command::TmuxCommand;
    /// let cmd = TmuxCommand::select_window("my-session:1");
    /// assert_eq!(cmd.build(), "tmux select-window -t my-session:1");
    /// ```
    pub fn select_window(target: &str) -> Self {
        Self::new("select-window").flag_with_value("-t", target)
    }

    /// Rename a window.
    ///
    /// # Arguments
    /// * `target` - Target window to rename
    /// * `new_name` - New name for the window
    ///
    /// # Example
    /// ```
    /// use binnacle::tmux::command::TmuxCommand;
    /// let cmd = TmuxCommand::rename_window("my-session:0", "editor");
    /// assert_eq!(cmd.build(), "tmux rename-window -t my-session:0 editor");
    /// ```
    pub fn rename_window(target: &str, new_name: &str) -> Self {
        Self::new("rename-window")
            .flag_with_value("-t", target)
            .arg(new_name)
    }

    /// Check if a session exists.
    ///
    /// # Arguments
    /// * `session_name` - Name of the session to check
    ///
    /// # Example
    /// ```
    /// use binnacle::tmux::command::TmuxCommand;
    /// let cmd = TmuxCommand::has_session("my-session");
    /// assert_eq!(cmd.build(), "tmux has-session -t my-session");
    /// ```
    pub fn has_session(session_name: &str) -> Self {
        Self::new("has-session").flag_with_value("-t", session_name)
    }

    /// Start the tmux server without creating any sessions.
    ///
    /// This command ensures the tmux server is running. It's idempotent -
    /// if the server is already running, this is a no-op.
    ///
    /// # Example
    /// ```
    /// use binnacle::tmux::command::TmuxCommand;
    /// let cmd = TmuxCommand::start_server();
    /// assert_eq!(cmd.build(), "tmux start-server");
    /// ```
    pub fn start_server() -> Self {
        Self::new("start-server")
    }

    /// Attach to an existing session.
    ///
    /// # Arguments
    /// * `session_name` - Name of the session to attach to
    ///
    /// # Example
    /// ```
    /// use binnacle::tmux::command::TmuxCommand;
    /// let cmd = TmuxCommand::attach_session("my-session");
    /// assert_eq!(cmd.build(), "tmux attach-session -t my-session");
    /// ```
    pub fn attach_session(session_name: &str) -> Self {
        Self::new("attach-session").flag_with_value("-t", session_name)
    }

    /// Get an environment variable from a session.
    ///
    /// # Arguments
    /// * `target` - Target session
    /// * `name` - Environment variable name
    ///
    /// # Example
    /// ```
    /// use binnacle::tmux::command::TmuxCommand;
    /// let cmd = TmuxCommand::show_environment(Some("my-session"), "BINNACLE_REPO_HASH");
    /// assert_eq!(cmd.build(), "tmux show-environment -t my-session BINNACLE_REPO_HASH");
    /// ```
    pub fn show_environment(target: Option<&str>, name: &str) -> Self {
        let mut cmd = Self::new("show-environment");
        if let Some(t) = target {
            cmd = cmd.flag_with_value("-t", t);
        }
        cmd = cmd.arg(name);
        cmd
    }

    /// Get the arguments as a Vec for execution.
    pub fn args(&self) -> &[String] {
        &self.args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_session_minimal() {
        let cmd = TmuxCommand::new_session("test-session", false, None);
        assert_eq!(cmd.build(), "tmux new-session -s test-session");
    }

    #[test]
    fn test_new_session_detached() {
        let cmd = TmuxCommand::new_session("test-session", true, None);
        assert_eq!(cmd.build(), "tmux new-session -d -s test-session");
    }

    #[test]
    fn test_new_session_with_directory() {
        let cmd = TmuxCommand::new_session("test-session", true, Some("/workspace"));
        assert_eq!(
            cmd.build(),
            "tmux new-session -d -s test-session -c /workspace"
        );
    }

    #[test]
    fn test_new_session_all_options() {
        let cmd = TmuxCommand::new_session("my-session", true, Some("/home/user"));
        assert_eq!(
            cmd.build(),
            "tmux new-session -d -s my-session -c /home/user"
        );
    }

    #[test]
    fn test_new_window_minimal() {
        let cmd = TmuxCommand::new_window("editor", None, None);
        assert_eq!(cmd.build(), "tmux new-window -n editor");
    }

    #[test]
    fn test_new_window_with_target() {
        let cmd = TmuxCommand::new_window("editor", Some("my-session"), None);
        assert_eq!(cmd.build(), "tmux new-window -t my-session -n editor");
    }

    #[test]
    fn test_new_window_with_directory() {
        let cmd = TmuxCommand::new_window("editor", None, Some("/workspace"));
        assert_eq!(cmd.build(), "tmux new-window -n editor -c /workspace");
    }

    #[test]
    fn test_new_window_all_options() {
        let cmd = TmuxCommand::new_window("editor", Some("my-session"), Some("/workspace"));
        assert_eq!(
            cmd.build(),
            "tmux new-window -t my-session -n editor -c /workspace"
        );
    }

    #[test]
    fn test_split_window_horizontal() {
        let cmd = TmuxCommand::split_window(None, Split::Horizontal, None, None);
        assert_eq!(cmd.build(), "tmux split-window -h");
    }

    #[test]
    fn test_split_window_vertical() {
        let cmd = TmuxCommand::split_window(None, Split::Vertical, None, None);
        assert_eq!(cmd.build(), "tmux split-window -v");
    }

    #[test]
    fn test_split_window_with_target() {
        let cmd = TmuxCommand::split_window(Some("my-session:1.0"), Split::Horizontal, None, None);
        assert_eq!(cmd.build(), "tmux split-window -t my-session:1.0 -h");
    }

    #[test]
    fn test_split_window_with_percentage_size() {
        let cmd =
            TmuxCommand::split_window(None, Split::Horizontal, Some(Size::Percentage(70)), None);
        assert_eq!(cmd.build(), "tmux split-window -h -p 70");
    }

    #[test]
    fn test_split_window_with_lines_size() {
        let cmd = TmuxCommand::split_window(None, Split::Vertical, Some(Size::Lines(20)), None);
        assert_eq!(cmd.build(), "tmux split-window -v -l 20");
    }

    #[test]
    fn test_split_window_with_directory() {
        let cmd = TmuxCommand::split_window(None, Split::Horizontal, None, Some("/workspace"));
        assert_eq!(cmd.build(), "tmux split-window -h -c /workspace");
    }

    #[test]
    fn test_split_window_all_options() {
        let cmd = TmuxCommand::split_window(
            Some("my-session:1.0"),
            Split::Horizontal,
            Some(Size::Percentage(70)),
            Some("/workspace"),
        );
        assert_eq!(
            cmd.build(),
            "tmux split-window -t my-session:1.0 -h -p 70 -c /workspace"
        );
    }

    #[test]
    fn test_send_keys_minimal() {
        let cmd = TmuxCommand::send_keys(None, "echo hello", false);
        assert_eq!(cmd.build(), "tmux send-keys echo hello");
    }

    #[test]
    fn test_send_keys_with_target() {
        let cmd = TmuxCommand::send_keys(Some("my-session:1.0"), "echo hello", false);
        assert_eq!(cmd.build(), "tmux send-keys -t my-session:1.0 echo hello");
    }

    #[test]
    fn test_send_keys_literal() {
        let cmd = TmuxCommand::send_keys(None, "echo hello", true);
        assert_eq!(cmd.build(), "tmux send-keys -l echo hello");
    }

    #[test]
    fn test_send_keys_all_options() {
        let cmd = TmuxCommand::send_keys(Some("my-session:1.0"), "echo hello", true);
        assert_eq!(
            cmd.build(),
            "tmux send-keys -t my-session:1.0 -l echo hello"
        );
    }

    #[test]
    fn test_send_keys_with_special_chars() {
        let cmd = TmuxCommand::send_keys(None, "echo 'hello world'", true);
        assert_eq!(cmd.build(), "tmux send-keys -l echo 'hello world'");
    }

    #[test]
    fn test_set_environment_minimal() {
        let cmd = TmuxCommand::set_environment(None, "PATH", "/usr/bin", false);
        assert_eq!(cmd.build(), "tmux set-environment PATH /usr/bin");
    }

    #[test]
    fn test_set_environment_with_target() {
        let cmd = TmuxCommand::set_environment(Some("my-session"), "PATH", "/usr/bin", false);
        assert_eq!(
            cmd.build(),
            "tmux set-environment -t my-session PATH /usr/bin"
        );
    }

    #[test]
    fn test_set_environment_global() {
        let cmd = TmuxCommand::set_environment(None, "PATH", "/usr/bin", true);
        assert_eq!(cmd.build(), "tmux set-environment -g PATH /usr/bin");
    }

    #[test]
    fn test_set_environment_all_options() {
        let cmd = TmuxCommand::set_environment(Some("my-session"), "PATH", "/usr/bin", true);
        assert_eq!(
            cmd.build(),
            "tmux set-environment -g -t my-session PATH /usr/bin"
        );
    }

    #[test]
    fn test_select_window() {
        let cmd = TmuxCommand::select_window("my-session:1");
        assert_eq!(cmd.build(), "tmux select-window -t my-session:1");
    }

    #[test]
    fn test_select_window_by_name() {
        let cmd = TmuxCommand::select_window("my-session:editor");
        assert_eq!(cmd.build(), "tmux select-window -t my-session:editor");
    }

    #[test]
    fn test_select_window_relative() {
        let cmd = TmuxCommand::select_window("+1");
        assert_eq!(cmd.build(), "tmux select-window -t +1");
    }

    #[test]
    fn test_builder_is_reusable() {
        // Verify that building doesn't consume the builder
        let cmd = TmuxCommand::new_session("test", false, None);
        let cmd_clone = cmd.clone();
        assert_eq!(cmd.build(), cmd_clone.build());
    }

    #[test]
    fn test_percentage_size_zero() {
        let cmd =
            TmuxCommand::split_window(None, Split::Horizontal, Some(Size::Percentage(0)), None);
        assert_eq!(cmd.build(), "tmux split-window -h -p 0");
    }

    #[test]
    fn test_percentage_size_hundred() {
        let cmd =
            TmuxCommand::split_window(None, Split::Horizontal, Some(Size::Percentage(100)), None);
        assert_eq!(cmd.build(), "tmux split-window -h -p 100");
    }

    #[test]
    fn test_lines_size_large() {
        let cmd = TmuxCommand::split_window(None, Split::Vertical, Some(Size::Lines(1000)), None);
        assert_eq!(cmd.build(), "tmux split-window -v -l 1000");
    }

    #[test]
    fn test_rename_window() {
        let cmd = TmuxCommand::rename_window("my-session:0", "editor");
        assert_eq!(cmd.build(), "tmux rename-window -t my-session:0 editor");
    }

    #[test]
    fn test_rename_window_with_spaces() {
        let cmd = TmuxCommand::rename_window("dev:1", "my window");
        assert_eq!(cmd.build(), "tmux rename-window -t dev:1 my window");
    }

    #[test]
    fn test_has_session() {
        let cmd = TmuxCommand::has_session("my-session");
        assert_eq!(cmd.build(), "tmux has-session -t my-session");
    }

    #[test]
    fn test_start_server() {
        let cmd = TmuxCommand::start_server();
        assert_eq!(cmd.build(), "tmux start-server");
    }

    #[test]
    fn test_attach_session() {
        let cmd = TmuxCommand::attach_session("my-session");
        assert_eq!(cmd.build(), "tmux attach-session -t my-session");
    }

    #[test]
    fn test_show_environment_with_target() {
        let cmd = TmuxCommand::show_environment(Some("my-session"), "BINNACLE_REPO_HASH");
        assert_eq!(
            cmd.build(),
            "tmux show-environment -t my-session BINNACLE_REPO_HASH"
        );
    }

    #[test]
    fn test_show_environment_without_target() {
        let cmd = TmuxCommand::show_environment(None, "PATH");
        assert_eq!(cmd.build(), "tmux show-environment PATH");
    }

    #[test]
    fn test_args_accessor() {
        let cmd = TmuxCommand::new_session("test", true, None);
        let args = cmd.args();
        assert_eq!(args, &["tmux", "new-session", "-d", "-s", "test"]);
    }
}
