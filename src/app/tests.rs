use super::*;
use crate::{motivation, strategy};
use crate::transition::{Effect, Transition};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

/// Render every screen at several sizes (including cramped ones) to prove
/// the direct buffer writes in font/duck/transition stay in bounds.
#[test]
fn rendering_never_panics() {
    for (w, h) in [(80, 24), (120, 40), (30, 12), (40, 10)] {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        let mut app = App::new(Config::default());
        app.set_area(Rect::new(0, 0, w, h));

        // Startup, menu (with name prompt), settings, stats.
        term.draw(|f| ui::draw(f, &app)).unwrap();
        app.screen = Screen::Menu;
        term.draw(|f| ui::draw(f, &app)).unwrap();
        app.naming = true;
        app.name_input = "Ada".to_string();
        term.draw(|f| ui::draw(f, &app)).unwrap();
        app.naming = false;
        app.open_settings();
        term.draw(|f| ui::draw(f, &app)).unwrap();
        app.screen = Screen::Stats;
        app.roster.current_mut().solved = [12, 7, 4, 3];
        app.roster.current_mut().best_streak = 9;
        term.draw(|f| ui::draw(f, &app)).unwrap();

        // Teacher area: login, then both manage views (adding + records).
        app.open_teacher();
        app.teacher_pw = "pw".to_string();
        term.draw(|f| ui::draw(f, &app)).unwrap();
        app.teacher_authed = true;
        app.teacher_view = TeacherView::Anecdotes;
        app.teacher_adding = true;
        // A long anecdote exercises the wrapping input box and overflow.
        app.teacher_text = "x".repeat(ANECDOTE_MAX);
        term.draw(|f| ui::draw(f, &app)).unwrap();
        app.teacher_text = "When I tiled my kitchen floor".to_string();
        term.draw(|f| ui::draw(f, &app)).unwrap();
        app.teacher_adding = false;
        app.teacher_view = TeacherView::Records;
        app.teacher_msg = Some("Reset all records for Ada.".to_string());
        term.draw(|f| ui::draw(f, &app)).unwrap();
        app.screen = Screen::Menu;

        // Practice in both layouts: cycle every strategy (each Viz), with
        // hint then revealed, plus the Why overlay, across all operations.
        for layout in [crate::config::Layout::Horizontal, crate::config::Layout::Vertical] {
            app.config.layout = layout;
            for &op in &Op::ALL {
                app.menu_ops = [op == Op::Add, op == Op::Sub, op == Op::Mul, op == Op::Div];
                app.start_session(false);
                app.set_area(Rect::new(0, 0, w, h));
                app.help_active = true;
                app.encourage = Some("Don't worry — mistakes help you grow!".to_string());
                for _ in 0..10 {
                    app.on_tick();
                }
                for idx in 0..6 {
                    app.strategy_index = idx;
                    for &rev in &[false, true] {
                        app.revealed = rev;
                        for frame in 0..4 {
                            app.anim_frame = frame * 5;
                            term.draw(|f| ui::draw(f, &app)).unwrap();
                        }
                    }
                }
                // Why overlay.
                app.help_active = false;
                app.why_active = true;
                app.refresh_why_items();
                term.draw(|f| ui::draw(f, &app)).unwrap();
                app.why_active = false;
            }
            app.feedback = Feedback::Wrong;
            term.draw(|f| ui::draw(f, &app)).unwrap();
        }

        // Challenge HUD + summary.
        app.start_session(true);
        app.set_area(Rect::new(0, 0, w, h));
        term.draw(|f| ui::draw(f, &app)).unwrap();
        if let Some(c) = &mut app.challenge {
            c.finished = true;
        }
        term.draw(|f| ui::draw(f, &app)).unwrap();

        // Units-only session (the Units checkbox on, no ops), sweeping the
        // duck-gag animation phases.
        app.menu_ops = [false, false, false, false];
        app.menu_units = true;
        app.start_session(false);
        app.set_area(Rect::new(0, 0, w, h));
        assert!(app.current_topic() == crate::topic::Topic::Units, "units-only session should show a unit problem");
        app.input = "8".to_string();
        for frame in [5u64, 45, 60, 90] {
            app.anim_frame = frame;
            term.draw(|f| ui::draw(f, &app)).unwrap();
        }
        app.menu_units = false;

        // Experimentation explorer: calm, scratch, and boom reactions.
        app.screen = Screen::Experiment;
        for amount in ["3", "300", "8000"] {
            app.exp_amount = amount.to_string();
            app.anim_frame = app.anim_frame.wrapping_add(7);
            term.draw(|f| ui::draw(f, &app)).unwrap();
        }

        // Fractions-only session: sweep the materialise animation + help duck.
        app.menu_ops = [false, false, false, false];
        app.menu_units = false;
        app.menu_fractions = true;
        app.start_session(false);
        app.set_area(Rect::new(0, 0, w, h));
        app.input = "1/2".to_string();
        app.help_active = true;
        for frame in [0u32, 6, 20, 80] {
            app.frac_anim = frame;
            app.help_in = 1.0;
            term.draw(|f| ui::draw(f, &app)).unwrap();
        }
        app.menu_fractions = false;

        // Teacher records (with the reveal-lock column).
        app.open_teacher();
        app.teacher_authed = true;
        app.teacher_view = TeacherView::Records;
        term.draw(|f| ui::draw(f, &app)).unwrap();
    }
}

