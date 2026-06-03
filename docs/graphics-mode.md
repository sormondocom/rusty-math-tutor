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

### Phase 0 — Decouple (the cross-platform foundation) — _in progress_

No new dependencies. Strangler-fig migration; the test suite stays green at every
step.

**Step 1 — Input seam ✅ done**
- [x] `InputEvent` / `Key` / `Mods` in [`src/input.rs`](../src/input.rs).
- [x] Terminal frontend maps `crossterm` → `InputEvent` in `main.rs::to_input`.
- [x] `App::on_event(InputEvent)`; `app` no longer imports `crossterm`.
      47 tests green, zero behavior change.

**Step 2 — Render seam ✅ done** _(reshaped after reading the code)_
- The code revealed the core *already* doesn't drive rendering (the frontend
  reads `&App`), and a shared low-level `Renderer` trait is the wrong tool —
  terminal cell-art and pixel-art genuinely diverge. The real coupling was the
  **transition machinery** (it lifts block-font `Cell`s off ratatui `Buffer`s).
- [x] Split it: the core keeps a ratatui-free **`TransitionPhase`**
      (`{ effect, progress }`) — timing + gating only — and the terminal
      frontend owns the visual `Transition` (captured buffers + particles).
- [x] Frontend `ui::Transitions` builds/syncs/drops the visuals each frame from
      the core's phase (`Transitions::sync`); `capture_*` moved out of the core.
- [x] Same treatment for the cinematic scene-to-scene transitions.
- [x] **`app` now has zero `ratatui` (and `crossterm`) imports.** 47 tests green,
      no behavior change, clean release build.

**Step 3 — Storage seam ✅ done**
- [x] `Storage` trait (`src/storage.rs`): `load(key)` / `save(key, data)`.
- [x] `Config`/`Roster`/`Extras` `load`/`save` take `&dyn Storage`; the
      filesystem impl (`FileStorage` + `data_path`) moved into the terminal
      frontend (`main.rs`). The app holds a `Box<dyn Storage>` and persists
      through it; tests use an in-memory `MemStorage` (no disk).
- [x] **`app`/`config`/`student`/`motivation` no longer call `std::fs`.**
      47 tests green, no behavior change.

**Step 4 — Relocate + finish decoupling ✅ done**
- [x] `core::Color` (`src/core/color.rs`): a neutral colour enum.
      `problem`/`fraction`/`cinematic` use it; the terminal frontend maps it to
      ratatui via `ui::rat()`. **The whole domain now imports zero ratatui.**
- [x] Moved modules into `src/core/` (15 domain modules) and
      `src/frontends/terminal/` (5 render modules); `main.rs` keeps flat crate
      paths via `#[path]`. 47 tests green, clean release build.

Exit criteria: identical behavior, full suite green, the core (`app` and every
domain module) has **no `crossterm`, `ratatui`, or `std::fs`**. **✅ met.**

## Phase 0: complete

All four seams are cut and the tree mirrors the split:

```
src/core/              <- domain: zero crossterm / ratatui / std::fs
src/frontends/terminal/ <- crossterm + ratatui cell renderer + transitions
src/main.rs            <- entry: terminal loop, InputEvent mapping, FileStorage
```

A second frontend (Phase 2 GUI, Phase 4 web) drops in beside
`frontends/terminal/`: it feeds `App::on_event`, reads `&App` to render, and
provides its own `Storage` — touching no core code.

### Phase 1 — Enhanced terminal ✅ done _(reshaped: no shared `Renderer`)_

A terminal-frontend-only fidelity bump — no core changes, no new deps, still one
`musl`-static binary.

- [x] `frontends/terminal/canvas.rs`: a reusable **half-block** (`▀▄█`) raster
      surface. Two sub-pixels per cell (top/bottom, coloured via fg/bg) → 2×
      vertical resolution and ~square pixels, so circles look round. Primitives:
      `set` / `line` (Bresenham) / `circle` (midpoint) / `blit`.
- [x] Smooth Geometry shapes: the circle is now a true round midpoint circle with
      a radius spoke; the triangle has clean Bresenham slopes (and a height line
      for area problems). Rectangle and 3-D box stay box-drawing (already crisp).
- Chose **half-blocks over braille** for universal terminal support (braille can
  render as tofu on minimal terminals — against the "runs everywhere" goal).
- The canvas is reusable for future sub-cell work (cinematic particles, etc.).

### Phase 2 — Native CPU window, 2D — _in progress_

A separate pixel frontend that reads the same `&App`, behind a `gui` Cargo
feature so the default terminal build stays lean and `musl`-static.

**Slice 1 — pipeline ✅ done**
- [x] `gui` feature → optional `winit` 0.30 + `softbuffer` 0.4 + `tiny-skia` 0.11
      (default build unaffected; 47 tests still green).
