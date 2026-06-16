//! Problem-card renderers for the terminal frontend.
//!
//! Each `render_*_card` function writes directly into a [`Buffer`] so the same
//! routine paints the live frame *and* can be captured off-screen for
//! transition effects.  The geometry helpers, theme palette, and arithmetic
//! layout routines live here alongside the cards that use them.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Widget};

use crate::canvas::Canvas;
use crate::config::Layout;
use crate::font;
use crate::problem::{Op, Problem};
use crate::units::{Theme, UnitProblem};

use super::*;

// ---------------------------------------------------------------------------
// Units of Measure
// ---------------------------------------------------------------------------

/// Geometry's accent colour.
const GEO_ACCENT: Color = Color::LightGreen;
/// Colour for the dimension numbers labelled on a shape's edges.
const GEO_DIM: Color = Color::LightYellow;

/// Draw a measurement card: question on top, the answer to type in the middle,
/// and the themed prop at the bottom.  Used live and for transition capture.
pub fn render_unit_card(area: Rect, buf: &mut Buffer, p: &UnitProblem, input: &str, banner: Option<(String, Color)>) {
    let accent = theme_color(p.theme);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(accent))
        .title(" Units of Measure ")
        .style(Style::default().bg(BG));
    let inner = block.inner(area);
    block.render(area, buf);

    // Question, word-wrapped and centred near the top.
    let qlines = wrap_text(&p.question, inner.width.saturating_sub(4));
    let mut y = inner.top() + 1;
    for line in qlines.iter().take(4) {
        put_str(buf, center(inner, line.chars().count() as u16), y, line, Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
        y += 1;
    }

    // The big answer the student types, with its unit beneath.
    let ans = if input.is_empty() { "?" } else { input };
    let ay = inner.top() + inner.height / 2;
    font::draw_text(buf, center(inner, font::text_width(ans)), ay.saturating_sub(2), ans, answer_style(input));
    put_str(buf, center(inner, p.unit_label.chars().count() as u16), ay + font::GLYPH_H.saturating_sub(2), &p.unit_label, Style::default().fg(accent).add_modifier(Modifier::BOLD));

    if let Some((text, color)) = banner {
        put_str(buf, center(inner, text.chars().count() as u16), ay + font::GLYPH_H + 1, text, Style::default().fg(color).add_modifier(Modifier::BOLD));
    }

    // Static themed prop at the bottom (the duck gag animates over this).
    let (px, py, prop) = prop_placement(inner, p.theme);
    for (i, line) in prop.iter().enumerate() {
        put_str(buf, px, py + i as u16, line, Style::default().fg(accent));
    }

    let hints = "Enter check    H help duck    Esc menu";
    put_str(buf, center(inner, hints.chars().count() as u16), inner.bottom().saturating_sub(1), hints, Style::default().fg(Color::DarkGray));
}

// ---------------------------------------------------------------------------
// Geometry — hollow (outlined) shapes for perimeter, area, and volume
// ---------------------------------------------------------------------------

/// Draw a geometry card: the question, an outlined shape with its dimensions
/// labelled, and the answer the student types (with its unit).
pub fn render_geometry_card(area: Rect, buf: &mut Buffer, p: &crate::geometry::GeometryProblem, input: &str, banner: Option<(String, Color)>) {
    use crate::geometry::GeoShape;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(GEO_ACCENT))
        .title(" Geometry ")
        .style(Style::default().bg(BG));
    let inner = block.inner(area);
    block.render(area, buf);

    // Question.
    let q = p.question();
    let mut y = inner.top() + 1;
    for line in wrap_text(&q, inner.width.saturating_sub(4)).iter().take(2) {
        put_str(buf, center(inner, line.chars().count() as u16), y, line, Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
        y += 1;
    }

    // The outlined shape occupies the middle band.
    let answer_h = 4;
    let band = Rect {
        x: inner.left() + 1,
        y: y + 1,
        width: inner.width.saturating_sub(2),
        height: inner.bottom().saturating_sub(y + 1 + answer_h + 1),
    };
    if band.height >= 4 && band.width >= 12 {
        match p.shape {
            GeoShape::Rect { w, h } => draw_geo_rect(buf, band, w, h),
            GeoShape::Triangle { base, height, sides } => draw_geo_triangle(buf, band, base, height, sides),
            GeoShape::Circle { r } => draw_geo_circle(buf, band, r),
            GeoShape::Box3 { l, w, h } => draw_geo_box(buf, band, l, w, h),
        }
    }

    // The answer the student types, with its unit (a π coefficient for circles).
    let ay = inner.bottom().saturating_sub(answer_h + 1);
    put_str(buf, center(inner, 12), ay, "Your answer:", Style::default().fg(Color::Gray));
    let num = if input.is_empty() { "?" } else { input };
    let shown = format!("{}{} {}", num, if p.pi { "π" } else { "" }, p.unit);
    put_str(buf, center(inner, shown.chars().count() as u16), ay + 1, &shown, Style::default().fg(Color::White).add_modifier(Modifier::BOLD));

    if let Some((text, color)) = banner {
        put_str(buf, center(inner, text.chars().count() as u16), ay + 2, &text, Style::default().fg(color).add_modifier(Modifier::BOLD));
    }

    let hints = "Type a whole number    Enter check    H help duck    Esc menu";
    put_str(buf, center(inner, hints.chars().count() as u16), inner.bottom().saturating_sub(1), hints, Style::default().fg(Color::DarkGray));
}

/// Map a shape's unit dimensions to cell dimensions, honouring the ~2:1
/// width:height aspect of a terminal cell, fitting within `(max_w, max_h)`.
fn fit_cells(uw: i64, uh: i64, max_w: u16, max_h: u16) -> (u16, u16) {
    let want_w = uw.max(1) as f32 * 2.0;
    let want_h = uh.max(1) as f32;
    let s = (max_w as f32 / want_w).min(max_h as f32 / want_h).clamp(0.05, 2.4);
    let cw = ((want_w * s).round() as u16).clamp(6, max_w.max(6));
    let ch = ((want_h * s).round() as u16).clamp(2, max_h.max(2));
    (cw, ch)
}

