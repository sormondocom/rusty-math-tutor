//! Application state machine: the start menu, a no-timer Practice session, and
//! an optional timed Challenge session.  Owns input handling, the per-tick
//! animation clock, and the hand-off into [`crate::transition`] when a problem
//! is answered correctly.

use std::time::{Duration, Instant};

use crate::input::{InputEvent, Key, Mods};
use rand::rngs::ThreadRng;
use rand::Rng;

use crate::config::Config;
use crate::problem::{self, Op};
use crate::section::Active;
use crate::storage::Storage;
use crate::topic::Topic;
use crate::transition::TransitionPhase;

/// How long a Challenge run lasts.
const CHALLENGE_LEN: Duration = Duration::from_secs(60);
/// Maximum digits a student can type for an answer.
const MAX_INPUT: usize = 7;
/// Number of selectable rows on the menu.
const MENU_ITEMS: usize = 17;
/// Ticks per shape region while a fraction shape materialises.
const FRAC_MAT_PER_REGION: u32 = 6;
/// Editable fields on the Experimentation explorer.
const EXP_FIELDS: usize = 4;
/// Editable rows on the Settings screen (grade + four range knobs).
pub const SETTINGS_FIELDS: usize = 5;

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
    Experiment,
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
    /// Ticks left to linger on the current scene (counts down from its dwell).
    pub dwell: u32,
    /// Scene-to-scene transition timing (the terminal frontend renders it).
    pub transition: Option<TransitionPhase>,
    started: Instant,
}

// Menu row indices.
const MI_STUDENT: usize = 0;
const MI_GRADE: usize = 1;
const MI_OPS: std::ops::RangeInclusive<usize> = 2..=5;
const MI_UNITS: usize = 6;
const MI_FRACTIONS: usize = 7;
const MI_PERCENTS: usize = 8;
const MI_GEOMETRY: usize = 9;
const MI_LAYOUT: usize = 10;
const MI_SETTINGS: usize = 11;
const MI_PROGRESS: usize = 12;
const MI_TEACHER: usize = 13;
const MI_PRACTICE: usize = 14;
const MI_CHALLENGE: usize = 15;
const MI_EXPERIMENT: usize = 16;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
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
    /// Where config / roster / "Why?" extras are persisted.  The terminal
    /// frontend backs this with files; tests use an in-memory store.
    storage: Box<dyn Storage>,

    // Adding a student from the menu.
    pub naming: bool,
    pub name_input: String,
    /// Current run of correct answers (for the personal-best streak).
    pub streak: u32,

    // Menu selections.
    pub menu_grade: u8,
    pub menu_ops: [bool; 4],
    /// Whether measurement problems are mixed into the session.
    pub menu_units: bool,
    /// Whether fraction problems are mixed into the session.
    pub menu_fractions: bool,
    /// Whether percentage problems are mixed into the session.
    pub menu_percents: bool,
    /// Whether geometry problems are mixed into the session.
    pub menu_geometry: bool,
    pub menu_index: usize,

    // Startup graphics-mode picker.
    pub startup_index: usize,

    // Settings screen.
    pub settings_grade: u8,
    pub settings_field: usize,

    // Active session.  Every section's problem flows through the one [`Active`]
    // enum, so the kind-specific bookkeeping (check / render / score / help)
    // lives in one place rather than as parallel booleans.
    grade: u8,
    ops: Vec<Op>,
    /// The problem on screen, whichever section produced it.
    pub current: Active,
    pub input: String,
    pub feedback: Feedback,

    // Which sections are mixed into the running session.
    units_enabled: bool,
    fractions_enabled: bool,
    percents_enabled: bool,
    geometry_enabled: bool,

    // Experimentation — free-form unit explorer.
    pub exp_category: usize,
    pub exp_amount: String,
    pub exp_from: usize,
    pub exp_to: usize,
    pub exp_field: usize,

    /// Ticks the current shape (fraction / percent) has been materialising.
    pub frac_anim: u32,

    // Transition between the answered card and the next one.  The core tracks
    // only the timing/phase; the terminal frontend owns the captured pixels.
    pub transition: Option<TransitionPhase>,
    /// The next problem, shown as the transition's destination card.  Public so
    /// the frontend can render it during a transition.
    pub pending: Option<Active>,

    // Deduction Duck help overlay: `help_in` eases 0..1 toward the target set
    // by `help_active`; `strategy_index` selects which method he shows.
    pub help_active: bool,
    pub help_in: f32,
    pub strategy_index: usize,
    /// Whether Deduction Duck has revealed the worked answer (hint-first).
    pub revealed: bool,
    /// Recent answer reveals; over [`REVEAL_LOCK`] the answer is on cooldown.
    peeks: u32,
    /// A clever reprimand shown when a student insists during the cooldown.
    pub reprimand: Option<String>,
    reprimand_index: usize,

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
    /// Topic (section) currently being edited (index into [`Topic::ALL`]).
    pub teacher_topic: usize,
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

    pub should_quit: bool,
}

