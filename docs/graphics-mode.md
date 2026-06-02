# 2D/3D Graphics Mode — Design & Roadmap

> Status: **planning**. This is a living document. The console (terminal) mode is
> the only frontend today; everything below is how we grow a CPU-rendered
> 2D/3D mode without breaking it.

## 1. Goal & constraints

- Add a **2D/3D graphics frontend** (`GraphicsMode::Cpu`, already reserved in
  [`src/config.rs`](../src/config.rs)) alongside the existing console mode.
- **CPU-only — never requires a GPU.** This is a hard rule. It rules out
  GPU/`wgpu` paths as the primary renderer.
- **Cross-platform now:** Windows + Chromebooks. **Nearly free bonus:** macOS,
  Linux desktop, and the browser.
- The **console mode stays lean** — small, `musl`-static, runs over SSH. Graphics
  is *additive* and feature-gated, never a tax on the terminal build.

## 2. Where we are today (the coupling to undo)

The domain logic is reasonably separable, but **rendering and input are bound to
the terminal**:

- [`src/main.rs`](../src/main.rs) runs a synchronous `crossterm` poll loop and
  calls `ui::draw(f, &app)` with a **ratatui `Frame`**.
- Every visual ([`ui.rs`](../src/ui.rs), [`font.rs`](../src/font.rs),
  [`duck.rs`](../src/duck.rs), [`shapes.rs`](../src/shapes.rs),
  [`transition.rs`](../src/transition.rs)) writes into a ratatui **cell
  `Buffer`**. All geometry is in **character cells** (`Rect`, `set_area`).
- Input is `crossterm::KeyEvent` → `app.on_key`.
- Transitions capture **cell buffers** and animate between them — an inherently
  cell-based concept.
- Persistence ([`config.rs`](../src/config.rs), [`student.rs`](../src/student.rs))
  uses `std::fs` via `config::data_path` — which does not exist on the web.

So "make it cross-platform capable" = **lift the core off ratatui/crossterm/fs**
so a pixel frontend can slot in. The rendering tech is the easy part; the
decoupling is the real work.

## 3. Direction (decided)

**Shared CPU rasterizer → native window first, then web.**

- Build a **software rasterizer once**: `tiny-skia` for 2D (pure-Rust,
  antialiased, no GPU) and a small **software 3D rasterizer** (`euc`, pure-Rust,
  or ~300 lines of our own: triangles + z-buffer + flat/Gouraud shading).
- Present that CPU pixel buffer with **`winit` + `softbuffer`**, which work on
  **Windows, macOS, Linux, *and* the browser canvas**.

The payoff: the native window (Phase 2–3) and the browser build (Phase 4) share
~all of the rendering code. One pixel frontend, two presentation backends.

Alternatives considered and parked:

- **Enhanced terminal** (half-blocks/braille/sixel): maximally portable, lowest
  effort, but blocky and not "real" 2D/3D. Kept as an *optional* Phase 1 win
  because it also validates the decoupling.
- **GPU/`wgpu`**: violates the CPU-only rule. Not pursued.

## 4. Target architecture

```
src/
  core/         <- no ratatui, no crossterm, no winit, no std::fs
    app, problem, geometry, fraction, units, percent,
    section, topic, student, motivation, cinematic (logic)
    render.rs   <- Renderer trait + primitives (LOGICAL units)
    input.rs    <- InputEvent enum
    storage.rs  <- Storage trait (save/load blobs)
  frontends/
    terminal/   <- crossterm+ratatui: InputEvent map, Renderer->cells,
                   fs storage, its own transitions
    gui/        <- Phase 2: winit+softbuffer, Renderer->pixels via rasterize/
    web/        <- Phase 4: winit(web)+softbuffer->canvas, localStorage
                   (reuses gui's Renderer)
  rasterize/    <- Phase 2+: shared CPU 2D (tiny-skia) + software 3D
  main.rs       <- selects a frontend from GraphicsMode / build flags
```

### The four seams to cut

These are **pure refactors — no new dependencies** — and they are what makes the
project cross-platform capable.

1. **Control inversion.** Replace the `while !quit` loop with a frontend-driven
   interface, so `winit`/WASM (which *own* the event loop) can drive the same
   core:

   ```rust
   impl App {
       fn on_event(&mut self, e: InputEvent);
       fn update(&mut self, dt: Duration);
       fn render(&self, r: &mut dyn Renderer);
   }
   ```

2. **`InputEvent`** — a neutral enum (key, char, resize; pointer/touch later).
   `crossterm` → it, `winit` → it, web → it. `app` stops importing `crossterm`.

3. **`Renderer` trait** (immediate-mode, logical coordinates) — the keystone:

   ```rust
   trait Renderer {
       fn viewport(&self) -> Rect;            // logical bounds
       fn text(&mut self, at: Vec2, s: &str, style: TextStyle);
       fn rect(&mut self, r: Rect, style: ShapeStyle);
       fn line(&mut self, a: Vec2, b: Vec2, style: Stroke);
       fn poly(&mut self, pts: &[Vec2], style: ShapeStyle);
       fn sprite(&mut self, at: Vec2, id: SpriteId);          // duck, props
       fn mesh3d(&mut self, m: &Mesh, xf: Mat4, mat: Material); // Phase 3; no-op on terminal
   }
   ```

   The terminal impl wraps today's `ui.rs`/`shapes.rs` (text→cells,
   poly→cell raster). The GUI/web impl maps to `tiny-skia` paths and routes
   `mesh3d` to the software 3D rasterizer.

