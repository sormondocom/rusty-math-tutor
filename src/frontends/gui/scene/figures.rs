//! The per-section problem *figures* drawn inside the card: the three arithmetic
//! layouts (horizontal / stacked / long-division house), the fraction &
//! percentage shapes (filled regions that materialise piece by piece, or chalk
//! outlines), the geometry shapes, and the themed Units-of-Measure prop.

use tiny_skia::Pixmap;

use crate::app::Feedback;
use crate::fraction::FractionProblem;
use crate::geometry::{GeometryProblem, GeoShape};
use crate::shapes::Shape;
use crate::units::Theme;

use super::*;

// -- Arithmetic — the three layouts -----------------------------------------

pub fn draw_arith(pm: &mut Pixmap, (cx0, cy0, cw, ch): (f32, f32, f32, f32), p: &crate::problem::Problem, input: &str, layout: crate::config::Layout, feedback: Feedback) {
    use crate::config::Layout;
    use crate::problem::Op;
    let accent = core_col(p.accent);
    let acol = ans_color(input, feedback);
    let cyc = cy0 + ch * 0.46;
    match layout {
        Layout::Vertical if p.op == Op::Div => draw_division_house(pm, cx0 + cw / 2.0, cyc, cw, p, input, accent, acol),
        Layout::Vertical => draw_vertical(pm, cx0 + cw / 2.0, cyc, cw, p, input, accent, acol),
        _ => draw_horizontal(pm, cx0 + cw / 2.0, cyc, cw, p, input, accent, acol),
    }
}

/// `a op b = answer` on one line, the answer to the right of the `=`.
#[allow(clippy::too_many_arguments)]
fn draw_horizontal(pm: &mut Pixmap, cxc: f32, cyc: f32, cw: f32, p: &crate::problem::Problem, input: &str, accent: Rgb, acol: Rgb) {
    let prompt  = p.prompt();
    let answer  = if input.is_empty() { "?" } else { input };
    let ans_ref = p.answer.to_string(); // stable sizing — never changes as user types

    // Scale and centering are fixed to the known answer width so the layout
    // never shifts when the typed answer grows from 1 to 2+ digits.
    let scale  = fit_scale(&format!("{}  {}", prompt, ans_ref), cw - 120.0, 6.0);
    let gap    = scale * 6.0;
    let pw     = text_width(&prompt,  scale);
    let aw_ref = text_width(&ans_ref, scale); // stable slot width
    let aw     = text_width(answer,   scale); // live width (may be smaller)
    let x      = cxc - (pw + gap + aw_ref) / 2.0;
    let y      = cyc - glyph_h(scale) / 2.0;
    text(pm, x,              y, scale, &prompt, accent);
    // Right-align the live input inside the stable answer slot so digits land
    // on the correct column as the student types (e.g. units before tens).
    text(pm, x + pw + gap + aw_ref - aw, y, scale, answer, acol);
}

