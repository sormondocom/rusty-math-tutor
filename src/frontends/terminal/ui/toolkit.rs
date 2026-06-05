//! Low-level drawing helpers shared across all `ui` submodules.
//!
//! Every submodule accesses these via `use super::*;`.  None of them depend on
//! app state — they're pure geometry and text utilities that work directly on
//! ratatui [`Buffer`] and [`Rect`] values.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

// ---------------------------------------------------------------------------
// Shared colour constants
// ---------------------------------------------------------------------------

/// Deduction Duck's feathers.
pub const DUCK_COLOR: Color = Color::Rgb(255, 214, 64);
/// Background tint of the help stage panel.
pub const STAGE_BG: Color = Color::Rgb(28, 30, 44);
/// Background colour shared by every screen.
pub const BG: Color = Color::Rgb(16, 18, 28);

// ---------------------------------------------------------------------------
// Colour mapping
// ---------------------------------------------------------------------------

/// Map a domain [`crate::color::Color`] to the ratatui colour the terminal
/// paints with.  The core describes colours renderer-neutrally; this is the
/// terminal frontend's translation point.
pub fn rat(c: crate::color::Color) -> Color {
    use crate::color::Color as C;
    match c {
        C::LightCyan => Color::LightCyan,
        C::LightGreen => Color::LightGreen,
        C::LightYellow => Color::LightYellow,
        C::LightMagenta => Color::LightMagenta,
        C::LightBlue => Color::LightBlue,
        C::LightRed => Color::LightRed,
        C::Cyan => Color::Cyan,
        C::Green => Color::Green,
        C::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

/// Place a short string at floating-point coordinates, clipped to the buffer.
pub fn put_float(buf: &mut Buffer, x: f32, y: f32, s: &str, style: Style) {
    if !x.is_finite() || !y.is_finite() || x < 0.0 || y < 0.0 {
        return;
    }
    put_str(buf, x.round() as u16, y.round() as u16, s, style);
}

/// Greedy word-wrap of `s` into lines no wider than `width` characters.
pub fn wrap_text(s: &str, width: u16) -> Vec<String> {
    wrap_prefixed("", "", s, width)
}

/// Hard character wrap: break `s` every `width` columns, preserving spaces.
/// Used for text-input echoes where the caret must always stay in view.
pub fn wrap_chars(s: &str, width: u16) -> Vec<String> {
    let width = width.max(1) as usize;
    let chars: Vec<char> = s.chars().collect();
    if chars.is_empty() {
        return vec![String::new()];
    }
    chars.chunks(width).map(|c| c.iter().collect()).collect()
}

/// Word-wrap `text` so that, once `first` is prepended to the first line and
/// `cont` to every continuation line, **no produced line exceeds `width`
/// columns**.  The prefixes are counted against the budget, which is the part
/// the plain wrapper missed.  A word longer than the budget still gets its own
/// line (it can only be clipped, never silently dropped).
pub fn wrap_prefixed(first: &str, cont: &str, text: &str, width: u16) -> Vec<String> {
    let first_budget = (width as usize).saturating_sub(first.chars().count()).max(1);
    let cont_budget = (width as usize).saturating_sub(cont.chars().count()).max(1);

    let mut bodies: Vec<String> = Vec::new();
    let mut line = String::new();
    let mut budget = first_budget;
    for word in text.split_whitespace() {
        if line.is_empty() {
            line = word.to_string();
        } else if line.chars().count() + 1 + word.chars().count() <= budget {
            line.push(' ');
            line.push_str(word);
        } else {
            bodies.push(std::mem::take(&mut line));
            line = word.to_string();
            budget = cont_budget; // every line after the first uses cont width
        }
    }
    if !line.is_empty() {
        bodies.push(line);
    }
    if bodies.is_empty() {
        bodies.push(String::new());
    }

    bodies
        .into_iter()
        .enumerate()
        .map(|(i, body)| if i == 0 { format!("{}{}", first, body) } else { format!("{}{}", cont, body) })
        .collect()
}

/// Draw a string only when its start cell is inside the buffer.  Ratatui's
/// `set_string` clips horizontally but panics on an out-of-range `y`, so this
/// guard keeps every label safe on tiny terminals.
pub fn put_str(b: &mut Buffer, x: u16, y: u16, s: impl AsRef<str>, style: Style) {
    let area = b.area;
    if x >= area.right() || y >= area.bottom() || x < area.left() || y < area.top() {
        return;
    }
    b.set_string(x, y, s.as_ref(), style);
}

/// Left x so that a span of `width` is centred within `area`.
pub fn center(area: Rect, width: u16) -> u16 {
    area.left() + area.width.saturating_sub(width) / 2
}

/// Compute a scroll window for a list of `total` one-line items shown between
/// rows `top` and `bottom` (exclusive), keeping `sel` visible.  Returns
/// `(first_visible_index, visible_count)`.
pub fn scroll_window(total: usize, sel: usize, top: u16, bottom: u16) -> (usize, usize) {
    let visible = bottom.saturating_sub(top) as usize;
    if visible == 0 {
        return (0, 0);
    }
    if total <= visible {
        return (0, total);
    }
    let mut scroll = (sel + 1).saturating_sub(visible).min(total - visible);
    if sel < scroll {
        scroll = sel;
    }
    (scroll, visible)
}

/// Truncate `s` to at most `width` characters (by char, ellipsis-free).
pub fn clip(s: &str, width: u16) -> String {
    s.chars().take(width as usize).collect()
}

/// The last `width` characters of `s` — keeps a text field's caret in view.
pub fn tail(s: &str, width: u16) -> String {
    let count = s.chars().count();
    let width = width as usize;
    if count <= width {
        s.to_string()
    } else {
        s.chars().skip(count - width).collect()
    }
}

#[cfg(test)]
mod tests;
