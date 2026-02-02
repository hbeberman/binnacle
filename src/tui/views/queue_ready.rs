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
    /// All work items (in_progress first, then queued, then ready)
    pub items: Vec<WorkItem>,
    /// Selected item index
    pub selected: usize,
    /// List widget state
    pub list_state: ListState,
    /// Number of in-progress items (for display separator)
    pub in_progress_count: usize,
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
            in_progress_count: 0,
            queued_count: 0,
        }
    }

    /// Update the view with new data from the server
    pub fn update_items_with_in_progress(
        &mut self,
        in_progress: Vec<WorkItem>,
        queued: Vec<WorkItem>,
        ready: Vec<WorkItem>,
    ) {
        self.in_progress_count = in_progress.len();
        self.queued_count = queued.len();

        // Combine items: in_progress first, then queued (sorted by priority), then ready (sorted by priority)
        let mut items = in_progress;
        items.extend(queued);
        items.extend(ready);

        self.items = items;

        // Keep selection valid
        if self.selected >= self.items.len() {
            self.selected = self.items.len().saturating_sub(1);
        }
        self.list_state.select(Some(self.selected));
    }

    /// Update the view with new data from the server (legacy - no in_progress)
    #[allow(dead_code)]
    pub fn update_items(&mut self, queued: Vec<WorkItem>, ready: Vec<WorkItem>) {
        self.update_items_with_in_progress(Vec::new(), queued, ready);
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
        // Account for header rows when mapping click position to item index
        // Headers take up space: section header (1 line), empty line + section header (2 lines) between sections
        // Sections in order: IN PROGRESS, QUEUED, READY
        let mut header_offset = 0;
        let mut item_index = index;

        // Handle IN PROGRESS section
        if self.in_progress_count > 0 {
            if index == 0 {
                return; // Click on IN PROGRESS header
            }
            item_index -= 1; // Skip IN PROGRESS header
            header_offset += 1;

            if item_index < self.in_progress_count {
                // Clicked on an in-progress item
                self.selected = item_index;
                self.list_state.select(Some(self.selected));
                return;
            }

            // Skip the empty line after in-progress
            if item_index == self.in_progress_count {
                return; // Click on empty line
            }
            item_index -= 1;
            header_offset += 1;
        }

        // Handle QUEUED section
        if self.queued_count > 0 {
            let queued_header_row = if self.in_progress_count > 0 {
                self.in_progress_count + 2 // items + header + empty line
            } else {
                0
            };

            if index == queued_header_row {
                return; // Click on QUEUED header
            }

            let queue_start = self.in_progress_count;
            let items_before_queue = item_index - (if self.in_progress_count > 0 { 0 } else { 1 });

            if items_before_queue < self.queued_count {
                self.selected = queue_start + items_before_queue;
                self.list_state.select(Some(self.selected));
                return;
            }

            // Skip the empty line after queued
            item_index -= 1;
        }

        // Handle READY section - simplified: just use the remaining index
        // This is a best-effort click handler; complex multi-section layouts
        // are easier to navigate with keyboard
        let ready_start = self.in_progress_count + self.queued_count;
        if ready_start < self.items.len() {
            let final_index = ready_start
                + item_index
                    .saturating_sub(header_offset + self.in_progress_count + self.queued_count);
            if final_index < self.items.len() {
                self.selected = final_index;
                self.list_state.select(Some(self.selected));
            }
        }
    }

    /// Get the currently selected item (for future detail view integration)
    #[allow(dead_code)]
    pub fn selected_item(&self) -> Option<&WorkItem> {
        self.items.get(self.selected)
    }

    /// Get total count of items (queued + ready)
    pub fn total_items(&self) -> usize {
        self.items.len()
    }

    /// Render the view
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        if self.items.is_empty() {
            let empty = Paragraph::new("No work items")
                .style(Style::default().fg(Color::DarkGray))
                .block(Block::default().borders(Borders::ALL).title(" Work Items "));
            frame.render_widget(empty, area);
            return;
        }

        // Calculate available width for title (accounting for other columns)
        // Format: " > bn-xxxx  [P1] title...                     @assignee"
        // Widths:  3 + 7 + 2 + 4 + 2 = 18 chars before title, ~20 for assignee
        let title_width = area.width.saturating_sub(40) as usize;

        // Build list items
        let mut list_items: Vec<ListItem> = Vec::new();

        // Calculate section boundaries
        let in_progress_end = self.in_progress_count;
        let queued_end = in_progress_end + self.queued_count;

        for (idx, item) in self.items.iter().enumerate() {
            // Add section headers at appropriate positions
            if idx == 0 && self.in_progress_count > 0 {
                // IN PROGRESS header (shown in green to indicate active work)
                list_items.push(ListItem::new(Line::from(vec![Span::styled(
                    format!(" ⚡ IN PROGRESS ({})", self.in_progress_count),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )])));
            } else if idx == in_progress_end && self.in_progress_count > 0 {
                // Separator before QUEUED or READY section
                list_items.push(ListItem::new(Line::from("")));
                if self.queued_count > 0 {
                    list_items.push(ListItem::new(Line::from(vec![Span::styled(
                        format!(" QUEUED ({})", self.queued_count),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    )])));
                } else {
                    let ready_count = self.items.len() - in_progress_end;
                    list_items.push(ListItem::new(Line::from(vec![Span::styled(
                        format!(" READY ({})", ready_count),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )])));
                }
            } else if idx == queued_end && self.queued_count > 0 {
                // READY header after QUEUED section
                list_items.push(ListItem::new(Line::from("")));
                let ready_count = self.items.len() - queued_end;
                list_items.push(ListItem::new(Line::from(vec![Span::styled(
                    format!(" READY ({})", ready_count),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )])));
            } else if idx == 0 && self.in_progress_count == 0 && self.queued_count > 0 {
                // QUEUED header when no in-progress items
                list_items.push(ListItem::new(Line::from(vec![Span::styled(
                    format!(" QUEUED ({})", self.queued_count),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )])));
            } else if idx == 0 && self.in_progress_count == 0 && self.queued_count == 0 {
                // READY header when no in-progress or queued items
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
