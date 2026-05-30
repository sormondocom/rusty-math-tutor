//! "Why am I learning this?" — real-world uses for each operation.
//!
//! Pressing **Y** asks Deduction Duck why the current subject matters.  Rather
//! than repeat one canned answer, [`pick`] draws a fresh handful from a big
//! per-operation list, so the same question keeps surfacing new (and hopefully
//! surprising) reasons.

use rand::seq::SliceRandom;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::problem::Op;

const ADDITION: &[&str] = &[
    "Totalling up the cost of everything in your shopping cart",
    "Counting the points your team scored across a whole game",
    "Adding up calories so astronauts pack enough food for space",
    "Combining ingredient amounts when you double a cookie recipe",
    "Tallying votes to decide a class election",
    "Working out how many days until your birthday",
    "Summing rainfall over a month to plan a garden",
    "Adding lengths of lumber to frame a wall in construction",
    "Counting steps from a fitness tracker to hit a daily goal",
    "Adding the fuel in each rocket stage to reach orbit",
    "Totalling donations during a charity fundraiser",
    "Stacking coins to see if you can afford a new game",
    "Adding passengers boarding at every bus stop",
    "Combining the megabytes of photos to fit on a phone",
    "Summing the weights of cargo so a plane stays balanced",
    "Counting bricks needed for each row of a building",
    "Adding scores from several judges in a competition",
    "Totalling the heartbeats a doctor counts in a minute",
    "Adding up screen-time minutes across the week",
    "Counting seeds planted across rows on a farm",
];

const SUBTRACTION: &[&str] = &[
    "Working out the change you get back at a store",
    "Counting how many days of vacation are left",
    "Finding how much fuel a rocket has burned so far",
    "Figuring out how far you still have to drive on a trip",
    "Calculating how much battery percentage you have left",
    "Seeing how many cookies remain after sharing some",
    "Working out a sale price after a discount is taken off",
    "Measuring how much taller you've grown since last year",
    "Finding the temperature drop overnight for a weather report",
    "Checking how much money is left in a budget after bills",
    "Counting laps remaining in a race",
    "Working out how much rope to cut from a longer piece",
    "Finding the difference in scores between two teams",
    "Figuring out how many tickets are still unsold for a show",
    "Calculating elevation lost as a hiker descends a mountain",
    "Seeing how much water evaporated from a reservoir",
    "Working out how many minutes until the bell rings",
    "Finding leftover space on a hard drive after saving files",
    "Counting down the seconds for a rocket launch",
    "Figuring out how much weight a diet plan has dropped",
];

const MULTIPLICATION: &[&str] = &[
    "Finding the area of a room to buy the right carpet",
    "Scaling a recipe up to feed a whole party",
    "Calculating total pay from an hourly wage",
    "Working out how many tiles cover a floor (rows x columns)",
    "Figuring out thrust by combining many rocket engines",
    "Counting pixels on a screen (width times height)",
    "Estimating bricks for a wall that is many rows tall",
    "Computing how far you travel at a speed over many hours",
    "Multiplying servings to cook for a big family dinner",
    "Working out the cost of buying many of the same item",
    "Finding the volume of a fish tank to add the right water",
    "Calculating how much paint covers a large surface",
    "Figuring out total seats in a stadium (sections x seats)",
    "Scaling a model to its real-world size",
    "Computing data usage: megabytes per minute over hours",
    "Finding how many beats are in a song (measures x beats)",
    "Estimating crop yield across many identical fields",
    "Working out forces in engineering to keep bridges standing",
    "Calculating interest that makes savings grow",
    "Counting LEGO studs on a baseplate at a glance",
];

const DIVISION: &[&str] = &[
    "Sharing pizza slices equally among friends",
    "Splitting a restaurant bill fairly between everyone",
    "Working out miles per gallon to plan a road trip",
    "Figuring out how many buses are needed for a field trip",
    "Calculating a unit price to find the better deal",
    "Dividing a rocket's journey into equal fuel-burn stages",
    "Splitting candy evenly so nobody feels left out",
    "Working out average speed over a distance",
    "Dividing land into equal plots for farming",
    "Figuring out how many days a battery lasts per charge",
    "Sharing chores equally among family members",
    "Calculating how many teams you can make from a class",
    "Working out servings per person from a big pot of soup",
    "Finding beats per minute by dividing beats over time",
    "Splitting a long video into equal-length clips",
    "Figuring out frames per second in an animation",
    "Dividing a paycheck into save, spend, and give jars",
    "Working out how many shelves fit books of equal width",
    "Calculating the average score across many tests",
    "Sharing water rations equally on a long expedition",
];

// ---------------------------------------------------------------------------
// "For future coders" — a real line from THIS app and why it matters
// ---------------------------------------------------------------------------

/// A peek at the app's own source: a line of code plus why the math behind it
/// matters.  Aimed at curious would-be programmers.
pub struct CodeNote {
    pub code: &'static str,
    pub why: &'static str,
}

