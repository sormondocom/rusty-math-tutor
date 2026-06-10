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
use crate::student::ChallengeRecord;

const TOPIC_SYMBOLS: [&str; 8] = ["+", "\u{2212}", "\u{00D7}", "\u{00F7}", "units", "%", "\u{2044}", "\u{03C0}"];
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
    items.push(("▶  Start Practice".to_string(), "(no timer)"));
    let challenge_hint = format!("({}s timer)", app.roster.current().challenge_secs);
    items.push(("▶  Start Challenge".to_string(), challenge_hint.as_str()));
    items.push(("▶  Time Explorer".to_string(), "(clocks & time zones)"));
    items.push(("▶  Experimentation".to_string(), "(explore unit conversions)"));
    items.push(("Settings (number ranges)...".to_string(), "(Enter to open)"));
    items.push(("My Progress...".to_string(), "(Enter to view)"));
    items.push(("Teacher Area...".to_string(), "(password)"));
    items.push(("Help / Key Guide...".to_string(), "(H on any screen)"));

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

    // Grade-level sparkline: always 3 rows so the section is visible even
    // before the student has any grade data (empty grades show as '·').
    {
        let grade_max = s.grade_solved.iter().copied().max().unwrap_or(0);
        const BLOCKS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
        const NAMES:  [&str; 9] = ["K", "1", "2", "3", "4", "5", "6", "7", "8"];
        const CW: usize = 4; // chars per grade column

        // Row 1: grade names.
        let mut row = "Grade: ".to_string();
        for name in NAMES { row.push_str(&format!("{:<CW$}", name)); }
        put_str(buf, col, y, row, Style::default().fg(Color::Gray));
        y += 1;

        // Row 2: block-character bars.
        let mut row = "       ".to_string();
        for i in 0..9usize {
            let n = s.grade_solved[i];
            let lvl = if n == 0 { 0 }
                      else { ((n as f32 / grade_max as f32 * 8.0).round() as usize).clamp(1, 8) };
            let ch = if lvl == 0 { '·' } else { BLOCKS[lvl] };
            row.push_str(&format!("{:<CW$}", ch));
        }
        put_str(buf, col, y, row, Style::default().fg(Color::LightCyan));
        y += 1;

        // Row 3: counts (blank when zero).
        let mut row = "       ".to_string();
        for i in 0..9usize {
            let n = s.grade_solved[i];
            if n > 0 { row.push_str(&format!("{:<CW$}", n)); }
            else      { row.push_str(&" ".repeat(CW)); }
        }
        put_str(buf, col, y, row, Style::default().fg(Color::LightYellow));
        y += 1;
    } // end grade sparkline block

    if s.best_streak > 0 {
        put_str(buf, col, y, format!("★ Best streak: {} in a row!", s.best_streak), Style::default().fg(Color::LightMagenta).add_modifier(Modifier::BOLD));
        y += 1;
    }

    // An encouraging message that grows with effort — never a comparison.
    let msg = encouragement(total);
    put_str(buf, col, y, msg, Style::default().fg(Color::LightCyan));
    y += 2;

    // Challenge history — most recent first, up to 5 rows.
    draw_challenge_history_rows(buf, &s.challenge_history, col, y, area);

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
    let col = area.left() + area.width.saturating_sub(70) / 2;

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
    // One compact column per section, then Total, reveal Lock, and Timer.
    let mut header = format!("{:<width$}", "Student", width = REC_NAME_W as usize);
    for c in REC_COLS {
        header.push_str(&format!("{:>w$}", c, w = REC_COL_W as usize));
    }
    header.push_str(&format!("{:>7}{:>6}{:>5}", "Total", "Lock", "Tmr"));
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
        let lock  = if s.reveal_lock == 0 { "off".to_string() } else { s.reveal_lock.to_string() };
        let timer = format!("{}s", s.challenge_secs);
        let mut row = format!("{:<width$}", clip(&s.name, REC_NAME_W), width = REC_NAME_W as usize);
        for &t in &crate::topic::Topic::ALL {
            row.push_str(&format!("{:>w$}", s.solved_for(t), w = REC_COL_W as usize));
        }
        row.push_str(&format!("{:>7}{:>6}{:>5}", s.grand_total(), lock, timer));
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
        "Up/Dn student   <> section   S/R reset   X remove   C clear history   +/- lock   [ ] timer   Esc out",
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

