# OSlings for the classroom: design proposal

**Two features for using OSlings in CS 326:** (1) configurable exercise
difficulty, and (2) student progress tracking for grading.

Status: proposal / for discussion. Nothing here changes current behavior yet;
every default is chosen so a fresh install behaves exactly as it does today.

---

## 0. Context and goals

OSlings today is tuned for a solo learner: each exercise ships a skeleton whose
`// IMPLEMENT` comments walk through the solution step by step, plus a
three-level `hints.md` whose last level is the full code. State lives in
`.oslings/state.toml` (which exercises are completed, how many hints were used).
Passing an exercise auto-advances and stages the next skeleton into `rv6/src`.

For a course we need two things that a solo tool does not:

1. **Instructors must be able to dial the amount of guidance up or down**, so an
   exercise can be a quick guided walk-through or a full-lab challenge.
2. **Progress must be trackable and gradable**, in a way that is hard to fake
   and that fits a normal gradebook.

Two design principles run through both features:

- **Backwards compatible by default.** With no configuration, OSlings behaves
  exactly as it does now (fully guided, trust-based local state).
- **One source of truth.** We do not fork the exercise content into multiple
  copies. Harder modes are *derived* from the existing guided skeletons, and
  grading is *derived* from the existing test harness.

---

## 1. Feature: configurable difficulty

### 1.1 The key insight

The biggest hint in OSlings is not `hints.md`. It is the **skeleton itself**.
Each `// IMPLEMENT` block names the task and then lists the exact steps (often
with the literal lines of code to write). So "make difficulty configurable" is
mostly about **controlling the in-skeleton guidance**, with hint gating as a
secondary knob.

There are three layers of guidance, and we want to control them together:

| Layer | Where | Most guidance |
|---|---|---|
| Step-by-step comments | `skeleton/*.rs` `// IMPLEMENT` blocks | highest |
| Leveled hints | `hints.md` (Hint 1/2/3) | high (Hint 3 = full code) |
| The lesson | `README.md` | conceptual only (always kept) |

### 1.2 Difficulty levels

Three presets, bundling the layers so an instructor sets one value:

| Level | Skeleton comments | Hints available | Intended use |
|---|---|---|---|
| `guided` (default) | full step-by-step | all three | homework, self-study, current behavior |
| `standard` | task line only (steps removed) | Hint 1 and 2 | normal lab work |
| `challenge` | one-line `// TODO` only | Hint 1 only (or none) | full-period / exam-style labs |

The `README.md` lesson is always shown at every level: understanding the concept
is never the thing we withhold; only the hand-holding toward the *answer* is.

### 1.3 Where the setting lives

- **Course default:** `info.toml` `[meta]` gains `difficulty = "guided"`. The
  instructor commits this once; it ships with the class distribution.
- **Local override:** `.oslings/config.toml` (already the natural home for
  local, non-committed settings) may set `difficulty` to override the course
  default on one machine. Useful for a student who wants a harder run, or a TA
  demoing a level.
- **Escape hatch:** an `OSLINGS_DIFFICULTY` environment variable overrides both,
  for scripted setups.
- **Per-exercise override (optional):** an exercise may set its own `difficulty`
  in `info.toml`, so a course can be mostly `standard` but keep, say, the first
  two exercises `guided`. Precedence: env > per-exercise > local > course > `guided`.

### 1.4 Mechanics: deriving harder skeletons from the guided source

We keep a single skeleton per exercise (the current guided one) and strip
guidance at **stage time** (the moment the CLI copies skeleton files into
`rv6/src`). To make stripping reliable rather than heuristic, we adopt a small,
explicit comment convention in skeletons:

```rust
// IMPLEMENT: fdalloc - find a free slot in the open-file table.   <- TASK line (kept at standard)
//   Scan (*p).ofile for the first slot whose kind is None,        <- DETAIL lines
//   store `file` there, and return its index; else return -1.     <-   (removed at standard/challenge)
```

Rule: within an `// IMPLEMENT:` block, the **first comment line is the task**
(kept in `standard`), and the contiguous comment lines after it are **detail**
(removed in `standard` and `challenge`). In `challenge`, the whole block becomes
a single `// TODO: implement (see the lesson and Hint 1).`

This is almost exactly how the skeletons are already written (a summary line,
then numbered steps), so the retrofit is small and mechanical. Crucially:

- Only comment lines inside a marked block are ever touched. Real code is never
  stripped, because we only remove lines matching `^\s*//` that fall inside a
  recognized block.
