#![no_std]
#![no_main]

// ╔══════════════════════════════════════════════════════════════════════╗
// ║  Exercise 02 — Physical Memory                                         ║
// ║  Goal: write the kernel's physical page allocator (kalloc / kfree).    ║
// ╚══════════════════════════════════════════════════════════════════════╝
//
// Boot (exercise 01) is done: the kernel reaches `kmain` and can print. Now we
// teach it to manage RAM. The work for this exercise is in `kalloc.rs`; this
// file is the test harness — read it (UNDERSTAND) to see exactly what your
// allocator must guarantee, but you don't need to edit it.

mod entry;
mod kalloc;
mod memlayout;
mod testdev;
mod uart;

use core::panic::PanicInfo;
use memlayout::{KERNBASE, PGSIZE, PHYSTOP};

#[no_mangle]
pub extern "C" fn kmain() -> ! {
    uart::puts("\nrv6 booting (exercise 02: physical memory)...\n");

    // Build the free list out of all the RAM above the kernel image.
    unsafe {
        kalloc::init();
    }

    // UNDERSTAND: this self-test exercises your allocator. Every check must
    // pass for the exercise to succeed.
    if unsafe { run_checks() } {
        uart::puts("OSLINGS:PASS\n");
    } else {
        uart::puts("OSLINGS:FAIL\n");
    }

    testdev::exit_success();
}

/// Returns true only if the page allocator behaves correctly.
unsafe fn run_checks() -> bool {
    // 1) we can allocate a page, and it isn't null
    let a = kalloc::kalloc();
    if a.is_null() {
        uart::puts("  [fail] kalloc returned null (free list empty?)\n");
        return false;
    }

    // 2) the page is page-aligned and inside physical RAM
    let pa = a as usize;
    if pa % PGSIZE != 0 || pa < KERNBASE || pa >= PHYSTOP {
        uart::puts("  [fail] page is misaligned or outside RAM\n");
        return false;
    }

    // 3) the whole page is real, writable memory: write a pattern, read it back
    for i in 0..PGSIZE {
        *a.add(i) = (i & 0xff) as u8;
    }
    for i in 0..PGSIZE {
        if *a.add(i) != (i & 0xff) as u8 {
            uart::puts("  [fail] page did not hold what we wrote\n");
            return false;
        }
    }

    // 4) a second allocation must give a *different* page
    let b = kalloc::kalloc();
    if b.is_null() || a == b {
        uart::puts("  [fail] second kalloc reused or failed\n");
        return false;
    }

    // 5) free a page, then alloc again: a LIFO free list hands the same one back
    kalloc::kfree(b);
    let c = kalloc::kalloc();
    if c != b {
        uart::puts("  [fail] free/alloc did not recycle the page\n");
        return false;
    }

    uart::puts("  [ok] allocate, write, distinct, recycle — all correct\n");
    true
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    uart::puts("OSLINGS:FAIL (panic)\n");
    testdev::exit_failure(1);
}
