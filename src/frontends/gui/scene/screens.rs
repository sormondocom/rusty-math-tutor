//! The full-screen, non-gameplay views: the startup graphics picker, the main
//! menu, the Settings knobs, the My Progress breakdown, the password-gated
//! Teacher Area (Why?-examples editor + records table), and the Experimentation
//! unit explorer.  Each is a `draw_*(pm, app, wf, hf)` entry called by `render`.

use tiny_skia::Pixmap;

use crate::app::{App, TeacherView, ANECDOTE_MAX};
use crate::student::ChallengeRecord;
use crate::topic::Topic;

const TOPIC_SYMBOLS: [&str; 8] = ["+", "\u{2212}", "\u{00D7}", "\u{00F7}", "units", "%", "\u{2044}", "\u{03C0}"];

use super::*;

// -- Startup + menu ---------------------------------------------------------

pub fn draw_startup(pm: &mut Pixmap, app: &App, wf: f32, hf: f32) {
    text_centered(pm, wf / 2.0, 90.0, 6.0, "RUSTY MATH TUTOR", WHITE);
    text_centered(pm, wf / 2.0, 170.0, 2.0, "Featuring:  Deduction Duck", YELLOW);
    text_centered(pm, wf / 2.0, 240.0, 2.0, "Choose how to draw:", GRAY);
    let modes = ["Low Graphics (console)", "CPU Graphics (window)"];
    let x = wf / 2.0 - 200.0;
    for (i, m) in modes.iter().enumerate() {
        let y = 300.0 + i as f32 * 56.0;
        let selected = i == app.startup_index;
        if selected {
            fill(pm, x - 16.0, y - 8.0, 420.0, 40.0, SEL_BG);
        }
        text(pm, x, y, 2.5, m, if selected { WHITE } else { GRAY });
    }
    text_centered(pm, wf / 2.0, hf - 50.0, 1.5, "Up / Down to choose    Enter to start", GRAY);
}

pub fn draw_menu(pm: &mut Pixmap, app: &App, wf: f32, hf: f32) {
    // Name-entry modal takes over the whole screen so the user can clearly see
    // they are in a text-input state and arrows / other keys don't get eaten.
    if app.naming {
        draw_name_prompt(pm, app, wf, hf);
        return;
    }

    // Scale 3.5 (was 5.0) — the title was too chunky and sat so close to
    // y=0 that it clipped into the chalk board's top frame in chalk themes.
    // y=38 gives a comfortable gap from both the chalk edge and the rows.
    text_centered(pm, wf / 2.0, 38.0, 3.5, "RUSTY MATH TUTOR", WHITE);

    let rows = menu_rows(app);
    let scale = 1.75;
    let row_h = 28.0;
    let top = 105.0; // was 130 — move up to reclaim the space the smaller title freed
    let box_s = scale * 9.0;
    let label_dx = box_s + scale * 7.0; // toggle rows indent past their checkbox
    let row_w = |r: &MenuRow| match r {
        MenuRow::Check(_, l) => label_dx + text_width(l, scale),
        MenuRow::Text(t) => text_width(t, scale),
    };
    // Skip the student row (index 0) when computing the layout width so the
    // rest of the menu never shifts when cycling between students with
    // different name lengths.  The student row is centred independently.
    let content_w = rows.iter().skip(1).map(row_w).fold(0.0_f32, f32::max);
    let x = (wf - content_w) / 2.0;
    let pad = 14.0;
    for (i, row) in rows.iter().enumerate() {
        let y = top + i as f32 * row_h;
        let selected = i == app.menu_index;
        let color = if selected { WHITE } else { GRAY };

        if i == 0 {
            // Student row: centred independently of x/content_w.  The highlight
            // bar is sized and positioned relative to the actual rendered text
            // so it tracks the name regardless of length.
            if let MenuRow::Text(t) = row {
                let tw = text_width(t, scale);
                let hx = wf / 2.0 - tw / 2.0 - pad;
                if selected {
                    fill(pm, hx, y - 5.0, tw + pad * 2.0, row_h - 2.0, SEL_BG);
                }
                text_centered(pm, wf / 2.0, y, scale, t, color);
            }
        } else {
            if selected {
                fill(pm, x - pad, y - 5.0, content_w + pad * 2.0, row_h - 2.0, SEL_BG);
            }
            match row {
                MenuRow::Check(on, label) => {
                    stroke_rect(pm, x, y, box_s, box_s, 2.0, color);
                    if *on {
                        fill(pm, x + 3.0, y + 3.0, box_s - 6.0, box_s - 6.0, ACCENT);
                    }
                    text(pm, x + label_dx, y, scale, label, color);
                }
                MenuRow::Text(t) => text(pm, x, y, scale, t, color),
            }
        }
    }
    text_centered(pm, wf / 2.0, hf - 54.0, 1.5, "Up / Down: move    Enter: select    Q: quit", GRAY);
    text_centered(pm, wf / 2.0, hf - 26.0, 1.5, "N: add new student    Left / Right: switch student", GRAY);

    // Left panel: "Featuring: Deduction Duck" label + duck, both centred on
    // the same column centre so they stay aligned at any window size.
    let left_w = x - 20.0;
    if left_w > 60.0 {
        // Centre of the left column — used for both label and duck so they
        // are always horizontally aligned with each other.
        let col_cx = 10.0 + left_w / 2.0;

        // Align with the menu rows exactly — same top, same bottom.
        let menu_top = top;
        let menu_bot = top + rows.len() as f32 * row_h;

        // Asset is portrait 850 × 1150.  Constrain duck height so the
        // display width never exceeds the column width.
        const W_OVER_H: f32 = 850.0 / 1150.0;
        let max_h_for_col = left_w / W_OVER_H;
        let duck_h = (menu_bot - menu_top).min(max_h_for_col).min(hf * 0.45).max(40.0);

        // Vertically centre the duck within the menu band.
        let group_top = menu_top + (menu_bot - menu_top - duck_h) / 2.0;

        // Label sits in the upper half of the label slot (vertically centred
        // within it) so there is visible space both above and below the text.
        let duck_y = group_top;
        draw_duck_png(pm, col_cx, duck_y, duck_h);
    }
}

fn draw_name_prompt(pm: &mut Pixmap, app: &App, wf: f32, hf: f32) {
    let cxc = wf / 2.0;
    let cy = hf / 2.0;

    text_centered(pm, cxc, cy - 110.0, 2.5, "Add a New Student", WHITE);
    text_centered(pm, cxc, cy - 68.0,  1.5, "Type the name, then press Enter to save.", GRAY);

    // Input box — SEL_BG background so it stands out clearly from the dark BG.
    let (bw, bh) = (580.0, 72.0);
    let bx = (wf - bw) / 2.0;
    let by = cy - 36.0;
    fill(pm, bx, by, bw, bh, SEL_BG);
    stroke_rect(pm, bx, by, bw, bh, 4.0, ACCENT);

    let shown = format!("{}_", app.name_input);
    text_centered(pm, cxc, by + 16.0, 2.5, &shown, WHITE);

    text_centered(pm, cxc, cy + 58.0, 1.5, "Enter: save    Esc: cancel", GRAY);
}

/// A menu row: a toggle with a drawn checkbox, or a plain text/setting row.
enum MenuRow {
    Check(bool, String),
    Text(String),
}

/// The menu rows, in the same order as the core's `menu_index`.
fn menu_rows(app: &App) -> Vec<MenuRow> {
    let grade = if app.menu_grade == 0 { "K".to_string() } else { app.menu_grade.to_string() };
    let ops = ["Add", "Subtract", "Multiply", "Divide"];
    let mut v = vec![
        MenuRow::Text(format!("Student:  < {} >", app.roster.current().name)),
        MenuRow::Text(format!("Grade level:  < {} >", grade)),
    ];
    for (i, name) in ops.iter().enumerate() {
        v.push(MenuRow::Check(app.menu_ops[i], name.to_string()));
    }
    v.push(MenuRow::Check(app.menu_units, "Units of Measure".to_string()));
    v.push(MenuRow::Check(app.menu_fractions, "Fractions".to_string()));
    v.push(MenuRow::Check(app.menu_percents, "Percentages".to_string()));
    v.push(MenuRow::Check(app.menu_geometry, "Geometry".to_string()));
    v.push(MenuRow::Text(">  Start Practice".to_string()));
    v.push(MenuRow::Text(format!(">  Start Challenge  ({}s)", app.roster.current().challenge_secs)));
    v.push(MenuRow::Text(">  Time Explorer".to_string()));
    v.push(MenuRow::Text(">  Experimentation".to_string()));
    v.push(MenuRow::Text("Settings...".to_string()));
    v.push(MenuRow::Text("My Progress...".to_string()));
    v.push(MenuRow::Text("Teacher Area...".to_string()));
    v.push(MenuRow::Text("Help / Key Guide...".to_string()));
    v
}

