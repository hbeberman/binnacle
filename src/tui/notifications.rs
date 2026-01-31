//! Notifications module for the TUI
//!
//! Provides a toast notification system with auto-dismiss, overflow handling,
//! and notification history.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};

/// Maximum number of toasts to display at once
const MAX_VISIBLE_TOASTS: usize = 3;

/// Default auto-dismiss duration in seconds
const DEFAULT_DISMISS_SECONDS: u64 = 5;

/// Maximum history entries to keep
const MAX_HISTORY_ENTRIES: usize = 100;

/// Notification level (determines styling and bell behavior)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationLevel {
    /// Informational message
    Info,
    /// Success message (task completed, etc.)
    Success,
    /// Warning message
    Warning,
    /// Error message
    Error,
}

impl NotificationLevel {
    /// Get ANSI color for this level
    pub fn color(&self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            NotificationLevel::Info => Color::Blue,
            NotificationLevel::Success => Color::Green,
            NotificationLevel::Warning => Color::Yellow,
            NotificationLevel::Error => Color::Red,
        }
    }

    /// Get icon/prefix for this level
    pub fn icon(&self) -> &'static str {
        match self {
            NotificationLevel::Info => "ℹ",
            NotificationLevel::Success => "✓",
            NotificationLevel::Warning => "⚠",
            NotificationLevel::Error => "✗",
        }
    }

    /// Whether this level should trigger a bell
    pub fn should_bell(&self) -> bool {
        matches!(self, NotificationLevel::Warning | NotificationLevel::Error)
    }
}

/// A single toast notification
#[derive(Debug, Clone)]
pub struct Toast {
    /// Unique ID for this toast
    pub id: u64,
    /// Notification level
    pub level: NotificationLevel,
    /// Message content
    pub message: String,
    /// When the toast was created
    pub created_at: Instant,
    /// How long before auto-dismiss (None = manual dismiss only)
    pub duration: Option<Duration>,
    /// Whether this toast has been dismissed
    pub dismissed: bool,
}

impl Toast {
    /// Create a new toast
    pub fn new(id: u64, level: NotificationLevel, message: impl Into<String>) -> Self {
        Self {
            id,
            level,
            message: message.into(),
            created_at: Instant::now(),
            duration: Some(Duration::from_secs(DEFAULT_DISMISS_SECONDS)),
            dismissed: false,
        }
    }

    /// Create a toast that won't auto-dismiss
    pub fn sticky(id: u64, level: NotificationLevel, message: impl Into<String>) -> Self {
        Self {
            id,
            level,
            message: message.into(),
            created_at: Instant::now(),
            duration: None,
            dismissed: false,
        }
    }

    /// Check if this toast should be dismissed due to timeout
    pub fn is_expired(&self) -> bool {
        if let Some(duration) = self.duration {
            self.created_at.elapsed() >= duration
        } else {
            false
        }
    }

    /// Mark this toast as dismissed
    pub fn dismiss(&mut self) {
        self.dismissed = true;
    }
}

/// Entry in the notification history
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    /// Notification level
    pub level: NotificationLevel,
    /// Message content
    pub message: String,
    /// When the notification was received
    pub timestamp: DateTime<Utc>,
}

impl HistoryEntry {
    /// Create from a toast
    pub fn from_toast(toast: &Toast) -> Self {
        Self {
            level: toast.level,
            message: toast.message.clone(),
            timestamp: Utc::now(),
        }
    }

    /// Format relative time since notification
    pub fn relative_time(&self) -> String {
        let now = Utc::now();
        let duration = now.signed_duration_since(self.timestamp);

        if duration.num_seconds() < 60 {
            "just now".to_string()
        } else if duration.num_minutes() < 60 {
            format!("{}m ago", duration.num_minutes())
        } else if duration.num_hours() < 24 {
            format!("{}h ago", duration.num_hours())
        } else {
            format!("{}d ago", duration.num_days())
        }
    }
}

/// Notification manager - handles toasts and history
#[derive(Debug)]
pub struct NotificationManager {
    /// Active toasts (newest first)
    toasts: VecDeque<Toast>,
    /// Notification history
    history: VecDeque<HistoryEntry>,
    /// Next toast ID
    next_id: u64,
    /// Whether bell is enabled
    pub bell_enabled: bool,
    /// Whether history overlay is visible
    pub history_visible: bool,
    /// Selected history index (for navigation)
    pub history_selected: usize,
    /// Count of pending (overflow) toasts not displayed
    pub overflow_count: usize,
}

impl Default for NotificationManager {
    fn default() -> Self {
        Self::new()
    }
}

impl NotificationManager {
    /// Create a new notification manager
    pub fn new() -> Self {
        Self {
            toasts: VecDeque::new(),
            history: VecDeque::new(),
            next_id: 1,
            bell_enabled: true,
            history_visible: false,
            history_selected: 0,
            overflow_count: 0,
        }
    }