// -- Shared challenge-history table (used by draw_stats) --------------------

fn draw_challenge_history_rows(buf: &mut Buffer, history: &[ChallengeRecord], col: u16, y: u16, area: Rect) {
    if history.is_empty() { return; }
    let max_rows = (area.bottom().saturating_sub(y + 3)) as usize;
    let max_rows = max_rows.min(5);
    if max_rows == 0 { return; }

    put_str(buf, col, y, "Challenge History:", Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD));
    put_str(buf, col, y + 1,
        &format!("{:<10} {:<8} {:<14} {}", "Solved", "/min", "Accuracy", "Streak"),
        Style::default().fg(Color::DarkGray));

    let entries: Vec<&ChallengeRecord> = history.iter().rev().take(max_rows).collect();
    for (i, rec) in entries.iter().enumerate() {
        let row_y = y + 2 + i as u16;
        let solved_str = format!("{}/{:.0}s", rec.solved, rec.duration_secs as f32);
        let rate_str   = format!("{:.1}", rec.rate());
        let acc_pct    = rec.accuracy_pct();
        let acc_str    = format!("{}% ({}/{})", acc_pct, rec.solved, rec.attempts);
        let streak_str = rec.streak_peak.to_string();
        let acc_color  = if acc_pct >= 80 { Color::LightGreen } else if acc_pct >= 60 { Color::LightYellow } else { Color::LightRed };

        put_str(buf, col,      row_y, format!("{:<10}", solved_str), Style::default().fg(Color::White));
        put_str(buf, col + 10, row_y, format!("{:<8}",  rate_str),  Style::default().fg(Color::LightYellow));
        put_str(buf, col + 18, row_y, format!("{:<14}", acc_str),   Style::default().fg(acc_color));
        put_str(buf, col + 32, row_y, &streak_str,                   Style::default().fg(Color::LightMagenta));
    }

    if history.len() > max_rows {
        let note_y = y + 2 + max_rows as u16;
        put_str(buf, col, note_y,
            &format!("+ {} more run{}", history.len() - max_rows, if history.len() - max_rows == 1 { "" } else { "s" }),
            Style::default().fg(Color::DarkGray));
    }
}

// -- Challenge End (summary) ------------------------------------------------