/// Every transition effect must play start-to-finish without panicking.
#[test]
fn all_transitions_play_safely() {
    let mut rng = rand::thread_rng();
    for (w, h) in [(80, 24), (32, 12)] {
        let area = Rect::new(0, 0, w, h);
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        for effect in Effect::all() {
            let mut from = Buffer::empty(area);
            let mut to = Buffer::empty(area);
            let p1 = problem::generate(Config::default().range(4), &Op::ALL, &mut rng);
            let p2 = problem::generate(Config::default().range(4), &Op::ALL, &mut rng);
            ui::render_card(area, &mut from, &p1, "12", crate::config::Layout::Horizontal, None);
            ui::render_card(area, &mut to, &p2, "", crate::config::Layout::Horizontal, None);
            let mut t = Transition::with_effect(effect, from, to, &mut rng);
            loop {
                term.draw(|f| t.render(area, f.buffer_mut())).unwrap();
                if t.advance() {
                    break;
                }
            }
        }
    }
}

#[test]
fn division_is_always_exact_and_answers_check_out() {
    let mut rng = rand::thread_rng();
    let cfg = Config::default();
    for grade in 0..=8u8 {
        for _ in 0..500 {
            let p = problem::generate(cfg.range(grade), &Op::ALL, &mut rng);
            let computed = match p.op {
                Op::Add => p.a + p.b,
                Op::Sub => p.a - p.b,
                Op::Mul => p.a * p.b,
                Op::Div => {
                    assert_eq!(p.a % p.b, 0, "division must be exact");
                    p.a / p.b
                }
            };
            assert_eq!(computed, p.answer);
        }
    }
}

/// The Why panel must show all content when there's room (no dropped
/// continuation line), and report a scroll range when there isn't.
#[test]
fn why_panel_fits_when_tall_and_scrolls_when_short() {
    let make = || {
        let mut app = App::new(Config::default());
        app.menu_ops = [true, false, false, false];
        app.start_session(false);
        app.why_active = true;
        app.why_items = vec![
            "Totalling up the cost of everything in your shopping cart".to_string(),
            "Adding up calories so astronauts pack enough food for space".to_string(),
            "Adding the fuel in each rocket stage to reach orbit".to_string(),
            "Summing the weights of cargo so a plane stays balanced".to_string(),
        ];
        app.why_code = Some((
            "self.progress += 1.0 / duration;".to_string(),
            "Adding a little each frame is how one problem smoothly melts into the next.".to_string(),
        ));
        app
    };

    // Plenty of height: everything fits, nothing scrolls.
    let app = make();
    let mut term = Terminal::new(TestBackend::new(80, 30)).unwrap();
    term.draw(|f| ui::draw(f, &app)).unwrap();
    assert_eq!(app.why_max_scroll.get(), 0, "tall panel should not need scrolling");

    // Cramped height: the overflow becomes scrollable.
    let app = make();
    let mut term = Terminal::new(TestBackend::new(80, 13)).unwrap();
    term.draw(|f| ui::draw(f, &app)).unwrap();
    assert!(app.why_max_scroll.get() > 0, "short panel should scroll");
}

#[test]
fn menu_duck_renders_across_cycle_without_panic() {
    // Sweep the whole appearance cycle (walk + both peeks) at a few sizes.
    for (w, h) in [(64u16, 40u16), (64, 24), (40, 12)] {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        let mut app = App::new(Config::default());
        app.screen = Screen::Menu;
        for frame in (0u64..2600).step_by(13) {
            app.anim_frame = frame;
            term.draw(|f| ui::draw(f, &app)).unwrap();
        }
    }
}

#[test]
fn every_unit_problem_has_a_help_hint() {
    use crate::units::Locality;
    let mut rng = rand::thread_rng();
    for loc in Locality::ALL {
        for _ in 0..400 {
            let p = crate::units::generate(loc, &mut rng);
            assert!(!p.hint.is_empty(), "no hint for: {}", p.question);
            assert!(p.hint.iter().all(|h| !h.trim().is_empty()));
        }
    }
}

