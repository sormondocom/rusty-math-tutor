//! Deduction Duck's solving strategies.
//!
//! Each [`Strategy`] is a method a teacher would actually demonstrate, split
//! into a **hint** ([`Strategy::steps`], shown first) and the worked **answer**
//! ([`Strategy::reveal`], shown only when the student asks).  A [`Viz`] tells
//! the UI how the duck should *act it out* — walking a number line, smashing a
//! number into place-value pieces, or just talking it through.
//!
//! The methods mirror the standard K-8 progressions (count-on / count-up
//! number lines, make-a-ten, partial sums & products / break-apart by place
//! value, expanded-form regrouping, skip counting, doubling, repeated
//! subtraction, and "think multiplication").

use crate::problem::{Op, Problem};

/// How the duck visualises a strategy.  The UI may fall back to [`Viz::Lines`]
/// when a richer view will not fit the available space.
pub enum Viz {
    /// Plain talk-through; duck stands and explains.
    Lines,
    /// Duck walks a number line, hopping between `stops`.  `hops[i]` labels the
    /// jump from `stops[i]` to `stops[i + 1]` (e.g. `"+1"`, `"-4"`).
    NumberLine { stops: Vec<i64>, hops: Vec<String> },
    /// Duck smashes `value` apart into `parts` (which sum back to `value`).
    Smash { value: i64, parts: Vec<i64> },
}

pub struct Strategy {
    pub title: String,
    /// The method, shown as the first hint — never states the final answer.
    pub steps: Vec<String>,
    /// The worked answer, revealed only on request.
    pub reveal: Vec<String>,
    pub viz: Viz,
}

impl Strategy {
    fn lines(title: &str, steps: Vec<String>, reveal: Vec<String>) -> Self {
        Strategy { title: title.to_string(), steps, reveal, viz: Viz::Lines }
    }
}

/// Build the list of strategies for `p`.  Always returns at least one.
pub fn strategies(p: &Problem) -> Vec<Strategy> {
    let mut v = match p.op {
        Op::Add => add(p),
        Op::Sub => sub(p),
        Op::Mul => mul(p),
        Op::Div => div(p),
    };
    if v.is_empty() {
        v.push(Strategy::lines(
            "Work It Out",
            vec![format!("Solve {} {} {}.", p.a, p.op.symbol(), p.b)],
            vec![format!("{} {} {} = {}", p.a, p.op.symbol(), p.b, p.answer)],
        ));
    }
    v
}

// ---------------------------------------------------------------------------
// Addition
// ---------------------------------------------------------------------------

fn add(p: &Problem) -> Vec<Strategy> {
    let (a, b, ans) = (p.a, p.b, p.answer);
    let mut v = Vec::new();

    // Number line — a +1 "count on" for small b, friendly ten-jumps otherwise.
    if let Some((stops, hops, title, hint)) = add_number_line(a, b) {
        v.push(Strategy {
            title,
            steps: vec![hint],
            reveal: vec![format!("You land on {}.", ans), format!("{} + {} = {}", a, b, ans)],
            viz: Viz::NumberLine { stops, hops },
        });
    } else if b >= 1 {
        // Numbers too big for a tidy line — describe the single jump.
        v.push(Strategy::lines(
            "Jump on a Number Line",
            vec![format!("Start at {} and add {}.", a, b)],
            vec![format!("{}  --+{}-->  {}", a, b, ans)],
        ));
    }

    // Make a Ten.
    if let Some(s) = make_ten(a, b, ans) {
        v.push(s);
    }

    // Break apart by place value — smash the bigger number.
    if a >= 10 || b >= 10 {
        let big = a.max(b);
        let parts = place_parts(big);
        v.push(Strategy {
            title: "Break Apart (Place Value)".to_string(),
            steps: vec![
                "Smash each number into tens and ones.".to_string(),
                format!("{} -> {}", big, join_plus(&parts)),
                "Add the tens, then add the ones.".to_string(),
            ],
            reveal: place_value_reveal(a, b, ans),
            viz: Viz::Smash { value: big, parts },
        });
    }

    v
}

