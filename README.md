# OSlings 🦀⚙️

OSlings is a [Rustlings](https://github.com/rust-lang/rustlings)-style CLI that
teaches operating-system concepts by having you build a tiny RISC-V kernel
called **rv6**, in Rust, one exercise at a time. Rust itself is taught in
context, right where the OS concept motivates it, so no separate Rust course is
needed.

Every exercise follows the same rhythm:

> **Learn** (read the concept) → **Understand** (read and annotate the code) →
> **Implement** (fill in the `IMPLEMENT` markers until the test passes).

## What the tool does so far

* **An interactive full-screen app** (the default when you run `oslings`). It
  opens on a welcome **menu** with an animated rv6 logo (a little crab walks on
  top of some machinery), then everything is one keypress away, with a progress
  bar pinned to the bottom.
* **Page-based navigation**: a Lesson page, a Watch page that runs the test and
  re-runs it automatically every time you save, a List of all exercises, and
  progressive Hints.
* **A clear split between Part 1 and Part 2** right in the exercise list, so you
  can see at a glance which exercises build the kernel and which boot it.
* **Scriptable subcommands** for non-interactive use.
* **Two grading modes per exercise**: a build mode (passes when the kernel
  compiles for the bare-metal target) and a qemu mode (boots the kernel in QEMU
  and checks the serial console for the marker `OSLINGS:PASS`).
* **A cumulative kernel**: each exercise's starter code already contains
  everything you finished earlier, plus fresh `IMPLEMENT` markers for the
  current step. By the end you have written a real little kernel.

## Requirements

* Rust **nightly** with the `riscv64gc-unknown-none-elf` target:
  `rustup target add riscv64gc-unknown-none-elf --toolchain nightly`
* `qemu-system-riscv64` (QEMU 7 or newer; tested on 8.2).

The kernel pins nightly automatically through `rv6/rust-toolchain.toml`, while
the CLI itself builds on stable.

## Getting started

```sh
# build the CLI once
cargo build --manifest-path oslings-cli/Cargo.toml

# then run it from the project root
oslings
```

Put `oslings-cli/target/debug/` on your PATH, or alias
`oslings=./oslings-cli/target/debug/oslings`.

## Using the interactive app

The flow is page-based:

```
            Continue
   Menu ───────────────▶ Lesson ─────(n)─────▶ Watch
     ▲                      ▲  ◀─────(p)─────── │
     │ m                    │                    │  edit rv6/src and save
     │                      └──── auto re-runs ──┘  (Watch re-runs the test)
     └──────  l: List      h: Hints      m: Menu      q: Quit   (from anywhere)
```

| Key | Where | Action |
|---|---|---|
| `n` | Lesson | Begin: run the test and watch for saves |
| `n` | Watch (passed) | Advance to the next exercise |
| `p` | Watch / overlays | Back to the previous page (the lesson) |
| `l` | anywhere | Open the exercise list (`↑`/`↓` move, Enter open, `p` back) |
| `h` | Lesson / Watch | Show a hint; press again for the next one |
| `r` | Watch | Reset the exercise's starter code and run again |
| `m` | anywhere | Return to the menu |
| `↑`/`↓`, `j`/`k`, PgUp/PgDn | content pages | Scroll |
| `q` / Ctrl-C | anywhere | Quit |

While on the Watch page, edit the kernel under `rv6/src` in your own editor and
save. The test re-runs on its own; when it passes, press `n` to move on.

## Scriptable subcommands

The same actions are available without the full-screen app:

| Command | What it does |
|---|---|
| `oslings run [exercise]` | Run one exercise's test once (defaults to current). |
| `oslings watch` | Re-run the current exercise on every save, headless. |
| `oslings hint [exercise]` | Reveal the next hint (`--all`, `--reset`). |
| `oslings progress` | Show how many exercises are complete. |
| `oslings list` | List every exercise, grouped by part. |
| `oslings lesson [exercise]` | Render an exercise lesson in the terminal. |
| `oslings reset [exercise]` | Restore an exercise's starter code. |
| `oslings solution [exercise]` | Show the reference solution (unlocks after you pass). |
| `oslings goto [exercise]` | Move the current pointer to another exercise. |

## How it is organized

```
info.toml                 ordered exercise registry and markers
rv6/                      the kernel you build (edit src here)
  .cargo/config.toml        target and QEMU runner
  kernel.ld                 linker script (loads at 0x8000_0000)
  src/                      your working kernel source
oslings-cli/              the CLI runner (a std host binary)
exercises/
  NN_name/
    README.md               the Learn lesson
    hints.md                three progressive hints
    skeleton/               starter files staged into rv6/src
    solution/               reference answer (locked until you pass)
.oslings/state.toml       your progress (gitignored)
```

## How grading works

* You edit `rv6/src` in your own editor.
* For a build exercise, the test passes when rv6 compiles for the bare-metal
  `riscv64gc-unknown-none-elf` target.
* For a qemu exercise, the test boots rv6 in QEMU and looks for `OSLINGS:PASS`
  on the serial console. The kernel powers off through the SiFive test device;
  a kernel that faults before printing simply times out, which is its own clear
  signal.

## Curriculum

The course is organized in parts, and the exercise list shows a divider between
them.

| Part | Goal | Status |
|---|---|---|
| Part 1 | Build the kernel (an OS is built) | Complete |
| Part 2 | Boot it and build a shell (bootable and runnable) | In progress |
| Part 3 | Persistence (files survive a reboot) | Future |

### Part 1 · Build the kernel (complete)

Twelve exercises build the kernel's components, each proven by a small in-kernel
self-test:

| Exercise | What you build |
|---|---|
| `00_rust_kernel_basics` | `no_std`, the panic handler, `no_main`, bare-metal setup |
| `01_boot` | the entry point, linker script, boot trampoline, UART printing |
| `02_physical_memory` | a page allocator (`kalloc`) with an intrusive free list; `unsafe`, raw pointers |
| `03_paging` | Sv39 page tables, the `Pte` newtype, bit-packing with `const fn`, `walk` and `mappages` |
| `04_processes` | the process control block, a state enum, a fixed process table, ownership by hand |
| `05_context_switch` | saving and restoring registers in assembly (`swtch`), `repr(C)`, `volatile` |
| `06_scheduling` | a round-robin scheduler driven by real context switches; traits and iterators |
| `07_spinlocks` | a `SpinLock` built on atomics, with `Send`, `Sync`, and an RAII guard |
| `08_semaphores` | a counting semaphore, and the kernel heap turns on (`Box`, `Vec`, `Arc` start working) |
| `09_virtual_memory` | build the kernel page table and turn the MMU on (`satp`, `sfence.vma`) |
| `10_filesystem` | an in-memory filesystem of inodes and directories; `Result` in `no_std` |
| `11_devices` | a real polled UART driver (status flags, transmit, receive), tested via loopback |

After Part 1, the operating system is built: every piece exists and is verified.

### Part 2 · Boot it and build a shell (in progress)

Part 2 assembles the pieces into an OS that actually boots and runs, then grows a
shell with commands. Plain `cargo run` now boots rv6 for real, while `oslings`
still grades each exercise.

| Exercise | What it adds | Status |
|---|---|---|
| `12_boot_to_life` | assemble the real boot sequence; `cargo run` boots rv6 | Done |
| `13_traps` | supervisor trap handling, so the kernel can take exceptions | Planned |
| `14_interrupts` | timer interrupts and a preemptive scheduler | Planned |
| `15_console` | interrupt-driven keyboard input via the PLIC; read a line | Planned |
| `16_shell` | a read-and-run loop with `pwd`, `ls`, `cd`, `mkdir` | Planned |
| `17_file_commands` | `touch`, `cat`, `echo` into a file, `rm`, `rmdir` | Planned |
| `18_user_mode` | run programs in user mode; the trampoline, trap frame, `ecall` | Planned |
| `19_exec` | load and run a user program in a fresh address space | Planned |
| `20_syscalls_fs` | `fork`, `wait`, `exit`, and a per-process file descriptor table | Planned |
| `21_userland` | a user-mode shell and commands running as real user programs | Planned |

After Part 2, rv6 is bootable and runnable, with a shell and commands.

### Part 3 · Persistence (future)

A virtio block driver and an on-disk filesystem (superblock, bitmaps, on-disk
inodes, a buffer cache, and a write-ahead log), plus a host tool that builds the
disk image, so files survive a reboot. Other ideas for later: pipes, more
commands such as `grep` and `wc`, demand paging, copy-on-write fork, a compact
heap, and support for multiple CPUs.

## Reference

The architectural reference, used as an answer key for structure and correctness
and never copied line by line, is [Octox](https://github.com/o8vm/octox), an
xv6-inspired Unix-like operating system written in Rust. rv6 is written fresh so
every line has a clear teaching purpose, while staying structurally compatible.
