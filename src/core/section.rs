//! Sections, unified at runtime as a single [`Active`] problem.
//!
//! Each "section" of the tutor (arithmetic, units, fractions, percentages…)
//! produces a problem of its own concrete type, but they all share one set of
//! behaviours: how an answer is *checked*, what the answer *reads* as when the
//! duck reveals it, which keys the answer field *accepts*, whether the section
//! carries step-by-step *strategies*, and — crucially — which [`Topic`] it
//! *scores* into (which in turn drives progress and the "Why?" panel).
//!
//! `Active` is the one place those behaviours live, so adding a section is a
//! matter of adding a variant and letting the compiler point at every hole.

use crate::fraction::{FractionProblem, Mode};
use crate::geometry::GeometryProblem;
use crate::problem::Problem;
use crate::topic::Topic;
use crate::units::UnitProblem;

/// The problem currently on screen, tagged by the section that made it.
pub enum Active {
    /// One of the four arithmetic operations.
    Arith(Problem),
    /// A Units of Measure conversion.
    Unit(UnitProblem),
    /// A visual shape problem — fractions and percentages share one type,
    /// distinguished by its [`Mode`].
    Shape(FractionProblem),
    /// A geometry problem — perimeter, area, or volume of an outlined shape.
    Geo(GeometryProblem),
}

impl Active {
    /// Which section (and therefore progress counter + Why? list) this scores
    /// into.
    pub fn topic(&self) -> Topic {
        match self {
            Active::Arith(p) => Topic::from_op(p.op),
            Active::Unit(_) => Topic::Units,
            Active::Shape(s) => match s.mode {
                Mode::Fraction => Topic::Fractions,
                Mode::Percent => Topic::Percentages,
            },
            Active::Geo(_) => Topic::Geometry,
        }
    }

    /// Whether the typed `input` is a correct answer for this problem.
    pub fn check(&self, input: &str) -> bool {
        match self {
            Active::Arith(p) => input.parse::<i64>().ok() == Some(p.answer),
            Active::Unit(u) => input.parse::<i64>().ok() == Some(u.answer),
            Active::Shape(s) => match s.mode {
                Mode::Fraction => {
                    crate::fraction::parse(input).is_some_and(|(n, d)| s.is_correct(n, d))
                }
                Mode::Percent => input.parse::<i64>().ok().is_some_and(|v| s.is_percent_correct(v)),
            },
            Active::Geo(g) => crate::geometry::parse(input).is_some_and(|v| g.is_correct(v)),
        }
    }

    /// The canonical correct answer as a string — a test helper for typing a
    /// known-good answer.  Fractions read as `"a/b"`, percentages as a number.
    #[cfg(test)]
    pub fn correct_answer_string(&self) -> String {
        match self {
            Active::Arith(p) => p.answer.to_string(),
            Active::Unit(u) => u.answer.to_string(),
            Active::Shape(s) => match s.mode {
                Mode::Fraction => format!("{}/{}", s.shaded, s.total),
                Mode::Percent => s.percent_answer().to_string(),
            },
            Active::Geo(g) => g.answer.to_string(),
        }
    }

    /// Whether the answer field accepts a `/` (only "a/b" fraction answers).
    pub fn accepts_slash(&self) -> bool {
        matches!(self, Active::Shape(s) if s.mode == Mode::Fraction)
    }

    /// Whether the answer field accepts a leading `-` (only signed arithmetic).
    pub fn accepts_minus(&self) -> bool {
        matches!(self, Active::Arith(_))
    }

    /// Arithmetic problems carry Deduction Duck strategies and the live layout
    /// toggle; the other sections just take an answer plus a how-to hint.
    pub fn has_strategies(&self) -> bool {
        matches!(self, Active::Arith(_))
    }

    /// A shape problem materialises its figure over a few ticks.
    pub fn materializes(&self) -> bool {
        matches!(self, Active::Shape(_))
    }

    /// How many pieces the shape splits into (1 for non-shape problems), used to
    /// pace the materialise animation.
    pub fn shape_regions(&self) -> u32 {
        match self {
            Active::Shape(s) => s.total.max(1) as u32,
            _ => 1,
        }
    }
}
