//! Celebratory milestone cinematics and gentle struggle encouragement.
//!
//! When a student's running total reaches one of the [`MILESTONES`], the app
//! plays a short, name-personalised cinematic — a handful of text "scenes" that
//! our [`crate::transition`] effects animate between.  Separately, when a
//! learner gets the *same* problem wrong a few times, Deduction Duck offers an
//! encouraging word ([`struggle_message`]).

use rand::seq::SliceRandom;
use rand::Rng;
use ratatui::style::Color;

/// Totals that trigger a celebration.
pub const MILESTONES: [u32; 6] = [7, 11, 33, 100, 123, 333];

/// Whether `total` is exactly a milestone worth celebrating.
pub fn is_milestone(total: u32) -> bool {
    MILESTONES.contains(&total)
}

const ACCENTS: [Color; 6] = [
    Color::LightCyan,
    Color::LightGreen,
    Color::LightYellow,
    Color::LightMagenta,
    Color::LightRed,
    Color::LightBlue,
];

/// How long (in ticks) a still text scene lingers before moving on.
pub const TEXT_DWELL: u32 = 24;
/// The animated night-sky scene lingers longer so the moon can finish rising
/// and the comet stream gets time to play.
pub const SKY_DWELL: u32 = 130;
/// The rocket scene needs time to launch, form the name, and let a comet pass.
pub const ROCKET_DWELL: u32 = 150;

/// What a scene draws.  Most scenes are still text; some are live animations.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SceneKind {
    /// A centred big number (optional) plus heading and sub-heading.
    Text,
    /// A night sky: a comet stream, the student's name written among the stars,
    /// and a moon rising underneath.  Driven by the scene's elapsed frame.
    NameSky,
    /// Rockets launch and become the stars that spell the learner's name, after
    /// which a blue comet passes behind it.  Driven by the scene's elapsed frame.
    RocketName,
}

/// One screen of a milestone cinematic.
pub struct Scene {
    pub kind: SceneKind,
    pub accent: Color,
    /// Optional block-font centrepiece (digits/symbols only — e.g. the count).
    pub big: Option<String>,
    pub heading: String,
    pub sub: String,
    /// How many ticks to linger on this scene before transitioning onward.
    pub dwell: u32,
}

impl Scene {
    /// A still text scene with the default dwell.
    fn text(accent: Color, big: Option<String>, heading: String, sub: String) -> Scene {
        Scene { kind: SceneKind::Text, accent, big, heading, sub, dwell: TEXT_DWELL }
    }
}

/// Build the cinematic for `name` reaching `count` problems solved.
pub fn scenes(name: &str, count: u32, rng: &mut impl Rng) -> Vec<Scene> {
    vec![
        Scene::text(
            *ACCENTS.choose(rng).unwrap(),
            Some(count.to_string()),
            "PROBLEMS SOLVED!".to_string(),
            "★   ★   ★".to_string(),
        ),
        // The showpiece — the learner's name in the sky.  Pick one of two for
        // variety across milestones.
        name_showpiece(name, rng),
        Scene::text(
            *ACCENTS.choose(rng).unwrap(),
            None,
            milestone_line(count).to_string(),
            "Keep up the amazing work!".to_string(),
        ),
    ]
}

/// A name-in-the-sky scene: either rockets that form the name (then a blue comet
/// passes behind), or a comet-streaked sky with a rising moon.
fn name_showpiece(name: &str, rng: &mut impl Rng) -> Scene {
    if rng.gen_bool(0.5) {
        Scene {
            kind: SceneKind::RocketName,
            accent: Color::Rgb(255, 232, 150), // warm starlight
            big: None,
            heading: name.to_string(),
            sub: "written in the stars!".to_string(),
            dwell: ROCKET_DWELL,
        }
    } else {
        Scene {
            kind: SceneKind::NameSky,
            accent: Color::LightCyan,
            big: None,
            heading: name.to_string(),
            sub: "written in the stars!".to_string(),
            dwell: SKY_DWELL,
        }
    }
}

fn milestone_line(count: u32) -> &'static str {
    match count {
        7 => "Seven solved — lucky and learning!",
        11 => "Eleven down — you're on a roll!",
        33 => "Thirty-three! You're a math machine!",
        100 => "ONE HUNDRED problems! Phenomenal!",
        123 => "1-2-3... 123 problems! So smooth!",
        333 => "333!! A true math champion!",
        _ => "What an amazing achievement!",
    }
}

const STRUGGLE: &[&str] = &[
    "Don't worry — mistakes help your brain grow!",
    "You're close! Let's break it down together.",
    "Tricky one! Take a breath and try my hint.",
    "Every expert was once a beginner. You've got this!",
    "Quack! We'll figure this out as a team.",
    "Slow and steady — I believe in you!",
];

/// A random encouraging line for a student who's stuck.
pub fn struggle_message(rng: &mut impl Rng) -> String {
    STRUGGLE.choose(rng).unwrap().to_string()
}

/// Clever, kind reprimands when a student keeps asking for the answer after the
/// peek limit.  They escalate gently — `index` is clamped to the last one.
const REPRIMANDS: &[&str] = &[
    "A garden won't grow unwatered — and your mind is the garden!",
    "Your brain's a muscle: it only grows when YOU lift it.",
    "The answer tastes sweeter when YOU find it. Have a go!",
    "Peeking won't make it stick — give it a real try!",
    "Even I had to practise, feather by feather. You've got this!",
    "No more peeks for now — I believe in your clever brain!",
];

pub fn peek_reprimand(index: usize) -> String {
    REPRIMANDS[index.min(REPRIMANDS.len() - 1)].to_string()
}
