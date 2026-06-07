//! Practice / Challenge session screens, the unit-explorer Experiment screen,
//! the Why overlay, Deduction Duck's hint panel, and the Challenge HUD + summary.
//! These are all the screens that appear once a student is actively working a
//! problem or exploring the lab.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Widget};
use ratatui::Frame;

use crate::app::{App, Feedback, Screen};
use crate::duck::{self, Pose};
use crate::font;
use crate::section::Active;

use super::*;

// ---------------------------------------------------------------------------
// Experimentation — free-form unit explorer
// ---------------------------------------------------------------------------

/// Deduction Duck only reacts when an experiment lands on something famous —
/// that way kids have to *induce* a reaction by trying interesting amounts.
enum ExpReaction {
    /// Most experiments: he stays out of it.
    None,
    /// The amount is about the size of a real-world thing.
    Match(String),
    /// Astronomically bigger than anything on the list — hat-explode time.
    Boom(String),
}

/// Decide the duck's reaction from the *physical* size in base units (so it's
/// the same whichever target unit is on screen) — deterministic, so a fun
/// discovery can be reproduced.
fn experiment_reaction(cat: crate::units::Category, base_value: f64) -> ExpReaction {
    let refs = crate::units::comparisons(cat);
    if refs.is_empty() || base_value <= 0.0 {
        return ExpReaction::None;
    }
    // Closest reference by ratio (log distance).
    let best = refs.iter().min_by(|a, b| {
        let da = (base_value / a.base).ln().abs();
        let db = (base_value / b.base).ln().abs();
        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
    });
    let largest = refs.last().unwrap();

    if let Some(r) = best {
        let ratio = base_value / r.base;
        if (0.55..=1.8).contains(&ratio) {
            let n = ratio.round() as i64;
            let fact = if n <= 1 {
                format!("That's about the same as {}!", r.one)
            } else {
                format!("That's about {} {}!", n, r.many)
            };
            return ExpReaction::Match(fact);
        }
    }
    if base_value >= largest.base * 2.0 {
        let n = base_value / largest.base;
        return ExpReaction::Boom(format!("That's about {} {}!", crate::units::format_amount(n), largest.many));
    }
    ExpReaction::None
}

pub fn draw_experiment(f: &mut Frame, app: &App, area: Rect) {
    use crate::units::{self, Category};

    let cat = Category::ALL[app.exp_category];
    let units_list = cat.units();
    let from = units_list[app.exp_from.min(units_list.len() - 1)];
    let to = units_list[app.exp_to.min(units_list.len() - 1)];
    let amount: f64 = app.exp_amount.parse().unwrap_or(0.0);
    let result = units::convert(amount, from, to);

    let buf = f.buffer_mut();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::LightMagenta))
        .title(" Experimentation — Unit Explorer ")
        .style(Style::default().bg(BG));
    let inner = block.inner(area);
    block.render(area, buf);

    put_str(buf, inner.left() + 2, inner.top() + 1, "Type any amount — even a silly one — and watch it convert!", Style::default().fg(Color::Gray));

    // Editable fields.
    let amount_display = if app.exp_amount.is_empty() { "0".to_string() } else { app.exp_amount.clone() };
    let caret = if app.exp_field == 0 { "_" } else { "" };
    let rows = [
        (0usize, "Amount".to_string(), format!("{}{}", amount_display, caret)),
        (1, "From".to_string(), format!("< {} >", from.plural)),
        (2, "To".to_string(), format!("< {} >", to.plural)),
        (3, "Category".to_string(), format!("< {} >", cat.name())),
    ];
    let col = inner.left() + 4;
    let mut y = inner.top() + 3;
    for (i, label, value) in &rows {
        let selected = *i == app.exp_field;
        let style = if selected {
            Style::default().fg(Color::Black).bg(Color::LightMagenta).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        put_str(buf, col, y, format!("{:<10}{}", label, value), style);
        y += 2;
    }

    // The result, shown large and friendly.
    let line = format!("= {} {}", units::format_amount(result), to.plural);
    put_str(buf, center(inner, line.chars().count() as u16), inner.top() + 10, &line, Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD));
    let recap = format!("{} {} is...", units::format_amount(amount), from.plural);
    put_str(buf, center(inner, recap.chars().count() as u16), inner.top() + 9, &recap, Style::default().fg(Color::Gray));

    // Deduction Duck only pops up (centre stage) for an interesting result —
    // the physical size, so the same landmark shows whichever unit is chosen.
    let base_value = amount * from.to_base;
    draw_exp_reaction(buf, inner, experiment_reaction(cat, base_value), app.anim_frame);

    put_str(buf, inner.left() + 2, inner.bottom().saturating_sub(1), "Up/Down field   Left/Right change   type digits   Esc menu", Style::default().fg(Color::DarkGray));
}

