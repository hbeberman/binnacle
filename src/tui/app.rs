//! TUI Application - main event loop and terminal management
//!
//! This module contains the core TUI application logic including:
//! - Terminal setup and restoration
//! - WebSocket connection handling
//! - Event loop for keyboard and server messages

use std::io::{self, stdout};
use std::time::Duration;

use crossterm::{
    ExecutableCommand,
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures::StreamExt;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};
use serde::Deserialize;
use tokio_tungstenite::tungstenite::Message as WsMessage;

use super::connection::ConnectionState;
use super::views::{QueueReadyView, WorkItem};

/// Default server port
pub const DEFAULT_PORT: u16 = 3030;

/// Response from /api/ready endpoint
#[derive(Debug, Deserialize)]
struct ReadyResponse {
    tasks: Vec<TaskData>,
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
        #[allow(dead_code)]
        id: String,
        entity: serde_json::Value,
    },
    /// Entity was updated
    EntityUpdated {
        entity_type: String,
        #[allow(dead_code)]
        id: String,
        entity: serde_json::Value,
    },
    /// Entity was removed
    EntityRemoved {
        #[allow(dead_code)]
        entity_type: String,
        #[allow(dead_code)]
        id: String,
    },
    /// Log entry (activity)
    LogEntry {
        #[allow(dead_code)]
        entry: serde_json::Value,
    },
    /// Edge added
    EdgeAdded {
        #[allow(dead_code)]
        id: String,
        #[allow(dead_code)]
        edge: serde_json::Value,
    },
    /// Edge removed
    EdgeRemoved {
        #[allow(dead_code)]
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
}

/// TUI Application state
pub struct TuiApp {
    /// Connection state
    connection_state: ConnectionState,
    /// Whether to quit the application
    should_quit: bool,
    /// Queue/Ready view
    queue_ready_view: QueueReadyView,
    /// HTTP base URL for API requests
    api_base: String,
    /// Flag indicating data needs refresh
    needs_refresh: bool,
    /// Last key pressed (for gg detection)
    last_key: Option<KeyCode>,
}

