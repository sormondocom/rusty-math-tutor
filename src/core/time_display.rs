//! Time display helpers — Roman numerals, time zones, word clock, calendar,
//! date arithmetic.  Pure Rust, uses web_time so it compiles to wasm32.

// ---------------------------------------------------------------------------
// Current time initialisation (web_time — safe on every target)
// ---------------------------------------------------------------------------

/// Read the current UTC date+time from the system clock.
/// Returns `(year, month, day, hour, minute, second)`.
pub fn now_components() -> (u16, u8, u8, u8, u8, u8) {
    let secs = web_time::SystemTime::now()
        .duration_since(web_time::SystemTime::UNIX_EPOCH)
        .unwrap_or(web_time::Duration::ZERO)
        .as_secs() as i64;
    let s  = (secs.rem_euclid(60)) as u8;
    let mi = ((secs / 60).rem_euclid(60)) as u8;
    let h  = ((secs / 3600).rem_euclid(24)) as u8;
    let days = secs / 86400;
    let (y, mo, d) = civil_from_days(days);
    (y as u16, mo as u8, d as u8, h, mi, s)
}

// ---------------------------------------------------------------------------
// Date arithmetic (Howard Hinnant's civil calendar algorithm)
// ---------------------------------------------------------------------------

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y   = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp  = (5 * doy + 2) / 153;
    let d   = doy - (153 * mp + 2) / 5 + 1;
    let m   = if mp < 10 { mp + 3 } else { mp - 9 };
    let y   = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Number of days in a given month (handles leap years).
pub fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11               => 30,
        2 => if is_leap(year) { 29 } else { 28 },
        _                            => 30,
    }
}

fn is_leap(y: u16) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// Day-of-week for the 1st of the month: 0 = Sunday … 6 = Saturday.
pub fn first_dow(year: u16, month: u8) -> u8 {
    let days = days_from_civil(year as i64, month as i64, 1);
    // Unix epoch (1970-01-01) was a Thursday = 4
    ((days + 4).rem_euclid(7)) as u8
}

/// Advance (or retreat) a date by `delta` days, returning the new (y, m, d).
pub fn date_shift(year: u16, month: u8, day: u8, delta: i32) -> (u16, u8, u8) {
    let epoch = days_from_civil(year as i64, month as i64, day as i64);
    let (y, m, d) = civil_from_days(epoch + delta as i64);
    (y as u16, m as u8, d as u8)
}

// ---------------------------------------------------------------------------
// Time zones  (DST-aware)
// ---------------------------------------------------------------------------

/// Which daylight-saving rule applies to a zone.
#[derive(Copy, Clone)]
pub enum DstKind {
    /// No DST — offset is fixed all year.
    None,
    /// US/Canada: spring forward 2nd Sunday in March, fall back 1st Sunday in November (+60 min).
    UsNorth,
    /// Europe: spring forward last Sunday in March, fall back last Sunday in October (+60 min).
    Eu,
}

pub struct Zone {
    pub city:        &'static str,
    /// Standard-time abbreviation.
    pub abbr:        &'static str,
    /// UTC offset in minutes during standard (winter) time.
    pub offset_mins: i32,
    pub dst_rule:    DstKind,
}

impl Zone {
    /// Effective UTC offset in minutes for the given calendar date, applying DST if active.
    pub fn offset_on(&self, year: u16, month: u8, day: u8) -> i32 {
        let dst = match self.dst_rule {
            DstKind::None    => false,
            DstKind::UsNorth => us_dst_active(year, month, day),
            DstKind::Eu      => eu_dst_active(year, month, day),
        };
        if dst { self.offset_mins + 60 } else { self.offset_mins }
    }
}

/// True when US/Canada DST is in effect: 2nd Sunday in March → 1st Sunday in November.
pub fn us_dst_active(year: u16, month: u8, day: u8) -> bool {
    if month > 3 && month < 11 { return true;  }
    if month < 3 || month > 11 { return false; }
    if month == 3 {
        return day >= nth_weekday(year, 3, 0, 2); // 2nd Sunday
    }
    day < nth_weekday(year, 11, 0, 1) // before 1st Sunday
}

