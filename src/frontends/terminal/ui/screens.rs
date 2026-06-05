//! The full-screen, non-gameplay terminal views: the startup graphics picker,
//! the main menu (with Deduction Duck wandering by), the My Progress breakdown,
//! the password-gated Teacher Area (anecdote editor + records table), and the
//! Settings knobs.  Each `draw_*(f, app, area)` is dispatched from `draw`.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Widget};
use ratatui::Frame;

use crate::app::{App, TeacherView, SETTINGS_FIELDS};
use crate::duck::{self, Pose};
use crate::{font, problem};

use super::*;

// -- Startup — graphics mode picker -----------------------------------------

pub fn draw_startup(f: &mut Frame, app: &App, area: Rect) {
    use crate::config::GraphicsMode;
    let buf = f.buffer_mut();

    let title = "1 + 1 = ?";
    font::draw_text(buf, center(area, font::text_width(title)), area.top() + 2, title, Style::default().fg(Color::LightCyan));
    put_str(buf, center(area, 17), area.top() + 8, "RUSTY MATH TUTOR", Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
    let featuring = "Featuring:  Deduction Duck";
    put_str(buf, center(area, featuring.chars().count() as u16), area.top() + 9, featuring, Style::default().fg(DUCK_COLOR).add_modifier(Modifier::BOLD));
    put_str(buf, center(area, 18), area.top() + 10, "Choose how to draw:", Style::default().fg(Color::Gray));

    let modes = [GraphicsMode::Low, GraphicsMode::Cpu];
    let col = area.left() + area.width.saturating_sub(38) / 2;
    let mut y = area.top() + 13;
    for (i, m) in modes.iter().enumerate() {
        let selected = i == app.startup_index;
        let style = if selected {
            Style::default().fg(Color::Black).bg(Color::LightCyan).add_modifier(Modifier::BOLD)
        } else if m.available() {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let cursor = if selected { "➤ " } else { "  " };
        put_str(buf, col, y, format!("{}{:<34}", cursor, m.name()), style);
        y += 2;
    }

    put_str(buf, center(area, 46), area.bottom().saturating_sub(4), "CPU graphics (2D/3D, no GPU needed) is coming soon.", Style::default().fg(Color::DarkGray));
    put_str(buf, center(area, 34), area.bottom().saturating_sub(2), "Up / Down to choose    Enter to start", Style::default().fg(Color::DarkGray));
}

// -- Menu -------------------------------------------------------------------

pub fn draw_menu(f: &mut Frame, app: &App, area: Rect) {
    let buf = f.buffer_mut();

    let title = "1 + 1 = ?";
    font::draw_text(buf, center(area, font::text_width(title)), area.top() + 1, title, Style::default().fg(Color::LightCyan));
    put_str(buf, center(area, 17), area.top() + 6, "RUSTY MATH TUTOR", Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
    let featuring = "Featuring:  Deduction Duck";
    put_str(buf, center(area, featuring.chars().count() as u16), area.top() + 7, featuring, Style::default().fg(DUCK_COLOR).add_modifier(Modifier::BOLD));

    let col = area.left() + area.width.saturating_sub(42) / 2;
    let sel = app.menu_index;

    // The menu as a flat list of (label, hint) — one line each, so it can be
    // windowed when the terminal is short.
    let mark = |on: bool| if on { "[x]" } else { "[ ]" };
    let mut items: Vec<(String, &str)> = vec![
        (format!("Student:  < {} >", app.roster.current().name), "(<>  N: add new)"),
        (format!("Grade level:  < {} >", grade_name(app.menu_grade)), "(Left / Right)"),
    ];
    for (i, op) in problem::Op::ALL.iter().enumerate() {
        items.push((format!("{} {}", mark(app.menu_ops[i]), op.name()), "(Enter toggles)"));
    }
    items.push((format!("{} Units of Measure", mark(app.menu_units)), "(measuring problems)"));
    items.push((format!("{} Fractions", mark(app.menu_fractions)), "(shapes & pieces)"));
    items.push((format!("{} Percentages", mark(app.menu_percents)), "(shapes out of 100)"));
    items.push((format!("{} Geometry", mark(app.menu_geometry)), "(perimeter, area, volume)"));
    items.push(("Settings (number ranges)...".to_string(), "(Enter to open)"));
    items.push(("My Progress...".to_string(), "(Enter to view)"));
    items.push(("Teacher Area...".to_string(), "(password)"));
    items.push(("▶  Start Practice".to_string(), "(no timer)"));
    items.push(("▶  Start Challenge".to_string(), "(60-second timer)"));
    items.push(("▶  Experimentation".to_string(), "(explore unit conversions)"));

    // Scroll the window so the selected item stays visible.
    let (scroll, visible) = scroll_window(items.len(), sel, area.top() + 8, area.bottom().saturating_sub(2));
    let list_top = area.top() + 8;
    let mut y = list_top;
    for (idx, (label, hint)) in items.iter().enumerate().skip(scroll).take(visible) {
        draw_menu_row(buf, col, y, idx == sel, label, hint);
        y += 1;
    }
    if scroll > 0 {
        put_str(buf, area.right().saturating_sub(9), list_top, "↑ more", Style::default().fg(Color::LightCyan));
    }
    if scroll + visible < items.len() {
        put_str(buf, area.right().saturating_sub(9), y.saturating_sub(1), "↓ more", Style::default().fg(Color::LightCyan));
    }

    put_str(buf, col, area.bottom().saturating_sub(1), "Up / Down to move    N: add student    Q: quit", Style::default().fg(Color::DarkGray));

    // Deduction Duck wanders by every so often.
    draw_menu_duck(buf, area, app.anim_frame);

    // New-student name entry.
    if app.naming {
        draw_name_prompt(buf, area, &app.name_input);
    }
}

/// Every so often Deduction Duck wanders by the menu: peeking in from a side
/// (clear of the centred text) or, when there's room below the menu, waddling
/// right across the bottom.  Driven entirely by the free-running clock.
fn draw_menu_duck(buf: &mut Buffer, area: Rect, frame: u64) {
    let style = Style::default().fg(DUCK_COLOR);
    let hgt = duck::HEIGHT as i32;

    // One appearance per long cycle; off-stage the rest of the time.
    const CYCLE: u64 = 640;
    let phase = frame % CYCLE;
    let y_at = |frac: f32| area.top() as i32 + (area.height as f32 * frac) as i32 - hgt / 2;

    match (frame / CYCLE) % 4 {
        // A full waddle across the bottom — only where the menu leaves space.
        0 if (area.bottom() as i32 - hgt) > area.top() as i32 + 27 => {
            const DUR: u64 = 200;
            if phase < DUR {
                let t = phase as f32 / DUR as f32;
                let span = (area.width as i32 + 2 * duck::WIDTH as i32 + 4) as f32;
                let x = (area.left() as i32 - duck::WIDTH as i32 - 2) as f32 + t * span;
                let y = area.bottom() as i32 - hgt - if (frame / 6).is_multiple_of(2) { 0 } else { 1 };
                duck::draw_pose_i32(buf, x.round() as i32, y, Pose::WalkRight, frame / 5, style);
            }
        }
        // Peek in from the right at a low spot.
        0 | 2 => peek_in(buf, area, phase, frame, true, y_at(0.62), style),
        // Peek in from the right, higher up.
        1 => peek_in(buf, area, phase, frame, true, y_at(0.32), style),
        // Peek in from the left.
        _ => peek_in(buf, area, phase, frame, false, y_at(0.5), style),
    }
}

/// Slide the duck in from a side edge, hold with a little "hi!", then back out.
fn peek_in(buf: &mut Buffer, area: Rect, phase: u64, frame: u64, from_right: bool, y: i32, style: Style) {
    use std::f32::consts::PI;
    const DUR: u64 = 150;
    if phase >= DUR {
        return;
    }
    let w = duck::WIDTH as i32;
    let t = phase as f32 / DUR as f32;
    let depth = ((t * PI).sin() * (w as f32 + 2.0)).round() as i32;
    let saying = (0.4..0.62).contains(&t);
    let hi = Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD);

    let x = if from_right {
        area.right() as i32 - depth
    } else {
        area.left() as i32 - w + depth
    };
    let pose = if from_right { Pose::Stand } else { Pose::WalkRight };
    duck::draw_pose_i32(buf, x, y, pose, frame / 8, style);

    // A little "hi!" just above his head, where it can't collide with the
    // centred menu text.
    if saying && y > area.top() as i32 {
        let hx = (x + 2).clamp(area.left() as i32, area.right() as i32 - 4);
        put_str(buf, hx as u16, (y - 1) as u16, "hi!", hi);
    }
}

/// A small centred box for typing a new student's name.
fn draw_name_prompt(buf: &mut Buffer, area: Rect, input: &str) {
    let w = 40.min(area.width.saturating_sub(2));
    let h = 4u16;
    let x = area.left() + area.width.saturating_sub(w) / 2;
    let y = area.top() + area.height.saturating_sub(h) / 2;
    let panel = Rect { x, y, width: w, height: h };
    Clear.render(panel, buf);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::LightGreen))
        .title(" New student ")
        .style(Style::default().bg(STAGE_BG));
    let inner = block.inner(panel);
    block.render(panel, buf);
    let shown = tail(input, inner.width.saturating_sub(1));
    put_str(buf, inner.left(), inner.top(), format!("{}_", shown), Style::default().fg(Color::White));
    put_str(buf, inner.left(), inner.bottom().saturating_sub(1), "Enter: save    Esc: cancel", Style::default().fg(Color::Gray));
}

