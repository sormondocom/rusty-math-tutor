//! Per-student profiles and progress.
//!
//! Every student is unique, so these statistics exist only to build a learner
//! up — total problems solved, a breakdown by operation, and their personal
//! best streak.  There is deliberately **no** ranking, grading, or comparison
//! between students: the numbers only ever go up and only ever celebrate the
//! one child looking at them.
//!
//! The roster persists to `students.json` next to the other settings.

use serde::{Deserialize, Serialize};

use crate::problem::Op;

#[derive(Clone, Serialize, Deserialize)]
pub struct Student {
    pub name: String,
    /// Problems solved, indexed by [`Op::index`].
    #[serde(default)]
    pub solved: [u32; 4],
    /// The student's personal best run of correct answers in a row.
    #[serde(default)]
    pub best_streak: u32,
}

impl Student {
    pub fn new(name: &str) -> Self {
        Student { name: name.to_string(), solved: [0; 4], best_streak: 0 }
    }

    pub fn total(&self) -> u32 {
        self.solved.iter().sum()
    }

    /// Record one correct answer for `op`.
    pub fn record(&mut self, op: Op) {
        self.solved[op.index()] += 1;
    }

    /// Note a streak, keeping only the personal best.
    pub fn note_streak(&mut self, streak: u32) {
        self.best_streak = self.best_streak.max(streak);
    }

    /// Wipe all progress back to zero (teacher records admin).
    pub fn reset(&mut self) {
        self.solved = [0; 4];
        self.best_streak = 0;
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Roster {
    pub students: Vec<Student>,
    pub current: usize,
}

impl Default for Roster {
    fn default() -> Self {
        Roster { students: vec![Student::new("Guest")], current: 0 }
    }
}

impl Roster {
    pub fn load() -> Roster {
        let mut r: Roster = crate::config::data_path("students.json")
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        if r.students.is_empty() {
            r.students.push(Student::new("Guest"));
        }
        r.current = r.current.min(r.students.len() - 1);
        r
    }

    pub fn save(&self) {
        let Some(path) = crate::config::data_path("students.json") else { return };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }

    pub fn current(&self) -> &Student {
        &self.students[self.current]
    }

    pub fn current_mut(&mut self) -> &mut Student {
        &mut self.students[self.current]
    }

    /// Move the selection by `delta` (wrapping), e.g. -1 / +1.
    pub fn cycle(&mut self, delta: i32) {
        let n = self.students.len() as i32;
        self.current = (((self.current as i32 + delta) % n + n) % n) as usize;
    }

    /// Remove a student, keeping the roster non-empty (a Guest is re-created if
    /// the last student is removed) and the selection in range.
    pub fn remove(&mut self, index: usize) {
        if index >= self.students.len() {
            return;
        }
        self.students.remove(index);
        if self.students.is_empty() {
            self.students.push(Student::new("Guest"));
        }
        if self.current >= self.students.len() {
            self.current = self.students.len() - 1;
        }
    }

    /// Add a student (trimmed, non-blank, de-duplicated) and select them.
    pub fn add(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        if let Some(i) = self.students.iter().position(|s| s.name.eq_ignore_ascii_case(name)) {
            self.current = i;
        } else {
            self.students.push(Student::new(name));
            self.current = self.students.len() - 1;
        }
    }
}