/// A hollow rectangle (or square), scaled to its proportions, with the width
/// labelled below and the height to the right.
fn draw_geo_rect(buf: &mut Buffer, band: Rect, w: i64, h: i64) {
    let style = Style::default().fg(GEO_ACCENT);
    let (bw, bh) = fit_cells(w, h, band.width.saturating_sub(8).min(40), band.height.saturating_sub(3).min(9));
    let x0 = center(band, bw);
    let y0 = band.top() + band.height.saturating_sub(bh + 1) / 2;

    put_str(buf, x0, y0, format!("┌{}┐", "─".repeat((bw - 2) as usize)), style);
    for r in 1..bh - 1 {
        put_str(buf, x0, y0 + r, "│", style);
        put_str(buf, x0 + bw - 1, y0 + r, "│", style);
    }
    put_str(buf, x0, y0 + bh - 1, format!("└{}┘", "─".repeat((bw - 2) as usize)), style);

    // Dimensions: width centred below the box, height to the right.
    let wl = w.to_string();
    put_str(buf, x0 + bw / 2 - (wl.len() as u16 / 2), y0 + bh, &wl, Style::default().fg(GEO_DIM));
    let hl = h.to_string();
    put_str(buf, x0 + bw + 1, y0 + bh / 2, &hl, Style::default().fg(GEO_DIM));
}

/// A hollow isosceles triangle with contiguous `/ \` slopes (one column per row,
/// so the edges never break up), labelled with its base+height (area) or its
/// three side lengths (perimeter).
fn draw_geo_triangle(buf: &mut Buffer, band: Rect, base: i64, height: i64, sides: Option<[i64; 3]>) {
    // Draw the outline on a half-block canvas so the slopes come out smooth.
    let cell_rows = band.height.saturating_sub(2).clamp(3, 12);
    let mut cv = Canvas::new(band.width, cell_rows);
    let px_h = cv.rows() as i32;
    let cx = (cv.cols() / 2) as i32;
    let (top, bot) = (1, px_h - 2);
    // Isosceles; base half-width tracks the height (square pixels keep the
    // slopes natural), clamped to the canvas.
    let half = (bot - top).clamp(2, cx - 1);
    cv.line(cx, top, cx - half, bot, GEO_ACCENT); // left slope
    cv.line(cx, top, cx + half, bot, GEO_ACCENT); // right slope
    cv.line(cx - half, bot, cx + half, bot, GEO_ACCENT); // base
    // For an area triangle, drop the height down the middle to match the label.
    if sides.is_none() {
        cv.line(cx, top, cx, bot, GEO_DIM);
    }
    cv.blit(buf, band.left(), band.top());

    let label = match sides {
        Some([a, b, c]) => format!("sides: {}, {}, {} cm", a, b, c),
        None => format!("base {} cm,  height {} cm", base, height),
    };
    put_str(buf, center(band, label.chars().count() as u16), band.top() + cell_rows + 1, &label, Style::default().fg(GEO_DIM));
}

/// A hollow circle on a half-block canvas — a genuinely round outline sized to
/// its radius, with a radius spoke and label.  Circumference / area are asked
/// in terms of π.
fn draw_geo_circle(buf: &mut Buffer, band: Rect, r: i64) {
    let cell_rows = band.height.saturating_sub(2).clamp(3, 12);
    let mut cv = Canvas::new(band.width, cell_rows);
    let (cx, cy) = ((cv.cols() / 2) as i32, cv.rows() as i32 / 2);
    let fit = cx.min(cy).saturating_sub(1).max(2);
    // Grow a little with r (2..=9), but keep it filling the band.
    let rad = (fit * ((r as i32).clamp(2, 9) + 11) / 20).clamp(2, fit);
    cv.circle(cx, cy, rad, GEO_ACCENT);
    cv.line(cx, cy, cx + rad, cy, GEO_DIM); // radius spoke
    cv.blit(buf, band.left(), band.top());

    let label = format!("radius {} cm", r);
    put_str(buf, center(band, label.chars().count() as u16), band.top() + cell_rows + 1, &label, Style::default().fg(GEO_DIM));
}

/// A crisp wireframe cuboid (or cube) in cabinet projection, scaled to its
/// length × height with a depth offset from its width, edges labelled.
fn draw_geo_box(buf: &mut Buffer, band: Rect, l: i64, w: i64, h: i64) {
    let style = Style::default().fg(GEO_ACCENT);
    let depth = (w as u16 + 1).div_ceil(2).clamp(2, 3);
    let (fw, fh) = fit_cells(
        l,
        h,
        band.width.saturating_sub(8 + depth).min(34),
        band.height.saturating_sub(4 + depth).min(8),
    );
    let fw = fw.max(6);
    let fh = fh.max(3);
    // Front-face top-left, leaving room above/right for the depth and labels.
    let x0 = band.left() + (band.width.saturating_sub(fw + depth)) / 2;
    let y0 = band.top() + depth + 1;

    // Front face.
    put_str(buf, x0, y0, format!("┌{}┐", "─".repeat((fw - 2) as usize)), style);
    for r in 1..fh - 1 {
        put_str(buf, x0, y0 + r, "│", style);
        put_str(buf, x0 + fw - 1, y0 + r, "│", style);
    }
    put_str(buf, x0, y0 + fh - 1, format!("└{}┘", "─".repeat((fw - 2) as usize)), style);

    // Depth: diagonals up-right from the two top corners and the bottom-right
    // corner, plus the visible back top edge and back right edge.
    for i in 1..depth {
        put_str(buf, x0 + i, y0.saturating_sub(i), "╱", style);
        put_str(buf, x0 + fw - 1 + i, y0.saturating_sub(i), "╱", style);
        put_str(buf, x0 + fw - 1 + i, y0 + fh - 1 - i, "╱", style);
    }
    let bx = x0 + depth;
    let by = y0.saturating_sub(depth);
    put_str(buf, bx, by, format!("┌{}┐", "─".repeat((fw - 2) as usize)), style);
    for r in 1..fh - 1 {
        put_str(buf, bx + fw - 1, by + r, "│", style);
    }
    put_str(buf, bx + fw - 1, by + fh - 1, "┘", style);

    // Edge labels: length below, height to the left, width along the top depth.
    let ll = l.to_string();
    put_str(buf, x0 + fw / 2 - (ll.len() as u16 / 2), y0 + fh, &ll, Style::default().fg(GEO_DIM));
    let hl = h.to_string();
    put_str(buf, x0.saturating_sub(1 + hl.len() as u16), y0 + fh / 2, &hl, Style::default().fg(GEO_DIM));
    // Width (depth) just past the back-top-right corner, with a gap.
    let wl = w.to_string();
    put_str(buf, bx + fw + 1, by, &wl, Style::default().fg(GEO_DIM));
}

