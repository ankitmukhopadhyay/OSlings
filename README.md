# OSlings 🦀⚙️

A [Rustlings](https://github.com/rust-lang/rustlings)-style CLI that teaches
operating-system concepts by having you build a small pedagogical RISC-V
kernel, **rv6**, from nothing — one exercise at a time.

Every exercise follows the same rhythm:

> **Learn** (read the concept) → **Understand** (read & annotate the code) →
> **Implement** (fill in the `IMPLEMENT` markers until the test passes).

Rust is taught *in context*, alongside the OS concept that motivates it — no
separate Rust course required.

## Requirements

- Rust **nightly** with the `riscv64gc-unknown-none-elf` target
  (`rustup target add riscv64gc-unknown-none-elf --toolchain nightly`)
- `qemu-system-riscv64` (QEMU 7+; tested on 8.2)

The kernel's `rv6/rust-toolchain.toml` pins nightly automatically; the CLI
itself builds on stable.

## Getting started

```sh
# build the CLI once
cargo build --manifest-path oslings-cli/Cargo.toml

# from the project root, just run it:
oslings
```

(Put `oslings-cli/target/debug/` on your PATH, or alias
`oslings=./oslings-cli/target/debug/oslings`.)

## The interactive app

Running `oslings` with no arguments launches a full-screen, Rustlings-style
terminal app. It opens on the **current exercise's lesson**; everything else is
one keypress away, and a progress bar stays pinned to the bottom.

It's page-based:

```
 Lesson  ──n──▶  Watch        (auto-runs the test; re-runs on every file save)
   ▲   ◀──p────────┘
   │
   ├── l ──▶  List            (jump to any exercise you've already reached)
   └── h ──▶  Hints           (reveal one more each press)
```

| Key | Where | Action |
|---|---|---|
| `n` | Lesson | Begin: run the test and watch for saves |
| `n` | Watch (passed) | Advance to the next exercise's lesson |
| `p` | Watch / overlays | Back to the previous page (the lesson) |
| `l` | anywhere | Open the exercise list (↑↓ move, ⏎ open, `p` back) |
| `h` | Lesson / Watch | Show hints; press again for the next one |
| `r` | Watch | Reset the exercise's starter code and re-run |
| `↑`/`↓` `j`/`k`, PgUp/PgDn | content pages | Scroll |
| `q` / Ctrl-C | anywhere | Quit |

While on the Watch page, **edit `rv6/src/…` in your editor and save** — the test
re-runs automatically. On success, press `n` to move on.

## Scriptable subcommands

The same actions are available non-interactively (handy for scripting or if you
prefer a plain shell loop):

| Command | What it does |
|---|---|
| `oslings run [exercise]` | Run one exercise's test once (defaults to current). |
| `oslings watch` | Headless: re-run the current exercise on every save. |
| `oslings hint [exercise]` | Reveal the next progressive hint. `--all`, `--reset`. |
| `oslings progress` | Show completed / remaining with a progress bar. |
| `oslings list` | List every exercise and its test mode. |
| `oslings lesson [exercise]` | Render an exercise's README in the terminal. |
| `oslings reset [exercise]` | Restore an exercise's starter code (discards edits). |
| `oslings solution [exercise]` | Show the reference solution (unlocks after you pass). |
| `oslings goto [exercise]` | Move the "current" pointer (no arg = next exercise). |

## How it works

- **You edit `rv6/src/`.** That directory *is* the kernel you are building.
- The kernel is built **cumulatively**: each exercise's starter code already
  contains the concepts you completed earlier, plus new `IMPLEMENT` markers for
  the current step. Passing an exercise stages the next one's files for you.
- **Two test modes** (declared per-exercise in `info.toml`):
  - `build` — passes when `rv6` compiles for the bare-metal target.
  - `qemu` — boots the kernel in QEMU and greps the serial console for the
    pass marker `OSLINGS:PASS`. The kernel powers off via QEMU's SiFive test
    finisher; a kernel that faults before printing simply times out.

## Layout

```
info.toml                 # ordered exercise registry + markers
rv6/                      # the kernel you build (edit src/ here)
  .cargo/config.toml      #   target + QEMU runner
  kernel.ld               #   linker script (load at 0x8000_0000)
  src/                    #   your working kernel source
oslings-cli/              # the CLI runner (std host binary)
exercises/
  NN_name/
    README.md             #   the Learn phase
    hints.md              #   3 progressive hints
    skeleton/             #   starter files (staged into rv6/src)
    solution/             #   reference answer key (locked until you pass)
.oslings/state.toml       # your progress (gitignored)
```

## Curriculum

Phase 1 (shipped): `00_rust_kernel_basics`, `01_boot`.

Planned: `02_physical_memory`, `03_paging`, `04_processes`,
`05_context_switch`, `06_scheduling`, `07_spinlocks`, `08_semaphores`,
`09_virtual_memory`, `10_filesystem`, `11_devices`.

The reference architecture / answer key is [Octox](https://github.com/o8vm/octox),
an xv6-inspired Unix-like OS in Rust. rv6 is built fresh so every line has a
clear pedagogical purpose, but stays structurally compatible.
