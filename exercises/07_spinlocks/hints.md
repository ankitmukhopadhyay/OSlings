# Hints — 07 Spinlocks

## Hint 1
Both methods are built on one atomic operation:

```rust
self.locked.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
```

It atomically changes `locked` from `false` to `true` and tells you whether
*you* were the one who changed it: `Ok(_)` = you got the lock, `Err(_)` = someone
else already held it.

- `try_lock` makes **one** attempt.
- `lock` keeps attempting in a loop until it succeeds.

When you've won the lock, hand back `SpinLockGuard { lock: self }`.

If the test says "try_lock failed on a free lock", you're returning `None`
unconditionally (the skeleton placeholder). If it says "try_lock succeeded on an
already-held lock", your `lock()` isn't actually setting the flag.

## Hint 2
`try_lock`:

```rust
if self
    .locked
    .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
    .is_ok()
{
    Some(SpinLockGuard { lock: self })
} else {
    None
}
```

`lock` is the same attempt, but spin until it works:

```rust
while self
    .locked
    .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
    .is_err()
{
    core::hint::spin_loop();
}
SpinLockGuard { lock: self }
```

## Hint 3
Full methods:

```rust
pub fn lock(&self) -> SpinLockGuard<'_, T> {
    while self
        .locked
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        core::hint::spin_loop();
    }
    SpinLockGuard { lock: self }
}

pub fn try_lock(&self) -> Option<SpinLockGuard<'_, T>> {
    if self
        .locked
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_ok()
    {
        Some(SpinLockGuard { lock: self })
    } else {
        None
    }
}
```

Why it passes: `compare_exchange` makes "see it's free and claim it" a single
atomic step, so once one guard exists, every other `try_lock` gets `Err` →
`None` (mutual exclusion). Releasing is automatic: the given `Drop` for the guard
calls `unlock`, which stores `false` with `Release` ordering, so the next
acquirer sees all your writes.