/// A built number-line walk: `(stops, hop labels, title, hint line)`.
type NumberLine = (Vec<i64>, Vec<String>, String, String);

/// Number line for `a + b`: single `+1` hops for small `b`, friendly ten-jumps
/// otherwise.  `None` when `b` is too large for a tidy line.
fn add_number_line(a: i64, b: i64) -> Option<NumberLine> {
    if b <= 0 {
        return None;
    }
    if b <= 8 {
        let (stops, hops) = unit_walk(a, b, true);
        return Some((stops, hops, "Count On (Number Line)".to_string(), format!("Start at {} and hop up {} ones.", a, b)));
    }
    let jumps = friendly_steps((10 - a % 10) % 10, b)?;
    let (stops, hops) = walk(a, &jumps, true);
    Some((stops, hops, "Jump on a Number Line".to_string(), format!("Start at {} and jump in friendly steps.", a)))
}

/// Number line counting **up** from `from` to `to` (the difference / "how far
/// apart").  `None` when the gap is too large for a tidy line.
fn count_up_line(from: i64, to: i64) -> Option<NumberLine> {
    let amount = to - from;
    if amount <= 0 {
        return None;
    }
    let hint = format!("Start at {} and count up to {}.", from, to);
    if amount <= 8 {
        let (stops, hops) = unit_walk(from, amount, true);
        return Some((stops, hops, "Count Up (Number Line)".to_string(), hint));
    }
    let jumps = friendly_steps((10 - from % 10) % 10, amount)?;
    let (stops, hops) = walk(from, &jumps, true);
    Some((stops, hops, "Count Up (Number Line)".to_string(), hint))
}

/// Number line counting **back** from `from` by `amount` (take-away).  `None`
/// when `amount` is too large for a tidy line.
fn count_back_line(from: i64, amount: i64) -> Option<NumberLine> {
    if amount <= 0 {
        return None;
    }
    if amount <= 8 {
        let (stops, hops) = unit_walk(from, amount, false);
        return Some((stops, hops, "Count Back (Number Line)".to_string(), format!("Start at {} and hop back {} ones.", from, amount)));
    }
    let jumps = friendly_steps(from % 10, amount)?;
    let (stops, hops) = walk(from, &jumps, false);
    Some((stops, hops, "Count Back (Number Line)".to_string(), format!("Start at {} and jump back in friendly steps.", from)))
}

/// Decompose `amount` into friendly jumps: a partial step to the nearest ten
/// (`first_partial`), then whole tens, then the leftover ones.  `None` if it
/// would take more than seven jumps.
fn friendly_steps(first_partial: i64, amount: i64) -> Option<Vec<i64>> {
    let mut jumps = Vec::new();
    let mut rem = amount;
    if first_partial > 0 && first_partial < amount {
        jumps.push(first_partial);
        rem -= first_partial;
    }
    jumps.extend(std::iter::repeat_n(10, (rem / 10) as usize));
    if rem % 10 > 0 {
        jumps.push(rem % 10);
    }
    if jumps.is_empty() || jumps.len() > 7 {
        None
    } else {
        Some(jumps)
    }
}

/// Stops and `±1` hop labels for a single-unit walk of length `n`.
fn unit_walk(start: i64, n: i64, up: bool) -> (Vec<i64>, Vec<String>) {
    let stops = (0..=n).map(|i| if up { start + i } else { start - i }).collect();
    let hops = (0..n).map(|_| if up { "+1".to_string() } else { "-1".to_string() }).collect();
    (stops, hops)
}

/// Stops and signed hop labels for a walk over the given `jumps`.
fn walk(start: i64, jumps: &[i64], up: bool) -> (Vec<i64>, Vec<String>) {
    let mut stops = vec![start];
    let mut hops = Vec::new();
    let mut cur = start;
    for j in jumps {
        cur = if up { cur + j } else { cur - j };
        stops.push(cur);
        hops.push(format!("{}{}", if up { "+" } else { "-" }, j));
    }
    (stops, hops)
}

