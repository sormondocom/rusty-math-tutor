//! Pixel renderer for the GUI frontend.
//!
//! This module is pure glue: it declares all submodules and re-exports their
//! public symbols so every submodule can reach the shared toolkit through a
//! single `use super::*;`.

mod board;
mod card;
mod cinematic;
mod duck;
mod figures;
mod help;
mod paint;
mod render;
mod screens;
mod sprites;
mod text;
mod transition;

#[cfg(test)]
mod tests;

pub use render::render;

// Everything below is re-exported so submodules see it through `use super::*;`.
pub use board::*;
pub use card::{ans_color, draw_card, draw_session, glyph_h, wrap_words};
pub use cinematic::*;
pub use duck::*;
pub use figures::*;
pub use help::*;
pub use paint::*;
pub use screens::*;
pub use sprites::*;
pub use text::*;
pub use transition::*;
