//! Archive loading for WASM viewer
//!
//! This module handles fetching and parsing `.bng` archive files in the browser.
//! The archive format is tar + zstd compression, containing JSONL data files.
//!
//! # Archive Structure
//!
//! A `.bng` archive contains:
//! - `binnacle-export/tasks.jsonl`: All entities (tasks, bugs, ideas, etc.)
//! - `binnacle-export/bugs.jsonl`: Bug entities (separate from tasks)
//! - `binnacle-export/edges.jsonl`: Entity relationships
//! - `binnacle-export/commits.jsonl`: Commit-to-entity links
//! - `binnacle-export/test-results.jsonl`: Test execution history
//! - `binnacle-export/manifest.json`: Archive metadata
//!
//! # Loading Flow
//!
//! 1. Accept compressed bytes (from fetch or file input)
//! 2. Decompress with zstd (using ruzstd for WASM compatibility)
//! 3. Extract tar entries
//! 4. Parse JSONL into graph data

use crate::gui::shared::{LayoutEdge, LayoutEngine, LayoutNode, NodeType};
use std::collections::HashMap;
use std::io::{Cursor, Read};

/// Archive manifest metadata
#[derive(Debug, Clone, Default)]
pub struct ArchiveManifest {
    /// When the archive was exported (RFC 3339 timestamp)
    pub exported_at: Option<String>,
    /// Source repository path
    pub source_repo: Option<String>,
    /// Binnacle version that created the archive
    pub binnacle_version: Option<String>,
}

/// Parsed graph data from a binnacle archive
#[derive(Debug, Default)]
pub struct GraphData {
    /// All entities in the graph
    pub entities: Vec<GraphEntity>,
    /// All edges between entities
    pub edges: Vec<GraphEdge>,
    /// Archive metadata (if available)
    pub manifest: ArchiveManifest,
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

    /// Load graph data from .bng archive bytes
    ///
    /// The archive is a tar file compressed with zstd containing JSONL files.
    /// Uses ruzstd for decompression when compiled for WASM.
    pub fn from_archive_bytes(data: &[u8]) -> Result<Self, ArchiveError> {
        // Decompress zstd
        let decompressed = decompress_zstd(data)?;

        // Extract tar entries
        let files = extract_tar(&decompressed)?;

        // Parse JSONL files into graph data
        parse_archive_files(&files)
    }
}

/// Error type for archive loading
#[derive(Debug)]
pub enum ArchiveError {
    /// Decompression failed
    DecompressError(String),
    /// Tar extraction failed
    TarError(String),
    /// JSON parsing failed
    ParseError(String),
    /// Missing required file
    MissingFile(String),
}

impl std::fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArchiveError::DecompressError(e) => write!(f, "Decompression error: {}", e),
            ArchiveError::TarError(e) => write!(f, "Tar extraction error: {}", e),
            ArchiveError::ParseError(e) => write!(f, "Parse error: {}", e),
            ArchiveError::MissingFile(file) => write!(f, "Missing file: {}", file),
        }
    }
}

impl std::error::Error for ArchiveError {}

/// Decompress zstd data
///
/// Uses ruzstd for WASM builds (pure Rust), native zstd otherwise.
fn decompress_zstd(data: &[u8]) -> Result<Vec<u8>, ArchiveError> {
    #[cfg(target_arch = "wasm32")]
    {
        use ruzstd::streaming_decoder::StreamingDecoder;
        let mut decoder = StreamingDecoder::new(data)
            .map_err(|e| ArchiveError::DecompressError(e.to_string()))?;
        let mut decompressed = Vec::new();
        decoder
            .read_to_end(&mut decompressed)
            .map_err(|e| ArchiveError::DecompressError(e.to_string()))?;
        Ok(decompressed)
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::io::Read;
        let mut decoder =
            zstd::Decoder::new(data).map_err(|e| ArchiveError::DecompressError(e.to_string()))?;
        let mut decompressed = Vec::new();
        decoder
            .read_to_end(&mut decompressed)
            .map_err(|e| ArchiveError::DecompressError(e.to_string()))?;
        Ok(decompressed)
    }
}

