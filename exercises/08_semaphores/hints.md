# Hints — 08 Semaphores

## Hint 1
Only `try_wait` and `post` need writing, and both are tiny. Each one locks the
count (`self.count.lock()` gives a guard you can read and write through `*`),
changes it, and lets the guard drop (which unlocks).

- `post` always adds one permit.
- `try_wait` removes one permit *only if* there's one to remove; it reports
  whether it succeeded.

If the test says "first permit should be available", your `try_wait` is always
returning `false` (the skeleton placeholder).

## Hint 2
`post`:

```rust
let mut count = self.count.lock();
*count += 1;
```

`try_wait` — take a permit only when one exists:

```rust
let mut count = self.count.lock();
if *count > 0 {
    *count -= 1;
    true
} else {
    false
}
```

The `mut` on the guard is needed because you write through it (`*count -= 1`).

## Hint 3
Full methods:

```rust
pub fn try_wait(&self) -> bool {
    let mut count = self.count.lock();
    if *count > 0 {
        *count -= 1;
        true
    } else {
        false
    }
}

pub fn post(&self) {
    let mut count = self.count.lock();
    *count += 1;
}
```

Why this passes: starting from 2 permits, two `try_wait`s succeed (count → 1 →
0), the next returns `false` (count is 0), a `post` brings it back to 1, and one
more `try_wait` succeeds. Because the count lives in a `SpinLock` shared via
`Arc`, every clone sees the same updates — that's the `Arc<…lock…>` shared-
mutable-state pattern at work.
