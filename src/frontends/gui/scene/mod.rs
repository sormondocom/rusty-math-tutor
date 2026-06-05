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

use std::cell::Cell;
use std::sync::OnceLock;

use ab_glyph::{Font, FontVec, ScaleFont};
use font8x8::UnicodeFonts;
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, PremultipliedColorU8, Rect, Stroke, Transform};

use crate::app::{App, Feedback, Screen};
use crate::fraction::{FractionProblem, Mode};
use crate::section::Active;

mod cinematic;
mod figures;
mod help;
mod screens;
mod transition;
use cinematic::*;
use figures::*;
use help::*;
use screens::*;
use transition::*;

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
const MAGENTA: Rgb = [200, 140, 230];
/// Deduction Duck's feathers, and the dim stage behind his help panel.
const DUCK: Rgb = [245, 205, 70];
const STAGE_BG: Rgb = [22, 24, 36];

// ---------------------------------------------------------------------------
// Themes — applied at the pixel-write layer so the whole palette re-tints with
// no change to any drawing call site.  `render()` sets the active theme; `col`,
// `blend`, and `fill_dev` (the only Rgb→pixel chokepoints) run colours through
// `themed()` first.
// ---------------------------------------------------------------------------

thread_local! {
    static THEME: Cell<crate::config::Theme> = const { Cell::new(crate::config::Theme::Default) };
}

fn set_theme(t: crate::config::Theme) {
    THEME.with(|c| c.set(t));
}

fn cur_theme() -> crate::config::Theme {
    THEME.with(|c| c.get())
}

/// Whether a chalk-on-board theme is active (Blackboard or Chalkboard).
fn is_chalk() -> bool {
    matches!(cur_theme(), crate::config::Theme::Blackboard | crate::config::Theme::Chalkboard)
}

/// Perceived brightness of a colour (Rec. 601 luma).
fn luma(c: Rgb) -> f32 {
    0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32
}

const CHALK_WHITE: Rgb = [236, 234, 222];

/// The board's darkest tone for the active chalk theme (black, or dark green).
fn board_dark() -> Rgb {
    match cur_theme() {
        crate::config::Theme::Chalkboard => [16, 40, 28],
        _ => [9, 9, 11],
    }
}

/// The board's faintly-lifted tone (cards / selection sit on this).
fn board_panel() -> Rgb {
    match cur_theme() {
        crate::config::Theme::Chalkboard => [26, 56, 41],
        _ => [22, 22, 26],
    }
}

/// Re-tint a default-palette colour as chalk on a board: dark colours become the
/// board slate (darker → darker); everything bright becomes **white chalk** (no
/// colour), with a little tonal range so secondary text still reads as dimmer.
/// The grainy, dusty *texture* is added afterwards by [`chalk_texture`].
fn chalkify(c: Rgb) -> Rgb {
    let l = luma(c);
    if l < 70.0 {
        lerp_rgb(board_dark(), board_panel(), (l / 70.0).clamp(0.0, 1.0))
    } else {
        // Brighter source = harder chalk press = whiter; all neutral (no hue).
        let t = ((l - 70.0) / 185.0).clamp(0.5, 1.0);
        lerp_rgb([196, 196, 190], CHALK_WHITE, t)
    }
}

/// Map a colour through the active theme.
fn themed(c: Rgb) -> Rgb {
    if is_chalk() {
        chalkify(c)
    } else {
        c
    }
}

fn col(c: Rgb) -> Color {
    let c = themed(c);
    Color::from_rgba8(c[0], c[1], c[2], 255)
}

/// Frame insets (logical px) for the chalk themes: a wooden border, with a taller
/// ledge along the bottom for the chalk tray.
const FRAME_T: u32 = 22;
const FRAME_TRAY: u32 = 40;