impl App {
    pub fn new(config: Config, storage: Box<dyn Storage>) -> Self {
        let mut rng = rand::thread_rng();
        let current = Active::Arith(problem::generate(config.range(1), &[Op::Add], &mut rng));
        let startup_index = if config.graphics == crate::config::GraphicsMode::Cpu { 1 } else { 0 };
        let roster = crate::student::Roster::load(storage.as_ref());
        let why_extras = crate::motivation::Extras::load(storage.as_ref());
        App {
            screen: Screen::Startup,
            rng,
            config,
            roster,
            storage,
            naming: false,
            name_input: String::new(),
            streak: 0,
            menu_grade: 1,
            menu_ops: [true, true, false, false],
            menu_units: false,
            menu_fractions: false,
            menu_percents: false,
            menu_geometry: false,
            menu_index: 0,
            startup_index,
            settings_grade: 1,
            settings_field: 0,
            grade: 1,
            ops: vec![Op::Add],
            current,
            units_enabled: false,
            fractions_enabled: false,
            percents_enabled: false,
            geometry_enabled: false,
            // Defaults primed for a fun "8000 gallons -> teaspoons" experiment.
            exp_category: 0,
            exp_amount: "8000".to_string(),
            exp_from: 6,
            exp_to: 0,
            exp_field: 0,
            frac_anim: 0,
            input: String::new(),
            feedback: Feedback::None,
            transition: None,
            pending: None,
            help_active: false,
            help_in: 0.0,
            strategy_index: 0,
            revealed: false,
            peeks: 0,
            reprimand: None,
            reprimand_index: 0,
            why_active: false,
            why_items: Vec::new(),
            why_code: None,
            why_scroll: 0,
            why_max_scroll: std::cell::Cell::new(0),
            why_extras,
            teacher_authed: false,
            teacher_view: TeacherView::Anecdotes,
            teacher_pw: String::new(),
            teacher_topic: 0,
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
            should_quit: false,
        }
    }

    // -- persistence (routed through the frontend's Storage) ----------------

    fn save_config(&self) {
        self.config.save(self.storage.as_ref());
    }

    fn save_roster(&self) {
        self.roster.save(self.storage.as_ref());
    }

    fn save_extras(&self) {
        self.why_extras.save(self.storage.as_ref());
    }

