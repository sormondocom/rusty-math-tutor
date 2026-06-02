//! A small, frontend-neutral colour type for the domain.
//!
//! Sections describe their accents and palettes with these values; the terminal
//! frontend maps them to `ratatui` colours when it paints (a GUI/web frontend
//! would map them to pixels instead).  This keeps the core from depending on any
//! renderer's colour type — the last ratatui type to leave the domain.

/// A colour, named (terminal-palette friendly) or true-colour RGB.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Color {
    LightCyan,
    LightGreen,
    LightYellow,
    LightMagenta,
    LightBlue,
    LightRed,
    Cyan,
    Green,
    Rgb(u8, u8, u8),
}
