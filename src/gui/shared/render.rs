//! Abstract rendering commands for binnacle GUI
//!
//! This module provides platform-agnostic rendering primitives that can be
//! used to render the graph visualization on different backends (Canvas, SVG, WASM).
//!
//! # Node Shapes
//!
//! Different entity types use different shapes:
//! - **Task/Milestone**: Circle
//! - **Bug**: Rounded square
//! - **Idea**: Cloud shape
//! - **Queue**: Hexagon
//! - **Agent**: Person silhouette
//! - **Doc**: Document shape with folded corner

use super::layout::Position;
use super::theme;
use std::f64::consts::PI;

/// A render command that can be executed on any rendering backend
#[derive(Debug, Clone, PartialEq)]
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
        dashed: bool,
    },
    /// Draw a filled path (for complex shapes like hexagons, clouds)
    FillPath {
        points: Vec<PathPoint>,
        color: String,
    },
    /// Draw a stroked path
    StrokePath {
        points: Vec<PathPoint>,
        color: String,
        line_width: f64,
        dashed: bool,
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
    /// Draw an arc (partial circle)
    Arc {
        cx: f64,
        cy: f64,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
        color: String,
        line_width: f64,
    },
    /// Save the current transform state
    Save,
    /// Restore the previous transform state
    Restore,
    /// Translate the coordinate system
    Translate { x: f64, y: f64 },
    /// Scale the coordinate system
    Scale { x: f64, y: f64 },
    /// Rotate the coordinate system
    Rotate { angle: f64 },
    /// Set global alpha (opacity)
    SetAlpha { alpha: f64 },
    /// Clear the canvas
    Clear { color: String },
}

/// A point in a path, which can be a line or a curve
#[derive(Debug, Clone, PartialEq)]
pub enum PathPoint {
    /// Move to a point without drawing
    MoveTo { x: f64, y: f64 },
    /// Draw a line to a point
    LineTo { x: f64, y: f64 },
    /// Draw a quadratic bezier curve
    QuadraticTo { cx: f64, cy: f64, x: f64, y: f64 },
    /// Draw an arc
    ArcTo {
        cx: f64,
        cy: f64,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
    },
    /// Close the path
    Close,
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

/// Node shape type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NodeShape {
    /// Circle (used for tasks, milestones)
    #[default]
    Circle,
    /// Hexagon (used for queues)
    Hexagon,
    /// Rounded square (used for bugs)
    Square,
    /// Cloud shape (used for ideas)
    Cloud,
    /// Person silhouette (used for agents)
    Person,
    /// Document with folded corner (used for docs)
    Document,
}

impl NodeShape {
    /// Get the shape for a given entity type
    pub fn for_entity_type(entity_type: &str) -> Self {
        match entity_type {
            "queue" => NodeShape::Hexagon,
            "bug" => NodeShape::Square,
            "idea" => NodeShape::Cloud,
            "agent" => NodeShape::Person,
            "doc" => NodeShape::Document,
            _ => NodeShape::Circle, // task, milestone, test
        }
    }
}

/// Node visual state flags
#[derive(Debug, Clone, Default)]
pub struct NodeState {
    /// Node is currently selected
    pub selected: bool,
    /// Node is being hovered
    pub hovered: bool,
    /// Node is being dragged
    pub dragging: bool,
    /// Node is dimmed (filtered out or doesn't match search)
    pub dimmed: bool,
    /// Node is in the work queue
    pub queued: bool,
    /// Node is an end goal (no dependants)
    pub end_goal: bool,
    /// Node is in_progress (for active animation)
    pub in_progress: bool,
    /// Animation time in milliseconds (for animated effects)
    pub animation_time: f64,
}

/// Node dimensions and styling
#[derive(Debug, Clone)]
pub struct NodeStyle {
    /// Node radius (for circular/shape-based nodes)
    pub radius: f64,
    /// Line width for borders
    pub border_width: f64,
    /// Base font size
    pub font_size: f64,
    /// Zoom factor
    pub zoom: f64,
}

impl Default for NodeStyle {
    fn default() -> Self {
        Self {
            radius: 30.0,
            border_width: 2.0,
            font_size: 12.0,
            zoom: 1.0,
        }
    }
}

/// Edge styling
#[derive(Debug, Clone)]
pub struct EdgeStyle {
    pub line_width: f64,
    pub arrow_size: f64,
    pub dashed: bool,
    /// Whether this edge has a marching animation
    pub animated: bool,
}

