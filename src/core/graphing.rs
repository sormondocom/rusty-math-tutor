//! Graphing topic — pictographs, bar charts, line graphs, pie charts, and
//! coordinate planes, with grade-appropriate K–8 problem generation.
//!
//! Two public entry points:
//!   [`generate`]     — single-graph reading / interpretation problem
//!   [`generate_cmp`] — two side-by-side graphs, comparison question

use rand::seq::SliceRandom;
use rand::Rng;

// ---------------------------------------------------------------------------
// Graph kind
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum GraphKind {
    Pictograph { scale: u32 },
    Bar        { scale: u32 },
    Line,
    Pie,
    Coordinate,
}

impl GraphKind {
    pub fn name(&self) -> &'static str {
        match self {
            GraphKind::Pictograph { .. } => "Pictograph",
            GraphKind::Bar        { .. } => "Bar Graph",
            GraphKind::Line              => "Line Graph",
            GraphKind::Pie               => "Pie Chart",
            GraphKind::Coordinate        => "Coordinate Plane",
        }
    }

    pub fn from_explorer_index(i: usize) -> GraphKind {
        match i {
            0 => GraphKind::Bar { scale: 1 },
            1 => GraphKind::Pictograph { scale: 1 },
            2 => GraphKind::Line,
            3 => GraphKind::Pie,
            _ => GraphKind::Coordinate,
        }
    }
}

// ---------------------------------------------------------------------------
// Graph data (shared by both problem types and the explorer)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct GraphData {
    pub title:      String,
    pub categories: Vec<String>,
    /// Values for Bar/Pictograph/Line: raw counts.
    /// Values for Pie: percentages (sum ≈ 100).
    /// Values for Coordinate: y-values at x = 1, 2, …, n.
    pub values:     Vec<i32>,
    pub x_label:    Option<String>,
    pub y_label:    Option<String>,
}

// ---------------------------------------------------------------------------
// Problem types
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum GraphQuestion {
    /// "How many [cat] are there?" → `values[i]`
    ReadValue(usize),
    /// "Which item has the most? (enter its number, 1-based)" → 1-based index
    FindMax,
    /// "Which item has the least? (enter its number, 1-based)"
    FindMin,
    /// "How many more [A] than [B]?" → `values[a] - values[b]` (a > b guaranteed)
    CompareTwo(usize, usize),
    /// "What is the total?" → sum of values
    Total,
    /// "What percent is [cat]?" (Pie only) → `values[i]` (already a pct)
    ReadPercent(usize),
    /// "What is y when x = [cat]?" (Coordinate) → `values[i]`
    ReadCoord(usize),
    /// "What are the coordinates of point N? (type x,y)" → `coord_answer`
    PlotPoint(usize),
}

#[derive(Clone, Debug)]
pub struct GraphProblem {
    pub kind:         GraphKind,
    pub data:         GraphData,
    pub question:     GraphQuestion,
    /// Integer answer for all question types except PlotPoint.
    pub answer:       i32,
    /// Set only for PlotPoint; the student types "x,y".
    pub coord_answer: Option<(i32, i32)>,
    pub hint:         Vec<String>,
}

#[derive(Clone, Debug)]
pub enum CmpQuestion {
    /// "Which group has the higher total? (1 or 2)" → 1 or 2
    WhichMore,
    /// "How many more does the higher group have?" → positive integer
    Difference,
    /// "By about what percent is the higher group ahead?" → whole number
    PercentMore,
}

#[derive(Clone, Debug)]
pub struct GraphCmpProblem {
    pub kind:     GraphKind,
    pub data_a:   GraphData,
    pub data_b:   GraphData,
    pub question: CmpQuestion,
    pub answer:   i32,
    pub hint:     Vec<String>,
}

// ---------------------------------------------------------------------------
// Theme pools (static category sets)
// ---------------------------------------------------------------------------

struct Theme {
    title:   &'static str,
    cats:    &'static [&'static str],
    y_label: &'static str,
}

