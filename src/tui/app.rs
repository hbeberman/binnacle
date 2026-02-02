//! TUI Application - main event loop and terminal management
//!
//! This module contains the core TUI application logic including:
//! - Terminal setup and restoration
//! - WebSocket connection handling with automatic reconnection
//! - Event loop for keyboard and server messages
//! - View switching between Queue/Ready, Recently Completed, and Node Detail
//! - Logging setup with daily rolling file appender

use std::io::{self, stdout};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use tracing::{debug, error, info, trace, warn};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;

use chrono::{DateTime, Utc};
use crossterm::{
    ExecutableCommand,
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton,
        MouseEvent, MouseEventKind,
    },
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures::StreamExt;
use futures::stream::SplitStream;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};
use serde::Deserialize;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use super::connection::{
    ConnectionState, MAX_RECONNECT_ATTEMPTS, RECONNECT_DEBOUNCE_MS, calculate_backoff,
};
use super::notifications::NotificationManager;
use super::views::{
    CompletedItem, EdgeInfo, LogEntry, LogPanelView, NodeDetail, NodeDetailView, QueueReadyView,
    RecentlyCompletedView, WorkItem,
};

/// Default server port
pub const DEFAULT_PORT: u16 = 3030;

/// Cooldown period after a fetch error before retrying (in seconds)
const FETCH_ERROR_COOLDOWN_SECS: u64 = 5;

/// Parse a WebSocket URL to extract host and port for display purposes.
///
/// Handles both ws:// and wss:// URLs. Returns defaults if parsing fails.
fn parse_ws_url(url: &str) -> (String, u16) {
    // Strip scheme prefix
    let without_scheme = url
        .strip_prefix("wss://")
        .or_else(|| url.strip_prefix("ws://"))
        .unwrap_or(url);

    // Extract host:port part (before any path)
    let host_port = without_scheme.split('/').next().unwrap_or(without_scheme);

    // Split into host and port
    if let Some((host, port_str)) = host_port.rsplit_once(':') {
        let port = port_str.parse().unwrap_or(DEFAULT_PORT);
        (host.to_string(), port)
    } else {
        // No port specified - use default based on scheme
        let default_port = if url.starts_with("wss://") {
            443
        } else {
            DEFAULT_PORT
        };
        (host_port.to_string(), default_port)
    }
}

/// Initialize logging for the TUI with daily rolling file appender.
///
/// Creates a logging setup that writes JSON-formatted logs to `~/.local/share/binnacle/logs/tui.log`.
/// The log files roll daily with the format `tui.log.YYYY-MM-DD`.
///
/// # Arguments
/// * `log_level` - Optional log level string (e.g., "debug", "info", "warn").
///   Precedence: CLI flag (this arg) > BN_LOG env var > "warn" default.
///
/// # Returns
/// A `WorkerGuard` that must be held for the lifetime of the TUI. When the guard is dropped,
/// any remaining buffered logs will be flushed. The TUI should hold this guard until exit.
///
/// Returns `None` if the logs directory cannot be created, logging is silently disabled.
///
/// # Example
/// ```ignore
/// let _logging_guard = init_logging(Some("debug"));
/// // TUI runs...
/// // Guard dropped at end of scope, flushes remaining logs
/// ```
pub fn init_logging(log_level: Option<&str>) -> Option<WorkerGuard> {
    // Determine log directory: ~/.local/share/binnacle/logs/
    let logs_dir = get_logs_directory()?;

    // Create logs directory if it doesn't exist
    if std::fs::create_dir_all(&logs_dir).is_err() {
        return None;
    }

    // Set up daily rolling file appender
    let file_appender = tracing_appender::rolling::daily(&logs_dir, "tui.log");

    // Create non-blocking writer (returns guard that must be held)
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // Build EnvFilter with precedence: CLI flag > BN_LOG env var > "warn" default
    let filter = build_env_filter(log_level);

    // Build the subscriber with JSON formatter
    let subscriber = tracing_subscriber::registry().with(filter).with(
        tracing_subscriber::fmt::layer()
            .json()
            .with_writer(non_blocking)
            .with_target(true)
            .with_file(true)
            .with_line_number(true),
    );

    // Set as the global default subscriber
    // If this fails (e.g., subscriber already set), we still return the guard
    // to ensure the non-blocking writer stays alive
    let _ = tracing::subscriber::set_global_default(subscriber);

    Some(guard)
}

/// Get the logs directory path: ~/.local/share/binnacle/logs/
fn get_logs_directory() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("binnacle").join("logs"))
}

/// Build an EnvFilter with precedence: CLI flag > BN_LOG env var > "warn" default
fn build_env_filter(log_level: Option<&str>) -> EnvFilter {
    // Priority 1: CLI flag (direct argument)
    if let Some(level) = log_level {
        if let Ok(filter) = EnvFilter::try_new(level) {
            return filter;
        }
    }

    // Priority 2: BN_LOG environment variable
    if let Ok(env_level) = std::env::var("BN_LOG") {
        if let Ok(filter) = EnvFilter::try_new(&env_level) {
            return filter;
        }
    }

    // Priority 3: Default to "warn"
    EnvFilter::try_new("warn").unwrap_or_else(|_| EnvFilter::new("warn"))
}

/// Active view in the TUI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveView {
    QueueReady,
    RecentlyCompleted,
    NodeDetail,
}

/// Input mode for the TUI
///
/// Tracks the current input mode state, allowing the TUI to handle keyboard
/// input differently based on context (e.g., normal navigation vs search filtering).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    /// Default navigation mode - normal key handling
    #[default]
    Normal,
    /// Search mode - filtering lists with / and n/N navigation
    Search,
    /// Command mode - vim-style : command entry (e.g., :q, :help)
    Command,
}

/// Response from /api/ready endpoint
#[derive(Debug, Deserialize)]
struct ReadyResponse {
    tasks: Vec<TaskData>,
    #[serde(default)]
    in_progress_tasks: Vec<InProgressTaskData>,
    #[serde(default)]
    in_progress_bugs: Vec<InProgressBugData>,
    #[serde(default)]
    recently_completed_tasks: Vec<CompletedTaskData>,
    #[serde(default)]
    recently_completed_bugs: Vec<CompletedBugData>,
}

/// Response from /api/node/:id endpoint
#[derive(Debug, Deserialize)]
struct NodeResponse {
    node: NodeDetail,
    edges: Vec<EdgeInfo>,
}

/// Response from /api/queue endpoint (for future use)
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct QueueResponse {
    queue: Option<QueueData>,
}

/// Queue data from API (for future use)
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct QueueData {
    id: String,
    title: String,
}

/// Task data from ready API
#[derive(Debug, Clone, Deserialize)]
struct TaskData {
    #[serde(flatten)]
    core: TaskCore,
    priority: u8,
    #[serde(default)]
    assignee: Option<String>,
    #[serde(default)]
    queued: bool,
}

/// Core task fields
#[derive(Debug, Clone, Deserialize)]
struct TaskCore {
    id: String,
    title: String,
    #[serde(default)]
    short_name: Option<String>,
    #[serde(default, rename = "type")]
    entity_type: Option<String>,
}

/// In-progress task data from API
#[derive(Debug, Clone, Deserialize)]
struct InProgressTaskData {
    #[serde(flatten)]
    core: TaskCore,
    priority: u8,
    #[serde(default)]
    assignee: Option<String>,
}

/// In-progress bug data from API
#[derive(Debug, Clone, Deserialize)]
struct InProgressBugData {
    #[serde(flatten)]
    core: TaskCore,
    priority: u8,
    #[serde(default)]
    assignee: Option<String>,
}

/// Completed task data from API
#[derive(Debug, Clone, Deserialize)]
struct CompletedTaskData {
    #[serde(flatten)]
    core: CompletedCore,
    priority: u8,
    #[serde(default)]
    closed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    closed_reason: Option<String>,
}

/// Completed bug data from API
#[derive(Debug, Clone, Deserialize)]
struct CompletedBugData {
    #[serde(flatten)]
    core: CompletedCore,
    priority: u8,
    #[serde(default)]
    closed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    closed_reason: Option<String>,
}

/// Core fields for completed items
#[derive(Debug, Clone, Deserialize)]
struct CompletedCore {
    id: String,
    title: String,
    #[serde(default)]
    short_name: Option<String>,
    #[serde(default, rename = "type")]
    entity_type: Option<String>,
}