/// True when EU DST is in effect: last Sunday in March → last Sunday in October.
pub fn eu_dst_active(year: u16, month: u8, day: u8) -> bool {
    if month > 3 && month < 10 { return true;  }
    if month < 3 || month > 10 { return false; }
    if month == 3 {
        return day >= last_sunday_of(year, 3);
    }
    day < last_sunday_of(year, 10)
}

/// Day-of-month of the nth occurrence of `weekday` (0=Sun) in the given month.
fn nth_weekday(year: u16, month: u8, weekday: u8, n: u8) -> u8 {
    let fw = first_dow(year, month);
    let delta = (weekday + 7 - fw) % 7;
    1 + delta + (n - 1) * 7
}

/// Day-of-month of the last occurrence of Sunday in the given month.
fn last_sunday_of(year: u16, month: u8) -> u8 {
    let days  = days_in_month(year, month);
    let last_dow = (first_dow(year, month) as u32 + days as u32 - 1).rem_euclid(7) as u8;
    days - last_dow  // subtract days since last Sunday
}

pub const ZONES: &[Zone] = &[
    Zone { city: "Los Angeles", abbr: "PST/PDT",  offset_mins: -480, dst_rule: DstKind::UsNorth },
    Zone { city: "Denver",      abbr: "MST/MDT",  offset_mins: -420, dst_rule: DstKind::UsNorth },
    Zone { city: "Chicago",     abbr: "CST/CDT",  offset_mins: -360, dst_rule: DstKind::UsNorth },
    Zone { city: "New York",    abbr: "EST/EDT",  offset_mins: -300, dst_rule: DstKind::UsNorth },
    Zone { city: "UTC / Zulu",  abbr: "Z",        offset_mins:    0, dst_rule: DstKind::None    },
    Zone { city: "London",      abbr: "GMT/BST",  offset_mins:    0, dst_rule: DstKind::Eu      },
    Zone { city: "Paris",       abbr: "CET/CEST", offset_mins:   60, dst_rule: DstKind::Eu      },
    Zone { city: "Dubai",       abbr: "GST",      offset_mins:  240, dst_rule: DstKind::None    },
    Zone { city: "Mumbai",      abbr: "IST",      offset_mins:  330, dst_rule: DstKind::None    },
    Zone { city: "Tokyo",       abbr: "JST",      offset_mins:  540, dst_rule: DstKind::None    },
    Zone { city: "Sydney",      abbr: "AEST",     offset_mins:  600, dst_rule: DstKind::None    },
    Zone { city: "Auckland",    abbr: "NZST",   offset_mins:  720, dst_rule: DstKind::None    },
];

/// Apply a UTC offset with full date tracking.
/// Returns `(year, month, day, hour, minute, day_delta)` where
/// `day_delta` is -1 / 0 / +1 indicating whether the date rolled back/forward.
pub fn apply_offset_dated(
    year: u16, month: u8, day: u8,
    hour: u8, min: u8,
    offset_mins: i32,
) -> (u16, u8, u8, u8, u8, i8) {
    let raw = hour as i32 * 60 + min as i32 + offset_mins;
    let delta: i8 = if raw < 0 { -1 } else if raw >= 24 * 60 { 1 } else { 0 };
    let clamped = raw.rem_euclid(24 * 60);
    let (lh, lm) = ((clamped / 60) as u8, (clamped % 60) as u8);
    let (ly, lmo, ld) = if delta != 0 {
        date_shift(year, month, day, delta as i32)
    } else {
        (year, month, day)
    };
    (ly, lmo, ld, lh, lm, delta)
}

// ---------------------------------------------------------------------------
// 12-hour conversion
// ---------------------------------------------------------------------------

pub fn to_12h(hour: u8) -> (u8, bool) {
    let am = hour < 12;
    let h  = if hour % 12 == 0 { 12 } else { hour % 12 };
    (h, am)
}

// ---------------------------------------------------------------------------
// Format helpers
// ---------------------------------------------------------------------------

pub fn fmt_12h(hour: u8, min: u8) -> String {
    let (h, am) = to_12h(hour);
    format!("{}:{:02} {}", h, min, if am { "AM" } else { "PM" })
}

pub fn fmt_24h(hour: u8, min: u8) -> String {
    format!("{:02}:{:02}", hour, min)
}

