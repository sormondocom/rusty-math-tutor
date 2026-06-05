//! Terminal-frontend transition render-state.
//!
//! The core ([`App`]) tracks only the timing; this module owns the captured
//! cell buffers and particle state that animate problem-to-problem and
//! scene-to-scene transitions.  [`Transitions::sync`] keeps the visuals in
//! step with the core each frame.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::app::{App, Cinematic};
use crate::config::Layout;
use crate::section::Active;
use crate::transition::{Effect, Transition};

use super::*;

#[derive(Default)]
pub struct Transitions {
    /// Problem-to-problem transition (Practice / Challenge).
    pub(super) problem: Option<Transition>,
    /// Scene-to-scene transition (milestone cinematics).
    pub(super) scene: Option<Transition>,
}

impl Transitions {
    /// Reconcile the visuals with the core's phases: build one when a
    /// transition starts, drive its progress while it plays, and drop it when
    /// it ends.
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

fn capture_card(area: Rect, active: &Active, input: &str, progress: f32, layout: Layout, banner: Option<(String, Color)>) -> Buffer {
    let mut buf = Buffer::empty(area);
    match active {
        Active::Shape(s) => render_shape_card(area, &mut buf, s, input, progress, banner),
        Active::Unit(u)  => render_unit_card(area, &mut buf, u, input, banner),
        Active::Geo(g)   => render_geometry_card(area, &mut buf, g, input, banner),
        Active::Arith(p) => render_card(area, &mut buf, p, input, layout, banner),
    }
    buf
}

fn build_problem_transition(app: &App, area: Rect, effect: Effect, rng: &mut impl rand::Rng) -> Transition {
    let card = card_area(app.screen, area);
    let from = capture_card(card, &app.current, &app.input, 1.0, app.config.layout, Some(("✓  Correct!".to_string(), Color::LightGreen)));
    let to = match &app.pending {
        Some(p) => capture_card(card, p, "", 0.0, app.config.layout, None),
        None    => Buffer::empty(card),
    };
    Transition::with_effect(effect, from, to, rng)
}

fn capture_scene(area: Rect, scene: &crate::cinematic::Scene, frame: u64) -> Buffer {
    let mut buf = Buffer::empty(area);
    render_scene(area, &mut buf, scene, frame);
    buf
}

fn build_scene_transition(cin: &Cinematic, area: Rect, effect: Effect, rng: &mut impl rand::Rng) -> Transition {
    let from = capture_scene(area, &cin.scenes[cin.index], cin.scenes[cin.index].dwell as u64);
    let to   = capture_scene(area, &cin.scenes[cin.index + 1], 0);
    Transition::with_effect(effect, from, to, rng)
}
