//! Application state machine: the start menu, a no-timer Practice session, and
//! an optional timed Challenge session.  Owns input handling, the per-tick
//! animation clock, and the hand-off into [`crate::transition`] when a problem
//! is answered correctly.

use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use rand::rngs::ThreadRng;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::config::Config;
use crate::problem::{self, Op, Problem};
use crate::transition::Transition;
use crate::ui;

/// How long a Challenge run lasts.
const CHALLENGE_LEN: Duration = Duration::from_secs(60);
/// Maximum digits a student can type for an answer.
const MAX_INPUT: usize = 7;
/// Number of selectable rows on the menu.
const MENU_ITEMS: usize = 12;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Screen {
    Startup,
    Menu,
    Settings,
    Stats,
    Teacher,
    Cinematic,
    Practice,
    Challenge,
}

/// How long each milestone-cinematic scene lingers before transitioning.
const SCENE_DWELL: u32 = 24;
/// Wrong answers on one problem before Deduction Duck steps in to encourage.
const STRUGGLE_THRESHOLD: u32 = 3;
/// Maximum length of a teacher-supplied "Why?" anecdote.
pub const ANECDOTE_MAX: usize = 1024;

/// A playing milestone celebration: a sequence of [`cinematic::Scene`]s that
/// our transitions animate between.
pub struct Cinematic {
    pub scenes: Vec<crate::cinematic::Scene>,
    pub index: usize,
    dwell: u32,
    pub transition: Option<Transition>,
    started: Instant,
}

// Menu row indices.
const MI_STUDENT: usize = 0;
const MI_GRADE: usize = 1;
const MI_OPS: std::ops::RangeInclusive<usize> = 2..=5;
const MI_LAYOUT: usize = 6;
const MI_SETTINGS: usize = 7;
const MI_PROGRESS: usize = 8;
const MI_TEACHER: usize = 9;
const MI_PRACTICE: usize = 10;
const MI_CHALLENGE: usize = 11;

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Feedback {
    None,
    Correct,
    Wrong,
}

/// Which teacher tool is on screen once logged in.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum TeacherView {
    /// Edit the "Why?" anecdotes shown to students.
    Anecdotes,
    /// Reset or remove student progress records.
    Records,
}

// ---------------------------------------------------------------------------
// Challenge timer
// ---------------------------------------------------------------------------

pub struct Challenge {
    start: Instant,
    duration: Duration,
    pub solved: u32,
    pub finished: bool,
}

impl Challenge {
    fn new(duration: Duration) -> Self {
        Challenge { start: Instant::now(), duration, solved: 0, finished: false }
    }

    /// Whole seconds left, rounded up so the clock starts at the full count.
    pub fn remaining_secs(&self) -> u64 {
        let rem = self.duration.checked_sub(self.start.elapsed()).unwrap_or(Duration::ZERO);
        let secs = rem.as_secs() + if rem.subsec_nanos() > 0 { 1 } else { 0 };
        secs.min(self.duration.as_secs())
    }

    pub fn duration_secs(&self) -> u64 {
        self.duration.as_secs()
    }

    /// Push the start forward by `d` — used to refund time spent in a
    /// milestone cinematic so the countdown stays fair.
    fn extend(&mut self, d: Duration) {
        if let Some(start) = self.start.checked_add(d) {
            self.start = start;
        }
    }
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

pub struct App {
    pub screen: Screen,
    rng: ThreadRng,
    pub config: Config,
    pub roster: crate::student::Roster,

    // Adding a student from the menu.
    pub naming: bool,
    pub name_input: String,
    /// Current run of correct answers (for the personal-best streak).
    pub streak: u32,

    // Menu selections.
    pub menu_grade: u8,
    pub menu_ops: [bool; 4],
    pub menu_index: usize,

    // Startup graphics-mode picker.
    pub startup_index: usize,

    // Settings screen.
    pub settings_grade: u8,
    pub settings_field: usize,

    // Active session.
    grade: u8,
    ops: Vec<Op>,
    pub current: Problem,
    pub input: String,
    pub feedback: Feedback,

    // Transition between the answered card and the next one.
    pub transition: Option<Transition>,
    pending: Option<Problem>,

    // Deduction Duck help overlay: `help_in` eases 0..1 toward the target set
    // by `help_active`; `strategy_index` selects which method he shows.
    pub help_active: bool,
    pub help_in: f32,
    pub strategy_index: usize,
    /// Whether Deduction Duck has revealed the worked answer (hint-first).
    pub revealed: bool,

    /// "Why am I learning this?" overlay (toggled with Y); `why_items` holds the
    /// randomly chosen real-world uses so they stay put while it's open.
    pub why_active: bool,
    pub why_items: Vec<String>,
    /// An optional "for future coders" peek: (code line, why it matters).
    pub why_code: Option<(String, String)>,
    /// First visible line when the Why panel scrolls.
    pub why_scroll: usize,
    /// Max scroll offset, written by the renderer each frame so input can clamp.
    pub why_max_scroll: std::cell::Cell<usize>,
    /// Teacher's own anecdotes, merged into the Why list.
    pub why_extras: crate::motivation::Extras,

