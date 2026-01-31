//! Queue/Ready View - Primary dashboard showing actionable work
//!
//! Displays queued and ready items with keyboard navigation.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use serde::{Deserialize, Serialize};

/// Item displayed in the queue/ready list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkItem {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub short_name: Option<String>,
    pub priority: u8,
    #[serde(default)]
    pub assignee: Option<String>,
    /// True if this item is queued (vs just ready)
    #[serde(default)]
    pub queued: bool,
    /// Entity type (task, bug)
    #[serde(default, rename = "type")]
    pub entity_type: Option<String>,
}

impl WorkItem {
    /// Get display title (prefer short_name if available)
    pub fn display_title(&self) -> &str {
        self.short_name.as_deref().unwrap_or(&self.title)
    }

    /// Format assignee for display
    pub fn display_assignee(&self) -> String {
        match &self.assignee {
            Some(a) if !a.is_empty() => format!("@{}", a),
            _ => "(unassigned)".to_string(),
        }
    }
}

/// State for the Queue/Ready view
pub struct QueueReadyView {
    /// All work items (queued first, then ready)
    pub items: Vec<WorkItem>,
    /// Selected item index
    pub selected: usize,
    /// List widget state
    pub list_state: ListState,
    /// Number of queued items (for display separator)
    pub queued_count: usize,
}

impl Default for QueueReadyView {
    fn default() -> Self {
        Self::new()
    }
}

impl QueueReadyView {
    pub fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            items: Vec::new(),
            selected: 0,
            list_state,
            queued_count: 0,
        }
    }

    /// Update the view with new data from the server
    pub fn update_items(&mut self, queued: Vec<WorkItem>, ready: Vec<WorkItem>) {
        self.queued_count = queued.len();

        // Combine items: queued first (sorted by priority), then ready (sorted by priority)
        let mut items = queued;
        items.extend(ready);

        self.items = items;

        // Keep selection valid
        if self.selected >= self.items.len() {
            self.selected = self.items.len().saturating_sub(1);
        }
        self.list_state.select(Some(self.selected));
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.selected = (self.selected + 1).min(self.items.len() - 1);
        self.list_state.select(Some(self.selected));
    }

    /// Move selection up
    pub fn select_previous(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.selected = self.selected.saturating_sub(1);
        self.list_state.select(Some(self.selected));
    }

    /// Jump to top
    pub fn select_first(&mut self) {
        self.selected = 0;
        self.list_state.select(Some(0));
    }

    /// Jump to bottom
    pub fn select_last(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.selected = self.items.len() - 1;
        self.list_state.select(Some(self.selected));
    }

    /// Get the currently selected item (for future detail view integration)
    #[allow(dead_code)]
    pub fn selected_item(&self) -> Option<&WorkItem> {
        self.items.get(self.selected)
    }

    /// Render the view
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        if self.items.is_empty() {
            let empty = Paragraph::new("No queued or ready items")
                .style(Style::default().fg(Color::DarkGray))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Queue/Ready "),
                );
            frame.render_widget(empty, area);
            return;
        }

        // Calculate available width for title (accounting for other columns)
        // Format: " > bn-xxxx  [P1] title...                     @assignee"
        // Widths:  3 + 7 + 2 + 4 + 2 = 18 chars before title, ~20 for assignee
        let title_width = area.width.saturating_sub(40) as usize;

        // Build list items
        let mut list_items: Vec<ListItem> = Vec::new();

        for (idx, item) in self.items.iter().enumerate() {
            // Add section headers
            if idx == 0 && self.queued_count > 0 {
                list_items.push(ListItem::new(Line::from(vec![Span::styled(
                    format!(" QUEUED ({})", self.queued_count),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )])));
            }
            if idx == self.queued_count && idx > 0 {
                // Add empty line between sections
                list_items.push(ListItem::new(Line::from("")));
                let ready_count = self.items.len() - self.queued_count;
                list_items.push(ListItem::new(Line::from(vec![Span::styled(
                    format!(" READY ({})", ready_count),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )])));
            } else if idx == 0 && self.queued_count == 0 {
                list_items.push(ListItem::new(Line::from(vec![Span::styled(
                    format!(" READY ({})", self.items.len()),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )])));
            }

            // Format item line
            let selected_marker = if idx == self.selected { ">" } else { " " };
            let priority_style = match item.priority {
                0 => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                1 => Style::default().fg(Color::LightRed),
                2 => Style::default().fg(Color::Yellow),
                3 => Style::default().fg(Color::Green),
                _ => Style::default().fg(Color::DarkGray),
            };

            // Truncate title to available width
            let title = item.display_title();
            let truncated_title = if title.len() > title_width {
                format!("{}...", &title[..title_width.saturating_sub(3)])
            } else {
                title.to_string()
            };

            let assignee = item.display_assignee();

            // Determine item type indicator
            let type_indicator = match item.entity_type.as_deref() {
                Some("bug") => "●",
                _ => " ",
            };

            let line = Line::from(vec![
                Span::raw(format!(" {} ", selected_marker)),
                Span::styled(format!("{:<7}", item.id), Style::default().fg(Color::Blue)),
                Span::raw(" "),
                Span::styled(format!("[P{}]", item.priority), priority_style),
                Span::raw(" "),
                Span::styled(type_indicator, Style::default().fg(Color::Red)),
                Span::raw(format!("{:<width$}", truncated_title, width = title_width)),
                Span::styled(
                    format!(" {:>15}", assignee),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);

            let item_style = if idx == self.selected {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };

            list_items.push(ListItem::new(line).style(item_style));
        }

        let list = List::new(list_items).block(Block::default().borders(Borders::ALL));

        frame.render_widget(list, area);
    }
}
