#![no_std]
#![no_main]

// ╔══════════════════════════════════════════════════════════════════════╗
// ║  Exercise 14 — Interrupts                                     PART 2    ║
// ║  Goal: take periodic timer interrupts — the basis of preemption.       ║
// ╚══════════════════════════════════════════════════════════════════════╝
//
// Exercise 13 handled an exception (a breakpoint). Now we handle an *interrupt*:
// a periodic timer that fires on its own while other code runs. Catching it lets
// the kernel take the CPU back from a running task whenever it wants — the
// mechanism behind preemptive multitasking.
//
// The work is in `trap.rs` (`intr_on` and the interrupt case of `kerneltrap`).
// The timer hardware is set up for you in `start.rs`.

#[allow(dead_code)]
mod entry;
#[allow(dead_code)]
mod fs;
#[allow(dead_code)]
mod kalloc;
#[allow(dead_code)]
mod kheap;
#[allow(dead_code)]
mod memlayout;
#[allow(dead_code)]
mod param;
#[allow(dead_code)]
mod proc;
#[allow(dead_code)]
mod sched;
#[allow(dead_code)]
mod semaphore;
#[allow(dead_code)]
mod spinlock;
#[allow(dead_code)]
mod start;
#[allow(dead_code)]
mod swtch;
#[allow(dead_code)]
mod testdev;
#[allow(dead_code)]
mod trap;
#[allow(dead_code)]
mod uart;
#[allow(dead_code)]
mod vm;

use core::panic::PanicInfo;

const BANNER: &str = r#"
                  __
 _ __            / /_
| '__|  \ \ / /  | '_ \
| |      \ V /   | (_) |
|_|       \_/     \___/

  A tiny interesting RISC-V OS
"#;

unsafe fn kinit() {
    uart::init();
    kalloc::init();
    vm::kvminithart(vm::kvmmake());
    proc::init();
    trap::init(); // supervisor trap vector (the timer is set up in start.rs)
}

#[no_mangle]
pub extern "C" fn kmain() -> ! {
    unsafe {
        kinit();
    }

    uart::puts("\n");
    uart::puts(BANNER);
    uart::puts("\nrv6: kernel booted.\n");

    #[cfg(feature = "harness")]
    {
        if unsafe { timer_self_check() } {
            uart::puts("OSLINGS:PASS\n");
        } else {
            uart::puts("OSLINGS:FAIL\n");
        }
        testdev::exit_success();
    }

    #[cfg(not(feature = "harness"))]
    {
        unsafe {
            trap::intr_on();
        }
        uart::puts("rv6: timer interrupts on; idling. (exit QEMU with Ctrl-A then X)\n");
        loop {
            unsafe { core::arch::asm!("wfi") };
        }
    }
}

/// The CLINT's time base on the QEMU `virt` machine: 10 MHz.
#[cfg(feature = "harness")]
const TIMEBASE: u64 = 10_000_000;

/// Read the `time` CSR (wall-clock-ish, independent of CPU speed).
#[cfg(feature = "harness")]
fn read_time() -> u64 {
    let t: u64;
    unsafe { core::arch::asm!("csrr {}, time", out(reg) t) };
    t
}

/// Turn interrupts on and confirm the timer fires, is handled, and is paced like
/// a real timer (not re-firing in a storm).
#[cfg(feature = "harness")]
unsafe fn timer_self_check() -> bool {
    trap::intr_on();

    let t0 = read_time();
    let start = trap::ticks();

    // wait for a few ticks (each is the timer preempting this loop), but give up
    // after ~2 seconds of real time so a broken setup fails cleanly.
    while trap::ticks() < start + 3 {
        if read_time() - t0 > 2 * TIMEBASE {
            uart::puts("  [fail] no timer ticks — interrupts not enabled or not handled\n");
            return false;
        }
        core::hint::spin_loop();
    }

    // Three ticks (about 0.1s apart) should span well over a hundredth of a
    // second. Near-instant means the interrupt re-fired in a storm because the
    // pending bit was never cleared.
    if read_time() - t0 < TIMEBASE / 100 {
        uart::puts("  [fail] interrupt storm — did kerneltrap clear the pending bit?\n");
        return false;
    }

    uart::puts("  [ok] timer interrupts fire, are paced, and preempt running code\n");
    true
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    uart::puts("OSLINGS:FAIL (panic)\n");
    testdev::exit_failure(1);
}
