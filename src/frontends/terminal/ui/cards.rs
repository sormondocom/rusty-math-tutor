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
