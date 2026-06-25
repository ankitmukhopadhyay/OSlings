# OSlings 🦀⚙️

OSlings is a Rustlings style command line tutor that teaches operating systems
by having you build a tiny RISC V kernel called **rv6**, in Rust, one exercise at
a time. Rust itself is taught in context, right where the OS concept motivates
it, so no separate Rust course is needed.

Every exercise follows the same rhythm:

> **Learn** (read the concept) → **Understand** (read and annotate the code) →
> **Implement** (fill in the `IMPLEMENT` markers until the test passes).

## What the tool does so far

* **An interactive full screen app** (the default when you run `oslings`). It
  opens on a welcome **menu** with an animated rv6 logo (a little crab walks on
  top of some machinery), then everything is one keypress away, with a progress
  bar pinned to the bottom.
* **Page based navigation**: a Lesson page, a Watch page that runs the test and
  re runs it automatically every time you save, a List of all exercises, and
  progressive Hints.
* **A clear split between Part 1 and Part 2** right in the exercise list, so you
  can see at a glance which exercises build the kernel and which boot it.
* **Scriptable subcommands** for non interactive use: `run`, `watch`, `hint`,
  `progress`, `list`, `lesson`, `reset`, `solution`, and `goto`.
* **Two grading modes per exercise**: a build mode (passes when the kernel
  compiles for the bare metal RISC V target) and a qemu mode (boots the kernel
  in QEMU and checks the serial console for the marker `OSLINGS:PASS`).
* **A cumulative kernel**: each exercise's starter code already contains
  everything you finished earlier, plus fresh `IMPLEMENT` markers for the
  current step. By the end you have written a real little kernel.

## Requirements

* Rust **nightly** with the bare metal RISC V target installed (add the target
  to your nightly toolchain with rustup).
* **QEMU** for RISC V (version 7 or newer; tested on 8.2).

The kernel pins nightly automatically through `rv6/rust-toolchain.toml`, while
the CLI itself builds on stable.

## Getting started

Build the CLI once with cargo (inside the CLI crate), then run it from the
project root:

```sh
cargo build
oslings
```

Put the built binary on your PATH, or make `oslings` an alias for it.

## Using the interactive app

The flow is page based:

```
Menu  →  Lesson  →(press n)→  Watch
Watch  →(press p)→  Lesson
from anywhere:   l → List      h → Hints      m → Menu      q → quit
```

Keys:

* `n` : begin (run the test and watch), or when an exercise has passed, advance
  to the next one.
* `p` : go back to the previous page (the lesson).
* `l` : open the exercise list (move with the arrow keys or `j` and `k`, open
  with Enter, go back with `p`).
* `h` : show a hint; press again to reveal the next one.
* `r` : reset the current exercise's starter code and run again.
* arrow keys or `j` and `k`, plus PgUp and PgDn : scroll the page.
* `m` : return to the menu from anywhere.
* `q` or Ctrl C : quit.

While on the Watch page, edit the kernel under `rv6/src` in your own editor and
save. The test re runs on its own; when it passes, press `n` to move on.

## Scriptable subcommands

* `oslings run [exercise]` runs one exercise's test once (defaults to the
  current one).
* `oslings watch` re runs the current exercise on every save, without the full
  screen app.
* `oslings hint [exercise]` reveals the next hint (with options to show every
  hint or to start the hints over).
* `oslings progress` shows how many exercises are complete.
* `oslings list` lists every exercise, grouped by part.
* `oslings lesson [exercise]` renders an exercise lesson in the terminal.
* `oslings reset [exercise]` restores an exercise's starter code.
* `oslings solution [exercise]` shows the reference solution (it unlocks after
  you pass).
* `oslings goto [exercise]` moves the current pointer to another exercise.

## How it is organized

The project contains:

* `info.toml`, the ordered registry of exercises and the pass and fail markers.
* `rv6`, the kernel you build. You edit your working source under `rv6/src`.
* The CLI runner crate, a standard host binary that provides the `oslings`
  command.
* `exercises`, one folder per exercise, each holding a README lesson, three
  progressive hints, a `skeleton` (the starter files staged into `rv6/src`), and
  a `solution` (the reference answer, locked until you pass).
* A gitignored state file that records your progress.

## How grading works

