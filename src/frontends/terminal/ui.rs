//! All rendering: the start menu, the single-problem card (reused for
//! transition capture), the optional challenge HUD, and Deduction Duck's
//! help overlay.
//!
//! The card is drawn by [`render_card`], which writes directly into a
//! [`Buffer`].  That lets the same routine paint the live frame *and* be
//! captured into off-screen buffers by [`crate::transition`].

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Widget};
use ratatui::Frame;

use crate::app::{App, Cinematic, Feedback, Screen, TeacherView};
use crate::canvas::Canvas;
use crate::config::Layout;
use crate::duck::Pose;
use crate::font;
use crate::problem::{Op, Problem};
use crate::section::Active;
use crate::transition::{Effect, Transition};
use crate::strategy::{Strategy, Viz};
use crate::units::{Theme, UnitProblem};
use crate::{duck, problem, strategy};

/// Deduction Duck's feathers.
const DUCK_COLOR: Color = Color::Rgb(255, 214, 64);
/// Background tint of the help stage panel.
const STAGE_BG: Color = Color::Rgb(28, 30, 44);

/// Background colour shared by every screen.
const BG: Color = Color::Rgb(16, 18, 28);

/// Map a domain [`crate::color::Color`] to the ratatui colour the terminal
/// paints with.  The core describes colours renderer-neutrally; this is where
/// the terminal frontend turns them into cells.
fn rat(c: crate::color::Color) -> Color {
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

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn draw(f: &mut Frame, app: &App, fx: &Transitions) {
    let area = f.area();
    // Paint the whole background first.
    Block::default()
        .style(Style::default().bg(BG))
        .render(area, f.buffer_mut());

    match app.screen {
        Screen::Startup => draw_startup(f, app, area),
        Screen::Menu => draw_menu(f, app, area),
        Screen::Settings => draw_settings(f, app, area),
        Screen::Stats => draw_stats(f, app, area),
        Screen::Teacher => draw_teacher(f, app, area),
        Screen::Cinematic => draw_cinematic(f, app, fx, area),
        Screen::Practice | Screen::Challenge => draw_session(f, app, fx, area),
        Screen::Experiment => draw_experiment(f, app, area),
    }
}

// ---------------------------------------------------------------------------
// Terminal transition visuals
// ---------------------------------------------------------------------------

/// The terminal frontend's transition render-state.  The core ([`App`]) tracks
/// only the timing ([`crate::transition::TransitionPhase`]); this owns the
/// captured cell buffers and particle state that animate it.  Kept in sync with
/// the core each frame via [`Transitions::sync`].
#[derive(Default)]
pub struct Transitions {
    /// Problem-to-problem transition (Practice / Challenge).
    problem: Option<Transition>,
    /// Scene-to-scene transition (milestone cinematics).
    scene: Option<Transition>,
}

impl Transitions {
    /// Reconcile the visuals with the core's phases: build one when a transition
    /// starts, drive its progress while it plays, and drop it when it ends.
    pub fn sync(&mut self, app: &App, area: Rect, rng: &mut impl rand::Rng) {
        match (&app.transition, &mut self.problem) {
            (Some(phase), Some(t)) => t.set_progress(phase.progress),
            (Some(phase), slot @ None) => *slot = Some(build_problem_transition(app, area, phase.effect, rng)),
            (None, slot) => *slot = None,
        }

        let scene_phase = app.cinematic.as_ref().and_then(|c| c.transition);
        match (scene_phase, &mut self.scene) {
            (Some(phase), Some(t)) => t.set_progress(phase.progress),
            (Some(phase), slot @ None) => {
                let cin = app.cinematic.as_ref().expect("scene phase implies a cinematic");
                *slot = Some(build_scene_transition(cin, area, phase.effect, rng));
            }
            (None, slot) => *slot = None,
        }
    }
}

/// Render a single section's card into a fresh buffer (for transition capture).
fn capture_card(area: Rect, active: &Active, input: &str, progress: f32, layout: Layout, banner: Option<(String, Color)>) -> Buffer {
    let mut buf = Buffer::empty(area);
    match active {
        Active::Shape(s) => render_shape_card(area, &mut buf, s, input, progress, banner),
        Active::Unit(u) => render_unit_card(area, &mut buf, u, input, banner),
        Active::Geo(g) => render_geometry_card(area, &mut buf, g, input, banner),
        Active::Arith(p) => render_card(area, &mut buf, p, input, layout, banner),
    }
    buf
}

/// Build the visual for a problem-to-problem transition: the answered card
/// (with its ✓ banner) melting into the next card.
fn build_problem_transition(app: &App, area: Rect, effect: Effect, rng: &mut impl rand::Rng) -> Transition {
    let card = card_area(app.screen, area);
    let from = capture_card(card, &app.current, &app.input, 1.0, app.config.layout, Some(("✓  Correct!".to_string(), Color::LightGreen)));
    let to = match &app.pending {
        // Capture the next card at progress 0 so a shape materialises live after.
        Some(p) => capture_card(card, p, "", 0.0, app.config.layout, None),
        None => Buffer::empty(card),
    };
    Transition::with_effect(effect, from, to, rng)
}

/// Render a cinematic scene into a fresh buffer, frozen at `frame` ticks.
fn capture_scene(area: Rect, scene: &crate::cinematic::Scene, frame: u64) -> Buffer {
    let mut buf = Buffer::empty(area);
    render_scene(area, &mut buf, scene, frame);
    buf
}

/// Build the visual for a scene-to-scene cinematic transition: the current scene
/// (settled) crossfading into the next (at its start frame).
fn build_scene_transition(cin: &Cinematic, area: Rect, effect: Effect, rng: &mut impl rand::Rng) -> Transition {
    let from = capture_scene(area, &cin.scenes[cin.index], cin.scenes[cin.index].dwell as u64);
    let to = capture_scene(area, &cin.scenes[cin.index + 1], 0);
    Transition::with_effect(effect, from, to, rng)
}

// ---------------------------------------------------------------------------
// Milestone cinematics
// ---------------------------------------------------------------------------

/// Draw one cinematic scene into `buf` (also used to capture scenes for the
/// transitions that animate between them).  `frame` is the scene's elapsed tick
/// count, used by animated scenes; still scenes ignore it.
pub fn render_scene(area: Rect, buf: &mut Buffer, scene: &crate::cinematic::Scene, frame: u64) {
    use crate::cinematic::SceneKind;
    Block::default().style(Style::default().bg(BG)).render(area, buf);
    let accent = Style::default().fg(rat(scene.accent));

    match scene.kind {
        SceneKind::NameSky => render_name_sky(area, buf, &scene.heading, &scene.sub, rat(scene.accent), frame),
        SceneKind::RocketName => render_rocket_name(area, buf, &scene.heading, &scene.sub, rat(scene.accent), frame),
        SceneKind::Text => {
            if let Some(big) = &scene.big {
                let by = (area.top() + area.height.saturating_sub(font::GLYPH_H) / 2).saturating_sub(2);
                font::draw_text(buf, center(area, font::text_width(big)), by, big, accent);
                let hy = by + font::GLYPH_H + 1;
                put_str(buf, center(area, scene.heading.chars().count() as u16), hy, &scene.heading, Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
                put_str(buf, center(area, scene.sub.chars().count() as u16), hy + 1, &scene.sub, accent.add_modifier(Modifier::BOLD));
            } else {
                let hy = area.top() + area.height / 2;
                put_str(buf, center(area, scene.heading.chars().count() as u16), hy.saturating_sub(1), &scene.heading, accent.add_modifier(Modifier::BOLD));
                put_str(buf, center(area, scene.sub.chars().count() as u16), hy + 1, &scene.sub, Style::default().fg(Color::White));
            }
        }
    }
}

/// The ASCII moon that rises during the [`SceneKind::NameSky`] scene.
const MOON: [&str; 5] = [
    "  .-\"\"\"-.  ",
    " /       \\ ",
    "|    .    |",
    " \\   .   / ",
    "  '-...-'  ",
];

/// A night sky: twinkling stars, a stream of comets, the student's `name`
/// written among the stars, and a moon rising from below.  `frame` is the
/// scene's elapsed ticks (comets stream with it; the moon rises over the first
/// stretch then rests).
fn render_name_sky(area: Rect, buf: &mut Buffer, name: &str, sub: &str, accent: Color, frame: u64) {
    let (w, h) = (area.width, area.height);

    // Tiny terminals: just the name and a star, no room for the show.
    if w < 16 || h < 9 {
        let line = format!("★  {}  ★", name);
        put_str(buf, center(area, line.chars().count() as u16), area.top() + h / 2, &line, Style::default().fg(accent).add_modifier(Modifier::BOLD));
        return;
    }

    // 1) Twinkling starfield — deterministic positions, phase shifted by frame.
    let star_glyphs = ['·', '.', '✦', '*'];
    let count = (w as u64 * h as u64) / 14;
    for i in 0..count {
        let hsh = i.wrapping_mul(2_654_435_761).wrapping_add(97);
        let sx = area.left() + (hsh % w as u64) as u16;
        let sy = area.top() + ((hsh / 11) % h as u64) as u16;
        if ((hsh / 13 + frame / 5) % 7) < 3 {
            let g = star_glyphs[(hsh % 4) as usize];
            put_str(buf, sx, sy, g.to_string(), Style::default().fg(Color::Rgb(120, 132, 170)));
        }
    }

    // 2) A stream of comets streaking left-to-right across the upper sky.
    let comets = (w / 22).clamp(2, 4) as u64;
    let span = w as u64 + 10;
    for c in 0..comets {
        let row = area.top() + 1 + (c as u16 * 2) % (h / 3).max(1);
        let speed = 1 + (c % 2); // 1–2 cells per tick
        let head = ((frame * speed + c * 23) % span) as i64 - 5;
        for t in 0..6i64 {
            let x = head - t;
            if x < 0 || x as u16 >= w {
                continue;
            }
            let (ch, color) = match t {
                0 => ('★', Color::White),
                1 => ('*', accent),
                _ => ('·', Color::Rgb(110, 140, 180)),
            };
            put_str(buf, area.left() + x as u16, row, ch.to_string(), Style::default().fg(color));
        }
    }

    // 3) The moon rising from below, resting in the lower third.
    let moon_w = MOON[0].chars().count() as u16;
    let moon_h = MOON.len() as u16;
    let rest_top = area.bottom().saturating_sub(moon_h + 1);
    let rise = (frame as f32 / 55.0).clamp(0.0, 1.0);
    let travel = area.bottom().saturating_sub(rest_top) as f32;
    let moon_top = rest_top + ((1.0 - rise) * travel) as u16;
    let moon_x = center(area, moon_w);
    for (i, line) in MOON.iter().enumerate() {
        let y = moon_top + i as u16;
        if y < area.bottom() {
            put_str(buf, moon_x, y, line, Style::default().fg(Color::Rgb(245, 240, 200)).add_modifier(Modifier::BOLD));
        }
    }

    // 4) The name written across the sky, with its tagline beneath.
    let name_line = format!("✦   {}   ✦", name);
    let ny = area.top() + h * 2 / 5;
    put_str(buf, center(area, name_line.chars().count() as u16), ny.saturating_sub(1), clip(&name_line, w), Style::default().fg(accent).add_modifier(Modifier::BOLD));
    put_str(buf, center(area, sub.chars().count() as u16), ny + 1, clip(sub, w), Style::default().fg(Color::White).add_modifier(Modifier::BOLD));

    // 5) Deduction Duck, planted below, gazing up in awe.
    draw_awe_duck(buf, area, frame);
}

/// Deduction Duck standing at the bottom of a celebratory sky, gazing upward in
/// starry-eyed awe.  A no-op when the scene is too small for the sprite.
fn draw_awe_duck(buf: &mut Buffer, area: Rect, frame: u64) {
    if area.width < 24 || area.height < 12 {
        return;
    }
    let x = area.left() + 3;
    let y = area.bottom().saturating_sub(duck::HEIGHT + 1);
    duck::draw_pose(buf, x, y, Pose::Awe, frame / 8, Style::default().fg(DUCK_COLOR));
}

/// Rockets launch from the ground, each one arriving at a letter of the
/// learner's `name` and igniting it as a star; once the name is spelled out, a
/// blue comet sweeps along behind it.  `frame` is the scene's elapsed ticks.
fn render_rocket_name(area: Rect, buf: &mut Buffer, name: &str, sub: &str, accent: Color, frame: u64) {
    let (w, h) = (area.width, area.height);

    // Tiny terminals: skip the show, just present the name.
    if w < 16 || h < 9 {
        let line = format!("★  {}  ★", name);
        put_str(buf, center(area, line.chars().count() as u16), area.top() + h / 2, &line, Style::default().fg(accent).add_modifier(Modifier::BOLD));
        return;
    }

    // 1) Faint twinkling backdrop.
    let star_glyphs = ['·', '.', '✦'];
    let count = (w as u64 * h as u64) / 16;
    for i in 0..count {
        let hsh = i.wrapping_mul(2_654_435_761).wrapping_add(31);
        let sx = area.left() + (hsh % w as u64) as u16;
        let sy = area.top() + ((hsh / 11) % h as u64) as u16;
        if ((hsh / 13 + frame / 6) % 9) < 2 {
            put_str(buf, sx, sy, star_glyphs[(hsh % 3) as usize].to_string(), Style::default().fg(Color::Rgb(90, 100, 140)));
        }
    }

    // Name layout (clipped to the width).
    let chars: Vec<char> = name.chars().take(w.saturating_sub(2) as usize).collect();
    let n = chars.len() as u16;
    let name_x = center(area, n.max(1));
    let ny = area.top() + h * 2 / 5;
    let ground = area.bottom().saturating_sub(1);

    const STAGGER: u64 = 4; // ticks between successive rocket launches
    const RISE: u64 = 22; // ticks for a rocket to climb to its letter
    let lit_at = |i: u64| i * STAGGER + RISE; // frame a letter finishes forming
    let form_done = lit_at(chars.len().saturating_sub(1) as u64);

    // 2) The blue comet — drawn BEFORE the letters so the name occludes it
    //    (it passes behind the name) once the name has formed.
    let comet_start = form_done + 12;
    let comet_len = 50u64;
    if frame >= comet_start {
        let p = (frame - comet_start) as f32 / comet_len as f32;
        let span = w as i64 + 18;
        let head = -9 + (p * span as f32) as i64;
        for t in 0..9i64 {
            let x = head - t;
            if x < 0 || x as u16 >= w {
                continue;
            }
            let (ch, color) = match t {
                0 => ('★', Color::Rgb(170, 210, 255)),
                1 | 2 => ('*', Color::Rgb(90, 150, 255)),
                _ => ('·', Color::Rgb(55, 95, 200)),
            };
            put_str(buf, area.left() + x as u16, ny, ch.to_string(), Style::default().fg(color));
        }
    }

    // 3) Rockets rising, then their letters lit as stars.
    for (i, &ch) in chars.iter().enumerate() {
        let tx = name_x + i as u16;
        if ch == ' ' {
            continue;
        }
        let start = i as u64 * STAGGER;
        if frame < start {
            continue;
        }
        let prog = ((frame - start) as f32 / RISE as f32).clamp(0.0, 1.0);
        if prog < 1.0 {
            // A rocket climbing from the ground up to the name row.
            let cur_y = ny + ((1.0 - prog) * (ground - ny) as f32) as u16;
            for (t, (g, col)) in [('!', Color::Rgb(255, 190, 70)), (':', Color::Rgb(255, 120, 40)), ('.', Color::Rgb(180, 60, 30))].iter().enumerate() {
                let ey = cur_y + 1 + t as u16;
                if ey <= ground {
                    put_str(buf, tx, ey, g.to_string(), Style::default().fg(*col));
                }
            }
            put_str(buf, tx, cur_y, "▲".to_string(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
        } else {
            // Letter formed: a bright star that flares white as it ignites.
            let fresh = frame < lit_at(i as u64) + 6;
            let style = Style::default().fg(if fresh { Color::White } else { accent }).add_modifier(Modifier::BOLD);
            put_str(buf, tx, ny, ch.to_string(), style);
            if fresh && ny > area.top() {
                put_str(buf, tx, ny - 1, "✦".to_string(), Style::default().fg(Color::LightYellow));
            }
        }
    }

    // 4) Tagline, once the name is fully written.
    if frame >= form_done {
        put_str(buf, center(area, sub.chars().count() as u16), ny + 2, clip(sub, w), Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
    }

    // 5) Deduction Duck, planted below, gazing up in awe at the launch.
    draw_awe_duck(buf, area, frame);
}

fn draw_cinematic(f: &mut Frame, app: &App, fx: &Transitions, area: Rect) {
    let buf = f.buffer_mut();
    if let Some(t) = &fx.scene {
        // A scene-to-scene transition is playing (owned by the frontend).
        t.render(area, buf);
    } else if let Some(c) = &app.cinematic {
        if let Some(scene) = c.scenes.get(c.index) {
            // Elapsed ticks into this scene (dwell counts down from scene.dwell).
            let frame = scene.dwell.saturating_sub(c.dwell) as u64;
            render_scene(area, buf, scene, frame);
        }
    }
    put_str(buf, center(area, 21), area.bottom().saturating_sub(1), "press any key to skip", Style::default().fg(Color::DarkGray));
}

// ---------------------------------------------------------------------------
// Units of Measure (a card shown inside Practice / Challenge sessions)
// ---------------------------------------------------------------------------

/// Draw a measurement card: question on top, the answer to type in the middle,
/// and the themed prop at the bottom.  Used live and for transition capture.
pub fn render_unit_card(area: Rect, buf: &mut Buffer, p: &UnitProblem, input: &str, banner: Option<(String, Color)>) {
    let accent = theme_color(p.theme);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(accent))
        .title(" Units of Measure ")
        .style(Style::default().bg(BG));
    let inner = block.inner(area);
    block.render(area, buf);

    // Question, word-wrapped and centred near the top.
    let qlines = wrap_text(&p.question, inner.width.saturating_sub(4));
    let mut y = inner.top() + 1;
    for line in qlines.iter().take(4) {
        put_str(buf, center(inner, line.chars().count() as u16), y, line, Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
        y += 1;
    }

    // The big answer the student types, with its unit beneath.
    let ans = if input.is_empty() { "?" } else { input };
    let ay = inner.top() + inner.height / 2;
    font::draw_text(buf, center(inner, font::text_width(ans)), ay.saturating_sub(2), ans, answer_style(input));
    put_str(buf, center(inner, p.unit_label.chars().count() as u16), ay + font::GLYPH_H.saturating_sub(2), &p.unit_label, Style::default().fg(accent).add_modifier(Modifier::BOLD));

    if let Some((text, color)) = banner {
        put_str(buf, center(inner, text.chars().count() as u16), ay + font::GLYPH_H + 1, text, Style::default().fg(color).add_modifier(Modifier::BOLD));
    }

    // Static themed prop at the bottom (the duck gag animates over this).
    let (px, py, prop) = prop_placement(inner, p.theme);
    for (i, line) in prop.iter().enumerate() {
        put_str(buf, px, py + i as u16, line, Style::default().fg(accent));
    }

    let hints = "Enter check    H help duck    Esc menu";
    put_str(buf, center(inner, hints.chars().count() as u16), inner.bottom().saturating_sub(1), hints, Style::default().fg(Color::DarkGray));
}

// ---------------------------------------------------------------------------
// Geometry — hollow (outlined) shapes for perimeter, area, and volume
// ---------------------------------------------------------------------------

/// Geometry's accent colour.
const GEO_ACCENT: Color = Color::LightGreen;
/// Colour for the dimension numbers labelled on a shape's edges.
const GEO_DIM: Color = Color::LightYellow;

/// Draw a geometry card: the question, an outlined shape with its dimensions
/// labelled, and the answer the student types (with its unit).
pub fn render_geometry_card(area: Rect, buf: &mut Buffer, p: &crate::geometry::GeometryProblem, input: &str, banner: Option<(String, Color)>) {
    use crate::geometry::GeoShape;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(GEO_ACCENT))
        .title(" Geometry ")
        .style(Style::default().bg(BG));
    let inner = block.inner(area);
    block.render(area, buf);

    // Question.
    let q = p.question();
    let mut y = inner.top() + 1;
    for line in wrap_text(&q, inner.width.saturating_sub(4)).iter().take(2) {
        put_str(buf, center(inner, line.chars().count() as u16), y, line, Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
        y += 1;
    }

    // The outlined shape occupies the middle band.
    let answer_h = 4;
    let band = Rect {
        x: inner.left() + 1,
        y: y + 1,
        width: inner.width.saturating_sub(2),
        height: inner.bottom().saturating_sub(y + 1 + answer_h + 1),
    };
    if band.height >= 4 && band.width >= 12 {
        match p.shape {
            GeoShape::Rect { w, h } => draw_geo_rect(buf, band, w, h),
            GeoShape::Triangle { base, height, sides } => draw_geo_triangle(buf, band, base, height, sides),
            GeoShape::Circle { r } => draw_geo_circle(buf, band, r),
            GeoShape::Box3 { l, w, h } => draw_geo_box(buf, band, l, w, h),
        }
    }

    // The answer the student types, with its unit (a π coefficient for circles).
    let ay = inner.bottom().saturating_sub(answer_h + 1);
    put_str(buf, center(inner, 12), ay, "Your answer:", Style::default().fg(Color::Gray));
    let num = if input.is_empty() { "?" } else { input };
    let shown = format!("{}{} {}", num, if p.pi { "π" } else { "" }, p.unit);
    put_str(buf, center(inner, shown.chars().count() as u16), ay + 1, &shown, Style::default().fg(Color::White).add_modifier(Modifier::BOLD));

    if let Some((text, color)) = banner {
        put_str(buf, center(inner, text.chars().count() as u16), ay + 2, &text, Style::default().fg(color).add_modifier(Modifier::BOLD));
    }

    let hints = "Type a whole number    Enter check    H help duck    Esc menu";
    put_str(buf, center(inner, hints.chars().count() as u16), inner.bottom().saturating_sub(1), hints, Style::default().fg(Color::DarkGray));
}

/// Map a shape's unit dimensions to cell dimensions, honouring the ~2:1
/// width:height aspect of a terminal cell, fitting within `(max_w, max_h)`.
fn fit_cells(uw: i64, uh: i64, max_w: u16, max_h: u16) -> (u16, u16) {
    let want_w = uw.max(1) as f32 * 2.0;
    let want_h = uh.max(1) as f32;
    let s = (max_w as f32 / want_w).min(max_h as f32 / want_h).min(2.4).max(0.05);
    let cw = ((want_w * s).round() as u16).clamp(6, max_w.max(6));
    let ch = ((want_h * s).round() as u16).clamp(2, max_h.max(2));
    (cw, ch)
}

/// A hollow rectangle (or square), scaled to its proportions, with the width
/// labelled below and the height to the right.
fn draw_geo_rect(buf: &mut Buffer, band: Rect, w: i64, h: i64) {
    let style = Style::default().fg(GEO_ACCENT);
    let (bw, bh) = fit_cells(w, h, band.width.saturating_sub(8).min(40), band.height.saturating_sub(3).min(9));
    let x0 = center(band, bw);
    let y0 = band.top() + band.height.saturating_sub(bh + 1) / 2;

    put_str(buf, x0, y0, format!("┌{}┐", "─".repeat((bw - 2) as usize)), style);
    for r in 1..bh - 1 {
        put_str(buf, x0, y0 + r, "│", style);
        put_str(buf, x0 + bw - 1, y0 + r, "│", style);
    }
    put_str(buf, x0, y0 + bh - 1, format!("└{}┘", "─".repeat((bw - 2) as usize)), style);

    // Dimensions: width centred below the box, height to the right.
    let wl = w.to_string();
    put_str(buf, x0 + bw / 2 - (wl.len() as u16 / 2), y0 + bh, &wl, Style::default().fg(GEO_DIM));
    let hl = h.to_string();
    put_str(buf, x0 + bw + 1, y0 + bh / 2, &hl, Style::default().fg(GEO_DIM));
}

/// A hollow isosceles triangle with contiguous `/ \` slopes (one column per row,
/// so the edges never break up), labelled with its base+height (area) or its
/// three side lengths (perimeter).
fn draw_geo_triangle(buf: &mut Buffer, band: Rect, base: i64, height: i64, sides: Option<[i64; 3]>) {
    // Draw the outline on a half-block canvas so the slopes come out smooth.
    let cell_rows = band.height.saturating_sub(2).clamp(3, 12);
    let mut cv = Canvas::new(band.width, cell_rows);
    let px_h = cv.rows() as i32;
    let cx = (cv.cols() / 2) as i32;
    let (top, bot) = (1, px_h - 2);
    // Isosceles; base half-width tracks the height (square pixels keep the
    // slopes natural), clamped to the canvas.
    let half = (bot - top).clamp(2, cx - 1);
    cv.line(cx, top, cx - half, bot, GEO_ACCENT); // left slope
    cv.line(cx, top, cx + half, bot, GEO_ACCENT); // right slope
    cv.line(cx - half, bot, cx + half, bot, GEO_ACCENT); // base
    // For an area triangle, drop the height down the middle to match the label.
    if sides.is_none() {
        cv.line(cx, top, cx, bot, GEO_DIM);
    }
    cv.blit(buf, band.left(), band.top());

    let label = match sides {
        Some([a, b, c]) => format!("sides: {}, {}, {} cm", a, b, c),
        None => format!("base {} cm,  height {} cm", base, height),
    };
    put_str(buf, center(band, label.chars().count() as u16), band.top() + cell_rows + 1, &label, Style::default().fg(GEO_DIM));
}

/// A hollow circle on a half-block canvas — a genuinely round outline sized to
/// its radius, with a radius spoke and label.  Circumference / area are asked
/// in terms of π.
fn draw_geo_circle(buf: &mut Buffer, band: Rect, r: i64) {
    let cell_rows = band.height.saturating_sub(2).clamp(3, 12);
    let mut cv = Canvas::new(band.width, cell_rows);
    let (cx, cy) = ((cv.cols() / 2) as i32, cv.rows() as i32 / 2);
    let fit = cx.min(cy).saturating_sub(1).max(2);
    // Grow a little with r (2..=9), but keep it filling the band.
    let rad = (fit * ((r as i32).clamp(2, 9) + 11) / 20).clamp(2, fit);
    cv.circle(cx, cy, rad, GEO_ACCENT);
    cv.line(cx, cy, cx + rad, cy, GEO_DIM); // radius spoke
    cv.blit(buf, band.left(), band.top());

    let label = format!("radius {} cm", r);
    put_str(buf, center(band, label.chars().count() as u16), band.top() + cell_rows + 1, &label, Style::default().fg(GEO_DIM));
}

/// A crisp wireframe cuboid (or cube) in cabinet projection, scaled to its
/// length × height with a depth offset from its width, edges labelled.
fn draw_geo_box(buf: &mut Buffer, band: Rect, l: i64, w: i64, h: i64) {
    let style = Style::default().fg(GEO_ACCENT);
    let depth = ((w as u16 + 1) / 2).clamp(2, 3);
    let (fw, fh) = fit_cells(
        l,
        h,
        band.width.saturating_sub(8 + depth).min(34),
        band.height.saturating_sub(4 + depth).min(8),
    );
    let fw = fw.max(6);
    let fh = fh.max(3);
    // Front-face top-left, leaving room above/right for the depth and labels.
    let x0 = band.left() + (band.width.saturating_sub(fw + depth)) / 2;
    let y0 = band.top() + depth + 1;

    // Front face.
    put_str(buf, x0, y0, format!("┌{}┐", "─".repeat((fw - 2) as usize)), style);
    for r in 1..fh - 1 {
        put_str(buf, x0, y0 + r, "│", style);
        put_str(buf, x0 + fw - 1, y0 + r, "│", style);
    }
    put_str(buf, x0, y0 + fh - 1, format!("└{}┘", "─".repeat((fw - 2) as usize)), style);

    // Depth: diagonals up-right from the two top corners and the bottom-right
    // corner, plus the visible back top edge and back right edge.
    for i in 1..depth {
        put_str(buf, x0 + i, y0.saturating_sub(i), "╱", style);
        put_str(buf, x0 + fw - 1 + i, y0.saturating_sub(i), "╱", style);
        put_str(buf, x0 + fw - 1 + i, y0 + fh - 1 - i, "╱", style);
    }
    let bx = x0 + depth;
    let by = y0.saturating_sub(depth);
    put_str(buf, bx, by, format!("┌{}┐", "─".repeat((fw - 2) as usize)), style);
    for r in 1..fh - 1 {
        put_str(buf, bx + fw - 1, by + r, "│", style);
    }
    put_str(buf, bx + fw - 1, by + fh - 1, "┘", style);

    // Edge labels: length below, height to the left, width along the top depth.
    let ll = l.to_string();
    put_str(buf, x0 + fw / 2 - (ll.len() as u16 / 2), y0 + fh, &ll, Style::default().fg(GEO_DIM));
    let hl = h.to_string();
    put_str(buf, x0.saturating_sub(1 + hl.len() as u16), y0 + fh / 2, &hl, Style::default().fg(GEO_DIM));
    // Width (depth) just past the back-top-right corner, with a gap.
    let wl = w.to_string();
    put_str(buf, bx + fw + 1, by, &wl, Style::default().fg(GEO_DIM));
}

fn theme_color(theme: Theme) -> Color {
    match theme {
        Theme::Cup => Color::LightYellow,
        Theme::Pool => Color::LightBlue,
        Theme::Bottle => Color::LightCyan,
        Theme::Scale => Color::LightMagenta,
        Theme::Ruler => Color::LightGreen,
        Theme::Coin => Color::Rgb(255, 200, 40), // gold
    }
}

/// The little ASCII prop for a theme.
fn theme_prop(theme: Theme) -> &'static [&'static str] {
    match theme {
        Theme::Cup => &[" ___", "|  |)", "|__|"],
        Theme::Pool => &["~~~~~~~~", "~~~~~~~~"],
        Theme::Bottle => &[" __ ", "|  |", "|~~|", "|__|"],
        Theme::Scale => &[" _^_ ", "/ | \\", "====="],
        Theme::Ruler => &["|.|.|.|.|"],
        Theme::Coin => &["( $ )", "(===)", "(===)"],
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

/// Bottom-centred placement of the prop within `inner`.
fn prop_placement(inner: Rect, theme: Theme) -> (u16, u16, &'static [&'static str]) {
    let prop = theme_prop(theme);
    let w = prop.iter().map(|l| l.chars().count()).max().unwrap_or(1) as u16;
    let x = center(inner, w);
    let y = inner.bottom().saturating_sub(prop.len() as u16 + 1);
    (x, y, prop)
}

/// The silly gag: Deduction Duck hops in from the side, splashes into the prop,
/// then pops back out — on a loop.
fn draw_duck_gag(buf: &mut Buffer, inner: Rect, theme: Theme, frame: u64) {
    let (px, py, prop) = prop_placement(inner, theme);
    let pw = prop.iter().map(|l| l.chars().count()).max().unwrap_or(1) as u16;
    let cx = (px + pw / 2) as f32;
    let top = py as f32;
    let color = theme_color(theme);
    let mini = "~(o>"; // tiny duck facing right

    const CYCLE: u64 = 96;
    let t = (frame % CYCLE) as f32 / CYCLE as f32;
    let land_x = cx - 2.0;

    if t < 0.45 {
        // Arc in from the left edge toward the prop.
        let p = t / 0.45;
        let start_x = inner.left() as f32 + 1.0;
        let x = start_x + (land_x - start_x) * p;
        let y = top - 1.0 - (p * std::f32::consts::PI).sin() * 5.0;
        put_float(buf, x, y, mini, Style::default().fg(color));
    } else if t < 0.58 {
        // Splash! — the word above, droplets down on the water rows.
        let word = theme_splash(theme);
        put_float(buf, cx - word.chars().count() as f32 / 2.0, top - 2.0, word, Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD));
        for (dx, dy) in [(-4.0, 0.0), (4.0, 0.0), (-3.0, 1.0), (3.0, 1.0)] {
            put_float(buf, cx + dx, top + dy, "°", Style::default().fg(Color::LightCyan));
        }
    } else {
        // Pop back up out of the prop.
        let p = (t - 0.58) / 0.42;
        let y = top - 1.0 - (p * std::f32::consts::PI).sin() * 4.0;
        put_float(buf, land_x, y, mini, Style::default().fg(color));
    }
}

/// Place a short string at floating-point coordinates, clipped to the buffer.
fn put_float(buf: &mut Buffer, x: f32, y: f32, s: &str, style: Style) {
    if !x.is_finite() || !y.is_finite() || x < 0.0 || y < 0.0 {
        return;
    }
    put_str(buf, x.round() as u16, y.round() as u16, s, style);
}

// ---------------------------------------------------------------------------
// Experimentation — free-form unit explorer
// ---------------------------------------------------------------------------

/// Deduction Duck only reacts when an experiment lands on something famous —
/// that way kids have to *induce* a reaction by trying interesting amounts.
enum ExpReaction {
    /// Most experiments: he stays out of it.
    None,
    /// The amount is about the size of a real-world thing.
    Match(String),
    /// Astronomically bigger than anything on the list — hat-explode time.
    Boom(String),
}

/// Decide the duck's reaction from the *physical* size in base units (so it's
/// the same whichever target unit is on screen) — deterministic, so a fun
/// discovery can be reproduced.
fn experiment_reaction(cat: crate::units::Category, base_value: f64) -> ExpReaction {
    let refs = crate::units::comparisons(cat);
    if refs.is_empty() || !(base_value > 0.0) {
        return ExpReaction::None;
    }
    // Closest reference by ratio (log distance).
    let best = refs.iter().min_by(|a, b| {
        let da = (base_value / a.base).ln().abs();
        let db = (base_value / b.base).ln().abs();
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

// ---------------------------------------------------------------------------
// Fractions — a shape that materialises, with a typed a/b answer
// ---------------------------------------------------------------------------

/// Draw a fraction problem: the question up top, the materialising shape in the
/// middle, and the student's stacked `a/b` answer beneath.
/// A visual shape card, shared by the Fractions and Percentages sections.  The
/// figure and question come from the problem; the answer area and footer adapt
/// to its [`Mode`] — a stacked `a/b` for fractions, a single `n%` for percents.
pub fn render_shape_card(area: Rect, buf: &mut Buffer, p: &crate::fraction::FractionProblem, input: &str, progress: f32, banner: Option<(String, Color)>) {
    use crate::fraction::Mode;
    let title = match p.mode {
        Mode::Fraction => " Fractions ",
        Mode::Percent => " Percentages ",
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(rat(p.shaded_color)))
        .title(title)
        .style(Style::default().bg(BG));
    let inner = block.inner(area);
    block.render(area, buf);

    // Question.
    let q = p.question();
    let mut y = inner.top() + 1;
    for line in wrap_text(&q, inner.width.saturating_sub(4)).iter().take(2) {
        put_str(buf, center(inner, line.chars().count() as u16), y, line, Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
        y += 1;
    }

    // The materialising shape occupies the middle band.
    let answer_h = 5; // "Your answer:" + the (stacked or inline) answer
    let shape_area = Rect {
        x: inner.left() + 2,
        y: y + 1,
        width: inner.width.saturating_sub(4),
        height: inner.bottom().saturating_sub(y + 1 + answer_h + 2),
    };
    if shape_area.height >= 3 {
        crate::shapes::render(buf, shape_area, p.shape, progress, |i| rat(p.color_of(i)));
    }

    let ay = inner.bottom().saturating_sub(answer_h + 1);
    put_str(buf, center(inner, 12), ay, "Your answer:", Style::default().fg(Color::Gray));
    let style = Style::default().fg(Color::White).add_modifier(Modifier::BOLD);
    match p.mode {
        Mode::Fraction => {
            // Stacked as a fraction: numerator over a bar over the denominator.
            let (num, den) = match input.split_once('/') {
                Some((a, b)) => (if a.is_empty() { "?" } else { a }, if b.is_empty() { "?" } else { b }),
                None => (if input.is_empty() { "?" } else { input }, "?"),
            };
            let bar_w = num.chars().count().max(den.chars().count()).max(1) as u16 + 2;
            put_str(buf, center(inner, num.chars().count() as u16), ay + 1, num, style);
            put_str(buf, center(inner, bar_w), ay + 2, &"─".repeat(bar_w as usize), Style::default().fg(rat(p.shaded_color)));
            put_str(buf, center(inner, den.chars().count() as u16), ay + 3, den, style);
        }
        Mode::Percent => {
            // A single whole number with a percent sign.
            let shown = if input.is_empty() { "?".to_string() } else { format!("{}%", input) };
            put_str(buf, center(inner, shown.chars().count() as u16), ay + 2, &shown, style);
        }
    }

    if let Some((text, color)) = banner {
        put_str(buf, center(inner, text.chars().count() as u16), ay + 4, &text, Style::default().fg(color).add_modifier(Modifier::BOLD));
    }

    let hints = match p.mode {
        Mode::Fraction => "Type like 2/4    Enter check    H help duck    Esc menu",
        Mode::Percent => "Type a number like 25    Enter check    H help duck    Esc menu",
    };
    put_str(buf, center(inner, hints.chars().count() as u16), inner.bottom().saturating_sub(1), hints, Style::default().fg(Color::DarkGray));
}

fn draw_experiment(f: &mut Frame, app: &App, area: Rect) {
    use crate::units::{self, Category};

    let cat = Category::ALL[app.exp_category];
    let units_list = cat.units();
    let from = units_list[app.exp_from.min(units_list.len() - 1)];
    let to = units_list[app.exp_to.min(units_list.len() - 1)];
    let amount: f64 = app.exp_amount.parse().unwrap_or(0.0);
    let result = units::convert(amount, from, to);

    let buf = f.buffer_mut();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::LightMagenta))
        .title(" Experimentation — Unit Explorer ")
        .style(Style::default().bg(BG));
    let inner = block.inner(area);
    block.render(area, buf);

    put_str(buf, inner.left() + 2, inner.top() + 1, "Type any amount — even a silly one — and watch it convert!", Style::default().fg(Color::Gray));

    // Editable fields.
    let amount_display = if app.exp_amount.is_empty() { "0".to_string() } else { app.exp_amount.clone() };
    let caret = if app.exp_field == 0 { "_" } else { "" };
    let rows = [
        (0usize, "Amount".to_string(), format!("{}{}", amount_display, caret)),
        (1, "From".to_string(), format!("< {} >", from.plural)),
        (2, "To".to_string(), format!("< {} >", to.plural)),
        (3, "Category".to_string(), format!("< {} >", cat.name())),
    ];
    let col = inner.left() + 4;
    let mut y = inner.top() + 3;
    for (i, label, value) in &rows {
        let selected = *i == app.exp_field;
        let style = if selected {
            Style::default().fg(Color::Black).bg(Color::LightMagenta).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        put_str(buf, col, y, format!("{:<10}{}", label, value), style);
        y += 2;
    }

    // The result, shown large and friendly.
    let line = format!("= {} {}", units::format_amount(result), to.plural);
    put_str(buf, center(inner, line.chars().count() as u16), inner.top() + 10, &line, Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD));
    let recap = format!("{} {} is...", units::format_amount(amount), from.plural);
    put_str(buf, center(inner, recap.chars().count() as u16), inner.top() + 9, &recap, Style::default().fg(Color::Gray));

    // Deduction Duck only pops up (centre stage) for an interesting result —
    // the physical size, so the same landmark shows whichever unit is chosen.
    let base_value = amount * from.to_base;
    draw_exp_reaction(buf, inner, experiment_reaction(cat, base_value), app.anim_frame);

    put_str(buf, inner.left() + 2, inner.bottom().saturating_sub(1), "Up/Down field   Left/Right change   type digits   Esc menu", Style::default().fg(Color::DarkGray));
}

/// Draw Deduction Duck's reaction, centred near the bottom of the explorer.
fn draw_exp_reaction(buf: &mut Buffer, inner: Rect, reaction: ExpReaction, frame: u64) {
    let (fact, boom) = match reaction {
        ExpReaction::None => return, // he stays quiet — keep experimenting!
        ExpReaction::Match(fact) => (fact, false),
        ExpReaction::Boom(fact) => (fact, true),
    };

    // Centre stage, lower middle.  The duck stays put; its life comes from the
    // waddle frame so the fixed text above it never gets overrun.
    let duck_x = center(inner, duck::WIDTH);
    let duck_y = inner.bottom().saturating_sub(duck::HEIGHT + 1);

    // Boom needs an extra line above for the bigger headline.
    let head_row = duck_y.saturating_sub(if boom { 3 } else { 2 });
    let fact_row = duck_y.saturating_sub(if boom { 2 } else { 1 });

    let headline = if boom { "BOOM!" } else { "Whoa!" };
    let head_color = if boom { Color::LightRed } else { Color::LightYellow };
    put_str(buf, center(inner, headline.chars().count() as u16), head_row, headline, Style::default().fg(head_color).add_modifier(Modifier::BOLD));
    put_str(buf, center(inner, fact.chars().count() as u16), fact_row, &fact, Style::default().fg(head_color));

    duck::draw_pose(buf, duck_x, duck_y, Pose::Stand, frame / 5, Style::default().fg(DUCK_COLOR));

    if boom {
        // Blow the mortarboard off in a small ring around the head (kept clear
        // of the fact line two rows above).
        for dx in 1..8u16 {
            put_str(buf, duck_x + dx, duck_y, " ", Style::default());
            put_str(buf, duck_x + dx, duck_y + 1, " ", Style::default());
        }
        let cx = duck_x as f32 + 4.0;
        let cy = duck_y as f32;
        let r = 1.0 + (frame % 12) as f32 * 0.2;
        let pieces = ['*', '✦', '!', '+', '#', '°'];
        for (k, ch) in pieces.iter().enumerate() {
            let ang = k as f32 * std::f32::consts::TAU / pieces.len() as f32;
            put_float(buf, cx + ang.cos() * r, cy + ang.sin() * r * 0.4, &ch.to_string(), Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD));
        }
    } else if (frame / 5) % 2 == 0 {
        // A couple of sparkles of delight flanking the duck.
        put_str(buf, duck_x.saturating_sub(2), duck_y + 1, "✦", Style::default().fg(Color::LightYellow));
        put_str(buf, duck_x + duck::WIDTH + 1, duck_y + 1, "✦", Style::default().fg(Color::LightYellow));
    }
}

// ---------------------------------------------------------------------------
// Startup — graphics mode picker
// ---------------------------------------------------------------------------

fn draw_startup(f: &mut Frame, app: &App, area: Rect) {
    use crate::config::GraphicsMode;
    let buf = f.buffer_mut();

    let title = "1 + 1 = ?";
    font::draw_text(buf, center(area, font::text_width(title)), area.top() + 2, title, Style::default().fg(Color::LightCyan));
    put_str(buf, center(area, 17), area.top() + 8, "RUSTY MATH TUTOR", Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
    let featuring = "Featuring:  Deduction Duck";
    put_str(buf, center(area, featuring.chars().count() as u16), area.top() + 9, featuring, Style::default().fg(DUCK_COLOR).add_modifier(Modifier::BOLD));
    put_str(buf, center(area, 18), area.top() + 10, "Choose how to draw:", Style::default().fg(Color::Gray));

    let modes = [GraphicsMode::Low, GraphicsMode::Cpu];
    let col = area.left() + area.width.saturating_sub(38) / 2;
    let mut y = area.top() + 13;
    for (i, m) in modes.iter().enumerate() {
        let selected = i == app.startup_index;
        let style = if selected {
            Style::default().fg(Color::Black).bg(Color::LightCyan).add_modifier(Modifier::BOLD)
        } else if m.available() {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let cursor = if selected { "➤ " } else { "  " };
        put_str(buf, col, y, format!("{}{:<34}", cursor, m.name()), style);
        y += 2;
    }

    put_str(buf, center(area, 46), area.bottom().saturating_sub(4), "CPU graphics (2D/3D, no GPU needed) is coming soon.", Style::default().fg(Color::DarkGray));
    put_str(buf, center(area, 34), area.bottom().saturating_sub(2), "Up / Down to choose    Enter to start", Style::default().fg(Color::DarkGray));
}

// ---------------------------------------------------------------------------
// Menu
// ---------------------------------------------------------------------------

fn draw_menu(f: &mut Frame, app: &App, area: Rect) {
    let buf = f.buffer_mut();

    let title = "1 + 1 = ?";
    font::draw_text(buf, center(area, font::text_width(title)), area.top() + 1, title, Style::default().fg(Color::LightCyan));
    put_str(buf, center(area, 17), area.top() + 6, "RUSTY MATH TUTOR", Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
    let featuring = "Featuring:  Deduction Duck";
    put_str(buf, center(area, featuring.chars().count() as u16), area.top() + 7, featuring, Style::default().fg(DUCK_COLOR).add_modifier(Modifier::BOLD));

    let col = area.left() + area.width.saturating_sub(42) / 2;
    let sel = app.menu_index;

    // The menu as a flat list of (label, hint) — one line each, so it can be
    // windowed when the terminal is short.
    let mark = |on: bool| if on { "[x]" } else { "[ ]" };
    let mut items: Vec<(String, &str)> = vec![
        (format!("Student:  < {} >", app.roster.current().name), "(<>  N: add new)"),
        (format!("Grade level:  < {} >", grade_name(app.menu_grade)), "(Left / Right)"),
    ];
    for (i, op) in problem::Op::ALL.iter().enumerate() {
        items.push((format!("{} {}", mark(app.menu_ops[i]), op.name()), "(Enter toggles)"));
    }
    items.push((format!("{} Units of Measure", mark(app.menu_units)), "(measuring problems)"));
    items.push((format!("{} Fractions", mark(app.menu_fractions)), "(shapes & pieces)"));
    items.push((format!("{} Percentages", mark(app.menu_percents)), "(shapes out of 100)"));
    items.push((format!("{} Geometry", mark(app.menu_geometry)), "(perimeter, area, volume)"));
    items.push(("Settings (number ranges)...".to_string(), "(Enter to open)"));
    items.push(("My Progress...".to_string(), "(Enter to view)"));
    items.push(("Teacher Area...".to_string(), "(password)"));
    items.push(("▶  Start Practice".to_string(), "(no timer)"));
    items.push(("▶  Start Challenge".to_string(), "(60-second timer)"));
    items.push(("▶  Experimentation".to_string(), "(explore unit conversions)"));

    // Scroll the window so the selected item stays visible.
    let (scroll, visible) = scroll_window(items.len(), sel, area.top() + 8, area.bottom().saturating_sub(2));
    let list_top = area.top() + 8;
    let mut y = list_top;
    for (idx, (label, hint)) in items.iter().enumerate().skip(scroll).take(visible) {
        draw_menu_row(buf, col, y, idx == sel, label, hint);
        y += 1;
    }
    if scroll > 0 {
        put_str(buf, area.right().saturating_sub(9), list_top, "↑ more", Style::default().fg(Color::LightCyan));
    }
    if scroll + visible < items.len() {
        put_str(buf, area.right().saturating_sub(9), y.saturating_sub(1), "↓ more", Style::default().fg(Color::LightCyan));
    }

    put_str(buf, col, area.bottom().saturating_sub(1), "Up / Down to move    N: add student    Q: quit", Style::default().fg(Color::DarkGray));

    // Deduction Duck wanders by every so often.
    draw_menu_duck(buf, area, app.anim_frame);

    // New-student name entry.
    if app.naming {
        draw_name_prompt(buf, area, &app.name_input);
    }
}

/// Every so often Deduction Duck wanders by the menu: peeking in from a side
/// (clear of the centred text) or, when there's room below the menu, waddling
/// right across the bottom.  Driven entirely by the free-running clock.
fn draw_menu_duck(buf: &mut Buffer, area: Rect, frame: u64) {
    let style = Style::default().fg(DUCK_COLOR);
    let hgt = duck::HEIGHT as i32;

    // One appearance per long cycle; off-stage the rest of the time.
    const CYCLE: u64 = 640;
    let phase = frame % CYCLE;
    let y_at = |frac: f32| area.top() as i32 + (area.height as f32 * frac) as i32 - hgt / 2;

    match (frame / CYCLE) % 4 {
        // A full waddle across the bottom — only where the menu leaves space.
        0 if (area.bottom() as i32 - hgt) > area.top() as i32 + 27 => {
            const DUR: u64 = 200;
            if phase < DUR {
                let t = phase as f32 / DUR as f32;
                let span = (area.width as i32 + 2 * duck::WIDTH as i32 + 4) as f32;
                let x = (area.left() as i32 - duck::WIDTH as i32 - 2) as f32 + t * span;
                let y = area.bottom() as i32 - hgt - if (frame / 6) % 2 == 0 { 0 } else { 1 };
                duck::draw_pose_i32(buf, x.round() as i32, y, Pose::WalkRight, frame / 5, style);
            }
        }
        // Peek in from the right at a low spot.
        0 | 2 => peek_in(buf, area, phase, frame, true, y_at(0.62), style),
        // Peek in from the right, higher up.
        1 => peek_in(buf, area, phase, frame, true, y_at(0.32), style),
        // Peek in from the left.
        _ => peek_in(buf, area, phase, frame, false, y_at(0.5), style),
    }
}

/// Slide the duck in from a side edge, hold with a little "hi!", then back out.
fn peek_in(buf: &mut Buffer, area: Rect, phase: u64, frame: u64, from_right: bool, y: i32, style: Style) {
    use std::f32::consts::PI;
    const DUR: u64 = 150;
    if phase >= DUR {
        return;
    }
    let w = duck::WIDTH as i32;
    let t = phase as f32 / DUR as f32;
    let depth = ((t * PI).sin() * (w as f32 + 2.0)).round() as i32;
    let saying = (0.4..0.62).contains(&t);
    let hi = Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD);

    let x = if from_right {
        area.right() as i32 - depth
    } else {
        area.left() as i32 - w + depth
    };
    let pose = if from_right { Pose::Stand } else { Pose::WalkRight };
    duck::draw_pose_i32(buf, x, y, pose, frame / 8, style);

    // A little "hi!" just above his head, where it can't collide with the
    // centred menu text.
    if saying && y > area.top() as i32 {
        let hx = (x + 2).clamp(area.left() as i32, area.right() as i32 - 4);
        put_str(buf, hx as u16, (y - 1) as u16, "hi!", hi);
    }
}

/// A small centred box for typing a new student's name.
fn draw_name_prompt(buf: &mut Buffer, area: Rect, input: &str) {
    let w = 40.min(area.width.saturating_sub(2));
    let h = 4u16;
    let x = area.left() + area.width.saturating_sub(w) / 2;
    let y = area.top() + area.height.saturating_sub(h) / 2;
    let panel = Rect { x, y, width: w, height: h };
    Clear.render(panel, buf);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::LightGreen))
        .title(" New student ")
        .style(Style::default().bg(STAGE_BG));
    let inner = block.inner(panel);
    block.render(panel, buf);
    let shown = tail(input, inner.width.saturating_sub(1));
    put_str(buf, inner.left(), inner.top(), format!("{}_", shown), Style::default().fg(Color::White));
    put_str(buf, inner.left(), inner.bottom().saturating_sub(1), "Enter: save    Esc: cancel", Style::default().fg(Color::Gray));
}

fn draw_menu_row(buf: &mut Buffer, x: u16, y: u16, selected: bool, label: &str, hint: &str) {
    let style = if selected {
        Style::default().fg(Color::Black).bg(Color::LightCyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let cursor = if selected { "➤ " } else { "  " };
    put_str(buf, x, y, format!("{}{:<26}", cursor, label), style);
    if selected {
        put_str(buf, x + 30, y, hint, Style::default().fg(Color::Gray));
    }
}

fn grade_name(grade: u8) -> String {
    if grade == 0 {
        "K".to_string()
    } else {
        format!("{}", grade)
    }
}

// ---------------------------------------------------------------------------
// My Progress — encouragement only, never comparison
// ---------------------------------------------------------------------------

fn draw_stats(f: &mut Frame, app: &App, area: Rect) {
    let buf = f.buffer_mut();
    let s = app.roster.current();
    let total = s.grand_total();
    let col = area.left() + area.width.saturating_sub(46) / 2;

    put_str(buf, col, area.top() + 2, format!("{}'s Math Journey", s.name), Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD));

    // Big celebratory total — every kind of problem counts.
    let total_str = total.to_string();
    font::draw_text(buf, center(area, font::text_width(&total_str)), area.top() + 4, &total_str, Style::default().fg(Color::LightGreen));
    put_str(buf, center(area, 20), area.top() + 10, "problems solved!", Style::default().fg(Color::White).add_modifier(Modifier::BOLD));

    // Breakdown with friendly bars across every section (one per Topic).
    let rows: Vec<(&str, u32)> =
        crate::topic::Topic::ALL.iter().map(|&t| (t.short(), s.solved_for(t))).collect();
    let max = rows.iter().map(|(_, n)| *n).max().unwrap_or(0).max(1);
    let mut y = area.top() + 12;
    for (label, n) in rows {
        let bar_w = 24u16;
        let filled = ((n as f32 / max as f32) * bar_w as f32).round() as u16;
        let bar: String = (0..bar_w).map(|i| if i < filled { '█' } else { '░' }).collect();
        put_str(buf, col, y, format!("{:<10} ", label), Style::default().fg(Color::White));
        put_str(buf, col + 11, y, &bar, Style::default().fg(Color::LightGreen));
        put_str(buf, col + 11 + bar_w + 1, y, format!("{}", n), Style::default().fg(Color::LightYellow));
        y += 1;
    }

    if s.best_streak > 0 {
        put_str(buf, col, y, format!("★ Best streak: {} in a row!", s.best_streak), Style::default().fg(Color::LightMagenta).add_modifier(Modifier::BOLD));
        y += 1;
    }

    // An encouraging message that grows with effort — never a comparison.
    let msg = encouragement(total);
    put_str(buf, col, y, msg, Style::default().fg(Color::LightCyan));

    put_str(buf, col, area.bottom().saturating_sub(2), "Every problem makes you stronger.   Esc: back", Style::default().fg(Color::DarkGray));
}

/// A warm message keyed only to the student's own total — no ranking.
fn encouragement(total: u32) -> &'static str {
    match total {
        0 => "Your journey starts with the very first problem. Let's go!",
        1..=9 => "You're off to a great start — keep that curiosity going!",
        10..=49 => "Look at you go! Your brain is getting stronger every day.",
        50..=149 => "Fantastic effort — you're becoming a real problem-solver!",
        150..=499 => "Incredible dedication. You should be proud of yourself!",
        _ => "You're a math powerhouse. Deduction Duck is so proud!",
    }
}

// ---------------------------------------------------------------------------
// Teacher Area — password-gated anecdote editor + records admin
// ---------------------------------------------------------------------------

fn draw_teacher(f: &mut Frame, app: &App, area: Rect) {
    if !app.teacher_authed {
        draw_teacher_login(f, app, area);
    } else {
        draw_teacher_manage(f, app, area);
    }
}

fn draw_teacher_login(f: &mut Frame, app: &App, area: Rect) {
    let buf = f.buffer_mut();
    let col = area.left() + area.width.saturating_sub(44) / 2;

    put_str(buf, col, area.top() + 3, "TEACHER AREA", Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD));

    let setting = !app.config.has_teacher_password();
    let prompt = if setting { "Create a teacher password:" } else { "Enter teacher password:" };
    put_str(buf, col, area.top() + 6, prompt, Style::default().fg(Color::White));

    let masked: String = "*".repeat(app.teacher_pw.chars().count());
    put_str(buf, col, area.top() + 7, format!("{}_", masked), Style::default().fg(Color::LightGreen));

    if setting {
        put_str(buf, col, area.top() + 9, "(Kids can't reach these tools without it.)", Style::default().fg(Color::DarkGray));
    }
    if let Some(msg) = &app.teacher_msg {
        put_str(buf, col, area.top() + 11, msg, Style::default().fg(Color::LightYellow));
    }

    put_str(buf, col, area.bottom().saturating_sub(2), "Enter: continue    Esc: back", Style::default().fg(Color::DarkGray));
}

fn draw_teacher_manage(f: &mut Frame, app: &App, area: Rect) {
    let buf = f.buffer_mut();
    let col = area.left() + area.width.saturating_sub(60) / 2;

    // Tabs.
    let (a_style, r_style) = match app.teacher_view {
        TeacherView::Anecdotes => (Style::default().fg(Color::Black).bg(Color::LightCyan).add_modifier(Modifier::BOLD), Style::default().fg(Color::Gray)),
        TeacherView::Records => (Style::default().fg(Color::Gray), Style::default().fg(Color::Black).bg(Color::LightCyan).add_modifier(Modifier::BOLD)),
    };
    put_str(buf, col, area.top() + 1, " Why? Examples ", a_style);
    put_str(buf, col + 16, area.top() + 1, " Student Records ", r_style);
    put_str(buf, col + 36, area.top() + 1, "Tab: switch", Style::default().fg(Color::DarkGray));

    // Measurement locality lives here now (used by Units of Measure problems).
    put_str(
        buf,
        col,
        area.bottom().saturating_sub(4),
        format!("Units locality: {}    (L: change)", app.config.locality.name()),
        Style::default().fg(Color::LightCyan),
    );

    match app.teacher_view {
        TeacherView::Anecdotes => draw_teacher_anecdotes(buf, app, area, col),
        TeacherView::Records => draw_teacher_records(buf, app, area, col),
    }

    if let Some(msg) = &app.teacher_msg {
        put_str(buf, col, area.bottom().saturating_sub(3), msg, Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD));
    }
}

fn draw_teacher_anecdotes(buf: &mut Buffer, app: &App, area: Rect, col: u16) {
    let topic = crate::topic::Topic::ALL[app.teacher_topic];
    put_str(buf, col, area.top() + 3, format!("Section:  < {} >", topic.name()), Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
    put_str(buf, col, area.top() + 4, "Your own real-life examples shown in the Why? panel:", Style::default().fg(Color::Gray));

    // When adding, a wrapping input box claims the lower part of the screen.
    let input_box = if app.teacher_adding {
        let box_h = 8u16.min(area.height.saturating_sub(9)).max(4);
        let box_w = area.right().saturating_sub(col).saturating_sub(2).min(66).max(20);
        let box_y = area.bottom().saturating_sub(box_h + 2);
        Some(Rect { x: col, y: box_y, width: box_w, height: box_h })
    } else {
        None
    };
    let list_bottom = input_box.map_or(area.bottom().saturating_sub(2), |b| b.y.saturating_sub(1));

    let items = app.why_extras.items(topic);
    let mut y = area.top() + 6;
    if items.is_empty() {
        put_str(buf, col + 2, y, "(none yet — press A to add one)", Style::default().fg(Color::DarkGray));
    } else {
        for it in items {
            if y >= list_bottom {
                break;
            }
            put_str(buf, col + 2, y, clip(&format!("• {}", it), area.width.saturating_sub(4)), Style::default().fg(Color::White));
            y += 1;
        }
    }

    if let Some(rect) = input_box {
        Clear.render(rect, buf);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::LightYellow))
            .title(" New example ")
            .style(Style::default().bg(STAGE_BG));
        let inner = block.inner(rect);
        block.render(rect, buf);

        // Echo the typed text wrapped to the box, with a caret, keeping the end
        // in view if it overflows.
        let rows = inner.height as usize;
        let lines = wrap_chars(&format!("{}_", app.teacher_text), inner.width);
        let start = lines.len().saturating_sub(rows);
        for (i, line) in lines[start..].iter().enumerate() {
            put_str(buf, inner.left(), inner.top() + i as u16, line, Style::default().fg(Color::White));
        }

        // Hint on the left, character count on the right, just below the box.
        let foot_y = rect.bottom();
        put_str(buf, col, foot_y, "Enter: save    Esc: cancel", Style::default().fg(Color::DarkGray));
        let count = format!("{}/{}", app.teacher_text.chars().count(), crate::app::ANECDOTE_MAX);
        put_str(buf, area.right().saturating_sub(count.len() as u16 + 1), foot_y, &count, Style::default().fg(Color::Gray));
    } else {
        put_str(buf, col, area.bottom().saturating_sub(2), "< > section    A: add    Tab: records    Esc: log out", Style::default().fg(Color::DarkGray));
    }
}

/// Short column headers for each section, in [`Topic::ALL`] order.  Kept to a
/// few characters so all sections fit one row.
const REC_COLS: [&str; 8] = ["Add", "Sub", "Mul", "Div", "Un", "Fr", "Pct", "Geo"];
/// Left edge of the name column within the 5-wide section grid.
const REC_NAME_W: u16 = 12;
/// Width of each per-section count column.
const REC_COL_W: u16 = 5;

fn draw_teacher_records(buf: &mut Buffer, app: &App, area: Rect, col: u16) {
    let sel_topic = app.teacher_topic;
    // One compact column per section, then Total and the reveal Lock.
    let mut header = format!("{:<width$}", "Student", width = REC_NAME_W as usize);
    for c in REC_COLS {
        header.push_str(&format!("{:>w$}", c, w = REC_COL_W as usize));
    }
    header.push_str(&format!("{:>7}{:>6}", "Total", "Lock"));
    let header_y = area.top() + 3;
    put_str(buf, col, header_y, &header, Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD));
    // Highlight the selected section's column header — this is what S/R resets.
    let sel_x = col + REC_NAME_W + sel_topic as u16 * REC_COL_W;
    put_str(
        buf,
        sel_x,
        header_y,
        format!("{:>w$}", REC_COLS[sel_topic], w = REC_COL_W as usize),
        Style::default().fg(Color::Black).bg(Color::LightYellow).add_modifier(Modifier::BOLD),
    );

    let list_top = area.top() + 4;
    let (scroll, visible) = scroll_window(app.roster.students.len(), app.teacher_rec_index, list_top, area.bottom().saturating_sub(4));
    let mut y = list_top;
    for (i, s) in app.roster.students.iter().enumerate().skip(scroll).take(visible) {
        let selected = i == app.teacher_rec_index;
        let base = if selected {
            Style::default().fg(Color::Black).bg(Color::LightCyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let lock = if s.reveal_lock == 0 { "off".to_string() } else { s.reveal_lock.to_string() };
        let mut row = format!("{:<width$}", clip(&s.name, REC_NAME_W), width = REC_NAME_W as usize);
        for &t in &crate::topic::Topic::ALL {
            row.push_str(&format!("{:>w$}", s.solved_for(t), w = REC_COL_W as usize));
        }
        row.push_str(&format!("{:>7}{:>6}", s.grand_total(), lock));
        put_str(buf, col, y, &row, base);
        y += 1;
    }
    if scroll > 0 {
        put_str(buf, col, list_top, "↑", Style::default().fg(Color::LightCyan));
    }
    if scroll + visible < app.roster.students.len() {
        put_str(buf, col, y.saturating_sub(1), "↓", Style::default().fg(Color::LightCyan));
    }

    put_str(
        buf,
        col,
        area.bottom().saturating_sub(2),
        "Up/Dn student   <> section   S/R reset   X remove   +/- lock(0=off)   Esc out",
        Style::default().fg(Color::DarkGray),
    );
}

// ---------------------------------------------------------------------------
// Settings — per-grade number ranges
// ---------------------------------------------------------------------------

use crate::app::SETTINGS_FIELDS;

fn draw_settings(f: &mut Frame, app: &App, area: Rect) {
    let buf = f.buffer_mut();
    let col = area.left() + (area.width.saturating_sub(46)) / 2;

    put_str(buf, col, area.top() + 2, "SETTINGS — NUMBER RANGES", Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD));
    put_str(buf, col, area.top() + 3, "Tune how big the numbers get for each grade.", Style::default().fg(Color::Gray));

    let g = app.settings_grade;
    let r = app.config.range(g);
    let sel = app.settings_field;

    let rows: [(String, String); SETTINGS_FIELDS] = [
        ("Grade".to_string(), format!("< {} >", grade_name(g))),
        ("Add / Subtract up to".to_string(), format!("< {} >", r.add_max)),
        ("Multiply factors up to".to_string(), format!("< {} >", r.mul_max)),
        ("Divide numbers up to".to_string(), format!("< {} >", r.div_max)),
        ("Allow negative answers".to_string(), format!("< {} >", if r.allow_negative { "Yes" } else { "No" })),
        ("Show problems".to_string(), format!("< {} >", app.config.layout.name())),
        ("Theme (graphics mode)".to_string(), format!("< {} >", app.config.theme.name())),
    ];

    let mut y = area.top() + 6;
    for (i, (label, value)) in rows.iter().enumerate() {
        let selected = i == sel;
        let style = if selected {
            Style::default().fg(Color::Black).bg(Color::LightCyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let cursor = if selected { "➤ " } else { "  " };
        put_str(buf, col, y, format!("{}{:<26}{}", cursor, label, value), style);
        y += 2;
    }

    put_str(
        buf,
        col,
        area.bottom().saturating_sub(2),
        "Up / Down choose    Left / Right change    Esc save & back",
        Style::default().fg(Color::DarkGray),
    );
}

// ---------------------------------------------------------------------------
// Practice / Challenge session
// ---------------------------------------------------------------------------

/// The area the problem card occupies, given the screen and full frame.
/// Shared with [`crate::app`] so transition capture lines up exactly.
pub fn card_area(screen: Screen, area: Rect) -> Rect {
    if screen == Screen::Challenge {
        Rect { y: area.y + 2, height: area.height.saturating_sub(2), ..area }
    } else {
        area
    }
}

fn draw_session(f: &mut Frame, app: &App, fx: &Transitions, area: Rect) {
    // Reserve a HUD row at the top for challenge mode.
    let card_area = card_area(app.screen, area);
    if app.screen == Screen::Challenge {
        let hud = Rect { height: 2, ..area };
        draw_challenge_hud(f, app, hud);
    }

    // Card: either an in-flight transition (owned by the frontend) or the
    // current problem, dispatched on whichever section produced it.
    if let Some(t) = &fx.problem {
        t.render(card_area, f.buffer_mut());
    } else if app.challenge.as_ref().map_or(false, |c| c.finished) {
        draw_challenge_summary(f, app, card_area);
    } else {
        match &app.current {
            Active::Shape(s) => {
                let banner = match app.feedback {
                    Feedback::Correct => Some(("✓  Correct!".to_string(), Color::LightGreen)),
                    Feedback::Wrong => Some(("✗  Not quite — count again!".to_string(), Color::LightRed)),
                    Feedback::None => None,
                };
                render_shape_card(card_area, f.buffer_mut(), s, &app.input, app.frac_progress(), banner);
            }
            Active::Unit(p) => {
                let banner = match app.feedback {
                    Feedback::Correct => Some(("✓  Correct!".to_string(), Color::LightGreen)),
                    Feedback::Wrong => Some(("✗  Not quite — try again!".to_string(), Color::LightRed)),
                    Feedback::None => None,
                };
                render_unit_card(card_area, f.buffer_mut(), p, &app.input, banner);
                if p.duck_jump {
                    let inner = Block::default().borders(Borders::ALL).inner(card_area);
                    draw_duck_gag(f.buffer_mut(), inner, p.theme, app.anim_frame);
                }
            }
            Active::Geo(g) => {
                let banner = match app.feedback {
                    Feedback::Correct => Some(("✓  Correct!".to_string(), Color::LightGreen)),
                    Feedback::Wrong => Some(("✗  Not quite — measure again!".to_string(), Color::LightRed)),
                    Feedback::None => None,
                };
                render_geometry_card(card_area, f.buffer_mut(), g, &app.input, banner);
            }
            Active::Arith(p) => {
                let banner = feedback_banner(app);
                render_card(card_area, f.buffer_mut(), p, &app.input, app.config.layout, banner);
            }
        }
    }

    // Deduction Duck helps with any problem; the Why panel works for every topic.
    if app.transition.is_none() && app.help_in > 0.0 {
        match &app.current {
            Active::Shape(s) => draw_hint_panel(f, app, card_area, &s.hint, s.answer_label()),
            Active::Unit(u) => draw_hint_panel(f, app, card_area, &u.hint, format!("Answer: {} {}", u.answer, u.unit_label)),
            Active::Geo(g) => draw_hint_panel(f, app, card_area, &g.hint, g.answer_label()),
            Active::Arith(p) => draw_help_overlay(f, app, card_area, p),
        }
    }
    if app.transition.is_none() && app.why_active {
        draw_why_overlay(f, app, card_area);
    }
}

/// Deduction Duck's hint panel: a how-to hint (never the answer) with R to
/// reveal the worked `answer`.  Shared by measurement and fraction problems,
/// and honours the encouragement, reprimand, and peek-cooldown state.
fn draw_hint_panel(f: &mut Frame, app: &App, area: Rect, hint: &[String], answer: String) {
    // Tagged lines: 0 plain, 1 answer, 2 encouragement, 3 header, 4 reprimand.
    let mut lines: Vec<(String, u8)> = Vec::new();
    if let Some(msg) = &app.encourage {
        lines.push((msg.clone(), 2));
        lines.push((String::new(), 0));
    }
    if let Some(r) = &app.reprimand {
        for l in wrap_text(r, 52) {
            lines.push((l, 4));
        }
        lines.push((String::new(), 0));
    }
    lines.push(("Deduction Duck's hint:".to_string(), 3));
    for h in hint {
        lines.push((format!("• {}", h), 0));
    }
    if app.revealed {
        lines.push((String::new(), 0));
        lines.push((answer, 1));
    }

    let content_w = lines.iter().map(|(l, _)| l.chars().count()).max().unwrap_or(20) as u16;
    let panel_w = (content_w + 6 + duck::WIDTH).min(area.width.saturating_sub(2)).max(20);
    let panel_h = ((lines.len() as u16).max(duck::HEIGHT) + 3).min(area.height.saturating_sub(2)).max(6);
    let rest_y = area.top() + area.height.saturating_sub(panel_h) / 2;
    let slide = ((1.0 - app.help_in.clamp(0.0, 1.0)) * (panel_h as f32 + 2.0)) as u16;
    let panel_x = area.left() + area.width.saturating_sub(panel_w) / 2;
    let panel_y = (rest_y + slide).min(area.bottom().saturating_sub(1));
    let panel = Rect {
        x: panel_x,
        y: panel_y,
        width: panel_w.min(area.right().saturating_sub(panel_x)),
        height: panel_h.min(area.bottom().saturating_sub(panel_y)),
    };
    if panel.width < 12 || panel.height < 4 {
        return;
    }

    let buf = f.buffer_mut();
    Clear.render(panel, buf);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::LightYellow))
        .title(" Deduction Duck ")
        .style(Style::default().bg(STAGE_BG));
    let inner = block.inner(panel);
    block.render(panel, buf);

    let duck_x = inner.right().saturating_sub(duck::WIDTH);
    let duck_y = inner.top() + inner.height.saturating_sub(duck::HEIGHT) / 2;
    duck::draw_pose(buf, duck_x, duck_y, Pose::Stand, app.anim_frame / 6, Style::default().fg(DUCK_COLOR));

    let text_w = duck_x.saturating_sub(inner.left() + 1);
    let mut y = inner.top();
    for (line, tag) in &lines {
        if y >= inner.bottom().saturating_sub(1) {
            break;
        }
        let style = match tag {
            1 | 2 => Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD),
            3 => Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD),
            4 => Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD),
            _ => Style::default().fg(Color::White),
        };
        put_str(buf, inner.left(), y, clip(line, text_w), style);
        y += 1;
    }

    let footer = if app.reveal_locked() {
        "Answer's on cooldown — try it yourself!    H: shoo"
    } else if app.revealed {
        "R: hide answer    H: shoo"
    } else {
        "R: show answer    H: shoo"
    };
    put_str(buf, inner.left(), inner.bottom().saturating_sub(1), footer, Style::default().fg(Color::Gray));
}