fn theme_color(theme: Theme) -> Color {
    match theme {
        Theme::Cup => Color::LightYellow,
        Theme::Pool => Color::LightBlue,
        Theme::Bottle => Color::LightCyan,
        Theme::Scale => Color::LightMagenta,
        Theme::Ruler => Color::LightGreen,
        Theme::Coin => Color::Rgb(255, 200, 40), // gold
    }
}

/// The little ASCII prop for a theme.
fn theme_prop(theme: Theme) -> &'static [&'static str] {
    match theme {
        Theme::Cup => &[" ___", "|  |)", "|__|"],
        Theme::Pool => &["~~~~~~~~", "~~~~~~~~"],
        Theme::Bottle => &[" __ ", "|  |", "|~~|", "|__|"],
        Theme::Scale => &[" _^_ ", "/ | \\", "====="],
        Theme::Ruler => &["|.|.|.|.|"],
        Theme::Coin => &["( $ )", "(===)", "(===)"],
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

/// Bottom-centred placement of the prop within `inner`.
fn prop_placement(inner: Rect, theme: Theme) -> (u16, u16, &'static [&'static str]) {
    let prop = theme_prop(theme);
    let w = prop.iter().map(|l| l.chars().count()).max().unwrap_or(1) as u16;
    let x = center(inner, w);
    let y = inner.bottom().saturating_sub(prop.len() as u16 + 1);
    (x, y, prop)
}

/// The silly gag: Deduction Duck hops in from the side, splashes into the prop,
/// then pops back out — on a loop.
pub fn draw_duck_gag(buf: &mut Buffer, inner: Rect, theme: Theme, frame: u64) {
    let (px, py, prop) = prop_placement(inner, theme);
    let pw = prop.iter().map(|l| l.chars().count()).max().unwrap_or(1) as u16;
    let cx = (px + pw / 2) as f32;
    let top = py as f32;
    let color = theme_color(theme);
    let mini = "~(o>"; // tiny duck facing right

    const CYCLE: u64 = 96;
    let t = (frame % CYCLE) as f32 / CYCLE as f32;
    let land_x = cx - 2.0;

    if t < 0.45 {
        // Arc in from the left edge toward the prop.
        let p = t / 0.45;
        let start_x = inner.left() as f32 + 1.0;
        let x = start_x + (land_x - start_x) * p;
        let y = top - 1.0 - (p * std::f32::consts::PI).sin() * 5.0;
        put_float(buf, x, y, mini, Style::default().fg(color));
    } else if t < 0.58 {
        // Splash! — the word above, droplets down on the water rows.
        let word = theme_splash(theme);
        put_float(buf, cx - word.chars().count() as f32 / 2.0, top - 2.0, word, Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD));
        for (dx, dy) in [(-4.0, 0.0), (4.0, 0.0), (-3.0, 1.0), (3.0, 1.0)] {
            put_float(buf, cx + dx, top + dy, "°", Style::default().fg(Color::LightCyan));
        }
    } else {
        // Pop back up out of the prop.
        let p = (t - 0.58) / 0.42;
        let y = top - 1.0 - (p * std::f32::consts::PI).sin() * 4.0;
        put_float(buf, land_x, y, mini, Style::default().fg(color));
    }
}

// ---------------------------------------------------------------------------
// Fractions / Percentages — a shape that materialises
// ---------------------------------------------------------------------------

