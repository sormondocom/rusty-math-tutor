//! Deduction Duck — the tutor's mascot.
//!
//! When a student presses **H**, Deduction Duck appears wearing a graduate's
//! mortarboard and acts out a strategy: standing and explaining, *walking* a
//! number line, or *smashing* a number into place-value pieces.  This module
//! owns the duck's sprites (a few poses, each a two-frame loop) and a single
//! clip-safe blitter.

use ratatui::buffer::Buffer;
use ratatui::style::Style;

/// Sprite width in cells.
pub const WIDTH: u16 = 9;
/// Sprite height in cells.
pub const HEIGHT: u16 = 7;

/// Which way (and how) the duck is posed.
#[derive(Copy, Clone)]
pub enum Pose {
    /// Standing, facing left toward the problem (used beside a speech bubble).
    Stand,
    /// Walking, facing right (used to travel a number line).
    WalkRight,
    /// Mid-karate-chop, smashing a number apart.
    Smash,
    /// Facing up, starry-eyed, wings thrown up — gazing at the sky in awe.
    Awe,
}

// Facing left — looks back toward the problem.  Frames differ at the feet.
const STAND_A: [&str; HEIGHT as usize] = [
    "    ___  ",
    "   [___] ",
    "     |   ",
    "    __   ",
    "  <(o )__",
    "   (  __)",
    "   ^^ ^^ ",
];
const STAND_B: [&str; HEIGHT as usize] = [
    "    ___  ",
    "   [___] ",
    "     |   ",
    "    __   ",
    "  <(o )__",
    "   (  __)",
    "  ^^ ^^  ",
];

// Facing right — walks along a number line from left to right.
const WALK_A: [&str; HEIGHT as usize] = [
    "  ___    ",
    " [___]   ",
    "   |     ",
    "   __    ",
    "__( o)>  ",
    "(  __ )  ",
    "  ^^ ^^  ",
];
const WALK_B: [&str; HEIGHT as usize] = [
    "  ___    ",
    " [___]   ",
    "   |     ",
    "   __    ",
    "__( o)>  ",
    "(  __ )  ",
    " ^^ ^^   ",
];

// Karate chop — a raised wing on the wind-up, slammed down on the hit.
const SMASH_A: [&str; HEIGHT as usize] = [
    "  ___  \\ ",
    " [___]  \\",
    "   |    O",
    "   __    ",
    " <(o )   ",
    "  (  )   ",
    "  ^^ ^^  ",
];
const SMASH_B: [&str; HEIGHT as usize] = [
    "  ___    ",
    " [___]   ",
    "   |     ",
    "   __  / ",
    " <(o )O  ",
    "  (  )   ",
    "  ^^ ^^  ",
];

// Gazing up in awe — starry eyes, wings flung up (they rise beside the cap on
// the second frame for a little "wow!" flutter).
const AWE_A: [&str; HEIGHT as usize] = [
    "   ___   ",
    "  [___]  ",
    "    |    ",
    "  \\(**)/ ",
    "   (  )  ",
    "   |  |  ",
    "   ^^^^  ",
];
const AWE_B: [&str; HEIGHT as usize] = [
    " \\ ___ / ",
    "  [___]  ",
    "    |    ",
    "   (°°)  ",
    "   (  )  ",
    "   |  |  ",
    "   ^^^^  ",
];

fn sprite(pose: Pose, frame: u64) -> &'static [&'static str; HEIGHT as usize] {
    let even = frame % 2 == 0;
    match pose {
        Pose::Stand => if even { &STAND_A } else { &STAND_B },
        Pose::WalkRight => if even { &WALK_A } else { &WALK_B },
        Pose::Smash => if even { &SMASH_A } else { &SMASH_B },
        Pose::Awe => if even { &AWE_A } else { &AWE_B },
    }
}

/// Draw the duck in `pose` with its top-left at `(x, y)`, clipped to the
/// buffer.  `frame` selects the two-frame loop; pass a steadily increasing
/// counter.
pub fn draw_pose(buf: &mut Buffer, x: u16, y: u16, pose: Pose, frame: u64, style: Style) {
    draw_pose_i32(buf, x as i32, y as i32, pose, frame, style);
}

/// Like [`draw_pose`] but with signed coordinates, so the duck can be half
/// off-screen (sliding in from an edge) and clip cleanly.
pub fn draw_pose_i32(buf: &mut Buffer, x: i32, y: i32, pose: Pose, frame: u64, style: Style) {
    let area = buf.area;
    for (row, line) in sprite(pose, frame).iter().enumerate() {
        let cy = y + row as i32;
        if cy < area.top() as i32 || cy >= area.bottom() as i32 {
            continue;
        }
        for (col, ch) in line.chars().enumerate() {
            if ch == ' ' {
                continue;
            }
            let cx = x + col as i32;
            if cx < area.left() as i32 || cx >= area.right() as i32 {
                continue;
            }
            buf[(cx as u16, cy as u16)].set_char(ch).set_style(style);
        }
    }
}

