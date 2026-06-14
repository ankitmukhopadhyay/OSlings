//! vm.rs — RISC-V Sv39 virtual memory: page tables.
//!
//! This file builds the data structure the CPU's memory-management unit (MMU)
//! reads to translate a *virtual* address into a *physical* one. We are not
//! turning the MMU on yet (that comes later) — here we build and verify the
//! tables themselves, which is where all the interesting structure lives.

use crate::kalloc;
use crate::memlayout::PGSIZE;
use core::ptr;

// ---- Page-table entry (PTE) flag bits ----------------------------------
// The low 10 bits of every PTE are flags describing the mapping.
pub const PTE_V: usize = 1 << 0; // Valid: this entry is in use
pub const PTE_R: usize = 1 << 1; // Readable
pub const PTE_W: usize = 1 << 2; // Writable
pub const PTE_X: usize = 1 << 3; // eXecutable
#[allow(dead_code)] // used when we map user-mode pages in a later exercise
pub const PTE_U: usize = 1 << 4; // User-mode accessible

/// A **page-table entry**: one 64-bit slot that packs a physical page number
/// together with the flag bits above.
///
/// `#[repr(transparent)]` means a `Pte` is laid out in memory exactly like the
/// `usize` it wraps — so an array of 512 `Pte`s *is* a hardware page table.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Pte(pub usize);

impl Pte {
    /// Build a PTE that points at physical address `pa` with the given `flags`.
    ///
    /// A PTE doesn't store the full physical address. It stores the **physical
    /// page number** (PPN = `pa >> 12`, i.e. the address with its low 12 offset
    /// bits dropped) up in bits 53..10, and the flags in bits 9..0.
    pub const fn new(pa: usize, flags: usize) -> Pte {
        // IMPLEMENT: pack it together.
        //   value = ((pa >> 12) << 10) | flags
        let _ = (pa, flags); // remove this line once you implement it
        Pte(0)
    }

    /// The physical (page-base) address this PTE points at — the inverse of
    /// `new`: take the PPN back out of bits 53..10 and shift it up by 12.
    pub const fn pa(self) -> usize {
        // IMPLEMENT:
        //   (self.0 >> 10) << 12
        0
    }

    /// The flag bits (the low 10 bits). (UNDERSTAND)
    pub const fn flags(self) -> usize {
        self.0 & 0x3ff
    }

    /// Is the Valid bit set? (UNDERSTAND)
    pub const fn is_valid(self) -> bool {
        self.0 & PTE_V != 0
    }
}

/// Pull the 9-bit page-table index for a given `level` out of a virtual
/// address. A 39-bit Sv39 virtual address is sliced into three 9-bit indices
/// (one per level) plus a 12-bit page offset:
///
/// ```text
///   bits: [38..30] VPN[2] | [29..21] VPN[1] | [20..12] VPN[0] | [11..0] offset
/// ```
///
/// level 2 selects an entry in the top table, level 0 in the leaf. (UNDERSTAND)
const fn px(level: usize, va: usize) -> usize {
    (va >> (12 + level * 9)) & 0x1ff
}

fn pgrounddown(a: usize) -> usize {
    a & !(PGSIZE - 1)
}

/// Walk the 3-level page table for virtual address `va` and return a pointer to
/// its **leaf** PTE (the level-0 entry that finally maps the page).
///
/// `table` is the root page table (a page of 512 PTEs). At each of the two
/// upper levels we either follow the existing entry to the next table, or — if
/// `alloc` is true — create a new (zeroed) table and link it in. Returns null
/// if a table is missing (and `alloc` is false) or if `kalloc` runs out.
pub unsafe fn walk(mut table: *mut Pte, va: usize, alloc: bool) -> *mut Pte {
    // IMPLEMENT: descend the two upper levels (2, then 1):
    //
    //   for level = 2 down to 1:
    //     let pte = table.add(px(level, va));   // the entry at this level
    //     if (*pte).is_valid() {
    //         table = (*pte).pa() as *mut Pte;  // follow it to the next table
    //     } else {
    //         if !alloc { return ptr::null_mut(); }
    //         let page = kalloc::kalloc();
    //         if page.is_null() { return ptr::null_mut(); }
    //         ptr::write_bytes(page, 0, PGSIZE); // a fresh table must be zeroed
    //         *pte = Pte::new(page as usize, PTE_V); // a non-leaf PTE: V only,
    //                                                // NO R/W/X
    //         table = page as *mut Pte;
    //     }
    //
    // Then return the leaf entry:  table.add(px(0, va))
    let _ = (alloc, px(2, va), &mut table); // remove once implemented
    ptr::null_mut()
}

/// Map `size` bytes of virtual addresses starting at `va` to physical `pa`,
/// with permission flags `perm`. `va` and `pa` should be page-aligned; `size`
/// is rounded up to whole pages. Relies on your `walk`. (UNDERSTAND — given.)
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
            return Err(()); // ran out of memory for a page table
        }
        // Install the leaf mapping: point at pa, mark Valid + the perms.
        *pte = Pte::new(pa, perm | PTE_V);
        if a == last {
            break;
        }
        a += PGSIZE;
        pa += PGSIZE;
    }
    Ok(())
}
