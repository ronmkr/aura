use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Cell, Clear, Row, Table},
    Frame,
};

pub fn draw_help(f: &mut Frame, app: &App) {
    let area = f.area();

    let help_area = centered_rect(60, 60, area);
    f.render_widget(Clear, help_area); // Clear the background for the modal

    let header_cells = ["Context", "Command", "Action"].iter().map(|h| {
        Cell::from(*h).style(
            Style::default()
                .fg(app.ui.theme.highlight)
                .add_modifier(Modifier::BOLD),
        )
    });
    let header_row = Row::new(header_cells)
        .style(Style::default().bg(app.ui.theme.background))
        .height(1);

    let rows = vec![
        Row::new(vec!["Global", "q", "Quit Aura TUI"]),
        Row::new(vec!["Global", "?", "Toggle Help Modal"]),
        Row::new(vec!["Dashboard", "j / Down", "Move Selection Down"]),
        Row::new(vec!["Dashboard", "k / Up", "Move Selection Up"]),
        Row::new(vec!["Dashboard", "g / Home", "Go to Top"]),
        Row::new(vec!["Dashboard", "G / End", "Go to Bottom"]),
        Row::new(vec!["Dashboard", "/", "Enter Search Mode"]),
        Row::new(vec!["Dashboard", "a", "Add Mission (Discovery)"]),
        Row::new(vec!["Dashboard", "p", "Pause Selected"]),
        Row::new(vec!["Dashboard", "r", "Resume Selected"]),
        Row::new(vec!["Dashboard", "Enter", "Open Mission Control"]),
        Row::new(vec!["Mission Control", "f", "Open File Selector"]),
        Row::new(vec!["Mission Control", "Esc", "Back to Dashboard"]),
        Row::new(vec!["File Selector", "Space / Enter", "Toggle File"]),
        Row::new(vec!["File Selector", "s", "Save Selection"]),
        Row::new(vec!["File Selector", "Esc", "Back to Mission Control"]),
        Row::new(vec!["Search / Discovery", "Esc", "Cancel / Exit"]),
        Row::new(vec!["Search / Discovery", "Enter", "Submit"]),
    ];

    let t = Table::new(
        rows,
        [
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(50),
        ],
    )
    .header(header_row)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Help / Keybindings "),
    )
    .column_spacing(1);

    f.render_widget(t, help_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1])[1]
}