    // Teacher Area (password-gated tools).
    pub teacher_authed: bool,
    /// Which tool is showing once authenticated.
    pub teacher_view: TeacherView,
    /// Password buffer while logging in / setting a password.
    pub teacher_pw: String,
    /// Operation currently being edited (index into [`Op::ALL`]).
    pub teacher_op: usize,
    /// While true, the teacher is typing a new anecdote into `teacher_text`.
    pub teacher_adding: bool,
    pub teacher_text: String,
    /// Selected student row in the records administration view.
    pub teacher_rec_index: usize,
    /// Transient status line (e.g. "Saved!", "Incorrect password").
    pub teacher_msg: Option<String>,

    // Free-running clock for sprite/waddle animation.
    pub anim_frame: u64,

    pub challenge: Option<Challenge>,

    // Milestone celebration.
    pub cinematic: Option<Cinematic>,
    cinematic_return: Screen,

    // Struggle support: consecutive wrong answers on the current problem, and a
    // one-shot encouraging line shown when Deduction Duck steps in.
    wrong_streak: u32,
    pub encourage: Option<String>,

    /// Most recent full-frame area, refreshed by [`App::set_area`] each loop so
    /// transition capture matches what is on screen.
    area: Rect,

    pub should_quit: bool,
}

impl App {
    pub fn new(config: Config) -> Self {
        let mut rng = rand::thread_rng();
        let current = problem::generate(config.range(1), &[Op::Add], &mut rng);
        let startup_index = if config.graphics == crate::config::GraphicsMode::Cpu { 1 } else { 0 };
        App {
            screen: Screen::Startup,
            rng,
            config,
            roster: crate::student::Roster::load(),
            naming: false,
            name_input: String::new(),
            streak: 0,
            menu_grade: 1,
            menu_ops: [true, true, false, false],
            menu_index: 0,
            startup_index,
            settings_grade: 1,
            settings_field: 0,
            grade: 1,
            ops: vec![Op::Add],
            current,
            input: String::new(),
            feedback: Feedback::None,
            transition: None,
            pending: None,
            help_active: false,
            help_in: 0.0,
            strategy_index: 0,
            revealed: false,
            why_active: false,
            why_items: Vec::new(),
            why_code: None,
            why_scroll: 0,
            why_max_scroll: std::cell::Cell::new(0),
            why_extras: crate::motivation::Extras::load(),
            teacher_authed: false,
            teacher_view: TeacherView::Anecdotes,
            teacher_pw: String::new(),
            teacher_op: 0,
            teacher_adding: false,
            teacher_text: String::new(),
            teacher_rec_index: 0,
            teacher_msg: None,
            anim_frame: 0,
            challenge: None,
            cinematic: None,
            cinematic_return: Screen::Practice,
            wrong_streak: 0,
            encourage: None,
            area: Rect::new(0, 0, 80, 24),
            should_quit: false,
        }
    }

    pub fn set_area(&mut self, area: Rect) {
        self.area = area;
    }

    // -- per-tick update ----------------------------------------------------

    pub fn on_tick(&mut self) {
        self.anim_frame = self.anim_frame.wrapping_add(1);

        // Ease the duck in or out.
        let target = if self.help_active { 1.0 } else { 0.0 };
        let step = 0.12;
        if self.help_in < target {
            self.help_in = (self.help_in + step).min(target);
        } else if self.help_in > target {
            self.help_in = (self.help_in - step).max(target);
        }

        // Advance an in-flight transition; swap problems when it finishes.
        if let Some(t) = &mut self.transition {
            if t.advance() {
                self.transition = None;
                if let Some(next) = self.pending.take() {
                    self.current = next;
                }
                self.input.clear();
                self.feedback = Feedback::None;
                self.strategy_index = 0;
                self.revealed = false;
                self.wrong_streak = 0;
                self.encourage = None;
            }
        }

        // A playing milestone cinematic advances on its own clock.
        if self.cinematic.is_some() {
            self.tick_cinematic();
        }

        // Challenge countdown — frozen while a cinematic plays (the time is
        // refunded when it ends, so a celebration never costs the student).
        if self.cinematic.is_none() {
            if let Some(c) = &mut self.challenge {
                if !c.finished && c.remaining_secs() == 0 {
                    c.finished = true;
                    self.help_active = false;
                    // Lock in the progress earned during the timed run.
                    self.roster.save();
                }
            }
        }
    }

    /// Advance the milestone cinematic: dwell on each scene, then transition to
    /// the next, finishing once the last scene has been shown.
    fn tick_cinematic(&mut self) {
        let Some(mut c) = self.cinematic.take() else { return };
        let mut finished = false;
        if let Some(t) = &mut c.transition {
            if t.advance() {
                c.transition = None;
                c.index += 1;
                c.dwell = SCENE_DWELL;
            }
        } else {
            c.dwell = c.dwell.saturating_sub(1);
            if c.dwell == 0 {
                if c.index + 1 < c.scenes.len() {
                    let area = self.area;
                    let from = capture_scene(area, &c.scenes[c.index]);
                    let to = capture_scene(area, &c.scenes[c.index + 1]);
                    c.transition = Some(Transition::new(from, to, &mut self.rng));
                } else {
                    finished = true;
                }
            }
        }
        if finished {
            self.finish_cinematic(c);
        } else {
            self.cinematic = Some(c);
        }
    }