/// Draw Deduction Duck's reaction, centred near the bottom of the explorer.
fn draw_exp_reaction(buf: &mut Buffer, inner: Rect, reaction: ExpReaction, frame: u64) {
    let (fact, boom) = match reaction {
        ExpReaction::None => return, // he stays quiet — keep experimenting!
        ExpReaction::Match(fact) => (fact, false),
        ExpReaction::Boom(fact) => (fact, true),
    };

    // Centre stage, lower middle.  The duck stays put; its life comes from the
    // waddle frame so the fixed text above it never gets overrun.
    let duck_x = center(inner, duck::WIDTH);
    let duck_y = inner.bottom().saturating_sub(duck::HEIGHT + 1);

    // Boom needs an extra line above for the bigger headline.
    let head_row = duck_y.saturating_sub(if boom { 3 } else { 2 });
    let fact_row = duck_y.saturating_sub(if boom { 2 } else { 1 });

    let headline = if boom { "BOOM!" } else { "Whoa!" };
    let head_color = if boom { Color::LightRed } else { Color::LightYellow };
    put_str(buf, center(inner, headline.chars().count() as u16), head_row, headline, Style::default().fg(head_color).add_modifier(Modifier::BOLD));
    put_str(buf, center(inner, fact.chars().count() as u16), fact_row, &fact, Style::default().fg(head_color));

    duck::draw_pose(buf, duck_x, duck_y, Pose::Stand, frame / 5, Style::default().fg(DUCK_COLOR));

    if boom {
        // Blow the mortarboard off in a small ring around the head (kept clear
        // of the fact line two rows above).
        for dx in 1..8u16 {
            put_str(buf, duck_x + dx, duck_y, " ", Style::default());
            put_str(buf, duck_x + dx, duck_y + 1, " ", Style::default());
        }
        let cx = duck_x as f32 + 4.0;
        let cy = duck_y as f32;
        let r = 1.0 + (frame % 12) as f32 * 0.2;
        let pieces = ['*', '✦', '!', '+', '#', '°'];
        for (k, ch) in pieces.iter().enumerate() {
            let ang = k as f32 * std::f32::consts::TAU / pieces.len() as f32;
            put_float(buf, cx + ang.cos() * r, cy + ang.sin() * r * 0.4, &ch.to_string(), Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD));
        }
    } else if (frame / 5).is_multiple_of(2) {
        // A couple of sparkles of delight flanking the duck.
        put_str(buf, duck_x.saturating_sub(2), duck_y + 1, "✦", Style::default().fg(Color::LightYellow));
        put_str(buf, duck_x + duck::WIDTH + 1, duck_y + 1, "✦", Style::default().fg(Color::LightYellow));
    }
}

// ---------------------------------------------------------------------------
// Practice / Challenge session
// ---------------------------------------------------------------------------

/// The area the problem card occupies, given the screen and full frame.
/// Shared with transition capture so both line up on the same rect.
pub fn card_area(screen: Screen, area: Rect) -> Rect {
    if screen == Screen::Challenge {
        Rect { y: area.y + 2, height: area.height.saturating_sub(2), ..area }
    } else {
        area
    }
}