const GENERAL_THEMES: &[Theme] = &[
    Theme { title: "Favorite Animals",        cats: &["Cats","Dogs","Birds","Fish","Rabbits"],        y_label: "Votes"    },
    Theme { title: "Fruit Picked",             cats: &["Apples","Bananas","Oranges","Grapes","Mangoes"], y_label: "Count"  },
    Theme { title: "Favorite Colors",          cats: &["Red","Blue","Green","Yellow","Purple"],        y_label: "Votes"    },
    Theme { title: "Favorite Sports",          cats: &["Soccer","Basketball","Swimming","Tennis"],      y_label: "Students" },
    Theme { title: "Favorite School Subjects", cats: &["Math","Science","English","Art","PE"],          y_label: "Students" },
    Theme { title: "Rainy Days per Season",    cats: &["Spring","Summer","Fall","Winter"],              y_label: "Days"     },
    Theme { title: "Books Read This Year",     cats: &["Jan","Feb","Mar","Apr","May"],                  y_label: "Books"    },
    Theme { title: "Points Scored",            cats: &["Game 1","Game 2","Game 3","Game 4"],            y_label: "Points"   },
];

const LINE_THEMES: &[Theme] = &[
    Theme { title: "Temperature This Week",  cats: &["Mon","Tue","Wed","Thu","Fri"],          y_label: "°F"    },
    Theme { title: "Weekly Quiz Scores",     cats: &["Wk 1","Wk 2","Wk 3","Wk 4","Wk 5"],  y_label: "Points" },
    Theme { title: "Plant Height",           cats: &["Day 1","Day 2","Day 3","Day 4","Day 5"], y_label: "cm"   },
    Theme { title: "Steps Walked",           cats: &["Mon","Tue","Wed","Thu","Fri"],          y_label: "Steps" },
];

// ---------------------------------------------------------------------------
// Generation helpers
// ---------------------------------------------------------------------------

fn pick_kind(grade: u8, rng: &mut impl Rng) -> GraphKind {
    match grade {
        // K-2: Pictograph, Bar, Pie (no Line or Coordinate yet)
        0..=2 => match rng.gen_range(0..3u32) {
            0 => GraphKind::Pictograph { scale: 1 },
            1 => GraphKind::Bar { scale: 1 },
            _ => GraphKind::Pie,
        },
        // Grade 3-4: same three types with higher scales
        3 | 4 => match rng.gen_range(0..3u32) {
            0 => GraphKind::Pictograph { scale: 2 },
            1 => GraphKind::Bar { scale: 5 },
            _ => GraphKind::Pie,
        },
        // Grade 5+: all five types; Line and Coordinate now included
        _ => match rng.gen_range(0..5u32) {
            0 => GraphKind::Pictograph { scale: 5 },
            1 => GraphKind::Bar { scale: 10 },
            2 => GraphKind::Line,
            3 => GraphKind::Pie,
            _ => GraphKind::Coordinate,
        },
    }
}

fn pick_kind_no_coord(grade: u8, rng: &mut impl Rng) -> GraphKind {
    // Coordinate is not supported in side-by-side view.
    // Pie always sums to 100; comparing totals would infinite-loop in the equality guard.
    loop {
        match pick_kind(grade, rng) {
            GraphKind::Coordinate | GraphKind::Pie => continue,
            k => return k,
        }
    }
}

fn n_cats(grade: u8, max_avail: usize, rng: &mut impl Rng) -> usize {
    let cap = match grade { 0 | 1 => 3, 2 | 3 => 4, _ => max_avail };
    let cap = cap.min(max_avail);
    rng.gen_range(2..=cap)
}

fn max_raw(grade: u8) -> i32 {
    match grade { 0 | 1 => 5, 2 => 10, 3 | 4 => 20, 5 | 6 => 40, _ => 80 }
}