/// Extract tar entries into a map of filename -> contents
fn extract_tar(data: &[u8]) -> Result<HashMap<String, Vec<u8>>, ArchiveError> {
    let cursor = Cursor::new(data);
    let mut archive = tar::Archive::new(cursor);

    let mut files = HashMap::new();

    for entry in archive
        .entries()
        .map_err(|e| ArchiveError::TarError(e.to_string()))?
    {
        let mut entry = entry.map_err(|e| ArchiveError::TarError(e.to_string()))?;

        let path = entry
            .path()
            .map_err(|e| ArchiveError::TarError(e.to_string()))?
            .to_string_lossy()
            .to_string();

        // Only extract files we care about (skip directories)
        let entry_type = entry.header().entry_type();
        if entry_type.is_file() || entry_type == tar::EntryType::Regular {
            let mut contents = Vec::new();
            entry
                .read_to_end(&mut contents)
                .map_err(|e| ArchiveError::TarError(e.to_string()))?;

            // Extract just the filename from the path (e.g., "binnacle-export/tasks.jsonl" -> "tasks.jsonl")
            let filename = path.rsplit('/').next().unwrap_or(&path).to_string();
            files.insert(filename, contents);
        }
    }

    Ok(files)
}

/// Parse extracted archive files into GraphData
fn parse_archive_files(files: &HashMap<String, Vec<u8>>) -> Result<GraphData, ArchiveError> {
    let mut graph = GraphData::new();
    let mut entity_ids = std::collections::HashSet::new();

    // Parse manifest.json for metadata
    if let Some(manifest_data) = files.get("manifest.json") {
        let content = String::from_utf8_lossy(manifest_data);
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            graph.manifest = ArchiveManifest {
                exported_at: json
                    .get("exported_at")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                source_repo: json
                    .get("source_repo")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                binnacle_version: json
                    .get("binnacle_version")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            };
        }
    }

    // Parse tasks.jsonl (contains tasks, ideas, milestones, queues, docs, agents)
    if let Some(tasks_data) = files.get("tasks.jsonl") {
        let content = String::from_utf8_lossy(tasks_data);
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Some(entity) = parse_entity_line(line) {
                entity_ids.insert(entity.id.clone());
                graph.entities.push(entity);
            }
        }
    }

    // Parse bugs.jsonl (separate file for bug entities)
    if let Some(bugs_data) = files.get("bugs.jsonl") {
        let content = String::from_utf8_lossy(bugs_data);
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Some(entity) = parse_entity_line(line) {
                entity_ids.insert(entity.id.clone());
                graph.entities.push(entity);
            }
        }
    }

    // Parse edges.jsonl
    if let Some(edges_data) = files.get("edges.jsonl") {
        let content = String::from_utf8_lossy(edges_data);
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Some(edge) = parse_edge_line(line) {
                // Only add edge if both source and target exist
                if entity_ids.contains(&edge.source) && entity_ids.contains(&edge.target) {
                    graph.edges.push(edge);
                }
            }
        }
    }

    Ok(graph)
}

/// Parse a single JSONL line into a GraphEntity
fn parse_entity_line(line: &str) -> Option<GraphEntity> {
    let json: serde_json::Value = serde_json::from_str(line).ok()?;

    let id = json.get("id")?.as_str()?.to_string();
    let entity_type = json
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("task")
        .to_string();
    let title = json
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let short_name = json
        .get("short_name")
        .and_then(|v| v.as_str())
        .map(String::from);
    let status = json
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("pending")
        .to_string();
    let priority = json.get("priority").and_then(|v| v.as_u64()).unwrap_or(2) as u8;
    let tags = json
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let doc_type = json
        .get("doc_type")
        .and_then(|v| v.as_str())
        .map(String::from);

    Some(GraphEntity {
        id,
        entity_type,
        title,
        short_name,
        status,
        priority,
        tags,
        doc_type,
    })
}

