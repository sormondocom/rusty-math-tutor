//! WASM frontend — the browser-facing entry point.
//!
//! `WasmApp` is the JavaScript-visible class.  The page:
//!   1. Calls `new WasmApp()` once — loads config + roster from localStorage.
//!   2. Calls `app.bind_canvas("canvas")` to attach the renderer.
//!   3. Calls `app.tick()` and `app.render()` on every `requestAnimationFrame`.
//!   4. Feeds events via `app.on_key(key, ctrl)`.
//!
//! Rendering uses the exact same tiny-skia pipeline as the desktop window:
//! `crate::scene::render()` produces a supersampled Pixmap, we box-filter it
//! to the canvas dimensions, and write the pixels via `ImageData`.
//!
//! Frame caching: `visual_hash()` fingerprints all render-relevant app state.
//! When the hash matches the previous frame's we skip the render entirely —
//! the canvas already contains the right pixels.

mod storage;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::app::{App, Feedback, Screen};
use crate::config::Config;
use crate::input::{InputEvent, Key, Mods};

// ---------------------------------------------------------------------------
// WasmApp
// ---------------------------------------------------------------------------

#[wasm_bindgen]
pub struct WasmApp {
    app:       App,
    canvas:    Option<web_sys::HtmlCanvasElement>,
    ctx2d:     Option<web_sys::CanvasRenderingContext2d>,
    /// Hash of the last rendered frame.  `0` forces the first render.
    last_hash: u64,
    /// Canvas dimensions at the last render.  A resize forces a re-render.
    last_size: (u32, u32),
    /// Pre-captured (from, to) card frames at output resolution.
    /// Populated once at the start of each non-chalk transition so subsequent
    /// ticks can call `blend_output` (output resolution) instead of re-rendering
    /// both cards at SS resolution on every tick — a 9× pixel reduction per tick.
    transition_frames: Option<(Vec<u32>, Vec<u32>)>,
}

#[wasm_bindgen]
impl WasmApp {
    /// Construct the app and load saved progress from `localStorage`.
    /// Falls back to in-memory storage when the browser blocks it.
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmApp {
        let store: Box<dyn crate::storage::Storage> =
            if let Some(ws) = storage::WebStorage::new() {
                Box::new(ws)
            } else {
                Box::new(crate::storage::MemStorage::default())
            };

        let config  = Config::load(store.as_ref());
        let mut app = App::new(config, store);
        app.screen  = Screen::Menu; // skip the "pick graphics mode" picker

        // Detect the browser's local timezone offset.
        // JS Date.getTimezoneOffset() returns minutes WEST of UTC (positive = behind).
        // We negate so our convention is minutes EAST (positive = ahead of UTC).
        app.time_local_offset =
            -(js_sys::Date::new_0().get_timezone_offset() as i32);
        WasmApp { app, canvas: None, ctx2d: None, last_hash: 0, last_size: (0, 0), transition_frames: None }
    }

    /// Attach the renderer to the `<canvas>` with the given element id.
    /// Returns `true` on success; must be called before `render()`.
    pub fn bind_canvas(&mut self, canvas_id: &str) -> bool {
        let result: Option<()> = (|| {
            let window   = web_sys::window()?;
            let document = window.document()?;
            let element  = document.get_element_by_id(canvas_id)?;
            let canvas: web_sys::HtmlCanvasElement = element.dyn_into().ok()?;
            let raw_ctx  = canvas.get_context("2d").ok()??;
            let ctx: web_sys::CanvasRenderingContext2d = raw_ctx.dyn_into().ok()?;
            self.canvas = Some(canvas);
            self.ctx2d  = Some(ctx);
            Some(())
        })();
        result.is_some()
    }

    /// Advance the simulation one logical tick (~33 ms).
    pub fn tick(&mut self) {
        self.app.on_tick();
    }

