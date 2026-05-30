//! Transitions between problem cards.
//!
//! Two families share one playback loop:
//!
//! * **Reveals** — cheap per-cell threshold maps (wipe, curtain, dissolve,
//!   blinds, circle, slide, diagonal).  Pure integer work, great on slow
//!   terminals.
//! * **Particle effects** — the old equation *explodes* into flying glyph
//!   pieces, *swirls* into the centre, or the new card is celebrated with
//!   *fireworks*.  These borrow the analytic-physics model from the
//!   console-music-player visualizer: every particle's position is a closed
//!   form of elapsed frames, so there is no per-tick simulation state to keep
//!   in sync.
//!
//! All effects animate two captured [`Buffer`]s — the outgoing `from` card and
//! the incoming `to` card — from one to the other.

use std::f32::consts::TAU;

use rand::seq::SliceRandom;
use rand::Rng;
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::Rect;
use ratatui::style::Color;

/// Card background, matching [`crate::ui`]'s `BG`.
const BG: Color = Color::Rgb(16, 18, 28);
/// The glyph cell the block font paints; particle effects lift these out.
const GLYPH: &str = "█";

// ---------------------------------------------------------------------------
// Effect selection
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug)]
pub enum Kind {
    Wipe,
    Curtain,
    Dissolve,
    Blinds,
    Circle,
    Slide,
    Diagonal,
}

#[derive(Copy, Clone, Debug)]
pub enum Effect {
    Reveal(Kind),
    Explode,
    Swirl,
    Fireworks,
}

impl Effect {
    /// Every effect, used both for random selection and for test coverage.
    pub fn all() -> Vec<Effect> {
        let mut v: Vec<Effect> = [
            Kind::Wipe,
            Kind::Curtain,
            Kind::Dissolve,
            Kind::Blinds,
            Kind::Circle,
            Kind::Slide,
            Kind::Diagonal,
        ]
        .into_iter()
        .map(Effect::Reveal)
        .collect();
        // The showy ones are weighted a little heavier — they hold attention.
        v.extend([Effect::Explode, Effect::Explode, Effect::Swirl, Effect::Swirl, Effect::Fireworks, Effect::Fireworks]);
        v
    }

    fn random(rng: &mut impl Rng) -> Effect {
        *Effect::all().choose(rng).unwrap()
    }