4. **`Storage` trait.** `std::fs` does not exist on web. Abstract
   `save(key, bytes)` / `load(key)`: terminal = filesystem (today's
   `data_path`), web = `localStorage`. Cheap to cut now, painful later.

**Transitions** stop being core/cell-based. The core only signals "transition
A→B"; each frontend captures its own frame type (cells vs RGBA) and blends.
Terminal keeps `transition.rs` as-is; the pixel frontend gets nicer alpha blends
later.

### Logical coordinate model

Move UI math to **logical units** (not cells, not pixels). Each frontend maps
logical → its surface and owns DPI/resize:

- terminal: logical → character cells.
- gui/web: logical → physical pixels (with a scale factor).

This keeps one layout in the core and lets each surface present it natively.

## 5. Phased roadmap

### Phase 0 — Decouple (the cross-platform foundation) — _next_

No new dependencies. Strangler-fig migration; the test suite stays green at every
step.

- [ ] Add `Renderer`, `InputEvent`, `Storage` traits + logical primitives.
- [ ] Implement all three for the **terminal** by wrapping existing code —
      **zero visual change** (the render/never-panic tests guard this).
- [ ] Flip `main.rs` to `on_event` / `update` / `render`, terminal frontend
      driving it.
- [ ] Move `app` off `crossterm`/`ratatui`/`fs` types; relocate modules into
      `core/` vs `frontends/terminal/`.

Exit criteria: identical behavior, full suite green, `app` has no `crossterm`,
`ratatui`, or `std::fs` imports.

### Phase 1 — Enhanced terminal (optional quick win)

- [ ] Render the same `Renderer` calls with half-blocks/braille for ~2× vertical
      resolution. Validates the abstraction; still one `musl`-static binary; no
      window, no new deps.

### Phase 2 — Native CPU window, 2D

- [ ] `frontends/gui` with `winit` + `softbuffer`.
- [ ] `rasterize/` 2D via `tiny-skia`; CPU glyphs via `fontdue`/`ab_glyph`.
- [ ] Map every `Renderer` 2D primitive to pixels; port the cards, duck, menus.
- [ ] Ship `GraphicsMode::Cpu` for real on **Windows desktop** first.

### Phase 3 — Software 3D

- [ ] CPU 3D rasterizer (`euc` or our own) behind `Renderer::mesh3d`.
- [ ] Geometry solids (box/cube, and future cylinder/sphere) as real meshes.
- [ ] Upgrade the milestone cinematics (rockets, comet, moon) to true 3D.

### Phase 4 — Web / WASM

- [ ] Same `winit` frontend → `<canvas>` via `softbuffer`; `Storage` →
      `localStorage`; `rand` with the `getrandom` `js` feature.
- [ ] Build with `trunk`/`wasm-pack`; runs on **any Chromebook via Chrome** (no
      Linux container — covers locked-down school devices) and every desktop
      browser.

## 6. Dependencies & cross-platform notes

| Crate | Role | Native | WASM | musl-static |
|---|---|---|---|---|
| `winit` | window + input | ✓ all OSes | ✓ canvas | ✗ (Linux needs X11/Wayland) |
| `softbuffer` | present CPU buffer | ✓ | ✓ | ✗ |
| `tiny-skia` | 2D raster | ✓ | ✓ | ✓ |
| `fontdue` / `ab_glyph` | CPU glyphs | ✓ | ✓ | ✓ |
| `euc` (or our own) | software 3D | ✓ | ✓ | ✓ |
| `rand` / `getrandom` | already used | ✓ | ✓ (needs `js` feature) | ✓ |

Everything is **feature-gated**:

- `default = ["terminal"]` — tiny, `musl`-static, unchanged.
- `gui` — `winit`, `softbuffer`, `tiny-skia`, font + 3D crates.
- `web` — the `gui` renderer compiled to `wasm32-unknown-unknown`.

**Blunt consequence:** the GUI **Linux** binary cannot be `musl`-static (X11/
Wayland), so for **Chromebooks the web build is the real answer** — it runs in
Chrome with no Linux/Crostini container.

## 7. Distribution / release impact

The release workflow ([`.github/workflows/release.yml`](../.github/workflows/release.yml))
grows as the frontends land:

- **Terminal** artifacts (Windows `.exe`, Chromebook `musl` x86_64/aarch64):
  unchanged.
- **GUI**: add a Windows GUI `.exe` (glibc-dynamic Linux GUI is possible but
  secondary).
- **Web**: build the WASM bundle and **publish to GitHub Pages from the same
  Actions run**, so Chromebooks just open a URL.

## 8. Open questions

- Exact `Renderer` primitive set — is immediate-mode enough, or do we want a
  small retained **Scene** (Vec of primitives) for easier testing and transition
  blending? (Leaning immediate-mode to match current code.)
- Logical coordinate space units and the layout strategy (fixed virtual canvas
  vs. responsive).
- Sprite model: keep the duck as ASCII art rasterized into pixels, or author a
  proper pixel/vector sprite for the GUI?
- 3D math: pull in a small linear-algebra crate (`glam`) or keep a minimal
  in-house `Vec3`/`Mat4`?
- Web persistence & first-run UX (no installer, no file picker).

---

_Decision log_

- **Direction:** shared CPU rasterizer, **native window first, then web**
  (chosen over web-first and terminal-enhanced-first).
- **Renderer style:** immediate-mode `Renderer` trait (tentative).
- **Transitions:** owned per-frontend, not core.