pub fn draw_challenge_end(f: &mut Frame, app: &App, area: Rect) {
    let Some(run) = &app.run else { return; };
    let buf = f.buffer_mut();
    let col = area.left() + area.width.saturating_sub(52) / 2;
    let mut y = area.top() + 1;

    // Title
    put_str(buf, col, y, "CHALLENGE COMPLETE!", Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD));
    y += 2;

    // Big solved count
    let solved_str = run.solved.to_string();
    font::draw_text(buf, center(area, font::text_width(&solved_str)), y, &solved_str, Style::default().fg(Color::White));
    y += 6;
    put_str(buf, center(area, 28), y,
        &format!("problem{} in {}s", if run.solved == 1 { "" } else { "s" }, run.duration_secs),
        Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD));
    y += 2;

    // Rate
    let rate = if run.duration_secs > 0 {
        run.solved as f32 * 60.0 / run.duration_secs as f32
    } else { 0.0 };
    put_str(buf, col, y, format!("Per minute:    {:.1}", rate), Style::default().fg(Color::LightYellow));
    y += 1;

    // Accuracy
    if run.attempts > 0 {
        let pct = (run.solved as f32 / run.attempts as f32 * 100.0).round() as u32;
        let color = if pct >= 80 { Color::LightGreen } else if pct >= 60 { Color::LightYellow } else { Color::LightRed };
        put_str(buf, col, y,
            format!("Accuracy:      {}%  ({} / {} answers)", pct, run.solved, run.attempts),
            Style::default().fg(color));
        y += 1;
    }

    // Best streak this run
    if run.streak_peak > 0 {
        put_str(buf, col, y, format!("Best streak:   {} in a row", run.streak_peak), Style::default().fg(Color::LightMagenta));
        y += 1;
    }

    // By-topic breakdown (non-zero only)
    let active: Vec<(usize, u32)> = run.by_topic.iter().copied().enumerate().filter(|(_, n)| *n > 0).collect();
    if !active.is_empty() {
        y += 1;
        let mut row = "By type:       ".to_string();
        for (i, n) in &active {
            row.push_str(&format!("{} {}   ", TOPIC_SYMBOLS[*i], n));
        }
        put_str(buf, col, y, row, Style::default().fg(Color::White));
        y += 1;
    }

    // Grade sparkline
    let grade_max = *run.by_grade.iter().max().unwrap_or(&0);
    if grade_max > 0 {
        y += 1;
        const BLOCKS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
        const NAMES:  [&str; 9] = ["K", "1", "2", "3", "4", "5", "6", "7", "8"];
        const CW: usize = 4;

        let mut grade_row   = "Grade:    ".to_string();
        let mut bar_row     = "          ".to_string();
        let mut count_row   = "          ".to_string();
        for i in 0..9usize {
            let n   = run.by_grade[i];
            let lvl = if n == 0 { 0 }
                      else { ((n as f32 / grade_max as f32 * 8.0).round() as usize).clamp(1, 8) };
            grade_row .push_str(&format!("{:<CW$}", NAMES[i]));
            bar_row   .push_str(&format!("{:<CW$}", if lvl == 0 { '·' } else { BLOCKS[lvl] }));
            if n > 0 { count_row.push_str(&format!("{:<CW$}", n)); }
            else      { count_row.push_str(&" ".repeat(CW)); }
        }
        put_str(buf, col, y,     grade_row,  Style::default().fg(Color::Gray));
        put_str(buf, col, y + 1, bar_row,    Style::default().fg(Color::LightCyan));
        put_str(buf, col, y + 2, count_row,  Style::default().fg(Color::LightYellow));
        y += 3;
    }
    let _ = y;

    // Footer
    put_str(buf, col, area.bottom().saturating_sub(2),
        "R / Enter: play again    M / Esc: menu",
        Style::default().fg(Color::DarkGray));
}

// -- Time Explorer ----------------------------------------------------------

