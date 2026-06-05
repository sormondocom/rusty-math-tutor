//! Milestone celebration cinematics for the terminal: still text scenes plus the
//! two animated showpieces — the student's name written among the stars with a
//! rising moon, or rockets that ignite each letter — with Deduction Duck gazing
//! up in awe.  `render_scene` is reused by the scene-to-scene transition capture.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Widget};
use ratatui::Frame;

use crate::app::App;
use crate::duck::{self, Pose};
use crate::font;

use super::*;

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
            put_str(buf, tx, cur_y, "▲", Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
        } else {
            // Letter formed: a bright star that flares white as it ignites.
            let fresh = frame < lit_at(i as u64) + 6;
            let style = Style::default().fg(if fresh { Color::White } else { accent }).add_modifier(Modifier::BOLD);
            put_str(buf, tx, ny, ch.to_string(), style);
            if fresh && ny > area.top() {
                put_str(buf, tx, ny - 1, "✦", Style::default().fg(Color::LightYellow));
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

pub fn draw_cinematic(f: &mut Frame, app: &App, fx: &Transitions, area: Rect) {
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