    /// Tear down a cinematic and resume the lesson with the next problem.
    fn finish_cinematic(&mut self, c: Cinematic) {
        if let Some(ch) = &mut self.challenge {
            ch.extend(c.started.elapsed());
        }
        self.screen = self.cinematic_return;
        if let Some(next) = self.pending.take() {
            self.current = next;
        }
        self.input.clear();
        self.feedback = Feedback::None;
        self.strategy_index = 0;
        self.revealed = false;
        self.wrong_streak = 0;
        self.encourage = None;
    }

    // -- input --------------------------------------------------------------

    pub fn on_key(&mut self, key: KeyEvent) {
        // Ctrl+C always quits.
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }
        match self.screen {
            Screen::Startup => self.on_startup_key(key),
            Screen::Menu => self.on_menu_key(key),
            Screen::Settings => self.on_settings_key(key),
            Screen::Stats => {
                if matches!(key.code, KeyCode::Esc | KeyCode::Enter) {
                    self.screen = Screen::Menu;
                }
            }
            Screen::Teacher => self.on_teacher_key(key),
            Screen::Cinematic => {
                // Any key skips the celebration and resumes the lesson.
                if let Some(c) = self.cinematic.take() {
                    self.finish_cinematic(c);
                }
            }
            Screen::Practice | Screen::Challenge => self.on_session_key(key),
        }
    }

    fn on_startup_key(&mut self, key: KeyEvent) {
        use crate::config::GraphicsMode;
        match key.code {
            KeyCode::Up | KeyCode::Down => self.startup_index ^= 1,
            KeyCode::Enter | KeyCode::Char(' ') => {
                // CPU mode is shelved; only Low actually proceeds.
                let mode = if self.startup_index == 1 { GraphicsMode::Cpu } else { GraphicsMode::Low };
                if mode.available() {
                    self.config.graphics = mode;
                    self.screen = Screen::Menu;
                }
            }
            KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => self.should_quit = true,
            _ => {}
        }
    }

    fn on_menu_key(&mut self, key: KeyEvent) {
        // Typing a new student's name captures everything until committed.
        if self.naming {
            self.on_name_input_key(key);
            return;
        }

        match key.code {
            KeyCode::Up => self.menu_index = (self.menu_index + MENU_ITEMS - 1) % MENU_ITEMS,
            KeyCode::Down => self.menu_index = (self.menu_index + 1) % MENU_ITEMS,
            KeyCode::Left if self.menu_index == MI_STUDENT => self.roster.cycle(-1),
            KeyCode::Right if self.menu_index == MI_STUDENT => self.roster.cycle(1),
            KeyCode::Left if self.menu_index == MI_GRADE => {
                self.menu_grade = self.menu_grade.saturating_sub(1);
            }
            KeyCode::Right if self.menu_index == MI_GRADE => {
                self.menu_grade = (self.menu_grade + 1).min(8);
            }
            // N adds a new student from anywhere on the menu.
            KeyCode::Char('n') | KeyCode::Char('N') => {
                self.naming = true;
                self.name_input.clear();
            }
            KeyCode::Char(' ') | KeyCode::Enter => match self.menu_index {
                i if MI_OPS.contains(&i) => {
                    let op = i - *MI_OPS.start();
                    self.menu_ops[op] = !self.menu_ops[op];
                }
                MI_LAYOUT => self.config.layout = self.config.layout.toggled(),
                MI_SETTINGS => self.open_settings(),
                MI_PROGRESS => self.screen = Screen::Stats,
                MI_TEACHER => self.open_teacher(),
                MI_STUDENT => {
                    self.naming = true;
                    self.name_input.clear();
                }
                MI_PRACTICE => self.start_session(false),
                MI_CHALLENGE => self.start_session(true),
                _ => {}
            },
            KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => self.should_quit = true,
            _ => {}
        }
    }

