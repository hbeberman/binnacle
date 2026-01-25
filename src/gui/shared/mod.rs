//! Shared rendering module for binnacle GUI
//!
//! This module contains platform-agnostic rendering logic that can be shared
//! between the native GUI server and the WASM-based viewer.
//!
//! # Modules
//!
//! - `theme`: Color scheme and CSS variables as Rust constants
//! - `layout`: Force-directed graph layout algorithm
//! - `render`: Abstract rendering commands for nodes and edges

pub mod layout;
pub mod render;
pub mod theme;

pub use layout::*;
pub use render::*;
pub use theme::*;
