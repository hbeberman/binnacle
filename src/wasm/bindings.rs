//! JavaScript bindings for WASM viewer
//!
//! This module provides wasm-bindgen exports that allow JavaScript to interact
//! with the binnacle graph viewer. It exposes a high-level API for:
//!
//! - Loading archives from bytes (ArrayBuffer)
//! - Running the layout algorithm
//! - Getting render commands for canvas drawing
//! - Camera control (pan, zoom, focus)
//!
//! When compiled for wasm32, this module uses wasm_bindgen to expose the API
//! to JavaScript. On other platforms, it provides the same API for testing.

use crate::gui::shared::{LayoutConfig, LayoutEdge, LayoutEngine, LayoutNode, NodeType, Position};

// Only import wasm_bindgen on wasm32 target
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

// Mock JsValue for non-wasm targets (testing only)
#[cfg(not(target_arch = "wasm32"))]
mod mock_js {
    #[derive(Debug)]
    pub struct JsValue(#[allow(dead_code)] String);

    impl JsValue {
        pub const NULL: JsValue = JsValue(String::new());

        pub fn from_str(s: &str) -> Self {
            JsValue(s.to_string())
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
use mock_js::JsValue;

/// Initialize the WASM module
///
/// This should be called once at startup to set up panic hooks for better
/// error messages in the browser console.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn init() {
    #[cfg(all(target_arch = "wasm32", feature = "wasm"))]
    console_error_panic_hook::set_once();
}

/// Get the binnacle version
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// The main viewer class exposed to JavaScript
///
/// This provides a high-level API for loading and displaying binnacle graphs
/// in the browser. Use it like:
///
/// ```javascript
/// import init, { BinnacleViewer } from './binnacle_wasm.js';
///
/// async function main() {
///     await init();
///     const viewer = new BinnacleViewer();
///     await viewer.loadFromBytes(archiveData);
///     viewer.runLayout(500);
///     const nodes = viewer.getNodesJson();
///     const edges = viewer.getEdgesJson();
/// }
/// ```
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct BinnacleViewer {
    state: ViewerState,
    /// Entity metadata stored separately for JavaScript access
    entities: Vec<EntityInfo>,
    /// Edge metadata for JavaScript access
    edge_info: Vec<EdgeInfo>,
}

/// Entity information for JavaScript
#[derive(Debug, Clone)]
struct EntityInfo {
    id: String,
    entity_type: String,
    title: String,
    short_name: Option<String>,
    status: String,
    priority: u8,
    tags: Vec<String>,
    doc_type: Option<String>,
}

/// Edge information for JavaScript
#[derive(Debug, Clone)]
struct EdgeInfo {
    source: String,
    target: String,
    edge_type: String,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl BinnacleViewer {
    /// Create a new empty viewer
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new() -> Self {
        Self {
            state: ViewerState::new(),
            entities: Vec::new(),
            edge_info: Vec::new(),
        }
    }

    /// Load graph data from parsed JSON
    ///
    /// Expects a JSON object with `entities` and `edges` arrays.
    /// This is typically parsed from a .bng archive.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = loadFromJson))]
    pub fn load_from_json(&mut self, json: &str) -> Result<(), JsValue> {
        let data: serde_json::Value =
            serde_json::from_str(json).map_err(|e| JsValue::from_str(&e.to_string()))?;

        // Clear existing data
        self.state = ViewerState::new();
        self.entities.clear();
        self.edge_info.clear();

        // Parse entities
        if let Some(entities) = data.get("entities").and_then(|v| v.as_array()) {
            for (i, entity) in entities.iter().enumerate() {
                let id = entity
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let entity_type = entity
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("task")
                    .to_string();
                let title = entity
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let short_name = entity
                    .get("short_name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let status = entity
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("pending")
                    .to_string();
                let priority = entity.get("priority").and_then(|v| v.as_u64()).unwrap_or(2) as u8;
                let tags = entity
                    .get("tags")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                let doc_type = entity
                    .get("doc_type")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                // Add layout node with initial circular position
                let angle =
                    (i as f64) * 2.0 * std::f64::consts::PI / (entities.len().max(1) as f64);
                let radius = 300.0;
                let x = angle.cos() * radius;
                let y = angle.sin() * radius;

                let node_type = if id.starts_with("bnq-") {
                    NodeType::Queue
                } else {
                    NodeType::Normal
                };

                self.state
                    .engine
                    .add_node(LayoutNode::with_position(&id, x, y).with_type(node_type));

                self.entities.push(EntityInfo {
                    id,
                    entity_type,
                    title,
                    short_name,
                    status,
                    priority,
                    tags,
                    doc_type,
                });
            }
        }

        // Parse edges
        if let Some(edges) = data.get("edges").and_then(|v| v.as_array()) {
            for edge in edges {
                let source = edge
                    .get("source")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let target = edge
                    .get("target")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let edge_type = edge
                    .get("edge_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("depends_on")
                    .to_string();

                // Disable spring for hierarchy edges
                let layout_edge = if edge_type == "child_of" || edge_type == "parent_of" {
                    LayoutEdge::without_spring(&source, &target)
                } else {
                    LayoutEdge::new(&source, &target)
                };
                self.state.engine.add_edge(layout_edge);

                self.edge_info.push(EdgeInfo {
                    source,
                    target,
                    edge_type,
                });
            }
        }

        Ok(())
    }

    /// Run the layout algorithm for up to max_iterations
    ///
    /// Returns the number of iterations actually run (may be less if converged early).
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = runLayout))]
    pub fn run_layout(&mut self, max_iterations: usize) -> usize {
        self.state.run_layout(max_iterations)
    }

    /// Run a single layout iteration
    ///
    /// Returns the maximum node speed (useful for checking convergence).
    pub fn tick(&mut self) -> f64 {
        self.state.tick()
    }

    /// Check if the layout has converged
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = isStable))]
    pub fn is_stable(&self) -> bool {
        self.state.is_stable()
    }

