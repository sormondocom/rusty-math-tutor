//! A tiny 5-row block font for drawing big, kid-friendly equations.
//!
//! Each glyph is 5 rows tall and [`GLYPH_W`] columns wide, stored with `#`
//! for an "on" cell and a space for "off".  [`draw_text`] stamps a string of
//! supported glyphs directly into a ratatui [`Buffer`] using the solid block
//! character `█`, so the same routine works whether we are drawing the live
//! frame or capturing a problem card for a transition.
//!
//! Supported glyphs: `0-9`, `+ - × ÷ = ?` and space.  Unknown characters
//! render as blank cells (so they simply take up space).

use ratatui::buffer::Buffer;
use ratatui::style::Style;

/// Width, in terminal cells, of a single glyph.
pub const GLYPH_W: u16 = 4;
/// Height, in terminal cells, of a single glyph.
pub const GLYPH_H: u16 = 5;
/// Blank columns drawn between adjacent glyphs.
pub const GLYPH_GAP: u16 = 1;

/// The solid cell used to paint an "on" pixel.
const ON: &str = "█";

/// Look up the 5-row bitmap for a character, or `None` if unsupported.
fn glyph(c: char) -> Option<[&'static str; 5]> {
    Some(match c {
        '0' => ["████", "█  █", "█  █", "█  █", "████"],
        '1' => ["  █ ", " ██ ", "  █ ", "  █ ", " ███"],
        '2' => ["████", "   █", "████", "█   ", "████"],
        '3' => ["████", "   █", " ███", "   █", "████"],
        '4' => ["█  █", "█  █", "████", "   █", "   █"],
        '5' => ["████", "█   ", "████", "   █", "████"],
        '6' => ["████", "█   ", "████", "█  █", "████"],
        '7' => ["████", "   █", "  █ ", " █  ", " █  "],
        '8' => ["████", "█  █", "████", "█  █", "████"],
        '9' => ["████", "█  █", "████", "   █", "████"],
        '+' => ["    ", " █  ", "████", " █  ", "    "],
        '-' => ["    ", "    ", "████", "    ", "    "],
        '×' => ["    ", "█  █", " ██ ", "█  █", "    "],
        '÷' => ["  █ ", "    ", "████", "    ", "  █ "],
        '=' => ["    ", "████", "    ", "████", "    "],
        '?' => ["████", "   █", " ███", "    ", "  █ "],
        ' ' => ["    ", "    ", "    ", "    ", "    "],
        _ => return None,
    })
}

/// Total pixel width of `text` when drawn (including inter-glyph gaps).
pub fn text_width(text: &str) -> u16 {
    let n = text.chars().count() as u16;
    if n == 0 {
        0
    } else {
        n * GLYPH_W + (n - 1) * GLYPH_GAP
    }
}

/// Draw `text` in the block font with its top-left at `(x, y)`, clipped to the
/// buffer's bounds.  `style` is applied to every "on" cell.
pub fn draw_text(buf: &mut Buffer, mut x: u16, y: u16, text: &str, style: Style) {
    let area = buf.area;
    for ch in text.chars() {
        let bitmap = glyph(ch).unwrap_or(["    ", "    ", "    ", "    ", "    "]);
        for (row, line) in bitmap.iter().enumerate() {
            let cy = y + row as u16;
            if cy < area.top() || cy >= area.bottom() {
                continue;
            }
            for (col, pixel) in line.chars().enumerate() {
                if pixel != '#' && pixel != '█' {
                    continue;
                }
                let cx = x + col as u16;
                if cx < area.left() || cx >= area.right() {
                    continue;
                }
                buf[(cx, cy)].set_symbol(ON).set_style(style);
            }
        }
        x += GLYPH_W + GLYPH_GAP;
    }
}
