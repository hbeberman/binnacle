//! Node Detail View - Shows full information about any node
//!
//! Displays complete node metadata, description, and edges with navigation.

use chrono::{DateTime, Utc};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use serde::Deserialize;

/// Edge information for display
#[derive(Debug, Clone, Deserialize)]
pub struct EdgeInfo {
    pub edge_type: String,
    pub direction: String,
    pub related_id: String,
    #[serde(default)]
    pub related_title: Option<String>,
}

impl EdgeInfo {
    /// Get display text for the edge (for rendering in the edge list)
    pub fn display_text(&self) -> String {
        let arrow = match self.direction.as_str() {
            "outbound" => "→",
            "inbound" => "←",
            "both" => "↔",
            _ => "?",
        };
        let title = self
            .related_title
            .as_deref()
            .unwrap_or("(untitled)")
            .chars()
            .take(40)
            .collect::<String>();
        format!(
            "{} {} {} \"{}\"",
            self.edge_type, arrow, self.related_id, title
        )
    }
}

/// Node detail data from the API
#[derive(Debug, Clone, Deserialize)]
pub struct NodeDetail {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub short_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub priority: Option<u8>,
    #[serde(default)]
    pub severity: Option<String>,
    #[serde(default)]
    pub assignee: Option<String>,
    #[serde(default)]
    pub doc_type: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub closed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub closed_reason: Option<String>,
    #[serde(default)]
    pub due_date: Option<String>,
    // Test-specific fields
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub working_dir: Option<String>,
}

impl NodeDetail {
    /// Get the display title (title or name, depending on entity type)
    pub fn display_title(&self) -> &str {
        self.title
            .as_deref()
            .or(self.name.as_deref())
            .unwrap_or("(untitled)")
    }

    /// Get the short display name if available
    pub fn display_short_name(&self) -> Option<&str> {
        self.short_name.as_deref()
    }
}

/// Navigation stack entry for going back
#[derive(Debug, Clone)]
pub struct NavigationEntry {
    pub node_id: String,
    /// Which view we came from (if not a node detail)
    pub from_list_view: bool,
}

/// State for the Node Detail view
pub struct NodeDetailView {
    /// Current node being displayed
    pub node: Option<NodeDetail>,
    /// Edges for this node
    pub edges: Vec<EdgeInfo>,
    /// Selected edge index
    pub edge_selection: usize,
    /// List state for edge rendering
    pub edge_list_state: ListState,
    /// Navigation stack for going back
    pub navigation_stack: Vec<NavigationEntry>,
    /// Scroll position for description/content
    pub scroll_offset: u16,
}

impl Default for NodeDetailView {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeDetailView {
    pub fn new() -> Self {
        let mut edge_list_state = ListState::default();
        edge_list_state.select(Some(0));
        Self {
            node: None,
            edges: Vec::new(),
            edge_selection: 0,
            edge_list_state,
            navigation_stack: Vec::new(),
            scroll_offset: 0,
        }
    }

    /// Set the current node and its edges
    pub fn set_node(&mut self, node: NodeDetail, edges: Vec<EdgeInfo>) {
        self.node = Some(node);
        self.edges = edges;
        self.edge_selection = 0;
        self.edge_list_state.select(Some(0));
        self.scroll_offset = 0;
    }

    /// Clear the current node
    pub fn clear(&mut self) {
        self.node = None;
        self.edges.clear();
        self.edge_selection = 0;
        self.edge_list_state.select(Some(0));
        self.scroll_offset = 0;
    }

    /// Push to navigation stack (call before navigating to a new node)
    pub fn push_navigation(&mut self, from_list: bool) {
        if let Some(node) = &self.node {
            self.navigation_stack.push(NavigationEntry {
                node_id: node.id.clone(),
                from_list_view: from_list,
            });
        }
    }

    /// Pop from navigation stack (returns ID to navigate to, or None if should go back to list)
    pub fn pop_navigation(&mut self) -> Option<NavigationEntry> {
        self.navigation_stack.pop()
    }

    /// Check if we can go back
    #[allow(dead_code)]
    pub fn can_go_back(&self) -> bool {
        !self.navigation_stack.is_empty()
    }

    /// Get currently selected edge
    pub fn selected_edge(&self) -> Option<&EdgeInfo> {
        self.edges.get(self.edge_selection)
    }

    /// Move edge selection down
    pub fn select_next_edge(&mut self) {
        if self.edges.is_empty() {
            return;
        }
        self.edge_selection = (self.edge_selection + 1).min(self.edges.len() - 1);
        self.edge_list_state.select(Some(self.edge_selection));
    }

