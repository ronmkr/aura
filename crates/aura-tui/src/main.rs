use anyhow::Result;
use bytesize::ByteSize;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Gauge, Paragraph, Row, Table, TableState},
    Terminal,
};
use serde_json::json;
use std::{
    io,
    time::{Duration, Instant},
};

// Default Theme Tokens (fallback)
const GALACTIC_BLUE: Color = Color::Rgb(0, 0, 255);
const NEBULA_CYAN: Color = Color::Rgb(0, 255, 255);
const STAR_YELLOW: Color = Color::Rgb(255, 255, 0);

#[allow(dead_code)]
struct Theme {
    primary: Color,
    accent: Color,
    highlight: Color,
    background: Color,
    foreground: Color,
    success: Color,
    error: Color,
    warning: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            primary: GALACTIC_BLUE,
            accent: NEBULA_CYAN,
            highlight: STAR_YELLOW,
            background: Color::Black,
            foreground: Color::White,
            success: Color::Green,
            error: Color::Red,
            warning: Color::Yellow,
        }
    }
}

impl Theme {
    fn from_config(config: &serde_json::Value) -> Self {
        let theme_json = &config["general"]["theme"];
        Self {
            primary: parse_hex(theme_json["primary"].as_str()).unwrap_or(GALACTIC_BLUE),
            accent: parse_hex(theme_json["accent"].as_str()).unwrap_or(NEBULA_CYAN),
            highlight: parse_hex(theme_json["highlight"].as_str()).unwrap_or(STAR_YELLOW),
            background: parse_hex(theme_json["background"].as_str()).unwrap_or(Color::Black),
            foreground: parse_hex(theme_json["foreground"].as_str()).unwrap_or(Color::White),
            success: parse_hex(theme_json["success"].as_str()).unwrap_or(Color::Green),
            error: parse_hex(theme_json["error"].as_str()).unwrap_or(Color::Red),
            warning: parse_hex(theme_json["warning"].as_str()).unwrap_or(Color::Yellow),
        }
    }
}

