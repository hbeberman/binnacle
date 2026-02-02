//! Recent Changes View - Shows recent activity in full-page format
//!
//! Displays activity log entries (task updates, commits, etc.) as Page 2.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use std::collections::VecDeque;

use super::log_panel::LogEntry;

/// Maximum number of entries to display
const MAX_ENTRIES: usize = 100;

/// State for the Recent Changes view
pub struct RecentChangesView {
    /// Log entries (newest first)
    entries: VecDeque<LogEntry>,
    /// Selected entry index
    pub selected: usize,
    /// List widget state
    pub list_state: ListState,
}

impl Default for RecentChangesView {
    fn default() -> Self {
        Self::new()
    }
}

impl RecentChangesView {
    pub fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            entries: VecDeque::new(),
            selected: 0,
            list_state,
        }
    }

    /// Get the entries (for potential future sharing with log panel)
    #[allow(dead_code)]
    pub fn entries(&self) -> &VecDeque<LogEntry> {
        &self.entries
    }

    /// Get mutable entries
    #[allow(dead_code)]
    pub fn entries_mut(&mut self) -> &mut VecDeque<LogEntry> {
        &mut self.entries
    }

    /// Add a log entry
    pub fn add_entry(&mut self, entry: LogEntry) {
        self.entries.push_front(entry);
        if self.entries.len() > MAX_ENTRIES {
            self.entries.pop_back();
        }
    }

    /// Add a simple message entry
    pub fn log(&mut self, action: impl Into<String>) {
        self.add_entry(LogEntry::new(action));
    }

    /// Add an entity event entry
    pub fn log_entity(
        &mut self,
        entity_type: impl Into<String>,
        entity_id: impl Into<String>,
        action: impl Into<String>,
    ) {
        self.add_entry(LogEntry::entity_event(entity_type, entity_id, action));
    }

    /// Get entry count
    #[allow(dead_code)]
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        self.selected = (self.selected + 1).min(self.entries.len() - 1);
        self.list_state.select(Some(self.selected));
    }

    /// Move selection up
    pub fn select_previous(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        self.selected = self.selected.saturating_sub(1);
        self.list_state.select(Some(self.selected));
    }

    /// Jump to top (most recent)
    pub fn select_first(&mut self) {
        self.selected = 0;
        self.list_state.select(Some(0));
    }

    /// Jump to bottom (oldest)
    pub fn select_last(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        self.selected = self.entries.len() - 1;
        self.list_state.select(Some(self.selected));
    }

    /// Select item at specific index (for mouse clicks)
    pub fn select_at(&mut self, index: usize) {
        if self.entries.is_empty() {
            return;
        }
        // Account for header row
        if index == 0 {
            return; // Click on header
        }
        let adjusted_index = index - 1;
        if adjusted_index < self.entries.len() {
            self.selected = adjusted_index;
            self.list_state.select(Some(self.selected));
        }
    }

    /// Get currently selected entry ID (if entity-related)
    pub fn selected_entity_id(&self) -> Option<&str> {
        self.entries
            .get(self.selected)
            .and_then(|e| e.entity_id.as_deref())
    }

    /// Render the view
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        if self.entries.is_empty() {
            let empty = Paragraph::new("No recent changes")
                .style(Style::default().fg(Color::DarkGray))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Recent Changes "),
                );
            frame.render_widget(empty, area);
            return;
        }

        // Calculate available width for message
        // Format: " > 10m ago  + task bn-xxxx created by agent-123"
        let msg_width = area.width.saturating_sub(20) as usize;

        // Build list items
        let mut list_items: Vec<ListItem> = Vec::new();

        // Add header
        list_items.push(ListItem::new(Line::from(vec![Span::styled(
            format!(" RECENT CHANGES ({})", self.entries.len()),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )])));

        for (idx, entry) in self.entries.iter().enumerate() {
            let selected_marker = if idx == self.selected { ">" } else { " " };

            let time = entry.relative_time();
            let icon = entry.icon();
            let color = entry.color();

            // Build display message with entity info
            let mut msg_parts = Vec::new();

            if let (Some(etype), Some(eid)) = (&entry.entity_type, &entry.entity_id) {
                msg_parts.push(format!("{} {}", etype, eid));
            }

            msg_parts.push(entry.action.clone());

            if let Some(agent) = &entry.agent_id {
                msg_parts.push(format!("by {}", agent));
            }

            if let Some(details) = &entry.details {
                if !details.is_empty() {
                    msg_parts.push(format!("({})", details));
                }
            }

            let msg = msg_parts.join(" ");

            // Truncate message if needed
            let display_msg = if msg.len() > msg_width {
                format!("{}...", &msg[..msg_width.saturating_sub(3)])
            } else {
                msg
            };

            let line = Line::from(vec![
                Span::raw(format!(" {} ", selected_marker)),
                Span::styled(format!("{:>8}", time), Style::default().fg(Color::DarkGray)),
                Span::raw("  "),
                Span::styled(icon, Style::default().fg(color)),
                Span::raw(" "),
                Span::styled(display_msg, Style::default().fg(Color::White)),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recent_changes_view_new() {
        let view = RecentChangesView::new();
        assert!(view.is_empty());
        assert_eq!(view.entry_count(), 0);
        assert_eq!(view.selected, 0);
    }

    #[test]
    fn test_add_entry() {
        let mut view = RecentChangesView::new();
        view.log("connected");
        assert_eq!(view.entry_count(), 1);
        assert!(!view.is_empty());
    }

    #[test]
    fn test_log_entity() {
        let mut view = RecentChangesView::new();
        view.log_entity("task", "bn-1234", "created");
        assert_eq!(view.entry_count(), 1);
        assert_eq!(view.selected_entity_id(), Some("bn-1234"));
    }

    #[test]
    fn test_navigation() {
        let mut view = RecentChangesView::new();
        view.log("entry 1");
        view.log("entry 2");
        view.log("entry 3");

        // Start at 0 (most recent)
        assert_eq!(view.selected, 0);

        // Move down
        view.select_next();
        assert_eq!(view.selected, 1);

        view.select_next();
        assert_eq!(view.selected, 2);

        // Can't go past end
        view.select_next();
        assert_eq!(view.selected, 2);

        // Move up
        view.select_previous();
        assert_eq!(view.selected, 1);

        // Jump to last
        view.select_last();
        assert_eq!(view.selected, 2);

        // Jump to first
        view.select_first();
        assert_eq!(view.selected, 0);
    }

    #[test]
    fn test_max_entries() {
        let mut view = RecentChangesView::new();
        for i in 0..(MAX_ENTRIES + 10) {
            view.log(format!("entry {}", i));
        }
        assert_eq!(view.entry_count(), MAX_ENTRIES);
    }

    #[test]
    fn test_select_at() {
        let mut view = RecentChangesView::new();
        view.log("entry 1");
        view.log("entry 2");
        view.log("entry 3");

        // Click on header (index 0) should not change selection
        view.select_at(0);
        assert_eq!(view.selected, 0);

        // Click on item at index 2 (item 1, 0-indexed)
        view.select_at(2);
        assert_eq!(view.selected, 1);

        // Click on item at index 3 (item 2, 0-indexed)
        view.select_at(3);
        assert_eq!(view.selected, 2);

        // Click beyond bounds should not change selection
        view.select_at(100);
        assert_eq!(view.selected, 2);
    }
}