    fn duration(self) -> f32 {
        match self {
            Effect::Reveal(_) => 24.0,
            _ => 40.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Particles (analytic — position is a function of elapsed frames)
// ---------------------------------------------------------------------------

/// A single glyph cell lifted off the outgoing card.
struct Glyph {
    x0: f32,
    y0: f32,
    vx: f32,
    vy: f32,
    grav: f32,
    spin: f32,
    fg: Color,
}

struct Spark {
    vx: f32,
    vy: f32,
    color: Color,
}

struct Rocket {
    x: f32,
    launch: f32,
    rise: f32,
    apex_y: f32,
    color: Color,
    sparks: Vec<Spark>,
}

const FIREWORK_COLORS: [Color; 6] = [
    Color::LightRed,
    Color::LightYellow,
    Color::LightGreen,
    Color::LightCyan,
    Color::LightMagenta,
    Color::White,
];

// ---------------------------------------------------------------------------
// Transition
// ---------------------------------------------------------------------------

pub struct Transition {
    effect: Effect,
    from: Buffer,
    to: Buffer,
    progress: f32,
    seed: u32,
    glyphs: Vec<Glyph>,
    rockets: Vec<Rocket>,
}

impl Transition {
    pub fn new(from: Buffer, to: Buffer, rng: &mut impl Rng) -> Self {
        Self::with_effect(Effect::random(rng), from, to, rng)
    }

    /// Construct with a specific effect (used by tests for full coverage).
    pub fn with_effect(effect: Effect, from: Buffer, to: Buffer, rng: &mut impl Rng) -> Self {
        let area = from.area;
        let glyphs = match effect {
            Effect::Explode => build_glyphs(&from, area, true, rng),
            Effect::Swirl => build_glyphs(&from, area, false, rng),
            _ => Vec::new(),
        };
        let rockets = match effect {
            Effect::Fireworks => build_rockets(area, effect.duration(), rng),
            _ => Vec::new(),
        };
        Transition { effect, from, to, progress: 0.0, seed: rng.gen(), glyphs, rockets }
    }

    /// Advance one tick; returns `true` once finished.
    pub fn advance(&mut self) -> bool {
        self.progress += 1.0 / self.effect.duration();
        self.progress >= 1.0
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let p = self.progress.clamp(0.0, 1.0);
        match self.effect {
            Effect::Reveal(kind) => self.render_reveal(kind, area, buf, p),
            Effect::Explode | Effect::Swirl => self.render_glyph_particles(area, buf, p),
            Effect::Fireworks => self.render_fireworks(area, buf, p),
        }
    }

    // -- reveals ------------------------------------------------------------

    fn render_reveal(&self, kind: Kind, area: Rect, buf: &mut Buffer, p: f32) {
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                buf[(x, y)] = match self.reveal_pick(kind, x, y, area, p) {
                    Pick::From(sx, sy) => self.from[(sx, sy)].clone(),
                    Pick::To(sx, sy) => self.to[(sx, sy)].clone(),
                };
            }
        }
    }

    fn reveal_pick(&self, kind: Kind, x: u16, y: u16, area: Rect, p: f32) -> Pick {
        let w = area.width.max(1) as f32;
        let h = area.height.max(1) as f32;
        let nx = (x - area.left()) as f32 / w;
        let ny = (y - area.top()) as f32 / h;
        let to = |b: bool| if b { Pick::To(x, y) } else { Pick::From(x, y) };
        match kind {
            Kind::Wipe => to(nx <= p),
            Kind::Curtain => to((nx - 0.5).abs() * 2.0 <= p),
            Kind::Dissolve => to(cell_noise(x, y, self.seed) <= p),
            Kind::Blinds => to((y % 4) as f32 / 4.0 <= p),
            Kind::Circle => {
                let dx = nx - 0.5;
                let dy = (ny - 0.5) * 2.0;
                to((dx * dx + dy * dy).sqrt() / 0.75 <= p)
            }
            Kind::Slide => {
                let shift = (p * w) as u16;
                let virt = x.saturating_sub(area.left()) + shift;
                if virt < area.width {
                    Pick::From(area.left() + virt, y)
                } else {
                    Pick::To(area.left() + (virt - area.width), y)
                }
            }
            Kind::Diagonal => to((nx + ny) * 0.5 <= p),
        }
    }

    // -- particle effects ---------------------------------------------------

    /// Draw the incoming card with its equation glyphs gated by `p` (so the new
    /// answer materialises), used as the backdrop for every particle effect.
    fn paint_backdrop(&self, area: Rect, buf: &mut Buffer, p: f32) {
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                let toc = &self.to[(x, y)];
                buf[(x, y)] = if toc.symbol() == GLYPH && cell_noise(x, y, self.seed) > p {
                    let mut blank = Cell::EMPTY;
                    blank.set_bg(BG);
                    blank
                } else {
                    toc.clone()
                };
            }
        }
    }

    fn render_glyph_particles(&self, area: Rect, buf: &mut Buffer, p: f32) {
        self.paint_backdrop(area, buf, p);
        let f = p * self.effect.duration();
        let dur = self.effect.duration();
        let cx = area.left() as f32 + area.width as f32 / 2.0;
        let cy = area.top() as f32 + area.height as f32 / 2.0;

        for g in &self.glyphs {
            let (px, py) = match self.effect {
                Effect::Swirl => {
                    let dx = g.x0 - cx;
                    let dy = g.y0 - cy;
                    let r0 = (dx * dx + dy * dy).sqrt();
                    let a0 = dy.atan2(dx);
                    let ang = a0 + g.spin * f;
                    let rad = r0 * (1.0 - f / dur).max(0.0);
                    (cx + rad * ang.cos(), cy + rad * ang.sin())
                }
                _ => {
                    // Explode: ballistic flight with gravity.
                    (g.x0 + g.vx * f, g.y0 + g.vy * f + 0.5 * g.grav * f * f)
                }
            };
            plot(buf, area, px, py, GLYPH, g.fg);
        }
    }

    fn render_fireworks(&self, area: Rect, buf: &mut Buffer, p: f32) {
        self.paint_backdrop(area, buf, p);
        let f = p * self.effect.duration();
        let bottom = area.bottom().saturating_sub(1) as f32;
        const SPARK_LIFE: f32 = 16.0;
        const GRAV: f32 = 0.03;
        let trail = ['.', '*', '|'];

        for r in &self.rockets {
            if f < r.launch {
                continue;
            }
            let local = f - r.launch;
            if local <= r.rise {
                // Rising: a little flickering tail.
                let t = local / r.rise;
                let y = bottom - (bottom - r.apex_y) * t;
                let ch = trail[(local as usize) % trail.len()];
                plot(buf, area, r.x, y, &ch.to_string(), r.color);
            } else {
                // Burst.
                let bt = local - r.rise;
                if bt > SPARK_LIFE {
                    continue;
                }
                let glyph = burst_glyph(bt);
                for s in &r.sparks {
                    let px = r.x + s.vx * bt;
                    let py = r.apex_y + s.vy * bt + 0.5 * GRAV * bt * bt;
                    plot(buf, area, px, py, glyph, s.color);
                }
            }
        }
    }
}