- `solution/` is never modified; it stays the reference answer.
- Changing difficulty **re-stages the current exercise** so the student
  immediately sees the right comment level (with a warning if they have
  unsaved work in `rv6/src`).

Alternative considered: maintain a separate `skeleton_challenge/` per exercise.
Rejected - it doubles the content to keep in sync across 23 exercises and drifts
over time. Deriving from one source is more maintainable.

### 1.5 Hint gating

`oslings hint` already serves leveled hints and records usage. Difficulty caps
the maximum level reachable (`guided` = 3, `standard` = 2, `challenge` = 1 or 0).
No change to `hints.md` files; the CLI simply refuses to reveal beyond the cap
and says so. Hint usage continues to be recorded (and becomes gradable input,
see 2.5).

### 1.6 UX / CLI changes

- `oslings difficulty [guided|standard|challenge]` - show or set the local level.
- The difficulty is shown in the TUI status line and in `oslings progress`.
- `oslings hint` past the cap prints a friendly "hints are limited in challenge
  mode" message.

### 1.7 Recommendation

Ship the three-level model with `guided` as the default. Do the one-time
skeleton comment-convention normalization so stripping is exact. This gives the
professor a single course-wide knob plus per-exercise overrides, with zero
change for anyone who does not opt in.

---

## 2. Feature: progress tracking and grading

### 2.1 The structural crux (read this first)

OSlings is **cumulative**: `rv6/src` only ever holds the *current* exercise, and
passing **auto-advances and overwrites it**. Three consequences shape the whole
design:

1. **Past solutions are not retained anywhere.** Once a student moves on, their
   earlier work is gone. There is nothing to re-grade or spot-check.
2. **You cannot re-verify past exercises from the working tree** - it only
   contains the current one.
3. **`.oslings/state.toml` is a plain editable file.** On its own, "completed"
   is a claim, not a proof.

So grading needs us to (a) *retain an artifact* of each passing solution, and
(b) offer a way to *re-verify* it independently of the student's own claim.

### 2.2 Data model

Extend local state with a per-exercise progress record. Keep the existing
`state.toml` shape and add a `progress.toml` (or a `[progress]` table):

```toml
# .oslings/progress.toml
[student]
name  = "Ada Lovelace"        # prompted once on first run, stored locally
email = "alovelace@usfca.edu"

[[exercise]]
name        = "20_file_descriptors"
passed_at   = "2026-09-18T14:03:11Z"
difficulty  = "standard"      # level in effect when passed
hints_used  = 1               # from state.toml
attempts    = 6               # compile/run count while on this exercise (optional)
snapshot    = "submissions/20_file_descriptors/"   # where the passing code was saved
checksum    = "sha256:9f2c..."                     # hash of the snapshot files
```

### 2.3 Snapshot on pass

When an exercise passes, the CLI copies the exercise's `files` from `rv6/src`
into `.oslings/submissions/NN_name/`, and records a checksum. This is the single
change that makes everything else possible: it turns each pass into a retained,
re-runnable artifact. Cost is tiny (a few small `.rs` files per exercise).

### 2.4 Integrity levels

Grading integrity is a spectrum; the course can pick a point on it.

| Level | Mechanism | Effort | Trust |
|---|---|---|---|
| L0 trust | `progress.toml` timestamps only | none | fakeable |
| L1 artifacts | snapshot-on-pass + export | low | must produce passing code; instructor can re-run |
| L2 git-verified | students push repo; instructor re-runs harness per snapshot | medium | strong |
| L3 signed log | append-only, HMAC-signed with a course secret | high | tamper-evident |

**Recommendation:** make **L1 the baseline** everyone gets (snapshots + export),
and use **L2 as the graded path**. L3 is available if the course ever needs
tamper-evidence, but is likely overkill.

The linchpin of L2 is one instructor-side command:

```
oslings grade <submission-dir>     # re-runs the harness against a student's
                                   # snapshots and prints a verified pass/fail
                                   # per exercise, ignoring their state.toml
```

Because grading re-runs the *actual test harness* against the *saved code*, a
student cannot get credit by editing `state.toml`; they must have submitted code
that genuinely passes. That is the property we want.

### 2.5 Grading model options (for the professor to choose)

- **Completion points:** N points per verified-passed exercise. Simplest.
- **Difficulty-weighted:** more points for exercises passed at `standard` /
  `challenge` (rewards doing labs without the full hand-holding).
