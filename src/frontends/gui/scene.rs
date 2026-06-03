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

use crate::app::{App, Feedback, Screen, TeacherView, ANECDOTE_MAX};
use crate::fraction::FractionProblem;
use crate::geometry::{GeometryProblem, GeoShape};
use crate::section::Active;
use crate::shapes::Shape;
use crate::topic::Topic;
use crate::units::Theme;

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
        Screen::Settings => draw_settings(&mut pm, app, wf, hf),
        Screen::Stats => draw_stats(&mut pm, app, wf, hf),
        Screen::Teacher => draw_teacher(&mut pm, app, wf, hf),
        Screen::Cinematic => draw_cinematic(&mut pm, app, wf, hf),
        Screen::Experiment => draw_experiment(&mut pm, app, wf, hf),
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
    let box_s = scale * 9.0;
    let label_dx = box_s + scale * 7.0; // toggle rows indent past their checkbox
    // Center the whole block on the widest row; the highlight bar matches that
    // width so the selection never clips, and the drawn checkbox keeps every row
    // a fixed width (no reflow when a section is toggled on/off).
    let row_w = |r: &MenuRow| match r {
        MenuRow::Check(_, l) => label_dx + text_width(l, scale),
        MenuRow::Text(t) => text_width(t, scale),
    };
    let content_w = rows.iter().map(row_w).fold(0.0_f32, f32::max);
    let x = (wf - content_w) / 2.0;
    let pad = 14.0;
    for (i, row) in rows.iter().enumerate() {
        let y = top + i as f32 * row_h;
        if i == app.menu_index {
            fill(pm, x - pad, y - 5.0, content_w + pad * 2.0, row_h - 2.0, SEL_BG);
        }
        let color = if i == app.menu_index { WHITE } else { GRAY };
        match row {
            MenuRow::Check(on, label) => {
                stroke_rect(pm, x, y, box_s, box_s, 2.0, color);
                if *on {
                    fill(pm, x + 3.0, y + 3.0, box_s - 6.0, box_s - 6.0, ACCENT);
                }
                text(pm, x + label_dx, y, scale, label, color);
            }
            MenuRow::Text(t) => text(pm, x, y, scale, t, color),
        }
    }
    text_centered(pm, wf / 2.0, hf - 36.0, 1.5, "Up / Down move    Enter select    Q quit", GRAY);
}

/// A menu row: a toggle with a drawn checkbox, or a plain text/setting row.
enum MenuRow {
    Check(bool, String),
    Text(String),
}

/// The menu rows, in the same order as the core's `menu_index`.
fn menu_rows(app: &App) -> Vec<MenuRow> {
    let grade = if app.menu_grade == 0 { "K".to_string() } else { app.menu_grade.to_string() };
    let ops = ["Add", "Subtract", "Multiply", "Divide"];
    let mut v = vec![
        MenuRow::Text(format!("Student:  < {} >", app.roster.current().name)),
        MenuRow::Text(format!("Grade level:  < {} >", grade)),
    ];
    for (i, name) in ops.iter().enumerate() {
        v.push(MenuRow::Check(app.menu_ops[i], name.to_string()));
    }
    v.push(MenuRow::Check(app.menu_units, "Units of Measure".to_string()));
    v.push(MenuRow::Check(app.menu_fractions, "Fractions".to_string()));
    v.push(MenuRow::Check(app.menu_percents, "Percentages".to_string()));
    v.push(MenuRow::Check(app.menu_geometry, "Geometry".to_string()));
    v.push(MenuRow::Text(format!("Show problems:  {}", app.config.layout.name())));
    v.push(MenuRow::Text("Settings...".to_string()));
    v.push(MenuRow::Text("My Progress...".to_string()));
    v.push(MenuRow::Text("Teacher Area...".to_string()));
    v.push(MenuRow::Text(">  Start Practice".to_string()));
    v.push(MenuRow::Text(">  Start Challenge".to_string()));
    v.push(MenuRow::Text(">  Experimentation".to_string()));
    v
}

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

// ---------------------------------------------------------------------------
// Problem-to-problem transitions — capture both cards, animate between them
// ---------------------------------------------------------------------------

/// Render the answered card melting into the next one, at the core's `progress`.
/// The two frames are captured fresh each tick (both states are static for the
/// transition's duration), then blended per the chosen effect.
fn draw_transition(pm: &mut Pixmap, app: &App, wf: f32, hf: f32) {
    let Some(phase) = &app.transition else { return };
    let (w, h) = (pm.width(), pm.height());
    let from = card_frame(w, h, |p| {
        draw_card(p, &app.current, &app.input, Feedback::Correct, 1.0, app.anim_frame, app.config.layout, wf, hf);
    });
    let to = card_frame(w, h, |p| {
        if let Some(next) = &app.pending {
            draw_card(p, next, "", Feedback::None, 0.0, app.anim_frame, app.config.layout, wf, hf);
        }
    });
    blend_transition(pm, &from, &to, phase.effect, phase.progress.clamp(0.0, 1.0));
}

/// Render a full-window card frame into a fresh pixmap (BG-filled) via `draw`.
fn card_frame(w: u32, h: u32, draw: impl FnOnce(&mut Pixmap)) -> Pixmap {
    let mut p = Pixmap::new(w, h).unwrap_or_else(|| Pixmap::new(1, 1).unwrap());
    p.fill(col(BG));
    draw(&mut p);
    p
}

/// Blend `from`→`to` (device-resolution pixmaps) into `pm` per `effect`/`t`.
fn blend_transition(pm: &mut Pixmap, from: &Pixmap, to: &Pixmap, effect: crate::transition::Effect, t: f32) {
    use crate::transition::{Effect, Kind};
    let (w, h) = (pm.width(), pm.height());
    let (wf, hf) = (w as f32, h as f32);
    let fpx = from.pixels();
    let tpx = to.pixels();
    let out = pm.pixels_mut();

    // A per-pixel "reveal threshold": once `t` passes it, the pixel shows `to`.
    let threshold = |kind: Kind, x: f32, y: f32| -> f32 {
        let (nx, ny) = (x / wf, y / hf);
        match kind {
            Kind::Wipe => nx,
            Kind::Curtain => 1.0 - (nx - 0.5).abs() * 2.0, // edges last, centre first
            Kind::Dissolve => hash01(x as u32, y as u32),
            Kind::Blinds => (y / (hf / 8.0)).fract(),
            Kind::Circle => {
                let (dx, dy) = (nx - 0.5, ny - 0.5);
                (dx * dx + dy * dy).sqrt() / 0.7071
            }
            Kind::Slide => return nx, // handled specially below; unused here
            Kind::Diagonal => (nx + ny) / 2.0,
        }
    };

    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            let src = match effect {
                // Slide: the new card slides in from the right, covering the old.
                Effect::Reveal(Kind::Slide) => {
                    let b = (wf * (1.0 - t)) as u32;
                    if x >= b {
                        let sx = x - b;
                        tpx[(y * w + sx.min(w - 1)) as usize]
                    } else {
                        fpx[i]
                    }
                }
                Effect::Reveal(kind) => {
                    if t >= threshold(kind, x as f32, y as f32) {
                        tpx[i]
                    } else {
                        fpx[i]
                    }
                }
                // Particle effects use a crossfade backdrop; the action is then
                // overlaid below (the new card materialises behind it).
                _ => {
                    out[i] = lerp_px(fpx[i], tpx[i], t);
                    continue;
                }
            };
            out[i] = src;
        }
    }

    // The showy effects draw their particles on top of the crossfade backdrop.
    let f = t * 40.0; // elapsed "frames", matching the terminal's effect duration
    match effect {
        Effect::Explode => overlay_glyphs(pm, from, t, true),
        Effect::Swirl => overlay_glyphs(pm, from, t, false),
        Effect::Fireworks => overlay_fireworks(pm, f),
        Effect::Starburst => overlay_starburst(pm, f),
        Effect::AlienShips => overlay_movers(pm, f, true),
        Effect::Asteroids => overlay_movers(pm, f, false),
        Effect::Reveal(_) => {}
    }
}

/// Linear blend of two premultiplied (opaque) pixels.
fn lerp_px(a: PremultipliedColorU8, b: PremultipliedColorU8, t: f32) -> PremultipliedColorU8 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    PremultipliedColorU8::from_rgba(m(a.red(), b.red()), m(a.green(), b.green()), m(a.blue(), b.blue()), 255).unwrap_or(a)
}

/// A cheap deterministic hash to [0,1), for the dissolve threshold map.
fn hash01(x: u32, y: u32) -> f32 {
    let mut n = x.wrapping_mul(374761393).wrapping_add(y.wrapping_mul(668265263));
    n = (n ^ (n >> 13)).wrapping_mul(1274126177);
    (n & 0xffff) as f32 / 65535.0
}

// ---------------------------------------------------------------------------
// Particle overlays (device-resolution; deterministic so they need no stored
// state — re-derived each tick, in lockstep with the core's progress)
// ---------------------------------------------------------------------------

const FIRE_C: [Rgb; 6] = [[235, 96, 96], [240, 210, 90], [120, 220, 150], [120, 200, 230], [210, 140, 230], [240, 240, 245]];
const STAR_C: [Rgb; 4] = [[240, 240, 245], [120, 200, 230], [240, 215, 110], [150, 156, 172]];
const ALIEN_C: [Rgb; 4] = [[120, 220, 150], [120, 200, 230], [210, 140, 230], [90, 190, 120]];
const ROCK_C: [Rgb; 3] = [[150, 130, 110], [120, 110, 100], [95, 90, 85]];

/// A deterministic pseudo-random in [0,1) for particle `i`.
fn rnd(i: u32) -> f32 {
    hash01(i.wrapping_mul(2654435761), 0x9e3779b9)
}

/// Fill an opaque device-pixel rect, clipped to the pixmap.
fn fill_dev(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, color: Rgb) {
    let (pw, ph) = (pm.width(), pm.height());
    let x0 = (x.floor() as i32).max(0);
    let y0 = (y.floor() as i32).max(0);
    let x1 = ((x + w).ceil() as i32).min(pw as i32);
    let y1 = ((y + h).ceil() as i32).min(ph as i32);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let Some(px) = PremultipliedColorU8::from_rgba(color[0], color[1], color[2], 255) else { return };
    let data = pm.pixels_mut();
    for yy in y0..y1 {
        let row = (yy as u32 * pw) as usize;
        for xx in x0..x1 {
            data[row + xx as usize] = px;
        }
    }
}

