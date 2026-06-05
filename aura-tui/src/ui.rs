use crate::app::{App, ViewState};
use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap},
};

pub fn draw_ui(f: &mut ratatui::Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(3), // Header
                Constraint::Min(0),    // Main Content
                Constraint::Length(3), // Footer
            ]
            .as_ref(),
        )
        .split(f.area());

    // 1. Draw Header
    let header_text = match &app.view_state {
        ViewState::Dashboard => "COMMAND CENTER",
        ViewState::MissionControl(_gid) => "MISSION CONTROL",
    };

    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            " AURA ",
            Style::default()
                .bg(app.theme.primary)
                .fg(app.theme.background)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | "),
        Span::styled(header_text, Style::default().fg(app.theme.accent)),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.primary)),
    );
    f.render_widget(header, chunks[0]);

    // 2. Draw Main Content
    match &app.view_state {
        ViewState::Dashboard => {
            let main_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(60), Constraint::Percentage(40)].as_ref())
                .split(chunks[1]);

            // Left side: Task Table
            let header_cells = ["Name", "Status", "Progress", "Size", "GID"]
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
            let detail_text = if let Some(i) = app.table_state.selected() {
                if let Some(dl) = app.downloads.get(i) {
                    let total = dl.total_length.parse::<u64>().unwrap_or(0);
                    let completed = dl.completed_length.parse::<u64>().unwrap_or(0);
                    let progress = if total > 0 {
                        (completed as f64 / total as f64) * 100.0
                    } else {
                        0.0
                    };

                    vec![
                        Line::from(vec![Span::styled(
                            "Mission Details",
                            Style::default().add_modifier(Modifier::BOLD),
                        )]),
                        Line::from(""),
                        Line::from(format!("Name: {}", dl.name)),
                        Line::from(format!("GID:  {}", dl.gid)),
                        Line::from(format!("Status: {}", dl.status)),
                        Line::from(format!("Size: {}", ByteSize::b(total))),
                        Line::from(format!("Downloaded: {}", ByteSize::b(completed))),
                        Line::from(format!("Progress: {:.1}%", progress)),
                        Line::from(""),
                        Line::from(vec![Span::styled(
                            "[Press Enter to view Mission Control]",
                            Style::default().fg(app.theme.highlight),
                        )]),
                    ]
                } else {
                    vec![Line::from("No mission selected.")]
                }
            } else {
                vec![Line::from("No missions available.")]
            };

            let detail_panel = Paragraph::new(detail_text)
                .block(Block::default().borders(Borders::ALL).title(" Details "))
                .wrap(Wrap { trim: true });
            f.render_widget(detail_panel, main_chunks[1]);
        }
        ViewState::MissionControl(gid) => {
            // Find the download info if possible
            let mut name = "Unknown".to_string();
            if let Some(dl) = app.downloads.iter().find(|d| &d.gid == gid) {
                name = dl.name.clone();
            }

            let text = vec![
                Line::from(vec![Span::styled(
                    format!("Mission Control: {}", name),
                    Style::default().add_modifier(Modifier::BOLD),
                )]),
                Line::from(format!("GID: {}", gid)),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "Sparkline and Advanced Details coming in next iteration.",
                    Style::default().fg(app.theme.warning),
                )]),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "[Press Esc to return to Dashboard]",
                    Style::default().fg(app.theme.highlight),
                )]),
            ];
            let mc_panel = Paragraph::new(text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Mission Control "),
                )
                .wrap(Wrap { trim: true });
            f.render_widget(mc_panel, chunks[1]);
        }
    }

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
        match &app.view_state {
            ViewState::Dashboard => Line::from(vec![
                Span::styled(
                    " COMMANDS ",
                    Style::default()
                        .bg(app.theme.accent)
                        .fg(app.theme.background),
                ),
                Span::raw(
                    " [q] Quit | [p] Pause | [r] Resume | [j/k/g/G] Navigate | [Enter] Select ",
                ),
            ]),
            ViewState::MissionControl(_) => Line::from(vec![
                Span::styled(
                    " COMMANDS ",
                    Style::default()
                        .bg(app.theme.accent)
                        .fg(app.theme.background),
                ),
                Span::raw(" [q] Quit | [Esc] Back to Dashboard "),
            ]),
        }
    };

    let footer = Paragraph::new(footer_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.accent)),
    );
    f.render_widget(footer, chunks[2]);
}