- **Hint-aware:** small deduction per hint used, or a "no-hints" bonus. OSlings
  already records hint usage, so this is free data. (The professor raised hints
  explicitly; this makes them a lever rather than an all-or-nothing.)
- **Milestones:** checkpoints at Part 1 complete (00-11) and Part 2 complete
  (12-22), each worth a chunk of the grade.

### 2.6 Instructor tooling

- `oslings progress` - student-facing: what is done, at what difficulty, hints used.
- `oslings progress --export [--format json|csv]` - a gradebook-ready report,
  identified by the student's name/email.
- `oslings grade <dir>` - instructor: re-verify a submission (2.4).
- `oslings roster <dir-of-exports>` - instructor: aggregate many students'
  exports into one class CSV (one row per student, one column per exercise).

### 2.7 Important integrity note: solutions ship in the repo

Right now, `solution/` directories are in the repository. The CLI locks the
`solution` command until an exercise is passed, but the files are on disk, so a
determined student can read them. For graded use, the course should distribute a
**student build that omits (or encrypts) `solution/`**. Options:

- A `make student-dist` / packaging step that strips `solution/` (and this
  `docs/` folder) from what students receive.
- Keep solutions in a private instructor repo; students get skeletons + tests
  only.

This does not block anything above, but it is the one real leak in a graded,
AI-free setting and is worth deciding early.

---

## 3. Cross-cutting: a "class mode"

Several of the above collapse into a single course profile the instructor sets
once, in `info.toml [meta]`:

```toml
[meta]
pass_marker = "OSLINGS:PASS"
fail_marker = "OSLINGS:FAIL"
difficulty  = "standard"     # course-wide default (1.3)
snapshots   = true           # snapshot on pass (2.3)
require_identity = true       # prompt for name/email on first run (2.2)
```

Students clone, install QEMU, run `oslings`, and everything (difficulty, hint
caps, snapshotting) follows the course profile automatically.

---

## 4. Curriculum integration and pacing

Not code, but part of "seamless integration":

- **Lab 0:** environment setup (QEMU + toolchain + oslings). Setup friction is
  real; budget the first lab, or make it pre-work with a checker script
  (`oslings doctor` that verifies the toolchain and QEMU).
- **Rust on-ramp:** OSlings already teaches the Rust each concept needs inline,
  so a separate Rust course is not required. If desired, an optional Rustlings
  warm-up before exercise 00 is a low-cost add.
- **Suggested mapping to the Tue-lecture / Thu-lecture / Fri-lab format:** pick a
  difficulty per phase (e.g. `guided` for Part 1 while students find their feet,
  `standard` or `challenge` for Part 2), and target a small number of exercises
  per lab. The two natural milestones (end of Part 1, end of Part 2) make clean
  grading checkpoints.

---

## 5. Suggested phased implementation

Small, independently shippable steps, each backwards compatible:

1. **Difficulty scaffolding.** Add `difficulty` config (info.toml + local +
   env), `oslings difficulty`, and hint gating. No skeleton changes yet -
   `standard`/`challenge` just cap hints. Low risk, immediately useful.
2. **Comment-convention normalization + stripping.** Normalize skeleton
   `// IMPLEMENT` blocks to the task-line/detail convention (1.4) and implement
   stage-time stripping for `standard`/`challenge`.
3. **Snapshots + progress record.** Snapshot on pass, `progress.toml`, identity
   prompt, `oslings progress --export`.
4. **Instructor tooling.** `oslings grade` (re-verify) and `oslings roster`
   (aggregate). This is what makes grades trustworthy.
5. **Student distribution / class mode.** Packaging that omits `solution/`, plus
   `oslings doctor` for setup.

Phases 1 and 3 deliver most of the classroom value on their own; 2 and 4 add
rigor; 5 hardens it for a graded, AI-free setting.

---

## 6. Open questions for the professor

1. **Difficulty granularity:** one course-wide level, or per-phase / per-lab? (We
   support both; which do you want to use?)
2. **Hints in the grade:** neutral, small penalty, or a no-hints bonus?
3. **Grading integrity target:** L1 (artifacts) or L2 (git-verified re-run)? This
   drives how much instructor tooling we build.
4. **Solutions distribution:** ship a student build without `solution/`? (Needed
   for a truly AI-free / closed-book grade.)
5. **Identity / roster:** name+email in a local file, or tie into an existing
   roster / LMS export format you already use?