/// Format a time according to the app's hour-format config setting.
pub fn fmt_time(hour: u8, min: u8, use_24h: bool) -> String {
    if use_24h { fmt_24h(hour, min) } else { fmt_12h(hour, min) }
}

pub fn month_name(month: u8) -> &'static str {
    const MONTHS: &[&str] = &[
        "", "January","February","March","April","May","June",
        "July","August","September","October","November","December",
    ];
    MONTHS.get(month as usize).copied().unwrap_or("?")
}

pub fn month_abbr(month: u8) -> &'static str {
    const MONTHS: &[&str] = &[
        "", "Jan","Feb","Mar","Apr","May","Jun",
        "Jul","Aug","Sep","Oct","Nov","Dec",
    ];
    MONTHS.get(month as usize).copied().unwrap_or("?")
}

pub fn weekday_name(dow: u8) -> &'static str {
    const DAYS: &[&str] = &["Sunday","Monday","Tuesday","Wednesday","Thursday","Friday","Saturday"];
    DAYS.get(dow as usize).copied().unwrap_or("?")
}

// ---------------------------------------------------------------------------
// Roman numerals
// ---------------------------------------------------------------------------

pub fn to_roman(mut n: u32) -> String {
    const PAIRS: &[(u32, &str)] = &[
        (1000,"M"),(900,"CM"),(500,"D"),(400,"CD"),
        (100,"C"),(90,"XC"),(50,"L"),(40,"XL"),
        (10,"X"),(9,"IX"),(5,"V"),(4,"IV"),(1,"I"),
    ];
    if n == 0 { return "—".into(); }
    let mut s = String::new();
    for &(v, r) in PAIRS { while n >= v { s.push_str(r); n -= v; } }
    s
}

/// Format 24-hour time as Roman numerals (traditional clock style — 4 = IIII).
pub fn fmt_roman(hour: u8, min: u8) -> String {
    let (h, _) = to_12h(hour);
    let h_rom = if h == 4 { "IIII".into() } else { to_roman(h as u32) };
    if min == 0 { h_rom } else { format!("{}:{}", h_rom, to_roman(min as u32)) }
}

/// Clock-face labels, index 0 = 12 o'clock position, clockwise.
pub const CLOCK_LABELS_ROMAN: &[&str] = &[
    "XII","I","II","III","IIII","V","VI","VII","VIII","IX","X","XI",
];

/// Standard Arabic clock labels.
pub const CLOCK_LABELS_ARABIC: &[&str] = &[
    "12","1","2","3","4","5","6","7","8","9","10","11",
];

// ---------------------------------------------------------------------------
// Word clock
// ---------------------------------------------------------------------------

const HOUR_WORDS: &[&str] = &[
    "twelve","one","two","three","four","five","six",
    "seven","eight","nine","ten","eleven",
];

const MIN_ONES: &[&str] = &[
    "","one","two","three","four","five","six","seven","eight","nine",
    "ten","eleven","twelve","thirteen","fourteen","fifteen",
    "sixteen","seventeen","eighteen","nineteen",
];
const MIN_TENS: &[&str] = &["","","twenty","thirty","forty","fifty"];

fn min_in_words(n: u8) -> String {
    if n == 0 { return String::new(); }
    if n < 20 { return MIN_ONES[n as usize].to_string(); }
    let t = MIN_TENS[(n / 10) as usize];
    let o = n % 10;
    if o == 0 { t.to_string() } else { format!("{}-{}", t, MIN_ONES[o as usize]) }
}

/// "half past two", "quarter to five", "twelve o'clock" — British style.
pub fn fmt_british(hour: u8, min: u8) -> String {
    let hw  = HOUR_WORDS[(hour % 12) as usize];
    let nhw = HOUR_WORDS[((hour + 1) % 12) as usize];
    match min {
        0  => format!("{} o'clock", hw),
        1..=4 => format!("just past {}", hw),
        5  => format!("five past {}", hw),
        10 => format!("ten past {}", hw),
        15 => format!("quarter past {}", hw),
        20 => format!("twenty past {}", hw),
        25 => format!("twenty-five past {}", hw),
        30 => format!("half past {}", hw),
        35 => format!("twenty-five to {}", nhw),
        40 => format!("twenty to {}", nhw),
        45 => format!("quarter to {}", nhw),
        50 => format!("ten to {}", nhw),
        55 => format!("five to {}", nhw),
        56..=59 => format!("nearly {} o'clock", nhw),
        _ => {
            if min < 30 {
                format!("{} past {}", min_in_words(min), hw)
            } else {
                format!("{} to {}", min_in_words(60 - min), nhw)
            }
        }
    }
}

