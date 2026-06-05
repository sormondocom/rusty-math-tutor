//! Units of Measure — measurement word problems with a settable locality.
//!
//! Problems come in three flavours: unit **conversions** ("how many cups are in
//! 2 quarts?"), **combining** measures ("3 cups of flour and 2 more…"), and
//! **differences** ("you need 8, you have 5…").  The [`Locality`] chooses the
//! measurement system, so a student in the US practises cups and gallons while
//! one almost anywhere else practises millilitres and litres.
//!
//! Each problem carries a [`Theme`] so the UI can play a matching, silly
//! Deduction Duck animation (hop into a cup, splash into a pool, …).

use rand::seq::SliceRandom;
use rand::Rng;
use serde::{Deserialize, Serialize};

/// Where the student is — picks the measurement system.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Locality {
    #[default]
    UnitedStates,
    UnitedKingdom,
    Metric,
    China,
    Japan,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum System {
    Imperial,
    Metric,
}

impl Locality {
    pub const ALL: [Locality; 5] =
        [Locality::UnitedStates, Locality::UnitedKingdom, Locality::Metric, Locality::China, Locality::Japan];

    pub fn name(self) -> &'static str {
        match self {
            Locality::UnitedStates => "United States (dollars, gallons)",
            Locality::UnitedKingdom => "United Kingdom (pounds, miles)",
            Locality::Metric => "Metric (euros, litres)",
            Locality::China => "China (yuan, metric)",
            Locality::Japan => "Japan (yen, metric)",
        }
    }

    pub fn system(self) -> System {
        match self {
            Locality::UnitedStates | Locality::UnitedKingdom => System::Imperial,
            Locality::Metric | Locality::China | Locality::Japan => System::Metric,
        }
    }

    /// The money used in this locality.
    pub fn currency(self) -> &'static Currency {
        match self {
            Locality::UnitedStates => &USD,
            Locality::UnitedKingdom => &GBP,
            Locality::Metric => &EUR,
            Locality::China => &CNY,
            Locality::Japan => &JPY,
        }
    }
}

/// A currency: its main unit, its smaller "minor" unit (if any), symbol, and
/// everyday coins (valued in minor units).
pub struct Currency {
    pub main_many: &'static str,
    /// Plural minor-unit name (e.g. "cents"); empty if there's no subunit.
    pub minor_many: &'static str,
    /// Minor units per main unit (100 for cents/pence/fen, 1 for the yen).
    pub minor_per_main: i64,
    pub symbol: &'static str,
    /// (coin name plural, value in minor units).
    pub coins: &'static [(&'static str, i64)],
}

const USD: Currency = Currency {
    main_many: "dollars",
    minor_many: "cents",
    minor_per_main: 100,
    symbol: "$",
    coins: &[("pennies", 1), ("nickels", 5), ("dimes", 10), ("quarters", 25)],
};

const GBP: Currency = Currency {
    main_many: "pounds",
    minor_many: "pence",
    minor_per_main: 100,
    symbol: "£",
    coins: &[("pennies", 1), ("5p coins", 5), ("10p coins", 10), ("20p coins", 20), ("50p coins", 50)],
};

const EUR: Currency = Currency {
    main_many: "euros",
    minor_many: "cents",
    minor_per_main: 100,
    symbol: "€",
    coins: &[("1-cent coins", 1), ("5-cent coins", 5), ("10-cent coins", 10), ("20-cent coins", 20), ("50-cent coins", 50)],
};

// Chinese yuan: 1 yuan = 10 jiao = 100 fen.
const CNY: Currency = Currency {
    main_many: "yuan",
    minor_many: "fen",
    minor_per_main: 100,
    symbol: "¥",
    coins: &[("1-jiao coins", 10), ("5-jiao coins", 50), ("1-yuan coins", 100)],
};

// Japanese yen: no subunit in everyday use.
const JPY: Currency = Currency {
    main_many: "yen",
    minor_many: "",
    minor_per_main: 1,
    symbol: "¥",
    coins: &[("1-yen coins", 1), ("5-yen coins", 5), ("10-yen coins", 10), ("50-yen coins", 50), ("100-yen coins", 100), ("500-yen coins", 500)],
};

/// Which silly animation accompanies a problem.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Theme {
    Cup,
    Pool,
    Bottle,
    Scale,
    Ruler,
    Coin,
}

