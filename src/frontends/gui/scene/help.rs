//! Deduction Duck's help (`H`) panel and the "Why am I learning this?" (`Y`)
//! overlay.  Arithmetic gets the duck *acting out* a strategy — walking a number
//! line or smashing a number into place-value pieces — while the other sections
//! get a how-to hint with the duck standing alongside.

use tiny_skia::Pixmap;

use crate::app::App;
use crate::section::Active;

use super::*;

pub fn draw_help(pm: &mut Pixmap, app: &App, (cx0, cy0, cw, ch): (f32, f32, f32, f32)) {
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
        Active::Shape(s) => {
            // Chalk: the hint talks about a colour ("...pieces are green") — swap
            // that for "shaded" so it matches the monochrome figure and question.
            let hint: Vec<String> = if is_chalk() {
                s.hint.iter().map(|h| h.replace(s.color_name, "shaded")).collect()
            } else {
                s.hint.clone()
            };
            hint_body(pm, inner_x, y, body_w, body_bottom, &hint, &s.answer_label(), app.revealed, app.anim_frame);
        }
        Active::Unit(u)     => hint_body(pm, inner_x, y, body_w, body_bottom, &u.hint, &format!("Answer: {} {}", u.answer, u.unit_label), app.revealed, app.anim_frame),
        Active::Geo(g)      => hint_body(pm, inner_x, y, body_w, body_bottom, &g.hint, &g.answer_label(), app.revealed, app.anim_frame),
        Active::Graph(g)    => hint_body(pm, inner_x, y, body_w, body_bottom, &g.hint, &g.answer_label(), app.revealed, app.anim_frame),
        Active::GraphCmp(g) => hint_body(pm, inner_x, y, body_w, body_bottom, &g.hint, &g.answer_label(), app.revealed, app.anim_frame),
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
    for (i, lbl) in labels.iter().enumerate().take(n) {
        let x = x_at(i);
        line(pm, x, axis_y - 7.0, x, axis_y + 7.0, accent, 2.0);
        let green = app.revealed && i == n - 1;
        text_centered(pm, x, label_y, lscale, lbl, if green { ACCENT } else { GRAY });
    }
    for (i, h) in hops.iter().enumerate().take(n - 1) {
        text_centered(pm, (x_at(i) + x_at(i + 1)) / 2.0, hop_y, 1.2, h, ACCENT);
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
pub fn draw_why(pm: &mut Pixmap, app: &App, (cx0, cy0, cw, ch): (f32, f32, f32, f32)) {
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
