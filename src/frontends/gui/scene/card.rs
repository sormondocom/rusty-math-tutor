//! The Practice / Challenge session card — the single problem card that fills
//! the screen for active gameplay, plus the session-screen dispatcher that
//! routes overlays (help, why, transitions, draw-on).

use tiny_skia::Pixmap;

use crate::app::{App, Feedback};
use crate::config::Layout;
use crate::fraction::{FractionProblem, Mode};
use crate::section::Active;

use super::*;

/// The card rectangle `(x, y, w, h)` for the session screen.
fn card_rect(wf: f32, hf: f32) -> (f32, f32, f32, f32) {
    let cw = (wf * 0.86).min(1020.0);
    let ch = (hf * 0.80).min(700.0);
    ((wf - cw) / 2.0, (hf - ch) / 2.0, cw, ch)
}

/// Draw one problem card.  Parameterised (not reading `app.current` directly) so
/// the transition layer can capture both the answered and the next card.
#[allow(clippy::too_many_arguments)]
pub fn draw_card(pm: &mut Pixmap, active: &Active, input: &str, feedback: Feedback, frac: f32, anim: u64, layout: Layout, wf: f32, hf: f32) {
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
            Active::Graph(g) => {
                // Push the chart down so there is breathing room under the question.
                let gband = (cx0 + 50.0, cy0 + 130.0, cw - 100.0, ch * 0.32);
                draw_graph_figure(pm, gband, &g.kind, &g.data);
                answer_y = gband.1 + gband.3 + 26.0;
                ans_scale = 4.0;
            }
            Active::GraphCmp(g) => {
                // Extra headroom for the "Group 1 / Group 2" labels above the charts.
                let gband = (cx0 + 50.0, cy0 + 150.0, cw - 100.0, ch * 0.30);
                let half  = (gband.0, gband.1, gband.2 / 2.0 - 4.0, gband.3);
                let right = (gband.0 + gband.2 / 2.0 + 4.0, gband.1, gband.2 / 2.0 - 4.0, gband.3);
                draw_graph_figure(pm, half,  &g.kind, &g.data_a);
                draw_graph_figure(pm, right, &g.kind, &g.data_b);
                text(pm, half.0,  half.1  - 18.0, 1.3, "Group 1", ACCENT);
                text(pm, right.0, right.1 - 18.0, 1.3, "Group 2", YELLOW);
                answer_y = gband.1 + gband.3 + 26.0;
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

pub fn draw_session(pm: &mut Pixmap, app: &App, wf: f32, hf: f32) {
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


/// Colour of the typed answer: green/red once checked, else cyan when typed.
pub fn ans_color(input: &str, feedback: Feedback) -> Rgb {
    match feedback {
        Feedback::Correct => ACCENT,
        Feedback::Wrong => RED,
        Feedback::None if input.is_empty() => GRAY,
        Feedback::None => CYAN,
    }
}

/// Approximate glyph cap height (logical px) for a given text `scale`.
pub fn glyph_h(scale: f32) -> f32 {
    scale * PX_PER_SCALE * 0.72
}

/// Greedy word-wrap to `max_w` (logical px) at the given `scale`.
pub fn wrap_words(s: &str, max_w: f32, scale: f32) -> Vec<String> {
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

/// The headline string for whichever section produced the active problem.
fn question_of(active: &Active) -> String {
    match active {
        Active::Arith(p) => p.prompt(),
        Active::Unit(u) => u.question.clone(),
        // Chalk is monochrome, so the colour ("...is purple?") is meaningless —
        // ask about the *shaded* (filled) share instead, to match the figure.
        Active::Shape(s) if is_chalk() => shaded_question(s),
        Active::Shape(s)    => s.question(),
        Active::Geo(g)      => g.question(),
        Active::Graph(g)    => g.question_text(),
        Active::GraphCmp(g) => g.question_text(),
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
