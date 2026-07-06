//! vm.rs — RISC-V Sv39 virtual memory. (Exercise 18 reference solution.)
//!
//! Extended for user mode: the kernel page table now also maps the
//! **trampoline** page, and this module gains the tools for building and
//! reaching into *user* address spaces: `map_user_pages`, `walkaddr`,
//! `copyin`/`copyout`, and teardown (`free_user_pagetable`).

use crate::kalloc;
use crate::memlayout::{
    KERNBASE, PGSIZE, PHYSTOP, PLIC, PLIC_SIZE, TEST_FINISHER, TRAMPOLINE, TRAPFRAME, UART0,
    USER_CODE, USER_STACK,
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

    // NEW: give the trampoline its very own page and map it at the top of the
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
//  User virtual memory (new in exercise 18).
// ========================================================================

/// Map a user program's two pages of memory into its page table:
///
///   - the code page at `USER_CODE` (virtual address 0): the program's
///     instructions. User mode must be able to Read and eXecute it.
///   - the stack page at `USER_STACK`: its working memory. User mode must be
///     able to Read and Write it.
///
/// Both need `PTE_U`, or the CPU will refuse to touch them from user mode.
pub unsafe fn map_user_pages(
    table: *mut Pte,
    code_page: usize,
    stack_page: usize,
) -> Result<(), ()> {
    // IMPLEMENT: two `mappages` calls (this is the same helper you used to
    // build the kernel's page table in exercise 09):
    //
    //   1. map USER_CODE  -> code_page,  one PGSIZE, PTE_R | PTE_X | PTE_U
    //   2. map USER_STACK -> stack_page, one PGSIZE, PTE_R | PTE_W | PTE_U
    //
    // `mappages` returns a Result, and so does this function, so you can use
    // the `?` operator on each call (exercise 10 introduced `?`).
    mappages(table, USER_CODE, PGSIZE, code_page, PTE_R | PTE_X | PTE_U)?;
    mappages(table, USER_STACK, PGSIZE, stack_page, PTE_R | PTE_W | PTE_U)?;
    Ok(())
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
/// virtual address `dstva`. (UNDERSTAND — given. This is the worked model
/// for `copyin` below: read them side by side.)
///
/// Why the loop? `dstva` is a virtual address in the USER's world, and the
/// user's pages can be scattered anywhere in physical memory. So we go page
/// by page: translate one page's address, copy what fits on that page, then
/// move to the next page and translate again.
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
/// user virtual address `srcva`. The kernel calls this whenever a user
/// program hands it a pointer (like `write`'s buffer): the kernel must
/// translate the user's addresses itself, page by page.
pub unsafe fn copyin(table: *mut Pte, dst: &mut [u8], mut srcva: usize) -> Result<(), ()> {
    // IMPLEMENT: the mirror image of `copyout` above. Loop until you have
    // copied `dst.len()` bytes:
    //
    //   1. `va0` = pgrounddown(srcva)         — the user page this address is on
    //   2. `pa0` = walkaddr(table, va0)       — its physical address; if 0,
    //      the user gave us a bad pointer: return Err(())
    //   3. copy up to the end of that page (or up to what remains of `dst`,
    //      whichever is smaller), from physical memory `pa0 + (srcva - va0)`
    //      into `dst`
    //   4. advance and continue on the next page: srcva = va0 + PGSIZE
    //
    // (`ptr::copy_nonoverlapping(src, dst, n)` copies n bytes, and
    // `dst.as_mut_ptr().add(i)` points i bytes into `dst`.)
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

/// Tear a user page table down: free the user's own pages (code + stack),
/// detach the shared trampoline and the trapframe (their pages are owned and
/// freed elsewhere), then free the page-table pages themselves.
/// (UNDERSTAND — given.)
pub unsafe fn free_user_pagetable(root: *mut Pte) {
    // free the user's data pages, if they were ever mapped
    for va in [USER_CODE, USER_STACK] {
        let pte = walk(root, va, false);
        if !pte.is_null() && (*pte).is_valid() {
            kalloc::kfree((*pte).pa() as *mut u8);
            *pte = Pte(0);
        }
    }
    // detach (but do NOT free) the trampoline and trapframe pages
    for va in [TRAMPOLINE, TRAPFRAME] {
        let pte = walk(root, va, false);
        if !pte.is_null() {
            *pte = Pte(0);
        }
    }
    freewalk(root);
}

/// Recursively free the pages that make up the page table itself. By the time
/// this runs, every leaf PTE must already be zero. A valid PTE with none of
/// R/W/X set is an interior node pointing at a lower-level table.
unsafe fn freewalk(table: *mut Pte) {
    for i in 0..512 {
        let pte = table.add(i);
        if (*pte).is_valid() {
            if (*pte).flags() & (PTE_R | PTE_W | PTE_X) == 0 {
                freewalk((*pte).pa() as *mut Pte);
            }
            *pte = Pte(0);
        }
    }
    kalloc::kfree(table as *mut u8);
}