    /// Deliver a `keydown` event.  `key` is `KeyboardEvent.key`.
    pub fn on_key(&mut self, key: &str, ctrl: bool) {
        let k = match key {
            "Enter"      => Key::Enter,
            "Escape"     => Key::Esc,
            "Backspace"  => Key::Backspace,
            "Tab"        => Key::Tab,
            "ArrowUp"    => Key::Up,
            "ArrowDown"  => Key::Down,
            "ArrowLeft"  => Key::Left,
            "ArrowRight" => Key::Right,
            s if s.chars().count() == 1 => Key::Char(s.chars().next().unwrap()),
            _ => return,
        };
        self.app.on_event(InputEvent::Key { key: k, mods: Mods { ctrl } });
    }

    /// Paint the current frame to the bound canvas.
    /// Skips the render if visual state is unchanged from the last call.
    pub fn render(&mut self) {
        let Some(canvas) = &self.canvas else { return };
        let Some(ctx)    = &self.ctx2d  else { return };

        let w = canvas.width();
        let h = canvas.height();
        if w == 0 || h == 0 { return; }

        // ── Frame cache check ─────────────────────────────────────────────
        // Fingerprint all render-relevant state.  If nothing changed and the
        // canvas didn't resize, the previous pixels are still correct.
        let new_hash = visual_hash(&self.app, w, h);
        let new_size = (w, h);
        if new_hash == self.last_hash && new_size == self.last_size {
            return;
        }
        self.last_hash = new_hash;
        self.last_size = new_size;

        // ── Scene render ──────────────────────────────────────────────────
        //
        // Fast path for non-chalk transitions: capture both card frames once
        // at the start of the transition, then blend at output resolution for
        // all subsequent ticks.  `blend_output` works on pre-downsampled
        // Vec<u32> buffers so it touches 4× (SS=2) instead of 9× (SS=3) the
        // output pixel count per tick — a large win for 30-tick transitions.
        //
        // Chalk transitions use the full scene render so the eraser animation
        // (which draws on the outer pixmap) renders correctly.
        let mut output = if let Some(ref phase) = self.app.transition {
            if !crate::scene::is_chalk() {
                // Capture both frames the first tick of the transition.
                if self.transition_frames.is_none() {
                    let sw = w * crate::scene::SS;
                    let sh = h * crate::scene::SS;
                    let wf = w as f32;
                    let hf = h as f32;

                    let from_pm = crate::scene::card_frame(sw, sh, |p| {
                        crate::scene::draw_card(
                            p, &self.app.current, &self.app.input,
                            self.app.feedback, self.app.frac_progress(),
                            self.app.anim_frame, self.app.config.layout, wf, hf,
                        );
                    });
                    let to_pm = crate::scene::card_frame(sw, sh, |p| {
                        if let Some(ref pending) = self.app.pending {
                            crate::scene::draw_card(
                                p, pending, "", crate::app::Feedback::None,
                                0.0, self.app.anim_frame, self.app.config.layout, wf, hf,
                            );
                        }
                    });
                    self.transition_frames = Some((downsample(&from_pm, w, h), downsample(&to_pm, w, h)));
                }

                // Blend at output resolution — no SS overhead.
                let effect   = phase.effect;
                let progress = phase.progress.clamp(0.0, 1.0);
                if let Some((ref from_out, ref to_out)) = self.transition_frames {
                    let mut blended = vec![0u32; (w * h) as usize];
                    crate::scene::blend_output(&mut blended, from_out, to_out, effect, progress, w, h);
                    blended
                } else {
                    downsample(&crate::scene::render(&self.app, w, h), w, h)
                }
            } else {
                // Chalk transition: full SS render (eraser effect needs the board frame).
                downsample(&crate::scene::render(&self.app, w, h), w, h)
            }
        } else {
            // No transition — clear any cached frames and do a normal render.
            self.transition_frames = None;
            downsample(&crate::scene::render(&self.app, w, h), w, h)
        };

        // ── Challenge HUD overlay ─────────────────────────────────────────
        if matches!(self.app.screen, Screen::Practice | Screen::Challenge) {
            if let Some(ref chg) = self.app.challenge {
                let hud = render_hud(w, h,
                    chg.remaining_secs(), chg.duration_secs(), chg.solved);
                for (o, &hp) in output.iter_mut().zip(hud.iter()) {
                    if hp != 0 { *o = hp; }
                }
            }
        }

        // ── Write to canvas ───────────────────────────────────────────────
        // ImageData wants straight RGBA bytes; output is 0x00RRGGBB.
        let mut rgba: Vec<u8> = Vec::with_capacity((w * h * 4) as usize);
        for px in &output {
            rgba.push((px >> 16) as u8); // R
            rgba.push((px >>  8) as u8); // G
            rgba.push(*px        as u8); // B
            rgba.push(255);               // A — fully opaque
        }
        if let Ok(image_data) = web_sys::ImageData::new_with_u8_clamped_array_and_sh(
            wasm_bindgen::Clamped(&rgba), w, h,
        ) {
            let _ = ctx.put_image_data(&image_data, 0.0, 0.0);
        }
    }

