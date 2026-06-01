# Rusty Math Tutor

```
                  .-----------------------.
       ___       (  Quack! Let's do math!  )
      [_____]     '--------------------.---'
         |       .------------------.  /
        __      /  RUSTY MATH TUTOR  \
      <(o )__   '--------------------'
       (  __)
        ^^ ^^      ( Deduction Duck, at your service )
```

**A console math tutor for kindergarten through 8th grade**, built in Rust.
One problem fills the screen at a time so a young learner can focus — no visible
queue, no ticking clock (the timed mode is opt-in). Mix and match the sections
you want practised — **arithmetic, units of measure, fractions, percentages, and
geometry** — and every correct answer melts into the next problem through a
randomly chosen, GPU-free transition. Press **H** any time to summon
**Deduction Duck** for a strategy or a hint.

It is designed to be *lightweight*: pure terminal-cell rendering, no graphics
card required. The goal is to help children, not to sell hardware.

---

## Highlights

- **One problem at a time**, drawn big in a hand-built block font.
- **Eight learnable sections**, any combination mixed into a session:
  - **Four operations** — addition, subtraction, multiplication, division — with
    per-grade number ranges (K–8) that you can tune.
  - **Units of Measure** — friendly conversion problems (cups, metres, grams,
    minutes…) with a themed prop and a Deduction Duck how-to hint. The
    **locality** sets the units and currency (US, UK, Canada, Australia, metric).
  - **Fractions** — read off the shaded share of a shape that **materialises**
    piece by piece: a bar, a grid, a **circle**, or a **triangle** (with clear
    dividers). Answers are checked for *equivalence*, so `1/2` is accepted for
    `2/4`.
  - **Percentages** — the same shapes, read as a whole-number percent out of 100.
  - **Geometry** — **perimeter, area, and volume** of outlined shapes drawn as
    polished text glyphs: rectangles, squares, triangles, **circles**, and a
    3-D wireframe **box/cube**, each labelled with its dimensions. Circles are
    answered **in terms of π** (you type the whole-number coefficient), so every
    answer stays exact.
  - **Experimentation** — a free-form unit explorer: type any amount (even a
    silly one) and watch it convert between units.
- **Horizontal or vertical layouts**: inline `12 + 7 = ?`, the stacked column
  form, or the proper **long-division house** for `÷`.
- **Deduction Duck 🦆🎓** — press **H** for help on *any* problem:
  - on arithmetic he doesn't just talk — he **walks a number line**, hopping
    stop to stop (count-on, count-up, count-back, skip-counting, repeated
    subtraction); **smashes** a number into place-value pieces (break-apart,
    partial products, regrouping); or explains it step by step (make-a-ten,
    doubling, think-multiplication, equal groups). Press **Space** for *another
    way*.
  - on units, fractions, percentages, and geometry he offers a how-to **hint**
    (e.g. "Area = ½ × base × height").
  - **Hint first** — the method shows first; press **R** to reveal the answer.
- **A gentle peek cooldown** — leaning on **R** too often puts the answer on a
  short cooldown, and Deduction Duck offers a kind, clever nudge instead ("A
  garden won't grow unwatered — and your mind is the garden!"). Solving one
  yourself earns the trust back. The limit is per-student and teacher-set.
- **"Why am I learning this?"** — press **Y** on any section for a fresh handful
  of real-world uses (sending rockets to space, carpet area, splitting a bill,
  reading a 70% chance of rain…), plus an occasional **"for future coders"** peek
  at a real line of this app's own code and why the math behind it matters.
- **Pleasing random transitions** between problems: wipe, curtain, dissolve,
  blinds, circle, slide, diagonal — and showy particle effects: the equation
  **explodes**, **swirls** into the centre, or is celebrated with **fireworks**.
- **Milestone celebrations** — reaching a milestone plays a short, name-
  personalised cinematic. The showpiece is a starry night sky: either **rockets
  launch and become the stars that spell the student's name** (then a blue comet
  sweeps behind it), or the name is **written among streaking comets with a moon
  rising beneath** — with Deduction Duck planted below, gazing up in awe.
- **Optional Challenge mode** — a 60-second timer, a score, and an encouraging
  summary. Totally opt-in. (A celebration never costs you time — the clock is
  refunded.)
- **Per-student progress** — each student has their own saved stats. The
  **My Progress** screen celebrates *their own* effort with a friendly breakdown
  across **every section** and a personal-best streak. There is deliberately
  **no ranking or comparison** between students.
- **Teacher Area** (password-gated) — add your own real-life "Why?" examples for
  *each section*, set the measurement locality, and administer student records
  (per-section counts, reset a section, reset a student, remove a student, set
  the peek limit).

---

## Build & run