/// A visual shape card, shared by the Fractions and Percentages sections.  The
/// figure and question come from the problem; the answer area and footer adapt
/// to its [`Mode`] — a stacked `a/b` for fractions, a single `n%` for percents.
pub fn render_shape_card(area: Rect, buf: &mut Buffer, p: &crate::fraction::FractionProblem, input: &str, progress: f32, banner: Option<(String, Color)>) {
    use crate::fraction::Mode;
    let title = match p.mode {
        Mode::Fraction => " Fractions ",
        Mode::Percent => " Percentages ",
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(rat(p.shaded_color)))
        .title(title)
        .style(Style::default().bg(BG));
    let inner = block.inner(area);
    block.render(area, buf);

    // Question.
    let q = p.question();
    let mut y = inner.top() + 1;
    for line in wrap_text(&q, inner.width.saturating_sub(4)).iter().take(2) {
        put_str(buf, center(inner, line.chars().count() as u16), y, line, Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
        y += 1;
    }

    // The materialising shape occupies the middle band.
    let answer_h = 5; // "Your answer:" + the (stacked or inline) answer
    let shape_area = Rect {
        x: inner.left() + 2,
        y: y + 1,
        width: inner.width.saturating_sub(4),
        height: inner.bottom().saturating_sub(y + 1 + answer_h + 2),
    };
    if shape_area.height >= 3 {
        crate::shapes::render(buf, shape_area, p.shape, progress, |i| rat(p.color_of(i)));
    }

    let ay = inner.bottom().saturating_sub(answer_h + 1);
    put_str(buf, center(inner, 12), ay, "Your answer:", Style::default().fg(Color::Gray));
    let style = Style::default().fg(Color::White).add_modifier(Modifier::BOLD);
    match p.mode {
        Mode::Fraction => {
            // Stacked as a fraction: numerator over a bar over the denominator.
            let (num, den) = match input.split_once('/') {
                Some((a, b)) => (if a.is_empty() { "?" } else { a }, if b.is_empty() { "?" } else { b }),
                None => (if input.is_empty() { "?" } else { input }, "?"),
            };
            let bar_w = num.chars().count().max(den.chars().count()).max(1) as u16 + 2;
            put_str(buf, center(inner, num.chars().count() as u16), ay + 1, num, style);
            put_str(buf, center(inner, bar_w), ay + 2, "─".repeat(bar_w as usize), Style::default().fg(rat(p.shaded_color)));
            put_str(buf, center(inner, den.chars().count() as u16), ay + 3, den, style);
        }
        Mode::Percent => {
            // A single whole number with a percent sign.
            let shown = if input.is_empty() { "?".to_string() } else { format!("{}%", input) };
            put_str(buf, center(inner, shown.chars().count() as u16), ay + 2, &shown, style);
        }
    }

    if let Some((text, color)) = banner {
        put_str(buf, center(inner, text.chars().count() as u16), ay + 4, &text, Style::default().fg(color).add_modifier(Modifier::BOLD));
    }

    let hints = match p.mode {
        Mode::Fraction => "Type like 2/4    Enter check    H help duck    Esc menu",
        Mode::Percent => "Type a number like 25    Enter check    H help duck    Esc menu",
    };
    put_str(buf, center(inner, hints.chars().count() as u16), inner.bottom().saturating_sub(1), hints, Style::default().fg(Color::DarkGray));
}

// ---------------------------------------------------------------------------
// Arithmetic — horizontal, stacked column, and long-division house
// ---------------------------------------------------------------------------

/// Draw a single problem card into `buf`.  Used live and for transition capture.
pub fn render_card(area: Rect, buf: &mut Buffer, p: &Problem, input: &str, layout: Layout, banner: Option<(String, Color)>) {
    // Bordered panel tinted with the problem's accent colour.
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(rat(p.accent)))
        .style(Style::default().bg(BG));
    let inner = block.inner(area);
    block.render(area, buf);

    // Division gets the long-division "house" when vertical; the other three
    // operations stack in columns; horizontal keeps everything on one line.
    match layout {
        Layout::Vertical if p.op == Op::Div => draw_division_house(inner, buf, p, input),
        Layout::Vertical => draw_vertical(inner, buf, p, input),
        _ => draw_horizontal(inner, buf, p, input),
    }

    // Feedback banner and footer hints share a fixed spot so the two layouts
    // never collide with them.
    if let Some((text, color)) = banner {
        put_str(
            buf,
            center(inner, text.chars().count() as u16),
            inner.bottom().saturating_sub(3),
            text,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        );
    }
    let hints = "Enter check   H help duck   Y why?   V view   Esc menu";
    put_str(
        buf,
        center(inner, hints.chars().count() as u16),
        inner.bottom().saturating_sub(1),
        hints,
        Style::default().fg(Color::DarkGray),
    );
}

/// Style for the answer text: bold white once typed, dim while it's still `?`.
fn answer_style(input: &str) -> Style {
    if input.is_empty() {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
    }
}

/// `a op b = answer` on one line, the answer sitting to the right of the `=`.
fn draw_horizontal(inner: Rect, buf: &mut Buffer, p: &Problem, input: &str) {
    let prompt = p.prompt(); // "a op b ="
    let answer = if input.is_empty() { "?" } else { input };
    let gap = font::GLYPH_W + font::GLYPH_GAP;
    let pw = font::text_width(&prompt);
    let aw = font::text_width(answer);
    let total = pw + gap + aw;

    let x = center(inner, total);
    let y = inner.top() + inner.height.saturating_sub(font::GLYPH_H) / 2;
    font::draw_text(buf, x, y, &prompt, Style::default().fg(rat(p.accent)));
    font::draw_text(buf, x + pw + gap, y, answer, answer_style(input));
}

/// The stacked column form, with digits right-aligned and the answer below the
/// line — the way a student writes it out by hand.
fn draw_vertical(inner: Rect, buf: &mut Buffer, p: &Problem, input: &str) {
    let a_str = p.a.to_string();
    let b_str = p.b.to_string();
    let ans = if input.is_empty() { "?".to_string() } else { input.to_string() };

    let field_digits = a_str.chars().count().max(b_str.chars().count()).max(ans.chars().count()) as u16;
    let field_px = field_digits * font::GLYPH_W + field_digits.saturating_sub(1) * font::GLYPH_GAP;
    let op_col = font::GLYPH_W + font::GLYPH_GAP; // operator sits to the left
    let total_w = op_col + field_px;

    let x0 = center(inner, total_w);
    let field_right = x0 + total_w;

    // Three glyph rows + a one-row divider, with single-row gaps.
    let total_h = 3 * font::GLYPH_H + 3;
    let top = inner.top() + inner.height.saturating_sub(total_h) / 2;

    let accent = Style::default().fg(rat(p.accent));
    let right = |buf: &mut Buffer, s: &str, y: u16, style: Style| {
        let x = field_right.saturating_sub(font::text_width(s));
        font::draw_text(buf, x, y, s, style);
    };

    // Top operand.
    let y1 = top;
    right(buf, &a_str, y1, accent);

    // Operator + second operand.
    let y2 = y1 + font::GLYPH_H + 1;
    font::draw_text(buf, x0, y2, &p.op.symbol().to_string(), accent);
    right(buf, &b_str, y2, accent);

    // Divider line spanning the operand field.
    let y_div = y2 + font::GLYPH_H;
    if y_div < inner.bottom() {
        for x in x0..field_right.min(inner.right()) {
            buf[(x, y_div)].set_symbol("─").set_fg(rat(p.accent));
        }
    }

    // Answer below the line.
    let y3 = y_div + 1;
    right(buf, &ans, y3, answer_style(input));
}

