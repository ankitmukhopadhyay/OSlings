//! spinlock.rs — a spinlock-based mutex for protecting shared kernel data.
//!
//! A kernel has data many things touch (the process table, the allocator, ...).
//! To keep accesses from interleaving and corrupting that data, we guard it with
//! a **lock**: a flag a CPU must claim before touching the data and release
//! after. A *spin*lock claims it by looping ("spinning") until the flag is free.

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

/// A mutual-exclusion lock wrapping a value of type `T`.
pub struct SpinLock<T> {
    /// true while some CPU holds the lock. An *atomic* so that claiming it is a
    /// single, indivisible step even if two CPUs try at the same instant.
    locked: AtomicBool,
    /// The protected data. `UnsafeCell` is the one type that lets us hand out
    /// `&mut T` through a shared `&self` — "interior mutability". The lock is
    /// what makes that actually safe.
    data: UnsafeCell<T>,
}

// SAFETY: `UnsafeCell` is not `Sync`, so the compiler refuses to share a
// `SpinLock` between threads by default. We promise it *is* safe to share,
// because the lock serializes every access to `data` — only one holder at a
// time ever has a reference. We require `T: Send` because the value can be
// touched (and dropped) by whichever CPU holds the lock. (UNDERSTAND.)
unsafe impl<T: Send> Sync for SpinLock<T> {}

impl<T> SpinLock<T> {
    /// Create a new, unlocked SpinLock. `const` so it can initialize a `static`.
    pub const fn new(data: T) -> SpinLock<T> {
        SpinLock {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }

    /// Acquire the lock, spinning until it's ours. Returns a guard that gives
    /// access to the data and releases the lock when dropped.
    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        // IMPLEMENT: spin until we win the lock.
        //   Atomically try to flip `locked` from false to true:
        //       self.locked
        //           .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        //   compare_exchange returns Ok if WE made the change (we now own the
        //   lock) and Err if someone else already holds it. Loop while it's Err,
        //   calling `core::hint::spin_loop()` each time, until it's Ok.
        //   Then return: SpinLockGuard { lock: self }
        //
        // (Placeholder below compiles but never actually acquires — replace it.)
        SpinLockGuard { lock: self }
    }

    /// Try to acquire the lock without spinning. Returns `Some(guard)` if it was
    /// free, or `None` if it's already held.
    pub fn try_lock(&self) -> Option<SpinLockGuard<'_, T>> {
        // IMPLEMENT: a single attempt.
        //   If compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        //   is Ok, return Some(SpinLockGuard { lock: self }); otherwise None.
        None
    }

    /// Release the lock. Called automatically when a guard is dropped.
    /// (UNDERSTAND — given.)
    fn unlock(&self) {
        // `Release` pairs with the `Acquire` in lock(): everything we wrote
        // while holding the lock becomes visible to the next CPU that acquires.
        self.locked.store(false, Ordering::Release);
    }

    /// Whether the lock is currently held. (UNDERSTAND — given.)
    pub fn is_locked(&self) -> bool {
        self.locked.load(Ordering::Relaxed)
    }
}

/// The handle returned by `lock`/`try_lock`. While it exists, you hold the lock;
/// when it's dropped, the lock is released. Access the data by dereferencing it
/// (`*guard`). (UNDERSTAND — given.)
pub struct SpinLockGuard<'a, T> {
    lock: &'a SpinLock<T>,
}

impl<T> Deref for SpinLockGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        // SAFETY: holding the guard means we hold the lock, so no one else has
        // a reference to the data right now.
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> DerefMut for SpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: as above; exclusive access is guaranteed by the lock.
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T> Drop for SpinLockGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.unlock();
    }
}
