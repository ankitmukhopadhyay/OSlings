//! vm.rs — RISC-V Sv39 virtual memory: page tables. (Exercise 03 solution.)

use crate::kalloc;
use crate::memlayout::PGSIZE;
use core::ptr;

pub const PTE_V: usize = 1 << 0;
pub const PTE_R: usize = 1 << 1;
pub const PTE_W: usize = 1 << 2;
pub const PTE_X: usize = 1 << 3;
#[allow(dead_code)] // used when we map user-mode pages in a later exercise
pub const PTE_U: usize = 1 << 4;

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
