//! Tmux layout management module.
//!
//! This module provides tmux session save/load functionality using KDL configuration.
//! It is feature-gated behind the `tmux` feature flag.

pub mod parser;
pub mod schema;

use crate::{Error, Result};
use std::process::Command;

/// Check if tmux binary is available in PATH.
pub fn check_tmux_binary() -> Result<()> {
    which_tmux()?;
    Ok(())
}

/// Find tmux binary in PATH.
fn which_tmux() -> Result<String> {
    let output = Command::new("which")
        .arg("tmux")
        .output()
        .map_err(|e| Error::Other(format!("Failed to run 'which tmux': {}", e)))?;

    if !output.status.success() {
        return Err(Error::Other(
            "tmux binary not found in PATH. Please install tmux to use this feature.".to_string(),
        ));
    }

    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        return Err(Error::Other(
            "tmux binary not found in PATH. Please install tmux to use this feature.".to_string(),
        ));
    }

    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_tmux_binary() {
        // This test will pass if tmux is installed, skip if not
        match check_tmux_binary() {
            Ok(_) => {
                // tmux is installed, verify we can find it
                assert!(which_tmux().is_ok());
            }
            Err(e) => {
                // tmux not installed, verify error message is helpful
                let msg = e.to_string();
                assert!(
                    msg.contains("tmux") && msg.contains("not found"),
                    "Error message should mention tmux and not found: {}",
                    msg
                );
            }
        }
    }
}
