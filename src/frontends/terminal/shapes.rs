//! Reusable ASCII shape glyphs.
//!
//! A [`Shape`] is a figure divided into equal regions that can be laid out and
//! drawn into a terminal buffer, with each region individually coloured and a
//! `progress` value that makes the regions **materialise** one at a time.
//!
//! Fractions are the first user (a square split into pieces, some shaded), but
//! the module is deliberately general so later geometry work — counting unit
//! squares for area, shading parts, identifying figures — can reuse the same
//! glyphs.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// Faint colour for a region that hasn't materialised yet.
const SLOT: Color = Color::Rgb(60, 62, 78);
/// Dark line drawn between regions so the pieces are clearly separated.
const SEP: Color = Color::Rgb(16, 18, 28);

/// A figure divided into equal regions.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Shape {
    /// A single row of `segments` cells (a bar / divided rectangle).
    Bar { segments: u16 },
    /// A `rows` × `cols` grid of cells.
    Grid { rows: u16, cols: u16 },
    /// A pie circle cut into `slices` equal wedges.
    Circle { slices: u16 },
    /// An upward triangle cut into `strips` horizontal layers.
    Triangle { strips: u16 },
}

impl Shape {
    /// Total number of regions.
    pub fn regions(&self) -> u16 {
        match self {
            Shape::Bar { segments } => *segments,
            Shape::Grid { rows, cols } => rows * cols,
            Shape::Circle { slices } => *slices,
            Shape::Triangle { strips } => *strips,
        }
    }

    /// Whether this shape reads as a square (for nicer wording).
    pub fn is_square(&self) -> bool {
        matches!(self, Shape::Grid { rows, cols } if rows == cols)
    }

    /// The cell rectangle for a rectangular shape's region `i` (centred), or
    /// `None` for the curved/angular shapes that fill per cell.
    pub fn region_rect(&self, area: Rect, i: u16) -> Option<Rect> {
        if i >= self.regions() {
            return None;
        }
        const GAP: u16 = 1;
        match *self {
            Shape::Circle { .. } | Shape::Triangle { .. } => None,
            Shape::Bar { segments } => {
                let cw = (area.width.saturating_sub((segments - 1) * GAP) / segments).clamp(2, 10);
                let ch = area.height.clamp(2, 6);
                let total_w = segments * cw + (segments - 1) * GAP;
                let x0 = area.left() + area.width.saturating_sub(total_w) / 2;
                let y0 = area.top() + area.height.saturating_sub(ch) / 2;
                Some(Rect { x: x0 + i * (cw + GAP), y: y0, width: cw, height: ch })
            }
            Shape::Grid { rows, cols } => {
                let cw = (area.width.saturating_sub((cols - 1) * GAP) / cols).clamp(2, 10);
                let ch = (area.height.saturating_sub((rows - 1) * GAP) / rows).clamp(1, 4);
                let total_w = cols * cw + (cols - 1) * GAP;
                let total_h = rows * ch + (rows - 1) * GAP;
                let x0 = area.left() + area.width.saturating_sub(total_w) / 2;
                let y0 = area.top() + area.height.saturating_sub(total_h) / 2;
                let (row, col) = (i / cols, i % cols);
                Some(Rect { x: x0 + col * (cw + GAP), y: y0 + row * (ch + GAP), width: cw, height: ch })
            }
        }
    }
}

/// Draw `shape` into `area`, materialising region by region as `progress` rises
/// from 0 to 1.  `color_of(i)` supplies each region's fill colour.
pub fn render(buf: &mut Buffer, area: Rect, shape: Shape, progress: f32, color_of: impl Fn(u16) -> Color) {
    let n = shape.regions().max(1);
    match shape {
        // Rectangular shapes draw as separated cells.
        Shape::Bar { .. } | Shape::Grid { .. } => {
            for i in 0..shape.regions() {
                let Some(rect) = shape.region_rect(area, i) else { continue };
                let (sym, color) = region_glyph(i, n, progress, &color_of);
                fill(buf, rect, sym, color);
                if sym == '▒' {
                    put(buf, rect.x + rect.width / 2, rect.y + rect.height / 2, '✦', Color::White);
                }
            }
        }
        // Curved / angular shapes fill per cell, then get separator lines so the
        // individual pieces stay clearly visible even when the same colour.
        Shape::Circle { slices } => {
            render_field(buf, area, n, progress, &color_of, |a, x, y| circle_region(a, x, y, slices));
            draw_circle_separators(buf, area, slices);
        }
        Shape::Triangle { strips } => {
            render_field(buf, area, n, progress, &color_of, |a, x, y| triangle_region(a, x, y, strips));
            draw_triangle_separators(buf, area, strips);
        }
    }
}