fn parse_hex(hex: Option<&str>) -> Option<Color> {
    let hex = hex?;
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

struct App {
    client: reqwest::Client,
    downloads: Vec<DownloadInfo>,
    table_state: TableState,
    should_quit: bool,
    error_msg: Option<String>,
    theme: Theme,
}

#[derive(Debug, serde::Deserialize, Clone)]
struct DownloadInfo {
    gid: String,
    status: String,
    #[serde(rename = "totalLength")]
    total_length: String,
    #[serde(rename = "completedLength")]
    completed_length: String,
    name: String,
}

impl App {
    fn new() -> App {
        let mut table_state = TableState::default();
        table_state.select(Some(0));
        App {
            client: reqwest::Client::new(),
            downloads: Vec::new(),
            table_state,
            should_quit: false,
            error_msg: None,
            theme: Theme::default(),
        }
    }

    async fn fetch_theme(&mut self) -> Result<()> {
        let res = self
            .client
            .post("http://localhost:6800/jsonrpc")
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "aura.getConfig",
                "id": "tui-theme"
            }))
            .send()
            .await;

        if let Ok(response) = res {
            let body: serde_json::Value = response.json().await?;
            if let Some(result) = body.get("result") {
                self.theme = Theme::from_config(result);
            }
        }
        Ok(())
    }

    async fn tick(&mut self) -> Result<()> {
        let res = self
            .client
            .post("http://localhost:6800/jsonrpc")
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "aria2.tellActive",
                "id": "tui"
            }))
            .send()
            .await;

        match res {
            Ok(response) => {
                let body: serde_json::Value = response.json().await?;
                if let Some(result) = body.get("result") {
                    self.downloads = serde_json::from_value(result.clone())?;
                    self.error_msg = None;
                }
            }
            Err(e) => {
                self.error_msg = Some(format!("Daemon Connection Error: {}", e));
            }
        }
        Ok(())
    }

    fn next(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => {
                if i >= self.downloads.len().saturating_sub(1) {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    fn previous(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.downloads.len().saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    async fn pause_selected(&mut self) -> Result<()> {
        if let Some(i) = self.table_state.selected() {
            if let Some(dl) = self.downloads.get(i) {
                let _ = self
                    .client
                    .post("http://localhost:6800/jsonrpc")
                    .json(&json!({
                        "jsonrpc": "2.0",
                        "method": "aria2.pause",
                        "params": [dl.gid],
                        "id": "tui"
                    }))
                    .send()
                    .await;
            }
        }
        Ok(())
    }

    async fn resume_selected(&mut self) -> Result<()> {
        if let Some(i) = self.table_state.selected() {
            if let Some(dl) = self.downloads.get(i) {
                let _ = self
                    .client
                    .post("http://localhost:6800/jsonrpc")
                    .json(&json!({
                        "jsonrpc": "2.0",
                        "method": "aria2.unpause",
                        "params": [dl.gid],
                        "id": "tui"
                    }))
                    .send()
                    .await;
            }
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let _ = app.fetch_theme().await; // Initial theme fetch

    let res = run_loop(&mut terminal, &mut app).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("TUI Error: {:?}", err);
    }

    Ok(())
}

async fn run_loop<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    let mut last_tick = Instant::now();
    let tick_rate = Duration::from_millis(500);
    loop {
        terminal.draw(|f| draw_ui(f, app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => app.should_quit = true,
                    KeyCode::Down | KeyCode::Char('j') => app.next(),
                    KeyCode::Up | KeyCode::Char('k') => app.previous(),
                    KeyCode::Char('p') => {
                        app.pause_selected().await?;
                    }
                    KeyCode::Char('r') => {
                        app.resume_selected().await?;
                    }
                    _ => {}
                }
            }
        }

        if app.should_quit {
            return Ok(());
        }

        if last_tick.elapsed() >= tick_rate {
            app.tick().await?;
            last_tick = Instant::now();
        }
    }
}

fn draw_ui(f: &mut ratatui::Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(3), // Header
                Constraint::Min(10),   // Main List
                Constraint::Length(7), // Details
                Constraint::Length(1), // Footer
            ]
            .as_ref(),
        )
        .split(f.size());

    draw_header(f, chunks[0], app);
    draw_task_list(f, chunks[1], app);
    draw_details(f, chunks[2], app);
    draw_footer(f, chunks[3], app);

    if let Some(ref err) = app.error_msg {
        let area = centered_rect(60, 20, f.size());
        let p = Paragraph::new(err.clone())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" CRITICAL ERROR ")
                    .fg(app.theme.error),
            )
            .wrap(ratatui::widgets::Wrap { trim: true });
        f.render_widget(ratatui::widgets::Clear, area);
        f.render_widget(p, area);
    }
}

fn draw_header(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let total_active = app.downloads.len();
    let title = Line::from(vec![
        Span::styled(
            " AURA ",
            Style::default()
                .fg(app.theme.background)
                .bg(app.theme.highlight)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " 🌌 Galactic Pilot ",
            Style::default().fg(app.theme.highlight),
        ),
        Span::raw(" | "),
        Span::styled(
            format!(" Tasks: {} ", total_active),
            Style::default().fg(app.theme.accent),
        ),
    ]);

    let header = Paragraph::new(title).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.primary)),
    );
    f.render_widget(header, area);
}