pub struct UnitProblem {
    pub question: String,
    pub answer: i64,
    pub unit_label: String,
    pub theme: Theme,
    /// Whether to play the full duck-jump gag (vs. just showing the prop).
    pub duck_jump: bool,
    /// Deduction Duck's hint for how to approach it (never gives the answer).
    pub hint: Vec<String>,
}

// ---------------------------------------------------------------------------
// Fact tables
// ---------------------------------------------------------------------------

struct Conv {
    small: &'static str,
    large_one: &'static str,
    large_many: &'static str,
    factor: i64,
    theme: Theme,
}

struct Combine {
    unit: &'static str,
    item: &'static str,
    theme: Theme,
}

const IMP_CONV: &[Conv] = &[
    Conv { small: "cups", large_one: "pint", large_many: "pints", factor: 2, theme: Theme::Cup },
    Conv { small: "cups", large_one: "quart", large_many: "quarts", factor: 4, theme: Theme::Cup },
    Conv { small: "cups", large_one: "gallon", large_many: "gallons", factor: 16, theme: Theme::Pool },
    Conv { small: "fluid ounces", large_one: "cup", large_many: "cups", factor: 8, theme: Theme::Cup },
    Conv { small: "quarts", large_one: "gallon", large_many: "gallons", factor: 4, theme: Theme::Pool },
    Conv { small: "ounces", large_one: "pound", large_many: "pounds", factor: 16, theme: Theme::Scale },
    Conv { small: "inches", large_one: "foot", large_many: "feet", factor: 12, theme: Theme::Ruler },
    Conv { small: "feet", large_one: "yard", large_many: "yards", factor: 3, theme: Theme::Ruler },
];

const MET_CONV: &[Conv] = &[
    Conv { small: "millilitres", large_one: "litre", large_many: "litres", factor: 1000, theme: Theme::Bottle },
    Conv { small: "grams", large_one: "kilogram", large_many: "kilograms", factor: 1000, theme: Theme::Scale },
    Conv { small: "centimetres", large_one: "metre", large_many: "metres", factor: 100, theme: Theme::Ruler },
    Conv { small: "millimetres", large_one: "centimetre", large_many: "centimetres", factor: 10, theme: Theme::Ruler },
    Conv { small: "metres", large_one: "kilometre", large_many: "kilometres", factor: 1000, theme: Theme::Ruler },
];

const IMP_COMB: &[Combine] = &[
    Combine { unit: "cups", item: "flour", theme: Theme::Cup },
    Combine { unit: "gallons", item: "water", theme: Theme::Pool },
    Combine { unit: "ounces", item: "sugar", theme: Theme::Scale },
    Combine { unit: "inches", item: "ribbon", theme: Theme::Ruler },
];

const MET_COMB: &[Combine] = &[
    Combine { unit: "millilitres", item: "water", theme: Theme::Bottle },
    Combine { unit: "litres", item: "juice", theme: Theme::Pool },
    Combine { unit: "grams", item: "sugar", theme: Theme::Scale },
    Combine { unit: "centimetres", item: "string", theme: Theme::Ruler },
];

fn conversions(sys: System) -> &'static [Conv] {
    match sys {
        System::Imperial => IMP_CONV,
        System::Metric => MET_CONV,
    }
}

fn combines(sys: System) -> &'static [Combine] {
    match sys {
        System::Imperial => IMP_COMB,
        System::Metric => MET_COMB,
    }
}

// ---------------------------------------------------------------------------
// Generation
// ---------------------------------------------------------------------------

/// A fresh measurement (or money) problem for `locality`.
pub fn generate(locality: Locality, rng: &mut impl Rng) -> UnitProblem {
    // Roughly a third of the time, practise the local money instead.
    if rng.gen_bool(0.3) {
        return currency_problem(locality, rng);
    }
    let sys = locality.system();
    let duck_jump = rng.gen_bool(0.7);
    match rng.gen_range(0..3) {
        0 => convert_problem(sys, rng, duck_jump),
        1 => combine(sys, rng, duck_jump),
        _ => difference(sys, rng, duck_jump),
    }
}