/// The stacked column form: operands right-aligned, a divider, the answer below.
#[allow(clippy::too_many_arguments)]
fn draw_vertical(pm: &mut Pixmap, cxc: f32, cyc: f32, cw: f32, p: &crate::problem::Problem, input: &str, accent: Rgb, acol: Rgb) {
    let (a, b) = (p.a.to_string(), p.b.to_string());
    let ans     = if input.is_empty() { "?".to_string() } else { input.to_string() };
    let ans_ref = p.answer.to_string(); // fixed reference — never changes as user types
    let op      = p.op.symbol().to_string();

    // Size the column from the ACTUAL answer so the field never widens mid-problem.
    let widest = [&a, &b, &ans_ref].iter().map(|s| s.len()).max().unwrap_or(1);
    let scale  = fit_scale(&format!("{} {}", op, "0".repeat(widest)), cw - 200.0, 6.0);

    let field = [&a, &b, &ans_ref].iter().map(|s| text_width(s, scale)).fold(0.0_f32, f32::max);
    let op_col = text_width(&op, scale) + scale * 8.0;
    let total_w = op_col + field;
    let x0 = cxc - total_w / 2.0;
    let field_right = x0 + total_w;
    let right = |pm: &mut Pixmap, s: &str, y: f32, c: Rgb| text(pm, field_right - text_width(s, scale), y, scale, s, c);

    // Spacing in glyph-relative terms so the divider keeps a clear margin from
    // both the operand above it and the answer below (no overlap with the digits).
    let px = scale * PX_PER_SCALE;
    let gbot = px * 0.80; // a glyph's visual bottom, below its line top
    let gtop = px * 0.08; // a glyph's visual top, below its line top
    let margin = px * 0.24; // clear gap on each side of the divider
    let row = px * 1.02; // operand-to-operand line pitch

    let total_h = row + 2.0 * gbot + 2.0 * margin - gtop;
    let y1 = cyc - total_h / 2.0;
    let y2 = y1 + row;
    let y_div = y2 + gbot + margin;
    let y3 = y_div + margin - gtop;

    right(pm, &a, y1, accent);
    text(pm, x0, y2, scale, &op, accent);
    right(pm, &b, y2, accent);
    line(pm, x0, y_div, field_right, y_div, accent, 3.0);
    right(pm, &ans, y3, acol);
}

/// Long-division "house": quotient on the roof, divisor outside, dividend inside.
#[allow(clippy::too_many_arguments)]
fn draw_division_house(pm: &mut Pixmap, cxc: f32, cyc: f32, cw: f32, p: &crate::problem::Problem, input: &str, accent: Rgb, acol: Rgb) {
    let dividend = p.a.to_string();
    let divisor = p.b.to_string();
    let quotient = if input.is_empty() { "?".to_string() } else { input.to_string() };
    let scale = fit_scale(&format!("{}  {}", divisor, dividend), cw - 200.0, 6.0);

    // Glyph-relative spacing so the roof/wall keep a clear margin from the digits
    // (matching the stacked layout) rather than sitting on top of them.
    let px = scale * PX_PER_SCALE;
    let gbot = px * 0.80; // a glyph's visual bottom, below its line top
    let gtop = px * 0.08; // a glyph's visual top, below its line top
    let margin = px * 0.24; // clear gap around the house lines
    let hgap = scale * 12.0; // horizontal gap divisor | wall | dividend

    let ans_ref   = p.answer.to_string(); // fixed reference for stable positioning
    let (wd, wv)  = (text_width(&dividend, scale), text_width(&divisor, scale));
    let wq_ref    = text_width(&ans_ref, scale); // stable quotient slot width
    let wq        = text_width(&quotient, scale); // live width (may be smaller)
    let total_w   = wv + hgap + hgap + wd;
    let x0        = cxc - total_w / 2.0;
    let wall_x    = x0 + wv + hgap;
    let dividend_x = wall_x + hgap;
    // Right-align the live input inside the stable answer slot (anchored to
    // dividend's right edge), so digits land on the correct column as typed.
    let quotient_x = dividend_x + (wd - wq_ref) + (wq_ref - wq);

    let total_h = 2.0 * gbot + 2.5 * margin - gtop;
    let q_y = cyc - total_h / 2.0;
    let roof_y = q_y + gbot + margin; // a margin below the quotient
    let dvd_y = roof_y + margin - gtop; // a margin above the dividend digits
    let wall_bottom = dvd_y + gbot + margin * 0.4;

    text(pm, quotient_x, q_y, scale, &quotient, acol); // quotient above the roof
    text(pm, x0, dvd_y, scale, &divisor, accent); // divisor left of the wall
    text(pm, dividend_x, dvd_y, scale, &dividend, accent); // dividend inside
    // The house: a left wall and a roof over the dividend.
    line(pm, wall_x, roof_y, wall_x, wall_bottom, accent, 3.0);
    line(pm, wall_x, roof_y, dividend_x + wd, roof_y, accent, 3.0);
}

// -- Fraction / percentage figures ------------------------------------------

