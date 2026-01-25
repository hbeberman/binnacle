//! Graph layout algorithm for binnacle GUI
//!
//! Implements force-directed layout for positioning nodes in the graph visualization.
//! Ported from the JavaScript implementation in index.html.
//!
//! The algorithm uses:
//! - Circular gravity: pulls all nodes toward a center point
//! - Node repulsion: nodes push each other apart (inverse square law)
//! - Edge attraction: connected nodes are pulled together (spring force)
//! - Velocity damping: prevents oscillation and helps convergence

/// 2D position/vector
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

impl Position {
    /// Create a new position
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Calculate the distance to another position
    pub fn distance(&self, other: &Position) -> f64 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Calculate the squared distance to another position (faster, avoids sqrt)
    pub fn distance_squared(&self, other: &Position) -> f64 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        dx * dx + dy * dy
    }

    /// Calculate the magnitude (length) of this vector
    pub fn magnitude(&self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
}

/// Node type for physics calculation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NodeType {
    #[default]
    Normal,
    /// Queue nodes are heavy anchors that barely move
    Queue,
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
    /// Whether this node's position is fixed (e.g., being dragged)
    pub fixed: bool,
    /// Node type affects physics behavior
    pub node_type: NodeType,
}

impl LayoutNode {
    /// Create a new layout node at the origin
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            position: Position::default(),
            velocity: Position::default(),
            fixed: false,
            node_type: NodeType::Normal,
        }
    }

    /// Create a new layout node at a specific position
    pub fn with_position(id: impl Into<String>, x: f64, y: f64) -> Self {
        Self {
            id: id.into(),
            position: Position { x, y },
            velocity: Position::default(),
            fixed: false,
            node_type: NodeType::Normal,
        }
    }

    /// Set the node type
    pub fn with_type(mut self, node_type: NodeType) -> Self {
        self.node_type = node_type;
        self
    }

    /// Get the current speed (magnitude of velocity)
    pub fn speed(&self) -> f64 {
        self.velocity.magnitude()
    }
}

/// Edge in the layout graph
#[derive(Debug, Clone)]
pub struct LayoutEdge {
    /// Source node ID
    pub source: String,
    /// Target node ID
    pub target: String,
    /// Whether this edge participates in spring physics
    pub spring_enabled: bool,
}

impl LayoutEdge {
    /// Create a new layout edge
    pub fn new(source: impl Into<String>, target: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            target: target.into(),
            spring_enabled: true,
        }
    }

    /// Create an edge with spring disabled
    pub fn without_spring(source: impl Into<String>, target: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            target: target.into(),
            spring_enabled: false,
        }
    }
}

/// Configuration for the force-directed layout algorithm
///
/// Default values match the JavaScript implementation in index.html
#[derive(Debug, Clone)]
pub struct LayoutConfig {
    /// Damping factor for velocity (0-1, higher = slower decay)
    pub damping: f64,
    /// Strength of circular gravity pulling nodes toward center
    pub gravity_strength: f64,
    /// Center point for gravity
    pub gravity_center: Position,
    /// Repulsion strength between nodes (inverse square law)
    pub repulsion_strength: f64,
    /// Spring strength for edge attraction
    pub spring_strength: f64,
    /// Ideal resting length for springs
    pub spring_resting_length: f64,
    /// Maximum velocity for normal nodes
    pub max_velocity: f64,
    /// Velocity threshold below which a node is considered stable
    pub stable_threshold: f64,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        // Values from JavaScript: index.html physics config
        Self {
            damping: 0.92,
            gravity_strength: 0.05,
            gravity_center: Position::default(),
            repulsion_strength: 2500.0,
            spring_strength: 0.08,
            spring_resting_length: 200.0,
            max_velocity: 3.0,
            stable_threshold: 0.1,
        }
    }
}