fn draw_menu_row(buf: &mut Buffer, x: u16, y: u16, selected: bool, label: &str, hint: &str) {
    let style = if selected {
        Style::default().fg(Color::Black).bg(Color::LightCyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let cursor = if selected { "➤ " } else { "  " };
    put_str(buf, x, y, format!("{}{:<26}", cursor, label), style);
    if selected {
        put_str(buf, x + 30, y, hint, Style::default().fg(Color::Gray));
    }
}

fn grade_name(grade: u8) -> String {
    if grade == 0 {
        "K".to_string()
    } else {
        format!("{}", grade)
    }
}

// -- My Progress — encouragement only, never comparison ---------------------

pub fn draw_stats(f: &mut Frame, app: &App, area: Rect) {
    let buf = f.buffer_mut();
    let s = app.roster.current();
    let total = s.grand_total();
    let col = area.left() + area.width.saturating_sub(46) / 2;

    put_str(buf, col, area.top() + 2, format!("{}'s Math Journey", s.name), Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD));

    // Big celebratory total — every kind of problem counts.
    let total_str = total.to_string();
    font::draw_text(buf, center(area, font::text_width(&total_str)), area.top() + 4, &total_str, Style::default().fg(Color::LightGreen));
    put_str(buf, center(area, 20), area.top() + 10, "problems solved!", Style::default().fg(Color::White).add_modifier(Modifier::BOLD));

    // Breakdown with friendly bars across every section (one per Topic).
    let rows: Vec<(&str, u32)> =
        crate::topic::Topic::ALL.iter().map(|&t| (t.short(), s.solved_for(t))).collect();
    let max = rows.iter().map(|(_, n)| *n).max().unwrap_or(0).max(1);
    let mut y = area.top() + 12;
    for (label, n) in rows {
        let bar_w = 24u16;
        let filled = ((n as f32 / max as f32) * bar_w as f32).round() as u16;
        let bar: String = (0..bar_w).map(|i| if i < filled { '█' } else { '░' }).collect();
        put_str(buf, col, y, format!("{:<10} ", label), Style::default().fg(Color::White));
        put_str(buf, col + 11, y, &bar, Style::default().fg(Color::LightGreen));
        put_str(buf, col + 11 + bar_w + 1, y, format!("{}", n), Style::default().fg(Color::LightYellow));
        y += 1;
    }

    if s.best_streak > 0 {
        put_str(buf, col, y, format!("★ Best streak: {} in a row!", s.best_streak), Style::default().fg(Color::LightMagenta).add_modifier(Modifier::BOLD));
        y += 1;
    }

    // An encouraging message that grows with effort — never a comparison.
    let msg = encouragement(total);
    put_str(buf, col, y, msg, Style::default().fg(Color::LightCyan));

    put_str(buf, col, area.bottom().saturating_sub(2), "Every problem makes you stronger.   Esc: back", Style::default().fg(Color::DarkGray));
}

/// A warm message keyed only to the student's own total — no ranking.
fn encouragement(total: u32) -> &'static str {
    match total {
        0 => "Your journey starts with the very first problem. Let's go!",
        1..=9 => "You're off to a great start — keep that curiosity going!",
        10..=49 => "Look at you go! Your brain is getting stronger every day.",
        50..=149 => "Fantastic effort — you're becoming a real problem-solver!",
        150..=499 => "Incredible dedication. You should be proud of yourself!",
        _ => "You're a math powerhouse. Deduction Duck is so proud!",
    }
}

