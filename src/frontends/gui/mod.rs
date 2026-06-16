//! CPU-rendered native window frontend (Phase 2) — `winit` + `softbuffer` +
//! `tiny-skia`, **no GPU**.
//!
//! It reads the same frontend-agnostic [`App`] the terminal does, maps `winit`
//! keyboard events to [`InputEvent`], drives the per-tick clock, and paints each
//! frame into a CPU pixel buffer (tiny-skia) that softbuffer presents to the
//! window.  This is the *pipeline* slice: window, input, tick, present, and a
//! first tiny-skia frame.  Porting the individual screens to pixels comes next.

mod scene;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::Result;
use softbuffer::{Context, Surface};
use tiny_skia::Pixmap;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key as WKey, ModifiersState, NamedKey};
use winit::window::{Fullscreen, Window, WindowId};

use crate::app::{App, Feedback, Screen};
use crate::input::{InputEvent, Key, Mods};

/// Per-frame interval — matches the terminal's ~30 fps tick.
const TICK: Duration = Duration::from_millis(33);

/// Run the app in a native CPU-rendered window until it quits or the user
/// switches back to console graphics.  Returns `true` when a switch is pending.
pub fn run(app: &mut App) -> Result<bool> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::wait_duration(TICK));
    let mut gui = Gui::new(app);
    event_loop.run_app(&mut gui)?;
    Ok(gui.switch)
}

struct Gui<'a> {
    app: &'a mut App,
    window: Option<Rc<Window>>,
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,
    mods: ModifiersState,
    last_tick: Instant,
    /// Set when the user picked console graphics — hand back to the terminal.
    switch: bool,
    /// Both card frames for the current transition, pre-downsampled to output
    /// resolution so the per-tick blend operates on w×h pixels, not w·SS × h·SS.
    transition_frames: Option<(Vec<u32>, Vec<u32>)>,
    /// Downsampled output pixels from the last render.  When the visual state
    /// hasn't changed we blit this directly and skip the full render + downsample.
    last_output: Vec<u32>,
    /// Hash of the visual state that produced `last_output`.
    last_hash: u64,
    /// True once the first frame has been presented.  Key events arriving before
    /// then are ignored — this absorbs any stale Enter/Space that was held in the
    /// OS event queue when the terminal handed off to the GUI window.
    ready: bool,
    /// Cached challenge HUD overlay (time bar + solved count) at output resolution.
    /// Keyed on (sw, sh, remaining_secs, solved, in_chalk) and blitted over every
    /// frame — blend or normal — so the timer never disappears during transitions.
    hud_cache: Option<(u32, u32, u64, u32, bool, Vec<u32>)>,
}

impl<'a> Gui<'a> {
    fn new(app: &'a mut App) -> Self {
        Gui {
            app,
            window: None,
            surface: None,
            mods: ModifiersState::empty(),
            last_tick: Instant::now(),
            switch: false,
            transition_frames: None,
            last_output: Vec::new(),
            last_hash: 0,
            ready: false,
            hud_cache: None,
        }
    }