impl Default for EdgeStyle {
    fn default() -> Self {
        Self {
            line_width: 2.0,
            arrow_size: 10.0,
            dashed: false,
            animated: false,
        }
    }
}

/// Get edge style based on edge type
pub fn edge_style_for_type(edge_type: &str) -> EdgeStyle {
    match edge_type {
        "depends_on" | "blocks" => EdgeStyle {
            line_width: 2.0,
            arrow_size: 10.0,
            dashed: false,
            animated: false,
        },
        "related_to" | "caused_by" | "duplicates" | "supersedes" => EdgeStyle {
            line_width: 1.5,
            arrow_size: 8.0,
            dashed: true,
            animated: false,
        },
        "fixes" | "tests" => EdgeStyle {
            line_width: 2.0,
            arrow_size: 10.0,
            dashed: false,
            animated: false,
        },
        "parent_of" | "child_of" => EdgeStyle {
            line_width: 2.0,
            arrow_size: 10.0,
            dashed: false,
            animated: false,
        },
        "queued" => EdgeStyle {
            line_width: 2.0,
            arrow_size: 10.0,
            dashed: true,
            animated: false,
        },
        "working_on" => EdgeStyle {
            line_width: 3.0,
            arrow_size: 12.0,
            dashed: true,
            animated: true,
        },
        "worked_on" => EdgeStyle {
            line_width: 2.0,
            arrow_size: 10.0,
            dashed: false,
            animated: false,
        },
        "pinned" => EdgeStyle {
            line_width: 3.0,
            arrow_size: 12.0,
            dashed: false,
            animated: false,
        },
        "documents" | "attached_to" => EdgeStyle {
            line_width: 2.0,
            arrow_size: 10.0,
            dashed: true,
            animated: false,
        },
        "impacts" => EdgeStyle {
            line_width: 2.0,
            arrow_size: 10.0,
            dashed: true,
            animated: false,
        },
        _ => EdgeStyle::default(),
    }
}

/// Parameters for rendering a node
#[derive(Debug, Clone)]
pub struct RenderNodeParams<'a> {
    /// Node ID
    pub id: &'a str,
    /// Node title or short_name
    pub title: &'a str,
    /// Optional short name for display
    pub short_name: Option<&'a str>,
    /// Entity type (task, bug, idea, etc.)
    pub entity_type: &'a str,
    /// Node status
    pub status: &'a str,
    /// Document type (for doc nodes)
    pub doc_type: Option<&'a str>,
    /// Node position (screen coordinates)
    pub position: Position,
    /// Visual style
    pub style: &'a NodeStyle,
    /// Visual state flags
    pub state: &'a NodeState,
}

/// Generate path points for a hexagon shape
pub fn hexagon_path(cx: f64, cy: f64, radius: f64) -> Vec<PathPoint> {
    let mut points = Vec::with_capacity(7);
    for i in 0..6 {
        let angle = (i as f64) * PI / 3.0 - PI / 6.0;
        let x = cx + radius * angle.cos();
        let y = cy + radius * angle.sin();
        if i == 0 {
            points.push(PathPoint::MoveTo { x, y });
        } else {
            points.push(PathPoint::LineTo { x, y });
        }
    }
    points.push(PathPoint::Close);
    points
}

/// Generate path points for a rounded square shape
pub fn square_path(cx: f64, cy: f64, radius: f64) -> Vec<PathPoint> {
    let size = radius * 1.6;
    let half = size / 2.0;
    let corner = size * 0.15;

    vec![
        PathPoint::MoveTo {
            x: cx - half + corner,
            y: cy - half,
        },
        PathPoint::LineTo {
            x: cx + half - corner,
            y: cy - half,
        },
        PathPoint::QuadraticTo {
            cx: cx + half,
            cy: cy - half,
            x: cx + half,
            y: cy - half + corner,
        },
        PathPoint::LineTo {
            x: cx + half,
            y: cy + half - corner,
        },
        PathPoint::QuadraticTo {
            cx: cx + half,
            cy: cy + half,
            x: cx + half - corner,
            y: cy + half,
        },
        PathPoint::LineTo {
            x: cx - half + corner,
            y: cy + half,
        },
        PathPoint::QuadraticTo {
            cx: cx - half,
            cy: cy + half,
            x: cx - half,
            y: cy + half - corner,
        },
        PathPoint::LineTo {
            x: cx - half,
            y: cy - half + corner,
        },
        PathPoint::QuadraticTo {
            cx: cx - half,
            cy: cy - half,
            x: cx - half + corner,
            y: cy - half,
        },
        PathPoint::Close,
    ]
}

