# 14 · Interrupts

> **Learn → Understand → Implement.** You'll turn on **timer interrupts** and
> handle them. A timer that fires on its own, interrupting whatever is running,
> is the mechanism that lets an OS preempt tasks.

## Learn

Exercise 13 handled an **exception**: something the running instruction itself
caused (a breakpoint), handled synchronously. This exercise handles an
**interrupt**: an event that arrives *asynchronously*, from outside the
instruction stream, while other code runs. The classic one is the **timer**.

### Why the timer matters

Without interrupts, the kernel only regains control when a program voluntarily
calls into it. A misbehaving or busy task could then hog the CPU forever. A
periodic timer interrupt fixes that: every so often the hardware forcibly stops
whatever is running and jumps into the kernel. That is exactly what makes
**preemptive multitasking** possible — the kernel can take the CPU back and
switch to another task whenever a tick arrives. This exercise gets those ticks
flowing and handled.

### How the timer reaches us (the path is given)

The timer hardware on this machine (the CLINT) only speaks **machine mode**, but
our kernel runs in **supervisor mode**. So `start.rs` (given) sets up a tiny
machine-mode handler, `timervec`, that on each timer interrupt reschedules the
next one and then **forwards** the tick to supervisor mode by raising a
*supervisor software interrupt*. Your kernel sees that software interrupt and
treats it as a timer tick. You don't have to write that machinery; you do have
to *receive* it.

### Telling an interrupt from an exception

Both arrive through the same trap path (`kernelvec` → `kerneltrap`) you built in
exercise 13. They're distinguished by the top bit of `scause`:

* top bit `1` → an **interrupt** (asynchronous). The low bits say which one; our
  forwarded timer is a *supervisor software interrupt*, cause code `1`.
* top bit `0` → an **exception** (synchronous), like the breakpoint from before.

### Two things you must do

1. **Enable interrupts.** Even with the timer firing, supervisor mode ignores
   interrupts until you switch them on. Two bits:
   * `sie.SSIE` (bit 1) — allow the supervisor *software* interrupt source (the
     line our forwarded timer uses).
   * `sstatus.SIE` (bit 1) — the global "interrupts on" switch for supervisor
     mode.

   You set bits in a CSR with the `csrs` instruction.

2. **Clear the pending bit when you handle the tick.** The forwarded timer sets
   `sip.SSIP` (bit 1 of the `sip` register). If your handler returns without
   clearing it, the interrupt is *still pending*, so `sret` immediately traps
   again — an **interrupt storm** that locks the kernel up. Clearing it after
   handling each tick is essential.

### The Rust you need

* **`csrs` / `csrr` / `csrw`** via `asm!` to set, read, and write the CSRs
  `sie`, `sstatus`, `scause`, and `sip`.
* a **`static mut`** tick counter, bumped on each handled tick.

## Understand

Read `rv6/src/start.rs` (the given timer setup and `timervec`), then
`rv6/src/trap.rs`: the `scause` interrupt-vs-exception split, the given
breakpoint case, and the two spots you fill — `intr_on` and the interrupt branch
of `kerneltrap`. Then `rv6/src/main.rs`: the harness turns interrupts on, then
busy-waits for several ticks (each tick is the timer preempting that very loop)
and checks they arrive at a sensible pace.

## Implement

In `rv6/src/trap.rs`:

1. **`intr_on`** — set `sie.SSIE` and `sstatus.SIE`.
2. **the interrupt branch of `kerneltrap`** — when `scause & 0xff == 1` (the
   forwarded timer), clear `sip.SSIP` and increment `TICKS`.

Check your work:

```sh
oslings run 14_interrupts
# or
oslings watch
```

It passes when timer ticks are firing, handled, and correctly paced. If it
reports **no ticks**, interrupts aren't enabled or the tick isn't counted. If it
reports an **interrupt storm**, your handler isn't clearing the pending bit.

You can also watch it live: `cd rv6 && cargo run` boots the kernel with the
timer running (it idles, waking on each tick).

Stuck? `oslings hint`.
