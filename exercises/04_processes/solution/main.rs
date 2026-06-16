#![no_std]
#![no_main]

// ╔══════════════════════════════════════════════════════════════════════╗
// ║  Exercise 04 — Processes                                               ║
// ║  Goal: build the process table — allocate and free PCBs.              ║
// ╚══════════════════════════════════════════════════════════════════════╝
//
// The work is in `proc.rs`. This file is the test harness — read it
// (UNDERSTAND) to see exactly what allocproc/freeproc must guarantee.

mod entry;
mod kalloc;
mod param;
mod testdev;
mod uart;
// Library modules carried from earlier exercises expose a fuller API than this
// exercise's test happens to use, and `proc` carries forward-looking fields and
// states — so we silence dead-code warnings for them.
#[allow(dead_code)]
mod memlayout;
#[allow(dead_code)]
mod proc;
#[allow(dead_code)]
mod vm;

use core::panic::PanicInfo;
use param::NPROC;
use proc::ProcState;

#[no_mangle]
pub extern "C" fn kmain() -> ! {
    uart::puts("\nrv6 booting (exercise 04: processes)...\n");
    unsafe {
        kalloc::init();
    }
    if unsafe { run_checks() } {
        uart::puts("OSLINGS:PASS\n");
    } else {
        uart::puts("OSLINGS:FAIL\n");
    }
    testdev::exit_success();
}

unsafe fn run_checks() -> bool {
    proc::init();

    // 1) allocate one process from an empty table
    let p1 = proc::allocproc();
    if p1.is_null() {
        uart::puts("  [fail] allocproc returned null on an empty table\n");
        return false;
    }
    if (*p1).state != ProcState::Runnable {
        uart::puts("  [fail] new process is not Runnable\n");
        return false;
    }
    if (*p1).pid == 0 {
        uart::puts("  [fail] new process has no pid\n");
        return false;
    }
    if (*p1).pagetable.is_null() {
        uart::puts("  [fail] new process has no page table\n");
        return false;
    }

    // 2) a second allocation is a different slot with a different pid
    let p2 = proc::allocproc();
    if p2.is_null() || p2 == p1 {
        uart::puts("  [fail] second allocproc reused the slot or failed\n");
        return false;
    }
    if (*p2).pid == (*p1).pid {
        uart::puts("  [fail] pids are not unique\n");
        return false;
    }

    // 3) we can allocate exactly NPROC processes in total
    let mut count = 2;
    loop {
        let p = proc::allocproc();
        if p.is_null() {
            break;
        }
        count += 1;
    }
    if count != NPROC {
        uart::puts("  [fail] table did not hold exactly NPROC processes\n");
        return false;
    }

    // 4) a full table refuses further allocations
    if !proc::allocproc().is_null() {
        uart::puts("  [fail] allocated past a full table\n");
        return false;
    }

    // 5) freeing a slot resets it and releases its page table...
    proc::freeproc(p1);
    if (*p1).state != ProcState::Unused {
        uart::puts("  [fail] freeproc did not reset the slot to Unused\n");
        return false;
    }
    if !(*p1).pagetable.is_null() {
        uart::puts("  [fail] freeproc did not drop the page table\n");
        return false;
    }

    // ...so exactly one more allocation now succeeds
    let p3 = proc::allocproc();
    if p3.is_null() {
        uart::puts("  [fail] could not reuse the freed slot\n");
        return false;
    }

    uart::puts("  [ok] alloc, uniqueness, NPROC limit, free/reuse — all correct\n");
    true
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    uart::puts("OSLINGS:FAIL (panic)\n");
    testdev::exit_failure(1);
}
