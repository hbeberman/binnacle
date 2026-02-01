//! KDL parser for tmux layouts.

use super::schema::{Layout, Pane, Size, Split, Window};
use crate::{Error, Result};
use kdl::{KdlDocument, KdlNode};
use std::path::PathBuf;

/// Parse a KDL document into a Layout.
pub fn parse_layout(kdl_str: &str) -> Result<Layout> {
    let doc: KdlDocument = kdl_str
        .parse()
        .map_err(|e| Error::Other(format!("Failed to parse KDL: {}", e)))?;

    // Find the layout node
    let layout_node = doc
        .nodes()
        .iter()
        .find(|n| n.name().value() == "layout")
        .ok_or_else(|| Error::Other("Missing 'layout' node in KDL document".to_string()))?;

    parse_layout_node(layout_node)
}

fn parse_layout_node(node: &KdlNode) -> Result<Layout> {
    let name = get_name_attr(node)?;

    let mut windows = Vec::new();
    if let Some(children) = node.children() {
        for child in children.nodes() {
            if child.name().value() == "window" {
                windows.push(parse_window_node(child)?);
            }
        }
    }

    if windows.is_empty() {
        return Err(Error::Other(
            "Layout must contain at least one window".to_string(),
        ));
    }

    Ok(Layout { name, windows })
}

fn parse_window_node(node: &KdlNode) -> Result<Window> {
    let name = get_name_attr(node)?;

    let mut panes = Vec::new();
    if let Some(children) = node.children() {
        for child in children.nodes() {
            if child.name().value() == "pane" {
                panes.push(parse_pane_node(child)?);
            }
        }
    }

    if panes.is_empty() {
        return Err(Error::Other(
            "Window must contain at least one pane".to_string(),
        ));
    }

    Ok(Window { name, panes })
}

fn parse_pane_node(node: &KdlNode) -> Result<Pane> {
    let split = get_optional_string_attr(node, "split")?
        .map(|s| Split::parse(&s))
        .transpose()?;

    let size = get_optional_string_attr(node, "size")?
        .map(|s| Size::parse(&s))
        .transpose()?;

    let dir = get_optional_string_attr(node, "dir")?.map(PathBuf::from);

    // Validate directory path if specified
    if let Some(ref path) = dir
        && !path.is_absolute()
        && !path.starts_with("~")
        && !path.starts_with(".")
    {
        return Err(Error::Other(format!(
            "Directory path must be absolute or start with ~ or .: {}",
            path.display()
        )));
    }

    let mut command = None;
    if let Some(children) = node.children() {
        for child in children.nodes() {
            if child.name().value() == "command"
                && let Some(arg) = child.entries().first()
                && let Some(val) = arg.value().as_string()
            {
                command = Some(val.to_string());
            }
        }
    }

    Ok(Pane {
        split,
        size,
        dir,
        command,
    })
}

fn get_string_attr(node: &KdlNode, attr_name: &str) -> Result<String> {
    node.entries()
        .iter()
        .find(|e| e.name().map(|n| n.value()) == Some(attr_name))
        .and_then(|e| e.value().as_string())
        .map(|s| s.to_string())
        .ok_or_else(|| Error::Other(format!("Missing required attribute '{}'", attr_name)))
}

/// Get name attribute from a node, trying positional argument first, then named attribute.
/// This allows both `layout "name"` and `layout name="name"` syntax.
fn get_name_attr(node: &KdlNode) -> Result<String> {
    // First, try to find a positional argument (entry without a name)
    for entry in node.entries().iter() {
        if entry.name().is_none() {
            if let Some(s) = entry.value().as_string() {
                return Ok(s.to_string());
            }
        }
    }

    // Fallback to named attribute
    get_string_attr(node, "name")
}