    /// Move edge selection up
    pub fn select_previous_edge(&mut self) {
        if self.edges.is_empty() {
            return;
        }
        self.edge_selection = self.edge_selection.saturating_sub(1);
        self.edge_list_state.select(Some(self.edge_selection));
    }

    /// Jump to first edge
    pub fn select_first_edge(&mut self) {
        self.edge_selection = 0;
        self.edge_list_state.select(Some(0));
    }

    /// Jump to last edge
    pub fn select_last_edge(&mut self) {
        if self.edges.is_empty() {
            return;
        }
        self.edge_selection = self.edges.len() - 1;
        self.edge_list_state.select(Some(self.edge_selection));
    }

    /// Select edge at specific index (for mouse clicks)
    pub fn select_edge_at(&mut self, index: usize) {
        if self.edges.is_empty() {
            return;
        }
        if index < self.edges.len() {
            self.edge_selection = index;
            self.edge_list_state.select(Some(self.edge_selection));
        }
    }

    /// Scroll content up (for future use with long descriptions)
    #[allow(dead_code)]
    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    /// Scroll content down (for future use with long descriptions)
    #[allow(dead_code)]
    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1);
    }

    /// Render the view
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let node = match &self.node {
            Some(n) => n,
            None => {
                let empty = Paragraph::new("No node selected")
                    .style(Style::default().fg(Color::DarkGray))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(" Node Detail "),
                    );
                frame.render_widget(empty, area);
                return;
            }
        };

        // Split the area: top for node info, bottom for edges
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(10),    // Node info (expandable)
                Constraint::Length(12), // Edges section (fixed)
            ])
            .split(area);

        // Render node information
        self.render_node_info(frame, chunks[0], node);

        // Render edges
        self.render_edges(frame, chunks[1]);
    }

    /// Render the node information section
    fn render_node_info(&self, frame: &mut Frame, area: Rect, node: &NodeDetail) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", node.id));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Build the info text
        let mut lines: Vec<Line> = Vec::new();

        // Title
        lines.push(Line::from(vec![
            Span::styled("Title: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(node.display_title()),
        ]));

        // Type, Status, Priority on one line
        let mut meta_spans = vec![
            Span::styled("Type: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                &node.node_type,
                Style::default().fg(get_type_color(&node.node_type)),
            ),
        ];

        if let Some(status) = &node.status {
            meta_spans.push(Span::raw("   "));
            meta_spans.push(Span::styled(
                "Status: ",
                Style::default().add_modifier(Modifier::BOLD),
            ));
            meta_spans.push(Span::styled(
                status,
                Style::default().fg(get_status_color(status)),
            ));
        }

        if let Some(priority) = node.priority {
            meta_spans.push(Span::raw("   "));
            meta_spans.push(Span::styled(
                "Priority: ",
                Style::default().add_modifier(Modifier::BOLD),
            ));
            meta_spans.push(Span::styled(
                format!("P{}", priority),
                Style::default().fg(get_priority_color(priority)),
            ));
        }

        if let Some(severity) = &node.severity {
            meta_spans.push(Span::raw("   "));
            meta_spans.push(Span::styled(
                "Severity: ",
                Style::default().add_modifier(Modifier::BOLD),
            ));
            meta_spans.push(Span::raw(severity));
        }

        lines.push(Line::from(meta_spans));

        // Assignee (if present)
        if let Some(assignee) = &node.assignee {
            if !assignee.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled("Assignee: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::styled(format!("@{}", assignee), Style::default().fg(Color::Cyan)),
                ]));
            }
        }

        // Doc type (for docs)
        if let Some(doc_type) = &node.doc_type {
            lines.push(Line::from(vec![
                Span::styled("Doc Type: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(doc_type),
            ]));
        }

        // Tags (if any)
        if !node.tags.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Tags: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(node.tags.join(", "), Style::default().fg(Color::Magenta)),
            ]));
        }

        // Timestamps
        let mut ts_spans = Vec::new();
        if let Some(created) = &node.created_at {
            ts_spans.push(Span::styled(
                "Created: ",
                Style::default().add_modifier(Modifier::BOLD),
            ));
            ts_spans.push(Span::raw(created.format("%Y-%m-%d %H:%M").to_string()));
        }
        if let Some(updated) = &node.updated_at {
            if !ts_spans.is_empty() {
                ts_spans.push(Span::raw("   "));
            }
            ts_spans.push(Span::styled(
                "Updated: ",
                Style::default().add_modifier(Modifier::BOLD),
            ));
            ts_spans.push(Span::raw(updated.format("%Y-%m-%d %H:%M").to_string()));
        }
        if !ts_spans.is_empty() {
            lines.push(Line::from(ts_spans));
        }

        // Due date (for milestones)
        if let Some(due) = &node.due_date {
            lines.push(Line::from(vec![
                Span::styled("Due Date: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(due),
            ]));
        }

        // Closed info
        if let Some(closed_at) = &node.closed_at {
            lines.push(Line::from(vec![
                Span::styled("Closed: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(closed_at.format("%Y-%m-%d %H:%M").to_string()),
            ]));
        }
        if let Some(reason) = &node.closed_reason {
            if !reason.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled("Reason: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(reason),
                ]));
            }
        }

        // Test-specific fields
        if let Some(cmd) = &node.command {
            lines.push(Line::from(vec![
                Span::styled("Command: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(cmd, Style::default().fg(Color::Yellow)),
            ]));
        }
        if let Some(dir) = &node.working_dir {
            lines.push(Line::from(vec![
                Span::styled(
                    "Working Dir: ",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(dir),
            ]));
        }

        // Empty line before description
        lines.push(Line::from(""));

        // Description or Summary
        if let Some(desc) = &node.description {
            if !desc.is_empty() {
                lines.push(Line::from(vec![Span::styled(
                    "Description:",
                    Style::default().add_modifier(Modifier::BOLD),
                )]));
                // Split description into lines
                for line in desc.lines() {
                    lines.push(Line::from(format!("  {}", line)));
                }
            }
        } else if let Some(summary) = &node.summary {
            if !summary.is_empty() {
                lines.push(Line::from(vec![Span::styled(
                    "Summary:",
                    Style::default().add_modifier(Modifier::BOLD),
                )]));
                for line in summary.lines() {
                    lines.push(Line::from(format!("  {}", line)));
                }
            }
        }

        let text = Text::from(lines);
        let para = Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .scroll((self.scroll_offset, 0));
        frame.render_widget(para, inner);
    }

    /// Render the edges section
    fn render_edges(&mut self, frame: &mut Frame, area: Rect) {
        if self.edges.is_empty() {
            let empty = Paragraph::new("  No edges")
                .style(Style::default().fg(Color::DarkGray))
                .block(Block::default().borders(Borders::ALL).title(" Edges "));
            frame.render_widget(empty, area);
            return;
        }

        // Build list items
        let list_items: Vec<ListItem> = self
            .edges
            .iter()
            .enumerate()
            .map(|(idx, edge)| {
                let selected_marker = if idx == self.edge_selection { ">" } else { " " };

                let direction_style = match edge.direction.as_str() {
                    "outbound" => Style::default().fg(Color::Green),
                    "inbound" => Style::default().fg(Color::Yellow),
                    _ => Style::default().fg(Color::Cyan),
                };

                let arrow = match edge.direction.as_str() {
                    "outbound" => "→",
                    "inbound" => "←",
                    "both" => "↔",
                    _ => "?",
                };

                let title = edge
                    .related_title
                    .as_deref()
                    .unwrap_or("(untitled)")
                    .chars()
                    .take(30)
                    .collect::<String>();

                let line = Line::from(vec![
                    Span::raw(format!(" {} ", selected_marker)),
                    Span::styled(
                        format!("{:<12}", edge.edge_type),
                        Style::default().fg(Color::Blue),
                    ),
                    Span::styled(format!(" {} ", arrow), direction_style),
                    Span::styled(
                        format!("{:<8}", edge.related_id),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::raw(" "),
                    Span::raw(format!("\"{}\"", title)),
                ]);

                let item_style = if idx == self.edge_selection {
                    Style::default().bg(Color::DarkGray)
                } else {
                    Style::default()
                };

                ListItem::new(line).style(item_style)
            })
            .collect();

        let edges_title = format!(" Edges ({}) ", self.edges.len());
        let list = List::new(list_items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(edges_title)
                .title_bottom(" [j/k] Navigate  [Enter] Go to  [Esc] Back "),
        );

        frame.render_widget(list, area);
    }
}

/// Get color for entity type
fn get_type_color(node_type: &str) -> Color {
    match node_type {
        "task" => Color::Blue,
        "bug" => Color::Red,
        "idea" => Color::Magenta,
        "milestone" => Color::Yellow,
        "doc" => Color::Green,
        "test" => Color::Cyan,
        "queue" => Color::LightYellow,
        _ => Color::White,
    }
}

/// Get color for status
fn get_status_color(status: &str) -> Color {
    match status {
        "done" => Color::Green,
        "in_progress" => Color::Yellow,
        "pending" => Color::White,
        "blocked" => Color::Red,
        "cancelled" => Color::DarkGray,
        "promoted" => Color::Cyan,
        _ => Color::White,
    }
}

/// Get color for priority
fn get_priority_color(priority: u8) -> Color {
    match priority {
        0 => Color::Red,
        1 => Color::LightRed,
        2 => Color::Yellow,
        3 => Color::Green,
        _ => Color::DarkGray,
    }
}
