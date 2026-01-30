//! Layout model bridging parsed KDL to tmux command generation.
//!
//! This module provides:
//! - Directory resolution (., .., ~, absolute paths)
//! - Multi-source layout discovery (project → session → user)
//! - Conversion from Layout to TmuxCommand sequences

use super::command::TmuxCommand;
use super::parser::parse_layout;
use super::schema::{Layout, Pane, Size, Split, Window};
use crate::{Error, Result};
use std::path::{Path, PathBuf};

/// Source of a discovered layout.
#[derive(Debug, Clone, PartialEq)]
pub enum LayoutSource {
    /// Project-level: .binnacle/tmux/
    Project,
    /// Session-level: ~/.local/share/binnacle/<hash>/tmux/
    Session,
    /// User-level: ~/.config/binnacle/tmux/
    User,
}

impl std::fmt::Display for LayoutSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayoutSource::Project => write!(f, "project"),
            LayoutSource::Session => write!(f, "session"),
            LayoutSource::User => write!(f, "user"),
        }
    }
}

/// A discovered layout with its source and path.
#[derive(Debug, Clone)]
pub struct DiscoveredLayout {
    /// Name of the layout (from filename without extension).
    pub name: String,
    /// Where the layout was discovered.
    pub source: LayoutSource,
    /// Full path to the KDL file.
    pub path: PathBuf,
}

/// A resolved pane with expanded paths.
#[derive(Debug, Clone)]
pub struct ResolvedPane {
    pub split: Option<Split>,
    pub size: Option<Size>,
    /// Resolved absolute directory path.
    pub dir: Option<PathBuf>,
    pub command: Option<String>,
}

/// A resolved window with resolved panes.
#[derive(Debug, Clone)]
pub struct ResolvedWindow {
    pub name: String,
    pub panes: Vec<ResolvedPane>,
}

/// A resolved layout with all paths expanded.
#[derive(Debug, Clone)]
pub struct ResolvedLayout {
    pub name: String,
    pub windows: Vec<ResolvedWindow>,
}

impl ResolvedLayout {
    /// Create a resolved layout from a parsed layout, expanding all paths.
    ///
    /// # Arguments
    /// * `layout` - The parsed layout
    /// * `base_dir` - Base directory for resolving relative paths (typically cwd)
    pub fn from_layout(layout: Layout, base_dir: &Path) -> Result<Self> {
        let windows = layout
            .windows
            .into_iter()
            .map(|w| resolve_window(w, base_dir))
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            name: layout.name,
            windows,
        })
    }

    /// Generate tmux commands to recreate this layout.
    ///
    /// Returns a sequence of TmuxCommand that creates the session with all
    /// windows and panes.
    pub fn to_commands(&self) -> Vec<TmuxCommand> {
        let mut commands = Vec::new();

        // Create detached session with first window
        if let Some(first_window) = self.windows.first() {
            let start_dir = first_window
                .panes
                .first()
                .and_then(|p| p.dir.as_ref())
                .map(|p| p.to_string_lossy().to_string());

            commands.push(TmuxCommand::new_session(
                &self.name,
                true,
                start_dir.as_deref(),
            ));

            // Rename first window
            commands.push(rename_window(&self.name, 0, &first_window.name));

            // Create panes in first window (skip first pane, it's created with window)
            for pane in first_window.panes.iter().skip(1) {
                let target = format!("{}:0", self.name);
                commands.push(create_pane(&target, pane));
            }

            // Send commands to panes in first window
            for (pane_idx, pane) in first_window.panes.iter().enumerate() {
                if let Some(cmd) = &pane.command {
                    let target = format!("{}:0.{}", self.name, pane_idx);
                    commands.push(TmuxCommand::send_keys(Some(&target), cmd, true));
                    commands.push(TmuxCommand::send_keys(Some(&target), "Enter", false));
                }
            }
        }

        // Create additional windows
        for (window_idx, window) in self.windows.iter().enumerate().skip(1) {
            let start_dir = window
                .panes
                .first()
                .and_then(|p| p.dir.as_ref())
                .map(|p| p.to_string_lossy().to_string());

            commands.push(TmuxCommand::new_window(
                &window.name,
                Some(&self.name),
                start_dir.as_deref(),
            ));

            // Create panes (skip first, it's created with window)
            for pane in window.panes.iter().skip(1) {
                let target = format!("{}:{}", self.name, window_idx);
                commands.push(create_pane(&target, pane));
            }

            // Send commands to panes
            for (pane_idx, pane) in window.panes.iter().enumerate() {
                if let Some(cmd) = &pane.command {
                    let target = format!("{}:{}.{}", self.name, window_idx, pane_idx);
                    commands.push(TmuxCommand::send_keys(Some(&target), cmd, true));
                    commands.push(TmuxCommand::send_keys(Some(&target), "Enter", false));
                }
            }
        }

        // Select first window
        if !self.windows.is_empty() {
            commands.push(TmuxCommand::select_window(&format!("{}:0", self.name)));
        }

        commands
    }
}

