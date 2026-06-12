use crate::app::App;
use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Sparkline, Wrap},
    Frame,
};

pub fn draw_mission_control(f: &mut Frame, app: &mut App, area: Rect, gid: &str) {
    let mut name = "Unknown".to_string();
    let mut status = "unknown".to_string();
    let mut speed = 0;
    let mut total = 0;
    let mut completed = 0;
    if let Some(dl) = app.data.downloads.iter().find(|d| d.gid == gid) {
        name = dl.name.clone();
        status = dl.status.clone();
        speed = dl.download_speed.parse().unwrap_or(0);
        total = dl.total_length.parse().unwrap_or(0);
        completed = dl.completed_length.parse().unwrap_or(0);
    }

    let mc_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(8), // Header info
                Constraint::Length(3), // Progress
                Constraint::Min(3),    // Sparkline
            ]
            .as_ref(),
        )
        .split(area);

    let mut error_hint = None;
    if status == "error" && app.data.downloads.iter().any(|d| d.gid == gid) {
        // Ideally we'd have a specific error code, but for now we'll string match
        // as a "World-Class" heuristic
        let err_msg = app.ui.error_msg.clone().unwrap_or_default().to_lowercase();
        if err_msg.contains("disk full") || err_msg.contains("no space") {
            error_hint = Some("[Action: Clear cache or free space, then hit 'r']");
        } else if err_msg.contains("connection") || err_msg.contains("timeout") {
            error_hint = Some("[Action: Check network and hit 'r' to retry]");
        } else {
            error_hint = Some("[Action: Hit 'r' to attempt manual retry]");
        }
    }

    let mut text = vec![
        Line::from(vec![Span::styled(
            format!("Mission Control: {}", name),
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(format!("GID: {}", gid)),
        Line::from(format!("Status: {}", status)),
        Line::from(""),
    ];

    if let Some(hint) = error_hint {
        text.push(Line::from(vec![Span::styled(
            hint,
            Style::default()
                .fg(app.ui.theme.error)
                .add_modifier(Modifier::BOLD),
        )]));
        text.push(Line::from(""));
    }

    text.extend(vec![
        Line::from(vec![Span::styled(
            "[Press 'f' to open File Selector]",
            Style::default().fg(app.ui.theme.highlight),
        )]),
        Line::from(vec![Span::styled(
            "[Press 'r' to Refresh/Retry Mission]",
            Style::default().fg(app.ui.theme.highlight),
        )]),
        Line::from(vec![Span::styled(
            "[Press Esc to return to Dashboard]",
            Style::default().fg(app.ui.theme.highlight),
        )]),
    ]);
    let mc_panel = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title(" Overview "))
        .wrap(Wrap { trim: true });
    f.render_widget(mc_panel, mc_chunks[0]);

    let progress = if total > 0 {
        (completed as f64 / total as f64) * 100.0
    } else {
        0.0
    };
    let progress_gauge = Gauge::default()
        .block(Block::default().title(" Progress ").borders(Borders::ALL))
        .gauge_style(
            Style::default()
                .fg(app.ui.theme.success)
                .bg(app.ui.theme.background),
        )
        .ratio(if total > 0 {
            completed as f64 / total as f64
        } else {
            0.0
        })
        .label(format!(
            "{:.1}% ({}/{})",
            progress,
            ByteSize::b(completed),
            ByteSize::b(total)
        ));
    f.render_widget(progress_gauge, mc_chunks[1]);

    let history_data: Vec<u64> = app
        .data
        .speed_history
        .get(gid)
        .map(|h| h.iter().copied().collect::<Vec<u64>>())
        .unwrap_or_default();
    let max_speed = history_data.iter().copied().max().unwrap_or(1).max(1);
    let sparkline = Sparkline::default()
        .block(
            Block::default()
                .title(format!(" Throughput ({}/s) ", ByteSize::b(speed)))
                .borders(Borders::ALL),
        )
        .data(&history_data)
        .max(max_speed)
        .style(Style::default().fg(app.ui.theme.accent));
    f.render_widget(sparkline, mc_chunks[2]);
}