pub fn draw_session(f: &mut Frame, app: &App, fx: &Transitions, area: Rect) {
    // Reserve a HUD row at the top for challenge mode.
    let card = card_area(app.screen, area);
    if app.screen == Screen::Challenge {
        let hud = Rect { height: 2, ..area };
        draw_challenge_hud(f, app, hud);
    }

    // Card: either an in-flight transition (owned by the frontend) or the
    // current problem, dispatched on whichever section produced it.
    if let Some(t) = &fx.problem {
        t.render(card, f.buffer_mut());
    } else if app.challenge.as_ref().is_some_and(|c| c.finished) {
        draw_challenge_summary(f, app, card);
    } else {
        match &app.current {
            Active::Shape(s) => {
                let banner = match app.feedback {
                    Feedback::Correct => Some(("✓  Correct!".to_string(), Color::LightGreen)),
                    Feedback::Wrong => Some(("✗  Not quite — count again!".to_string(), Color::LightRed)),
                    Feedback::None => None,
                };
                render_shape_card(card, f.buffer_mut(), s, &app.input, app.frac_progress(), banner);
            }
            Active::Unit(p) => {
                let banner = match app.feedback {
                    Feedback::Correct => Some(("✓  Correct!".to_string(), Color::LightGreen)),
                    Feedback::Wrong => Some(("✗  Not quite — try again!".to_string(), Color::LightRed)),
                    Feedback::None => None,
                };
                render_unit_card(card, f.buffer_mut(), p, &app.input, banner);
                if p.duck_jump {
                    let inner = Block::default().borders(Borders::ALL).inner(card);
                    draw_duck_gag(f.buffer_mut(), inner, p.theme, app.anim_frame);
                }
            }
            Active::Geo(g) => {
                let banner = match app.feedback {
                    Feedback::Correct => Some(("✓  Correct!".to_string(), Color::LightGreen)),
                    Feedback::Wrong => Some(("✗  Not quite — measure again!".to_string(), Color::LightRed)),
                    Feedback::None => None,
                };
                render_geometry_card(card, f.buffer_mut(), g, &app.input, banner);
            }
            Active::Arith(p) => {
                let banner = feedback_banner(app);
                render_card(card, f.buffer_mut(), p, &app.input, app.config.layout, banner);
            }
        }
    }

    // Deduction Duck helps with any problem; the Why panel works for every topic.
    if app.transition.is_none() && app.help_in > 0.0 {
        match &app.current {
            Active::Shape(s) => draw_hint_panel(f, app, card, &s.hint, s.answer_label()),
            Active::Unit(u) => draw_hint_panel(f, app, card, &u.hint, format!("Answer: {} {}", u.answer, u.unit_label)),
            Active::Geo(g) => draw_hint_panel(f, app, card, &g.hint, g.answer_label()),
            Active::Arith(p) => draw_help_overlay(f, app, card, p),
        }
    }
    if app.transition.is_none() && app.why_active {
        draw_why_overlay(f, app, card);
    }
}

fn feedback_banner(app: &App) -> Option<(String, Color)> {
    match app.feedback {
        Feedback::None => None,
        Feedback::Correct => Some(("✓  Correct!  Quack-tastic!".to_string(), Color::LightGreen)),
        Feedback::Wrong => Some(("✗  Not yet — try again, or press H for help.".to_string(), Color::LightRed)),
    }
}

