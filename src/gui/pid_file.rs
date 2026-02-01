//! PID file management for GUI server process tracking.
//!
//! This module provides the `GuiPidFile` struct for tracking the GUI server process
//! across invocations. The PID file is stored in the binnacle data directory.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Result of process verification
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessStatus {
    /// Process exists and appears to be a binnacle GUI process
    Running,
    /// Process with this PID does not exist
    NotRunning,
    /// Process exists but is not a binnacle process (PID was recycled)
    Stale,
}

/// Information stored in the GUI PID file
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiPidInfo {
    /// Process ID of the running GUI server
    pub pid: u32,
    /// Port the server is listening on
    pub port: u16,
    /// Host/address the server is bound to
    pub host: String,
}

/// Manages the GUI server PID file for process lifecycle tracking.
///
/// The PID file is stored in the binnacle storage directory as `gui.pid`
/// and contains the process ID, port, and host in a simple format:
/// ```text
/// PID=12345
/// PORT=3030
/// HOST=127.0.0.1
/// ```
#[derive(Debug)]
pub struct GuiPidFile {
    path: PathBuf,
}

impl GuiPidFile {
    /// Create a new GuiPidFile for the given binnacle storage directory.
    ///
    /// # Arguments
    /// * `storage_dir` - The binnacle storage directory (e.g., ~/.local/share/binnacle/<hash>)
    pub fn new(storage_dir: &Path) -> Self {
        Self {
            path: storage_dir.join("gui.pid"),
        }
    }

    /// Get the path to the PID file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Write the PID file with the given process info.
    ///
    /// Creates the parent directory if it doesn't exist.
    ///
    /// # Arguments
    /// * `info` - Process information to write
    ///
    /// # Errors
    /// Returns an IO error if the file cannot be written.
    pub fn write(&self, info: &GuiPidInfo) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let contents = format!("PID={}\nPORT={}\nHOST={}\n", info.pid, info.port, info.host);

        let mut file = fs::File::create(&self.path)?;
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;

        Ok(())
    }

    /// Read the PID file and parse its contents.
    ///
    /// # Returns
    /// * `Ok(Some(info))` if the file exists and was parsed successfully
    /// * `Ok(None)` if the file doesn't exist
    /// * `Err(e)` if there was an IO error or parse error
    pub fn read(&self) -> io::Result<Option<GuiPidInfo>> {
        match fs::read_to_string(&self.path) {
            Ok(contents) => {
                let info = Self::parse_contents(&contents)?;
                Ok(Some(info))
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Delete the PID file if it exists.
    ///
    /// Does nothing if the file doesn't exist.
    ///
    /// # Errors
    /// Returns an IO error if the file exists but cannot be deleted.
    pub fn delete(&self) -> io::Result<()> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Check if a PID file exists.
    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    /// Verify if the process recorded in the PID file is still running.
    ///
    /// This reads the PID file and checks if the process is:
    /// - Still running
    /// - Actually a binnacle process (not a recycled PID)
    ///
    /// # Returns
    /// * `Ok(Some(ProcessStatus::Running, info))` if process exists and is binnacle
    /// * `Ok(Some(ProcessStatus::NotRunning, info))` if process no longer exists
    /// * `Ok(Some(ProcessStatus::Stale, info))` if PID was recycled to another process
    /// * `Ok(None)` if no PID file exists
    /// * `Err(e)` on IO errors
    pub fn check_running(&self) -> io::Result<Option<(ProcessStatus, GuiPidInfo)>> {
        match self.read()? {
            Some(info) => {
                let status = verify_process(info.pid);
                Ok(Some((status, info)))
            }
            None => Ok(None),
        }
    }

    /// Parse PID file contents into GuiPidInfo.
    fn parse_contents(contents: &str) -> io::Result<GuiPidInfo> {
        let mut pid: Option<u32> = None;
        let mut port: Option<u16> = None;
        let mut host: Option<String> = None;

        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                match key {
                    "PID" => {
                        pid = Some(value.parse().map_err(|_| {
                            io::Error::new(io::ErrorKind::InvalidData, "Invalid PID value")
                        })?);
                    }
                    "PORT" => {
                        port = Some(value.parse().map_err(|_| {
                            io::Error::new(io::ErrorKind::InvalidData, "Invalid PORT value")
                        })?);
                    }
                    "HOST" => {
                        host = Some(value.to_string());
                    }
                    _ => {} // Ignore unknown keys for forward compatibility
                }
            }
        }

        let pid =
            pid.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Missing PID field"))?;
        let port =
            port.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Missing PORT field"))?;
        let host =
            host.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Missing HOST field"))?;

        Ok(GuiPidInfo { pid, port, host })
    }
}

