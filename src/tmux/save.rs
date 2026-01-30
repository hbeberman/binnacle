//! Tmux session capture and save functionality.

use crate::tmux::schema::{Layout, Pane, Split, Window};
use crate::{Error, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Capture the current tmux session state.
pub fn capture_session() -> Result<Layout> {
    // Check if we're in a tmux session
    if !is_in_tmux_session() {
        return Err(Error::Other(
            "Not in a tmux session. Please run this command from inside tmux.".to_string(),
        ));
    }

    let session_name = get_current_session_name()?;
    let windows = capture_windows(&session_name)?;

    Ok(Layout {
        name: session_name,
        windows,
    })
}

/// Check if currently inside a tmux session.
fn is_in_tmux_session() -> bool {
    std::env::var("TMUX").is_ok()
}

/// Get the current tmux session name.
fn get_current_session_name() -> Result<String> {
    let output = Command::new("tmux")
        .args(["display-message", "-p", "#S"])
        .output()
        .map_err(|e| Error::Other(format!("Failed to get session name: {}", e)))?;

    if !output.status.success() {
        return Err(Error::Other("Failed to get tmux session name".to_string()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Capture all windows in the session.
fn capture_windows(session_name: &str) -> Result<Vec<Window>> {
    let window_list = list_windows(session_name)?;
    let mut windows = Vec::new();

    for window_id in window_list {
        let window_name = get_window_name(session_name, &window_id)?;
        let panes = capture_panes(session_name, &window_id)?;
        windows.push(Window {
            name: window_name,
            panes,
        });
    }

    Ok(windows)
}

/// List window IDs in the session.
fn list_windows(session_name: &str) -> Result<Vec<String>> {
    let output = Command::new("tmux")
        .args(["list-windows", "-t", session_name, "-F", "#{window_index}"])
        .output()
        .map_err(|e| Error::Other(format!("Failed to list windows: {}", e)))?;

    if !output.status.success() {
        return Err(Error::Other("Failed to list tmux windows".to_string()));
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

/// Get window name.
fn get_window_name(session_name: &str, window_id: &str) -> Result<String> {
    let target = format!("{}:{}", session_name, window_id);
    let output = Command::new("tmux")
        .args(["display-message", "-t", &target, "-p", "#{window_name}"])
        .output()
        .map_err(|e| Error::Other(format!("Failed to get window name: {}", e)))?;

    if !output.status.success() {
        return Err(Error::Other("Failed to get tmux window name".to_string()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Capture all panes in a window.
fn capture_panes(session_name: &str, window_id: &str) -> Result<Vec<Pane>> {
    let target = format!("{}:{}", session_name, window_id);
    let pane_list = list_panes(&target)?;
    let mut panes = Vec::new();

    for (index, pane_id) in pane_list.iter().enumerate() {
        let dir = get_pane_current_path(pane_id)?;
        let command = get_pane_command(pane_id)?;

        // First pane has no split, subsequent panes need split info
        let split = if index > 0 {
            Some(determine_split_orientation(pane_id)?)
        } else {
            None
        };

        panes.push(Pane {
            split,
            size: None, // Size detection is complex, can be added later
            dir: Some(dir),
            command: if command.is_empty() || is_shell_command(&command) {
                None
            } else {
                Some(command)
            },
        });
    }

    Ok(panes)
}

/// List pane IDs in a window.
fn list_panes(target: &str) -> Result<Vec<String>> {
    let output = Command::new("tmux")
        .args(["list-panes", "-t", target, "-F", "#{pane_id}"])
        .output()
        .map_err(|e| Error::Other(format!("Failed to list panes: {}", e)))?;

    if !output.status.success() {
        return Err(Error::Other("Failed to list tmux panes".to_string()));
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

/// Get pane's current working directory.
fn get_pane_current_path(pane_id: &str) -> Result<PathBuf> {
    let output = Command::new("tmux")
        .args([
            "display-message",
            "-t",
            pane_id,
            "-p",
            "#{pane_current_path}",
        ])
        .output()
        .map_err(|e| Error::Other(format!("Failed to get pane path: {}", e)))?;

    if !output.status.success() {
        return Err(Error::Other("Failed to get pane path".to_string()));
    }

    let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(PathBuf::from(path_str))
}

/// Get command running in pane.
fn get_pane_command(pane_id: &str) -> Result<String> {
    let output = Command::new("tmux")
        .args([
            "display-message",
            "-t",
            pane_id,
            "-p",
            "#{pane_current_command}",
        ])
        .output()
        .map_err(|e| Error::Other(format!("Failed to get pane command: {}", e)))?;

    if !output.status.success() {
        return Err(Error::Other("Failed to get pane command".to_string()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Determine split orientation for a pane.
fn determine_split_orientation(pane_id: &str) -> Result<Split> {
    // Get pane width and height
    let width = get_pane_dimension(pane_id, "#{pane_width}")?;
    let height = get_pane_dimension(pane_id, "#{pane_height}")?;

    // Simple heuristic: if wider than tall, assume horizontal split
    // This is a simplification - actual split history isn't available
    if width >= height {
        Ok(Split::Horizontal)
    } else {
        Ok(Split::Vertical)
    }
}

/// Get pane dimension (width or height).
fn get_pane_dimension(pane_id: &str, format: &str) -> Result<u32> {
    let output = Command::new("tmux")
        .args(["display-message", "-t", pane_id, "-p", format])
        .output()
        .map_err(|e| Error::Other(format!("Failed to get pane dimension: {}", e)))?;

    if !output.status.success() {
        return Err(Error::Other("Failed to get pane dimension".to_string()));
    }

    let dim_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    dim_str
        .parse()
        .map_err(|_| Error::Other("Invalid pane dimension".to_string()))
}

/// Check if command is a shell.
fn is_shell_command(cmd: &str) -> bool {
    matches!(cmd, "bash" | "zsh" | "fish" | "sh" | "ksh" | "tcsh" | "csh")
}

/// Save layout to a KDL file.
pub fn save_layout_to_file(layout: &Layout, path: &Path) -> Result<()> {
    let kdl_content = layout_to_kdl(layout);

    // Create parent directory if it doesn't exist
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| Error::Other(format!("Failed to create directory: {}", e)))?;
    }

    std::fs::write(path, kdl_content)
        .map_err(|e| Error::Other(format!("Failed to write layout file: {}", e)))?;

    Ok(())
}

/// Convert layout to KDL format.
fn layout_to_kdl(layout: &Layout) -> String {
    let mut kdl = String::new();

    kdl.push_str(&format!("layout \"{}\" {{\n", layout.name));

    for window in &layout.windows {
        kdl.push_str(&format!("    window \"{}\" {{\n", window.name));

        for pane in &window.panes {
            kdl.push_str("        pane");

            // Add split attribute
            if let Some(split) = &pane.split {
                let split_str = match split {
                    Split::Horizontal => "horizontal",
                    Split::Vertical => "vertical",
                };
                kdl.push_str(&format!(" split=\"{}\"", split_str));
            }

            // Add size attribute
            if let Some(size) = &pane.size {
                let size_str = match size {
                    crate::tmux::schema::Size::Percentage(p) => format!("{}%", p),
                    crate::tmux::schema::Size::Lines(l) => l.to_string(),
                };
                kdl.push_str(&format!(" size=\"{}\"", size_str));
            }

            // Add dir attribute
            if let Some(dir) = &pane.dir {
                kdl.push_str(&format!(" dir=\"{}\"", dir.display()));
            }

            // Add command as child node
            if let Some(cmd) = &pane.command {
                kdl.push_str(" {\n");
                kdl.push_str(&format!("            cmd \"{}\"\n", cmd));
                kdl.push_str("        }\n");
            } else {
                kdl.push('\n');
            }
        }

        kdl.push_str("    }\n");
    }

    kdl.push_str("}\n");

    kdl
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_shell_command() {
        assert!(is_shell_command("bash"));
        assert!(is_shell_command("zsh"));
        assert!(is_shell_command("fish"));
        assert!(is_shell_command("sh"));
        assert!(!is_shell_command("nvim"));
        assert!(!is_shell_command("cargo"));
    }

    #[test]
    fn test_layout_to_kdl_simple() {
        let layout = Layout {
            name: "test-session".to_string(),
            windows: vec![Window {
                name: "editor".to_string(),
                panes: vec![Pane {
                    split: None,
                    size: None,
                    dir: Some(PathBuf::from("/workspace")),
                    command: Some("nvim".to_string()),
                }],
            }],
        };

        let kdl = layout_to_kdl(&layout);
        assert!(kdl.contains("layout \"test-session\""));
        assert!(kdl.contains("window \"editor\""));
        assert!(kdl.contains("dir=\"/workspace\""));
        assert!(kdl.contains("cmd \"nvim\""));
    }

    #[test]
    fn test_layout_to_kdl_with_splits() {
        let layout = Layout {
            name: "dev".to_string(),
            windows: vec![Window {
                name: "code".to_string(),
                panes: vec![
                    Pane {
                        split: None,
                        size: None,
                        dir: Some(PathBuf::from(".")),
                        command: None,
                    },
                    Pane {
                        split: Some(Split::Horizontal),
                        size: None,
                        dir: Some(PathBuf::from(".")),
                        command: Some("cargo watch".to_string()),
                    },
                ],
            }],
        };

        let kdl = layout_to_kdl(&layout);
        assert!(kdl.contains("split=\"horizontal\""));
        assert!(kdl.contains("cmd \"cargo watch\""));
    }
}