fn dot_dev(pm: &mut Pixmap, x: f32, y: f32, s: f32, color: Rgb) {
    fill_dev(pm, x - s / 2.0, y - s / 2.0, s, s, color);
}

fn sample_dev(pm: &Pixmap, x: f32, y: f32) -> Rgb {
    let (pw, ph) = (pm.width(), pm.height());
    let xi = (x as i32).clamp(0, pw as i32 - 1) as u32;
    let yi = (y as i32).clamp(0, ph as i32 - 1) as u32;
    let p = pm.pixels()[(yi * pw + xi) as usize];
    [p.red(), p.green(), p.blue()]
}

/// Near-background tiles are invisible, so we skip flinging them about.
fn is_bg(c: Rgb) -> bool {
    let d = |a: u8, b: u8| (a as i32 - b as i32).abs();
    d(c[0], BG[0]) + d(c[1], BG[1]) + d(c[2], BG[2]) < 22
}

/// Explode / Swirl: shatter the outgoing card into a grid of coloured tiles that
/// either fly out ballistically (with gravity) or spiral into the centre.
fn overlay_glyphs(pm: &mut Pixmap, from: &Pixmap, p: f32, explode: bool) {
    let (w, h) = (pm.width() as f32, pm.height() as f32);
    let (cx, cy) = (w / 2.0, h / 2.0);
    let (gw, gh) = (16u32, 10u32);
    let (tw, th) = (w / gw as f32, h / gh as f32);
    let f = p * 40.0;
    for j in 0..gh {
        for i in 0..gw {
            let (x0, y0) = ((i as f32 + 0.5) * tw, (j as f32 + 0.5) * th);
            let color = sample_dev(from, x0, y0);
            if is_bg(color) {
                continue;
            }
            let id = j * gw + i;
            let (px, py, size) = if explode {
                let (dx, dy) = (x0 - cx, y0 - cy);
                let nrm = (dx * dx + dy * dy).sqrt().max(1.0);
                let speed = (5.0 + rnd(id) * 5.0) * SSF;
                let pxp = x0 + dx / nrm * speed * f + (rnd(id + 99) - 0.5) * 40.0;
                let pyp = y0 + dy / nrm * speed * f + 0.5 * 0.5 * SSF * f * f;
                (pxp, pyp, tw.min(th) * (1.0 - p))
            } else {
                let (dx, dy) = (x0 - cx, y0 - cy);
                let (r0, a0) = ((dx * dx + dy * dy).sqrt(), dy.atan2(dx));
                let ang = a0 + (3.0 + rnd(id)) * p;
                let rad = r0 * (1.0 - p);
                (cx + rad * ang.cos(), cy + rad * ang.sin(), tw.min(th) * (1.0 - p * 0.7))
            };
            if size >= 1.0 {
                fill_dev(pm, px - size / 2.0, py - size / 2.0, size, size, color);
            }
        }
    }
}

/// Fireworks: rockets rise from the base, then burst into radial sparks.
fn overlay_fireworks(pm: &mut Pixmap, f: f32) {
    let (w, h) = (pm.width() as f32, pm.height() as f32);
    let bottom = h - 2.0;
    let n = 5u32;
    let grav = 0.03 * SSF;
    for k in 0..n {
        let rx = w * (k as f32 + 0.5) / n as f32;
        let launch = k as f32 * 4.0;
        if f < launch {
            continue;
        }
        let local = f - launch;
        let rise = 16.0;
        let apex_y = h * 0.26 + rnd(k) * h * 0.12;
        if local <= rise {
            let y = bottom - (bottom - apex_y) * (local / rise);
            dot_dev(pm, rx, y, 4.0 * SSF, FIRE_C[k as usize % FIRE_C.len()]);
        } else {
            let bt = local - rise;
            if bt > 16.0 {
                continue;
            }
            for s in 0..14u32 {
                let ang = s as f32 * std::f32::consts::TAU / 14.0;
                let spd = (2.5 + rnd(s)) * SSF;
                let px = rx + ang.cos() * spd * bt;
                let py = apex_y + ang.sin() * spd * bt + 0.5 * grav * bt * bt;
                dot_dev(pm, px, py, 3.0 * SSF, FIRE_C[s as usize % FIRE_C.len()]);
            }
        }
    }
}

/// Starburst: stars streak straight out from the centre.
fn overlay_starburst(pm: &mut Pixmap, f: f32) {
    let (w, h) = (pm.width() as f32, pm.height() as f32);
    let (cx, cy) = (w / 2.0, h / 2.0);
    for i in 0..48u32 {
        let ang = i as f32 * std::f32::consts::TAU / 48.0 + rnd(i) * 0.3;
        let spd = (6.0 + rnd(i + 5) * 6.0) * SSF;
        let (px, py) = (cx + ang.cos() * spd * f, cy + ang.sin() * spd * f);
        dot_dev(pm, px, py, (2.0 + rnd(i) * 2.0) * SSF, STAR_C[i as usize % STAR_C.len()]);
    }
}

/// Alien ships cruise across, or asteroids tumble across with a debris trail.
fn overlay_movers(pm: &mut Pixmap, f: f32, ufo: bool) {
    let (w, h) = (pm.width() as f32, pm.height() as f32);
    let dur = 40.0;
    let n = if ufo { 4u32 } else { 6u32 };
    for k in 0..n {
        if ufo {
            let lane = h * (k as f32 + 1.0) / (n as f32 + 1.0);
            let x = -120.0 * SSF + (w + 240.0 * SSF) * (f / dur);
            let by = lane + (f * 0.25 + k as f32).sin() * 10.0 * SSF;
            draw_ufo(pm, x, by, ALIEN_C[k as usize % ALIEN_C.len()]);
        } else {
            let start_x = w + 100.0 * SSF;
            let sy = h * (rnd(k) * 0.8 + 0.1);
            let (vx, vy) = (-(8.0 + rnd(k + 3) * 6.0) * SSF, (rnd(k + 7) - 0.5) * 6.0 * SSF);
            draw_rock(pm, start_x + vx * f, sy + vy * f, k);
        }
    }
}

fn draw_ufo(pm: &mut Pixmap, x: f32, y: f32, c: Rgb) {
    let bw = 70.0 * SSF;
    let bh = 14.0 * SSF;
    fill_dev(pm, x - bw / 2.0, y, bw, bh, c); // saucer body
    fill_dev(pm, x - bw * 0.28, y - bh * 0.8, bw * 0.56, bh, [230, 240, 245]); // dome
    for d in 0..3 {
        dot_dev(pm, x + (d as f32 - 1.0) * bw * 0.28, y + bh, 5.0 * SSF, [250, 240, 120]); // lights
    }
}