/// Force-directed graph layout engine
///
/// Implements a physics-based layout algorithm with:
/// - Circular gravity pulling nodes toward a center
/// - Node-node repulsion (inverse square law)
/// - Edge spring attraction
/// - Velocity damping for stability
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

    /// Find index of node by ID
    fn node_index(&self, id: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.id == id)
    }

    /// Apply circular gravity force to all nodes
    ///
    /// Pulls nodes toward the gravity center. Queue nodes have reduced gravity
    /// to act as "heavy anchors" that other nodes cluster around.
    fn apply_circular_gravity(&mut self) {
        let center = self.config.gravity_center;
        let strength = self.config.gravity_strength;

        for node in &mut self.nodes {
            if node.fixed {
                continue;
            }

            let dx = center.x - node.position.x;
            let dy = center.y - node.position.y;
            let distance = (dx * dx + dy * dy).sqrt();

            if distance > 0.0 {
                // Queue nodes are heavy - they barely respond to gravity
                let gravity_multiplier = match node.node_type {
                    NodeType::Queue => 0.1,
                    NodeType::Normal => 1.0,
                };
                let force = strength * gravity_multiplier;
                node.velocity.x += (dx / distance) * force;
                node.velocity.y += (dy / distance) * force;
            }
        }
    }

    /// Apply repulsion forces between all node pairs
    ///
    /// Uses inverse square law: force = repulsion_strength / distance²
    fn apply_repulsion_forces(&mut self) {
        let repulsion = self.config.repulsion_strength;
        let n = self.nodes.len();

        // Collect forces first, then apply (to avoid borrow issues)
        let mut forces: Vec<(f64, f64)> = vec![(0.0, 0.0); n];

        for i in 0..n {
            for j in (i + 1)..n {
                let n1 = &self.nodes[i];
                let n2 = &self.nodes[j];

                let dx = n2.position.x - n1.position.x;
                let dy = n2.position.y - n1.position.y;
                let distance_sq = dx * dx + dy * dy;

                if distance_sq == 0.0 {
                    continue;
                }

                let force = repulsion / distance_sq;
                let distance = distance_sq.sqrt();
                let fx = (dx / distance) * force;
                let fy = (dy / distance) * force;

                // n1 gets pushed away from n2 (negative direction)
                if !self.nodes[i].fixed {
                    forces[i].0 -= fx;
                    forces[i].1 -= fy;
                }
                // n2 gets pushed away from n1 (positive direction)
                if !self.nodes[j].fixed {
                    forces[j].0 += fx;
                    forces[j].1 += fy;
                }
            }
        }

        // Apply collected forces
        for (i, node) in self.nodes.iter_mut().enumerate() {
            node.velocity.x += forces[i].0;
            node.velocity.y += forces[i].1;
        }
    }

    /// Apply spring attraction forces along edges
    ///
    /// Uses Hooke's law: force = spring_strength * (distance - resting_length)
    /// Bidirectional: pulls together if extended, pushes apart if compressed.
    fn apply_edge_attraction(&mut self) {
        let spring_strength = self.config.spring_strength;
        let resting_length = self.config.spring_resting_length;

        // Collect forces to avoid borrow issues
        let mut forces: Vec<(f64, f64)> = vec![(0.0, 0.0); self.nodes.len()];

        for edge in &self.edges {
            if !edge.spring_enabled {
                continue;
            }

            let from_idx = match self.node_index(&edge.source) {
                Some(idx) => idx,
                None => continue,
            };
            let to_idx = match self.node_index(&edge.target) {
                Some(idx) => idx,
                None => continue,
            };

            let from = &self.nodes[from_idx];
            let to = &self.nodes[to_idx];

            // Don't apply if both are fixed
            if from.fixed && to.fixed {
                continue;
            }

            let dx = to.position.x - from.position.x;
            let dy = to.position.y - from.position.y;
            let distance = (dx * dx + dy * dy).sqrt();

            if distance == 0.0 {
                continue;
            }

            // Bidirectional spring: deviation from resting length
            let deviation = distance - resting_length;
            let force = spring_strength * deviation;

            let fx = (dx / distance) * force;
            let fy = (dy / distance) * force;

            // from node: pulled toward to node
            if !from.fixed {
                forces[from_idx].0 += fx;
                forces[from_idx].1 += fy;
            }
            // to node: pulled toward from node
            if !to.fixed {
                forces[to_idx].0 -= fx;
                forces[to_idx].1 -= fy;
            }
        }

        // Apply collected forces
        for (i, node) in self.nodes.iter_mut().enumerate() {
            node.velocity.x += forces[i].0;
            node.velocity.y += forces[i].1;
        }
    }

    /// Update node positions based on velocity
    ///
    /// Applies damping and velocity caps, then updates positions.
    fn update_positions(&mut self) {
        let damping = self.config.damping;
        let max_velocity = self.config.max_velocity;

        for node in &mut self.nodes {
            if node.fixed {
                node.velocity.x = 0.0;
                node.velocity.y = 0.0;
                continue;
            }

            // Apply damping
            node.velocity.x *= damping;
            node.velocity.y *= damping;

            // Cap velocity - queue nodes move very slowly
            let velocity_limit = match node.node_type {
                NodeType::Queue => max_velocity * 0.3,
                NodeType::Normal => max_velocity,
            };

            let speed = node.speed();
            if speed > velocity_limit {
                node.velocity.x = (node.velocity.x / speed) * velocity_limit;
                node.velocity.y = (node.velocity.y / speed) * velocity_limit;
            }

            // Update position
            node.position.x += node.velocity.x;
            node.position.y += node.velocity.y;
        }
    }

    /// Run one iteration of the force simulation
    ///
    /// Returns the maximum speed of any node (for convergence checking).
    pub fn tick(&mut self) -> f64 {
        self.apply_circular_gravity();
        self.apply_repulsion_forces();
        self.apply_edge_attraction();
        self.update_positions();

        // Return max speed for convergence checking
        self.nodes
            .iter()
            .filter(|n| !n.fixed)
            .map(|n| n.speed())
            .fold(0.0, f64::max)
    }

    /// Check if the layout has converged (all nodes below stable threshold)
    pub fn is_stable(&self) -> bool {
        let threshold = self.config.stable_threshold;
        self.nodes
            .iter()
            .filter(|n| !n.fixed)
            .all(|n| n.speed() < threshold)
    }

    /// Run the simulation until convergence or max iterations
    ///
    /// Returns the number of iterations run.
    pub fn run(&mut self, max_iterations: usize) -> usize {
        for i in 0..max_iterations {
            self.tick();
            if self.is_stable() {
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
    fn test_position_new() {
        let pos = Position::new(10.0, 20.0);
        assert_eq!(pos.x, 10.0);
        assert_eq!(pos.y, 20.0);
    }

    #[test]
    fn test_position_distance() {
        let p1 = Position::new(0.0, 0.0);
        let p2 = Position::new(3.0, 4.0);
        assert!((p1.distance(&p2) - 5.0).abs() < 0.0001);
    }

    #[test]
    fn test_position_magnitude() {
        let pos = Position::new(3.0, 4.0);
        assert!((pos.magnitude() - 5.0).abs() < 0.0001);
    }

    #[test]
    fn test_layout_node_new() {
        let node = LayoutNode::new("bn-1234");
        assert_eq!(node.id, "bn-1234");
        assert_eq!(node.position.x, 0.0);
        assert_eq!(node.position.y, 0.0);
        assert!(!node.fixed);
        assert_eq!(node.node_type, NodeType::Normal);
    }

    #[test]
    fn test_layout_node_with_position() {
        let node = LayoutNode::with_position("bn-1234", 100.0, 200.0);
        assert_eq!(node.position.x, 100.0);
        assert_eq!(node.position.y, 200.0);
    }

    #[test]
    fn test_layout_node_with_type() {
        let node = LayoutNode::new("bnq-1234").with_type(NodeType::Queue);
        assert_eq!(node.node_type, NodeType::Queue);
    }

    #[test]
    fn test_layout_node_speed() {
        let mut node = LayoutNode::new("bn-1234");
        node.velocity = Position::new(3.0, 4.0);
        assert!((node.speed() - 5.0).abs() < 0.0001);
    }

    #[test]
    fn test_layout_edge_new() {
        let edge = LayoutEdge::new("bn-1234", "bn-5678");
        assert_eq!(edge.source, "bn-1234");
        assert_eq!(edge.target, "bn-5678");
        assert!(edge.spring_enabled);
    }

    #[test]
    fn test_layout_edge_without_spring() {
        let edge = LayoutEdge::without_spring("bn-1234", "bn-5678");
        assert!(!edge.spring_enabled);
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
        // Values should match JavaScript defaults
        assert!((config.damping - 0.92).abs() < 0.001);
        assert!((config.gravity_strength - 0.05).abs() < 0.001);
        assert!((config.repulsion_strength - 2500.0).abs() < 0.1);
        assert!((config.spring_strength - 0.08).abs() < 0.001);
        assert!((config.spring_resting_length - 200.0).abs() < 0.1);
        assert!((config.max_velocity - 3.0).abs() < 0.001);
    }

    #[test]
    fn test_layout_engine_tick_moves_nodes() {
        let mut engine = LayoutEngine::new();
        // Place two nodes far from center
        engine.add_node(LayoutNode::with_position("bn-1", 1000.0, 0.0));
        engine.add_node(LayoutNode::with_position("bn-2", -1000.0, 0.0));

        let initial_x1 = engine.nodes[0].position.x;
        let initial_x2 = engine.nodes[1].position.x;

        // Run a tick - gravity should pull nodes toward center
        engine.tick();

        // Nodes should have moved toward center
        assert!(engine.nodes[0].position.x < initial_x1);
        assert!(engine.nodes[1].position.x > initial_x2);
    }

    #[test]
    fn test_layout_engine_repulsion() {
        let mut engine = LayoutEngine::new();
        // Place two nodes very close together
        engine.add_node(LayoutNode::with_position("bn-1", 0.0, 0.0));
        engine.add_node(LayoutNode::with_position("bn-2", 10.0, 0.0));

        // Run several ticks
        for _ in 0..10 {
            engine.tick();
        }

        // Nodes should have pushed apart
        let distance = engine.nodes[0].position.distance(&engine.nodes[1].position);
        assert!(
            distance > 10.0,
            "Nodes should repel: distance = {}",
            distance
        );
    }

    #[test]
    fn test_layout_engine_edge_attraction() {
        let mut engine = LayoutEngine::new();
        // Place two connected nodes far apart
        engine.add_node(LayoutNode::with_position("bn-1", 0.0, 0.0));
        engine.add_node(LayoutNode::with_position("bn-2", 500.0, 0.0));
        engine.add_edge(LayoutEdge::new("bn-1", "bn-2"));

        let initial_distance = engine.nodes[0].position.distance(&engine.nodes[1].position);

        // Run several ticks
        for _ in 0..20 {
            engine.tick();
        }

        // Connected nodes should have pulled closer (toward resting length)
        let final_distance = engine.nodes[0].position.distance(&engine.nodes[1].position);
        assert!(
            final_distance < initial_distance,
            "Connected nodes should attract: {} -> {}",
            initial_distance,
            final_distance
        );
    }

    #[test]
    fn test_layout_engine_fixed_node() {
        let mut engine = LayoutEngine::new();
        let mut fixed = LayoutNode::with_position("bn-1", 100.0, 100.0);
        fixed.fixed = true;
        engine.add_node(fixed);
        engine.add_node(LayoutNode::with_position("bn-2", 0.0, 0.0));

        // Run several ticks
        for _ in 0..10 {
            engine.tick();
        }

        // Fixed node should not have moved
        assert_eq!(engine.nodes[0].position.x, 100.0);
        assert_eq!(engine.nodes[0].position.y, 100.0);
    }

    #[test]
    fn test_layout_engine_queue_node_heavy() {
        let mut engine = LayoutEngine::new();
        // Queue node at origin
        engine.add_node(LayoutNode::new("bnq-1").with_type(NodeType::Queue));
        // Normal node at same position
        let mut normal_engine = LayoutEngine::new();
        normal_engine.add_node(LayoutNode::new("bn-1"));

        // Both start at origin, gravity center is at origin - no movement expected
        // Let's place them off-center
        engine.nodes[0].position = Position::new(100.0, 0.0);
        normal_engine.nodes[0].position = Position::new(100.0, 0.0);

        // Run ticks
        engine.tick();
        normal_engine.tick();

        // Queue node should move less than normal node due to reduced gravity
        let queue_speed = engine.nodes[0].speed();
        let normal_speed = normal_engine.nodes[0].speed();
        assert!(
            queue_speed < normal_speed,
            "Queue node should be slower: {} vs {}",
            queue_speed,
            normal_speed
        );
    }

    #[test]
    fn test_layout_engine_convergence() {
        let mut engine = LayoutEngine::new();
        // Place nodes that will naturally find equilibrium
        engine.add_node(LayoutNode::with_position("bn-1", 100.0, 0.0));
        engine.add_node(LayoutNode::with_position("bn-2", -100.0, 0.0));

        // Run until stable
        let iterations = engine.run(1000);

        // Should converge before max iterations
        assert!(
            iterations < 1000,
            "Should converge: {} iterations",
            iterations
        );
        assert!(engine.is_stable());
    }

    #[test]
    fn test_layout_engine_clear() {
        let mut engine = LayoutEngine::new();
        engine.add_node(LayoutNode::new("bn-1"));
        engine.add_edge(LayoutEdge::new("bn-1", "bn-2"));

        engine.clear();

        assert!(engine.nodes.is_empty());
        assert!(engine.edges.is_empty());
    }

    #[test]
    fn test_layout_engine_with_config() {
        let config = LayoutConfig {
            repulsion_strength: 5000.0,
            ..Default::default()
        };
        let engine = LayoutEngine::with_config(config);
        assert!((engine.config.repulsion_strength - 5000.0).abs() < 0.1);
    }
}
