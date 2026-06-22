//! semaphore.rs — a counting semaphore, built on the spinlock from exercise 07.
//!
//! A **semaphore** hands out a limited number of *permits*. It holds a count:
//!   - "wait" (a.k.a. P, acquire) takes a permit — the count goes down;
//!   - "post" (a.k.a. V, release) returns a permit — the count goes up.
//! When no permits are left, a waiter can't proceed. Semaphores are how kernels
//! limit access to a finite resource (e.g. "at most N users of this buffer").

use crate::spinlock::SpinLock;

/// A counting semaphore. The permit count lives inside a `SpinLock`, so every
/// change to it is atomic with respect to other CPUs — note we mutate it
/// through `&self` (shared reference): that's *interior mutability*, provided by
/// the lock.
pub struct Semaphore {
    count: SpinLock<i64>,
}

impl Semaphore {
    /// A semaphore that starts with `permits` permits available.
    pub fn new(permits: i64) -> Semaphore {
        Semaphore {
            count: SpinLock::new(permits),
        }
    }

    /// Try to take one permit. Returns true if one was available (and consumes
    /// it), or false if none are left. (Non-blocking — see the README note on
    /// why we don't block on a single CPU.)
    pub fn try_wait(&self) -> bool {
        // IMPLEMENT:
        //   Lock the count. If it is greater than 0, decrement it and return
        //   true (we took a permit). Otherwise return false (none available).
        //
        //   let mut count = self.count.lock();
        //   if *count > 0 { *count -= 1; true } else { false }
        false
    }

    /// Return one permit, making it available to a future `try_wait`.
    pub fn post(&self) {
        // IMPLEMENT:
        //   Lock the count and increment it by one.
        //
        //   let mut count = self.count.lock();
        //   *count += 1;
    }

    /// How many permits are currently available. (UNDERSTAND — given.)
    pub fn available(&self) -> i64 {
        *self.count.lock()
    }
}