    /// Add a new notification
    pub fn notify(&mut self, level: NotificationLevel, message: impl Into<String>) {
        let toast = Toast::new(self.next_id, level, message);
        self.next_id += 1;

        // Add to history
        self.history.push_front(HistoryEntry::from_toast(&toast));
        if self.history.len() > MAX_HISTORY_ENTRIES {
            self.history.pop_back();
        }

        // Add to active toasts
        self.toasts.push_front(toast);

        // Update overflow count
        self.update_overflow();

        // TODO: Trigger bell if enabled and level warrants it
    }

    /// Add an info notification
    pub fn info(&mut self, message: impl Into<String>) {
        self.notify(NotificationLevel::Info, message);
    }

    /// Add a success notification
    pub fn success(&mut self, message: impl Into<String>) {
        self.notify(NotificationLevel::Success, message);
    }

    /// Add a warning notification
    pub fn warning(&mut self, message: impl Into<String>) {
        self.notify(NotificationLevel::Warning, message);
    }

    /// Add an error notification
    pub fn error(&mut self, message: impl Into<String>) {
        self.notify(NotificationLevel::Error, message);
    }

    /// Remove expired and dismissed toasts
    pub fn cleanup(&mut self) {
        self.toasts.retain(|t| !t.dismissed && !t.is_expired());
        self.update_overflow();
    }

    /// Dismiss the oldest visible toast
    pub fn dismiss_oldest(&mut self) {
        // Find the oldest non-dismissed toast among visible ones
        let visible_count = self.toasts.len().min(MAX_VISIBLE_TOASTS);
        if visible_count > 0 {
            // Dismiss from the end (oldest of the visible ones)
            let dismiss_idx = visible_count - 1;
            if let Some(toast) = self.toasts.get_mut(dismiss_idx) {
                toast.dismiss();
            }
        }
        self.cleanup();
    }

    /// Dismiss all toasts
    pub fn dismiss_all(&mut self) {
        for toast in &mut self.toasts {
            toast.dismiss();
        }
        self.cleanup();
    }

    /// Get visible toasts (limited by MAX_VISIBLE_TOASTS)
    pub fn visible_toasts(&self) -> impl Iterator<Item = &Toast> {
        self.toasts.iter().take(MAX_VISIBLE_TOASTS)
    }

    /// Check if there are any visible toasts
    pub fn has_toasts(&self) -> bool {
        !self.toasts.is_empty()
    }

    /// Update overflow count
    fn update_overflow(&mut self) {
        self.overflow_count = self.toasts.len().saturating_sub(MAX_VISIBLE_TOASTS);
    }

    /// Toggle history overlay visibility
    pub fn toggle_history(&mut self) {
        self.history_visible = !self.history_visible;
        if self.history_visible {
            self.history_selected = 0;
        }
    }

    /// Close history overlay
    pub fn close_history(&mut self) {
        self.history_visible = false;
    }

    /// Get history entries
    pub fn history(&self) -> impl Iterator<Item = &HistoryEntry> {
        self.history.iter()
    }

    /// Check if history is empty
    pub fn history_is_empty(&self) -> bool {
        self.history.is_empty()
    }

    /// Navigate history selection down
    pub fn history_next(&mut self) {
        if self.history_selected < self.history.len().saturating_sub(1) {
            self.history_selected += 1;
        }
    }

    /// Navigate history selection up
    pub fn history_previous(&mut self) {
        self.history_selected = self.history_selected.saturating_sub(1);
    }

    /// Clear all history
    pub fn clear_history(&mut self) {
        self.history.clear();
        self.history_selected = 0;
    }

    /// Should terminal bell be rung? Returns true once and resets.
    pub fn should_ring_bell(&self) -> bool {
        if !self.bell_enabled {
            return false;
        }
        // Check if any recent toast should trigger bell
        self.toasts
            .front()
            .map(|t| t.level.should_bell() && t.created_at.elapsed() < Duration::from_millis(100))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_levels() {
        assert!(NotificationLevel::Error.should_bell());
        assert!(NotificationLevel::Warning.should_bell());
        assert!(!NotificationLevel::Info.should_bell());
        assert!(!NotificationLevel::Success.should_bell());
    }

    #[test]
    fn test_toast_expiry() {
        let toast = Toast::new(1, NotificationLevel::Info, "test");
        assert!(!toast.is_expired());

        let mut sticky = Toast::sticky(2, NotificationLevel::Info, "sticky");
        assert!(!sticky.is_expired());
        sticky.dismiss();
        assert!(sticky.dismissed);
    }

    #[test]
    fn test_notification_manager() {
        let mut manager = NotificationManager::new();
        assert!(!manager.has_toasts());

        manager.info("Test message");
        assert!(manager.has_toasts());
        assert_eq!(manager.visible_toasts().count(), 1);

        // Add more than MAX_VISIBLE_TOASTS
        for i in 0..5 {
            manager.info(format!("Message {}", i));
        }
        assert_eq!(manager.visible_toasts().count(), MAX_VISIBLE_TOASTS);
        assert!(manager.overflow_count > 0);
    }

    #[test]
    fn test_history() {
        let mut manager = NotificationManager::new();
        manager.info("Message 1");
        manager.warning("Message 2");

        assert!(!manager.history_is_empty());
        assert_eq!(manager.history().count(), 2);
    }
}
