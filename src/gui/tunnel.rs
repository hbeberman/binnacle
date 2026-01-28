//! Devtunnel manager for creating public URLs
//!
//! This module provides `TunnelManager` for spawning and managing Azure Dev Tunnels.
//! When started, devtunnel creates a temporary public URL that proxies traffic
//! to the local GUI server.
//!
//! Requires: `devtunnel user login` (one-time authentication before first use)

use regex::Regex;
use std::io::{self, BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Timeout for waiting for devtunnel to produce a public URL
const URL_TIMEOUT_SECS: u64 = 30;

/// Grace period before sending SIGKILL after SIGTERM
const SHUTDOWN_GRACE_SECS: u64 = 5;

/// Error type for tunnel operations
#[derive(Debug)]
pub enum TunnelError {
    /// devtunnel binary not found in PATH
    DevtunnelNotFound,
    /// User not authenticated (needs `devtunnel user login`)
    NotAuthenticated,
    /// Failed to spawn devtunnel process
    SpawnFailed(io::Error),
    /// Timed out waiting for public URL
    UrlTimeout,
    /// devtunnel process exited unexpectedly
    ProcessExited(Option<i32>),
    /// Internal error
    Internal(String),
}

impl std::fmt::Display for TunnelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TunnelError::DevtunnelNotFound => {
                write!(
                    f,
                    "devtunnel not found in PATH. Install it with: \
                     just install-devtunnel"
                )
            }
            TunnelError::NotAuthenticated => {
                write!(f, "devtunnel not authenticated. Run: devtunnel user login")
            }
            TunnelError::SpawnFailed(e) => write!(f, "Failed to spawn devtunnel: {}", e),
            TunnelError::UrlTimeout => write!(
                f,
                "Timed out waiting for devtunnel to provide public URL ({}s)",
                URL_TIMEOUT_SECS
            ),
            TunnelError::ProcessExited(code) => {
                write!(f, "devtunnel exited unexpectedly with code: {:?}", code)
            }
            TunnelError::Internal(msg) => write!(f, "Internal tunnel error: {}", msg),
        }
    }
}

impl std::error::Error for TunnelError {}

/// Manages a devtunnel process
///
/// The tunnel provides a public URL that proxies to a local port.
/// Uses Azure Dev Tunnels with anonymous access enabled.
///
/// Requires one-time authentication: `devtunnel user login`
#[derive(Debug)]
pub struct TunnelManager {
    /// The child process handle (if running)
    child: Option<Child>,
    /// The public URL once available
    public_url: Arc<Mutex<Option<String>>>,
    /// The local port being tunneled
    port: u16,
}

