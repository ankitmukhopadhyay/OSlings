//! kheap.rs — the kernel heap: a global allocator so `alloc` types work.
//! (UNDERSTAND — given. This is the milestone that turns on Box/Vec/Arc.)
//!
//! Rust's `alloc` crate (Box, Vec, Arc, ...) needs *somebody* to provide raw
//! memory on demand. That "somebody" is a type implementing the `GlobalAlloc`
//! trait, registered with `#[global_allocator]`. Once it exists, `extern crate
//! alloc` lights up and heap types Just Work.
//!
//! Ours is deliberately tiny: it serves each allocation from one whole physical
//! page via the `kalloc` allocator you wrote in exercise 02. That's wasteful
//! (a 16-byte `Arc` still costs 4096 bytes) and it can't satisfy requests
//! larger than a page — but it's real, it frees memory back, and it's enough to
//! teach `Arc`. A space-efficient heap (sub-page allocation) is a later concern.

use crate::kalloc;
use crate::memlayout::PGSIZE;
use core::alloc::{GlobalAlloc, Layout};
use core::ptr;

pub struct KernelHeap;

unsafe impl GlobalAlloc for KernelHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // A page is 4096-byte aligned, so it satisfies any alignment up to a
        // page. We only handle requests that fit in a single page.
        if layout.size() > PGSIZE || layout.align() > PGSIZE {
            return ptr::null_mut();
        }
        kalloc::kalloc()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        // Each allocation was exactly one page; hand it straight back.
        kalloc::kfree(ptr);
    }
}

/// Register our allocator as *the* global heap. From here on, `Box`, `Vec`,
/// `Arc`, etc. allocate through `KernelHeap` above.
#[global_allocator]
static ALLOCATOR: KernelHeap = KernelHeap;
