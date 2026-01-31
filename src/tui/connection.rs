//! WebSocket connection management for TUI
//!
//! Handles connection state tracking and automatic reconnection with exponential backoff.

use std::time::{Duration, Instant};

/// Maximum reconnection attempts before giving up
pub const MAX_RECONNECT_ATTEMPTS: u32 = 10;

/// Maximum backoff duration in seconds
pub const MAX_BACKOFF_SECS: u64 = 8;

/// Debounce delay before state refresh after reconnect (milliseconds)
pub const RECONNECT_DEBOUNCE_MS: u64 = 500;

/// Connection state enum
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    /// Connected to the server
    Connected,
    /// Attempting to reconnect
    Reconnecting {
        attempt: u32,
        next_retry: Option<Instant>,
    },
    /// Connection failed, needs manual intervention (max retries exceeded)
    Disconnected,
}

impl ConnectionState {
    /// Check if currently connected
    pub fn is_connected(&self) -> bool {
        matches!(self, ConnectionState::Connected)
    }

    /// Check if in reconnecting state
    #[allow(dead_code)]
    pub fn is_reconnecting(&self) -> bool {
        matches!(self, ConnectionState::Reconnecting { .. })
    }

    /// Get current reconnect attempt number (0 if not reconnecting)
    #[allow(dead_code)]
    pub fn reconnect_attempt(&self) -> u32 {
        match self {
            ConnectionState::Reconnecting { attempt, .. } => *attempt,
            _ => 0,
        }
    }
}

/// Calculate exponential backoff duration for a given attempt number
///
/// Attempt 1: 0 seconds (immediate)
/// Attempt 2: 1 second
/// Attempt 3: 2 seconds
/// Attempt 4: 4 seconds
/// Attempt 5+: 8 seconds (max)
pub fn calculate_backoff(attempt: u32) -> Duration {
    if attempt <= 1 {
        Duration::from_secs(0)
    } else {
        let exponent = attempt.saturating_sub(2);
        // Cap the exponent to avoid overflow (2^63 would overflow)
        // We only need up to 2^3 = 8 anyway since MAX_BACKOFF_SECS is 8
        let secs = if exponent >= 63 {
            MAX_BACKOFF_SECS
        } else {
            2u64.pow(exponent).min(MAX_BACKOFF_SECS)
        };
        Duration::from_secs(secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backoff_attempt_1_immediate() {
        assert_eq!(calculate_backoff(1), Duration::from_secs(0));
    }

    #[test]
    fn test_backoff_attempt_2_one_second() {
        assert_eq!(calculate_backoff(2), Duration::from_secs(1));
    }

    #[test]
    fn test_backoff_attempt_3_two_seconds() {
        assert_eq!(calculate_backoff(3), Duration::from_secs(2));
    }

    #[test]
    fn test_backoff_attempt_4_four_seconds() {
        assert_eq!(calculate_backoff(4), Duration::from_secs(4));
    }

    #[test]
    fn test_backoff_attempt_5_capped_at_max() {
        assert_eq!(calculate_backoff(5), Duration::from_secs(MAX_BACKOFF_SECS));
    }

    #[test]
    fn test_backoff_large_attempt_capped() {
        assert_eq!(
            calculate_backoff(100),
            Duration::from_secs(MAX_BACKOFF_SECS)
        );
    }

    #[test]
    fn test_connection_state_is_connected() {
        assert!(ConnectionState::Connected.is_connected());
        assert!(!ConnectionState::Disconnected.is_connected());
        assert!(
            !ConnectionState::Reconnecting {
                attempt: 1,
                next_retry: None
            }
            .is_connected()
        );
    }

    #[test]
    fn test_connection_state_is_reconnecting() {
        assert!(!ConnectionState::Connected.is_reconnecting());
        assert!(!ConnectionState::Disconnected.is_reconnecting());
        assert!(
            ConnectionState::Reconnecting {
                attempt: 1,
                next_retry: None
            }
            .is_reconnecting()
        );
    }

    #[test]
    fn test_connection_state_reconnect_attempt() {
        assert_eq!(ConnectionState::Connected.reconnect_attempt(), 0);
        assert_eq!(ConnectionState::Disconnected.reconnect_attempt(), 0);
        assert_eq!(
            ConnectionState::Reconnecting {
                attempt: 5,
                next_retry: None
            }
            .reconnect_attempt(),
            5
        );
    }
}
