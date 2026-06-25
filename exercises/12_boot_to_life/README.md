# 12 · Boot to life  ·  Part 2 begins

> **Welcome to Part 2.** In Part 1 you *built* the kernel's parts, each proven by
> a self-test. Now you *assemble* them into an OS that actually **boots** — and
> from here on, `cargo run` launches rv6 for real.

## Learn

Every exercise in Part 1 ended the same way: a little self-test printed
`OSLINGS:PASS` and powered the machine off. Nothing ever *ran* as an operating
system. Part 2 changes that. This first exercise wires the subsystems you built
into a real **boot sequence**, so the machine comes up, initializes itself, and
keeps running.

### What "booting" means here

When QEMU starts, control reaches your `_entry` (exercise 01), which sets up a
stack and calls `kmain`. Booting is what `kmain` does next: bring each subsystem
online, in an order where every step is ready for the next, until the kernel is
a working environment. After that, a real OS would start doing useful work
(running a shell — coming in later exercises). For now it boots and idles.

### The boot order (and why it matters)

`kinit` brings up four things, and **the order is not arbitrary** — each depends
on the previous:

1. **`uart::init()`** — the console first, so everything afterward can print.
2. **`kalloc::init()`** — the physical page allocator. Nothing that needs memory
   can run before this: not the page tables, not the heap.
3. **`vm::kvminithart(vm::kvmmake())`** — build the kernel page table and **turn
   the MMU on**. `kvmmake` *allocates* its page-table pages with `kalloc`, which
   is exactly why `kalloc::init()` had to come first. (Turning the MMU on with a
   broken or empty page table faults instantly and hangs the kernel — so the
   order here is load-bearing.)
4. **`proc::init()`** — the process table, ready to hold processes.

This is the same dependency reasoning real kernels use: console → memory →
virtual memory → processes → devices → user space.

### Two ways the kernel runs: harness vs. real

Here's a new idea that runs through all of Part 2. The *same* kernel code can
boot in two modes, chosen at compile time with a **cargo feature**:

- **Graded mode** — `oslings` builds with `--features harness`. After `kinit`,
  the kernel runs a small **boot self-check** (is the allocator up? is the MMU
  on? is the process table ready?), prints `OSLINGS:PASS`, and powers off. This
  keeps the automated grading exactly like Part 1.
- **Real mode** — plain `cargo run` (no feature). After `kinit`, the kernel
  prints its banner and **idles** (a `wfi` loop). This is the OS actually
  running; as Part 2 continues, this path grows a console you can type into and
  a shell.

In code, this is `#[cfg(feature = "harness")]` — *conditional compilation*: the
compiler includes one block or the other depending on the feature. So **try it
both ways**: `oslings run 12` to grade, and `cargo run` (from `rv6/`) to watch
your kernel boot.

### The Rust you need

- **`#[cfg(feature = "...")]`** — compile a piece of code only when a cargo
  feature is enabled. Defined in `rv6/Cargo.toml` under `[features]`.
- **Module wiring** — `kinit` just calls the public `init` functions of the
  modules you already wrote; this exercise is about *composition*, not new
  algorithms.
- **Inline asm you'll see**: `wfi` ("wait for interrupt" — idle the CPU until
  something happens) in the idle loop, and `csrr {}, satp` (read a control
  register) in the self-check to confirm the MMU is on.

## Understand

Read `rv6/src/main.rs`: the module list, `kinit` (what you implement), `kmain`
(prints the banner, then splits into the harness vs. real paths via `#[cfg]`),
and `boot_self_check` (the graded checks). Notice the carried Part 1 modules are
all here — this *is* the whole kernel now.

## Implement

In `rv6/src/main.rs`, fill in **`kinit`** with the four init calls, in order:
`uart::init()`, `kalloc::init()`, `vm::kvminithart(vm::kvmmake())`,
`proc::init()`.

Check your work:

```sh
oslings run 12_boot_to_life      # graded: boots, self-checks, OSLINGS:PASS
# or
oslings watch
```

And see it boot for real:

```sh
cd rv6 && cargo run              # boots rv6, prints the banner, idles
#                                  (exit QEMU with Ctrl-A then X)
```

It passes when the booted kernel's self-check confirms the console, allocator,
MMU, and process table are all up. A failing check names exactly which subsystem
isn't ready — usually a missing or out-of-order `kinit` call.

Stuck? `oslings hint`.