/// Faint fill for a region that hasn't materialised yet.
const SLOT: Rgb = [60, 62, 78];
/// Thin separator / outline tone between pieces.
const SEP: Rgb = [92, 96, 116];

/// Draw the fraction/percent figure inside `(rx, ry, rw, rh)`, the shaded pieces
/// first, materialising as `progress` rises from 0 to 1.
pub fn draw_shape(pm: &mut Pixmap, (rx, ry, rw, rh): (f32, f32, f32, f32), s: &FractionProblem, progress: f32) {
    // Chalk is monochrome: shade by *filling* (vs hollow outline), not colour.
    if is_chalk() {
        draw_shape_chalk(pm, rx, ry, rw, rh, s, progress);
        return;
    }
    let color_of = |i: u16| core_col(s.color_of(i));
    match s.shape {
        Shape::Bar { segments } => draw_cells(pm, rx, ry, rw, rh, 1, segments, progress, &color_of),
        Shape::Grid { rows, cols } => draw_cells(pm, rx, ry, rw, rh, rows, cols, progress, &color_of),
        Shape::Circle { slices } => draw_pie(pm, rx, ry, rw, rh, slices, progress, &color_of),
        Shape::Triangle { strips } => draw_tri_strips(pm, rx, ry, rw, rh, strips, progress, &color_of),
    };
}

/// Has region `i` of `n` appeared yet at this `progress`?
fn appeared(i: u16, n: u16, progress: f32) -> bool {
    progress + 0.001 >= (i as f32 + 1.0) / n as f32
}

/// Chalk figure: every piece is outlined in chalk; the *shaded* pieces are filled
/// in solid (so the share reads in black-and-white without any colour).
fn draw_shape_chalk(pm: &mut Pixmap, rx: f32, ry: f32, rw: f32, rh: f32, s: &FractionProblem, progress: f32) {
    let shaded = s.shaded as u16;
    let fill_in = |i: u16, n: u16| i < shaded && appeared(i, n, progress);
    match s.shape {
        Shape::Bar { segments } => draw_cells_chalk(pm, rx, ry, rw, rh, 1, segments, &fill_in),
        Shape::Grid { rows, cols } => draw_cells_chalk(pm, rx, ry, rw, rh, rows, cols, &fill_in),
        Shape::Circle { slices } => draw_pie_chalk(pm, rx, ry, rw, rh, slices, &fill_in),
        Shape::Triangle { strips } => draw_tri_chalk(pm, rx, ry, rw, rh, strips, &fill_in),
    }
}

/// Cells (bar / grid): every cell outlined, shaded cells filled solid.
#[allow(clippy::too_many_arguments)]
fn draw_cells_chalk(pm: &mut Pixmap, rx: f32, ry: f32, rw: f32, rh: f32, rows: u16, cols: u16, fill_in: &impl Fn(u16, u16) -> bool) {
    let n = rows * cols;
    let gap = 7.0;
    let cw = ((rw - (cols - 1) as f32 * gap) / cols as f32).min(70.0);
    let cht = ((rh - (rows - 1) as f32 * gap) / rows as f32).min(if rows == 1 { 96.0 } else { 64.0 });
    let total_w = cols as f32 * cw + (cols - 1) as f32 * gap;
    let total_h = rows as f32 * cht + (rows - 1) as f32 * gap;
    let x0 = rx + (rw - total_w) / 2.0;
    let y0 = ry + (rh - total_h) / 2.0;
    for i in 0..n {
        let (row, c) = (i / cols, i % cols);
        let x = x0 + c as f32 * (cw + gap);
        let y = y0 + row as f32 * (cht + gap);
        if fill_in(i, n) {
            fill(pm, x, y, cw, cht, WHITE);
        }
        stroke_rect(pm, x, y, cw, cht, 2.0, WHITE);
    }
}