fn draw_rock(pm: &mut Pixmap, x: f32, y: f32, k: u32) {
    let s = (16.0 + rnd(k) * 8.0) * SSF;
    fill_dev(pm, x - s / 2.0, y - s / 2.0, s, s, ROCK_C[k as usize % ROCK_C.len()]);
    fill_dev(pm, x - s * 0.32, y - s * 0.55, s * 0.64, s * 0.5, ROCK_C[(k as usize + 1) % ROCK_C.len()]);
    for t in 1..4u32 {
        dot_dev(pm, x + t as f32 * 12.0 * SSF, y - (rnd(k + t) - 0.5) * 8.0 * SSF, (4.0 - t as f32) * SSF, [90, 80, 70]);
    }
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

// ---------------------------------------------------------------------------
// Arithmetic — the three layouts (horizontal, stacked, long-division house)
// ---------------------------------------------------------------------------

fn draw_arith(pm: &mut Pixmap, (cx0, cy0, cw, ch): (f32, f32, f32, f32), p: &crate::problem::Problem, input: &str, layout: crate::config::Layout, feedback: Feedback) {
    use crate::config::Layout;
    use crate::problem::Op;
    let accent = core_col(p.accent);
    let acol = ans_color(input, feedback);
    let cyc = cy0 + ch * 0.46;
    match layout {
        Layout::Vertical if p.op == Op::Div => draw_division_house(pm, cx0 + cw / 2.0, cyc, cw, p, input, accent, acol),
        Layout::Vertical => draw_vertical(pm, cx0 + cw / 2.0, cyc, cw, p, input, accent, acol),
        _ => draw_horizontal(pm, cx0 + cw / 2.0, cyc, cw, p, input, accent, acol),
    }
}

/// `a op b = answer` on one line, the answer to the right of the `=`.
fn draw_horizontal(pm: &mut Pixmap, cxc: f32, cyc: f32, cw: f32, p: &crate::problem::Problem, input: &str, accent: Rgb, acol: Rgb) {
    let prompt = p.prompt();
    let answer = if input.is_empty() { "?" } else { input };
    let scale = fit_scale(&format!("{}  {}", prompt, answer), cw - 120.0, 6.0);
    let gap = scale * 6.0;
    let (pw, aw) = (text_width(&prompt, scale), text_width(answer, scale));
    let x = cxc - (pw + gap + aw) / 2.0;
    let y = cyc - glyph_h(scale) / 2.0;
    text(pm, x, y, scale, &prompt, accent);
    text(pm, x + pw + gap, y, scale, answer, acol);
}

/// The stacked column form: operands right-aligned, a divider, the answer below.
fn draw_vertical(pm: &mut Pixmap, cxc: f32, cyc: f32, cw: f32, p: &crate::problem::Problem, input: &str, accent: Rgb, acol: Rgb) {
    let (a, b) = (p.a.to_string(), p.b.to_string());
    let ans = if input.is_empty() { "?".to_string() } else { input.to_string() };
    let op = p.op.symbol().to_string();

    let widest = [&a, &b, &ans].iter().map(|s| s.len()).max().unwrap_or(1);
    let scale = fit_scale(&format!("{} {}", op, "0".repeat(widest)), cw - 200.0, 6.0);

    let field = [&a, &b, &ans].iter().map(|s| text_width(s, scale)).fold(0.0_f32, f32::max);
    let op_col = text_width(&op, scale) + scale * 8.0;
    let total_w = op_col + field;
    let x0 = cxc - total_w / 2.0;
    let field_right = x0 + total_w;
    let right = |pm: &mut Pixmap, s: &str, y: f32, c: Rgb| text(pm, field_right - text_width(s, scale), y, scale, s, c);

    // Spacing in glyph-relative terms so the divider keeps a clear margin from
    // both the operand above it and the answer below (no overlap with the digits).
    let px = scale * PX_PER_SCALE;
    let gbot = px * 0.80; // a glyph's visual bottom, below its line top
    let gtop = px * 0.08; // a glyph's visual top, below its line top
    let margin = px * 0.24; // clear gap on each side of the divider
    let row = px * 1.02; // operand-to-operand line pitch

    let total_h = row + 2.0 * gbot + 2.0 * margin - gtop;
    let y1 = cyc - total_h / 2.0;
    let y2 = y1 + row;
    let y_div = y2 + gbot + margin;
    let y3 = y_div + margin - gtop;

    right(pm, &a, y1, accent);
    text(pm, x0, y2, scale, &op, accent);
    right(pm, &b, y2, accent);
    line(pm, x0, y_div, field_right, y_div, accent, 3.0);
    right(pm, &ans, y3, acol);
}

/// Long-division "house": quotient on the roof, divisor outside, dividend inside.
fn draw_division_house(pm: &mut Pixmap, cxc: f32, cyc: f32, cw: f32, p: &crate::problem::Problem, input: &str, accent: Rgb, acol: Rgb) {
    let dividend = p.a.to_string();
    let divisor = p.b.to_string();
    let quotient = if input.is_empty() { "?".to_string() } else { input.to_string() };
    let scale = fit_scale(&format!("{}  {}", divisor, dividend), cw - 200.0, 6.0);
    let gh = glyph_h(scale);
    let pad = scale * 8.0;

    let (wd, wq, wv) = (text_width(&dividend, scale), text_width(&quotient, scale), text_width(&divisor, scale));
    let total_w = wv + pad + wd;
    let x0 = cxc - total_w / 2.0;
    let wall_x = x0 + wv + pad * 0.5;
    let dividend_x = wall_x + pad * 0.5;
    let quotient_x = dividend_x + (wd - wq);

    let total_h = 2.0 * gh + gh * 0.3;
    let q_y = cyc - total_h / 2.0;
    let roof_y = q_y + gh + gh * 0.1;
    let dvd_y = roof_y + gh * 0.2;

    text(pm, quotient_x, q_y, scale, &quotient, acol); // quotient above the roof
    text(pm, x0, dvd_y, scale, &divisor, accent); // divisor left of the wall
    text(pm, dividend_x, dvd_y, scale, &dividend, accent); // dividend inside
    // The house: a left wall and a roof over the dividend.
    line(pm, wall_x, roof_y, wall_x, dvd_y + gh, accent, 3.0);
    line(pm, wall_x, roof_y, dividend_x + wd, roof_y, accent, 3.0);
}

/// Approximate glyph cap height (logical px) for a given text `scale`.
fn glyph_h(scale: f32) -> f32 {
    scale * PX_PER_SCALE * 0.72
}

// ---------------------------------------------------------------------------
// Settings — the per-grade number-range knobs
// ---------------------------------------------------------------------------

fn draw_settings(pm: &mut Pixmap, app: &App, wf: f32, hf: f32) {
    let cxc = wf / 2.0;
    text_centered(pm, cxc, 50.0, 2.5, "SETTINGS - NUMBER RANGES", CYAN);
    text_centered(pm, cxc, 96.0, 1.5, "Tune how big the numbers get for each grade.", GRAY);

    let g = app.settings_grade;
    let r = app.config.range(g);
    let rows: [(&str, String); 5] = [
        ("Grade", format!("< {} >", grade_name(g))),
        ("Add / Subtract up to", format!("< {} >", r.add_max)),
        ("Multiply factors up to", format!("< {} >", r.mul_max)),
        ("Divide numbers up to", format!("< {} >", r.div_max)),
        ("Allow negative answers", format!("< {} >", if r.allow_negative { "Yes" } else { "No" })),
    ];

    let block_w = 560.0;
    let x = (wf - block_w) / 2.0;
    let value_x = x + 360.0;
    let row_h = 52.0;
    let top = 162.0;
    for (i, (label, value)) in rows.iter().enumerate() {
        let y = top + i as f32 * row_h;
        let selected = i == app.settings_field;
        if selected {
            fill(pm, x - 16.0, y - 8.0, block_w + 32.0, 40.0, SEL_BG);
        }
        text(pm, x, y, 1.75, label, if selected { WHITE } else { GRAY });
        text(pm, value_x, y, 1.75, value, YELLOW);
    }

    text_centered(pm, cxc, hf - 44.0, 1.5, "Up / Down choose    Left / Right change    Esc save & back", GRAY);
}

fn grade_name(grade: u8) -> String {
    if grade == 0 {
        "K".to_string()
    } else {
        grade.to_string()
    }
}

// ---------------------------------------------------------------------------
// My Progress — a celebratory, comparison-free breakdown
// ---------------------------------------------------------------------------

fn draw_stats(pm: &mut Pixmap, app: &App, wf: f32, hf: f32) {
    let s = app.roster.current();
    let total = s.grand_total();
    let cxc = wf / 2.0;

    text_centered(pm, cxc, 34.0, 2.5, &format!("{}'s Math Journey", s.name), CYAN);
    // The big celebratory total — every kind of problem counts.
    text_centered(pm, cxc, 78.0, 8.0, &total.to_string(), ACCENT);
    text_centered(pm, cxc, 178.0, 2.0, "problems solved!", WHITE);

    // A friendly bar per section (one per Topic), scaled to the student's own best.
    let rows: Vec<(&str, u32)> = Topic::ALL.iter().map(|&t| (t.short(), s.solved_for(t))).collect();
    let max = rows.iter().map(|(_, n)| *n).max().unwrap_or(0).max(1) as f32;
    const TRACK: Rgb = [44, 48, 64];
    let block_w = 520.0;
    let x = (wf - block_w) / 2.0;
    let bar_x = x + 150.0;
    let bar_w = block_w - 150.0 - 60.0;
    let row_h = 30.0;
    let mut y = 224.0;
    for (label, n) in &rows {
        text(pm, x, y, 1.4, label, WHITE);
        fill(pm, bar_x, y + 2.0, bar_w, 14.0, TRACK);
        fill(pm, bar_x, y + 2.0, bar_w * (*n as f32 / max), 14.0, ACCENT);
        text(pm, bar_x + bar_w + 12.0, y, 1.4, &n.to_string(), YELLOW);
        y += row_h;
    }

    y += 8.0;
    if s.best_streak > 0 {
        text_centered(pm, cxc, y, 1.6, &format!("\u{2605} Best streak: {} in a row!", s.best_streak), YELLOW);
        y += 30.0;
    }
    text_centered(pm, cxc, y, 1.5, encouragement(total), CYAN);

    text_centered(pm, cxc, hf - 44.0, 1.5, "Every problem makes you stronger.    Esc: back", GRAY);
}

/// A warm message keyed only to the student's own total — never a comparison.
fn encouragement(total: u32) -> &'static str {
    match total {
        0 => "Your journey starts with the very first problem. Let's go!",
        1..=9 => "You're off to a great start - keep that curiosity going!",
        10..=49 => "Look at you go! Your brain is getting stronger every day.",
        50..=149 => "Fantastic effort - you're becoming a real problem-solver!",
        150..=499 => "Incredible dedication. You should be proud of yourself!",
        _ => "You're a math powerhouse. Deduction Duck is so proud!",
    }
}

// ---------------------------------------------------------------------------
// Teacher Area — password gate, Why? examples editor, records admin
// ---------------------------------------------------------------------------

/// Short section headers for the records table, in [`Topic::ALL`] order.
const REC_COLS: [&str; 8] = ["Add", "Sub", "Mul", "Div", "Un", "Fr", "Pct", "Geo"];

fn draw_teacher(pm: &mut Pixmap, app: &App, wf: f32, hf: f32) {
    if app.teacher_authed {
        draw_teacher_manage(pm, app, wf, hf);
    } else {
        draw_teacher_login(pm, app, wf, hf);
    }
}

fn draw_teacher_login(pm: &mut Pixmap, app: &App, wf: f32, hf: f32) {
    let cxc = wf / 2.0;
    text_centered(pm, cxc, 80.0, 3.0, "TEACHER AREA", CYAN);

    let setting = !app.config.has_teacher_password();
    let prompt = if setting { "Create a teacher password:" } else { "Enter teacher password:" };
    text_centered(pm, cxc, 168.0, 1.75, prompt, WHITE);

    let masked: String = "*".repeat(app.teacher_pw.chars().count());
    text_centered(pm, cxc, 208.0, 2.5, &format!("{}_", masked), ACCENT);

    if setting {
        text_centered(pm, cxc, 268.0, 1.4, "(Kids can't reach these tools without it.)", GRAY);
    }
    if let Some(msg) = &app.teacher_msg {
        text_centered(pm, cxc, 308.0, 1.5, msg, YELLOW);
    }
    text_centered(pm, cxc, hf - 48.0, 1.5, "Enter: continue    Esc: back", GRAY);
}

fn draw_teacher_manage(pm: &mut Pixmap, app: &App, wf: f32, hf: f32) {
    // Tabs.
    let s = 1.6;
    let (l0, l1) = ("Why? Examples", "Student Records");
    let (w0, w1, gap) = (text_width(l0, s) + 28.0, text_width(l1, s) + 28.0, 24.0);
    let tx = (wf - (w0 + gap + w1)) / 2.0;
    teacher_tab(pm, tx, 30.0, s, l0, app.teacher_view == TeacherView::Anecdotes);
    teacher_tab(pm, tx + w0 + gap, 30.0, s, l1, app.teacher_view == TeacherView::Records);

    match app.teacher_view {
        TeacherView::Anecdotes => draw_teacher_anecdotes(pm, app, wf, hf),
        TeacherView::Records => draw_teacher_records(pm, app, wf, hf),
    }

    // Locality + any status message, just above the footer.
    text_centered(pm, wf / 2.0, hf - 92.0, 1.4, &format!("Units locality: {}    (L: change)", app.config.locality.name()), CYAN);
    if let Some(msg) = &app.teacher_msg {
        text_centered(pm, wf / 2.0, hf - 62.0, 1.5, msg, ACCENT);
    }
}