/// Verify if a process with the given PID is a running binnacle process.
///
/// Cross-platform implementation:
/// - On Unix: checks /proc/<pid>/comm or uses kill(pid, 0) as fallback
/// - On Windows: uses process snapshot APIs
pub fn verify_process(pid: u32) -> ProcessStatus {
    #[cfg(target_os = "linux")]
    {
        verify_process_linux(pid)
    }

    #[cfg(target_os = "macos")]
    {
        verify_process_macos(pid)
    }

    #[cfg(target_os = "windows")]
    {
        verify_process_windows(pid)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        verify_process_fallback(pid)
    }
}

/// Linux implementation using /proc filesystem
#[cfg(target_os = "linux")]
fn verify_process_linux(pid: u32) -> ProcessStatus {
    use std::fs;

    let comm_path = format!("/proc/{}/comm", pid);
    match fs::read_to_string(&comm_path) {
        Ok(comm) => {
            let comm = comm.trim();
            // Check if the process name matches "bn" (our binary name)
            if comm == "bn" {
                ProcessStatus::Running
            } else {
                ProcessStatus::Stale
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => ProcessStatus::NotRunning,
        Err(_) => {
            // Permission denied or other error - fall back to signal check
            verify_process_signal(pid)
        }
    }
}

/// macOS implementation using sysctl
#[cfg(target_os = "macos")]
fn verify_process_macos(pid: u32) -> ProcessStatus {
    use std::process::Command;

    // Use ps command to check process name
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let comm = String::from_utf8_lossy(&out.stdout);
            let comm = comm.trim();
            // On macOS, the full path may be shown or just the name
            if comm.ends_with("bn") || comm == "bn" {
                ProcessStatus::Running
            } else {
                ProcessStatus::Stale
            }
        }
        Ok(_) => ProcessStatus::NotRunning,
        Err(_) => verify_process_signal(pid),
    }
}

/// Windows implementation using process APIs
#[cfg(target_os = "windows")]
fn verify_process_windows(pid: u32) -> ProcessStatus {
    use std::process::Command;

    // Use tasklist to check process
    let output = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/FO", "CSV", "/NH"])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let output_str = String::from_utf8_lossy(&out.stdout);
            if output_str.contains("bn.exe") {
                ProcessStatus::Running
            } else if output_str.contains(&pid.to_string()) {
                // Process exists but not our process
                ProcessStatus::Stale
            } else {
                ProcessStatus::NotRunning
            }
        }
        Ok(_) => ProcessStatus::NotRunning,
        Err(_) => ProcessStatus::NotRunning,
    }
}

/// Fallback for unsupported platforms using signal 0
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn verify_process_fallback(pid: u32) -> ProcessStatus {
    verify_process_signal(pid)
}

