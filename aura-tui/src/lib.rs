mod app;
mod theme;
mod ui;

use anyhow::Result;
use app::App;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};
use std::{
    io,
    time::{Duration, Instant},
};
use ui::draw_ui;

pub async fn run() -> Result<()> {
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

async fn run_loop<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()>
where
    <B as Backend>::Error: Send + Sync + 'static,
{
    let mut last_tick = Instant::now();
    let tick_rate = Duration::from_millis(500);
    loop {
        terminal.draw(|f| draw_ui(f, app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') {
                    app.should_quit = true;
                } else if key.code == KeyCode::Char('?') {
                    if app.view_state == app::ViewState::Help {
                        app.view_state = app::ViewState::Dashboard;
                    } else {
                        app.view_state = app::ViewState::Help;
                    }
                } else {
                    let view_state = app.view_state.clone();
                    match view_state {
                        app::ViewState::Dashboard => match key.code {
                            KeyCode::Down | KeyCode::Char('j') => app.next(),
                            KeyCode::Up | KeyCode::Char('k') => app.previous(),
                            KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter => {
                                let filtered = app.filtered_downloads();
                                if let Some(i) = app.table_state.selected() {
                                    if let Some(dl) = filtered.get(i) {
                                        app.view_state =
                                            app::ViewState::MissionControl(dl.gid.clone());
                                    }
                                }
                            }
                            KeyCode::Home | KeyCode::Char('g') => app.first(),
                            KeyCode::End | KeyCode::Char('G') => app.last(),
                            KeyCode::Char('/') => {
                                app.search_query.clear();
                                app.view_state = app::ViewState::Search;
                            }
                            KeyCode::Char('p') => {
                                app.pause_selected().await?;
                            }
                            KeyCode::Char('r') => {
                                app.resume_selected().await?;
                            }
                            KeyCode::Char('a') => {
                                app.discovery_input.clear();
                                app.view_state = app::ViewState::Discovery;
                            }
                            _ => {}
                        },
                        app::ViewState::MissionControl(gid) => match key.code {
                            KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') => {
                                app.view_state = app::ViewState::Dashboard
                            }
                            KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('f') => {
                                let gid_clone = gid.clone();
                                app.fetch_files(&gid_clone).await?;
                                app.view_state = app::ViewState::FileSelector(gid.clone())
                            }
                            KeyCode::Char('r') => {
                                let gid_clone = gid.clone();
                                if let Ok(id) = gid_clone.parse::<u64>() {
                                    app.client
                                        .post("http://localhost:6800/jsonrpc")
                                        .json(&serde_json::json!({
                                            "jsonrpc": "2.0",
                                            "method": "aura.refreshUri",
                                            "params": [id.to_string()],
                                            "id": "tui-refresh"
                                        }))
                                        .send()
                                        .await?;
                                }
                            }
                            _ => {}
                        },
                        app::ViewState::FileSelector(gid) => match key.code {
                            KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') => {
                                app.view_state = app::ViewState::MissionControl(gid.clone());
                            }
                            KeyCode::Down | KeyCode::Char('j') => app.file_next(),
                            KeyCode::Up | KeyCode::Char('k') => app.file_previous(),
                            KeyCode::Char(' ') | KeyCode::Enter => {
                                app.toggle_file_selection();
                            }
                            KeyCode::Char('s') => {
                                app.submit_file_selection(&gid).await?;
                                app.view_state = app::ViewState::MissionControl(gid.clone());
                            }
                            _ => {}
                        },
                        app::ViewState::Discovery => match key.code {
                            KeyCode::Esc => {
                                app.discovery_input.clear();
                                app.view_state = app::ViewState::Dashboard;
                            }
                            KeyCode::Enter => {
                                app.submit_discovery().await?;
                            }
                            KeyCode::Char(c) => {
                                app.discovery_input.push(c);
                            }
                            KeyCode::Backspace => {
                                app.discovery_input.pop();
                            }
                            KeyCode::Tab => {
                                app.discovery_recursive = !app.discovery_recursive;
                            }
                            _ => {}
                        },
                        app::ViewState::Search => match key.code {
                            KeyCode::Esc | KeyCode::Enter => {
                                app.view_state = app::ViewState::Dashboard;
                            }
                            KeyCode::Char(c) => {
                                app.search_query.push(c);
                                app.clamp_selection();
                            }
                            KeyCode::Backspace => {
                                app.search_query.pop();
                                app.clamp_selection();
                            }
                            _ => {}
                        },
                        app::ViewState::CommandPalette => match key.code {
                            KeyCode::Esc => {
                                app.command_input.clear();
                                app.view_state = app::ViewState::Dashboard;
                            }
                            KeyCode::Enter => {
                                app.submit_command().await?;
                            }
                            KeyCode::Char(c) => {
                                app.command_input.push(c);
                            }
                            KeyCode::Backspace => {
                                app.command_input.pop();
                            }
                            _ => {}
                        },
                        app::ViewState::Help => {
                            if key.code == KeyCode::Esc || key.code == KeyCode::Char('?') {
                                app.view_state = app::ViewState::Dashboard;
                            }
                        }
                    }
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
