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

// Module files live under src/core/ (the frontend-agnostic domain) and
// src/frontends/terminal/ (the crossterm + ratatui renderer).  `#[path]` keeps
// the crate paths flat (`crate::problem`, `crate::ui`, …) so the split is purely
// organisational.  See docs/graphics-mode.md.

// --- core: the frontend-agnostic domain (no crossterm / ratatui / std::fs) ---
#[path = "core/app.rs"]
mod app;
#[path = "core/cinematic.rs"]
mod cinematic;
#[path = "core/color.rs"]
mod color;
#[path = "core/config.rs"]
mod config;
#[path = "core/fraction.rs"]
mod fraction;
#[path = "core/geometry.rs"]
mod geometry;
#[path = "core/input.rs"]
mod input;
#[path = "core/motivation.rs"]
mod motivation;
#[path = "core/problem.rs"]
mod problem;
#[path = "core/section.rs"]
mod section;
#[path = "core/storage.rs"]
mod storage;
#[path = "core/strategy.rs"]
mod strategy;
#[path = "core/student.rs"]
mod student;
#[path = "core/topic.rs"]
mod topic;
#[path = "core/units.rs"]
mod units;

// --- terminal frontend: the cell renderer + its transition visuals ---
#[path = "frontends/terminal/canvas.rs"]
mod canvas;
#[path = "frontends/terminal/duck.rs"]
mod duck;
#[path = "frontends/terminal/font.rs"]
mod font;
#[path = "frontends/terminal/shapes.rs"]
mod shapes;
#[path = "frontends/terminal/transition.rs"]
mod transition;
#[path = "frontends/terminal/ui.rs"]
mod ui;

// --- CPU graphics frontend (feature = "gui"): winit + softbuffer + tiny-skia ---
#[cfg(feature = "gui")]
#[path = "frontends/gui/mod.rs"]
mod gui;

use std::io::{self, Stdout};
use std::path::PathBuf;
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
use storage::Storage;

/// Target frame interval — fast enough for smooth transitions, idle-cheap.
const TICK: Duration = Duration::from_millis(33);

fn main() -> Result<()> {
    let storage: Box<dyn Storage> = Box::new(FileStorage);
    let config = Config::load(storage.as_ref());

    // CPU graphics (feature = "gui"): launch the native window when it's the
    // saved preference (set in the Startup picker) or forced with `--gui`.
    #[cfg(feature = "gui")]
    if config.graphics == config::GraphicsMode::Cpu || std::env::args().any(|a| a == "--gui") {
        return gui::run(App::new(config, storage));
    }

    let mut terminal = setup_terminal()?;
    let result = run(&mut terminal, config, storage);
    restore_terminal(&mut terminal)?;
    result
}

fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>, config: Config, storage: Box<dyn Storage>) -> Result<()> {
    let mut app = App::new(config, storage);
    let mut last_tick = Instant::now();
    // Terminal-frontend render state: the captured pixels for any in-flight
    // transition, kept in step with the core's timing each frame.
    let mut fx = ui::Transitions::default();
    let mut rng = rand::thread_rng();

    while !app.should_quit {
        let size = terminal.size()?;
        let area = Rect::new(0, 0, size.width, size.height);
        fx.sync(&app, area, &mut rng);

        terminal.draw(|f| ui::draw(f, &app, &fx))?;

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
    app.persist();
    Ok(())
}

// ---------------------------------------------------------------------------
// Terminal-frontend persistence: JSON files under the platform config dir.
// ---------------------------------------------------------------------------

struct FileStorage;

impl Storage for FileStorage {
    fn load(&self, key: &str) -> Option<String> {
        data_path(key).and_then(|p| std::fs::read_to_string(p).ok())
    }

    fn save(&self, key: &str, data: &str) {
        let Some(path) = data_path(key) else { return };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, data);
    }
}

/// `…/rusty-math-tutor/<file>` under the platform config directory, so every
/// persisted file (config, student roster, custom motivations) lives together.
fn data_path(file: &str) -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    }?;
    Some(base.join("rusty-math-tutor").join(file))
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
