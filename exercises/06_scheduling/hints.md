# Hints — 06 Scheduling

## Hint 1
`pick_next` returns the index of the next `Runnable` slot, scanning in
round-robin order. The whole trick is *where you start scanning*: not always at
0, but at `self.next` — the spot just after whatever you picked last time. That
rotation is what makes it round-robin instead of "always pick the first one."

If the test says nothing ran (wrong number of runs), you're returning `None`
(the skeleton's placeholder). If it says the order isn't round-robin, you're
probably scanning from a fixed 0 each time and not advancing `self.next`.

## Hint 2
With `n = states.len()`, the indices to examine, in order, are:

```
(self.next + 0) % n, (self.next + 1) % n, ... , (self.next + n - 1) % n
```

Walk them and return the first whose state is `ProcState::Runnable`. Before you
return index `i`, set `self.next = (i + 1) % n` so the next call continues after
it. If none are Runnable, return `None`.

You can write this with a plain loop, but an iterator chain is cleaner: a range
`(0..n)`, `.map(...)` to turn each offset into a wrapped index, then `.find(...)`
to get the first Runnable.

## Hint 3
Full body:

```rust
fn pick_next(&mut self, states: &[ProcState]) -> Option<usize> {
    let n = states.len();
    (0..n)
        .map(|off| (self.next + off) % n)        // indices to examine, in order
        .find(|&i| states[i] == ProcState::Runnable)
        .map(|i| {
            self.next = (i + 1) % n;             // resume after this one next time
            i
        })
}
```

Why this produces the interleaving `1, 3, 4, 1, 3, 4, ...`: after running the
process in slot `i`, `self.next` points just past it, so the next pick continues
forward and lands on the *next* runnable process rather than the same one. Slot
1 (the Sleeping pid 2) is never `Runnable`, so `.find` skips it automatically.
`.find` returns an `Option`, and the final `.map` both records `self.next` and
unwraps to `Some(i)` — or stays `None` if the table has nothing runnable.