/// Pie: every wedge outlined by spokes + rim, shaded wedges filled solid.
fn draw_pie_chalk(pm: &mut Pixmap, rx: f32, ry: f32, rw: f32, rh: f32, slices: u16, fill_in: &impl Fn(u16, u16) -> bool) {
    use std::f32::consts::TAU;
    let (cx, cy) = (rx + rw / 2.0, ry + rh / 2.0);
    let radius = (rw / 2.0).min(rh / 2.0) * 0.92;
    let ang = |k: u16| (k as f32 / slices as f32 - 0.5) * TAU;
    for k in 0..slices {
        if !fill_in(k, slices) {
            continue;
        }
        let (t0, t1) = (ang(k), ang(k + 1));
        let mut pts = vec![(cx, cy)];
        for sgmt in 0..=10 {
            let a = t0 + (t1 - t0) * (sgmt as f32 / 10.0);
            pts.push((cx + a.cos() * radius, cy + a.sin() * radius));
        }
        if let Some(p) = poly(&pts) {
            fill_path(pm, &p, WHITE);
        }
    }
    // Spokes: spoke k separates slice k-1 (prev) from slice k (this).
    //   Both sides shaded  → BG void so the board shows through adjacent fills.
    //   Either side unshaded → WHITE chalk so the boundary is visible on the board.
    for k in 0..slices {
        let a = ang(k);
        let this_filled = fill_in(k, slices);
        let prev_filled = fill_in((k + slices - 1) % slices, slices);
        if this_filled && prev_filled {
            line(pm, cx, cy, cx + a.cos() * radius, cy + a.sin() * radius, BG, 3.5);
        } else {
            line(pm, cx, cy, cx + a.cos() * radius, cy + a.sin() * radius, WHITE, 2.0);
        }
    }
    circle(pm, cx, cy, radius, WHITE, 2.0);
}

/// Triangle: every strip outlined, shaded strips filled solid.
fn draw_tri_chalk(pm: &mut Pixmap, rx: f32, ry: f32, rw: f32, rh: f32, strips: u16, fill_in: &impl Fn(u16, u16) -> bool) {
    let th = rh * 0.9;
    let base_w = (rw * 0.82).min(th * 1.5);
    let cx = rx + rw / 2.0;
    let apex_y = ry + (rh - th) / 2.0;
    let half = |fy: f32| fy * base_w / 2.0;
    let yat = |fy: f32| apex_y + fy * th;
    for i in 0..strips {
        if !fill_in(i, strips) {
            continue;
        }
        let (f0, f1) = (i as f32 / strips as f32, (i + 1) as f32 / strips as f32);
        let (y0, y1) = (yat(f0), yat(f1));
        let (h0, h1) = (half(f0), half(f1));
        if let Some(p) = poly(&[(cx - h0, y0), (cx + h0, y0), (cx + h1, y1), (cx - h1, y1)]) {
            fill_path(pm, &p, WHITE);
        }
    }
    // Separator at strip boundary i separates strip i-1 (above) from strip i (below).
    //   Both filled   → BG void so the board shows through adjacent chalk fills.
    //   Either empty  → WHITE chalk so the boundary is visible on the board.
    // Outer edges (sloped sides + base) are always WHITE chalk on board.
    for i in 1..strips {
        let fy = i as f32 / strips as f32;
        let above_filled = fill_in(i - 1, strips);
        let below_filled = fill_in(i, strips);
        let (color, width) = if above_filled && below_filled { (BG, 3.5) } else { (WHITE, 2.0) };
        line(pm, cx - half(fy), yat(fy), cx + half(fy), yat(fy), color, width);
    }
    let (bl, br, top) = ((cx - half(1.0), yat(1.0)), (cx + half(1.0), yat(1.0)), (cx, apex_y));
    line(pm, top.0, top.1, bl.0, bl.1, WHITE, 2.0);
    line(pm, top.0, top.1, br.0, br.1, WHITE, 2.0);
    line(pm, bl.0, bl.1, br.0, br.1, WHITE, 2.0);
}