/// Paint a full frame for the app's current screen, into a pixmap that is `SS`×
/// the logical `(w, h)`.  Drawing is in *logical* coordinates; the low-level
/// helpers scale up by `SS`.  The window step downscales the result.
pub fn render(app: &App, w: u32, h: u32) -> Pixmap {
    set_theme(app.config.theme);
    let mut pm = Pixmap::new((w * SS).max(1), (h * SS).max(1)).unwrap_or_else(|| Pixmap::new(1, 1).unwrap());
    pm.fill(col(BG));

    if is_chalk() && w > 2 * FRAME_T && h > FRAME_T + FRAME_TRAY {
        // Render the scene into the inset board area, texture it as chalk, blit it
        // inside the wooden frame, then paint the frame in the margins.
        let (iw, ih) = (w - 2 * FRAME_T, h - FRAME_T - FRAME_TRAY);
        let mut inner = Pixmap::new(iw * SS, ih * SS).unwrap_or_else(|| Pixmap::new(1, 1).unwrap());
        inner.fill(col(BG));
        draw_screen(&mut inner, app, iw as f32, ih as f32);
        chalk_texture(&mut inner);
        blit(&mut pm, &inner, FRAME_T * SS, FRAME_T * SS);
        draw_board_frame(&mut pm);
    } else {
        draw_screen(&mut pm, app, w as f32, h as f32);
    }
    pm
}

/// Dispatch the active screen into `pm` at logical size `(wf, hf)`.
fn draw_screen(pm: &mut Pixmap, app: &App, wf: f32, hf: f32) {
    match app.screen {
        Screen::Startup => draw_startup(pm, app, wf, hf),
        Screen::Menu => draw_menu(pm, app, wf, hf),
        Screen::Practice | Screen::Challenge => draw_session(pm, app, wf, hf),
        Screen::Settings => draw_settings(pm, app, wf, hf),
        Screen::Stats => draw_stats(pm, app, wf, hf),
        Screen::Teacher => draw_teacher(pm, app, wf, hf),
        Screen::Cinematic => draw_cinematic(pm, app, wf, hf),
        Screen::Experiment => draw_experiment(pm, app, wf, hf),
    }
}

/// Copy `src` into `dst` with its top-left at device offset `(ox, oy)`.
fn blit(dst: &mut Pixmap, src: &Pixmap, ox: u32, oy: u32) {
    let (dw, dh) = (dst.width(), dst.height());
    let (sw, sh) = (src.width(), src.height());
    let spx = src.pixels();
    let dpx = dst.pixels_mut();
    for y in 0..sh {
        let dy = oy + y;
        if dy >= dh {
            break;
        }
        let drow = (dy * dw) as usize;
        let srow = (y * sw) as usize;
        for x in 0..sw {
            let dx = ox + x;
            if dx >= dw {
                break;
            }
            dpx[drow + dx as usize] = spx[srow + x as usize];
        }
    }
}

/// Paint the wooden blackboard frame (and its chalk tray) in `pm`'s margins.
fn draw_board_frame(pm: &mut Pixmap) {
    let (w, h) = (pm.width() as i32, pm.height() as i32);
    let t = (FRAME_T * SS) as i32;
    let tray = (FRAME_TRAY * SS) as i32;
    let wood = [120, 80, 44];
    let wood_lo = [84, 54, 28];
    let wood_hi = [158, 112, 66];
    let bevel = (3.0 * SSF) as i32;

    // The four wooden borders (the bottom one is the taller tray).
    fill_raw(pm, 0, 0, w, t, wood);
    fill_raw(pm, 0, 0, t, h, wood);
    fill_raw(pm, w - t, 0, t, h, wood);
    fill_raw(pm, 0, h - tray, w, tray, wood);

    // Subtle grain: faint streaks along each member.
    for x in (0..w).step_by((7.0 * SSF) as usize + 1) {
        if hash01(x as u32, 3) > 0.6 {
            fill_raw(pm, x, 0, bevel.max(1), t, wood_lo);
            fill_raw(pm, x, h - tray, bevel.max(1), tray, wood_lo);
        }
    }
    for y in (0..h).step_by((7.0 * SSF) as usize + 1) {
        if hash01(5, y as u32) > 0.6 {
            fill_raw(pm, 0, y, t, bevel.max(1), wood_lo);
            fill_raw(pm, w - t, y, t, bevel.max(1), wood_lo);
        }
    }

    // Bevels for a little depth: highlight on the outer rim, shadow at the board.
    fill_raw(pm, 0, 0, w, bevel, wood_hi);
    fill_raw(pm, 0, 0, bevel, h, wood_hi);
    fill_raw(pm, w - bevel, 0, bevel, h, wood_lo);
    fill_raw(pm, t - bevel, t, w - 2 * t + 2 * bevel, bevel, wood_lo); // board top inner edge
    fill_raw(pm, t - bevel, h - tray, bevel, tray, wood_hi);
    fill_raw(pm, w - t, h - tray, bevel, tray, wood_lo);
    fill_raw(pm, t, h - tray, w - 2 * t, bevel, wood_hi); // tray ledge highlight

    // A stick of chalk and a felt eraser resting on the tray.
    let ty = h - tray + (10.0 * SSF) as i32;
    fill_raw(pm, w / 2 - (210.0 * SSF) as i32, ty + (4.0 * SSF) as i32, (96.0 * SSF) as i32, (9.0 * SSF) as i32, [242, 240, 230]);
    let er_x = w / 2 + (120.0 * SSF) as i32;
    fill_raw(pm, er_x, ty, (74.0 * SSF) as i32, (18.0 * SSF) as i32, [120, 120, 128]);
    fill_raw(pm, er_x, ty, (74.0 * SSF) as i32, (6.0 * SSF) as i32, [150, 110, 70]);
}

