# 08 · Semaphores

> **Learn → Understand → Implement.** You'll build a counting semaphore on top
> of your spinlock — and, for the first time, the kernel gets a **heap**, so
> `Box`, `Vec`, and especially **`Arc`** start working.

## Learn

This exercise has two threads that come together: a new synchronization tool
(the semaphore) and a major new capability (dynamic memory).

### What is a semaphore?

A **semaphore** is a counter that hands out a fixed number of **permits**. It
supports two operations (the traditional names are P and V):

- **wait** (P, "acquire") — take a permit; the count goes **down**.
- **post** (V, "release") — return a permit; the count goes **up**.

If the count is `2`, two users can hold permits at once; a third must wait until
someone posts. Semaphores are how a kernel caps access to a finite resource:
"at most N processes in this region," "this many free slots in a buffer," and so
on. A semaphore that only ever holds 0 or 1 permits is effectively a lock (a
"binary semaphore"); one that counts higher is a **counting semaphore**, which
is what we build.

#### Why ours is non-blocking here

In a full kernel, `wait` on an empty semaphore *blocks*: the process goes to
sleep and the scheduler runs someone else, who eventually `post`s and wakes it.
That sleep/wake-up machinery is a later exercise. On our single CPU, a process
that blocked itself with nothing else running would simply hang forever. So this
semaphore's `try_wait` is **non-blocking**: it takes a permit if one is
available and otherwise returns `false`, rather than sleeping. The counting
logic is identical; only the "what to do when empty" part is deferred.

### Built on the spinlock

The permit count is shared mutable state, so it must be protected — we store it
in a `SpinLock<i64>` (your exercise 07 lock). Notice that `try_wait`/`post` take
`&self` (a shared reference) yet change the count: that's **interior
mutability** again, and the lock is what makes it safe. Building the semaphore
*on top of* the lock is a small lesson in composition — higher-level tools made
from lower-level ones.

### The heap: dynamic memory at last

Until now, every object lived either in a `static` or on a stack — sizes fixed
at compile time. A **heap** lets the kernel allocate memory whose size and
lifetime are decided at *run* time. Rust's heap types live in the **`alloc`**
crate: `Box<T>` (one owned value on the heap), `Vec<T>` (a growable array),
`Arc<T>` (shared ownership — below).

`alloc` doesn't know *how* to get raw memory; you must supply an allocator —
a type implementing the **`GlobalAlloc`** trait, registered with
`#[global_allocator]`. Once one exists, `extern crate alloc` lights up and all
those types work. Ours (`kheap.rs`, given) is tiny: it serves each allocation
from one physical page via the `kalloc` you wrote in exercise 02, and frees it
back on drop. (It's wasteful and capped at one page per allocation — fine for
now; a compact heap is a later refinement.) The heap sits *on top of* the page
allocator, so `kalloc::init()` must run before the first allocation — `kmain`
does that.

### `Arc`: shared ownership

Sometimes several parts of the kernel need to own the *same* heap value, and it
should be freed only when the *last* of them is done. That's **`Arc<T>`** —
"Atomically Reference-Counted." It keeps a count of how many owners exist:

- `Arc::new(x)` puts `x` on the heap with a count of 1.
- `Arc::clone(&a)` makes another owner of the *same* value and bumps the count
  (it does **not** copy `x`).
- Dropping an `Arc` lowers the count; when it reaches 0, the value is freed.
- `Arc::strong_count(&a)` reports the current number of owners.

"Atomically" means the count is updated with atomics (like your spinlock flag),
so sharing across CPUs is safe. Crucially, `Arc<T>` only ever gives you a
**shared** `&T` — never `&mut T` — because there could be other owners. So to
*mutate* something shared via `Arc`, the value must have interior mutability
inside it. That's exactly our `Arc<Semaphore>`: the `Arc` shares ownership, and
the `Semaphore`'s internal `SpinLock` allows the count to change through `&self`.
This `Arc<…lock…>` combination is the standard Rust pattern for shared mutable
state, and you'll use it constantly later.

## Understand

Read `rv6/src/kheap.rs` (the `GlobalAlloc` impl and the `#[global_allocator]`),
then `rv6/src/semaphore.rs` (the `SpinLock<i64>` count, and the given
`new`/`available`). Then read `rv6/src/main.rs`: it makes an `Arc<Semaphore>`
with 2 permits, clones it (two owners of one semaphore), takes both permits
through the two clones, checks the semaphore is then empty, posts through one
clone and sees it via the other, and checks `strong_count` as a clone is
dropped.

## Implement

In `rv6/src/semaphore.rs`:

1. **`try_wait`** — lock the count; if it's `> 0`, decrement and return `true`,
   else return `false`.
2. **`post`** — lock the count and increment it.

Check your work:

```sh
oslings run 08_semaphores
# or
oslings watch
```

Passes when the heap is up, `Arc` sharing works, and the semaphore counts
correctly — printing `OSLINGS:PASS`. The failure messages say which property
broke (e.g. `first permit should be available` means `try_wait` isn't handing
out permits).

Stuck? `oslings hint`.
