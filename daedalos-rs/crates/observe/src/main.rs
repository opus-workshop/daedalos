//! Daedalos Observe - Real-time TUI dashboard
//!
//! See what's happening. Right now. All of it.

mod app;
mod ui;
mod data;

use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io;
use std::time::{Duration, Instant};

use app::App;

#[derive(Parser)]
#[command(name = "observe")]
#[command(about = "Real-time TUI dashboard for Daedalos")]
#[command(version)]
struct Args {
    /// Refresh interval in seconds
    #[arg(short, long, default_value = "2.0")]
    interval: f64,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run
    let mut app = App::new(args.interval);
    let result = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = result {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    let tick_rate = Duration::from_secs_f64(app.refresh_interval);
    let mut last_tick = Instant::now();

    // Initial data fetch
    app.refresh();

    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        KeyCode::Char('r') => app.refresh(),
                        KeyCode::Char('p') => app.toggle_pause(),
                        KeyCode::Char('l') => app.focus_panel(app::Panel::Loops),
                        KeyCode::Char('a') => app.focus_panel(app::Panel::Agents),
                        KeyCode::Char('d') => app.focus_panel(app::Panel::Daemons),
                        KeyCode::Char('e') => app.focus_panel(app::Panel::Events),
                        KeyCode::Char('j') | KeyCode::Down => app.scroll_down(),
                        KeyCode::Char('k') | KeyCode::Up => app.scroll_up(),
                        KeyCode::Tab => app.next_panel(),
                        KeyCode::BackTab => app.prev_panel(),
                        KeyCode::Char('?') => app.toggle_help(),
                        _ => {}
                    }
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            if !app.paused {
                app.refresh();
            }
            last_tick = Instant::now();
        }
    }
}