fn generate_values_for_kind(kind: &GraphKind, n: usize, grade: u8, rng: &mut impl Rng) -> Vec<i32> {
    let mr = max_raw(grade);
    match kind {
        GraphKind::Pictograph { scale } | GraphKind::Bar { scale } => {
            let s = (*scale).max(1) as i32;
            (0..n).map(|_| rng.gen_range(1..=((mr / s).max(1))) * s).collect()
        }
        GraphKind::Line => {
            let base  = rng.gen_range(5..=(mr / 2).max(6));
            let step  = rng.gen_range(-3i32..=6);
            (0..n as i32).map(|i| (base + step * i + rng.gen_range(-2..=2)).max(1)).collect()
        }
        GraphKind::Pie => {
            // Percentages summing to 100, each a multiple of 5, each ≥ 5.
            let mut pcts: Vec<i32> = Vec::new();
            let mut rem = 100i32;
            for i in 0..n {
                if i == n - 1 {
                    pcts.push(rem);
                } else {
                    let min_remain = (n - i - 1) as i32 * 5;
                    let max_this   = (rem - min_remain).max(5);
                    let units      = rng.gen_range(1..=(max_this / 5).max(1));
                    let v          = (units * 5).clamp(5, rem - min_remain);
                    pcts.push(v);
                    rem -= v;
                }
            }
            pcts.shuffle(rng);
            pcts
        }
        GraphKind::Coordinate => {
            // y = m·x + b, x = 1 … n
            let m = rng.gen_range(1i32..=4);
            let b = rng.gen_range(0i32..=8);
            (1..=n as i32).map(|x| m * x + b).collect()
        }
    }
}

fn pick_question(
    kind: &GraphKind,
    _cats: &[String],
    values: &[i32],
    rng: &mut impl Rng,
) -> (GraphQuestion, i32, Option<(i32, i32)>) {
    let n = values.len();
    let max_idx = values.iter().enumerate().max_by_key(|(_, v)| *v).map(|(i, _)| i).unwrap_or(0);
    let min_idx = values.iter().enumerate().min_by_key(|(_, v)| *v).map(|(i, _)| i).unwrap_or(0);

    match kind {
        GraphKind::Pie => {
            let i = rng.gen_range(0..n);
            (GraphQuestion::ReadPercent(i), values[i], None)
        }
        GraphKind::Coordinate => {
            if rng.gen_bool(0.5) {
                let i = rng.gen_range(0..n);
                (GraphQuestion::ReadCoord(i), values[i], None)
            } else {
                let i   = rng.gen_range(0..n);
                let x   = (i as i32) + 1;
                let y   = values[i];
                (GraphQuestion::PlotPoint(i), 0, Some((x, y)))
            }
        }
        _ => {
            // For 2-cat graphs skip FindMin/FindMax (would be trivial)
            let q_types: &[u32] = if n > 2 { &[0, 1, 2, 3, 4] } else { &[0, 3, 4] };
            match q_types.choose(rng).copied().unwrap_or(0) {
                0 => {
                    let i = rng.gen_range(0..n);
                    (GraphQuestion::ReadValue(i), values[i], None)
                }
                1 => (GraphQuestion::FindMax, (max_idx + 1) as i32, None),
                2 => (GraphQuestion::FindMin, (min_idx + 1) as i32, None),
                3 => {
                    // Pick two distinct categories; ensure cats[a] >= cats[b]
                    let a = rng.gen_range(0..n);
                    let b = (a + 1 + rng.gen_range(0..n - 1)) % n;
                    let diff = (values[a] - values[b]).abs();
                    if values[a] >= values[b] {
                        (GraphQuestion::CompareTwo(a, b), diff, None)
                    } else {
                        (GraphQuestion::CompareTwo(b, a), diff, None)
                    }
                }
                _ => {
                    let total: i32 = values.iter().sum();
                    (GraphQuestion::Total, total, None)
                }
            }
        }
    }
}