fn make_ten(a: i64, b: i64, ans: i64) -> Option<Strategy> {
    let (big, small) = if a >= b { (a, b) } else { (b, a) };
    if big >= 100 || small == 0 {
        return None;
    }
    let need = (10 - big % 10) % 10;
    if need == 0 || need > small {
        return None;
    }
    let ten = big + need;
    let rest = small - need;
    Some(Strategy::lines(
        "Make a Ten",
        vec![
            format!("{} + {}", a, b),
            format!("Move {} over: {} + {} = {}", need, big, need, ten),
            format!("Now it's the easy sum {} + {}.", ten, rest),
        ],
        vec![format!("{} + {} = {}", ten, rest, ans)],
    ))
}

fn place_value_reveal(a: i64, b: i64, ans: i64) -> Vec<String> {
    let mut lines = Vec::new();
    let mut partials = Vec::new();
    for pv in [1000, 100, 10, 1] {
        let av = place(a, pv);
        let bv = place(b, pv);
        if av + bv > 0 {
            lines.push(format!("{} + {} = {}", av, bv, av + bv));
            partials.push(av + bv);
        }
    }
    lines.push(format!("{} = {}", join_plus(&partials), ans));
    lines
}

// ---------------------------------------------------------------------------
// Subtraction
// ---------------------------------------------------------------------------

fn sub(p: &Problem) -> Vec<Strategy> {
    let (a, b, ans) = (p.a, p.b, p.answer);
    let mut v = Vec::new();

    if a < b {
        v.push(Strategy::lines(
            "Cross Below Zero",
            vec![format!("{} is smaller than {}.", a, b), "Count back past 0.".to_string()],
            vec![format!("{} - {} = {}", a, b, ans)],
        ));
        return v;
    }

    // Count Up — find the difference by walking from b up to a.
    if let Some((stops, hops, title, hint)) = count_up_line(b, a) {
        v.push(Strategy {
            title,
            steps: vec![hint],
            reveal: vec![format!("The hops add up to {}.", ans), format!("{} - {} = {}", a, b, ans)],
            viz: Viz::NumberLine { stops, hops },
        });
    }
    // Count Back — take away by walking from a down to the answer.
    if let Some((stops, hops, title, hint)) = count_back_line(a, b) {
        v.push(Strategy {
            title,
            steps: vec![hint],
            reveal: vec![format!("{} - {} = {}", a, b, ans)],
            viz: Viz::NumberLine { stops, hops },
        });
    }
    // Neither line fit (very large numbers) — a plain take-away.
    if v.is_empty() {
        v.push(Strategy::lines(
            "Count Back",
            vec![format!("Start at {}, take away {}.", a, b)],
            vec![format!("{} - {} = {}", a, b, ans)],
        ));
    }

    // Expanded-form regrouping — smash a ten open to borrow.
    if a < 100 && b < 100 && a % 10 < b % 10 {
        let a_tens = (a / 10) * 10 - 10;
        let a_ones = a % 10 + 10;
        let b_tens = (b / 10) * 10;
        let b_ones = b % 10;
        v.push(Strategy {
            title: "Regroup (Borrowing)".to_string(),
            steps: vec![
                format!("{} ones isn't enough to take {}.", a % 10, b_ones),
                format!("Smash a ten open: {} -> {} + {}", a, a_tens, a_ones),
                "Now subtract each place.".to_string(),
            ],
            reveal: vec![
                format!("Tens: {} - {} = {}", a_tens, b_tens, a_tens - b_tens),
                format!("Ones: {} - {} = {}", a_ones, b_ones, a_ones - b_ones),
                format!("{} + {} = {}", a_tens - b_tens, a_ones - b_ones, ans),
            ],
            viz: Viz::Smash { value: a, parts: vec![a_tens, a_ones] },
        });
    }

    v
}

// ---------------------------------------------------------------------------
// Multiplication
// ---------------------------------------------------------------------------

