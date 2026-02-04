//! KDL schema definitions for tmux layouts.
//!
//! # Layout File Structure
//!
//! ```kdl
//! layout name="my-session" {
//!   window name="editor" {
//!     pane split="horizontal" size="70%" dir="/path/to/project" {
//!       cmd "git status"
//!     }
//!     pane size="30%" {
//!       cmd "bn"
//!     }
//!   }
//! }
//! ```
//!
//! # The `cmd` Attribute
//!
//! The `cmd` (or `command`) attribute specifies a command to run when the pane
//! is created. This attribute is intended for **short commands that produce
//! output and exit**, such as:
//!
//! - `git status` - Display repository status
//! - `bn` - Show binnacle status
//! - `ls -la` - List directory contents
//! - `cat file.txt` - Display file contents
//!
//! When a command finishes, the pane stays open with an interactive shell
//! (the command is wrapped with `; exec ${SHELL:-/bin/sh}`).
//!
//! ## Long-Running Interactive Tools
//!
//! For long-running interactive tools like `htop`, `watch`, `vim`, or `nvim`,
//! **launch them directly in the terminal** rather than using the `cmd` attribute.
//! These tools are designed to run interactively and don't benefit from the
//! shell-persistence wrapping.
//!
//! **Why?** The `cmd` attribute wraps commands with `exec $SHELL` to keep panes
//! alive after the command exits. For tools that don't exit (or that you interact
//! with directly), this wrapping is unnecessary and may cause unexpected behavior
//! when the tool eventually terminates.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Represents a complete tmux layout.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Layout {
    pub name: String,
    pub windows: Vec<Window>,
}

/// Represents a tmux window within a layout.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Window {
    pub name: String,
    pub panes: Vec<Pane>,
}

/// Represents a tmux pane within a window.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Pane {
    pub split: Option<Split>,
    pub size: Option<Size>,
    pub dir: Option<PathBuf>,
    pub command: Option<String>,
}

/// Split orientation for panes.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Split {
    Horizontal,
    Vertical,
}

impl Split {
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "horizontal" | "h" => Ok(Split::Horizontal),
            "vertical" | "v" => Ok(Split::Vertical),
            _ => Err(Error::Other(format!(
                "Invalid split value '{}'. Must be 'horizontal' or 'vertical'",
                s
            ))),
        }
    }
}

/// Size specification for panes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Size {
    Percentage(u8), // 0-100
    Lines(u16),
}

impl Size {
    pub fn parse(s: &str) -> Result<Self> {
        if let Some(percent_str) = s.strip_suffix('%') {
            let value = percent_str
                .parse::<u8>()
                .map_err(|_| Error::Other(format!("Invalid percentage value: {}", s)))?;
            if value > 100 {
                return Err(Error::Other(format!(
                    "Percentage must be 0-100, got {}",
                    value
                )));
            }
            Ok(Size::Percentage(value))
        } else {
            let value = s
                .parse::<u16>()
                .map_err(|_| Error::Other(format!("Invalid size value: {}", s)))?;
            Ok(Size::Lines(value))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_from_str() {
        assert_eq!(Split::parse("horizontal").unwrap(), Split::Horizontal);
        assert_eq!(Split::parse("Horizontal").unwrap(), Split::Horizontal);
        assert_eq!(Split::parse("h").unwrap(), Split::Horizontal);
        assert_eq!(Split::parse("vertical").unwrap(), Split::Vertical);
        assert_eq!(Split::parse("Vertical").unwrap(), Split::Vertical);
        assert_eq!(Split::parse("v").unwrap(), Split::Vertical);
        assert!(Split::parse("invalid").is_err());
    }

    #[test]
    fn test_size_from_str_percentage() {
        match Size::parse("50%").unwrap() {
            Size::Percentage(val) => assert_eq!(val, 50),
            _ => panic!("Expected Percentage"),
        }
        match Size::parse("100%").unwrap() {
            Size::Percentage(val) => assert_eq!(val, 100),
            _ => panic!("Expected Percentage"),
        }
        assert!(Size::parse("101%").is_err());
    }

    #[test]
    fn test_size_from_str_lines() {
        match Size::parse("20").unwrap() {
            Size::Lines(val) => assert_eq!(val, 20),
            _ => panic!("Expected Lines"),
        }
        match Size::parse("1000").unwrap() {
            Size::Lines(val) => assert_eq!(val, 1000),
            _ => panic!("Expected Lines"),
        }
    }

    #[test]
    fn test_size_from_str_invalid() {
        assert!(Size::parse("abc").is_err());
        assert!(Size::parse("50.5%").is_err());
        assert!(Size::parse("-10").is_err());
    }
}