/// Generate path points for a cloud shape (for ideas)
pub fn cloud_path(cx: f64, cy: f64, radius: f64) -> Vec<PathPoint> {
    // Cloud is approximated with multiple overlapping circles
    // We create a bumpy outline using arcs
    let r = radius * 0.9;
    vec![
        // Start at bottom left
        PathPoint::MoveTo {
            x: cx - r * 0.8,
            y: cy + r * 0.3,
        },
        // Bottom arc
        PathPoint::QuadraticTo {
            cx: cx - r * 0.4,
            cy: cy + r * 0.7,
            x: cx,
            y: cy + r * 0.5,
        },
        PathPoint::QuadraticTo {
            cx: cx + r * 0.4,
            cy: cy + r * 0.7,
            x: cx + r * 0.8,
            y: cy + r * 0.3,
        },
        // Right bump
        PathPoint::QuadraticTo {
            cx: cx + r * 1.1,
            cy,
            x: cx + r * 0.9,
            y: cy - r * 0.3,
        },
        // Top bumps
        PathPoint::QuadraticTo {
            cx: cx + r * 0.7,
            cy: cy - r * 0.8,
            x: cx + r * 0.2,
            y: cy - r * 0.6,
        },
        PathPoint::QuadraticTo {
            cx,
            cy: cy - r * 0.9,
            x: cx - r * 0.3,
            y: cy - r * 0.6,
        },
        PathPoint::QuadraticTo {
            cx: cx - r * 0.8,
            cy: cy - r * 0.7,
            x: cx - r * 0.9,
            y: cy - r * 0.2,
        },
        // Left side back to start
        PathPoint::QuadraticTo {
            cx: cx - r * 1.1,
            cy: cy + r * 0.1,
            x: cx - r * 0.8,
            y: cy + r * 0.3,
        },
        PathPoint::Close,
    ]
}

/// Generate path points for a person silhouette (for agents)
pub fn person_path(cx: f64, cy: f64, radius: f64) -> Vec<PathPoint> {
    let r = radius;
    let head_r = r * 0.35;
    let head_cy = cy - r * 0.35;

    vec![
        // Head (circle approximated with bezier)
        PathPoint::MoveTo {
            x: cx + head_r,
            y: head_cy,
        },
        PathPoint::ArcTo {
            cx,
            cy: head_cy,
            radius: head_r,
            start_angle: 0.0,
            end_angle: PI * 2.0,
        },
        // Body (shoulders and torso)
        PathPoint::MoveTo {
            x: cx - r * 0.6,
            y: cy + r * 0.1,
        },
        // Shoulders
        PathPoint::QuadraticTo {
            cx: cx - r * 0.7,
            cy: cy - r * 0.1,
            x: cx - r * 0.3,
            y: cy - r * 0.05,
        },
        // Neck
        PathPoint::LineTo {
            x: cx - r * 0.15,
            y: cy - r * 0.05,
        },
        PathPoint::LineTo {
            x: cx + r * 0.15,
            y: cy - r * 0.05,
        },
        // Right shoulder
        PathPoint::QuadraticTo {
            cx: cx + r * 0.7,
            cy: cy - r * 0.1,
            x: cx + r * 0.6,
            y: cy + r * 0.1,
        },
        // Torso
        PathPoint::QuadraticTo {
            cx: cx + r * 0.5,
            cy: cy + r * 0.7,
            x: cx,
            y: cy + r * 0.7,
        },
        PathPoint::QuadraticTo {
            cx: cx - r * 0.5,
            cy: cy + r * 0.7,
            x: cx - r * 0.6,
            y: cy + r * 0.1,
        },
        PathPoint::Close,
    ]
}

/// Generate path points for a document shape (for docs)
pub fn document_path(cx: f64, cy: f64, radius: f64) -> Vec<PathPoint> {
    let w = radius * 1.4;
    let h = radius * 1.6;
    let fold = radius * 0.35;

    vec![
        PathPoint::MoveTo {
            x: cx - w / 2.0,
            y: cy - h / 2.0,
        },
        // Top edge (stopping before fold)
        PathPoint::LineTo {
            x: cx + w / 2.0 - fold,
            y: cy - h / 2.0,
        },
        // Fold diagonal
        PathPoint::LineTo {
            x: cx + w / 2.0,
            y: cy - h / 2.0 + fold,
        },
        // Right edge
        PathPoint::LineTo {
            x: cx + w / 2.0,
            y: cy + h / 2.0,
        },
        // Bottom edge
        PathPoint::LineTo {
            x: cx - w / 2.0,
            y: cy + h / 2.0,
        },
        // Left edge back to start
        PathPoint::Close,
    ]
}

