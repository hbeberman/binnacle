//! GUI module for web-based visualization of binnacle data
//!
//! This module provides a local web server with live-updating views of tasks,
//! dependencies, tests, and activity logs.

#[cfg(feature = "gui")]
pub mod embedded;
#[cfg(feature = "gui")]
mod pid_file;
/// WebSocket protocol types for session server communication.
#[cfg(feature = "gui")]
pub mod protocol;
#[cfg(feature = "gui")]
mod server;
#[cfg(feature = "gui")]
pub mod session_log;
#[cfg(feature = "gui")]
pub mod tunnel;
#[cfg(feature = "gui")]
mod watcher;
#[cfg(feature = "gui")]
mod websocket;

/// Shared rendering module for platform-agnostic graph visualization
///
/// This module contains code that can be shared between the native GUI server
/// and the WASM-based viewer.
pub mod shared;

#[cfg(feature = "gui")]
pub use pid_file::{GuiPidFile, GuiPidInfo, ProcessStatus, verify_process};
#[cfg(feature = "gui")]
pub use protocol::{Change, ClientMessage, GraphState, ServerMessage, StateSummary};
#[cfg(feature = "gui")]
pub use server::{
    DEFAULT_PORT, derive_repo_name, find_available_port, get_current_branch, get_repo_display_name,
    start_server, start_session_server,
};
#[cfg(feature = "gui")]
pub use tunnel::{TunnelError, TunnelManager};