impl TunnelManager {
    /// Check if devtunnel is available in PATH
    ///
    /// # Returns
    /// `true` if devtunnel is found and executable, `false` otherwise
    pub fn check_devtunnel() -> bool {
        Command::new("devtunnel")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Check if devtunnel is authenticated
    ///
    /// # Returns
    /// `true` if user is logged in, `false` otherwise
    pub fn check_authenticated() -> bool {
        // `devtunnel user show` returns exit code 0 if logged in
        Command::new("devtunnel")
            .args(["user", "show"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Start a tunnel to the given local port
    ///
    /// Spawns devtunnel and waits for it to provide a public URL.
    /// The tunnel proxies traffic from the public URL to `http://localhost:<port>`.
    ///
    /// # Arguments
    /// * `port` - Local port to tunnel to
    ///
    /// # Returns
    /// A `TunnelManager` instance on success, or a `TunnelError` on failure
    ///
    /// # Errors
    /// - `DevtunnelNotFound` if devtunnel is not in PATH
    /// - `NotAuthenticated` if user hasn't run `devtunnel user login`
    /// - `SpawnFailed` if the process cannot be started
    /// - `UrlTimeout` if no URL is provided within the timeout
    /// - `ProcessExited` if devtunnel exits unexpectedly
    pub fn start(port: u16) -> Result<Self, TunnelError> {
        if !Self::check_devtunnel() {
            return Err(TunnelError::DevtunnelNotFound);
        }

        if !Self::check_authenticated() {
            return Err(TunnelError::NotAuthenticated);
        }

        let port_str = port.to_string();

        // Spawn devtunnel with stdout piped for URL extraction
        // --allow-anonymous enables access without Microsoft account
        let mut child = Command::new("devtunnel")
            .args(["host", "--port-numbers", &port_str, "--allow-anonymous"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(TunnelError::SpawnFailed)?;

        let public_url = Arc::new(Mutex::new(None));
        let url_clone = Arc::clone(&public_url);

        // Take stdout before moving child (devtunnel outputs URL to stdout)
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| TunnelError::Internal("Failed to capture stdout".to_string()))?;

        // Spawn thread to read stdout and extract URL
        // devtunnel outputs: "Connect via browser: https://<id>-<port>.<region>.devtunnels.ms"
        // Region codes include: use, usw2, eus, etc.
        let url_regex =
            Regex::new(r"https://[a-z0-9-]+\.[a-z0-9]+\.devtunnels\.ms").expect("Invalid regex");

        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                // Log stdout for debugging
                eprintln!("[devtunnel] {}", line);

                if let Some(captures) = url_regex.find(&line) {
                    let mut url = url_clone.lock().unwrap();
                    if url.is_none() {
                        *url = Some(captures.as_str().to_string());
                    }
                }
            }
        });

        // Wait for URL with timeout
        let start = Instant::now();
        let timeout = Duration::from_secs(URL_TIMEOUT_SECS);

        loop {
            // Check if URL is available
            {
                let url = public_url.lock().unwrap();
                if url.is_some() {
                    break;
                }
            }

            // Check for timeout
            if start.elapsed() > timeout {
                // Kill the process before returning error
                let _ = child.kill();
                return Err(TunnelError::UrlTimeout);
            }

            // Check if process exited
            match child.try_wait() {
                Ok(Some(status)) => {
                    return Err(TunnelError::ProcessExited(status.code()));
                }
                Ok(None) => {} // Still running
                Err(e) => {
                    return Err(TunnelError::Internal(format!(
                        "Failed to check process status: {}",
                        e
                    )));
                }
            }

            // Small sleep to avoid busy-waiting
            thread::sleep(Duration::from_millis(100));
        }

        Ok(TunnelManager {
            child: Some(child),
            public_url,
            port,
        })
    }

    /// Get the public URL if available
    ///
    /// # Returns
    /// The public tunnel URL, or `None` if not yet available
    pub fn public_url(&self) -> Option<String> {
        self.public_url.lock().unwrap().clone()
    }

    /// Get the local port being tunneled
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Check if the tunnel process is still running
    pub fn is_running(&mut self) -> bool {
        if let Some(ref mut child) = self.child {
            match child.try_wait() {
                Ok(None) => true,     // Still running
                Ok(Some(_)) => false, // Exited
                Err(_) => false,      // Error checking
            }
        } else {
            false
        }
    }

    /// Gracefully shutdown the tunnel
    ///
    /// Sends SIGTERM first, waits up to 5 seconds, then SIGKILL if needed.
    /// This is called automatically on drop.
    pub fn shutdown(&mut self) {
        if let Some(ref mut child) = self.child {
            // First try graceful termination
            #[cfg(unix)]
            {
                let pid = child.id();
                // Send SIGTERM
                unsafe {
                    libc::kill(pid as i32, libc::SIGTERM);
                }
            }

            #[cfg(not(unix))]
            {
                // On non-Unix, just kill directly
                let _ = child.kill();
                let _ = child.wait();
                return;
            }

            // Wait for graceful exit
            let start = Instant::now();
            let grace = Duration::from_secs(SHUTDOWN_GRACE_SECS);

            loop {
                match child.try_wait() {
                    Ok(Some(_)) => {
                        // Process exited cleanly
                        return;
                    }
                    Ok(None) => {
                        // Still running
                        if start.elapsed() > grace {
                            // Grace period expired, force kill
                            let _ = child.kill();
                            let _ = child.wait();
                            return;
                        }
                        thread::sleep(Duration::from_millis(100));
                    }
                    Err(_) => {
                        // Error checking, try to kill anyway
                        let _ = child.kill();
                        let _ = child.wait();
                        return;
                    }
                }
            }
        }
    }
}

impl Drop for TunnelManager {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tunnel_error_display() {
        let err = TunnelError::DevtunnelNotFound;
        let msg = format!("{}", err);
        assert!(msg.contains("devtunnel not found"));
        assert!(msg.contains("just install-devtunnel"));

        let err = TunnelError::NotAuthenticated;
        let msg = format!("{}", err);
        assert!(msg.contains("not authenticated"));
        assert!(msg.contains("devtunnel user login"));

        let err = TunnelError::UrlTimeout;
        let msg = format!("{}", err);
        assert!(msg.contains("Timed out"));
        assert!(msg.contains(&URL_TIMEOUT_SECS.to_string()));

        let err = TunnelError::ProcessExited(Some(1));
        let msg = format!("{}", err);
        assert!(msg.contains("exited unexpectedly"));
        assert!(msg.contains("1"));

        let err = TunnelError::SpawnFailed(io::Error::new(io::ErrorKind::NotFound, "test"));
        let msg = format!("{}", err);
        assert!(msg.contains("Failed to spawn"));

        let err = TunnelError::Internal("test error".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("test error"));
    }

    #[test]
    fn test_check_devtunnel_returns_bool() {
        // Just verify it returns a bool without panicking
        // The actual result depends on whether devtunnel is installed
        let _result: bool = TunnelManager::check_devtunnel();
    }

    #[test]
    fn test_tunnel_error_is_error() {
        // Verify TunnelError implements std::error::Error
        fn assert_error<T: std::error::Error>() {}
        assert_error::<TunnelError>();
    }

    #[test]
    fn test_url_regex_pattern() {
        let regex = Regex::new(r"https://[a-z0-9-]+\.[a-z0-9]+\.devtunnels\.ms").unwrap();

        // Should match valid devtunnels URLs (various region codes)
        assert!(regex.is_match("https://7gs626gx-9999.use.devtunnels.ms"));
        assert!(regex.is_match("https://1kzpxxln-3030.usw2.devtunnels.ms"));
        assert!(regex.is_match("https://abc-123.eus.devtunnels.ms"));
        assert!(regex.is_match("https://a.use.devtunnels.ms"));

        // Should not match invalid URLs
        assert!(!regex.is_match("http://random.use.devtunnels.ms")); // http not https
        assert!(!regex.is_match("https://random.example.com")); // wrong domain
        assert!(!regex.is_match("random.use.devtunnels.ms")); // missing https
        assert!(!regex.is_match("https://random.trycloudflare.com")); // old cloudflare domain
    }

    #[test]
    fn test_tunnel_manager_port() {
        // We can't easily test start() without devtunnel installed,
        // but we can at least verify the struct is properly defined
        let url = Arc::new(Mutex::new(Some(
            "https://test-3030.use.devtunnels.ms".to_string(),
        )));

        // Manually construct for testing (not public API)
        let manager = TunnelManager {
            child: None,
            public_url: url,
            port: 3030,
        };

        assert_eq!(manager.port(), 3030);
        assert_eq!(
            manager.public_url(),
            Some("https://test-3030.use.devtunnels.ms".to_string())
        );
    }

    #[test]
    fn test_is_running_with_no_child() {
        let url = Arc::new(Mutex::new(None));
        let mut manager = TunnelManager {
            child: None,
            public_url: url,
            port: 3030,
        };

        assert!(!manager.is_running());
    }

    #[test]
    fn test_shutdown_with_no_child() {
        let url = Arc::new(Mutex::new(None));
        let mut manager = TunnelManager {
            child: None,
            public_url: url,
            port: 3030,
        };

        // Should not panic
        manager.shutdown();
    }
}