/// Create a split-window command for a pane.
fn create_pane(target: &str, pane: &ResolvedPane) -> TmuxCommand {
    let split = pane.split.unwrap_or(Split::Vertical);
    let dir = pane.dir.as_ref().map(|p| p.to_string_lossy().to_string());

    TmuxCommand::split_window(Some(target), split, pane.size.clone(), dir.as_deref())
}

/// Create a rename-window command.
fn rename_window(session: &str, window_idx: usize, name: &str) -> TmuxCommand {
    // Use select-window with rename-window
    TmuxCommand::rename_window(&format!("{}:{}", session, window_idx), name)
}

fn resolve_window(window: Window, base_dir: &Path) -> Result<ResolvedWindow> {
    let panes = window
        .panes
        .into_iter()
        .map(|p| resolve_pane(p, base_dir))
        .collect::<Result<Vec<_>>>()?;

    Ok(ResolvedWindow {
        name: window.name,
        panes,
    })
}

fn resolve_pane(pane: Pane, base_dir: &Path) -> Result<ResolvedPane> {
    let dir = match pane.dir {
        Some(path) => Some(resolve_path(&path, base_dir)?),
        None => None,
    };

    Ok(ResolvedPane {
        split: pane.split,
        size: pane.size,
        dir,
        command: pane.command,
    })
}

/// Resolve a path to an absolute path.
///
/// Handles:
/// - Absolute paths: returned as-is
/// - `~` prefix: expanded to home directory
/// - `.` or `..` prefixes: resolved relative to base_dir
pub fn resolve_path(path: &Path, base_dir: &Path) -> Result<PathBuf> {
    let path_str = path.to_string_lossy();

    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else if path_str.starts_with("~/") || path_str == "~" {
        expand_tilde(path)
    } else if path_str.starts_with("./") || path_str.starts_with("../") || path_str == "." {
        // Resolve relative to base_dir
        let resolved = base_dir.join(path);
        canonicalize_path(&resolved)
    } else {
        // Bare relative path - treat as relative to base_dir
        let resolved = base_dir.join(path);
        canonicalize_path(&resolved)
    }
}

/// Expand tilde in path to home directory.
fn expand_tilde(path: &Path) -> Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| Error::Other("Could not determine home directory".to_string()))?;

    let path_str = path.to_string_lossy();
    if path_str == "~" {
        Ok(home)
    } else if let Some(rest) = path_str.strip_prefix("~/") {
        Ok(home.join(rest))
    } else {
        Ok(path.to_path_buf())
    }
}

/// Canonicalize a path, normalizing . and .. components.
/// Unlike std::fs::canonicalize, this doesn't require the path to exist.
fn canonicalize_path(path: &Path) -> Result<PathBuf> {
    let mut result = PathBuf::new();

    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                if !result.pop() {
                    // Can't go above root
                    return Err(Error::Other(format!(
                        "Cannot resolve path: too many '..' components in {}",
                        path.display()
                    )));
                }
            }
            std::path::Component::CurDir => {
                // Skip current dir components
            }
            _ => {
                result.push(component);
            }
        }
    }

    Ok(result)
}