- [x] `src/frontends/gui/mod.rs`: opens a window, maps `winit` keys →
      `InputEvent`, drives `App::on_tick` on the 33 ms clock, paints a CPU
      `tiny-skia` frame and presents it via `softbuffer`. Compiles clean against
      the real APIs.
- [x] Launch with `cargo run --features gui -- --gui` (terminal stays the
      default). Persists via `App::persist` on exit.

**Slice 2 — render the screens — ✅ done**
- [x] Real **TTF text via `ab_glyph`**: a system font is loaded at runtime
      (Segoe UI / Arial / Verdana on Windows, DejaVu / Liberation on Linux,
      Arial on macOS) and glyphs are rasterised to alpha coverage and blended in.
      The public-domain **`font8x8`** bitmap is the fallback when no font is found
      (its ASCII-only `× ÷ − π ² ³` map to `x / - P 2 3`). A gui-gated test pins
      that text renders and that widths grow with length.
- [x] `src/frontends/gui/scene.rs`: the **Startup → Menu → Practice/Challenge**
      path is rendered for real — the window is navigable and playable. Reads the
      same `&App` the terminal does (menu rows, `Active` question, typed answer,
      feedback). The menu highlight bar is sized to the widest row so no label
      clips.
- [x] Geometry shapes as **tiny-skia paths** (circle, triangle, rect, wireframe
      box), scaled to real dimensions — pixels are square here, so no aspect
      fudge. Drawn in the Practice card.
- [x] **Anti-aliasing via supersampling** (the fix for tiny-skia 0.11.4's
      AA rasteriser panic, `hairline_aa.rs assertion failed`): the whole scene is
      rendered into a pixmap scaled up by `scene::SS` (3×) with `anti_alias` *off*,
      and the window step box-averages each `SS×SS` block back down. The averaging
      *is* the AA — smooth shapes and glyph edges, never touching the buggy path.
      Degenerate paths are still guarded.
- [x] **Every screen is now rendered** — no placeholders left:
      - **Settings / My Progress / Teacher Area / Experiment** full screens (the
        Teacher records table, the Why?-examples add-box, the unit explorer).
      - **Per-section figures** in the card: fraction/percent shaded shapes that
        materialise piece by piece (pie wedges, triangle strips, bar, grid),
        arithmetic horizontal / stacked / long-division layouts (so **V** works),
        and a themed **units prop** (cup, pool, bottle, scale, ruler, coin).
      - **Deduction Duck** (`H`): the slide-up hint panel for every section, the
        animated **number-line walk** and **smash** strategy visuals for
        arithmetic, the **Why?** overlay (`Y`), the unit duck-jump gag, and the
        explorer "Whoa/BOOM" reaction. The ASCII sprites render as monospaced
        pixels.
      - **Transitions**: all 7 reveals as per-pixel blends, plus pixel ports of
        the 6 particle effects (explode/swirl shatter the card into flying tiles;
        fireworks/starburst/alien-ships/asteroids as deterministic overlays on a
        crossfade backdrop), driven by the core's `TransitionPhase`.
      - **Milestone cinematics**: the Text, NameSky (comet stream + rising moon),
        and RocketName (rockets igniting each letter + blue comet) scenes, with
        Deduction Duck gazing up in `Awe`, and scene-to-scene transitions reusing
        the same blend engine.

**Slice 3 — ship it ✅ done**
- [x] `GraphicsMode::Cpu::available()` is true when built `--features gui`, and
      the Startup picker offers "CPU Graphics (window)".
- [x] `main.rs` launches the window when CPU is the saved preference (or `--gui`)
      — set it from the picker, it sticks. Terminal stays the default build.
- [x] Release workflow builds a **`...-windows-x86_64-gui.exe`** artifact
      (`--features gui`); README links it. 47 tests green; both builds clean.

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
- **Phase 0 step 1 (input seam): done** — `InputEvent`/`Key`/`Mods`; `app` is
  off `crossterm`; crossterm now lives only in the terminal frontend (main.rs).
- **Phase 0 step 2 (render seam): done** — the only core↔ratatui coupling was the
  transition machinery; split into a core `TransitionPhase` (timing) + a
  frontend-owned visual `Transition`. **`app` is now ratatui- and
  crossterm-free.** No shared low-level `Renderer` trait — terminal vs pixel art
  diverge, so each frontend renders `&App` its own way.
- **Revised plan:** a shared semantic `Renderer`/`Scene` is *not* pursued; the
  GUI frontend (Phase 2) will read `&App` and render pixels independently, the
  way `ui.rs` reads `&App` and renders cells.
- **Phase 0 step 3 (storage seam): done** — `Storage` trait; `config`/`student`/
  `motivation` off `std::fs`; `FileStorage` lives in the terminal frontend.
  `app` now has no `crossterm`, `ratatui`, or `std::fs` imports — **Phase 0 exit
  criterion met.** Remaining: relocate modules (step 4) + neutralize `Color`.