/// Long-division "house": quotient on the roof, divisor outside the wall,
/// dividend inside.  `p.a` is the dividend, `p.b` the divisor.
fn draw_division_house(inner: Rect, buf: &mut Buffer, p: &Problem, input: &str) {
    let dividend = p.a.to_string();
    let divisor = p.b.to_string();
    let quotient = if input.is_empty() { "?".to_string() } else { input.to_string() };

    let wd = font::text_width(&dividend);
    let wq = font::text_width(&quotient);
    let wv = font::text_width(&divisor);

    // Horizontal anchors: "divisor | dividend", quotient right-aligned over it.
    let total_w = wv + 1 + 1 + 2 + wd;
    let x0 = center(inner, total_w);
    let wall_x = x0 + wv + 1;
    let dividend_x = wall_x + 2;
    let house_right = (dividend_x + wd).min(inner.right());
    let quotient_x = dividend_x + wd.saturating_sub(wq);

    // Vertical anchors: quotient row, roof line, dividend row.
    let total_h = 2 * font::GLYPH_H + 1;
    let top = inner.top() + inner.height.saturating_sub(total_h) / 2;
    let q_y = top;
    let roof_y = top + font::GLYPH_H;
    let dvd_y = roof_y + 1;

    let accent = Style::default().fg(rat(p.accent));

    // Quotient (the answer) sits above the roof, over the dividend.
    font::draw_text(buf, quotient_x, q_y, &quotient, answer_style(input));

    // Roof: corner at the wall, bar across the dividend.
    if roof_y < inner.bottom() {
        buf[(wall_x.min(inner.right().saturating_sub(1)), roof_y)].set_symbol("┌").set_fg(rat(p.accent));
        for x in (wall_x + 1)..house_right {
            buf[(x, roof_y)].set_symbol("─").set_fg(rat(p.accent));
        }
    }
    // Wall down the left of the dividend.
    for y in (roof_y + 1)..(dvd_y + font::GLYPH_H).min(inner.bottom()) {
        buf[(wall_x.min(inner.right().saturating_sub(1)), y)].set_symbol("│").set_fg(rat(p.accent));
    }

    // Divisor outside the wall, dividend inside.
    font::draw_text(buf, x0, dvd_y, &divisor, accent);
    font::draw_text(buf, dividend_x, dvd_y, &dividend, accent);
}

// ---------------------------------------------------------------------------
// Graphing
// ---------------------------------------------------------------------------

const GRAPH_ACCENT: Color = Color::LightMagenta;
const GRAPH_BAR: Color = Color::LightBlue;
const GRAPH_CMP_A: Color = Color::LightCyan;
const GRAPH_CMP_B: Color = Color::LightYellow;

/// ASCII bar chart drawn into `band` (y grows down, bar grows up).
/// `numbered`: when true, prefixes each label with its 1-based index so students
/// can enter the number directly for FindMax / FindMin questions.
fn draw_ascii_bar(buf: &mut Buffer, band: Rect, cats: &[String], values: &[i32], accent: Color, numbered: bool) {
    if band.height < 3 || band.width < 4 || cats.is_empty() { return; }
    let n = cats.len();
    let max_v = values.iter().copied().max().unwrap_or(1).max(1);
    // chart_h: rows available for bars; -2 for baseline row + label row
    let chart_h = band.height.saturating_sub(2);
    let col_w   = (band.width / n as u16).max(2);
    let bar_w   = col_w.saturating_sub(1).max(1); // leave 1-char gap between bars

    // Baseline across the full band width
    for x in band.left()..band.right() {
        buf[(x, band.top() + chart_h)].set_char('─').set_style(Style::default().fg(Color::Gray));
    }

    let st = Style::default().fg(accent);
    for (i, (cat, &val)) in cats.iter().zip(values.iter()).enumerate() {
        let bar_h = ((val as f32 / max_v as f32) * chart_h as f32).round() as u16;
        let bx    = band.left() + i as u16 * col_w;
        if bx >= band.right() { break; }

        // Draw filled bar (bar_w columns wide)
        for col in 0..bar_w {
            let cx = bx + col;
            if cx >= band.right() { break; }
            for row in 0..chart_h {
                if chart_h - 1 - row < bar_h {
                    buf[(cx, band.top() + row)].set_char('█').set_style(st);
                }
            }
        }

        // Tick on baseline at bar center
        let center_x = bx + bar_w / 2;
        if center_x < band.right() {
            buf[(center_x, band.top() + chart_h)].set_char('┼').set_style(Style::default().fg(Color::Gray));
        }

        // Value label just above the bar top, centered in the bar
        let val_str = val.to_string();
        let v_len   = val_str.len() as u16;
        let v_x     = bx + bar_w / 2 - v_len / 2;
        let bar_top_y = band.top() + chart_h.saturating_sub(bar_h);
        let v_y = if bar_top_y > band.top() { bar_top_y - 1 } else { band.top() };
        if v_x < band.right() {
            put_str(buf, v_x, v_y, &val_str, Style::default().fg(Color::White));
        }

        // Category label below baseline, centered in the column, truncated to col_w
        let label = if numbered {
            let prefix = format!("{}.", i + 1);
            let remaining = col_w as usize - prefix.len().min(col_w as usize);
            format!("{}{}", prefix, cat.chars().take(remaining).collect::<String>())
        } else {
            cat.chars().take(col_w as usize).collect()
        };
        let l_len = label.chars().count() as u16;
        let l_x   = bx + bar_w / 2 - l_len / 2;
        let l_y   = band.top() + chart_h + 1;
        if l_y < band.bottom() && l_x < band.right() {
            put_str(buf, l_x, l_y, &label, Style::default().fg(Color::Gray));
        }
    }
}

// Per-slice colour palette used for both pie circle and legend.
const PIE_COLORS: [Color; 6] = [
    Color::LightCyan, Color::LightYellow, Color::LightGreen,
    Color::LightMagenta, Color::LightRed, Color::LightBlue,
];