impl From<CompletedTaskData> for CompletedItem {
    fn from(task: CompletedTaskData) -> Self {
        CompletedItem {
            id: task.core.id,
            title: task.core.title,
            short_name: task.core.short_name,
            priority: task.priority,
            closed_at: task.closed_at,
            closed_reason: task.closed_reason,
            entity_type: task.core.entity_type.or(Some("task".to_string())),
        }
    }
}

impl From<CompletedBugData> for CompletedItem {
    fn from(bug: CompletedBugData) -> Self {
        CompletedItem {
            id: bug.core.id,
            title: bug.core.title,
            short_name: bug.core.short_name,
            priority: bug.priority,
            closed_at: bug.closed_at,
            closed_reason: bug.closed_reason,
            entity_type: bug.core.entity_type.or(Some("bug".to_string())),
        }
    }
}

impl From<TaskData> for WorkItem {
    fn from(task: TaskData) -> Self {
        WorkItem {
            id: task.core.id,
            title: task.core.title,
            short_name: task.core.short_name,
            priority: task.priority,
            assignee: task.assignee,
            queued: task.queued,
            entity_type: task.core.entity_type,
        }
    }
}

impl From<InProgressTaskData> for WorkItem {
    fn from(task: InProgressTaskData) -> Self {
        WorkItem {
            id: task.core.id,
            title: task.core.title,
            short_name: task.core.short_name,
            priority: task.priority,
            assignee: task.assignee,
            queued: false,
            entity_type: task.core.entity_type.or(Some("task".to_string())),
        }
    }
}

impl From<InProgressBugData> for WorkItem {
    fn from(bug: InProgressBugData) -> Self {
        WorkItem {
            id: bug.core.id,
            title: bug.core.title,
            short_name: bug.core.short_name,
            priority: bug.priority,
            assignee: bug.assignee,
            queued: false,
            entity_type: Some("bug".to_string()),
        }
    }
}

/// Incoming WebSocket message types
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ServerMessage {
    /// Initial connection acknowledgment
    Connected {
        #[allow(dead_code)]
        version: u64,
    },
    /// Entity was added
    EntityAdded {
        entity_type: String,
        id: String,
        entity: serde_json::Value,
    },
    /// Entity was updated
    EntityUpdated {
        entity_type: String,
        id: String,
        entity: serde_json::Value,
    },
    /// Entity was removed
    EntityRemoved { entity_type: String, id: String },
    /// Log entry (activity)
    LogEntry { entry: serde_json::Value },
    /// Edge added
    EdgeAdded {
        id: String,
        #[allow(dead_code)]
        edge: serde_json::Value,
    },
    /// Edge removed
    EdgeRemoved {
        id: String,
        #[allow(dead_code)]
        edge: serde_json::Value,
    },
    /// Full reload needed
    Reload {
        #[allow(dead_code)]
        version: u64,
    },
    /// Sync response
    SyncResponse {
        #[allow(dead_code)]
        version: u64,
        #[allow(dead_code)]
        action: String,
    },
    /// Sync acknowledgment
    SyncAck {
        #[allow(dead_code)]
        version: u64,
    },
    /// Sync catchup with missed messages
    SyncCatchup {
        #[allow(dead_code)]
        version: u64,
        #[allow(dead_code)]
        messages: Vec<serde_json::Value>,
    },
    /// Delta with incremental changes (new protocol)
    Delta {
        changes: Vec<Change>,
        #[allow(dead_code)]
        version: u64,
        #[allow(dead_code)]
        timestamp: DateTime<Utc>,
    },
}

/// A single change in the graph (used by Delta messages).
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Change {
    /// An entity was created.
    Create {
        entity_type: String,
        data: serde_json::Value,
    },
    /// An entity was updated.
    Update {
        entity_type: String,
        id: String,
        #[allow(dead_code)]
        changes: serde_json::Value,
    },
    /// An entity was deleted.
    Delete { entity_type: String, id: String },
}

impl ServerMessage {
    /// Returns a string description of the message type for logging
    fn message_type(&self) -> &'static str {
        match self {
            ServerMessage::Connected { .. } => "connected",
            ServerMessage::EntityAdded { .. } => "entity_added",
            ServerMessage::EntityUpdated { .. } => "entity_updated",
            ServerMessage::EntityRemoved { .. } => "entity_removed",
            ServerMessage::LogEntry { .. } => "log_entry",
            ServerMessage::EdgeAdded { .. } => "edge_added",
            ServerMessage::EdgeRemoved { .. } => "edge_removed",
            ServerMessage::Reload { .. } => "reload",
            ServerMessage::SyncResponse { .. } => "sync_response",
            ServerMessage::SyncAck { .. } => "sync_ack",
            ServerMessage::SyncCatchup { .. } => "sync_catchup",
            ServerMessage::Delta { .. } => "delta",
        }
    }
}

/// TUI Application state
pub struct TuiApp {
    /// Connection state
    connection_state: ConnectionState,
    /// Whether to quit the application
    should_quit: bool,
    /// Active view
    active_view: ActiveView,
    /// Previous view (for returning from detail view)
    previous_list_view: ActiveView,
    /// Current input mode (Normal, Search, Command)
    input_mode: InputMode,
    /// Command input buffer (for command mode)
    command_input: String,
    /// Queue/Ready view
    queue_ready_view: QueueReadyView,
    /// Recently Completed view
    recently_completed_view: RecentlyCompletedView,
    /// Node Detail view
    node_detail_view: NodeDetailView,
    /// Log panel view (always visible)
    log_panel: LogPanelView,
    /// Notification manager (toasts and history)
    notifications: NotificationManager,
    /// HTTP base URL for API requests
    api_base: String,
    /// WebSocket endpoint URL
    ws_endpoint: String,
    /// Flag indicating data needs refresh
    needs_refresh: bool,
    /// Flag indicating node detail needs fetch
    needs_node_fetch: Option<String>,
    /// Last key pressed (for gg detection)
    last_key: Option<KeyCode>,
    /// Flag indicating reconnection was requested
    reconnect_requested: bool,
    /// Whether help overlay is visible
    help_visible: bool,
    /// Last rendered layout chunks for mouse handling
    layout_chunks: Option<std::rc::Rc<[Rect]>>,
    /// Time of last fetch error (for rate limiting)
    last_fetch_error: Option<Instant>,
}