pub fn draw_time(f: &mut Frame, app: &App, area: Rect) {
    use crate::time_display as td;

    let buf = f.buffer_mut();
    let col = area.left() + 2;
    let h   = app.time_hour;
    let m   = app.time_min;

    // ── Prominent local time (top) ─────────────────────────────────────────
    let use_24h = app.config.hour_format.is_24h();
    let (_, lmo_l, ldy_l, lh_loc, lm_loc, _) =
        td::apply_offset_dated(app.time_year, app.time_month, app.time_day,
                               h, m, app.time_local_offset);
    let (l12, lam) = td::to_12h(lh_loc);
    let big_time = if use_24h {
        format!("{:02}:{:02}", lh_loc, lm_loc)
    } else {
        format!("{}:{:02} {}", l12, lm_loc, if lam { "AM" } else { "PM" })
    };
    let big_date = format!("{} {} {}",
        ldy_l, td::month_abbr(lmo_l), app.time_year);
    let live_tag = if app.time_auto { " (live)" } else { "" };
    put_str(buf, center(area, big_time.chars().count() as u16), area.top() + 1,
        &big_time,
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
    put_str(buf, center(area, (big_date.len() + live_tag.len()) as u16), area.top() + 2,
        &format!("{}{}", big_date, live_tag),
        Style::default().fg(Color::Gray));

    // ── UTC reference row (editable) ───────────────────────────────────────
    let mm_str = format!("{:02}", m);
    let (h12, am) = td::to_12h(h);
    let hh_str = if use_24h { format!("{:02}", h) } else { format!("{}", h12) };
    let ampm_str = if use_24h { "" } else if am { " AM" } else { " PM" };
    let ss_str = format!("{:02}", app.time_sec);
    let dy_str = format!("{:02}", app.time_day);
    let mo_str = td::month_abbr(app.time_month);
    let yr_str = format!("{}", app.time_year);
    // Stored value is always UTC; label it so kids understand the reference.
    let input_str = format!("  {}:{}:{}{ampm_str} UTC  {} {} {}  ",
        hh_str, mm_str, ss_str, dy_str, mo_str, yr_str);
    let iy  = area.top() + 4; // below the big local-time display
    let ixc = center(area, input_str.chars().count() as u16);
    put_str(buf, ixc, iy, &input_str, Style::default().fg(Color::White));

    // Overlay the focused field in yellow.
    // Field byte positions depend on hour-format string length.
    let hh_len = hh_str.len() as u16;
    let ampm_len = ampm_str.len() as u16;
    // input_str layout: "  HH:MM:SS[ampm]  DD Mon YYYY  "
    // offsets:           0  2 5  8  11   16 19  23
    let f0_off = 2u16;                           // hour
    let f1_off = f0_off + hh_len + 1;            // min (skip ':')
    let f2_off = f1_off + 2 + 1 + 2 + ampm_len + 2; // day (skip mm:ss+ampm+gap)
    let f3_off = f2_off + 3;                     // month (skip dd+space)
    let f4_off = f3_off + 4;                     // year (skip Mon+space)
    let field_offsets = [f0_off, f1_off, f2_off, f3_off, f4_off];
    let field_lens    = [hh_len, 2u16, 2, 3, 4];
    let fi = app.time_field as usize;
    if fi < field_offsets.len() {
        let start = field_offsets[fi] as usize;
        let end   = (start + field_lens[fi] as usize).min(input_str.len());
        put_str(buf, ixc + field_offsets[fi], iy,
            &input_str[start..end],
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
    }

    let auto_hint = if app.time_auto {
        "LIVE  —  N: stays live   ↑ ↓ to freeze & adjust   ← → field"
    } else {
        "FROZEN   ↑ ↓ change   ← → field   N: back to now   Esc: menu"
    };
    put_str(buf, center(area, auto_hint.chars().count() as u16), iy + 1, auto_hint,
        Style::default().fg(if app.time_auto { Color::LightCyan } else { Color::DarkGray }));

    let panel_y = iy + 3;

    // ── Format panel (left) ────────────────────────────────────────────────
    let formats = [
        ("12-hour :", td::fmt_12h(h, m)),
        ("24-hour :", td::fmt_24h(h, m)),
        ("British :", td::fmt_british(h, m)),
        ("American:", td::fmt_american(h, m)),
        ("Roman   :", td::fmt_roman(h, m)),
    ];
    for (i, (label, value)) in formats.iter().enumerate() {
        put_str(buf, col, panel_y + i as u16,
            label, Style::default().fg(Color::DarkGray));
        // Highlight the active format
        let is_active = (i == 0 && !use_24h) || (i == 1 && use_24h);
        let st = if is_active {
            Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        put_str(buf, col + 10, panel_y + i as u16, value, st);
    }

    // T toggle hint under the format panel
    let toggle_hint = format!("T: switch to {}  N: now  Esc: menu",
        if use_24h { "12-hour" } else { "24-hour" });
    put_str(buf, col, panel_y + formats.len() as u16 + 1,
        &toggle_hint, Style::default().fg(Color::DarkGray));

    // ── ASCII analog clock ─────────────────────────────────────────────────
    // Position clock at ~col 29, calendar at col 52 (4-char cells, 28 wide).
    let clock_x: u16 = (area.left() + 28).min(area.right().saturating_sub(23));
    let cal_x:   u16 = (clock_x + 24).min(area.right().saturating_sub(28));

    let clock_lines = td::ascii_clock(h, m);
    let clock_row   = panel_y;
    for (i, line) in clock_lines.iter().enumerate() {
        for (j, ch) in line.chars().enumerate() {
            let style = match ch {
                'H' => Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                '*' => Style::default().fg(Color::Cyan),
                '+' => Style::default().fg(Color::LightYellow),
                '.' => Style::default().fg(Color::DarkGray),
                _ if ch.is_ascii_alphanumeric() => Style::default().fg(Color::Yellow),
                _ => Style::default().fg(Color::DarkGray),
            };
            let x = clock_x + j as u16;
            let y = clock_row + i as u16;
            if x < area.right() && y < area.bottom() {
                buf[(x, y)].set_char(ch).set_style(style);
            }
        }
    }

    // ── Calendar (4-char cells → 28 chars wide) ────────────────────────────
    let yr  = app.time_year;
    let mo  = app.time_month;
    let dy  = app.time_day;
    let loc_off = app.time_local_offset;

    // "Tomorrow" in the current month (wrap to next month → not shown in this grid).
    let days_in    = td::days_in_month(yr, mo);
    let tomorrow   = if dy < days_in { dy + 1 } else { 0 }; // 0 = no tomorrow in this month

    // Collect days shifted to a different date in any timezone.
    let mut shifted_days: Vec<u8> = Vec::new();
    for z in td::ZONES {
        let (_, zmo2, zd2, _, _, delta) =
            td::apply_offset_dated(yr, mo, dy, h, m, z.offset_on(yr, mo, dy));
        if delta != 0 && zmo2 == mo { shifted_days.push(zd2); }
    }

    // Header (centred over 28 chars)
    let cal_header = format!("{} {}", td::month_abbr(mo), yr);
    put_str(buf, cal_x, clock_row,
        &format!("{:^28}", cal_header),
        Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD));

    // Weekday header — 4 chars each: " Su  Mo  Tu  We  Th  Fr  Sa"
    put_str(buf, cal_x, clock_row + 1,
        " Su  Mo  Tu  We  Th  Fr  Sa",
        Style::default().fg(Color::DarkGray));

    // Day grid — 4 chars per cell.
    // Today:    "(dd)"   LightYellow + Bold
    // Tomorrow: "[dd]"   LightCyan
    // Shifted:  " dd*"   LightMagenta (timezone-shifted date in this month)
    // Normal:   "  dd"   Gray
    let first_dow = td::first_dow(yr, mo) as u16; // 0=Sun
    let mut col_pos = first_dow;
    let mut row_pos: u16 = 2;
    for d in 1u8..=days_in {
        let cx = cal_x + col_pos * 4;
        let cy = clock_row + row_pos;
        if cy < area.bottom() {
            let (label, style) = if d == dy {
                (format!("({:>2})", d),
                 Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD))
            } else if d == tomorrow {
                (format!("[{:>2}]", d),
                 Style::default().fg(Color::LightCyan))
            } else if shifted_days.contains(&d) {
                (format!(" {:>2}*", d),
                 Style::default().fg(Color::LightMagenta))
            } else {
                (format!("  {:>2}", d),
                 Style::default().fg(Color::Gray))
            };
            put_str(buf, cx, cy, &label, style);
        }
        col_pos += 1;
        if col_pos == 7 { col_pos = 0; row_pos += 1; }
    }

    // ── Time zones ─────────────────────────────────────────────────────────
    // The stored h/m/d is UTC.  Each zone: time = UTC + zone.offset_mins.
    // "My Time" uses time_local_offset which is DST-aware from the OS.
    struct TzRow<'a> { city: &'a str, abbr: &'a str, eff: i32 }
    let mut tz_entries: Vec<TzRow> = vec![
        TzRow { city: "My Time", abbr: "local", eff: loc_off },
    ];
    for z in td::ZONES {
        tz_entries.push(TzRow { city: z.city, abbr: z.abbr, eff: z.offset_on(yr, mo, dy) });
    }

    let tz_y  = clock_row + clock_lines.len() as u16 + 1;
    let n_tz  = tz_entries.len();
    let half  = n_tz / 2;
    let zone_col_w = ((area.width as usize).saturating_sub(2) / half).max(16) as u16;

    for (i, z) in tz_entries.iter().enumerate() {
        let (_, zmo2, zd2, zh2, zm2, delta) =
            td::apply_offset_dated(yr, mo, dy, h, m, z.eff);
        let row_i = (i / half) as u16;
        let col_i = (i % half) as u16;
        let zx    = area.left() + col_i * zone_col_w + 1;
        let zy    = tz_y + row_i * 2;
        if zy + 1 >= area.bottom() { break; }

        let time_str = td::fmt_time(zh2, zm2, use_24h);
        let date_info = if delta != 0 {
            format!(" {:+}day {}{}", delta, zd2, td::month_abbr(zmo2))
        } else { String::new() };

        let is_local   = i == 0;
        let time_color = if is_local { Color::LightYellow }
                         else if delta == 0 { Color::White }
                         else { Color::LightMagenta };
        put_str(buf, zx, zy,
            &format!("{:<13}{}{}", z.city, time_str, date_info),
            Style::default().fg(time_color));
        put_str(buf, zx, zy + 1,
            &format!("             ({})", z.abbr),
            Style::default().fg(Color::DarkGray));
    }

    // Footer
    put_str(buf, col, area.bottom().saturating_sub(1),
        "UTC offsets only (no DST)   (dd)=today  [dd]=tomorrow  dd*=tz shift  H=help",
        Style::default().fg(Color::DarkGray));

    // ── Time reference overlay (H key) ─────────────────────────────────────
    if app.time_help_active {
        draw_time_help_overlay(buf, area);
    }
}

fn draw_time_help_overlay(buf: &mut ratatui::buffer::Buffer, area: Rect) {
    use crate::time_display::TIME_FACTS;
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::widgets::{Block, Borders, Clear, Widget};

    // Clear background for the overlay
    let overlay = Rect {
        x:      area.x + 2,
        y:      area.y + 1,
        width:  area.width.saturating_sub(4),
        height: area.height.saturating_sub(2),
    };
    Clear.render(overlay, buf);

    let bx = overlay.x;
    let by = overlay.y;
    let bw = overlay.width;
    let bh = overlay.height;

    // Draw border manually
    for x in bx..bx+bw {
        buf[(x, by)]          .set_char('═').set_style(Style::default().fg(Color::Cyan));
        buf[(x, by+bh-1)]     .set_char('═').set_style(Style::default().fg(Color::Cyan));
    }
    for y in by..by+bh {
        buf[(bx, y)]          .set_char('║').set_style(Style::default().fg(Color::Cyan));
        buf[(bx+bw-1, y)]     .set_char('║').set_style(Style::default().fg(Color::Cyan));
    }
    buf[(bx, by)]             .set_char('╔').set_style(Style::default().fg(Color::Cyan));
    buf[(bx+bw-1, by)]        .set_char('╗').set_style(Style::default().fg(Color::Cyan));
    buf[(bx, by+bh-1)]        .set_char('╚').set_style(Style::default().fg(Color::Cyan));
    buf[(bx+bw-1, by+bh-1)]   .set_char('╝').set_style(Style::default().fg(Color::Cyan));

    // Title
    let title = "  TIME REFERENCE  ";
    let tx = bx + (bw / 2).saturating_sub(title.len() as u16 / 2);
    put_str(buf, tx, by, title,
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));

    // Two-column layout
    let inner_x  = bx + 2;
    let inner_y  = by + 2;
    let half_w   = (bw / 2).saturating_sub(2);
    let col2_x   = bx + bw / 2 + 1;

    let left_cats  = &TIME_FACTS[..3];
    let right_cats = &TIME_FACTS[3..];

    for (col_x, cats) in [(inner_x, left_cats), (col2_x, right_cats)] {
        let mut y = inner_y;
        for (cat_title, facts) in cats.iter() {
            if y + 1 >= by + bh { break; }
            put_str(buf, col_x, y, cat_title,
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
            y += 1;
            // Rule line
            for x in col_x..col_x+half_w {
                if x < bx + bw - 1 {
                    buf[(x, y)].set_char('─').set_style(Style::default().fg(Color::DarkGray));
                }
            }
            y += 1;
            for fact in facts.iter() {
                if y >= by + bh - 1 { break; }
                put_str(buf, col_x + 1, y, fact, Style::default().fg(Color::White));
                y += 1;
            }
            y += 1;
        }
    }

    // Footer
    let footer = " H or Esc: close ";
    let fx = bx + (bw / 2).saturating_sub(footer.len() as u16 / 2);
    put_str(buf, fx, by + bh - 1, footer,
        Style::default().fg(Color::DarkGray));
}

// -- Help / keyboard-shortcut screen ----------------------------------------

pub fn draw_help(f: &mut Frame, area: Rect) {
    use crate::time_display::HOTKEYS;

    let buf = f.buffer_mut();
    let bx  = area.x;
    let by  = area.y;
    let bw  = area.width;
    let bh  = area.height;

    // Background + border
    for y in by..by + bh {
        for x in bx..bx + bw {
            buf[(x, y)].set_char(' ').set_style(Style::default().bg(Color::Black));
        }
    }
    for x in bx..bx + bw {
        buf[(x, by)]      .set_char('═').set_style(Style::default().fg(Color::Cyan));
        buf[(x, by+bh-1)] .set_char('═').set_style(Style::default().fg(Color::Cyan));
    }
    for y in by..by + bh {
        buf[(bx, y)]       .set_char('║').set_style(Style::default().fg(Color::Cyan));
        buf[(bx+bw-1, y)]  .set_char('║').set_style(Style::default().fg(Color::Cyan));
    }
    buf[(bx, by)]           .set_char('╔').set_style(Style::default().fg(Color::Cyan));
    buf[(bx+bw-1, by)]      .set_char('╗').set_style(Style::default().fg(Color::Cyan));
    buf[(bx, by+bh-1)]      .set_char('╚').set_style(Style::default().fg(Color::Cyan));
    buf[(bx+bw-1, by+bh-1)] .set_char('╝').set_style(Style::default().fg(Color::Cyan));

    let title = "  KEYBOARD SHORTCUTS  ";
    let tx = bx + (bw / 2).saturating_sub(title.len() as u16 / 2);
    put_str(buf, tx, by, title,
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));

    // Two columns
    let inner_y  = by + 2;
    let half_w   = (bw / 2).saturating_sub(2);
    let col1_x   = bx + 2;
    let col2_x   = bx + bw / 2 + 1;
    let n        = HOTKEYS.len();
    let left_n   = (n + 1) / 2;

    let key_col_w = 14u16; // fixed width for the key column

    for (col_idx, range) in [(0usize, 0..left_n), (1, left_n..n)] {
        let col_x = if col_idx == 0 { col1_x } else { col2_x };
        let mut y = inner_y;

        for sec_idx in range {
            let (title, keys) = &HOTKEYS[sec_idx];
            if y >= by + bh - 1 { break; }

            // Section heading
            put_str(buf, col_x, y, title,
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
            y += 1;

            // Rule
            for x in col_x..col_x + half_w.min(bw / 2 - 1) {
                buf[(x, y)].set_char('─').set_style(Style::default().fg(Color::DarkGray));
            }
            y += 1;

            // Key / description rows
            for (key, desc) in keys.iter() {
                if y >= by + bh - 1 { break; }
                put_str(buf, col_x + 1, y,
                    &format!("{:<width$}", key, width = key_col_w as usize),
                    Style::default().fg(Color::LightCyan));
                let desc_x = col_x + 1 + key_col_w;
                if desc_x < bx + bw - 1 {
                    put_str(buf, desc_x, y, desc, Style::default().fg(Color::White));
                }
                y += 1;
            }
            y += 1; // gap
        }
    }

    // Footer
    let footer = " Esc · Enter · H: close ";
    let fx = bx + (bw / 2).saturating_sub(footer.len() as u16 / 2);
    put_str(buf, fx, by + bh - 1, footer,
        Style::default().fg(Color::DarkGray));
}