/// Draw an ASCII circle pie chart with a colour-coded legend to the right.
///
/// Circle width ≈ 2 × height (character aspect ratio correction).
/// Each cell's fill colour is determined by its angle from the centre.
pub fn draw_ascii_pie(buf: &mut Buffer, band: Rect, cats: &[String], values: &[i32]) {
    if band.height < 4 || band.width < 10 || cats.is_empty() { return; }

    let n      = cats.len();
    let total  = values.iter().sum::<i32>().max(1);

    // Cumulative fractions [0.0..1.0], 12 o'clock = 0, clockwise.
    let mut cum = vec![0.0f64];
    for &v in values {
        let last = *cum.last().unwrap();
        cum.push(last + v as f64 / total as f64);
    }

    // Circle: height = band.height, width ≈ 2× height so it looks round.
    let circle_h = band.height as usize;
    let circle_w = (circle_h * 2).min(band.width as usize / 2 + 2).min(band.width as usize);
    let cx = (circle_w as f64 - 1.0) / 2.0;
    let cy = (circle_h as f64 - 1.0) / 2.0;

    for row in 0..circle_h {
        for col in 0..circle_w {
            let bx = band.left() + col as u16;
            let by = band.top()  + row as u16;
            if bx >= band.right() || by >= band.bottom() { continue; }

            // Normalised coords (–1..1); both axes scaled identically → round circle.
            let nx = if cx > 0.0 { (col as f64 - cx) / cx } else { 0.0 };
            let ny = if cy > 0.0 { (row as f64 - cy) / cy } else { 0.0 };

            if nx * nx + ny * ny > 1.0 { continue; } // outside circle

            // Angle from 12 o'clock, clockwise (screen y increases downward).
            use std::f64::consts::{FRAC_PI_2, PI};
            let raw  = ny.atan2(nx); // –π..π, 0 = right (3 o'clock)
            let frac = ((raw + FRAC_PI_2) / (2.0 * PI) + 1.0) % 1.0;

            // Which slice?
            let idx = cum.partition_point(|&c| c <= frac).saturating_sub(1).min(n - 1);
            buf[(bx, by)].set_char('█').set_style(Style::default().fg(PIE_COLORS[idx % PIE_COLORS.len()]));
        }
    }

    // Legend to the right of the circle.
    let legend_x = band.left() + circle_w as u16 + 1;
    for (i, (cat, &val)) in cats.iter().zip(values.iter()).enumerate() {
        let pct   = (val as f64 / total as f64 * 100.0).round() as i32;
        let color = PIE_COLORS[i % PIE_COLORS.len()];
        let ly    = band.top() + i as u16;
        if ly >= band.bottom() || legend_x + 1 >= band.right() { break; }

        buf[(legend_x, ly)].set_char('█').set_style(Style::default().fg(color));
        if legend_x + 1 < band.right() {
            buf[(legend_x + 1, ly)].set_char('█').set_style(Style::default().fg(color));
        }
        let label  = format!(" {}. {} {}%", i + 1, cat, pct);
        let avail  = band.right().saturating_sub(legend_x + 2) as usize;
        let text: String = label.chars().take(avail).collect();
        put_str(buf, legend_x + 2, ly, &text, Style::default().fg(Color::White));
    }
}