fn get_optional_string_attr(node: &KdlNode, attr_name: &str) -> Result<Option<String>> {
    Ok(node
        .entries()
        .iter()
        .find(|e| e.name().map(|n| n.value()) == Some(attr_name))
        .and_then(|e| e.value().as_string())
        .map(|s| s.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_layout() {
        let kdl = r#"
layout name="test" {
    window name="main" {
        pane dir="/tmp" {
            command "echo hello"
        }
    }
}
"#;
        let layout = parse_layout(kdl).unwrap();
        assert_eq!(layout.name, "test");
        assert_eq!(layout.windows.len(), 1);
        assert_eq!(layout.windows[0].name, "main");
        assert_eq!(layout.windows[0].panes.len(), 1);
        assert_eq!(layout.windows[0].panes[0].dir, Some(PathBuf::from("/tmp")));
        assert_eq!(
            layout.windows[0].panes[0].command,
            Some("echo hello".to_string())
        );
    }

    #[test]
    fn test_parse_layout_with_splits() {
        let kdl = r#"
layout name="dev" {
    window name="editor" {
        pane split="horizontal" size="70%" dir="/workspace" {
            command "nvim ."
        }
        pane size="30%" {
            command "git status"
        }
    }
}
"#;
        let layout = parse_layout(kdl).unwrap();
        assert_eq!(layout.windows[0].panes.len(), 2);
        assert_eq!(layout.windows[0].panes[0].split, Some(Split::Horizontal));
        assert_eq!(layout.windows[0].panes[0].size, Some(Size::Percentage(70)));
        assert_eq!(layout.windows[0].panes[1].split, None);
        assert_eq!(layout.windows[0].panes[1].size, Some(Size::Percentage(30)));
    }

    #[test]
    fn test_parse_multiple_windows() {
        let kdl = r#"
layout name="multi" {
    window name="one" {
        pane {}
    }
    window name="two" {
        pane {}
    }
}
"#;
        let layout = parse_layout(kdl).unwrap();
        assert_eq!(layout.windows.len(), 2);
        assert_eq!(layout.windows[0].name, "one");
        assert_eq!(layout.windows[1].name, "two");
    }

    #[test]
    fn test_parse_invalid_split() {
        let kdl = r#"
layout name="test" {
    window name="main" {
        pane split="diagonal" {}
    }
}
"#;
        assert!(parse_layout(kdl).is_err());
    }

    #[test]
    fn test_parse_invalid_size() {
        let kdl = r#"
layout name="test" {
    window name="main" {
        pane size="150%" {}
    }
}
"#;
        assert!(parse_layout(kdl).is_err());
    }

    #[test]
    fn test_parse_missing_layout_name() {
        let kdl = r#"
layout {
    window name="main" {
        pane {}
    }
}
"#;
        assert!(parse_layout(kdl).is_err());
    }

    #[test]
    fn test_parse_missing_window_name() {
        let kdl = r#"
layout name="test" {
    window {
        pane {}
    }
}
"#;
        assert!(parse_layout(kdl).is_err());
    }

    #[test]
    fn test_parse_no_windows() {
        let kdl = r#"
layout name="test" {}
"#;
        let result = parse_layout(kdl);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("at least one window")
        );
    }

    #[test]
    fn test_parse_no_panes() {
        let kdl = r#"
layout name="test" {
    window name="main" {}
}
"#;
        let result = parse_layout(kdl);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("at least one pane")
        );
    }

    #[test]
    fn test_parse_relative_dir_path() {
        let kdl = r#"
layout name="test" {
    window name="main" {
        pane dir="./relative" {}
    }
}
"#;
        let layout = parse_layout(kdl).unwrap();
        assert_eq!(
            layout.windows[0].panes[0].dir,
            Some(PathBuf::from("./relative"))
        );
    }

    #[test]
    fn test_parse_tilde_dir_path() {
        let kdl = r#"
layout name="test" {
    window name="main" {
        pane dir="~/home" {}
    }
}
"#;
        let layout = parse_layout(kdl).unwrap();
        assert_eq!(
            layout.windows[0].panes[0].dir,
            Some(PathBuf::from("~/home"))
        );
    }

    #[test]
    fn test_parse_invalid_dir_path() {
        let kdl = r#"
layout name="test" {
    window name="main" {
        pane dir="relative/without/prefix" {}
    }
}
"#;
        assert!(parse_layout(kdl).is_err());
    }

    #[test]
    fn test_parse_vertical_split() {
        let kdl = r#"
layout name="test" {
    window name="main" {
        pane split="vertical" {}
    }
}
"#;
        let layout = parse_layout(kdl).unwrap();
        assert_eq!(layout.windows[0].panes[0].split, Some(Split::Vertical));
    }

    #[test]
    fn test_parse_size_in_lines() {
        let kdl = r#"
layout name="test" {
    window name="main" {
        pane size="20" {}
    }
}
"#;
        let layout = parse_layout(kdl).unwrap();
        assert_eq!(layout.windows[0].panes[0].size, Some(Size::Lines(20)));
    }

    #[test]
    fn test_parse_positional_name_syntax() {
        // This is the syntax produced by `bn session tmux save`
        let kdl = r#"
layout "test-session" {
    window "main" {
        pane dir="/workspace" {
            cmd "nvim"
        }
    }
}
"#;
        let layout = parse_layout(kdl).unwrap();
        assert_eq!(layout.name, "test-session");
        assert_eq!(layout.windows.len(), 1);
        assert_eq!(layout.windows[0].name, "main");
    }

    #[test]
    fn test_parse_mixed_syntax() {
        // Mix of positional and named attributes
        let kdl = r#"
layout "my-layout" {
    window name="editor" {
        pane split="horizontal" dir="/workspace" {}
    }
}
"#;
        let layout = parse_layout(kdl).unwrap();
        assert_eq!(layout.name, "my-layout");
        assert_eq!(layout.windows[0].name, "editor");
        assert_eq!(layout.windows[0].panes[0].split, Some(Split::Horizontal));
    }
}