fn mul(p: &Problem) -> Vec<Strategy> {
    let (a, b, ans) = (p.a, p.b, p.answer);
    let mut v = Vec::new();

    // Repeated addition.
    if let Some((groups, each)) = if (1..=6).contains(&b) {
        Some((b, a))
    } else if (1..=6).contains(&a) {
        Some((a, b))
    } else {
        None
    } {
        let terms = vec![each.to_string(); groups as usize].join(" + ");
        v.push(Strategy::lines(
            "Repeated Addition",
            vec![format!("{} x {} means {} groups of {}.", a, b, groups, each), format!("Add: {}", terms)],
            vec![format!("{} = {}", terms, ans)],
        ));
    }

    // Skip counting — hop along a number line by `a`.
    if (2..=8).contains(&b) && ans <= 200 {
        let stops: Vec<i64> = (0..=b).map(|i| a * i).collect();
        let hops: Vec<String> = (0..b).map(|_| format!("+{}", a)).collect();
        v.push(Strategy {
            title: "Skip Count".to_string(),
            steps: vec![format!("Hop along by {}, {} times.", a, b)],
            reveal: vec![format!("You land on {}.", ans)],
            viz: Viz::NumberLine { stops, hops },
        });
    }

    // Break apart (partial products) — smash the bigger factor.
    if a >= 10 || b >= 10 {
        let (m, k) = if a >= b { (a, b) } else { (b, a) };
        let parts = place_parts(m);
        let mut reveal = Vec::new();
        let mut partials = Vec::new();
        for &pv in &parts {
            reveal.push(format!("{} x {} = {}", k, pv, k * pv));
            partials.push(k * pv);
        }
        reveal.push(format!("{} = {}", join_plus(&partials), ans));
        v.push(Strategy {
            title: "Break Apart (Partial Products)".to_string(),
            steps: vec![
                format!("Smash {} into place parts.", m),
                format!("{} -> {}", m, join_plus(&parts)),
                format!("Multiply each part by {}.", k),
            ],
            reveal,
            viz: Viz::Smash { value: m, parts },
        });
    }

    // Doubling.
    if (2..=20).contains(&a) && a % 2 == 0 {
        let half = a / 2;
        v.push(Strategy::lines(
            "Use Doubling",
            vec![format!("{} x {} = double of {} x {}.", a, b, half, b), format!("{} x {} = {}", half, b, half * b)],
            vec![format!("Double {} = {}", half * b, ans)],
        ));
    }

    v
}

// ---------------------------------------------------------------------------
// Division
// ---------------------------------------------------------------------------

fn div(p: &Problem) -> Vec<Strategy> {
    let (a, b, ans) = (p.a, p.b, p.answer);
    let mut v = Vec::new();

    v.push(Strategy::lines(
        "Equal Groups",
        vec![format!("Share {} into groups of {}.", a, b), "How many groups can you make?".to_string()],
        vec![format!("{} groups: {} / {} = {}", ans, a, b, ans)],
    ));

    v.push(Strategy::lines(
        "Think Multiplication",
        vec![format!("{} / {}  ->  {} x ? = {}", a, b, b, a), format!("What times {} makes {}?", b, a)],
        vec![format!("{} x {} = {}, so the answer is {}", b, ans, a, ans)],
    ));

    // Repeated subtraction — hop backward by b until reaching 0.
    if (1..=8).contains(&ans) {
        let stops: Vec<i64> = (0..=ans).map(|i| a - i * b).collect();
        let hops: Vec<String> = (0..ans).map(|_| format!("-{}", b)).collect();
        v.push(Strategy {
            title: "Repeated Subtraction".to_string(),
            steps: vec![format!("Start at {} and take away {} until you hit 0.", a, b)],
            reveal: vec![format!("That took {} hops -> {}", ans, ans)],
            viz: Viz::NumberLine { stops, hops },
        });
    }

    v
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// The value contributed by `n`'s digit at place value `pv` (1, 10, 100, …).
fn place(n: i64, pv: i64) -> i64 {
    (n / pv) % 10 * pv
}

/// Non-zero place-value pieces of `n`, high to low (e.g. 405 -> [400, 5]).
fn place_parts(n: i64) -> Vec<i64> {
    let mut parts: Vec<i64> = [1000, 100, 10, 1].iter().map(|&pv| place(n, pv)).filter(|&v| v > 0).collect();
    if parts.is_empty() {
        parts.push(0);
    }
    parts
}

fn join_plus(parts: &[i64]) -> String {
    parts.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(" + ")
}
