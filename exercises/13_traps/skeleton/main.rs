#![no_std]
#![no_main]

// ╔══════════════════════════════════════════════════════════════════════╗
// ║  Exercise 13 — Traps                                          PART 2    ║
// ║  Goal: handle a supervisor trap (a breakpoint exception) and resume.   ║
// ╚══════════════════════════════════════════════════════════════════════╝
//
// Your kernel boots (exercise 12). Now teach it to take a *trap*: when the CPU
// hits something that needs the kernel (here, an `ebreak` breakpoint), it jumps
// to our handler, which deals with it and lets execution continue. This is the
// foundation for interrupts, system calls, and everything in the rest of Part 2.
//
// The work is in `trap.rs`. This file boots the kernel and (in harness mode)
// fires a breakpoint to check your handler caught it and resumed.

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
mod start;
#[allow(dead_code)]
mod spinlock;
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

  A tiny RISC-V OS
"#;

/// Bring the kernel up, then install trap handling.
unsafe fn kinit() {
    uart::init(); // console
    kalloc::init(); // physical page allocator
    vm::kvminithart(vm::kvmmake()); // kernel page table + MMU on
    proc::init(); // process table
    trap::init(); // supervisor trap vector (your work, in trap.rs)
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
        if unsafe { trap_self_check() } {
            uart::puts("OSLINGS:PASS\n");
        } else {
            uart::puts("OSLINGS:FAIL\n");
        }
        testdev::exit_success();
    }

    #[cfg(not(feature = "harness"))]
    {
        uart::puts("rv6: trap handling installed; idling. (exit QEMU with Ctrl-A then X)\n");
        loop {
            unsafe { core::arch::asm!("wfi") };
        }
    }
}

/// Confirm trap handling works: stvec points at our vector, and a deliberate
/// breakpoint is caught by the handler, which then resumes us right here.
#[cfg(feature = "harness")]
unsafe fn trap_self_check() -> bool {
    // 1) did `trap::init` point stvec at our vector?
    let stvec: usize;
    core::arch::asm!("csrr {}, stvec", out(reg) stvec);
    if stvec != trap::vector_addr() {
        uart::puts("  [fail] stvec is not pointing at the trap vector (trap::init)\n");
        return false;
    }

    // 2) fire a breakpoint. If the handler works, it counts the trap, advances
    //    past the ebreak, and execution continues on the next line. If it
    //    doesn't advance sepc, the kernel loops on the ebreak forever (timeout).
    let before = trap::trap_count();
    core::arch::asm!(".word 0x00100073"); // a 4-byte ebreak instruction
    let after = trap::trap_count();

    if after != before + 1 {
        uart::puts("  [fail] the breakpoint was not handled (check kerneltrap)\n");
        return false;
    }

    uart::puts("  [ok] trap vector installed, breakpoint handled, execution resumed\n");
    true
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    uart::puts("OSLINGS:FAIL (panic)\n");
    testdev::exit_failure(1);
}