enum Pick {
    From(u16, u16),
    To(u16, u16),
}

// ---------------------------------------------------------------------------
// Builders
// ---------------------------------------------------------------------------

/// Lift every block-font glyph cell off `from` into a particle.  When
/// `explode` is true they get outward ballistic velocities; otherwise they get
/// a per-particle spin for the swirl.
fn build_glyphs(from: &Buffer, area: Rect, explode: bool, rng: &mut impl Rng) -> Vec<Glyph> {
    let cx = area.left() as f32 + area.width as f32 / 2.0;
    let cy = area.top() as f32 + area.height as f32 / 2.0;
    let mut out = Vec::new();
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if from[(x, y)].symbol() != GLYPH {
                continue;
            }
            let fg = from[(x, y)].fg;
            if explode {
                let dx = x as f32 - cx;
                let dy = (y as f32 - cy) * 0.5; // cells are ~twice as tall
                let len = (dx * dx + dy * dy).sqrt().max(0.5);
                let speed = rng.gen_range(0.6..1.8);
                out.push(Glyph {
                    x0: x as f32,
                    y0: y as f32,
                    vx: dx / len * speed,
                    vy: dy / len * speed - rng.gen_range(0.0..0.3),
                    grav: 0.02,
                    spin: 0.0,
                    fg,
                });
            } else {
                out.push(Glyph {
                    x0: x as f32,
                    y0: y as f32,
                    vx: 0.0,
                    vy: 0.0,
                    grav: 0.0,
                    spin: rng.gen_range(0.10..0.22) * if rng.gen_bool(0.5) { 1.0 } else { -1.0 },
                    fg,
                });
            }
            if out.len() >= 1500 {
                return out;
            }
        }
    }
    out
}

fn build_rockets(area: Rect, duration: f32, rng: &mut impl Rng) -> Vec<Rocket> {
    let count = rng.gen_range(5..=8);
    let mut rockets = Vec::with_capacity(count);
    for _ in 0..count {
        let x = area.left() as f32 + rng.gen_range(0.1..0.9) * area.width as f32;
        let apex_y = area.top() as f32 + rng.gen_range(0.1..0.45) * area.height as f32;
        let color = *FIREWORK_COLORS.choose(rng).unwrap();
        let n = rng.gen_range(12..=18);
        let mut sparks = Vec::with_capacity(n);
        for _ in 0..n {
            let ang = rng.gen_range(0.0..TAU);
            let speed = rng.gen_range(0.3..0.9);
            sparks.push(Spark {
                vx: ang.cos() * speed,
                vy: ang.sin() * speed * 0.5 - 0.2, // bias upward, halve for aspect
                color,
            });
        }
        rockets.push(Rocket {
            x,
            launch: rng.gen_range(0.0..duration * 0.45),
            rise: rng.gen_range(0.25..0.35) * duration,
            apex_y,
            color,
            sparks,
        });
    }
    rockets
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Stamp a single cell at floating-point `(x, y)`, clipped to `area`.
fn plot(buf: &mut Buffer, area: Rect, x: f32, y: f32, symbol: &str, fg: Color) {
    if !x.is_finite() || !y.is_finite() {
        return;
    }
    let xi = x.round();
    let yi = y.round();
    if xi < area.left() as f32 || xi >= area.right() as f32 || yi < area.top() as f32 || yi >= area.bottom() as f32 {
        return;
    }
    buf[(xi as u16, yi as u16)].set_symbol(symbol).set_fg(fg);
}

fn burst_glyph(bt: f32) -> &'static str {
    // Sparks brighten then fade to embers as they age.
    match bt as u32 {
        0..=3 => "*",
        4..=8 => "+",
        9..=12 => "•",
        _ => ".",
    }
}

/// Deterministic per-cell noise in `0.0..1.0` from a cheap integer hash.
fn cell_noise(x: u16, y: u16, seed: u32) -> f32 {
    let mut h = seed ^ (x as u32).wrapping_mul(0x9E3779B1) ^ (y as u32).wrapping_mul(0x85EBCA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 65535.0
}