/// The Y overlay: a panel listing real-world uses of the current topic.
fn draw_why_overlay(f: &mut Frame, app: &App, area: Rect) {
    let heading = crate::motivation::heading(app.current_topic());

    // Each tagged line carries a style: 0 bullet, 1 code header, 2 code, 3 why.
    let panel_w = area.width.saturating_sub(4).min(72).max(24).min(area.width);
    let wrap_w = panel_w.saturating_sub(4);
    let mut lines: Vec<(String, u8)> = Vec::new();
    for item in &app.why_items {
        for line in wrap_prefixed("• ", "  ", item, wrap_w) {
            lines.push((line, 0));
        }
    }
    // A code peek for budding programmers.
    if let Some((code, why)) = &app.why_code {
        lines.push((String::new(), 0));
        for line in wrap_text("For future coders — a line from this app:", wrap_w) {
            lines.push((line, 1));
        }
        lines.push((format!("  {}", code), 2));
        // Prefix-aware wrap: "Why it matters: …" stays inside the panel.
        for line in wrap_prefixed("  Why it matters: ", "    ", why, wrap_w) {
            lines.push((line, 3));
        }
    }

    // Grow to fit the content (heading + gap + lines + footer + borders), but
    // never taller than the available area — the leftover scrolls.
    let wanted_h = lines.len() as u16 + 5;
    let panel_h = wanted_h.min(area.height.saturating_sub(2)).max(7);
    let panel_x = area.left() + area.width.saturating_sub(panel_w) / 2;
    let panel_y = area.top() + area.height.saturating_sub(panel_h) / 2;
    let panel = Rect {
        x: panel_x,
        y: panel_y,
        width: panel_w.min(area.right().saturating_sub(panel_x)),
        height: panel_h.min(area.bottom().saturating_sub(panel_y)),
    };
    if panel.width < 12 || panel.height < 5 {
        return;
    }

    let buf = f.buffer_mut();
    Clear.render(panel, buf);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::LightCyan))
        .title(" Deduction Duck ")
        .style(Style::default().bg(STAGE_BG));
    let inner = block.inner(panel);
    block.render(panel, buf);

    put_str(buf, inner.left(), inner.top(), clip(&heading, inner.width), Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD));

    // Visible content window: rows between the gap under the heading and the
    // footer.  Anything beyond scrolls, controlled by Up/Down.
    let first_row = inner.top() + 2;
    let footer_row = inner.bottom().saturating_sub(1);
    let visible = footer_row.saturating_sub(first_row) as usize;
    let total = lines.len();
    let max_scroll = total.saturating_sub(visible);
    app.why_max_scroll.set(max_scroll);
    let scroll = app.why_scroll.min(max_scroll);

    let mut y = first_row;
    for (line, tag) in lines.iter().skip(scroll).take(visible) {
        let style = match tag {
            1 => Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD),
            2 => Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD),
            3 => Style::default().fg(Color::Gray),
            _ => Style::default().fg(Color::White),
        };
        put_str(buf, inner.left(), y, clip(line, inner.width), style);
        y += 1;
    }

    // Scroll affordances.
    let scrollable = max_scroll > 0;
    if scroll > 0 {
        put_str(buf, inner.right().saturating_sub(7), inner.top() + 1, "↑ more", Style::default().fg(Color::LightCyan));
    }
    if scroll < max_scroll {
        put_str(buf, inner.right().saturating_sub(7), footer_row, "↓ more", Style::default().fg(Color::LightCyan));
    }

    let footer = if scrollable { "Y/Esc: close    ↑↓ scroll" } else { "Y or Esc: close" };
    put_str(buf, inner.left(), footer_row, footer, Style::default().fg(Color::Gray));
}

