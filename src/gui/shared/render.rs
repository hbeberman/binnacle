//! Abstract rendering commands for binnacle GUI
//!
//! This module provides platform-agnostic rendering primitives that can be
//! used to render the graph visualization on different backends (Canvas, SVG, WASM).

use super::layout::Position;
use super::theme;

/// A render command that can be executed on any rendering backend
#[derive(Debug, Clone)]
pub enum RenderCommand {
    /// Draw a filled rectangle
    FillRect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        color: String,
        corner_radius: f64,
    },
    /// Draw a stroked rectangle
    StrokeRect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        color: String,
        line_width: f64,
        corner_radius: f64,
    },
    /// Draw a filled circle
    FillCircle {
        cx: f64,
        cy: f64,
        radius: f64,
        color: String,
    },
    /// Draw a stroked circle
    StrokeCircle {
        cx: f64,
        cy: f64,
        radius: f64,
        color: String,
        line_width: f64,
    },
    /// Draw a line
    Line {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        color: String,
        line_width: f64,
        dashed: bool,
    },
    /// Draw text
    Text {
        x: f64,
        y: f64,
        text: String,
        color: String,
        font_size: f64,
        font_weight: FontWeight,
        align: TextAlign,
        baseline: TextBaseline,
    },
    /// Draw an arrow (line with arrowhead)
    Arrow {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        color: String,
        line_width: f64,
        arrow_size: f64,
        dashed: bool,
    },
    /// Save the current transform state
    Save,
    /// Restore the previous transform state
    Restore,
    /// Translate the coordinate system
    Translate { x: f64, y: f64 },
    /// Scale the coordinate system
    Scale { x: f64, y: f64 },
    /// Clear the canvas
    Clear { color: String },
}

/// Font weight for text rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontWeight {
    #[default]
    Normal,
    Bold,
}

/// Text alignment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// Text baseline
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextBaseline {
    Top,
    Middle,
    Bottom,
    #[default]
    Alphabetic,
}

/// Node dimensions and styling
#[derive(Debug, Clone)]
pub struct NodeStyle {
    pub width: f64,
    pub height: f64,
    pub corner_radius: f64,
    pub border_width: f64,
    pub font_size: f64,
    pub padding: f64,
}

impl Default for NodeStyle {
    fn default() -> Self {
        Self {
            width: 200.0,
            height: 80.0,
            corner_radius: 8.0,
            border_width: 2.0,
            font_size: 14.0,
            padding: 10.0,
        }
    }
}

/// Edge styling
#[derive(Debug, Clone)]
pub struct EdgeStyle {
    pub line_width: f64,
    pub arrow_size: f64,
    pub dashed: bool,
}

impl Default for EdgeStyle {
    fn default() -> Self {
        Self {
            line_width: 2.0,
            arrow_size: 10.0,
            dashed: false,
        }
    }
}

/// Parameters for rendering a node
#[derive(Debug, Clone)]
pub struct RenderNodeParams<'a> {
    /// Node ID
    pub id: &'a str,
    /// Node title
    pub title: &'a str,
    /// Entity type (task, bug, idea, etc.)
    pub entity_type: &'a str,
    /// Node status
    pub status: &'a str,
    /// Document type (for doc nodes)
    pub doc_type: Option<&'a str>,
    /// Node position
    pub position: Position,
    /// Visual style
    pub style: &'a NodeStyle,
    /// Whether the node is selected
    pub selected: bool,
}