// -- Settings ---------------------------------------------------------------

pub fn draw_settings(pm: &mut Pixmap, app: &App, wf: f32, hf: f32) {
    let cxc = wf / 2.0;
    text_centered(pm, cxc, 50.0, 2.5, "SETTINGS - NUMBER RANGES", CYAN);
    text_centered(pm, cxc, 96.0, 1.5, "Tune how big the numbers get for each grade.", GRAY);

    let g = app.settings_grade;
    let r = app.config.range(g);
    let rows: [(&str, String); 7] = [
        ("Grade", format!("< {} >", grade_name(g))),
        ("Add / Subtract up to", format!("< {} >", r.add_max)),
        ("Multiply factors up to", format!("< {} >", r.mul_max)),
        ("Divide numbers up to", format!("< {} >", r.div_max)),
        ("Allow negative answers", format!("< {} >", if r.allow_negative { "Yes" } else { "No" })),
        ("Show problems", format!("< {} >", app.config.layout.name())),
        ("Theme", format!("< {} >", app.config.theme.name())),
    ];

    let block_w = 560.0;
    let x = (wf - block_w) / 2.0;
    let value_x = x + 360.0;
    let row_h = 52.0;
    let top = 162.0;
    for (i, (label, value)) in rows.iter().enumerate() {
        let y = top + i as f32 * row_h;
        let selected = i == app.settings_field;
        if selected {
            fill(pm, x - 16.0, y - 8.0, block_w + 32.0, 40.0, SEL_BG);
        }
        text(pm, x, y, 1.75, label, if selected { WHITE } else { GRAY });
        text(pm, value_x, y, 1.75, value, YELLOW);
    }

    text_centered(pm, cxc, hf - 44.0, 1.5, "Up / Down choose    Left / Right change    Esc save & back", GRAY);
}

fn grade_name(grade: u8) -> String {
    if grade == 0 {
        "K".to_string()
    } else {
        grade.to_string()
    }
}

// -- My Progress ------------------------------------------------------------

pub fn draw_stats(pm: &mut Pixmap, app: &App, wf: f32, hf: f32) {
    let s = app.roster.current();
    let total = s.grand_total();
    let cxc = wf / 2.0;

    text_centered(pm, cxc, 34.0, 2.5, &format!("{}'s Math Journey", s.name), CYAN);
    // The big celebratory total — every kind of problem counts.
    text_centered(pm, cxc, 78.0, 8.0, &total.to_string(), ACCENT);
    text_centered(pm, cxc, 178.0, 2.0, "problems solved!", WHITE);

    // A friendly bar per section (one per Topic), scaled to the student's own best.
    let rows: Vec<(&str, u32)> = Topic::ALL.iter().map(|&t| (t.short(), s.solved_for(t))).collect();
    let max = rows.iter().map(|(_, n)| *n).max().unwrap_or(0).max(1) as f32;
    const TRACK: Rgb = [44, 48, 64];
    let block_w = 520.0;
    let x = (wf - block_w) / 2.0;
    let bar_x = x + 150.0;
    let bar_w = block_w - 150.0 - 60.0;
    let row_h = 30.0;
    let mut y = 224.0;
    for (label, n) in &rows {
        text(pm, x, y, 1.4, label, WHITE);
        fill(pm, bar_x, y + 2.0, bar_w, 14.0, TRACK);
        fill(pm, bar_x, y + 2.0, bar_w * (*n as f32 / max), 14.0, ACCENT);
        text(pm, bar_x + bar_w + 12.0, y, 1.4, &n.to_string(), YELLOW);
        y += row_h;
    }

    // Grade-level sparkline — always rendered so students can see the section
    // even before they have any grade data.  Empty grades show a dim slot.
    {
        y += 12.0;
        let grade_max   = s.grade_solved.iter().copied().max().unwrap_or(0);
        let grade_max_f = grade_max.max(1) as f32;
        let bar_max_h   = 32.0_f32;
        let cell_w      = block_w / 9.0;
        let bw          = (cell_w * 0.55).max(8.0);
        let baseline    = y + bar_max_h;

        // Dim track line at the baseline so the section is always visible.
        fill(pm, x, baseline, block_w, 1.0, TRACK);

        for i in 0..9usize {
            let count  = s.grade_solved[i];
            let bar_h  = if count > 0 { (count as f32 / grade_max_f * bar_max_h).max(3.0) } else { 0.0 };
            let cell_x = x + i as f32 * cell_w;
            let bar_x  = cell_x + (cell_w - bw) / 2.0;

            if bar_h > 0.0 {
                fill(pm, bar_x, baseline - bar_h, bw, bar_h, CYAN);
            }

            // Grade label centred below the baseline.
            let gname = if i == 0 { "K".to_string() } else { i.to_string() };
            let lw    = text_width(&gname, 1.3);
            let color = if count > 0 { WHITE } else { GRAY };
            text(pm, cell_x + (cell_w - lw) / 2.0, baseline + 4.0, 1.3, &gname, color);

            // Count below the label, only for non-zero grades.
            if count > 0 {
                let ns = count.to_string();
                let nw = text_width(&ns, 1.2);
                text(pm, cell_x + (cell_w - nw) / 2.0, baseline + 18.0, 1.2, &ns, YELLOW);
            }
        }

        y = baseline + 36.0;
    }

    if s.best_streak > 0 {
        text_centered(pm, cxc, y, 1.6, &format!("\u{2605} Best streak: {} in a row!", s.best_streak), YELLOW);
        y += 30.0;
    }
    text_centered(pm, cxc, y, 1.5, encouragement(total), CYAN);
    y += 34.0;

    // Challenge history — most recent entries, newest first.
    draw_challenge_history_table(pm, &s.challenge_history, cxc, y, hf);

    text_centered(pm, cxc, hf - 44.0, 1.5, "Every problem makes you stronger.    Esc: back", GRAY);
}

/// A warm message keyed only to the student's own total — never a comparison.
fn encouragement(total: u32) -> &'static str {
    match total {
        0 => "Your journey starts with the very first problem. Let's go!",
        1..=9 => "You're off to a great start - keep that curiosity going!",
        10..=49 => "Look at you go! Your brain is getting stronger every day.",
        50..=149 => "Fantastic effort - you're becoming a real problem-solver!",
        150..=499 => "Incredible dedication. You should be proud of yourself!",
        _ => "You're a math powerhouse. Deduction Duck is so proud!",
    }
}

// -- Teacher Area -----------------------------------------------------------

/// Short section headers for the records table, in [`Topic::ALL`] order.
const REC_COLS: [&str; 8] = ["Add", "Sub", "Mul", "Div", "Un", "Fr", "Pct", "Geo"];

pub fn draw_teacher(pm: &mut Pixmap, app: &App, wf: f32, hf: f32) {
    if app.teacher_authed {
        draw_teacher_manage(pm, app, wf, hf);
    } else {
        draw_teacher_login(pm, app, wf, hf);
    }
}

fn draw_teacher_login(pm: &mut Pixmap, app: &App, wf: f32, hf: f32) {
    let cxc = wf / 2.0;
    text_centered(pm, cxc, 80.0, 3.0, "TEACHER AREA", CYAN);

    let setting = !app.config.has_teacher_password();
    let prompt = if setting { "Create a teacher password:" } else { "Enter teacher password:" };
    text_centered(pm, cxc, 168.0, 1.75, prompt, WHITE);

    let masked: String = "*".repeat(app.teacher_pw.chars().count());
    text_centered(pm, cxc, 208.0, 2.5, &format!("{}_", masked), ACCENT);

    if setting {
        text_centered(pm, cxc, 268.0, 1.4, "(Kids can't reach these tools without it.)", GRAY);
    }
    if let Some(msg) = &app.teacher_msg {
        text_centered(pm, cxc, 308.0, 1.5, msg, YELLOW);
    }
    text_centered(pm, cxc, hf - 48.0, 1.5, "Enter: continue    Esc: back", GRAY);
}

