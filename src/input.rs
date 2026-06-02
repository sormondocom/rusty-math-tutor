//! Frontend-neutral input events.
//!
//! The core ([`crate::app`]) reacts to these, never to a specific backend's key
//! types.  The terminal frontend maps `crossterm` events into them; future GUI
//! and web frontends will map `winit` / DOM events the same way.  This is the
//! first of the Phase 0 seams in [`docs/graphics-mode.md`](../docs/graphics-mode.md).

/// A single key press, independent of any backend.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Key {
    /// A printable character (already case-folded by the backend, e.g. Shift+a
    /// arrives as `'A'`).
    Char(char),
    Enter,
    Esc,
    Backspace,
    Tab,
    Up,
    Down,
    Left,
    Right,
}

/// Modifier keys held during an event.  Only Ctrl is consulted today; more can
/// be added without touching call sites.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub struct Mods {
    pub ctrl: bool,
}

/// An input event delivered to [`crate::app::App::on_event`].
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum InputEvent {
    Key { key: Key, mods: Mods },
}

impl InputEvent {
    /// A bare key press with no modifiers — handy in tests and simple frontends.
    /// (Frontends that carry modifiers build the [`InputEvent::Key`] variant
    /// directly; today only the tests use this shorthand.)
    #[cfg(test)]
    pub fn key(key: Key) -> InputEvent {
        InputEvent::Key { key, mods: Mods::default() }
    }
}