/// Draw one tab box; the active one gets the selection fill.
fn teacher_tab(pm: &mut Pixmap, x: f32, y: f32, scale: f32, label: &str, active: bool) {
    if active {
        fill(pm, x, y - 6.0, text_width(label, scale) + 28.0, 34.0, SEL_BG);
    }
    text(pm, x + 14.0, y, scale, label, if active { WHITE } else { GRAY });
}

fn draw_teacher_anecdotes(pm: &mut Pixmap, app: &App, wf: f32, hf: f32) {
    let cxc = wf / 2.0;
    let topic = Topic::ALL[app.teacher_topic];
    text_centered(pm, cxc, 96.0, 1.75, &format!("Section:  < {} >", topic.name()), WHITE);
    text_centered(pm, cxc, 138.0, 1.3, "Your own real-life examples shown in the Why? panel:", GRAY);

    let items = app.why_extras.items(topic);
    let x = (wf - 640.0) / 2.0;
    let list_bottom = if app.teacher_adding { hf - 280.0 } else { hf - 120.0 };
    let mut y = 176.0;
    if items.is_empty() {
        text(pm, x + 16.0, y, 1.4, "(none yet - press A to add one)", GRAY);
    } else {
        for it in items {
            if y > list_bottom {
                break;
            }
            text(pm, x, y, 1.4, &format!("- {}", clip_chars(it, 78)), WHITE);
            y += 26.0;
        }
    }

    if app.teacher_adding {
        let (bx, bw, bh) = (x, 640.0, 150.0);
        let by = hf - 250.0;
        fill(pm, bx, by, bw, bh, CARD_BG);
        stroke_rect(pm, bx, by, bw, bh, 2.0, YELLOW);
        text(pm, bx + 12.0, by - 26.0, 1.4, "New example", YELLOW);

        // Echo the typed text wrapped to the box, with a caret; keep the tail in view.
        let lines = wrap_words(&format!("{}_", app.teacher_text), bw - 28.0, 1.4);
        let rows = ((bh - 24.0) / 24.0) as usize;
        let start = lines.len().saturating_sub(rows);
        for (i, line) in lines[start..].iter().enumerate() {
            text(pm, bx + 14.0, by + 12.0 + i as f32 * 24.0, 1.4, line, WHITE);
        }

        text(pm, bx, by + bh + 8.0, 1.3, "Enter: save    Esc: cancel", GRAY);
        text_right(pm, bx + bw, by + bh + 8.0, 1.3, &format!("{}/{}", app.teacher_text.chars().count(), ANECDOTE_MAX), GRAY);
    } else {
        text_centered(pm, cxc, hf - 30.0, 1.4, "< > section    A: add    Tab: records    Esc: log out", GRAY);
    }
}

fn draw_teacher_records(pm: &mut Pixmap, app: &App, wf: f32, hf: f32) {
    let (name_w, col_w, total_w, lock_w) = (150.0, 52.0, 80.0, 70.0);
    let table_w = name_w + 8.0 * col_w + total_w + lock_w;
    let x0 = (wf - table_w) / 2.0;
    let scale = 1.3;
    // Right edge of section column `i` (with a little padding inside the cell).
    let col_right = |i: usize| x0 + name_w + (i as f32 + 1.0) * col_w - 8.0;

    // Header row.
    let hy = 100.0;
    text(pm, x0, hy, scale, "Student", GRAY);
    for (i, c) in REC_COLS.iter().enumerate() {
        if i == app.teacher_topic {
            fill(pm, x0 + name_w + i as f32 * col_w, hy - 4.0, col_w, 24.0, YELLOW);
        }
        let color = if i == app.teacher_topic { CARD_BG } else { GRAY };
        text_right(pm, col_right(i), hy, scale, c, color);
    }
    text_right(pm, x0 + name_w + 8.0 * col_w + total_w - 8.0, hy, scale, "Total", GRAY);
    text_right(pm, x0 + table_w - 8.0, hy, scale, "Lock", GRAY);

    // Student rows, scrolled to keep the selection visible.
    let students = &app.roster.students;
    let top = 132.0;
    let row_h = 26.0;
    let visible = (((hf - 150.0) - top) / row_h).max(1.0) as usize;
    let scroll = app.teacher_rec_index.saturating_sub(visible.saturating_sub(1));
    let mut y = top;
    for (i, st) in students.iter().enumerate().skip(scroll).take(visible) {
        let selected = i == app.teacher_rec_index;
        if selected {
            fill(pm, x0 - 10.0, y - 4.0, table_w + 20.0, row_h - 2.0, SEL_BG);
        }
        let name_color = if selected { WHITE } else { GRAY };
        text(pm, x0, y, scale, &clip_chars(&st.name, 14), name_color);
        for (c, &t) in Topic::ALL.iter().enumerate() {
            text_right(pm, col_right(c), y, scale, &st.solved_for(t).to_string(), WHITE);
        }
        text_right(pm, x0 + name_w + 8.0 * col_w + total_w - 8.0, y, scale, &st.grand_total().to_string(), CYAN);
        let lock = if st.reveal_lock == 0 { "off".to_string() } else { st.reveal_lock.to_string() };
        text_right(pm, x0 + table_w - 8.0, y, scale, &lock, YELLOW);
        y += row_h;
    }

    text_centered(pm, wf / 2.0, hf - 30.0, 1.3, "Up/Dn student   < > section   S/R reset   X remove   +/- lock(0=off)   Esc out", GRAY);
}

/// Draw `s` right-aligned so its right edge sits at `right`.
fn text_right(pm: &mut Pixmap, right: f32, y: f32, scale: f32, s: &str, color: Rgb) {
    text(pm, right - text_width(s, scale), y, scale, s, color);
}

/// Truncate to at most `n` chars, adding an ellipsis when clipped.
fn clip_chars(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(n.saturating_sub(1)).collect();
        t.push('…');
        t
    }
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
// Experimentation — free-form unit explorer
// ---------------------------------------------------------------------------

fn draw_experiment(pm: &mut Pixmap, app: &App, wf: f32, hf: f32) {
    use crate::units::{self, Category};

    let cat = Category::ALL[app.exp_category];
    let units_list = cat.units();
    let from = units_list[app.exp_from.min(units_list.len() - 1)];
    let to = units_list[app.exp_to.min(units_list.len() - 1)];
    let amount: f64 = app.exp_amount.parse().unwrap_or(0.0);
    let result = units::convert(amount, from, to);

    // A card, like the Practice session.
    let cw = (wf * 0.86).min(900.0);
    let ch = (hf * 0.80).min(620.0);
    let (cx0, cy0) = ((wf - cw) / 2.0, (hf - ch) / 2.0);
    let cxc = wf / 2.0;
    fill(pm, cx0, cy0, cw, ch, CARD_BG);
    stroke_rect(pm, cx0, cy0, cw, ch, 2.0, MAGENTA);

    text_centered(pm, cxc, cy0 + 40.0, 2.2, "Experimentation - Unit Explorer", MAGENTA);
    text_centered(pm, cxc, cy0 + 84.0, 1.3, "Type any amount - even a silly one - and watch it convert!", GRAY);

    // Editable fields.
    let amount_display = if app.exp_amount.is_empty() { "0".to_string() } else { app.exp_amount.clone() };
    let caret = if app.exp_field == 0 { "_" } else { "" };
    let rows = [
        ("Amount", format!("{}{}", amount_display, caret)),
        ("From", format!("< {} >", from.plural)),
        ("To", format!("< {} >", to.plural)),
        ("Category", format!("< {} >", cat.name())),
    ];
    let x = cx0 + 90.0;
    let value_x = x + 200.0;
    let row_h = 46.0;
    let top = cy0 + 140.0;
    for (i, (label, value)) in rows.iter().enumerate() {
        let y = top + i as f32 * row_h;
        let selected = i == app.exp_field;
        if selected {
            fill(pm, x - 16.0, y - 8.0, cw - 148.0, 38.0, MAGENTA);
        }
        let (lc, vc) = if selected { (CARD_BG, CARD_BG) } else { (WHITE, CYAN) };
        text(pm, x, y, 1.75, label, lc);
        text(pm, value_x, y, 1.75, value, vc);
    }

    // The conversion, shown large and friendly.
    text_centered(pm, cxc, cy0 + ch * 0.64, 1.6, &format!("{} {} is...", units::format_amount(amount), from.plural), GRAY);
    text_centered(pm, cxc, cy0 + ch * 0.64 + 40.0, 3.2, &format!("= {} {}", units::format_amount(result), to.plural), CYAN);

    // Deduction Duck reacts (upper-right) when the physical size hits something
    // recognisable — or is astronomically larger than anything on the list.
    let base_value = amount * from.to_base;
    draw_exp_reaction(pm, (cx0, cy0, cw, ch), exp_reaction(cat, base_value), app.anim_frame);

    text_centered(pm, cxc, cy0 + ch - 30.0, 1.4, "Up/Down field   Left/Right change   type digits   Esc menu", GRAY);
}

enum ExpReaction {
    None,
    /// The amount is about the size of a real-world thing.
    Match(String),
    /// Astronomically bigger than anything on the list — hat-explode time.
    Boom(String),
}

