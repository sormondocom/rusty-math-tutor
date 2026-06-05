//! The chalk-theme blackboard frame: wooden borders, a chalk tray, and the
//! chalk-texture post-process pass that makes smooth renders look hand-drawn.

use tiny_skia::{Pixmap, PremultipliedColorU8};

use super::*;

/// Frame insets (logical px) for chalk themes: a wooden border, with a taller
/// ledge along the bottom for the chalk tray.
pub const FRAME_T: u32 = 22;
pub const FRAME_TRAY: u32 = 40;

/// Paint the wooden blackboard frame (and its chalk tray) in `pm`'s margins.
pub fn draw_board_frame(pm: &mut Pixmap) {
    let (w, h) = (pm.width() as i32, pm.height() as i32);
    let t    = (FRAME_T    * SS) as i32;
    let tray = (FRAME_TRAY * SS) as i32;
    let wood    = [120u8, 80,  44];
    let wood_lo = [84u8,  54,  28];
    let wood_hi = [158u8, 112, 66];
    let bevel = (3.0 * SSF) as i32;

    fill_raw(pm, 0,     0,     w,     t,    wood);
    fill_raw(pm, 0,     0,     t,     h,    wood);
    fill_raw(pm, w - t, 0,     t,     h,    wood);
    fill_raw(pm, 0,     h - tray, w,  tray, wood);

    for x in (0..w).step_by((7.0 * SSF) as usize + 1) {
        if hash01(x as u32, 3) > 0.6 {
            fill_raw(pm, x, 0,        bevel.max(1), t,    wood_lo);
            fill_raw(pm, x, h - tray, bevel.max(1), tray, wood_lo);
        }
    }
    for y in (0..h).step_by((7.0 * SSF) as usize + 1) {
        if hash01(5, y as u32) > 0.6 {
            fill_raw(pm, 0,     y, t, bevel.max(1), wood_lo);
            fill_raw(pm, w - t, y, t, bevel.max(1), wood_lo);
        }
    }

    fill_raw(pm, 0,            0,     w,                      bevel, wood_hi);
    fill_raw(pm, 0,            0,     bevel,                  h,     wood_hi);
    fill_raw(pm, w - bevel,    0,     bevel,                  h,     wood_lo);
    fill_raw(pm, t - bevel,    t,     w - 2*t + 2*bevel,     bevel, wood_lo);
    fill_raw(pm, t - bevel,    h - tray, bevel,               tray,  wood_hi);
    fill_raw(pm, w - t,        h - tray, bevel,               tray,  wood_lo);
    fill_raw(pm, t,            h - tray, w - 2*t,             bevel, wood_hi);

    let ty = h - tray + (10.0 * SSF) as i32;
    // Chalk stick
    fill_raw(pm, w/2 - (210.0*SSF) as i32, ty + (4.0*SSF) as i32, (96.0*SSF) as i32, (9.0*SSF) as i32, [242, 240, 230]);
    // Felt eraser
    let er_x = w/2 + (120.0*SSF) as i32;
    fill_raw(pm, er_x, ty, (74.0*SSF) as i32, (18.0*SSF) as i32, [120, 120, 128]);
    fill_raw(pm, er_x, ty, (74.0*SSF) as i32, (6.0 *SSF) as i32, [150, 110,  70]);
}

/// Fill an opaque device-pixel rect with a *raw* (un-themed) colour, clipped.
/// Used by the board frame which must not be chalk-tinted.
pub fn fill_raw(pm: &mut Pixmap, x: i32, y: i32, w: i32, h: i32, rgb: Rgb) {
    let (pw, ph) = (pm.width() as i32, pm.height() as i32);
    let (x0, y0) = (x.max(0), y.max(0));
    let (x1, y1) = ((x + w).min(pw), (y + h).min(ph));
    if x1 <= x0 || y1 <= y0 { return; }
    let px = prem(rgb);
    let pwu = pm.width();
    let data = pm.pixels_mut();
    for yy in y0..y1 {
        let row = (yy as u32 * pwu) as usize;
        for xx in x0..x1 {
            data[row + xx as usize] = px;
        }
    }
}

/// Post-process the frame to make chalk marks look genuinely chalky: grainy
/// strokes, dusty gaps, and occasional bright flecks.  Applied only to
/// foreground pixels (the board itself is left alone).
pub fn chalk_texture(pm: &mut Pixmap) {
    let board = themed(BG);
    let board_luma = luma(board);
    let (w, h) = (pm.width(), pm.height());
    let data = pm.pixels_mut();
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            let p = data[i];
            let c = [p.red(), p.green(), p.blue()];
            if luma(c) - board_luma < 14.0 { continue; }
            let grain = 0.84 + 0.16 * hash01(x / 3, y / 3);
            let speck = hash01(x.wrapping_add(7), y.wrapping_mul(3).wrapping_add(13));
            let out = if speck < 0.025 {
                board
            } else if speck > 0.985 {
                lerp_rgb(c, CHALK_WHITE, 0.6)
            } else {
                lerp_rgb(board, c, grain)
            };
            if let Some(px) = PremultipliedColorU8::from_rgba(out[0], out[1], out[2], 255) {
                data[i] = px;
            }
        }
    }
}
