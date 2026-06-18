#![no_std]
#![no_main]

// ╔══════════════════════════════════════════════════════════════════════╗
// ║  Exercise 06 — Scheduling                                              ║
// ║  Goal: write the round-robin policy that drives a real scheduler loop.  ║
// ╚══════════════════════════════════════════════════════════════════════╝
//
// The work is in `sched.rs` (the `RoundRobin::pick_next` policy). This file is
// the test harness — read it (UNDERSTAND). It runs a real cooperative
// scheduler: each "process" does a little work, records that it ran, and
// yields back to the scheduler with `swtch`. With a correct round-robin policy
// the processes run *interleaved* (1,3,4,1,3,4,...), which is what we check.

mod entry;
mod kalloc;
mod param;
mod sched;
mod swtch;
mod testdev;
mod uart;
// Carried library modules whose full API this test doesn't use:
#[allow(dead_code)]
mod memlayout;
#[allow(dead_code)]
mod proc;
#[allow(dead_code)]
mod vm;

use core::panic::PanicInfo;
use core::ptr;
use memlayout::PGSIZE;
use proc::ProcState;
use sched::{RoundRobin, Scheduler};
use swtch::Context;

const SLICES: usize = 3; // how many turns each process takes before retiring
const ORDER_CAP: usize = 64;
const SCHED_CAP: usize = 10_000; // safety cap so a broken policy can't hang us

// The scheduler's own saved context, and a pointer to the process currently
// switched-in (so a running task knows which process it is).
static mut SCHED_CTX: Context = Context::zero();
static mut CURRENT: *mut proc::Proc = ptr::null_mut();

// A log of the order in which processes actually ran (by pid).
static mut ORDER: [usize; ORDER_CAP] = [0; ORDER_CAP];
static mut ORDER_LEN: usize = 0;

unsafe fn record(pid: usize) {
    if ORDER_LEN < ORDER_CAP {
        *ptr::addr_of_mut!(ORDER[ORDER_LEN]) = pid;
        ORDER_LEN += 1;
    }
}

/// Body every test process runs. It takes `SLICES` turns; each turn it records
/// that it ran and yields back to the scheduler. The local variables `p`,
/// `pid`, and the loop counter survive each yield precisely because `swtch`
/// saves and restores this context.
extern "C" fn task() -> ! {
    let p = unsafe { CURRENT };
    let pid = unsafe { (*p).pid };
    for k in 0..SLICES {
        unsafe {
            record(pid);
            if k == SLICES - 1 {
                (*p).state = ProcState::Zombie; // last turn: retire, don't reschedule
            }
            // Yield: save our place, switch back to the scheduler.
            swtch::swtch(ptr::addr_of_mut!((*p).context), ptr::addr_of_mut!(SCHED_CTX));
        }
    }
    loop {} // unreachable: we're Zombie and never scheduled again
}

/// The scheduler loop: repeatedly ask the policy which process to run, switch
/// into it, and (when it yields) make it Runnable again unless it retired.
unsafe fn run_round_robin() {
    let mut sched = RoundRobin::new();
    for _ in 0..SCHED_CAP {
        // Snapshot every slot's state for the policy to look at.
        let mut states = [ProcState::Unused; param::NPROC];
        for i in 0..param::NPROC {
            states[i] = (*proc::proc_at(i)).state;
        }
        match sched.pick_next(&states) {
            Some(i) => {
                let p = proc::proc_at(i);
                (*p).state = ProcState::Running;
                CURRENT = p;
                swtch::swtch(ptr::addr_of_mut!(SCHED_CTX), ptr::addr_of_mut!((*p).context));
                // The task yielded back. If it didn't retire, let it run again.
                if (*p).state == ProcState::Running {
                    (*p).state = ProcState::Runnable;
                }
            }
            None => return, // nothing left to run
        }
    }
}

#[no_mangle]
pub extern "C" fn kmain() -> ! {
    uart::puts("\nrv6 booting (exercise 06: scheduling)...\n");
    unsafe {
        kalloc::init();
        proc::init();
    }
    if unsafe { run_checks() } {
        uart::puts("OSLINGS:PASS\n");
    } else {
        uart::puts("OSLINGS:FAIL\n");
    }
    testdev::exit_success();
}

/// Set up one process slot: give it a pid, and if runnable, a stack and a
/// context that starts at `task`.
unsafe fn setup_proc(slot: usize, pid: usize, runnable: bool) -> bool {
    let p = proc::proc_at(slot);
    (*p).pid = pid;
    if runnable {
        let stack = kalloc::kalloc();
        if stack.is_null() {
            return false;
        }
        let stack_top = stack as usize + PGSIZE;
        let entry: extern "C" fn() -> ! = task;
        swtch::init_context(ptr::addr_of_mut!((*p).context), entry as usize, stack_top);
        (*p).state = ProcState::Runnable;
    } else {
        (*p).state = ProcState::Sleeping; // present, but never schedulable
    }
    true
}

unsafe fn run_checks() -> bool {
    // Three runnable processes (pids 1, 3, 4) plus one sleeping (pid 2) that the
    // scheduler must skip.
    if !setup_proc(0, 1, true)
        || !setup_proc(1, 2, false)
        || !setup_proc(2, 3, true)
        || !setup_proc(3, 4, true)
    {
        uart::puts("  [fail] could not set up processes\n");
        return false;
    }

    run_round_robin();

    // Correct round-robin: cycle through the runnable pids, skipping the
    // sleeper, until each has taken SLICES turns.
    let expected = [1usize, 3, 4, 1, 3, 4, 1, 3, 4];
    if ORDER_LEN != expected.len() {
        uart::puts("  [fail] wrong number of runs (policy not round-robin?)\n");
        return false;
    }
    for i in 0..ORDER_LEN {
        if *ptr::addr_of!(ORDER[i]) != expected[i] {
            uart::puts("  [fail] run order is not round-robin\n");
            return false;
        }
    }

    uart::puts("  [ok] round-robin scheduled processes in interleaved order\n");
    true
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    uart::puts("OSLINGS:FAIL (panic)\n");
    testdev::exit_failure(1);
}
