//! WebSocket connection management for TUI
//!
//! Handles connection state tracking for the TUI.

/// Connection state enum
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    /// Connected to the server
    Connected,
    /// Attempting to reconnect (future use)
    #[allow(dead_code)]
    Reconnecting { attempt: u32 },
    /// Connection failed, needs manual intervention
    Disconnected,
}
