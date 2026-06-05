//! Deduction Duck — the mascot sprite rendered as monospaced pixels in the GUI.
//!
//! The ASCII art is identical to the terminal version; here each character cell
//! is rendered as a block of logical pixels via the scene's text layer so the
//! duck scales naturally with the rest of the UI.

use tiny_skia::Pixmap;

use super::*;

#[derive(Copy, Clone)]
#[allow(dead_code)] // WalkRight / Smash / Awe land with strategy visuals + cinematic
pub enum DuckPose {
    Stand,
    WalkRight,
    Smash,
    Awe,
}

pub const DUCK_H: usize = 7;

const STAND_A: [&str; DUCK_H] = ["    ___  ", "   [___] ", "     |   ", "    __   ", "  <(o )__", "   (  __)", "   ^^ ^^ "];
const STAND_B: [&str; DUCK_H] = ["    ___  ", "   [___] ", "     |   ", "    __   ", "  <(o )__", "   (  __)", "  ^^ ^^  "];
const WALK_A:  [&str; DUCK_H] = ["  ___    ", " [___]   ", "   |     ", "   __    ", "__( o)>  ", "(  __ )  ", "  ^^ ^^  "];
const WALK_B:  [&str; DUCK_H] = ["  ___    ", " [___]   ", "   |     ", "   __    ", "__( o)>  ", "(  __ )  ", " ^^ ^^   "];
const SMASH_A: [&str; DUCK_H] = ["  ___  \\ ", " [___]  \\", "   |    O", "   __    ", " <(o )   ", "  (  )   ", "  ^^ ^^  "];
const SMASH_B: [&str; DUCK_H] = ["  ___    ", " [___]   ", "   |     ", "   __  / ", " <(o )O  ", "  (  )   ", "  ^^ ^^  "];
const AWE_A:   [&str; DUCK_H] = ["   ___   ", "  [___]  ", "    |    ", "  \\(**)/ ", "   (  )  ", "   |  |  ", "   ^^^^  "];
const AWE_B:   [&str; DUCK_H] = [" \\ ___ / ", "  [___]  ", "    |    ", "   (°°)  ", "   (  )  ", "   |  |  ", "   ^^^^  "];

pub fn duck_sprite(pose: DuckPose, frame: u64) -> &'static [&'static str; DUCK_H] {
    let even = frame.is_multiple_of(2);
    match pose {
        DuckPose::Stand    => if even { &STAND_A } else { &STAND_B },
        DuckPose::WalkRight => if even { &WALK_A  } else { &WALK_B  },
        DuckPose::Smash    => if even { &SMASH_A } else { &SMASH_B },
        DuckPose::Awe      => if even { &AWE_A   } else { &AWE_B   },
    }
}

/// Draw the duck in `pose` with its top-left at `(x, y)`; `cell` is one
/// character cell's width in logical px (height is `cell * 1.25`).
pub fn draw_duck(pm: &mut Pixmap, x: f32, y: f32, pose: DuckPose, frame: u64, cell: f32, color: Rgb) {
    let scale = cell / 10.0;
    let ch = cell * 1.25;
    for (row, line) in duck_sprite(pose, frame).iter().enumerate() {
        for (col, c) in line.chars().enumerate() {
            if c == ' ' {
                continue;
            }
            let s = c.to_string();
            let gx = x + col as f32 * cell + (cell - text_width(&s, scale)) / 2.0;
            text(pm, gx, y + row as f32 * ch, scale, &s, color);
        }
    }
}
