//! Deduction Duck's help stage: the panel that slides up over a problem when the
//! learner presses **H**, in which the duck acts out the chosen strategy — walking
//! a number line, smashing a number into its place-value pieces, or talking it
//! through as plain hint steps.  Hint steps show first; the answer only once revealed.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Widget};
use ratatui::Frame;

use crate::app::App;
use crate::duck::{self, Pose};
use crate::font;
use crate::problem::Problem;
use crate::strategy::{self, Strategy, Viz};

use super::*;

/// The duck's stage: a panel sliding up from the bottom in which he acts out
/// the chosen strategy — walking a number line, smashing a number, or talking
/// it through.  Hint steps show first; the answer appears only once revealed.
pub fn draw_help_overlay(f: &mut Frame, app: &App, area: Rect, problem: &Problem) {
    let strats = strategy::strategies(problem);
    let idx = app.strategy_index % strats.len();
    let s = &strats[idx];

    // Panel geometry; slides up from below as `help_in` eases 0..1.
    let panel_w = area.width.saturating_sub(4).clamp(20, 72).min(area.width);
    let panel_h = area.height.saturating_sub(2).clamp(9, 18).min(area.height);
    let rest_y = area.top() + area.height.saturating_sub(panel_h) / 2;
    let slide = ((1.0 - app.help_in.clamp(0.0, 1.0)) * (panel_h as f32 + 2.0)) as u16;
    let panel_x = area.left() + area.width.saturating_sub(panel_w) / 2;
    let panel_y = (rest_y + slide).min(area.bottom().saturating_sub(1));
    let panel = Rect {
        x: panel_x,
        y: panel_y,
        width: panel_w.min(area.right().saturating_sub(panel_x)),
        height: panel_h.min(area.bottom().saturating_sub(panel_y)),
    };
    if panel.width < 10 || panel.height < 5 {
        return;
    }

    let buf = f.buffer_mut();
    Clear.render(panel, buf);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::LightYellow))
        .title(" Deduction Duck ")
        .style(Style::default().bg(STAGE_BG));
    let inner = block.inner(panel);
    block.render(panel, buf);

    // The top line carries a peek reprimand (red) or, failing that, an
    // encouraging word (green) when the student is struggling.
    let enc_off: u16 = if let Some(msg) = &app.reprimand {
        put_str(buf, inner.left(), inner.top(), clip(msg, inner.width), Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD));
        1
    } else if let Some(msg) = &app.encourage {
        put_str(buf, inner.left(), inner.top(), clip(msg, inner.width), Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD));
        1
    } else {
        0
    };

    // Header and footer controls.
    let header = format!("Way {}/{}: {}", idx + 1, strats.len(), s.title);
    put_str(buf, inner.left(), inner.top() + enc_off, clip(&header, inner.width), Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD));

    let reveal_ctl = if app.reveal_locked() {
        "R: locked!"
    } else if app.revealed {
        "R: hide answer"
    } else {
        "R: show answer"
    };
    let mut controls = vec![reveal_ctl.to_string()];
    if strats.len() > 1 {
        controls.push("Space: another way".to_string());
    }
    controls.push("H: shoo".to_string());
    let footer = controls.join("    ");
    put_str(buf, inner.left(), inner.bottom().saturating_sub(1), clip(&footer, inner.width), Style::default().fg(Color::Gray));

    // Content sits between header and footer.
    let content = Rect {
        x: inner.left(),
        y: inner.top() + 2 + enc_off,
        width: inner.width,
        height: inner.height.saturating_sub(3 + enc_off),
    };
    if content.height == 0 {
        return;
    }

    let accent = rat(problem.accent);
    let drawn = match &s.viz {
        Viz::NumberLine { stops, hops } => draw_number_line(buf, content, s, stops, hops, app, accent),
        Viz::Smash { value, parts } => draw_smash(buf, content, s, *value, parts, app, accent),
        Viz::Lines => false,
    };
    if !drawn {
        draw_lines(buf, content, s, app);
    }
}