/// Fill an opaque device-pixel rect with a *raw* (un-themed) colour, clipped.
fn fill_raw(pm: &mut Pixmap, x: i32, y: i32, w: i32, h: i32, rgb: Rgb) {
    let (pw, ph) = (pm.width() as i32, pm.height() as i32);
    let (x0, y0) = (x.max(0), y.max(0));
    let (x1, y1) = ((x + w).min(pw), (y + h).min(ph));
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let px = prem(rgb);
    let pwu = pm.width();
    let data = pm.pixels_mut();
    for yy in y0..y1 {
        let row = (yy as u32 * pwu) as usize;
        for xx in x0..x1 {
            data[row + xx as usize] = px;
        }
    }
}

/// Post-process the rendered frame to make the chalk marks look like *chalk*:
/// grainy strokes, dusty gaps where the board shows through, and the odd bright
/// fleck — applied only to foreground pixels (the board itself is left alone).
/// Runs at supersampled resolution; the downscale then softens it into fine dust.
fn chalk_texture(pm: &mut Pixmap) {
    let board = themed(BG);
    let board_luma = luma(board);
    let (w, h) = (pm.width(), pm.height());
    let data = pm.pixels_mut();
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            let p = data[i];
            let c = [p.red(), p.green(), p.blue()];
            // Only texture the chalk (anything clearly brighter than the board).
            if luma(c) - board_luma < 14.0 {
                continue;
            }
            // Coarse grain (features survive the SS downscale) modulates coverage;
            // a sparse second channel punches dust gaps and bright flecks.  Kept
            // gentle so small text stays solid rather than ghostly.
            let grain = 0.84 + 0.16 * hash01(x / 3, y / 3);
            let speck = hash01(x.wrapping_add(7), y.wrapping_mul(3).wrapping_add(13));
            let out = if speck < 0.025 {
                board // a dust gap — board shows through
            } else if speck > 0.985 {
                lerp_rgb(c, CHALK_WHITE, 0.6) // a bright fleck of chalk dust
            } else {
                lerp_rgb(board, c, grain) // grainy stroke body
            };
            if let Some(px) = PremultipliedColorU8::from_rgba(out[0], out[1], out[2], 255) {
                data[i] = px;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Practice / Challenge session card
// ---------------------------------------------------------------------------

/// The card rectangle `(x, y, w, h)` for the session screen.
fn card_rect(wf: f32, hf: f32) -> (f32, f32, f32, f32) {
    let cw = (wf * 0.86).min(1020.0);
    let ch = (hf * 0.80).min(700.0);
    ((wf - cw) / 2.0, (hf - ch) / 2.0, cw, ch)
}

/// Draw one problem card.  Parameterised (not reading `app.current` directly) so
/// the transition layer can capture both the answered and the next card.
fn draw_card(pm: &mut Pixmap, active: &Active, input: &str, feedback: Feedback, frac: f32, anim: u64, layout: crate::config::Layout, wf: f32, hf: f32) {
    let (cx0, cy0, cw, ch) = card_rect(wf, hf);
    let cxc = wf / 2.0;
    let accent = if let Active::Arith(p) = active { core_col(p.accent) } else { ACCENT };
    fill(pm, cx0, cy0, cw, ch, CARD_BG);
    stroke_rect(pm, cx0, cy0, cw, ch, 2.0, accent);

    // Arithmetic owns the whole card with its layout form (so the V key — which
    // toggles horizontal / stacked / long-division — actually changes the view).
    // The other sections show a question, a per-section figure, then the answer.
    if let Active::Arith(p) = active {
        draw_arith(pm, (cx0, cy0, cw, ch), p, input, layout, feedback);
    } else {
        let q = question_of(active);
        let qscale = fit_scale(&q, cw - 80.0, 5.0);
        text_centered(pm, cxc, cy0 + 50.0, qscale, &q, WHITE);

        // The per-section visual takes the upper band; the answer moves below it.
        let mut answer_y = cy0 + ch * 0.42;
        let mut ans_scale = 7.0;
        let band = (cx0 + 50.0, cy0 + 96.0, cw - 100.0, ch * 0.34);
        match active {
            Active::Geo(g) => {
                draw_geo(pm, band, g);
                answer_y = band.1 + band.3 + 26.0;
                ans_scale = 4.0;
            }
            Active::Shape(s) => {
                draw_shape(pm, band, s, frac);
                answer_y = band.1 + band.3 + 26.0;
                ans_scale = 4.0;
            }
            Active::Unit(u) => {
                draw_unit_prop(pm, band, u.theme);
                if u.duck_jump {
                    draw_unit_gag(pm, band, u.theme, anim);
                }
                answer_y = band.1 + band.3 + 26.0;
                ans_scale = 4.0;
            }
            _ => {}
        }

        let ans = if input.is_empty() { "?".to_string() } else { input.to_string() };
        text_centered(pm, cxc, answer_y, 2.0, "Your answer:", GRAY);
        text_centered(pm, cxc, answer_y + 28.0, ans_scale, &ans, ans_color(input, feedback));
        // Units: name the unit beneath the number, so the answer reads in full.
        if let Active::Unit(u) = active {
            text_centered(pm, cxc, answer_y + 68.0, 1.75, &u.unit_label, theme_col(u.theme));
        }
    }

    // Feedback line, just above the footer.
    if let Some((msg, color)) = match feedback {
        Feedback::Correct => Some(("Correct!", ACCENT)),
        Feedback::Wrong => Some(("Not quite - try again", RED)),
        Feedback::None => None,
    } {
        text_centered(pm, cxc, cy0 + ch - 84.0, 3.0, msg, color);
    }

    let footer = if active.has_strategies() {
        "Enter check    H help    Y why    V view    Esc menu"
    } else {
        "H help    Y why    Enter check    Esc menu"
    };
    text_centered(pm, cxc, cy0 + ch - 36.0, 1.5, footer, GRAY);
}

fn draw_session(pm: &mut Pixmap, app: &App, wf: f32, hf: f32) {
    // A problem-to-problem transition (owned by the core's timing) takes over.
    if app.transition.is_some() {
        draw_transition(pm, app, wf, hf);
        return;
    }

    // Blackboard: the first problem of a session "draws itself on" (later ones
    // are written by the eraser transition).  No overlays during the draw-on.
    if is_chalk() && app.card_progress() < 1.0 && app.help_in == 0.0 && !app.why_active {
        let (w, h) = (pm.width(), pm.height());
        let card = card_frame(w, h, |p| {
            draw_card(p, &app.current, &app.input, app.feedback, app.frac_progress(), app.anim_frame, app.config.layout, wf, hf);
        });
        write_on(pm, card.pixels(), app.card_progress());
        return;
    }

    draw_card(pm, &app.current, &app.input, app.feedback, app.frac_progress(), app.anim_frame, app.config.layout, wf, hf);
    let (cx0, cy0, cw, ch) = card_rect(wf, hf);

    // Deduction Duck's help panel slides up over the card when summoned (H).
    if app.help_in > 0.0 {
        draw_help(pm, app, (cx0, cy0, cw, ch));
    }
    // The "Why am I learning this?" overlay (Y) sits over everything.
    if app.why_active {
        draw_why(pm, app, (cx0, cy0, cw, ch));
    }
}

/// A cheap deterministic hash to [0,1), for the dissolve threshold map.
fn hash01(x: u32, y: u32) -> f32 {
    let mut n = x.wrapping_mul(374761393).wrapping_add(y.wrapping_mul(668265263));
    n = (n ^ (n >> 13)).wrapping_mul(1274126177);
    (n & 0xffff) as f32 / 65535.0
}

/// An opaque premultiplied pixel (alpha 255 → premultiplied == straight).
fn prem(c: Rgb) -> PremultipliedColorU8 {
    PremultipliedColorU8::from_rgba(c[0], c[1], c[2], 255).unwrap_or(PremultipliedColorU8::TRANSPARENT)
}

/// Colour of the typed answer: green/red once checked, else cyan when typed.
fn ans_color(input: &str, feedback: Feedback) -> Rgb {
    match feedback {
        Feedback::Correct => ACCENT,
        Feedback::Wrong => RED,
        Feedback::None if input.is_empty() => GRAY,
        Feedback::None => CYAN,
    }
}

/// Approximate glyph cap height (logical px) for a given text `scale`.
fn glyph_h(scale: f32) -> f32 {
    scale * PX_PER_SCALE * 0.72
}

/// Greedy word-wrap to `max_w` (logical px) at the given `scale`.
fn wrap_words(s: &str, max_w: f32, scale: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in s.split(' ') {
        let trial = if cur.is_empty() { word.to_string() } else { format!("{} {}", cur, word) };
        if !cur.is_empty() && text_width(&trial, scale) > max_w {
            lines.push(std::mem::take(&mut cur));
            cur = word.to_string();
        } else {
            cur = trial;
        }
    }
    lines.push(cur);
    lines
}

// ---------------------------------------------------------------------------
// Deduction Duck — the mascot sprite (his help panel lives in `help`)
// ---------------------------------------------------------------------------

#[derive(Copy, Clone)]
#[allow(dead_code)] // WalkRight/Smash/Awe land with the strategy visuals + cinematic
enum DuckPose {
    Stand,
    WalkRight,
    Smash,
    Awe,
}

const DUCK_H: usize = 7;

// The same ASCII sprites the terminal uses, rendered here as monospaced pixels.
const STAND_A: [&str; DUCK_H] = ["    ___  ", "   [___] ", "     |   ", "    __   ", "  <(o )__", "   (  __)", "   ^^ ^^ "];
const STAND_B: [&str; DUCK_H] = ["    ___  ", "   [___] ", "     |   ", "    __   ", "  <(o )__", "   (  __)", "  ^^ ^^  "];
const WALK_A: [&str; DUCK_H] = ["  ___    ", " [___]   ", "   |     ", "   __    ", "__( o)>  ", "(  __ )  ", "  ^^ ^^  "];
const WALK_B: [&str; DUCK_H] = ["  ___    ", " [___]   ", "   |     ", "   __    ", "__( o)>  ", "(  __ )  ", " ^^ ^^   "];
const SMASH_A: [&str; DUCK_H] = ["  ___  \\ ", " [___]  \\", "   |    O", "   __    ", " <(o )   ", "  (  )   ", "  ^^ ^^  "];
const SMASH_B: [&str; DUCK_H] = ["  ___    ", " [___]   ", "   |     ", "   __  / ", " <(o )O  ", "  (  )   ", "  ^^ ^^  "];
const AWE_A: [&str; DUCK_H] = ["   ___   ", "  [___]  ", "    |    ", "  \\(**)/ ", "   (  )  ", "   |  |  ", "   ^^^^  "];
const AWE_B: [&str; DUCK_H] = [" \\ ___ / ", "  [___]  ", "    |    ", "   (°°)  ", "   (  )  ", "   |  |  ", "   ^^^^  "];

fn duck_sprite(pose: DuckPose, frame: u64) -> &'static [&'static str; DUCK_H] {
    let even = frame % 2 == 0;
    match pose {
        DuckPose::Stand => if even { &STAND_A } else { &STAND_B },
        DuckPose::WalkRight => if even { &WALK_A } else { &WALK_B },
        DuckPose::Smash => if even { &SMASH_A } else { &SMASH_B },
        DuckPose::Awe => if even { &AWE_A } else { &AWE_B },
    }
}

/// Draw the duck in `pose` with its top-left at `(x, y)`; `cell` is one
/// character cell's width in logical px (height is `cell * 1.25`).
fn draw_duck(pm: &mut Pixmap, x: f32, y: f32, pose: DuckPose, frame: u64, cell: f32, color: Rgb) {
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

/// The headline string for whichever section produced the active problem.
fn question_of(active: &Active) -> String {
    match active {
        Active::Arith(p) => p.prompt(),
        Active::Unit(u) => u.question.clone(),
        // Chalk is monochrome, so the colour ("...is purple?") is meaningless —
        // ask about the *shaded* (filled) share instead, to match the figure.
        Active::Shape(s) if is_chalk() => shaded_question(s),
        Active::Shape(s) => s.question(),
        Active::Geo(g) => g.question(),
    }
}

/// The colour-free, chalk-mode framing of a fraction/percent question.
fn shaded_question(s: &FractionProblem) -> String {
    let kind = match s.mode {
        Mode::Fraction => "fraction",
        Mode::Percent => "percent",
    };
    format!("What {} of the {} is shaded?", kind, s.shape_word())
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
    let chalk = chalk();
    // Chalk weight scales with the glyph size so small text (menu, footers) stays
    // crisp while big formulas read as chunky chalk.  Wobble only the large text —
    // jittering small UI text just makes it look fuzzy.
    let bold = px * 0.03;
    let wob = if chalk && px > 90.0 { px * 0.05 } else { 0.0 };
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
                if chalk {
                    // Dilate by a size-proportional amount for a chalk stroke.
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
    let color = themed(color);
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

fn lerp_rgb(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2])]
}

/// Map a frontend-neutral [`crate::color::Color`] to a pixel RGB triple.
fn core_col(c: crate::color::Color) -> Rgb {
    use crate::color::Color as C;
    match c {
        C::LightCyan => [120, 220, 240],
        C::LightGreen => [120, 220, 150],
        C::LightYellow => [240, 215, 110],
        C::LightMagenta => [210, 140, 230],
        C::LightBlue => [120, 170, 240],
        C::LightRed => [235, 110, 110],
        C::Cyan => [80, 180, 200],
        C::Green => [90, 190, 120],
        C::Rgb(r, g, b) => [r, g, b],
    }
}

/// Build a closed polygon path from logical points (scaled to device pixels).
fn poly(points: &[(f32, f32)]) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    let (x0, y0) = points[0];
    pb.move_to(x0 * SSF, y0 * SSF);
    for &(x, y) in &points[1..] {
        pb.line_to(x * SSF, y * SSF);
    }
    pb.close();
    pb.finish()
}

