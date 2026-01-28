//! Cloudflared tunnel manager for creating public URLs
//!
//! This module provides `TunnelManager` for spawning and managing cloudflared
//! quick tunnels. When started, cloudflared creates a temporary public URL
//! that proxies traffic to the local GUI server.

use regex::Regex;
use std::io::{self, BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Timeout for waiting for cloudflared to produce a public URL
const URL_TIMEOUT_SECS: u64 = 30;

/// Grace period before sending SIGKILL after SIGTERM
const SHUTDOWN_GRACE_SECS: u64 = 5;

/// Error type for tunnel operations
#[derive(Debug)]
pub enum TunnelError {
    /// cloudflared binary not found in PATH
    CloudflaredNotFound,
    /// Failed to spawn cloudflared process
    SpawnFailed(io::Error),
    /// Timed out waiting for public URL
    UrlTimeout,
    /// cloudflared process exited unexpectedly
    ProcessExited(Option<i32>),
    /// Internal error
    Internal(String),
}

impl std::fmt::Display for TunnelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TunnelError::CloudflaredNotFound => {
                write!(
                    f,
                    "cloudflared not found in PATH. Install it with: \
                     brew install cloudflared (macOS) or see https://developers.cloudflare.com/cloudflare-one/connections/connect-apps/install-and-setup/installation/"
                )
            }
            TunnelError::SpawnFailed(e) => write!(f, "Failed to spawn cloudflared: {}", e),
            TunnelError::UrlTimeout => write!(
                f,
                "Timed out waiting for cloudflared to provide public URL ({}s)",
                URL_TIMEOUT_SECS
            ),
            TunnelError::ProcessExited(code) => {
                write!(f, "cloudflared exited unexpectedly with code: {:?}", code)
            }
            TunnelError::Internal(msg) => write!(f, "Internal tunnel error: {}", msg),
        }
    }
}

impl std::error::Error for TunnelError {}

/// Manages a cloudflared tunnel process
///
/// The tunnel provides a public URL that proxies to a local port.
/// Uses cloudflared's "quick tunnel" feature which requires no configuration.
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
    /// Check if cloudflared is available in PATH
    ///
    /// # Returns
    /// `true` if cloudflared is found and executable, `false` otherwise
    pub fn check_cloudflared() -> bool {
        Command::new("cloudflared")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Start a tunnel to the given local port
    ///
    /// Spawns cloudflared and waits for it to provide a public URL.
    /// The tunnel proxies traffic from the public URL to `http://localhost:<port>`.
    ///
    /// # Arguments
    /// * `port` - Local port to tunnel to
    ///
    /// # Returns
    /// A `TunnelManager` instance on success, or a `TunnelError` on failure
    ///
    /// # Errors
    /// - `CloudflaredNotFound` if cloudflared is not in PATH
    /// - `SpawnFailed` if the process cannot be started
    /// - `UrlTimeout` if no URL is provided within the timeout
    /// - `ProcessExited` if cloudflared exits unexpectedly
    pub fn start(port: u16) -> Result<Self, TunnelError> {
        if !Self::check_cloudflared() {
            return Err(TunnelError::CloudflaredNotFound);
        }

        let local_url = format!("http://localhost:{}", port);

        // Spawn cloudflared with stderr piped for URL extraction
        let mut child = Command::new("cloudflared")
            .args(["tunnel", "--url", &local_url])
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(TunnelError::SpawnFailed)?;

        let public_url = Arc::new(Mutex::new(None));
        let url_clone = Arc::clone(&public_url);

        // Take stderr before moving child
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| TunnelError::Internal("Failed to capture stderr".to_string()))?;

        // Spawn thread to read stderr and extract URL
        let url_regex =
            Regex::new(r"https://[a-z0-9-]+\.trycloudflare\.com").expect("Invalid regex");

        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                // Log stderr for debugging (could add tracing later)
                eprintln!("[cloudflared] {}", line);

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
        let err = TunnelError::CloudflaredNotFound;
        let msg = format!("{}", err);
        assert!(msg.contains("cloudflared not found"));
        assert!(msg.contains("brew install"));

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
    fn test_check_cloudflared_returns_bool() {
        // Just verify it returns a bool without panicking
        // The actual result depends on whether cloudflared is installed
        let _result: bool = TunnelManager::check_cloudflared();
    }

    #[test]
    fn test_tunnel_error_is_error() {
        // Verify TunnelError implements std::error::Error
        fn assert_error<T: std::error::Error>() {}
        assert_error::<TunnelError>();
    }

    #[test]
    fn test_url_regex_pattern() {
        let regex = Regex::new(r"https://[a-z0-9-]+\.trycloudflare\.com").unwrap();

        // Should match valid cloudflare URLs
        assert!(regex.is_match("https://random-words-here.trycloudflare.com"));
        assert!(regex.is_match("https://abc-123.trycloudflare.com"));
        assert!(regex.is_match("https://a.trycloudflare.com"));

        // Should not match invalid URLs
        assert!(!regex.is_match("http://random.trycloudflare.com")); // http not https
        assert!(!regex.is_match("https://random.example.com")); // wrong domain
        assert!(!regex.is_match("random.trycloudflare.com")); // missing https
    }

    #[test]
    fn test_tunnel_manager_port() {
        // We can't easily test start() without cloudflared installed,
        // but we can at least verify the struct is properly defined
        let url = Arc::new(Mutex::new(Some(
            "https://test.trycloudflare.com".to_string(),
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
            Some("https://test.trycloudflare.com".to_string())
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