/// Get the path for a node shape
pub fn node_shape_path(shape: NodeShape, cx: f64, cy: f64, radius: f64) -> Vec<PathPoint> {
    match shape {
        NodeShape::Hexagon => hexagon_path(cx, cy, radius),
        NodeShape::Square => square_path(cx, cy, radius),
        NodeShape::Cloud => cloud_path(cx, cy, radius),
        NodeShape::Person => person_path(cx, cy, radius),
        NodeShape::Document => document_path(cx, cy, radius),
        NodeShape::Circle => {
            // Circle approximated with a single arc
            vec![PathPoint::ArcTo {
                cx,
                cy,
                radius,
                start_angle: 0.0,
                end_angle: PI * 2.0,
            }]
        }
    }
}

/// Generate render commands for a node
pub fn render_node(params: &RenderNodeParams) -> Vec<RenderCommand> {
    let mut commands = Vec::new();
    let zoom = params.style.zoom;
    let radius = params.style.radius * zoom;
    let x = params.position.x;
    let y = params.position.y;

    let color = theme::node_color(params.entity_type, params.status, params.doc_type);
    let shape = NodeShape::for_entity_type(params.entity_type);

    // Set opacity if dimmed
    if params.state.dimmed {
        commands.push(RenderCommand::SetAlpha { alpha: 0.3 });
    }

    // Draw selection highlight
    if params.state.selected {
        let highlight_radius = radius + 10.0 * zoom;
        if shape == NodeShape::Circle {
            commands.push(RenderCommand::FillCircle {
                cx: x,
                cy: y,
                radius: highlight_radius,
                color: "rgba(240, 173, 78, 0.15)".to_string(),
            });
            commands.push(RenderCommand::StrokeCircle {
                cx: x,
                cy: y,
                radius: highlight_radius,
                color: "#f0ad4e".to_string(),
                line_width: 4.0,
                dashed: false,
            });
        } else {
            let path = node_shape_path(shape, x, y, highlight_radius);
            commands.push(RenderCommand::FillPath {
                points: path.clone(),
                color: "rgba(240, 173, 78, 0.15)".to_string(),
            });
            commands.push(RenderCommand::StrokePath {
                points: path,
                color: "#f0ad4e".to_string(),
                line_width: 4.0,
                dashed: false,
            });
        }
    }

    // Draw drag highlight
    if params.state.dragging {
        let drag_radius = radius + 8.0 * zoom;
        if shape == NodeShape::Circle {
            commands.push(RenderCommand::FillCircle {
                cx: x,
                cy: y,
                radius: drag_radius,
                color: "rgba(74, 144, 226, 0.3)".to_string(),
            });
            commands.push(RenderCommand::StrokeCircle {
                cx: x,
                cy: y,
                radius: drag_radius,
                color: "#4a90e2".to_string(),
                line_width: 3.0,
                dashed: false,
            });
        } else {
            let path = node_shape_path(shape, x, y, drag_radius);
            commands.push(RenderCommand::FillPath {
                points: path.clone(),
                color: "rgba(74, 144, 226, 0.3)".to_string(),
            });
            commands.push(RenderCommand::StrokePath {
                points: path,
                color: "#4a90e2".to_string(),
                line_width: 3.0,
                dashed: false,
            });
        }
    } else if params.state.hovered {
        // Draw hover highlight
        let hover_radius = radius + 8.0 * zoom;
        if shape == NodeShape::Circle {
            commands.push(RenderCommand::FillCircle {
                cx: x,
                cy: y,
                radius: hover_radius,
                color: "rgba(74, 144, 226, 0.2)".to_string(),
            });
            commands.push(RenderCommand::StrokeCircle {
                cx: x,
                cy: y,
                radius: hover_radius,
                color: "#6aa8f0".to_string(),
                line_width: 3.0,
                dashed: false,
            });
        } else {
            let path = node_shape_path(shape, x, y, hover_radius);
            commands.push(RenderCommand::FillPath {
                points: path.clone(),
                color: "rgba(74, 144, 226, 0.2)".to_string(),
            });
            commands.push(RenderCommand::StrokePath {
                points: path,
                color: "#6aa8f0".to_string(),
                line_width: 3.0,
                dashed: false,
            });
        }
    }

    // Draw queued indicator (teal glow)
    if params.state.queued
        && params.entity_type != "queue"
        && params.entity_type != "agent"
        && params.entity_type != "doc"
    {
        let queued_radius = radius + 6.0 * zoom;
        if shape == NodeShape::Circle {
            commands.push(RenderCommand::FillCircle {
                cx: x,
                cy: y,
                radius: queued_radius - 2.0 * zoom,
                color: "rgba(32, 178, 170, 0.15)".to_string(),
            });
            commands.push(RenderCommand::StrokeCircle {
                cx: x,
                cy: y,
                radius: queued_radius,
                color: theme::queue::COLOR.to_string(),
                line_width: 3.0,
                dashed: false,
            });
        } else {
            let path = node_shape_path(shape, x, y, queued_radius);
            commands.push(RenderCommand::FillPath {
                points: node_shape_path(shape, x, y, queued_radius - 2.0 * zoom),
                color: "rgba(32, 178, 170, 0.15)".to_string(),
            });
            commands.push(RenderCommand::StrokePath {
                points: path,
                color: theme::queue::COLOR.to_string(),
                line_width: 3.0,
                dashed: false,
            });
        }
    }

    // Draw end goal indicator (golden rays)
    if params.state.end_goal && (params.entity_type == "task" || params.entity_type == "bug") {
        commands.extend(render_end_goal_rays(x, y, radius, zoom));
    }

    // Draw dotted yellow border for tasks (not in_progress)
    if params.entity_type == "task" && !params.state.in_progress {
        commands.push(RenderCommand::StrokeCircle {
            cx: x,
            cy: y,
            radius: radius + 4.0 * zoom,
            color: "rgba(255, 215, 0, 0.7)".to_string(),
            line_width: 2.0 * zoom,
            dashed: true,
        });
    }

    // Draw in_progress animation rings
    if params.state.in_progress
        && params.entity_type != "queue"
        && params.entity_type != "agent"
        && params.entity_type != "doc"
    {
        commands.extend(render_in_progress_rings(
            x,
            y,
            radius,
            zoom,
            params.state.animation_time,
        ));
    }

    // Draw the main node shape
    let border_color = if params.state.hovered || params.state.dragging {
        "#ffffff"
    } else {
        theme::text::PRIMARY
    };
    let border_width = if params.state.hovered || params.state.dragging {
        3.0
    } else {
        params.style.border_width
    };

    if shape == NodeShape::Circle {
        commands.push(RenderCommand::FillCircle {
            cx: x,
            cy: y,
            radius,
            color: color.to_string(),
        });
        commands.push(RenderCommand::StrokeCircle {
            cx: x,
            cy: y,
            radius,
            color: border_color.to_string(),
            line_width: border_width,
            dashed: false,
        });
    } else {
        let path = node_shape_path(shape, x, y, radius);
        commands.push(RenderCommand::FillPath {
            points: path.clone(),
            color: color.to_string(),
        });
        commands.push(RenderCommand::StrokePath {
            points: path,
            color: border_color.to_string(),
            line_width: border_width,
            dashed: false,
        });
    }

    // Draw node text (skip for agents)
    if params.entity_type != "agent" {
        let base_font_size = params.style.font_size * zoom.clamp(0.8, 1.5);
        let small_font_size = base_font_size * 0.75;

        let display_text = params.short_name.unwrap_or(params.id);
        let lines = wrap_text(display_text, 8, 2);
        let line_height = base_font_size * 1.2;
        let total_lines = lines.len() + if params.short_name.is_some() { 1 } else { 0 };
        let total_height = (total_lines as f64 - 1.0) * line_height;
        let start_y = y - total_height / 2.0;

        // Draw text lines
        for (i, line) in lines.iter().enumerate() {
            commands.push(RenderCommand::Text {
                x,
                y: start_y + i as f64 * line_height,
                text: line.clone(),
                color: theme::background::PRIMARY.to_string(),
                font_size: base_font_size,
                font_weight: if params.state.hovered || params.state.dragging {
                    FontWeight::Bold
                } else {
                    FontWeight::Normal
                },
                align: TextAlign::Center,
                baseline: TextBaseline::Middle,
            });
        }

        // Draw ID below if showing short_name
        if params.short_name.is_some() {
            commands.push(RenderCommand::Text {
                x,
                y: start_y + lines.len() as f64 * line_height,
                text: params.id.to_string(),
                color: "rgba(26, 35, 50, 0.7)".to_string(),
                font_size: small_font_size,
                font_weight: FontWeight::Normal,
                align: TextAlign::Center,
                baseline: TextBaseline::Middle,
            });
        }
    }

    // Reset alpha if we changed it
    if params.state.dimmed {
        commands.push(RenderCommand::SetAlpha { alpha: 1.0 });
    }

    commands
}

