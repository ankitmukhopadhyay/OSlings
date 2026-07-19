//! memlayout.rs — the physical memory layout of the QEMU `virt` machine,
//! plus the layout of a user address space.
//! (UNDERSTAND — you reuse and extend these constants across exercises.)

/// Bytes per page. Hardware manages memory in fixed-size 4096-byte chunks
/// called pages; this is also the size of one page table.
pub const PGSIZE: usize = 4096;

/// Where RAM begins on the `virt` board (see the memory map in exercise 01).
pub const KERNBASE: usize = 0x8000_0000;

/// One byte past the top of RAM (`-m 128M`). Physical RAM is KERNBASE..PHYSTOP.
pub const PHYSTOP: usize = KERNBASE + 128 * 1024 * 1024; // 0x8800_0000

/// MMIO base of the UART (serial port). We map this page so printing keeps
/// working after the MMU is turned on.
pub const UART0: usize = 0x1000_0000;

/// MMIO address of the SiFive test finisher (used to power off / report). We map
/// this page so the kernel can still exit QEMU once paging is on.
pub const TEST_FINISHER: usize = 0x10_0000;

/// MMIO base of the PLIC (Platform-Level Interrupt Controller), which routes
/// device interrupts (like the UART's) to the CPU. We map this region so the
/// kernel can talk to it with paging on.
pub const PLIC: usize = 0x0c00_0000;
pub const PLIC_SIZE: usize = 0x40_0000; // 4 MiB, covers all PLIC registers

// ========================================================================
//  The user address space (updated in exercise 19: programs can now be
//  bigger than one page, so the stack moves to a fixed home above the
//  largest possible image).
//
//    virtual address       what lives there              who may touch it
//    ---------------       --------------------------    ----------------
//    0x3F_FFFF_F000        TRAMPOLINE (uservec/userret)   kernel only (R X)
//    0x3F_FFFF_E000        TRAPFRAME (saved registers)    kernel only (R W)
//         ...                    (unmapped)
//    0x0001_1000           <- initial stack pointer
//    0x0001_0000           the stack page                 user (R W U)
//         ...                    (unmapped guard gap)
//    0x0000_0000 ..        the program image, 1..16 pages user (R X U)
// ========================================================================

/// One past the highest virtual address Sv39 paging can use. Sv39 has 39
/// address bits, but we stop one bit short (1 << 38) so we never build an
/// address whose top bit is set (those must be sign-extended, which is an
/// easy source of bugs).
pub const MAXVA: usize = 1 << 38;

/// The very top page of EVERY address space (kernel's and each user's) holds
/// the trampoline: the tiny assembly that switches between the two worlds.
pub const TRAMPOLINE: usize = MAXVA - PGSIZE;

/// Just below the trampoline, each process has its own trapframe page: the
/// parking lot where all 31 user registers are saved on every trap.
pub const TRAPFRAME: usize = TRAMPOLINE - PGSIZE;

/// Where a user program's image is loaded: the very first page. Address 0 is
/// a perfectly ordinary address in a fresh user address space.
pub const USER_CODE: usize = 0x0;

/// The most pages a program image may occupy (16 pages = 64 KiB — palatial,
/// by our standards).
pub const MAX_PROG_PAGES: usize = 16;

/// The stack page lives at a FIXED address, just above the largest possible
/// image. For any smaller program, the pages in between simply stay unmapped,
/// and that gap is a feature: a program that runs off the end of its memory
/// hits a page fault (which the kernel catches cleanly) instead of silently
/// corrupting its own stack.
pub const USER_STACK: usize = MAX_PROG_PAGES * PGSIZE; // 0x1_0000

/// The stack pointer starts at the top of the stack page (stacks grow down).
pub const USER_STACK_TOP: usize = USER_STACK + PGSIZE; // 0x1_1000