    /// Check if layout has been computed
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = isLayoutReady))]
    pub fn is_layout_ready(&self) -> bool {
        self.state.layout_ready
    }

    /// Pan the camera by the given delta
    pub fn pan(&mut self, dx: f64, dy: f64) {
        self.state.pan(dx, dy);
    }

    /// Set the zoom level (clamped to 0.1 - 5.0)
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = setZoom))]
    pub fn set_zoom(&mut self, zoom: f64) {
        self.state.set_zoom(zoom);
    }

    /// Get the current zoom level
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = getZoom))]
    pub fn get_zoom(&self) -> f64 {
        self.state.zoom
    }

    /// Get camera X position
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = getCameraX))]
    pub fn get_camera_x(&self) -> f64 {
        self.state.camera.x
    }

    /// Get camera Y position
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = getCameraY))]
    pub fn get_camera_y(&self) -> f64 {
        self.state.camera.y
    }

    /// Focus the camera on a specific node
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = focusNode))]
    pub fn focus_node(&mut self, id: &str) {
        self.state.focus_node(id);
    }

    /// Get the number of nodes
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = nodeCount))]
    pub fn node_count(&self) -> usize {
        self.state.node_count()
    }

    /// Get the number of edges
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = edgeCount))]
    pub fn edge_count(&self) -> usize {
        self.state.edge_count()
    }

    /// Get all nodes as JSON
    ///
    /// Returns a JSON array with node positions and metadata:
    /// ```json
    /// [
    ///   {
    ///     "id": "bn-1234",
    ///     "x": 100.0,
    ///     "y": 200.0,
    ///     "type": "task",
    ///     "title": "My Task",
    ///     "short_name": "task",
    ///     "status": "pending",
    ///     "priority": 2,
    ///     "tags": ["feature"]
    ///   }
    /// ]
    /// ```
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = getNodesJson))]
    pub fn get_nodes_json(&self) -> String {
        let mut nodes = Vec::new();

        for entity in &self.entities {
            if let Some(layout_node) = self.state.engine.get_node(&entity.id) {
                let node = serde_json::json!({
                    "id": entity.id,
                    "x": layout_node.position.x,
                    "y": layout_node.position.y,
                    "type": entity.entity_type,
                    "title": entity.title,
                    "short_name": entity.short_name,
                    "status": entity.status,
                    "priority": entity.priority,
                    "tags": entity.tags,
                    "doc_type": entity.doc_type,
                });
                nodes.push(node);
            }
        }

        serde_json::to_string(&nodes).unwrap_or_else(|_| "[]".to_string())
    }

    /// Get all edges as JSON
    ///
    /// Returns a JSON array with edge information:
    /// ```json
    /// [
    ///   {
    ///     "source": "bn-1234",
    ///     "target": "bn-5678",
    ///     "edge_type": "depends_on",
    ///     "source_x": 100.0,
    ///     "source_y": 200.0,
    ///     "target_x": 300.0,
    ///     "target_y": 400.0
    ///   }
    /// ]
    /// ```
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = getEdgesJson))]
    pub fn get_edges_json(&self) -> String {
        let mut edges = Vec::new();

        for edge in &self.edge_info {
            let source_pos = self.state.engine.get_node(&edge.source).map(|n| n.position);
            let target_pos = self.state.engine.get_node(&edge.target).map(|n| n.position);

            if let (Some(src), Some(tgt)) = (source_pos, target_pos) {
                let edge_json = serde_json::json!({
                    "source": edge.source,
                    "target": edge.target,
                    "edge_type": edge.edge_type,
                    "source_x": src.x,
                    "source_y": src.y,
                    "target_x": tgt.x,
                    "target_y": tgt.y,
                });
                edges.push(edge_json);
            }
        }

        serde_json::to_string(&edges).unwrap_or_else(|_| "[]".to_string())
    }

    /// Get a single node's position
    ///
    /// Returns null if node not found, otherwise { x, y }
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = getNodePosition))]
    pub fn get_node_position(&self, id: &str) -> JsValue {
        if let Some(node) = self.state.engine.get_node(id) {
            let pos = serde_json::json!({
                "x": node.position.x,
                "y": node.position.y,
            });
            JsValue::from_str(&pos.to_string())
        } else {
            JsValue::NULL
        }
    }

    /// Find node at screen coordinates
    ///
    /// Takes screen coordinates and returns the ID of the node at that position,
    /// or null if no node is found. Uses a hit radius of 30 pixels.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = findNodeAt))]
    pub fn find_node_at(&self, screen_x: f64, screen_y: f64) -> JsValue {
        // Convert screen coordinates to world coordinates
        let world_x = (screen_x / self.state.zoom) + self.state.camera.x;
        let world_y = (screen_y / self.state.zoom) + self.state.camera.y;

        let hit_radius = 30.0;
        let hit_radius_sq = hit_radius * hit_radius;

        for node in &self.state.engine.nodes {
            let dx = node.position.x - world_x;
            let dy = node.position.y - world_y;
            if dx * dx + dy * dy <= hit_radius_sq {
                return JsValue::from_str(&node.id);
            }
        }

        JsValue::NULL
    }

    /// Set a node's position (for dragging)
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = setNodePosition))]
    pub fn set_node_position(&mut self, id: &str, x: f64, y: f64) {
        if let Some(node) = self.state.engine.get_node_mut(id) {
            node.position = Position::new(x, y);
            node.velocity = Position::default();
        }
    }

    /// Fix a node's position (prevent layout from moving it)
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = fixNode))]
    pub fn fix_node(&mut self, id: &str, fixed: bool) {
        if let Some(node) = self.state.engine.get_node_mut(id) {
            node.fixed = fixed;
        }
    }
}

