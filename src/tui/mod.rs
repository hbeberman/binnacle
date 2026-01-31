//! Terminal User Interface module for binnacle
//!
//! This module provides a keyboard-driven TUI for real-time cluster monitoring.
//! It connects to a running `bn serve` instance via WebSocket and displays
//! queue status, recently completed items, and node details.

#[cfg(feature = "tui")]
mod app;
#[cfg(feature = "tui")]
mod connection;

#[cfg(feature = "tui")]
pub use app::run_tui;
