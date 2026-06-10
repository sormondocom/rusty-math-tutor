//! Terminal renderer — module root and glue.
//!
//! This file is pure declarations and re-exports.  Every submodule accesses
//! the shared drawing toolkit through `use super::*;`.

mod cards;
mod cinematic;
mod help;
mod render;
mod screens;
mod session;
mod toolkit;
mod transitions;

pub use cinematic::render_scene;
pub use cards::{draw_duck_gag, render_card, render_geometry_card, render_shape_card, render_unit_card};
pub use cinematic::draw_cinematic;
pub use help::draw_help_overlay;
pub use render::draw;
pub use screens::{draw_challenge_end, draw_help, draw_menu, draw_settings, draw_startup, draw_stats, draw_teacher, draw_time};
pub use session::{card_area, draw_experiment, draw_session};
pub use toolkit::*;
pub use transitions::Transitions;