    fn redraw(&mut self) {
        let (Some(window), Some(surface)) = (&self.window, &mut self.surface) else { return };
        let size = window.inner_size();
        let (w, h) = (size.width.max(1), size.height.max(1));
        let (Some(nw), Some(nh)) = (NonZeroU32::new(w), NonZeroU32::new(h)) else { return };
        if surface.resize(nw, nh).is_err() { return; }

        scene::set_theme(self.app.config.theme);

        // Check whether the visual state changed since the last frame.
        // If not, skip the expensive render+downsample and just blit the cache.
        let new_hash = visual_hash(self.app, w, h);
        let need_render = new_hash != self.last_hash
            || self.last_output.len() != (w * h) as usize;

        if need_render {
            let (sw, sh) = (w * scene::SS, h * scene::SS);

            // Transition: capture both cards once through the same pipeline as
            // render() — including chalk inset, chalk_texture, and board frame —
            // then downsample.  Blending pre-downsampled frames costs 9× fewer
            // pixels per tick than blending at SS resolution.
            if self.app.transition.is_some() {
                let stale = self.transition_frames.as_ref()
                    .map_or(true, |(v, _)| v.len() != (w * h) as usize);
                if stale {
                    let in_chalk = scene::is_chalk()
                        && w > 2 * scene::FRAME_T
                        && h > scene::FRAME_T + scene::FRAME_TRAY;
                    let from_out = capture_frame(in_chalk, w, h, sw, sh, |p, cwf, chf| {
                        scene::draw_card(
                            p, &self.app.current, &self.app.input,
                            Feedback::Correct, 1.0, self.app.anim_frame,
                            self.app.config.layout, cwf, chf,
                        );
                    });
                    let to_out = capture_frame(in_chalk, w, h, sw, sh, |p, cwf, chf| {
                        if let Some(next) = &self.app.pending {
                            scene::draw_card(
                                p, next, "",
                                Feedback::None, 0.0, self.app.anim_frame,
                                self.app.config.layout, cwf, chf,
                            );
                        }
                    });
                    self.transition_frames = Some((from_out, to_out));
                }
            } else {
                self.transition_frames = None;
            }

            if let (Some(phase), Some((from_out, to_out))) =
                (&self.app.transition, &self.transition_frames)
            {
                // Blend pre-downsampled frames at output (w×h) resolution.
                self.last_output.resize((w * h) as usize, 0);
                scene::blend_output(
                    &mut self.last_output, from_out, to_out,
                    phase.effect,
                    phase.progress.clamp(0.0, 1.0),
                    w, h,
                );
            } else {
                // Normal render — full SS render then downsample.
                let pixmap = scene::render(self.app, w, h);
                self.last_output = downsample(&pixmap, w, h);
            }
            // Challenge HUD overlay — time + progress bar + solved count.
            // Applied here (inside need_render) so it is baked into last_output
            // after every fresh render/blend.  On subsequent frames where nothing
            // changed the hash is stable, need_render is false, and the cached
            // last_output (which already contains the HUD) is presented directly.
            // remaining_secs() is frozen during transitions via pause/resume, so
            // the overlay doesn't change during the blend — no extra renders.
            {
                use crate::app::Screen;
                let is_session = matches!(self.app.screen, Screen::Practice | Screen::Challenge);
                if is_session {
                    if let Some(ref chg) = self.app.challenge {
                        let remaining = chg.remaining_secs();
                        let total     = chg.duration_secs();
                        let solved    = chg.solved;
                        let in_chalk  = scene::is_chalk()
                            && w > 2 * scene::FRAME_T
                            && h > scene::FRAME_T + scene::FRAME_TRAY;

                        let stale = self.hud_cache.as_ref().map_or(true, |(csw, csh, cr, cs, cic, _)| {
                            *csw != sw || *csh != sh || *cr != remaining || *cs != solved || *cic != in_chalk
                        });
                        if stale {
                            let hud = render_challenge_hud(w, h, sw, sh, remaining, total, solved, in_chalk);
                            self.hud_cache = Some((sw, sh, remaining, solved, in_chalk, hud));
                        }
                        if let Some((_, _, _, _, _, ref hud)) = self.hud_cache {
                            for (out, &hp) in self.last_output.iter_mut().zip(hud.iter()) {
                                if hp != 0 { *out = hp; }
                            }
                        }
                    } else {
                        self.hud_cache = None;
                    }
                } else {
                    self.hud_cache = None;
                }
            }

            self.last_hash = new_hash;
        }

        // Present — always needed (softbuffer owns the surface buffer each frame).
        let Ok(mut buffer) = surface.buffer_mut() else { return };
        buffer.copy_from_slice(&self.last_output);
        let _ = buffer.present();
        // Gate key input until after the first frame is on screen, absorbing any
        // stale Enter/Space held in the OS queue from the terminal hand-off.
        self.ready = true;
    }
}

