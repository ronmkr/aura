use crate::app::App;
use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};

pub fn draw_file_selector(f: &mut Frame, app: &mut App, area: Rect, _gid: &str) {
    let header_cells = ["S", "Path", "Size"].iter().map(|h| {
        Cell::from(*h).style(
            Style::default()
                .fg(app.ui.theme.highlight)
                .add_modifier(Modifier::BOLD),
        )
    });
    let header_row = Row::new(header_cells)
        .style(Style::default().bg(app.ui.theme.background))
        .height(1)
        .bottom_margin(1);

    let rows = app.data.files.iter().map(|item| {
        let selected_marker = if item.selected { "[x]" } else { "[ ]" };
        let path = item.path.join("/");

        Row::new(vec![
            Cell::from(selected_marker),
            Cell::from(path),
            Cell::from(ByteSize::b(item.length).to_string()),
        ])
        .height(1)
        .bottom_margin(0)
    });

    let t = Table::new(
        rows,
        [
            Constraint::Length(3),  // [x]
            Constraint::Min(20),    // Path
            Constraint::Length(10), // Size
        ],
    )
    .header(header_row)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" File Selection (Space/Enter to Toggle, 's' to Save) "),
    )
    .row_highlight_style(
        Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(app.ui.theme.accent),
    )
    .highlight_symbol(">> ");

    f.render_stateful_widget(t, area, &mut app.ui.file_table_state);
}
