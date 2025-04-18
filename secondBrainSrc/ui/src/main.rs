use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Widget},
    Frame,
};
use std::{io, io::Write, net::TcpStream};
use unicode_width::UnicodeWidthStr;

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

    // Response area with markdown rendering
    let rendered_text = render_markdown(&app.response);

    let response_widget = Paragraph::new(rendered_text)
        .block(Block::default().borders(Borders::ALL).title("Summary"))
        .scroll((app.scroll, 0));

    f.render_widget(response_widget, chunks[1]);

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

// Renders markdown with proper table alignment
fn render_markdown(text: &str) -> Vec<Line> {
    let mut result = Vec::new();
    let mut in_table = false;
    let mut table_alignments = Vec::new();
    let mut table_column_widths = Vec::new();
    let mut table_rows = Vec::new();

    // Process markdown line by line
    for line in text.lines() {
        if line.trim().starts_with('|') && line.trim().ends_with('|') {
            // This is a table row
            if !in_table {
                // Start of a new table
                in_table = true;
                table_rows.clear();
                table_alignments.clear();
                table_column_widths.clear();
            }

            let row = line.trim();

            // Parse header separator to determine column alignments
            if row.contains("---") {
                // This is the header separator defining alignments
                let cols = row
                    .split('|')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.trim())
                    .collect::<Vec<_>>();

                for col in cols {
                    if col.starts_with(':') && col.ends_with(':') {
                        // Center aligned
                        table_alignments.push(Alignment::Center);
                    } else if col.ends_with(':') {
                        // Right aligned
                        table_alignments.push(Alignment::Right);
                    } else {
                        // Left aligned (default)
                        table_alignments.push(Alignment::Left);
                    }
                }
            } else {
                // This is a data row
                let cells = row
                    .split('|')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.trim().to_string())
                    .collect::<Vec<_>>();

                // Update column widths
                if table_column_widths.is_empty() {
                    // Initialize column widths
                    table_column_widths = cells.iter().map(|s| s.width()).collect();
                } else {
                    // Update column widths if needed
                    for (i, cell) in cells.iter().enumerate() {
                        if i < table_column_widths.len() {
                            table_column_widths[i] = table_column_widths[i].max(cell.width());
                        } else {
                            table_column_widths.push(cell.width());
                        }
                    }
                }

                table_rows.push(cells);
            }
        } else if in_table {
            // End of table - render it
            in_table = false;

            // Apply alignments and padding to make the table look nice
            for row in &table_rows {
                let mut line_spans = Vec::new();

                for (i, cell) in row.iter().enumerate() {
                    if i >= table_column_widths.len() {
                        continue;
                    }

                    let width = table_column_widths[i];
                    let alignment = if i < table_alignments.len() {
                        table_alignments[i]
                    } else {
                        Alignment::Left
                    };
                    let styled_cell = match alignment {
                        Alignment::Left => format!("{:<width$}", cell, width = width),
                        Alignment::Center => format!("{:^width$}", cell, width = width),
                        Alignment::Right => format!("{:>width$}", cell, width = width),
                    };

                    let style = if i == 0 || table_rows.first() == Some(row) {
                        // First column or header row - make it bold
                        Style::default().add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };

                    line_spans.push(Span::styled(styled_cell, style));
                    line_spans.push(Span::raw(" | "));
                }

                // Remove the last separator
                if !line_spans.is_empty() {
                    line_spans.pop();
                }

                result.push(Line::from(line_spans));
            }

            // Add an empty line after table
            result.push(Line::from(""));

            // Process current line
            result.push(Line::from(line.to_string()));
        } else {
            // Regular text - add formatting for headers, bold, etc.
            if line.starts_with("# ") {
                // H1 header
                result.push(Line::from(vec![Span::styled(
                    line[2..].to_string(),
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .fg(Color::Yellow),
                )]));
            } else if line.starts_with("## ") {
                // H2 header
                result.push(Line::from(vec![Span::styled(
                    line[3..].to_string(),
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .fg(Color::Blue),
                )]));
            } else if line.starts_with("### ") {
                // H3 header
                result.push(Line::from(vec![Span::styled(
                    line[4..].to_string(),
                    Style::default().add_modifier(Modifier::BOLD),
                )]));
            } else if line.starts_with("- ") {
                // Bullet point
                result.push(Line::from(vec![
                    Span::raw("• "),
                    Span::raw(line[2..].to_string()),
                ]));
            } else {
                // Regular text
                result.push(Line::from(line.to_string()));
            }
        }
    }

    // Handle case where table is at the end of text
    if in_table {
        // Render the final table
        for row in &table_rows {
            let mut line_spans = Vec::new();

            for (i, cell) in row.iter().enumerate() {
                if i >= table_column_widths.len() {
                    continue;
                }

                let width = table_column_widths[i];
                let alignment = if i < table_alignments.len() {
                    table_alignments[i]
                } else {
                    Alignment::Left
                };
                let styled_cell = match alignment {
                    Alignment::Left => format!("{:<width$}", cell, width = width),
                    Alignment::Center => format!("{:^width$}", cell, width = width),
                    Alignment::Right => format!("{:>width$}", cell, width = width),
                };

                let style = if i == 0 || table_rows.first() == Some(row) {
                    // First column or header row - make it bold
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                line_spans.push(Span::styled(styled_cell, style));
                line_spans.push(Span::raw(" | "));
            }

            // Remove the last separator
            if !line_spans.is_empty() {
                line_spans.pop();
            }

            result.push(Line::from(line_spans));
        }
    }

    result
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
                        KeyCode::Char('c')
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

        // Calculate max scroll based on rendered text length
        let rendered_lines = render_markdown(&app.response).len() as u16;
        app.max_scroll = rendered_lines.saturating_sub(terminal.size()?.height - 5);
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
        eprintln!("Error: {:?}", err);
    }
    Ok(())
}

// Required for crossterm
use std::io::Read;

