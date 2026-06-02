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
mod fraction;
mod geometry;
mod input;
mod motivation;
mod problem;
mod section;
mod shapes;
mod strategy;
mod student;
mod topic;
mod transition;
mod ui;
mod units;

use std::io::{self, Stdout};
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{cursor, execute};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

use app::App;
use config::Config;
use input::{InputEvent, Key, Mods};

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
                    if let Some(ev) = to_input(key) {
                        app.on_event(ev);
                    }
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

/// Map a crossterm key press into a frontend-neutral [`InputEvent`].  Keys the
/// app never uses are dropped (returns `None`).  This is the terminal
/// frontend's input adapter; GUI/web frontends provide their own.
fn to_input(key: KeyEvent) -> Option<InputEvent> {
    let k = match key.code {
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Enter => Key::Enter,
        KeyCode::Esc => Key::Esc,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Tab => Key::Tab,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        _ => return None,
    };
    let mods = Mods { ctrl: key.modifiers.contains(KeyModifiers::CONTROL) };
    Some(InputEvent::Key { key: k, mods })
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
