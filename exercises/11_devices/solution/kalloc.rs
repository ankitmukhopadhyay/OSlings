//! kalloc.rs — the physical page allocator. (Exercise 02 reference solution.)

use crate::memlayout::{PGSIZE, PHYSTOP};
use core::ptr;

#[repr(C)]
struct Run {
    next: *mut Run,
}

static mut FREELIST: *mut Run = ptr::null_mut();

extern "C" {
    static end: u8;
}

fn pgroundup(addr: usize) -> usize {
    (addr + PGSIZE - 1) & !(PGSIZE - 1)
}

pub unsafe fn init() {
    let start = &end as *const u8 as usize;
    free_range(start, PHYSTOP);
}

unsafe fn free_range(start: usize, stop: usize) {
    let mut p = pgroundup(start);
    while p + PGSIZE <= stop {
        kfree(p as *mut u8);
        p += PGSIZE;
    }
}

pub unsafe fn kfree(pa: *mut u8) {
    let r = pa as *mut Run;
    (*r).next = FREELIST;
    FREELIST = r;
}

pub unsafe fn kalloc() -> *mut u8 {
    let r = FREELIST;
    if !r.is_null() {
        FREELIST = (*r).next;
    }
    r as *mut u8
}
