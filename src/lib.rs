//! WASM library root — the entry point for the browser frontend.
// All core domain symbols are used by the binary target, not by lib itself.
// The lib crate exists purely as the WASM entry point; suppress dead-code noise.
#![allow(dead_code)]
//!
//! `wasm-pack build --target web` compiles this file as the library crate
//! root.  The native binary uses `main.rs` as its root instead; both roots
//! share the same `src/core/` domain files but each assembles its own module
//! tree using `#[path]` attributes.
//!
//! Why #[path]?  The files live in subdirectories for organisation, but we
//! want flat crate paths (`crate::app`, `crate::student` …) so every module
//! can cross-reference the others without long `super::super::` chains.  This
//! mirrors exactly what `main.rs` does — see the comment block there.
//!
//! Phase 0 ✓  toolchain verified (wasm-pack build + greet())
//! Phase 1 ✓  core domain compiles to WASM (this file)
//! Phase 2    WebStorage impl + App construction
//! Phase 3    canvas render loop + keyboard events

// ---------------------------------------------------------------------------
// Core domain modules — pure Rust, no OS or rendering dependencies.
// These are the same source files main.rs uses, just wired up from lib.rs.
// ---------------------------------------------------------------------------

#[path = "core/app.rs"]        mod app;
#[path = "core/cinematic.rs"]  mod cinematic;
#[path = "core/color.rs"]      mod color;
#[path = "core/config.rs"]     mod config;
#[path = "core/fraction.rs"]   mod fraction;
// WASM-safe shape enum — omits region_rect() which takes a ratatui::Rect.
// main.rs points crate::shapes at the terminal file (full drawing surface).
#[path = "core/shapes.rs"]     mod shapes;
#[path = "core/geometry.rs"]   mod geometry;
#[path = "core/graphing.rs"]   mod graphing;
#[path = "core/input.rs"]      mod input;
#[path = "core/motivation.rs"] mod motivation;
#[path = "core/problem.rs"]    mod problem;
#[path = "core/section.rs"]    mod section;
#[path = "core/storage.rs"]    mod storage;
#[path = "core/strategy.rs"]   mod strategy;
#[path = "core/student.rs"]    mod student;
#[path = "core/topic.rs"]      mod topic;
// WASM-safe transition types only — no ratatui rendering code.
// main.rs points crate::transition at frontends/terminal/transition.rs
// (same types + rendering).  lib.rs points here (types only).
#[path = "core/transition.rs"] mod transition;
#[path = "core/units.rs"]        mod units;
#[path = "core/time_display.rs"] mod time_display;

// ---------------------------------------------------------------------------
// GUI scene renderer (WASM only)
// ---------------------------------------------------------------------------
//
// The scene module uses tiny-skia, font8x8, and ab_glyph — all in the WASM
// deps.  It resolves `crate::app`, `crate::config`, etc. from the module
// tree declared above, exactly as the desktop GUI does.
//
// On WASM the TTF font loader (ab_glyph) calls std::fs::read(), which fails
// silently and falls back to the font8x8 bitmap glyphs — no code changes
// needed, the fallback path already exists for headless / CI builds.

#[cfg(target_arch = "wasm32")]
#[path = "frontends/gui/scene/mod.rs"]
mod scene;

// ---------------------------------------------------------------------------
// Web frontend (WASM only)
// ---------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
#[path = "frontends/web/mod.rs"]
mod web;

// Re-export WasmApp at the crate root so wasm-bindgen finds it and generates
// the JavaScript class without needing an explicit `pub use` everywhere.
#[cfg(target_arch = "wasm32")]
pub use web::WasmApp;

// ---------------------------------------------------------------------------
// WASM utility exports
// ---------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
mod wasm {
    use wasm_bindgen::prelude::*;

    /// Called automatically when the WASM module loads.
    /// Hooks Rust panics into the browser's DevTools console so they're
    /// readable rather than a silent abort.
    #[wasm_bindgen(start)]
    pub fn start() {
        console_error_panic_hook::set_once();
    }

    /// Phase 0 / Phase 2 smoke test: confirms the WASM module loaded.
    #[wasm_bindgen]
    pub fn greet() -> String {
        format!("Rusty Math Tutor v{} — WASM build ready.", env!("CARGO_PKG_VERSION"))
    }

    /// Returns the number of problem sections (8: Add, Sub, Mul, Div,
    /// Units, Fractions, Percentages, Geometry).
    #[wasm_bindgen]
    pub fn section_count() -> u32 {
        crate::topic::Topic::ALL.len() as u32
    }
}