/// Greedy word-wrap of `s` into lines no wider than `width` characters.
fn wrap_text(s: &str, width: u16) -> Vec<String> {
    wrap_prefixed("", "", s, width)
}

/// Hard character wrap: break `s` every `width` columns, preserving spaces.
/// Used for text-input echoes where the caret must always stay in view.
fn wrap_chars(s: &str, width: u16) -> Vec<String> {
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
fn wrap_prefixed(first: &str, cont: &str, text: &str, width: u16) -> Vec<String> {
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

fn feedback_banner(app: &App) -> Option<(String, Color)> {
    match app.feedback {
        Feedback::None => None,
        Feedback::Correct => Some(("✓  Correct!  Quack-tastic!".to_string(), Color::LightGreen)),
        Feedback::Wrong => Some(("✗  Not yet — try again, or press H for help.".to_string(), Color::LightRed)),
    }
}

/// Draw a single problem card into `buf`.  Used live and for transition capture.
pub fn render_card(area: Rect, buf: &mut Buffer, p: &Problem, input: &str, layout: Layout, banner: Option<(String, Color)>) {
    // Bordered panel tinted with the problem's accent colour.
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(rat(p.accent)))
        .style(Style::default().bg(BG));
    let inner = block.inner(area);
    block.render(area, buf);

    // Division gets the long-division "house" when vertical; the other three
    // operations stack in columns; horizontal keeps everything on one line.
    match layout {
        Layout::Vertical if p.op == Op::Div => draw_division_house(inner, buf, p, input),
        Layout::Vertical => draw_vertical(inner, buf, p, input),
        _ => draw_horizontal(inner, buf, p, input),
    }

    // Feedback banner and footer hints share a fixed spot so the two layouts
    // never collide with them.
    if let Some((text, color)) = banner {
        put_str(
            buf,
            center(inner, text.chars().count() as u16),
            inner.bottom().saturating_sub(3),
            text,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        );
    }
    let hints = "Enter check   H help duck   Y why?   V view   Esc menu";
    put_str(
        buf,
        center(inner, hints.chars().count() as u16),
        inner.bottom().saturating_sub(1),
        hints,
        Style::default().fg(Color::DarkGray),
    );
}

/// Style for the answer text: bold white once typed, dim while it's still `?`.
fn answer_style(input: &str) -> Style {
    if input.is_empty() {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
    }
}

/// `a op b = answer` on one line, the answer sitting to the right of the `=`.
fn draw_horizontal(inner: Rect, buf: &mut Buffer, p: &Problem, input: &str) {
    let prompt = p.prompt(); // "a op b ="
    let answer = if input.is_empty() { "?" } else { input };
    let gap = font::GLYPH_W + font::GLYPH_GAP;
    let pw = font::text_width(&prompt);
    let aw = font::text_width(answer);
    let total = pw + gap + aw;

    let x = center(inner, total);
    let y = inner.top() + inner.height.saturating_sub(font::GLYPH_H) / 2;
    font::draw_text(buf, x, y, &prompt, Style::default().fg(rat(p.accent)));
    font::draw_text(buf, x + pw + gap, y, answer, answer_style(input));
}

/// The stacked column form, with digits right-aligned and the answer below the
/// line — the way a student writes it out by hand.
fn draw_vertical(inner: Rect, buf: &mut Buffer, p: &Problem, input: &str) {
    let a_str = p.a.to_string();
    let b_str = p.b.to_string();
    let ans = if input.is_empty() { "?".to_string() } else { input.to_string() };

    let field_digits = a_str.chars().count().max(b_str.chars().count()).max(ans.chars().count()) as u16;
    let field_px = field_digits * font::GLYPH_W + field_digits.saturating_sub(1) * font::GLYPH_GAP;
    let op_col = font::GLYPH_W + font::GLYPH_GAP; // operator sits to the left
    let total_w = op_col + field_px;

    let x0 = center(inner, total_w);
    let field_right = x0 + total_w;

    // Three glyph rows + a one-row divider, with single-row gaps.
    let total_h = 3 * font::GLYPH_H + 3;
    let top = inner.top() + inner.height.saturating_sub(total_h) / 2;

    let accent = Style::default().fg(rat(p.accent));
    let right = |buf: &mut Buffer, s: &str, y: u16, style: Style| {
        let x = field_right.saturating_sub(font::text_width(s));
        font::draw_text(buf, x, y, s, style);
    };

    // Top operand.
    let y1 = top;
    right(buf, &a_str, y1, accent);

    // Operator + second operand.
    let y2 = y1 + font::GLYPH_H + 1;
    font::draw_text(buf, x0, y2, &p.op.symbol().to_string(), accent);
    right(buf, &b_str, y2, accent);

    // Divider line spanning the operand field.
    let y_div = y2 + font::GLYPH_H;
    if y_div < inner.bottom() {
        for x in x0..field_right.min(inner.right()) {
            buf[(x, y_div)].set_symbol("─").set_fg(rat(p.accent));
        }
    }

    // Answer below the line.
    let y3 = y_div + 1;
    right(buf, &ans, y3, answer_style(input));
}

/// Long-division "house": quotient on the roof, divisor outside the wall,
/// dividend inside.  `p.a` is the dividend, `p.b` the divisor.
fn draw_division_house(inner: Rect, buf: &mut Buffer, p: &Problem, input: &str) {
    let dividend = p.a.to_string();
    let divisor = p.b.to_string();
    let quotient = if input.is_empty() { "?".to_string() } else { input.to_string() };

    let wd = font::text_width(&dividend);
    let wq = font::text_width(&quotient);
    let wv = font::text_width(&divisor);

    // Horizontal anchors: "divisor | dividend", quotient right-aligned over it.
    let total_w = wv + 1 + 1 + 2 + wd;
    let x0 = center(inner, total_w);
    let wall_x = x0 + wv + 1;
    let dividend_x = wall_x + 2;
    let house_right = (dividend_x + wd).min(inner.right());
    let quotient_x = dividend_x + wd.saturating_sub(wq);

    // Vertical anchors: quotient row, roof line, dividend row.
    let total_h = 2 * font::GLYPH_H + 1;
    let top = inner.top() + inner.height.saturating_sub(total_h) / 2;
    let q_y = top;
    let roof_y = top + font::GLYPH_H;
    let dvd_y = roof_y + 1;

    let accent = Style::default().fg(rat(p.accent));

    // Quotient (the answer) sits above the roof, over the dividend.
    font::draw_text(buf, quotient_x, q_y, &quotient, answer_style(input));

    // Roof: corner at the wall, bar across the dividend.
    if roof_y < inner.bottom() {
        buf[(wall_x.min(inner.right().saturating_sub(1)), roof_y)].set_symbol("┌").set_fg(rat(p.accent));
        for x in (wall_x + 1)..house_right {
            buf[(x, roof_y)].set_symbol("─").set_fg(rat(p.accent));
        }
    }
    // Wall down the left of the dividend.
    for y in (roof_y + 1)..(dvd_y + font::GLYPH_H).min(inner.bottom()) {
        buf[(wall_x.min(inner.right().saturating_sub(1)), y)].set_symbol("│").set_fg(rat(p.accent));
    }

    // Divisor outside the wall, dividend inside.
    font::draw_text(buf, x0, dvd_y, &divisor, accent);
    font::draw_text(buf, dividend_x, dvd_y, &dividend, accent);
}

// ---------------------------------------------------------------------------
// Challenge HUD + summary
// ---------------------------------------------------------------------------

fn draw_challenge_hud(f: &mut Frame, app: &App, area: Rect) {
    let buf = f.buffer_mut();
    let Some(c) = &app.challenge else { return };
    let remaining = c.remaining_secs();

    let label = format!(" ⏱  {:>2}s    ★ Solved: {} ", remaining, c.solved);
    put_str(buf, area.left() + 1, area.top(), label, Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD));

    // A simple time bar across the row beneath the label.
    let total = c.duration_secs().max(1);
    let frac = remaining as f32 / total as f32;
    let bar_w = area.width.saturating_sub(2);
    let filled = (frac * bar_w as f32).round() as u16;
    let bar_color = if frac > 0.5 {
        Color::LightGreen
    } else if frac > 0.25 {
        Color::LightYellow
    } else {
        Color::LightRed
    };
    for i in 0..bar_w {
        let ch = if i < filled { "█" } else { "░" };
        let style = Style::default().fg(if i < filled { bar_color } else { Color::DarkGray });
        buf[(area.left() + 1 + i, area.top() + 1)].set_symbol(ch).set_style(style);
    }
}

