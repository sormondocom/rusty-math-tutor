//! Transitions between problem cards.
//!
//! Two families share one playback loop:
//!
//! * **Reveals** — cheap per-cell threshold maps (wipe, curtain, dissolve,
//!   blinds, circle, slide, diagonal).  Pure integer work, great on slow
//!   terminals.
//! * **Particle effects** — the old equation *explodes* into flying glyph
//!   pieces or *swirls* into the centre; the new card warps in behind a
//!   *starburst*, gets celebrated with *fireworks*, or is buzzed by a fleet of
//!   *alien ships* / a shower of tumbling *asteroids*.  These borrow the
//!   analytic-physics model from the console-music-player visualizer: every
//!   particle's position is a closed form of elapsed frames, so there is no
//!   per-tick simulation state to keep in sync.
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
    /// Stars zoom outward from the centre as the new card warps in.
    Starburst,
    /// Flying-saucer fleet crosses the screen with tractor beams.
    AlienShips,
    /// Tumbling asteroids streak across, trailing debris.
    Asteroids,
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
        v.extend([
            Effect::Explode,
            Effect::Swirl,
            Effect::Fireworks,
            Effect::Starburst,
            Effect::Starburst,
            Effect::AlienShips,
            Effect::AlienShips,
            Effect::Asteroids,
            Effect::Asteroids,
        ]);
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

/// The frontend-neutral state of a transition: which effect, and how far along.
///
/// The **core** owns this — it carries the timing and lets the app gate input
/// and know when to swap problems — but holds no pixels.  The terminal frontend
/// builds the matching visual [`Transition`] and renders it at [`progress`].
#[derive(Copy, Clone, Debug)]
pub struct TransitionPhase {
    pub effect: Effect,
    pub progress: f32,
}

impl TransitionPhase {
    /// Begin a transition with a randomly chosen effect.
    pub fn new(rng: &mut impl Rng) -> Self {
        TransitionPhase { effect: Effect::random(rng), progress: 0.0 }
    }

    /// Advance one tick; returns `true` once finished.
    pub fn advance(&mut self) -> bool {
        self.progress += 1.0 / self.effect.duration();
        self.progress >= 1.0
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

/// A star in the [`Effect::Starburst`] effect — flies straight out from centre.
struct Star {
    vx: f32,
    vy: f32,
    color: Color,
}

#[derive(Copy, Clone)]
enum SpriteKind {
    Ufo,
    Asteroid,
}

/// A sprite that drifts linearly across the card (saucer or asteroid).
struct Mover {
    x0: f32,
    y0: f32,
    vx: f32,
    vy: f32,
    kind: SpriteKind,
    color: Color,
    phase: f32,
}

const FIREWORK_COLORS: [Color; 6] = [
    Color::LightRed,
    Color::LightYellow,
    Color::LightGreen,
    Color::LightCyan,
    Color::LightMagenta,
    Color::White,
];

const STAR_COLORS: [Color; 4] = [Color::White, Color::LightCyan, Color::LightYellow, Color::Gray];

const ALIEN_COLORS: [Color; 4] = [Color::LightGreen, Color::LightCyan, Color::LightMagenta, Color::Green];

const ROCK_COLORS: [Color; 3] = [Color::Rgb(150, 130, 110), Color::Rgb(120, 110, 100), Color::Rgb(95, 90, 85)];

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
    stars: Vec<Star>,
    movers: Vec<Mover>,
}

impl Transition {
    /// Construct the visual for a specific effect.  The effect itself is chosen
    /// by the core ([`TransitionPhase`]); the terminal frontend builds this to
    /// match, capturing the outgoing/incoming cards.
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
        let stars = match effect {
            Effect::Starburst => build_stars(rng),
            _ => Vec::new(),
        };
        let movers = match effect {
            Effect::AlienShips => build_movers(area, SpriteKind::Ufo, effect.duration(), rng),
            Effect::Asteroids => build_movers(area, SpriteKind::Asteroid, effect.duration(), rng),
            _ => Vec::new(),
        };
        Transition { effect, from, to, progress: 0.0, seed: rng.gen(), glyphs, rockets, stars, movers }
    }