/// Render end goal golden rays
fn render_end_goal_rays(cx: f64, cy: f64, radius: f64, zoom: f64) -> Vec<RenderCommand> {
    let mut commands = Vec::new();
    let ray_count = 8;
    let inner_radius = radius + 8.0 * zoom;
    let outer_radius = radius + 16.0 * zoom;

    for i in 0..ray_count {
        let angle = (i as f64 / ray_count as f64) * PI * 2.0 - PI / 2.0;
        let x1 = cx + angle.cos() * inner_radius;
        let y1 = cy + angle.sin() * inner_radius;
        let x2 = cx + angle.cos() * outer_radius;
        let y2 = cy + angle.sin() * outer_radius;

        commands.push(RenderCommand::Line {
            x1,
            y1,
            x2,
            y2,
            color: "#fbbf24".to_string(),
            line_width: 2.0 * zoom,
            dashed: false,
        });
    }

    // Subtle golden glow circle
    commands.push(RenderCommand::StrokeCircle {
        cx,
        cy,
        radius: radius + 10.0 * zoom,
        color: "rgba(251, 191, 36, 0.4)".to_string(),
        line_width: 3.0 * zoom,
        dashed: false,
    });

    commands
}

/// Render in_progress animated rings (counter-rotating hatched rings)
fn render_in_progress_rings(
    cx: f64,
    cy: f64,
    radius: f64,
    zoom: f64,
    animation_time: f64,
) -> Vec<RenderCommand> {
    let mut commands = Vec::new();
    let rotation_speed = 0.001; // radians per ms
    let outer_ring_radius = radius + 14.0 * zoom;
    let inner_ring_radius = radius + 8.0 * zoom;
    let ring_width = 2.5 * zoom;
    let hatch_count = 12;
    let hatch_length = PI / 18.0;

    // Outer ring - rotates clockwise
    let outer_offset = animation_time * rotation_speed;
    for i in 0..hatch_count {
        let start_angle = (i as f64 * PI * 2.0) / hatch_count as f64 + outer_offset;
        commands.push(RenderCommand::Arc {
            cx,
            cy,
            radius: outer_ring_radius,
            start_angle,
            end_angle: start_angle + hatch_length,
            color: "rgba(240, 173, 78, 0.8)".to_string(),
            line_width: ring_width,
        });
    }

    // Inner ring - rotates counter-clockwise (faster)
    let inner_offset = -animation_time * rotation_speed * 1.5;
    for i in 0..hatch_count {
        let start_angle =
            (i as f64 * PI * 2.0) / hatch_count as f64 + PI / hatch_count as f64 + inner_offset;
        commands.push(RenderCommand::Arc {
            cx,
            cy,
            radius: inner_ring_radius,
            start_angle,
            end_angle: start_angle + hatch_length * 0.8,
            color: "rgba(255, 200, 100, 0.6)".to_string(),
            line_width: ring_width * 0.8,
        });
    }

    commands
}