#[test]
fn unit_problem_help_renders() {
    let mut app = App::new(Config::default());
    app.menu_ops = [false, false, false, false];
    app.menu_units = true;
    app.start_session(false);
    app.set_area(Rect::new(0, 0, 80, 24));
    assert!(app.current_topic() == crate::topic::Topic::Units);
    // Pressing H opens the duck; R reveals the answer.
    use crossterm::event::{KeyCode, KeyEvent};
    app.on_key(KeyEvent::from(KeyCode::Char('h')));
    assert!(app.help_active, "H should summon the duck on a unit problem");
    app.on_key(KeyEvent::from(KeyCode::Char('r')));
    assert!(app.revealed);
    let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
    for _ in 0..6 {
        app.on_tick();
        term.draw(|f| ui::draw(f, &app)).unwrap();
    }
}

#[test]
fn peeking_too_much_locks_the_answer_with_reprimands() {
    let mut app = App::new(Config::default());
    app.roster = crate::student::Roster::default();
    app.menu_ops = [true, false, false, false];
    app.start_session(false);
    app.set_area(Rect::new(0, 0, 80, 24));
    app.help_active = true;

    // Reveal then hide, up to the student's limit — each reveal spends a peek.
    let lock = app.roster.current().reveal_lock;
    assert!(lock > 0);
    for _ in 0..lock {
        app.reveal_answer(); // reveal
        assert!(app.revealed);
        app.reveal_answer(); // hide
        assert!(!app.revealed);
    }
    // Now the answer is on cooldown.
    assert!(app.reveal_locked(), "should be locked after too many peeks");
    app.reveal_answer();
    assert!(!app.revealed, "locked: must not reveal");
    assert!(app.reprimand.is_some(), "insisting earns a reprimand");
    let first = app.reprimand.clone();
    app.reveal_answer();
    assert_ne!(app.reprimand, first, "reprimands escalate");

    // Solving a problem on your own earns a peek back and unlocks.
    app.help_active = false;
    app.input = app.current.correct_answer_string();
    app.check_answer();
    assert!(!app.reveal_locked(), "solving it yourself earns trust back");
}

#[test]
fn reveal_lock_is_per_student_and_can_be_disabled() {
    use crossterm::event::{KeyCode, KeyEvent};
    let mut app = App::new(Config::default());
    // Use a clean single-student roster (tests share the on-disk one).
    app.roster = crate::student::Roster::default();

    // Teacher can adjust the selected student's lock in the Records view.
    app.open_teacher();
    app.teacher_authed = true;
    app.teacher_view = TeacherView::Records;
    app.teacher_rec_index = 0;
    let start = app.roster.students[0].reveal_lock;
    app.on_key(KeyEvent::from(KeyCode::Char('+')));
    assert_eq!(app.roster.students[0].reveal_lock, start + 1);
    for _ in 0..9 {
        app.on_key(KeyEvent::from(KeyCode::Char('-')));
    }
    assert_eq!(app.roster.students[0].reveal_lock, 0, "can drop to 0 (never lock)");

    // Lock of 0 means reveals never go on cooldown.
    app.start_session(false);
    app.help_active = true;
    for _ in 0..20 {
        app.reveal_answer();
    }
    assert!(!app.reveal_locked(), "lock 0 never locks");
}

#[test]
fn fractions_check_equivalent_answers() {
    use crate::fraction::{self, FractionProblem};
    use crate::shapes::Shape;
    let p = FractionProblem {
        mode: crate::fraction::Mode::Fraction,
        total: 4,
        shaded: 2,
        shape: Shape::Grid { rows: 2, cols: 2 },
        shaded_color: ratatui::style::Color::LightGreen,
        other_color: ratatui::style::Color::LightBlue,
        color_name: "green",
        hint: vec![],
    };
    assert!(p.is_correct(2, 4));
    assert!(p.is_correct(1, 2)); // equivalent form
    assert!(!p.is_correct(1, 4));
    assert!(!p.is_correct(3, 4));
    assert!(!p.is_correct(1, 0)); // no divide-by-zero
    assert_eq!(fraction::parse("2/4"), Some((2, 4)));
    assert_eq!(fraction::parse(" 1 / 2 "), Some((1, 2)));
    assert_eq!(fraction::parse("3"), None);
    assert_eq!(fraction::parse("a/b"), None);
}

#[test]
fn fraction_asked_colour_pieces_equal_the_numerator() {
    // The question asks about the shaded colour; the number of pieces painted
    // that colour must equal `shaded` (the answer's numerator) — guards against
    // the inverted-answer bug.
    let mut rng = rand::thread_rng();
    for _ in 0..500 {
        let p = crate::fraction::generate(&mut rng);
        assert!(p.shaded >= 1 && p.shaded < p.total);
        assert_ne!(p.shaded_color, p.other_color, "two pieces must be tellable apart");
        let shaded_pieces = (0..p.shape.regions()).filter(|&i| p.color_of(i) == p.shaded_color).count() as i64;
        assert_eq!(shaded_pieces, p.shaded, "pieces in the asked colour must equal the numerator");
    }
}