/// A `rows × cols` block of separated cells (covers both the bar and the grid).
#[allow(clippy::too_many_arguments)]
fn draw_cells(pm: &mut Pixmap, rx: f32, ry: f32, rw: f32, rh: f32, rows: u16, cols: u16, progress: f32, color_of: &impl Fn(u16) -> Rgb) {
    let n = rows * cols;
    let gap = 7.0;
    let cw = ((rw - (cols - 1) as f32 * gap) / cols as f32).min(70.0);
    let cht = ((rh - (rows - 1) as f32 * gap) / rows as f32).min(if rows == 1 { 96.0 } else { 64.0 });
    let total_w = cols as f32 * cw + (cols - 1) as f32 * gap;
    let total_h = rows as f32 * cht + (rows - 1) as f32 * gap;
    let x0 = rx + (rw - total_w) / 2.0;
    let y0 = ry + (rh - total_h) / 2.0;
    for i in 0..n {
        let (row, c) = (i / cols, i % cols);
        let x = x0 + c as f32 * (cw + gap);
        let y = y0 + row as f32 * (cht + gap);
        fill(pm, x, y, cw, cht, region_fill(i, n, progress, color_of(i)));
    }
}

/// A pie circle of `slices` wedges, shaded pieces first.
#[allow(clippy::too_many_arguments)]
fn draw_pie(pm: &mut Pixmap, rx: f32, ry: f32, rw: f32, rh: f32, slices: u16, progress: f32, color_of: &impl Fn(u16) -> Rgb) {
    use std::f32::consts::TAU;
    let (cx, cy) = (rx + rw / 2.0, ry + rh / 2.0);
    let radius = (rw / 2.0).min(rh / 2.0) * 0.92;
    let ang = |k: u16| (k as f32 / slices as f32 - 0.5) * TAU;
    for k in 0..slices {
        let (t0, t1) = (ang(k), ang(k + 1));
        let steps = 10;
        let mut pts = vec![(cx, cy)];
        for sgmt in 0..=steps {
            let a = t0 + (t1 - t0) * (sgmt as f32 / steps as f32);
            pts.push((cx + a.cos() * radius, cy + a.sin() * radius));
        }
        if let Some(p) = poly(&pts) {
            fill_path(pm, &p, region_fill(k, slices, progress, color_of(k)));
        }
    }
    // Spokes between wedges + a rim, so same-coloured neighbours stay distinct.
    for k in 0..slices {
        let a = ang(k);
        line(pm, cx, cy, cx + a.cos() * radius, cy + a.sin() * radius, BG, 2.0);
    }
    circle(pm, cx, cy, radius, SEP, 2.0);
}

/// An upward triangle cut into `strips` horizontal layers (apex = region 0).
#[allow(clippy::too_many_arguments)]
fn draw_tri_strips(pm: &mut Pixmap, rx: f32, ry: f32, rw: f32, rh: f32, strips: u16, progress: f32, color_of: &impl Fn(u16) -> Rgb) {
    let th = rh * 0.9;
    let base_w = (rw * 0.82).min(th * 1.5);
    let cx = rx + rw / 2.0;
    let apex_y = ry + (rh - th) / 2.0;
    let half = |fy: f32| fy * base_w / 2.0;
    let yat = |fy: f32| apex_y + fy * th;
    for i in 0..strips {
        let (f0, f1) = (i as f32 / strips as f32, (i + 1) as f32 / strips as f32);
        let (y0, y1) = (yat(f0), yat(f1));
        let (h0, h1) = (half(f0), half(f1));
        let pts = [(cx - h0, y0), (cx + h0, y0), (cx + h1, y1), (cx - h1, y1)];
        if let Some(p) = poly(&pts) {
            fill_path(pm, &p, region_fill(i, strips, progress, color_of(i)));
        }
    }
    // Separators between strips + the two sloped edges.
    for i in 1..strips {
        let fy = i as f32 / strips as f32;
        line(pm, cx - half(fy), yat(fy), cx + half(fy), yat(fy), BG, 2.0);
    }
    let (bl, br, top) = ((cx - half(1.0), yat(1.0)), (cx + half(1.0), yat(1.0)), (cx, apex_y));
    line(pm, top.0, top.1, bl.0, bl.1, SEP, 2.0);
    line(pm, top.0, top.1, br.0, br.1, SEP, 2.0);
    line(pm, bl.0, bl.1, br.0, br.1, SEP, 2.0);
}