    /// Handle a keystroke while typing a new student's name.
    fn on_name_input_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.naming = false;
                self.name_input.clear();
            }
            KeyCode::Enter => {
                if !self.name_input.trim().is_empty() {
                    self.roster.add(&self.name_input);
                    self.roster.save();
                }
                self.naming = false;
                self.name_input.clear();
            }
            KeyCode::Backspace => {
                self.name_input.pop();
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.name_input.chars().count() < 20 {
                    self.name_input.push(c);
                }
            }
            _ => {}
        }
    }

    fn open_settings(&mut self) {
        self.settings_grade = self.menu_grade;
        self.settings_field = 0;
        self.screen = Screen::Settings;
    }

    fn open_teacher(&mut self) {
        self.screen = Screen::Teacher;
        self.teacher_authed = false;
        self.teacher_view = TeacherView::Anecdotes;
        self.teacher_pw.clear();
        self.teacher_text.clear();
        self.teacher_adding = false;
        self.teacher_op = 0;
        self.teacher_rec_index = 0;
        self.teacher_msg = None;
    }

    fn on_teacher_key(&mut self, key: KeyEvent) {
        if !self.teacher_authed {
            self.on_teacher_login_key(key);
        } else if self.teacher_adding {
            self.on_teacher_add_key(key);
        } else {
            self.on_teacher_manage_key(key);
        }
    }

    /// Setting (first visit) or entering the teacher password.
    fn on_teacher_login_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.screen = Screen::Menu,
            KeyCode::Backspace => {
                self.teacher_pw.pop();
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.teacher_pw.chars().count() < 32 {
                    self.teacher_pw.push(c);
                }
            }
            KeyCode::Enter => {
                if self.config.has_teacher_password() {
                    if self.config.verify_teacher_password(&self.teacher_pw) {
                        self.teacher_authed = true;
                        self.teacher_msg = None;
                    } else {
                        self.teacher_msg = Some("Incorrect password — try again.".to_string());
                    }
                } else if self.teacher_pw.trim().is_empty() {
                    self.teacher_msg = Some("Pick a password to protect teacher tools.".to_string());
                } else {
                    self.config.set_teacher_password(&self.teacher_pw);
                    self.config.save();
                    self.teacher_authed = true;
                    self.teacher_msg = Some("Password set! You're logged in.".to_string());
                }
                self.teacher_pw.clear();
            }
            _ => {}
        }
    }

    /// Typing a new anecdote for the selected operation.
    fn on_teacher_add_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.teacher_adding = false;
                self.teacher_text.clear();
            }
            KeyCode::Backspace => {
                self.teacher_text.pop();
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.teacher_text.chars().count() < ANECDOTE_MAX {
                    self.teacher_text.push(c);
                }
            }
            KeyCode::Enter => {
                let op = Op::ALL[self.teacher_op];
                if !self.teacher_text.trim().is_empty() {
                    self.why_extras.add(op, &self.teacher_text);
                    self.why_extras.save();
                    self.teacher_msg = Some("Saved your example. Thank you!".to_string());
                }
                self.teacher_adding = false;
                self.teacher_text.clear();
            }
            _ => {}
        }
    }

    /// Browsing tools once authenticated.  Tab switches between editing the
    /// "Why?" anecdotes and administering student records.
    fn on_teacher_manage_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Esc {
            self.teacher_authed = false;
            self.screen = Screen::Menu;
            return;
        }
        if key.code == KeyCode::Tab {
            self.teacher_view = match self.teacher_view {
                TeacherView::Anecdotes => TeacherView::Records,
                TeacherView::Records => TeacherView::Anecdotes,
            };
            self.teacher_msg = None;
            return;
        }
        match self.teacher_view {
            TeacherView::Anecdotes => match key.code {
                KeyCode::Left => self.teacher_op = (self.teacher_op + Op::ALL.len() - 1) % Op::ALL.len(),
                KeyCode::Right => self.teacher_op = (self.teacher_op + 1) % Op::ALL.len(),
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    self.teacher_adding = true;
                    self.teacher_text.clear();
                    self.teacher_msg = None;
                }
                _ => {}
            },
            TeacherView::Records => self.on_teacher_records_key(key),
        }
    }

    /// Reset or remove student progress records.
    fn on_teacher_records_key(&mut self, key: KeyEvent) {
        let n = self.roster.students.len();
        self.teacher_rec_index = self.teacher_rec_index.min(n - 1);
        match key.code {
            KeyCode::Up => self.teacher_rec_index = (self.teacher_rec_index + n - 1) % n,
            KeyCode::Down => self.teacher_rec_index = (self.teacher_rec_index + 1) % n,
            KeyCode::Left => self.teacher_op = (self.teacher_op + Op::ALL.len() - 1) % Op::ALL.len(),
            KeyCode::Right => self.teacher_op = (self.teacher_op + 1) % Op::ALL.len(),
            // S: reset just the selected operation's count for this student.
            KeyCode::Char('s') | KeyCode::Char('S') => {
                let op = Op::ALL[self.teacher_op];
                let name = self.roster.students[self.teacher_rec_index].name.clone();
                self.roster.students[self.teacher_rec_index].solved[op.index()] = 0;
                self.roster.save();
                self.teacher_msg = Some(format!("Reset {} for {}.", op.name(), name));
            }
            // R: reset all of this student's records.
            KeyCode::Char('r') | KeyCode::Char('R') => {
                let name = self.roster.students[self.teacher_rec_index].name.clone();
                self.roster.students[self.teacher_rec_index].reset();
                self.roster.save();
                self.teacher_msg = Some(format!("Reset all records for {}.", name));
            }
            // X: remove the student entirely (a Guest is kept if it's the last).
            KeyCode::Char('x') | KeyCode::Char('X') => {
                let name = self.roster.students[self.teacher_rec_index].name.clone();
                self.roster.remove(self.teacher_rec_index);
                self.teacher_rec_index = self.teacher_rec_index.min(self.roster.students.len() - 1);
                self.roster.save();
                self.teacher_msg = Some(format!("Removed {}.", name));
            }
            _ => {}
        }
    }

    fn on_settings_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => self.settings_field = (self.settings_field + ui::SETTINGS_FIELDS - 1) % ui::SETTINGS_FIELDS,
            KeyCode::Down => self.settings_field = (self.settings_field + 1) % ui::SETTINGS_FIELDS,
            KeyCode::Left => self.adjust_setting(false),
            KeyCode::Right => self.adjust_setting(true),
            KeyCode::Esc | KeyCode::Enter => {
                // Save and return to the menu.
                self.config.save();
                self.screen = Screen::Menu;
            }
            _ => {}
        }
    }

    /// Nudge the selected Settings value up (`up`) or down.  Range knobs step
    /// by a fraction of their size so they move quickly when large.
    fn adjust_setting(&mut self, up: bool) {
        if self.settings_field == 0 {
            self.settings_grade = if up {
                (self.settings_grade + 1).min(8)
            } else {
                self.settings_grade.saturating_sub(1)
            };
            return;
        }
        let grade = self.settings_grade;
        let r = self.config.range_mut(grade);
        let step = |v: i64| (v / 10).max(1); // ~10% steps, minimum 1
        match self.settings_field {
            1 => r.add_max = bump(r.add_max, up, step(r.add_max), 1, 100_000),
            2 => r.mul_max = bump(r.mul_max, up, 1, 1, 1_000),
            3 => r.div_max = bump(r.div_max, up, 1, 1, 1_000),
            4 => r.allow_negative = up,
            _ => {}
        }
    }

    fn on_session_key(&mut self, key: KeyEvent) {
        // Esc peels back one overlay at a time, then returns to the menu.
        if key.code == KeyCode::Esc {
            if self.why_active {
                self.why_active = false;
            } else if self.help_active {
                self.help_active = false;
            } else {
                self.to_menu();
            }
            return;
        }

        // Y answers "Why am I learning this?" with real-world uses.
        if matches!(key.code, KeyCode::Char('y') | KeyCode::Char('Y')) {
            self.why_active = !self.why_active;
            if self.why_active {
                self.help_active = false;
                self.refresh_why_items();
            }
            return;
        }

        // While the Why panel is open, Up/Down scroll long lists.
        if self.why_active && matches!(key.code, KeyCode::Up | KeyCode::Down) {
            if key.code == KeyCode::Up {
                self.why_scroll = self.why_scroll.saturating_sub(1);
            } else {
                self.why_scroll = (self.why_scroll + 1).min(self.why_max_scroll.get());
            }
            return;
        }

        // Help toggle works any time; opening always starts at the first method
        // with the answer hidden (hint-first).
        if matches!(key.code, KeyCode::Char('h') | KeyCode::Char('H')) {
            self.help_active = !self.help_active;
            if self.help_active {
                self.why_active = false;
                self.strategy_index = 0;
                self.revealed = false;
            }
            return;
        }

        // While the duck is out, Space cycles strategy, R reveals the answer.
        if self.help_active {
            match key.code {
                KeyCode::Char(' ') => {
                    self.strategy_index = self.strategy_index.wrapping_add(1);
                    self.revealed = false;
                    return;
                }
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    self.revealed = !self.revealed;
                    return;
                }
                _ => {}
            }
        }

        // V switches the problem layout live.
        if matches!(key.code, KeyCode::Char('v') | KeyCode::Char('V')) {
            self.config.layout = self.config.layout.toggled();
            return;
        }

        // Finished challenge: Enter starts a fresh run.
        if self.challenge.as_ref().map_or(false, |c| c.finished) {
            if key.code == KeyCode::Enter {
                self.start_session(true);
            }
            return;
        }

        // Ignore answer input while a transition is playing.
        if self.transition.is_some() {
            return;
        }

        match key.code {
            KeyCode::Char(d @ '0'..='9') => {
                if self.feedback == Feedback::Wrong {
                    self.input.clear();
                }
                self.feedback = Feedback::None;
                if self.input.len() < MAX_INPUT {
                    self.input.push(d);
                }
            }
            KeyCode::Char('-') => {
                // Leading minus only (for grades that allow negative answers).
                if self.input.is_empty() {
                    self.input.push('-');
                }
            }
            KeyCode::Backspace => {
                self.feedback = Feedback::None;
                self.input.pop();
            }
            KeyCode::Enter => self.check_answer(),
            _ => {}
        }
    }

    fn refresh_why_items(&mut self) {
        let op = self.current.op;
        self.why_items = crate::motivation::pick(op, &self.why_extras, &mut self.rng, 4);
        self.why_code = crate::motivation::pick_code(op, &mut self.rng);
        self.why_scroll = 0;
    }

    // -- session helpers ----------------------------------------------------

    fn start_session(&mut self, challenge: bool) {
        self.grade = self.menu_grade;
        self.ops = Op::ALL.iter().copied().enumerate().filter(|(i, _)| self.menu_ops[*i]).map(|(_, op)| op).collect();
        if self.ops.is_empty() {
            self.ops.push(Op::Add);
        }
        self.screen = if challenge { Screen::Challenge } else { Screen::Practice };
        self.current = problem::generate(self.config.range(self.grade), &self.ops, &mut self.rng);
        self.input.clear();
        self.feedback = Feedback::None;
        self.transition = None;
        self.pending = None;
        self.help_active = false;
        self.help_in = 0.0;
        self.strategy_index = 0;
        self.revealed = false;
        self.why_active = false;
        self.streak = 0;
        self.wrong_streak = 0;
        self.encourage = None;
        self.cinematic = None;
        self.challenge = if challenge { Some(Challenge::new(CHALLENGE_LEN)) } else { None };
    }

    fn to_menu(&mut self) {
        self.screen = Screen::Menu;
        self.transition = None;
        self.pending = None;
        self.help_active = false;
        self.help_in = 0.0;
        self.why_active = false;
        self.challenge = None;
        // Persist progress earned this session.
        self.roster.save();
    }

    fn check_answer(&mut self) {
        let parsed = self.input.parse::<i64>().ok();
        if parsed == Some(self.current.answer) {
            self.feedback = Feedback::Correct;
            if let Some(c) = &mut self.challenge {
                c.solved += 1;
            }
            // Credit the current student and track their personal-best streak.
            let op = self.current.op;
            self.streak += 1;
            let streak = self.streak;
            let student = self.roster.current_mut();
            student.record(op);
            student.note_streak(streak);
            let total = student.total();

            self.wrong_streak = 0;
            self.encourage = None;
            self.help_active = false;

            let next = problem::generate(self.config.range(self.grade), &self.ops, &mut self.rng);
            if crate::cinematic::is_milestone(total) {
                self.start_cinematic(total, next);
            } else {
                self.begin_transition(next);
            }
        } else {
            self.feedback = Feedback::Wrong;
            self.streak = 0;
            self.wrong_streak += 1;
            // After a few tries, Deduction Duck steps in with a kind word.
            if self.wrong_streak >= STRUGGLE_THRESHOLD {
                self.encourage = Some(crate::cinematic::struggle_message(&mut self.rng));
                self.help_active = true;
                self.strategy_index = 0;
                self.revealed = false;
            }
        }
    }

    /// Capture the (correct) current card and the next card, then kick off a
    /// random transition between them.
    fn begin_transition(&mut self, next: Problem) {
        let area = ui::card_area(self.screen, self.area);
        let from = self.capture(area, &self.current, &self.input, Some(("✓  Correct!".to_string(), ratatui::style::Color::LightGreen)));
        let to = self.capture(area, &next, "", None);
        self.pending = Some(next);
        self.transition = Some(Transition::new(from, to, &mut self.rng));
    }

    /// Kick off a milestone celebration, holding `next` to resume afterwards.
    fn start_cinematic(&mut self, total: u32, next: Problem) {
        let name = self.roster.current().name.clone();
        let scenes = crate::cinematic::scenes(&name, total, &mut self.rng);
        self.pending = Some(next);
        self.cinematic_return = self.screen;
        self.cinematic = Some(Cinematic {
            scenes,
            index: 0,
            dwell: SCENE_DWELL,
            transition: None,
            started: Instant::now(),
        });
        self.screen = Screen::Cinematic;
    }

    fn capture(&self, area: Rect, p: &Problem, input: &str, banner: Option<(String, ratatui::style::Color)>) -> Buffer {
        let mut buf = Buffer::empty(area);
        ui::render_card(area, &mut buf, p, input, self.config.layout, banner);
        buf
    }
}

