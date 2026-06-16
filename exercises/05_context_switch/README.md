# 05 · Context Switch

> **Learn → Understand → Implement.** You'll write `swtch`, the assembly routine
> that pauses one running context and resumes another — the mechanism behind all
> multitasking. You meet `global_asm!`, `#[repr(C)]`, and `volatile`.

## Learn

We have processes (exercise 04), but only one thing has ever run at a time:
`kmain`, start to finish. To run *many* processes on *one* CPU, the kernel must
be able to **pause** whatever is running and **resume** something else — then
later switch back. That operation is a **context switch**, and this exercise
builds its core.

### What is a "context"?

A CPU is, at any instant, defined by the contents of its **registers** — small
named storage slots inside the chip (we met them in exercise 01). The most
important for "where am I and what am I doing" are:

- **`ra`** (return address) — where the current function will jump when it
  returns.
- **`sp`** (stack pointer) — the current stack.
- **`s0`–`s11`** — long-lived working values the compiler keeps in registers.

If you **save** these registers to memory, you've captured a snapshot of "this
execution, frozen." If you later **load** them back, execution continues exactly
where it left off. That snapshot is the **context**, and saving one set while
loading another *is* the context switch.

### Callee-saved vs caller-saved (why only those registers)

The RISC-V **calling convention** (the ABI — the agreed rules for how functions
call each other) splits registers into two groups:

- **caller-saved** (temporaries like `t0`, `a0`…): a function is free to clobber
  these, so the *caller* saves any it still needs before making a call.
- **callee-saved** (`ra`, `sp`, `s0`–`s11`): a function must leave these exactly
  as it found them, so a *callee* saves/restores any it uses.

`swtch` is itself a normal function call. So when the compiler emits the call to
`swtch`, it has *already* spilled any caller-saved registers it cared about to
the stack. That means `swtch` only needs to preserve the **callee-saved** set —
14 registers. Saving fewer would lose state; saving all of them would be wasted
work. This is why the `Context` struct has exactly those 14 fields.

### The `Context` struct and `#[repr(C)]`

```rust
#[repr(C)]
pub struct Context { pub ra: usize, pub sp: usize, pub s0: usize, /* ...s11 */ }
```

Our assembly reaches each field by a fixed byte offset: `ra` at 0, `sp` at 8,
`s0` at 16, and so on (each `usize` is 8 bytes). For that to be safe, the fields
must sit in memory in *exactly* the declared order. Plain Rust does **not**
promise that — it may reorder struct fields. **`#[repr(C)]`** forces the
predictable C layout, locking the offsets. Forgetting it would let the compiler
silently rearrange fields and your offsets would point at the wrong registers.

### What `swtch(old, new)` does

```
swtch(old, new):
    save  ra, sp, s0..s11  →  *old      # freeze the current context
    load  ra, sp, s0..s11  ←  *new      # thaw the target context
    ret                                  # `ret` jumps to the NEW ra
```

The trick is the final `ret`. `ret` jumps to whatever is in `ra` — and we just
loaded `ra` from `new`. So `swtch` "returns" not to its caller, but into
wherever `new` was last paused. Control reappears in the original caller only
when some other context later switches back into `old`.

Arguments arrive in registers `a0` (old) and `a1` (new) — again, the ABI.

### Bootstrapping a brand-new context

A context that has never run has nothing saved. To start a fresh task we hand-
build its context: set `ra` to the task's entry function (so the `ret` jumps
there) and `sp` to the top of a fresh stack (stacks grow downward, so we point
at the high end). That's all `init_context` does — read it.

### Why this exercise uses `volatile`

The test has a task set a flag, then we read that flag back in `kmain` after the
switch. Because the flag is changed by code reached through a hand-written
assembly switch — a path the optimizer can't follow — we read and write it with
`ptr::read_volatile` / `write_volatile`. **`volatile`** tells the compiler "this
memory really changes; don't cache it in a register or optimize the access away."
(We first met volatile for device registers in exercise 01; here the reason is a
control-flow the compiler can't see, rather than hardware.)

### The Rust you need

- **`global_asm!`** — defines a whole assembly function at global scope (as
  opposed to `asm!`, which injects assembly *inside* a Rust function). `swtch`
  must not have a normal compiler-generated prologue/epilogue messing with `sp`,
  so we write the entire routine in assembly and expose its label.
- **`extern "C" { pub fn swtch(...); }`** — declares the assembly symbol to Rust
  with the C ABI so we can call it. Calling it is `unsafe`.
- **`task_entry as usize`** — turns a function into its address, to store in
  `ra`.
- **`#[repr(C)]`, `volatile`, raw pointers / `addr_of_mut!`** — as above and as
  in earlier exercises.

## Understand

Read `rv6/src/swtch.rs`: the `Context` struct (and why `#[repr(C)]`), the
`extern` declaration, and the given `init_context`. Then read `rv6/src/main.rs`:
two static contexts (`SCHED_CTX`, `TASK_CTX`), a task that sets `TASK_RAN` and
switches back, and `run_checks`, which allocates a stack, bootstraps the task
context, switches into it, and verifies it ran.

## Implement

In `rv6/src/swtch.rs`, fill in the `global_asm!` body of `swtch`:

1. Store `ra`, `sp`, `s0`–`s11` into the OLD context (`a0`) at offsets
   `0, 8, 16, …, 104` using `sd`.
2. Load the same registers from the NEW context (`a1`) at the same offsets using
   `ld`.
3. `ret`.

Check your work:

```sh
oslings run 05_context_switch
# or
oslings watch
```

Passes when the switch round-trips and prints `OSLINGS:PASS`. If `swtch` doesn't
switch, the task never runs and you'll see `[fail] task never ran`; if offsets
are wrong the kernel usually faults and the run times out — both point you back
at the save/load block.

Stuck? `oslings hint`.