/// Fallback / plain view: hint steps on the left, duck standing on the right.
fn draw_lines(buf: &mut Buffer, content: Rect, s: &Strategy, app: &App) {
    let reveal_start = s.steps.len() + 1;
    let mut lines: Vec<String> = s.steps.clone();
    if app.revealed {
        lines.push(String::new());
        lines.extend(s.reveal.iter().cloned());
    }

    let duck_x = content.right().saturating_sub(duck::WIDTH + 1);
    let text_w = duck_x.saturating_sub(content.left() + 1);

    let mut y = content.top();
    for (i, l) in lines.iter().enumerate() {
        if y >= content.bottom() {
            break;
        }
        let style = if i >= reveal_start {
            Style::default().fg(Color::LightGreen)
        } else {
            Style::default().fg(Color::White)
        };
        put_str(buf, content.left(), y, clip(l, text_w), style);
        y += 1;
    }

    let duck_y = content.top() + content.height.saturating_sub(duck::HEIGHT) / 2;
    duck::draw_pose(buf, duck_x, duck_y, Pose::Stand, app.anim_frame / 6, Style::default().fg(DUCK_COLOR));
}

/// The duck walks a number line, hopping stop to stop.  Returns false (so the
/// caller falls back to [`draw_lines`]) when it will not fit.
fn draw_number_line(buf: &mut Buffer, content: Rect, s: &Strategy, stops: &[i64], hops: &[String], app: &App, accent: Color) -> bool {
    let n = stops.len();
    if n < 2 || content.height < 12 {
        return false;
    }
    let labels: Vec<String> = stops.iter().map(|v| v.to_string()).collect();
    let maxlabel = labels.iter().map(|l| l.len()).max().unwrap_or(1) as u16;
    let margin = maxlabel / 2 + 1;
    let ax0 = content.left() + margin;
    let ax1 = content.right().saturating_sub(margin + 1);
    if ax1 <= ax0 {
        return false;
    }
    let seg = (ax1 - ax0) as f32 / (n - 1) as f32;
    if seg < (maxlabel + 2) as f32 {
        return false;
    }
    let x_at = |i: usize| -> u16 { ax0 + (i as f32 * seg).round() as u16 };

    let axis_y = content.bottom().saturating_sub(2);
    let label_y = axis_y + 1;
    let hop_y = axis_y.saturating_sub(1);

    // Subtitle (hint) and, once revealed, the worked hops.
    put_str(buf, content.left(), content.top(), clip(&s.steps[0], content.width), Style::default().fg(Color::White));
    if app.revealed {
        let mut ry = content.top() + 1;
        for l in &s.reveal {
            put_str(buf, content.left(), ry, clip(l, content.width), Style::default().fg(Color::LightGreen));
            ry += 1;
        }
    }

    // Axis, ticks, labels.
    for x in ax0..=ax1.min(content.right().saturating_sub(1)) {
        buf[(x, axis_y)].set_symbol("─").set_fg(Color::DarkGray);
    }
    for (i, lbl) in labels.iter().enumerate().take(n) {
        let x = x_at(i).min(content.right().saturating_sub(1));
        buf[(x, axis_y)].set_symbol("┼").set_fg(accent);
        let lx = x.saturating_sub(lbl.len() as u16 / 2);
        let lstyle = if app.revealed && i == n - 1 {
            Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        put_str(buf, lx, label_y, lbl, lstyle);
    }
    // Hop labels above each segment.
    for (i, h) in hops.iter().enumerate().take(n - 1) {
        let xm = (x_at(i) + x_at(i + 1)) / 2;
        put_str(buf, xm.saturating_sub(h.len() as u16 / 2), hop_y, h, Style::default().fg(Color::LightGreen));
    }

    // Walking duck: travel each segment, dwelling on the final stop.  A gentle
    // pace (one hop roughly every two-thirds of a second) keeps it calm rather
    // than distracting, plus a short pause on the final stop before looping.
    const TICKS_PER_HOP: u64 = 20;
    let pos = (app.anim_frame / TICKS_PER_HOP) as f32 % (n as f32 + 1.0);
    let segi = pos.floor() as usize;
    let (dx, lift) = if segi >= n - 1 {
        (x_at(n - 1) as f32, 0.0)
    } else {
        let frac = pos - segi as f32;
        let xa = x_at(segi) as f32;
        let xb = x_at(segi + 1) as f32;
        (xa + frac * (xb - xa), (frac * std::f32::consts::PI).sin() * 2.0)
    };
    let duck_left = (dx.round() as i32 - duck::WIDTH as i32 / 2).max(content.left() as i32) as u16;
    let feet_y = hop_y.saturating_sub(1);
    let duck_top = ((feet_y as f32) - (duck::HEIGHT as f32 - 1.0) - lift).max(content.top() as f32 + 2.0) as u16;
    // Waddle only while actually moving (and slowly).
    let waddle = if segi >= n - 1 { 0 } else { app.anim_frame / 7 };
    duck::draw_pose(buf, duck_left, duck_top, Pose::WalkRight, waddle, Style::default().fg(DUCK_COLOR));
    true
}

/// The duck smashes a number apart into its place-value pieces.  Returns false
/// (falling back to [`draw_lines`]) when it will not fit.
fn draw_smash(buf: &mut Buffer, content: Rect, s: &Strategy, value: i64, parts: &[i64], app: &App, accent: Color) -> bool {
    let whole = value.to_string();
    let parts_str = parts.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(" + ");
    let ww = font::text_width(&whole);
    let pw = font::text_width(&parts_str);
    if ww.max(pw) + 2 > content.width || content.height < font::GLYPH_H + 4 {
        return false;
    }

    // Subtitle hint up top.
    put_str(buf, content.left(), content.top(), clip(&s.steps[0], content.width), Style::default().fg(Color::White));

    // Cycle: show the whole number (wind-up + chop), then the pieces.
    const PERIOD: u64 = 18;
    let show_parts = parts.len() > 1 && (app.anim_frame / PERIOD) % 2 == 1;
    let burst = parts.len() > 1 && (app.anim_frame % PERIOD) < 3;

    let num_y = content.top() + 1 + content.height.saturating_sub(font::GLYPH_H + 1) / 2;
    if show_parts {
        font::draw_text(buf, center(content, pw), num_y, &parts_str, Style::default().fg(accent));
    } else {
        font::draw_text(buf, center(content, ww), num_y, &whole, Style::default().fg(accent).add_modifier(Modifier::BOLD));
    }

    // Impact sparks at the moment of the smash.
    if burst {
        let cx = content.left() + content.width / 2;
        let cy = num_y + font::GLYPH_H / 2;
        for (dx, dy) in [(-3i16, -1i16), (3, -1), (-2, 1), (2, 1), (0, -2), (0, 2)] {
            let x = cx as i16 + dx;
            let y = cy as i16 + dy;
            if x >= 0 && y >= 0 {
                put_str(buf, x as u16, y as u16, "*", Style::default().fg(Color::LightYellow));
            }
        }
        put_str(buf, content.left(), num_y.saturating_sub(1), "POW!", Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD));
    }

    // Revealed working sits along the bottom of the content area.
    if app.revealed {
        let start = content.bottom().saturating_sub(s.reveal.len() as u16);
        for (i, l) in s.reveal.iter().enumerate() {
            put_str(buf, content.left(), start + i as u16, clip(l, content.width), Style::default().fg(Color::LightGreen));
        }
    }

    // The duck, chopping (whole) or recoiling (parts), on the left.
    let pose = if show_parts { Pose::Stand } else { Pose::Smash };
    let duck_y = (num_y + font::GLYPH_H).saturating_sub(duck::HEIGHT).max(content.top() + 1);
    duck::draw_pose(buf, content.left(), duck_y, pose, app.anim_frame / 3, Style::default().fg(DUCK_COLOR));
    true
}