/// Decide the duck's reaction from the *physical* size in base units (so it's
/// the same whichever target unit is on screen).  Mirrors the terminal frontend.
fn exp_reaction(cat: crate::units::Category, base_value: f64) -> ExpReaction {
    let refs = crate::units::comparisons(cat);
    if refs.is_empty() || !(base_value > 0.0) {
        return ExpReaction::None;
    }
    let best = refs.iter().min_by(|a, b| {
        let (da, db) = ((base_value / a.base).ln().abs(), (base_value / b.base).ln().abs());
        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
    });
    let largest = refs.last().unwrap();
    if let Some(r) = best {
        let ratio = base_value / r.base;
        if (0.55..=1.8).contains(&ratio) {
            let n = ratio.round() as i64;
            let fact = if n <= 1 {
                format!("That's about the same as {}!", r.one)
            } else {
                format!("That's about {} {}!", n, r.many)
            };
            return ExpReaction::Match(fact);
        }
    }
    if base_value >= largest.base * 2.0 {
        let n = base_value / largest.base;
        return ExpReaction::Boom(format!("That's about {} {}!", crate::units::format_amount(n), largest.many));
    }
    ExpReaction::None
}

/// Draw the duck's reaction in the card's upper-right, clear of the centred result.
fn draw_exp_reaction(pm: &mut Pixmap, (cx0, cy0, cw, _ch): (f32, f32, f32, f32), reaction: ExpReaction, frame: u64) {
    use std::f32::consts::TAU;
    let (fact, boom) = match reaction {
        ExpReaction::None => return,
        ExpReaction::Match(f) => (f, false),
        ExpReaction::Boom(f) => (f, true),
    };
    let cell = 12.0;
    let (duck_w, duck_h) = (9.0 * cell, 7.0 * cell * 1.25);
    let duck_x = cx0 + cw - duck_w - 70.0;
    let duck_y = cy0 + 150.0;
    let dcx = duck_x + duck_w / 2.0;

    let (headline, hc) = if boom { ("BOOM!", RED) } else { ("Whoa!", YELLOW) };
    text_centered(pm, dcx, duck_y - 54.0, 2.2, headline, hc);
    text_centered(pm, dcx, duck_y - 22.0, 1.3, &fact, hc);
    draw_duck(pm, duck_x, duck_y, DuckPose::Stand, frame / 5, cell, DUCK);

    if boom {
        // The mortarboard blows off in a ring of pieces around the head.
        let (hx, hy) = (dcx, duck_y + duck_h * 0.1);
        let r = 26.0 + (frame % 12) as f32 * 3.0;
        for k in 0..6 {
            let ang = k as f32 * TAU / 6.0;
            text_centered(pm, hx + ang.cos() * r, hy + ang.sin() * r * 0.6, 1.4, "*", RED);
        }
    } else if (frame / 5) % 2 == 0 {
        text_centered(pm, duck_x - 10.0, duck_y + duck_h * 0.2, 1.4, "*", YELLOW);
        text_centered(pm, duck_x + duck_w + 10.0, duck_y + duck_h * 0.2, 1.4, "*", YELLOW);
    }
}

// ---------------------------------------------------------------------------
// Deduction Duck — the mascot sprite + his help panel
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

/// Deduction Duck's help panel: a how-to hint (never the answer up front), with
/// R to reveal, slid up from below as `help_in` eases 0..1.  Works for every
/// section — arithmetic shows the current strategy, the rest a how-to hint.
fn draw_help(pm: &mut Pixmap, app: &App, (cx0, cy0, cw, ch): (f32, f32, f32, f32)) {
    let pw = (cw * 0.88).min(860.0);
    let ph = (ch * 0.78).min(470.0);
    let px = cx0 + (cw - pw) / 2.0;
    let rest_y = cy0 + (ch - ph) / 2.0;
    let py = rest_y + (1.0 - app.help_in.clamp(0.0, 1.0)) * (ph + 24.0);

    fill(pm, px, py, pw, ph, STAGE_BG);
    stroke_rect(pm, px, py, pw, ph, 2.0, DUCK);

    let pad = 26.0;
    let inner_x = px + pad;
    text(pm, inner_x, py + pad, 1.4, "Deduction Duck", DUCK);

    let mut y = py + pad + 40.0;
    let body_w = pw - pad * 2.0;
    // A peek reprimand (red) wins the top line; else an encouraging word (green).
    if let Some(m) = &app.reprimand {
        for l in wrap_words(m, body_w, 1.4) {
            text(pm, inner_x, y, 1.4, &l, RED);
            y += 26.0;
        }
        y += 6.0;
    } else if let Some(m) = &app.encourage {
        text(pm, inner_x, y, 1.5, m, ACCENT);
        y += 32.0;
    }

    let body_bottom = py + ph - 44.0;
    let mut footer_space = false;
    match &app.current {
        Active::Arith(p) => {
            use crate::strategy::Viz;
            let strats = crate::strategy::strategies(p);
            let i = app.strategy_index % strats.len();
            let s = &strats[i];
            footer_space = strats.len() > 1;
            text(pm, inner_x, y, 1.5, &format!("Way {}/{}: {}", i + 1, strats.len(), s.title), DUCK);
            y += 34.0;
            let accent = core_col(p.accent);
            let region = (inner_x, y, body_w, (body_bottom - y).max(0.0));
            let drawn = match &s.viz {
                Viz::NumberLine { stops, hops } => draw_number_line(pm, region, s, stops, hops, app, accent),
                Viz::Smash { value, parts } => draw_smash(pm, region, s, *value, parts, app, accent),
                Viz::Lines => false,
            };
            if !drawn {
                draw_strategy_text(pm, region, s, app.revealed, app.anim_frame);
            }
        }
        Active::Shape(s) => hint_body(pm, inner_x, y, body_w, body_bottom, &s.hint, &s.answer_label(), app.revealed, app.anim_frame),
        Active::Unit(u) => hint_body(pm, inner_x, y, body_w, body_bottom, &u.hint, &format!("Answer: {} {}", u.answer, u.unit_label), app.revealed, app.anim_frame),
        Active::Geo(g) => hint_body(pm, inner_x, y, body_w, body_bottom, &g.hint, &g.answer_label(), app.revealed, app.anim_frame),
    }

    // Footer controls.
    let reveal = if app.reveal_locked() {
        "Answer's on cooldown - try it yourself!"
    } else if app.revealed {
        "R: hide answer"
    } else {
        "R: show answer"
    };
    let footer = if footer_space {
        format!("{}    Space: another way    H: shoo", reveal)
    } else {
        format!("{}    H: shoo", reveal)
    };
    text(pm, inner_x, py + ph - 32.0, 1.3, &footer, GRAY);
}

/// A how-to hint header + bulleted lines, then the revealed answer (green), with
/// the duck standing on the right of the region.
#[allow(clippy::too_many_arguments)]
fn hint_body(pm: &mut Pixmap, x: f32, mut y: f32, body_w: f32, body_bottom: f32, hint: &[String], answer: &str, revealed: bool, frame: u64) {
    let (cell, region_right) = (14.0, x + body_w);
    let duck_w = 9.0 * cell;
    let wrap_w = body_w - duck_w - 24.0;
    draw_duck(pm, region_right - duck_w, (y + body_bottom) / 2.0 - 7.0 * cell * 1.25 / 2.0, DuckPose::Stand, frame / 6, cell, DUCK);

    text(pm, x, y, 1.5, "Deduction Duck's hint:", DUCK);
    y += 34.0;
    let bulleted: Vec<String> = hint.iter().map(|h| format!("- {}", h)).collect();
    y = wrap_block(pm, x, y, wrap_w, 1.35, &bulleted, WHITE);
    if revealed {
        y += 10.0;
        text(pm, x, y, 1.5, answer, ACCENT);
    }
}

/// Plain talk-through: the strategy's steps (and revealed working) on the left,
/// the duck standing on the right.  The fallback when no richer viz fits.
fn draw_strategy_text(pm: &mut Pixmap, (rx, ry, rw, rh): (f32, f32, f32, f32), s: &crate::strategy::Strategy, revealed: bool, frame: u64) {
    let cell = 14.0;
    let duck_w = 9.0 * cell;
    let wrap_w = rw - duck_w - 24.0;
    draw_duck(pm, rx + rw - duck_w, ry + rh / 2.0 - 7.0 * cell * 1.25 / 2.0, DuckPose::Stand, frame / 6, cell, DUCK);

    let mut y = wrap_block(pm, rx, ry, wrap_w, 1.35, &s.steps, WHITE);
    if revealed {
        y += 10.0;
        wrap_block(pm, rx, y, wrap_w, 1.35, &s.reveal, ACCENT);
    }
}

/// Draw each string in `lines` word-wrapped to `wrap_w`; returns the new `y`.
fn wrap_block(pm: &mut Pixmap, x: f32, mut y: f32, wrap_w: f32, scale: f32, lines: &[String], color: Rgb) -> f32 {
    let lh = glyph_h(scale) + 8.0;
    for line in lines {
        for l in wrap_words(line, wrap_w, scale) {
            text(pm, x, y, scale, &l, color);
            y += lh;
        }
    }
    y
}

