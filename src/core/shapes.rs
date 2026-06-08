//! Frontend-neutral shape taxonomy — the data the core owns.
//!
//! The `Shape` enum and its renderer-agnostic methods.  No ratatui, no
//! tiny-skia: this file compiles to every target including wasm32.
//!
//! The terminal frontend (`frontends/terminal/shapes.rs`) re-defines the same
//! enum and adds `region_rect()` (which takes a `ratatui::Rect`) and all the
//! drawing helpers.  The GUI frontend has its own drawing code in
//! `frontends/gui/scene/card.rs`.
//!
//! Used by `lib.rs` (`[lib]` WASM crate); `main.rs` still points
//! `crate::shapes` at the terminal file which re-exports the full surface.

/// A figure divided into equal regions, used for fraction and percentage
/// problems.  Each region can be shaded independently and materialises one
/// at a time as the student first sees the problem.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Shape {
    /// A single row of `segments` cells (bar / divided rectangle).
    Bar { segments: u16 },
    /// A `rows` × `cols` grid of cells.
    Grid { rows: u16, cols: u16 },
    /// A pie circle cut into `slices` equal wedges.
    Circle { slices: u16 },
    /// An upward triangle cut into `strips` horizontal layers.
    Triangle { strips: u16 },
}

impl Shape {
    /// Total number of regions.
    pub fn regions(&self) -> u16 {
        match self {
            Shape::Bar { segments }     => *segments,
            Shape::Grid { rows, cols }  => rows * cols,
            Shape::Circle { slices }    => *slices,
            Shape::Triangle { strips }  => *strips,
        }
    }

    /// Whether this shape reads as a square (for nicer wording).
    pub fn is_square(&self) -> bool {
        matches!(self, Shape::Grid { rows, cols } if rows == cols)
    }
}