fn draw_challenge_summary(f: &mut Frame, app: &App, area: Rect) {
    let buf = f.buffer_mut();
    let solved = app.challenge.as_ref().map_or(0, |c| c.solved);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::LightGreen))
        .style(Style::default().bg(BG));
    let inner = block.inner(area);
    block.render(area, buf);

    let big = format!("{}", solved);
    let bw = font::text_width(&big);
    font::draw_text(buf, center(inner, bw), inner.top() + inner.height / 2 - 3, &big, Style::default().fg(Color::LightGreen));

    let msg = "Time! Problems solved:";
    put_str(buf, center(inner, msg.chars().count() as u16), inner.top() + inner.height / 2 - 5, msg, Style::default().fg(Color::White).add_modifier(Modifier::BOLD));

    let hint = "Enter = play again    Esc = menu";
    put_str(buf, center(inner, hint.chars().count() as u16), inner.bottom().saturating_sub(2), hint, Style::default().fg(Color::DarkGray));
}

// ---------------------------------------------------------------------------
// Deduction Duck help stage
// ---------------------------------------------------------------------------

/// The duck's stage: a panel sliding up from the bottom in which he acts out
/// the chosen strategy — walking a number line, smashing a number, or talking
/// it through.  Hint steps show first; the answer appears only once revealed.
fn draw_help_overlay(f: &mut Frame, app: &App, area: Rect, problem: &Problem) {
    let strats = strategy::strategies(problem);
    let idx = app.strategy_index % strats.len();
    let s = &strats[idx];

    // Panel geometry; slides up from below as `help_in` eases 0..1.
    let panel_w = area.width.saturating_sub(4).min(72).max(20).min(area.width);
    let panel_h = area.height.saturating_sub(2).min(18).max(9).min(area.height);
    let rest_y = area.top() + area.height.saturating_sub(panel_h) / 2;
    let slide = ((1.0 - app.help_in.clamp(0.0, 1.0)) * (panel_h as f32 + 2.0)) as u16;
    let panel_x = area.left() + area.width.saturating_sub(panel_w) / 2;
    let panel_y = (rest_y + slide).min(area.bottom().saturating_sub(1));
    let panel = Rect {
        x: panel_x,
        y: panel_y,
        width: panel_w.min(area.right().saturating_sub(panel_x)),
        height: panel_h.min(area.bottom().saturating_sub(panel_y)),
    };
    if panel.width < 10 || panel.height < 5 {
        return;
    }

    let buf = f.buffer_mut();
    Clear.render(panel, buf);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::LightYellow))
        .title(" Deduction Duck ")
        .style(Style::default().bg(STAGE_BG));
    let inner = block.inner(panel);
    block.render(panel, buf);

    // The top line carries a peek reprimand (red) or, failing that, an
    // encouraging word (green) when the student is struggling.
    let enc_off: u16 = if let Some(msg) = &app.reprimand {
        put_str(buf, inner.left(), inner.top(), clip(msg, inner.width), Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD));
        1
    } else if let Some(msg) = &app.encourage {
        put_str(buf, inner.left(), inner.top(), clip(msg, inner.width), Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD));
        1
    } else {
        0
    };

    // Header and footer controls.
    let header = format!("Way {}/{}: {}", idx + 1, strats.len(), s.title);
    put_str(buf, inner.left(), inner.top() + enc_off, clip(&header, inner.width), Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD));

    let reveal_ctl = if app.reveal_locked() {
        "R: locked!"
    } else if app.revealed {
        "R: hide answer"
    } else {
        "R: show answer"
    };
    let mut controls = vec![reveal_ctl.to_string()];
    if strats.len() > 1 {
        controls.push("Space: another way".to_string());
    }
    controls.push("H: shoo".to_string());
    let footer = controls.join("    ");
    put_str(buf, inner.left(), inner.bottom().saturating_sub(1), clip(&footer, inner.width), Style::default().fg(Color::Gray));

    // Content sits between header and footer.
    let content = Rect {
        x: inner.left(),
        y: inner.top() + 2 + enc_off,
        width: inner.width,
        height: inner.height.saturating_sub(3 + enc_off),
    };
    if content.height == 0 {
        return;
    }

    let accent = rat(problem.accent);
    let drawn = match &s.viz {
        Viz::NumberLine { stops, hops } => draw_number_line(buf, content, s, stops, hops, app, accent),
        Viz::Smash { value, parts } => draw_smash(buf, content, s, *value, parts, app, accent),
        Viz::Lines => false,
    };
    if !drawn {
        draw_lines(buf, content, s, app);
    }
}

