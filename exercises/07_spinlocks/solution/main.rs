#![no_std]
#![no_main]

// ╔══════════════════════════════════════════════════════════════════════╗
// ║  Exercise 07 — Spinlocks                                               ║
// ║  Goal: write a SpinLock<T> mutex using atomics.                        ║
// ╚══════════════════════════════════════════════════════════════════════╝
//
// The work is in `spinlock.rs` (the `lock` and `try_lock` methods). This file
// is the test harness — read it (UNDERSTAND).
//
// Notice `COUNTER` below is a plain `static`, NOT `static mut`: the whole point
// of the lock (plus the `UnsafeCell` inside it) is that we can safely mutate
// shared data through an ordinary shared reference. So this file needs almost no
// `unsafe` at all — a sharp contrast with the raw `static mut` tables earlier.

mod entry;
mod spinlock;
mod testdev;
mod uart;
// Carried from earlier exercises; not exercised by this test.
#[allow(dead_code)]
mod kalloc;
#[allow(dead_code)]
mod memlayout;
#[allow(dead_code)]
mod param;
#[allow(dead_code)]
mod proc;
#[allow(dead_code)]
mod sched;
#[allow(dead_code)]
mod swtch;
#[allow(dead_code)]
mod vm;

use core::panic::PanicInfo;
use spinlock::SpinLock;

// Shared, lock-protected state. No `mut`, no raw pointers — the lock handles it.
static COUNTER: SpinLock<u64> = SpinLock::new(0);

#[no_mangle]
pub extern "C" fn kmain() -> ! {
    uart::puts("\nrv6 booting (exercise 07: spinlocks)...\n");
    if run_checks() {
        uart::puts("OSLINGS:PASS\n");
    } else {
        uart::puts("OSLINGS:FAIL\n");
    }
    testdev::exit_success();
}

fn run_checks() -> bool {
    // 1) try_lock on a free lock should succeed and actually hold it.
    let g = COUNTER.try_lock();
    if g.is_none() {
        uart::puts("  [fail] try_lock failed on a free lock\n");
        return false;
    }
    let g = g.unwrap();
    if !COUNTER.is_locked() {
        uart::puts("  [fail] lock not marked held while a guard is alive\n");
        return false;
    }
    // 2) ...and while held, a second attempt must fail (mutual exclusion).
    if COUNTER.try_lock().is_some() {
        uart::puts("  [fail] try_lock succeeded on an already-held lock\n");
        return false;
    }
    drop(g);
    if COUNTER.is_locked() {
        uart::puts("  [fail] lock still held after the guard was dropped\n");
        return false;
    }

    // 3) lock() must actually acquire and exclude.
    {
        let mut held = COUNTER.lock();
        *held += 1;
        if !COUNTER.is_locked() {
            uart::puts("  [fail] lock() did not actually acquire the lock\n");
            return false;
        }
        if COUNTER.try_lock().is_some() {
            uart::puts("  [fail] lock() does not exclude try_lock\n");
            return false;
        }
    } // guard dropped here → released

    // 4) data integrity across many lock/modify/unlock cycles.
    let start = *COUNTER.lock();
    for _ in 0..1000 {
        *COUNTER.lock() += 1;
    }
    let end = *COUNTER.lock();
    if end != start + 1000 {
        uart::puts("  [fail] count is wrong after locked updates\n");
        return false;
    }

    uart::puts("  [ok] acquire, exclude, release, and protect data — all correct\n");
    true
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    uart::puts("OSLINGS:FAIL (panic)\n");
    testdev::exit_failure(1);
}
