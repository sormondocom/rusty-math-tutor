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
        Screen::Cinematic => placeholder(&mut pm, wf, hf, "Milestone!"),
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
    let accent = if let Active::Arith(p) = &app.current { core_col(p.accent) } else { ACCENT };
    fill(pm, cx0, cy0, cw, ch, CARD_BG);
    stroke_rect(pm, cx0, cy0, cw, ch, 2.0, accent);

    // Arithmetic owns the whole card with its layout form (so the V key — which
    // toggles horizontal / stacked / long-division — actually changes the view).
    // The other sections show a question, a per-section figure, then the answer.
    if let Active::Arith(p) = &app.current {
        draw_arith(pm, (cx0, cy0, cw, ch), p, &app.input, app.config.layout, app.feedback);
    } else {
        let q = question_of(&app.current);
        let qscale = fit_scale(&q, cw - 80.0, 5.0);
        text_centered(pm, cxc, cy0 + 50.0, qscale, &q, WHITE);

        // The per-section visual takes the upper band; the answer moves below it.
        let mut answer_y = cy0 + ch * 0.42;
        let mut ans_scale = 7.0;
        let band = (cx0 + 50.0, cy0 + 96.0, cw - 100.0, ch * 0.34);
        match &app.current {
            Active::Geo(g) => {
                draw_geo(pm, band, g);
                answer_y = band.1 + band.3 + 26.0;
                ans_scale = 4.0;
            }
            Active::Shape(s) => {
                draw_shape(pm, band, s, app.frac_progress());
                answer_y = band.1 + band.3 + 26.0;
                ans_scale = 4.0;
            }
            Active::Unit(u) => {
                draw_unit_prop(pm, band, u.theme);
                answer_y = band.1 + band.3 + 26.0;
                ans_scale = 4.0;
            }
            _ => {}
        }

        let ans = if app.input.is_empty() { "?".to_string() } else { app.input.clone() };
        text_centered(pm, cxc, answer_y, 2.0, "Your answer:", GRAY);
        text_centered(pm, cxc, answer_y + 28.0, ans_scale, &ans, ans_color(&app.input, app.feedback));
        // Units: name the unit beneath the number, so the answer reads in full.
        if let Active::Unit(u) = &app.current {
            text_centered(pm, cxc, answer_y + 68.0, 1.75, &u.unit_label, theme_col(u.theme));
        }
    }

    // Feedback line, just above the footer.
    if let Some((msg, color)) = match app.feedback {
        Feedback::Correct => Some(("Correct!", ACCENT)),
        Feedback::Wrong => Some(("Not quite - try again", RED)),
        Feedback::None => None,
    } {
        text_centered(pm, cxc, cy0 + ch - 84.0, 3.0, msg, color);
    }

    let footer = if app.current.has_strategies() {
        "Enter check    H help    Y why    V view    Esc menu"
    } else {
        "H help    Y why    Enter check    Esc menu"
    };
    text_centered(pm, cxc, cy0 + ch - 36.0, 1.5, footer, GRAY);
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
    let gh = glyph_h(scale);
    let gap = gh * 0.28;

    let field = [&a, &b, &ans].iter().map(|s| text_width(s, scale)).fold(0.0_f32, f32::max);
    let op_col = text_width(&op, scale) + scale * 8.0;
    let total_w = op_col + field;
    let x0 = cxc - total_w / 2.0;
    let field_right = x0 + total_w;
    let right = |pm: &mut Pixmap, s: &str, y: f32, c: Rgb| text(pm, field_right - text_width(s, scale), y, scale, s, c);

    let total_h = 3.0 * gh + 2.0 * gap;
    let y1 = cyc - total_h / 2.0;
    let y2 = y1 + gh + gap;
    let y_div = y2 + gh + gap * 0.4;
    let y3 = y_div + gap * 0.6;

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

    text_centered(pm, cxc, cy0 + ch - 30.0, 1.4, "Up/Down field   Left/Right change   type digits   Esc menu", GRAY);
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
