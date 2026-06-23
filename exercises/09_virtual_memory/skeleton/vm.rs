//! vm.rs — RISC-V Sv39 virtual memory.
//!
//! Exercises 03 gave us page tables (`Pte`, `walk`, `mappages`). Now we use
//! them for real: build the kernel's own page table and *turn the MMU on*, so
//! every address the CPU touches goes through translation.

use crate::kalloc;
use crate::memlayout::{KERNBASE, PGSIZE, PHYSTOP, TEST_FINISHER, UART0};
use core::arch::asm;
use core::ptr;

// ---- Page-table entry (PTE) flag bits (from exercise 03) ----------------
pub const PTE_V: usize = 1 << 0; // Valid
pub const PTE_R: usize = 1 << 1; // Readable
pub const PTE_W: usize = 1 << 2; // Writable
pub const PTE_X: usize = 1 << 3; // eXecutable
#[allow(dead_code)] // used when we map user-mode pages in a later exercise
pub const PTE_U: usize = 1 << 4; // User-mode accessible

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Pte(pub usize);

impl Pte {
    pub const fn new(pa: usize, flags: usize) -> Pte {
        Pte(((pa >> 12) << 10) | flags)
    }
    pub const fn pa(self) -> usize {
        (self.0 >> 10) << 12
    }
    pub const fn flags(self) -> usize {
        self.0 & 0x3ff
    }
    pub const fn is_valid(self) -> bool {
        self.0 & PTE_V != 0
    }
}

const fn px(level: usize, va: usize) -> usize {
    (va >> (12 + level * 9)) & 0x1ff
}

fn pgrounddown(a: usize) -> usize {
    a & !(PGSIZE - 1)
}

/// Walk the 3-level page table for `va`, returning its leaf PTE (from ex 03).
pub unsafe fn walk(mut table: *mut Pte, va: usize, alloc: bool) -> *mut Pte {
    let mut level = 2;
    while level > 0 {
        let pte = table.add(px(level, va));
        if (*pte).is_valid() {
            table = (*pte).pa() as *mut Pte;
        } else {
            if !alloc {
                return ptr::null_mut();
            }
            let page = kalloc::kalloc();
            if page.is_null() {
                return ptr::null_mut();
            }
            ptr::write_bytes(page, 0, PGSIZE);
            *pte = Pte::new(page as usize, PTE_V);
            table = page as *mut Pte;
        }
        level -= 1;
    }
    table.add(px(0, va))
}

/// Map `size` bytes of VA `va` → PA `pa` with `perm` (from exercise 03).
pub unsafe fn mappages(
    table: *mut Pte,
    va: usize,
    size: usize,
    pa: usize,
    perm: usize,
) -> Result<(), ()> {
    let mut a = pgrounddown(va);
    let last = pgrounddown(va + size - 1);
    let mut pa = pa;
    loop {
        let pte = walk(table, a, true);
        if pte.is_null() {
            return Err(());
        }
        *pte = Pte::new(pa, perm | PTE_V);
        if a == last {
            break;
        }
        a += PGSIZE;
        pa += PGSIZE;
    }
    Ok(())
}

// ========================================================================
//  Kernel virtual memory: build the kernel page table and switch it on.
// ========================================================================

/// Sv39 mode selector, lives in the top 4 bits (63..60) of `satp`. The value 8
/// means "Sv39, 3-level, 39-bit virtual addresses".
pub const SATP_SV39: usize = 8 << 60;

/// Build the value to write into the `satp` register to activate page table
/// `root`. `satp` packs the mode (top bits) with the root table's physical page
/// number (PPN = physical address >> 12).
pub fn make_satp(root: *mut Pte) -> usize {
    // IMPLEMENT: combine the Sv39 mode bits with the root table's PPN.
    //   SATP_SV39 | ((root as usize) >> 12)
    let _ = root; // remove once implemented
    0
}

/// Build the kernel's page table: an identity map (virtual address == physical
/// address) of everything the kernel needs to keep running once paging is on.
///
/// Returns the root table, or null if out of memory.
pub unsafe fn kvmmake() -> *mut Pte {
    // Allocate and zero a fresh root page table.
    let root = kalloc::kalloc() as *mut Pte;
    if root.is_null() {
        return ptr::null_mut();
    }
    ptr::write_bytes(root as *mut u8, 0, PGSIZE);

    // IMPLEMENT: identity-map (va == pa) the regions the kernel needs. Use
    //   `mappages(root, ADDR, SIZE, ADDR, PERMS)` for each; bail out returning
    //   null if any mapping fails. Map these three:
    //
    //     1. the UART page:           UART0,          PGSIZE,            R + W
    //     2. the test finisher page:  TEST_FINISHER,  PGSIZE,            R + W
    //     3. all of RAM (kernel code, data, stacks, page tables):
    //                                 KERNBASE,  PHYSTOP - KERNBASE,     R + W + X
    //
    //   (We give RAM all of R+W+X for simplicity; a real kernel splits code as
    //   R+X and data as R+W. Identity mapping — va == pa — means addresses don't
    //   change when the MMU turns on, so execution continues seamlessly.)
    //
    //   Example shape:
    //     if mappages(root, UART0, PGSIZE, UART0, PTE_R | PTE_W).is_err() {
    //         return ptr::null_mut();
    //     }

    root
}

/// Turn the MMU on for this CPU: point `satp` at `root` and flush stale
/// translations. After this returns, every memory access is translated.
/// (UNDERSTAND — given. This is the actual "switch".)
pub unsafe fn kvminithart(root: *mut Pte) {
    let satp = make_satp(root);
    // Writing satp installs the page table; sfence.vma flushes the TLB (the
    // CPU's cache of translations) so no stale entries remain.
    asm!("csrw satp, {}", in(reg) satp);
    asm!("sfence.vma zero, zero");
}