impl TuiApp {
    /// Create a new TUI application
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            connection_state: ConnectionState::Disconnected,
            should_quit: false,
            queue_ready_view: QueueReadyView::new(),
            api_base: format!("http://{}:{}", host, port),
            needs_refresh: true,
            last_key: None,
        }
    }

    /// Handle keyboard events
    fn handle_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('j') | KeyCode::Down => {
                self.queue_ready_view.select_next();
                self.last_key = Some(key);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.queue_ready_view.select_previous();
                self.last_key = Some(key);
            }
            KeyCode::Char('g') => {
                // Check for gg sequence
                if self.last_key == Some(KeyCode::Char('g')) {
                    self.queue_ready_view.select_first();
                    self.last_key = None;
                } else {
                    self.last_key = Some(key);
                }
            }
            KeyCode::Char('G') | KeyCode::End => {
                self.queue_ready_view.select_last();
                self.last_key = Some(key);
            }
            KeyCode::Home => {
                self.queue_ready_view.select_first();
                self.last_key = Some(key);
            }
            KeyCode::Char('r') => {
                // Request data refresh
                self.needs_refresh = true;
                self.last_key = Some(key);
            }
            _ => {
                self.last_key = Some(key);
            }
        }
    }

    /// Handle incoming WebSocket message
    fn handle_ws_message(&mut self, msg: WsMessage) {
        if let WsMessage::Text(text) = msg {
            // Try to parse as a server message
            if let Ok(server_msg) = serde_json::from_str::<ServerMessage>(&text) {
                match server_msg {
                    ServerMessage::Connected { .. } => {
                        // Initial connection, fetch data
                        self.needs_refresh = true;
                    }
                    ServerMessage::EntityAdded {
                        entity_type,
                        entity,
                        ..
                    }
                    | ServerMessage::EntityUpdated {
                        entity_type,
                        entity,
                        ..
                    } => {
                        // Check if this affects ready/queued items
                        if entity_type == "task" || entity_type == "bug" {
                            // Check if status changed to/from actionable states
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
                    ServerMessage::EntityRemoved { .. } => {
                        self.needs_refresh = true;
                    }
                    ServerMessage::EdgeAdded { .. } | ServerMessage::EdgeRemoved { .. } => {
                        // Edge changes might affect ready status (dependencies)
                        self.needs_refresh = true;
                    }
                    ServerMessage::Reload { .. } => {
                        self.needs_refresh = true;
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
        let ready_data: ReadyResponse = ready_resp.json().await?;

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

        self.queue_ready_view.update_items(queued, ready);
        self.needs_refresh = false;

        Ok(())
    }

    /// Mark connection as disconnected
    fn set_disconnected(&mut self) {
        self.connection_state = ConnectionState::Disconnected;
    }

    /// Mark connection as connected
    fn set_connected(&mut self) {
        self.connection_state = ConnectionState::Connected;
    }

    /// Render the UI
    fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();

        // Create main layout
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title bar
                Constraint::Min(5),    // Main content
                Constraint::Length(3), // Status bar
            ])
            .split(area);

        // Title bar with connection status
        self.render_title_bar(frame, chunks[0]);

        // Main content: Queue/Ready view
        self.queue_ready_view.render(frame, chunks[1]);

        // Status bar with keybindings
        self.render_status_bar(frame, chunks[2]);
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
            ConnectionState::Reconnecting { attempt } => {
                format!("Reconnecting (attempt {})...", attempt)
            }
            ConnectionState::Disconnected => "Disconnected".to_string(),
        };

        // Calculate padding to right-align status
        let title_text = " Queue/Ready";
        let status_display = format!("[{}] {}", status_indicator, status_text);
        let padding = area
            .width
            .saturating_sub(title_text.len() as u16 + status_display.len() as u16 + 4);

        let title = Paragraph::new(Line::from(vec![
            Span::styled(title_text, Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" ".repeat(padding as usize)),
            Span::styled(status_display, Style::default().fg(status_color)),
        ]))
        .block(Block::default().borders(Borders::ALL));

        frame.render_widget(title, area);
    }

    /// Render the status bar with keybindings
    fn render_status_bar(&self, frame: &mut Frame, area: Rect) {
        let status = Paragraph::new(" j/k:Navigate  gg/G:Top/Bottom  r:Refresh  q:Quit")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(status, area);
    }
}

/// Setup the terminal for TUI mode
fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout());
    Terminal::new(backend)
}

/// Restore the terminal to normal mode
fn restore_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

/// Run the TUI application
///
/// # Arguments
/// * `port` - Server port to connect to (default: 3030)
/// * `host` - Server host to connect to (default: localhost)
///
/// # Errors
/// Returns an error if the server is not running or connection fails.
pub async fn run_tui(
    port: Option<u16>,
    host: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let port = port.unwrap_or(DEFAULT_PORT);
    let host = host.unwrap_or_else(|| "localhost".to_string());
    let endpoint = format!("ws://{}:{}/ws", host, port);

    let mut app = TuiApp::new(&host, port);

    // Try to connect to the server
    let (ws_stream, _response) = match tokio_tungstenite::connect_async(&endpoint).await {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Error: No binnacle server detected at {}:{}", host, port);
            eprintln!("Start the server with: bn serve");
            eprintln!("\nDetails: {}", e);
            std::process::exit(1);
        }
    };

    app.set_connected();
    let (_write, mut read) = ws_stream.split();

    // Setup terminal
    let mut terminal = setup_terminal()?;

    // Main event loop
    loop {
        // Fetch data if needed
        if app.needs_refresh {
            if let Err(e) = app.fetch_data().await {
                // Log error but don't crash - data will be stale
                eprintln!("Error fetching data: {}", e);
            }
        }

        // Render the UI
        terminal.draw(|f| app.render(f))?;

        // Handle events with a timeout
        tokio::select! {
            // Check for keyboard events
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                if event::poll(Duration::from_millis(0))? {
                    if let Event::Key(key) = event::read()? {
                        if key.kind == KeyEventKind::Press {
                            app.handle_key(key.code);
                        }
                    }
                }
            }
            // Check for WebSocket messages
            msg = read.next() => {
                match msg {
                    Some(Ok(message)) => {
                        app.handle_ws_message(message);
                    }
                    Some(Err(_)) | None => {
                        // Connection closed or error
                        app.set_disconnected();
                    }
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    restore_terminal()?;

    Ok(())
}
