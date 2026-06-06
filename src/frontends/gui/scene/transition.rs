//! Problem-to-problem transitions: capture the answered card and the next card,
//! then animate between them.  Two families:
//!
//! * **Default themes** — the seven cell-style reveals (wipe, dissolve, …) plus
//!   the six showy particle effects (explode/swirl shatter the card into flying
//!   tiles; fireworks/starburst/ships/asteroids overlay a crossfade), driven by
//!   the core's [`crate::transition::Effect`].
//! * **Chalk themes** — the effect is ignored: a felt eraser wipes the old card
//!   away in back-and-forth strokes, then the new card is "written" back on.
//!
//! Everything here works at *device* (supersampled) resolution and is
//! deterministic, so the particles need no stored state.

use tiny_skia::{Pixmap, PremultipliedColorU8};

use crate::app::{App, Feedback};

use super::*;

/// Render the answered card melting into the next one, at the core's `progress`.
/// The two frames are captured fresh each tick (both states are static for the
/// transition's duration), then blended per the chosen effect.
pub fn draw_transition(pm: &mut Pixmap, app: &App, wf: f32, hf: f32) {
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
pub fn card_frame(w: u32, h: u32, draw: impl FnOnce(&mut Pixmap)) -> Pixmap {
    let mut p = Pixmap::new(w, h).unwrap_or_else(|| Pixmap::new(1, 1).unwrap());
    p.fill(col(BG));
    draw(&mut p);
    p
}

/// Blend `from`→`to` (device-resolution pixmaps) into `pm` per `effect`/`t`.
pub fn blend_transition(pm: &mut Pixmap, from: &Pixmap, to: &Pixmap, effect: crate::transition::Effect, t: f32) {
    use crate::transition::{Effect, Kind};

    // Chalk themes ignore the random effect: the board is erased, then re-written.
    if is_chalk() {
        eraser_transition(pm, from, to, t);
        return;
    }
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
                (dx * dx + dy * dy).sqrt() / std::f32::consts::FRAC_1_SQRT_2
            }
            Kind::Slide => nx, // handled specially below; unused here
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

// -- chalk eraser -----------------------------------------------------------

/// The blackboard transition: a felt eraser wipes the old card away in
/// back-and-forth strokes (first half), then the new card is "written" on band
/// by band behind a chalk nib (second half).
fn eraser_transition(pm: &mut Pixmap, from: &Pixmap, to: &Pixmap, t: f32) {
    if t < 0.5 {
        chalk_erase(pm, from.pixels(), (t * 2.0).min(1.0));
    } else {
        write_on(pm, to.pixels(), ((t - 0.5) * 2.0).min(1.0));
    }
}

const ERASE_BANDS: u32 = 4;

/// Wipe `src` away to the board in back-and-forth strokes, a felt eraser at the
/// frontier.  `progress` 0..1.
fn chalk_erase(pm: &mut Pixmap, src: &[PremultipliedColorU8], progress: f32) {
    let (w, h) = (pm.width(), pm.height());
    let board = prem(themed(BG));
    let bh = h as f32 / ERASE_BANDS as f32;
    let total = (progress * ERASE_BANDS as f32).min(ERASE_BANDS as f32);
    let cur = (total.floor() as u32).min(ERASE_BANDS - 1);
    let within = total - cur as f32;
    {
        let out = pm.pixels_mut();
        for y in 0..h {
            let b = ((y as f32) / bh) as u32;
            let rtl = b % 2 == 1; // boustrophedon
            for x in 0..w {
                let i = (y * w + x) as usize;
                let nx = x as f32 / w as f32;
                let passed = if rtl { nx >= 1.0 - within } else { nx <= within };
                out[i] = if b < cur || (b == cur && passed) { board } else { src[i] };
            }
        }
    }
    let band_top = (cur as f32 * bh) as u32;
    let rtl = cur % 2 == 1;
    let fx = if rtl { (1.0 - within) * w as f32 } else { within * w as f32 } as i32;
    draw_eraser_block(pm, fx, band_top, bh as u32, rtl);
}

/// "Write" `src` on, band by band left-to-right, a chalk nib at the frontier.
pub fn write_on(pm: &mut Pixmap, src: &[PremultipliedColorU8], progress: f32) {
    let (w, h) = (pm.width(), pm.height());
    let board = prem(themed(BG));
    let bh = h as f32 / ERASE_BANDS as f32;
    let total = (progress * ERASE_BANDS as f32).min(ERASE_BANDS as f32);
    let cur = (total.floor() as u32).min(ERASE_BANDS - 1);
    let within = total - cur as f32;
    {
        let out = pm.pixels_mut();
        for y in 0..h {
            let b = ((y as f32) / bh) as u32;
            for x in 0..w {
                let i = (y * w + x) as usize;
                let nx = x as f32 / w as f32;
                out[i] = if b < cur || (b == cur && nx <= within) { src[i] } else { board };
            }
        }
    }
    let band_top = (cur as f32 * bh) as u32;
    draw_chalk_nib(pm, (within * w as f32) as i32, band_top, bh as u32);
}

/// A chunky felt eraser (wooden back) trailing the wipe frontier.
fn draw_eraser_block(pm: &mut Pixmap, fx: i32, band_top: u32, band_h: u32, rtl: bool) {
    let (w, h) = (pm.width(), pm.height());
    let ew = (44.0 * SSF) as i32;
    let (x0, x1) = if rtl { (fx, fx + ew) } else { (fx - ew, fx) };
    let pad = (6.0 * SSF) as i32;
    let wood = (12.0 * SSF) as i32;
    let y0 = band_top as i32 + pad;
    let y1 = (band_top + band_h) as i32 - pad;
    let data = pm.pixels_mut();
    for y in y0.max(0)..y1.min(h as i32) {
        let felt = if y - y0 < wood { [150, 110, 70] } else { [120, 120, 128] };
        for x in x0.max(0)..x1.min(w as i32) {
            data[(y as u32 * w + x as u32) as usize] = prem(felt);
        }
    }
}

/// A small chalk nib at the writing frontier.
fn draw_chalk_nib(pm: &mut Pixmap, fx: i32, band_top: u32, band_h: u32) {
    let (w, h) = (pm.width(), pm.height());
    let r = (5.0 * SSF) as i32;
    let cy = band_top as i32 + band_h as i32 / 2;
    let data = pm.pixels_mut();
    for y in (cy - r).max(0)..(cy + r).min(h as i32) {
        for x in (fx - r).max(0)..(fx + r).min(w as i32) {
            data[(y as u32 * w + x as u32) as usize] = prem(CHALK_WHITE);
        }
    }
}

// -- particle overlays ------------------------------------------------------

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
    let color = themed(color);
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
    // The captured frame is already themed, so compare against the themed bg.
    let b = themed(BG);
    let d = |a: u8, b: u8| (a as i32 - b as i32).abs();
    d(c[0], b[0]) + d(c[1], b[1]) + d(c[2], b[2]) < 22
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

// ---------------------------------------------------------------------------
// Output-resolution blend (9× fewer pixels than SS)
// ---------------------------------------------------------------------------
// Works on pre-downsampled `u32` buffers (0x00RRGGBB, no premultiplication).
// Particle sizes and speeds use logical-pixel values — the SSF multiplier is
// absent because we're already at output (1×) resolution.

/// Blend `from`→`to` output-resolution buffers into `out` using `effect`/`t`.
/// Each element is `0x00RRGGBB`.  This path is 9× cheaper than blending at
/// SS resolution and is used by the GUI for all problem-to-problem transitions.
pub fn blend_output(
    out:    &mut [u32],
    from:   &[u32],
    to:     &[u32],
    effect: crate::transition::Effect,
    t:      f32,
    w:      u32,
    h:      u32,
) {
    use crate::transition::{Effect, Kind};
    let (wf, hf) = (w as f32, h as f32);

    let thresh = |kind: Kind, x: f32, y: f32| -> f32 {
        let (nx, ny) = (x / wf, y / hf);
        match kind {
            Kind::Wipe     => nx,
            Kind::Curtain  => 1.0 - (nx - 0.5).abs() * 2.0,
            Kind::Dissolve => hash01(x as u32, y as u32),
            Kind::Blinds   => (y / (hf / 8.0)).fract(),
            Kind::Circle   => { let (dx, dy) = (nx - 0.5, ny - 0.5); (dx*dx + dy*dy).sqrt() / std::f32::consts::FRAC_1_SQRT_2 }
            Kind::Slide    => nx,
            Kind::Diagonal => (nx + ny) / 2.0,
        }
    };

    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            out[i] = match effect {
                Effect::Reveal(Kind::Slide) => {
                    let b = (wf * (1.0 - t)) as u32;
                    if x >= b { to[(y * w + x.saturating_sub(b).min(w - 1)) as usize] }
                    else       { from[i] }
                }
                Effect::Reveal(kind) => {
                    if t >= thresh(kind, x as f32, y as f32) { to[i] } else { from[i] }
                }
                _ => lerp_u32(from[i], to[i], t),
            };
        }
    }

    let f = t * 40.0;
    match effect {
        Effect::Explode    => out_glyphs(out, from, t, w, h, true),
        Effect::Swirl      => out_glyphs(out, from, t, w, h, false),
        Effect::Fireworks  => out_fireworks(out, f, w, h),
        Effect::Starburst  => out_starburst(out, f, w, h),
        Effect::AlienShips => out_movers(out, f, w, h, true),
        Effect::Asteroids  => out_movers(out, f, w, h, false),
        Effect::Reveal(_)  => {}
    }
}