fn build_hint(
    kind: &GraphKind,
    q: &GraphQuestion,
    cats: &[String],
    values: &[i32],
    coord: &Option<(i32, i32)>,
) -> Vec<String> {
    let mut h = Vec::new();
    h.push(format!("Look at the {} carefully.", kind.name()));
    match q {
        GraphQuestion::ReadValue(i) => {
            h.push(format!("Find the bar or symbol for \"{}\".", cats[*i]));
            h.push("Read its height or count to find the answer.".to_string());
        }
        GraphQuestion::FindMax => {
            h.push("Compare all bars or symbol groups.".to_string());
            h.push("Find the tallest or most and enter its position (1 = first, 2 = second …).".to_string());
        }
        GraphQuestion::FindMin => {
            h.push("Compare all bars or symbol groups.".to_string());
            h.push("Find the shortest or least and enter its position (1 = first, 2 = second …).".to_string());
        }
        GraphQuestion::CompareTwo(a, b) => {
            h.push(format!("Find {} (= {}) and {} (= {}).", cats[*a], values[*a], cats[*b], values[*b]));
            h.push("Subtract the smaller from the larger to find the difference.".to_string());
        }
        GraphQuestion::Total => {
            h.push("Add up every bar, symbol count, or data point.".to_string());
            let sum: i32 = values.iter().sum();
            h.push(format!("Sum of all values = {}", sum));
        }
        GraphQuestion::ReadPercent(i) => {
            h.push(format!("Find the slice for \"{}\" in the pie chart.", cats[*i]));
            h.push("Each value in this pie chart is already its percentage.".to_string());
        }
        GraphQuestion::ReadCoord(i) => {
            h.push(format!("Find x = {} on the horizontal axis.", cats[*i]));
            h.push("Read straight up to the plotted point, then left to the y-axis.".to_string());
        }
        GraphQuestion::PlotPoint(i) => {
            h.push(format!("Find point {} on the coordinate plane.", i + 1));
            h.push("Read its x-value (across) first, then y-value (up).".to_string());
            if let Some((x, y)) = coord {
                h.push(format!("Type the answer as:  x,y   e.g.  {},{}", x, y));
            }
        }
    }
    h
}

fn build_cmp_hint(q: &CmpQuestion, total_a: i32, total_b: i32) -> Vec<String> {
    let mut h = Vec::new();
    h.push("Compare both graphs carefully.".to_string());
    h.push(format!("Group 1 total = {}    Group 2 total = {}", total_a, total_b));
    match q {
        CmpQuestion::WhichMore => {
            h.push("Which total is larger?  Enter 1 for Group 1 or 2 for Group 2.".to_string());
        }
        CmpQuestion::Difference => {
            let diff = (total_a - total_b).abs();
            h.push(format!("Subtract the smaller from the larger:  |{} - {}| = {}", total_a, total_b, diff));
        }
        CmpQuestion::PercentMore => {
            let hi  = total_a.max(total_b);
            let lo  = total_a.min(total_b);
            let pct = if lo == 0 { 100 } else { ((hi - lo) as f32 / lo as f32 * 100.0).round() as i32 };
            h.push(format!("(Higher − Lower) ÷ Lower × 100  =  ({} − {}) ÷ {} × 100  ≈  {}%", hi, lo, lo, pct));
        }
    }
    h
}

// ---------------------------------------------------------------------------
// Public generation functions
// ---------------------------------------------------------------------------