/// "three forty-five PM" — American colloquial.
pub fn fmt_american(hour: u8, min: u8) -> String {
    let (h, am) = to_12h(hour);
    let hw = HOUR_WORDS[(h % 12) as usize];
    let suffix = if am { "AM" } else { "PM" };
    if min == 0 {
        format!("{} {}", hw, suffix)
    } else {
        format!("{} {} {}", hw, min_in_words(min), suffix)
    }
}

/// Spoken military: "zero-eight-thirty hours", "fourteen-hundred hours".
pub fn fmt_military_spoken(hour: u8, min: u8) -> String {
    let hour_word = match hour {
        0  => "zero-zero".to_string(),
        1..=9  => format!("zero-{}", HOUR_WORDS[hour as usize]),
        10..=19 => format!("{}", MIN_ONES[hour as usize]),
        _  => {
            let t = MIN_TENS[(hour / 10) as usize];
            let o = hour % 10;
            if o == 0 { t.to_string() }
            else { format!("{}-{}", t, MIN_ONES[o as usize]) }
        }
    };
    if min == 0 {
        format!("{}-hundred hours", hour_word)
    } else {
        format!("{}-{} hours", hour_word, min_in_words(min))
    }
}

// ---------------------------------------------------------------------------
// Keyboard shortcut reference — shared by GUI, terminal, and WASM renderers.
// Each entry: (screen_title, &[(key, description)])
// ---------------------------------------------------------------------------

pub const HOTKEYS: &[(&str, &[(&str, &str)])] = &[
    ("Menu", &[
        ("↑ / ↓",     "Move the cursor"),
        ("Enter / →",  "Select item or toggle checkbox"),
        ("← ",         "Un-check a checkbox"),
        ("H",          "This help screen"),
    ]),
    ("Practice & Challenge", &[
        ("0 – 9",      "Type digits of your answer"),
        ("−",          "Toggle negative sign"),
        ("Backspace",  "Delete the last digit"),
        ("Enter",      "Submit your answer"),
        ("H",          "Show strategy hint (Deduction Duck)"),
        ("Y",          "Why am I learning this?"),
        ("Esc",        "Return to menu"),
    ]),
    ("Challenge Results", &[
        ("R / Enter",  "Play the challenge again"),
        ("M / Esc",    "Return to menu"),
    ]),
    ("Time Explorer", &[
        ("↑ / ↓",      "Change the selected field"),
        ("← / →",      "Switch between fields"),
        ("N",           "Jump to current time (live mode)"),
        ("T",           "Toggle 12-hour / 24-hour display"),
        ("H",           "Time reference facts (conversions & rules)"),
        ("Esc",         "Return to menu"),
    ]),
    ("Settings", &[
        ("↑ / ↓",      "Move between grade levels"),
        ("← / →",      "Adjust number range for that grade"),
        ("Esc",         "Return to menu"),
    ]),
    ("Experimentation", &[
        ("↑ / ↓",      "Switch the active field"),
        ("← / →",      "Change category or unit"),
        ("0 – 9 / .",   "Type an amount"),
        ("Backspace",   "Delete last digit"),
        ("Esc",         "Return to menu"),
    ]),
    ("Graph Explorer", &[
        ("↑ / ↓",      "Switch the selected field"),
        ("← / →",      "Change graph type or adjust value"),
        ("0 – 9",       "Type a value for the selected bar"),
        ("Backspace",   "Delete last digit"),
        ("Enter",       "Confirm current edit"),
        ("Esc",         "Return to menu"),
    ]),
    ("General", &[
        ("Esc",         "Always returns to the previous screen"),
        ("H",           "Context-sensitive help on most screens"),
    ]),
];

// ---------------------------------------------------------------------------
// Time reference facts (K-8 curriculum)
// ---------------------------------------------------------------------------