const ADD_CODE: &[CodeNote] = &[
    CodeNote {
        code: "self.progress += 1.0 / duration;",
        why: "Adding a little each frame is how one problem smoothly melts into the next.",
    },
    CodeNote {
        code: "self.streak += 1;",
        why: "Adding one every time you're right is how the app tracks your win streak.",
    },
    CodeNote {
        code: "(a + i) -> next stop on the line",
        why: "Adding one repeatedly is exactly what Deduction Duck does walking the number line.",
    },
];

const SUB_CODE: &[CodeNote] = &[
    CodeNote {
        code: "let timeout = TICK - last_tick.elapsed();",
        why: "Subtracting time already used from the frame budget keeps the animation smooth.",
    },
    CodeNote {
        code: "y = bottom - (bottom - apex) * t;",
        why: "Subtraction finds how high a firework rocket has climbed each frame.",
    },
];

const MUL_CODE: &[CodeNote] = &[
    CodeNote {
        code: "let f = progress * duration;",
        why: "Multiplying progress by length picks which animation frame to draw in a transition.",
    },
    CodeNote {
        code: "y += 0.5 * grav * f * f;",
        why: "Multiplication makes the gravity curve so exploding equation pieces arc and fall.",
    },
];

const DIV_CODE: &[CodeNote] = &[
    CodeNote {
        code: "let seg = axis_width / (n - 1);",
        why: "Dividing the width by the hops spaces the number-line stops out evenly.",
    },
    CodeNote {
        code: "let frac = remaining / total;",
        why: "Division turns time-left into the fraction that sizes the challenge countdown bar.",
    },
];

fn code_notes(op: Op) -> &'static [CodeNote] {
    match op {
        Op::Add => ADD_CODE,
        Op::Sub => SUB_CODE,
        Op::Mul => MUL_CODE,
        Op::Div => DIV_CODE,
    }
}

/// Pick a random code peek for `op`: (code line, why it matters).
pub fn pick_code(op: Op, rng: &mut impl Rng) -> Option<(String, String)> {
    code_notes(op).choose(rng).map(|n| (n.code.to_string(), n.why.to_string()))
}

fn applications(op: Op) -> &'static [&'static str] {
    match op {
        Op::Add => ADDITION,
        Op::Sub => SUBTRACTION,
        Op::Mul => MULTIPLICATION,
        Op::Div => DIVISION,
    }
}

/// A short heading for the overlay, e.g. "Why learn addition?".
pub fn heading(op: Op) -> &'static str {
    match op {
        Op::Add => "Why learn addition?",
        Op::Sub => "Why learn subtraction?",
        Op::Mul => "Why learn multiplication?",
        Op::Div => "Why learn division?",
    }
}

/// Pick up to `n` distinct real-world uses for `op`, in random order, drawing
/// from both the built-in list and the teacher's own [`Extras`].
pub fn pick(op: Op, extras: &Extras, rng: &mut impl Rng, n: usize) -> Vec<String> {
    let mut all: Vec<String> = applications(op).iter().map(|s| s.to_string()).collect();
    all.extend(extras.list(op).iter().cloned());
    all.choose_multiple(rng, n).cloned().collect()
}

// ---------------------------------------------------------------------------
// Teacher-supplied extras
// ---------------------------------------------------------------------------

/// Anecdotes a teacher adds from their own life, merged with the built-ins.
/// Persisted to `why_extras.json` so they survive between sessions and can be
/// edited by hand as well as from inside the app.
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Extras {
    #[serde(default)]
    add: Vec<String>,
    #[serde(default)]
    sub: Vec<String>,
    #[serde(default)]
    mul: Vec<String>,
    #[serde(default)]
    div: Vec<String>,
}

impl Extras {
    pub fn load() -> Extras {
        crate::config::data_path("why_extras.json")
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(path) = crate::config::data_path("why_extras.json") else { return };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }

    fn list(&self, op: Op) -> &[String] {
        match op {
            Op::Add => &self.add,
            Op::Sub => &self.sub,
            Op::Mul => &self.mul,
            Op::Div => &self.div,
        }
    }

    /// The teacher's own anecdotes for `op` (for display in the Teacher Area).
    pub fn items(&self, op: Op) -> &[String] {
        self.list(op)
    }

    fn list_mut(&mut self, op: Op) -> &mut Vec<String> {
        match op {
            Op::Add => &mut self.add,
            Op::Sub => &mut self.sub,
            Op::Mul => &mut self.mul,
            Op::Div => &mut self.div,
        }
    }

    /// Append a teacher's own reason for `op` (trimmed; blanks ignored).
    pub fn add(&mut self, op: Op, text: &str) {
        let text = text.trim();
        if !text.is_empty() {
            self.list_mut(op).push(text.to_string());
        }
    }
}