/// The duck walks a number line, hopping stop to stop.  Returns false (so the
/// caller falls back to the text view) when the stops are too cramped to read.
fn draw_number_line(pm: &mut Pixmap, (rx, ry, rw, rh): (f32, f32, f32, f32), s: &crate::strategy::Strategy, stops: &[i64], hops: &[String], app: &App, accent: Rgb) -> bool {
    let n = stops.len();
    if n < 2 {
        return false;
    }
    let labels: Vec<String> = stops.iter().map(|v| v.to_string()).collect();
    let lscale = 1.3;
    let maxlabel = labels.iter().map(|l| text_width(l, lscale)).fold(0.0_f32, f32::max);
    let margin = maxlabel / 2.0 + 12.0;
    let (ax0, ax1) = (rx + margin, rx + rw - margin);
    let seg = (ax1 - ax0) / (n - 1) as f32;
    if ax1 <= ax0 || seg < maxlabel + 18.0 {
        return false; // too tight — let the caller fall back to text
    }
    let x_at = |i: usize| ax0 + i as f32 * seg;

    let axis_y = ry + rh - 40.0;
    let label_y = axis_y + 10.0;
    let hop_y = axis_y - 30.0;

    // Subtitle (hint) and, once revealed, the worked hops up top.
    text(pm, rx, ry, 1.3, &s.steps[0], WHITE);
    if app.revealed {
        let mut yy = ry + 26.0;
        for l in &s.reveal {
            text(pm, rx, yy, 1.2, l, ACCENT);
            yy += 22.0;
        }
    }

    // Axis, ticks, labels.
    line(pm, ax0, axis_y, ax1, axis_y, [78, 82, 100], 2.0);
    for i in 0..n {
        let x = x_at(i);
        line(pm, x, axis_y - 7.0, x, axis_y + 7.0, accent, 2.0);
        let green = app.revealed && i == n - 1;
        text_centered(pm, x, label_y, lscale, &labels[i], if green { ACCENT } else { GRAY });
    }
    for i in 0..n - 1 {
        text_centered(pm, (x_at(i) + x_at(i + 1)) / 2.0, hop_y, 1.2, &hops[i], ACCENT);
    }

    // Walking duck: travel each segment, dwelling on the final stop, then loop.
    const TICKS_PER_HOP: u64 = 20;
    let pos = (app.anim_frame / TICKS_PER_HOP) as f32 % (n as f32 + 1.0);
    let segi = pos.floor() as usize;
    let (dx, lift) = if segi >= n - 1 {
        (x_at(n - 1), 0.0)
    } else {
        let frac = pos - segi as f32;
        (x_at(segi) + frac * seg, (frac * std::f32::consts::PI).sin() * 16.0)
    };
    let cell = 12.0;
    let (duck_w, duck_h) = (9.0 * cell, 7.0 * cell * 1.25);
    let duck_left = dx - duck_w / 2.0;
    let duck_top = (hop_y - 8.0) - duck_h - lift;
    let waddle = if segi >= n - 1 { 0 } else { app.anim_frame / 7 };
    draw_duck(pm, duck_left, duck_top, DuckPose::WalkRight, waddle, cell, DUCK);
    true
}

/// The duck smashes a number apart into its place-value pieces.
fn draw_smash(pm: &mut Pixmap, (rx, ry, rw, rh): (f32, f32, f32, f32), s: &crate::strategy::Strategy, value: i64, parts: &[i64], app: &App, accent: Rgb) -> bool {
    let whole = value.to_string();
    let parts_str = parts.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(" + ");

    text(pm, rx, ry, 1.3, &s.steps[0], WHITE);

    // Cycle: show the whole number (wind-up + chop), then the pieces.
    const PERIOD: u64 = 18;
    let show_parts = parts.len() > 1 && (app.anim_frame / PERIOD) % 2 == 1;
    let burst = parts.len() > 1 && (app.anim_frame % PERIOD) < 3;

    let scale = fit_scale(&parts_str, rw - 200.0, 5.0);
    let ncx = rx + rw * 0.56;
    let ncy = ry + rh * 0.42;
    let shown = if show_parts { &parts_str } else { &whole };
    text_centered(pm, ncx, ncy, scale, shown, accent);

    if burst {
        for (dx, dy) in [(-46.0, -10.0), (46.0, -10.0), (-30.0, 18.0), (30.0, 18.0), (0.0, -32.0), (0.0, 30.0)] {
            text_centered(pm, ncx + dx, ncy + glyph_h(scale) / 2.0 + dy, 1.6, "*", YELLOW);
        }
        text(pm, rx, ry + 24.0, 1.6, "POW!", RED);
    }

    if app.revealed {
        let mut yy = ry + rh - (s.reveal.len() as f32) * 22.0;
        for l in &s.reveal {
            text(pm, rx, yy, 1.2, l, ACCENT);
            yy += 22.0;
        }
    }

    // The duck chops (whole) or recoils (parts), on the left.
    let pose = if show_parts { DuckPose::Stand } else { DuckPose::Smash };
    let cell = 13.0;
    draw_duck(pm, rx, ncy - 7.0 * cell * 1.25 / 2.0, pose, app.anim_frame / 3, cell, DUCK);
    true
}

/// "Why am I learning this?" — real-life uses for the section, plus an occasional
/// peek at a line of this app's own code.  Scrolls with Up/Down when it overflows.
fn draw_why(pm: &mut Pixmap, app: &App, (cx0, cy0, cw, ch): (f32, f32, f32, f32)) {
    let heading = crate::motivation::heading(app.current_topic());
    let pw = (cw * 0.9).min(900.0);
    let ph = (ch * 0.86).min(560.0);
    let px = cx0 + (cw - pw) / 2.0;
    let py = cy0 + (ch - ph) / 2.0;
    fill(pm, px, py, pw, ph, STAGE_BG);
    stroke_rect(pm, px, py, pw, ph, 2.0, CYAN);

    let pad = 26.0;
    let inner_x = px + pad;
    let wrap_w = pw - pad * 2.0 - 20.0;
    let scale = 1.3;
    text(pm, inner_x, py + pad, 1.6, &heading, CYAN);

    // Tagged content lines: 0 bullet, 1 code header, 2 code, 3 why-it-matters.
    let mut lines: Vec<(String, u8)> = Vec::new();
    for item in &app.why_items {
        for l in wrap_words(&format!("- {}", item), wrap_w, scale) {
            lines.push((l, 0));
        }
    }
    if let Some((code, why)) = &app.why_code {
        lines.push((String::new(), 0));
        lines.push(("For future coders - a line from this app:".to_string(), 1));
        lines.push((format!("  {}", code), 2));
        for l in wrap_words(&format!("  Why it matters: {}", why), wrap_w, scale) {
            lines.push((l, 3));
        }
    }

    // Scrolling window between the heading and the footer.
    let first_y = py + pad + 42.0;
    let footer_y = py + ph - 30.0;
    let lh = glyph_h(scale) + 8.0;
    let visible = (((footer_y - 12.0) - first_y) / lh).max(1.0) as usize;
    let max_scroll = lines.len().saturating_sub(visible);
    app.why_max_scroll.set(max_scroll);
    let scroll = app.why_scroll.min(max_scroll);

    let mut y = first_y;
    for (line, tag) in lines.iter().skip(scroll).take(visible) {
        let color = match tag {
            1 => YELLOW,
            2 => ACCENT,
            3 => GRAY,
            _ => WHITE,
        };
        text(pm, inner_x, y, scale, line, color);
        y += lh;
    }

    // Scroll affordances + footer.
    let more_x = px + pw - pad - 80.0;
    if scroll > 0 {
        text(pm, more_x, first_y - 26.0, 1.2, "^ more", CYAN);
    }
    if scroll < max_scroll {
        text(pm, more_x, footer_y, 1.2, "v more", CYAN);
    }
    let footer = if max_scroll > 0 { "Y/Esc: close    Up/Down scroll" } else { "Y or Esc: close" };
    text(pm, inner_x, footer_y, 1.3, footer, GRAY);
}

// ---------------------------------------------------------------------------
// Milestone cinematics — starry name-in-the-sky scenes with the awe duck
// ---------------------------------------------------------------------------

fn draw_cinematic(pm: &mut Pixmap, app: &App, wf: f32, hf: f32) {
    let Some(c) = &app.cinematic else { return };
    if let Some(t) = &c.transition {
        // A scene-to-scene transition: capture the settled current scene melting
        // into the start of the next, and reuse the same blend engine.
        if let (Some(cur), Some(next)) = (c.scenes.get(c.index), c.scenes.get(c.index + 1)) {
            let (w, h) = (pm.width(), pm.height());
            let from = card_frame(w, h, |p| render_scene(p, cur, cur.dwell as u64, wf, hf));
            let to = card_frame(w, h, |p| render_scene(p, next, 0, wf, hf));
            blend_transition(pm, &from, &to, t.effect, t.progress.clamp(0.0, 1.0));
        }
    } else if let Some(scene) = c.scenes.get(c.index) {
        // Elapsed ticks into this scene (dwell counts down from scene.dwell).
        let frame = scene.dwell.saturating_sub(c.dwell) as u64;
        render_scene(pm, scene, frame, wf, hf);
    }
    text_centered(pm, wf / 2.0, hf - 24.0, 1.3, "press any key to skip", GRAY);
}

fn render_scene(pm: &mut Pixmap, scene: &crate::cinematic::Scene, frame: u64, wf: f32, hf: f32) {
    use crate::cinematic::SceneKind;
    let accent = core_col(scene.accent);
    match scene.kind {
        SceneKind::NameSky => render_name_sky(pm, &scene.heading, &scene.sub, accent, frame, wf, hf),
        SceneKind::RocketName => render_rocket_name(pm, &scene.heading, &scene.sub, accent, frame, wf, hf),
        SceneKind::Text => {
            let cxc = wf / 2.0;
            if let Some(big) = &scene.big {
                let bscale = fit_scale(big, wf * 0.7, 9.0);
                text_centered(pm, cxc, hf / 2.0 - glyph_h(bscale) - 20.0, bscale, big, accent);
                text_centered(pm, cxc, hf / 2.0 + 30.0, 2.5, &scene.heading, WHITE);
                text_centered(pm, cxc, hf / 2.0 + 76.0, 1.75, &scene.sub, accent);
            } else {
                text_centered(pm, cxc, hf / 2.0 - 24.0, 2.6, &scene.heading, accent);
                text_centered(pm, cxc, hf / 2.0 + 26.0, 1.9, &scene.sub, WHITE);
            }
        }
    }
}

/// A small filled square (logical coords), centred at `(x, y)`.
fn dot(pm: &mut Pixmap, x: f32, y: f32, s: f32, color: Rgb) {
    fill(pm, x - s / 2.0, y - s / 2.0, s, s, color);
}

/// A filled disc, approximated by a fan polygon.
fn fill_circle(pm: &mut Pixmap, cx: f32, cy: f32, r: f32, color: Rgb) {
    use std::f32::consts::TAU;
    let steps = 30;
    let pts: Vec<(f32, f32)> = (0..steps).map(|k| {
        let a = k as f32 * TAU / steps as f32;
        (cx + a.cos() * r, cy + a.sin() * r)
    }).collect();
    if let Some(p) = poly(&pts) {
        fill_path(pm, &p, color);
    }
}