/// Get the tmux directory for a given source.
pub fn get_tmux_dir(source: LayoutSource, repo_path: &Path) -> Result<PathBuf> {
    match source {
        LayoutSource::Project => Ok(repo_path.join(".binnacle").join("tmux")),
        LayoutSource::Session => {
            let data_dir = dirs::data_local_dir()
                .ok_or_else(|| Error::Other("Failed to find data directory".to_string()))?;
            let repo_hash = crate::storage::compute_repo_hash(repo_path)?;
            Ok(data_dir.join("binnacle").join(&repo_hash).join("tmux"))
        }
        LayoutSource::User => {
            let config_dir = dirs::config_dir()
                .ok_or_else(|| Error::Other("Failed to find config directory".to_string()))?;
            Ok(config_dir.join("binnacle").join("tmux"))
        }
    }
}

/// Find a layout by name, searching in order: project → session → user.
///
/// # Arguments
/// * `name` - Layout name (without .kdl extension)
/// * `repo_path` - Path to the repository root
pub fn find_layout(name: &str, repo_path: &Path) -> Result<Option<DiscoveredLayout>> {
    let filename = format!("{}.kdl", name);

    // Search order: project → session → user
    let sources = [
        LayoutSource::Project,
        LayoutSource::Session,
        LayoutSource::User,
    ];

    for source in sources {
        let dir = get_tmux_dir(source.clone(), repo_path)?;
        let path = dir.join(&filename);

        if path.exists() {
            return Ok(Some(DiscoveredLayout {
                name: name.to_string(),
                source,
                path,
            }));
        }
    }

    Ok(None)
}

/// List all available layouts from all sources.
///
/// Returns layouts in discovery order (project first, then session, then user).
/// Layouts with the same name from earlier sources shadow later sources.
///
/// # Arguments
/// * `repo_path` - Path to the repository root
pub fn list_layouts(repo_path: &Path) -> Result<Vec<DiscoveredLayout>> {
    let mut layouts = Vec::new();
    let mut seen_names = std::collections::HashSet::new();

    let sources = [
        LayoutSource::Project,
        LayoutSource::Session,
        LayoutSource::User,
    ];

    for source in sources {
        let dir = match get_tmux_dir(source.clone(), repo_path) {
            Ok(d) => d,
            Err(_) => continue,
        };

        if !dir.exists() {
            continue;
        }

        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "kdl")
                && let Some(stem) = path.file_stem()
            {
                let name = stem.to_string_lossy().to_string();
                // Only add if not already seen (earlier sources shadow later ones)
                if !seen_names.contains(&name) {
                    seen_names.insert(name.clone());
                    layouts.push(DiscoveredLayout {
                        name,
                        source: source.clone(),
                        path,
                    });
                }
            }
        }
    }

    Ok(layouts)
}

