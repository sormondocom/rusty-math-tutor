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

use crate::topic::Topic;

fn default_reveal_lock() -> u32 {
    3
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Student {
    pub name: String,
    /// Arithmetic problems solved, indexed by [`Topic::index`] (Add..Div = 0..3).
    #[serde(default)]
    pub solved: [u32; 4],
    /// Measurement (Units of Measure) problems solved.
    #[serde(default)]
    pub units_solved: u32,
    /// Fraction problems solved.
    #[serde(default)]
    pub fractions_solved: u32,
    /// Percentage problems solved.
    #[serde(default)]
    pub percents_solved: u32,
    /// Geometry problems solved.
    #[serde(default)]
    pub geometry_solved: u32,
    /// The student's personal best run of correct answers in a row.
    #[serde(default)]
    pub best_streak: u32,
    /// How many answer reveals before the duck puts a peek on cooldown.
    /// `0` means reveals are never locked.  Set by the teacher per student.
    #[serde(default = "default_reveal_lock")]
    pub reveal_lock: u32,
}

impl Student {
    pub fn new(name: &str) -> Self {
        Student {
            name: name.to_string(),
            solved: [0; 4],
            units_solved: 0,
            fractions_solved: 0,
            percents_solved: 0,
            geometry_solved: 0,
            best_streak: 0,
            reveal_lock: default_reveal_lock(),
        }
    }

    /// Arithmetic problems solved (sum of the four operations).
    pub fn total(&self) -> u32 {
        self.solved.iter().sum()
    }

    /// Every kind of problem solved — across all topics.
    pub fn grand_total(&self) -> u32 {
        self.total() + self.units_solved + self.fractions_solved + self.percents_solved + self.geometry_solved
    }

    /// How many problems this student has solved for a given [`Topic`].
    pub fn solved_for(&self, topic: Topic) -> u32 {
        match topic {
            Topic::Add => self.solved[0],
            Topic::Sub => self.solved[1],
            Topic::Mul => self.solved[2],
            Topic::Div => self.solved[3],
            Topic::Units => self.units_solved,
            Topic::Fractions => self.fractions_solved,
            Topic::Percentages => self.percents_solved,
            Topic::Geometry => self.geometry_solved,
        }
    }

    /// Record one correct answer for `topic`.
    pub fn record_topic(&mut self, topic: Topic) {
        match topic {
            Topic::Add | Topic::Sub | Topic::Mul | Topic::Div => self.solved[topic.index()] += 1,
            Topic::Units => self.units_solved += 1,
            Topic::Fractions => self.fractions_solved += 1,
            Topic::Percentages => self.percents_solved += 1,
            Topic::Geometry => self.geometry_solved += 1,
        }
    }

    /// Zero just one topic's count (teacher records admin).
    pub fn reset_topic(&mut self, topic: Topic) {
        match topic {
            Topic::Add | Topic::Sub | Topic::Mul | Topic::Div => self.solved[topic.index()] = 0,
            Topic::Units => self.units_solved = 0,
            Topic::Fractions => self.fractions_solved = 0,
            Topic::Percentages => self.percents_solved = 0,
            Topic::Geometry => self.geometry_solved = 0,
        }
    }

    /// Note a streak, keeping only the personal best.
    pub fn note_streak(&mut self, streak: u32) {
        self.best_streak = self.best_streak.max(streak);
    }

    /// Wipe all progress back to zero (teacher records admin).
    pub fn reset(&mut self) {
        self.solved = [0; 4];
        self.units_solved = 0;
        self.fractions_solved = 0;
        self.percents_solved = 0;
        self.geometry_solved = 0;
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
    pub fn load(storage: &dyn crate::storage::Storage) -> Roster {
        let mut r: Roster = storage
            .load("students.json")
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        if r.students.is_empty() {
            r.students.push(Student::new("Guest"));
        }
        r.current = r.current.min(r.students.len() - 1);
        r
    }

    pub fn save(&self, storage: &dyn crate::storage::Storage) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            storage.save("students.json", &json);
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
