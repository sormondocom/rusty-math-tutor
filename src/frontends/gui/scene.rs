//! Pixel rendering for the GUI frontend.
//!
//! Reads the same `&App` the terminal does and paints it into a `tiny-skia` CPU
//! pixmap.  Two quality tricks, both done on the CPU:
//!
//! * **Supersampling.** Everything is rendered into a pixmap scaled up by [`SS`]
//!   with anti-aliasing *off* (tiny-skia's AA rasteriser panics on some inputs),
//!   then the window step ([`crate::gui`]) box-downscales it.  The averaging *is*
//!   the anti-aliasing — smooth shapes and text without ever touching the buggy
//!   AA path.
//! * **A real font.** Text uses a TTF via `ab_glyph` when a system font can be
//!   found, falling back to the public-domain 8×8 bitmap font.

use std::sync::OnceLock;

use ab_glyph::{Font, FontVec, ScaleFont};
use font8x8::UnicodeFonts;
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, PremultipliedColorU8, Rect, Stroke, Transform};

use crate::app::{App, Feedback, Screen};
use crate::geometry::{GeometryProblem, GeoShape};
use crate::section::Active;

/// Supersample factor: render this many device pixels per logical pixel, then
/// downscale for anti-aliasing.  Must match the divisor in [`crate::gui`].
pub const SS: u32 = 3;
const SSF: f32 = SS as f32;

type Rgb = [u8; 3];
const BG: Rgb = [16, 18, 28];
const CARD_BG: Rgb = [24, 27, 40];
const SEL_BG: Rgb = [40, 60, 80];
const WHITE: Rgb = [236, 236, 242];
const GRAY: Rgb = [150, 156, 172];
const ACCENT: Rgb = [120, 210, 150];
const CYAN: Rgb = [120, 200, 230];
const YELLOW: Rgb = [240, 210, 90];
const RED: Rgb = [230, 96, 96];

fn col(c: Rgb) -> Color {
    Color::from_rgba8(c[0], c[1], c[2], 255)
}

/// Paint a full frame for the app's current screen, into a pixmap that is `SS`×
/// the logical `(w, h)`.  Drawing is in *logical* coordinates; the low-level
/// helpers scale up by `SS`.  The window step downscales the result.
pub fn render(app: &App, w: u32, h: u32) -> Pixmap {
    let mut pm = Pixmap::new((w * SS).max(1), (h * SS).max(1)).unwrap_or_else(|| Pixmap::new(1, 1).unwrap());
    pm.fill(col(BG));
    let (wf, hf) = (w as f32, h as f32);
    match app.screen {
        Screen::Startup => draw_startup(&mut pm, app, wf, hf),
        Screen::Menu => draw_menu(&mut pm, app, wf, hf),
        Screen::Practice | Screen::Challenge => draw_session(&mut pm, app, wf, hf),
        Screen::Settings => placeholder(&mut pm, wf, hf, "Settings"),
        Screen::Stats => placeholder(&mut pm, wf, hf, "My Progress"),
        Screen::Teacher => placeholder(&mut pm, wf, hf, "Teacher Area"),
        Screen::Cinematic => placeholder(&mut pm, wf, hf, "Milestone!"),
        Screen::Experiment => placeholder(&mut pm, wf, hf, "Experimentation"),
    }
    pm
}

// ---------------------------------------------------------------------------
// Screens
// ---------------------------------------------------------------------------

fn draw_startup(pm: &mut Pixmap, app: &App, wf: f32, hf: f32) {
    text_centered(pm, wf / 2.0, 90.0, 6.0, "RUSTY MATH TUTOR", WHITE);
    text_centered(pm, wf / 2.0, 170.0, 2.0, "Featuring:  Deduction Duck", YELLOW);
    text_centered(pm, wf / 2.0, 240.0, 2.0, "Choose how to draw:", GRAY);
    let modes = ["Low Graphics (console)", "CPU Graphics (window)"];
    let x = wf / 2.0 - 200.0;
    for (i, m) in modes.iter().enumerate() {
        let y = 300.0 + i as f32 * 56.0;
        let selected = i == app.startup_index;
        if selected {
            fill(pm, x - 16.0, y - 8.0, 420.0, 40.0, SEL_BG);
        }
        text(pm, x, y, 2.5, m, if selected { WHITE } else { GRAY });
    }
    text_centered(pm, wf / 2.0, hf - 50.0, 1.5, "Up / Down to choose    Enter to start", GRAY);
}

