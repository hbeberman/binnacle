//! GUI module for web-based visualization of binnacle data
//!
//! This module provides a local web server with live-updating views of tasks,
//! dependencies, tests, and activity logs.

#[cfg(feature = "gui")]
mod pid_file;
#[cfg(feature = "gui")]
mod server;
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
pub use server::{DEFAULT_PORT, find_available_port, start_server};
