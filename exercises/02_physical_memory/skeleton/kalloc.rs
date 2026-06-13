//! kalloc.rs — the physical page allocator.
//!
//! This is the kernel's most fundamental memory service: hand out and reclaim
//! 4096-byte pages of physical RAM. Everything later (page tables, process
//! stacks, the heap) is built on top of it.
//!
//! The design is the classic **intrusive free list**. We keep a singly linked
//! list of every page that is currently free. The clever part: we don't need a
//! separate array to store the list — a free page has nothing useful in it, so
//! we store the "next free page" pointer *inside the free page itself*. That's
//! what `Run` is.

use crate::memlayout::{PGSIZE, PHYSTOP};
use core::ptr;

// A free page, reinterpreted as a list node. We only ever look at its first
// field, `next`, which points at the next free page (or null at the end).
// `#[repr(C)]` pins down the field layout so the pointer lives at offset 0.
#[repr(C)]
struct Run {
    next: *mut Run,
}

// The head of the free list. `static mut` is a single global, mutable variable;
// touching it requires `unsafe` because nothing stops two pieces of code from
// racing on it (we'll add locking in a later exercise).
static mut FREELIST: *mut Run = ptr::null_mut();

extern "C" {
    // `end` is a symbol defined by the linker script (kernel.ld): the first
    // address just past the kernel image. RAM from here up to PHYSTOP is unused
    // and free for us to manage.
    static end: u8;
}

/// Round an address UP to the next page boundary.
///
/// `& !(PGSIZE - 1)` clears the low bits, snapping down to a multiple of 4096;
/// adding `PGSIZE - 1` first makes it snap *up*. (UNDERSTAND)
fn pgroundup(addr: usize) -> usize {
    (addr + PGSIZE - 1) & !(PGSIZE - 1)
}

/// Build the initial free list: give every page between the kernel and PHYSTOP
/// to `kfree`. After this runs, `kalloc` has pages to hand out. (UNDERSTAND)
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

/// Return one 4096-byte physical page to the free list.
///
/// `pa` must be the start of a page that is not currently in use.
pub unsafe fn kfree(pa: *mut u8) {
    // IMPLEMENT: push this page onto the front of the free list.
    //
    //   The page itself stores the link, so no extra memory is needed:
    //     1. reinterpret `pa` as a `*mut Run`  (a raw pointer cast: `as *mut Run`).
    //     2. write the current FREELIST into that page's `next` field.
    //        (write through a raw pointer with `(*r).next = ...;`)
    //     3. set FREELIST to point at this page.
    //
    //   Order matters: save the old head into the new node *before* you move
    //   the head, or you'll lose the rest of the list.
    let _ = pa; // delete this line once you implement the function
}

/// Take one 4096-byte physical page off the free list.
///
/// Returns a null pointer if no memory is left.
pub unsafe fn kalloc() -> *mut u8 {
    // IMPLEMENT: pop the front page off the free list and return it.
    //
    //     1. read the current head:           let r = FREELIST;
    //     2. if r isn't null, advance the head: FREELIST = (*r).next;
    //     3. return r as a `*mut u8`  (it may be null when we're out of RAM —
    //        that's a valid answer, the caller checks for null).
    ptr::null_mut() // replace with your implementation
}