    /// Name of the screen currently displayed.
    pub fn screen_name(&self) -> String {
        match self.app.screen {
            Screen::Startup      => "Startup",
            Screen::Menu         => "Menu",
            Screen::Settings     => "Settings",
            Screen::Stats        => "Stats",
            Screen::Teacher      => "Teacher",
            Screen::Cinematic    => "Cinematic",
            Screen::Practice     => "Practice",
            Screen::Challenge    => "Challenge",
            Screen::ChallengeEnd => "ChallengeEnd",
            Screen::Experiment   => "Experiment",
            Screen::Time         => "Time",
        }
        .to_string()
    }

    /// `true` when the user pressed Q or Ctrl-C to quit.
    pub fn should_quit(&self) -> bool {
        self.app.should_quit
    }
}

// ---------------------------------------------------------------------------
// Frame caching — visual hash
// ---------------------------------------------------------------------------

/// Cheap fingerprint of all app state that affects what gets rendered.
/// Ported directly from `gui/mod.rs` so both frontends skip on the same signal.
fn visual_hash(app: &App, w: u32, h: u32) -> u64 {
    let mut s = DefaultHasher::new();

    w.hash(&mut s);
    h.hash(&mut s);
    std::mem::discriminant(&app.screen).hash(&mut s);
    std::mem::discriminant(&app.config.theme).hash(&mut s);
    std::mem::discriminant(&app.config.layout).hash(&mut s);

    match app.screen {
        Screen::Practice | Screen::Challenge => {
            app.input.hash(&mut s);
            std::mem::discriminant(&app.feedback).hash(&mut s);
            app.help_active.hash(&mut s);
            ((app.help_in * 64.0) as u8).hash(&mut s);
            app.why_active.hash(&mut s);
            app.why_scroll.hash(&mut s);
            app.frac_anim.hash(&mut s);
            app.card_anim.hash(&mut s);
            app.revealed.hash(&mut s);
            app.encourage.hash(&mut s);
            app.reprimand.hash(&mut s);
            if app.help_in > 0.01 { app.anim_frame.hash(&mut s); }
            if let Some(ref c) = app.challenge { c.remaining_secs().hash(&mut s); }
            if let Some(ref t) = app.transition {
                ((t.progress * 128.0) as u8).hash(&mut s);
            }
        }
        Screen::Menu => {
            app.menu_index.hash(&mut s);
            app.naming.hash(&mut s);
            app.name_input.hash(&mut s);
            app.roster.current.hash(&mut s);
            app.anim_frame.hash(&mut s);
        }
        Screen::Stats => {
            app.roster.current.hash(&mut s);
        }
        Screen::Settings => {
            app.settings_grade.hash(&mut s);
            app.settings_field.hash(&mut s);
        }
        Screen::Teacher => {
            app.teacher_authed.hash(&mut s);
            std::mem::discriminant(&app.teacher_view).hash(&mut s);
            app.teacher_topic.hash(&mut s);
            app.teacher_rec_index.hash(&mut s);
            app.teacher_adding.hash(&mut s);
            app.teacher_text.hash(&mut s);
            app.teacher_msg.hash(&mut s);
            if !app.teacher_authed { app.teacher_pw.len().hash(&mut s); }
            if app.teacher_authed {
                // Roster length catches student add / remove.
                app.roster.students.len().hash(&mut s);
                // Selected student mutable fields: timer ([/]), lock (+/-), counts (S/R).
                if let Some(st) = app.roster.students.get(app.teacher_rec_index) {
                    st.challenge_secs.hash(&mut s);
                    st.reveal_lock.hash(&mut s);
                    st.grand_total().hash(&mut s);
                }
            }
        }
        Screen::Startup => {
            app.startup_index.hash(&mut s);
        }
        Screen::ChallengeEnd => {
            if let Some(ref r) = app.run { r.solved.hash(&mut s); }
        }
        Screen::Cinematic => {
            app.anim_frame.hash(&mut s);
        }
        Screen::Experiment => {
            app.exp_category.hash(&mut s);
            app.exp_amount.hash(&mut s);
            app.exp_from.hash(&mut s);
            app.exp_to.hash(&mut s);
            app.exp_field.hash(&mut s);
            app.anim_frame.hash(&mut s); // duck reaction animates
        }
        Screen::Time => {
            std::mem::discriminant(&app.config.hour_format).hash(&mut s);
            app.time_auto.hash(&mut s);
            app.time_year.hash(&mut s);
            app.time_month.hash(&mut s);
            app.time_day.hash(&mut s);
            app.time_hour.hash(&mut s);
            app.time_min.hash(&mut s);
            app.time_sec.hash(&mut s);
            app.time_field.hash(&mut s);
        }
    }

    s.finish()
}