// -- output-resolution helpers ----------------------------------------------

#[inline]
fn lerp_u32(a: u32, b: u32, t: f32) -> u32 {
    let ch = |ac: u32, bc: u32| ((ac as f32 + (bc as f32 - ac as f32) * t) as u32).min(255);
    (ch((a >> 16) & 0xFF, (b >> 16) & 0xFF) << 16)
        | (ch((a >> 8) & 0xFF, (b >> 8) & 0xFF) << 8)
        |  ch(a & 0xFF, b & 0xFF)
}

fn rgb_to_u32(c: Rgb) -> u32 {
    let c = themed(c);
    ((c[0] as u32) << 16) | ((c[1] as u32) << 8) | (c[2] as u32)
}

fn fill_out(buf: &mut [u32], rx: f32, ry: f32, rw: f32, rh: f32, color: Rgb, w: u32, h: u32) {
    let pix = rgb_to_u32(color);
    // Clamp to [0, dim] *before* casting to u32 so negative values don't
    // wrap to ~4 billion and cause an overflow panic in the inner loop.
    let x0 = (rx.floor()        as i32).clamp(0, w as i32) as u32;
    let y0 = (ry.floor()        as i32).clamp(0, h as i32) as u32;
    let x1 = ((rx + rw).ceil() as i32).clamp(0, w as i32) as u32;
    let y1 = ((ry + rh).ceil() as i32).clamp(0, h as i32) as u32;
    for yy in y0..y1 {
        let row = (yy * w) as usize;
        for xx in x0..x1 { buf[row + xx as usize] = pix; }
    }
}