// -- Teacher Area — password-gated anecdote editor + records admin ----------

pub fn draw_teacher(f: &mut Frame, app: &App, area: Rect) {
    if !app.teacher_authed {
        draw_teacher_login(f, app, area);
    } else {
        draw_teacher_manage(f, app, area);
    }
}

fn draw_teacher_login(f: &mut Frame, app: &App, area: Rect) {
    let buf = f.buffer_mut();
    let col = area.left() + area.width.saturating_sub(44) / 2;

    put_str(buf, col, area.top() + 3, "TEACHER AREA", Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD));

    let setting = !app.config.has_teacher_password();
    let prompt = if setting { "Create a teacher password:" } else { "Enter teacher password:" };
    put_str(buf, col, area.top() + 6, prompt, Style::default().fg(Color::White));

    let masked: String = "*".repeat(app.teacher_pw.chars().count());
    put_str(buf, col, area.top() + 7, format!("{}_", masked), Style::default().fg(Color::LightGreen));

    if setting {
        put_str(buf, col, area.top() + 9, "(Kids can't reach these tools without it.)", Style::default().fg(Color::DarkGray));
    }
    if let Some(msg) = &app.teacher_msg {
        put_str(buf, col, area.top() + 11, msg, Style::default().fg(Color::LightYellow));
    }

    put_str(buf, col, area.bottom().saturating_sub(2), "Enter: continue    Esc: back", Style::default().fg(Color::DarkGray));
}