/// Fill a closed path (device coords); supersampling supplies the smooth edges.
fn fill_path(pm: &mut Pixmap, path: &tiny_skia::Path, color: Rgb) {
    let mut paint = Paint::default();
    paint.set_color(col(color));
    paint.anti_alias = false;
    pm.fill_path(path, &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
}

/// A wavy horizontal line from `x0` to `x1` at height `y`, amplitude `amp`.
fn wavy_line(pm: &mut Pixmap, x0: f32, x1: f32, y: f32, amp: f32, color: Rgb, width: f32) {
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

/// Stroke a path already expressed in **device** (supersampled) coordinates.
fn stroke(pm: &mut Pixmap, path: &tiny_skia::Path, color: Rgb, width: f32) {
    let mut paint = Paint::default();
    paint.set_color(col(color));
    // Anti-aliasing stays off here on purpose: tiny-skia's AA hairline rasteriser
    // panics on some inputs.  Smooth edges instead come from rendering at `SS×`
    // and box-averaging back down (see mod.rs) — AA without the buggy path.
    paint.anti_alias = false;
    let mut s = Stroke::default();
    s.width = width * SSF * chalk_mul();
    pm.stroke_path(path, &paint, &s, Transform::identity(), None);
}

/// Stroke / chalk thickening factor for the active theme (chalk is fatter).
fn chalk_mul() -> f32 {
    if is_chalk() {
        2.4
    } else {
        1.0
    }
}

/// Whether a chalk theme is active — for the bold/wobble paths.
fn chalk() -> bool {
    is_chalk()
}

fn line(pm: &mut Pixmap, x0: f32, y0: f32, x1: f32, y1: f32, color: Rgb, width: f32) {
    // Skip degenerate (zero-length) segments — they have no direction to stroke.
    if (x0 - x1).abs() < 0.5 && (y0 - y1).abs() < 0.5 {
        return;
    }
    let (dx, dy) = (x1 - x0, y1 - y0);
    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    let mut pb = PathBuilder::new();
    pb.move_to(x0 * SSF, y0 * SSF);
    if chalk() {
        // Hand-drawn wobble: break the line into segments whose interior joints
        // wander a little perpendicular to it (endpoints stay put).
        let (px, py) = (-dy / len, dx / len); // perpendicular unit
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