fn draw_menu(pm: &mut Pixmap, app: &App, wf: f32, hf: f32) {
    text_centered(pm, wf / 2.0, 26.0, 5.0, "RUSTY MATH TUTOR", WHITE);
    text_centered(pm, wf / 2.0, 84.0, 1.75, "Featuring:  Deduction Duck", YELLOW);

    let rows = menu_rows(app);
    let scale = 1.75;
    let row_h = 28.0;
    let top = 130.0;
    // Center the whole block on the widest row, so it sits under the title; the
    // highlight bar matches that width (+ padding) so the selection never clips.
    let text_w = rows.iter().map(|r| text_width(r, scale)).fold(0.0_f32, f32::max);
    let x = (wf - text_w) / 2.0;
    let pad = 14.0;
    for (i, label) in rows.iter().enumerate() {
        let y = top + i as f32 * row_h;
        if i == app.menu_index {
            fill(pm, x - pad, y - 5.0, text_w + pad * 2.0, row_h - 2.0, SEL_BG);
        }
        let color = if i == app.menu_index { WHITE } else { GRAY };
        text(pm, x, y, scale, label, color);
    }
    text_centered(pm, wf / 2.0, hf - 36.0, 1.5, "Up / Down move    Enter select    Q quit", GRAY);
}

/// The menu rows, in the same order as the core's `menu_index`.
fn menu_rows(app: &App) -> Vec<String> {
    let mark = |on: bool| if on { "[x]" } else { "[ ]" };
    let grade = if app.menu_grade == 0 { "K".to_string() } else { app.menu_grade.to_string() };
    let ops = ["Add", "Subtract", "Multiply", "Divide"];
    let mut v = vec![
        format!("Student:  < {} >", app.roster.current().name),
        format!("Grade level:  < {} >", grade),
    ];
    for (i, name) in ops.iter().enumerate() {
        v.push(format!("{} {}", mark(app.menu_ops[i]), name));
    }
    v.push(format!("{} Units of Measure", mark(app.menu_units)));
    v.push(format!("{} Fractions", mark(app.menu_fractions)));
    v.push(format!("{} Percentages", mark(app.menu_percents)));
    v.push(format!("{} Geometry", mark(app.menu_geometry)));
    v.push(format!("Show problems:  {}", app.config.layout.name()));
    v.push("Settings...".to_string());
    v.push("My Progress...".to_string());
    v.push("Teacher Area...".to_string());
    v.push(">  Start Practice".to_string());
    v.push(">  Start Challenge".to_string());
    v.push(">  Experimentation".to_string());
    v
}

fn draw_session(pm: &mut Pixmap, app: &App, wf: f32, hf: f32) {
    let cw = (wf * 0.86).min(1020.0);
    let ch = (hf * 0.80).min(700.0);
    let (cx0, cy0) = ((wf - cw) / 2.0, (hf - ch) / 2.0);
    let cxc = wf / 2.0;
    fill(pm, cx0, cy0, cw, ch, CARD_BG);
    stroke_rect(pm, cx0, cy0, cw, ch, 2.0, ACCENT);

    // The question / prompt, scaled to fit the card width.
    let q = question_of(&app.current);
    let qscale = fit_scale(&q, cw - 80.0, 5.0);
    text_centered(pm, cxc, cy0 + 50.0, qscale, &q, WHITE);

    // The per-section visual (geometry shapes as crisp tiny-skia paths).  When a
    // shape is shown it takes the upper band and the answer moves below it.
    let mut answer_y = cy0 + ch * 0.42;
    let mut ans_scale = 7.0;
    if let Active::Geo(g) = &app.current {
        let band_h = ch * 0.34;
        draw_geo(pm, (cx0 + 50.0, cy0 + 96.0, cw - 100.0, band_h), g);
        answer_y = cy0 + 96.0 + band_h + 26.0;
        ans_scale = 4.0;
    }

    // The big answer (typed, or "?").
    let ans = if app.input.is_empty() { "?".to_string() } else { app.input.clone() };
    text_centered(pm, cxc, answer_y, 2.0, "Your answer:", GRAY);
    let ans_color = match app.feedback {
        Feedback::Correct => ACCENT,
        Feedback::Wrong => RED,
        Feedback::None => CYAN,
    };
    text_centered(pm, cxc, answer_y + 28.0, ans_scale, &ans, ans_color);

    // Feedback line, just above the footer.
    if let Some((msg, color)) = match app.feedback {
        Feedback::Correct => Some(("Correct!", ACCENT)),
        Feedback::Wrong => Some(("Not quite - try again", RED)),
        Feedback::None => None,
    } {
        text_centered(pm, cxc, cy0 + ch - 84.0, 3.0, msg, color);
    }

    text_centered(pm, cxc, cy0 + ch - 36.0, 1.5, "H help    Y why    Enter check    Esc menu", GRAY);
}

