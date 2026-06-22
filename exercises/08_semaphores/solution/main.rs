#![no_std]
#![no_main]

// ╔══════════════════════════════════════════════════════════════════════╗
// ║  Exercise 08 — Semaphores                                              ║
// ║  Goal: write a counting Semaphore, and meet the heap + Arc.            ║
// ╚══════════════════════════════════════════════════════════════════════╝
//
// Two things come together here:
//   1. The kernel heap turns on (see kheap.rs), so `alloc` types — Box, Vec,
//      and especially Arc — work for the first time.
//   2. You implement a counting Semaphore (semaphore.rs), built on your ex07
//      spinlock, and the test shares ONE semaphore between several owners with
//      Arc. The work is in semaphore.rs; this file is the test harness.

// Turn on the `alloc` crate. This is allowed in `no_std` once a #[global_allocator]
// exists — ours is registered in kheap.rs.
extern crate alloc;

mod entry;
mod kalloc;
mod kheap;
mod semaphore;
mod testdev;
mod uart;
// Carried from earlier exercises; not all of their API is used here.
#[allow(dead_code)]
mod memlayout;
#[allow(dead_code)]
mod param;
#[allow(dead_code)]
mod proc;
#[allow(dead_code)]
mod sched;
#[allow(dead_code)]
mod spinlock;
#[allow(dead_code)]
mod swtch;
#[allow(dead_code)]
mod vm;

use alloc::sync::Arc;
use core::panic::PanicInfo;
use semaphore::Semaphore;

#[no_mangle]
pub extern "C" fn kmain() -> ! {
    uart::puts("\nrv6 booting (exercise 08: semaphores)...\n");
    // The heap is layered on the page allocator, so kalloc must be ready first.
    unsafe {
        kalloc::init();
    }
    if run_checks() {
        uart::puts("OSLINGS:PASS\n");
    } else {
        uart::puts("OSLINGS:FAIL\n");
    }
    testdev::exit_success();
}

fn run_checks() -> bool {
    // Arc<T> = Atomically Reference-Counted shared ownership. `Arc::new` heap-
    // allocates the value; cloning makes another owner of the SAME value.
    let sem = Arc::new(Semaphore::new(2)); // a semaphore with 2 permits
    if Arc::strong_count(&sem) != 1 {
        uart::puts("  [fail] a fresh Arc should have exactly 1 owner\n");
        return false;
    }

    let sem2 = Arc::clone(&sem); // a second owner of the same semaphore
    if Arc::strong_count(&sem) != 2 {
        uart::puts("  [fail] cloning an Arc should make 2 owners\n");
        return false;
    }
    if sem.available() != 2 {
        uart::puts("  [fail] semaphore should start with 2 permits\n");
        return false;
    }

    // Take both permits — through *different* clones of the same semaphore.
    if !sem.try_wait() {
        uart::puts("  [fail] first permit should be available\n");
        return false;
    }
    if !sem2.try_wait() {
        uart::puts("  [fail] second permit should be available\n");
        return false;
    }

    // Now empty: another wait must fail.
    if sem.try_wait() {
        uart::puts("  [fail] no permits should remain after taking both\n");
        return false;
    }
    if sem.available() != 0 {
        uart::puts("  [fail] available count should be 0\n");
        return false;
    }

    // Release one through a clone; the change is visible through the other.
    sem2.post();
    if sem.available() != 1 {
        uart::puts("  [fail] post() via one clone not visible via the other\n");
        return false;
    }
    if !sem.try_wait() {
        uart::puts("  [fail] the released permit should be takeable\n");
        return false;
    }

    // Dropping one owner drops a reference, not the shared semaphore.
    drop(sem2);
    if Arc::strong_count(&sem) != 1 {
        uart::puts("  [fail] dropping a clone should leave 1 owner\n");
        return false;
    }

    uart::puts("  [ok] heap + Arc sharing + semaphore counting all correct\n");
    true
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    uart::puts("OSLINGS:FAIL (panic)\n");
    testdev::exit_failure(1);
}