fn draw_task_list(f: &mut ratatui::Frame, area: Rect, app: &mut App) {
    let selected_style = Style::default()
        .add_modifier(Modifier::REVERSED)
        .fg(app.theme.accent);
    let header_style = Style::default()
        .fg(app.theme.highlight)
        .bg(app.theme.primary)
        .add_modifier(Modifier::BOLD);

    let header_cells = ["ID", "Name", "Status", "Progress", "Size"]
        .iter()
        .map(|h| Cell::from(*h));
    let header = Row::new(header_cells).style(header_style).height(1);

    let rows = app.downloads.iter().map(|item| {
        let total = item.total_length.parse::<u64>().unwrap_or(0);
        let completed = item.completed_length.parse::<u64>().unwrap_or(0);
        let progress = if total > 0 {
            (completed as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        let status_style = match item.status.as_str() {
            "downloading" => Style::default().fg(app.theme.success),
            "paused" => Style::default().fg(Color::Gray),
            "error" => Style::default().fg(app.theme.error),
            _ => Style::default().fg(app.theme.foreground),
        };

        let cells = vec![
            Cell::from(item.gid.chars().take(8).collect::<String>()),
            Cell::from(item.name.clone()),
            Cell::from(item.status.clone()).style(status_style),
            Cell::from(format!("{:.1}%", progress)),
            Cell::from(ByteSize::b(total).to_string()),
        ];
        Row::new(cells).height(1)
    });

    let t = Table::new(
        rows,
        [
            Constraint::Length(10),
            Constraint::Min(30),
            Constraint::Length(15),
            Constraint::Length(10),
            Constraint::Length(12),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Active Swarms ")
            .border_style(Style::default().fg(app.theme.primary)),
    )
    .highlight_style(selected_style)
    .highlight_symbol(">> ");

    f.render_stateful_widget(t, area, &mut app.table_state);
}

fn draw_details(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let selected = app
        .table_state
        .selected()
        .and_then(|i| app.downloads.get(i));

    let content = if let Some(dl) = selected {
        let total = dl.total_length.parse::<u64>().unwrap_or(0);
        let completed = dl.completed_length.parse::<u64>().unwrap_or(0);
        let progress = if total > 0 {
            completed as f64 / total as f64
        } else {
            0.0
        };

        let details = vec![
            Line::from(vec![
                Span::styled("Name: ", Style::default().fg(app.theme.highlight)),
                Span::raw(&dl.name),
            ]),
            Line::from(vec![
                Span::styled("GID:  ", Style::default().fg(app.theme.highlight)),
                Span::raw(&dl.gid),
            ]),
            Line::from(vec![
                Span::styled("Phase: ", Style::default().fg(app.theme.highlight)),
                Span::styled(&dl.status, Style::default().fg(app.theme.accent)),
            ]),
        ];

        f.render_widget(Paragraph::new(details), area);

        // Render a progress gauge at the bottom of the details block
        let gauge_area = Rect::new(area.x + 2, area.y + 4, area.width - 4, 1);
        let gauge = Gauge::default()
            .block(Block::default())
            .gauge_style(Style::default().fg(app.theme.accent).bg(app.theme.primary))
            .ratio(progress)
            .label(format!("{:.1}%", progress * 100.0));
        f.render_widget(gauge, gauge_area);

        return;
    } else {
        "Select a task to view telemetry data..."
    };

    let p = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Telemetry Dashboard ")
            .border_style(Style::default().fg(app.theme.primary)),
    );
    f.render_widget(p, area);
}

fn draw_footer(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let keys = vec![
        Span::styled(
            " (q) ",
            Style::default().fg(app.theme.background).bg(Color::Gray),
        ),
        Span::raw(" Quit "),
        Span::styled(
            " (p) ",
            Style::default().fg(app.theme.background).bg(Color::Gray),
        ),
        Span::raw(" Pause "),
        Span::styled(
            " (r) ",
            Style::default().fg(app.theme.background).bg(Color::Gray),
        ),
        Span::raw(" Resume "),
        Span::styled(
            " (↑↓) ",
            Style::default().fg(app.theme.background).bg(Color::Gray),
        ),
        Span::raw(" Navigate "),
    ];
    let p = Paragraph::new(Line::from(keys));
    f.render_widget(p, area);
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