/// Step `v` by `delta` in the given direction, clamped to `[min, max]`.
fn bump(v: i64, up: bool, delta: i64, min: i64, max: i64) -> i64 {
    let next = if up { v + delta } else { v - delta };
    next.clamp(min, max)
}

/// Render a cinematic scene into a fresh buffer for transition capture.
fn capture_scene(area: Rect, scene: &crate::cinematic::Scene) -> Buffer {
    let mut buf = Buffer::empty(area);
    ui::render_scene(area, &mut buf, scene);
    buf
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{motivation, strategy};
    use crate::transition::{Effect, Transition};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    /// Render every screen at several sizes (including cramped ones) to prove
    /// the direct buffer writes in font/duck/transition stay in bounds.
    #[test]
    fn rendering_never_panics() {
        for (w, h) in [(80, 24), (120, 40), (30, 12), (40, 10)] {
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            let mut app = App::new(Config::default());
            app.set_area(Rect::new(0, 0, w, h));

            // Startup, menu (with name prompt), settings, stats.
            term.draw(|f| ui::draw(f, &app)).unwrap();
            app.screen = Screen::Menu;
            term.draw(|f| ui::draw(f, &app)).unwrap();
            app.naming = true;
            app.name_input = "Ada".to_string();
            term.draw(|f| ui::draw(f, &app)).unwrap();
            app.naming = false;
            app.open_settings();
            term.draw(|f| ui::draw(f, &app)).unwrap();
            app.screen = Screen::Stats;
            app.roster.current_mut().solved = [12, 7, 4, 3];
            app.roster.current_mut().best_streak = 9;
            term.draw(|f| ui::draw(f, &app)).unwrap();

            // Teacher area: login, then both manage views (adding + records).
            app.open_teacher();
            app.teacher_pw = "pw".to_string();
            term.draw(|f| ui::draw(f, &app)).unwrap();
            app.teacher_authed = true;
            app.teacher_view = TeacherView::Anecdotes;
            app.teacher_adding = true;
            // A long anecdote exercises the wrapping input box and overflow.
            app.teacher_text = "x".repeat(ANECDOTE_MAX);
            term.draw(|f| ui::draw(f, &app)).unwrap();
            app.teacher_text = "When I tiled my kitchen floor".to_string();
            term.draw(|f| ui::draw(f, &app)).unwrap();
            app.teacher_adding = false;
            app.teacher_view = TeacherView::Records;
            app.teacher_msg = Some("Reset all records for Ada.".to_string());
            term.draw(|f| ui::draw(f, &app)).unwrap();
            app.screen = Screen::Menu;

            // Practice in both layouts: cycle every strategy (each Viz), with
            // hint then revealed, plus the Why overlay, across all operations.
            for layout in [crate::config::Layout::Horizontal, crate::config::Layout::Vertical] {
                app.config.layout = layout;
                for &op in &Op::ALL {
                    app.menu_ops = [op == Op::Add, op == Op::Sub, op == Op::Mul, op == Op::Div];
                    app.start_session(false);
                    app.set_area(Rect::new(0, 0, w, h));
                    app.help_active = true;
                    app.encourage = Some("Don't worry — mistakes help you grow!".to_string());
                    for _ in 0..10 {
                        app.on_tick();
                    }
                    for idx in 0..6 {
                        app.strategy_index = idx;
                        for &rev in &[false, true] {
                            app.revealed = rev;
                            for frame in 0..4 {
                                app.anim_frame = frame * 5;
                                term.draw(|f| ui::draw(f, &app)).unwrap();
                            }
                        }
                    }
                    // Why overlay.
                    app.help_active = false;
                    app.why_active = true;
                    app.refresh_why_items();
                    term.draw(|f| ui::draw(f, &app)).unwrap();
                    app.why_active = false;
                }
                app.feedback = Feedback::Wrong;
                term.draw(|f| ui::draw(f, &app)).unwrap();
            }

            // Challenge HUD + summary.
            app.start_session(true);
            app.set_area(Rect::new(0, 0, w, h));
            term.draw(|f| ui::draw(f, &app)).unwrap();
            if let Some(c) = &mut app.challenge {
                c.finished = true;
            }
            term.draw(|f| ui::draw(f, &app)).unwrap();
        }
    }

    /// Every transition effect must play start-to-finish without panicking.
    #[test]
    fn all_transitions_play_safely() {
        let mut rng = rand::thread_rng();
        for (w, h) in [(80, 24), (32, 12)] {
            let area = Rect::new(0, 0, w, h);
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            for effect in Effect::all() {
                let mut from = Buffer::empty(area);
                let mut to = Buffer::empty(area);
                let p1 = problem::generate(Config::default().range(4), &Op::ALL, &mut rng);
                let p2 = problem::generate(Config::default().range(4), &Op::ALL, &mut rng);
                ui::render_card(area, &mut from, &p1, "12", crate::config::Layout::Horizontal, None);
                ui::render_card(area, &mut to, &p2, "", crate::config::Layout::Horizontal, None);
                let mut t = Transition::with_effect(effect, from, to, &mut rng);
                loop {
                    term.draw(|f| t.render(area, f.buffer_mut())).unwrap();
                    if t.advance() {
                        break;
                    }
                }
            }
        }
    }

    #[test]
    fn division_is_always_exact_and_answers_check_out() {
        let mut rng = rand::thread_rng();
        let cfg = Config::default();
        for grade in 0..=8u8 {
            for _ in 0..500 {
                let p = problem::generate(cfg.range(grade), &Op::ALL, &mut rng);
                let computed = match p.op {
                    Op::Add => p.a + p.b,
                    Op::Sub => p.a - p.b,
                    Op::Mul => p.a * p.b,
                    Op::Div => {
                        assert_eq!(p.a % p.b, 0, "division must be exact");
                        p.a / p.b
                    }
                };
                assert_eq!(computed, p.answer);
            }
        }
    }

    /// The Why panel must show all content when there's room (no dropped
    /// continuation line), and report a scroll range when there isn't.
    #[test]
    fn why_panel_fits_when_tall_and_scrolls_when_short() {
        let make = || {
            let mut app = App::new(Config::default());
            app.menu_ops = [true, false, false, false];
            app.start_session(false);
            app.why_active = true;
            app.why_items = vec![
                "Totalling up the cost of everything in your shopping cart".to_string(),
                "Adding up calories so astronauts pack enough food for space".to_string(),
                "Adding the fuel in each rocket stage to reach orbit".to_string(),
                "Summing the weights of cargo so a plane stays balanced".to_string(),
            ];
            app.why_code = Some((
                "self.progress += 1.0 / duration;".to_string(),
                "Adding a little each frame is how one problem smoothly melts into the next.".to_string(),
            ));
            app
        };

        // Plenty of height: everything fits, nothing scrolls.
        let app = make();
        let mut term = Terminal::new(TestBackend::new(80, 30)).unwrap();
        term.draw(|f| ui::draw(f, &app)).unwrap();
        assert_eq!(app.why_max_scroll.get(), 0, "tall panel should not need scrolling");

        // Cramped height: the overflow becomes scrollable.
        let app = make();
        let mut term = Terminal::new(TestBackend::new(80, 13)).unwrap();
        term.draw(|f| ui::draw(f, &app)).unwrap();
        assert!(app.why_max_scroll.get() > 0, "short panel should scroll");
    }

    #[test]
    fn milestone_triggers_cinematic_then_resumes() {
        let mut app = App::new(Config::default());
        app.menu_ops = [true, false, false, false]; // addition only
        app.start_session(false);
        app.set_area(Rect::new(0, 0, 80, 24));
        // Sit at 6 solved so the next correct answer hits the 7 milestone.
        app.roster.current_mut().solved = [6, 0, 0, 0];

        app.input = app.current.answer.to_string();
        app.check_answer();
        assert_eq!(app.screen, Screen::Cinematic);
        assert!(app.cinematic.is_some());

        // Play it out; it must hand control back to the lesson.
        let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
        for _ in 0..600 {
            term.draw(|f| ui::draw(f, &app)).unwrap();
            app.on_tick();
            if app.cinematic.is_none() {
                break;
            }
        }
        assert!(app.cinematic.is_none(), "cinematic should finish");
        assert_eq!(app.screen, Screen::Practice);
    }

    #[test]
    fn repeated_wrong_answers_summon_encouragement() {
        let mut app = App::new(Config::default());
        app.menu_ops = [true, false, false, false];
        app.start_session(false);
        let wrong = (app.current.answer + 1).to_string();
        for _ in 0..STRUGGLE_THRESHOLD {
            app.input = wrong.clone();
            app.check_answer();
        }
        assert!(app.encourage.is_some(), "duck should offer encouragement");
        assert!(app.help_active, "duck should appear");

        // Getting it right clears the support and stops the duck struggling.
        app.input = app.current.answer.to_string();
        app.check_answer();
        assert!(app.encourage.is_none());
    }

    #[test]
    fn student_progress_only_grows() {
        use crate::student::Student;
        let mut s = Student::new("Sam");
        assert_eq!(s.total(), 0);
        s.record(Op::Add);
        s.record(Op::Add);
        s.record(Op::Mul);
        s.note_streak(3);
        s.note_streak(1); // a worse streak never lowers the best
        assert_eq!(s.solved[Op::Add.index()], 2);
        assert_eq!(s.solved[Op::Mul.index()], 1);
        assert_eq!(s.total(), 3);
        assert_eq!(s.best_streak, 3);
    }

    #[test]
    fn teacher_password_set_and_verify() {
        let mut cfg = Config::default();
        assert!(!cfg.has_teacher_password());
        cfg.set_teacher_password("  "); // blank ignored
        assert!(!cfg.has_teacher_password());
        cfg.set_teacher_password("ducks123");
        assert!(cfg.has_teacher_password());
        assert!(cfg.verify_teacher_password("ducks123"));
        assert!(!cfg.verify_teacher_password("wrong"));
    }

    #[test]
    fn records_admin_reset_and_remove() {
        use crate::student::{Roster, Student};
        let mut r = Roster::default();
        r.add("Ada");
        r.add("Grace");
        r.current_mut().record(Op::Mul);
        r.current_mut().note_streak(4);
        assert_eq!(r.current().total(), 1);

        // Reset zeroes the records.
        r.current_mut().reset();
        assert_eq!(r.current().total(), 0);
        assert_eq!(r.current().best_streak, 0);

        // Removing every student still leaves a Guest behind.
        let mut solo = Roster { students: vec![Student::new("Only")], current: 0 };
        solo.remove(0);
        assert_eq!(solo.students.len(), 1);
        assert_eq!(solo.students[0].name, "Guest");
    }

    #[test]
    fn teacher_extras_join_the_why_list() {
        use crate::motivation::Extras;
        let mut rng = rand::thread_rng();
        let mut extras = Extras::default();
        let custom = "Splitting my paycheck into savings jars";
        extras.add(Op::Div, custom);
        // With a large draw the custom entry should be reachable.
        let mut seen = false;
        for _ in 0..200 {
            if motivation::pick(Op::Div, &extras, &mut rng, 50).iter().any(|s| s == custom) {
                seen = true;
                break;
            }
        }
        assert!(seen, "teacher's custom reason never appeared");
    }

    /// Moderate add/subtract problems should get a *visual* number line, not the
    /// plain text fallback — and every line's hops must land on the answer.
    #[test]
    fn arithmetic_keeps_a_visual_number_line() {
        use crate::strategy::{strategies, Viz};
        let cases = [
            (12i64, 7i64, Op::Add),
            (40, 30, Op::Add),
            (23, 45, Op::Add),
            (8, 6, Op::Add),
            (53, 27, Op::Sub),
            (50, 20, Op::Sub),
            (12, 5, Op::Sub),
            (100, 64, Op::Sub),
        ];
        for (a, b, op) in cases {
            let answer = if op == Op::Add { a + b } else { a - b };
            let p = problem::Problem { a, b, op, answer, accent: ratatui::style::Color::White };
            let mut found = false;
            for s in strategies(&p) {
                if let Viz::NumberLine { stops, hops } = &s.viz {
                    found = true;
                    assert_eq!(stops.len(), hops.len() + 1, "stop/hop mismatch for {} {:?} {}", a, op, b);
                    // A line lands on the answer (count-back/add) or on `a`
                    // (count-up walks from b up to a).
                    let last = *stops.last().unwrap();
                    assert!(last == answer || last == a, "line for {} {:?} {} ends at {}", a, op, b, last);
                }
            }
            assert!(found, "{} {:?} {} lost its number line", a, op, b);
        }
    }

    /// Strategies are produced for every operation and grade, none empty.
    #[test]
    fn strategies_exist_for_all_problems() {
        let mut rng = rand::thread_rng();
        let cfg = Config::default();
        for grade in 0..=8u8 {
            for _ in 0..200 {
                let p = problem::generate(cfg.range(grade), &Op::ALL, &mut rng);
                let strats = strategy::strategies(&p);
                assert!(!strats.is_empty(), "no strategy for {:?}", p.op);
            }
        }
    }
}