/// Deduction Duck's hint panel: a how-to hint (never the answer) with R to
/// reveal the worked `answer`.  Shared by measurement and fraction problems,
/// and honours the encouragement, reprimand, and peek-cooldown state.
fn draw_hint_panel(f: &mut Frame, app: &App, area: Rect, hint: &[String], answer: String) {
    // Tagged lines: 0 plain, 1 answer, 2 encouragement, 3 header, 4 reprimand.
    let mut lines: Vec<(String, u8)> = Vec::new();
    if let Some(msg) = &app.encourage {
        lines.push((msg.clone(), 2));
        lines.push((String::new(), 0));
    }
    if let Some(r) = &app.reprimand {
        for l in wrap_text(r, 52) {
            lines.push((l, 4));
        }
        lines.push((String::new(), 0));
    }
    lines.push(("Deduction Duck's hint:".to_string(), 3));
    for h in hint {
        lines.push((format!("• {}", h), 0));
    }
    if app.revealed {
        lines.push((String::new(), 0));
        lines.push((answer, 1));
    }

    let content_w = lines.iter().map(|(l, _)| l.chars().count()).max().unwrap_or(20) as u16;
    let panel_w = (content_w + 6 + duck::WIDTH).min(area.width.saturating_sub(2)).max(20);
    let panel_h = ((lines.len() as u16).max(duck::HEIGHT) + 3).min(area.height.saturating_sub(2)).max(6);
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
    if panel.width < 12 || panel.height < 4 {
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

    let duck_x = inner.right().saturating_sub(duck::WIDTH);
    let duck_y = inner.top() + inner.height.saturating_sub(duck::HEIGHT) / 2;
    duck::draw_pose(buf, duck_x, duck_y, Pose::Stand, app.anim_frame / 6, Style::default().fg(DUCK_COLOR));

    let text_w = duck_x.saturating_sub(inner.left() + 1);
    let mut y = inner.top();
    for (line, tag) in &lines {
        if y >= inner.bottom().saturating_sub(1) {
            break;
        }
        let style = match tag {
            1 | 2 => Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD),
            3 => Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD),
            4 => Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD),
            _ => Style::default().fg(Color::White),
        };
        put_str(buf, inner.left(), y, clip(line, text_w), style);
        y += 1;
    }

    let footer = if app.reveal_locked() {
        "Answer's on cooldown — try it yourself!    H: shoo"
    } else if app.revealed {
        "R: hide answer    H: shoo"
    } else {
        "R: show answer    H: shoo"
    };
    put_str(buf, inner.left(), inner.bottom().saturating_sub(1), footer, Style::default().fg(Color::Gray));
}

/// The Y overlay: a panel listing real-world uses of the current topic.
fn draw_why_overlay(f: &mut Frame, app: &App, area: Rect) {
    let heading = crate::motivation::heading(app.current_topic());

    // Each tagged line carries a style: 0 bullet, 1 code header, 2 code, 3 why.
    let panel_w = area.width.saturating_sub(4).clamp(24, 72).min(area.width);
    let wrap_w = panel_w.saturating_sub(4);
    let mut lines: Vec<(String, u8)> = Vec::new();
    for item in &app.why_items {
        for line in wrap_prefixed("• ", "  ", item, wrap_w) {
            lines.push((line, 0));
        }
    }
    // A code peek for budding programmers.
    if let Some((code, why)) = &app.why_code {
        lines.push((String::new(), 0));
        for line in wrap_text("For future coders — a line from this app:", wrap_w) {
            lines.push((line, 1));
        }
        lines.push((format!("  {}", code), 2));
        // Prefix-aware wrap: "Why it matters: …" stays inside the panel.
        for line in wrap_prefixed("  Why it matters: ", "    ", why, wrap_w) {
            lines.push((line, 3));
        }
    }

    // Grow to fit the content (heading + gap + lines + footer + borders), but
    // never taller than the available area — the leftover scrolls.
    let wanted_h = lines.len() as u16 + 5;
    let panel_h = wanted_h.min(area.height.saturating_sub(2)).max(7);
    let panel_x = area.left() + area.width.saturating_sub(panel_w) / 2;
    let panel_y = area.top() + area.height.saturating_sub(panel_h) / 2;
    let panel = Rect {
        x: panel_x,
        y: panel_y,
        width: panel_w.min(area.right().saturating_sub(panel_x)),
        height: panel_h.min(area.bottom().saturating_sub(panel_y)),
    };
    if panel.width < 12 || panel.height < 5 {
        return;
    }

    let buf = f.buffer_mut();
    Clear.render(panel, buf);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::LightCyan))
        .title(" Deduction Duck ")
        .style(Style::default().bg(STAGE_BG));
    let inner = block.inner(panel);
    block.render(panel, buf);

    put_str(buf, inner.left(), inner.top(), clip(&heading, inner.width), Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD));

    // Visible content window: rows between the gap under the heading and the
    // footer.  Anything beyond scrolls, controlled by Up/Down.
    let first_row = inner.top() + 2;
    let footer_row = inner.bottom().saturating_sub(1);
    let visible = footer_row.saturating_sub(first_row) as usize;
    let total = lines.len();
    let max_scroll = total.saturating_sub(visible);
    app.why_max_scroll.set(max_scroll);
    let scroll = app.why_scroll.min(max_scroll);

    let mut y = first_row;
    for (line, tag) in lines.iter().skip(scroll).take(visible) {
        let style = match tag {
            1 => Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD),
            2 => Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD),
            3 => Style::default().fg(Color::Gray),
            _ => Style::default().fg(Color::White),
        };
        put_str(buf, inner.left(), y, clip(line, inner.width), style);
        y += 1;
    }

    // Scroll affordances.
    let scrollable = max_scroll > 0;
    if scroll > 0 {
        put_str(buf, inner.right().saturating_sub(7), inner.top() + 1, "↑ more", Style::default().fg(Color::LightCyan));
    }
    if scroll < max_scroll {
        put_str(buf, inner.right().saturating_sub(7), footer_row, "↓ more", Style::default().fg(Color::LightCyan));
    }

    let footer = if scrollable { "Y/Esc: close    ↑↓ scroll" } else { "Y or Esc: close" };
    put_str(buf, inner.left(), footer_row, footer, Style::default().fg(Color::Gray));
}

