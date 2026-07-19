#![no_std]
#![no_main]

// ╔══════════════════════════════════════════════════════════════════════╗
// ║  Exercise 12 — Boot to life            ·····  PART 2 begins  ·····      ║
// ║  Goal: assemble the real boot sequence so rv6 boots as an OS.          ║
// ╚══════════════════════════════════════════════════════════════════════╝
//
// In Part 1 each exercise was proven by a self-test that printed OSLINGS:PASS
// and powered off — nothing actually booted. Part 2 turns rv6 into a real OS.
//
// This file is the kernel's entry into Rust (`kmain`). The work is in `kinit`
// below: bring the subsystems you built in Part 1 up, in the right order.
//
// DUAL MODE (read this):
//   • `oslings` builds with `--features harness` → runs a boot self-check →
//     prints OSLINGS:PASS → powers off (so it can be graded).
//   • Plain `cargo run` (no feature) → boots the real OS: prints the banner and
//     idles. As Part 2 continues, this path grows a console and a shell.

// Every module is carried from Part 1; this exercise wires them into a boot.
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
mod swtch;
#[allow(dead_code)]
mod testdev;
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

/// Bring the kernel's core subsystems up, in order. After this returns, the
/// kernel is "booted": memory works, paging is on, processes can be made.
unsafe fn kinit() {
    // IMPLEMENT: call the Part 1 init functions in the correct order. The order
    // matters — each step depends on the one before it:
    //
    //   1. uart::init();                       // the console, so we can print
    //   2. kalloc::init();                     // physical page allocator
    //   3. vm::kvminithart(vm::kvmmake());     // build the kernel page table,
    //                                          //   then TURN ON the MMU
    //   4. proc::init();                       // the process table
    //
    // Why this order: kvmmake() allocates page-table pages with kalloc, so
    // kalloc::init() MUST run first. (Turning on the MMU with a broken page
    // table will hang the kernel — keep the order exactly as above.)
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
        if unsafe { boot_self_check() } {
            uart::puts("OSLINGS:PASS\n");
        } else {
            uart::puts("OSLINGS:FAIL\n");
        }
        testdev::exit_success();
    }

    #[cfg(not(feature = "harness"))]
    {
        uart::puts("rv6: nothing to do yet — idling. (exit QEMU with Ctrl-A then X)\n");
        loop {
            unsafe { core::arch::asm!("wfi") };
        }
    }
}

/// The boot self-check used when grading: confirm the subsystems `kinit` was
/// supposed to bring up are actually up.
#[cfg(feature = "harness")]
unsafe fn boot_self_check() -> bool {
    // 1) the physical page allocator works
    let page = kalloc::kalloc();
    if page.is_null() {
        uart::puts("  [fail] kalloc is not initialized\n");
        return false;
    }
    kalloc::kfree(page);

    // 2) the MMU is on: satp's mode field (bits 63..60) is 8 for Sv39
    let satp: usize;
    core::arch::asm!("csrr {}, satp", out(reg) satp);
    if (satp >> 60) != 8 {
        uart::puts("  [fail] the MMU is not on (satp mode is not Sv39)\n");
        return false;
    }

    // 3) the process table is ready
    if proc::allocproc().is_null() {
        uart::puts("  [fail] the process table is not initialized\n");
        return false;
    }

    uart::puts("  [ok] console, allocator, MMU, and process table are all up\n");
    true
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    uart::puts("OSLINGS:FAIL (panic)\n");
    testdev::exit_failure(1);
}