fn dot_out(buf: &mut [u32], x: f32, y: f32, s: f32, color: Rgb, w: u32, h: u32) {
    fill_out(buf, x - s / 2.0, y - s / 2.0, s, s, color, w, h);
}

fn sample_out(src: &[u32], x: f32, y: f32, w: u32, h: u32) -> Rgb {
    let xi = (x as i32).clamp(0, w as i32 - 1) as u32;
    let yi = (y as i32).clamp(0, h as i32 - 1) as u32;
    let px = src[(yi * w + xi) as usize];
    [((px >> 16) & 0xFF) as u8, ((px >> 8) & 0xFF) as u8, (px & 0xFF) as u8]
}

fn out_glyphs(buf: &mut [u32], from: &[u32], p: f32, w: u32, h: u32, explode: bool) {
    let (wf, hf) = (w as f32, h as f32);
    let (cx, cy) = (wf / 2.0, hf / 2.0);
    let (gw, gh) = (16u32, 10u32);
    let (tw, th) = (wf / gw as f32, hf / gh as f32);
    let f = p * 40.0;
    for j in 0..gh {
        for i in 0..gw {
            let (x0, y0) = ((i as f32 + 0.5) * tw, (j as f32 + 0.5) * th);
            let color = sample_out(from, x0, y0, w, h);
            if is_bg(color) { continue; }
            let id = j * gw + i;
            let (px, py, size) = if explode {
                let (dx, dy) = (x0 - cx, y0 - cy);
                let nrm = (dx * dx + dy * dy).sqrt().max(1.0);
                let speed = 5.0 + rnd(id) * 5.0; // no SSF — output resolution
                let pxp = x0 + dx / nrm * speed * f + (rnd(id + 99) - 0.5) * 13.0;
                let pyp = y0 + dy / nrm * speed * f + 0.5 * 0.17 * f * f;
                (pxp, pyp, tw.min(th) * (1.0 - p))
            } else {
                let (dx, dy) = (x0 - cx, y0 - cy);
                let (r0, a0) = ((dx * dx + dy * dy).sqrt(), dy.atan2(dx));
                let ang = a0 + (3.0 + rnd(id)) * p;
                let rad = r0 * (1.0 - p);
                (cx + rad * ang.cos(), cy + rad * ang.sin(), tw.min(th) * (1.0 - p * 0.7))
            };
            if size >= 0.5 {
                let pix = ((color[0] as u32) << 16) | ((color[1] as u32) << 8) | (color[2] as u32);
                let x0b = ((px - size / 2.0).floor() as i32).clamp(0, w as i32) as u32;
                let y0b = ((py - size / 2.0).floor() as i32).clamp(0, h as i32) as u32;
                let x1b = ((px + size / 2.0).ceil()  as i32).clamp(0, w as i32) as u32;
                let y1b = ((py + size / 2.0).ceil()  as i32).clamp(0, h as i32) as u32;
                for yy in y0b..y1b {
                    let row = (yy * w) as usize;
                    for xx in x0b..x1b { buf[row + xx as usize] = pix; }
                }
            }
        }
    }
}