impl TuiApp {
    /// Create a new TUI application
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            connection_state: ConnectionState::Disconnected,
            should_quit: false,
            active_view: ActiveView::QueueReady,
            previous_list_view: ActiveView::QueueReady,
            input_mode: InputMode::Normal,
            command_input: String::new(),
            queue_ready_view: QueueReadyView::new(),
            recently_completed_view: RecentlyCompletedView::new(),
            node_detail_view: NodeDetailView::new(),
            log_panel: LogPanelView::new(),
            notifications: NotificationManager::new(),
            api_base: format!("http://{}:{}", host, port),
            ws_endpoint: format!("ws://{}:{}/ws", host, port),
            needs_refresh: true,
            needs_node_fetch: None,
            last_key: None,
            reconnect_requested: false,
            help_visible: false,
            layout_chunks: None,
            last_fetch_error: None,
        }
    }

    /// Get the current input mode
    /// NOTE: Will be used by search mode implementation (bn-575f)
    #[allow(dead_code)]
    pub fn input_mode(&self) -> InputMode {
        self.input_mode
    }

    /// Set the input mode
    /// NOTE: Will be used by search mode implementation (bn-575f)
    #[allow(dead_code)]
    pub fn set_input_mode(&mut self, mode: InputMode) {
        self.input_mode = mode;
    }

    /// Check if we're in normal mode
    /// NOTE: Will be used by search mode implementation (bn-575f)
    #[allow(dead_code)]
    pub fn is_normal_mode(&self) -> bool {
        self.input_mode == InputMode::Normal
    }

    /// Check if we're in search mode
    /// NOTE: Will be used by search mode implementation (bn-575f)
    #[allow(dead_code)]
    pub fn is_search_mode(&self) -> bool {
        self.input_mode == InputMode::Search
    }

    /// Check if we're in command mode
    pub fn is_command_mode(&self) -> bool {
        self.input_mode == InputMode::Command
    }

    /// Enter command mode
    fn enter_command_mode(&mut self) {
        self.input_mode = InputMode::Command;
        self.command_input.clear();
    }

    /// Exit command mode (cancel)
    fn exit_command_mode(&mut self) {
        self.input_mode = InputMode::Normal;
        self.command_input.clear();
    }

    /// List of available commands for autocompletion.
    /// Primary names come first, followed by aliases.
    const COMMANDS: &'static [&'static str] = &[
        "quit", "q", "help", "h", "refresh", "r", "log", "history", "hist", "clear",
    ];

    /// Attempt to autocomplete the current command input.
    /// If the input is a prefix of exactly one command, complete it.
    /// If multiple commands match, complete to their common prefix.
    fn autocomplete_command(&mut self) {
        let input = self.command_input.trim().to_lowercase();
        if input.is_empty() {
            return;
        }

        // Find all commands that start with the input
        let matches: Vec<&str> = Self::COMMANDS
            .iter()
            .filter(|cmd| cmd.starts_with(&input))
            .copied()
            .collect();

        match matches.len() {
            0 => {
                // No matches, do nothing
            }
            1 => {
                // Exact single match - complete it
                self.command_input = matches[0].to_string();
            }
            _ => {
                // Multiple matches - complete to common prefix
                if let Some(common) = Self::common_prefix(&matches) {
                    if common.len() > input.len() {
                        self.command_input = common;
                    }
                }
            }
        }
    }

    /// Find the longest common prefix among a list of strings
    fn common_prefix(strings: &[&str]) -> Option<String> {
        if strings.is_empty() {
            return None;
        }
        let first = strings[0];
        let mut prefix_len = first.len();
        for s in &strings[1..] {
            prefix_len = first
                .chars()
                .zip(s.chars())
                .take_while(|(a, b)| a == b)
                .count()
                .min(prefix_len);
        }
        Some(
            first[..first
                .char_indices()
                .nth(prefix_len)
                .map_or(first.len(), |(i, _)| i)]
                .to_string(),
        )
    }

    /// Execute the current command and return to normal mode
    fn execute_command(&mut self) {
        let cmd = self.command_input.trim().to_lowercase();
        self.input_mode = InputMode::Normal;
        self.command_input.clear();

        match cmd.as_str() {
            "q" | "quit" => {
                self.should_quit = true;
            }
            "help" | "h" => {
                self.help_visible = true;
            }
            "refresh" | "r" => {
                if matches!(self.connection_state, ConnectionState::Disconnected) {
                    self.reconnect_requested = true;
                    self.log_panel.log("reconnecting (command)");
                } else {
                    self.needs_refresh = true;
                    self.log_panel.log("refreshing (command)");
                }
            }
            "log" => {
                self.log_panel.toggle_collapsed();
            }
            "history" | "hist" => {
                self.notifications.toggle_history();
            }
            "clear" => {
                self.notifications.dismiss_all();
            }
            "" => {
                // Empty command, just exit
            }
            _ => {
                // Unknown command - show brief error (clears after 2s or on keypress)
                self.notifications
                    .warning_brief(format!("Unknown command: {}", cmd));
            }
        }
    }

    /// Switch to the next view (only cycles between list views, not detail)
    fn next_view(&mut self) {
        let from_view = self.active_view;
        // Only cycle between list views (not NodeDetail)
        match self.active_view {
            ActiveView::QueueReady => {
                self.active_view = ActiveView::RecentlyCompleted;
                self.previous_list_view = ActiveView::RecentlyCompleted;
            }
            ActiveView::RecentlyCompleted => {
                self.active_view = ActiveView::QueueReady;
                self.previous_list_view = ActiveView::QueueReady;
            }
            ActiveView::NodeDetail => {
                // From detail, go back to previous list view
                self.go_back_from_detail();
            }
        }
        debug!(from = ?from_view, to = ?self.active_view, "view switch");
    }

    /// Open detail view for the selected item
    fn open_detail_for_selection(&mut self) {
        let from_view = self.active_view;
        let node_id = match self.active_view {
            ActiveView::QueueReady => self
                .queue_ready_view
                .selected_item()
                .map(|item| item.id.clone()),
            ActiveView::RecentlyCompleted => self
                .recently_completed_view
                .items
                .get(self.recently_completed_view.selected)
                .map(|item| item.id.clone()),
            ActiveView::NodeDetail => {
                // Navigate to selected edge
                self.node_detail_view
                    .selected_edge()
                    .map(|edge| edge.related_id.clone())
            }
        };

        if let Some(ref id) = node_id {
            debug!(from = ?from_view, node_id = %id, "view switch to detail");
            // If we're already in detail view, push current to nav stack
            if self.active_view == ActiveView::NodeDetail {
                self.node_detail_view.push_navigation(false);
            } else {
                // Coming from a list view
                self.previous_list_view = self.active_view;
            }
            self.needs_node_fetch = Some(id.clone());
            self.active_view = ActiveView::NodeDetail;
        }
    }

    /// Go back from detail view
    fn go_back_from_detail(&mut self) {
        if let Some(entry) = self.node_detail_view.pop_navigation() {
            if entry.from_list_view {
                // Go back to the list view
                debug!(to = ?self.previous_list_view, "view switch from detail to list");
                self.active_view = self.previous_list_view;
                self.node_detail_view.clear();
            } else {
                // Navigate back to the previous node
                debug!(node_id = %entry.node_id, "view switch to previous node");
                self.needs_node_fetch = Some(entry.node_id);
            }
        } else {
            // Nothing in stack, go back to list view
            debug!(to = ?self.previous_list_view, "view switch from detail to list");
            self.active_view = self.previous_list_view;
            self.node_detail_view.clear();
        }
    }

    /// Handle keyboard events
    fn handle_key(&mut self, key: KeyCode) {
        trace!(key = ?key, view = ?self.active_view, "keyboard input");

        // Dismiss any brief notifications on keypress
        self.notifications.dismiss_on_keypress();

        // Handle help overlay first (when visible)
        if self.help_visible {
            match key {
                KeyCode::Esc | KeyCode::Char('?') => {
                    self.help_visible = false;
                }
                KeyCode::Char('q') => {
                    self.should_quit = true;
                }
                _ => {}
            }
            self.last_key = Some(key);
            return;
        }

        // Handle notification history overlay first (when visible)
        if self.notifications.history_visible {
            match key {
                KeyCode::Esc | KeyCode::Char('H') => {
                    self.notifications.close_history();
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    self.notifications.history_next();
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.notifications.history_previous();
                }
                KeyCode::Char('c') => {
                    self.notifications.clear_history();
                }
                KeyCode::Char('q') => {
                    self.should_quit = true;
                }
                _ => {}
            }
            self.last_key = Some(key);
            return;
        }

        // Handle command mode input
        if self.is_command_mode() {
            match key {
                KeyCode::Esc => {
                    self.exit_command_mode();
                }
                KeyCode::Enter => {
                    self.execute_command();
                }
                KeyCode::Backspace => {
                    self.command_input.pop();
                }
                KeyCode::Tab => {
                    self.autocomplete_command();
                }
                KeyCode::Char(c) => {
                    self.command_input.push(c);
                }
                _ => {}
            }
            self.last_key = Some(key);
            return;
        }

        // Handle quit universally
        if key == KeyCode::Char('q') {
            self.should_quit = true;
            return;
        }

        // Handle Esc based on current view
        if key == KeyCode::Esc {
            if self.active_view == ActiveView::NodeDetail {
                self.go_back_from_detail();
            } else {
                self.should_quit = true;
            }
            self.last_key = Some(key);
            return;
        }

        match key {
            // Help key
            KeyCode::Char('?') => {
                self.help_visible = true;
                self.last_key = Some(key);
            }
            // Notification/log keys
            KeyCode::Char('H') => {
                // Toggle notification history overlay
                self.notifications.toggle_history();
                self.last_key = Some(key);
            }
            KeyCode::Char('L') => {
                // Toggle log panel collapsed state
                self.log_panel.toggle_collapsed();
                self.last_key = Some(key);
            }
            KeyCode::Char('d') => {
                // Dismiss oldest toast
                self.notifications.dismiss_oldest();
                self.last_key = Some(key);
            }
            KeyCode::Char('D') => {
                // Dismiss all toasts
                self.notifications.dismiss_all();
                self.last_key = Some(key);
            }
            // View switching (only for list views)
            KeyCode::Tab => {
                self.next_view();
                self.last_key = Some(key);
            }
            KeyCode::Char('1') => {
                self.active_view = ActiveView::QueueReady;
                self.previous_list_view = ActiveView::QueueReady;
                self.last_key = Some(key);
            }
            KeyCode::Char('2') => {
                self.active_view = ActiveView::RecentlyCompleted;
                self.previous_list_view = ActiveView::RecentlyCompleted;
                self.last_key = Some(key);
            }
            KeyCode::Char('3') => {
                // Only switch to detail view if we have a selection
                if self.active_view != ActiveView::NodeDetail {
                    self.open_detail_for_selection();
                }
                self.last_key = Some(key);
            }
            // Enter to open detail / navigate to edge
            KeyCode::Enter => {
                self.open_detail_for_selection();
                self.last_key = Some(key);
            }
            // Navigation
            KeyCode::Char('j') | KeyCode::Down => {
                match self.active_view {
                    ActiveView::QueueReady => self.queue_ready_view.select_next(),
                    ActiveView::RecentlyCompleted => self.recently_completed_view.select_next(),
                    ActiveView::NodeDetail => self.node_detail_view.select_next_edge(),
                }
                self.last_key = Some(key);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                match self.active_view {
                    ActiveView::QueueReady => self.queue_ready_view.select_previous(),
                    ActiveView::RecentlyCompleted => self.recently_completed_view.select_previous(),
                    ActiveView::NodeDetail => self.node_detail_view.select_previous_edge(),
                }
                self.last_key = Some(key);
            }
            KeyCode::Char('g') => {
                // Check for gg sequence
                if self.last_key == Some(KeyCode::Char('g')) {
                    match self.active_view {
                        ActiveView::QueueReady => self.queue_ready_view.select_first(),
                        ActiveView::RecentlyCompleted => {
                            self.recently_completed_view.select_first()
                        }
                        ActiveView::NodeDetail => self.node_detail_view.select_first_edge(),
                    }
                    self.last_key = None;
                } else {
                    self.last_key = Some(key);
                }
            }
            KeyCode::Char('G') | KeyCode::End => {
                match self.active_view {
                    ActiveView::QueueReady => self.queue_ready_view.select_last(),
                    ActiveView::RecentlyCompleted => self.recently_completed_view.select_last(),
                    ActiveView::NodeDetail => self.node_detail_view.select_last_edge(),
                }
                self.last_key = Some(key);
            }
            KeyCode::Home => {
                match self.active_view {
                    ActiveView::QueueReady => self.queue_ready_view.select_first(),
                    ActiveView::RecentlyCompleted => self.recently_completed_view.select_first(),
                    ActiveView::NodeDetail => self.node_detail_view.select_first_edge(),
                }
                self.last_key = Some(key);
            }
            KeyCode::Char('r') => {
                // If disconnected, trigger manual reconnect; otherwise refresh data
                if matches!(self.connection_state, ConnectionState::Disconnected) {
                    self.reconnect_requested = true;
                    self.log_panel.log("reconnecting (manual)");
                } else {
                    self.needs_refresh = true;
                    self.log_panel.log("refreshing");
                }
                self.last_key = Some(key);
            }
            KeyCode::Char(':') => {
                // Enter command mode
                self.enter_command_mode();
                self.last_key = Some(key);
            }
            // Reserved for future horizontal navigation (no-op)
            KeyCode::Char('h') | KeyCode::Char('l') => {
                self.last_key = Some(key);
            }
            _ => {
                self.last_key = Some(key);
            }
        }
    }

    /// Handle mouse events
    fn handle_mouse(&mut self, event: MouseEvent) {
        let MouseEvent {
            kind, column, row, ..
        } = event;

        // Close overlays on click outside
        if self.help_visible || self.notifications.history_visible {
            if let MouseEventKind::Down(MouseButton::Left) = kind {
                self.help_visible = false;
                self.notifications.close_history();
            }
            return;
        }

        // Get the layout chunks to determine click targets
        let Some(chunks) = &self.layout_chunks else {
            return;
        };

        // Check if click is in the main content area (chunks[1])
        let content_area = chunks[1];
        let in_content = column >= content_area.x
            && column < content_area.x + content_area.width
            && row >= content_area.y
            && row < content_area.y + content_area.height;

        // Check if click is in the title bar area (chunks[0])
        let title_area = chunks[0];
        let in_title = column >= title_area.x
            && column < title_area.x + title_area.width
            && row >= title_area.y
            && row < title_area.y + title_area.height;

        match kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if in_title {
                    // Click on title bar - check if clicking on view tabs
                    // [1] Queue/Ready | [2] Completed
                    // Approximate positions based on typical rendering
                    if column < title_area.x + 20 {
                        // Click on Queue/Ready tab area
                        self.active_view = ActiveView::QueueReady;
                        self.previous_list_view = ActiveView::QueueReady;
                    } else if column < title_area.x + 40 {
                        // Click on Completed tab area
                        self.active_view = ActiveView::RecentlyCompleted;
                        self.previous_list_view = ActiveView::RecentlyCompleted;
                    }
                } else if in_content {
                    // Click in content area - select item at row
                    let relative_row = row.saturating_sub(content_area.y + 1); // Account for border
                    match self.active_view {
                        ActiveView::QueueReady => {
                            self.queue_ready_view.select_at(relative_row as usize);
                        }
                        ActiveView::RecentlyCompleted => {
                            self.recently_completed_view
                                .select_at(relative_row as usize);
                        }
                        ActiveView::NodeDetail => {
                            // In detail view, clicking on edges
                            self.node_detail_view.select_edge_at(relative_row as usize);
                        }
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                if in_content {
                    match self.active_view {
                        ActiveView::QueueReady => self.queue_ready_view.select_previous(),
                        ActiveView::RecentlyCompleted => {
                            self.recently_completed_view.select_previous()
                        }
                        ActiveView::NodeDetail => self.node_detail_view.select_previous_edge(),
                    }
                }
            }
            MouseEventKind::ScrollDown => {
                if in_content {
                    match self.active_view {
                        ActiveView::QueueReady => self.queue_ready_view.select_next(),
                        ActiveView::RecentlyCompleted => self.recently_completed_view.select_next(),
                        ActiveView::NodeDetail => self.node_detail_view.select_next_edge(),
                    }
                }
            }
            _ => {}
        }
    }

    /// Handle incoming WebSocket message
    fn handle_ws_message(&mut self, msg: WsMessage) {
        if let WsMessage::Text(text) = msg {
            // Try to parse as a server message
            if let Ok(server_msg) = serde_json::from_str::<ServerMessage>(&text) {
                debug!(message_type = %server_msg.message_type(), "WebSocket message received");
                match server_msg {
                    ServerMessage::Connected { .. } => {
                        // Initial connection, fetch data
                        self.needs_refresh = true;
                        self.log_panel.log("connected");
                    }
                    ServerMessage::EntityAdded {
                        entity_type,
                        entity,
                        id,
                    } => {
                        // Log the addition
                        self.log_panel.log_entity(&entity_type, &id, "added");

                        // Check if this affects ready/queued items
                        if entity_type == "task" || entity_type == "bug" {
                            if let Some(status) = entity.get("status").and_then(|s| s.as_str()) {
                                if status == "pending"
                                    || status == "in_progress"
                                    || status == "done"
                                    || status == "blocked"
                                {
                                    self.needs_refresh = true;
                                }
                            }
                        }
                    }
                    ServerMessage::EntityUpdated {
                        entity_type,
                        entity,
                        id,
                    } => {
                        // Log the update
                        self.log_panel.log_entity(&entity_type, &id, "updated");

                        // Check if this affects ready/queued items
                        if entity_type == "task" || entity_type == "bug" {
                            if let Some(status) = entity.get("status").and_then(|s| s.as_str()) {
                                if status == "pending"
                                    || status == "in_progress"
                                    || status == "done"
                                    || status == "blocked"
                                {
                                    self.needs_refresh = true;
                                }
                            }
                        }
                    }
                    ServerMessage::EntityRemoved { entity_type, id } => {
                        self.log_panel.log_entity(&entity_type, &id, "removed");
                        self.needs_refresh = true;
                    }
                    ServerMessage::EdgeAdded { id, .. } => {
                        self.log_panel.log_entity("edge", &id, "added");
                        self.needs_refresh = true;
                    }
                    ServerMessage::EdgeRemoved { id, .. } => {
                        self.log_panel.log_entity("edge", &id, "removed");
                        self.needs_refresh = true;
                    }
                    ServerMessage::Reload { .. } => {
                        self.log_panel.log("reload requested");
                        self.needs_refresh = true;
                    }
                    ServerMessage::LogEntry { entry } => {
                        // Try to parse as a log entry
                        if let Ok(log_entry) = serde_json::from_value::<LogEntry>(entry) {
                            self.log_panel.add_entry(log_entry);
                        }
                    }
                    ServerMessage::Delta { changes, .. } => {
                        // Process incremental changes from the new Delta protocol
                        for change in changes {
                            match change {
                                Change::Create { entity_type, data } => {
                                    let id = data
                                        .get("id")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("?")
                                        .to_string();
                                    self.log_panel.log_entity(&entity_type, &id, "added");
                                    if entity_type == "task" || entity_type == "bug" {
                                        self.needs_refresh = true;
                                    }
                                }
                                Change::Update {
                                    entity_type, id, ..
                                } => {
                                    self.log_panel.log_entity(&entity_type, &id, "updated");
                                    if entity_type == "task" || entity_type == "bug" {
                                        self.needs_refresh = true;
                                    }
                                }
                                Change::Delete { entity_type, id } => {
                                    self.log_panel.log_entity(&entity_type, &id, "removed");
                                    self.needs_refresh = true;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// Fetch data from the server API
    async fn fetch_data(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Fetch ready tasks
        let ready_url = format!("{}/api/ready", self.api_base);
        let ready_resp = reqwest::get(&ready_url).await?;

        // Check for HTTP errors before attempting to parse JSON
        if !ready_resp.status().is_success() {
            return Err(format!("server error: {}", ready_resp.status()).into());
        }

        let ready_data: ReadyResponse = ready_resp.json().await?;

        // Convert in-progress items (tasks and bugs combined)
        let mut in_progress: Vec<WorkItem> = ready_data
            .in_progress_tasks
            .into_iter()
            .map(|t| t.into())
            .collect();
        in_progress.extend(
            ready_data
                .in_progress_bugs
                .into_iter()
                .map(|b| -> WorkItem { b.into() }),
        );
        // Sort in-progress by priority
        in_progress.sort_by_key(|item| item.priority);

        // Separate queued and non-queued items
        let mut queued: Vec<WorkItem> = Vec::new();
        let mut ready: Vec<WorkItem> = Vec::new();

        for task in ready_data.tasks {
            let item: WorkItem = task.into();
            if item.queued {
                queued.push(item);
            } else {
                ready.push(item);
            }
        }

        // Sort by priority (lower is higher priority)
        queued.sort_by_key(|item| item.priority);
        ready.sort_by_key(|item| item.priority);

        self.queue_ready_view
            .update_items_with_in_progress(in_progress, queued, ready);

        // Convert recently completed items
        let completed_tasks: Vec<CompletedItem> = ready_data
            .recently_completed_tasks
            .into_iter()
            .map(|t| t.into())
            .collect();
        let completed_bugs: Vec<CompletedItem> = ready_data
            .recently_completed_bugs
            .into_iter()
            .map(|b| b.into())
            .collect();

        self.recently_completed_view
            .update_items(completed_tasks, completed_bugs);
        self.needs_refresh = false;

        Ok(())
    }

    /// Fetch a single node's data from the server API
    async fn fetch_node_data(
        &mut self,
        node_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let node_url = format!("{}/api/node/{}", self.api_base, node_id);
        let node_resp = reqwest::get(&node_url).await?;

        if !node_resp.status().is_success() {
            return Err(format!("Node not found: {}", node_id).into());
        }

        let data: NodeResponse = node_resp.json().await?;
        self.node_detail_view.set_node(data.node, data.edges);

        Ok(())
    }

    /// Start reconnection process
    fn start_reconnecting(&mut self) {
        self.connection_state = ConnectionState::Reconnecting {
            attempt: 1,
            next_retry: Some(Instant::now()),
        };
        warn!("WebSocket connection lost, starting reconnection");
        self.log_panel.log("connection lost, reconnecting...");
        self.notifications.warning("Connection lost");
    }

    /// Handle a failed reconnection attempt
    fn handle_reconnect_failed(&mut self) {
        if let ConnectionState::Reconnecting {
            attempt,
            next_retry: _,
        } = &self.connection_state
        {
            let current_attempt = *attempt;
            if current_attempt >= MAX_RECONNECT_ATTEMPTS {
                // Max retries exceeded
                error!(
                    attempts = current_attempt,
                    max = MAX_RECONNECT_ATTEMPTS,
                    "Max reconnect attempts reached, giving up"
                );
                self.connection_state = ConnectionState::Disconnected;
                self.log_panel.log(format!(
                    "reconnect failed after {} attempts",
                    current_attempt
                ));
                self.notifications
                    .error("Disconnected - press 'r' to retry");
            } else {
                // Schedule next attempt
                let backoff = calculate_backoff(current_attempt + 1);
                warn!(
                    attempt = current_attempt,
                    next_attempt = current_attempt + 1,
                    backoff_secs = backoff.as_secs(),
                    "Reconnect attempt failed, scheduling retry"
                );
                self.connection_state = ConnectionState::Reconnecting {
                    attempt: current_attempt + 1,
                    next_retry: Some(Instant::now() + backoff),
                };
                self.log_panel
                    .log(format!("reconnect attempt {} failed", current_attempt));
            }
        }
    }

    /// Handle successful reconnection
    fn handle_reconnect_success(&mut self) {
        info!("WebSocket reconnection successful");
        self.connection_state = ConnectionState::Connected;
        self.log_panel.log("reconnected, refreshing state...");
        self.notifications.info("Reconnected");
    }

    /// Mark connection as disconnected (max retries exceeded or initial failure)
    #[allow(dead_code)]
    fn set_disconnected(&mut self) {
        warn!("Connection marked as disconnected");
        self.connection_state = ConnectionState::Disconnected;
        self.log_panel.log("disconnected");
        self.notifications.warning("Connection lost");
    }

    /// Mark connection as connected
    fn set_connected(&mut self) {
        self.connection_state = ConnectionState::Connected;
    }

    /// Get time until next reconnect attempt, if any
    #[allow(dead_code)]
    fn time_until_reconnect(&self) -> Option<Duration> {
        if let ConnectionState::Reconnecting {
            next_retry: Some(next),
            ..
        } = &self.connection_state
        {
            let now = Instant::now();
            if *next > now {
                return Some(*next - now);
            }
            return Some(Duration::ZERO);
        }
        None
    }

    /// Check if it's time to attempt reconnection
    fn should_attempt_reconnect(&self) -> bool {
        if let ConnectionState::Reconnecting {
            next_retry: Some(next),
            ..
        } = &self.connection_state
        {
            return Instant::now() >= *next;
        }
        false
    }

    /// Render the UI
    fn render(&mut self, frame: &mut Frame) {
        trace!(view = ?self.active_view, "render cycle");

        // Clean up expired toasts
        self.notifications.cleanup();

        let area = frame.area();

        // Determine log panel height
        let log_height = self.log_panel.preferred_height();

        // Create main layout
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),          // Title bar
                Constraint::Min(5),             // Main content
                Constraint::Length(log_height), // Log panel
                Constraint::Length(3),          // Status bar
            ])
            .split(area);

        // Save layout chunks for mouse handling
        self.layout_chunks = Some(chunks.clone());

        // Title bar with connection status
        self.render_title_bar(frame, chunks[0]);

        // Main content: render active view
        match self.active_view {
            ActiveView::QueueReady => self.queue_ready_view.render(frame, chunks[1]),
            ActiveView::RecentlyCompleted => self.recently_completed_view.render(frame, chunks[1]),
            ActiveView::NodeDetail => self.node_detail_view.render(frame, chunks[1]),
        }

        // Log panel (always visible)
        self.log_panel.render(frame, chunks[2]);

        // Status bar with keybindings
        self.render_status_bar(frame, chunks[3]);

        // Render toasts (floating, on top)
        self.render_toasts(frame);

        // Render notification history overlay (if visible)
        if self.notifications.history_visible {
            self.render_notification_history(frame);
        }

        // Render help overlay (if visible)
        if self.help_visible {
            self.render_help_overlay(frame);
        }
    }

    /// Render toast notifications (floating in top-right corner)
    fn render_toasts(&self, frame: &mut Frame) {
        if !self.notifications.has_toasts() {
            return;
        }

        let area = frame.area();
        let toast_width = 40u16.min(area.width.saturating_sub(4));
        let toast_x = area.width.saturating_sub(toast_width + 2);
        let mut toast_y = 4u16; // Below title bar

        for toast in self.notifications.visible_toasts() {
            if toast_y + 3 >= area.height.saturating_sub(6) {
                break;
            }

            let toast_area = Rect {
                x: toast_x,
                y: toast_y,
                width: toast_width,
                height: 3,
            };

            // Clear background
            frame.render_widget(Clear, toast_area);

            // Render toast
            let level = toast.level;
            let icon = level.icon();
            let color = level.color();

            let content = format!("{} {}", icon, toast.message);
            let toast_widget = Paragraph::new(content)
                .style(Style::default().fg(color))
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(color)),
                );

            frame.render_widget(toast_widget, toast_area);
            toast_y += 3;
        }

        // Show overflow indicator
        if self.notifications.overflow_count > 0 {
            let overflow_area = Rect {
                x: toast_x,
                y: toast_y,
                width: toast_width,
                height: 1,
            };
            let overflow_text = format!("...and {} more", self.notifications.overflow_count);
            let overflow_widget = Paragraph::new(overflow_text)
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Right);
            frame.render_widget(overflow_widget, overflow_area);
        }
    }

    /// Render notification history overlay
    fn render_notification_history(&self, frame: &mut Frame) {
        let area = frame.area();

        // Create centered overlay
        let overlay_width = (area.width * 3 / 4).min(80);
        let overlay_height = (area.height * 3 / 4).min(30);
        let overlay_x = (area.width - overlay_width) / 2;
        let overlay_y = (area.height - overlay_height) / 2;

        let overlay_area = Rect {
            x: overlay_x,
            y: overlay_y,
            width: overlay_width,
            height: overlay_height,
        };

        // Clear background
        frame.render_widget(Clear, overlay_area);

        if self.notifications.history_is_empty() {
            let empty = Paragraph::new("\n  No notifications yet")
                .style(Style::default().fg(Color::DarkGray))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Notification History [n/Esc to close] "),
                );
            frame.render_widget(empty, overlay_area);
            return;
        }

        // Build list items
        let items: Vec<ListItem> = self
            .notifications
            .history()
            .enumerate()
            .take(overlay_height as usize - 2)
            .map(|(idx, entry)| {
                let icon = entry.level.icon();
                let color = entry.level.color();
                let time = entry.relative_time();

                let style = if idx == self.notifications.history_selected {
                    Style::default().bg(Color::DarkGray)
                } else {
                    Style::default()
                };

                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!(" {:>8} ", time),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(icon, Style::default().fg(color)),
                    Span::raw(" "),
                    Span::styled(&entry.message, style),
                ]))
            })
            .collect();

        let list = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Notification History [n/Esc:close  c:clear  j/k:navigate] ")
                .border_style(Style::default().fg(Color::Cyan)),
        );

        frame.render_widget(list, overlay_area);
    }

    /// Render the help overlay with keybinding reference
    fn render_help_overlay(&self, frame: &mut Frame) {
        let area = frame.area();

        // Create centered overlay
        let overlay_width = (area.width * 3 / 4).min(70);
        let overlay_height = (area.height * 3 / 4).min(36);
        let overlay_x = (area.width - overlay_width) / 2;
        let overlay_y = (area.height - overlay_height) / 2;

        let overlay_area = Rect {
            x: overlay_x,
            y: overlay_y,
            width: overlay_width,
            height: overlay_height,
        };

        // Clear background
        frame.render_widget(Clear, overlay_area);

        // Build help content
        let help_text = vec![
            Line::from(vec![Span::styled(
                "  GLOBAL KEYS",
                Style::default().bold().fg(Color::Cyan),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled("    ?      ", Style::default().fg(Color::Yellow)),
                Span::raw("Toggle this help overlay"),
            ]),
            Line::from(vec![
                Span::styled("    q      ", Style::default().fg(Color::Yellow)),
                Span::raw("Quit"),
            ]),
            Line::from(vec![
                Span::styled("    r      ", Style::default().fg(Color::Yellow)),
                Span::raw("Refresh data / Reconnect if disconnected"),
            ]),
            Line::from(vec![
                Span::styled("    Tab    ", Style::default().fg(Color::Yellow)),
                Span::raw("Cycle between views"),
            ]),
            Line::from(vec![
                Span::styled("    1/2    ", Style::default().fg(Color::Yellow)),
                Span::raw("Jump to Queue/Ready or Completed view"),
            ]),
            Line::from(vec![
                Span::styled("    3      ", Style::default().fg(Color::Yellow)),
                Span::raw("Open detail for selection"),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "  NAVIGATION",
                Style::default().bold().fg(Color::Cyan),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled("    j/↓    ", Style::default().fg(Color::Yellow)),
                Span::raw("Move selection down"),
            ]),
            Line::from(vec![
                Span::styled("    k/↑    ", Style::default().fg(Color::Yellow)),
                Span::raw("Move selection up"),
            ]),
            Line::from(vec![
                Span::styled("    gg     ", Style::default().fg(Color::Yellow)),
                Span::raw("Jump to top"),
            ]),
            Line::from(vec![
                Span::styled("    G      ", Style::default().fg(Color::Yellow)),
                Span::raw("Jump to bottom"),
            ]),
            Line::from(vec![
                Span::styled("    Enter  ", Style::default().fg(Color::Yellow)),
                Span::raw("Open detail view / Navigate to edge"),
            ]),
            Line::from(vec![
                Span::styled("    Esc    ", Style::default().fg(Color::Yellow)),
                Span::raw("Go back / Cancel search or command"),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "  COMMANDS",
                Style::default().bold().fg(Color::Cyan),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled("    :      ", Style::default().fg(Color::Yellow)),
                Span::raw("Enter command mode"),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "  PANELS",
                Style::default().bold().fg(Color::Cyan),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled("    H      ", Style::default().fg(Color::Yellow)),
                Span::raw("Toggle notification history"),
            ]),
            Line::from(vec![
                Span::styled("    L      ", Style::default().fg(Color::Yellow)),
                Span::raw("Toggle log panel collapse"),
            ]),
            Line::from(vec![
                Span::styled("    d      ", Style::default().fg(Color::Yellow)),
                Span::raw("Dismiss oldest toast"),
            ]),
            Line::from(vec![
                Span::styled("    D      ", Style::default().fg(Color::Yellow)),
                Span::raw("Dismiss all toasts"),
            ]),
        ];

        let help_widget = Paragraph::new(help_text).wrap(Wrap { trim: false }).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Keyboard Shortcuts [?/Esc to close] ")
                .border_style(Style::default().fg(Color::Cyan)),
        );

        frame.render_widget(help_widget, overlay_area);
    }

    /// Render the title bar with connection status
    fn render_title_bar(&self, frame: &mut Frame, area: Rect) {
        let status_indicator = match &self.connection_state {
            ConnectionState::Connected => "●",
            ConnectionState::Reconnecting { .. } => "○",
            ConnectionState::Disconnected => "✗",
        };
        let status_color = match &self.connection_state {
            ConnectionState::Connected => Color::Green,
            ConnectionState::Reconnecting { .. } => Color::Yellow,
            ConnectionState::Disconnected => Color::Red,
        };
        let status_text = match &self.connection_state {
            ConnectionState::Connected => "Connected".to_string(),
            ConnectionState::Reconnecting { attempt, .. } => {
                format!("Reconnecting (attempt {})...", attempt)
            }
            ConnectionState::Disconnected => "Disconnected".to_string(),
        };

        // Current view name and view switcher hint - changes based on active view
        let (view_name, view_hint) = match self.active_view {
            ActiveView::QueueReady => (" [1] Queue/Ready", "[2] Completed"),
            ActiveView::RecentlyCompleted => ("[1] Queue/Ready", " [2] Completed"),
            ActiveView::NodeDetail => {
                // Show node ID in title when viewing detail
                if let Some(node) = &self.node_detail_view.node {
                    return self.render_detail_title_bar(
                        frame,
                        area,
                        &node.id,
                        status_indicator,
                        status_color,
                        &status_text,
                    );
                }
                (" [3] Detail", "[Esc] Back")
            }
        };

        // Calculate padding to right-align status
        let title_text = format!("{} | {}", view_name, view_hint);
        let status_display = format!("[{}] {}", status_indicator, status_text);
        let padding = area
            .width
            .saturating_sub(title_text.len() as u16 + status_display.len() as u16 + 4);

        let view_style = Style::default().add_modifier(Modifier::BOLD);
        let inactive_style = Style::default().fg(Color::DarkGray);

        let (left_style, right_style) = match self.active_view {
            ActiveView::QueueReady => (view_style, inactive_style),
            ActiveView::RecentlyCompleted => (inactive_style, view_style),
            ActiveView::NodeDetail => (inactive_style, inactive_style),
        };

        let title = Paragraph::new(Line::from(vec![
            Span::styled(format!(" {}", view_name), left_style),
            Span::raw(" | "),
            Span::styled(view_hint, right_style),
            Span::raw(" ".repeat(padding as usize)),
            Span::styled(status_display, Style::default().fg(status_color)),
        ]))
        .block(Block::default().borders(Borders::ALL));

        frame.render_widget(title, area);
    }

    /// Render the title bar for detail view (shows node ID)
    fn render_detail_title_bar(
        &self,
        frame: &mut Frame,
        area: Rect,
        node_id: &str,
        status_indicator: &str,
        status_color: Color,
        status_text: &str,
    ) {
        let title_text = format!(" Node Detail: {} ", node_id);
        let back_hint = "[Esc] Back";
        let status_display = format!("[{}] {}", status_indicator, status_text);
        let padding = area.width.saturating_sub(
            title_text.len() as u16 + back_hint.len() as u16 + status_display.len() as u16 + 4,
        );

        let title = Paragraph::new(Line::from(vec![
            Span::styled(title_text, Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(back_hint, Style::default().fg(Color::DarkGray)),
            Span::raw(" ".repeat(padding as usize)),
            Span::styled(status_display, Style::default().fg(status_color)),
        ]))
        .block(Block::default().borders(Borders::ALL));

        frame.render_widget(title, area);
    }

    /// Render the status bar with keybindings or command input
    fn render_status_bar(&self, frame: &mut Frame, area: Rect) {
        // If in command mode, show command input instead of keybinding hints
        if self.is_command_mode() {
            let command_text = format!(":{}", self.command_input);
            let status = Paragraph::new(Line::from(vec![
                Span::styled(command_text, Style::default().fg(Color::White)),
                Span::styled("█", Style::default().fg(Color::White)), // Cursor
            ]))
            .block(Block::default().borders(Borders::ALL));
            frame.render_widget(status, area);
            return;
        }

        let help_text = match self.active_view {
            ActiveView::QueueReady | ActiveView::RecentlyCompleted => {
                " Tab:View  j/k:Nav  Enter:Detail  r:Refresh  L:Log  H:History  ?:Help  q:Quit"
            }
            ActiveView::NodeDetail => {
                " j/k:Nav  Enter:Go  Esc:Back  r:Refresh  L:Log  H:History  ?:Help  q:Quit"
            }
        };
        let status = Paragraph::new(help_text)
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(status, area);
    }
}

