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
use tokio_tungstenite::tungstenite::Message as WsMessage;

use super::connection::ConnectionState;

/// Default server port
pub const DEFAULT_PORT: u16 = 3030;

/// TUI Application state
pub struct TuiApp {
    /// Connection state
    connection_state: ConnectionState,
    /// Whether to quit the application
    should_quit: bool,
    /// Last received message (for display)
    last_message: Option<String>,
}

impl TuiApp {
    /// Create a new TUI application
    pub fn new() -> Self {
        Self {
            connection_state: ConnectionState::Disconnected,
            should_quit: false,
            last_message: None,
        }
    }

    /// Handle keyboard events
    fn handle_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            _ => {}
        }
    }

    /// Handle incoming WebSocket message
    fn handle_ws_message(&mut self, msg: WsMessage) {
        if let WsMessage::Text(text) = msg {
            self.last_message = Some(text.to_string());
        }
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
    fn render(&self, frame: &mut Frame) {
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
            ConnectionState::Connected => "Connected",
            ConnectionState::Reconnecting { attempt } => {
                let title = Paragraph::new(format!(
                    " Queue/Ready                                    [{status_indicator}] Reconnecting (attempt {attempt})..."
                ))
                .style(Style::default().fg(status_color))
                .block(Block::default().borders(Borders::ALL));
                frame.render_widget(title, chunks[0]);

                // Render rest of UI
                let content_text = match &self.last_message {
                    Some(msg) => msg.clone(),
                    None => "Waiting for data...".to_string(),
                };
                let content = Paragraph::new(content_text)
                    .block(Block::default().borders(Borders::ALL).title(" Messages "));
                frame.render_widget(content, chunks[1]);

                let status = Paragraph::new(" Press 'q' to quit | 'r' to reconnect")
                    .style(Style::default().fg(Color::DarkGray))
                    .block(Block::default().borders(Borders::ALL));
                frame.render_widget(status, chunks[2]);
                return;
            }
            ConnectionState::Disconnected => "Disconnected",
        };

        let title = Paragraph::new(format!(
            " Queue/Ready                                    [{status_indicator}] {status_text}"
        ))
        .style(Style::default().fg(status_color))
        .block(Block::default().borders(Borders::ALL));
        frame.render_widget(title, chunks[0]);

        // Main content area
        let content_text = match &self.last_message {
            Some(msg) => msg.clone(),
            None => "Waiting for data...".to_string(),
        };
        let content = Paragraph::new(content_text)
            .block(Block::default().borders(Borders::ALL).title(" Messages "));
        frame.render_widget(content, chunks[1]);

        // Status bar
        let status = Paragraph::new(" Press 'q' to quit | 'r' to reconnect")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(status, chunks[2]);
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

    let mut app = TuiApp::new();

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
