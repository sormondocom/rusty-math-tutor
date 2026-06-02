//! Math problem generation for kindergarten through 8th grade.
//!
//! A [`Problem`] is a single `a op b = answer` fact plus a per-problem accent
//! colour (so every card looks a little different — part of keeping a young
//! student's attention).  [`generate`] scales the operand ranges by grade so
//! the same four operations grow with the learner.
//!
//! Difficulty by grade (operand magnitude / rules):
//! - K–1  : add & subtract within 10 / 20, never negative
//! - 2–3  : add & subtract within 100, times-tables to 10
//! - 4–5  : 2-digit × 1-digit, exact division
//! - 6–8  : larger operands, subtraction may go negative

use rand::seq::SliceRandom;
use rand::Rng;
use crate::color::Color;

use crate::config::GradeRange;

// ---------------------------------------------------------------------------
// Operations
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
}

impl Op {
    /// The glyph used both in the big block font and in plain labels.
    pub fn symbol(self) -> char {
        match self {
            Op::Add => '+',
            Op::Sub => '-',
            Op::Mul => '×',
            Op::Div => '÷',
        }
    }

    /// All four operations, in menu order.
    pub const ALL: [Op; 4] = [Op::Add, Op::Sub, Op::Mul, Op::Div];

    pub fn name(self) -> &'static str {
        match self {
            Op::Add => "Add",
            Op::Sub => "Subtract",
            Op::Mul => "Multiply",
            Op::Div => "Divide",
        }
    }
}

// ---------------------------------------------------------------------------
// A single problem
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Problem {
    pub a: i64,
    pub b: i64,
    pub op: Op,
    pub answer: i64,
    /// Cheerful accent colour chosen per problem for visual variety.
    pub accent: Color,
}

impl Problem {
    /// The prompt as it appears in the big font, e.g. `"12 + 7 ="`.
    pub fn prompt(&self) -> String {
        format!("{} {} {} =", self.a, self.op.symbol(), self.b)
    }
}

/// A small palette of friendly, high-contrast accent colours.
const ACCENTS: [Color; 8] = [
    Color::LightCyan,
    Color::LightGreen,
    Color::LightYellow,
    Color::LightMagenta,
    Color::LightBlue,
    Color::LightRed,
    Color::Cyan,
    Color::Green,
];

// ---------------------------------------------------------------------------
// Generation
// ---------------------------------------------------------------------------

/// Build a fresh random problem within `range`, choosing among the enabled
/// `ops`.  If `ops` is empty, defaults to addition so generation never panics.
pub fn generate(range: GradeRange, ops: &[Op], rng: &mut impl Rng) -> Problem {
    let op = ops.choose(rng).copied().unwrap_or(Op::Add);
    let accent = *ACCENTS.choose(rng).unwrap();

    let (a, b, answer) = match op {
        Op::Add => {
            let a = rng.gen_range(0..=range.add_max.max(1));
            let b = rng.gen_range(0..=range.add_max.max(1));
            (a, b, a + b)
        }
        Op::Sub => {
            let x = rng.gen_range(0..=range.add_max.max(1));
            let y = rng.gen_range(0..=range.add_max.max(1));
            if range.allow_negative {
                (x, y, x - y)
            } else {
                // Order so the answer is never negative for young grades.
                let (hi, lo) = if x >= y { (x, y) } else { (y, x) };
                (hi, lo, hi - lo)
            }
        }
        Op::Mul => {
            let a = rng.gen_range(0..=range.mul_max.max(1));
            let b = rng.gen_range(0..=range.mul_max.max(1));
            (a, b, a * b)
        }
        Op::Div => {
            // Build from the answer so division is always exact.
            let b = rng.gen_range(1..=range.div_max.max(1));
            let q = rng.gen_range(0..=range.div_max.max(1));
            (b * q, b, q)
        }
    };

    Problem { a, b, op, answer, accent }
}
