use crate::app::App;
use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};

pub fn draw_ui(f: &mut ratatui::Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(3), // Header
                Constraint::Min(0),    // Table
                Constraint::Length(3), // Footer
            ]
            .as_ref(),
        )
        .split(f.area());

    // 1. Draw Header
    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            " AURA ",
            Style::default()
                .bg(app.theme.primary)
                .fg(app.theme.background)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | "),
        Span::styled(
            "GALACTIC PILOT DASHBOARD",
            Style::default().fg(app.theme.accent),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.primary)),
    );
    f.render_widget(header, chunks[0]);

    // 2. Draw Table
    let header_cells = ["Name", "Status", "Progress", "Size", "GID"]
        .iter()
        .map(|h| {
            Cell::from(*h).style(
                Style::default()
                    .fg(app.theme.highlight)
                    .add_modifier(Modifier::BOLD),
            )
        });
    let header = Row::new(header_cells)
        .style(Style::default().bg(app.theme.background))
        .height(1)
        .bottom_margin(1);

    let rows = app.downloads.iter().map(|item| {
        let total = item.total_length.parse::<u64>().unwrap_or(0);
        let completed = item.completed_length.parse::<u64>().unwrap_or(0);
        let progress = if total > 0 {
            (completed as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        let status_style = match item.status.as_str() {
            "active" => Style::default().fg(app.theme.success),
            "paused" => Style::default().fg(app.theme.warning),
            "error" => Style::default().fg(app.theme.error),
            _ => Style::default().fg(app.theme.foreground),
        };

        let cells = vec![
            Cell::from(item.name.clone()),
            Cell::from(item.status.clone()).style(status_style),
            Cell::from(format!("{:.1}%", progress)),
            Cell::from(ByteSize::b(total).to_string()),
            Cell::from(item.gid.clone()),
        ];
        Row::new(cells).height(1).bottom_margin(0)
    });

    let t = Table::new(
        rows,
        [
            Constraint::Percentage(40),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(" Downloads "))
    .row_highlight_style(
        Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(app.theme.accent),
    )
    .highlight_symbol(">> ");

    f.render_stateful_widget(t, chunks[1], &mut app.table_state);

    // 3. Draw Footer / Status Bar
    let footer_text = if let Some(err) = &app.error_msg {
        Line::from(vec![
            Span::styled(
                " ERROR ",
                Style::default()
                    .bg(app.theme.error)
                    .fg(app.theme.background),
            ),
            Span::raw(" "),
            Span::styled(err, Style::default().fg(app.theme.error)),
        ])
    } else {
        Line::from(vec![
            Span::styled(
                " COMMANDS ",
                Style::default()
                    .bg(app.theme.accent)
                    .fg(app.theme.background),
            ),
            Span::raw(" [q] Quit | [p] Pause | [r] Resume | [j/k] Navigate "),
        ])
    };

    let footer = Paragraph::new(footer_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.accent)),
    );
    f.render_widget(footer, chunks[2]);
}
