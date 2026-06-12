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

pub async fn run(rpc_url: String, rpc_secret: Option<String>) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        crossterm::event::EnableBracketedPaste
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(rpc_url, rpc_secret);
    let _ = app.fetch_config().await; // Initial config fetch

    let res = run_loop(&mut terminal, &mut app).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        crossterm::event::DisableBracketedPaste
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
    loop {
        let tick_rate = app.tick_rate;
        terminal.draw(|f| draw_ui(f, app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => {
                    if key
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL)
                        && key.code == KeyCode::Char('p')
                    {
                        app.ui.command_input.clear();
                        app.ui.view_state = app::ViewState::CommandPalette;
                    } else if key.code == KeyCode::Char('q') {
                        app.should_quit = true;
                    } else if key.code == KeyCode::Char('?') {
                        if app.ui.view_state == app::ViewState::Help {
                            app.ui.view_state = app::ViewState::Dashboard;
                        } else {
                            app.ui.view_state = app::ViewState::Help;
                        }
                    } else {
                        let view_state = app.ui.view_state.clone();
                        match view_state {
                            app::ViewState::Dashboard => match key.code {
                                KeyCode::Down | KeyCode::Char('j') => app.next(),
                                KeyCode::Up | KeyCode::Char('k') => app.previous(),
                                KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter => {
                                    let filtered = app.filtered_downloads();
                                    if let Some(i) = app.ui.table_state.selected() {
                                        if let Some(dl) = filtered.get(i) {
                                            app.ui.view_state =
                                                app::ViewState::MissionControl(dl.gid.clone());
                                        }
                                    }
                                }
                                KeyCode::Home | KeyCode::Char('g') => app.first(),
                                KeyCode::End | KeyCode::Char('G') => app.last(),
                                KeyCode::Char('/') => {
                                    app.ui.search_query.clear();
                                    app.ui.view_state = app::ViewState::Search;
                                }
                                KeyCode::Char('p') => {
                                    app.pause_selected().await?;
                                }
                                KeyCode::Char('r') => {
                                    app.resume_selected().await?;
                                }
                                KeyCode::Char('a') => {
                                    app.ui.discovery_input.clear();
                                    app.ui.view_state = app::ViewState::Discovery;
                                }
                                KeyCode::Char(':') => {
                                    app.ui.command_input.clear();
                                    app.ui.view_state = app::ViewState::CommandPalette;
                                }
                                _ => {}
                            },
                            app::ViewState::MissionControl(gid) => match key.code {
                                KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') => {
                                    app.ui.view_state = app::ViewState::Dashboard
                                }
                                KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('f') => {
                                    let gid_clone = gid.clone();
                                    app.fetch_files(&gid_clone).await?;
                                    app.ui.view_state = app::ViewState::FileSelector(gid.clone())
                                }
                                KeyCode::Char('r') => {
                                    let gid_clone = gid.clone();
                                    if let Ok(id) = gid_clone.parse::<u64>() {
                                        app.call_rpc(
                                            "aura.refreshUri",
                                            Some(serde_json::json!([id.to_string()])),
                                            "tui-refresh",
                                        )
                                        .await?;
                                    }
                                }
                                _ => {}
                            },
                            app::ViewState::FileSelector(gid) => match key.code {
                                KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') => {
                                    app.ui.view_state = app::ViewState::MissionControl(gid.clone());
                                }
                                KeyCode::Down | KeyCode::Char('j') => app.file_next(),
                                KeyCode::Up | KeyCode::Char('k') => app.file_previous(),
                                KeyCode::Char(' ') | KeyCode::Enter => {
                                    app.toggle_file_selection();
                                }
                                KeyCode::Char('s') => {
                                    app.submit_file_selection(&gid).await?;
                                    app.ui.view_state = app::ViewState::MissionControl(gid.clone());
                                }
                                _ => {}
                            },
                            app::ViewState::Discovery => match key.code {
                                KeyCode::Esc => {
                                    app.ui.discovery_input.clear();
                                    app.ui.view_state = app::ViewState::Dashboard;
                                }
                                KeyCode::Enter => {
                                    app.submit_discovery().await?;
                                }
                                KeyCode::Char(c) => {
                                    app.ui.discovery_input.push(c);
                                }
                                KeyCode::Backspace => {
                                    app.ui.discovery_input.pop();
                                }
                                KeyCode::Tab => {
                                    app.ui.discovery_recursive = !app.ui.discovery_recursive;
                                }
                                _ => {}
                            },
                            app::ViewState::Search => match key.code {
                                KeyCode::Esc | KeyCode::Enter => {
                                    app.ui.view_state = app::ViewState::Dashboard;
                                }
                                KeyCode::Char(c) => {
                                    app.ui.search_query.push(c);
                                    app.clamp_selection();
                                }
                                KeyCode::Backspace => {
                                    app.ui.search_query.pop();
                                    app.clamp_selection();
                                }
                                _ => {}
                            },
                            app::ViewState::CommandPalette => match key.code {
                                KeyCode::Esc => {
                                    app.ui.command_input.clear();
                                    app.ui.view_state = app::ViewState::Dashboard;
                                }
                                KeyCode::Enter => {
                                    app.submit_command().await?;
                                }
                                KeyCode::Char(c) => {
                                    app.ui.command_input.push(c);
                                }
                                KeyCode::Backspace => {
                                    app.ui.command_input.pop();
                                }
                                _ => {}
                            },
                            app::ViewState::Help => {
                                if key.code == KeyCode::Esc || key.code == KeyCode::Char('?') {
                                    app.ui.view_state = app::ViewState::Dashboard;
                                }
                            }
                        }
                    }
                }
                Event::Paste(text) => {
                    app.ui.discovery_input = text.trim().to_string();
                    app.ui.view_state = app::ViewState::Discovery;
                }
                _ => {}
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