#[test]
fn fractions_mix_into_a_session_and_advance() {
    // A fractions-only session: every problem is a fraction, and solving one
    // (in any equivalent form) advances to the next via the transition.
    let mut app = App::new(Config::default());
    app.menu_ops = [false, false, false, false];
    app.menu_fractions = true;
    app.start_session(false);
    app.set_area(Rect::new(0, 0, 80, 24));
    assert!(app.current_topic() == crate::topic::Topic::Fractions, "fractions-only session shows a fraction");

    app.input = app.current.correct_answer_string();
    app.check_answer();
    assert_eq!(app.feedback, Feedback::Correct);
    assert!(app.transition.is_some(), "correct answer kicks off a transition");

    let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
    for _ in 0..60 {
        term.draw(|f| ui::draw(f, &app)).unwrap();
        app.on_tick();
        if app.transition.is_none() {
            break;
        }
    }
    assert!(app.transition.is_none());
    assert!(app.current_topic() == crate::topic::Topic::Fractions, "still fractions after advancing");
    assert!(app.input.is_empty());
}

#[test]
fn slash_key_types_a_fraction_answer() {
    // Regression: pressing digits and '/' must build an "a/b" answer.
    let mut app = App::new(Config::default());
    app.roster = crate::student::Roster::default();
    app.menu_ops = [false, false, false, false];
    app.menu_fractions = true;
    app.start_session(false);
    app.set_area(Rect::new(0, 0, 80, 24));
    assert!(app.current_topic() == crate::topic::Topic::Fractions);
    for c in ['2', '/', '4'] {
        app.on_key(KeyEvent::from(KeyCode::Char(c)));
    }
    assert_eq!(app.input, "2/4", "the '/' key should be accepted for fractions");
}

