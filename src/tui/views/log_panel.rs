//! Log Panel View - Always visible activity log
//!
//! Shows real-time activity log entries at the bottom of the screen.

use chrono::{DateTime, Utc};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use serde::Deserialize;
use std::collections::VecDeque;

/// Maximum number of log entries to keep
const MAX_LOG_ENTRIES: usize = 100;

/// Log entry from the server
#[derive(Debug, Clone, Deserialize)]
pub struct LogEntry {
    /// Timestamp of the log entry
    #[serde(default = "Utc::now")]
    pub timestamp: DateTime<Utc>,
    /// Entity type that was affected (task, bug, etc.)
    #[serde(default)]
    pub entity_type: Option<String>,
    /// Entity ID that was affected
    #[serde(default)]
    pub entity_id: Option<String>,
    /// Action that occurred (created, updated, closed, etc.)
    #[serde(default)]
    pub action: String,
    /// Optional additional details
    #[serde(default)]
    pub details: Option<String>,
    /// Agent ID if applicable
    #[serde(default)]
    pub agent_id: Option<String>,
}

impl LogEntry {
    /// Create a new log entry for an event
    pub fn new(action: impl Into<String>) -> Self {
        Self {
            timestamp: Utc::now(),
            entity_type: None,
            entity_id: None,
            action: action.into(),
            details: None,
            agent_id: None,
        }
    }

    /// Create a log entry for an entity event
    pub fn entity_event(
        entity_type: impl Into<String>,
        entity_id: impl Into<String>,
        action: impl Into<String>,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            entity_type: Some(entity_type.into()),
            entity_id: Some(entity_id.into()),
            action: action.into(),
            details: None,
            agent_id: None,
        }
    }

    /// Format relative time since this entry
    pub fn relative_time(&self) -> String {
        let now = Utc::now();
        let duration = now.signed_duration_since(self.timestamp);

        if duration.num_seconds() < 60 {
            format!("{}s", duration.num_seconds().max(0))
        } else if duration.num_minutes() < 60 {
            format!("{}m", duration.num_minutes())
        } else if duration.num_hours() < 24 {
            format!("{}h", duration.num_hours())
        } else {
            self.timestamp.format("%H:%M").to_string()
        }
    }

    /// Get a compact display string
    pub fn display(&self) -> String {
        let mut parts = Vec::new();

        if let (Some(etype), Some(eid)) = (&self.entity_type, &self.entity_id) {
            parts.push(format!("{} {}", etype, eid));
        }

        parts.push(self.action.clone());

        if let Some(agent) = &self.agent_id {
            parts.push(format!("by {}", agent));
        }

        if let Some(details) = &self.details {
            parts.push(details.clone());
        }

        parts.join(" - ")
    }

    /// Get icon for this entry based on action
    pub fn icon(&self) -> &'static str {
        match self.action.as_str() {
            "connected" | "reconnected" => "●",
            "disconnected" => "○",
            "created" | "added" => "+",
            "closed" | "completed" => "✓",
            "updated" | "modified" => "~",
            "deleted" | "removed" => "×",
            "error" | "failed" => "!",
            _ => "·",
        }
    }

    /// Get color for this entry based on action
    pub fn color(&self) -> Color {
        match self.action.as_str() {
            "connected" | "reconnected" => Color::Green,
            "disconnected" => Color::Red,
            "created" | "added" => Color::Cyan,
            "closed" | "completed" => Color::Green,
            "updated" | "modified" => Color::Yellow,
            "deleted" | "removed" => Color::Red,
            "error" | "failed" => Color::LightRed,
            _ => Color::DarkGray,
        }
    }
}

/// State for the Log Panel view
pub struct LogPanelView {
    /// Log entries (newest first)
    entries: VecDeque<LogEntry>,
    /// List widget state (for scrolling)
    list_state: ListState,
    /// Whether the panel is collapsed
    pub collapsed: bool,
    /// Selected entry (for potential copy/inspect)
    pub selected: usize,
    /// Whether log panel has focus
    pub focused: bool,
}

impl Default for LogPanelView {
    fn default() -> Self {
        Self::new()
    }
}

