//! Frontend-neutral transition state — the data the core owns.
//!
//! This file contains **only** the pure data types and timing logic that
//! `app.rs` needs.  It has no rendering code and no OS dependencies, so it
//! compiles to every target including `wasm32-unknown-unknown`.
//!
//! The full rendering implementations live alongside each frontend:
//! - Terminal: `src/frontends/terminal/transition.rs`
//! - GUI:      `src/frontends/gui/scene/transition.rs`
//!
//! Both of those files re-define these same types (matching layout) and add
//! their rendering code on top.  This file is used exclusively by the
//! `[lib]` WASM crate, whose `crate::transition` points here via `#[path]`.

use rand::Rng;
use rand::seq::SliceRandom;

// ---------------------------------------------------------------------------
// Effect taxonomy
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
    pub fn all() -> Vec<Effect> {
        let mut v: Vec<Effect> = [
            Kind::Wipe, Kind::Curtain, Kind::Dissolve,
            Kind::Blinds, Kind::Circle, Kind::Slide, Kind::Diagonal,
        ]
        .into_iter()
        .map(Effect::Reveal)
        .collect();
        // Showy effects weighted a little heavier — they hold attention.
        v.extend([
            Effect::Explode, Effect::Swirl,
            Effect::Fireworks,
            Effect::Starburst, Effect::Starburst,
            Effect::AlienShips, Effect::AlienShips,
            Effect::Asteroids, Effect::Asteroids,
        ]);
        v
    }

    fn random(rng: &mut impl Rng) -> Effect {
        *Effect::all().choose(rng).unwrap()
    }

    fn duration(self) -> f32 {
        match self {
            Effect::Reveal(_) => 24.0,
            _                 => 40.0,
        }
    }
}

// ---------------------------------------------------------------------------
// TransitionPhase — the only state the core keeps during a transition
// ---------------------------------------------------------------------------

/// Frontend-neutral transition state: which effect is playing and how far along.
///
/// The **core** owns this — it gates input and knows when to swap problems —
/// but holds no pixels.  Each frontend builds its own visual representation
/// and advances the phase to completion.
#[derive(Copy, Clone, Debug)]
pub struct TransitionPhase {
    pub effect:   Effect,
    pub progress: f32,
}

impl TransitionPhase {
    /// Begin a transition with a randomly chosen effect.
    pub fn new(rng: &mut impl Rng) -> Self {
        TransitionPhase { effect: Effect::random(rng), progress: 0.0 }
    }

    /// Advance one tick; returns `true` when the transition is finished.
    pub fn advance(&mut self) -> bool {
        self.progress += 1.0 / self.effect.duration();
        self.progress >= 1.0
    }
}