fn draw_teacher_manage(pm: &mut Pixmap, app: &App, wf: f32, hf: f32) {
    // Tabs.
    let s = 1.6;
    let (l0, l1) = ("Why? Examples", "Student Records");
    let (w0, w1, gap) = (text_width(l0, s) + 28.0, text_width(l1, s) + 28.0, 24.0);
    let tx = (wf - (w0 + gap + w1)) / 2.0;
    teacher_tab(pm, tx, 30.0, s, l0, app.teacher_view == TeacherView::Anecdotes);
    teacher_tab(pm, tx + w0 + gap, 30.0, s, l1, app.teacher_view == TeacherView::Records);

    match app.teacher_view {
        TeacherView::Anecdotes => draw_teacher_anecdotes(pm, app, wf, hf),
        TeacherView::Records => draw_teacher_records(pm, app, wf, hf),
    }

    // Locality + any status message, just above the footer.
    text_centered(pm, wf / 2.0, hf - 92.0, 1.4, &format!("Units locality: {}    (L: change)", app.config.locality.name()), CYAN);
    if let Some(msg) = &app.teacher_msg {
        text_centered(pm, wf / 2.0, hf - 62.0, 1.5, msg, ACCENT);
    }
}

/// Draw one tab box; the active one gets the selection fill.
fn teacher_tab(pm: &mut Pixmap, x: f32, y: f32, scale: f32, label: &str, active: bool) {
    if active {
        fill(pm, x, y - 6.0, text_width(label, scale) + 28.0, 34.0, SEL_BG);
    }
    text(pm, x + 14.0, y, scale, label, if active { WHITE } else { GRAY });
}

fn draw_teacher_anecdotes(pm: &mut Pixmap, app: &App, wf: f32, hf: f32) {
    let cxc = wf / 2.0;
    let topic = Topic::ALL[app.teacher_topic];
    text_centered(pm, cxc, 96.0, 1.75, &format!("Section:  < {} >", topic.name()), WHITE);
    text_centered(pm, cxc, 138.0, 1.3, "Your own real-life examples shown in the Why? panel:", GRAY);

    let items = app.why_extras.items(topic);
    let x = (wf - 640.0) / 2.0;
    let list_bottom = if app.teacher_adding { hf - 280.0 } else { hf - 120.0 };
    let mut y = 176.0;
    if items.is_empty() {
        text(pm, x + 16.0, y, 1.4, "(none yet - press A to add one)", GRAY);
    } else {
        for it in items {
            if y > list_bottom {
                break;
            }
            text(pm, x, y, 1.4, &format!("- {}", clip_chars(it, 78)), WHITE);
            y += 26.0;
        }
    }

    if app.teacher_adding {
        let (bx, bw, bh) = (x, 640.0, 150.0);
        let by = hf - 250.0;
        fill(pm, bx, by, bw, bh, CARD_BG);
        stroke_rect(pm, bx, by, bw, bh, 2.0, YELLOW);
        text(pm, bx + 12.0, by - 26.0, 1.4, "New example", YELLOW);

        // Echo the typed text wrapped to the box, with a caret; keep the tail in view.
        let lines = wrap_words(&format!("{}_", app.teacher_text), bw - 28.0, 1.4);
        let rows = ((bh - 24.0) / 24.0) as usize;
        let start = lines.len().saturating_sub(rows);
        for (i, line) in lines[start..].iter().enumerate() {
            text(pm, bx + 14.0, by + 12.0 + i as f32 * 24.0, 1.4, line, WHITE);
        }

        text(pm, bx, by + bh + 8.0, 1.3, "Enter: save    Esc: cancel", GRAY);
        text_right(pm, bx + bw, by + bh + 8.0, 1.3, &format!("{}/{}", app.teacher_text.chars().count(), ANECDOTE_MAX), GRAY);
    } else {
        text_centered(pm, cxc, hf - 30.0, 1.4, "< > section    A: add    Tab: records    Esc: log out", GRAY);
    }
}

fn draw_teacher_records(pm: &mut Pixmap, app: &App, wf: f32, hf: f32) {
    let (name_w, col_w, total_w, lock_w, timer_w) = (150.0, 52.0, 80.0, 70.0, 70.0);
    let table_w = name_w + 8.0 * col_w + total_w + lock_w + timer_w;
    let x0 = (wf - table_w) / 2.0;
    let scale = 1.3;
    // Right edge of section column `i` (with a little padding inside the cell).
    let col_right = |i: usize| x0 + name_w + (i as f32 + 1.0) * col_w - 8.0;

    // Header row.
    let hy = 100.0;
    text(pm, x0, hy, scale, "Student", GRAY);
    for (i, c) in REC_COLS.iter().enumerate() {
        if i == app.teacher_topic {
            fill(pm, x0 + name_w + i as f32 * col_w, hy - 4.0, col_w, 24.0, YELLOW);
        }
        let color = if i == app.teacher_topic { CARD_BG } else { GRAY };
        text_right(pm, col_right(i), hy, scale, c, color);
    }
    text_right(pm, x0 + name_w + 8.0 * col_w + total_w - 8.0, hy, scale, "Total", GRAY);
    let lock_right  = x0 + name_w + 8.0 * col_w + total_w + lock_w - 8.0;
    let timer_right = x0 + table_w - 8.0;
    text_right(pm, lock_right,  hy, scale, "Lock",  GRAY);
    text_right(pm, timer_right, hy, scale, "Timer", GRAY);

    // Student rows, scrolled to keep the selection visible.
    let students = &app.roster.students;
    let top = 132.0;
    let row_h = 26.0;
    let visible = (((hf - 150.0) - top) / row_h).max(1.0) as usize;
    let scroll = app.teacher_rec_index.saturating_sub(visible.saturating_sub(1));
    let mut y = top;
    for (i, st) in students.iter().enumerate().skip(scroll).take(visible) {
        let selected = i == app.teacher_rec_index;
        if selected {
            fill(pm, x0 - 10.0, y - 4.0, table_w + 20.0, row_h - 2.0, SEL_BG);
        }
        let name_color = if selected { WHITE } else { GRAY };
        text(pm, x0, y, scale, &clip_chars(&st.name, 14), name_color);
        for (c, &t) in Topic::ALL.iter().enumerate() {
            text_right(pm, col_right(c), y, scale, &st.solved_for(t).to_string(), WHITE);
        }
        text_right(pm, x0 + name_w + 8.0 * col_w + total_w - 8.0, y, scale, &st.grand_total().to_string(), CYAN);
        let lock  = if st.reveal_lock == 0 { "off".to_string() } else { st.reveal_lock.to_string() };
        let timer = format!("{}s", st.challenge_secs);
        text_right(pm, lock_right,  y, scale, &lock,  YELLOW);
        text_right(pm, timer_right, y, scale, &timer, CYAN);
        y += row_h;
    }

    text_centered(pm, wf / 2.0, hf - 30.0, 1.3,
        "Up/Dn student   < > section   S/R reset   X remove   C clear history   +/- lock   [ ] timer   Esc out", GRAY);
}

/// Draw `s` right-aligned so its right edge sits at `right`.
fn text_right(pm: &mut Pixmap, right: f32, y: f32, scale: f32, s: &str, color: Rgb) {
    text(pm, right - text_width(s, scale), y, scale, s, color);
}

/// Truncate to at most `n` chars, adding an ellipsis when clipped.
fn clip_chars(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(n.saturating_sub(1)).collect();
        t.push('…');
        t
    }
}

// -- Experimentation --------------------------------------------------------

