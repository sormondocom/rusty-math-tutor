//! Milestone celebration cinematics: a handful of starry "scenes" the core
//! steps through, with Deduction Duck gazing up in awe.  Two showpieces — the
//! student's name written among the stars with a rising moon, or rockets that
//! ignite each letter as a star — plus plain text scenes, and scene-to-scene
//! transitions that reuse the same blend engine as the problem transitions.

use tiny_skia::Pixmap;

use crate::app::App;

use super::transition::{blend_transition, card_frame};
use super::*;

pub(super) fn draw_cinematic(pm: &mut Pixmap, app: &App, wf: f32, hf: f32) {
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