/// Load a layout from a discovered layout.
pub fn load_layout(discovered: &DiscoveredLayout) -> Result<Layout> {
    let content = std::fs::read_to_string(&discovered.path)
        .map_err(|e| Error::Other(format!("Failed to read layout file: {}", e)))?;
    parse_layout(&content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_resolve_absolute_path() {
        let base = PathBuf::from("/home/user/project");
        let path = PathBuf::from("/etc/config");
        let resolved = resolve_path(&path, &base).unwrap();
        assert_eq!(resolved, PathBuf::from("/etc/config"));
    }

    #[test]
    fn test_resolve_tilde_path() {
        let base = PathBuf::from("/home/user/project");
        let path = PathBuf::from("~/documents");
        let resolved = resolve_path(&path, &base).unwrap();

        let home = dirs::home_dir().unwrap();
        assert_eq!(resolved, home.join("documents"));
    }

    #[test]
    fn test_resolve_tilde_only() {
        let base = PathBuf::from("/home/user/project");
        let path = PathBuf::from("~");
        let resolved = resolve_path(&path, &base).unwrap();

        let home = dirs::home_dir().unwrap();
        assert_eq!(resolved, home);
    }

    #[test]
    fn test_resolve_dot_path() {
        let base = PathBuf::from("/home/user/project");
        let path = PathBuf::from("./src");
        let resolved = resolve_path(&path, &base).unwrap();
        assert_eq!(resolved, PathBuf::from("/home/user/project/src"));
    }

    #[test]
    fn test_resolve_dotdot_path() {
        let base = PathBuf::from("/home/user/project/deep/nested");
        let path = PathBuf::from("../sibling");
        let resolved = resolve_path(&path, &base).unwrap();
        assert_eq!(resolved, PathBuf::from("/home/user/project/deep/sibling"));
    }

    #[test]
    fn test_resolve_current_dir() {
        let base = PathBuf::from("/home/user/project");
        let path = PathBuf::from(".");
        let resolved = resolve_path(&path, &base).unwrap();
        assert_eq!(resolved, PathBuf::from("/home/user/project"));
    }

    #[test]
    fn test_resolve_relative_path() {
        let base = PathBuf::from("/home/user/project");
        let path = PathBuf::from("subdir/file");
        let resolved = resolve_path(&path, &base).unwrap();
        assert_eq!(resolved, PathBuf::from("/home/user/project/subdir/file"));
    }

    #[test]
    fn test_canonicalize_removes_dots() {
        let path = PathBuf::from("/a/b/./c/../d");
        let result = canonicalize_path(&path).unwrap();
        assert_eq!(result, PathBuf::from("/a/b/d"));
    }

    #[test]
    fn test_canonicalize_too_many_dotdot() {
        let path = PathBuf::from("/a/../../b");
        let result = canonicalize_path(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_find_layout_project_level() {
        let temp = TempDir::new().unwrap();
        let repo = temp.path();

        // Create project-level layout
        let tmux_dir = repo.join(".binnacle").join("tmux");
        fs::create_dir_all(&tmux_dir).unwrap();
        fs::write(
            tmux_dir.join("dev.kdl"),
            r#"layout name="dev" { window name="main" { pane {} } }"#,
        )
        .unwrap();

        let found = find_layout("dev", repo).unwrap().unwrap();
        assert_eq!(found.name, "dev");
        assert_eq!(found.source, LayoutSource::Project);
        assert_eq!(found.path, tmux_dir.join("dev.kdl"));
    }

    #[test]
    fn test_find_layout_not_found() {
        let temp = TempDir::new().unwrap();
        let repo = temp.path();

        let found = find_layout("nonexistent", repo).unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn test_list_layouts() {
        let temp = TempDir::new().unwrap();
        let repo = temp.path();

        // Create project-level layouts
        let tmux_dir = repo.join(".binnacle").join("tmux");
        fs::create_dir_all(&tmux_dir).unwrap();
        fs::write(
            tmux_dir.join("dev.kdl"),
            r#"layout name="dev" { window name="main" { pane {} } }"#,
        )
        .unwrap();
        fs::write(
            tmux_dir.join("work.kdl"),
            r#"layout name="work" { window name="main" { pane {} } }"#,
        )
        .unwrap();

        let layouts = list_layouts(repo).unwrap();

        // Both should be found
        assert_eq!(layouts.len(), 2);
        let names: Vec<_> = layouts.iter().map(|l| l.name.as_str()).collect();
        assert!(names.contains(&"dev"));
        assert!(names.contains(&"work"));
    }

    #[test]
    fn test_list_layouts_shadowing() {
        let temp = TempDir::new().unwrap();
        let repo = temp.path();

        // Create project-level layout
        let project_tmux = repo.join(".binnacle").join("tmux");
        fs::create_dir_all(&project_tmux).unwrap();
        fs::write(
            project_tmux.join("dev.kdl"),
            r#"layout name="dev" { window name="project" { pane {} } }"#,
        )
        .unwrap();

        // The project layout should shadow any user-level layout with the same name
        let layouts = list_layouts(repo).unwrap();
        let dev_layout = layouts.iter().find(|l| l.name == "dev").unwrap();
        assert_eq!(dev_layout.source, LayoutSource::Project);
    }

    #[test]
    fn test_resolved_layout_from_layout() {
        let layout = Layout {
            name: "test".to_string(),
            windows: vec![Window {
                name: "editor".to_string(),
                panes: vec![Pane {
                    split: None,
                    size: None,
                    dir: Some(PathBuf::from("./src")),
                    command: Some("nvim".to_string()),
                }],
            }],
        };

        let base = PathBuf::from("/workspace");
        let resolved = ResolvedLayout::from_layout(layout, &base).unwrap();

        assert_eq!(resolved.name, "test");
        assert_eq!(resolved.windows.len(), 1);
        assert_eq!(
            resolved.windows[0].panes[0].dir,
            Some(PathBuf::from("/workspace/src"))
        );
    }

    #[test]
    fn test_resolved_layout_to_commands() {
        let resolved = ResolvedLayout {
            name: "test-session".to_string(),
            windows: vec![ResolvedWindow {
                name: "editor".to_string(),
                panes: vec![ResolvedPane {
                    split: None,
                    size: None,
                    dir: Some(PathBuf::from("/workspace")),
                    command: Some("nvim".to_string()),
                }],
            }],
        };

        let commands = resolved.to_commands();

        // Should have: new-session, rename-window, send-keys (command), send-keys (Enter), select-window
        assert!(!commands.is_empty());

        let cmd_strings: Vec<_> = commands.into_iter().map(|c| c.build()).collect();
        assert!(cmd_strings.iter().any(|s| s.contains("new-session")));
        assert!(cmd_strings.iter().any(|s| s.contains("test-session")));
    }

    #[test]
    fn test_resolved_layout_multiple_windows() {
        let resolved = ResolvedLayout {
            name: "multi".to_string(),
            windows: vec![
                ResolvedWindow {
                    name: "first".to_string(),
                    panes: vec![ResolvedPane {
                        split: None,
                        size: None,
                        dir: Some(PathBuf::from("/a")),
                        command: None,
                    }],
                },
                ResolvedWindow {
                    name: "second".to_string(),
                    panes: vec![ResolvedPane {
                        split: None,
                        size: None,
                        dir: Some(PathBuf::from("/b")),
                        command: None,
                    }],
                },
            ],
        };

        let commands = resolved.to_commands();
        let cmd_strings: Vec<_> = commands.into_iter().map(|c| c.build()).collect();

        // Should create second window
        assert!(cmd_strings.iter().any(|s| s.contains("new-window")));
    }

    #[test]
    fn test_resolved_layout_multiple_panes() {
        let resolved = ResolvedLayout {
            name: "splits".to_string(),
            windows: vec![ResolvedWindow {
                name: "main".to_string(),
                panes: vec![
                    ResolvedPane {
                        split: None,
                        size: None,
                        dir: Some(PathBuf::from("/workspace")),
                        command: None,
                    },
                    ResolvedPane {
                        split: Some(Split::Horizontal),
                        size: Some(Size::Percentage(30)),
                        dir: Some(PathBuf::from("/workspace")),
                        command: Some("htop".to_string()),
                    },
                ],
            }],
        };

        let commands = resolved.to_commands();
        let cmd_strings: Vec<_> = commands.into_iter().map(|c| c.build()).collect();

        // Should have split-window
        assert!(cmd_strings.iter().any(|s| s.contains("split-window")));
        assert!(cmd_strings.iter().any(|s| s.contains("-h"))); // horizontal
        assert!(cmd_strings.iter().any(|s| s.contains("-p 30"))); // percentage
    }

    #[test]
    fn test_layout_source_display() {
        assert_eq!(LayoutSource::Project.to_string(), "project");
        assert_eq!(LayoutSource::Session.to_string(), "session");
        assert_eq!(LayoutSource::User.to_string(), "user");
    }
}