fn placeholder(pm: &mut Pixmap, wf: f32, hf: f32, title: &str) {
    text_centered(pm, wf / 2.0, hf / 2.0 - 24.0, 4.0, title, WHITE);
    text_centered(pm, wf / 2.0, hf / 2.0 + 30.0, 1.75, "(full GUI rendering coming soon)", GRAY);
    text_centered(pm, wf / 2.0, hf - 50.0, 1.5, "Esc / Enter to go back", GRAY);
}

/// The headline string for whichever section produced the active problem.
fn question_of(active: &Active) -> String {
    match active {
        Active::Arith(p) => p.prompt(),
        Active::Unit(u) => u.question.clone(),
        Active::Shape(s) => s.question(),
        Active::Geo(g) => g.question(),
    }
}

// ---------------------------------------------------------------------------
// Text — a real TTF via ab_glyph, with an 8×8 bitmap fallback
// ---------------------------------------------------------------------------

/// A `scale` of 1.0 is roughly an 8 px cap height (matching the old 8×8 font);
/// the TTF size that gives that is a bit larger because of ascenders.
const PX_PER_SCALE: f32 = 11.0;
/// 8×8 fallback glyph advance, in font pixels (8 wide + 1 spacing).
const ADVANCE: f32 = 9.0;

static FONT: OnceLock<Option<FontVec>> = OnceLock::new();

/// The loaded system TTF, if any (`None` → use the 8×8 bitmap fallback).
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
fn text(pm: &mut Pixmap, x: f32, y: f32, scale: f32, s: &str, color: Rgb) {
    match font() {
        Some(font) => draw_ttf(pm, font, x * SSF, y * SSF, scale * PX_PER_SCALE * SSF, s, color),
        None => draw_bitmap(pm, x, y, scale, s, color),
    }
}

fn draw_ttf(pm: &mut Pixmap, font: &FontVec, x: f32, y: f32, px: f32, s: &str, color: Rgb) {
    let scaled = font.as_scaled(px);
    let baseline = y + scaled.ascent();
    let mut caret = x;
    for ch in s.chars() {
        let gid = font.glyph_id(ch);
        let glyph = gid.with_scale_and_position(px, ab_glyph::point(caret, baseline));
        if let Some(outline) = font.outline_glyph(glyph) {
            let bb = outline.px_bounds();
            outline.draw(|gx, gy, cov| blend(pm, bb.min.x + gx as f32, bb.min.y + gy as f32, color, cov));
        }
        caret += scaled.h_advance(gid);
    }
}

