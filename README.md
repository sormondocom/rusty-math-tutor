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
queue, no ticking clock (the timed mode is opt-in). Correct answers melt into
the next problem through a randomly chosen, GPU-free transition, and pressing
**H** summons **Deduction Duck** to act out a solving strategy.

It is designed to be *lightweight*: pure terminal-cell rendering, no graphics
card required. The goal is to help children, not to sell hardware.

---

## Highlights

- **One problem at a time**, drawn big in a hand-built block font.
- **Four operations** — addition, subtraction, multiplication, division — with
  per-grade number ranges (K–8) that you can tune.
- **Horizontal or vertical layouts**: inline `12 + 7 = ?`, the stacked column
  form, or the proper **long-division house** for `÷`.
- **Deduction Duck 🦆🎓** — press **H** for a strategy. He doesn't just talk:
  - **walks a number line**, hopping stop to stop (count-on, count-up,
    count-back, skip-counting, repeated subtraction),
  - **smashes** a number apart into place-value pieces (break-apart, partial
    products, regrouping),
  - or explains it step by step (make-a-ten, doubling, think-multiplication,
    equal groups).
  - **Hint first** — the method shows first; press **R** to reveal the answer.
  - Press **Space** to see *another way*.
- **"Why am I learning this?"** — press **Y** for a fresh handful of real-world
  uses (sending rockets to space, carpet area, splitting a bill…), plus an
  occasional **"for future coders"** peek at a real line of this app's own code
  and why the math behind it matters.
- **Pleasing random transitions** between problems: wipe, curtain, dissolve,
  blinds, circle, slide, diagonal — and showy particle effects: the equation
  **explodes**, **swirls** into the centre, or is celebrated with **fireworks**.
- **Optional Challenge mode** — a 60-second timer, a score, and an encouraging
  summary. Totally opt-in.
- **Per-student progress** — each student has their own saved stats. The
  **My Progress** screen celebrates *their own* effort with a friendly
  per-operation breakdown and a personal-best streak. There is deliberately
  **no ranking or comparison** between students.
- **Teacher Area** (password-gated) — add your own real-life "Why?" examples and
  administer student records (reset a section, reset a student, remove a
  student).

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

Rows: **Student**, **Grade**, the four **operation toggles**, **Show problems**
(layout), **Settings**, **My Progress**, **Teacher Area**, **Start Practice**,
**Start Challenge**.

### Practice / Challenge
| Key | Action |
|-----|--------|
| `0`–`9`, `-` | Type your answer |
| `Enter` | Check |
| `Backspace` | Edit |
| `H` | Summon / dismiss Deduction Duck |
| `R` | Reveal / hide the worked answer |
| `Space` | Show another strategy (while the duck is out) |
| `Y` | "Why am I learning this?" |
| `V` | Switch horizontal / vertical layout |
| `Esc` | Peel back an overlay, then return to the menu |

### Teacher Area
First visit asks you to **create a password**; later visits ask for it.
`Tab` switches between the two tools:
- **Why? Examples** — `← →` pick an operation, `A` to add your own.
- **Student Records** — `↑↓` pick a student, `← →` pick a section,
  `S` reset that section, `R` reset all of a student's records, `X` remove a
  student.

---

## Saved data

Everything persists as plain JSON under your platform's config directory
(`%APPDATA%\rusty-math-tutor\` on Windows, `~/.config/rusty-math-tutor/`
elsewhere):

| File | Holds |
|------|-------|
| `config.json` | Per-grade number ranges, layout, graphics mode, teacher password (a salted hash) |
| `students.json` | The roster and each student's progress |
| `why_extras.json` | Teacher-added "Why?" examples |

These are hand-editable. Forgot the teacher password? Delete the `"teacher"`
field from `config.json` and the next visit will let you set a new one.

---

## Current state

This is an actively evolving first-pass project. **Implemented and working
today:**

- All four operations, K–8 grade scaling, configurable ranges.
- Horizontal / vertical layouts incl. the long-division house.
- Deduction Duck with number-line, smash, and talk-through strategies, hint
  -first reveal, and strategy cycling.
- "Why am I learning this?" with teacher-editable examples and code peeks.
- Seven cell-based transitions plus explode / swirl / fireworks particle
  effects.
- Practice and timed Challenge modes.
- Per-student profiles, progress screen, and the password-gated Teacher Area
  with records administration.

**Shelved / coming soon:**

- **CPU Graphics mode** — a future software-rendered (2D/3D) frontend, *CPU
  only, never requiring a GPU*. The startup picker already offers it, and the
  domain layer (problems, strategies, transitions-as-data, students) is kept
  deliberately renderer-agnostic so a second frontend can slot in behind the
  same core. For now, **Low Graphics (console)** is the supported mode.
- More strategy visualisations, fractions/negatives as grades grow, and richer
  student insights.

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
