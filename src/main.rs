//! Rusty Math Tutor — a console math tutor for kindergarten through 8th grade.
//!
//! One problem fills the screen at a time so the learner can focus without the
//! pressure of a visible queue or clock (the timed Challenge mode is opt-in).
//! Correct answers melt into the next problem through a randomly chosen,
//! GPU-free transition, and pressing **H** summons Deduction Duck to break the
//! current problem down step by step.
//!
//! The whole thing is a plain synchronous poll loop over [`crossterm`] events
//! with a fixed ~33 ms tick — deliberately lightweight for slow hardware.

mod app;
mod cinematic;
mod config;
mod duck;
mod font;
mod motivation;
mod problem;
mod strategy;
mod student;
mod transition;
mod ui;
mod units;

use std::io::{self, Stdout};
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{cursor, execute};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

use app::App;
use config::Config;

/// Target frame interval — fast enough for smooth transitions, idle-cheap.
const TICK: Duration = Duration::from_millis(33);

fn main() -> Result<()> {
    let mut terminal = setup_terminal()?;
    let result = run(&mut terminal);
    restore_terminal(&mut terminal)?;
    result
}

fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    let mut app = App::new(Config::load());
    let mut last_tick = Instant::now();

    while !app.should_quit {
        // Keep the app's notion of the screen size current for transition capture.
        let size = terminal.size()?;
        app.set_area(Rect::new(0, 0, size.width, size.height));

        terminal.draw(|f| ui::draw(f, &app))?;

        // Wait for input up to the remaining tick budget.
        let timeout = TICK.checked_sub(last_tick.elapsed()).unwrap_or(Duration::ZERO);
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == event::KeyEventKind::Press {
                    app.on_key(key);
                }
            }
        }

        if last_tick.elapsed() >= TICK {
            app.on_tick();
            last_tick = Instant::now();
        }
    }
    // Persist any layout change and the latest progress on the way out.
    app.config.save();
    app.roster.save();
    Ok(())
}

// ---------------------------------------------------------------------------
// Terminal lifecycle
// ---------------------------------------------------------------------------

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, cursor::Show)?;
    terminal.show_cursor()?;
    Ok(())
}