fn currency_problem(locality: Locality, rng: &mut impl Rng) -> UnitProblem {
    let c = locality.currency();
    let duck_jump = rng.gen_bool(0.7);
    let has_minor = c.minor_per_main > 1;
    // Currencies with no subunit (the yen) skip the cents <-> main problem.
    let kind = if has_minor { rng.gen_range(0..4) } else { rng.gen_range(1..4) };
    match kind {
        // Main unit <-> minor unit (exact).
        0 => {
            if rng.gen_bool(0.5) {
                let n = rng.gen_range(1..=6);
                UnitProblem {
                    question: format!("How many {} are in {}{}?", c.minor_many, c.symbol, n),
                    answer: n * c.minor_per_main,
                    unit_label: c.minor_many.to_string(),
                    theme: Theme::Coin,
                    duck_jump,
                    hint: vec![
                        format!("{}1 is the same as {} {}.", c.symbol, c.minor_per_main, c.minor_many),
                        format!("So multiply {} by {}.", n, c.minor_per_main),
                    ],
                }
            } else {
                let m = rng.gen_range(1..=6);
                UnitProblem {
                    question: format!("How many {} are in {} {}?", c.main_many, m * c.minor_per_main, c.minor_many),
                    answer: m,
                    unit_label: c.main_many.to_string(),
                    theme: Theme::Coin,
                    duck_jump,
                    hint: vec![
                        format!("{} {} make {}1.", c.minor_per_main, c.minor_many, c.symbol),
                        format!("So divide by {}.", c.minor_per_main),
                    ],
                }
            }
        }
        // Coin counting: how many of a coin make a tidy amount.
        1 => {
            let (coin, value) = *c.coins.choose(rng).unwrap();
            let k = rng.gen_range(2..=12);
            let total_minor = k * value;
            // Phrase the target as a whole main amount when it divides evenly,
            // otherwise in the minor unit.
            let target = if total_minor % c.minor_per_main == 0 {
                format!("{}{}", c.symbol, total_minor / c.minor_per_main)
            } else {
                format!("{} {}", total_minor, c.minor_many)
            };
            let worth = if c.minor_per_main > 1 {
                format!("{} {}", value, c.minor_many)
            } else {
                format!("{}{}", c.symbol, value)
            };
            UnitProblem {
                question: format!("How many {} make {}?", coin, target),
                answer: k,
                unit_label: coin.to_string(),
                theme: Theme::Coin,
                duck_jump,
                hint: vec![
                    format!("Each {} is worth {}.", coin, worth),
                    format!("Count how many fit into {}.", target),
                ],
            }
        }
        // Combining money.
        2 => {
            let a = rng.gen_range(1..=9);
            let b = rng.gen_range(1..=9);
            UnitProblem {
                question: format!("You have {}{} and find {}{} more. How much in all?", c.symbol, a, c.symbol, b),
                answer: a + b,
                unit_label: c.main_many.to_string(),
                theme: Theme::Coin,
                duck_jump,
                hint: vec!["Putting money together means adding.".to_string(), "Add the two amounts.".to_string()],
            }
        }
        // Making change.
        _ => {
            let cost = rng.gen_range(3..=12);
            let paid = rng.gen_range(cost..=cost + 8);
            UnitProblem {
                question: format!("A toy costs {}{}. You pay {}{}. How much change?", c.symbol, cost, c.symbol, paid),
                answer: paid - cost,
                unit_label: c.main_many.to_string(),
                theme: Theme::Coin,
                duck_jump,
                hint: vec!["Change is the money left after paying.".to_string(), "Subtract the cost from what you paid.".to_string()],
            }
        }
    }
}

fn convert_problem(sys: System, rng: &mut impl Rng, duck_jump: bool) -> UnitProblem {
    let c = conversions(sys).choose(rng).unwrap();
    if rng.gen_bool(0.5) {
        // Big unit down to small unit.
        let n = rng.gen_range(1..=6);
        let large = if n == 1 { c.large_one } else { c.large_many };
        UnitProblem {
            question: format!("How many {} are in {} {}?", c.small, n, large),
            answer: n * c.factor,
            unit_label: c.small.to_string(),
            theme: c.theme,
            duck_jump,
            hint: vec![
                format!("1 {} = {} {}.", c.large_one, c.factor, c.small),
                format!("So multiply {} by {}.", n, c.factor),
            ],
        }
    } else {
        // Small unit up to big unit (always exact).
        let q = rng.gen_range(1..=6);
        let total = q * c.factor;
        UnitProblem {
            question: format!("How many {} are in {} {}?", c.large_many, total, c.small),
            answer: q,
            unit_label: c.large_many.to_string(),
            theme: c.theme,
            duck_jump,
            hint: vec![
                format!("{} {} make 1 {}.", c.factor, c.small, c.large_one),
                format!("So divide {} by {}.", total, c.factor),
            ],
        }
    }
}

