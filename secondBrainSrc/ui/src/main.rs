use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};
use std::{io, io::Write, net::TcpStream};

struct App {
    input: String,
    response: String,
    scroll: u16,
    max_scroll: u16,
}

impl App {
    fn new() -> Self {
        Self {
            input: String::new(),
            response: String::new(),
            scroll: 0,
            max_scroll: 0,
        }
    }

    fn submit_query(&mut self) -> Result<()> {
        if self.input.is_empty() {
            return Ok(());
        }

        // Connect to the recall module's TCP server
        match TcpStream::connect("127.0.0.1:8080") {
            Ok(mut stream) => {
                // Send the query
                stream.write_all(self.input.as_bytes())?;
                stream.flush()?;

                // Read the response
                let mut buffer = [0; 4096];
                match stream.read(&mut buffer) {
                    Ok(size) => {
                        if size > 0 {
                            self.response = String::from_utf8_lossy(&buffer[..size]).to_string();
                        } else {
                            self.response = "Received empty response from server".to_string();
                        }
                    }
                    Err(e) => {
                        self.response = format!("Error reading response: {}", e);
                    }
                }
            }
            Err(e) => {
                self.response = format!("Failed to connect to recall module: {}", e);
            }
        }

        self.input.clear();
        Ok(())
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Input area
            Constraint::Min(0),    // Response area
        ])
        .split(f.size());

    // Input box
    let input_widget = Paragraph::new(app.input.as_str())
        .block(Block::default().borders(Borders::ALL).title("Query"));
    f.render_widget(input_widget, chunks[0]);

    // Response area with scrollbar
    let response_text = Paragraph::new(app.response.as_str())
        .block(Block::default().borders(Borders::ALL).title("Summary"))
        .scroll((app.scroll, 0));

    f.render_widget(response_text, chunks[1]);

    // Scrollbar for response
    let scrollbar = Scrollbar::default()
        .orientation(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None);

    f.render_stateful_widget(
        scrollbar,
        chunks[1].inner(&Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut ScrollbarState::new(app.max_scroll as usize).position(app.scroll as usize),
    );

    // Cursor position
    f.set_cursor(chunks[0].x + app.input.len() as u16 + 1, chunks[0].y + 1);
}

fn run_app() -> Result<()> {
    // Terminal initialization
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // App state
    let mut app = App::new();

    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        // Exit
                        KeyCode::Char('q')
                            if key
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL) =>
                        {
                            break;
                        }
                        // Submit query
                        KeyCode::Enter => {
                            app.submit_query()?;
                        }
                        // Handle backspace
                        KeyCode::Backspace => {
                            app.input.pop();
                        }
                        // Handle typing
                        KeyCode::Char(c) => {
                            app.input.push(c);
                        }
                        // Scrolling
                        KeyCode::Up => {
                            if app.scroll > 0 {
                                app.scroll -= 1;
                            }
                        }
                        KeyCode::Down => {
                            if app.scroll < app.max_scroll {
                                app.scroll += 1;
                            }
                        }
                        KeyCode::PageUp => {
                            app.scroll = app.scroll.saturating_sub(10);
                        }
                        KeyCode::PageDown => {
                            app.scroll = std::cmp::min(app.scroll + 10, app.max_scroll);
                        }
                        _ => {}
                    }
                }
            }
        }

        // Calculate max scroll based on response length
        let response_lines = app.response.lines().count() as u16;
        app.max_scroll = response_lines.saturating_sub(terminal.size()?.height - 5);
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

fn main() -> Result<()> {
    if let Err(err) = run_app() {
        println!("Error: {:?}", err);
    }
    Ok(())
}

// Required for crossterm
use std::io::Read;

