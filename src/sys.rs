//! System utilities for privilege management and OS-level operations

use std::path::PathBuf;

/// Context information when running under sudo
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SudoContext {
    /// The UID of the user who invoked sudo
    pub uid: u32,
    /// The GID of the user who invoked sudo
    pub gid: u32,
    /// The home directory of the user who invoked sudo
    pub home: PathBuf,
}

/// Detects if the current process is running as root via sudo.
///
/// Returns `Some(SudoContext)` with the original user's uid, gid, and home
/// directory if all of the following are true:
/// 1. The process is running as root (UID 0)
/// 2. SUDO_USER environment variable is set
/// 3. SUDO_UID and SUDO_GID environment variables are set and parseable
///
/// Returns `None` if:
/// - Not running as root
/// - SUDO_USER is not set (direct root login, not via sudo)
/// - SUDO_UID or SUDO_GID cannot be parsed
///
/// # Examples
///
/// ```no_run
/// use binnacle::sys::detect_sudo_context;
///
/// if let Some(ctx) = detect_sudo_context() {
///     println!("Running as root via sudo for user {} (UID: {}, GID: {})",
///              ctx.home.display(), ctx.uid, ctx.gid);
/// } else {
///     println!("Not running under sudo");
/// }
/// ```
#[cfg(unix)]
pub fn detect_sudo_context() -> Option<SudoContext> {
    // Check if running as root by checking effective UID
    let current_uid = nix::unistd::geteuid();
    if !current_uid.is_root() {
        return None;
    }

    // Check for SUDO_USER environment variable
    let sudo_user = std::env::var("SUDO_USER").ok()?;

    // Parse SUDO_UID and SUDO_GID
    let uid: u32 = std::env::var("SUDO_UID").ok()?.parse().ok()?;

    let gid: u32 = std::env::var("SUDO_GID").ok()?.parse().ok()?;

    // Determine home directory
    // Try HOME env var first (it should be the sudo user's home)
    let home = if let Ok(home_str) = std::env::var("HOME") {
        PathBuf::from(home_str)
    } else {
        // Fall back to looking up home from /etc/passwd
        // For simplicity, construct typical home path
        PathBuf::from(format!("/home/{}", sudo_user))
    };

    Some(SudoContext { uid, gid, home })
}

#[cfg(not(unix))]
pub fn detect_sudo_context() -> Option<SudoContext> {
    // Sudo is a Unix concept, not applicable on Windows
    None
}

/// Drops privileges to the specified UID and GID.
///
/// This function permanently drops root privileges by setting the process's
/// real and effective GID and UID to the specified values.
///
/// **Important:** GID must be dropped before UID, as once the UID is dropped,
/// the process loses the ability to change GID.
///
/// # Arguments
///
/// * `uid` - The target user ID to drop to
/// * `gid` - The target group ID to drop to
///
/// # Errors
///
/// Returns an error if:
/// - The `setgid` syscall fails (e.g., invalid GID, insufficient permissions)
/// - The `setuid` syscall fails (e.g., invalid UID, insufficient permissions)
///
/// # Examples
///
/// ```no_run
/// use binnacle::sys::{detect_sudo_context, drop_privileges};
///
/// if let Some(ctx) = detect_sudo_context() {
///     drop_privileges(ctx.uid, ctx.gid).expect("Failed to drop privileges");
///     println!("Dropped privileges to UID {} GID {}", ctx.uid, ctx.gid);
/// }
/// ```
#[cfg(unix)]
pub fn drop_privileges(uid: u32, gid: u32) -> Result<(), String> {
    use nix::unistd::{Gid, Uid, setgid, setuid};

    // Drop GID first - we can't change GID after dropping UID
    let target_gid = Gid::from_raw(gid);
    setgid(target_gid).map_err(|e| format!("Failed to set GID to {}: {}", gid, e))?;

    // Now drop UID
    let target_uid = Uid::from_raw(uid);
    setuid(target_uid).map_err(|e| format!("Failed to set UID to {}: {}", uid, e))?;

    eprintln!("Dropped privileges to UID {} GID {}", uid, gid);

    Ok(())
}

#[cfg(not(unix))]
pub fn drop_privileges(_uid: u32, _gid: u32) -> Result<(), String> {
    // Privilege dropping is a Unix concept, not applicable on Windows
    Err("Privilege dropping is not supported on Windows".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn test_detect_sudo_context_not_root() {
        // When not running as root, should return None
        // This test will pass when run as a normal user
        let current_uid = nix::unistd::geteuid();
        if !current_uid.is_root() {
            assert_eq!(detect_sudo_context(), None);
        }
    }

    #[test]
    #[cfg(unix)]
    fn test_detect_sudo_context_no_sudo_env() {
        // Even if running as root, without SUDO_USER it should return None
        // This is hard to test without actually being root, so we just document the behavior
        let current_uid = nix::unistd::geteuid();
        if current_uid.is_root() {
            // If we're root but SUDO_USER isn't set, should return None
            if std::env::var("SUDO_USER").is_err() {
                assert_eq!(detect_sudo_context(), None);
            }
        }
    }

    #[test]
    #[cfg(not(unix))]
    fn test_detect_sudo_context_windows() {
        // On Windows, always returns None
        assert_eq!(detect_sudo_context(), None);
    }

    #[test]
    #[cfg(unix)]
    fn test_drop_privileges_when_not_root() {
        // When not running as root, drop_privileges should fail
        // (unless we're already running as the target UID/GID)
        let current_uid = nix::unistd::geteuid();
        if !current_uid.is_root() {
            // Try to drop to a different UID (we should fail)
            let result = super::drop_privileges(65534, 65534); // nobody user typically
            // If we're not already UID 65534, this should fail
            if current_uid.as_raw() != 65534 {
                assert!(result.is_err());
            }
        }
    }

    #[test]
    #[cfg(unix)]
    fn test_drop_privileges_to_same_user() {
        // Dropping to the current UID/GID should succeed
        let current_uid = nix::unistd::geteuid();
        let current_gid = nix::unistd::getegid();

        let result = super::drop_privileges(current_uid.as_raw(), current_gid.as_raw());
        assert!(result.is_ok());
    }

    #[test]
    #[cfg(not(unix))]
    fn test_drop_privileges_windows() {
        // On Windows, drop_privileges should always return an error
        let result = super::drop_privileges(1000, 1000);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Windows"));
    }
}