/// Generate render commands for a node
pub fn render_node(params: &RenderNodeParams) -> Vec<RenderCommand> {
    let mut commands = Vec::new();

    let color = theme::node_color(params.entity_type, params.status, params.doc_type);
    let x = params.position.x - params.style.width / 2.0;
    let y = params.position.y - params.style.height / 2.0;

    // Draw selection highlight if selected
    if params.selected {
        commands.push(RenderCommand::StrokeRect {
            x: x - 4.0,
            y: y - 4.0,
            width: params.style.width + 8.0,
            height: params.style.height + 8.0,
            color: theme::accent::LIGHT.to_string(),
            line_width: 3.0,
            corner_radius: params.style.corner_radius + 2.0,
        });
    }

    // Draw node background
    commands.push(RenderCommand::FillRect {
        x,
        y,
        width: params.style.width,
        height: params.style.height,
        color: color.to_string(),
        corner_radius: params.style.corner_radius,
    });

    // Draw node border
    commands.push(RenderCommand::StrokeRect {
        x,
        y,
        width: params.style.width,
        height: params.style.height,
        color: theme::border::DEFAULT.to_string(),
        line_width: params.style.border_width,
        corner_radius: params.style.corner_radius,
    });

    // Draw node ID
    commands.push(RenderCommand::Text {
        x: params.position.x,
        y: y + params.style.padding + 10.0,
        text: params.id.to_string(),
        color: theme::text::SECONDARY.to_string(),
        font_size: 11.0,
        font_weight: FontWeight::Normal,
        align: TextAlign::Center,
        baseline: TextBaseline::Top,
    });

    // Draw node title
    commands.push(RenderCommand::Text {
        x: params.position.x,
        y: y + params.style.padding + 28.0,
        text: truncate_text(params.title, 25),
        color: theme::text::PRIMARY.to_string(),
        font_size: params.style.font_size,
        font_weight: FontWeight::Bold,
        align: TextAlign::Center,
        baseline: TextBaseline::Top,
    });

    // Draw entity type badge
    commands.push(RenderCommand::Text {
        x: params.position.x,
        y: y + params.style.height - params.style.padding - 8.0,
        text: params.entity_type.to_uppercase(),
        color: theme::text::SECONDARY.to_string(),
        font_size: 9.0,
        font_weight: FontWeight::Normal,
        align: TextAlign::Center,
        baseline: TextBaseline::Bottom,
    });

    commands
}

/// Generate render commands for an edge
pub fn render_edge(
    source_pos: Position,
    target_pos: Position,
    edge_type: &str,
    style: &EdgeStyle,
) -> Vec<RenderCommand> {
    let color = theme::edge_color(edge_type);

    // Determine if edge should be dashed based on type
    let dashed = matches!(edge_type, "related_to" | "worked_on");

    vec![RenderCommand::Arrow {
        x1: source_pos.x,
        y1: source_pos.y,
        x2: target_pos.x,
        y2: target_pos.y,
        color: color.to_string(),
        line_width: style.line_width,
        arrow_size: style.arrow_size,
        dashed: dashed || style.dashed,
    }]
}

/// Truncate text to a maximum length with ellipsis
fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!("{}…", &text[..max_len.saturating_sub(1)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_node_basic() {
        let style = NodeStyle::default();
        let commands = render_node(&RenderNodeParams {
            id: "bn-1234",
            title: "Test Task",
            entity_type: "task",
            status: "pending",
            doc_type: None,
            position: Position { x: 100.0, y: 100.0 },
            style: &style,
            selected: false,
        });

        // Should have at least: background, border, id text, title text, type badge
        assert!(commands.len() >= 5);
    }

    #[test]
    fn test_render_node_selected() {
        let style = NodeStyle::default();
        let commands = render_node(&RenderNodeParams {
            id: "bn-1234",
            title: "Test Task",
            entity_type: "task",
            status: "pending",
            doc_type: None,
            position: Position { x: 100.0, y: 100.0 },
            style: &style,
            selected: true,
        });

        // Should have selection highlight plus normal elements
        assert!(commands.len() >= 6);
    }

    #[test]
    fn test_render_edge() {
        let commands = render_edge(
            Position { x: 0.0, y: 0.0 },
            Position { x: 100.0, y: 100.0 },
            "depends_on",
            &EdgeStyle::default(),
        );

        assert_eq!(commands.len(), 1);
        match &commands[0] {
            RenderCommand::Arrow { color, .. } => {
                assert_eq!(color, theme::edge::BLOCKING);
            }
            _ => panic!("Expected Arrow command"),
        }
    }

    #[test]
    fn test_truncate_text() {
        assert_eq!(truncate_text("Short", 10), "Short");
        assert_eq!(truncate_text("This is a very long title", 10), "This is a…");
    }

    #[test]
    fn test_edge_dashed_related() {
        let commands = render_edge(
            Position { x: 0.0, y: 0.0 },
            Position { x: 100.0, y: 100.0 },
            "related_to",
            &EdgeStyle::default(),
        );

        match &commands[0] {
            RenderCommand::Arrow { dashed, .. } => {
                assert!(*dashed);
            }
            _ => panic!("Expected Arrow command"),
        }
    }
}