impl LogPanelView {
    /// Create a new log panel
    pub fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            entries: VecDeque::new(),
            list_state,
            collapsed: false,
            selected: 0,
            focused: false,
        }
    }

    /// Add a log entry
    pub fn add_entry(&mut self, entry: LogEntry) {
        self.entries.push_front(entry);
        if self.entries.len() > MAX_LOG_ENTRIES {
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

    /// Clear all entries
    pub fn clear(&mut self) {
        self.entries.clear();
        self.selected = 0;
    }

    /// Toggle collapsed state
    pub fn toggle_collapsed(&mut self) {
        self.collapsed = !self.collapsed;
    }

    /// Get entry count
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Move selection down (newer entries are at top, so "down" goes to older)
    pub fn select_next(&mut self) {
        if self.selected < self.entries.len().saturating_sub(1) {
            self.selected += 1;
            self.list_state.select(Some(self.selected));
        }
    }

    /// Move selection up (to newer entries)
    pub fn select_previous(&mut self) {
        self.selected = self.selected.saturating_sub(1);
        self.list_state.select(Some(self.selected));
    }

    /// Jump to most recent entry
    pub fn select_first(&mut self) {
        self.selected = 0;
        self.list_state.select(Some(0));
    }

    /// Jump to oldest entry
    pub fn select_last(&mut self) {
        if !self.entries.is_empty() {
            self.selected = self.entries.len() - 1;
            self.list_state.select(Some(self.selected));
        }
    }

    /// Get the height this panel wants (for layout)
    pub fn preferred_height(&self) -> u16 {
        if self.collapsed {
            1
        } else {
            // Show a few entries by default
            5
        }
    }

    /// Render the log panel
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        if self.collapsed {
            self.render_collapsed(frame, area);
        } else {
            self.render_expanded(frame, area);
        }
    }

    /// Render collapsed view (single line summary)
    fn render_collapsed(&self, frame: &mut Frame, area: Rect) {
        let latest = self.entries.front().map(|e| e.display());
        let text = match latest {
            Some(msg) => format!(
                " Log: {} ({} entries) [L to expand]",
                msg,
                self.entries.len()
            ),
            None => " Log: (empty) [L to expand]".to_string(),
        };

        let style = if self.focused {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let paragraph = Paragraph::new(text).style(style);
        frame.render_widget(paragraph, area);
    }

    /// Render expanded view with entry list
    fn render_expanded(&mut self, frame: &mut Frame, area: Rect) {
        if self.entries.is_empty() {
            let empty = Paragraph::new(" No activity yet")
                .style(Style::default().fg(Color::DarkGray))
                .block(
                    Block::default()
                        .borders(Borders::TOP)
                        .border_style(Style::default().fg(Color::DarkGray))
                        .title(" Recent Activity [L to collapse] "),
                );
            frame.render_widget(empty, area);
            return;
        }

        // Calculate available width for message
        // Format: " 10s + task bn-xxxx created"
        let msg_width = area.width.saturating_sub(8) as usize;

        let items: Vec<ListItem> = self
            .entries
            .iter()
            .enumerate()
            .take(area.height as usize - 1) // Account for border
            .map(|(idx, entry)| {
                let time = entry.relative_time();
                let icon = entry.icon();
                let color = entry.color();
                let msg = entry.display();

                // Truncate message if needed
                let display_msg = if msg.len() > msg_width {
                    format!("{}...", &msg[..msg_width.saturating_sub(3)])
                } else {
                    msg
                };

                let style = if idx == self.selected && self.focused {
                    Style::default().bg(Color::DarkGray)
                } else {
                    Style::default()
                };

                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!(" {:>3} ", time),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(icon, Style::default().fg(color)),
                    Span::raw(" "),
                    Span::styled(display_msg, style),
                ]))
            })
            .collect();

        let title = if self.focused {
            " Recent Activity [L to collapse] "
        } else {
            " Recent Activity "
        };

        let border_style = if self.focused {
            Style::default().fg(Color::Blue)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let list = List::new(items).block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(border_style)
                .title(title),
        );

        frame.render_widget(list, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_entry_creation() {
        let entry = LogEntry::new("connected");
        assert_eq!(entry.action, "connected");
        assert_eq!(entry.icon(), "●");
        assert_eq!(entry.color(), Color::Green);
    }

    #[test]
    fn test_entity_event() {
        let entry = LogEntry::entity_event("task", "bn-1234", "created");
        assert_eq!(entry.entity_type, Some("task".to_string()));
        assert_eq!(entry.entity_id, Some("bn-1234".to_string()));
        assert_eq!(entry.action, "created");
    }

    #[test]
    fn test_log_panel() {
        let mut panel = LogPanelView::new();
        assert!(panel.is_empty());

        panel.log("connected");
        assert_eq!(panel.entry_count(), 1);
        assert!(!panel.is_empty());

        panel.log_entity("task", "bn-1234", "created");
        assert_eq!(panel.entry_count(), 2);
    }

    #[test]
    fn test_log_panel_max_entries() {
        let mut panel = LogPanelView::new();
        for i in 0..(MAX_LOG_ENTRIES + 10) {
            panel.log(format!("entry {}", i));
        }
        assert_eq!(panel.entry_count(), MAX_LOG_ENTRIES);
    }
}
