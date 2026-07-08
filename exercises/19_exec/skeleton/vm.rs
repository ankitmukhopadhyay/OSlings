//! vm.rs — RISC-V Sv39 virtual memory. (Exercise 19 reference solution.)
//!
//! Extended for `exec`: where exercise 18 mapped exactly one code page,
//! this module now *loads a whole program image* — however many pages it
//! takes — with `load_segment`, and gives it a stack with `map_user_stack`.
//! Teardown (`free_user_pagetable`) is generalized to free every user page,
//! no matter how many there are.

use crate::kalloc;
use crate::memlayout::{
    KERNBASE, PGSIZE, PHYSTOP, PLIC, PLIC_SIZE, TEST_FINISHER, TRAMPOLINE, UART0, USER_CODE,
    USER_STACK,
};
use core::arch::asm;
use core::ptr;

pub const PTE_V: usize = 1 << 0;
pub const PTE_R: usize = 1 << 1;
pub const PTE_W: usize = 1 << 2;
pub const PTE_X: usize = 1 << 3;
/// The "user" bit: only PTEs with this bit set can be touched from user mode.
/// This single bit is the wall between user programs and the kernel.
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

// ========================================================================
//  Kernel virtual memory.
// ========================================================================

pub const SATP_SV39: usize = 8 << 60;

pub fn make_satp(root: *mut Pte) -> usize {
    SATP_SV39 | ((root as usize) >> 12)
}

// The trampoline assembly lives in usermode.rs; these symbols mark where its
// instructions start and end inside the kernel image.
extern "C" {
    fn trampoline();
    fn trampoline_end();
}

/// The physical page holding the (copied) trampoline code. `kvmmake` fills
/// this in; every user page table maps the same page.
static mut TRAMP_PAGE: usize = 0;

pub fn trampoline_page() -> usize {
    unsafe { TRAMP_PAGE }
}

pub unsafe fn kvmmake() -> *mut Pte {
    let root = kalloc::kalloc() as *mut Pte;
    if root.is_null() {
        return ptr::null_mut();
    }
    ptr::write_bytes(root as *mut u8, 0, PGSIZE);

    if mappages(root, UART0, PGSIZE, UART0, PTE_R | PTE_W).is_err() {
        return ptr::null_mut();
    }
    if mappages(root, TEST_FINISHER, PGSIZE, TEST_FINISHER, PTE_R | PTE_W).is_err() {
        return ptr::null_mut();
    }
    if mappages(root, PLIC, PLIC_SIZE, PLIC, PTE_R | PTE_W).is_err() {
        return ptr::null_mut();
    }
    if mappages(
        root,
        KERNBASE,
        PHYSTOP - KERNBASE,
        KERNBASE,
        PTE_R | PTE_W | PTE_X,
    )
    .is_err()
    {
        return ptr::null_mut();
    }

    // Give the trampoline its very own page and map it at the top of the
    // kernel's address space. The trampoline must sit alone on a page that is
    // mapped at the SAME virtual address in the kernel's page table and in
    // every user page table, so we copy its instructions out of the kernel
    // image onto a fresh page and map that page at TRAMPOLINE.
    let tramp = kalloc::kalloc();
    if tramp.is_null() {
        return ptr::null_mut();
    }
    let src = trampoline as *const () as usize;
    let len = trampoline_end as *const () as usize - src;
    if len > PGSIZE {
        return ptr::null_mut(); // the trampoline must fit on one page
    }
    ptr::copy_nonoverlapping(src as *const u8, tramp, len);
    asm!("fence.i"); // we just wrote instructions: flush the instruction fetch path
    if mappages(root, TRAMPOLINE, PGSIZE, tramp as usize, PTE_R | PTE_X).is_err() {
        return ptr::null_mut();
    }
    TRAMP_PAGE = tramp as usize;

    root
}

pub unsafe fn kvminithart(root: *mut Pte) {
    let satp = make_satp(root);
    asm!("csrw satp, {}", in(reg) satp);
    asm!("sfence.vma zero, zero");
}

// ========================================================================
//  User virtual memory.
// ========================================================================

/// Load a program image into a fresh user page table, starting at
/// `USER_CODE` (virtual address 0). The image can be any size: a small
/// program fits on one page, a bigger one needs several. For EACH page the
/// image needs, you allocate a physical page, copy that slice of the image
/// onto it, and map it into the user's page table with R + X + U.
///
/// This is the general form of exercise 18's `map_user_pages`: same idea
/// (allocate a page, copy code in, map it with PTE_U), but now in a loop so
/// the program is not limited to a single page.
pub unsafe fn load_segment(table: *mut Pte, image: &[u8]) -> Result<(), ()> {
    // IMPLEMENT: walk the image one PGSIZE chunk at a time. For chunk number
    // `i` (starting at 0), the virtual address is `USER_CODE + i * PGSIZE`.
    //
    // For each chunk:
    //   1. `let page = kalloc::kalloc();`  — a fresh physical page.
    //      If it is null, `return Err(())` (out of memory).
    //   2. `ptr::write_bytes(page, 0, PGSIZE);` — zero it, so the leftover
    //      tail of the last (partial) page is clean.
    //   3. copy this chunk of the image onto the page. The chunk is
    //      `image[off .. off + n]` where `off = i * PGSIZE` and `n` is
    //      `min(PGSIZE, image.len() - off)` (the last chunk is usually short):
    //          ptr::copy_nonoverlapping(image.as_ptr().add(off), page, n);
    //   4. map it: `mappages(table, USER_CODE + off, PGSIZE, page as usize,
    //      PTE_R | PTE_X | PTE_U)?;`
    //
    // Loop while `off < image.len()`. `fence.i` once at the end, since you
    // just wrote instructions into memory (see kvmmake for the same trick).
    let _ = (table, image, USER_CODE); // remove this line once you implement the loop
    Err(()) // an unimplemented loader must not claim it loaded the program
}

