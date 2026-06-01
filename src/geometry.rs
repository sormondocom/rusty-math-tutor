//! Geometry problems: perimeter, area, and volume of hollow (outlined) shapes.
//!
//! Each problem shows an outlined figure with its dimensions labelled on the
//! edges, and asks for one measurement.  Every answer is a whole number, so the
//! generators pick dimensions that keep it tidy (triangle areas use an even
//! base × height, and circles are left for later).
//!
//! The shapes are drawn by [`crate::ui`]; this module owns the maths and the
//! per-problem text.  Polished 3-D glyphs for the volume box are a future step —
//! for now it is a simple wireframe.

use rand::seq::SliceRandom;
use rand::Rng;

/// Which measurement the problem asks for.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Measure {
    Perimeter,
    Area,
    Volume,
}

/// The outlined figure, carrying the dimensions it is labelled with.
#[derive(Copy, Clone, Debug)]
pub enum GeoShape {
    /// A rectangle (or square, when `w == h`).
    Rect { w: i64, h: i64 },
    /// A right triangle with a horizontal `base` and vertical `height`; when
    /// `sides` is set the three side lengths are labelled instead (perimeter).
    Triangle { base: i64, height: i64, sides: Option<[i64; 3]> },
    /// A circle of the given radius (circumference / area are asked in terms
    /// of π so the answer stays a whole number).
    Circle { r: i64 },
    /// A rectangular box (or cube, when all equal): length, width (depth), height.
    Box3 { l: i64, w: i64, h: i64 },
}

pub struct GeometryProblem {
    pub measure: Measure,
    pub shape: GeoShape,
    pub answer: i64,
    /// Unit suffix shown with the answer: "cm", "cm²", or "cm³".
    pub unit: &'static str,
    /// When set, the answer is a coefficient of π (e.g. circle problems), shown
    /// as "{answer}π {unit}".
    pub pi: bool,
    pub hint: Vec<String>,
}

impl GeometryProblem {
    /// The friendly name of the figure, e.g. "rectangle" or "box".
    pub fn shape_word(&self) -> &'static str {
        match self.shape {
            GeoShape::Rect { w, h } if w == h => "square",
            GeoShape::Rect { .. } => "rectangle",
            GeoShape::Triangle { .. } => "triangle",
            GeoShape::Circle { .. } => "circle",
            GeoShape::Box3 { l, w, h } if l == w && w == h => "cube",
            GeoShape::Box3 { .. } => "box",
        }
    }

    pub fn question(&self) -> String {
        let what = match self.measure {
            Measure::Perimeter if matches!(self.shape, GeoShape::Circle { .. }) => "circumference",
            Measure::Perimeter => "perimeter",
            Measure::Area => "area",
            Measure::Volume => "volume",
        };
        let mut q = format!("What is the {} of this {}?", what, self.shape_word());
        if self.pi {
            q.push_str("  (in terms of π)");
        }
        q
    }

    /// The reveal label, e.g. "Answer: 24 cm²" or "Answer: 10π cm".
    pub fn answer_label(&self) -> String {
        let pi = if self.pi { "π" } else { "" };
        format!("Answer: {}{} {}", self.answer, pi, self.unit)
    }

    pub fn is_correct(&self, value: i64) -> bool {
        value == self.answer
    }
}

/// Generate a random geometry problem with a whole-number answer.
pub fn generate(rng: &mut impl Rng) -> GeometryProblem {
    match *[Measure::Perimeter, Measure::Area, Measure::Volume].choose(rng).unwrap() {
        Measure::Perimeter => perimeter(rng),
        Measure::Area => area(rng),
        Measure::Volume => volume(rng),
    }
}