/// Draw an ASCII coordinate plane with all four quadrants into `band`.
/// `cats` supplies the x-value labels ("1","2"…); `values[i]` is the y-value
/// at that x.  `selected` is a 1-based data-point index to highlight (None in
/// card / read-only mode).
pub fn draw_ascii_coord(
    buf:      &mut Buffer,
    band:     Rect,
    cats:     &[String],
    values:   &[i32],
    selected: Option<usize>,
) {
    if band.height < 5 || band.width < 10 || cats.is_empty() { return; }

    let n     = cats.len();
    let max_y = values.iter().copied().max().unwrap_or(1).max(1);

    // Y-axis column: leave 4 chars on the left for numeric labels.
    let y_label_w: u16 = 4;
    let y_ax = band.left() + y_label_w;

    // X-axis row: put it 3/4 down so there is a small negative-y strip below.
    let neg_rows: u16 = (band.height / 4).max(1);
    let x_ax  = band.bottom().saturating_sub(neg_rows + 1);

    let pos_rows = x_ax.saturating_sub(band.top()).max(1);
    let pos_cols = band.right().saturating_sub(y_ax + 1).max(1);

    let row_per_y = pos_rows as f32 / (max_y as f32 + 0.5);
    let col_per_x = pos_cols as f32 / (n as f32 + 0.5);

    let to_col = |xi: usize| -> u16 { y_ax + ((xi as f32 + 0.5) * col_per_x).round() as u16 };
    let to_row = |yi: i32|  -> u16  { x_ax.saturating_sub((yi as f32 * row_per_y).round() as u16) };

    // Y-axis
    for row in band.top()..band.bottom() {
        if y_ax >= band.right() { break; }
        let ch = if row == x_ax { '┼' } else { '│' };
        buf[(y_ax, row)].set_char(ch).set_style(Style::default().fg(Color::Gray));
    }
    if y_ax < band.right() {
        buf[(y_ax, band.top())].set_char('↑').set_style(Style::default().fg(Color::Gray));
    }

    // X-axis
    if x_ax < band.bottom() {
        for col in band.left()..band.right() {
            if col == y_ax { continue; }
            buf[(col, x_ax)].set_char('─').set_style(Style::default().fg(Color::Gray));
        }
        if band.right() > 0 {
            buf[(band.right() - 1, x_ax)].set_char('→').set_style(Style::default().fg(Color::Gray));
        }
    }

    // "0" near origin
    if y_ax + 1 < band.right() && x_ax + 1 < band.bottom() {
        put_str(buf, y_ax + 1, x_ax + 1, "0", Style::default().fg(Color::DarkGray));
    }

    // Y-axis tick marks and numeric labels
    let y_step = if max_y > 20 { 5i32 } else if max_y > 10 { 2 } else { 1 };
    let mut yi = y_step;
    while yi <= max_y + y_step {
        let row = to_row(yi);
        if row < band.top() || row >= x_ax { break; }
        if y_ax < band.right() {
            buf[(y_ax, row)].set_char('├').set_style(Style::default().fg(Color::Gray));
        }
        let label = format!("{:>3}", yi);
        put_str(buf, band.left(), row, &label, Style::default().fg(Color::DarkGray));
        yi += y_step;
    }
    // Show "-1" below the x-axis if there is room
    let neg1_row = x_ax + (row_per_y.max(1.0)).round() as u16;
    if neg1_row < band.bottom().saturating_sub(1) {
        if y_ax < band.right() {
            buf[(y_ax, neg1_row)].set_char('├').set_style(Style::default().fg(Color::Gray));
        }
        put_str(buf, band.left(), neg1_row, " -1", Style::default().fg(Color::DarkGray));
    }

    // X-axis tick marks and labels (below axis)
    for (i, cat) in cats.iter().enumerate() {
        let col = to_col(i);
        if col >= band.right().saturating_sub(1) { break; }
        if x_ax < band.bottom() {
            buf[(col, x_ax)].set_char('┬').set_style(Style::default().fg(Color::Gray));
            if x_ax + 1 < band.bottom() {
                put_str(buf, col, x_ax + 1, cat, Style::default().fg(Color::DarkGray));
            }
        }
    }

    // Quadrant labels (very dim)
    let ql = Style::default().fg(Color::DarkGray);
    let qr = (band.top() + x_ax) / 2;   // mid row of positive y
    let qnr = (x_ax + band.bottom()) / 2; // mid row of negative y
    let qc  = (y_ax + band.right()) / 2;  // mid col of positive x
    let qnc = (band.left() + y_ax) / 2;   // mid col of negative x
    for &(lbl, col, row) in &[("I", qc, qr), ("II", qnc, qr), ("III", qnc, qnr), ("IV", qc, qnr)] {
        if col < band.right() && row > band.top() && row < band.bottom() {
            put_str(buf, col, row, lbl, ql);
        }
    }

    // Data points
    for (i, (&val, cat)) in values.iter().zip(cats.iter()).enumerate() {
        let col = to_col(i);
        let row = to_row(val);
        if col >= band.right() || row < band.top() || row >= band.bottom() { continue; }

        let is_sel = selected.map_or(false, |s| s == i + 1);
        let dot_st = if is_sel {
            Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::LightBlue)
        };
        buf[(col, row)].set_char('●').set_style(dot_st);

        // "(x,y)" label — prefer to the right, fall back to the left
        let xi  = i + 1;
        let lbl = format!("({},{})", xi, val);
        let ll  = lbl.len() as u16;
        let lbl_col = if col + 1 + ll < band.right() { col + 1 }
                      else { col.saturating_sub(ll) };
        let lbl_st = Style::default().fg(if is_sel { Color::White } else { Color::Gray });
        if lbl_col < band.right() {
            put_str(buf, lbl_col, row, &lbl, lbl_st);
        }
    }
}

/// Draw a single graphing problem card.
pub fn render_graph_card(area: Rect, buf: &mut Buffer, p: &crate::graphing::GraphProblem, input: &str, banner: Option<(String, Color)>) {
    use crate::graphing::GraphKind;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(GRAPH_ACCENT))
        .title(format!(" {} — {} ", p.kind.name(), p.data.title))
        .style(Style::default().bg(BG));
    let inner = block.inner(area);
    block.render(area, buf);

    let mut y = inner.top() + 1;

    // Question.
    let q = p.question_text();
    for line in wrap_text(&q, inner.width.saturating_sub(4)).iter().take(2) {
        put_str(buf, center(inner, line.chars().count() as u16), y,
            line, Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
        y += 1;
    }
    y += 1;

    // Chart area — allocate middle band.
    let answer_h: u16 = 3;
    let hint_h: u16 = 1;
    let chart_h_avail = inner.bottom().saturating_sub(y + answer_h + hint_h + 1);
    let band = Rect { x: inner.left() + 1, y, width: inner.width.saturating_sub(2), height: chart_h_avail };

    match &p.kind {
        GraphKind::Pictograph { scale } => {
            // Numbered, fixed-width table: "1. label │ * * *   count"
            let num_w   = (p.data.categories.len() as f32).log10() as usize + 2; // "1. " … "9. "
            let max_lbl = p.data.categories.iter().map(|c| c.chars().count()).max().unwrap_or(1);
            let col_w   = num_w + max_lbl;
            let sym_avail = (band.width as usize).saturating_sub(col_w + 6);
            let max_syms  = (sym_avail / 2).max(1);
            for (i, (cat, &val)) in p.data.categories.iter().zip(p.data.values.iter()).enumerate() {
                let label    = format!("{}. {:>width$}", i + 1, cat, width = max_lbl);
                let symbols  = (val / (*scale as i32).max(1)).max(0) as usize;
                let sym_str  = vec!["*"; symbols.min(max_syms)].join(" ");
                let row = format!("{} │ {:<sym_w$}  {}", label, sym_str, val, sym_w = max_syms * 2);
                if band.top() + (i as u16) < band.bottom() {
                    put_str(buf, band.left(), band.top() + (i as u16), &row, Style::default().fg(GRAPH_BAR));
                }
            }
            let note_y = band.top() + p.data.categories.len() as u16 + 1;
            if note_y < band.bottom() {
                let unit = if *scale == 1 { "1 unit".to_string() } else { format!("{} units", scale) };
                put_str(buf, band.left(), note_y, &format!("(* = {})", unit), Style::default().fg(Color::DarkGray));
            }
        }
        GraphKind::Pie => {
            let n_cats = p.data.categories.len();
            // Upper portion: ASCII circle (up to 12 rows, at least as many as n_cats).
            let circle_rows = band.height.min(12).max(n_cats as u16);
            let circle_band = Rect { height: circle_rows.min(band.height), ..band };
            draw_ascii_pie(buf, circle_band, &p.data.categories, &p.data.values);

            // Lower portion: horizontal proportion bars, if there is room.
            let bars_y = band.top() + circle_rows + 1;
            if bars_y + n_cats as u16 <= band.bottom() {
                let total = p.data.values.iter().sum::<i32>().max(1);
                let max_lbl   = p.data.categories.iter().map(|c| c.chars().count()).max().unwrap_or(1);
                let bar_avail = (band.width as usize).saturating_sub(max_lbl + 9);
                for (i, (cat, &pct)) in p.data.categories.iter().zip(p.data.values.iter()).enumerate() {
                    let label   = format!("{:>width$}", cat, width = max_lbl);
                    let bar_len = ((pct as f32 / total as f32) * bar_avail as f32).round() as usize;
                    let row_str = format!("{} │ {:<bar_w$}  {:>3}%",
                        label, "█".repeat(bar_len), pct, bar_w = bar_avail);
                    let ly = bars_y + i as u16;
                    if ly < band.bottom() {
                        put_str(buf, band.left(), ly, &row_str, Style::default().fg(GRAPH_BAR));
                    }
                }
            }
        }
        GraphKind::Coordinate => {
            draw_ascii_coord(buf, band, &p.data.categories, &p.data.values, None);
        }
        _ => {
            // Bar / Line — always number bars so the category index is visible
            draw_ascii_bar(buf, band, &p.data.categories, &p.data.values, GRAPH_BAR, true);
        }
    }

    // Answer input.
    let ay = inner.bottom().saturating_sub(answer_h + 1);
    put_str(buf, center(inner, 14), ay, "Your answer:", Style::default().fg(Color::Gray));
    let num = if input.is_empty() { "?" } else { input };
    put_str(buf, center(inner, num.chars().count() as u16), ay + 1, num,
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD));

    if let Some((text, color)) = banner {
        put_str(buf, center(inner, text.chars().count() as u16), ay + 2, &text,
            Style::default().fg(color).add_modifier(Modifier::BOLD));
    }

    let hints = "Enter check    H help    Y why?    Esc menu";
    put_str(buf, center(inner, hints.chars().count() as u16), inner.bottom().saturating_sub(1),
        hints, Style::default().fg(Color::DarkGray));
}