/// Give the user one page of stack, mapped at `USER_STACK` with R + W + U.
/// (UNDERSTAND — given; it is `load_segment`'s single-page cousin, without
/// the copy: a stack starts out as blank scratch memory.)
pub unsafe fn map_user_stack(table: *mut Pte) -> Result<(), ()> {
    let page = kalloc::kalloc();
    if page.is_null() {
        return Err(());
    }
    ptr::write_bytes(page, 0, PGSIZE);
    mappages(table, USER_STACK, PGSIZE, page as usize, PTE_R | PTE_W | PTE_U)
}

/// Look up virtual address `va` in a user page table and return the physical
/// address it maps to, or 0 if it is not mapped (or not a user page). This is
/// `walk`, plus the safety checks that make it safe to use on addresses a
/// user program handed us. (UNDERSTAND — given.)
pub unsafe fn walkaddr(table: *mut Pte, va: usize) -> usize {
    if va >= crate::memlayout::MAXVA {
        return 0;
    }
    let pte = walk(table, va, false);
    if pte.is_null() || !(*pte).is_valid() || (*pte).flags() & PTE_U == 0 {
        return 0;
    }
    (*pte).pa()
}

/// Copy the bytes of `src` INTO a user address space, starting at user
/// virtual address `dstva`. In exercise 18 this was given but nothing used
/// it yet; now `exec`'s argv setup (in exec.rs) copies argument strings and
/// the argv pointer array onto the user's stack with it. (UNDERSTAND —
/// given; you wrote its mirror image `copyin` in exercise 18.)
pub unsafe fn copyout(table: *mut Pte, mut dstva: usize, src: &[u8]) -> Result<(), ()> {
    let mut copied = 0;
    while copied < src.len() {
        let va0 = pgrounddown(dstva); // the page this address lives on
        let pa0 = walkaddr(table, va0); // where that page really is
        if pa0 == 0 {
            return Err(()); // not mapped, or not a user page
        }
        let off = dstva - va0; // how far into the page we start
        let mut n = PGSIZE - off; // bytes left on this page
        if n > src.len() - copied {
            n = src.len() - copied; // don't copy more than we have
        }
        ptr::copy_nonoverlapping(src.as_ptr().add(copied), (pa0 + off) as *mut u8, n);
        copied += n;
        dstva = va0 + PGSIZE; // continue at the start of the next page
    }
    Ok(())
}

/// Copy `dst.len()` bytes OUT of a user address space into `dst`, starting at
/// user virtual address `srcva`. (UNDERSTAND — you wrote this in exercise 18;
/// `write` still uses it to read the user's buffer.)
pub unsafe fn copyin(table: *mut Pte, dst: &mut [u8], mut srcva: usize) -> Result<(), ()> {
    let mut copied = 0;
    while copied < dst.len() {
        let va0 = pgrounddown(srcva);
        let pa0 = walkaddr(table, va0);
        if pa0 == 0 {
            return Err(());
        }
        let off = srcva - va0;
        let mut n = PGSIZE - off;
        if n > dst.len() - copied {
            n = dst.len() - copied;
        }
        ptr::copy_nonoverlapping((pa0 + off) as *const u8, dst.as_mut_ptr().add(copied), n);
        copied += n;
        srcva = va0 + PGSIZE;
    }
    Ok(())
}

/// Tear a user page table down completely. Freeing has to handle a program
/// of ANY size now, so instead of naming specific pages it walks the whole
/// tree: every user leaf page (PTE_U) is ours to free; the trampoline and
/// trapframe leaves (no PTE_U) are owned elsewhere, so we only drop the
/// mapping; and every page-table page itself is freed on the way back up.
/// (UNDERSTAND — given; the generalization of exercise 18's teardown.)
pub unsafe fn free_user_pagetable(root: *mut Pte) {
    free_pt(root);
}

unsafe fn free_pt(table: *mut Pte) {
    for i in 0..512 {
        let pte = table.add(i);
        if (*pte).is_valid() {
            let is_leaf = (*pte).flags() & (PTE_R | PTE_W | PTE_X) != 0;
            if is_leaf {
                // a user page belongs to this process; free it. A non-user
                // leaf (trampoline / trapframe) is shared or owned elsewhere,
                // so we drop the mapping but leave the page alone.
                if (*pte).flags() & PTE_U != 0 {
                    kalloc::kfree((*pte).pa() as *mut u8);
                }
            } else {
                // an interior node: recurse into the next level down.
                free_pt((*pte).pa() as *mut Pte);
            }
            *pte = Pte(0);
        }
    }
    kalloc::kfree(table as *mut u8);
}