fn combine(sys: System, rng: &mut impl Rng, duck_jump: bool) -> UnitProblem {
    let cb = combines(sys).choose(rng).unwrap();
    let a = rng.gen_range(1..=9);
    let b = rng.gen_range(1..=9);
    UnitProblem {
        question: format!("A recipe needs {} {} of {} and {} more {}. How many {} in all?", a, cb.unit, cb.item, b, cb.unit, cb.unit),
        answer: a + b,
        unit_label: cb.unit.to_string(),
        theme: cb.theme,
        duck_jump,
        hint: vec!["Combining amounts means adding.".to_string(), format!("Add {} and {} together.", a, b)],
    }
}

fn difference(sys: System, rng: &mut impl Rng, duck_jump: bool) -> UnitProblem {
    let cb = combines(sys).choose(rng).unwrap();
    let a = rng.gen_range(4..=12);
    let b = rng.gen_range(1..a);
    UnitProblem {
        question: format!("You need {} {} of {} but only have {}. How many more {} do you need?", a, cb.unit, cb.item, b, cb.unit),
        answer: a - b,
        unit_label: cb.unit.to_string(),
        theme: cb.theme,
        duck_jump,
        hint: vec!["Find how much is missing.".to_string(), format!("Subtract {} from {}.", b, a)],
    }
}

// ---------------------------------------------------------------------------
// Free-form unit explorer (the "Experimentation" section)
// ---------------------------------------------------------------------------

/// A unit in the converter, with its size relative to its category's base.
#[derive(Copy, Clone)]
pub struct ConvUnit {
    pub plural: &'static str,
    /// How many base units (ml / g / mm) one of these equals.
    pub to_base: f64,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Category {
    Volume,
    Mass,
    Length,
}

// Volume factors are exact US relations (1 gallon = 768 teaspoons, …) so the
// common conversions land on round numbers rather than drifting.
const VOLUME: &[ConvUnit] = &[
    ConvUnit { plural: "teaspoons", to_base: 4.92892159375 },
    ConvUnit { plural: "tablespoons", to_base: 14.78676478125 },
    ConvUnit { plural: "fluid ounces", to_base: 29.5735295625 },
    ConvUnit { plural: "cups", to_base: 236.5882365 },
    ConvUnit { plural: "pints", to_base: 473.176473 },
    ConvUnit { plural: "quarts", to_base: 946.352946 },
    ConvUnit { plural: "gallons", to_base: 3785.411784 },
    ConvUnit { plural: "millilitres", to_base: 1.0 },
    ConvUnit { plural: "litres", to_base: 1000.0 },
];

const MASS: &[ConvUnit] = &[
    ConvUnit { plural: "grams", to_base: 1.0 },
    ConvUnit { plural: "kilograms", to_base: 1000.0 },
    ConvUnit { plural: "ounces", to_base: 28.349523125 },
    ConvUnit { plural: "pounds", to_base: 453.59237 },
    ConvUnit { plural: "tonnes", to_base: 1_000_000.0 },
];

const LENGTH: &[ConvUnit] = &[
    ConvUnit { plural: "millimetres", to_base: 1.0 },
    ConvUnit { plural: "centimetres", to_base: 10.0 },
    ConvUnit { plural: "metres", to_base: 1000.0 },
    ConvUnit { plural: "kilometres", to_base: 1_000_000.0 },
    ConvUnit { plural: "inches", to_base: 25.4 },
    ConvUnit { plural: "feet", to_base: 304.8 },
    ConvUnit { plural: "yards", to_base: 914.4 },
    ConvUnit { plural: "miles", to_base: 1_609_344.0 },
];

impl Category {
    pub const ALL: [Category; 3] = [Category::Volume, Category::Mass, Category::Length];

    pub fn name(self) -> &'static str {
        match self {
            Category::Volume => "Volume",
            Category::Mass => "Mass",
            Category::Length => "Length",
        }
    }

    pub fn units(self) -> &'static [ConvUnit] {
        match self {
            Category::Volume => VOLUME,
            Category::Mass => MASS,
            Category::Length => LENGTH,
        }
    }
}

/// Convert `amount` of `from` into `to` (same category assumed).
pub fn convert(amount: f64, from: ConvUnit, to: ConvUnit) -> f64 {
    amount * from.to_base / to.to_base
}

