<div align="center">

# Rusty Math Tutor

<table border="0" cellpadding="16"><tr>
<td align="center" valign="middle">
<img src="src/assets/duck_with_tie_and_hat.png" alt="Deduction Duck" height="220" />
</td>
<td align="left" valign="middle">
<pre>
     ___        .---------------------------.
   [_____]     (   Quack! Let's do math!    )
      |          '---------------------------'
     __
   &lt;(o )__
    (  __)       RUSTY MATH TUTOR
     ^^ ^^       — Deduction Duck, at your service
</pre>
</td>
</tr></table>

[![Latest release](https://img.shields.io/github/v/release/sormondocom/rusty-math-tutor?label=download&sort=semver)](https://github.com/sormondocom/rusty-math-tutor/releases/latest)
[![Play in Browser](https://img.shields.io/badge/Play%20in%20Browser-WebAssembly-7ecfff?style=flat-square&logo=webassembly&logoColor=white)](https://sormondocom.github.io/rusty-math-tutor/)

<a href="https://buymeacoffee.com/sormondocom">
  <img src="https://cdn.buymeacoffee.com/buttons/v2/default-yellow.png" alt="Buy Me A Coffee" height="45" />
</a>

</div>

---

## 🌐 Play in the browser — no install needed

**[→ Open Rusty Math Tutor in your browser](https://sormondocom.github.io/rusty-math-tutor/)**

The full app runs as a WebAssembly build — same game logic, same GPU-free pixel renderer, same transitions and themes, everything. Works on any modern browser on desktop, Chromebook, or phone. Progress is saved to your browser's `localStorage` and persists across visits.

---

## ⬇️ Download — desktop app

Grab a ready-to-run program from the [**latest release**](https://github.com/sormondocom/rusty-math-tutor/releases/latest) — no Rust, no build step:

| Your device | Download |
|-------------|----------|
| **Windows** (most PCs) | [rusty-math-tutor-windows-x86_64.exe](https://github.com/sormondocom/rusty-math-tutor/releases/latest/download/rusty-math-tutor-windows-x86_64.exe) |
| **Chromebook** — Intel / AMD | [rusty-math-tutor-linux-x86_64](https://github.com/sormondocom/rusty-math-tutor/releases/latest/download/rusty-math-tutor-linux-x86_64) |
| **Chromebook** — ARM | [rusty-math-tutor-linux-aarch64](https://github.com/sormondocom/rusty-math-tutor/releases/latest/download/rusty-math-tutor-linux-aarch64) |

- **Windows:** double-click the `.exe` or run from a terminal. It starts in the **console** view; pick **"CPU Graphics"** on the start screen (or run with `--gui`) and it opens a real **window** with software-rendered 2-D graphics — **no GPU required**. Switch between the two views any time. If SmartScreen warns about an unrecognised app, choose *More info → Run anyway*.
- **Chromebook:** these run in the built-in **Linux (Crostini)** environment — turn it on at *Settings → Advanced → Developers → Linux development environment*, then in the Linux terminal:
  ```sh
  chmod +x rusty-math-tutor-linux-x86_64   # the file you downloaded
  ./rusty-math-tutor-linux-x86_64
  ```
  Most Chromebooks are Intel/AMD (`x86_64`); pick `aarch64` only if `uname -m` prints `aarch64`.

> Want the build from a specific commit? Every run of the **Build executables** workflow also uploads the binaries as downloadable artifacts on the [Actions tab](https://github.com/sormondocom/rusty-math-tutor/actions) (those need a GitHub login and expire after 90 days).

---

**A math tutor for kindergarten through 8th grade**, built in Rust. One problem fills the screen at a time so a young learner can focus. Mix and match any combination of **arithmetic, units of measure, fractions, percentages, and geometry** — every correct answer melts into the next through a randomly chosen GPU-free transition. Press **H** any time to summon **Deduction Duck** for a strategy or a hint.

Three frontends, one domain: the **browser** (WASM), the **console TUI**, and the **CPU Graphics window** all share the same game logic — pick whichever fits your screen.

---

## Highlights

- **One problem at a time**, drawn large so a learner can focus.
- **Eight learnable sections**, any combination mixed into a session:
  - **Four arithmetic operations** — addition, subtraction, multiplication, division — with per-grade number ranges (K–8) that you can tune from the Teacher Area.
  - **Units of Measure** — friendly conversion problems (cups, metres, grams, minutes…) with a how-to hint from Deduction Duck. The **locality** sets the units and currency (US, UK, Canada, Australia, metric).
  - **Fractions** — read off the shaded share of a shape that **materialises** piece by piece: a bar, a grid, a **circle**, or a **triangle** (with clear dividers). Answers are checked for *equivalence*, so `1/2` is accepted for `2/4`.
  - **Percentages** — the same shapes, read as a whole-number percent out of 100.
  - **Geometry** — **perimeter, area, and volume** of outlined shapes drawn as polished glyphs: rectangles, squares, triangles, **circles**, and a 3-D wireframe **box/cube**, each labelled with dimensions. Circle answers are typed **in terms of π** (the whole-number coefficient), so every answer stays exact.
  - **Experimentation** — a free-form unit explorer: type any amount (even a silly one) and watch it convert between units while Deduction Duck reacts with a real-world comparison.
- **Horizontal or vertical layouts**: inline `12 + 7 = ?`, the stacked column form, or the proper **long-division house** for `÷`. The answer slot is sized to the actual answer at problem generation time, so the layout never shifts as you type.
- **Deduction Duck 🦆🎓** — press **H** for help on *any* problem:
  - On arithmetic he **walks a number line** (count-on, count-up, count-back, skip-counting, repeated subtraction); **smashes** a number into place-value pieces (break-apart, partial products, regrouping); or explains step by step (make-a-ten, doubling, think-multiplication, equal groups). Press **Space** for *another strategy*.
  - On units, fractions, percentages, and geometry he offers a targeted **how-to hint**.
  - **Hint first** — the method shows first; press **R** to reveal the worked answer.
- **A gentle peek cooldown** — leaning on **R** too often puts the answer on a short cooldown, and the Duck offers a kind nudge instead. Solving one problem yourself earns back the trust. The limit is per-student and teacher-set.
- **"Why am I learning this?"** — press **Y** on any section for a handful of real-world uses, plus occasional **code peeks** at a real line of this app's source and the math behind it.
- **Pleasing random transitions** between problems: wipe, curtain, dissolve, blinds, circle, slide, diagonal — and showy particle effects: the equation **explodes**, **swirls** into the centre, or is celebrated with **fireworks**, **starburst**, alien ships, or asteroids.
- **Milestone celebrations** — reaching a milestone plays a short, name-personalised cinematic with Deduction Duck looking on in awe.
- **Challenge mode** — a timed run with a configurable countdown per student (15–300 seconds, set by the teacher), a live HUD above the card, and a full **end-of-run summary**:
  - Problems solved per minute, accuracy %, and best streak this run.
  - Breakdown by topic type and grade level.
  - **R / Enter** to play again immediately; **Esc** to return to the menu.
  - The clock pauses automatically during transitions and milestone cinematics — a fair chess-timer, not a harsh stopwatch.
  - **Challenge history** persisted per student (last 50 runs); the Progress screen shows the most recent runs with rate, accuracy, and streak columns.
- **Per-student preferences** — grade level, selected operations, and enabled sections are saved per student and restored automatically when switching students.
- **Per-student progress** — the **My Progress** screen shows a friendly bar chart across every section, a **grade-level sparkline** (K–8), personal-best streak, and the recent challenge history. Deliberately **no ranking or comparison** between students.
- **Teacher Area** (password-gated):
  - Add your own real-life "Why?" examples per section.
  - Set the measurement locality.
  - Administer student records: view counts per section, reset a section, reset all records, remove a student, set the answer-peek limit, configure the challenge timer, and **clear challenge history**.
- **Themes** — choose a look in Settings: Default, Blackboard, or Chalkboard. The chalk themes render an animated wooden board frame with a chalk-texture overlay.

---

## Pixel renderer — browser, window, and beyond

All three graphical frontends (browser, CPU window, and future targets) use the same `tiny-skia` software pixel pipeline:

**Performance details** worth knowing:
- **Hash-based frame caching** — the renderer fingerprints all visual state before each draw. If nothing changed (student is thinking, timer is between seconds) the render is skipped entirely and the cached frame is reused; the CPU is essentially idle between user actions.
- **Transition optimisation** — both card frames are captured once at transition start and pre-downsampled to output resolution. The per-tick blend cost is just a pixel-merge over the output buffer rather than a full supersampled re-render, making transitions smooth at any screen size.
- **Challenge HUD overlay** — the timer bar and solved count are rendered as a separate overlay and blitted after every frame, so they are never baked into transition captures. The clock is also paused during transitions, so there are no jumps.
- **Supersampled rendering (SS=3)** — every scene is rendered at 3× output resolution and box-filtered down, giving clean anti-aliased edges on shapes, geometry figures, and the duck without a GPU.

**Browser specifics:**
- The WASM build uses the same pixel pipeline with the rendered frames written to a `<canvas>` via `ImageData`. Touch events (swipe to navigate, on-screen numeric keypad) are supported for phones and tablets.
- Progress is persisted in `localStorage`; a first visit creates the default roster automatically.
- The GitHub Actions workflow rebuilds and deploys the WASM bundle to GitHub Pages on every push to `main`.

---

## Build & run

> Just want to use it? [**Play in the browser**](https://sormondocom.github.io/rusty-math-tutor/) or [**download an executable**](https://github.com/sormondocom/rusty-math-tutor/releases/latest) — no toolchain required. Build from source only if you want to hack on it.

### Desktop (console + window)

Requires a [Rust toolchain](https://rustup.rs/) (nightly).

```sh
cargo run --release           # console TUI + CPU-graphics window
cargo run --release -- --gui  # boot straight into the window
```

Run it from a real terminal (it's a full-screen TUI). A window of about **80×24 or larger** is recommended.

For a lean console-only binary (no window deps):

```sh
cargo run --release --no-default-features
```

```sh
cargo test --bin tutor   # unit + render tests
```

### Browser (WASM)

Requires [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/) and the nightly `wasm32-unknown-unknown` target:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack

wasm-pack build --target web     # release build → pkg/
```

Serve locally (the browser requires HTTP — file:// won't work for WASM):

```sh
python -m http.server 8080
# open http://localhost:8080/www/
```

---

## Controls

### Menu
| Key | Action |
|-----|--------|
| `↑` / `↓` | Move between rows |
| `←` / `→` | Change the highlighted setting (student, grade) |
| `Enter` / `Space` | Toggle / open the highlighted row |
| `N` | Add a new student |
| `Q` | Quit |

Rows: **Student**, **Grade**, the four **operation toggles**, **Units of Measure**, **Fractions**, **Percentages**, **Geometry**, **Settings**, **My Progress**, **Teacher Area**, **Start Practice**, **Start Challenge**, **Experimentation**.

### Practice / Challenge
| Key | Action |
|-----|--------|
| `0`–`9` | Type your answer |
| `/` | Separate a fraction answer (`2/4`) |
| `-` | Leading minus (when negatives are allowed) |
| `Enter` | Check |
| `Backspace` | Edit |
| `H` | Summon / dismiss Deduction Duck |
| `R` | Reveal / hide the worked answer |
| `Space` | Another strategy (arithmetic, while the duck is out) |
| `Y` | "Why am I learning this?" |
| `V` | Switch horizontal / vertical layout (arithmetic) |
| `Esc` | Close an overlay, then return to the menu |

Fractions are typed as `a/b` (any equivalent form accepted); geometry and circle answers (the coefficient before π) are typed as whole numbers.

On touch devices the on-screen keypad appears automatically during practice and challenge sessions.

### Challenge summary screen
Appears automatically when the countdown reaches zero.

| Key | Action |
|-----|--------|
| `R` / `Enter` | Start a new challenge immediately |
| `M` / `Esc` | Return to the menu |

### Teacher Area
First visit prompts you to **create a password**; later visits require it.  
`Tab` switches between the two tools; `L` cycles the measurement **locality**.

**Why? Examples** — `←` / `→` pick a section, `A` to add your own real-life example for it.

**Student Records** — a column per section:

| Key | Action |
|-----|--------|
| `↑` / `↓` | Pick a student |
| `←` / `→` | Pick the active section column |
| `S` | Reset that section's count for this student |
| `R` | Reset all records for this student |
| `X` | Remove this student |
| `+` / `-` | Adjust the answer-peek limit (0 = never lock, max 9) |
| `[` / `]` | Decrease / increase the challenge timer (15 s steps, 15–300 s) |
| `C` | Clear this student's challenge history |

---

## Saved data

### Desktop
Everything persists as plain JSON under your platform's config directory  
(`%APPDATA%\rusty-math-tutor\` on Windows, `~/.config/rusty-math-tutor/` elsewhere):

| File | Holds |
|------|-------|
| `config.json` | Per-grade number ranges, layout, graphics mode, measurement locality, theme, teacher password (salted hash) |
| `students.json` | Roster and each student's progress — per-section counts, grade-level breakdown, best streak, peek limit, challenge timer, menu preferences, and challenge history (last 50 runs) |
| `why_extras.json` | Teacher-added "Why?" examples, per section |

These are hand-editable. Forgot the teacher password? Delete the `"teacher"` field from `config.json` and the next visit will let you set a new one.

### Browser (WASM)
The same three JSON blobs are stored in `localStorage` under the same key names. Clear site data in your browser settings to reset, or open DevTools → Application → Local Storage to edit individual entries.

---

## What's built

- **Eight problem sections** — four arithmetic operations, Units of Measure, Fractions, Percentages, and Geometry — freely mixed into a session, with K–8 grade scaling and configurable number ranges.
- **Experimentation** unit explorer with per-locality units and currency, and Deduction Duck real-world size reactions.
- **Horizontal / vertical / long-division** layouts with stable field sizing.
- **Deduction Duck** with number-line walking, place-value smash, and step-by-step strategies for arithmetic; targeted hints for all other sections; hint-first reveal; strategy cycling; and a gentle answer-peek cooldown with a teacher-set limit.
- **"Why am I learning this?"** for every section — teacher-editable examples and code peeks.
- **Seven cell-based transitions** plus explode / swirl / fireworks / starburst / alien ships / asteroids particle effects, all GPU-free.
- **Name-personalised milestone cinematics** — rocket-name and comet-and-moon night skies.
- **Challenge mode** — per-student configurable timer, chess-timer pause, end-of-run summary with accuracy and grade breakdown, and persistent challenge history.
- **Per-student preferences** and **per-student progress** with section bar chart, grade sparkline, and challenge history.
- **Teacher Area** — Why? example editor, locality selector, student records table with per-section reset, student removal, peek-limit and challenge-timer adjustment, and challenge history clearing.
- **Three frontends, one domain**:
  - **Console TUI** — `ratatui` + `crossterm`, runs anywhere a terminal does.
  - **CPU Graphics window** — `winit` + `softbuffer` + `tiny-skia`, software-rendered fullscreen window with chalk/blackboard themes, animated duck, and particle transitions.
  - **Browser / WASM** — same pixel renderer compiled to WebAssembly, written to a `<canvas>` via `ImageData`, with `localStorage` persistence, touch input, and GitHub Pages deployment.

**Coming up:**
- More strategy visualisations and additional section types.

---

## License

GPL-3.0. Built with [`ratatui`](https://ratatui.rs/) + `crossterm` (terminal), [`winit`](https://github.com/rust-windowing/winit) + [`softbuffer`](https://github.com/rust-windowing/softbuffer) + [`tiny-skia`](https://github.com/RazrFalcon/tiny-skia) (window), and [`wasm-bindgen`](https://github.com/rustwasm/wasm-bindgen) + [`web-sys`](https://github.com/rustwasm/wasm-bindgen/tree/main/crates/web-sys) (browser).
