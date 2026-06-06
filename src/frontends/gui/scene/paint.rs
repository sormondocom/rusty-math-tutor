//! Palette, theme system, and drawing primitives for the GUI scene.
//!
//! This module is the foundation everything else builds on.  Every submodule
//! receives it through `use super::*;` via the scene re-export in mod.rs.
//!
//! The theme system is a thread-local so it can be read from any drawing call
//! without threading it through every function signature.  `render()` sets it
//! once at the top of each frame.

use std::cell::Cell;

use tiny_skia::{Color, Paint, PathBuilder, Pixmap, PremultipliedColorU8, Rect, Stroke, Transform};

// ---------------------------------------------------------------------------
// Supersampling
// ---------------------------------------------------------------------------

/// Supersample factor: render this many device pixels per logical pixel, then
/// downscale for anti-aliasing.  Must match the divisor in [`crate::gui`].
pub const SS: u32 = 3;
pub const SSF: f32 = SS as f32;

// ---------------------------------------------------------------------------
// Palette
// ---------------------------------------------------------------------------

pub type Rgb = [u8; 3];

pub const BG:      Rgb = [16, 18, 28];
pub const CARD_BG: Rgb = [24, 27, 40];
pub const SEL_BG:  Rgb = [40, 60, 80];
pub const WHITE:   Rgb = [236, 236, 242];
pub const GRAY:    Rgb = [150, 156, 172];
pub const ACCENT:  Rgb = [120, 210, 150];
pub const CYAN:    Rgb = [120, 200, 230];
pub const YELLOW:  Rgb = [240, 210, 90];
pub const RED:     Rgb = [230, 96, 96];
pub const MAGENTA: Rgb = [200, 140, 230];
/// Deduction Duck's feathers, and the dim stage behind his help panel.
pub const DUCK:    Rgb = [245, 205, 70];
pub const STAGE_BG: Rgb = [22, 24, 36];

// ---------------------------------------------------------------------------
// Theme system
// ---------------------------------------------------------------------------

thread_local! {
    static THEME: Cell<crate::config::Theme> = const { Cell::new(crate::config::Theme::Default) };
}


pub fn set_theme(t: crate::config::Theme) {
    THEME.with(|c| c.set(t));
}

pub fn cur_theme() -> crate::config::Theme {
    THEME.with(|c| c.get())
}

/// Whether a chalk-on-board theme is active (Blackboard or Chalkboard).
pub fn is_chalk() -> bool {
    matches!(cur_theme(), crate::config::Theme::Blackboard | crate::config::Theme::Chalkboard)
}

pub fn luma(c: Rgb) -> f32 {
    0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32
}

pub const CHALK_WHITE: Rgb = [236, 234, 222];

pub fn board_dark() -> Rgb {
    match cur_theme() {
        crate::config::Theme::Chalkboard => [16, 40, 28],
        _ => [9, 9, 11],
    }
}

pub fn board_panel() -> Rgb {
    match cur_theme() {
        crate::config::Theme::Chalkboard => [26, 56, 41],
        _ => [22, 22, 26],
    }
}

/// Re-tint a default-palette colour as chalk: dark → board slate; bright → white chalk.
pub fn chalkify(c: Rgb) -> Rgb {
    let l = luma(c);
    if l < 70.0 {
        lerp_rgb(board_dark(), board_panel(), (l / 70.0).clamp(0.0, 1.0))
    } else {
        let t = ((l - 70.0) / 185.0).clamp(0.5, 1.0);
        lerp_rgb([196, 196, 190], CHALK_WHITE, t)
    }
}

pub fn themed(c: Rgb) -> Rgb {
    if is_chalk() { chalkify(c) } else { c }
}

pub fn col(c: Rgb) -> Color {
    let c = themed(c);
    Color::from_rgba8(c[0], c[1], c[2], 255)
}

// ---------------------------------------------------------------------------
// Drawing primitives
// ---------------------------------------------------------------------------

pub fn blend(pm: &mut Pixmap, x: f32, y: f32, color: Rgb, a: f32) {
    if a <= 0.0 || x < 0.0 || y < 0.0 { return; }
    let color = themed(color);
    let (w, h) = (pm.width(), pm.height());
    let (xi, yi) = (x as u32, y as u32);
    if xi >= w || yi >= h { return; }
    let a = a.min(1.0);
    let inv = 1.0 - a;
    let data = pm.pixels_mut();
    let i = (yi * w + xi) as usize;
    let d = data[i];
    let mix = |s: u8, dst: u8| (s as f32 * a + dst as f32 * inv) as u8;
    let (r, g, b) = (mix(color[0], d.red()), mix(color[1], d.green()), mix(color[2], d.blue()));
    if let Some(px) = PremultipliedColorU8::from_rgba(r, g, b, 255) {
        data[i] = px;
    }
}

pub fn fill(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, color: Rgb) {
    if w < 0.4 || h < 0.4 { return; }
    if let Some(rect) = Rect::from_xywh(x * SSF, y * SSF, w * SSF, h * SSF) {
        let mut paint = Paint::default();
        paint.set_color(col(color));
        paint.anti_alias = false;
        pm.fill_rect(rect, &paint, Transform::identity(), None);
    }
}

pub fn stroke_rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, t: f32, color: Rgb) {
    fill(pm, x,         y,         w, t, color);
    fill(pm, x,         y + h - t, w, t, color);
    fill(pm, x,         y,         t, h, color);
    fill(pm, x + w - t, y,         t, h, color);
}

