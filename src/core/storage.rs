//! Frontend-neutral persistence.
//!
//! The core saves and loads small JSON blobs *by key* (e.g. `"config.json"`)
//! through this trait, never touching the filesystem directly.  The terminal
//! frontend backs it with files under the platform config dir; a future web
//! frontend will back it with `localStorage`.  This is the third Phase 0 seam
//! in [`docs/graphics-mode.md`](../docs/graphics-mode.md).

/// A simple key→blob store.  Both methods are best-effort: a missing key reads
/// as `None`, and write errors are swallowed so persistence never interrupts a
/// lesson.
pub trait Storage {
    fn load(&self, key: &str) -> Option<String>;
    fn save(&self, key: &str, data: &str);
}

/// An in-memory store — used by tests and as a "no persistence" fallback.
/// (Test-only today; a future headless/web frontend can promote it.)
#[cfg(test)]
#[derive(Default)]
pub struct MemStorage {
    cells: std::cell::RefCell<std::collections::HashMap<String, String>>,
}

#[cfg(test)]
impl Storage for MemStorage {
    fn load(&self, key: &str) -> Option<String> {
        self.cells.borrow().get(key).cloned()
    }

    fn save(&self, key: &str, data: &str) {
        self.cells.borrow_mut().insert(key.to_string(), data.to_string());
    }
}
