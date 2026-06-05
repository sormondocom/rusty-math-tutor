//! Persisted configuration: the per-grade number ranges (so a teacher or
//! parent can tune difficulty) and the preferred problem layout.
//!
//! Settings live in a small JSON file under the user's config directory and
//! are loaded at startup / saved whenever the Settings screen is left.  All
//! I/O is best-effort: a missing or unreadable file simply falls back to the
//! built-in defaults so the app always starts.

use serde::{Deserialize, Serialize};

use crate::storage::Storage;

/// Which rendering frontend drives the app.
///
/// The domain layer ([`crate::app`], [`crate::problem`], [`crate::strategy`],
/// [`crate::transition`] as data) is deliberately renderer-agnostic, so a
/// second frontend can sit behind the same core.  Today only [`Low`] (the
/// terminal-cell renderer in [`crate::ui`]) is implemented; [`Cpu`] is reserved
/// for a future software-rendered 2D/3D frontend — CPU only, never a GPU
/// requirement, since this is aimed at helping children, not selling hardware.
///
/// [`Low`]: GraphicsMode::Low
/// [`Cpu`]: GraphicsMode::Cpu
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphicsMode {
    /// Low-graphics console renderer (the current, fully-supported mode).
    #[default]
    Low,
    /// CPU-driven graphical renderer (shelved — "coming soon").
    Cpu,
}

impl GraphicsMode {
    pub fn name(self) -> &'static str {
        match self {
            GraphicsMode::Low => "Low Graphics (console)",
            GraphicsMode::Cpu if cfg!(feature = "gui") => "CPU Graphics (window)",
            GraphicsMode::Cpu => "CPU Graphics (coming soon)",
        }
    }

    /// Whether this mode can actually run in this build.  CPU graphics needs the
    /// `gui` Cargo feature (winit + softbuffer + tiny-skia).
    pub fn available(self) -> bool {
        match self {
            GraphicsMode::Low => true,
            GraphicsMode::Cpu => cfg!(feature = "gui"),
        }
    }
}

/// Visual theme for the CPU-graphics window (its palette / "look").  The console
/// renderer has its own fixed styling and ignores this.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    /// The standard dark-slate palette.
    #[default]
    Default,
    /// White chalk on a black board.
    Blackboard,
    /// White chalk on a dark-green board.
    Chalkboard,
}

impl Theme {
    /// Every theme, in cycle order.
    pub const ALL: [Theme; 3] = [Theme::Default, Theme::Blackboard, Theme::Chalkboard];

    pub fn name(self) -> &'static str {
        match self {
            Theme::Default => "Default",
            Theme::Blackboard => "Blackboard",
            Theme::Chalkboard => "Chalkboard",
        }
    }

    /// The next theme `dir` steps along (wrapping), for the Settings cycler.
    pub fn cycled(self, dir: i32) -> Theme {
        let n = Self::ALL.len() as i32;
        let i = Self::ALL.iter().position(|&t| t == self).unwrap_or(0) as i32;
        Self::ALL[(((i + dir) % n + n) % n) as usize]
    }
}

/// How a problem is presented on the card.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Layout {
    /// `12 + 7 = 19` on one line.
    Horizontal,
    /// Stacked column form (the way it's written out by hand).
    Vertical,
}

impl Layout {
    pub fn toggled(self) -> Layout {
        match self {
            Layout::Horizontal => Layout::Vertical,
            Layout::Vertical => Layout::Horizontal,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Layout::Horizontal => "Horizontal",
            Layout::Vertical => "Vertical",
        }
    }
}

/// Difficulty knobs for a single grade.
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct GradeRange {
    /// Largest operand used for addition and subtraction.
    pub add_max: i64,
    /// Largest factor used for multiplication.
    pub mul_max: i64,
    /// Largest divisor / quotient used for division.
    pub div_max: i64,
    /// Whether subtraction may produce a negative answer.
    pub allow_negative: bool,
}

/// The number of supported grades: K (0) through 8.
pub const GRADES: usize = 9;