/// Wrap text into lines of max_chars, up to max_lines
fn wrap_text(text: &str, max_chars: usize, max_lines: usize) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut lines = Vec::new();
    let mut current_line = String::new();

    for word in words {
        if current_line.is_empty() {
            current_line = word.to_string();
        } else if current_line.len() + 1 + word.len() <= max_chars {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            lines.push(current_line);
            current_line = word.to_string();
            if lines.len() >= max_lines {
                break;
            }
        }
    }

    if !current_line.is_empty() && lines.len() < max_lines {
        lines.push(current_line);
    }

    // Truncate last line if needed
    if let Some(last) = lines.last_mut()
        && last.len() > max_chars + 3
    {
        *last = format!("{}…", &last[..max_chars]);
    }

    if lines.is_empty() {
        lines.push(truncate_text(text, max_chars));
    }

    lines
}

/// Generate render commands for an edge
pub fn render_edge(
    source_pos: Position,
    target_pos: Position,
    edge_type: &str,
    style: &EdgeStyle,
) -> Vec<RenderCommand> {
    let color = theme::edge_color(edge_type);

    // Determine if edge should be dashed based on type or style override
    let dashed = style.dashed
        || matches!(
            edge_type,
            "related_to" | "worked_on" | "queued" | "documents" | "attached_to" | "impacts"
        );

    vec![RenderCommand::Arrow {
        x1: source_pos.x,
        y1: source_pos.y,
        x2: target_pos.x,
        y2: target_pos.y,
        color: color.to_string(),
        line_width: style.line_width,
        arrow_size: style.arrow_size,
        dashed,
    }]
}

