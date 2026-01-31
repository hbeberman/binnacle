//! Recently Completed View - Shows recently closed tasks and bugs
//!
//! Displays completed items with relative timestamp display.

use chrono::{DateTime, Utc};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use serde::{Deserialize, Serialize};

/// Item displayed in the recently completed list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedItem {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub short_name: Option<String>,
    pub priority: u8,
    /// When the item was closed
    #[serde(default)]
    pub closed_at: Option<DateTime<Utc>>,
    /// Reason for closing
    #[serde(default)]
    pub closed_reason: Option<String>,
    /// Entity type (task, bug)
    #[serde(default, rename = "type")]
    pub entity_type: Option<String>,
}

impl CompletedItem {
    /// Get display title (prefer short_name if available)
    pub fn display_title(&self) -> &str {
        self.short_name.as_deref().unwrap_or(&self.title)
    }

    /// Format relative time since completion
    pub fn relative_time(&self) -> String {
        match self.closed_at {
            Some(closed) => {
                let now = Utc::now();
                let duration = now.signed_duration_since(closed);

                if duration.num_seconds() < 60 {
                    "just now".to_string()
                } else if duration.num_minutes() < 60 {
                    let mins = duration.num_minutes();
                    format!("{}m ago", mins)
                } else if duration.num_hours() < 24 {
                    let hours = duration.num_hours();
                    format!("{}h ago", hours)
                } else if duration.num_days() < 7 {
                    let days = duration.num_days();
                    format!("{}d ago", days)
                } else {
                    // Format as date
                    closed.format("%Y-%m-%d").to_string()
                }
            }
            None => "unknown".to_string(),
        }
    }
}

/// State for the Recently Completed view
pub struct RecentlyCompletedView {
    /// Completed tasks
    pub tasks: Vec<CompletedItem>,
    /// Completed bugs
    pub bugs: Vec<CompletedItem>,
    /// All items combined (bugs first, then tasks)
    pub items: Vec<CompletedItem>,
    /// Selected item index
    pub selected: usize,
    /// List widget state
    pub list_state: ListState,
    /// Number of bugs (for display separator)
    pub bug_count: usize,
}

impl Default for RecentlyCompletedView {
    fn default() -> Self {
        Self::new()
    }
}

impl RecentlyCompletedView {
    pub fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            tasks: Vec::new(),
            bugs: Vec::new(),
            items: Vec::new(),
            selected: 0,
            list_state,
            bug_count: 0,
        }
    }

    /// Update the view with new data from the server
    pub fn update_items(&mut self, tasks: Vec<CompletedItem>, bugs: Vec<CompletedItem>) {
        self.bug_count = bugs.len();
        self.tasks = tasks;
        self.bugs = bugs.clone();

        // Combine items: bugs first, then tasks (both sorted by closed_at descending)
        let mut items = bugs;
        items.extend(self.tasks.clone());

        // Sort by closed_at descending (most recent first)
        items.sort_by(|a, b| {
            let a_time = a.closed_at.unwrap_or(DateTime::UNIX_EPOCH);
            let b_time = b.closed_at.unwrap_or(DateTime::UNIX_EPOCH);
            b_time.cmp(&a_time)
        });

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

    /// Select item at specific index (for mouse clicks)
    pub fn select_at(&mut self, index: usize) {
        if self.items.is_empty() {
            return;
        }
        // Account for header row (COMPLETED header)
        if index == 0 {
            return; // Click on header
        }
        let adjusted_index = index - 1;
        if adjusted_index < self.items.len() {
            self.selected = adjusted_index;
            self.list_state.select(Some(self.selected));
        }
    }

    /// Render the view
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        if self.items.is_empty() {
            let empty = Paragraph::new("No recently completed items")
                .style(Style::default().fg(Color::DarkGray))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Recently Completed "),
                );
            frame.render_widget(empty, area);
            return;
        }

        // Calculate available width for title
        // Format: " > bn-xxxx  [P1] title...                     10m ago"
        let title_width = area.width.saturating_sub(38) as usize;

        // Build list items
        let mut list_items: Vec<ListItem> = Vec::new();

        // Add header
        list_items.push(ListItem::new(Line::from(vec![Span::styled(
            format!(" COMPLETED ({})", self.items.len()),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )])));

        for (idx, item) in self.items.iter().enumerate() {
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

            let relative_time = item.relative_time();

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
                    format!(" {:>10}", relative_time),
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
