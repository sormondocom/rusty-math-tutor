//! "Why am I learning this?" — real-world uses for each topic.
//!
//! Pressing **Y** asks Deduction Duck why the current subject matters.  Rather
//! than repeat one canned answer, [`pick`] draws a fresh handful from a big
//! per-topic list, so the same question keeps surfacing new (and hopefully
//! surprising) reasons.

use rand::seq::SliceRandom;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::topic::Topic;

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

const UNITS: &[&str] = &[
    "Converting inches to feet when measuring for new furniture",
    "Swapping cups for millilitres so a recipe works anywhere",
    "Reading a map's scale to know how far a trip really is",
    "Turning grams into kilograms to weigh luggage for a flight",
    "Knowing how many seconds are in an hour to time an experiment",
    "Converting Celsius to Fahrenheit to dress for the weather",
    "Changing metres to kilometres to log a run's distance",
    "Working out how many millilitres of medicine a dose needs",
    "Reading speed limits when a sign is in km/h instead of mph",
    "Measuring lumber in centimetres but buying it by the metre",
    "Converting ounces to pounds at the grocery deli counter",
    "Knowing litres of fuel to budget for a long drive",
    "Turning minutes into hours to plan a school schedule",
    "Scaling a model's millimetres up to a building's metres",
    "Comparing pack sizes (grams vs. kilograms) for the best deal",
    "Converting feet to metres when sharing a height worldwide",
];

const FRACTIONS: &[&str] = &[
    "Splitting a pizza so everyone gets an equal slice",
    "Halving a recipe when you only need a smaller batch",
    "Reading a tape measure marked in halves and quarters",
    "Sharing a chocolate bar fairly between friends",
    "Telling time: a quarter past and half past the hour",
    "Filling a measuring cup to two-thirds for baking",
    "Splitting a bill three ways at a restaurant",
    "Knowing a tank is a quarter full before a long drive",
    "Mixing paint in parts to get just the right shade",
    "Dividing a garden bed into equal planting rows",
    "Cutting fabric into equal pieces for a sewing project",
    "Understanding a sale: take a third off the price",
    "Sharing screen time so each kid gets a fair fraction",
    "Reading sheet music where a note lasts half a beat",
    "Splitting a long hike into equal-distance segments",
    "Measuring a half-cup of flour with a one-third scoop",
];

const GEOMETRY: &[&str] = &[
    "Working out how much carpet covers a bedroom floor (area)",
    "Buying the right length of fence for a backyard (perimeter)",
    "Filling a fish tank with the right amount of water (volume)",
    "Cutting wrapping paper to fit a present's surface",
    "Measuring trim to run around a window frame",
    "Figuring out how much paint covers a wall",
    "Packing boxes so they fill a moving truck efficiently",
    "Designing a soccer field with the right dimensions",
    "Sizing a rug so it fits a living room",
    "Working out how much soil fills a raised garden bed",
    "Planning tiles to cover a kitchen backsplash",
    "Estimating concrete to pour a rectangular patio",
    "Knowing how much ribbon goes around a gift box",
    "Calculating storage space inside a closet",
    "Laying out a running track's distance around the oval",
    "Building a sandbox and filling it with the right sand",
];

