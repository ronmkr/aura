use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn draw_discovery(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(3), // Instructions
                Constraint::Length(3), // Input field
                Constraint::Length(3), // Options
                Constraint::Min(0),    // Padding
            ]
            .as_ref(),
        )
        .split(area);

    let instructions = Paragraph::new(
        "Enter a Magnet URI, Info-Hash, or Local Path to a .torrent / .metalink file / directory.",
    )
    .style(Style::default().fg(app.ui.theme.highlight));
    f.render_widget(instructions, chunks[0]);

    let input = Paragraph::new(app.ui.discovery_input.as_str())
        .block(Block::default().borders(Borders::ALL).title(" Input "))
        .style(Style::default().fg(app.ui.theme.accent));
    f.render_widget(input, chunks[1]);

    let recursive_status = if app.ui.discovery_recursive {
        "ENABLED"
    } else {
        "DISABLED"
    };
    let options = Paragraph::new(format!("Recursive Scanning: {}", recursive_status)).style(
        Style::default().fg(if app.ui.discovery_recursive {
            app.ui.theme.success
        } else {
            app.ui.theme.foreground
        }),
    );
    f.render_widget(options, chunks[2]);
}