/// Blend a region from the faint slot toward its `base` colour as it appears.
fn region_fill(i: u16, n: u16, progress: f32, base: Rgb) -> Rgb {
    let appear = (i + 1) as f32 / n as f32;
    let start = i as f32 / n as f32;
    if progress >= appear {
        base
    } else if progress > start {
        lerp_rgb(SLOT, base, (progress - start) / (appear - start))
    } else {
        SLOT
    }
}

// -- Geometry shapes --------------------------------------------------------

/// Draw the section's shape inside the region `(x, y, w, h)`, labelled.
pub fn draw_geo(pm: &mut Pixmap, (rx, ry, rw, rh): (f32, f32, f32, f32), g: &GeometryProblem) {
    let (cx, cy) = (rx + rw / 2.0, ry + rh / 2.0);
    match g.shape {
        GeoShape::Rect { w, h } => {
            let (bw, bh) = fit_box(w, h, rw * 0.66, rh * 0.78);
            let (x0, y0) = (cx - bw / 2.0, cy - bh / 2.0);
            stroke_rect(pm, x0, y0, bw, bh, 3.0, ACCENT);
            text_centered(pm, cx, y0 + bh + 10.0, 1.5, &w.to_string(), YELLOW);
            text(pm, x0 + bw + 14.0, cy - 6.0, 1.5, &h.to_string(), YELLOW);
        }
        GeoShape::Triangle { base, height, sides } => {
            let half = (rw * 0.4).min(rh * 0.9);
            let th = rh * 0.78;
            let (ay, by) = (cy - th / 2.0, cy + th / 2.0);
            tri(pm, cx, ay, cx - half, by, cx + half, by, ACCENT, 3.0);
            if sides.is_none() {
                line(pm, cx, ay, cx, by, YELLOW, 2.0); // the height
            }
            let label = match sides {
                Some([a, b, c]) => format!("sides: {}, {}, {} cm", a, b, c),
                None => format!("base {} cm,  height {} cm", base, height),
            };
            text_centered(pm, cx, by + 14.0, 1.5, &label, GRAY);
        }
        GeoShape::Circle { r } => {
            // Cap to half the band (less the label row) so the disc stays inside
            // the band and never reaches up into the question text above it.
            let max_rad = (rh * 0.5 - 26.0).min(rw * 0.5).max(8.0);
            let rad = max_rad * (0.62 + (r.clamp(2, 9) as f32) / 30.0).min(1.0);
            circle(pm, cx, cy - 8.0, rad, ACCENT, 3.0);
            line(pm, cx, cy - 8.0, cx + rad, cy - 8.0, YELLOW, 2.0); // the radius
            text_centered(pm, cx, cy + rad - 2.0, 1.5, &format!("radius {} cm", r), GRAY);
        }
        GeoShape::Box3 { l, w, h } => {
            let fw = (rw * 0.42).min(rh * 1.2);
            let fh = rh * 0.5;
            let d = rh * 0.18;
            let (x0, y0) = (cx - (fw + d) / 2.0, cy - fh / 2.0 + d / 2.0);
            stroke_rect(pm, x0, y0, fw, fh, 2.0, ACCENT); // front face
            stroke_rect(pm, x0 + d, y0 - d, fw, fh, 2.0, ACCENT); // back face
            line(pm, x0, y0, x0 + d, y0 - d, ACCENT, 2.0); // connect the corners
            line(pm, x0 + fw, y0, x0 + fw + d, y0 - d, ACCENT, 2.0);
            line(pm, x0, y0 + fh, x0 + d, y0 + fh - d, ACCENT, 2.0);
            line(pm, x0 + fw, y0 + fh, x0 + fw + d, y0 + fh - d, ACCENT, 2.0);
            text_centered(pm, cx, y0 + fh + 16.0, 1.5, &format!("{} x {} x {} cm", l, w, h), GRAY);
        }
    }
}

