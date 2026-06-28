# Hints — 14 Interrupts

## Hint 1
Two pieces in `trap.rs`:

* `intr_on` switches interrupts on. Nothing fires until you do this, so the
  "no ticks" failure usually means `intr_on` is still empty.
* the interrupt branch of `kerneltrap` runs on each tick. It must do two things:
  clear the pending interrupt and count the tick. If you forget to clear it, the
  interrupt fires again the instant you return — that's the "interrupt storm"
  failure.

Use `csrs` to set bits, `csrr` to read, and `csrw` to write a CSR.

## Hint 2
`intr_on` — set the software-interrupt enable and the global enable:

```rust
asm!("csrs sie, {}", in(reg) 1usize << 1);      // SSIE
asm!("csrs sstatus, {}", in(reg) 1usize << 1);  // SIE
```

In `kerneltrap`, the interrupt branch (top bit of `scause` is 1). The forwarded
timer has cause code 1:

```rust
if scause & 0xff == 1 {
    let sip: usize;
    asm!("csrr {}, sip", out(reg) sip);
    asm!("csrw sip, {}", in(reg) sip & !2);  // clear SSIP (bit 1)
    TICKS += 1;
}
```

## Hint 3
Full pieces:

```rust
pub unsafe fn intr_on() {
    asm!("csrs sie, {}", in(reg) 1usize << 1);
    asm!("csrs sstatus, {}", in(reg) 1usize << 1);
}

// inside kerneltrap, the `(scause >> 63) == 1` branch:
if scause & 0xff == 1 {
    let sip: usize;
    asm!("csrr {}, sip", out(reg) sip);
    asm!("csrw sip, {}", in(reg) sip & !2);
    TICKS += 1;
}
```

Why it works: `intr_on` lets supervisor mode actually take the forwarded timer
interrupt. Each time it fires, `kerneltrap` clears `sip.SSIP` (so the next `sret`
doesn't immediately re-trap) and bumps `TICKS`. The machine-mode `timervec`
(given) keeps rescheduling the timer, so ticks keep coming about every tenth of
a second — and the test's busy loop, spinning millions of times between ticks,
confirms they're real and paced rather than a storm.