/// Dark radial spokes between pie slices (and the rim), so each wedge is clear.
fn draw_circle_separators(buf: &mut Buffer, area: Rect, slices: u16) {
    if slices < 2 {
        return;
    }
    let cx = area.left() as f32 + area.width as f32 / 2.0;
    let cy = area.top() as f32 + area.height as f32 / 2.0;
    let radius = (area.width as f32 / 2.0).min(area.height as f32);
    for k in 0..slices {
        let theta = (k as f32 / slices as f32 - 0.5) * std::f32::consts::TAU;
        let mut rr = 0.0;
        while rr <= radius {
            let x = cx + theta.cos() * rr;
            let y = cy + theta.sin() * rr * 0.5; // cells are ~twice as tall
            put_f(buf, x, y, '·', SEP);
            rr += 0.5;
        }
    }
}

/// Dark horizontal lines between the triangle's strips.
fn draw_triangle_separators(buf: &mut Buffer, area: Rect, strips: u16) {
    if strips < 2 || area.height < 2 {
        return;
    }
    let cx = area.left() as f32 + area.width as f32 / 2.0;
    for k in 1..strips {
        let fy = k as f32 / strips as f32;
        let y = area.top() as f32 + fy * (area.height - 1) as f32;
        let half = fy * (area.width as f32 / 2.0);
        let mut x = cx - half;
        while x <= cx + half {
            put_f(buf, x, y, '·', SEP);
            x += 1.0;
        }
    }
}

/// Plot a char at floating-point coordinates, rounded and clipped.
fn put_f(buf: &mut Buffer, x: f32, y: f32, ch: char, color: Color) {
    if x.is_finite() && y.is_finite() && x >= 0.0 && y >= 0.0 {
        put(buf, x.round() as u16, y.round() as u16, ch, color);
    }
}

/// The glyph and colour for region `i` of `n` at the given `progress`.
fn region_glyph(i: u16, n: u16, progress: f32, color_of: &impl Fn(u16) -> Color) -> (char, Color) {
    let appear = (i + 1) as f32 / n as f32;
    let start = i as f32 / n as f32;
    if progress >= appear {
        ('█', color_of(i))
    } else if progress >= start {
        ('▒', color_of(i))
    } else {
        ('·', SLOT)
    }
}

/// Fill every cell inside the shape (per `region_fn`) with its materialise
/// glyph.  Cells outside the shape are left untouched.
fn render_field(buf: &mut Buffer, area: Rect, n: u16, progress: f32, color_of: &impl Fn(u16) -> Color, region_fn: impl Fn(Rect, u16, u16) -> Option<u16>) {
    let bounds = buf.area;
    for y in area.top()..area.bottom().min(bounds.bottom()) {
        for x in area.left()..area.right().min(bounds.right()) {
            if let Some(r) = region_fn(area, x, y) {
                let (sym, color) = region_glyph(r, n, progress, color_of);
                let mut tmp = [0u8; 4];
                buf[(x, y)].set_symbol(sym.encode_utf8(&mut tmp)).set_fg(color);
            }
        }
    }
}

/// Which pie slice (0..slices) the cell falls in, or `None` if outside the disk.
fn circle_region(area: Rect, x: u16, y: u16, slices: u16) -> Option<u16> {
    let cx = area.left() as f32 + area.width as f32 / 2.0;
    let cy = area.top() as f32 + area.height as f32 / 2.0;
    let radius = (area.width as f32 / 2.0).min(area.height as f32);
    if radius < 1.0 {
        return None;
    }
    let dx = x as f32 - cx;
    let dy = (y as f32 - cy) * 2.0; // cells are ~twice as tall: round it out
    if (dx * dx + dy * dy).sqrt() > radius {
        return None;
    }
    let ang = dy.atan2(dx) / std::f32::consts::TAU + 0.5; // 0.0..1.0
    Some(((ang * slices as f32) as u16).min(slices - 1))
}

/// Which horizontal strip the cell falls in for an upward triangle, or `None`.
fn triangle_region(area: Rect, x: u16, y: u16, strips: u16) -> Option<u16> {
    if area.height < 2 {
        return None;
    }
    let fy = (y - area.top()) as f32 / (area.height - 1) as f32; // 0 apex .. 1 base
    let cx = area.left() as f32 + area.width as f32 / 2.0;
    let half = fy * (area.width as f32 / 2.0);
    if (x as f32 - cx).abs() > half + 0.5 {
        return None;
    }
    Some(((fy * strips as f32) as u16).min(strips - 1))
}

fn fill(buf: &mut Buffer, rect: Rect, ch: char, color: Color) {
    let area = buf.area;
    let mut tmp = [0u8; 4];
    let s = ch.encode_utf8(&mut tmp);
    for y in rect.top()..rect.bottom().min(area.bottom()) {
        for x in rect.left()..rect.right().min(area.right()) {
            buf[(x, y)].set_symbol(s).set_fg(color);
        }
    }
}

fn put(buf: &mut Buffer, x: u16, y: u16, ch: char, color: Color) {
    let area = buf.area;
    if x >= area.left() && x < area.right() && y >= area.top() && y < area.bottom() {
        let mut tmp = [0u8; 4];
        buf[(x, y)].set_symbol(ch.encode_utf8(&mut tmp)).set_fg(color);
    }
}