fn draw_teacher_manage(f: &mut Frame, app: &App, area: Rect) {
    let buf = f.buffer_mut();
    let col = area.left() + area.width.saturating_sub(60) / 2;

    // Tabs.
    let (a_style, r_style) = match app.teacher_view {
        TeacherView::Anecdotes => (Style::default().fg(Color::Black).bg(Color::LightCyan).add_modifier(Modifier::BOLD), Style::default().fg(Color::Gray)),
        TeacherView::Records => (Style::default().fg(Color::Gray), Style::default().fg(Color::Black).bg(Color::LightCyan).add_modifier(Modifier::BOLD)),
    };
    put_str(buf, col, area.top() + 1, " Why? Examples ", a_style);
    put_str(buf, col + 16, area.top() + 1, " Student Records ", r_style);
    put_str(buf, col + 36, area.top() + 1, "Tab: switch", Style::default().fg(Color::DarkGray));

    // Measurement locality lives here now (used by Units of Measure problems).
    put_str(
        buf,
        col,
        area.bottom().saturating_sub(4),
        format!("Units locality: {}    (L: change)", app.config.locality.name()),
        Style::default().fg(Color::LightCyan),
    );

    match app.teacher_view {
        TeacherView::Anecdotes => draw_teacher_anecdotes(buf, app, area, col),
        TeacherView::Records => draw_teacher_records(buf, app, area, col),
    }

    if let Some(msg) = &app.teacher_msg {
        put_str(buf, col, area.bottom().saturating_sub(3), msg, Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD));
    }
}