/// A deterministic twinkling starfield over the whole sky.
fn starfield(pm: &mut Pixmap, wf: f32, hf: f32, frame: u64, density: f32, salt: u64) {
    let count = (wf * hf / density) as u64;
    for i in 0..count {
        let hsh = i.wrapping_mul(2_654_435_761).wrapping_add(salt);
        let sx = (hsh % wf as u64) as f32;
        let sy = ((hsh / 11) % hf as u64) as f32;
        if (hsh / 13 + frame / 5) % 7 < 3 {
            dot(pm, sx, sy, 3.0, [120, 132, 170]);
        }
    }
}

/// Deduction Duck planted at the bottom-left, gazing up in starry-eyed awe.
fn awe_duck(pm: &mut Pixmap, hf: f32, frame: u64) {
    let cell = 16.0;
    draw_duck(pm, 36.0, hf - 7.0 * cell * 1.25 - 64.0, DuckPose::Awe, frame / 8, cell, DUCK);
}

/// A night sky: twinkling stars, a comet stream, the name written among the
/// stars, and a moon rising from below.
fn render_name_sky(pm: &mut Pixmap, name: &str, sub: &str, accent: Rgb, frame: u64, wf: f32, hf: f32) {
    starfield(pm, wf, hf, frame, 900.0, 97);

    // A stream of comets streaking left-to-right across the upper sky.
    let span = wf + 80.0;
    for c in 0..3u64 {
        let row = 44.0 + (c as f32) * 48.0;
        let speed = 4.0 + (c % 2) as f32 * 3.0;
        let head = (frame as f32 * speed + c as f32 * 120.0) % span - 40.0;
        for t in 0..7i32 {
            let x = head - t as f32 * 14.0;
            if x < 0.0 || x > wf {
                continue;
            }
            let (sz, color) = match t {
                0 => (7.0, [245, 245, 250]),
                1 => (5.0, accent),
                _ => (3.0, [110, 140, 180]),
            };
            dot(pm, x, row, sz, color);
        }
    }

    // The moon rising from below, resting in the lower third.
    let moon_r = 48.0;
    let rest_cy = hf - moon_r - 40.0;
    let rise = (frame as f32 / 55.0).clamp(0.0, 1.0);
    let moon_cy = (hf + moon_r) - rise * ((hf + moon_r) - rest_cy);
    let moon_cx = wf / 2.0;
    fill_circle(pm, moon_cx, moon_cy, moon_r, [245, 240, 200]);
    for (dx, dy, r) in [(-16.0, -10.0, 9.0), (12.0, 6.0, 7.0), (4.0, -18.0, 5.0)] {
        fill_circle(pm, moon_cx + dx, moon_cy + dy, r, [225, 220, 178]);
    }

    // The name across the sky, with its tagline beneath.
    let ny = hf * 0.36;
    text_centered(pm, wf / 2.0, ny, 3.0, &format!("*   {}   *", name), accent);
    text_centered(pm, wf / 2.0, ny + 54.0, 1.9, sub, WHITE);

    awe_duck(pm, hf, frame);
}

