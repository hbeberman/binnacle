//! JavaScript bindings for WASM viewer
//!
//! This module provides wasm-bindgen exports that allow JavaScript to interact
//! with the binnacle graph viewer. It exposes a high-level API for:
//!
//! - Loading archives from URLs or ArrayBuffers
//! - Running the layout algorithm
//! - Getting render commands for canvas drawing
//!
//! See task bn-eff1 for full implementation.

use crate::gui::shared::{LayoutConfig, LayoutEngine, Position};

/// Viewer state for the WASM module
///
/// This struct holds the graph data and layout state, and is exposed
/// to JavaScript via wasm-bindgen.
#[derive(Debug, Default)]
pub struct ViewerState {
    /// Layout engine with node positions
    pub engine: LayoutEngine,
    /// Whether the layout has been computed
    pub layout_ready: bool,
    /// Camera position (center of viewport)
    pub camera: Position,
    /// Zoom level (1.0 = 100%)
    pub zoom: f64,
}

impl ViewerState {
    /// Create a new viewer state
    pub fn new() -> Self {
        Self {
            engine: LayoutEngine::new(),
            layout_ready: false,
            camera: Position::default(),
            zoom: 1.0,
        }
    }

    /// Create a viewer state with custom layout config
    pub fn with_config(config: LayoutConfig) -> Self {
        Self {
            engine: LayoutEngine::with_config(config),
            layout_ready: false,
            camera: Position::default(),
            zoom: 1.0,
        }
    }

    /// Run one iteration of the layout algorithm
    ///
    /// Returns the maximum speed of any node (for convergence checking).
    pub fn tick(&mut self) -> f64 {
        self.engine.tick()
    }

    /// Run layout until convergence or max iterations
    ///
    /// Returns the number of iterations run.
    pub fn run_layout(&mut self, max_iterations: usize) -> usize {
        let iterations = self.engine.run(max_iterations);
        self.layout_ready = true;
        iterations
    }

    /// Check if layout has converged
    pub fn is_stable(&self) -> bool {
        self.engine.is_stable()
    }

    /// Pan the camera
    pub fn pan(&mut self, dx: f64, dy: f64) {
        self.camera.x += dx / self.zoom;
        self.camera.y += dy / self.zoom;
    }

    /// Zoom the camera
    pub fn set_zoom(&mut self, zoom: f64) {
        self.zoom = zoom.clamp(0.1, 5.0);
    }

    /// Focus on a specific node
    pub fn focus_node(&mut self, id: &str) {
        if let Some(node) = self.engine.get_node(id) {
            self.camera = node.position;
        }
    }

    /// Get node count
    pub fn node_count(&self) -> usize {
        self.engine.nodes.len()
    }

    /// Get edge count
    pub fn edge_count(&self) -> usize {
        self.engine.edges.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::shared::LayoutNode;

    #[test]
    fn test_viewer_state_new() {
        let state = ViewerState::new();
        assert_eq!(state.node_count(), 0);
        assert_eq!(state.edge_count(), 0);
        assert!(!state.layout_ready);
        assert!((state.zoom - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_viewer_state_pan() {
        let mut state = ViewerState::new();
        state.pan(100.0, 50.0);
        assert!((state.camera.x - 100.0).abs() < 0.0001);
        assert!((state.camera.y - 50.0).abs() < 0.0001);
    }

    #[test]
    fn test_viewer_state_pan_with_zoom() {
        let mut state = ViewerState::new();
        state.set_zoom(2.0);
        state.pan(100.0, 50.0);
        // At 2x zoom, pan should be halved
        assert!((state.camera.x - 50.0).abs() < 0.0001);
        assert!((state.camera.y - 25.0).abs() < 0.0001);
    }

    #[test]
    fn test_viewer_state_zoom_clamp() {
        let mut state = ViewerState::new();

        state.set_zoom(0.01);
        assert!((state.zoom - 0.1).abs() < 0.0001);

        state.set_zoom(10.0);
        assert!((state.zoom - 5.0).abs() < 0.0001);
    }

    #[test]
    fn test_viewer_state_focus_node() {
        let mut state = ViewerState::new();
        state
            .engine
            .add_node(LayoutNode::with_position("bn-1234", 100.0, 200.0));

        state.focus_node("bn-1234");
        assert!((state.camera.x - 100.0).abs() < 0.0001);
        assert!((state.camera.y - 200.0).abs() < 0.0001);

        // Focus on non-existent node should do nothing
        let old_camera = state.camera;
        state.focus_node("bn-9999");
        assert!((state.camera.x - old_camera.x).abs() < 0.0001);
    }

    #[test]
    fn test_viewer_state_run_layout() {
        let mut state = ViewerState::new();
        state
            .engine
            .add_node(LayoutNode::with_position("bn-1", 100.0, 0.0));
        state
            .engine
            .add_node(LayoutNode::with_position("bn-2", -100.0, 0.0));

        assert!(!state.layout_ready);
        let iterations = state.run_layout(100);
        assert!(state.layout_ready);
        assert!(iterations > 0);
    }
}
