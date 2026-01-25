//! Graph layout algorithm for binnacle GUI
//!
//! Implements force-directed layout for positioning nodes in the graph visualization.
//! This is a placeholder module - the actual algorithm will be ported from JavaScript.

/// 2D position
#[derive(Debug, Clone, Copy, Default)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

/// Node in the layout graph
#[derive(Debug, Clone)]
pub struct LayoutNode {
    /// Node ID
    pub id: String,
    /// Current position
    pub position: Position,
    /// Velocity for physics simulation
    pub velocity: Position,
    /// Whether this node's position is fixed
    pub fixed: bool,
}

impl LayoutNode {
    /// Create a new layout node at the origin
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            position: Position::default(),
            velocity: Position::default(),
            fixed: false,
        }
    }

    /// Create a new layout node at a specific position
    pub fn with_position(id: impl Into<String>, x: f64, y: f64) -> Self {
        Self {
            id: id.into(),
            position: Position { x, y },
            velocity: Position::default(),
            fixed: false,
        }
    }
}

/// Edge in the layout graph
#[derive(Debug, Clone)]
pub struct LayoutEdge {
    /// Source node ID
    pub source: String,
    /// Target node ID
    pub target: String,
    /// Edge weight (affects spring strength)
    pub weight: f64,
}

impl LayoutEdge {
    /// Create a new layout edge
    pub fn new(source: impl Into<String>, target: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            target: target.into(),
            weight: 1.0,
        }
    }
}

/// Configuration for the force-directed layout algorithm
#[derive(Debug, Clone)]
pub struct LayoutConfig {
    /// Repulsion strength between nodes
    pub repulsion: f64,
    /// Spring strength for edges
    pub spring_strength: f64,
    /// Ideal spring length
    pub spring_length: f64,
    /// Damping factor for velocity
    pub damping: f64,
    /// Maximum velocity
    pub max_velocity: f64,
    /// Convergence threshold
    pub convergence_threshold: f64,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            repulsion: 10000.0,
            spring_strength: 0.1,
            spring_length: 150.0,
            damping: 0.8,
            max_velocity: 50.0,
            convergence_threshold: 0.1,
        }
    }
}

/// Force-directed graph layout engine
///
/// This is a placeholder implementation. The full algorithm will be ported
/// from the JavaScript implementation in index.html.
#[derive(Debug)]
pub struct LayoutEngine {
    /// Layout nodes
    pub nodes: Vec<LayoutNode>,
    /// Layout edges
    pub edges: Vec<LayoutEdge>,
    /// Configuration
    pub config: LayoutConfig,
}

impl LayoutEngine {
    /// Create a new layout engine
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            config: LayoutConfig::default(),
        }
    }

    /// Create a layout engine with custom configuration
    pub fn with_config(config: LayoutConfig) -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            config,
        }
    }

    /// Add a node to the layout
    pub fn add_node(&mut self, node: LayoutNode) {
        self.nodes.push(node);
    }

    /// Add an edge to the layout
    pub fn add_edge(&mut self, edge: LayoutEdge) {
        self.edges.push(edge);
    }

    /// Clear all nodes and edges
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.edges.clear();
    }

    /// Get a node by ID
    pub fn get_node(&self, id: &str) -> Option<&LayoutNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Get a mutable node by ID
    pub fn get_node_mut(&mut self, id: &str) -> Option<&mut LayoutNode> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    /// Run one iteration of the force simulation
    ///
    /// Returns the total kinetic energy (sum of squared velocities).
    /// When this drops below the convergence threshold, the layout is stable.
    pub fn tick(&mut self) -> f64 {
        // TODO: Port force-directed algorithm from JavaScript
        // For now, return 0 to indicate convergence
        0.0
    }

    /// Run the simulation until convergence or max iterations
    pub fn run(&mut self, max_iterations: usize) -> usize {
        for i in 0..max_iterations {
            let energy = self.tick();
            if energy < self.config.convergence_threshold {
                return i + 1;
            }
        }
        max_iterations
    }
}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_node_new() {
        let node = LayoutNode::new("bn-1234");
        assert_eq!(node.id, "bn-1234");
        assert_eq!(node.position.x, 0.0);
        assert_eq!(node.position.y, 0.0);
        assert!(!node.fixed);
    }

    #[test]
    fn test_layout_node_with_position() {
        let node = LayoutNode::with_position("bn-1234", 100.0, 200.0);
        assert_eq!(node.position.x, 100.0);
        assert_eq!(node.position.y, 200.0);
    }

    #[test]
    fn test_layout_edge_new() {
        let edge = LayoutEdge::new("bn-1234", "bn-5678");
        assert_eq!(edge.source, "bn-1234");
        assert_eq!(edge.target, "bn-5678");
        assert_eq!(edge.weight, 1.0);
    }

    #[test]
    fn test_layout_engine_add_nodes() {
        let mut engine = LayoutEngine::new();
        engine.add_node(LayoutNode::new("bn-1234"));
        engine.add_node(LayoutNode::new("bn-5678"));
        assert_eq!(engine.nodes.len(), 2);
    }

    #[test]
    fn test_layout_engine_get_node() {
        let mut engine = LayoutEngine::new();
        engine.add_node(LayoutNode::with_position("bn-1234", 10.0, 20.0));

        let node = engine.get_node("bn-1234").unwrap();
        assert_eq!(node.position.x, 10.0);

        assert!(engine.get_node("nonexistent").is_none());
    }

    #[test]
    fn test_layout_config_default() {
        let config = LayoutConfig::default();
        assert!(config.repulsion > 0.0);
        assert!(config.spring_strength > 0.0);
        assert!(config.spring_length > 0.0);
    }
}
