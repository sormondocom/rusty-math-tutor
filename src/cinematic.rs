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

/// One screen of a milestone cinematic.
pub struct Scene {
    pub accent: Color,
    /// Optional block-font centrepiece (digits/symbols only — e.g. the count).
    pub big: Option<String>,
    pub heading: String,
    pub sub: String,
}

/// Build the cinematic for `name` reaching `count` problems solved.
pub fn scenes(name: &str, count: u32, rng: &mut impl Rng) -> Vec<Scene> {
    vec![
        Scene {
            accent: *ACCENTS.choose(rng).unwrap(),
            big: Some(count.to_string()),
            heading: "PROBLEMS SOLVED!".to_string(),
            sub: "★   ★   ★".to_string(),
        },
        Scene {
            accent: *ACCENTS.choose(rng).unwrap(),
            big: None,
            heading: format!("Way to go, {}!", name),
            sub: "You're unstoppable!".to_string(),
        },
        Scene {
            accent: *ACCENTS.choose(rng).unwrap(),
            big: None,
            heading: milestone_line(count).to_string(),
            sub: "Keep up the amazing work!".to_string(),
        },
    ]
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