/// A famous real-world thing of a known size, for "that's about as much as…"
/// reactions.  Magnitudes are in each category's base unit (ml / g / mm) and
/// listed smallest → largest.
#[derive(Copy, Clone)]
pub struct Comparison {
    pub base: f64,
    /// Singular with article, e.g. "an Olympic pool".
    pub one: &'static str,
    /// Plural, e.g. "Olympic pools".
    pub many: &'static str,
}

const VOLUME_CMP: &[Comparison] = &[
    Comparison { base: 5.0, one: "a teaspoon", many: "teaspoons" },
    Comparison { base: 355.0, one: "a soda can", many: "soda cans" },
    Comparison { base: 500.0, one: "a water bottle", many: "water bottles" },
    Comparison { base: 3785.0, one: "a milk jug", many: "milk jugs" },
    Comparison { base: 150_000.0, one: "a bathtub", many: "bathtubs" },
    Comparison { base: 1_500_000.0, one: "a hot tub", many: "hot tubs" },
    Comparison { base: 30_000_000.0, one: "a tanker truck", many: "tanker trucks" },
    Comparison { base: 2_500_000_000.0, one: "an Olympic pool", many: "Olympic pools" },
];

const MASS_CMP: &[Comparison] = &[
    Comparison { base: 1.0, one: "a paperclip", many: "paperclips" },
    Comparison { base: 150.0, one: "an apple", many: "apples" },
    Comparison { base: 1000.0, one: "a bag of sugar", many: "bags of sugar" },
    Comparison { base: 4500.0, one: "a house cat", many: "house cats" },
    Comparison { base: 75_000.0, one: "a person", many: "people" },
    Comparison { base: 1_500_000.0, one: "a small car", many: "small cars" },
    Comparison { base: 6_000_000.0, one: "an elephant", many: "elephants" },
    Comparison { base: 150_000_000.0, one: "a blue whale", many: "blue whales" },
];

const LENGTH_CMP: &[Comparison] = &[
    Comparison { base: 5.0, one: "an ant", many: "ants" },
    Comparison { base: 85.0, one: "a credit card", many: "credit cards" },
    Comparison { base: 1700.0, one: "a tall person", many: "tall people" },
    Comparison { base: 12_000.0, one: "a school bus", many: "school buses" },
    Comparison { base: 100_000.0, one: "a football field", many: "football fields" },
    Comparison { base: 330_000.0, one: "the Eiffel Tower", many: "Eiffel Towers" },
    Comparison { base: 8_849_000.0, one: "Mount Everest", many: "Mount Everests" },
    Comparison { base: 42_195_000.0, one: "a marathon", many: "marathons" },
    Comparison { base: 384_400_000_000.0, one: "the trip to the Moon", many: "trips to the Moon" },
];

/// Real-world size comparisons for a category, smallest → largest.
pub fn comparisons(cat: Category) -> &'static [Comparison] {
    match cat {
        Category::Volume => VOLUME_CMP,
        Category::Mass => MASS_CMP,
        Category::Length => LENGTH_CMP,
    }
}

/// Format a (possibly enormous) number kid-friendly: grouped thousands, a
/// couple of decimals when needed, scientific notation once it's absurd.
pub fn format_amount(x: f64) -> String {
    if !x.is_finite() {
        return "∞".to_string();
    }
    if x.abs() >= 1e15 {
        return format!("{:.3e}", x);
    }
    let neg = x < 0.0;
    let rounded = (x.abs() * 100.0).round() / 100.0;
    let int_part = rounded.trunc() as u128;
    let frac = ((rounded - rounded.trunc()) * 100.0).round() as u64;
    let grouped = group_thousands(int_part);
    let body = if frac > 0 { format!("{}.{:02}", grouped, frac) } else { grouped };
    if neg {
        format!("-{}", body)
    } else {
        body
    }
}

fn group_thousands(mut n: u128) -> String {
    if n == 0 {
        return "0".to_string();
    }
    let mut parts = Vec::new();
    while n > 0 {
        parts.push(format!("{:03}", n % 1000));
        n /= 1000;
    }
    parts.reverse();
    // Trim the leading zeros of the most-significant group.
    let mut s = parts.join(",");
    while s.starts_with('0') && s.len() > 1 && s.as_bytes()[1] != b',' {
        s.remove(0);
    }
    s
}
