//! Terminal User Interface module for binnacle
//!
//! This module provides a keyboard-driven TUI for real-time cluster monitoring.
//! It connects to a session server (`bn session serve`) via WebSocket and displays
//! queue status, recently completed items, and node details.

#[cfg(feature = "tui")]
mod app;
#[cfg(feature = "tui")]
mod connection;
#[cfg(feature = "tui")]
mod notifications;
#[cfg(feature = "tui")]
mod views;

#[cfg(feature = "tui")]
pub use app::DEFAULT_PORT;
#[cfg(feature = "tui")]
pub use app::InputMode;
#[cfg(feature = "tui")]
pub use app::run_tui;
#[cfg(feature = "tui")]
pub use notifications::{NotificationLevel, NotificationManager, Toast};
#[cfg(feature = "tui")]
pub use views::CompletedItem;
#[cfg(feature = "tui")]
pub use views::EdgeInfo;
#[cfg(feature = "tui")]
pub use views::LogEntry;
#[cfg(feature = "tui")]
pub use views::LogPanelView;
#[cfg(feature = "tui")]
pub use views::NodeDetail;
#[cfg(feature = "tui")]
pub use views::WorkItem;