    /// Persist everything that changes during play (config + roster).  The
    /// frontend calls this on the way out.
    pub fn persist(&self) {
        self.save_config();
        self.save_roster();
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
                self.commit_pending();
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
                    self.save_roster();
                }
            }
        }

        // A shape problem (fraction / percent) materialises its figure over time.
        if self.current.materializes() && self.transition.is_none() {
            self.frac_anim = self.frac_anim.saturating_add(1);
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
                c.dwell = c.scenes.get(c.index).map_or(SCENE_DWELL, |s| s.dwell);
            }
        } else {
            c.dwell = c.dwell.saturating_sub(1);
            if c.dwell == 0 {
                if c.index + 1 < c.scenes.len() {
                    // Start a scene-to-scene transition; the terminal frontend
                    // blends scene `index` (settled) into `index + 1` (frame 0).
                    c.transition = Some(TransitionPhase::new(&mut self.rng));
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
        self.commit_pending();
        self.input.clear();
        self.feedback = Feedback::None;
        self.strategy_index = 0;
        self.revealed = false;
        self.wrong_streak = 0;
        self.encourage = None;
    }

    // -- input --------------------------------------------------------------

    /// Frontend-neutral entry point: every backend maps its native input into an
    /// [`InputEvent`] and feeds it here.
    pub fn on_event(&mut self, ev: InputEvent) {
        match ev {
            InputEvent::Key { key, mods } => self.on_key(key, mods),
        }
    }

    fn on_key(&mut self, key: Key, mods: Mods) {
        // Ctrl+C always quits.
        if mods.ctrl && key == Key::Char('c') {
            self.should_quit = true;
            return;
        }
        match self.screen {
            Screen::Startup => self.on_startup_key(key),
            Screen::Menu => self.on_menu_key(key, mods),
            Screen::Settings => self.on_settings_key(key),
            Screen::Stats => {
                if matches!(key, Key::Esc | Key::Enter) {
                    self.screen = Screen::Menu;
                }
            }
            Screen::Teacher => self.on_teacher_key(key, mods),
            Screen::Cinematic => {
                // Any key skips the celebration and resumes the lesson.
                if let Some(c) = self.cinematic.take() {
                    self.finish_cinematic(c);
                }
            }
            Screen::Practice | Screen::Challenge => self.on_session_key(key),
            Screen::Experiment => self.on_experiment_key(key),
        }
    }

    fn on_startup_key(&mut self, key: Key) {
        use crate::config::GraphicsMode;
        match key {
            Key::Up | Key::Down => self.startup_index ^= 1,
            Key::Enter | Key::Char(' ') => {
                // CPU mode is shelved; only Low actually proceeds.
                let mode = if self.startup_index == 1 { GraphicsMode::Cpu } else { GraphicsMode::Low };
                if mode.available() {
                    self.config.graphics = mode;
                    self.screen = Screen::Menu;
                }
            }
            Key::Char('q') | Key::Char('Q') | Key::Esc => self.should_quit = true,
            _ => {}
        }
    }

    fn on_menu_key(&mut self, key: Key, mods: Mods) {
        // Typing a new student's name captures everything until committed.
        if self.naming {
            self.on_name_input_key(key, mods);
            return;
        }

        match key {
            Key::Up => self.menu_index = (self.menu_index + MENU_ITEMS - 1) % MENU_ITEMS,
            Key::Down => self.menu_index = (self.menu_index + 1) % MENU_ITEMS,
            Key::Left if self.menu_index == MI_STUDENT => self.roster.cycle(-1),
            Key::Right if self.menu_index == MI_STUDENT => self.roster.cycle(1),
            Key::Left if self.menu_index == MI_GRADE => {
                self.menu_grade = self.menu_grade.saturating_sub(1);
            }
            Key::Right if self.menu_index == MI_GRADE => {
                self.menu_grade = (self.menu_grade + 1).min(8);
            }
            // N adds a new student from anywhere on the menu.
            Key::Char('n') | Key::Char('N') => {
                self.naming = true;
                self.name_input.clear();
            }
            Key::Char(' ') | Key::Enter => match self.menu_index {
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
                MI_UNITS => self.menu_units = !self.menu_units,
                MI_FRACTIONS => self.menu_fractions = !self.menu_fractions,
                MI_PERCENTS => self.menu_percents = !self.menu_percents,
                MI_GEOMETRY => self.menu_geometry = !self.menu_geometry,
                MI_PRACTICE => self.start_session(false),
                MI_CHALLENGE => self.start_session(true),
                MI_EXPERIMENT => self.screen = Screen::Experiment,
                _ => {}
            },
            Key::Char('q') | Key::Char('Q') | Key::Esc => self.should_quit = true,
            _ => {}
        }
    }

    /// Handle a keystroke while typing a new student's name.
    fn on_name_input_key(&mut self, key: Key, mods: Mods) {
        match key {
            Key::Esc => {
                self.naming = false;
                self.name_input.clear();
            }
            Key::Enter => {
                if !self.name_input.trim().is_empty() {
                    self.roster.add(&self.name_input);
                    self.save_roster();
                }
                self.naming = false;
                self.name_input.clear();
            }
            Key::Backspace => {
                self.name_input.pop();
            }
            Key::Char(c) if !mods.ctrl => {
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
        self.teacher_topic = 0;
        self.teacher_rec_index = 0;
        self.teacher_msg = None;
    }

    fn on_teacher_key(&mut self, key: Key, mods: Mods) {
        if !self.teacher_authed {
            self.on_teacher_login_key(key, mods);
        } else if self.teacher_adding {
            self.on_teacher_add_key(key, mods);
        } else {
            self.on_teacher_manage_key(key);
        }
    }

    /// Setting (first visit) or entering the teacher password.
    fn on_teacher_login_key(&mut self, key: Key, mods: Mods) {
        match key {
            Key::Esc => self.screen = Screen::Menu,
            Key::Backspace => {
                self.teacher_pw.pop();
            }
            Key::Char(c) if !mods.ctrl => {
                if self.teacher_pw.chars().count() < 32 {
                    self.teacher_pw.push(c);
                }
            }
            Key::Enter => {
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
                    self.save_config();
                    self.teacher_authed = true;
                    self.teacher_msg = Some("Password set! You're logged in.".to_string());
                }
                self.teacher_pw.clear();
            }
            _ => {}
        }
    }

    /// Typing a new anecdote for the selected operation.
    fn on_teacher_add_key(&mut self, key: Key, mods: Mods) {
        match key {
            Key::Esc => {
                self.teacher_adding = false;
                self.teacher_text.clear();
            }
            Key::Backspace => {
                self.teacher_text.pop();
            }
            Key::Char(c) if !mods.ctrl => {
                if self.teacher_text.chars().count() < ANECDOTE_MAX {
                    self.teacher_text.push(c);
                }
            }
            Key::Enter => {
                let topic = Topic::ALL[self.teacher_topic];
                if !self.teacher_text.trim().is_empty() {
                    self.why_extras.add(topic, &self.teacher_text);
                    self.save_extras();
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
    fn on_teacher_manage_key(&mut self, key: Key) {
        if key == Key::Esc {
            self.teacher_authed = false;
            self.screen = Screen::Menu;
            return;
        }
        if key == Key::Tab {
            self.teacher_view = match self.teacher_view {
                TeacherView::Anecdotes => TeacherView::Records,
                TeacherView::Records => TeacherView::Anecdotes,
            };
            self.teacher_msg = None;
            return;
        }
        // L cycles the measurement locality (used by Units of Measure).
        if matches!(key, Key::Char('l') | Key::Char('L')) {
            self.cycle_locality(true);
            self.teacher_msg = Some(format!("Units locality: {}", self.config.locality.name()));
            return;
        }
        match self.teacher_view {
            TeacherView::Anecdotes => match key {
                Key::Left => self.teacher_topic = (self.teacher_topic + Topic::ALL.len() - 1) % Topic::ALL.len(),
                Key::Right => self.teacher_topic = (self.teacher_topic + 1) % Topic::ALL.len(),
                Key::Char('a') | Key::Char('A') => {
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
    fn on_teacher_records_key(&mut self, key: Key) {
        let n = self.roster.students.len();
        self.teacher_rec_index = self.teacher_rec_index.min(n - 1);
        match key {
            Key::Up => self.teacher_rec_index = (self.teacher_rec_index + n - 1) % n,
            Key::Down => self.teacher_rec_index = (self.teacher_rec_index + 1) % n,
            Key::Left => self.teacher_topic = (self.teacher_topic + Topic::ALL.len() - 1) % Topic::ALL.len(),
            Key::Right => self.teacher_topic = (self.teacher_topic + 1) % Topic::ALL.len(),
            // S: reset just the selected topic's count for this student.
            Key::Char('s') | Key::Char('S') => {
                let topic = Topic::ALL[self.teacher_topic];
                let name = self.roster.students[self.teacher_rec_index].name.clone();
                self.roster.students[self.teacher_rec_index].reset_topic(topic);
                self.save_roster();
                self.teacher_msg = Some(format!("Reset {} for {}.", topic.name(), name));
            }
            // R: reset all of this student's records.
            Key::Char('r') | Key::Char('R') => {
                let name = self.roster.students[self.teacher_rec_index].name.clone();
                self.roster.students[self.teacher_rec_index].reset();
                self.save_roster();
                self.teacher_msg = Some(format!("Reset all records for {}.", name));
            }
            // X: remove the student entirely (a Guest is kept if it's the last).
            Key::Char('x') | Key::Char('X') => {
                let name = self.roster.students[self.teacher_rec_index].name.clone();
                self.roster.remove(self.teacher_rec_index);
                self.teacher_rec_index = self.teacher_rec_index.min(self.roster.students.len() - 1);
                self.save_roster();
                self.teacher_msg = Some(format!("Removed {}.", name));
            }
            // + / - : adjust this student's reveal limit (0 = never lock, max 9).
            Key::Char('+') | Key::Char('=') => {
                let s = &mut self.roster.students[self.teacher_rec_index];
                s.reveal_lock = (s.reveal_lock + 1).min(9);
                self.save_roster();
            }
            Key::Char('-') | Key::Char('_') => {
                let s = &mut self.roster.students[self.teacher_rec_index];
                s.reveal_lock = s.reveal_lock.saturating_sub(1);
                self.save_roster();
            }
            _ => {}
        }
    }

    fn on_settings_key(&mut self, key: Key) {
        match key {
            Key::Up => self.settings_field = (self.settings_field + SETTINGS_FIELDS - 1) % SETTINGS_FIELDS,
            Key::Down => self.settings_field = (self.settings_field + 1) % SETTINGS_FIELDS,
            Key::Left => self.adjust_setting(false),
            Key::Right => self.adjust_setting(true),
            Key::Esc | Key::Enter => {
                // Save and return to the menu.
                self.save_config();
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

    fn on_session_key(&mut self, key: Key) {
        // Esc peels back one overlay at a time, then returns to the menu.
        if key == Key::Esc {
            if self.why_active {
                self.why_active = false;
            } else if self.help_active {
                self.help_active = false;
            } else {
                self.to_menu();
            }
            return;
        }

        // The help duck (H) and answer reveal (R) work for every problem —
        // arithmetic shows strategies, measurement shows a how-to hint.
        if matches!(key, Key::Char('h') | Key::Char('H')) {
            self.help_active = !self.help_active;
            if self.help_active {
                self.why_active = false;
                self.strategy_index = 0;
                self.revealed = false;
            }
            return;
        }
        if self.help_active && matches!(key, Key::Char('r') | Key::Char('R')) {
            self.reveal_answer();
            return;
        }

        // The Why panel works for every section — each topic carries its own
        // real-world uses and teacher anecdotes.
        if matches!(key, Key::Char('y') | Key::Char('Y')) {
            self.why_active = !self.why_active;
            if self.why_active {
                self.help_active = false;
                self.refresh_why_items();
            }
            return;
        }

        // While the Why panel is open, Up/Down scroll long lists.
        if self.why_active && matches!(key, Key::Up | Key::Down) {
            if key == Key::Up {
                self.why_scroll = self.why_scroll.saturating_sub(1);
            } else {
                self.why_scroll = (self.why_scroll + 1).min(self.why_max_scroll.get());
            }
            return;
        }

        // Strategy cycling and the layout toggle belong to sections that carry
        // strategies (arithmetic); the others just take an answer + a hint.
        if self.current.has_strategies() {
            // While the duck is out, Space cycles to another strategy.
            if self.help_active && key == Key::Char(' ') {
                self.strategy_index = self.strategy_index.wrapping_add(1);
                self.revealed = false;
                return;
            }

            // V switches the problem layout live.
            if matches!(key, Key::Char('v') | Key::Char('V')) {
                self.config.layout = self.config.layout.toggled();
                return;
            }
        }

        // Finished challenge: Enter starts a fresh run.
        if self.challenge.as_ref().map_or(false, |c| c.finished) {
            if key == Key::Enter {
                self.start_session(true);
            }
            return;
        }

        // Ignore answer input while a transition is playing.
        if self.transition.is_some() {
            return;
        }

        match key {
            Key::Char(d @ '0'..='9') => {
                if self.feedback == Feedback::Wrong {
                    self.input.clear();
                }
                self.feedback = Feedback::None;
                if self.input.len() < MAX_INPUT {
                    self.input.push(d);
                }
            }
            Key::Char('-') if self.current.accepts_minus() => {
                // Leading minus only (for grades that allow negative answers).
                if self.input.is_empty() {
                    self.input.push('-');
                }
            }
            // Fractions are entered as "a/b".
            Key::Char('/') if self.current.accepts_slash() => {
                if !self.input.is_empty() && !self.input.contains('/') {
                    self.input.push('/');
                }
            }
            Key::Backspace => {
                self.feedback = Feedback::None;
                self.input.pop();
            }
            Key::Enter => self.check_answer(),
            _ => {}
        }
    }

    /// Which [`Topic`] the active problem belongs to — the hub the per-section
    /// components (progress, Why?, anecdotes) hang off of.
    pub fn current_topic(&self) -> Topic {
        self.current.topic()
    }

    fn refresh_why_items(&mut self) {
        let topic = self.current_topic();
        self.why_items = crate::motivation::pick(topic, &self.why_extras, &mut self.rng, 4);
        self.why_code = crate::motivation::pick_code(topic, &mut self.rng);
        self.why_scroll = 0;
    }

    /// The active student's reveal limit (0 = never lock).
    fn reveal_lock(&self) -> u32 {
        self.roster.current().reveal_lock
    }

    /// Whether the answer is currently on peek-cooldown (hidden, can't reveal).
    pub fn reveal_locked(&self) -> bool {
        let lock = self.reveal_lock();
        lock > 0 && !self.revealed && self.peeks >= lock
    }

    /// Handle the R key: reveal the answer, hide it again, or — if the student
    /// has been peeking too much — deliver a clever reprimand instead.
    fn reveal_answer(&mut self) {
        let lock = self.reveal_lock();
        if self.revealed {
            self.revealed = false;
            self.reprimand = None;
        } else if lock > 0 && self.peeks >= lock {
            self.reprimand = Some(crate::cinematic::peek_reprimand(self.reprimand_index));
            self.reprimand_index += 1;
        } else {
            self.revealed = true;
            self.peeks += 1;
            self.reprimand = None;
        }
    }

    // -- session helpers ----------------------------------------------------

    fn start_session(&mut self, challenge: bool) {
        self.grade = self.menu_grade;
        self.ops = Op::ALL.iter().copied().enumerate().filter(|(i, _)| self.menu_ops[*i]).map(|(_, op)| op).collect();
        self.units_enabled = self.menu_units;
        self.fractions_enabled = self.menu_fractions;
        self.percents_enabled = self.menu_percents;
        self.geometry_enabled = self.menu_geometry;
        // Need at least one problem type — fall back to addition.
        if self.ops.is_empty() && !self.units_enabled && !self.fractions_enabled && !self.percents_enabled && !self.geometry_enabled {
            self.ops.push(Op::Add);
        }
        self.screen = if challenge { Screen::Challenge } else { Screen::Practice };
        self.input.clear();
        self.feedback = Feedback::None;
        self.transition = None;
        self.pending = None;
        self.frac_anim = 0;
        self.help_active = false;
        self.help_in = 0.0;
        self.strategy_index = 0;
        self.revealed = false;
        self.peeks = 0;
        self.reprimand = None;
        self.reprimand_index = 0;
        self.why_active = false;
        self.streak = 0;
        self.wrong_streak = 0;
        self.encourage = None;
        self.cinematic = None;
        self.challenge = if challenge { Some(Challenge::new(CHALLENGE_LEN)) } else { None };
        // Pick the first problem (arithmetic or measurement).
        self.generate_pending();
        self.commit_pending();
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
        self.save_roster();
    }

    fn check_answer(&mut self) {
        // Each section knows how to check its own answer format.
        let correct = self.current.check(&self.input);
        if correct {
            self.feedback = Feedback::Correct;
            if let Some(c) = &mut self.challenge {
                c.solved += 1;
            }
            self.streak += 1;
            let streak = self.streak;
            self.roster.current_mut().note_streak(streak);
            self.wrong_streak = 0;
            self.encourage = None;
            self.help_active = false;
            // Solving it yourself (no peek) earns back a reveal.
            if !self.revealed {
                self.peeks = self.peeks.saturating_sub(1);
            }

            // Credit the topic's counter.  Only arithmetic drives the milestone
            // celebrations (which key off the arithmetic total).
            let topic = self.current_topic();
            self.roster.current_mut().record_topic(topic);
            if matches!(topic, Topic::Add | Topic::Sub | Topic::Mul | Topic::Div) {
                let total = self.roster.current().total();
                if crate::cinematic::is_milestone(total) {
                    self.generate_pending();
                    self.start_cinematic(total);
                    return;
                }
            }
            self.begin_transition();
        } else {
            self.feedback = Feedback::Wrong;
            self.streak = 0;
            // After a few tries the duck steps in (with a hint for either kind).
            self.wrong_streak += 1;
            if self.wrong_streak >= STRUGGLE_THRESHOLD {
                self.encourage = Some(crate::cinematic::struggle_message(&mut self.rng));
                self.help_active = true;
                self.strategy_index = 0;
                self.revealed = false;
            }
        }
    }

    /// Pick the next problem and start a transition to it.  The core only tracks
    /// the timing; the terminal frontend captures the cards and animates them.
    fn begin_transition(&mut self) {
        self.generate_pending();
        self.transition = Some(TransitionPhase::new(&mut self.rng));
    }

    // -- Units of Measure (mixed into sessions) -----------------------------

    fn cycle_locality(&mut self, forward: bool) {
        let all = crate::units::Locality::ALL;
        let i = all.iter().position(|&l| l == self.config.locality).unwrap_or(0);
        let n = all.len();
        let next = if forward { (i + 1) % n } else { (i + n - 1) % n };
        self.config.locality = all[next];
        self.save_config();
    }

    /// Generate the next problem, randomly choosing a section from the enabled
    /// mix (arithmetic, units, fractions, percentages).
    fn generate_pending(&mut self) {
        // Kinds: 0 = arithmetic, 1 = units, 2 = fractions, 3 = percentages,
        // 4 = geometry.
        let mut kinds: Vec<u8> = Vec::new();
        if !self.ops.is_empty() {
            kinds.push(0);
        }
        if self.units_enabled {
            kinds.push(1);
        }
        if self.fractions_enabled {
            kinds.push(2);
        }
        if self.percents_enabled {
            kinds.push(3);
        }
        if self.geometry_enabled {
            kinds.push(4);
        }
        if kinds.is_empty() {
            kinds.push(0);
        }
        let kind = kinds[self.rng.gen_range(0..kinds.len())];
        self.pending = Some(match kind {
            1 => Active::Unit(crate::units::generate(self.config.locality, &mut self.rng)),
            2 => Active::Shape(crate::fraction::generate(&mut self.rng)),
            3 => Active::Shape(crate::fraction::generate_percent(&mut self.rng)),
            4 => Active::Geo(crate::geometry::generate(&mut self.rng)),
            _ => Active::Arith(problem::generate(self.config.range(self.grade), &self.ops, &mut self.rng)),
        });
    }

    /// Promote the pending problem to the current one (after a transition or
    /// cinematic).
    fn commit_pending(&mut self) {
        if let Some(next) = self.pending.take() {
            if next.materializes() {
                self.frac_anim = 0; // start materialising the new shape
            }
            self.current = next;
        }
        // Fresh problem: hide the answer and clear any reprimand escalation.
        self.revealed = false;
        self.reprimand = None;
        self.reprimand_index = 0;
    }

    // -- Experimentation (free-form unit explorer) --------------------------

    fn exp_unit_count(&self) -> usize {
        crate::units::Category::ALL[self.exp_category].units().len()
    }

    fn on_experiment_key(&mut self, key: Key) {
        match key {
            Key::Esc => self.to_menu(),
            Key::Up => self.exp_field = (self.exp_field + EXP_FIELDS - 1) % EXP_FIELDS,
            Key::Down => self.exp_field = (self.exp_field + 1) % EXP_FIELDS,
            Key::Left | Key::Right => {
                let fwd = key == Key::Right;
                self.adjust_experiment(fwd);
            }
            // Digits and a single decimal point edit the amount from any field.
            Key::Char(c @ '0'..='9') => {
                if self.exp_amount.chars().count() < 12 {
                    self.exp_amount.push(c);
                }
            }
            Key::Char('.') => {
                if !self.exp_amount.contains('.') && self.exp_amount.chars().count() < 12 {
                    self.exp_amount.push('.');
                }
            }
            Key::Backspace => {
                self.exp_amount.pop();
            }
            _ => {}
        }
    }

    /// How far the current shape (fraction / percent) has materialised, `0.0..=1.0`.
    pub fn frac_progress(&self) -> f32 {
        let regions = self.current.shape_regions();
        (self.frac_anim as f32 / (regions * FRAC_MAT_PER_REGION) as f32).min(1.0)
    }

    fn adjust_experiment(&mut self, fwd: bool) {
        let step = |i: usize, n: usize| if fwd { (i + 1) % n } else { (i + n - 1) % n };
        match self.exp_field {
            // 0 = amount (no left/right effect)
            1 => self.exp_from = step(self.exp_from, self.exp_unit_count()),
            2 => self.exp_to = step(self.exp_to, self.exp_unit_count()),
            3 => {
                let n = crate::units::Category::ALL.len();
                self.exp_category = step(self.exp_category, n);
                // Keep unit choices valid for the new category.
                let count = self.exp_unit_count();
                self.exp_from = self.exp_from.min(count - 1);
                self.exp_to = self.exp_to.min(count - 1);
            }
            _ => {}
        }
    }

    /// Kick off a milestone celebration; the already-generated pending problem
    /// resumes the lesson afterwards.
    fn start_cinematic(&mut self, total: u32) {
        let name = self.roster.current().name.clone();
        let scenes = crate::cinematic::scenes(&name, total, &mut self.rng);
        self.cinematic_return = self.screen;
        let dwell = scenes.first().map_or(SCENE_DWELL, |s| s.dwell);
        self.cinematic = Some(Cinematic {
            scenes,
            index: 0,
            dwell,
            transition: None,
            started: Instant::now(),
        });
        self.screen = Screen::Cinematic;
    }
}

/// Step `v` by `delta` in the given direction, clamped to `[min, max]`.
fn bump(v: i64, up: bool, delta: i64, min: i64, max: i64) -> i64 {
    let next = if up { v + delta } else { v - delta };
    next.clamp(min, max)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;