// -- Units of Measure — a themed prop per problem ---------------------------

/// The accent colour for a unit problem's theme (matches the terminal palette).
pub fn theme_col(t: Theme) -> Rgb {
    match t {
        Theme::Cup => [240, 215, 110],
        Theme::Pool => [120, 170, 240],
        Theme::Bottle => [120, 220, 240],
        Theme::Scale => [210, 140, 230],
        Theme::Ruler => [120, 220, 150],
        Theme::Coin => [255, 200, 40],
    }
}

/// Draw a small prop for `theme`, centred in `(rx, ry, rw, rh)`.
pub fn draw_unit_prop(pm: &mut Pixmap, (rx, ry, rw, rh): (f32, f32, f32, f32), theme: Theme) {
    let c = theme_col(theme);
    let (cx, cy) = (rx + rw / 2.0, ry + rh / 2.0);
    let h = rh * 0.74;
    match theme {
        Theme::Cup => {
            let (ht, topw, botw) = (h, h * 0.95, h * 0.66);
            let (top, bot) = (cy - ht / 2.0, cy + ht / 2.0);
            let hw = |y: f32| {
                let t = (y - top) / (bot - top);
                topw / 2.0 + (botw / 2.0 - topw / 2.0) * t
            };
            let lt = top + ht * 0.45; // liquid line
            if let Some(p) = poly(&[(cx - hw(lt), lt), (cx + hw(lt), lt), (cx + hw(bot), bot), (cx - hw(bot), bot)]) {
                fill_path(pm, &p, [90, 150, 220]);
            }
            line(pm, cx - topw / 2.0, top, cx + topw / 2.0, top, c, 3.0);
            line(pm, cx - topw / 2.0, top, cx - botw / 2.0, bot, c, 3.0);
            line(pm, cx + topw / 2.0, top, cx + botw / 2.0, bot, c, 3.0);
            line(pm, cx - botw / 2.0, bot, cx + botw / 2.0, bot, c, 3.0);
            // Handle, following the cup's slope on the right.
            let (ya, yb) = (top + 12.0, cy + 10.0);
            line(pm, cx + hw(ya), ya, cx + hw(ya) + 30.0, ya, c, 3.0);
            line(pm, cx + hw(ya) + 30.0, ya, cx + hw(yb) + 30.0, yb, c, 3.0);
            line(pm, cx + hw(yb) + 30.0, yb, cx + hw(yb), yb, c, 3.0);
            for k in 1..4 {
                let y = top + ht * 0.18 * k as f32;
                line(pm, cx - topw * 0.30, y, cx - topw * 0.15, y, c, 2.0);
            }
        }
        Theme::Pool => {
            let (pw, ph) = (rw * 0.46, h * 0.7);
            let (x0, y0) = (cx - pw / 2.0, cy - ph / 2.0);
            fill(pm, x0 + 4.0, y0 + ph * 0.34, pw - 8.0, ph * 0.66 - 4.0, [70, 120, 200]);
            stroke_rect(pm, x0, y0, pw, ph, 3.0, c);
            for k in 0..3 {
                wavy_line(pm, x0 + 10.0, x0 + pw - 10.0, y0 + ph * 0.46 + k as f32 * 16.0, 5.0, [185, 212, 240], 2.0);
            }
        }
        Theme::Bottle => {
            let (bw, bh) = (h * 0.5, h * 0.96);
            let (x0, top) = (cx - bw / 2.0, cy - bh / 2.0);
            let (neck_w, neck_h) = (bw * 0.4, bh * 0.22);
            fill(pm, cx - neck_w / 2.0 - 2.0, top, neck_w + 4.0, 10.0, c); // cap
            stroke_rect(pm, cx - neck_w / 2.0, top + 10.0, neck_w, neck_h, 3.0, c);
            let body_top = top + 10.0 + neck_h;
            let body_h = bh - 10.0 - neck_h;
            fill(pm, x0 + 4.0, body_top + body_h * 0.42, bw - 8.0, body_h * 0.58 - 4.0, [90, 180, 210]);
            stroke_rect(pm, x0, body_top, bw, body_h, 3.0, c);
        }
        Theme::Scale => {
            let bw = rw * 0.4;
            let beam_y = cy - h * 0.26;
            line(pm, cx, beam_y, cx, cy + h * 0.4, c, 3.0); // post
            line(pm, cx - h * 0.3, cy + h * 0.4, cx + h * 0.3, cy + h * 0.4, c, 4.0); // base
            line(pm, cx - bw / 2.0, beam_y, cx + bw / 2.0, beam_y, c, 3.0); // beam
            for sx in [cx - bw / 2.0, cx + bw / 2.0] {
                line(pm, sx, beam_y, sx, beam_y + 30.0, c, 2.0);
                if let Some(p) = poly(&[(sx - 26.0, beam_y + 30.0), (sx + 26.0, beam_y + 30.0), (sx + 16.0, beam_y + 44.0), (sx - 16.0, beam_y + 44.0)]) {
                    fill_path(pm, &p, c);
                }
            }
        }
        Theme::Ruler => {
            let (rw2, rh2) = (rw * 0.58, h * 0.34);
            let (x0, y0) = (cx - rw2 / 2.0, cy - rh2 / 2.0);
            stroke_rect(pm, x0, y0, rw2, rh2, 3.0, c);
            let n = 10;
            for k in 0..=n {
                let x = x0 + rw2 * (k as f32 / n as f32);
                let len = if k % 5 == 0 { rh2 * 0.6 } else { rh2 * 0.35 };
                line(pm, x, y0, x, y0 + len, c, 2.0);
            }
        }
        Theme::Coin => {
            let r = h * 0.32;
            for off in [16.0, 8.0, 0.0] {
                circle(pm, cx, cy + off, r, c, 3.0);
            }
            text_centered(pm, cx, cy - glyph_h(1.8) / 2.0, 1.8, "$", c);
        }
    }
}