/// Generate a single-graph problem appropriate for `grade` (0 = K, 1–8).
pub fn generate(grade: u8, rng: &mut impl Rng) -> GraphProblem {
    let kind = pick_kind(grade, rng);

    // Coordinate gets special treatment (categories = x-values)
    if matches!(kind, GraphKind::Coordinate) {
        let n = rng.gen_range(4..=6usize);
        let cats: Vec<String> = (1..=n as i32).map(|x| x.to_string()).collect();
        let values = generate_values_for_kind(&kind, n, grade, rng);
        let (question, answer, coord_answer) = pick_question(&kind, &cats, &values, rng);
        let hint = build_hint(&kind, &question, &cats, &values, &coord_answer);
        return GraphProblem {
            kind,
            data: GraphData {
                title: "Coordinate Plane".to_string(),
                categories: cats,
                values,
                x_label: Some("x".to_string()),
                y_label: Some("y".to_string()),
            },
            question,
            answer,
            coord_answer,
            hint,
        };
    }

    let pool = if matches!(kind, GraphKind::Line) { LINE_THEMES } else { GENERAL_THEMES };
    let theme = pool.choose(rng).unwrap();
    let n     = n_cats(grade, theme.cats.len(), rng);
    let cats: Vec<String> = theme.cats[..n].iter().map(|s| s.to_string()).collect();
    let values = generate_values_for_kind(&kind, n, grade, rng);
    let (question, answer, coord_answer) = pick_question(&kind, &cats, &values, rng);
    let hint = build_hint(&kind, &question, &cats, &values, &coord_answer);

    let (x_label, y_label) = match &kind {
        GraphKind::Line => (None, Some(theme.y_label.to_string())),
        _               => (None, Some(theme.y_label.to_string())),
    };

    GraphProblem {
        kind,
        data: GraphData {
            title: theme.title.to_string(),
            categories: cats,
            values,
            x_label,
            y_label,
        },
        question,
        answer,
        coord_answer,
        hint,
    }
}

/// Generate a two-graph comparison problem appropriate for `grade`.
pub fn generate_cmp(grade: u8, rng: &mut impl Rng) -> GraphCmpProblem {
    let kind = pick_kind_no_coord(grade, rng);

    let pool = if matches!(kind, GraphKind::Line) { LINE_THEMES } else { GENERAL_THEMES };
    let theme = pool.choose(rng).unwrap();
    let n     = n_cats(grade, theme.cats.len().min(4), rng); // ≤ 4 for side-by-side
    let cats: Vec<String> = theme.cats[..n].iter().map(|s| s.to_string()).collect();

    let vals_a = generate_values_for_kind(&kind, n, grade, rng);
    let total_a: i32 = vals_a.iter().sum();

    // Ensure Group 2 total differs from Group 1 so WhichMore has a clear answer.
    // Cap at 100 iterations to guard against any edge case where values collapse.
    let (vals_b, total_b) = {
        let mut tries = 0u32;
        loop {
            let v = generate_values_for_kind(&kind, n, grade, rng);
            let t: i32 = v.iter().sum();
            tries += 1;
            if t != total_a || tries >= 100 { break (v, t); }
        }
    };

    let question = pick_cmp_question(grade, rng);
    let answer = match &question {
        CmpQuestion::WhichMore   => if total_a > total_b { 1 } else { 2 },
        CmpQuestion::Difference  => (total_a - total_b).abs(),
        CmpQuestion::PercentMore => {
            let hi = total_a.max(total_b) as f32;
            let lo = total_a.min(total_b) as f32;
            if lo == 0.0 { 100 } else { ((hi - lo) / lo * 100.0).round() as i32 }
        }
    };

    let hint    = build_cmp_hint(&question, total_a, total_b);
    let y_label = Some(theme.y_label.to_string());

    GraphCmpProblem {
        kind,
        data_a: GraphData { title: "Group 1".to_string(), categories: cats.clone(), values: vals_a, x_label: None, y_label: y_label.clone() },
        data_b: GraphData { title: "Group 2".to_string(), categories: cats,          values: vals_b, x_label: None, y_label },
        question,
        answer,
        hint,
    }
}

fn pick_cmp_question(grade: u8, rng: &mut impl Rng) -> CmpQuestion {
    match grade {
        0..=3 => CmpQuestion::WhichMore,
        4..=6 => if rng.gen_bool(0.5) { CmpQuestion::WhichMore } else { CmpQuestion::Difference },
        _     => match rng.gen_range(0..3u32) { 0 => CmpQuestion::WhichMore, 1 => CmpQuestion::Difference, _ => CmpQuestion::PercentMore },
    }
}