fn perimeter(rng: &mut impl Rng) -> GeometryProblem {
    // Rectangle/square, a triangle (three labelled sides), or a circle.
    match rng.gen_range(0..3) {
        0 => {
            let (w, h) = rect_dims(rng);
            GeometryProblem {
                measure: Measure::Perimeter,
                shape: GeoShape::Rect { w, h },
                answer: 2 * (w + h),
                unit: "cm",
                pi: false,
                hint: vec![
                    "Perimeter is the distance all the way around.".to_string(),
                    if w == h {
                        "A square: add the side four times (or 4 × side).".to_string()
                    } else {
                        "Add every side: 2 × (width + height).".to_string()
                    },
                ],
            }
        }
        1 => {
            let [a, b, c] = triangle_sides(rng);
            GeometryProblem {
                measure: Measure::Perimeter,
                shape: GeoShape::Triangle { base: c, height: 0, sides: Some([a, b, c]) },
                answer: a + b + c,
                unit: "cm",
                pi: false,
                hint: vec![
                    "Perimeter is the distance all the way around.".to_string(),
                    "Add the three side lengths together.".to_string(),
                ],
            }
        }
        _ => {
            // Circumference = 2 × π × radius, answered as the coefficient of π.
            let r = rng.gen_range(2..=9);
            GeometryProblem {
                measure: Measure::Perimeter,
                shape: GeoShape::Circle { r },
                answer: 2 * r,
                unit: "cm",
                pi: true,
                hint: vec![
                    "The circumference is the distance around a circle.".to_string(),
                    "Circumference = 2 × π × radius.".to_string(),
                    "Give the whole number that multiplies π.".to_string(),
                ],
            }
        }
    }
}

fn area(rng: &mut impl Rng) -> GeometryProblem {
    match rng.gen_range(0..3) {
        0 => {
            let (w, h) = rect_dims(rng);
            GeometryProblem {
                measure: Measure::Area,
                shape: GeoShape::Rect { w, h },
                answer: w * h,
                unit: "cm²",
                pi: false,
                hint: vec![
                    "Area is how much surface is inside.".to_string(),
                    if w == h {
                        "A square: side × side.".to_string()
                    } else {
                        "Area = width × height.".to_string()
                    },
                ],
            }
        }
        1 => {
            let base = rng.gen_range(4..=14);
            let mut height = rng.gen_range(3..=10);
            if (base * height) % 2 != 0 {
                height += 1; // keep ½ · base · height a whole number
            }
            GeometryProblem {
                measure: Measure::Area,
                shape: GeoShape::Triangle { base, height, sides: None },
                answer: base * height / 2,
                unit: "cm²",
                pi: false,
                hint: vec![
                    "A triangle is half of a rectangle.".to_string(),
                    "Area = ½ × base × height.".to_string(),
                ],
            }
        }
        _ => {
            // Area = π × radius², answered as the coefficient of π.
            let r = rng.gen_range(2..=9);
            GeometryProblem {
                measure: Measure::Area,
                shape: GeoShape::Circle { r },
                answer: r * r,
                unit: "cm²",
                pi: true,
                hint: vec![
                    "Area is how much surface is inside the circle.".to_string(),
                    "Area = π × radius × radius.".to_string(),
                    "Give the whole number that multiplies π.".to_string(),
                ],
            }
        }
    }
}

fn volume(rng: &mut impl Rng) -> GeometryProblem {
    let cube = rng.gen_bool(0.4);
    let (l, w, h) = if cube {
        let s = rng.gen_range(2..=6);
        (s, s, s)
    } else {
        (rng.gen_range(2..=8), rng.gen_range(2..=8), rng.gen_range(2..=8))
    };
    GeometryProblem {
        measure: Measure::Volume,
        shape: GeoShape::Box3 { l, w, h },
        answer: l * w * h,
        unit: "cm³",
        pi: false,
        hint: vec![
            "Volume is how much space fills the inside.".to_string(),
            if cube {
                "A cube: side × side × side.".to_string()
            } else {
                "Volume = length × width × height.".to_string()
            },
        ],
    }
}

/// A rectangle's width and height; sometimes a square (`w == h`).
fn rect_dims(rng: &mut impl Rng) -> (i64, i64) {
    if rng.gen_bool(0.25) {
        let s = rng.gen_range(2..=15);
        (s, s)
    } else {
        (rng.gen_range(2..=15), rng.gen_range(2..=12))
    }
}

/// Three integer side lengths that form a valid triangle.
fn triangle_sides(rng: &mut impl Rng) -> [i64; 3] {
    loop {
        let a = rng.gen_range(3..=12);
        let b = rng.gen_range(3..=12);
        let c = rng.gen_range(3..=12);
        // Strict triangle inequality on every pair.
        if a + b > c && a + c > b && b + c > a {
            return [a, b, c];
        }
    }
}

/// Parse a typed whole-number answer.
pub fn parse(input: &str) -> Option<i64> {
    input.trim().parse().ok()
}