fn draw_bitmap(pm: &mut Pixmap, x: f32, y: f32, scale: f32, s: &str, color: Rgb) {
    let mut cx = x;
    for ch in s.chars() {
        let ch = match ch {
            '×' => 'x',
            '÷' => '/',
            '−' => '-',
            'π' => 'P',
            '²' => '2',
            '³' => '3',
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

/// Logical width of `s` at `scale` (for centring/fitting).
fn text_width(s: &str, scale: f32) -> f32 {
    match font() {
        Some(font) => {
            let scaled = font.as_scaled(scale * PX_PER_SCALE);
            s.chars().map(|ch| scaled.h_advance(font.glyph_id(ch))).sum()
        }
        None => s.chars().count() as f32 * ADVANCE * scale,
    }
}

fn text_centered(pm: &mut Pixmap, cx: f32, y: f32, scale: f32, s: &str, color: Rgb) {
    text(pm, cx - text_width(s, scale) / 2.0, y, scale, s, color);
}

/// Largest scale (≤ `max`) at which `s` fits within `max_w`.
fn fit_scale(s: &str, max_w: f32, max: f32) -> f32 {
    let w1 = text_width(s, 1.0).max(1.0); // width scales linearly with `scale`
    (max_w / w1).min(max).max(0.8)
}

/// Blend a coverage pixel (0..1) at device `(x, y)` over the pixmap.
fn blend(pm: &mut Pixmap, x: f32, y: f32, color: Rgb, a: f32) {
    if a <= 0.0 || x < 0.0 || y < 0.0 {
        return;
    }
    let (w, h) = (pm.width(), pm.height());
    let (xi, yi) = (x as u32, y as u32);
    if xi >= w || yi >= h {
        return;
    }
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

// ---------------------------------------------------------------------------
// Shapes — supersampled tiny-skia (anti-aliasing via the downscale)
// ---------------------------------------------------------------------------

fn fill(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, color: Rgb) {
    if w < 0.4 || h < 0.4 {
        return;
    }
    if let Some(rect) = Rect::from_xywh(x * SSF, y * SSF, w * SSF, h * SSF) {
        let mut paint = Paint::default();
        paint.set_color(col(color));
        paint.anti_alias = false; // AA comes from the supersample downscale
        pm.fill_rect(rect, &paint, Transform::identity(), None);
    }
}

fn stroke_rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, t: f32, color: Rgb) {
    fill(pm, x, y, w, t, color); // top
    fill(pm, x, y + h - t, w, t, color); // bottom
    fill(pm, x, y, t, h, color); // left
    fill(pm, x + w - t, y, t, h, color); // right
}

// ---------------------------------------------------------------------------
// Geometry shapes — smooth anti-aliased tiny-skia paths
// ---------------------------------------------------------------------------

/// Draw the section's shape inside the region `(x, y, w, h)`, labelled.
fn draw_geo(pm: &mut Pixmap, (rx, ry, rw, rh): (f32, f32, f32, f32), g: &GeometryProblem) {
    let (cx, cy) = (rx + rw / 2.0, ry + rh / 2.0);
    match g.shape {
        GeoShape::Rect { w, h } => {
            let (bw, bh) = fit_box(w, h, rw * 0.66, rh * 0.78);
            let (x0, y0) = (cx - bw / 2.0, cy - bh / 2.0);
            stroke_rect(pm, x0, y0, bw, bh, 3.0, ACCENT);
            text_centered(pm, cx, y0 + bh + 10.0, 1.5, &w.to_string(), YELLOW);
            text(pm, x0 + bw + 14.0, cy - 6.0, 1.5, &h.to_string(), YELLOW);
        }
        GeoShape::Triangle { base, height, sides } => {
            let half = (rw * 0.4).min(rh * 0.9);
            let th = rh * 0.78;
            let (ay, by) = (cy - th / 2.0, cy + th / 2.0);
            tri(pm, cx, ay, cx - half, by, cx + half, by, ACCENT, 3.0);
            if sides.is_none() {
                line(pm, cx, ay, cx, by, YELLOW, 2.0); // the height
            }
            let label = match sides {
                Some([a, b, c]) => format!("sides: {}, {}, {} cm", a, b, c),
                None => format!("base {} cm,  height {} cm", base, height),
            };
            text_centered(pm, cx, by + 14.0, 1.5, &label, GRAY);
        }
        GeoShape::Circle { r } => {
            let rad = rh.min(rw / 2.0) * 0.78 * (0.55 + (r.clamp(2, 9) as f32) / 22.0);
            circle(pm, cx, cy, rad, ACCENT, 3.0);
            line(pm, cx, cy, cx + rad, cy, YELLOW, 2.0); // the radius
            text_centered(pm, cx, cy + rad + 16.0, 1.5, &format!("radius {} cm", r), GRAY);
        }
        GeoShape::Box3 { l, w, h } => {
            let fw = (rw * 0.42).min(rh * 1.2);
            let fh = rh * 0.5;
            let d = rh * 0.18;
            let (x0, y0) = (cx - (fw + d) / 2.0, cy - fh / 2.0 + d / 2.0);
            stroke_rect(pm, x0, y0, fw, fh, 2.0, ACCENT); // front face
            stroke_rect(pm, x0 + d, y0 - d, fw, fh, 2.0, ACCENT); // back face
            line(pm, x0, y0, x0 + d, y0 - d, ACCENT, 2.0); // connect the corners
            line(pm, x0 + fw, y0, x0 + fw + d, y0 - d, ACCENT, 2.0);
            line(pm, x0, y0 + fh, x0 + d, y0 + fh - d, ACCENT, 2.0);
            line(pm, x0 + fw, y0 + fh, x0 + fw + d, y0 + fh - d, ACCENT, 2.0);
            text_centered(pm, cx, y0 + fh + 16.0, 1.5, &format!("{} x {} x {} cm", l, w, h), GRAY);
        }
    }
}

/// Stroke a path already expressed in **device** (supersampled) coordinates.
fn stroke(pm: &mut Pixmap, path: &tiny_skia::Path, color: Rgb, width: f32) {
    let mut paint = Paint::default();
    paint.set_color(col(color));
    // Anti-aliasing stays off here on purpose: tiny-skia's AA hairline rasteriser
    // panics on some inputs.  Smooth edges instead come from rendering at `SS×`
    // and box-averaging back down (see mod.rs) — AA without the buggy path.
    paint.anti_alias = false;
    let mut s = Stroke::default();
    s.width = width * SSF;
    pm.stroke_path(path, &paint, &s, Transform::identity(), None);
}

fn line(pm: &mut Pixmap, x0: f32, y0: f32, x1: f32, y1: f32, color: Rgb, width: f32) {
    // Skip degenerate (zero-length) segments — they have no direction to stroke.
    if (x0 - x1).abs() < 0.5 && (y0 - y1).abs() < 0.5 {
        return;
    }
    let mut pb = PathBuilder::new();
    pb.move_to(x0 * SSF, y0 * SSF);
    pb.line_to(x1 * SSF, y1 * SSF);
    if let Some(path) = pb.finish() {
        stroke(pm, &path, color, width);
    }
}

fn circle(pm: &mut Pixmap, cx: f32, cy: f32, r: f32, color: Rgb, width: f32) {
    if r < 1.0 {
        return;
    }
    let mut pb = PathBuilder::new();
    pb.push_circle(cx * SSF, cy * SSF, r * SSF);
    if let Some(path) = pb.finish() {
        stroke(pm, &path, color, width);
    }
}

fn tri(pm: &mut Pixmap, x0: f32, y0: f32, x1: f32, y1: f32, x2: f32, y2: f32, color: Rgb, width: f32) {
    let mut pb = PathBuilder::new();
    pb.move_to(x0 * SSF, y0 * SSF);
    pb.line_to(x1 * SSF, y1 * SSF);
    pb.line_to(x2 * SSF, y2 * SSF);
    pb.close();
    if let Some(path) = pb.finish() {
        stroke(pm, &path, color, width);
    }
}

/// Cell-free pixel sizing: scale unit `w × h` to fit `(maxw, maxh)`, preserving
/// the ratio (GUI pixels are square, so no aspect fudge needed).
fn fit_box(uw: i64, uh: i64, maxw: f32, maxh: f32) -> (f32, f32) {
    let (uw, uh) = (uw.max(1) as f32, uh.max(1) as f32);
    let s = (maxw / uw).min(maxh / uh);
    (uw * s, uh * s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_renders_pixels() {
        // Font-agnostic smoke test (TTF or 8×8 fallback): drawing text must light
        // *some* pixels, and `text_width` must grow with the string length.  `text`
        // scales coords by SS internally, so the pixmap is sized generously.
        let mut pm = Pixmap::new(400, 120).unwrap();
        text(&mut pm, 6.0, 6.0, 2.0, "Hi", WHITE);
        let any_lit = pm.pixels().iter().any(|p| p.red() > 30);
        assert!(any_lit, "text should light at least one pixel");
        assert!(text_width("WWWW", 2.0) > text_width("W", 2.0), "wider strings measure wider");
    }

    #[test]
    fn fit_scale_is_bounded() {
        // Never returns above the cap, nor a useless sub-pixel scale.
        let s = fit_scale("a very long label that must shrink", 100.0, 6.0);
        assert!(s <= 6.0 && s >= 0.8, "fit_scale stays within [0.8, max]");
    }
}