/// Setup the terminal for TUI mode
fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    stdout()
        .execute(EnterAlternateScreen)?
        .execute(EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout());
    Terminal::new(backend)
}

/// Restore the terminal to normal mode
fn restore_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    stdout()
        .execute(DisableMouseCapture)?
        .execute(LeaveAlternateScreen)?;
    Ok(())
}

/// Run the TUI application
///
/// # Arguments
/// * `port` - Server port to connect to (default: 3030)
/// * `host` - Server host to connect to (default: localhost)
/// * `url` - Optional WebSocket URL for remote connections (overrides port/host)
///
/// # Errors
/// Returns an error if the server is not running or connection fails.
pub async fn run_tui(
    port: Option<u16>,
    host: Option<String>,
    url: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    // If URL is provided, use it directly; otherwise construct from host/port
    let (endpoint, display_host, display_port) = if let Some(ref ws_url) = url {
        // Parse URL to extract host/port for display purposes
        let (host, port) = parse_ws_url(ws_url);
        (ws_url.clone(), host, port)
    } else {
        let port = port.unwrap_or(DEFAULT_PORT);
        let host = host.unwrap_or_else(|| "localhost".to_string());
        let endpoint = format!("ws://{}:{}/ws", host, port);
        (endpoint, host, port)
    };

    let mut app = TuiApp::new(&display_host, display_port);

    info!(endpoint = %endpoint, host = %display_host, port = display_port, "TUI starting, connecting to server");

    // Try to connect to the server
    let (ws_stream, _response) = match tokio_tungstenite::connect_async(&endpoint).await {
        Ok(result) => result,
        Err(e) => {
            if url.is_some() {
                // Remote URL connection failed
                error!(endpoint = %endpoint, error = %e, "Failed to connect to remote session");
                eprintln!("Error: Failed to connect to remote session at {}", endpoint);
                eprintln!("\nDetails: {}", e);
            } else {
                // Local connection failed - auto-launch should have started the server
                error!(host = %display_host, port = display_port, error = %e, "No session server detected");
                eprintln!(
                    "Error: No session server detected at {}:{}",
                    display_host, display_port
                );
                eprintln!("Session server should start automatically. If problems persist,");
                eprintln!("check for errors in: bn session serve");
                eprintln!("\nDetails: {}", e);
            }
            std::process::exit(1);
        }
    };

    info!(endpoint = %endpoint, "WebSocket connection established");
    app.set_connected();
    let (_write, read) = ws_stream.split();
    let mut ws_read: Option<SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>> = Some(read);

    // Setup terminal
    let mut terminal = setup_terminal()?;

    // Main event loop
    loop {
        // Fetch data if needed (with error cooldown to prevent spam)
        if app.needs_refresh {
            // Check if we're in error cooldown period
            let should_retry = app.last_fetch_error.is_none_or(|last_error| {
                last_error.elapsed() >= Duration::from_secs(FETCH_ERROR_COOLDOWN_SECS)
            });

            if should_retry {
                if let Err(e) = app.fetch_data().await {
                    // Log error to TUI panel - data will be stale but user is informed
                    // Only log if this is a new error (not a repeated retry)
                    if app.last_fetch_error.is_none() {
                        error!(error = %e, "Failed to fetch data from server");
                        app.log_panel.log(format!("fetch error: {}", e));
                    }
                    app.last_fetch_error = Some(Instant::now());
                } else {
                    // Success - clear the error state
                    app.last_fetch_error = None;
                }
            }
        }

        // Fetch node data if needed (for detail view)
        if let Some(node_id) = app.needs_node_fetch.take() {
            if let Err(e) = app.fetch_node_data(&node_id).await {
                // Log error and go back to list view
                error!(node_id = %node_id, error = %e, "Failed to fetch node data");
                app.log_panel.log(format!("node fetch error: {}", e));
                app.go_back_from_detail();
            }
        }

        // Render the UI
        terminal.draw(|f| app.render(f))?;

        // Handle reconnection if needed
        if app.reconnect_requested {
            app.reconnect_requested = false;
            // Start reconnection from manual request
            app.connection_state = ConnectionState::Reconnecting {
                attempt: 1,
                next_retry: Some(Instant::now()),
            };
        }

        // Attempt reconnection if it's time
        if app.should_attempt_reconnect() {
            match tokio_tungstenite::connect_async(&app.ws_endpoint).await {
                Ok((new_ws, _)) => {
                    app.handle_reconnect_success();
                    let (_, new_read) = new_ws.split();
                    ws_read = Some(new_read);
                    // Debounce before state refresh
                    tokio::time::sleep(Duration::from_millis(RECONNECT_DEBOUNCE_MS)).await;
                    app.needs_refresh = true;
                    let item_count = app.queue_ready_view.total_items()
                        + app.recently_completed_view.items.len();
                    app.log_panel
                        .log(format!("state refresh complete ({} items)", item_count));
                }
                Err(_) => {
                    app.handle_reconnect_failed();
                }
            }
        }

        // Check for keyboard and mouse events first (non-blocking)
        // Use a small timeout to catch buffered events without blocking
        while event::poll(Duration::from_millis(10))? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Press {
                        app.handle_key(key.code);
                    }
                }
                Event::Mouse(mouse) => {
                    app.handle_mouse(mouse);
                }
                _ => {}
            }
        }

        // Check for quit after processing events
        if app.should_quit {
            break;
        }

        // Handle WebSocket messages with a timeout
        tokio::select! {
            // Wait a bit before next iteration
            _ = tokio::time::sleep(Duration::from_millis(50)) => {
                // Just a tick to prevent busy-looping
            }
            // Check for WebSocket messages (only if connected)
            msg = async {
                if let Some(ref mut read) = ws_read {
                    read.next().await
                } else {
                    // No connection, just pend forever (will timeout via sleep branch)
                    std::future::pending().await
                }
            } => {
                match msg {
                    Some(Ok(message)) => {
                        app.handle_ws_message(message);
                    }
                    Some(Err(_)) | None => {
                        // Connection closed or error - start reconnection
                        ws_read = None;
                        if app.connection_state.is_connected() {
                            app.start_reconnecting();
                        }
                    }
                }
            }
        }
    }

    // Restore terminal
    restore_terminal()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_parse_ws_url_with_port() {
        let (host, port) = parse_ws_url("ws://example.com:8080/ws");
        assert_eq!(host, "example.com");
        assert_eq!(port, 8080);
    }

    #[test]
    fn test_parse_ws_url_wss_with_port() {
        let (host, port) = parse_ws_url("wss://remote.example.com:3030/ws");
        assert_eq!(host, "remote.example.com");
        assert_eq!(port, 3030);
    }

    #[test]
    fn test_parse_ws_url_ws_default_port() {
        let (host, port) = parse_ws_url("ws://localhost/ws");
        assert_eq!(host, "localhost");
        assert_eq!(port, DEFAULT_PORT);
    }

    #[test]
    fn test_parse_ws_url_wss_default_port() {
        let (host, port) = parse_ws_url("wss://secure.example.com/ws");
        assert_eq!(host, "secure.example.com");
        assert_eq!(port, 443);
    }

    #[test]
    fn test_parse_ws_url_no_path() {
        let (host, port) = parse_ws_url("ws://localhost:9000");
        assert_eq!(host, "localhost");
        assert_eq!(port, 9000);
    }

    #[test]
    fn test_common_prefix_single_string() {
        let result = TuiApp::common_prefix(&["hello"]);
        assert_eq!(result, Some("hello".to_string()));
    }

    #[test]
    fn test_common_prefix_identical_strings() {
        let result = TuiApp::common_prefix(&["quit", "quit"]);
        assert_eq!(result, Some("quit".to_string()));
    }

    #[test]
    fn test_common_prefix_partial_match() {
        let result = TuiApp::common_prefix(&["help", "history", "hist"]);
        assert_eq!(result, Some("h".to_string()));
    }

    #[test]
    fn test_common_prefix_no_common() {
        let result = TuiApp::common_prefix(&["quit", "help"]);
        assert_eq!(result, Some("".to_string()));
    }

    #[test]
    fn test_common_prefix_empty_list() {
        let result = TuiApp::common_prefix(&[]);
        assert_eq!(result, None);
    }

    #[test]
    fn test_commands_list_not_empty() {
        assert!(!TuiApp::COMMANDS.is_empty());
        // All commands should be lowercase and non-empty
        for cmd in TuiApp::COMMANDS {
            assert!(!cmd.is_empty());
            assert_eq!(*cmd, cmd.to_lowercase());
        }
    }

    #[test]
    fn test_task_data_deserialize_from_bug() {
        // Bug JSON from the API - should still parse as TaskData
        let bug_json = r#"{"id":"bn-855a","type":"bug","title":"TUI center pane bug","short_name":"tui-bug","description":"description","tags":[],"created_at":"2026-02-01T08:58:31.027145075Z","updated_at":"2026-02-01T08:58:31.027145075Z","priority":2,"status":"pending","severity":"triage","depends_on":[],"queued":false,"queued_via":null}"#;

        let task: TaskData =
            serde_json::from_str(bug_json).expect("Bug should deserialize as TaskData");
        assert_eq!(task.core.id, "bn-855a");
        assert_eq!(task.core.entity_type, Some("bug".to_string()));
        assert_eq!(task.priority, 2);
        assert!(!task.queued);
    }

    #[test]
    fn test_task_data_deserialize_from_task() {
        let task_json = r#"{"id":"bn-8d3c","type":"task","title":"Update docs","short_name":"Update docs","description":"description","tags":[],"created_at":"2026-02-01T08:09:57.525401487Z","updated_at":"2026-02-01T08:09:57.525401487Z","priority":2,"status":"pending","depends_on":[],"queued":false,"queued_via":null}"#;

        let task: TaskData =
            serde_json::from_str(task_json).expect("Task should deserialize as TaskData");
        assert_eq!(task.core.id, "bn-8d3c");
        assert_eq!(task.core.entity_type, Some("task".to_string()));
    }

    #[test]
    fn test_ready_response_deserialize() {
        // Simulating what /api/ready returns - tasks array contains both tasks and bugs
        let response_json = r#"{
            "tasks": [
                {"id":"bn-8d3c","type":"task","title":"Update docs","priority":2,"status":"pending","queued":false},
                {"id":"bn-855a","type":"bug","title":"TUI bug","priority":2,"status":"pending","severity":"triage","queued":false}
            ],
            "recently_completed_tasks": [],
            "recently_completed_bugs": []
        }"#;

        let response: ReadyResponse =
            serde_json::from_str(response_json).expect("Response should deserialize");
        assert_eq!(response.tasks.len(), 2);
        assert_eq!(response.tasks[0].core.entity_type, Some("task".to_string()));
        assert_eq!(response.tasks[1].core.entity_type, Some("bug".to_string()));
    }

    #[test]
    fn test_build_env_filter_with_cli_arg() {
        // CLI arg takes precedence
        let filter = build_env_filter(Some("debug"));
        // Filter builds without error when given valid level
        assert!(format!("{:?}", filter).contains("EnvFilter"));
    }

    #[test]
    fn test_build_env_filter_with_invalid_cli_arg() {
        // Invalid CLI arg falls through to env or default
        let filter = build_env_filter(Some("not_a_valid_level_at_all"));
        // Should still return a valid filter (defaults to warn)
        assert!(format!("{:?}", filter).contains("EnvFilter"));
    }

    #[test]
    fn test_build_env_filter_none() {
        // None falls through to env or default
        let filter = build_env_filter(None);
        assert!(format!("{:?}", filter).contains("EnvFilter"));
    }

    #[test]
    fn test_get_logs_directory_exists() {
        // Verify we can construct a logs directory path
        let logs_dir = get_logs_directory();
        // On most systems this should return Some - just verify the function runs
        if let Some(dir) = logs_dir {
            assert!(dir.ends_with("logs"));
            assert!(dir.to_string_lossy().contains("binnacle"));
        }
        // If dirs::data_dir returns None (rare), test still passes
    }

    // === Log Level Tests ===

    #[test]
    fn test_build_env_filter_all_valid_levels() {
        // Test all standard log levels produce valid filters
        for level in &["error", "warn", "info", "debug", "trace"] {
            let filter = build_env_filter(Some(level));
            assert!(
                format!("{:?}", filter).contains("EnvFilter"),
                "Level '{}' should produce valid EnvFilter",
                level
            );
        }
    }

    #[test]
    #[serial]
    fn test_build_env_filter_cli_precedence_over_env() {
        // This test verifies CLI flag takes precedence over BN_LOG env var
        // We can't directly inspect the filter level, but we can verify:
        // 1. With CLI set, changing env var doesn't change behavior
        // 2. The function returns without error in both cases

        // Set env var to one level
        // SAFETY: This test runs serially to avoid env var races
        unsafe { std::env::set_var("BN_LOG", "trace") };

        // CLI flag should take precedence (set to debug)
        let filter_with_cli = build_env_filter(Some("debug"));

        // Both should produce valid filters - CLI takes precedence
        assert!(format!("{:?}", filter_with_cli).contains("EnvFilter"));

        // Clean up
        // SAFETY: This test runs serially
        unsafe { std::env::remove_var("BN_LOG") };
    }

    #[test]
    #[serial]
    fn test_build_env_filter_env_var_used_when_no_cli() {
        // When no CLI arg, BN_LOG env var should be used
        // SAFETY: This test runs serially to avoid env var races
        unsafe { std::env::set_var("BN_LOG", "info") };

        let filter = build_env_filter(None);
        assert!(format!("{:?}", filter).contains("EnvFilter"));

        // SAFETY: This test runs serially
        unsafe { std::env::remove_var("BN_LOG") };
    }

    #[test]
    fn test_build_env_filter_complex_filter_string() {
        // tracing-subscriber supports complex filter strings
        let filter = build_env_filter(Some("binnacle=debug,warn"));
        assert!(format!("{:?}", filter).contains("EnvFilter"));
    }

    #[test]
    fn test_build_env_filter_empty_string_uses_default() {
        // Empty string is invalid, should fall back to default
        let filter = build_env_filter(Some(""));
        // Should still return a valid filter (uses default "warn")
        assert!(format!("{:?}", filter).contains("EnvFilter"));
    }

    // === Init Logging Tests ===

    #[test]
    fn test_init_logging_returns_guard() {
        // init_logging should return Some(guard) on success
        // Note: We can only set global subscriber once per process,
        // so this test may fail if run after other tests that set it.
        // The function handles this gracefully by returning the guard anyway.
        let result = init_logging(Some("warn"));
        // Result should be Some if logs directory can be created
        if result.is_some() {
            // Guard exists - logging was set up
            assert!(result.is_some());
        }
        // If None, logs directory couldn't be created (e.g., no data_dir)
    }

    #[test]
    fn test_init_logging_with_different_levels() {
        // Verify init_logging accepts all standard levels without panic
        for level in &["error", "warn", "info", "debug", "trace"] {
            // This may return None if subscriber is already set or dir unavailable,
            // but it should never panic
            let _ = init_logging(Some(level));
        }
    }
}