/// Render an edge with automatic styling based on edge type
pub fn render_edge_auto(
    source_pos: Position,
    target_pos: Position,
    edge_type: &str,
) -> Vec<RenderCommand> {
    let style = edge_style_for_type(edge_type);
    render_edge(source_pos, target_pos, edge_type, &style)
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
    fn test_node_shape_for_entity_type() {
        assert_eq!(NodeShape::for_entity_type("task"), NodeShape::Circle);
        assert_eq!(NodeShape::for_entity_type("milestone"), NodeShape::Circle);
        assert_eq!(NodeShape::for_entity_type("bug"), NodeShape::Square);
        assert_eq!(NodeShape::for_entity_type("idea"), NodeShape::Cloud);
        assert_eq!(NodeShape::for_entity_type("queue"), NodeShape::Hexagon);
        assert_eq!(NodeShape::for_entity_type("agent"), NodeShape::Person);
        assert_eq!(NodeShape::for_entity_type("doc"), NodeShape::Document);
    }

    #[test]
    fn test_hexagon_path() {
        let path = hexagon_path(100.0, 100.0, 30.0);
        assert_eq!(path.len(), 7); // 6 points + close
        assert!(matches!(path[0], PathPoint::MoveTo { .. }));
        assert!(matches!(path[6], PathPoint::Close));
    }

    #[test]
    fn test_square_path() {
        let path = square_path(100.0, 100.0, 30.0);
        assert!(!path.is_empty());
        assert!(matches!(path[0], PathPoint::MoveTo { .. }));
        assert!(matches!(path.last(), Some(PathPoint::Close)));
    }

    #[test]
    fn test_cloud_path() {
        let path = cloud_path(100.0, 100.0, 30.0);
        assert!(!path.is_empty());
        assert!(matches!(path[0], PathPoint::MoveTo { .. }));
    }

    #[test]
    fn test_person_path() {
        let path = person_path(100.0, 100.0, 30.0);
        assert!(!path.is_empty());
    }

    #[test]
    fn test_document_path() {
        let path = document_path(100.0, 100.0, 30.0);
        assert_eq!(path.len(), 6); // 5 edges + close
    }

    #[test]
    fn test_render_node_basic() {
        let style = NodeStyle::default();
        let state = NodeState::default();
        let commands = render_node(&RenderNodeParams {
            id: "bn-1234",
            title: "Test Task",
            short_name: None,
            entity_type: "task",
            status: "pending",
            doc_type: None,
            position: Position { x: 100.0, y: 100.0 },
            style: &style,
            state: &state,
        });

        // Should have at least: fill circle, stroke circle, text
        assert!(commands.len() >= 3);
    }

    #[test]
    fn test_render_node_selected() {
        let style = NodeStyle::default();
        let state = NodeState {
            selected: true,
            ..Default::default()
        };
        let commands = render_node(&RenderNodeParams {
            id: "bn-1234",
            title: "Test Task",
            short_name: None,
            entity_type: "task",
            status: "pending",
            doc_type: None,
            position: Position { x: 100.0, y: 100.0 },
            style: &style,
            state: &state,
        });

        // Should have selection highlight plus normal elements
        assert!(commands.len() >= 5);
    }

    #[test]
    fn test_render_node_with_short_name() {
        let style = NodeStyle::default();
        let state = NodeState::default();
        let commands = render_node(&RenderNodeParams {
            id: "bn-1234",
            title: "Full Task Title",
            short_name: Some("short"),
            entity_type: "task",
            status: "pending",
            doc_type: None,
            position: Position { x: 100.0, y: 100.0 },
            style: &style,
            state: &state,
        });

        // Should include text commands for short name and ID
        let text_commands: Vec<_> = commands
            .iter()
            .filter(|c| matches!(c, RenderCommand::Text { .. }))
            .collect();
        assert!(text_commands.len() >= 2); // short name + id
    }

    #[test]
    fn test_render_node_queued() {
        let style = NodeStyle::default();
        let state = NodeState {
            queued: true,
            ..Default::default()
        };
        let commands = render_node(&RenderNodeParams {
            id: "bn-1234",
            title: "Test Task",
            short_name: None,
            entity_type: "task",
            status: "pending",
            doc_type: None,
            position: Position { x: 100.0, y: 100.0 },
            style: &style,
            state: &state,
        });

        // Should have queued indicator (teal glow)
        assert!(commands.len() >= 5);
    }

    #[test]
    fn test_render_node_bug_shape() {
        let style = NodeStyle::default();
        let state = NodeState::default();
        let commands = render_node(&RenderNodeParams {
            id: "bn-1234",
            title: "Bug Report",
            short_name: None,
            entity_type: "bug",
            status: "pending",
            doc_type: None,
            position: Position { x: 100.0, y: 100.0 },
            style: &style,
            state: &state,
        });

        // Bugs use FillPath/StrokePath for square shape
        let has_path = commands
            .iter()
            .any(|c| matches!(c, RenderCommand::FillPath { .. }));
        assert!(has_path, "Bug nodes should use path-based rendering");
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
            RenderCommand::Arrow { color, dashed, .. } => {
                assert_eq!(color, theme::edge::BLOCKING);
                assert!(!dashed);
            }
            _ => panic!("Expected Arrow command"),
        }
    }

    #[test]
    fn test_render_edge_dashed_related() {
        let commands = render_edge_auto(
            Position { x: 0.0, y: 0.0 },
            Position { x: 100.0, y: 100.0 },
            "related_to",
        );

        match &commands[0] {
            RenderCommand::Arrow { dashed, .. } => {
                assert!(*dashed);
            }
            _ => panic!("Expected Arrow command"),
        }
    }

    #[test]
    fn test_edge_style_for_type() {
        let blocking = edge_style_for_type("depends_on");
        assert!(!blocking.dashed);
        assert_eq!(blocking.line_width, 2.0);

        let related = edge_style_for_type("related_to");
        assert!(related.dashed);

        let working = edge_style_for_type("working_on");
        assert!(working.animated);
        assert_eq!(working.line_width, 3.0);
    }

    #[test]
    fn test_truncate_text() {
        assert_eq!(truncate_text("Short", 10), "Short");
        assert_eq!(truncate_text("This is a very long title", 10), "This is a…");
    }

    #[test]
    fn test_wrap_text() {
        let lines = wrap_text("Hello World Test", 8, 2);
        assert_eq!(lines.len(), 2);

        let lines = wrap_text("Short", 10, 2);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "Short");
    }

    #[test]
    fn test_render_end_goal_rays() {
        let commands = render_end_goal_rays(100.0, 100.0, 30.0, 1.0);
        // 8 rays + 1 glow circle
        assert_eq!(commands.len(), 9);
    }

    #[test]
    fn test_render_in_progress_rings() {
        let commands = render_in_progress_rings(100.0, 100.0, 30.0, 1.0, 1000.0);
        // 12 outer hatches + 12 inner hatches
        assert_eq!(commands.len(), 24);
    }

    #[test]
    fn test_render_node_in_progress() {
        let style = NodeStyle::default();
        let state = NodeState {
            in_progress: true,
            animation_time: 1000.0,
            ..Default::default()
        };
        let commands = render_node(&RenderNodeParams {
            id: "bn-1234",
            title: "Working",
            short_name: None,
            entity_type: "task",
            status: "in_progress",
            doc_type: None,
            position: Position { x: 100.0, y: 100.0 },
            style: &style,
            state: &state,
        });

        // Should have animated rings (many arc commands)
        let arc_count = commands
            .iter()
            .filter(|c| matches!(c, RenderCommand::Arc { .. }))
            .count();
        assert_eq!(arc_count, 24); // 12 outer + 12 inner hatches
    }

    #[test]
    fn test_render_node_dimmed() {
        let style = NodeStyle::default();
        let state = NodeState {
            dimmed: true,
            ..Default::default()
        };
        let commands = render_node(&RenderNodeParams {
            id: "bn-1234",
            title: "Filtered Out",
            short_name: None,
            entity_type: "task",
            status: "pending",
            doc_type: None,
            position: Position { x: 100.0, y: 100.0 },
            style: &style,
            state: &state,
        });

        // Should start with SetAlpha and end with reset
        assert!(matches!(
            commands.first(),
            Some(RenderCommand::SetAlpha { alpha }) if *alpha == 0.3
        ));
        assert!(matches!(
            commands.last(),
            Some(RenderCommand::SetAlpha { alpha }) if *alpha == 1.0
        ));
    }
}
