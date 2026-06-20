# 07 · Spinlocks

> **Learn → Understand → Implement.** You'll build a `SpinLock<T>` — a mutual-
> exclusion lock — using atomics. You meet `Atomic*`, `UnsafeCell`, the RAII
> guard pattern, and the `Send`/`Sync` traits.

## Learn

Up to now the kernel has done one thing at a time. Real kernels don't: multiple
CPUs (and, soon, interrupts) touch the same data — the process table, the
allocator's free list — at overlapping times. When two pieces of code modify the
same data at once, you get a **race condition**, and the data can be corrupted.

### Why a simple flag isn't enough

Imagine guarding data with an ordinary `bool` "busy" flag:

```text
if !busy {      // CPU A reads busy = false
                // CPU B reads busy = false  (at the same instant!)
    busy = true; // both set it to true
    ...          // both think they have exclusive access — corruption
}
```

The read-then-write is two separate steps, and another CPU can slip in between.
We need that "check it's free **and** claim it" to happen as one **indivisible**
step that nothing can interrupt or interleave. That indivisible step is an
**atomic** operation.

### Atomics

An *atomic* type (here `AtomicBool`) supports operations the hardware guarantees
happen all-at-once. The one we need is **compare-and-exchange**:

```rust
locked.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
```

This means, atomically: "if `locked` is currently `false`, set it to `true`."
It returns `Ok(_)` if *we* made that change (we now own the lock) or `Err(_)` if
it was already `true` (someone else holds it). Because it's atomic, exactly one
CPU can ever win the transition from `false` to `true`. That's mutual exclusion.

A **spin**lock simply retries this in a loop — "spinning" — until it wins:

```rust
while locked.compare_exchange(false, true, Acquire, Relaxed).is_err() {
    core::hint::spin_loop(); // hint to the CPU that we're busy-waiting
}
```

#### Memory ordering (the `Ordering` arguments)

Modern CPUs and compilers reorder memory operations for speed. The `Ordering`
argument controls how much reordering is allowed around the atomic, so that
locking actually protects the data:

- Acquire on lock + Release on unlock form a pair: everything written by the
  previous holder (before its Release) is guaranteed visible to the next holder
  (after its Acquire). You don't need to master this yet — just use **Acquire**
  when taking the lock and **Release** when dropping it.

### Interior mutability: `UnsafeCell`

Here's a puzzle: `lock(&self)` takes `&self` (a *shared* reference), yet it must
hand out `&mut T` to let you modify the data. Rust normally forbids getting
`&mut` from `&`. The single escape hatch is **`UnsafeCell<T>`** — the only legal
way to obtain a `*mut T` (and thus `&mut T`) from a shared reference. This is
called **interior mutability**. `UnsafeCell` does no checking itself; it just
unlocks the ability. *We* make it sound by ensuring (via the lock) that only one
`&mut` exists at a time.

A nice consequence: the protected data lives in a plain `static`, **not** a
`static mut`. The lock + `UnsafeCell` give us *safe* shared mutable state, so the
test code needs almost no `unsafe` — unlike the raw `static mut` tables in
exercises 02–06.

### `Send` and `Sync`

These two marker traits are how Rust reasons about thread safety:

- **`Send`** — a value of this type can be *moved* to another thread.
- **`Sync`** — a `&T` can be *shared* with another thread (equivalently, `&T` is
  `Send`).

Most types are automatically `Send`/`Sync`. But `UnsafeCell` is deliberately
**not** `Sync` — because, on its own, sharing it across threads would be a data
race. So `SpinLock<T>` (which contains an `UnsafeCell`) isn't automatically
`Sync` either, and the compiler won't let you share it. We override that with an
explicit promise:

```rust
unsafe impl<T: Send> Sync for SpinLock<T> {}
```

"`unsafe`" because *we* are vouching for what the compiler can't check: the lock
serializes access, so sharing is in fact safe. We require `T: Send` because the
inner value may be touched by whichever CPU holds the lock. This is exactly what
makes `static COUNTER: SpinLock<u64>` legal.

### The RAII guard

`lock()` returns a **guard** (`SpinLockGuard`). While the guard is alive you hold
the lock; you reach the data by dereferencing it (`*guard`). When the guard goes
out of scope, its `Drop` releases the lock automatically. This pattern — tying a
resource's lifetime to a value's scope — is called **RAII**, and it means you can
never forget to unlock.

> Note: real spinlocks also disable interrupts while held and are never held
> while sleeping, to avoid deadlock. We keep this one minimal and run on a single
> CPU, so this exercise verifies the lock's *logic* (acquire, exclude, release,
> protect) rather than true multi-core contention.

## Understand

Read `rv6/src/spinlock.rs`: the `SpinLock<T>` fields (`AtomicBool` + `UnsafeCell`),
the given `new`, `unlock`, `is_locked`, the `unsafe impl Sync`, and the
`SpinLockGuard` with its `Deref`/`DerefMut`/`Drop`. Then read `rv6/src/main.rs`:
a `static COUNTER: SpinLock<u64>` and `run_checks`, which checks `try_lock`
succeeds when free, that the lock excludes a second acquire while held, that it
releases on drop, and that a counter stays correct across many locked updates.

## Implement

In `rv6/src/spinlock.rs`:

1. **`lock`** — spin on `compare_exchange(false, true, Acquire, Relaxed)` until it
   succeeds, then return the guard.
2. **`try_lock`** — one `compare_exchange`; return `Some(guard)` on success, else
   `None`.

Check your work:

```sh
oslings run 07_spinlocks
# or
oslings watch
```

Passes when the lock acquires, excludes, releases, and protects the counter, and
it prints `OSLINGS:PASS`. The failure messages pinpoint which property broke
(e.g. `try_lock succeeded on an already-held lock` means it isn't excluding).

Stuck? `oslings hint`.
