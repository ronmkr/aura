use crate::app::App;
use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Gauge, Paragraph, Row, Sparkline, Table, Wrap},
    Frame,
};

pub fn draw_dashboard(f: &mut Frame, app: &mut App, area: Rect) {
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)].as_ref())
        .split(area);

    // Left side: Task Table
    let header_cells = ["Name", "Status", "Progress", "Speed", "Size"]
        .iter()
        .map(|h| {
            Cell::from(*h).style(
                Style::default()
                    .fg(app.theme.highlight)
                    .add_modifier(Modifier::BOLD),
            )
        });
    let header_row = Row::new(header_cells)
        .style(Style::default().bg(app.theme.background))
        .height(1)
        .bottom_margin(1);

    let rows = app.downloads.iter().map(|item| {
        let total = item.total_length.parse::<u64>().unwrap_or(0);
        let completed = item.completed_length.parse::<u64>().unwrap_or(0);
        let speed = item.download_speed.parse::<u64>().unwrap_or(0);
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

        Row::new(vec![
            Cell::from(item.name.clone()),
            Cell::from(item.status.clone()).style(status_style),
            Cell::from(format!("{:.1}%", progress)),
            Cell::from(format!("{}/s", ByteSize::b(speed))),
            Cell::from(ByteSize::b(total).to_string()),
        ])
        .height(1)
        .bottom_margin(0)
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
    .header(header_row)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Active Missions "),
    )
    .row_highlight_style(
        Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(app.theme.accent),
    )
    .highlight_symbol(">> ");

    f.render_stateful_widget(t, main_chunks[0], &mut app.table_state);

    // Right side: Detail Panel
    if let Some(i) = app.table_state.selected() {
        if let Some(dl) = app.downloads.get(i) {
            let total = dl.total_length.parse::<u64>().unwrap_or(0);
            let completed = dl.completed_length.parse::<u64>().unwrap_or(0);
            let speed = dl.download_speed.parse::<u64>().unwrap_or(0);
            let progress = if total > 0 {
                (completed as f64 / total as f64) * 100.0
            } else {
                0.0
            };

            let detail_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(
                    [
                        Constraint::Length(8), // Text info
                        Constraint::Length(3), // Progress Gauge
                        Constraint::Min(3),    // Sparkline
                    ]
                    .as_ref(),
                )
                .split(main_chunks[1]);

            let text = vec![
                Line::from(vec![Span::styled(
                    "Mission Details",
                    Style::default().add_modifier(Modifier::BOLD),
                )]),
                Line::from(""),
                Line::from(format!("Name: {}", dl.name)),
                Line::from(format!("GID:  {}", dl.gid)),
                Line::from(format!("Status: {}", dl.status)),
                Line::from(format!("Size: {}", ByteSize::b(total))),
            ];

            let detail_panel = Paragraph::new(text)
                .block(Block::default().borders(Borders::ALL).title(" Details "))
                .wrap(Wrap { trim: true });
            f.render_widget(detail_panel, detail_chunks[0]);

            let progress_gauge = Gauge::default()
                .block(Block::default().title(" Progress ").borders(Borders::ALL))
                .gauge_style(
                    Style::default()
                        .fg(app.theme.success)
                        .bg(app.theme.background),
                )
                .ratio(if total > 0 {
                    completed as f64 / total as f64
                } else {
                    0.0
                })
                .label(format!("{:.1}%", progress));
            f.render_widget(progress_gauge, detail_chunks[1]);

            let history = app
                .speed_history
                .get(&dl.gid)
                .map(|h| h.iter().copied().collect::<Vec<_>>())
                .unwrap_or_default();
            let max_speed = history.iter().copied().max().unwrap_or(1).max(1);
            let sparkline = Sparkline::default()
                .block(
                    Block::default()
                        .title(format!(" Throughput ({}/s) ", ByteSize::b(speed)))
                        .borders(Borders::ALL),
                )
                .data(&history)
                .max(max_speed)
                .style(Style::default().fg(app.theme.accent));
            f.render_widget(sparkline, detail_chunks[2]);
        }
    } else {
        let detail_panel = Paragraph::new("No missions available.")
            .block(Block::default().borders(Borders::ALL).title(" Details "));
        f.render_widget(detail_panel, main_chunks[1]);
    }
}
