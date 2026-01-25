//! WASM module for binnacle browser-based viewer
//!
//! This module contains WebAssembly-specific code for running binnacle's
//! graph viewer in the browser. It uses the shared rendering module from
//! `gui::shared` and provides JavaScript bindings via wasm-bindgen.
//!
//! # Architecture
//!
//! The WASM viewer is designed to be a self-contained, read-only viewer that
//! can load and display binnacle graphs from `.bng` archive files. It shares
//! the core rendering logic with the native GUI server.
//!
//! ```text
//! ┌─────────────────────────────────────────────────┐
//! │                  Browser                         │
//! │  ┌──────────────────────────────────────────┐   │
//! │  │            JavaScript Layer               │   │
//! │  │  - UI event handling                      │   │
//! │  │  - Canvas rendering                       │   │
//! │  │  - Archive fetching                       │   │
//! │  └────────────────┬─────────────────────────┘   │
//! │                   │ wasm-bindgen                 │
//! │  ┌────────────────▼─────────────────────────┐   │
//! │  │              WASM Module                   │   │
//! │  │  - Archive parsing (.bng → Graph)         │   │
//! │  │  - Layout computation                     │   │
//! │  │  - Render command generation              │   │
//! │  └────────────────┬─────────────────────────┘   │
//! │                   │                             │
//! │  ┌────────────────▼─────────────────────────┐   │
//! │  │         gui::shared Module                │   │
//! │  │  - Layout engine                          │   │
//! │  │  - Render commands                        │   │
//! │  │  - Theme constants                        │   │
//! │  └──────────────────────────────────────────┘   │
//! └─────────────────────────────────────────────────┘
//! ```
//!
//! # Modules
//!
//! - `archive`: Load and parse `.bng` archive files
//! - `bindings`: wasm-bindgen exports for JavaScript interop
//!
//! # Usage
//!
//! The WASM module is compiled with `wasm-pack build --target web` and
//! produces a JavaScript module that can be imported in the browser:
//!
//! ```javascript
//! import init, { BinnacleViewer } from './binnacle_wasm.js';
//!
//! async function main() {
//!     await init();
//!     const viewer = new BinnacleViewer();
//!     await viewer.loadFromUrl('https://example.com/project.bng');
//!     viewer.render(canvas);
//! }
//! ```

// Archive module - contains graph data structures that work on all platforms
mod archive;
// Bindings module - includes both wasm_bindgen exports for wasm32 and
// the internal structs for testing on all platforms
mod bindings;

pub use archive::*;
// Export bindings on all platforms for testing
pub use bindings::*;

/// Initialize WASM panic hook for better error messages in browser console
///
/// This should be called once at startup to convert Rust panics into
/// readable JavaScript errors with stack traces.
#[cfg(target_arch = "wasm32")]
pub fn init_panic_hook() {
    #[cfg(feature = "wasm")]
    console_error_panic_hook::set_once();
}

/// Version information for the WASM module
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_is_set() {
        assert!(!VERSION.is_empty());
    }
}