// ---------------------------------------------------------------------------
// Rendering helpers
// ---------------------------------------------------------------------------

/// Box-downsample a supersampled Pixmap to output (w × h) pixels.
fn downsample(pm: &tiny_skia::Pixmap, w: u32, h: u32) -> Vec<u32> {
    let ss     = crate::scene::SS;
    let stride = w * ss;
    let n      = (ss * ss).max(1);
    let pixels = pm.pixels();
    let mut out = vec![0u32; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
            for dy in 0..ss {
                let row = ((y * ss + dy) * stride) as usize;
                for dx in 0..ss {
                    let p = pixels[row + (x * ss + dx) as usize];
                    r += p.red()   as u32;
                    g += p.green() as u32;
                    b += p.blue()  as u32;
                }
            }
            out[(y * w + x) as usize] = ((r / n) << 16) | ((g / n) << 8) | (b / n);
        }
    }
    out
}

/// Render the challenge HUD (timer + solved count) into a transparent overlay.
fn render_hud(w: u32, h: u32, remaining: u64, total: u64, solved: u32) -> Vec<u32> {
    let sw = w * crate::scene::SS;
    let sh = h * crate::scene::SS;
    let mut pm = tiny_skia::Pixmap::new(sw, sh)
        .unwrap_or_else(|| tiny_skia::Pixmap::new(1, 1).unwrap());

    let wf   = w as f32;
    let frac = (remaining as f32 / total.max(1) as f32).clamp(0.0, 1.0);
    let color = if frac > 0.5 { crate::scene::ACCENT }
                else if frac > 0.25 { crate::scene::YELLOW }
                else { crate::scene::RED };

    let mins = remaining / 60;
    let secs = remaining % 60;
    let time_str = if mins > 0 { format!("{}:{:02}", mins, secs) } else { format!("{}s", secs) };
    crate::scene::text_centered(
        &mut pm, wf / 2.0, 14.0, 2.0,
        &format!("{}    Solved: {}", time_str, solved),
        color,
    );

    let card_w = (wf * 0.86).min(1020.0);
    let bx = (wf - card_w) / 2.0;
    let by = 14.0 + 2.0 * crate::scene::PX_PER_SCALE + 6.0;
    crate::scene::fill(&mut pm, bx, by, card_w,        6.0, crate::scene::GRAY);
    crate::scene::fill(&mut pm, bx, by, card_w * frac, 6.0, color);

    downsample(&pm, w, h)
}

// ---------------------------------------------------------------------------
// Suppress dead-code warnings for Feedback variants only used in hash
// ---------------------------------------------------------------------------
#[allow(dead_code)]
fn _use_feedback(f: Feedback) -> u8 {
    match f { Feedback::None => 0, Feedback::Correct => 1, Feedback::Wrong => 2 }
}