pub fn draw_experiment(pm: &mut Pixmap, app: &App, wf: f32, hf: f32) {
    use crate::units::{self, Category};

    let cat = Category::ALL[app.exp_category];
    let units_list = cat.units();
    let from = units_list[app.exp_from.min(units_list.len() - 1)];
    let to   = units_list[app.exp_to  .min(units_list.len() - 1)];
    let amount: f64 = app.exp_amount.parse().unwrap_or(0.0);
    let result = units::convert(amount, from, to);

    // Card.
    let cw = (wf * 0.86).min(900.0);
    let ch = (hf * 0.80).min(620.0);
    let (cx0, cy0) = ((wf - cw) / 2.0, (hf - ch) / 2.0);
    let cxc = wf / 2.0;
    fill(pm, cx0, cy0, cw, ch, CARD_BG);
    stroke_rect(pm, cx0, cy0, cw, ch, 2.0, MAGENTA);

    text_centered(pm, cxc, cy0 + 40.0, 2.2, "Experimentation - Unit Explorer", MAGENTA);
    text_centered(pm, cxc, cy0 + 84.0, 1.3, "Type any amount - even a silly one - and watch it convert!", GRAY);

    // Split the card: left 58 % for fields + result, right 42 % for the duck.
    let left_w  = cw * 0.58;
    let right_x = cx0 + left_w;       // where the duck column begins

    // --- Left column: editable fields ---
    let amount_display = if app.exp_amount.is_empty() { "0".to_string() } else { app.exp_amount.clone() };
    let caret = if app.exp_field == 0 { "_" } else { "" };
    let rows = [
        ("Amount",   format!("{}{}", amount_display, caret)),
        ("From",     format!("< {} >", from.plural)),
        ("To",       format!("< {} >", to.plural)),
        ("Category", format!("< {} >", cat.name())),
    ];
    let label_x = cx0 + 36.0;
    let value_x = label_x + 160.0;
    let row_h   = 46.0;
    let top     = cy0 + 140.0;

    for (i, (label, value)) in rows.iter().enumerate() {
        let y = top + i as f32 * row_h;
        let selected = i == app.exp_field;
        if selected {
            // Highlight only the label + value area — clipped to the left column.
            let hi_w = (value_x - (label_x - 12.0) + text_width(value, 1.75) + 16.0)
                .min(left_w - 24.0);
            fill(pm, label_x - 12.0, y - 8.0, hi_w, 38.0, MAGENTA);
        }
        let (lc, vc) = if selected { (CARD_BG, CARD_BG) } else { (WHITE, CYAN) };
        text(pm, label_x, y, 1.75, label, lc);
        text(pm, value_x, y, 1.75, value, vc);
    }

    // Conversion result, centred in the left column.
    let left_cx = cx0 + left_w / 2.0;
    let res_y   = cy0 + ch * 0.64;
    text_centered(pm, left_cx, res_y,        1.6, &format!("{} {} =", units::format_amount(amount), from.plural), GRAY);
    text_centered(pm, left_cx, res_y + 42.0, 3.2, &format!("{} {}", units::format_amount(result), to.plural), CYAN);

    // --- Right column: Deduction Duck reaction ---
    let base_value = amount * from.to_base;
    draw_exp_reaction(pm, cx0, cy0, ch, right_x, cw - left_w, app.anim_frame,
                      exp_reaction(cat, base_value));

    text_centered(pm, cxc, cy0 + ch - 30.0, 1.4,
        "Up/Down field   Left/Right change   type digits   Esc menu", GRAY);
}

enum ExpReaction {
    None,
    /// The amount is about the size of a real-world thing.
    Match(String),
    /// Astronomically bigger than anything on the list — hat-explode time.
    Boom(String),
}