impl Default for BinnacleViewer {
    fn default() -> Self {
        Self::new()
    }
}

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

    #[test]
    fn test_binnacle_viewer_new() {
        let viewer = BinnacleViewer::new();
        assert_eq!(viewer.node_count(), 0);
        assert_eq!(viewer.edge_count(), 0);
        assert!(!viewer.is_layout_ready());
        assert!((viewer.get_zoom() - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_binnacle_viewer_load_from_json() {
        let mut viewer = BinnacleViewer::new();

        let json = r#"{
            "entities": [
                {"id": "bn-1234", "type": "task", "title": "Test Task", "status": "pending", "priority": 2, "tags": ["test"]},
                {"id": "bnq-5678", "type": "queue", "title": "Work Queue", "status": "active", "priority": 0, "tags": []}
            ],
            "edges": [
                {"source": "bn-1234", "target": "bnq-5678", "edge_type": "queued"}
            ]
        }"#;

        viewer.load_from_json(json).unwrap();

        assert_eq!(viewer.node_count(), 2);
        assert_eq!(viewer.edge_count(), 1);
        assert_eq!(viewer.entities.len(), 2);
        assert_eq!(viewer.edge_info.len(), 1);

        // Check entity data was parsed
        assert_eq!(viewer.entities[0].id, "bn-1234");
        assert_eq!(viewer.entities[0].entity_type, "task");
        assert_eq!(viewer.entities[0].title, "Test Task");
        assert_eq!(viewer.entities[0].status, "pending");
        assert_eq!(viewer.entities[0].priority, 2);
        assert_eq!(viewer.entities[0].tags, vec!["test"]);
    }

    #[test]
    fn test_binnacle_viewer_get_nodes_json() {
        let mut viewer = BinnacleViewer::new();

        let json = r#"{
            "entities": [
                {"id": "bn-1234", "type": "task", "title": "Test Task", "status": "pending", "priority": 2, "tags": []}
            ],
            "edges": []
        }"#;

        viewer.load_from_json(json).unwrap();

        let nodes_json = viewer.get_nodes_json();
        let nodes: serde_json::Value = serde_json::from_str(&nodes_json).unwrap();

        assert!(nodes.is_array());
        let arr = nodes.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["id"], "bn-1234");
        assert_eq!(arr[0]["type"], "task");
        assert!(arr[0]["x"].is_f64());
        assert!(arr[0]["y"].is_f64());
    }

    #[test]
    fn test_binnacle_viewer_get_edges_json() {
        let mut viewer = BinnacleViewer::new();

        let json = r#"{
            "entities": [
                {"id": "bn-1234", "type": "task", "title": "Task 1", "status": "pending"},
                {"id": "bn-5678", "type": "task", "title": "Task 2", "status": "pending"}
            ],
            "edges": [
                {"source": "bn-1234", "target": "bn-5678", "edge_type": "depends_on"}
            ]
        }"#;

        viewer.load_from_json(json).unwrap();

        let edges_json = viewer.get_edges_json();
        let edges: serde_json::Value = serde_json::from_str(&edges_json).unwrap();

        assert!(edges.is_array());
        let arr = edges.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["source"], "bn-1234");
        assert_eq!(arr[0]["target"], "bn-5678");
        assert_eq!(arr[0]["edge_type"], "depends_on");
    }

    #[test]
    fn test_binnacle_viewer_run_layout() {
        let mut viewer = BinnacleViewer::new();

        let json = r#"{
            "entities": [
                {"id": "bn-1234", "type": "task", "title": "Task 1", "status": "pending"},
                {"id": "bn-5678", "type": "task", "title": "Task 2", "status": "pending"}
            ],
            "edges": [
                {"source": "bn-1234", "target": "bn-5678", "edge_type": "depends_on"}
            ]
        }"#;

        viewer.load_from_json(json).unwrap();
        assert!(!viewer.is_layout_ready());

        let iterations = viewer.run_layout(100);
        assert!(iterations > 0);
        assert!(viewer.is_layout_ready());
    }

    #[test]
    fn test_binnacle_viewer_camera_controls() {
        let mut viewer = BinnacleViewer::new();

        // Pan
        viewer.pan(100.0, 50.0);
        assert!((viewer.get_camera_x() - 100.0).abs() < 0.0001);
        assert!((viewer.get_camera_y() - 50.0).abs() < 0.0001);

        // Zoom
        viewer.set_zoom(2.0);
        assert!((viewer.get_zoom() - 2.0).abs() < 0.0001);

        // Pan at 2x zoom
        viewer.pan(100.0, 50.0);
        assert!((viewer.get_camera_x() - 150.0).abs() < 0.0001); // 100 + 100/2
        assert!((viewer.get_camera_y() - 75.0).abs() < 0.0001); // 50 + 50/2
    }

    #[test]
    fn test_binnacle_viewer_focus_node() {
        let mut viewer = BinnacleViewer::new();

        let json = r#"{
            "entities": [
                {"id": "bn-1234", "type": "task", "title": "Test Task", "status": "pending"}
            ],
            "edges": []
        }"#;

        viewer.load_from_json(json).unwrap();
        viewer.focus_node("bn-1234");

        // Camera should be at the node's position
        let node_pos = viewer.state.engine.get_node("bn-1234").unwrap().position;
        assert!((viewer.get_camera_x() - node_pos.x).abs() < 0.0001);
        assert!((viewer.get_camera_y() - node_pos.y).abs() < 0.0001);
    }

    #[test]
    fn test_binnacle_viewer_set_node_position() {
        let mut viewer = BinnacleViewer::new();

        let json = r#"{
            "entities": [
                {"id": "bn-1234", "type": "task", "title": "Test Task", "status": "pending"}
            ],
            "edges": []
        }"#;

        viewer.load_from_json(json).unwrap();
        viewer.set_node_position("bn-1234", 500.0, 600.0);

        let node = viewer.state.engine.get_node("bn-1234").unwrap();
        assert!((node.position.x - 500.0).abs() < 0.0001);
        assert!((node.position.y - 600.0).abs() < 0.0001);
    }

    #[test]
    fn test_binnacle_viewer_fix_node() {
        let mut viewer = BinnacleViewer::new();

        let json = r#"{
            "entities": [
                {"id": "bn-1234", "type": "task", "title": "Test Task", "status": "pending"}
            ],
            "edges": []
        }"#;

        viewer.load_from_json(json).unwrap();

        assert!(!viewer.state.engine.get_node("bn-1234").unwrap().fixed);

        viewer.fix_node("bn-1234", true);
        assert!(viewer.state.engine.get_node("bn-1234").unwrap().fixed);

        viewer.fix_node("bn-1234", false);
        assert!(!viewer.state.engine.get_node("bn-1234").unwrap().fixed);
    }

    #[test]
    fn test_version() {
        let v = version();
        assert!(!v.is_empty());
        // Should be semver format
        assert!(v.contains('.'));
    }
}