/// A salted hash of the teacher's password.  This is a casual gate to keep
/// young students out of the teacher tools — not real cryptography — so a
/// deterministic, dependency-free hash is plenty.
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct TeacherAuth {
    salt: u64,
    hash: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    /// One entry per grade, indexed 0 (K) … 8.
    pub grades: Vec<GradeRange>,
    pub layout: Layout,
    /// Selected rendering frontend; defaults for older config files.
    #[serde(default)]
    pub graphics: GraphicsMode,
    /// Teacher password gate (set on first visit to the Teacher Area).
    #[serde(default)]
    pub teacher: Option<TeacherAuth>,
    /// Measurement locality for the Units of Measure section.
    #[serde(default)]
    pub locality: crate::units::Locality,
    /// Visual theme for the CPU-graphics window.
    #[serde(default)]
    pub theme: Theme,
}

impl Default for Config {
    fn default() -> Self {
        // Deliberately moderate ranges — the previous build let 8th grade run
        // up to 9999 × 99, which is far too high.  These stay teachable and
        // can be tuned per grade in the Settings screen.
        let grades = vec![
            GradeRange { add_max: 10, mul_max: 5, div_max: 5, allow_negative: false }, // K
            GradeRange { add_max: 20, mul_max: 5, div_max: 5, allow_negative: false }, // 1
            GradeRange { add_max: 50, mul_max: 9, div_max: 9, allow_negative: false }, // 2
            GradeRange { add_max: 100, mul_max: 10, div_max: 10, allow_negative: false }, // 3
            GradeRange { add_max: 200, mul_max: 12, div_max: 12, allow_negative: false }, // 4
            GradeRange { add_max: 500, mul_max: 15, div_max: 12, allow_negative: false }, // 5
            GradeRange { add_max: 1000, mul_max: 20, div_max: 15, allow_negative: true }, // 6
            GradeRange { add_max: 2000, mul_max: 25, div_max: 20, allow_negative: true }, // 7
            GradeRange { add_max: 5000, mul_max: 30, div_max: 25, allow_negative: true }, // 8
        ];
        Config {
            grades,
            layout: Layout::Horizontal,
            graphics: GraphicsMode::Low,
            teacher: None,
            locality: crate::units::Locality::UnitedStates,
            theme: Theme::Default,
        }
    }
}

/// Deterministic salted hash of a password (SipHash with fixed keys).
fn hash_password(salt: u64, password: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    salt.hash(&mut h);
    password.trim().hash(&mut h);
    h.finish()
}

impl Config {
    /// The range for `grade`, clamped to the valid range as a safety net.
    pub fn range(&self, grade: u8) -> GradeRange {
        let i = (grade as usize).min(self.grades.len().saturating_sub(1));
        self.grades[i]
    }

    pub fn range_mut(&mut self, grade: u8) -> &mut GradeRange {
        let i = (grade as usize).min(self.grades.len().saturating_sub(1));
        &mut self.grades[i]
    }

    /// Load config from `storage`, falling back to defaults on any problem.  A
    /// short/old blob (fewer grades than expected) is topped up from defaults.
    pub fn load(storage: &dyn Storage) -> Config {
        let mut cfg = storage
            .load("config.json")
            .and_then(|s| serde_json::from_str::<Config>(&s).ok())
            .unwrap_or_default();
        if cfg.grades.len() < GRADES {
            let defaults = Config::default();
            cfg.grades.extend_from_slice(&defaults.grades[cfg.grades.len()..]);
        }
        cfg
    }

    /// Whether a teacher password has been set yet.
    pub fn has_teacher_password(&self) -> bool {
        self.teacher.is_some()
    }

    /// Set (or replace) the teacher password.  Blank passwords are ignored.
    pub fn set_teacher_password(&mut self, password: &str) {
        if password.trim().is_empty() {
            return;
        }
        let salt = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x1234_5678)
            | 1;
        self.teacher = Some(TeacherAuth { salt, hash: hash_password(salt, password) });
    }

    /// Check a password against the stored hash.
    pub fn verify_teacher_password(&self, password: &str) -> bool {
        self.teacher.as_ref().is_some_and(|t| hash_password(t.salt, password) == t.hash)
    }

    /// Best-effort save; errors are ignored so they never interrupt a lesson.
    pub fn save(&self, storage: &dyn Storage) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            storage.save("config.json", &json);
        }
    }
}