    /// Drive the playback position.  The terminal frontend sets this from the
    /// core's [`TransitionPhase::progress`] each frame, so the visual stays in
    /// lockstep with the (headless) core that owns the timing.
    pub fn set_progress(&mut self, p: f32) {
        self.progress = p;
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let p = self.progress.clamp(0.0, 1.0);
        match self.effect {
            Effect::Reveal(kind) => self.render_reveal(kind, area, buf, p),
            Effect::Explode | Effect::Swirl => self.render_glyph_particles(area, buf, p),
            Effect::Fireworks => self.render_fireworks(area, buf, p),
            Effect::Starburst => self.render_starburst(area, buf, p),
            Effect::AlienShips | Effect::Asteroids => self.render_movers(area, buf, p),
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

    /// Draw the incoming card, hiding its equation glyphs until `reveal(x, y)`
    /// returns true — used as the backdrop for every particle effect so the new
    /// answer materialises behind the action.
    fn paint_backdrop(&self, area: Rect, buf: &mut Buffer, reveal: impl Fn(u16, u16) -> bool) {
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                let toc = &self.to[(x, y)];
                buf[(x, y)] = if toc.symbol() == GLYPH && !reveal(x, y) {
                    let mut blank = Cell::EMPTY;
                    blank.set_bg(BG);
                    blank
                } else {
                    toc.clone()
                };
            }
        }
    }

