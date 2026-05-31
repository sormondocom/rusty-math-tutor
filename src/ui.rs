//! All rendering: the start menu, the single-problem card (reused for
//! transition capture), the optional challenge HUD, and Deduction Duck's
//! help overlay.
//!
//! The card is drawn by [`render_card`], which writes directly into a
//! [`Buffer`].  That lets the same routine paint the live frame *and* be
//! captured into off-screen buffers by [`crate::transition`].

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Widget};
use ratatui::Frame;

use crate::app::{App, Feedback, Screen, TeacherView};
use crate::config::Layout;
use crate::duck::Pose;
use crate::font;
use crate::problem::{Op, Problem};
use crate::strategy::{Strategy, Viz};
use crate::{duck, problem, strategy};

/// Deduction Duck's feathers.
const DUCK_COLOR: Color = Color::Rgb(255, 214, 64);
/// Background tint of the help stage panel.
const STAGE_BG: Color = Color::Rgb(28, 30, 44);

/// Background colour shared by every screen.
const BG: Color = Color::Rgb(16, 18, 28);

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    // Paint the whole background first.
    Block::default()
        .style(Style::default().bg(BG))
        .render(area, f.buffer_mut());

    match app.screen {
        Screen::Startup => draw_startup(f, app, area),
        Screen::Menu => draw_menu(f, app, area),
        Screen::Settings => draw_settings(f, app, area),
        Screen::Stats => draw_stats(f, app, area),
        Screen::Teacher => draw_teacher(f, app, area),
        Screen::Cinematic => draw_cinematic(f, app, area),
        Screen::Practice | Screen::Challenge => draw_session(f, app, area),
    }
}

// ---------------------------------------------------------------------------
// Milestone cinematics
// ---------------------------------------------------------------------------

