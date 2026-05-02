use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table},
    Terminal,
};
use serde_json::json;
use std::{io, time::{Duration, Instant}};

struct App {
    client: reqwest::Client,
    downloads: Vec<DownloadInfo>,
}

#[derive(Debug, serde::Deserialize)]
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
        App {
            client: reqwest::Client::new(),
            downloads: Vec::new(),
        }
    }

    async fn tick(&mut self) -> Result<()> {
        let res = self.client.post("http://localhost:6800/jsonrpc")
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "aria2.tellActive",
                "id": "tui"
            }))
            .send()
            .await?;

        let body: serde_json::Value = res.json().await?;
        if let Some(result) = body.get("result") {
            self.downloads = serde_json::from_value(result.clone())?;
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let mut app = App::new();
    let res = run_app(&mut terminal, &mut app).await;

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    let mut last_tick = Instant::now();
    let tick_rate = Duration::from_millis(500);
    loop {
        terminal.draw(|f| ui(f, app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if let KeyCode::Char('q') = key.code {
                    return Ok(());
                }
            }
        }
        if last_tick.elapsed() >= tick_rate {
            let _ = app.tick().await;
            last_tick = Instant::now();
        }
    }
}

fn ui(f: &mut ratatui::Frame, app: &App) {
    let rects = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(100)].as_ref())
        .split(f.size());

    let selected_style = Style::default().add_modifier(Modifier::REVERSED);
    let normal_style = Style::default().bg(Color::Blue);
    let header_cells = ["Name", "Status", "Progress", "GID"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow)));
    let header = Row::new(header_cells)
        .style(normal_style)
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

        let cells = vec![
            Cell::from(item.name.clone()),
            Cell::from(item.status.clone()),
            Cell::from(format!("{:.1}%", progress)),
            Cell::from(item.gid.clone()),
        ];
        Row::new(cells).height(1)
    });

    let t = Table::new(rows, [
        Constraint::Percentage(40),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
    ])
    .header(header)
    .block(Block::default().borders(Borders::ALL).title("Aura Downloads"))
    .highlight_style(selected_style)
    .highlight_symbol(">> ");

    f.render_widget(t, rects[0]);
}