/// Cheap fingerprint of all app state that affects what gets rendered.
/// If this matches the previous frame's hash we can skip rendering entirely.
fn visual_hash(app: &App, w: u32, h: u32) -> u64 {
    let mut s = DefaultHasher::new();

    // Window dimensions — a resize always forces a full re-render.
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
            // Duck ease: quantise to 64 steps so minor float noise doesn't retrigger.
            ((app.help_in * 64.0) as u8).hash(&mut s);
            app.why_active.hash(&mut s);
            app.why_scroll.hash(&mut s);
            app.frac_anim.hash(&mut s);
            app.card_anim.hash(&mut s);
            app.revealed.hash(&mut s);
            app.encourage.hash(&mut s);
            app.reprimand.hash(&mut s);
            // Duck waddle only matters when the panel is visible.
            if app.help_in > 0.01 { app.anim_frame.hash(&mut s); }
            // Challenge countdown — 1-second granularity is enough for the display.
            if let Some(ref c) = app.challenge { c.remaining_secs().hash(&mut s); }
            // Transition progress — changes every tick during a transition.
            if let Some(ref t) = app.transition {
                ((t.progress * 128.0) as u8).hash(&mut s);
            }
        }
        Screen::Menu => {
            app.menu_index.hash(&mut s);
            app.naming.hash(&mut s);
            app.name_input.hash(&mut s);
            app.roster.current.hash(&mut s);
            app.anim_frame.hash(&mut s); // duck waddle in the margin
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
            // Static once shown; hash solved count so a fresh run forces redraw.
            if let Some(ref r) = app.run { r.solved.hash(&mut s); }
        }
        Screen::Cinematic => {
            // Cinematics animate every tick.
            app.anim_frame.hash(&mut s);
        }
        Screen::Experiment => {
            app.exp_category.hash(&mut s);
            app.exp_amount.hash(&mut s);
            app.exp_from.hash(&mut s);
            app.exp_to.hash(&mut s);
            app.exp_field.hash(&mut s);
        }
        Screen::Help => {} // static content — frame hash is stable
        Screen::GraphExplorer => {
            app.graph_exp_kind.hash(&mut s);
            app.graph_exp_values.hash(&mut s);
            app.graph_exp_field.hash(&mut s);
            app.graph_exp_input.hash(&mut s);
        }
        Screen::Time => {
            std::mem::discriminant(&app.config.hour_format).hash(&mut s);
            app.time_help_active.hash(&mut s);
            app.time_auto.hash(&mut s);
            app.time_year.hash(&mut s);
            app.time_month.hash(&mut s);
            app.time_day.hash(&mut s);
            app.time_hour.hash(&mut s);
            app.time_min.hash(&mut s);
            app.time_sec.hash(&mut s);  // drives re-render every second in auto mode
            app.time_field.hash(&mut s);
        }
    }

    s.finish()
}

/// Capture one transition card frame through the same pipeline that `render()`
/// uses, so the captured pixel data matches normal rendering exactly.
///
/// In chalk mode: render into the inset area, apply `chalk_texture`, blit into
/// the full frame, and add the board frame — then downsample.
/// In other modes: render at full window size and downsample.
fn capture_frame(
    in_chalk: bool,
    w: u32, h: u32,
    sw: u32, sh: u32,
    draw: impl FnOnce(&mut Pixmap, f32, f32),
) -> Vec<u32> {
    if in_chalk {
        let ft   = scene::FRAME_T;
        let ftry = scene::FRAME_TRAY;
        let iw   = w - 2 * ft;
        let ih   = h - ft - ftry;

        // Render at inner (inset) dimensions — same as render() in chalk mode.
        let mut inner = scene::card_frame(iw * scene::SS, ih * scene::SS,
            |p| draw(p, iw as f32, ih as f32));
        scene::chalk_texture(&mut inner);

        // Blit into a full-size pixmap and paint the wooden frame.
        let mut full = Pixmap::new(sw, sh)
            .unwrap_or_else(|| Pixmap::new(1, 1).unwrap());
        full.fill(scene::col(scene::BG));
        blit_pixmap(&mut full, &inner, ft * scene::SS, ft * scene::SS);
        scene::draw_board_frame(&mut full);

        downsample(&full, w, h)
    } else {
        let pm = scene::card_frame(sw, sh, |p| draw(p, w as f32, h as f32));
        downsample(&pm, w, h)
    }
}

/// Copy `src` into `dst` at pixel offset `(ox, oy)` — equivalent to the
/// private `blit` in `render.rs`.
fn blit_pixmap(dst: &mut Pixmap, src: &Pixmap, ox: u32, oy: u32) {
    let (dw, dh) = (dst.width(), dst.height());
    let (sw, sh) = (src.width(), src.height());
    let spx = src.pixels();
    let dpx = dst.pixels_mut();
    for y in 0..sh {
        let dy = oy + y;
        if dy >= dh { break; }
        let drow = (dy * dw) as usize;
        let srow = (y  * sw) as usize;
        for x in 0..sw {
            let dx = ox + x;
            if dx >= dw { break; }
            dpx[drow + dx as usize] = spx[srow + x as usize];
        }
    }
}

