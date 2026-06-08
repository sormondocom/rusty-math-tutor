//! `localStorage`-backed [`Storage`] implementation for the WASM frontend.
//!
//! `web_sys::Storage` is the DOM Storage interface (what you get from
//! `window.localStorage`).  We give it an alias to avoid a naming clash with
//! our own `crate::storage::Storage` trait in the same file.

type DomStorage = web_sys::Storage;

/// Persists the app's JSON blobs in the browser's `localStorage`.
///
/// Keys are used as-is (`"config.json"`, `"students.json"`, etc.) — they're
/// already short and unique.  `localStorage` is scoped per origin, so there
/// are no collisions with other sites.
///
/// Both methods are best-effort.  A blocked or full store is silently ignored
/// so persistence problems never interrupt a lesson.
pub struct WebStorage {
    inner: DomStorage,
}

impl WebStorage {
    /// Returns `Some` if `window.localStorage` is accessible (requires a
    /// secure context or non-private browsing in most browsers).
    pub fn new() -> Option<Self> {
        web_sys::window()?
            .local_storage()
            .ok()        // JsValue error → None
            .flatten()   // Option<Option<_>> → Option<_>
            .map(|s| WebStorage { inner: s })
    }
}

// Implement our trait.  Full path avoids any ambiguity with web_sys::Storage.
impl crate::storage::Storage for WebStorage {
    fn load(&self, key: &str) -> Option<String> {
        // get_item returns Ok(None) for a missing key, Ok(Some(_)) for a hit.
        self.inner.get_item(key).ok().flatten()
    }

    fn save(&self, key: &str, data: &str) {
        // Errors (quota exceeded, security exception) are silently swallowed.
        let _ = self.inner.set_item(key, data);
    }
}
