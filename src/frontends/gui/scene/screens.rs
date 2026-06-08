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
    v.push(MenuRow::Text("Settings...".to_string()));
    v.push(MenuRow::Text("My Progress...".to_string()));
    v.push(MenuRow::Text("Teacher Area...".to_string()));
    v.push(MenuRow::Text(">  Start Practice".to_string()));
    v.push(MenuRow::Text(">  Start Challenge".to_string()));
    v.push(MenuRow::Text(">  Experimentation".to_string()));
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