/// Render the challenge HUD (time text + progress bar + solved count) into a
/// transparent overlay at output resolution.  Non-zero pixels are blitted over
/// `last_output`; zero pixels (transparent background) are left untouched.
///
/// Placed above the card in the top margin, matching the console HUD layout.
fn render_challenge_hud(
    w: u32, h: u32, sw: u32, sh: u32,
    remaining: u64, total: u64, solved: u32,
    in_chalk: bool,
) -> Vec<u32> {
    let mut pm = Pixmap::new(sw, sh)
        .unwrap_or_else(|| Pixmap::new(1, 1).unwrap());
    // No fill — all pixels start transparent (zero); only drawn pixels are non-zero.

    let wf  = w as f32;
    let ft  = scene::FRAME_T as f32;
    // Position in the margin above the card.  In chalk mode offset by FRAME_T
    // so the HUD lands inside the board area, not in the wooden frame.
    let top = if in_chalk { ft + 14.0 } else { 14.0 };

    let frac  = (remaining as f32 / total.max(1) as f32).clamp(0.0, 1.0);
    let color = if frac > 0.5 { scene::ACCENT } else if frac > 0.25 { scene::YELLOW } else { scene::RED };

    let mins = remaining / 60;
    let secs = remaining % 60;
    let time_str = if mins > 0 { format!("{}:{:02}", mins, secs) } else { format!("{}s", secs) };
    let label    = format!("{}    Solved: {}", time_str, solved);

    scene::text_centered(&mut pm, wf / 2.0, top, 2.0, &label, color);

    // Progress bar aligned with the card width, just below the text.
    let card_w = (wf * 0.86).min(1020.0);
    let bx     = (wf - card_w) / 2.0;
    let by     = top + 2.0 * scene::PX_PER_SCALE + 6.0;
    let bh     = 6.0;
    scene::fill(&mut pm, bx,             by, card_w,        bh, scene::GRAY);
    scene::fill(&mut pm, bx,             by, card_w * frac, bh, color);

    downsample(&pm, w, h)
}

/// Box-downsample an SS×-resolution pixmap to output (`w`×`h`) pixels.
/// Returns a `Vec<u32>` in `0x00RRGGBB` order suitable for softbuffer.
fn downsample(pm: &Pixmap, w: u32, h: u32) -> Vec<u32> {
    let ss     = scene::SS;
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

impl ApplicationHandler for Gui<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let attrs = Window::default_attributes()
            .with_title("Rusty Math Tutor")
            .with_active(true)
            .with_fullscreen(Some(Fullscreen::Borderless(None)));
        let Ok(window) = event_loop.create_window(attrs) else { return };
        let window = Rc::new(window);
        window.focus_window();
        let Ok(context) = Context::new(window.clone()) else { return };
        let Ok(surface) = Surface::new(&context, window.clone()) else { return };
        window.request_redraw();
        self.window = Some(window);
        self.surface = Some(surface);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::KeyboardInput { event, .. } => {
                if self.ready && event.state == ElementState::Pressed {
                    if let Some(ev) = to_input(&event, self.mods) {
                        self.app.on_event(ev);
                    }
                    if self.app.should_quit {
                        event_loop.exit();
                    } else if self.app.config.graphics != crate::config::GraphicsMode::Cpu {
                        self.switch = true;
                        event_loop.exit();
                    } else if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
            }
            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.last_tick.elapsed() >= TICK {
            self.app.on_tick();
            self.last_tick = Instant::now();
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.last_tick + TICK));
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.app.persist();
    }
}

/// Map a `winit` key press into a frontend-neutral [`InputEvent`].
fn to_input(event: &KeyEvent, mods: ModifiersState) -> Option<InputEvent> {
    let key = match &event.logical_key {
        WKey::Named(NamedKey::Enter)     => Key::Enter,
        WKey::Named(NamedKey::Escape)    => Key::Esc,
        WKey::Named(NamedKey::Backspace) => Key::Backspace,
        WKey::Named(NamedKey::Tab)       => Key::Tab,
        WKey::Named(NamedKey::ArrowUp)   => Key::Up,
        WKey::Named(NamedKey::ArrowDown) => Key::Down,
        WKey::Named(NamedKey::ArrowLeft) => Key::Left,
        WKey::Named(NamedKey::ArrowRight)=> Key::Right,
        WKey::Named(NamedKey::Space)     => Key::Char(' '),
        WKey::Character(s)               => Key::Char(s.chars().next()?),
        _                                => return None,
    };
    Some(InputEvent::Key { key, mods: Mods { ctrl: mods.control_key() } })
}