/// Fallback / plain view: hint steps on the left, duck standing on the right.
fn draw_lines(buf: &mut Buffer, content: Rect, s: &Strategy, app: &App) {
    let reveal_start = s.steps.len() + 1;
    let mut lines: Vec<String> = s.steps.clone();
    if app.revealed {
        lines.push(String::new());
        lines.extend(s.reveal.iter().cloned());
    }

    let duck_x = content.right().saturating_sub(duck::WIDTH + 1);
    let text_w = duck_x.saturating_sub(content.left() + 1);

    let mut y = content.top();
    for (i, l) in lines.iter().enumerate() {
        if y >= content.bottom() {
            break;
        }
        let style = if i >= reveal_start {
            Style::default().fg(Color::LightGreen)
        } else {
            Style::default().fg(Color::White)
        };
        put_str(buf, content.left(), y, clip(l, text_w), style);
        y += 1;
    }

    let duck_y = content.top() + content.height.saturating_sub(duck::HEIGHT) / 2;
    duck::draw_pose(buf, duck_x, duck_y, Pose::Stand, app.anim_frame / 6, Style::default().fg(DUCK_COLOR));
}

/// The duck walks a number line, hopping stop to stop.  Returns false (so the
/// caller falls back to [`draw_lines`]) when it will not fit.
fn draw_number_line(buf: &mut Buffer, content: Rect, s: &Strategy, stops: &[i64], hops: &[String], app: &App, accent: Color) -> bool {
    let n = stops.len();
    if n < 2 || content.height < 12 {
        return false;
    }
    let labels: Vec<String> = stops.iter().map(|v| v.to_string()).collect();
    let maxlabel = labels.iter().map(|l| l.len()).max().unwrap_or(1) as u16;
    let margin = maxlabel / 2 + 1;
    let ax0 = content.left() + margin;
    let ax1 = content.right().saturating_sub(margin + 1);
    if ax1 <= ax0 {
        return false;
    }
    let seg = (ax1 - ax0) as f32 / (n - 1) as f32;
    if seg < (maxlabel + 2) as f32 {
        return false;
    }
    let x_at = |i: usize| -> u16 { ax0 + (i as f32 * seg).round() as u16 };

    let axis_y = content.bottom().saturating_sub(2);
    let label_y = axis_y + 1;
    let hop_y = axis_y.saturating_sub(1);

    // Subtitle (hint) and, once revealed, the worked hops.
    put_str(buf, content.left(), content.top(), clip(&s.steps[0], content.width), Style::default().fg(Color::White));
    if app.revealed {
        let mut ry = content.top() + 1;
        for l in &s.reveal {
            put_str(buf, content.left(), ry, clip(l, content.width), Style::default().fg(Color::LightGreen));
            ry += 1;
        }
    }

    // Axis, ticks, labels.
    for x in ax0..=ax1.min(content.right().saturating_sub(1)) {
        buf[(x, axis_y)].set_symbol("─").set_fg(Color::DarkGray);
    }
    for i in 0..n {
        let x = x_at(i).min(content.right().saturating_sub(1));
        buf[(x, axis_y)].set_symbol("┼").set_fg(accent);
        let lbl = &labels[i];
        let lx = x.saturating_sub(lbl.len() as u16 / 2);
        let last = i == n - 1;
        let lstyle = if app.revealed && last {
            Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        put_str(buf, lx, label_y, lbl, lstyle);
    }
    // Hop labels above each segment.
    for i in 0..n - 1 {
        let xm = (x_at(i) + x_at(i + 1)) / 2;
        let h = &hops[i];
        put_str(buf, xm.saturating_sub(h.len() as u16 / 2), hop_y, h, Style::default().fg(Color::LightGreen));
    }

    // Walking duck: travel each segment, dwelling on the final stop.  A gentle
    // pace (one hop roughly every two-thirds of a second) keeps it calm rather
    // than distracting, plus a short pause on the final stop before looping.
    const TICKS_PER_HOP: u64 = 20;
    let pos = (app.anim_frame / TICKS_PER_HOP) as f32 % (n as f32 + 1.0);
    let segi = pos.floor() as usize;
    let (dx, lift) = if segi >= n - 1 {
        (x_at(n - 1) as f32, 0.0)
    } else {
        let frac = pos - segi as f32;
        let xa = x_at(segi) as f32;
        let xb = x_at(segi + 1) as f32;
        (xa + frac * (xb - xa), (frac * std::f32::consts::PI).sin() * 2.0)
    };
    let duck_left = (dx.round() as i32 - duck::WIDTH as i32 / 2).max(content.left() as i32) as u16;
    let feet_y = hop_y.saturating_sub(1);
    let duck_top = ((feet_y as f32) - (duck::HEIGHT as f32 - 1.0) - lift).max(content.top() as f32 + 2.0) as u16;
    // Waddle only while actually moving (and slowly).
    let waddle = if segi >= n - 1 { 0 } else { app.anim_frame / 7 };
    duck::draw_pose(buf, duck_left, duck_top, Pose::WalkRight, waddle, Style::default().fg(DUCK_COLOR));
    true
}

/// The duck smashes a number apart into its place-value pieces.  Returns false
/// (falling back to [`draw_lines`]) when it will not fit.
fn draw_smash(buf: &mut Buffer, content: Rect, s: &Strategy, value: i64, parts: &[i64], app: &App, accent: Color) -> bool {
    let whole = value.to_string();
    let parts_str = parts.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(" + ");
    let ww = font::text_width(&whole);
    let pw = font::text_width(&parts_str);
    if ww.max(pw) + 2 > content.width || content.height < font::GLYPH_H + 4 {
        return false;
    }

    // Subtitle hint up top.
    put_str(buf, content.left(), content.top(), clip(&s.steps[0], content.width), Style::default().fg(Color::White));

    // Cycle: show the whole number (wind-up + chop), then the pieces.
    const PERIOD: u64 = 18;
    let show_parts = parts.len() > 1 && (app.anim_frame / PERIOD) % 2 == 1;
    let burst = parts.len() > 1 && (app.anim_frame % PERIOD) < 3;

    let num_y = content.top() + 1 + content.height.saturating_sub(font::GLYPH_H + 1) / 2;
    if show_parts {
        font::draw_text(buf, center(content, pw), num_y, &parts_str, Style::default().fg(accent));
    } else {
        font::draw_text(buf, center(content, ww), num_y, &whole, Style::default().fg(accent).add_modifier(Modifier::BOLD));
    }

    // Impact sparks at the moment of the smash.
    if burst {
        let cx = content.left() + content.width / 2;
        let cy = num_y + font::GLYPH_H / 2;
        for (dx, dy) in [(-3i16, -1i16), (3, -1), (-2, 1), (2, 1), (0, -2), (0, 2)] {
            let x = cx as i16 + dx;
            let y = cy as i16 + dy;
            if x >= 0 && y >= 0 {
                put_str(buf, x as u16, y as u16, "*", Style::default().fg(Color::LightYellow));
            }
        }
        put_str(buf, content.left(), num_y.saturating_sub(1), "POW!", Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD));
    }

    // Revealed working sits along the bottom of the content area.
    if app.revealed {
        let start = content.bottom().saturating_sub(s.reveal.len() as u16);
        for (i, l) in s.reveal.iter().enumerate() {
            put_str(buf, content.left(), start + i as u16, clip(l, content.width), Style::default().fg(Color::LightGreen));
        }
    }

    // The duck, chopping (whole) or recoiling (parts), on the left.
    let pose = if show_parts { Pose::Stand } else { Pose::Smash };
    let duck_y = (num_y + font::GLYPH_H).saturating_sub(duck::HEIGHT).max(content.top() + 1);
    duck::draw_pose(buf, content.left(), duck_y, pose, app.anim_frame / 3, Style::default().fg(DUCK_COLOR));
    true
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Draw a string only when its start cell is inside the buffer.  Ratatui's
/// `set_string` clips horizontally but panics on an out-of-range `y`, so this
/// guard keeps every label safe on tiny terminals.
fn put_str(b: &mut Buffer, x: u16, y: u16, s: impl AsRef<str>, style: Style) {
    let area = b.area;
    if x >= area.right() || y >= area.bottom() || x < area.left() || y < area.top() {
        return;
    }
    b.set_string(x, y, s.as_ref(), style);
}

/// Left x so that a span of `width` is centred within `area`.
fn center(area: Rect, width: u16) -> u16 {
    area.left() + area.width.saturating_sub(width) / 2
}

/// Compute a scroll window for a list of `total` one-line items shown between
/// rows `top` and `bottom` (exclusive), keeping `sel` visible.  Returns
/// `(first_visible_index, visible_count)`.
fn scroll_window(total: usize, sel: usize, top: u16, bottom: u16) -> (usize, usize) {
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
fn clip(s: &str, width: u16) -> String {
    s.chars().take(width as usize).collect()
}

/// The last `width` characters of `s` — keeps a text field's caret in view.
fn tail(s: &str, width: u16) -> String {
    let count = s.chars().count();
    let width = width as usize;
    if count <= width {
        s.to_string()
    } else {
        s.chars().skip(count - width).collect()
    }
}

#[cfg(test)]
#[path = "ui/tests.rs"]
mod tests;
