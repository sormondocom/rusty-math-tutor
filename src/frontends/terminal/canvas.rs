//! A half-block (`▀ ▄ █`) raster surface for the terminal frontend.
//!
//! Each terminal cell holds two vertically-stacked "pixels" (top and bottom),
//! coloured independently via the cell's foreground/background.  That doubles
//! the vertical resolution — and since a character cell is about twice as tall
//! as it is wide, the pixels come out roughly **square**, so circles drawn here
//! look round and diagonals come out smooth.
//!
//! It's a plain CPU raster (no GPU, no new dependencies) used for the smooth
//! Geometry shapes, and reusable for any sub-cell terminal drawing.

use ratatui::buffer::Buffer;
use ratatui::style::Color;

/// A grid of sub-cell pixels.  `rows` is in pixels (2 per character cell).
pub struct Canvas {
    cols: u16,
    rows: u16,
    px: Vec<Option<Color>>,
}

impl Canvas {
    /// A canvas `cols` wide and `cell_rows` character-rows tall (so `2*cell_rows`
    /// pixels tall).
    pub fn new(cols: u16, cell_rows: u16) -> Self {
        let cols = cols.max(1);
        let rows = cell_rows.max(1).saturating_mul(2);
        Canvas { cols, rows, px: vec![None; cols as usize * rows as usize] }
    }

    pub fn cols(&self) -> u16 {
        self.cols
    }

    pub fn rows(&self) -> u16 {
        self.rows
    }

    /// Light a pixel (out-of-bounds is ignored).
    pub fn set(&mut self, x: i32, y: i32, color: Color) {
        if x >= 0 && y >= 0 && (x as u16) < self.cols && (y as u16) < self.rows {
            self.px[y as usize * self.cols as usize + x as usize] = Some(color);
        }
    }

    fn get(&self, x: u16, y: u16) -> Option<Color> {
        self.px[y as usize * self.cols as usize + x as usize]
    }

    /// A straight line between two pixels (Bresenham).
    pub fn line(&mut self, mut x0: i32, mut y0: i32, x1: i32, y1: i32, color: Color) {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.set(x0, y0, color);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    /// A circle outline centred at `(cx, cy)` (midpoint algorithm; square pixels
    /// keep it round).
    pub fn circle(&mut self, cx: i32, cy: i32, radius: i32, color: Color) {
        let mut x = radius;
        let mut y = 0;
        let mut err = 1 - radius;
        while x >= y {
            for (px, py) in [(x, y), (y, x), (-x, y), (-y, x), (-x, -y), (-y, -x), (x, -y), (y, -x)] {
                self.set(cx + px, cy + py, color);
            }
            y += 1;
            if err < 0 {
                err += 2 * y + 1;
            } else {
                x -= 1;
                err += 2 * (y - x) + 1;
            }
        }
    }

    /// Paint the canvas into `buf`, top-left at cell `(ox, oy)`.  Empty cells are
    /// left untouched, so whatever was already there (the card background) shows
    /// through.
    pub fn blit(&self, buf: &mut Buffer, ox: u16, oy: u16) {
        let area = buf.area;
        for cy in 0..self.rows / 2 {
            for cx in 0..self.cols {
                let (ch, fg, bg) = match (self.get(cx, cy * 2), self.get(cx, cy * 2 + 1)) {
                    (None, None) => continue, // leave the background be
                    (Some(t), None) => ('▀', t, None),
                    (None, Some(b)) => ('▄', b, None),
                    (Some(t), Some(b)) if t == b => ('█', t, None),
                    (Some(t), Some(b)) => ('▀', t, Some(b)), // top fg, bottom bg
                };
                let (x, y) = (ox + cx, oy + cy);
                if x < area.left() || x >= area.right() || y < area.top() || y >= area.bottom() {
                    continue;
                }
                let cell = &mut buf[(x, y)];
                cell.set_char(ch).set_fg(fg);
                if let Some(bg) = bg {
                    cell.set_bg(bg);
                }
            }
        }
    }
}