    /// The default dissolve reveal (a glyph cell appears once its noise ≤ p).
    fn dissolve_reveal(&self, p: f32) -> impl Fn(u16, u16) -> bool + '_ {
        move |x, y| cell_noise(x, y, self.seed) <= p
    }

    fn render_glyph_particles(&self, area: Rect, buf: &mut Buffer, p: f32) {
        self.paint_backdrop(area, buf, self.dissolve_reveal(p));
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
        self.paint_backdrop(area, buf, self.dissolve_reveal(p));
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

    fn render_starburst(&self, area: Rect, buf: &mut Buffer, p: f32) {
        let cx = area.left() as f32 + area.width as f32 / 2.0;
        let cy = area.top() as f32 + area.height as f32 / 2.0;
        let hw = (area.width as f32 / 2.0).max(1.0);
        let hh = (area.height as f32 / 2.0).max(1.0);

        // The new card warps in from the centre outward, in step with the stars.
        self.paint_backdrop(area, buf, |x, y| {
            let dx = (x as f32 - cx) / hw;
            let dy = (y as f32 - cy) / hh;
            (dx * dx + dy * dy).sqrt() <= p * 1.5
        });

        let f = p * self.effect.duration();
        for (i, s) in self.stars.iter().enumerate() {
            plot(buf, area, cx + s.vx * f, cy + s.vy * f, star_glyph(f, i), s.color);
        }
    }

    fn render_movers(&self, area: Rect, buf: &mut Buffer, p: f32) {
        self.paint_backdrop(area, buf, self.dissolve_reveal(p));
        let f = p * self.effect.duration();

        for m in &self.movers {
            let px = m.x0 + m.vx * f;
            let py = m.y0 + m.vy * f;
            match m.kind {
                SpriteKind::Ufo => {
                    // A gentle hover bob on top of the cruise.
                    let by = py + (f * 0.25 + m.phase).sin();
                    draw_ufo(buf, area, px, by, m.color, f);
                }
                SpriteKind::Asteroid => {
                    // A short debris trail behind the direction of travel.
                    let speed = (m.vx * m.vx + m.vy * m.vy).sqrt().max(0.001);
                    for k in 1..=2 {
                        let tx = px - m.vx / speed * (k as f32 * 1.6);
                        let ty = py - m.vy / speed * (k as f32 * 1.6);
                        plot(buf, area, tx, ty, "·", Color::Rgb(80, 75, 70));
                    }
                    draw_asteroid(buf, area, px, py, m.color, f + m.phase);
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

fn build_stars(rng: &mut impl Rng) -> Vec<Star> {
    let count = rng.gen_range(45..=75);
    (0..count)
        .map(|_| {
            let ang = rng.gen_range(0.0..TAU);
            let speed = rng.gen_range(0.5..1.8);
            Star {
                vx: ang.cos() * speed,
                vy: ang.sin() * speed * 0.5, // halve vertically: cells are tall
                color: *STAR_COLORS.choose(rng).unwrap(),
            }
        })
        .collect()
}

fn build_movers(area: Rect, kind: SpriteKind, dur: f32, rng: &mut impl Rng) -> Vec<Mover> {
    let count = match kind {
        SpriteKind::Ufo => rng.gen_range(3..=5),
        SpriteKind::Asteroid => rng.gen_range(4..=7),
    };
    let span = area.width as f32 + 16.0;
    let top = area.top() as f32;
    let h = area.height as f32;

    (0..count)
        .map(|_| {
            let from_left = rng.gen_bool(0.5);
            let dir = if from_left { 1.0 } else { -1.0 };
            let x0 = if from_left { area.left() as f32 - 8.0 } else { area.right() as f32 + 8.0 };
            match kind {
                SpriteKind::Ufo => Mover {
                    x0,
                    y0: top + rng.gen_range(0.12..0.78) * h,
                    vx: dir * span / dur * rng.gen_range(0.9..1.3),
                    vy: 0.0,
                    kind,
                    color: *ALIEN_COLORS.choose(rng).unwrap(),
                    phase: rng.gen_range(0.0..TAU),
                },
                SpriteKind::Asteroid => Mover {
                    x0,
                    y0: top + rng.gen_range(0.0..1.0) * h,
                    vx: dir * span / dur * rng.gen_range(0.8..1.2),
                    vy: rng.gen_range(-0.25..0.25),
                    kind,
                    color: *ROCK_COLORS.choose(rng).unwrap(),
                    phase: rng.gen_range(0.0..50.0),
                },
            }
        })
        .collect()
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

/// A twinkling star glyph that cycles as the star travels.
fn star_glyph(f: f32, i: usize) -> &'static str {
    match (f as usize + i) % 8 {
        0 | 1 => "+",
        2 | 3 => "*",
        4 | 5 => "·",
        _ => ".",
    }
}

/// Stamp a one-row sprite string (spaces are transparent) with its left edge at
/// `left`, clipped to `area`.
fn stamp(buf: &mut Buffer, area: Rect, line: &str, left: f32, y: f32, color: Color) {
    for (i, ch) in line.chars().enumerate() {
        if ch == ' ' {
            continue;
        }
        let mut tmp = [0u8; 4];
        plot(buf, area, left + i as f32, y, ch.encode_utf8(&mut tmp), color);
    }
}

/// A little flying saucer centred at `(cx, cy)`, with a blinking tractor beam.
fn draw_ufo(buf: &mut Buffer, area: Rect, cx: f32, cy: f32, color: Color, f: f32) {
    let left = cx - 3.0; // sprites are 7 cells wide
    stamp(buf, area, " .-^-. ", left, cy - 1.0, color);
    stamp(buf, area, "(__o__)", left, cy, color);
    // Tractor beam pulses below the saucer.
    if (f as u32 / 3) % 2 == 0 {
        for k in 1..=3 {
            plot(buf, area, cx, cy + 1.0 + k as f32, ":", Color::Rgb(120, 220, 160));
        }
    }
}

/// A tumbling asteroid centred at `(cx, cy)`; `t` advances the rotation.
fn draw_asteroid(buf: &mut Buffer, area: Rect, cx: f32, cy: f32, color: Color, t: f32) {
    // Four rough rotation frames (3 wide, 2 tall).
    const FRAMES: [[&str; 2]; 4] = [
        [" # ", "###"],
        ["## ", " ##"],
        ["###", " # "],
        [" ##", "## "],
    ];
    let frame = &FRAMES[(t as usize / 3) % FRAMES.len()];
    stamp(buf, area, frame[0], cx - 1.0, cy, color);
    stamp(buf, area, frame[1], cx - 1.0, cy + 1.0, color);
}

/// Deterministic per-cell noise in `0.0..1.0` from a cheap integer hash.
fn cell_noise(x: u16, y: u16, seed: u32) -> f32 {
    let mut h = seed ^ (x as u32).wrapping_mul(0x9E3779B1) ^ (y as u32).wrapping_mul(0x85EBCA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 65535.0
}