/// Signal-based check (Unix only) - cannot distinguish our process from others
#[cfg(unix)]
fn verify_process_signal(pid: u32) -> ProcessStatus {
    use std::process::Command;

    // Try kill -0 to check if process exists
    let result = Command::new("kill").args(["-0", &pid.to_string()]).output();

    match result {
        Ok(out) if out.status.success() => {
            // Process exists, but we can't verify it's ours
            // Return Running as a best-effort (safer to assume it's ours)
            ProcessStatus::Running
        }
        _ => ProcessStatus::NotRunning,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, GuiPidFile) {
        let temp_dir = TempDir::new().unwrap();
        let pid_file = GuiPidFile::new(temp_dir.path());
        (temp_dir, pid_file)
    }

    #[test]
    fn test_new_creates_correct_path() {
        let (_temp_dir, pid_file) = setup();
        assert!(pid_file.path().ends_with("gui.pid"));
    }

    #[test]
    fn test_write_and_read() {
        let (_temp_dir, pid_file) = setup();

        let info = GuiPidInfo {
            pid: 12345,
            port: 3030,
            host: "127.0.0.1".to_string(),
        };

        pid_file.write(&info).unwrap();
        let read_info = pid_file.read().unwrap().unwrap();

        assert_eq!(read_info, info);
    }

    #[test]
    fn test_read_nonexistent_returns_none() {
        let (_temp_dir, pid_file) = setup();
        assert_eq!(pid_file.read().unwrap(), None);
    }

    #[test]
    fn test_delete_existing_file() {
        let (_temp_dir, pid_file) = setup();

        let info = GuiPidInfo {
            pid: 12345,
            port: 3030,
            host: "127.0.0.1".to_string(),
        };

        pid_file.write(&info).unwrap();
        assert!(pid_file.exists());

        pid_file.delete().unwrap();
        assert!(!pid_file.exists());
    }

    #[test]
    fn test_delete_nonexistent_file_succeeds() {
        let (_temp_dir, pid_file) = setup();
        assert!(!pid_file.exists());

        // Should not error on nonexistent file
        pid_file.delete().unwrap();
    }

    #[test]
    fn test_exists() {
        let (_temp_dir, pid_file) = setup();

        assert!(!pid_file.exists());

        let info = GuiPidInfo {
            pid: 1,
            port: 8080,
            host: "localhost".to_string(),
        };
        pid_file.write(&info).unwrap();

        assert!(pid_file.exists());
    }

    #[test]
    fn test_parse_ignores_unknown_keys() {
        let contents = "PID=100\nFUTURE_KEY=value\nPORT=8080\nHOST=0.0.0.0\n";
        let info = GuiPidFile::parse_contents(contents).unwrap();

        assert_eq!(info.pid, 100);
        assert_eq!(info.port, 8080);
        assert_eq!(info.host, "0.0.0.0");
    }

    #[test]
    fn test_parse_handles_empty_lines() {
        let contents = "PID=100\n\nPORT=8080\n\nHOST=localhost\n";
        let info = GuiPidFile::parse_contents(contents).unwrap();

        assert_eq!(info.pid, 100);
        assert_eq!(info.port, 8080);
        assert_eq!(info.host, "localhost");
    }

    #[test]
    fn test_parse_missing_pid_errors() {
        let contents = "PORT=8080\nHOST=localhost\n";
        let result = GuiPidFile::parse_contents(contents);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_missing_port_errors() {
        let contents = "PID=100\nHOST=localhost\n";
        let result = GuiPidFile::parse_contents(contents);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_missing_host_errors() {
        let contents = "PID=100\nPORT=8080\n";
        let result = GuiPidFile::parse_contents(contents);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_pid_errors() {
        let contents = "PID=notanumber\nPORT=8080\nHOST=localhost\n";
        let result = GuiPidFile::parse_contents(contents);
        assert!(result.is_err());
    }

    #[test]
    fn test_write_creates_parent_directory() {
        let temp_dir = TempDir::new().unwrap();
        let nested_dir = temp_dir.path().join("nested").join("dir");
        let pid_file = GuiPidFile::new(&nested_dir);

        let info = GuiPidInfo {
            pid: 1,
            port: 3030,
            host: "127.0.0.1".to_string(),
        };

        pid_file.write(&info).unwrap();
        assert!(pid_file.exists());
    }

    #[test]
    fn test_ipv6_host() {
        let (_temp_dir, pid_file) = setup();

        let info = GuiPidInfo {
            pid: 12345,
            port: 3030,
            host: "::1".to_string(),
        };

        pid_file.write(&info).unwrap();
        let read_info = pid_file.read().unwrap().unwrap();

        assert_eq!(read_info.host, "::1");
    }

    #[test]
    fn test_check_running_no_pid_file() {
        let (_temp_dir, pid_file) = setup();
        let result = pid_file.check_running().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_check_running_with_pid_file() {
        let (_temp_dir, pid_file) = setup();

        // Use a PID that definitely doesn't exist (very high number)
        let info = GuiPidInfo {
            pid: 999999999,
            port: 3030,
            host: "127.0.0.1".to_string(),
        };
        pid_file.write(&info).unwrap();

        let result = pid_file.check_running().unwrap();
        assert!(result.is_some());
        let (status, read_info) = result.unwrap();
        assert_eq!(status, ProcessStatus::NotRunning);
        assert_eq!(read_info.pid, 999999999);
    }

    #[test]
    fn test_verify_nonexistent_process() {
        // PID that almost certainly doesn't exist
        let status = super::verify_process(999999999);
        assert_eq!(status, ProcessStatus::NotRunning);
    }

    #[test]
    #[cfg(unix)]
    fn test_verify_current_process() {
        // Current process exists, but isn't named "bn" so should be Stale
        let current_pid = std::process::id();
        let status = super::verify_process(current_pid);
        // Should be either Stale (not named "bn") or Running (fallback mode)
        assert!(status == ProcessStatus::Stale || status == ProcessStatus::Running);
    }

    #[test]
    fn test_process_status_clone_eq() {
        let s1 = ProcessStatus::Running;
        let s2 = s1.clone();
        assert_eq!(s1, s2);

        let s3 = ProcessStatus::NotRunning;
        assert_ne!(s1, s3);
    }
}
