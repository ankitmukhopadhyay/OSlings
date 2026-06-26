# 13 · Traps

> **Learn → Understand → Implement.** You'll catch a CPU **trap** (a breakpoint
> exception), handle it, and resume. This is the gateway to interrupts, system
> calls, and the rest of Part 2.

## Learn

So far our kernel runs straight through: one instruction after another. But a
real OS must react to events that interrupt that flow. The mechanism for that is
a **trap**.

### What a trap is

A **trap** is the CPU pausing the current instruction stream and jumping into the
kernel because something needs handling. There are two kinds:

* **Exceptions** — the running instruction itself caused it: a breakpoint
  (`ebreak`), a bad memory access, a divide error, or a deliberate call into the
  kernel (`ecall`, which we use for system calls later).
* **Interrupts** — something external wants attention: a timer tick or a device.
  (That's the next exercise.)

This exercise handles the simplest exception: a **breakpoint**, caused by the
`ebreak` instruction.

### New here: the kernel now runs in supervisor mode

There are three privilege levels on RISC-V: **machine** (M, most privileged),
**supervisor** (S, where kernels run), and **user** (U, where programs run).
QEMU starts us in machine mode, and up to now rv6 has quietly stayed there.
Starting with this exercise, a small machine-mode routine, `start.rs` (given),
drops the kernel into **supervisor mode** at boot before calling `kmain`. Two
nice consequences: the page table from exercises 09 and 12 now genuinely
translates (machine mode had been ignoring it), and the supervisor trap
registers below (`stvec`, `scause`, `sepc`, and the `sret` instruction) are the
right ones to use. You don't edit `start.rs`, but it is worth a read.

### How the hardware delivers a trap

When a trap happens in supervisor mode, the CPU automatically does three things,
using special **control and status registers (CSRs)**:

* writes *why* into **`scause`** (a number identifying the cause; a breakpoint is
  cause `3`),
* writes *where* into **`sepc`** (the address of the instruction that trapped),
* and jumps to the address held in **`stvec`** (the "supervisor trap vector").

Crucially, the hardware does **not** save the general-purpose registers for you.
It just jumps. So the kernel has to set this up:

```
  trap happens
      │  (hardware sets scause, sepc; jumps to stvec)
      ▼
  kernelvec   (assembly)  save registers  →  call kerneltrap  →  restore  →  sret
                                                    │                          │
                                              (your Rust handler)        resume at sepc
```

* **`stvec`** must point at an assembly entry, **`kernelvec`** (given). Setting
  it is what `trap::init` does.
* **`kernelvec`** saves the registers our handler might clobber, calls the Rust
  handler **`kerneltrap`**, restores the registers, and runs **`sret`** to
  return. Because it saves and restores everything, the trap is invisible to the
  code that was interrupted. (Kernel traps save onto the stack like this;
  user-mode traps will use a structured trap frame in a later exercise.)
* **`kerneltrap`** (your Rust code) decides what to do based on `scause`.

### The one rule you must not forget: advance `sepc`

`sret` returns execution to whatever address is in `sepc` — which is the address
of the instruction that trapped. If your handler returns without changing
`sepc`, `sret` runs that *same* `ebreak` again, which traps again, forever: an
**infinite trap loop** that hangs the machine. So after handling a breakpoint,
the handler must move `sepc` **past** the instruction. The test uses a 4-byte
`ebreak`, so the handler advances `sepc` by 4.

### Reading and writing CSRs from Rust

CSRs aren't normal memory; you access them with special instructions, via inline
assembly:

```rust
let scause: usize;
asm!("csrr {}, scause", out(reg) scause);   // csrr = CSR read
asm!("csrw sepc, {}", in(reg) new_sepc);    // csrw = CSR write
```

### The Rust you need

* **`asm!`** with `csrr` / `csrw` to read and write `scause`, `sepc`, `stvec`.
* **`global_asm!`** defines the `kernelvec` trap vector (given; read it).
* **`#[no_mangle] pub extern "C" fn kerneltrap`** — a function the assembly can
  `call` by name, using the C calling convention.
* a **`static mut`** counter, bumped each time a trap is handled, so the test can
  confirm the handler ran.

## Understand

Read `rv6/src/trap.rs`: the `scause`/`sepc`/`stvec` story in the comments, the
given `kernelvec` assembly (save registers, `call kerneltrap`, restore, `sret`),
the given `vector_addr`/`trap_count`, and the two functions you write:
`init` and `kerneltrap`. Then read `rv6/src/main.rs`: `kinit` now calls
`trap::init()`, and the harness check fires a breakpoint and confirms it was
caught and that execution continued.

## Implement

In `rv6/src/trap.rs`:

1. **`init`** — write `vector_addr()` into `stvec` with `csrw`.
2. **`kerneltrap`** — read `scause`; if it's a breakpoint (`scause == 3`), bump
   `TRAP_COUNT` and advance `sepc` by 4 so `sret` resumes after the `ebreak`.

Check your work:

```sh
oslings run 13_traps
# or
oslings watch
```

It passes when `stvec` is installed and the breakpoint is handled so execution
resumes (the counter goes up by one). If the run **times out**, your handler
isn't advancing `sepc` — the kernel is looping on the same `ebreak`.

Stuck? `oslings hint`.
