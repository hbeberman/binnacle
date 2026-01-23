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

#[cfg(feature = "gui")]
pub use pid_file::{GuiPidFile, GuiPidInfo, ProcessStatus, verify_process};
#[cfg(feature = "gui")]
pub use server::start_server;
