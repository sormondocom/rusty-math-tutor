//! Top-level render entry point: sets the active theme, applies the chalk-board
//! frame when a chalk theme is selected, then dispatches to the active screen.

use tiny_skia::Pixmap;

use crate::app::{App, Screen};

use super::*;

/// Paint a full frame for the app's current screen, into a pixmap that is `SS`×
/// the logical `(w, h)`.  Drawing is in *logical* coordinates; the low-level
/// helpers in `paint` and `text` scale up by `SS` internally.  The window step
/// ([`crate::gui`]) box-downscales the result for anti-aliasing.
pub fn render(app: &App, w: u32, h: u32) -> Pixmap {
    set_theme(app.config.theme);
    let mut pm = Pixmap::new((w * SS).max(1), (h * SS).max(1)).unwrap_or_else(|| Pixmap::new(1, 1).unwrap());
    pm.fill(col(BG));

    if is_chalk() && w > 2 * FRAME_T && h > FRAME_T + FRAME_TRAY {
        // Render the scene into the inset board area, texture it as chalk, blit
        // it inside the wooden frame, then paint the frame in the margins.
        let (iw, ih) = (w - 2 * FRAME_T, h - FRAME_T - FRAME_TRAY);
        let mut inner = Pixmap::new(iw * SS, ih * SS).unwrap_or_else(|| Pixmap::new(1, 1).unwrap());
        inner.fill(col(BG));
        draw_screen(&mut inner, app, iw as f32, ih as f32);
        chalk_texture(&mut inner);
        blit(&mut pm, &inner, FRAME_T * SS, FRAME_T * SS);
        draw_board_frame(&mut pm);
    } else {
        draw_screen(&mut pm, app, w as f32, h as f32);
    }
    pm
}

fn draw_screen(pm: &mut Pixmap, app: &App, wf: f32, hf: f32) {
    match app.screen {
        Screen::Startup                      => draw_startup(pm, app, wf, hf),
        Screen::Menu                         => draw_menu(pm, app, wf, hf),
        Screen::Practice | Screen::Challenge => draw_session(pm, app, wf, hf),
        Screen::ChallengeEnd                 => draw_challenge_end(pm, app, wf, hf),
        Screen::Settings                     => draw_settings(pm, app, wf, hf),
        Screen::Stats                        => draw_stats(pm, app, wf, hf),
        Screen::Teacher                      => draw_teacher(pm, app, wf, hf),
        Screen::Cinematic                    => draw_cinematic(pm, app, wf, hf),
        Screen::Experiment                   => draw_experiment(pm, app, wf, hf),
        Screen::Time                         => draw_time(pm, app, wf, hf),
        Screen::Help                         => draw_key_guide(pm, wf, hf),
        Screen::GraphExplorer                => draw_graph_explorer(pm, app, wf, hf),
    }
}

fn blit(dst: &mut Pixmap, src: &Pixmap, ox: u32, oy: u32) {
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