fn draw_teacher_anecdotes(buf: &mut Buffer, app: &App, area: Rect, col: u16) {
    let topic = crate::topic::Topic::ALL[app.teacher_topic];
    put_str(buf, col, area.top() + 3, format!("Section:  < {} >", topic.name()), Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
    put_str(buf, col, area.top() + 4, "Your own real-life examples shown in the Why? panel:", Style::default().fg(Color::Gray));

    // When adding, a wrapping input box claims the lower part of the screen.
    let input_box = if app.teacher_adding {
        let box_h = area.height.saturating_sub(9).clamp(4, 8);
        let box_w = area.right().saturating_sub(col).saturating_sub(2).clamp(20, 66);
        let box_y = area.bottom().saturating_sub(box_h + 2);
        Some(Rect { x: col, y: box_y, width: box_w, height: box_h })
    } else {
        None
    };
    let list_bottom = input_box.map_or(area.bottom().saturating_sub(2), |b| b.y.saturating_sub(1));

    let items = app.why_extras.items(topic);
    let mut y = area.top() + 6;
    if items.is_empty() {
        put_str(buf, col + 2, y, "(none yet — press A to add one)", Style::default().fg(Color::DarkGray));
    } else {
        for it in items {
            if y >= list_bottom {
                break;
            }
            put_str(buf, col + 2, y, clip(&format!("• {}", it), area.width.saturating_sub(4)), Style::default().fg(Color::White));
            y += 1;
        }
    }

    if let Some(rect) = input_box {
        Clear.render(rect, buf);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::LightYellow))
            .title(" New example ")
            .style(Style::default().bg(STAGE_BG));
        let inner = block.inner(rect);
        block.render(rect, buf);

        // Echo the typed text wrapped to the box, with a caret, keeping the end
        // in view if it overflows.
        let rows = inner.height as usize;
        let lines = wrap_chars(&format!("{}_", app.teacher_text), inner.width);
        let start = lines.len().saturating_sub(rows);
        for (i, line) in lines[start..].iter().enumerate() {
            put_str(buf, inner.left(), inner.top() + i as u16, line, Style::default().fg(Color::White));
        }

        // Hint on the left, character count on the right, just below the box.
        let foot_y = rect.bottom();
        put_str(buf, col, foot_y, "Enter: save    Esc: cancel", Style::default().fg(Color::DarkGray));
        let count = format!("{}/{}", app.teacher_text.chars().count(), crate::app::ANECDOTE_MAX);
        put_str(buf, area.right().saturating_sub(count.len() as u16 + 1), foot_y, &count, Style::default().fg(Color::Gray));
    } else {
        put_str(buf, col, area.bottom().saturating_sub(2), "< > section    A: add    Tab: records    Esc: log out", Style::default().fg(Color::DarkGray));
    }
}

/// Short column headers for each section, in [`Topic::ALL`] order.  Kept to a
/// few characters so all sections fit one row.
const REC_COLS: [&str; 8] = ["Add", "Sub", "Mul", "Div", "Un", "Fr", "Pct", "Geo"];
/// Left edge of the name column within the 5-wide section grid.
const REC_NAME_W: u16 = 12;
/// Width of each per-section count column.
const REC_COL_W: u16 = 5;