Requires a [Rust toolchain](https://rustup.rs/) (stable).

```sh
cargo run --release
```

Run it from a real terminal (it's a full-screen TUI). A window of about
**80×24 or larger** is recommended — the walking-duck and smash animations fall
back to a plain text view on very small windows.

```sh
cargo test     # unit tests, including render-never-panics across screen sizes
```

---

## Controls

### Startup
Choose a graphics mode (see *Current state* below), then **Enter**.

### Menu
| Key | Action |
|-----|--------|
| `↑` / `↓` | Move between rows |
| `← / →` | Change the highlighted setting (student, grade) |
| `Enter` / `Space` | Toggle / open the highlighted row |
| `N` | Add a new student |
| `Q` | Quit |

Rows: **Student**, **Grade**, the four **operation toggles**, **Units of
Measure**, **Fractions**, **Percentages**, **Geometry**, **Show problems**
(layout), **Settings**, **My Progress**, **Teacher Area**, **Start Practice**,
**Start Challenge**, **Experimentation**.

### Practice / Challenge
| Key | Action |
|-----|--------|
| `0`–`9` | Type your answer |
| `/` | Separate a fraction answer (e.g. `2/4`) |
| `-` | Leading minus (when negatives are allowed) |
| `Enter` | Check |
| `Backspace` | Edit |
| `H` | Summon / dismiss Deduction Duck (works on every problem) |
| `R` | Reveal / hide the worked answer |
| `Space` | Show another strategy (arithmetic, while the duck is out) |
| `Y` | "Why am I learning this?" (every section) |
| `V` | Switch horizontal / vertical layout (arithmetic) |
| `Esc` | Peel back an overlay, then return to the menu |

Fractions are typed as `a/b` (any equivalent form is accepted); percentages,
geometry, and circle answers (the number before π) are typed as a whole number.

### Teacher Area
First visit asks you to **create a password**; later visits ask for it.
`Tab` switches between the two tools; `L` cycles the measurement **locality**.
- **Why? Examples** — `← →` pick a **section** (any of the eight), `A` to add
  your own real-life example for it.
- **Student Records** — a column per section. `↑↓` pick a student, `← →` pick
  the section to act on, `S` reset that section, `R` reset all of a student's
  records, `X` remove a student, `+ / -` set the student's answer-peek limit.

---

## Saved data

Everything persists as plain JSON under your platform's config directory
(`%APPDATA%\rusty-math-tutor\` on Windows, `~/.config/rusty-math-tutor/`
elsewhere):

| File | Holds |
|------|-------|
| `config.json` | Per-grade number ranges, layout, graphics mode, measurement locality, teacher password (a salted hash) |
| `students.json` | The roster and each student's progress (per-section counts, best streak, peek limit) |
| `why_extras.json` | Teacher-added "Why?" examples, per section |

These are hand-editable. Forgot the teacher password? Delete the `"teacher"`
field from `config.json` and the next visit will let you set a new one.

---

## Current state

This is an actively evolving first-pass project. **Implemented and working
today:**

- Eight sections — four operations, Units of Measure, Fractions, Percentages,
  and Geometry (perimeter / area / volume of outlined shapes, including circles
  in terms of π) — freely mixed into a session, with K–8 grade scaling and
  configurable ranges.
- The Experimentation unit explorer and per-locality units/currency.
- Horizontal / vertical layouts incl. the long-division house.
- Deduction Duck with number-line, smash, and talk-through strategies for
  arithmetic and how-to hints for the other sections, hint-first reveal,
  strategy cycling, and a gentle answer-peek cooldown.
- "Why am I learning this?" for every section, with teacher-editable examples
  and code peeks.
- Seven cell-based transitions plus explode / swirl / fireworks particle
  effects, and name-personalised milestone cinematics (rocket-name and
  comet-and-moon night skies, with Deduction Duck looking on in awe).
- Practice and timed Challenge modes.
- Per-student profiles, a whole-journey progress screen, and the password-gated
  Teacher Area with per-section Why? examples and records administration.

**Shelved / coming soon:**

- **CPU Graphics mode** — a future software-rendered (2D/3D) frontend, *CPU
  only, never requiring a GPU*. The startup picker already offers it, and the
  domain layer (problems, strategies, transitions-as-data, students) is kept
  deliberately renderer-agnostic so a second frontend can slot in behind the
  same core. For now, **Low Graphics (console)** is the supported mode.
- More strategy visualisations, more section types, and richer student
  insights.

---

## License

GPL-3.0. Built with [`ratatui`](https://ratatui.rs/) + `crossterm`.

---

## Support

If Rusty Math Tutor helps a kid in your life, you can buy me a coffee:

### ☕ https://buymeacoffee.com/sormondocom

```
        ___
      [_____]
         |
        __
      <(o )__     Thanks for stopping by!
       (  __)     — Deduction Duck
        ^^ ^^
```