fn out_fireworks(buf: &mut [u32], f: f32, w: u32, h: u32) {
    let (wf, hf) = (w as f32, h as f32);
    let bottom = hf - 2.0;
    let n = 5u32;
    let grav = 0.01;
    for k in 0..n {
        let rx = wf * (k as f32 + 0.5) / n as f32;
        let launch = k as f32 * 4.0;
        if f < launch { continue; }
        let local = f - launch;
        let rise = 16.0;
        let apex_y = hf * 0.26 + rnd(k) * hf * 0.12;
        if local <= rise {
            let y = bottom - (bottom - apex_y) * (local / rise);
            dot_out(buf, rx, y, 4.0, FIRE_C[k as usize % FIRE_C.len()], w, h);
        } else {
            let bt = local - rise;
            if bt > 16.0 { continue; }
            for s in 0..14u32 {
                let ang = s as f32 * std::f32::consts::TAU / 14.0;
                let spd = 2.5 + rnd(s);
                let px = rx + ang.cos() * spd * bt;
                let py = apex_y + ang.sin() * spd * bt + 0.5 * grav * bt * bt;
                dot_out(buf, px, py, 3.0, FIRE_C[s as usize % FIRE_C.len()], w, h);
            }
        }
    }
}

fn out_starburst(buf: &mut [u32], f: f32, w: u32, h: u32) {
    let (wf, hf) = (w as f32, h as f32);
    let (cx, cy) = (wf / 2.0, hf / 2.0);
    for i in 0..48u32 {
        let ang = i as f32 * std::f32::consts::TAU / 48.0 + rnd(i) * 0.3;
        let spd = 6.0 + rnd(i + 5) * 6.0;
        let (px, py) = (cx + ang.cos() * spd * f, cy + ang.sin() * spd * f);
        dot_out(buf, px, py, 2.0 + rnd(i) * 2.0, STAR_C[i as usize % STAR_C.len()], w, h);
    }
}

fn out_movers(buf: &mut [u32], f: f32, w: u32, h: u32, ufo: bool) {
    let (wf, hf) = (w as f32, h as f32);
    let dur = 40.0;
    let n = if ufo { 4u32 } else { 6u32 };
    for k in 0..n {
        if ufo {
            let lane = hf * (k as f32 + 1.0) / (n as f32 + 1.0);
            let x = -40.0 + (wf + 80.0) * (f / dur);
            let by = lane + (f * 0.25 + k as f32).sin() * 10.0;
            out_ufo(buf, x, by, ALIEN_C[k as usize % ALIEN_C.len()], w, h);
        } else {
            let sx = wf + 33.0;
            let sy = hf * (rnd(k) * 0.8 + 0.1);
            let (vx, vy) = (-(8.0 + rnd(k + 3) * 6.0), (rnd(k + 7) - 0.5) * 6.0);
            out_rock(buf, sx + vx * f, sy + vy * f, k, w, h);
        }
    }
}

fn out_ufo(buf: &mut [u32], x: f32, y: f32, c: Rgb, w: u32, h: u32) {
    let bw = 23.0;
    let bh = 5.0;
    fill_out(buf, x - bw / 2.0, y,           bw,       bh,       c,               w, h);
    fill_out(buf, x - bw * 0.28, y - bh * 0.8, bw * 0.56, bh, [230, 240, 245],   w, h);
    for d in 0..3 {
        dot_out(buf, x + (d as f32 - 1.0) * bw * 0.28, y + bh, 2.0, [250, 240, 120], w, h);
    }
}

fn out_rock(buf: &mut [u32], x: f32, y: f32, k: u32, w: u32, h: u32) {
    let s = 5.0 + rnd(k) * 3.0;
    fill_out(buf, x - s / 2.0,   y - s / 2.0,   s,        s,        ROCK_C[k as usize % ROCK_C.len()],           w, h);
    fill_out(buf, x - s * 0.32,  y - s * 0.55,  s * 0.64, s * 0.5,  ROCK_C[(k as usize + 1) % ROCK_C.len()],    w, h);
    for t in 1..4u32 {
        dot_out(buf, x + t as f32 * 4.0, y - (rnd(k + t) - 0.5) * 3.0, 4.0 - t as f32, [90, 80, 70], w, h);
    }
}