/// Parse a single JSONL line into a GraphEdge
fn parse_edge_line(line: &str) -> Option<GraphEdge> {
    let json: serde_json::Value = serde_json::from_str(line).ok()?;

    let source = json.get("source")?.as_str()?.to_string();
    let target = json.get("target")?.as_str()?.to_string();
    let edge_type = json
        .get("edge_type")
        .and_then(|v| v.as_str())
        .unwrap_or("depends_on")
        .to_string();

    Some(GraphEdge {
        source,
        target,
        edge_type,
    })
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

    #[test]
    fn test_parse_entity_line() {
        let line = r#"{"id":"bn-1234","type":"task","title":"Test Task","status":"pending","priority":1,"tags":["test"]}"#;
        let entity = parse_entity_line(line).unwrap();
        assert_eq!(entity.id, "bn-1234");
        assert_eq!(entity.entity_type, "task");
        assert_eq!(entity.title, "Test Task");
        assert_eq!(entity.status, "pending");
        assert_eq!(entity.priority, 1);
        assert_eq!(entity.tags, vec!["test"]);
    }

    #[test]
    fn test_parse_entity_line_with_short_name() {
        let line = r#"{"id":"bn-5678","type":"bug","title":"Bug Title","short_name":"bug fix","status":"in_progress","priority":0}"#;
        let entity = parse_entity_line(line).unwrap();
        assert_eq!(entity.id, "bn-5678");
        assert_eq!(entity.short_name, Some("bug fix".to_string()));
    }

    #[test]
    fn test_parse_entity_line_invalid() {
        let line = "invalid json";
        assert!(parse_entity_line(line).is_none());
    }

    #[test]
    fn test_parse_entity_line_missing_id() {
        let line = r#"{"type":"task","title":"No ID"}"#;
        assert!(parse_entity_line(line).is_none());
    }

    #[test]
    fn test_parse_edge_line() {
        let line = r#"{"source":"bn-1234","target":"bn-5678","edge_type":"depends_on"}"#;
        let edge = parse_edge_line(line).unwrap();
        assert_eq!(edge.source, "bn-1234");
        assert_eq!(edge.target, "bn-5678");
        assert_eq!(edge.edge_type, "depends_on");
    }

    #[test]
    fn test_parse_edge_line_missing_type() {
        let line = r#"{"source":"bn-1234","target":"bn-5678"}"#;
        let edge = parse_edge_line(line).unwrap();
        assert_eq!(edge.edge_type, "depends_on"); // Default value
    }

    #[test]
    fn test_parse_edge_line_invalid() {
        let line = "not json";
        assert!(parse_edge_line(line).is_none());
    }

    #[test]
    fn test_extract_tar() {
        // Create a minimal tar archive in memory
        let mut builder = tar::Builder::new(Vec::new());

        // Add a simple file
        let content = b"test content";
        let mut header = tar::Header::new_gnu();
        header.set_path("test.txt").unwrap();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append(&header, &content[..]).unwrap();

        let tar_data = builder.into_inner().unwrap();
        let files = extract_tar(&tar_data).unwrap();

        assert_eq!(files.len(), 1);
        assert_eq!(files.get("test.txt").unwrap(), b"test content");
    }

    #[test]
    fn test_parse_archive_files() {
        let mut files = HashMap::new();

        // Add tasks.jsonl
        let tasks = r#"{"id":"bn-1234","type":"task","title":"Task 1","status":"pending","priority":2}
{"id":"bn-5678","type":"task","title":"Task 2","status":"done","priority":1}"#;
        files.insert("tasks.jsonl".to_string(), tasks.as_bytes().to_vec());

        // Add edges.jsonl
        let edges = r#"{"source":"bn-1234","target":"bn-5678","edge_type":"depends_on"}"#;
        files.insert("edges.jsonl".to_string(), edges.as_bytes().to_vec());

        let graph = parse_archive_files(&files).unwrap();

        assert_eq!(graph.entities.len(), 2);
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.entities[0].id, "bn-1234");
        assert_eq!(graph.entities[1].id, "bn-5678");
        assert_eq!(graph.edges[0].source, "bn-1234");
        assert_eq!(graph.edges[0].target, "bn-5678");
    }

    #[test]
    fn test_parse_archive_files_with_manifest() {
        let mut files = HashMap::new();

        // Add manifest.json
        let manifest = r#"{
            "version": 1,
            "format": "binnacle-store-v1",
            "exported_at": "2026-01-25T12:00:00Z",
            "source_repo": "/path/to/repo",
            "binnacle_version": "0.1.0"
        }"#;
        files.insert("manifest.json".to_string(), manifest.as_bytes().to_vec());

        // Add tasks.jsonl
        let tasks =
            r#"{"id":"bn-1234","type":"task","title":"Task 1","status":"pending","priority":2}"#;
        files.insert("tasks.jsonl".to_string(), tasks.as_bytes().to_vec());

        let graph = parse_archive_files(&files).unwrap();

        assert_eq!(
            graph.manifest.exported_at,
            Some("2026-01-25T12:00:00Z".to_string())
        );
        assert_eq!(
            graph.manifest.source_repo,
            Some("/path/to/repo".to_string())
        );
        assert_eq!(graph.manifest.binnacle_version, Some("0.1.0".to_string()));
    }

    #[test]
    fn test_parse_archive_files_without_manifest() {
        let mut files = HashMap::new();

        // No manifest.json
        let tasks =
            r#"{"id":"bn-1234","type":"task","title":"Task 1","status":"pending","priority":2}"#;
        files.insert("tasks.jsonl".to_string(), tasks.as_bytes().to_vec());

        let graph = parse_archive_files(&files).unwrap();

        // Manifest should have default (None) values
        assert!(graph.manifest.exported_at.is_none());
        assert!(graph.manifest.source_repo.is_none());
        assert!(graph.manifest.binnacle_version.is_none());
    }

    #[test]
    fn test_parse_archive_files_filters_orphan_edges() {
        let mut files = HashMap::new();

        // Add tasks.jsonl with one task
        let tasks =
            r#"{"id":"bn-1234","type":"task","title":"Task 1","status":"pending","priority":2}"#;
        files.insert("tasks.jsonl".to_string(), tasks.as_bytes().to_vec());

        // Add edge referencing non-existent task
        let edges = r#"{"source":"bn-1234","target":"bn-9999","edge_type":"depends_on"}"#;
        files.insert("edges.jsonl".to_string(), edges.as_bytes().to_vec());

        let graph = parse_archive_files(&files).unwrap();

        assert_eq!(graph.entities.len(), 1);
        assert_eq!(graph.edges.len(), 0); // Edge should be filtered out
    }

    #[test]
    fn test_decompress_zstd() {
        // Create zstd-compressed data
        use std::io::Write;
        let original = b"Hello, World!";
        let mut encoder = zstd::Encoder::new(Vec::new(), 3).unwrap();
        encoder.write_all(original).unwrap();
        let compressed = encoder.finish().unwrap();

        let decompressed = decompress_zstd(&compressed).unwrap();
        assert_eq!(decompressed, original);
    }

    #[test]
    fn test_from_archive_bytes() {
        use std::io::Write;

        // Create a proper .bng archive (tar + zstd)
        let mut tar_builder = tar::Builder::new(Vec::new());

        // Add tasks.jsonl
        let tasks =
            r#"{"id":"bn-1234","type":"task","title":"Test Task","status":"pending","priority":2}"#;
        let mut header = tar::Header::new_gnu();
        header.set_path("binnacle-export/tasks.jsonl").unwrap();
        header.set_size(tasks.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar_builder.append(&header, tasks.as_bytes()).unwrap();

        // Add edges.jsonl (empty)
        let edges = "";
        let mut header = tar::Header::new_gnu();
        header.set_path("binnacle-export/edges.jsonl").unwrap();
        header.set_size(edges.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar_builder.append(&header, edges.as_bytes()).unwrap();

        let tar_data = tar_builder.into_inner().unwrap();

        // Compress with zstd
        let mut encoder = zstd::Encoder::new(Vec::new(), 3).unwrap();
        encoder.write_all(&tar_data).unwrap();
        let compressed = encoder.finish().unwrap();

        // Load the archive
        let graph = GraphData::from_archive_bytes(&compressed).unwrap();

        assert_eq!(graph.entities.len(), 1);
        assert_eq!(graph.entities[0].id, "bn-1234");
        assert_eq!(graph.entities[0].title, "Test Task");
    }

    #[test]
    fn test_archive_error_display() {
        let err = ArchiveError::DecompressError("test error".to_string());
        assert_eq!(err.to_string(), "Decompression error: test error");

        let err = ArchiveError::TarError("tar error".to_string());
        assert_eq!(err.to_string(), "Tar extraction error: tar error");

        let err = ArchiveError::ParseError("parse error".to_string());
        assert_eq!(err.to_string(), "Parse error: parse error");

        let err = ArchiveError::MissingFile("tasks.jsonl".to_string());
        assert_eq!(err.to_string(), "Missing file: tasks.jsonl");
    }
}