/// The onomatopoeia the duck makes diving into a prop.
fn theme_splash(theme: Theme) -> &'static str {
    match theme {
        Theme::Cup | Theme::Pool | Theme::Bottle => "SPLASH!",
        Theme::Scale => "THUD!",
        Theme::Ruler => "BONK!",
        Theme::Coin => "CHA-CHING!",
    }
}

/// The silly gag: a mini duck arcs in from the left, splashes into the prop,
/// then pops back out — on a loop, while `duck_jump` is set.
pub fn draw_unit_gag(pm: &mut Pixmap, (rx, ry, rw, rh): (f32, f32, f32, f32), theme: Theme, frame: u64) {
    use std::f32::consts::PI;
    let c = theme_col(theme);
    let cx = rx + rw / 2.0;
    let top = ry + rh * 0.30;
    let land_x = cx - 16.0;
    let mini = "~(o>";
    let scale = 1.6;

    const CYCLE: u64 = 96;
    let t = (frame % CYCLE) as f32 / CYCLE as f32;
    if t < 0.45 {
        let p = t / 0.45;
        let start_x = rx + 24.0;
        let x = start_x + (land_x - start_x) * p;
        let y = top - 8.0 - (p * PI).sin() * 46.0;
        text(pm, x, y, scale, mini, c);
    } else if t < 0.58 {
        text_centered(pm, cx, top - 26.0, scale, theme_splash(theme), YELLOW);
        for (dx, dy) in [(-40.0, 0.0), (40.0, 0.0), (-28.0, 12.0), (28.0, 12.0)] {
            text_centered(pm, cx + dx, top + dy, 1.4, "\u{00b0}", CYAN);
        }
    } else {
        let p = (t - 0.58) / 0.42;
        let y = top - 8.0 - (p * PI).sin() * 40.0;
        text(pm, land_x, y, scale, mini, c);
    }
}