pub fn lerp_rgb(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2])]
}

/// Map a frontend-neutral [`crate::color::Color`] to a pixel RGB triple.
pub fn core_col(c: crate::color::Color) -> Rgb {
    use crate::color::Color as C;
    match c {
        C::LightCyan    => [120, 220, 240],
        C::LightGreen   => [120, 220, 150],
        C::LightYellow  => [240, 215, 110],
        C::LightMagenta => [210, 140, 230],
        C::LightBlue    => [120, 170, 240],
        C::LightRed     => [235, 110, 110],
        C::Cyan         => [80,  180, 200],
        C::Green        => [90,  190, 120],
        C::Rgb(r, g, b) => [r, g, b],
    }
}

pub fn poly(points: &[(f32, f32)]) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    let (x0, y0) = points[0];
    pb.move_to(x0 * SSF, y0 * SSF);
    for &(x, y) in &points[1..] {
        pb.line_to(x * SSF, y * SSF);
    }
    pb.close();
    pb.finish()
}

pub fn fill_path(pm: &mut Pixmap, path: &tiny_skia::Path, color: Rgb) {
    let mut paint = Paint::default();
    paint.set_color(col(color));
    paint.anti_alias = false;
    pm.fill_path(path, &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
}

pub fn wavy_line(pm: &mut Pixmap, x0: f32, x1: f32, y: f32, amp: f32, color: Rgb, width: f32) {
    use std::f32::consts::TAU;
    let steps = (((x1 - x0) / 8.0) as i32).max(2);
    let mut prev = (x0, y);
    for s in 1..=steps {
        let t = s as f32 / steps as f32;
        let x = x0 + (x1 - x0) * t;
        let yy = y + (t * TAU * 1.5).sin() * amp;
        line(pm, prev.0, prev.1, x, yy, color, width);
        prev = (x, yy);
    }
}

fn stroke_path(pm: &mut Pixmap, path: &tiny_skia::Path, color: Rgb, width: f32) {
    let mut paint = Paint::default();
    paint.set_color(col(color));
    // AA stays off: tiny-skia's AA hairline rasteriser panics on some inputs.
    // Smooth edges come from rendering at SS× and box-averaging back down.
    paint.anti_alias = false;
    let s = Stroke { width: width * SSF * chalk_mul(), ..Default::default() };
    pm.stroke_path(path, &paint, &s, Transform::identity(), None);
}

pub fn chalk_mul() -> f32 {
    if is_chalk() { 2.4 } else { 1.0 }
}

pub fn chalk() -> bool {
    is_chalk()
}

pub fn line(pm: &mut Pixmap, x0: f32, y0: f32, x1: f32, y1: f32, color: Rgb, width: f32) {
    if (x0 - x1).abs() < 0.5 && (y0 - y1).abs() < 0.5 { return; }
    let (dx, dy) = (x1 - x0, y1 - y0);
    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    let mut pb = PathBuilder::new();
    pb.move_to(x0 * SSF, y0 * SSF);
    if chalk() {
        let (px, py) = (-dy / len, dx / len);
        let segs = (len / 36.0).clamp(2.0, 8.0) as u32;
        for k in 1..=segs {
            let t = k as f32 / segs as f32;
            let (bx, by) = (x0 + dx * t, y0 + dy * t);
            let amp = if k == segs { 0.0 } else { (hash01((bx * 5.0) as u32, (by * 5.0) as u32) - 0.5) * 5.0 };
            pb.line_to((bx + px * amp) * SSF, (by + py * amp) * SSF);
        }
    } else {
        pb.line_to(x1 * SSF, y1 * SSF);
    }
    if let Some(path) = pb.finish() {
        stroke_path(pm, &path, color, width);
    }
}

pub fn circle(pm: &mut Pixmap, cx: f32, cy: f32, r: f32, color: Rgb, width: f32) {
    if r < 1.0 { return; }
    let mut pb = PathBuilder::new();
    pb.push_circle(cx * SSF, cy * SSF, r * SSF);
    if let Some(path) = pb.finish() {
        stroke_path(pm, &path, color, width);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn tri(pm: &mut Pixmap, x0: f32, y0: f32, x1: f32, y1: f32, x2: f32, y2: f32, color: Rgb, width: f32) {
    let mut pb = PathBuilder::new();
    pb.move_to(x0 * SSF, y0 * SSF);
    pb.line_to(x1 * SSF, y1 * SSF);
    pb.line_to(x2 * SSF, y2 * SSF);
    pb.close();
    if let Some(path) = pb.finish() {
        stroke_path(pm, &path, color, width);
    }
}

pub fn fit_box(uw: i64, uh: i64, maxw: f32, maxh: f32) -> (f32, f32) {
    let (uw, uh) = (uw.max(1) as f32, uh.max(1) as f32);
    let s = (maxw / uw).min(maxh / uh);
    (uw * s, uh * s)
}

pub fn hash01(x: u32, y: u32) -> f32 {
    let mut n = x.wrapping_mul(374761393).wrapping_add(y.wrapping_mul(668265263));
    n = (n ^ (n >> 13)).wrapping_mul(1274126177);
    (n & 0xffff) as f32 / 65535.0
}

pub fn prem(c: Rgb) -> PremultipliedColorU8 {
    PremultipliedColorU8::from_rgba(c[0], c[1], c[2], 255).unwrap_or(PremultipliedColorU8::TRANSPARENT)
}