// ---------------------------------------------------------------------------
// Impl blocks
// ---------------------------------------------------------------------------

impl GraphProblem {
    pub fn question_text(&self) -> String {
        let d  = &self.data;
        let yl = d.y_label.as_deref().unwrap_or("count");
        let n  = d.categories.len();
        match &self.question {
            GraphQuestion::ReadValue(i)     => format!("How many {} are shown?", d.categories[*i]),
            GraphQuestion::FindMax          => format!("Which item has the most {}? (enter 1–{})", yl, n),
            GraphQuestion::FindMin          => format!("Which item has the least {}? (enter 1–{})", yl, n),
            GraphQuestion::CompareTwo(a, b) => format!("How many more {} than {}?", d.categories[*a], d.categories[*b]),
            GraphQuestion::Total            => format!("What is the total {} across all items?", yl),
            GraphQuestion::ReadPercent(i)   => format!("What percent of the total is {}?", d.categories[*i]),
            GraphQuestion::ReadCoord(i)     => format!("What is the y-value when x = {}?", d.categories[*i]),
            GraphQuestion::PlotPoint(i)     => format!("What are the coordinates of point {}?  (type x,y)", i + 1),
        }
    }

    pub fn answer_label(&self) -> String {
        if let Some((x, y)) = self.coord_answer {
            format!("Answer: {},{}", x, y)
        } else {
            format!("Answer: {}", self.answer)
        }
    }

    pub fn is_correct(&self, input: &str) -> bool {
        let s = input.trim();
        if let Some((ex, ey)) = self.coord_answer {
            return parse_coord(s).is_some_and(|(x, y)| x == ex && y == ey);
        }
        s.parse::<i32>().ok() == Some(self.answer)
    }
}

impl GraphCmpProblem {
    pub fn question_text(&self) -> String {
        match &self.question {
            CmpQuestion::WhichMore   => "Which group has a higher total? (type 1 or 2)".to_string(),
            CmpQuestion::Difference  => "How many more does the higher group have?".to_string(),
            CmpQuestion::PercentMore => "By about what percent is the higher group ahead? (whole number)".to_string(),
        }
    }

    pub fn answer_label(&self) -> String {
        match &self.question {
            CmpQuestion::WhichMore   => format!("Answer: Group {}", self.answer),
            CmpQuestion::Difference  => format!("Answer: {} more", self.answer),
            CmpQuestion::PercentMore => format!("Answer: {}%", self.answer),
        }
    }

    pub fn is_correct(&self, input: &str) -> bool {
        input.trim().parse::<i32>().ok() == Some(self.answer)
    }
}

/// Parse `"x,y"` coordinate input.
pub fn parse_coord(input: &str) -> Option<(i32, i32)> {
    let mut parts = input.splitn(2, ',');
    let x = parts.next()?.trim().parse::<i32>().ok()?;
    let y = parts.next()?.trim().parse::<i32>().ok()?;
    Some((x, y))
}

// ---------------------------------------------------------------------------
// Graph Explorer helpers
// ---------------------------------------------------------------------------

/// Category labels for each explorer kind index (0=Bar,1=Pictograph,2=Line,3=Pie,4=Coord).
pub const EXPLORER_LABELS: &[&[&str]] = &[
    &["A", "B", "C", "D", "E"],           // Bar
    &["A", "B", "C", "D", "E"],           // Pictograph
    &["Mon", "Tue", "Wed", "Thu", "Fri"], // Line
    &["A", "B", "C", "D"],                // Pie
    &["1", "2", "3", "4", "5"],           // Coordinate (x-values)
];

/// Explorer kind names in index order.
pub const EXPLORER_KIND_NAMES: &[&str] = &["Bar Graph", "Pictograph", "Line Graph", "Pie Chart", "Coordinate Plane"];