/// Rockets launch from the ground, each arriving at a letter of the name and
/// igniting it as a star; once the name is spelled, a blue comet sweeps behind.
fn render_rocket_name(pm: &mut Pixmap, name: &str, sub: &str, accent: Rgb, frame: u64, wf: f32, hf: f32) {
    starfield(pm, wf, hf, frame, 1100.0, 31);

    let scale = 3.0;
    let ny = hf * 0.36;
    let ground = hf - 30.0;
    // Letter positions: lay the name out centred, measuring each glyph.
    let total_w = text_width(name, scale);
    let mut lx = wf / 2.0 - total_w / 2.0;
    let letters: Vec<(char, f32)> = name
        .chars()
        .map(|ch| {
            let x = lx;
            lx += text_width(&ch.to_string(), scale);
            (ch, x)
        })
        .collect();

    const STAGGER: u64 = 4;
    const RISE: u64 = 22;
    let lit_at = |i: u64| i * STAGGER + RISE;
    let form_done = lit_at(letters.len().saturating_sub(1) as u64);

    // The blue comet, drawn before the letters so the name occludes it.
    let comet_start = form_done + 12;
    if frame >= comet_start {
        let p = (frame - comet_start) as f32 / 50.0;
        let head = -40.0 + p * (wf + 80.0);
        for t in 0..9i32 {
            let x = head - t as f32 * 13.0;
            if x < 0.0 || x > wf {
                continue;
            }
            let (sz, color) = match t {
                0 => (7.0, [170, 210, 255]),
                1 | 2 => (5.0, [90, 150, 255]),
                _ => (3.0, [55, 95, 200]),
            };
            dot(pm, x, ny + glyph_h(scale) / 2.0, sz, color);
        }
    }

    // Rockets rising, then their letters lit as stars.
    for (i, (ch, lx)) in letters.iter().enumerate() {
        if *ch == ' ' {
            continue;
        }
        let cxl = lx + text_width(&ch.to_string(), scale) / 2.0;
        let start = i as u64 * STAGGER;
        if frame < start {
            continue;
        }
        let prog = ((frame - start) as f32 / RISE as f32).clamp(0.0, 1.0);
        if prog < 1.0 {
            let cur_y = ny + (1.0 - prog) * (ground - ny);
            // A rocket: a white nose-triangle with a short fiery exhaust.
            tri(pm, cxl, cur_y - 14.0, cxl - 9.0, cur_y + 8.0, cxl + 9.0, cur_y + 8.0, [245, 245, 250], 3.0);
            for (t, col) in [[255, 190, 70], [255, 120, 40], [180, 60, 30]].iter().enumerate() {
                dot(pm, cxl, cur_y + 16.0 + t as f32 * 9.0, 6.0 - t as f32, *col);
            }
        } else {
            let fresh = frame < lit_at(i as u64) + 6;
            text(pm, *lx, ny, scale, &ch.to_string(), if fresh { WHITE } else { accent });
            if fresh {
                dot(pm, cxl, ny - 14.0, 7.0, [240, 215, 110]);
            }
        }
    }

    if frame >= form_done {
        text_centered(pm, wf / 2.0, ny + 54.0, 1.9, sub, WHITE);
    }
    awe_duck(pm, hf, frame);
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
// Fraction / percentage figures — equal regions that materialise piece by piece
// ---------------------------------------------------------------------------

/// Faint fill for a region that hasn't materialised yet.
const SLOT: Rgb = [60, 62, 78];
/// Thin separator / outline tone between pieces.
const SEP: Rgb = [92, 96, 116];

/// Draw the fraction/percent figure inside `(rx, ry, rw, rh)`, the shaded pieces
/// first, materialising as `progress` rises from 0 to 1.
fn draw_shape(pm: &mut Pixmap, (rx, ry, rw, rh): (f32, f32, f32, f32), s: &FractionProblem, progress: f32) {
    let color_of = |i: u16| core_col(s.color_of(i));
    match s.shape {
        Shape::Bar { segments } => draw_cells(pm, rx, ry, rw, rh, 1, segments, progress, &color_of),
        Shape::Grid { rows, cols } => draw_cells(pm, rx, ry, rw, rh, rows, cols, progress, &color_of),
        Shape::Circle { slices } => draw_pie(pm, rx, ry, rw, rh, slices, progress, &color_of),
        Shape::Triangle { strips } => draw_tri_strips(pm, rx, ry, rw, rh, strips, progress, &color_of),
    };
}

/// A `rows × cols` block of separated cells (covers both the bar and the grid).
fn draw_cells(pm: &mut Pixmap, rx: f32, ry: f32, rw: f32, rh: f32, rows: u16, cols: u16, progress: f32, color_of: &impl Fn(u16) -> Rgb) {
    let n = rows * cols;
    let gap = 7.0;
    let cw = ((rw - (cols - 1) as f32 * gap) / cols as f32).min(70.0);
    let cht = ((rh - (rows - 1) as f32 * gap) / rows as f32).min(if rows == 1 { 96.0 } else { 64.0 });
    let total_w = cols as f32 * cw + (cols - 1) as f32 * gap;
    let total_h = rows as f32 * cht + (rows - 1) as f32 * gap;
    let x0 = rx + (rw - total_w) / 2.0;
    let y0 = ry + (rh - total_h) / 2.0;
    for i in 0..n {
        let (row, c) = (i / cols, i % cols);
        let x = x0 + c as f32 * (cw + gap);
        let y = y0 + row as f32 * (cht + gap);
        fill(pm, x, y, cw, cht, region_fill(i, n, progress, color_of(i)));
    }
}

/// A pie circle of `slices` wedges, shaded pieces first.
fn draw_pie(pm: &mut Pixmap, rx: f32, ry: f32, rw: f32, rh: f32, slices: u16, progress: f32, color_of: &impl Fn(u16) -> Rgb) {
    use std::f32::consts::TAU;
    let (cx, cy) = (rx + rw / 2.0, ry + rh / 2.0);
    let radius = (rw / 2.0).min(rh / 2.0) * 0.92;
    let ang = |k: u16| (k as f32 / slices as f32 - 0.5) * TAU;
    for k in 0..slices {
        let (t0, t1) = (ang(k), ang(k + 1));
        let steps = 10;
        let mut pts = vec![(cx, cy)];
        for sgmt in 0..=steps {
            let a = t0 + (t1 - t0) * (sgmt as f32 / steps as f32);
            pts.push((cx + a.cos() * radius, cy + a.sin() * radius));
        }
        if let Some(p) = poly(&pts) {
            fill_path(pm, &p, region_fill(k, slices, progress, color_of(k)));
        }
    }
    // Spokes between wedges + a rim, so same-coloured neighbours stay distinct.
    for k in 0..slices {
        let a = ang(k);
        line(pm, cx, cy, cx + a.cos() * radius, cy + a.sin() * radius, BG, 2.0);
    }
    circle(pm, cx, cy, radius, SEP, 2.0);
}

/// An upward triangle cut into `strips` horizontal layers (apex = region 0).
fn draw_tri_strips(pm: &mut Pixmap, rx: f32, ry: f32, rw: f32, rh: f32, strips: u16, progress: f32, color_of: &impl Fn(u16) -> Rgb) {
    let th = rh * 0.9;
    let base_w = (rw * 0.82).min(th * 1.5);
    let cx = rx + rw / 2.0;
    let apex_y = ry + (rh - th) / 2.0;
    let half = |fy: f32| fy * base_w / 2.0;
    let yat = |fy: f32| apex_y + fy * th;
    for i in 0..strips {
        let (f0, f1) = (i as f32 / strips as f32, (i + 1) as f32 / strips as f32);
        let (y0, y1) = (yat(f0), yat(f1));
        let (h0, h1) = (half(f0), half(f1));
        let pts = [(cx - h0, y0), (cx + h0, y0), (cx + h1, y1), (cx - h1, y1)];
        if let Some(p) = poly(&pts) {
            fill_path(pm, &p, region_fill(i, strips, progress, color_of(i)));
        }
    }
    // Separators between strips + the two sloped edges.
    for i in 1..strips {
        let fy = i as f32 / strips as f32;
        line(pm, cx - half(fy), yat(fy), cx + half(fy), yat(fy), BG, 2.0);
    }
    let (bl, br, top) = ((cx - half(1.0), yat(1.0)), (cx + half(1.0), yat(1.0)), (cx, apex_y));
    line(pm, top.0, top.1, bl.0, bl.1, SEP, 2.0);
    line(pm, top.0, top.1, br.0, br.1, SEP, 2.0);
    line(pm, bl.0, bl.1, br.0, br.1, SEP, 2.0);
}

/// Blend a region from the faint slot toward its `base` colour as it appears.
fn region_fill(i: u16, n: u16, progress: f32, base: Rgb) -> Rgb {
    let appear = (i + 1) as f32 / n as f32;
    let start = i as f32 / n as f32;
    if progress >= appear {
        base
    } else if progress > start {
        lerp_rgb(SLOT, base, (progress - start) / (appear - start))
    } else {
        SLOT
    }
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

// ---------------------------------------------------------------------------
// Units of Measure — a simple themed prop per problem
// ---------------------------------------------------------------------------

/// The accent colour for a unit problem's theme (matches the terminal palette).
fn theme_col(t: Theme) -> Rgb {
    match t {
        Theme::Cup => [240, 215, 110],
        Theme::Pool => [120, 170, 240],
        Theme::Bottle => [120, 220, 240],
        Theme::Scale => [210, 140, 230],
        Theme::Ruler => [120, 220, 150],
        Theme::Coin => [255, 200, 40],
    }
}

/// Draw a small prop for `theme`, centred in `(rx, ry, rw, rh)`.
fn draw_unit_prop(pm: &mut Pixmap, (rx, ry, rw, rh): (f32, f32, f32, f32), theme: Theme) {
    let c = theme_col(theme);
    let (cx, cy) = (rx + rw / 2.0, ry + rh / 2.0);
    let h = rh * 0.74;
    match theme {
        Theme::Cup => {
            let (ht, topw, botw) = (h, h * 0.95, h * 0.66);
            let (top, bot) = (cy - ht / 2.0, cy + ht / 2.0);
            let hw = |y: f32| {
                let t = (y - top) / (bot - top);
                topw / 2.0 + (botw / 2.0 - topw / 2.0) * t
            };
            let lt = top + ht * 0.45; // liquid line
            if let Some(p) = poly(&[(cx - hw(lt), lt), (cx + hw(lt), lt), (cx + hw(bot), bot), (cx - hw(bot), bot)]) {
                fill_path(pm, &p, [90, 150, 220]);
            }
            line(pm, cx - topw / 2.0, top, cx + topw / 2.0, top, c, 3.0);
            line(pm, cx - topw / 2.0, top, cx - botw / 2.0, bot, c, 3.0);
            line(pm, cx + topw / 2.0, top, cx + botw / 2.0, bot, c, 3.0);
            line(pm, cx - botw / 2.0, bot, cx + botw / 2.0, bot, c, 3.0);
            // Handle, following the cup's slope on the right.
            let (ya, yb) = (top + 12.0, cy + 10.0);
            line(pm, cx + hw(ya), ya, cx + hw(ya) + 30.0, ya, c, 3.0);
            line(pm, cx + hw(ya) + 30.0, ya, cx + hw(yb) + 30.0, yb, c, 3.0);
            line(pm, cx + hw(yb) + 30.0, yb, cx + hw(yb), yb, c, 3.0);
            for k in 1..4 {
                let y = top + ht * 0.18 * k as f32;
                line(pm, cx - topw * 0.30, y, cx - topw * 0.15, y, c, 2.0);
            }
        }
        Theme::Pool => {
            let (pw, ph) = (rw * 0.46, h * 0.7);
            let (x0, y0) = (cx - pw / 2.0, cy - ph / 2.0);
            fill(pm, x0 + 4.0, y0 + ph * 0.34, pw - 8.0, ph * 0.66 - 4.0, [70, 120, 200]);
            stroke_rect(pm, x0, y0, pw, ph, 3.0, c);
            for k in 0..3 {
                wavy_line(pm, x0 + 10.0, x0 + pw - 10.0, y0 + ph * 0.46 + k as f32 * 16.0, 5.0, [185, 212, 240], 2.0);
            }
        }
        Theme::Bottle => {
            let (bw, bh) = (h * 0.5, h * 0.96);
            let (x0, top) = (cx - bw / 2.0, cy - bh / 2.0);
            let (neck_w, neck_h) = (bw * 0.4, bh * 0.22);
            fill(pm, cx - neck_w / 2.0 - 2.0, top, neck_w + 4.0, 10.0, c); // cap
            stroke_rect(pm, cx - neck_w / 2.0, top + 10.0, neck_w, neck_h, 3.0, c);
            let body_top = top + 10.0 + neck_h;
            let body_h = bh - 10.0 - neck_h;
            fill(pm, x0 + 4.0, body_top + body_h * 0.42, bw - 8.0, body_h * 0.58 - 4.0, [90, 180, 210]);
            stroke_rect(pm, x0, body_top, bw, body_h, 3.0, c);
        }
        Theme::Scale => {
            let bw = rw * 0.4;
            let beam_y = cy - h * 0.26;
            line(pm, cx, beam_y, cx, cy + h * 0.4, c, 3.0); // post
            line(pm, cx - h * 0.3, cy + h * 0.4, cx + h * 0.3, cy + h * 0.4, c, 4.0); // base
            line(pm, cx - bw / 2.0, beam_y, cx + bw / 2.0, beam_y, c, 3.0); // beam
            for sx in [cx - bw / 2.0, cx + bw / 2.0] {
                line(pm, sx, beam_y, sx, beam_y + 30.0, c, 2.0);
                if let Some(p) = poly(&[(sx - 26.0, beam_y + 30.0), (sx + 26.0, beam_y + 30.0), (sx + 16.0, beam_y + 44.0), (sx - 16.0, beam_y + 44.0)]) {
                    fill_path(pm, &p, c);
                }
            }
        }
        Theme::Ruler => {
            let (rw2, rh2) = (rw * 0.58, h * 0.34);
            let (x0, y0) = (cx - rw2 / 2.0, cy - rh2 / 2.0);
            stroke_rect(pm, x0, y0, rw2, rh2, 3.0, c);
            let n = 10;
            for k in 0..=n {
                let x = x0 + rw2 * (k as f32 / n as f32);
                let len = if k % 5 == 0 { rh2 * 0.6 } else { rh2 * 0.35 };
                line(pm, x, y0, x, y0 + len, c, 2.0);
            }
        }
        Theme::Coin => {
            let r = h * 0.32;
            for off in [16.0, 8.0, 0.0] {
                circle(pm, cx, cy + off, r, c, 3.0);
            }
            text_centered(pm, cx, cy - glyph_h(1.8) / 2.0, 1.8, "$", c);
        }
    }
}

/// The onomatopoeia the duck makes diving into a prop.
fn theme_splash(theme: Theme) -> &'static str {
    match theme {
        Theme::Cup | Theme::Pool | Theme::Bottle => "SPLASH!",
        Theme::Scale => "THUD!",
        Theme::Ruler => "BONK!",
        Theme::Coin => "CHA-CHING!",
    }
}

/// The silly gag: a mini duck arcs in from the left, splashes into the prop,
/// then pops back out — on a loop, while `duck_jump` is set.
fn draw_unit_gag(pm: &mut Pixmap, (rx, ry, rw, rh): (f32, f32, f32, f32), theme: Theme, frame: u64) {
    use std::f32::consts::PI;
    let c = theme_col(theme);
    let cx = rx + rw / 2.0;
    let top = ry + rh * 0.30;
    let land_x = cx - 16.0;
    let mini = "~(o>";
    let scale = 1.6;

    const CYCLE: u64 = 96;
    let t = (frame % CYCLE) as f32 / CYCLE as f32;
    if t < 0.45 {
        let p = t / 0.45;
        let start_x = rx + 24.0;
        let x = start_x + (land_x - start_x) * p;
        let y = top - 8.0 - (p * PI).sin() * 46.0;
        text(pm, x, y, scale, mini, c);
    } else if t < 0.58 {
        text_centered(pm, cx, top - 26.0, scale, theme_splash(theme), YELLOW);
        for (dx, dy) in [(-40.0, 0.0), (40.0, 0.0), (-28.0, 12.0), (28.0, 12.0)] {
            text_centered(pm, cx + dx, top + dy, 1.4, "\u{00b0}", CYAN);
        }
    } else {
        let p = (t - 0.58) / 0.42;
        let y = top - 8.0 - (p * PI).sin() * 40.0;
        text(pm, land_x, y, scale, mini, c);
    }
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
            // Cap to half the band (less the label row) so the disc stays inside
            // the band and never reaches up into the question text above it.
            let max_rad = (rh * 0.5 - 26.0).min(rw * 0.5).max(8.0);
            let rad = max_rad * (0.62 + (r.clamp(2, 9) as f32) / 30.0).min(1.0);
            circle(pm, cx, cy - 8.0, rad, ACCENT, 3.0);
            line(pm, cx, cy - 8.0, cx + rad, cy - 8.0, YELLOW, 2.0); // the radius
            text_centered(pm, cx, cy + rad - 2.0, 1.5, &format!("radius {} cm", r), GRAY);
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