/// Draw one cinematic scene into `buf` (also used to capture scenes for the
/// transitions that animate between them).
pub fn render_scene(area: Rect, buf: &mut Buffer, scene: &crate::cinematic::Scene) {
    Block::default().style(Style::default().bg(BG)).render(area, buf);
    let accent = Style::default().fg(scene.accent);

    if let Some(big) = &scene.big {
        let by = (area.top() + area.height.saturating_sub(font::GLYPH_H) / 2).saturating_sub(2);
        font::draw_text(buf, center(area, font::text_width(big)), by, big, accent);
        let hy = by + font::GLYPH_H + 1;
        put_str(buf, center(area, scene.heading.chars().count() as u16), hy, &scene.heading, Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
        put_str(buf, center(area, scene.sub.chars().count() as u16), hy + 1, &scene.sub, accent.add_modifier(Modifier::BOLD));
    } else {
        let hy = area.top() + area.height / 2;
        put_str(buf, center(area, scene.heading.chars().count() as u16), hy.saturating_sub(1), &scene.heading, accent.add_modifier(Modifier::BOLD));
        put_str(buf, center(area, scene.sub.chars().count() as u16), hy + 1, &scene.sub, Style::default().fg(Color::White));
    }
}

fn draw_cinematic(f: &mut Frame, app: &App, area: Rect) {
    let buf = f.buffer_mut();
    if let Some(c) = &app.cinematic {
        if let Some(t) = &c.transition {
            t.render(area, buf);
        } else if let Some(scene) = c.scenes.get(c.index) {
            render_scene(area, buf, scene);
        }
    }
    put_str(buf, center(area, 21), area.bottom().saturating_sub(1), "press any key to skip", Style::default().fg(Color::DarkGray));
}

// ---------------------------------------------------------------------------
// Startup — graphics mode picker
// ---------------------------------------------------------------------------

fn draw_startup(f: &mut Frame, app: &App, area: Rect) {
    use crate::config::GraphicsMode;
    let buf = f.buffer_mut();

    let title = "1 + 1 = ?";
    font::draw_text(buf, center(area, font::text_width(title)), area.top() + 2, title, Style::default().fg(Color::LightCyan));
    put_str(buf, center(area, 17), area.top() + 8, "RUSTY MATH TUTOR", Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
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

// ---------------------------------------------------------------------------
// Menu
// ---------------------------------------------------------------------------

fn draw_menu(f: &mut Frame, app: &App, area: Rect) {
    let buf = f.buffer_mut();

    let title = "1 + 1 = ?";
    font::draw_text(buf, center(area, font::text_width(title)), area.top() + 1, title, Style::default().fg(Color::LightCyan));
    put_str(buf, center(area, 17), area.top() + 6, "RUSTY MATH TUTOR", Style::default().fg(Color::White).add_modifier(Modifier::BOLD));

    let mut y = area.top() + 8;
    let col = area.left() + area.width.saturating_sub(42) / 2;
    let sel = app.menu_index;

    // Student selector.
    draw_menu_row(buf, col, y, sel == 0, &format!("Student:  < {} >", app.roster.current().name), "(<>  N: add new)");
    y += 2;

    // Grade.
    draw_menu_row(buf, col, y, sel == 1, &format!("Grade level:  < {} >", grade_name(app.menu_grade)), "(Left / Right)");
    y += 2;

    // Operation toggles (rows 2..=5).
    for (i, op) in problem::Op::ALL.iter().enumerate() {
        let mark = if app.menu_ops[i] { "[x]" } else { "[ ]" };
        draw_menu_row(buf, col, y, sel == 2 + i, &format!("{} {}", mark, op.name()), "(Enter toggles)");
        y += 1;
    }
    y += 1;

    draw_menu_row(buf, col, y, sel == 6, &format!("Show problems:  {}", app.config.layout.name()), "(Enter to switch)");
    y += 1;
    draw_menu_row(buf, col, y, sel == 7, "Settings (number ranges)...", "(Enter to open)");
    y += 1;
    draw_menu_row(buf, col, y, sel == 8, "My Progress...", "(Enter to view)");
    y += 1;
    draw_menu_row(buf, col, y, sel == 9, "Teacher Area...", "(password)");
    y += 2;

    draw_menu_row(buf, col, y, sel == 10, "▶  Start Practice", "(no timer)");
    y += 1;
    draw_menu_row(buf, col, y, sel == 11, "▶  Start Challenge", "(60-second timer)");

    put_str(buf, col, area.bottom().saturating_sub(2), "Up / Down to move    N: add student    Q: quit", Style::default().fg(Color::DarkGray));

    // New-student name entry.
    if app.naming {
        draw_name_prompt(buf, area, &app.name_input);
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

// ---------------------------------------------------------------------------
// My Progress — encouragement only, never comparison
// ---------------------------------------------------------------------------

fn draw_stats(f: &mut Frame, app: &App, area: Rect) {
    let buf = f.buffer_mut();
    let s = app.roster.current();
    let total = s.total();
    let col = area.left() + area.width.saturating_sub(46) / 2;

    put_str(buf, col, area.top() + 2, format!("{}'s Math Journey", s.name), Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD));

    // Big celebratory total.
    let total_str = total.to_string();
    font::draw_text(buf, center(area, font::text_width(&total_str)), area.top() + 4, &total_str, Style::default().fg(Color::LightGreen));
    put_str(buf, center(area, 20), area.top() + 10, "problems solved!", Style::default().fg(Color::White).add_modifier(Modifier::BOLD));

    // Per-operation breakdown with friendly bars.
    let mut y = area.top() + 12;
    let max = s.solved.iter().copied().max().unwrap_or(0).max(1);
    for op in problem::Op::ALL {
        let n = s.solved[op.index()];
        let bar_w = 24u16;
        let filled = ((n as f32 / max as f32) * bar_w as f32).round() as u16;
        let bar: String = (0..bar_w).map(|i| if i < filled { '█' } else { '░' }).collect();
        put_str(buf, col, y, format!("{:<10} ", op.name()), Style::default().fg(Color::White));
        put_str(buf, col + 11, y, &bar, Style::default().fg(Color::LightGreen));
        put_str(buf, col + 11 + bar_w + 1, y, format!("{}", n), Style::default().fg(Color::LightYellow));
        y += 1;
    }
    y += 1;

    if s.best_streak > 0 {
        put_str(buf, col, y, format!("★ Best streak: {} in a row!", s.best_streak), Style::default().fg(Color::LightMagenta).add_modifier(Modifier::BOLD));
        y += 1;
    }

    // An encouraging message that grows with effort — never a comparison.
    let msg = encouragement(total);
    put_str(buf, col, y + 1, msg, Style::default().fg(Color::LightCyan));

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

// ---------------------------------------------------------------------------
// Teacher Area — password-gated anecdote editor + records admin
// ---------------------------------------------------------------------------

fn draw_teacher(f: &mut Frame, app: &App, area: Rect) {
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

    match app.teacher_view {
        TeacherView::Anecdotes => draw_teacher_anecdotes(buf, app, area, col),
        TeacherView::Records => draw_teacher_records(buf, app, area, col),
    }

    if let Some(msg) = &app.teacher_msg {
        put_str(buf, col, area.bottom().saturating_sub(3), msg, Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD));
    }
}

fn draw_teacher_anecdotes(buf: &mut Buffer, app: &App, area: Rect, col: u16) {
    let op = problem::Op::ALL[app.teacher_op];
    put_str(buf, col, area.top() + 3, format!("Operation:  < {} >", op.name()), Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
    put_str(buf, col, area.top() + 4, "Your own real-life examples shown in the Why? panel:", Style::default().fg(Color::Gray));

    // When adding, a wrapping input box claims the lower part of the screen.
    let input_box = if app.teacher_adding {
        let box_h = 8u16.min(area.height.saturating_sub(9)).max(4);
        let box_w = area.right().saturating_sub(col).saturating_sub(2).min(66).max(20);
        let box_y = area.bottom().saturating_sub(box_h + 2);
        Some(Rect { x: col, y: box_y, width: box_w, height: box_h })
    } else {
        None
    };
    let list_bottom = input_box.map_or(area.bottom().saturating_sub(2), |b| b.y.saturating_sub(1));

    let items = app.why_extras.items(op);
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
        put_str(buf, col, area.bottom().saturating_sub(2), "< > operation    A: add    Tab: records    Esc: log out", Style::default().fg(Color::DarkGray));
    }
}

fn draw_teacher_records(buf: &mut Buffer, app: &App, area: Rect, col: u16) {
    let sel_op = app.teacher_op;
    // Column header.
    let header = format!("{:<14}{:>6}{:>6}{:>6}{:>6}{:>8}", "Student", "Add", "Sub", "Mul", "Div", "Total");
    put_str(buf, col, area.top() + 3, &header, Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD));

    let mut y = area.top() + 4;
    for (i, s) in app.roster.students.iter().enumerate() {
        if y >= area.bottom().saturating_sub(4) {
            break;
        }
        let selected = i == app.teacher_rec_index;
        let base = if selected {
            Style::default().fg(Color::Black).bg(Color::LightCyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let row = format!(
            "{:<14}{:>6}{:>6}{:>6}{:>6}{:>8}",
            clip(&s.name, 14),
            s.solved[0],
            s.solved[1],
            s.solved[2],
            s.solved[3],
            s.total()
        );
        put_str(buf, col, y, &row, base);
        // Mark the selected section column on the selected row.
        if selected {
            let mark_x = col + 14 + sel_op as u16 * 6;
            put_str(buf, mark_x, y.saturating_sub(1), "▼", Style::default().fg(Color::LightYellow));
        }
        y += 1;
    }

    put_str(
        buf,
        col,
        area.bottom().saturating_sub(2),
        "Up/Down student   < > section   S: reset section   R: reset all   X: remove   Esc: log out",
        Style::default().fg(Color::DarkGray),
    );
}

// ---------------------------------------------------------------------------
// Settings — per-grade number ranges
// ---------------------------------------------------------------------------

/// Number of editable rows on the Settings screen (grade + four range knobs).
pub const SETTINGS_FIELDS: usize = 5;

fn draw_settings(f: &mut Frame, app: &App, area: Rect) {
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

// ---------------------------------------------------------------------------
// Practice / Challenge session
// ---------------------------------------------------------------------------

/// The area the problem card occupies, given the screen and full frame.
/// Shared with [`crate::app`] so transition capture lines up exactly.
pub fn card_area(screen: Screen, area: Rect) -> Rect {
    if screen == Screen::Challenge {
        Rect { y: area.y + 2, height: area.height.saturating_sub(2), ..area }
    } else {
        area
    }
}

fn draw_session(f: &mut Frame, app: &App, area: Rect) {
    // Reserve a HUD row at the top for challenge mode.
    let card_area = card_area(app.screen, area);
    if app.screen == Screen::Challenge {
        let hud = Rect { height: 2, ..area };
        draw_challenge_hud(f, app, hud);
    }

    // Card: either an in-flight transition or the current problem.
    if let Some(t) = &app.transition {
        t.render(card_area, f.buffer_mut());
    } else if app.challenge.as_ref().map_or(false, |c| c.finished) {
        draw_challenge_summary(f, app, card_area);
    } else {
        let banner = feedback_banner(app);
        render_card(card_area, f.buffer_mut(), &app.current, &app.input, app.config.layout, banner);
    }

    // Deduction Duck rides on top of everything when summoned.
    if app.help_in > 0.0 && app.transition.is_none() {
        draw_help_overlay(f, app, card_area);
    }
    // "Why am I learning this?" sits above even the duck.
    if app.why_active && app.transition.is_none() {
        draw_why_overlay(f, app, card_area);
    }
}

/// The Y overlay: a panel listing real-world uses of the current operation.
fn draw_why_overlay(f: &mut Frame, app: &App, area: Rect) {
    let heading = crate::motivation::heading(app.current.op);

    // Each tagged line carries a style: 0 bullet, 1 code header, 2 code, 3 why.
    let panel_w = area.width.saturating_sub(4).min(72).max(24).min(area.width);
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

    put_str(buf, inner.left(), inner.top(), clip(heading, inner.width), Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD));

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

/// Greedy word-wrap of `s` into lines no wider than `width` characters.
fn wrap_text(s: &str, width: u16) -> Vec<String> {
    wrap_prefixed("", "", s, width)
}

/// Hard character wrap: break `s` every `width` columns, preserving spaces.
/// Used for text-input echoes where the caret must always stay in view.
fn wrap_chars(s: &str, width: u16) -> Vec<String> {
    let width = width.max(1) as usize;
    let chars: Vec<char> = s.chars().collect();
    if chars.is_empty() {
        return vec![String::new()];
    }
    chars.chunks(width).map(|c| c.iter().collect()).collect()
}

/// Word-wrap `text` so that, once `first` is prepended to the first line and
/// `cont` to every continuation line, **no produced line exceeds `width`
/// columns**.  The prefixes are counted against the budget, which is the part
/// the plain wrapper missed.  A word longer than the budget still gets its own
/// line (it can only be clipped, never silently dropped).
fn wrap_prefixed(first: &str, cont: &str, text: &str, width: u16) -> Vec<String> {
    let first_budget = (width as usize).saturating_sub(first.chars().count()).max(1);
    let cont_budget = (width as usize).saturating_sub(cont.chars().count()).max(1);

    let mut bodies: Vec<String> = Vec::new();
    let mut line = String::new();
    let mut budget = first_budget;
    for word in text.split_whitespace() {
        if line.is_empty() {
            line = word.to_string();
        } else if line.chars().count() + 1 + word.chars().count() <= budget {
            line.push(' ');
            line.push_str(word);
        } else {
            bodies.push(std::mem::take(&mut line));
            line = word.to_string();
            budget = cont_budget; // every line after the first uses cont width
        }
    }
    if !line.is_empty() {
        bodies.push(line);
    }
    if bodies.is_empty() {
        bodies.push(String::new());
    }

    bodies
        .into_iter()
        .enumerate()
        .map(|(i, body)| if i == 0 { format!("{}{}", first, body) } else { format!("{}{}", cont, body) })
        .collect()
}

fn feedback_banner(app: &App) -> Option<(String, Color)> {
    match app.feedback {
        Feedback::None => None,
        Feedback::Correct => Some(("✓  Correct!  Quack-tastic!".to_string(), Color::LightGreen)),
        Feedback::Wrong => Some(("✗  Not yet — try again, or press H for help.".to_string(), Color::LightRed)),
    }
}

/// Draw a single problem card into `buf`.  Used live and for transition capture.
pub fn render_card(area: Rect, buf: &mut Buffer, p: &Problem, input: &str, layout: Layout, banner: Option<(String, Color)>) {
    // Bordered panel tinted with the problem's accent colour.
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(p.accent))
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
    font::draw_text(buf, x, y, &prompt, Style::default().fg(p.accent));
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

    let accent = Style::default().fg(p.accent);
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
            buf[(x, y_div)].set_symbol("─").set_fg(p.accent);
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

    let accent = Style::default().fg(p.accent);

    // Quotient (the answer) sits above the roof, over the dividend.
    font::draw_text(buf, quotient_x, q_y, &quotient, answer_style(input));

    // Roof: corner at the wall, bar across the dividend.
    if roof_y < inner.bottom() {
        buf[(wall_x.min(inner.right().saturating_sub(1)), roof_y)].set_symbol("┌").set_fg(p.accent);
        for x in (wall_x + 1)..house_right {
            buf[(x, roof_y)].set_symbol("─").set_fg(p.accent);
        }
    }
    // Wall down the left of the dividend.
    for y in (roof_y + 1)..(dvd_y + font::GLYPH_H).min(inner.bottom()) {
        buf[(wall_x.min(inner.right().saturating_sub(1)), y)].set_symbol("│").set_fg(p.accent);
    }

    // Divisor outside the wall, dividend inside.
    font::draw_text(buf, x0, dvd_y, &divisor, accent);
    font::draw_text(buf, dividend_x, dvd_y, &dividend, accent);
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
    let frac = remaining as f32 / total as f32;
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

// ---------------------------------------------------------------------------
// Deduction Duck help stage
// ---------------------------------------------------------------------------

/// The duck's stage: a panel sliding up from the bottom in which he acts out
/// the chosen strategy — walking a number line, smashing a number, or talking
/// it through.  Hint steps show first; the answer appears only once revealed.
fn draw_help_overlay(f: &mut Frame, app: &App, area: Rect) {
    let strats = strategy::strategies(&app.current);
    let idx = app.strategy_index % strats.len();
    let s = &strats[idx];

    // Panel geometry; slides up from below as `help_in` eases 0..1.
    let panel_w = area.width.saturating_sub(4).min(72).max(20).min(area.width);
    let panel_h = area.height.saturating_sub(2).min(18).max(9).min(area.height);
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

    // An encouraging word takes the top line when the student is struggling.
    let enc_off: u16 = if let Some(msg) = &app.encourage {
        put_str(buf, inner.left(), inner.top(), clip(msg, inner.width), Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD));
        1
    } else {
        0
    };

    // Header and footer controls.
    let header = format!("Way {}/{}: {}", idx + 1, strats.len(), s.title);
    put_str(buf, inner.left(), inner.top() + enc_off, clip(&header, inner.width), Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD));

    let mut controls = vec![if app.revealed { "R: hide answer" } else { "R: show answer" }.to_string()];
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

    let accent = app.current.accent;
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
    for i in 0..n {
        let x = x_at(i).min(content.right().saturating_sub(1));
        buf[(x, axis_y)].set_symbol("┼").set_fg(accent);
        let lbl = &labels[i];
        let lx = x.saturating_sub(lbl.len() as u16 / 2);
        let last = i == n - 1;
        let lstyle = if app.revealed && last {
            Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        put_str(buf, lx, label_y, lbl, lstyle);
    }
    // Hop labels above each segment.
    for i in 0..n - 1 {
        let xm = (x_at(i) + x_at(i + 1)) / 2;
        let h = &hops[i];
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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Draw a string only when its start cell is inside the buffer.  Ratatui's
/// `set_string` clips horizontally but panics on an out-of-range `y`, so this
/// guard keeps every label safe on tiny terminals.
fn put_str(b: &mut Buffer, x: u16, y: u16, s: impl AsRef<str>, style: Style) {
    let area = b.area;
    if x >= area.right() || y >= area.bottom() || x < area.left() || y < area.top() {
        return;
    }
    b.set_string(x, y, s.as_ref(), style);
}

/// Left x so that a span of `width` is centred within `area`.
fn center(area: Rect, width: u16) -> u16 {
    area.left() + area.width.saturating_sub(width) / 2
}

/// Truncate `s` to at most `width` characters (by char, ellipsis-free).
fn clip(s: &str, width: u16) -> String {
    s.chars().take(width as usize).collect()
}

/// The last `width` characters of `s` — keeps a text field's caret in view.
fn tail(s: &str, width: u16) -> String {
    let count = s.chars().count();
    let width = width as usize;
    if count <= width {
        s.to_string()
    } else {
        s.chars().skip(count - width).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_prefixed_keeps_every_line_within_width() {
        // The guarantee holds once the width clears the prefix plus the longest
        // word — the Why panel's real operating range.  (Below that, greedy
        // wrap can't split a word and the render pass clips as a safety net.)
        let why = "Dividing the width by the hops spaces the number-line stops out evenly.";
        for width in [30u16, 40, 50, 60] {
            let lines = wrap_prefixed("  Why it matters: ", "    ", why, width);
            assert!(lines.len() > 1, "text should wrap onto multiple lines at width {}", width);
            for line in &lines {
                assert!(
                    line.chars().count() <= width as usize,
                    "line {:?} ({} cols) exceeds width {}",
                    line,
                    line.chars().count(),
                    width
                );
            }
            assert!(lines[0].contains("Why it matters:"));
        }
    }

    #[test]
    fn wrap_chars_stays_within_width_and_preserves_content() {
        let s = "When I built a deck I added up every board length precisely.";
        for width in [10u16, 24, 40] {
            let lines = wrap_chars(s, width);
            for line in &lines {
                assert!(line.chars().count() <= width as usize);
            }
            // No characters are lost or invented.
            assert_eq!(lines.concat(), s);
        }
        assert_eq!(wrap_chars("", 8), vec![String::new()]);
    }

    #[test]
    fn wrap_text_handles_blank_and_single_word() {
        assert_eq!(wrap_text("", 10), vec![String::new()]);
        // A word longer than the width still survives as its own line.
        let lines = wrap_text("supercalifragilistic", 8);
        assert_eq!(lines.len(), 1);
    }
}