/// Each entry is (category_title, &[fact_lines]).
pub const TIME_FACTS: &[(&str, &[&str])] = &[
    ("Seconds, Minutes & Hours", &[
        "60 seconds  =  1 minute",
        "60 minutes  =  1 hour",
        "3,600 seconds  =  1 hour",
        "hours → minutes:  × 60",
        "minutes → seconds:  × 60",
        "hours → seconds:  × 3,600",
    ]),
    ("Hours & Days", &[
        "24 hours  =  1 day",
        "12 hours  =  half a day",
        "AM = before noon  (12:00 AM – 11:59 AM)",
        "PM = after noon   (12:00 PM – 11:59 PM)",
        "12:00 AM  =  midnight",
        "12:00 PM  =  noon",
    ]),
    ("Days, Weeks & Years", &[
        "7 days  =  1 week",
        "28–31 days  =  1 month",
        "4 weeks  ≈  1 month",
        "52 weeks  =  1 year",
        "12 months  =  1 year",
        "365 days  =  1 year  (366 in a leap year)",
    ]),
    ("Reading a Clock", &[
        "o'clock  =  exactly on the hour",
        "quarter past  =  15 minutes after",
        "half past  =  30 minutes after",
        "quarter to  =  15 minutes before",
        "Example:  quarter to 3  =  2:45",
        "Example:  half past 7  =  7:30",
    ]),
    ("Time Zones & UTC", &[
        "Earth is divided into 24 time zones",
        "Moving east  →  add hours",
        "Moving west  →  subtract hours",
        "UTC = the world's time reference",
        "Zulu (Z) = UTC — used by pilots & military",
        "DST shifts clocks ±1 hour (spring forward, fall back)",
    ]),
];

// ---------------------------------------------------------------------------
// Roman numerals — ASCII analog clock  (23 chars × 11 rows)
// ---------------------------------------------------------------------------

pub fn ascii_clock(hour: u8, min: u8) -> Vec<String> {
    const W: usize = 23;
    const H: usize = 11;
    let cx = 11.0_f32;
    let cy =  5.0_f32;
    let rx = 10.0_f32;
    let ry =  4.5_f32;

    let mut grid = vec![vec![' '; W]; H];

    for deg in 0..720u32 {
        let a = (deg as f32 * 0.5).to_radians();
        let x = (cx + rx * a.cos()).round() as i32;
        let y = (cy + ry * a.sin()).round() as i32;
        if x >= 0 && y >= 0 && (x as usize) < W && (y as usize) < H
            && grid[y as usize][x as usize] == ' '
        {
            grid[y as usize][x as usize] = '.';
        }
    }

    let lr = 7.5_f32; let lry = 3.1_f32;
    for (i, &label) in CLOCK_LABELS_ROMAN.iter().enumerate() {
        let a = (i as f32 * 30.0 - 90.0).to_radians();
        let lx = (cx + lr * a.cos()).round() as i32;
        let ly = (cy + lry * a.sin()).round() as i32;
        let off = -(label.len() as i32 / 2);
        for (j, ch) in label.chars().enumerate() {
            let xj = lx + off + j as i32;
            if xj >= 0 && ly >= 0 && (xj as usize) < W && (ly as usize) < H {
                grid[ly as usize][xj as usize] = ch;
            }
        }
    }

    grid[cy as usize][cx as usize] = '+';

    let ma = (min as f32 * 6.0 - 90.0).to_radians();
    draw_hand(&mut grid, cx, cy, ma, 8.5, 3.8, W, H, '*');

    let hf = (hour % 12) as f32 + min as f32 / 60.0;
    draw_hand(&mut grid, cx, cy, (hf * 30.0 - 90.0).to_radians(), 5.5, 2.3, W, H, 'H');

    grid.into_iter().map(|r| r.into_iter().collect()).collect()
}

fn draw_hand(grid: &mut Vec<Vec<char>>, cx: f32, cy: f32,
             angle: f32, rx: f32, ry: f32, w: usize, h: usize, ch: char) {
    for step in 1..=20u32 {
        let t = step as f32 / 20.0;
        let x = (cx + rx * t * angle.cos()).round() as i32;
        let y = (cy + ry * t * angle.sin()).round() as i32;
        if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h {
            grid[y as usize][x as usize] = ch;
        }
    }
}

// ---------------------------------------------------------------------------
// Calendar rendering (terminal)