* You edit `rv6/src` in your own editor.
* For a build exercise, the test passes when rv6 compiles for the bare metal
  RISC V target.
* For a qemu exercise, the test boots rv6 in QEMU and looks for `OSLINGS:PASS`
  on the serial console. The kernel powers off through the SiFive test device;
  a kernel that faults before printing simply times out, which is its own clear
  signal.

## Curriculum

The course is organized in parts, and the exercise list shows a divider between
them.

### Part 1 · Build the kernel (complete)

Twelve exercises build the kernel's components, each proven by a small in kernel
self test:

* `00_rust_kernel_basics` : `no_std`, the panic handler, `no_main`, bare metal
  setup.
* `01_boot` : the entry point, the linker script, the boot trampoline, and
  printing over the UART.
* `02_physical_memory` : a physical page allocator (`kalloc`) using an intrusive
  free list; `unsafe` and raw pointers.
* `03_paging` : Sv39 page tables, the `Pte` newtype, bit packing with `const fn`,
  and the `walk` and `mappages` routines (the MMU stays off here).
* `04_processes` : the process control block, a process state enum, a fixed
  process table, and ownership by hand.
* `05_context_switch` : saving and restoring registers in assembly (`swtch`),
  `repr(C)`, and `volatile`.
* `06_scheduling` : a round robin scheduler driven by real context switches;
  traits and iterators.
* `07_spinlocks` : a `SpinLock` built on atomics, with `Send`, `Sync`, and an
  RAII guard.
* `08_semaphores` : a counting semaphore, and the moment the kernel heap turns
  on (a global allocator over `kalloc`), so `Box`, `Vec`, and `Arc` start
  working.
* `09_virtual_memory` : build the kernel page table and turn the MMU on (load
  `satp`, then `sfence.vma`), using a verify before enable approach.
* `10_filesystem` : an in memory filesystem of inodes and directories with a
  `Result` based API; error handling in `no_std`.
* `11_devices` : a real polled UART driver (status flags, transmit, receive),
  tested end to end through loopback.

After Part 1, the operating system is built: every piece exists and is verified.

### Part 2 · Boot it and build a shell (in progress)

Part 2 assembles the pieces into an OS that actually boots and runs, and then
grows a shell with commands. Plain `cargo run` now boots rv6 for real, while
`oslings` still grades each exercise.

* `12_boot_to_life` (done) : assemble the real boot sequence (console, page
  allocator, MMU on, process table), print a banner, and idle. From here,
  `cargo run` boots rv6.

Planned next:

* `13_traps` : supervisor trap handling (the trap vector, the trap frame,
  decoding the cause), so the kernel can take exceptions.
* `14_interrupts` : timer interrupts and a preemptive scheduler.
* `15_console` : interrupt driven keyboard input through the PLIC, with a
  blocking read of a line. The OS can read what you type.
* `16_shell` : a read and run loop, with the first commands `pwd`, `ls`, `cd`,
  and `mkdir`.
* `17_file_commands` : `touch`, `cat`, writing into a file with `echo`, `rm`, and
  `rmdir`, giving a usable interactive shell over the in memory filesystem.

Then the user space arc:

* `18_user_mode` : run programs in user mode; the trampoline, the trap frame, the
  `ecall` path, and copying data safely across the boundary.
* `19_exec` : load and run a user program in a fresh address space.
* `20_syscalls_fs` : `fork`, `wait`, `exit`, and a per process file descriptor
  table over the filesystem.
* `21_userland` : a user mode shell and commands (`ls`, `cat`, `echo`, and more)
  running as real user programs that talk to the kernel through system calls.

After Part 2, rv6 is bootable and runnable, with a shell and commands.

### Part 3 · Persistence (future)

A virtio block driver and an on disk filesystem (superblock, bitmaps, on disk
inodes, a buffer cache, and a write ahead log), plus a host tool that builds the
disk image, so files survive a reboot.

Other ideas for later: pipes, more commands such as `grep` and `wc`, demand
paging, copy on write fork, a compact heap, and support for multiple CPUs.

## Reference

The architectural reference, used as an answer key for structure and
correctness and never copied line by line, is Octox, an xv6 inspired Unix like
operating system written in Rust. rv6 is written fresh so every line has a clear
teaching purpose, while staying structurally compatible.
