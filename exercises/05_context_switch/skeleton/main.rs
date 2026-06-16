#![no_std]
#![no_main]

// ╔══════════════════════════════════════════════════════════════════════╗
// ║  Exercise 05 — Context Switch                                          ║
// ║  Goal: write `swtch` — pause one execution context, resume another.    ║
// ╚══════════════════════════════════════════════════════════════════════╝
//
// The work is in `swtch.rs`. This file is the test harness — read it
// (UNDERSTAND). It switches from the "scheduler" context into a task, lets the
// task run, and the task switches back. If `swtch` works, control flows:
//
//   kmain → swtch(SCHED→TASK) → task_entry runs → swtch(TASK→SCHED) → back in kmain

mod entry;
mod kalloc;
mod param;
mod swtch;
mod testdev;
mod uart;
// Library modules carried from earlier exercises expose more API than this
// test uses, so silence their dead-code warnings.
#[allow(dead_code)]
mod memlayout;
#[allow(dead_code)]
mod proc;
#[allow(dead_code)]
mod vm;

use core::panic::PanicInfo;
use core::ptr;
use memlayout::PGSIZE;
use swtch::Context;

// The "scheduler" context (kmain's saved state) and the task's context.
static mut SCHED_CTX: Context = Context::zero();
static mut TASK_CTX: Context = Context::zero();

// A flag the task sets, so we can prove the task actually ran. Accessed with
// volatile reads/writes: the value is changed by code reached through a
// hand-written context switch the compiler can't see across, so we forbid it
// from caching the value in a register.
static mut TASK_RAN: usize = 0;

/// Runs after we switch INTO the task. Sets the flag, then switches back to the
/// scheduler context. It never returns normally — `swtch` carries control away.
extern "C" fn task_entry() -> ! {
    unsafe {
        ptr::write_volatile(ptr::addr_of_mut!(TASK_RAN), 0xABCD);
        swtch::swtch(ptr::addr_of_mut!(TASK_CTX), ptr::addr_of_mut!(SCHED_CTX));
    }
    loop {} // unreachable: we switched away above
}

#[no_mangle]
pub extern "C" fn kmain() -> ! {
    uart::puts("\nrv6 booting (exercise 05: context switch)...\n");
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
    // Give the task its own stack (one page; the stack grows down from the top).
    let stack = kalloc::kalloc();
    if stack.is_null() {
        uart::puts("  [fail] could not allocate a task stack\n");
        return false;
    }
    let stack_top = stack as usize + PGSIZE;

    // Arrange for a switch into TASK_CTX to begin at task_entry on that stack.
    // Coerce the function *item* to a function *pointer* before taking its
    // address as a number (a direct `fn-item as usize` cast is discouraged).
    let entry: extern "C" fn() -> ! = task_entry;
    swtch::init_context(ptr::addr_of_mut!(TASK_CTX), entry as usize, stack_top);

    // Switch into the task. It sets TASK_RAN and switches back here.
    swtch::swtch(ptr::addr_of_mut!(SCHED_CTX), ptr::addr_of_mut!(TASK_CTX));

    // We're back in kmain. Did the task run?
    if ptr::read_volatile(ptr::addr_of!(TASK_RAN)) != 0xABCD {
        uart::puts("  [fail] task never ran (did swtch switch?)\n");
        return false;
    }

    uart::puts("  [ok] switched into a task and back again\n");
    true
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    uart::puts("OSLINGS:FAIL (panic)\n");
    testdev::exit_failure(1);
}