/// Flatten a rendered buffer into one string (rows newline-separated).
fn buffer_to_string(buf: &ratatui::buffer::Buffer) -> String {
    let area = buf.area;
    let mut out = String::new();
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

#[test]
fn title_screens_feature_deduction_duck() {
    // The "Featuring: Deduction Duck" tagline appears under the title on both
    // the startup picker and the main menu.
    for screen in [Screen::Startup, Screen::Menu] {
        let mut app = App::new(Config::default());
        app.roster = crate::student::Roster::default();
        app.screen = screen;
        app.set_area(Rect::new(0, 0, 70, 22));
        let mut term = Terminal::new(TestBackend::new(70, 22)).unwrap();
        term.draw(|f| ui::draw(f, &app)).unwrap();
        let text = buffer_to_string(term.backend().buffer());
        assert!(text.contains("Featuring:  Deduction Duck"), "{screen:?} should feature Deduction Duck");
    }
}

fn sky_scene(name: &str) -> crate::cinematic::Scene {
    use crate::cinematic::{Scene, SceneKind, SKY_DWELL};
    Scene {
        kind: SceneKind::NameSky,
        accent: ratatui::style::Color::LightCyan,
        big: None,
        heading: name.to_string(),
        sub: "written in the stars!".to_string(),
        dwell: SKY_DWELL,
    }
}

#[test]
fn deduction_duck_gazes_up_in_awe() {
    let area = Rect::new(0, 0, 70, 22);

    // The awe duck appears in both showpiece scenes, wearing its graduate cap.
    for scene in [rocket_scene("Ada"), sky_scene("Ada")] {
        let text = render_at(&scene, area, 70);
        assert!(text.contains("[___]"), "Deduction Duck (graduate cap) should be present");
        assert!(text.contains("(**)") || text.contains("(°°)"), "the duck gazes up, starry-eyed");
    }

    // Its eyes twinkle across the two-frame loop (** on even, °° on odd).
    let scene = sky_scene("Ada");
    let even = render_at(&scene, area, 64); // 64/8 = 8, even
    let odd = render_at(&scene, area, 72); //  72/8 = 9, odd
    assert!(even.contains("(**)"), "even frame: starry eyes");
    assert!(odd.contains("(°°)"), "odd frame: wide-eyed wonder");

    // No room on a tiny terminal — the duck bows out rather than overflow.
    let tiny = Rect::new(0, 0, 22, 11);
    let text = render_at(&rocket_scene("Ada"), tiny, 70);
    assert!(!text.contains("[___]"), "no duck when the sky is too small");
}

fn rocket_scene(name: &str) -> crate::cinematic::Scene {
    use crate::cinematic::{Scene, SceneKind, ROCKET_DWELL};
    Scene {
        kind: SceneKind::RocketName,
        accent: ratatui::style::Color::Rgb(255, 232, 150),
        big: None,
        heading: name.to_string(),
        sub: "written in the stars!".to_string(),
        dwell: ROCKET_DWELL,
    }
}

fn render_at(scene: &crate::cinematic::Scene, area: Rect, frame: u64) -> String {
    let mut buf = ratatui::buffer::Buffer::empty(area);
    crate::ui::render_scene(area, &mut buf, scene, frame);
    buffer_to_string(&buf)
}

#[test]
fn rockets_launch_then_form_the_name() {
    let scene = rocket_scene("Ada");
    let area = Rect::new(0, 0, 70, 22);

    // Early on, rockets are climbing and the name has not formed yet.
    let early = render_at(&scene, area, 4);
    assert!(early.contains('▲'), "rockets should be launching early");
    assert!(!early.contains("Ada"), "the name should not be spelled yet");

    // After the launch, the name is written in stars (no rockets left).
    let formed = render_at(&scene, area, 40);
    assert!(formed.contains("Ada"), "rockets should have formed the name");
    assert!(!formed.contains('▲'), "no rockets remain once the name is formed");
}

#[test]
fn blue_comet_passes_behind_the_formed_name() {
    let scene = rocket_scene("Ada");
    let area = Rect::new(0, 0, 70, 22);
    // During the comet sweep the name stays fully legible (it's drawn on top)
    // and the comet's head star is present somewhere in the sky.
    let mut saw_comet = false;
    for frame in 55..95 {
        let text = render_at(&scene, area, frame);
        assert!(text.contains("Ada"), "the name must remain on top of the comet");
        if text.contains('★') {
            saw_comet = true;
        }
    }
    assert!(saw_comet, "a comet should sweep through during the scene");
}

#[test]
fn rocket_name_survives_a_cramped_sky() {
    let scene = rocket_scene("Zo");
    for (w, h) in [(10u16, 6u16), (16, 9), (40, 10), (120, 40)] {
        let area = Rect::new(0, 0, w, h);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        for frame in [0u64, 30, 80, 150] {
            crate::ui::render_scene(area, &mut buf, &scene, frame);
        }
    }
}

#[test]
fn name_sky_cinematic_writes_the_name_and_raises_the_moon() {
    use crate::cinematic::{Scene, SceneKind, SKY_DWELL};
    let scene = Scene {
        kind: SceneKind::NameSky,
        accent: ratatui::style::Color::LightCyan,
        big: None,
        heading: "Ada".to_string(),
        sub: "written in the stars!".to_string(),
        dwell: SKY_DWELL,
    };

    // At the start the moon is still below; well into the scene it has risen.
    let area = Rect::new(0, 0, 70, 22);
    let moon_top_row = |frame: u64| -> Option<u16> {
        let mut buf = ratatui::buffer::Buffer::empty(area);
        crate::ui::render_scene(area, &mut buf, &scene, frame);
        let text = buffer_to_string(&buf);
        // The name is always written in the sky.
        assert!(text.contains("Ada"), "the learner's name should be in the sky");
        // Find the row holding the moon's top arc.
        text.lines().position(|l| l.contains(".-\"\"\"-.")).map(|r| r as u16)
    };
    let early = moon_top_row(2).unwrap();
    let late = moon_top_row(60).unwrap();
    assert!(late < early, "the moon should rise (move up) as the scene plays");
}

#[test]
fn name_sky_survives_a_cramped_sky() {
    // The animation must never write out of bounds, even on a tiny terminal.
    use crate::cinematic::{Scene, SceneKind, SKY_DWELL};
    let scene = Scene {
        kind: SceneKind::NameSky,
        accent: ratatui::style::Color::LightCyan,
        big: None,
        heading: "Zo".to_string(),
        sub: "written in the stars!".to_string(),
        dwell: SKY_DWELL,
    };
    for (w, h) in [(10u16, 6u16), (16, 9), (40, 10), (120, 40)] {
        let area = Rect::new(0, 0, w, h);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        for frame in [0u64, 30, SKY_DWELL as u64] {
            crate::ui::render_scene(area, &mut buf, &scene, frame);
        }
    }
}

#[test]
fn teacher_records_show_every_section_score() {
    use crate::topic::Topic;
    let mut app = App::new(Config::default());
    app.roster = crate::student::Roster::default();
    // Give the current student one solve in each non-arithmetic section.
    {
        let s = app.roster.current_mut();
        s.record_topic(Topic::Units);
        s.record_topic(Topic::Fractions);
        s.record_topic(Topic::Percentages);
        s.record_topic(Topic::Percentages);
    }
    app.screen = Screen::Teacher;
    app.teacher_authed = true;
    app.teacher_view = TeacherView::Records;
    app.set_area(Rect::new(0, 0, 80, 24));

    let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
    term.draw(|f| ui::draw(f, &app)).unwrap();
    let text = buffer_to_string(term.backend().buffer());

    // The header carries a column for every section beyond the four ops.
    for h in ["Un", "Fr", "Pct", "Total", "Lock"] {
        assert!(text.contains(h), "records header missing {h:?} column");
    }
    // The Percentages count (2) and grand total (4) show in the row.
    assert!(app.roster.current().solved_for(Topic::Percentages) == 2);
    assert!(app.roster.current().grand_total() == 4);
}

#[test]
fn percent_card_reads_as_a_percent_not_a_fraction() {
    // Regression: a percentage problem reuses the shape card, but must NOT tell
    // the child to "type like 2/4" — it asks for a whole number percent.
    use crate::fraction::{generate, generate_percent, Mode};
    let area = Rect::new(0, 0, 60, 20);
    let mut rng = rand::thread_rng();

    let p = generate_percent(&mut rng);
    assert_eq!(p.mode, Mode::Percent);
    let mut buf = ratatui::buffer::Buffer::empty(area);
    crate::ui::render_shape_card(area, &mut buf, &p, "25", 1.0, None);
    let text = buffer_to_string(&buf);
    assert!(text.contains("Percentages"), "percent card should be titled Percentages");
    assert!(text.contains("25%"), "percent answer should show with a % sign");
    assert!(!text.contains("2/4"), "percent card must not instruct typing a fraction");

    // The fraction card still coaches the "a/b" form in its footer.
    let f = generate(&mut rng);
    let mut buf = ratatui::buffer::Buffer::empty(area);
    crate::ui::render_shape_card(area, &mut buf, &f, "2/4", 1.0, None);
    let text = buffer_to_string(&buf);
    assert!(text.contains("Fractions"));
    assert!(text.contains("2/4"), "fraction card coaches the a/b form");
}

#[test]
fn currency_matches_locality() {
    use crate::units::{generate, Locality, Theme};
    let mut rng = rand::thread_rng();
    for loc in Locality::ALL {
        let cur = loc.currency();
        let mut saw_money = false;
        for _ in 0..1000 {
            let p = generate(loc, &mut rng);
            if p.theme == Theme::Coin {
                saw_money = true;
                assert!(p.answer >= 0, "negative money answer for {}", loc.name());
                assert!(!p.unit_label.trim().is_empty());
                // Each prompt names this locality's money — its symbol or
                // (for amounts shown in minor units) the minor-unit name.
                let by_minor = !cur.minor_many.is_empty() && p.question.contains(cur.minor_many);
                assert!(p.question.contains(cur.symbol) || by_minor, "{:?} money not identifiable in: {}", loc.name(), p.question);
            }
        }
        assert!(saw_money, "no money problems appeared for {}", loc.name());
    }
    // The yen has no subunit, so its prompts never mention cents/fen.
    for _ in 0..1000 {
        let p = generate(Locality::Japan, &mut rng);
        assert!(!p.question.contains("cents") && !p.question.contains("fen"));
    }
}

#[test]
fn explorer_converts_and_handles_outrageous_values() {
    use crate::units::{convert, format_amount, Category};
    let vol = Category::Volume.units();
    let gallons = vol[6];
    let teaspoons = vol[0];
    assert_eq!(gallons.plural, "gallons");
    assert_eq!(teaspoons.plural, "teaspoons");
    // 8000 gallons is exactly 6,144,000 teaspoons.
    let r = convert(8000.0, gallons, teaspoons);
    assert_eq!(format_amount(r), "6,144,000");
    // Outrageous values don't blow up the formatter.
    let huge = convert(1e12, gallons, teaspoons);
    assert!(format_amount(huge).contains('e') || format_amount(huge).contains(','));
    assert_eq!(format_amount(0.0), "0");
}

#[test]
fn experiment_keys_drive_the_explorer() {
    use crossterm::event::{KeyCode, KeyEvent};
    let mut app = App::new(Config::default());
    app.screen = Screen::Experiment;
    // Cycle to the Category field and switch category; indices stay valid.
    app.exp_field = 3;
    app.on_key(KeyEvent::from(KeyCode::Right));
    let count = crate::units::Category::ALL[app.exp_category].units().len();
    assert!(app.exp_from < count && app.exp_to < count);
    // Typing edits the amount from any field.
    app.exp_amount.clear();
    for c in ['4', '2', '.', '5'] {
        app.on_key(KeyEvent::from(KeyCode::Char(c)));
    }
    assert_eq!(app.exp_amount, "42.5");
}

#[test]
fn unit_problems_are_well_formed_for_every_locality() {
    use crate::units::Locality;
    let mut rng = rand::thread_rng();
    for loc in Locality::ALL {
        for _ in 0..500 {
            let p = crate::units::generate(loc, &mut rng);
            assert!(p.answer >= 0, "negative measurement answer");
            assert!(!p.question.trim().is_empty());
            assert!(!p.unit_label.trim().is_empty());
        }
    }
}

#[test]
fn units_checkbox_makes_a_unit_session_that_advances() {
    let mut app = App::new(Config::default());
    // Only the Units checkbox on -> every problem is a measurement.
    app.menu_ops = [false, false, false, false];
    app.menu_units = true;
    app.start_session(false);
    app.set_area(Rect::new(0, 0, 80, 24));
    assert!(app.current_topic() == crate::topic::Topic::Units, "units-only session must show a unit problem");

    app.input = app.current.correct_answer_string();
    app.check_answer();
    assert!(app.transition.is_some(), "correct answer should start a transition");

    let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
    for _ in 0..60 {
        term.draw(|f| ui::draw(f, &app)).unwrap();
        app.on_tick();
        if app.transition.is_none() {
            break;
        }
    }
    assert!(app.transition.is_none());
    assert!(app.input.is_empty(), "input clears for the next problem");
    assert!(app.current_topic() == crate::topic::Topic::Units, "next problem is still a measurement");
}

#[test]
fn mixed_session_can_show_both_problem_types() {
    // Arithmetic + units enabled: over many problems we should see both.
    let mut app = App::new(Config::default());
    app.menu_ops = [true, false, false, false];
    app.menu_units = true;
    app.set_area(Rect::new(0, 0, 80, 24));
    let mut saw_unit = false;
    let mut saw_arith = false;
    for _ in 0..200 {
        app.start_session(false);
        if app.current_topic() == crate::topic::Topic::Units {
            saw_unit = true;
        } else {
            saw_arith = true;
        }
        if saw_unit && saw_arith {
            break;
        }
    }
    assert!(saw_unit && saw_arith, "a mixed session should produce both kinds");
}

#[test]
fn milestone_triggers_cinematic_then_resumes() {
    let mut app = App::new(Config::default());
    app.menu_ops = [true, false, false, false]; // addition only
    app.start_session(false);
    app.set_area(Rect::new(0, 0, 80, 24));
    // Sit at 6 solved so the next correct answer hits the 7 milestone.
    app.roster.current_mut().solved = [6, 0, 0, 0];

    app.input = app.current.correct_answer_string();
    app.check_answer();
    assert_eq!(app.screen, Screen::Cinematic);
    assert!(app.cinematic.is_some());

    // Play it out; it must hand control back to the lesson.
    let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
    for _ in 0..600 {
        term.draw(|f| ui::draw(f, &app)).unwrap();
        app.on_tick();
        if app.cinematic.is_none() {
            break;
        }
    }
    assert!(app.cinematic.is_none(), "cinematic should finish");
    assert_eq!(app.screen, Screen::Practice);
}

#[test]
fn repeated_wrong_answers_summon_encouragement() {
    let mut app = App::new(Config::default());
    app.menu_ops = [true, false, false, false];
    app.start_session(false);
    let wrong = (app.current.correct_answer_string().parse::<i64>().unwrap() + 1).to_string();
    for _ in 0..STRUGGLE_THRESHOLD {
        app.input = wrong.clone();
        app.check_answer();
    }
    assert!(app.encourage.is_some(), "duck should offer encouragement");
    assert!(app.help_active, "duck should appear");

    // Getting it right clears the support and stops the duck struggling.
    app.input = app.current.correct_answer_string();
    app.check_answer();
    assert!(app.encourage.is_none());
}

#[test]
fn student_progress_only_grows() {
    use crate::student::Student;
    use crate::topic::Topic;
    let mut s = Student::new("Sam");
    assert_eq!(s.total(), 0);
    s.record_topic(Topic::Add);
    s.record_topic(Topic::Add);
    s.record_topic(Topic::Mul);
    s.note_streak(3);
    s.note_streak(1); // a worse streak never lowers the best
    assert_eq!(s.solved_for(Topic::Add), 2);
    assert_eq!(s.solved_for(Topic::Mul), 1);
    assert_eq!(s.total(), 3);
    assert_eq!(s.best_streak, 3);
}

#[test]
fn teacher_password_set_and_verify() {
    let mut cfg = Config::default();
    assert!(!cfg.has_teacher_password());
    cfg.set_teacher_password("  "); // blank ignored
    assert!(!cfg.has_teacher_password());
    cfg.set_teacher_password("ducks123");
    assert!(cfg.has_teacher_password());
    assert!(cfg.verify_teacher_password("ducks123"));
    assert!(!cfg.verify_teacher_password("wrong"));
}

#[test]
fn records_admin_reset_and_remove() {
    use crate::student::{Roster, Student};
    let mut r = Roster::default();
    r.add("Ada");
    r.add("Grace");
    r.current_mut().record_topic(crate::topic::Topic::Mul);
    r.current_mut().note_streak(4);
    assert_eq!(r.current().total(), 1);

    // Reset zeroes the records.
    r.current_mut().reset();
    assert_eq!(r.current().total(), 0);
    assert_eq!(r.current().best_streak, 0);

    // Removing every student still leaves a Guest behind.
    let mut solo = Roster { students: vec![Student::new("Only")], current: 0 };
    solo.remove(0);
    assert_eq!(solo.students.len(), 1);
    assert_eq!(solo.students[0].name, "Guest");
}

#[test]
fn teacher_extras_join_the_why_list() {
    use crate::motivation::Extras;
    use crate::topic::Topic;
    let mut rng = rand::thread_rng();
    let mut extras = Extras::default();
    let custom = "Splitting my paycheck into savings jars";
    extras.add(Topic::Div, custom);
    // With a large draw the custom entry should be reachable.
    let mut seen = false;
    for _ in 0..200 {
        if motivation::pick(Topic::Div, &extras, &mut rng, 50).iter().any(|s| s == custom) {
            seen = true;
            break;
        }
    }
    assert!(seen, "teacher's custom reason never appeared");
}

#[test]
fn every_topic_offers_a_why_anecdote() {
    use crate::motivation::{self, Extras};
    use crate::topic::Topic;
    let mut rng = rand::thread_rng();
    let extras = Extras::default();
    // Each section must surface at least one built-in reason and a heading.
    for topic in Topic::ALL {
        let items = motivation::pick(topic, &extras, &mut rng, 4);
        assert!(!items.is_empty(), "{:?} has no built-in Why reasons", topic);
        assert!(motivation::heading(topic).contains(topic.name()));
    }
}

#[test]
fn each_topic_records_into_its_own_progress() {
    use crate::student::Student;
    use crate::topic::Topic;
    let mut s = Student::new("Pat");
    for topic in Topic::ALL {
        s.record_topic(topic);
    }
    // One per section: each counter is exactly 1, and the grand total is 7.
    for topic in Topic::ALL {
        assert_eq!(s.solved_for(topic), 1, "{:?} did not record", topic);
    }
    assert_eq!(s.grand_total(), Topic::ALL.len() as u32);
    // total() still counts only arithmetic (drives milestones).
    assert_eq!(s.total(), 4);
    // A per-topic reset zeroes just that section.
    s.reset_topic(Topic::Percentages);
    assert_eq!(s.solved_for(Topic::Percentages), 0);
    assert_eq!(s.grand_total(), Topic::ALL.len() as u32 - 1);
}

#[test]
fn percentages_mix_into_a_session_and_record() {
    // A percentages-only session: every problem is a percent, answered with a
    // plain integer, and a correct answer credits the percentages counter.
    let mut app = App::new(Config::default());
    app.roster = crate::student::Roster::default();
    app.menu_ops = [false, false, false, false];
    app.menu_percents = true;
    app.start_session(false);
    app.set_area(Rect::new(0, 0, 80, 24));
    assert_eq!(app.current_topic(), crate::topic::Topic::Percentages, "percentages-only session shows a percent");

    app.input = app.current.correct_answer_string();
    app.check_answer();
    assert_eq!(app.feedback, Feedback::Correct);
    assert_eq!(app.roster.current().solved_for(crate::topic::Topic::Percentages), 1);
    assert!(app.transition.is_some(), "correct answer kicks off a transition");
}

/// Moderate add/subtract problems should get a *visual* number line, not the
/// plain text fallback — and every line's hops must land on the answer.
#[test]
fn arithmetic_keeps_a_visual_number_line() {
    use crate::strategy::{strategies, Viz};
    let cases = [
        (12i64, 7i64, Op::Add),
        (40, 30, Op::Add),
        (23, 45, Op::Add),
        (8, 6, Op::Add),
        (53, 27, Op::Sub),
        (50, 20, Op::Sub),
        (12, 5, Op::Sub),
        (100, 64, Op::Sub),
    ];
    for (a, b, op) in cases {
        let answer = if op == Op::Add { a + b } else { a - b };
        let p = problem::Problem { a, b, op, answer, accent: ratatui::style::Color::White };
        let mut found = false;
        for s in strategies(&p) {
            if let Viz::NumberLine { stops, hops } = &s.viz {
                found = true;
                assert_eq!(stops.len(), hops.len() + 1, "stop/hop mismatch for {} {:?} {}", a, op, b);
                // A line lands on the answer (count-back/add) or on `a`
                // (count-up walks from b up to a).
                let last = *stops.last().unwrap();
                assert!(last == answer || last == a, "line for {} {:?} {} ends at {}", a, op, b, last);
            }
        }
        assert!(found, "{} {:?} {} lost its number line", a, op, b);
    }
}

/// Strategies are produced for every operation and grade, none empty.
#[test]
fn strategies_exist_for_all_problems() {
    let mut rng = rand::thread_rng();
    let cfg = Config::default();
    for grade in 0..=8u8 {
        for _ in 0..200 {
            let p = problem::generate(cfg.range(grade), &Op::ALL, &mut rng);
            let strats = strategy::strategies(&p);
            assert!(!strats.is_empty(), "no strategy for {:?}", p.op);
        }
    }
}
