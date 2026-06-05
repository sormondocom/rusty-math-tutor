//! Text rendering for the GUI scene: a real TTF via `ab_glyph` when a system
//! font can be found, falling back to the public-domain 8×8 bitmap font.
//!
//! The scale system is designed so `scale = 1.0` matches the old 8×8 font
//! cap height, making it trivial to port terminal-derived sizing.

use std::sync::OnceLock;

use ab_glyph::{Font, FontVec, ScaleFont};
use font8x8::UnicodeFonts;
use tiny_skia::Pixmap;

use super::*;

/// A `scale` of 1.0 is roughly an 8 px cap height (matching the 8×8 fallback).
pub const PX_PER_SCALE: f32 = 11.0;
/// 8×8 fallback glyph advance, in font pixels (8 wide + 1 spacing).
const ADVANCE: f32 = 9.0;

static FONT: OnceLock<Option<FontVec>> = OnceLock::new();

fn font() -> Option<&'static FontVec> {
    FONT.get_or_init(load_font).as_ref()
}

fn load_font() -> Option<FontVec> {
    const PATHS: &[&str] = &[
        r"C:\Windows\Fonts\segoeui.ttf",
        r"C:\Windows\Fonts\arial.ttf",
        r"C:\Windows\Fonts\verdana.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/liberation/LiberationSans-Regular.ttf",
        "/Library/Fonts/Arial.ttf",
    ];
    for p in PATHS {
        if let Ok(bytes) = std::fs::read(p) {
            if let Ok(font) = FontVec::try_from_vec(bytes) {
                return Some(font);
            }
        }
    }
    None
}

/// Draw `s` with its top-left near `(x, y)` (logical coords); `scale` ≈ cap
/// height in 8ths of a logical pixel.  Renders supersampled (`× SS`).
pub fn text(pm: &mut Pixmap, x: f32, y: f32, scale: f32, s: &str, color: Rgb) {
    match font() {
        Some(font) => draw_ttf(pm, font, x * SSF, y * SSF, scale * PX_PER_SCALE * SSF, s, color),
        None => draw_bitmap(pm, x, y, scale, s, color),
    }
}

fn draw_ttf(pm: &mut Pixmap, font: &FontVec, x: f32, y: f32, px: f32, s: &str, color: Rgb) {
    let scaled = font.as_scaled(px);
    let baseline = y + scaled.ascent();
    let is_chk = chalk();
    let bold = px * 0.03;
    let wob = if is_chk && px > 90.0 { px * 0.05 } else { 0.0 };
    let mut caret = x;
    for (gi, ch) in s.chars().enumerate() {
        let gid = font.glyph_id(ch);
        let (jx, jy) = if wob > 0.0 {
            let h = gi as u32 + ch as u32;
            ((hash01(h, 11) - 0.5) * wob, (hash01(h, 71) - 0.5) * wob * 1.4)
        } else {
            (0.0, 0.0)
        };
        let glyph = gid.with_scale_and_position(px, ab_glyph::point(caret + jx, baseline + jy));
        if let Some(outline) = font.outline_glyph(glyph) {
            let bb = outline.px_bounds();
            outline.draw(|gx, gy, cov| {
                let (gx, gy) = (bb.min.x + gx as f32, bb.min.y + gy as f32);
                blend(pm, gx, gy, color, cov);
                if is_chk {
                    for (ox, oy) in [(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)] {
                        blend(pm, gx + ox * bold, gy + oy * bold, color, cov * 0.7);
                    }
                }
            });
        }
        caret += scaled.h_advance(gid);
    }
}

fn draw_bitmap(pm: &mut Pixmap, x: f32, y: f32, scale: f32, s: &str, color: Rgb) {
    let mut cx = x;
    for ch in s.chars() {
        let ch = match ch {
            '×' => 'x', '÷' => '/', '−' => '-', 'π' => 'P', '²' => '2', '³' => '3',
            other => other,
        };
        if let Some(bitmap) = font8x8::BASIC_FONTS.get(ch) {
            for (row, byte) in bitmap.iter().enumerate() {
                for c in 0..8u32 {
                    if byte & (1 << c) != 0 {
                        fill(pm, cx + c as f32 * scale, y + row as f32 * scale, scale, scale, color);
                    }
                }
            }
        }
        cx += ADVANCE * scale;
    }
}

/// Logical width of `s` at `scale` (for centring and fitting).
pub fn text_width(s: &str, scale: f32) -> f32 {
    match font() {
        Some(font) => {
            let scaled = font.as_scaled(scale * PX_PER_SCALE);
            s.chars().map(|ch| scaled.h_advance(font.glyph_id(ch))).sum()
        }
        None => s.chars().count() as f32 * ADVANCE * scale,
    }
}

pub fn text_centered(pm: &mut Pixmap, cx: f32, y: f32, scale: f32, s: &str, color: Rgb) {
    text(pm, cx - text_width(s, scale) / 2.0, y, scale, s, color);
}

/// Largest scale (≤ `max`) at which `s` fits within `max_w`.
pub fn fit_scale(s: &str, max_w: f32, max: f32) -> f32 {
    let w1 = text_width(s, 1.0).max(1.0);
    (max_w / w1).min(max).max(0.8)
}
