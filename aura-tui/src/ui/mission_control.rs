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
    let mut speed = 0;
    let mut total = 0;
    let mut completed = 0;
    if let Some(dl) = app.downloads.iter().find(|d| d.gid == gid) {
        name = dl.name.clone();
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

    let text = vec![
        Line::from(vec![Span::styled(
            format!("Mission Control: {}", name),
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(format!("GID: {}", gid)),
        Line::from(""),
        Line::from(vec![Span::styled(
            "[Press 'f' to open File Selector]",
            Style::default().fg(app.theme.highlight),
        )]),
        Line::from(vec![Span::styled(
            "[Press Esc to return to Dashboard]",
            Style::default().fg(app.theme.highlight),
        )]),
    ];
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
                .fg(app.theme.success)
                .bg(app.theme.background),
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

    let history = app
        .speed_history
        .get(gid)
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
    f.render_widget(sparkline, mc_chunks[2]);
}
