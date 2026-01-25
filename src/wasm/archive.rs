//! Archive loading for WASM viewer
//!
//! This module handles fetching and parsing `.bng` archive files in the browser.
//! The archive format is tar + zstd compression, containing JSONL data files.
//!
//! # Archive Structure
//!
//! A `.bng` archive contains:
//! - `tasks.jsonl`: All entities (tasks, bugs, ideas, etc.)
//! - `commits.jsonl`: Commit-to-entity links
//! - `test-results.jsonl`: Test execution history
//!
//! # Loading Flow
//!
//! 1. Fetch archive from URL (using web_sys fetch)
//! 2. Decompress with zstd
//! 3. Extract tar entries
//! 4. Parse JSONL into graph data
//!
//! See task bn-057f for full implementation.

use crate::gui::shared::{LayoutEdge, LayoutEngine, LayoutNode, NodeType};

/// Parsed graph data from a binnacle archive
#[derive(Debug, Default)]
pub struct GraphData {
    /// All entities in the graph
    pub entities: Vec<GraphEntity>,
    /// All edges between entities
    pub edges: Vec<GraphEdge>,
}

/// A graph entity (task, bug, idea, etc.)
#[derive(Debug, Clone)]
pub struct GraphEntity {
    /// Entity ID (bn-xxxx, bnq-xxxx, etc.)
    pub id: String,
    /// Entity type (task, bug, idea, queue, agent, doc, milestone)
    pub entity_type: String,
    /// Display title
    pub title: String,
    /// Short name for compact display
    pub short_name: Option<String>,
    /// Status (pending, in_progress, done, etc.)
    pub status: String,
    /// Priority (0-4)
    pub priority: u8,
    /// Tags
    pub tags: Vec<String>,
    /// Document type (for doc entities)
    pub doc_type: Option<String>,
}

/// A graph edge (dependency, hierarchy, etc.)
#[derive(Debug, Clone)]
pub struct GraphEdge {
    /// Source entity ID
    pub source: String,
    /// Target entity ID
    pub target: String,
    /// Edge type (depends_on, child_of, queued, etc.)
    pub edge_type: String,
}

impl GraphData {
    /// Create a new empty graph
    pub fn new() -> Self {
        Self::default()
    }

    /// Convert to a layout engine with positioned nodes
    pub fn to_layout_engine(&self) -> LayoutEngine {
        let mut engine = LayoutEngine::new();

        // Add nodes with random initial positions
        for (i, entity) in self.entities.iter().enumerate() {
            // Spread nodes in a circle initially
            let angle = (i as f64) * 2.0 * std::f64::consts::PI / (self.entities.len() as f64);
            let radius = 300.0;
            let x = angle.cos() * radius;
            let y = angle.sin() * radius;

            let node_type = if entity.id.starts_with("bnq-") {
                NodeType::Queue
            } else {
                NodeType::Normal
            };

            engine.add_node(LayoutNode::with_position(&entity.id, x, y).with_type(node_type));
        }

        // Add edges
        for edge in &self.edges {
            // Disable spring for hierarchy edges to avoid visual clutter
            let layout_edge = if edge.edge_type == "child_of" || edge.edge_type == "parent_of" {
                LayoutEdge::without_spring(&edge.source, &edge.target)
            } else {
                LayoutEdge::new(&edge.source, &edge.target)
            };
            engine.add_edge(layout_edge);
        }

        engine
    }

    /// Get entity by ID
    pub fn get_entity(&self, id: &str) -> Option<&GraphEntity> {
        self.entities.iter().find(|e| e.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_data_new() {
        let graph = GraphData::new();
        assert!(graph.entities.is_empty());
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn test_graph_data_to_layout_engine() {
        let mut graph = GraphData::new();
        graph.entities.push(GraphEntity {
            id: "bn-1234".to_string(),
            entity_type: "task".to_string(),
            title: "Test Task".to_string(),
            short_name: None,
            status: "pending".to_string(),
            priority: 2,
            tags: vec![],
            doc_type: None,
        });
        graph.entities.push(GraphEntity {
            id: "bnq-5678".to_string(),
            entity_type: "queue".to_string(),
            title: "Work Queue".to_string(),
            short_name: None,
            status: "pending".to_string(),
            priority: 2,
            tags: vec![],
            doc_type: None,
        });
        graph.edges.push(GraphEdge {
            source: "bn-1234".to_string(),
            target: "bnq-5678".to_string(),
            edge_type: "queued".to_string(),
        });

        let engine = graph.to_layout_engine();
        assert_eq!(engine.nodes.len(), 2);
        assert_eq!(engine.edges.len(), 1);

        // Check queue node has correct type
        let queue_node = engine.get_node("bnq-5678").unwrap();
        assert_eq!(queue_node.node_type, NodeType::Queue);
    }

    #[test]
    fn test_graph_data_get_entity() {
        let mut graph = GraphData::new();
        graph.entities.push(GraphEntity {
            id: "bn-1234".to_string(),
            entity_type: "task".to_string(),
            title: "Test Task".to_string(),
            short_name: None,
            status: "pending".to_string(),
            priority: 2,
            tags: vec![],
            doc_type: None,
        });

        assert!(graph.get_entity("bn-1234").is_some());
        assert!(graph.get_entity("bn-9999").is_none());
    }
}