/// Decide the duck's reaction from the *physical* size in base units (so it's
/// the same whichever target unit is on screen).  Mirrors the terminal frontend.
fn exp_reaction(cat: crate::units::Category, base_value: f64) -> ExpReaction {
    let refs = crate::units::comparisons(cat);
    if refs.is_empty() || base_value <= 0.0 {
        return ExpReaction::None;
    }
    let best = refs.iter().min_by(|a, b| {
        let (da, db) = ((base_value / a.base).ln().abs(), (base_value / b.base).ln().abs());
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

/// Draw the duck reaction inside the right column `[col_x, col_x + col_w]`.
/// Text is word-wrapped to stay within the column so it never bleeds into the
/// fields on the left.
#[allow(clippy::too_many_arguments)]
fn draw_exp_reaction(
    pm: &mut Pixmap,
    _cx0: f32, cy0: f32, ch: f32,
    col_x: f32, col_w: f32,
    frame: u64,
    reaction: ExpReaction,
) {
    use std::f32::consts::TAU;
    let (fact, boom) = match reaction {
        ExpReaction::None => return,
        ExpReaction::Match(f) => (f, false),
        ExpReaction::Boom(f) => (f, true),
    };

    let col_cx = col_x + col_w / 2.0;

    // Duck sized to fit comfortably inside the column.
    let cell   = 11.0_f32.min(col_w / 12.0);
    let duck_w = 9.0 * cell;
    let duck_h = 7.0 * cell * 1.25;
    let duck_x = col_x + (col_w - duck_w) / 2.0;
    // Centre the duck vertically in the card, leaving room for text above.
    let duck_y = cy0 + (ch - duck_h) / 2.0 + 20.0;

    // Word-wrap the fact text to the column width so it never overlaps left.
    let fact_scale = 1.3_f32;
    let max_text_w = col_w - 16.0;
    let lines = wrap_words(&fact, max_text_w, fact_scale);

    let (headline, hc) = if boom { ("BOOM!", RED) } else { ("Whoa!", YELLOW) };
    let n_lines   = lines.len() as f32;
    let line_h    = fact_scale * PX_PER_SCALE + 4.0;
    let text_block_h = 2.2 * PX_PER_SCALE + 6.0 + n_lines * line_h;
    // Place text block above the duck with a small gap.
    let text_top  = duck_y - text_block_h - 10.0;

    text_centered(pm, col_cx, text_top, 2.2, headline, hc);
    let mut ty = text_top + 2.2 * PX_PER_SCALE + 6.0;
    for line in &lines {
        text_centered(pm, col_cx, ty, fact_scale, line, hc);
        ty += line_h;
    }

    draw_duck(pm, duck_x, duck_y, DuckPose::Stand, frame / 5, cell, DUCK);

    if boom {
        let (hx, hy) = (col_cx, duck_y + duck_h * 0.1);
        let r = 26.0 + (frame % 12) as f32 * 3.0;
        for k in 0..6 {
            let ang = k as f32 * TAU / 6.0;
            text_centered(pm, hx + ang.cos() * r, hy + ang.sin() * r * 0.6, 1.4, "*", RED);
        }
    } else if (frame / 5).is_multiple_of(2) {
        text_centered(pm, duck_x - 10.0, duck_y + duck_h * 0.2, 1.4, "*", YELLOW);
        text_centered(pm, duck_x + duck_w + 10.0, duck_y + duck_h * 0.2, 1.4, "*", YELLOW);
    }
}

// -- Shared challenge-history table (used by draw_stats and draw_challenge_end)

/// Render a compact table of the last ≤5 challenge runs, newest first.
/// `cx` is the horizontal centre; `y0` is the top of the table.
fn draw_challenge_history_table(pm: &mut Pixmap, history: &[ChallengeRecord], cx: f32, y0: f32, hf: f32) {
    if history.is_empty() { return; }

    let block_w = 520.0_f32;
    let lx = cx - block_w / 2.0;
    let available = hf - 60.0 - y0; // keep footer clear
    if available < 30.0 { return; }

    text(pm, lx, y0, 1.6, "Challenge History", CYAN);
    let header_y = y0 + 22.0;
    const COLS: [(&str, f32); 4] = [("Solved", 0.0), ("/min", 120.0), ("Accuracy", 220.0), ("Streak", 370.0)];
    for (label, dx) in COLS {
        text(pm, lx + dx, header_y, 1.3, label, GRAY);
    }

    let row_h = 22.0_f32;
    let max_rows = ((available - 50.0) / row_h).floor().min(5.0) as usize;
    let entries: Vec<&ChallengeRecord> = history.iter().rev().take(max_rows).collect();

    for (i, rec) in entries.iter().enumerate() {
        let ry = header_y + 18.0 + i as f32 * row_h;
        let solved_str = format!("{}/{:.0}s", rec.solved, rec.duration_secs as f32);
        let rate_str   = format!("{:.1}", rec.rate());
        let acc_str    = format!("{}%  ({}/{})", rec.accuracy_pct(), rec.solved, rec.attempts);
        let streak_str = rec.streak_peak.to_string();
        text(pm, lx,         ry, 1.3, &solved_str, WHITE);
        text(pm, lx + 120.0, ry, 1.3, &rate_str,   YELLOW);
        text(pm, lx + 220.0, ry, 1.3, &acc_str,
            if rec.accuracy_pct() >= 80 { ACCENT } else if rec.accuracy_pct() >= 60 { YELLOW } else { RED });
        text(pm, lx + 370.0, ry, 1.3, &streak_str, YELLOW);
    }

    if history.len() > max_rows {
        let remaining = history.len() - max_rows;
        let note_y = header_y + 18.0 + max_rows as f32 * row_h;
        text(pm, lx, note_y, 1.2, &format!("+ {} more run{}", remaining, if remaining == 1 { "" } else { "s" }), GRAY);
    }
}

// -- Challenge End (summary) ------------------------------------------------

pub fn draw_challenge_end(pm: &mut Pixmap, app: &App, wf: f32, hf: f32) {
    let Some(run) = &app.run else { return; };
    let cx = wf / 2.0;

    // Title
    text_centered(pm, cx, 46.0, 4.0, "CHALLENGE COMPLETE!", ACCENT);

    // Big solved number
    text_centered(pm, cx, 110.0, 10.0, &run.solved.to_string(), WHITE);
    text_centered(pm, cx, 210.0, 2.5,
        &format!("problem{} in {}s", if run.solved == 1 { "" } else { "s" }, run.duration_secs),
        CYAN);

    // Stats block
    let block_w = 500.0_f32;
    let lx = (wf - block_w) / 2.0;   // left edge of stat labels
    let vx = lx + 210.0;              // left edge of values
    let mut y = 270.0_f32;
    let row = 38.0_f32;

    // Rate
    let rate = if run.duration_secs > 0 {
        run.solved as f32 * 60.0 / run.duration_secs as f32
    } else { 0.0 };
    text(pm, lx, y, 1.8, "Per minute:", GRAY);
    text(pm, vx, y, 1.8, &format!("{:.1}", rate), YELLOW);
    y += row;

    // Accuracy
    if run.attempts > 0 {
        let pct = (run.solved as f32 / run.attempts as f32 * 100.0).round() as u32;
        text(pm, lx, y, 1.8, "Accuracy:", GRAY);
        text(pm, vx, y, 1.8,
            &format!("{}%  ({} / {} answers)", pct, run.solved, run.attempts),
            if pct >= 80 { ACCENT } else if pct >= 60 { YELLOW } else { RED });
        y += row;
    }

    // Best streak
    if run.streak_peak > 0 {
        text(pm, lx, y, 1.8, "Best streak:", GRAY);
        text(pm, vx, y, 1.8, &format!("{} in a row", run.streak_peak), YELLOW);
        y += row;
    }

    // By-topic breakdown (only show non-zero topics)
    let active_topics: Vec<(usize, u32)> = run.by_topic.iter()
        .copied().enumerate().filter(|(_, n)| *n > 0).collect();
    if !active_topics.is_empty() {
        y += 8.0;
        text(pm, lx, y, 1.8, "By type:", GRAY);
        let mut tx = vx;
        for (i, n) in &active_topics {
            let s = format!("{} {}", TOPIC_SYMBOLS[*i], n);
            text(pm, tx, y, 1.8, &s, WHITE);
            tx += text_width(&s, 1.8) + 24.0;
        }
        y += row;
    }

    // Grade sparkline
    let grade_max = *run.by_grade.iter().max().unwrap_or(&0);
    if grade_max > 0 {
        y += 8.0;
        text(pm, lx, y, 1.6, "By grade:", GRAY);
        let spark_x = vx;
        let spark_w = block_w - (vx - lx);
        let cell_w  = spark_w / 9.0;
        let bw      = (cell_w * 0.55).max(6.0);
        let bar_max_h = 24.0_f32;
        let baseline  = y + bar_max_h + 4.0;
        fill(pm, spark_x, baseline, spark_w, 1.0, [44, 48, 64]);
        for i in 0..9usize {
            let n = run.by_grade[i];
            let bar_h = if n > 0 { (n as f32 / grade_max as f32 * bar_max_h).max(3.0) } else { 0.0 };
            let cx_i  = spark_x + i as f32 * cell_w;
            let bx    = cx_i + (cell_w - bw) / 2.0;
            if bar_h > 0.0 { fill(pm, bx, baseline - bar_h, bw, bar_h, CYAN); }
            let gname = if i == 0 { "K".to_string() } else { i.to_string() };
            let lw    = text_width(&gname, 1.2);
            text(pm, cx_i + (cell_w - lw) / 2.0, baseline + 4.0, 1.2, &gname,
                if n > 0 { WHITE } else { GRAY });
        }
        y = baseline + 28.0;
    }
    let _ = y;

    // Footer
    text_centered(pm, cx, hf - 44.0, 1.5,
        "R / Enter: play again    M / Esc: menu", GRAY);
}

// -- Time Explorer ----------------------------------------------------------

pub fn draw_time(pm: &mut Pixmap, app: &App, wf: f32, hf: f32) {
    use crate::time_display as td;
    use std::f32::consts::TAU;

    let h  = app.time_hour;
    let m  = app.time_min;
    let s  = app.time_sec;
    let yr = app.time_year;
    let mo = app.time_month;
    let dy = app.time_day;
    let cxc = wf / 2.0;

    // ── Prominent local time display (top of screen) ───────────────────────
    // Convert stored UTC → local so kids see their own wall-clock time first.
    let use_24h = app.config.hour_format.is_24h();
    let (lyr, lmo, ldy, lh_loc, lm_loc, _) =
        td::apply_offset_dated(yr, mo, dy, h, m, app.time_local_offset);
    let (l12, lam) = td::to_12h(lh_loc);
    // HH:MM — large, centred hero text.
    let main_part = if use_24h {
        format!("{:02}:{:02}", lh_loc, lm_loc)
    } else {
        format!("{}:{:02}", l12, lm_loc)
    };
    // Seconds and AM/PM — rendered at a smaller scale to the right.
    let sec_part  = format!(":{:02}", s);
    let ampm_part = if use_24h { String::new() }
                    else { format!(" {}", if lam { "AM" } else { "PM" }) };
    let big_date = format!("{}, {} {} {}",
        td::weekday_name(td::first_dow(lyr, lmo)),
        td::month_name(lmo), ldy, lyr);

    // Composite render: two scales, vertically centred on the same baseline zone.
    // Main scale 4.5 → 36 px tall.  Secondary scale 2.6 → 20.8 px tall.
    let sc_main = 4.5_f32;
    let sc_sec  = 2.6_f32;
    let main_h  = sc_main * PX_PER_SCALE;
    let sec_h   = sc_sec  * PX_PER_SCALE;
    let top_y   = 12.0_f32;

    let w_main = text_width(&main_part, sc_main);
    let w_sec  = text_width(&sec_part,  sc_sec);
    let w_ampm = text_width(&ampm_part, sc_sec);
    let total_big_w = w_main + w_sec + w_ampm;
    let x0 = cxc - total_big_w / 2.0;

    // Main HH:MM — sits at top_y
    text(pm, x0, top_y, sc_main, &main_part, WHITE);
    // Seconds — vertically centred beside main
    let sec_y = top_y + (main_h - sec_h) / 2.0;
    text(pm, x0 + w_main,          sec_y, sc_sec, &sec_part,  [180, 180, 180]);
    text(pm, x0 + w_main + w_sec,  sec_y, sc_sec, &ampm_part, [140, 140, 140]);

    // Date: leave an 8 px gap below the big time.
    text_centered(pm, cxc, top_y + main_h + 8.0, 1.8, &big_date, GRAY);
    // Compact hint line below date.
    // date bottom = top_y + main_h + 8 + 1.8*8 = 12 + 36 + 8 + 14.4 = 70.4
    let hint_y = top_y + main_h + 8.0 + 1.8 * PX_PER_SCALE + 8.0; // ≈ 78
    text_centered(pm, cxc, hint_y, 1.1,
        "TIME EXPLORER  ·  H: help  ·  T: 12/24h  ·  N: now  ·  Esc: menu",
        if app.time_auto { ACCENT } else { GRAY });

    // ── UTC reference row (editable — secondary display) ───────────────────
    let field_col = |f: u8| if app.time_field == f { YELLOW } else { WHITE };
    let is_f = |f: u8| app.time_field == f;

    let input_y = hint_y + 1.1 * PX_PER_SCALE + 8.0; // 10 px gap after hint
    let sc = 1.5; // deliberately compact — this is not the hero number

    // Build the date string pieces for the UTC reference row.
    let (h12, am) = td::to_12h(h);
    let hh_s  = if use_24h { format!("{:02}", h) } else { format!("{}", h12) };
    let ampm_s = if use_24h { String::new() } else { if am { " AM".into() } else { " PM".into() } };
    let mm_s  = format!("{:02}", m);
    let ss_s  = format!("{:02}", s);
    let dy_s  = format!("{:02}", dy);
    let mo_s  = td::month_abbr(mo).to_string();
    let yr_s  = format!("{}", yr);
    let sep   = "  ";

    let w_hh   = text_width(&hh_s, sc);
    let w_col  = text_width(":", sc);
    let w_mm   = text_width(&mm_s, sc);
    let w_ss   = text_width(&ss_s, sc);
    let w_ampm = text_width(&ampm_s, sc * 0.7);
    let w_dy   = text_width(&dy_s, sc);
    let w_mo   = text_width(&mo_s, sc);
    let w_yr   = text_width(&yr_s, sc);
    let w_sep  = text_width(sep, sc);
    let total  = w_hh + w_col + w_mm + w_col + w_ss + w_ampm + w_sep + w_dy + w_sep + w_mo + w_sep + w_yr;
    let mut cx = cxc - total / 2.0;

    text(pm, cx, input_y, sc, &hh_s, field_col(0)); cx += w_hh;
    text(pm, cx, input_y, sc, ":", GRAY);             cx += w_col;
    text(pm, cx, input_y, sc, &mm_s, field_col(1));  cx += w_mm;
    text(pm, cx, input_y, sc, ":", GRAY);             cx += w_col;
    let ss_col = if app.time_auto { [160, 160, 160] } else { GRAY };
    text(pm, cx, input_y + sc * PX_PER_SCALE * 0.18, sc * 0.7, &ss_s, ss_col); cx += w_ss;
    // AM/PM badge (not editable — changes automatically with the hour)
    if !ampm_s.is_empty() {
        text(pm, cx, input_y + sc * PX_PER_SCALE * 0.18, sc * 0.7,
             &ampm_s, if app.time_field == 0 { YELLOW } else { GRAY });
    }
    cx += w_ampm + w_sep;
    text(pm, cx, input_y, sc, &dy_s, field_col(2)); cx += w_dy + w_sep;
    text(pm, cx, input_y, sc, &mo_s, field_col(3)); cx += w_mo + w_sep;
    text(pm, cx, input_y, sc, &yr_s, field_col(4));

    // Field labels
    let lsc = 1.1;
    let label_y = input_y + sc * PX_PER_SCALE + 3.0;
    {
        // Position labels under each editable field
        let mut px = cxc - total / 2.0;
        let pairs: &[(&str, f32, u8)] = &[
            ("HOUR",  w_hh,                          0),
            ("MIN",   w_mm + w_col + w_ss + w_sep,   1), // skip :ss
            ("DAY",   w_dy + w_sep,                  2),
            ("MONTH", w_mo + w_sep,                  3),
            ("YEAR",  w_yr,                          4),
        ];
        for (label, fw, fi) in pairs {
            text_centered(pm, px + fw / 2.0, label_y, lsc, label,
                if is_f(*fi) { YELLOW } else { GRAY });
            px += fw + if *fi == 0 { w_col } else { 0.0 };
        }
    }
    // One-line hint: field labels already show which is which; keep it short.
    text_centered(pm, cxc, label_y + lsc * PX_PER_SCALE + 4.0, 1.0,
        "UTC reference  ·  ↑ ↓ change  ·  ← → switch field", GRAY);

    // ── Clocks (analog pair + nixie digital) ──────────────────────────────
    let clock_top      = label_y + lsc * PX_PER_SCALE + 18.0;
    let bottom_strip_h = (hf * 0.19).max(120.0).min(150.0);
    let total_clock_h  = hf - clock_top - bottom_strip_h;

    // Reserve the bottom 22 % of the clock band (min 65 px) for the nixie clock.
    let nixie_h    = (total_clock_h * 0.22).max(65.0).min(90.0);
    let nixie_gap  = 10.0_f32;
    let analog_h   = total_clock_h - nixie_h - nixie_gap;

    let r        = (analog_h / 2.1).min(wf * 0.155).max(36.0);
    let clock_cy = clock_top + analog_h * 0.50;
    // Nixie clock vertically centred in its reserved strip.
    let nixie_cy = clock_top + analog_h + nixie_gap + nixie_h * 0.50;

    // Clock positions: Roman (left) | Arabic (centre) | Word panel (right)
    let cx_roman  = wf * 0.18;
    let cx_arabic = wf * 0.50;
    let word_x    = wf * 0.68;
    let word_w    = wf * 0.30;

    // Helper closure: draw one analog clock (including second hand)
    let draw_clock = |pm: &mut Pixmap, ccx: f32, labels: &[&str], label_scale_q: f32, label_scale_n: f32| {
        circle_fill(pm, ccx, clock_cy, r, CARD_BG);
        circle_stroke(pm, ccx, clock_cy, r, WHITE, 2.0);
        // 60 minute-tick marks — must be wide enough to survive chalk_mul().
        // [80,80,96] is bright enough for chalk themes; 1.5 width at SS=3 → visible.
        for i in 0..60usize {
            if i % 5 == 0 { continue; } // hour positions handled with dedicated ticks
            let angle = (i as f32 * 6.0 - 90.0) * TAU / 360.0;
            let outer = r * 0.94;
            let inner = r * 0.90;
            line(pm, ccx + outer * angle.cos(), clock_cy + outer * angle.sin(),
                     ccx + inner * angle.cos(), clock_cy + inner * angle.sin(),
                 [80, 80, 96], 1.5);
        }
        // 12 hour tick marks and labels
        for i in 0..12usize {
            let angle = (i as f32 * 30.0 - 90.0) * TAU / 360.0;
            let is_q  = i % 3 == 0;
            let outer = r * 0.94;
            let inner = if is_q { r * 0.80 } else { r * 0.88 };
            line(pm, ccx + outer * angle.cos(), clock_cy + outer * angle.sin(),
                     ccx + inner * angle.cos(), clock_cy + inner * angle.sin(),
                 if is_q { WHITE } else { GRAY }, if is_q { 2.5 } else { 1.2 });
            let ls  = if is_q { label_scale_q } else { label_scale_n };
            // Pull labels slightly inward to keep clearance from the tick ring
            // as font size grows.
            let lr  = r * (if is_q { 0.57 } else { 0.63 });
            let lx  = ccx + lr * angle.cos();
            let ly  = clock_cy + lr * angle.sin() - ls * PX_PER_SCALE * 0.5;
            text_centered(pm, lx, ly, ls, labels[i], if is_q { YELLOW } else { GRAY });
        }
        // Hands use LOCAL time so the clock face matches the big display above.
        let ha = ((lh_loc % 12) as f32 + lm_loc as f32 / 60.0 + s as f32 / 3600.0) * 30.0 - 90.0;
        let ha = ha * TAU / 360.0;
        line(pm, ccx, clock_cy, ccx + r*0.50*ha.cos(), clock_cy + r*0.50*ha.sin(), WHITE, 4.5);
        let ma = (lm_loc as f32 + s as f32 / 60.0) * 6.0 - 90.0;
        let ma = ma * TAU / 360.0;
        line(pm, ccx, clock_cy, ccx + r*0.76*ma.cos(), clock_cy + r*0.76*ma.sin(), ACCENT, 2.8);
        // Second hand — thin red, sweeps smoothly
        let sa = (s as f32 * 6.0 - 90.0) * TAU / 360.0;
        line(pm, ccx, clock_cy, ccx + r*0.88*sa.cos(), clock_cy + r*0.88*sa.sin(), RED, 1.2);
        // Counter-weight tail
        line(pm, ccx, clock_cy, ccx - r*0.20*sa.cos(), clock_cy - r*0.20*sa.sin(), RED, 1.2);
        // Centre
        circle_fill(pm, ccx, clock_cy, 5.0, WHITE);
        circle_fill(pm, ccx, clock_cy, 2.5, RED);
    };

    // Roman numeral clock — larger quarter labels, smaller non-quarter
    draw_clock(pm, cx_roman,  td::CLOCK_LABELS_ROMAN,  2.6, 1.7);
    // Arabic numeral clock
    draw_clock(pm, cx_arabic, td::CLOCK_LABELS_ARABIC, 2.1, 1.6);

    // Clock titles float just above each face — not at the top of the screen.
    let title_y = clock_cy - r - 14.0;
    text_centered(pm, cx_roman,  title_y, 1.2, "Roman numerals", GRAY);
    text_centered(pm, cx_arabic, title_y, 1.2, "Standard clock", GRAY);

    // ── Word clock panel ───────────────────────────────────────────────────
    // Spans from just below clock_top to the bottom of the nixie strip.
    let wp_y   = clock_top + 14.0;
    let wp_h   = total_clock_h - 18.0; // reach from analog top to nixie bottom
    fill(pm, word_x, wp_y, word_w, wp_h, CARD_BG);
    stroke_rect(pm, word_x, wp_y, word_w, wp_h, 1.5, [44, 48, 64]);
    text_centered(pm, word_x + word_w / 2.0, wp_y + 12.0, 1.3, "HOW WE SAY IT", CYAN);

    let wx = word_x + 14.0;
    let mut wy = wp_y + 36.0;
    let row_h = 28.0;
    let wrap_w = word_w - 28.0;

    let entries: &[(&str, String, [u8; 3])] = &[
        ("British:",  td::fmt_british(h, m),         WHITE),
        ("American:", td::fmt_american(h, m),         WHITE),
        ("Military:", td::fmt_military_spoken(h, m),  WHITE),
        ("24-hour:",  td::fmt_24h(h, m),              CYAN),
        ("12-hour:",  td::fmt_12h(h, m),              CYAN),
        ("Roman:",    td::fmt_roman(h, m),             YELLOW),
    ];
    for (label, value, col) in entries {
        text(pm, wx, wy, 1.2, label, GRAY);
        // Wrap value if needed
        for (li, line_str) in wrap_words(value, wrap_w, 1.5).iter().enumerate() {
            text(pm, wx, wy + (li as f32 + 1.0) * 18.0, 1.5, line_str, *col);
        }
        wy += row_h + 6.0;
    }

    // ── Nixie tube digital clock ───────────────────────────────────────────
    // Spans the area beneath the two analog clocks (left 2/3 of screen).
    let nixie_x  = (cx_roman  - r * 0.90).max(4.0);
    let nixie_x2 = (cx_arabic + r * 0.90).min(word_x - 8.0);
    draw_nixie_clock(pm, (nixie_x + nixie_x2) / 2.0, nixie_cy,
                     nixie_x2 - nixie_x, nixie_h,
                     lh_loc, lm_loc, s, use_24h);

    // ── Bottom strip: time zones (left) + calendar (right) ─────────────────
    let strip_y = hf - bottom_strip_h + 4.0;
    let cal_w   = 180.0;
    let tz_area_w = wf - cal_w - 16.0;

    // The stored h/m/d is UTC.  Each zone's time = UTC + zone.offset_mins.
    // "My Time" uses time_local_offset (detected from OS/browser, DST-aware).
    let zones   = td::ZONES;
    let loc_off = app.time_local_offset;
    let tz_sc   = 1.1;

    struct ZoneEntry<'a> { city: &'a str, abbr: &'a str, eff: i32 }
    let mut entries: Vec<ZoneEntry> = vec![
        ZoneEntry { city: "My Time", abbr: "local", eff: loc_off },
    ];
    for z in zones {
        entries.push(ZoneEntry { city: z.city, abbr: z.abbr, eff: z.offset_on(yr, mo, dy) });
    }

    // Ceiling division → always exactly 2 rows regardless of entry count.
    let n_entries = entries.len();
    let half      = (n_entries + 1) / 2;
    let tz_col_w  = tz_area_w / half as f32;
    // Zone row height: time (13px) + optional date (11px) + city (10px) = max ~37px.
    let tz_row_h  = (bottom_strip_h * 0.45).max(36.0).min(48.0);

    for (i, z) in entries.iter().enumerate() {
        let (zy2, zmo2, zd2, zh2, zm2, delta) =
            td::apply_offset_dated(yr, mo, dy, h, m, z.eff);
        let row = (i / half) as f32;
        let col = (i % half) as f32;
        let zx  = col * tz_col_w + tz_col_w / 2.0;
        let zy  = strip_y + row * tz_row_h;

        let day_tag = if delta == 0 { String::new() }
                      else { format!("{:+}", delta) };
        let time_str = td::fmt_time(zh2, zm2, use_24h);
        let date_str = if delta != 0 {
            format!("{} {} {} {}", zd2, td::month_abbr(zmo2), zy2, day_tag)
        } else { String::new() };

        let is_local = i == 0;
        text_centered(pm, zx, zy, tz_sc, &time_str,
            if is_local { YELLOW } else if delta == 0 { WHITE } else { ACCENT });
        if !date_str.is_empty() {
            text_centered(pm, zx, zy + 14.0, 1.0, &date_str, ACCENT);
        }
        let city_y = zy + if delta != 0 { 26.0 } else { 16.0 };
        let city_label = if is_local {
            format!("{} ({})", z.city, z.abbr)
        } else {
            format!("{}", z.city)
        };
        text_centered(pm, zx, city_y, 1.0, &city_label,
            if is_local { YELLOW } else { GRAY });
    }

    // Calendar
    let cal_x  = wf - cal_w - 8.0;
    let cal_y  = strip_y;
    fill(pm, cal_x, cal_y, cal_w, bottom_strip_h - 8.0, CARD_BG);
    stroke_rect(pm, cal_x, cal_y, cal_w, bottom_strip_h - 8.0, 1.2, [44, 48, 64]);

    let cal_header = format!("{} {}", td::month_name(mo), yr);
    text_centered(pm, cal_x + cal_w / 2.0, cal_y + 5.0, 1.2, &cal_header, CYAN);

    // Weekday row
    let cell_w = cal_w / 7.0;
    let dow_abbrs = ["Su","Mo","Tu","We","Th","Fr","Sa"];
    for (i, d_abbr) in dow_abbrs.iter().enumerate() {
        text_centered(pm, cal_x + (i as f32 + 0.5) * cell_w, cal_y + 20.0, 1.0, d_abbr, GRAY);
    }

    // Day cells
    let first_col = td::first_dow(yr, mo) as f32;
    let days_in   = td::days_in_month(yr, mo) as f32;
    // cell_h scales to fit 6 rows + header + weekday row + hint inside the strip.
    let grid_rows = ((first_col as usize + days_in as usize + 6) / 7 + 1).max(5);
    let grid_avail = bottom_strip_h - 42.0; // header(15) + weekday(13) + hint(14)
    let cell_h = (grid_avail / grid_rows as f32).max(10.0).min(15.0);
    let grid_top  = cal_y + 32.0;

    // Collect dates that appear shifted in any zone.
    let mut shifted_days: Vec<u8> = Vec::new();
    for z in zones {
        let (_, zmo2, zd2, _, _, delta) =
            td::apply_offset_dated(yr, mo, dy, h, m, z.offset_on(yr, mo, dy));
        if delta != 0 && zmo2 == mo { shifted_days.push(zd2); }
    }

    for d_num in 1u8..=(days_in as u8) {
        let idx  = first_col + (d_num - 1) as f32;
        let col  = (idx as usize) % 7;
        let row  = (idx as usize) / 7;
        let dcx  = cal_x + (col as f32 + 0.5) * cell_w;
        let dcy  = grid_top + row as f32 * cell_h + cell_h * 0.4;
        let is_today = d_num == dy;
        let is_shift = shifted_days.contains(&d_num);
        let color = if is_today { YELLOW }
                    else if is_shift { ACCENT }
                    else { GRAY };
        // Circle is drawn with stroke_path so it stays visible in chalk themes.
        if is_today {
            circle_stroke(pm, dcx, dcy, cell_h * 0.52, YELLOW, 1.5);
        } else if is_shift {
            circle_stroke(pm, dcx, dcy, cell_h * 0.48, ACCENT, 1.0);
        }
        text_centered(pm, dcx, dcy - PX_PER_SCALE * 0.5, 1.0, &d_num.to_string(), color);
    }

    // Format hint sits at the very bottom of the calendar box.
    text_centered(pm, cal_x + cal_w / 2.0, cal_y + bottom_strip_h - 12.0, 1.0,
        "T: 12 ↔ 24 h", GRAY);

    // ── Time reference overlay (H key) ─────────────────────────────────────
    if app.time_help_active {
        draw_time_help(pm, wf, hf);
    }
}

// ---------------------------------------------------------------------------
// Keyboard-shortcut help screen (accessible from the main menu)
// ---------------------------------------------------------------------------

pub fn draw_key_guide(pm: &mut Pixmap, wf: f32, hf: f32) {
    use crate::time_display::HOTKEYS;

    let cx = wf / 2.0;
    fill(pm, 0.0, 0.0, wf, hf, [6, 6, 12]);

    // Card
    let pad   = wf * 0.04;
    let crd_x = pad;
    let crd_y = hf * 0.04;
    let crd_w = wf - pad * 2.0;
    let crd_h = hf * 0.92;
    fill(pm, crd_x, crd_y, crd_w, crd_h, [12, 14, 24]);
    stroke_rect(pm, crd_x, crd_y, crd_w, crd_h, 2.0, CYAN);

    text_centered(pm, cx, crd_y + 10.0, 2.2, "KEYBOARD  SHORTCUTS", CYAN);
    text_centered(pm, cx, crd_y + 30.0, 1.1, "Esc, Enter, or H to close", GRAY);

    // Two-column layout
    let n_sections = HOTKEYS.len();
    let left_n  = (n_sections + 1) / 2;           // ceiling half
    let col_w   = crd_w / 2.0 - 16.0;
    let col1_x  = crd_x + 12.0;
    let col2_x  = crd_x + crd_w / 2.0 + 4.0;
    let start_y = crd_y + 50.0;

    for (col_idx, range) in [(0, 0..left_n), (1, left_n..n_sections)] {
        let col_x = if col_idx == 0 { col1_x } else { col2_x };
        let mut y = start_y;

        for sec_idx in range {
            let (title, keys) = &HOTKEYS[sec_idx];

            // Section heading
            text(pm, col_x, y, 1.5, title, YELLOW);
            y += 1.5 * PX_PER_SCALE + 3.0;
            line(pm, col_x, y, col_x + col_w, y, [40, 44, 60], 1.0);
            y += 5.0;

            // Key rows
            for (key, desc) in keys.iter() {
                let kw = text_width(key, 1.3);
                text(pm, col_x + 4.0,        y, 1.3, key,  ACCENT);
                text(pm, col_x + kw + 14.0,  y, 1.3, desc, WHITE);
                y += 1.3 * PX_PER_SCALE + 5.0;
            }
            y += 12.0; // gap between sections
        }
    }

    text_centered(pm, cx, crd_y + crd_h - 14.0, 1.1, "Esc · Enter · H  to close", GRAY);
}

// ---------------------------------------------------------------------------
// Time reference facts overlay (H key on the Time Explorer screen)
// ---------------------------------------------------------------------------

fn draw_time_help(pm: &mut Pixmap, wf: f32, hf: f32) {
    use crate::time_display::TIME_FACTS;

    // Dark backdrop
    fill(pm, 0.0, 0.0, wf, hf, [4, 4, 8]);

    // Card
    let cx = wf / 2.0;
    let pad = wf * 0.04;
    let card_x = pad;
    let card_y = hf * 0.04;
    let card_w = wf - pad * 2.0;
    let card_h = hf * 0.92;
    fill(pm, card_x, card_y, card_w, card_h, [12, 14, 24]);
    stroke_rect(pm, card_x, card_y, card_w, card_h, 2.0, CYAN);

    // Header
    text_centered(pm, cx, card_y + 10.0, 2.0, "TIME  REFERENCE", CYAN);
    text_centered(pm, cx, card_y + 30.0, 1.1, "H or Esc to close", GRAY);

    // Two-column layout for the five categories
    let col_w   = card_w / 2.0 - 12.0;
    let col1_x  = card_x + 12.0;
    let col2_x  = card_x + card_w / 2.0 + 4.0;
    let start_y = card_y + 48.0;

    // Left column: categories 0–2,  right column: 3–4
    let left_cats  = &TIME_FACTS[..3];
    let right_cats = &TIME_FACTS[3..];

    for (col_x, cats) in [(col1_x, left_cats), (col2_x, right_cats)] {
        let mut y = start_y;
        for (cat_title, facts) in cats.iter() {
            // Category heading
            text(pm, col_x, y, 1.5, cat_title, YELLOW);
            y += 1.5 * PX_PER_SCALE + 4.0;

            // Rule line under heading
            line(pm, col_x, y, col_x + col_w, y, [40, 44, 60], 1.0);
            y += 5.0;

            // Facts
            for fact in facts.iter() {
                text(pm, col_x + 8.0, y, 1.3, fact, WHITE);
                y += 1.3 * PX_PER_SCALE + 6.0;
            }
            y += 14.0; // gap between categories
        }
    }

    // Footer
    text_centered(pm, cx, card_y + card_h - 14.0, 1.1,
        "H or Esc to close", GRAY);
}

// ---------------------------------------------------------------------------
// Nixie tube digital clock
// ---------------------------------------------------------------------------

fn draw_nixie_clock(pm: &mut Pixmap, cx: f32, cy: f32, _avail_w: f32, avail_h: f32,
                    hour: u8, min: u8, sec: u8, use_24h: bool) {
    use crate::time_display::to_12h;

    let (h12, am) = to_12h(hour);

    // Build digit strings
    let h_str  = if use_24h { format!("{:02}", hour) } else { h12.to_string() };
    let m_str  = format!("{:02}", min);
    let s_str  = format!("{:02}", sec);
    let ampm   = if use_24h { "" } else { if am { "AM" } else { "PM" } };

    // Nixie tube colour palette
    const OUTER_BG:  Rgb = [9, 4, 0];
    const TUBE_BG:   Rgb = [15, 6, 0];
    const TUBE_EDGE: Rgb = [55, 24, 3];
    const GLOW_DIM:  Rgb = [80, 28, 2];
    const AMBER:     Rgb = [238, 98, 12];
    const AMBER_DIM: Rgb = [160, 60, 7];

    // Scale the digit to fill ~70 % of the available height
    let digit_sc  = (avail_h / PX_PER_SCALE * 0.68).max(2.5).min(6.0);
    let tube_h    = avail_h * 0.88;
    let tube_top  = cy - tube_h / 2.0;

    // Measure a representative digit to size the tubes
    let char_w    = text_width("0", digit_sc);
    let tube_w    = char_w * 1.9;   // tubes are wider than their digit
    let colon_w   = tube_w * 0.45;  // colons get a narrower tube
    let gap       = tube_w * 0.12;  // inter-tube gap
    let ampm_sc   = digit_sc * 0.42;

    // Build the ordered segment list: (label_string, tube_width)
    let mut segs: Vec<(String, f32)> = Vec::new();
    for c in h_str.chars()  { segs.push((c.to_string(), tube_w)); }
    segs.push((":".to_string(), colon_w));
    for c in m_str.chars()  { segs.push((c.to_string(), tube_w)); }
    segs.push((":".to_string(), colon_w));
    for c in s_str.chars()  { segs.push((c.to_string(), tube_w)); }
    if !ampm.is_empty() {
        segs.push((" ".to_string(), gap));
        segs.push((ampm.to_string(), text_width(ampm, ampm_sc) * 1.4));
    }

    // Total width of all tubes + gaps
    let total_w = segs.iter().map(|(_, w)| w).sum::<f32>()
                + gap * (segs.len().saturating_sub(1)) as f32;

    // Outer background panel — slightly wider than the tubes
    let pad = 10.0_f32;
    fill(pm, cx - total_w / 2.0 - pad, cy - avail_h / 2.0,
         total_w + pad * 2.0, avail_h, OUTER_BG);
    stroke_rect(pm, cx - total_w / 2.0 - pad, cy - avail_h / 2.0,
                total_w + pad * 2.0, avail_h, 1.5, TUBE_EDGE);

    // Draw each tube
    let text_y    = cy - digit_sc * PX_PER_SCALE / 2.0;
    let ampm_y    = cy - ampm_sc  * PX_PER_SCALE / 2.0;
    let mut x     = cx - total_w / 2.0;

    for (i, (label, tw)) in segs.iter().enumerate() {
        let is_colon = label == ":";
        let is_ampm  = label == "AM" || label == "PM";
        let is_space = label == " ";

        if !is_space {
            // Tube housing
            fill(pm, x, tube_top, *tw, tube_h, TUBE_BG);
            stroke_rect(pm, x, tube_top, *tw, tube_h, 0.8, TUBE_EDGE);
        }

        if is_colon {
            // Two glowing dots instead of the text colon
            let ccx = x + tw / 2.0;
            let dot_r = digit_sc * PX_PER_SCALE * 0.09;
            let dot_off = digit_sc * PX_PER_SCALE * 0.22;
            circle_fill(pm, ccx, cy - dot_off, dot_r * 1.8, GLOW_DIM);
            circle_fill(pm, ccx, cy + dot_off, dot_r * 1.8, GLOW_DIM);
            circle_fill(pm, ccx, cy - dot_off, dot_r, AMBER);
            circle_fill(pm, ccx, cy + dot_off, dot_r, AMBER);
        } else if is_ampm {
            let tw2 = text_width(label, ampm_sc);
            let tx = x + (tw - tw2) / 2.0;
            text(pm, tx, ampm_y, ampm_sc, label, AMBER_DIM);
        } else if !is_space {
            // Digit: glow first, then bright core
            let tw2 = text_width(label, digit_sc);
            let tx = x + (tw - tw2) / 2.0;
            // Soft glow halo
            for off in &[(-1.0_f32, 0.0_f32), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
                text(pm, tx + off.0, text_y + off.1, digit_sc, label, GLOW_DIM);
            }
            // Bright amber core
            text(pm, tx, text_y, digit_sc, label, AMBER);
        }

        x += tw + if i + 1 < segs.len() { gap } else { 0.0 };
    }
}
