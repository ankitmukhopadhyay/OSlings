#![no_std]
#![no_main]

// ╔══════════════════════════════════════════════════════════════════════╗
// ║  Exercise 03 — Paging                                                  ║
// ║  Goal: build RISC-V Sv39 page tables (PTE bit-packing + the walk).     ║
// ╚══════════════════════════════════════════════════════════════════════╝
//
// The work is in `vm.rs`. This file is the test harness — read it (UNDERSTAND)
// to see exactly what your page-table code must do, but you don't edit it.

mod entry;
mod kalloc;
mod memlayout;
mod testdev;
mod uart;
mod vm;

use core::panic::PanicInfo;
use core::ptr;
use memlayout::{PGSIZE, UART0};
use vm::{Pte, PTE_R, PTE_V, PTE_W, PTE_X};

#[no_mangle]
pub extern "C" fn kmain() -> ! {
    uart::puts("\nrv6 booting (exercise 03: paging)...\n");
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

/// Software emulation of what the MMU would do: walk the table for `va` and, if
/// it's mapped, combine the leaf's page address with `va`'s page offset.
unsafe fn translate(root: *mut Pte, va: usize) -> Option<usize> {
    let pte = vm::walk(root, va, false);
    if pte.is_null() || !(*pte).is_valid() {
        return None;
    }
    Some((*pte).pa() | (va & (PGSIZE - 1)))
}

unsafe fn run_checks() -> bool {
    // 0) PTE encoding must round-trip: build one, read the parts back out.
    let some_pa = 0x8765_4000usize;
    let e = Pte::new(some_pa, PTE_R | PTE_W | PTE_V);
    if e.pa() != some_pa {
        uart::puts("  [fail] Pte::pa did not recover the address\n");
        return false;
    }
    if e.flags() & (PTE_R | PTE_W | PTE_V) != (PTE_R | PTE_W | PTE_V) {
        uart::puts("  [fail] Pte flags wrong\n");
        return false;
    }

    // A fresh, empty root page table.
    let root = kalloc::kalloc() as *mut Pte;
    if root.is_null() {
        uart::puts("  [fail] out of memory for the root table\n");
        return false;
    }
    ptr::write_bytes(root as *mut u8, 0, PGSIZE);

    // 1) Identity-map the UART page (virtual == physical), read+write only.
    if vm::mappages(root, UART0, PGSIZE, UART0, PTE_R | PTE_W).is_err() {
        uart::puts("  [fail] mappages(UART) failed\n");
        return false;
    }
    match translate(root, UART0 + 0x10) {
        Some(p) if p == UART0 + 0x10 => {}
        _ => {
            uart::puts("  [fail] UART address did not translate correctly\n");
            return false;
        }
    }
    let upte = vm::walk(root, UART0, false);
    if upte.is_null() || (*upte).flags() & PTE_R == 0 || (*upte).flags() & PTE_X != 0 {
        uart::puts("  [fail] UART leaf has wrong permission flags\n");
        return false;
    }

    // 2) Map a code page at a high virtual address to a fresh physical page,
    //    read+execute. This VA lands in different upper-level slots than the
    //    UART page, so `walk` must allocate new intermediate tables.
    let code_pa = kalloc::kalloc() as usize;
    let code_va = 0x0040_0000usize;
    if vm::mappages(root, code_va, PGSIZE, code_pa, PTE_R | PTE_X).is_err() {
        uart::puts("  [fail] mappages(code) failed\n");
        return false;
    }
    match translate(root, code_va + 0x123) {
        Some(p) if p == code_pa + 0x123 => {}
        _ => {
            uart::puts("  [fail] code address did not translate correctly\n");
            return false;
        }
    }

    // 3) An address we never mapped must not translate.
    if translate(root, 0x0080_0000).is_some() {
        uart::puts("  [fail] an unmapped address translated\n");
        return false;
    }

    uart::puts("  [ok] encode/decode, map, translate, unmapped — all correct\n");
    true
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    uart::puts("OSLINGS:FAIL (panic)\n");
    testdev::exit_failure(1);
}
