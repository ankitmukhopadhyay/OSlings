#![no_std]
#![no_main]

// ╔══════════════════════════════════════════════════════════════════════╗
// ║  Exercise 09 — Virtual Memory                                          ║
// ║  Goal: build the kernel page table and turn the MMU ON.                ║
// ╚══════════════════════════════════════════════════════════════════════╝
//
// The work is in `vm.rs` (`make_satp` and `kvmmake`). This file is the test
// harness — read it (UNDERSTAND).
//
// Turning the MMU on is dangerous: if the page table is wrong, the very next
// instruction fetch faults and the kernel hangs. So this harness VERIFIES every
// critical mapping with `walk` while paging is still OFF — giving you a precise
// error if something's missing — and only switches the MMU on once the table
// checks out.

mod entry;
mod kalloc;
mod memlayout;
mod testdev;
mod uart;
mod vm;
// Carried from earlier exercises; not exercised by this test.
#[allow(dead_code)]
mod kheap;
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

use core::panic::PanicInfo;
use memlayout::{KERNBASE, PGSIZE, TEST_FINISHER, UART0};
use vm::{Pte, PTE_R, PTE_W, PTE_X};

#[no_mangle]
pub extern "C" fn kmain() -> ! {
    uart::puts("\nrv6 booting (exercise 09: virtual memory)...\n");
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

/// Look up `va` in `root` (with the MMU still off) and return the leaf entry's
/// (page-base physical address, flags), or None if it isn't mapped.
unsafe fn leaf(root: *mut Pte, va: usize) -> Option<(usize, usize)> {
    let pte = vm::walk(root, va, false);
    if pte.is_null() || !(*pte).is_valid() {
        return None;
    }
    Some(((*pte).pa(), (*pte).flags()))
}

/// Check `va` is identity-mapped (pa == its own page) with at least `perm`.
unsafe fn mapped(root: *mut Pte, va: usize, perm: usize) -> bool {
    match leaf(root, va) {
        Some((pa, flags)) => pa == (va & !(PGSIZE - 1)) && (flags & perm) == perm,
        None => false,
    }
}

unsafe fn run_checks() -> bool {
    let root = vm::kvmmake();
    if root.is_null() {
        uart::puts("  [fail] kvmmake returned null (out of memory or unmapped)\n");
        return false;
    }

    // --- verify the table BEFORE we dare switch it on ---

    if !mapped(root, UART0, PTE_R | PTE_W) {
        uart::puts("  [fail] UART page not identity-mapped read+write\n");
        return false;
    }
    if !mapped(root, TEST_FINISHER, PTE_R | PTE_W) {
        uart::puts("  [fail] test-finisher page not identity-mapped read+write\n");
        return false;
    }
    // Kernel code lives at the bottom of RAM; it must be executable.
    if !mapped(root, KERNBASE, PTE_R | PTE_W | PTE_X) {
        uart::puts("  [fail] kernel RAM (KERNBASE) not identity-mapped R+W+X\n");
        return false;
    }
    // Our current stack is somewhere in RAM; it must be mapped too.
    let probe = 0u8;
    let sp_here = &probe as *const u8 as usize;
    if !mapped(root, sp_here, PTE_R | PTE_W) {
        uart::puts("  [fail] kernel stack page is not mapped\n");
        return false;
    }

    // Verify the satp value before installing it.
    let satp = vm::make_satp(root);
    if (satp >> 60) != 8 {
        uart::puts("  [fail] make_satp: mode field is not Sv39 (8)\n");
        return false;
    }
    if (satp & ((1usize << 44) - 1)) != (root as usize >> 12) {
        uart::puts("  [fail] make_satp: wrong root page number\n");
        return false;
    }

    // --- the table checks out: turn the MMU on ---
    vm::kvminithart(root);

    // If we reach here and can still print, every address we touch is now being
    // translated — paging is on and the identity map is correct.
    uart::puts("  [ok] kernel page table verified; MMU is now ON\n");
    true
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    uart::puts("OSLINGS:FAIL (panic)\n");
    testdev::exit_failure(1);
}
