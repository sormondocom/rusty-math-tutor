//! Visual fraction problems built on the reusable [`crate::shapes`] glyphs.
//!
//! A shape is split into equal pieces, some shaded one colour and the rest
//! another; the student reads off the fraction that is shaded (e.g. a square in
//! four pieces with two green → `2/4`).  Answers are checked for *equivalence*,
//! so `1/2` is accepted too.

use rand::seq::SliceRandom;
use rand::Rng;
use ratatui::style::Color;

use crate::shapes::Shape;

/// How the shaded amount is read off — as a fraction (`2/4`) or a percent
/// (`50`).  Percentages reuse the very same shapes; only the question, the
/// accepted answer, and the hint differ.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Mode {
    Fraction,
    Percent,
}

pub struct FractionProblem {
    pub mode: Mode,
    pub total: i64,
    pub shaded: i64,
    pub shape: Shape,
    pub shaded_color: Color,
    pub other_color: Color,
    /// Colour word used in the question ("green", "red", …).
    pub color_name: &'static str,
    pub hint: Vec<String>,
}

/// (shaded colour, other colour, colour word) — the word names the *shaded*
/// colour, since that's the one the question asks about and the answer counts.
const PALETTE: &[(Color, Color, &str)] = &[
    (Color::LightGreen, Color::LightMagenta, "green"),
    (Color::LightRed, Color::LightCyan, "red"),
    (Color::LightYellow, Color::LightBlue, "yellow"),
    (Color::LightMagenta, Color::LightGreen, "purple"),
    (Color::LightBlue, Color::LightYellow, "blue"),
];

pub fn generate(rng: &mut impl Rng) -> FractionProblem {
    build(Mode::Fraction, *[2i64, 3, 4, 5, 6, 8, 9].choose(rng).unwrap(), rng)
}

/// A percentage problem: the very same shapes, but the total always divides
/// 100 so the shaded share is a whole percent.
pub fn generate_percent(rng: &mut impl Rng) -> FractionProblem {
    build(Mode::Percent, *[2i64, 4, 5, 10].choose(rng).unwrap(), rng)
}

fn build(mode: Mode, total: i64, rng: &mut impl Rng) -> FractionProblem {
    let shaded = rng.gen_range(1..total);
    let n = total as u16;
    // A grid only when the count factors nicely; circle / triangle / bar suit
    // any number.
    let grid = match total {
        4 => Some(Shape::Grid { rows: 2, cols: 2 }),
        6 => Some(Shape::Grid { rows: 2, cols: 3 }),
        8 => Some(Shape::Grid { rows: 2, cols: 4 }),
        9 => Some(Shape::Grid { rows: 3, cols: 3 }),
        10 => Some(Shape::Grid { rows: 2, cols: 5 }),
        _ => None,
    };
    let shape = match rng.gen_range(0..4) {
        0 => Shape::Circle { slices: n },
        1 => Shape::Triangle { strips: n },
        2 => grid.unwrap_or(Shape::Bar { segments: n }),
        _ => Shape::Bar { segments: n },
    };
    let (shaded_color, other_color, color_name) = *PALETTE.choose(rng).unwrap();
    let hint = match mode {
        Mode::Fraction => vec![
            format!("The top number is how many pieces are {}.", color_name),
            "The bottom number is the total number of pieces.".to_string(),
            "Count them up and write top over bottom!".to_string(),
        ],
        Mode::Percent => vec![
            format!("First make the fraction that is {}: pieces over total.", color_name),
            format!("Each of the {} pieces is worth {}%.", total, 100 / total),
            "Percent means 'out of 100' — scale the fraction up to 100.".to_string(),
        ],
    };
    FractionProblem { mode, total, shaded, shape, shaded_color, other_color, color_name, hint }
}

impl FractionProblem {
    /// The word for the figure, for friendlier questions.
    pub fn shape_word(&self) -> &'static str {
        match self.shape {
            Shape::Bar { .. } => "bar",
            Shape::Circle { .. } => "circle",
            Shape::Triangle { .. } => "triangle",
            s if s.is_square() => "square",
            _ => "rectangle",
        }
    }

    pub fn question(&self) -> String {
        match self.mode {
            Mode::Fraction => format!("What fraction of the {} is {}?", self.shape_word(), self.color_name),
            Mode::Percent => format!("What percent of the {} is {}?", self.shape_word(), self.color_name),
        }
    }

    /// The shaded share as a whole percent (only meaningful in [`Mode::Percent`],
    /// where `total` always divides 100).
    pub fn percent_answer(&self) -> i64 {
        self.shaded * 100 / self.total
    }

    /// Whether `value` is the right percent (for [`Mode::Percent`]).
    pub fn is_percent_correct(&self, value: i64) -> bool {
        value == self.percent_answer()
    }

    /// The answer string shown when the duck reveals it.
    pub fn answer_label(&self) -> String {
        match self.mode {
            Mode::Fraction => format!("Answer: {}/{}", self.shaded, self.total),
            Mode::Percent => format!("Answer: {}%", self.percent_answer()),
        }
    }

    /// The fill colour of region `i`: the shaded pieces come first.
    pub fn color_of(&self, i: u16) -> Color {
        if (i as i64) < self.shaded {
            self.shaded_color
        } else {
            self.other_color
        }
    }

    /// Whether `num/den` equals the shaded fraction (any equivalent form).
    pub fn is_correct(&self, num: i64, den: i64) -> bool {
        den > 0 && num >= 0 && num * self.total == den * self.shaded
    }
}

/// Parse a typed `"a/b"` answer into `(numerator, denominator)`.
pub fn parse(input: &str) -> Option<(i64, i64)> {
    let (a, b) = input.split_once('/')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}
