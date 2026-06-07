use crate::app::{App, ViewState};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

mod dashboard;
mod file_selector;
mod mission_control;

use dashboard::draw_dashboard;
use file_selector::draw_file_selector;
use mission_control::draw_mission_control;

pub fn draw_ui(f: &mut Frame, app: &mut App) {
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

    draw_header(f, app, chunks[0]);

    let view_state = app.view_state.clone();
    match view_state {
        ViewState::Dashboard => draw_dashboard(f, app, chunks[1]),
        ViewState::MissionControl(gid) => draw_mission_control(f, app, chunks[1], &gid),
        ViewState::FileSelector(gid) => draw_file_selector(f, app, chunks[1], &gid),
    }

    draw_footer(f, app, chunks[2]);
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let header_text = match &app.view_state {
        ViewState::Dashboard => "COMMAND CENTER",
        ViewState::MissionControl(_) => "MISSION CONTROL",
        ViewState::FileSelector(_) => "FILE SELECTOR",
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
    f.render_widget(header, area);
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
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
                Span::raw(" [q] Quit | [f] File Selector | [Esc] Back to Dashboard "),
            ]),
            ViewState::FileSelector(_) => Line::from(vec![
                Span::styled(
                    " COMMANDS ",
                    Style::default()
                        .bg(app.theme.accent)
                        .fg(app.theme.background),
                ),
                Span::raw(" [q] Quit | [Space] Toggle | [s] Save | [Esc] Back to Mission Control "),
            ]),
        }
    };

    let footer = Paragraph::new(footer_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.accent)),
    );
    f.render_widget(footer, area);
}