// ---------------------------------------------------------------------------
// Challenge HUD + summary
// ---------------------------------------------------------------------------

fn draw_challenge_hud(f: &mut Frame, app: &App, area: Rect) {
    let buf = f.buffer_mut();
    let Some(c) = &app.challenge else { return };
    let remaining = c.remaining_secs();

    let label = format!(" ⏱  {:>2}s    ★ Solved: {} ", remaining, c.solved);
    put_str(buf, area.left() + 1, area.top(), label, Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD));

    // A simple time bar across the row beneath the label.
    let total = c.duration_secs().max(1);
    let frac = (remaining as f32 / total as f32).min(1.0);
    let bar_w = area.width.saturating_sub(2);
    let filled = (frac * bar_w as f32).round() as u16;
    let bar_color = if frac > 0.5 {
        Color::LightGreen
    } else if frac > 0.25 {
        Color::LightYellow
    } else {
        Color::LightRed
    };
    for i in 0..bar_w {
        let ch = if i < filled { "█" } else { "░" };
        let style = Style::default().fg(if i < filled { bar_color } else { Color::DarkGray });
        buf[(area.left() + 1 + i, area.top() + 1)].set_symbol(ch).set_style(style);
    }
}

fn draw_challenge_summary(f: &mut Frame, app: &App, area: Rect) {
    let buf = f.buffer_mut();
    let solved = app.challenge.as_ref().map_or(0, |c| c.solved);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::LightGreen))
        .style(Style::default().bg(BG));
    let inner = block.inner(area);
    block.render(area, buf);

    let big = format!("{}", solved);
    let bw = font::text_width(&big);
    font::draw_text(buf, center(inner, bw), inner.top() + inner.height / 2 - 3, &big, Style::default().fg(Color::LightGreen));

    let msg = "Time! Problems solved:";
    put_str(buf, center(inner, msg.chars().count() as u16), inner.top() + inner.height / 2 - 5, msg, Style::default().fg(Color::White).add_modifier(Modifier::BOLD));

    let hint = "Enter = play again    Esc = menu";
    put_str(buf, center(inner, hint.chars().count() as u16), inner.bottom().saturating_sub(2), hint, Style::default().fg(Color::DarkGray));
}
