//! CPU-rendered native window frontend (Phase 2) — `winit` + `softbuffer` +
//! `tiny-skia`, **no GPU**.
//!
//! It reads the same frontend-agnostic [`App`] the terminal does, maps `winit`
//! keyboard events to [`InputEvent`], drives the per-tick clock, and paints each
//! frame into a CPU pixel buffer (tiny-skia) that softbuffer presents to the
//! window.  This is the *pipeline* slice: window, input, tick, present, and a
//! first tiny-skia frame.  Porting the individual screens to pixels comes next.

mod scene;

use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::Result;
use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key as WKey, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

use crate::app::App;
use crate::input::{InputEvent, Key, Mods};

/// Per-frame interval — matches the terminal's ~30 fps tick.
const TICK: Duration = Duration::from_millis(33);

/// Run the app in a native CPU-rendered window until it quits.
pub fn run(app: App) -> Result<()> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::wait_duration(TICK));
    let mut gui = Gui::new(app);
    event_loop.run_app(&mut gui)?;
    Ok(())
}

struct Gui {
    app: App,
    window: Option<Rc<Window>>,
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,
    mods: ModifiersState,
    last_tick: Instant,
}

impl Gui {
    fn new(app: App) -> Self {
        Gui { app, window: None, surface: None, mods: ModifiersState::empty(), last_tick: Instant::now() }
    }

    fn redraw(&mut self) {
        let (Some(window), Some(surface)) = (&self.window, &mut self.surface) else { return };
        let size = window.inner_size();
        let (w, h) = (size.width.max(1), size.height.max(1));
        let (Some(nw), Some(nh)) = (NonZeroU32::new(w), NonZeroU32::new(h)) else { return };
        if surface.resize(nw, nh).is_err() {
            return;
        }

        // The scene renders at `SS×` resolution; box-average each SS×SS block
        // down to one output pixel.  That supersample is our anti-aliasing —
        // smooth edges without tiny-skia's (panicky) AA rasteriser.
        let pixmap = scene::render(&self.app, w, h);
        let Ok(mut buffer) = surface.buffer_mut() else { return };
        let ss = scene::SS;
        let sw = w * ss; // supersampled row stride
        let n = (ss * ss).max(1);
        let pixels = pixmap.pixels();
        // tiny-skia premultiplied RGBA → softbuffer 0x00RRGGBB (frames are opaque).
        for y in 0..h {
            for x in 0..w {
                let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
                for dy in 0..ss {
                    let row = ((y * ss + dy) * sw) as usize;
                    for dx in 0..ss {
                        let p = pixels[row + (x * ss + dx) as usize];
                        r += p.red() as u32;
                        g += p.green() as u32;
                        b += p.blue() as u32;
                    }
                }
                buffer[(y * w + x) as usize] = ((r / n) << 16) | ((g / n) << 8) | (b / n);
            }
        }
        let _ = buffer.present();
    }
}

impl ApplicationHandler for Gui {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let attrs = Window::default_attributes()
            .with_title("Rusty Math Tutor")
            .with_inner_size(winit::dpi::LogicalSize::new(960.0, 680.0));
        let Ok(window) = event_loop.create_window(attrs) else { return };
        let window = Rc::new(window);
        let Ok(context) = Context::new(window.clone()) else { return };
        let Ok(surface) = Surface::new(&context, window.clone()) else { return };
        self.window = Some(window);
        self.surface = Some(surface);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    if let Some(ev) = to_input(&event, self.mods) {
                        self.app.on_event(ev);
                    }
                    if self.app.should_quit {
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
        // Persist progress / settings on the way out.
        self.app.persist();
    }
}

/// Map a `winit` key press into a frontend-neutral [`InputEvent`].
fn to_input(event: &KeyEvent, mods: ModifiersState) -> Option<InputEvent> {
    let key = match &event.logical_key {
        WKey::Named(NamedKey::Enter) => Key::Enter,
        WKey::Named(NamedKey::Escape) => Key::Esc,
        WKey::Named(NamedKey::Backspace) => Key::Backspace,
        WKey::Named(NamedKey::Tab) => Key::Tab,
        WKey::Named(NamedKey::ArrowUp) => Key::Up,
        WKey::Named(NamedKey::ArrowDown) => Key::Down,
        WKey::Named(NamedKey::ArrowLeft) => Key::Left,
        WKey::Named(NamedKey::ArrowRight) => Key::Right,
        WKey::Named(NamedKey::Space) => Key::Char(' '),
        WKey::Character(s) => Key::Char(s.chars().next()?),
        _ => return None,
    };
    Some(InputEvent::Key { key, mods: Mods { ctrl: mods.control_key() } })
}