const PERCENTAGES: &[&str] = &[
    "Working out how much you save with 25% off at a sale",
    "Reading a phone battery that says it's 80% charged",
    "Figuring out a tip — 15% or 20% — at a restaurant",
    "Understanding a test score given as a percentage",
    "Seeing what percent of a goal a fundraiser has reached",
    "Comparing loan or savings interest rates as percentages",
    "Reading a weather forecast: a 70% chance of rain",
    "Knowing a juice is 100% fruit with no added sugar",
    "Working out sales tax added to the price of a toy",
    "Tracking what percent of a video game level is complete",
    "Understanding nutrition labels: percent of daily value",
    "Seeing a poll where a candidate has 52% support",
    "Figuring out a 10% discount coupon at checkout",
    "Reading how much brighter a screen is at 50% vs 100%",
    "Comparing two phones by percent of storage used",
    "Knowing a hill has a 6% grade before biking up it",
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

const UNITS_CODE: &[CodeNote] = &[
    CodeNote {
        code: "let metres = feet as f64 * 0.3048;",
        why: "A conversion factor is just one number you multiply by to switch units.",
    },
];

const FRACTIONS_CODE: &[CodeNote] = &[
    CodeNote {
        code: "let lit = (slices as f64 * progress) as usize;",
        why: "Scaling a fraction by progress is how shaded pieces materialize one at a time.",
    },
];

const PERCENTAGES_CODE: &[CodeNote] = &[
    CodeNote {
        code: "let pct = shaded as f64 / total as f64 * 100.0;",
        why: "A percent is just a fraction scaled so the whole is always 100.",
    },
];

const GEOMETRY_CODE: &[CodeNote] = &[
    CodeNote {
        code: "let area = w * h;",
        why: "Multiplying width by height is how a rectangle's whole inside is counted.",
    },
    CodeNote {
        code: "let volume = l * w * h;",
        why: "Stacking area through a depth — three multiplications — fills a box with space.",
    },
];

fn code_notes(topic: Topic) -> &'static [CodeNote] {
    match topic {
        Topic::Add => ADD_CODE,
        Topic::Sub => SUB_CODE,
        Topic::Mul => MUL_CODE,
        Topic::Div => DIV_CODE,
        Topic::Units => UNITS_CODE,
        Topic::Fractions => FRACTIONS_CODE,
        Topic::Percentages => PERCENTAGES_CODE,
        Topic::Geometry => GEOMETRY_CODE,
    }
}

/// Pick a random code peek for `topic`: (code line, why it matters).
pub fn pick_code(topic: Topic, rng: &mut impl Rng) -> Option<(String, String)> {
    code_notes(topic).choose(rng).map(|n| (n.code.to_string(), n.why.to_string()))
}

fn applications(topic: Topic) -> &'static [&'static str] {
    match topic {
        Topic::Add => ADDITION,
        Topic::Sub => SUBTRACTION,
        Topic::Mul => MULTIPLICATION,
        Topic::Div => DIVISION,
        Topic::Units => UNITS,
        Topic::Fractions => FRACTIONS,
        Topic::Percentages => PERCENTAGES,
        Topic::Geometry => GEOMETRY,
    }
}

/// A short heading for the overlay, e.g. "Why learn Addition?".
pub fn heading(topic: Topic) -> String {
    format!("Why learn {}?", topic.name())
}

/// Pick up to `n` distinct real-world uses for `topic`, in random order, drawing
/// from both the built-in list and the teacher's own [`Extras`].
pub fn pick(topic: Topic, extras: &Extras, rng: &mut impl Rng, n: usize) -> Vec<String> {
    let mut all: Vec<String> = applications(topic).iter().map(|s| s.to_string()).collect();
    all.extend(extras.list(topic).iter().cloned());
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
    #[serde(default)]
    units: Vec<String>,
    #[serde(default)]
    fractions: Vec<String>,
    #[serde(default)]
    percentages: Vec<String>,
    #[serde(default)]
    geometry: Vec<String>,
}

impl Extras {
    pub fn load(storage: &dyn crate::storage::Storage) -> Extras {
        storage
            .load("why_extras.json")
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, storage: &dyn crate::storage::Storage) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            storage.save("why_extras.json", &json);
        }
    }

    fn list(&self, topic: Topic) -> &[String] {
        match topic {
            Topic::Add => &self.add,
            Topic::Sub => &self.sub,
            Topic::Mul => &self.mul,
            Topic::Div => &self.div,
            Topic::Units => &self.units,
            Topic::Fractions => &self.fractions,
            Topic::Percentages => &self.percentages,
            Topic::Geometry => &self.geometry,
        }
    }

    /// The teacher's own anecdotes for `topic` (for display in the Teacher Area).
    pub fn items(&self, topic: Topic) -> &[String] {
        self.list(topic)
    }

    fn list_mut(&mut self, topic: Topic) -> &mut Vec<String> {
        match topic {
            Topic::Add => &mut self.add,
            Topic::Sub => &mut self.sub,
            Topic::Mul => &mut self.mul,
            Topic::Div => &mut self.div,
            Topic::Units => &mut self.units,
            Topic::Fractions => &mut self.fractions,
            Topic::Percentages => &mut self.percentages,
            Topic::Geometry => &mut self.geometry,
        }
    }

    /// Append a teacher's own reason for `topic` (trimmed; blanks ignored).
    pub fn add(&mut self, topic: Topic, text: &str) {
        let text = text.trim();
        if !text.is_empty() {
            self.list_mut(topic).push(text.to_string());
        }
    }
}
