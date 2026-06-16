//! A learnable **section** of the tutor, unified as a [`Topic`].
//!
//! Each topic bundles the components that travel with a section: its display
//! name, its progress counter ([`crate::student`]), its "Why?" motivation and
//! teacher-editable anecdotes ([`crate::motivation`]).  Adding a section means
//! adding a variant here and filling in its data where the compiler points.

use crate::problem::Op;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Topic {
    Add,
    Sub,
    Mul,
    Div,
    Units,
    Fractions,
    Percentages,
    Geometry,
    Graphing,
}

impl Topic {
    /// Every topic, in menu / progress order.
    pub const ALL: [Topic; 9] = [
        Topic::Add,
        Topic::Sub,
        Topic::Mul,
        Topic::Div,
        Topic::Units,
        Topic::Fractions,
        Topic::Percentages,
        Topic::Geometry,
        Topic::Graphing,
    ];

    pub fn index(self) -> usize {
        match self {
            Topic::Add => 0,
            Topic::Sub => 1,
            Topic::Mul => 2,
            Topic::Div => 3,
            Topic::Units => 4,
            Topic::Fractions => 5,
            Topic::Percentages => 6,
            Topic::Geometry => 7,
            Topic::Graphing => 8,
        }
    }

    /// Full name, e.g. for the "Why learn …?" heading.
    pub fn name(self) -> &'static str {
        match self {
            Topic::Add => "Addition",
            Topic::Sub => "Subtraction",
            Topic::Mul => "Multiplication",
            Topic::Div => "Division",
            Topic::Units => "Units of Measure",
            Topic::Fractions => "Fractions",
            Topic::Percentages => "Percentages",
            Topic::Geometry => "Geometry",
            Topic::Graphing => "Graphing",
        }
    }

    /// Short label for tables and bars.
    pub fn short(self) -> &'static str {
        match self {
            Topic::Add => "Add",
            Topic::Sub => "Subtract",
            Topic::Mul => "Multiply",
            Topic::Div => "Divide",
            Topic::Units => "Units",
            Topic::Fractions => "Fractions",
            Topic::Percentages => "Percents",
            Topic::Geometry => "Geometry",
            Topic::Graphing => "Graphing",
        }
    }

    pub fn from_op(op: Op) -> Topic {
        match op {
            Op::Add => Topic::Add,
            Op::Sub => Topic::Sub,
            Op::Mul => Topic::Mul,
            Op::Div => Topic::Div,
        }
    }
}