fn draw_teacher_records(buf: &mut Buffer, app: &App, area: Rect, col: u16) {
    let sel_topic = app.teacher_topic;
    // One compact column per section, then Total and the reveal Lock.
    let mut header = format!("{:<width$}", "Student", width = REC_NAME_W as usize);
    for c in REC_COLS {
        header.push_str(&format!("{:>w$}", c, w = REC_COL_W as usize));
    }
    header.push_str(&format!("{:>7}{:>6}", "Total", "Lock"));
    let header_y = area.top() + 3;
    put_str(buf, col, header_y, &header, Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD));
    // Highlight the selected section's column header — this is what S/R resets.
    let sel_x = col + REC_NAME_W + sel_topic as u16 * REC_COL_W;
    put_str(
        buf,
        sel_x,
        header_y,
        format!("{:>w$}", REC_COLS[sel_topic], w = REC_COL_W as usize),
        Style::default().fg(Color::Black).bg(Color::LightYellow).add_modifier(Modifier::BOLD),
    );

    let list_top = area.top() + 4;
    let (scroll, visible) = scroll_window(app.roster.students.len(), app.teacher_rec_index, list_top, area.bottom().saturating_sub(4));
    let mut y = list_top;
    for (i, s) in app.roster.students.iter().enumerate().skip(scroll).take(visible) {
        let selected = i == app.teacher_rec_index;
        let base = if selected {
            Style::default().fg(Color::Black).bg(Color::LightCyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let lock = if s.reveal_lock == 0 { "off".to_string() } else { s.reveal_lock.to_string() };
        let mut row = format!("{:<width$}", clip(&s.name, REC_NAME_W), width = REC_NAME_W as usize);
        for &t in &crate::topic::Topic::ALL {
            row.push_str(&format!("{:>w$}", s.solved_for(t), w = REC_COL_W as usize));
        }
        row.push_str(&format!("{:>7}{:>6}", s.grand_total(), lock));
        put_str(buf, col, y, &row, base);
        y += 1;
    }
    if scroll > 0 {
        put_str(buf, col, list_top, "↑", Style::default().fg(Color::LightCyan));
    }
    if scroll + visible < app.roster.students.len() {
        put_str(buf, col, y.saturating_sub(1), "↓", Style::default().fg(Color::LightCyan));
    }

    put_str(
        buf,
        col,
        area.bottom().saturating_sub(2),
        "Up/Dn student   <> section   S/R reset   X remove   +/- lock(0=off)   Esc out",
        Style::default().fg(Color::DarkGray),
    );
}

// -- Settings — per-grade number ranges -------------------------------------

pub fn draw_settings(f: &mut Frame, app: &App, area: Rect) {
    let buf = f.buffer_mut();
    let col = area.left() + (area.width.saturating_sub(46)) / 2;

    put_str(buf, col, area.top() + 2, "SETTINGS — NUMBER RANGES", Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD));
    put_str(buf, col, area.top() + 3, "Tune how big the numbers get for each grade.", Style::default().fg(Color::Gray));

    let g = app.settings_grade;
    let r = app.config.range(g);
    let sel = app.settings_field;

    let rows: [(String, String); SETTINGS_FIELDS] = [
        ("Grade".to_string(), format!("< {} >", grade_name(g))),
        ("Add / Subtract up to".to_string(), format!("< {} >", r.add_max)),
        ("Multiply factors up to".to_string(), format!("< {} >", r.mul_max)),
        ("Divide numbers up to".to_string(), format!("< {} >", r.div_max)),
        ("Allow negative answers".to_string(), format!("< {} >", if r.allow_negative { "Yes" } else { "No" })),
        ("Show problems".to_string(), format!("< {} >", app.config.layout.name())),
        ("Theme (graphics mode)".to_string(), format!("< {} >", app.config.theme.name())),
    ];

    let mut y = area.top() + 6;
    for (i, (label, value)) in rows.iter().enumerate() {
        let selected = i == sel;
        let style = if selected {
            Style::default().fg(Color::Black).bg(Color::LightCyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let cursor = if selected { "➤ " } else { "  " };
        put_str(buf, col, y, format!("{}{:<26}{}", cursor, label, value), style);
        y += 2;
    }

    put_str(
        buf,
        col,
        area.bottom().saturating_sub(2),
        "Up / Down choose    Left / Right change    Esc save & back",
        Style::default().fg(Color::DarkGray),
    );
}
