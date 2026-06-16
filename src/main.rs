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
#[path = "core/graphing.rs"]
mod graphing;
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
#[path = "core/time_display.rs"]
mod time_display;

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
#[path = "frontends/terminal/ui/mod.rs"]
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

/// Detect the system's UTC offset in minutes (positive = east / ahead of UTC).
/// Uses platform APIs directly so no extra crate is needed.
fn local_offset_mins() -> i32 {
    #[cfg(windows)]
    {
        // GetTimeZoneInformation is a single atomic read — no race condition
        // between two separate clock calls.  Bias is minutes WEST of UTC;
        // we negate to get our east-positive convention.
        // result: 0=unknown, 1=standard time, 2=daylight saving time.
        #[repr(C)]
        struct SysTime { year: u16, month: u16, dow: u16, day: u16,
                         hour: u16, min: u16, sec: u16, ms: u16 }
        #[repr(C)]
        struct TzInfo {
            bias:          i32,
            std_name:      [u16; 32],
            std_date:      SysTime,
            std_bias:      i32,
            dst_name:      [u16; 32],
            dst_date:      SysTime,
            dst_bias:      i32,
        }
        extern "system" { fn GetTimeZoneInformation(p: *mut TzInfo) -> u32; }
        unsafe {
            let mut tz: TzInfo = std::mem::zeroed();
            let result = GetTimeZoneInformation(&mut tz);
            // During DST (result == 2) the effective offset = Bias + DaylightBias.
            // During standard time (result == 1) it's Bias + StandardBias (usually 0).
            let extra = if result == 2 { tz.dst_bias } else { tz.std_bias };
            -(tz.bias + extra)
        }
    }

    #[cfg(unix)]
    {
        // localtime_r fills tm_gmtoff: seconds east of UTC (including DST).
        #[repr(C)]
        struct Tm {
            sec: i32, min: i32, hour: i32, mday: i32, mon: i32,
            year: i32, wday: i32, yday: i32, isdst: i32,
            gmtoff: i64,
            _zone: *const u8,
        }
        extern "C" { fn localtime_r(timep: *const i64, result: *mut Tm) -> *mut Tm; }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let mut tm: Tm = unsafe { std::mem::zeroed() };
        unsafe { localtime_r(&now, &mut tm); }
        (tm.gmtoff / 60) as i32
    }

    #[cfg(not(any(windows, unix)))]
    { 0 }
}

fn main() -> Result<()> {
    let storage: Box<dyn Storage> = Box::new(FileStorage);
    let config = Config::load(storage.as_ref());
    let mut app = App::new(config, storage);
    app.time_local_offset = local_offset_mins();

    // Always default to the console, whatever was last persisted — the CPU-graphics
    // window opens only when the user picks it in the picker.  `--gui` skips the
    // picker entirely and opens straight on the menu.
    app.config.graphics = config::GraphicsMode::Low;
    app.startup_index = 0;
    #[cfg(feature = "gui")]
    if std::env::args().any(|a| a == "--gui") {
        app.config.graphics = config::GraphicsMode::Cpu;
        app.startup_index = 1;
        app.screen = crate::app::Screen::Menu;
    }

    // Run the frontend for this mode.  If the user switches mode in the picker,
    // hand off by launching a *fresh* process in the other mode and exiting — a
    // clean process avoids the console↔window keyboard-focus clash that an
    // in-process switch hits on Windows once the terminal has held the console.
    #[cfg_attr(not(feature = "gui"), allow(unused_variables))]
    let switching = run_frontend(&mut app)?;
    #[cfg(feature = "gui")]
    if switching {
        relaunch(app.config.graphics)?;
    }
    Ok(())
}

/// Run whichever frontend matches the app's current graphics mode, until the app
/// quits or the user switches mode.  Returns `true` when a switch is pending.
fn run_frontend(app: &mut App) -> Result<bool> {
    #[cfg(feature = "gui")]
    if app.config.graphics == config::GraphicsMode::Cpu {
        return gui::run(app);
    }
    run_terminal(app)
}

/// Launch a fresh process in the chosen graphics `mode`, skipping the picker
/// (`--menu`).  On Windows, give the console build its own console window and the
/// window build none, so neither leaves a stray window behind.
#[cfg(feature = "gui")]
fn relaunch(mode: config::GraphicsMode) -> Result<()> {
    use std::process::Command;
    let exe = std::env::current_exe()?;
    let to_gui = mode == config::GraphicsMode::Cpu;
    let mut cmd = Command::new(exe);
    if to_gui {
        cmd.arg("--gui");
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(if to_gui { CREATE_NO_WINDOW } else { CREATE_NEW_CONSOLE });
    }
    let _child = cmd.spawn()?;

    // Windows blocks a spawned process from stealing the foreground (so the new
    // window would open without keyboard focus).  Grant this child that right so
    // its `focus_window()` is honoured.
    #[cfg(windows)]
    if to_gui {
        extern "system" {
            fn AllowSetForegroundWindow(dwProcessId: u32) -> i32;
        }
        unsafe {
            AllowSetForegroundWindow(_child.id());
        }
    }
    Ok(())
}

/// The console (crossterm + ratatui) frontend: set up the alternate screen, run
/// the loop, then restore the terminal whatever the outcome.
fn run_terminal(app: &mut App) -> Result<bool> {
    let mut terminal = setup_terminal()?;
    let result = terminal_loop(&mut terminal, app);
    restore_terminal(&mut terminal)?;
    result
}

fn terminal_loop(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> Result<bool> {
    let mut last_tick = Instant::now();
    // Terminal-frontend render state: the captured pixels for any in-flight
    // transition, kept in step with the core's timing each frame.
    let mut fx = ui::Transitions::default();
    let mut rng = rand::thread_rng();

    while !app.should_quit {
        let size = terminal.size()?;
        let area = Rect::new(0, 0, size.width, size.height);
        fx.sync(app, area, &mut rng);

        terminal.draw(|f| ui::draw(f, app, &fx))?;

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

        // The user just chose CPU Graphics — hand off to the window.
        #[cfg(feature = "gui")]
        if app.config.graphics != config::GraphicsMode::Low {
            app.persist();
            return Ok(true);
        }

        if last_tick.elapsed() >= TICK {
            app.on_tick();
            last_tick = Instant::now();
        }
    }
    // Persist any layout change and the latest progress on the way out.
    app.persist();
    Ok(false)
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
