# 06 · Scheduling

> **Learn → Understand → Implement.** You'll write the round-robin policy that
> decides which process runs next, and watch it drive a real scheduler that
> `swtch`-es between processes. You meet Rust **traits** and **iterators**.

## Learn

We can now switch between execution contexts (exercise 05) and we have a table
of processes (exercise 04). This exercise combines them into a **scheduler**:
the part of the kernel that shares one CPU among many processes by repeatedly
choosing one to run, running it for a while, then choosing again.

### Mechanism vs. policy

It helps to separate two ideas:

- **Mechanism** — *how* you switch from one process to another. That's `swtch`
  (exercise 05): save the old registers, load the new ones.
- **Policy** — *which* process to run next. That's what you write here.

Keeping these apart is good design: you can change the policy (round-robin
today, priorities tomorrow) without touching the delicate assembly mechanism.

### Cooperative scheduling and "yielding"

There are two ways a process can give up the CPU:

- **Preemptive** — a timer interrupt forcibly pauses it (a later exercise).
- **Cooperative** — the process voluntarily calls into the kernel to give up the
  CPU. That voluntary hand-back is called **yielding**.

This exercise is cooperative. The scheduler `swtch`-es *into* a process; the
process does a little work and then `swtch`-es back to the scheduler (yields);
the scheduler picks the next process and `swtch`-es into that one; and so on.

A neat thing to notice in the test's `task`: its local variables (which process
it is, how many turns it has taken) survive across every yield. That's not
magic — it's exactly what `swtch` guarantees by saving and restoring the
context. Pausing and resuming a computation *is* the whole point.

### Round-robin

The simplest fair policy is **round-robin**: keep the processes in a circle and
give each Runnable one a turn, then come back around to the start. No process is
**starved** (left waiting forever) because the rotation always returns to it.
Contrast with a naive "always run the first Runnable process" policy, which
would run one process to completion before ever touching the next — not fair.

Your `pick_next` must:
1. start scanning where it left off last time (not always from 0 — that's the
   "rotation"),
2. skip slots that aren't `Runnable` (e.g. a `Sleeping` process),
3. wrap around the end of the table,
4. return the chosen index (and remember it for next time), or `None` if nothing
   is Runnable.

The test sets up four slots — three Runnable (pids 1, 3, 4) and one Sleeping
(pid 2) — and checks the processes actually run **interleaved**:
`1, 3, 4, 1, 3, 4, 1, 3, 4`. A non-round-robin policy would produce a different
order (e.g. `1, 1, 1, 3, 3, 3, 4, 4, 4`), so the order is a real test of fairness.

### The Rust you need

**Traits.** A *trait* is a named set of methods a type promises to implement —
like an interface in other languages:

```rust
pub trait Scheduler {
    fn pick_next(&mut self, states: &[ProcState]) -> Option<usize>;
}
```

`RoundRobin` provides that method via `impl Scheduler for RoundRobin { ... }`.
The scheduler loop is written against the *trait*, so any policy that implements
`Scheduler` can be dropped in unchanged. (That's why the loop says
`sched.pick_next(...)` — it doesn't care *which* policy `sched` is.) `&mut self`
lets the policy remember state between calls — here, where the rotation left off.

**Iterators.** An *iterator* produces a sequence of values lazily, and you
transform it with adapter methods instead of writing index loops. The pieces
you'll want:

- `(0..n)` — a range, an iterator over `0, 1, ..., n-1`.
- `.map(|off| ...)` — transform each value (turn an offset into a wrapped
  index `(self.next + off) % n`).
- `.find(|&i| condition)` — return the first item matching a condition, as an
  `Option`, stopping early.
- `.map(|i| ...)` on the resulting `Option` — adjust the found value (record
  `self.next` and return `i`).

Chaining these expresses "scan in this order, take the first Runnable" directly,
with no manual loop or mutable index. `pick_next` returns an `Option<usize>` —
`Some(i)` when a process is chosen, `None` when none can run.

## Understand

Read `rv6/src/sched.rs`: the `Scheduler` trait and the `RoundRobin` struct (note
the `next` field — the rotation cursor). Then read `rv6/src/main.rs`: the `task`
body (records a run, then yields with `swtch`), `run_round_robin` (the loop that
snapshots states, asks the policy, switches in, and re-marks Runnable on yield),
and `run_checks` (sets up the four processes and compares the run order).

Control flow per turn:
```
run_round_robin → pick_next → swtch(sched → proc) → task records + yields →
swtch(proc → sched) → (mark Runnable) → repeat
```

## Implement

In `rv6/src/sched.rs`, fill in `RoundRobin::pick_next`: scan from `self.next`,
wrapping, for the first `Runnable` slot; update `self.next` past it; return its
index (or `None`).

Check your work:

```sh
oslings run 06_scheduling
# or
oslings watch
```

Passes when the processes run in round-robin order and it prints `OSLINGS:PASS`.
If the order is wrong you'll see `[fail] run order is not round-robin`; if
nothing runs, `pick_next` is returning `None`.

Stuck? `oslings hint`.