/// Draw a two-graph comparison card (side by side bar charts + question).
pub fn render_graph_cmp_card(area: Rect, buf: &mut Buffer, p: &crate::graphing::GraphCmpProblem, input: &str, banner: Option<(String, Color)>) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(GRAPH_ACCENT))
        .title(" Graph Comparison ")
        .style(Style::default().bg(BG));
    let inner = block.inner(area);
    block.render(area, buf);

    let mut y = inner.top() + 1;

    // Question.
    let q = p.question_text();
    for line in wrap_text(&q, inner.width.saturating_sub(4)).iter().take(2) {
        put_str(buf, center(inner, line.chars().count() as u16), y,
            line, Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
        y += 1;
    }
    y += 1;

    // Two side-by-side bar charts.
    let answer_h: u16 = 3;
    let chart_h_avail = inner.bottom().saturating_sub(y + answer_h + 2);
    let half_w = inner.width / 2;

    let band_a = Rect { x: inner.left(), y, width: half_w.saturating_sub(1), height: chart_h_avail };
    let band_b = Rect { x: inner.left() + half_w, y, width: half_w, height: chart_h_avail };

    // Group 1 header
    put_str(buf, band_a.left(), band_a.top(), "Group 1", Style::default().fg(GRAPH_CMP_A).add_modifier(Modifier::BOLD));
    let sub_a = Rect { y: band_a.top() + 1, height: band_a.height.saturating_sub(1), ..band_a };
    draw_ascii_bar(buf, sub_a, &p.data_a.categories, &p.data_a.values, GRAPH_CMP_A, false);

    // Group 2 header
    put_str(buf, band_b.left(), band_b.top(), "Group 2", Style::default().fg(GRAPH_CMP_B).add_modifier(Modifier::BOLD));
    let sub_b = Rect { y: band_b.top() + 1, height: band_b.height.saturating_sub(1), ..band_b };
    draw_ascii_bar(buf, sub_b, &p.data_b.categories, &p.data_b.values, GRAPH_CMP_B, false);

    // Divider
    for ry in y..y + chart_h_avail + 1 {
        if ry < inner.bottom() {
            buf[(inner.left() + half_w.saturating_sub(1), ry)].set_char('│').set_style(Style::default().fg(Color::DarkGray));
        }
    }

    // Totals beneath charts
    let tot_a: i32 = p.data_a.values.iter().sum();
    let tot_b: i32 = p.data_b.values.iter().sum();
    let tot_y = y + chart_h_avail + 1;
    if tot_y < inner.bottom().saturating_sub(answer_h + 1) {
        put_str(buf, band_a.left(), tot_y, &format!("Total: {}", tot_a), Style::default().fg(GRAPH_CMP_A));
        put_str(buf, band_b.left(), tot_y, &format!("Total: {}", tot_b), Style::default().fg(GRAPH_CMP_B));
    }

    // Answer input.
    let ay = inner.bottom().saturating_sub(answer_h + 1);
    put_str(buf, center(inner, 14), ay, "Your answer:", Style::default().fg(Color::Gray));
    let num = if input.is_empty() { "?" } else { input };
    put_str(buf, center(inner, num.chars().count() as u16), ay + 1, num,
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD));

    if let Some((text, color)) = banner {
        put_str(buf, center(inner, text.chars().count() as u16), ay + 2, &text,
            Style::default().fg(color).add_modifier(Modifier::BOLD));
    }

    let hints = "Enter check    H help    Y why?    Esc menu";
    put_str(buf, center(inner, hints.chars().count() as u16), inner.bottom().saturating_sub(1),
        hints, Style::default().fg(Color::DarkGray));
}
