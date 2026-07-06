//! memlayout.rs — the physical memory layout of the QEMU `virt` machine,
//! plus (new in this exercise) the layout of a *user* address space.
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
//  The user address space (new in exercise 18).
//
//  A user process gets its OWN page table, so it sees its own private
//  little world of addresses, laid out like this:
//
//    virtual address       what lives there              who may touch it
//    ---------------       --------------------------    ----------------
//    0x3F_FFFF_F000        TRAMPOLINE (uservec/userret)   kernel only (R X)
//    0x3F_FFFF_E000        TRAPFRAME (saved registers)    kernel only (R W)
//         ...                    (unmapped)
//    0x0000_2000           <- initial stack pointer
//    0x0000_1000           the stack page                 user (R W U)
//    0x0000_0000           the program's code             user (R X U)
// ========================================================================

/// One past the highest virtual address Sv39 paging can use. Sv39 has 39
/// address bits, but we stop one bit short (1 << 38) so we never build an
/// address whose top bit is set (those must be sign-extended, which is an
/// easy source of bugs).
pub const MAXVA: usize = 1 << 38;

/// The very top page of EVERY address space (kernel's and each user's) holds
/// the trampoline: the tiny assembly that switches between the two worlds.
/// It is mapped at the same virtual address in both page tables on purpose;
/// see the lesson for why that is the trick that makes switching possible.
pub const TRAMPOLINE: usize = MAXVA - PGSIZE;

/// Just below the trampoline, each process has its own trapframe page: the
/// parking lot where all 31 user registers are saved on every trap.
pub const TRAPFRAME: usize = TRAMPOLINE - PGSIZE;

/// Where a user program's code is loaded: the very first page. Address 0 is a
/// perfectly ordinary address in a fresh user address space.
pub const USER_CODE: usize = 0x0;

/// The user program's stack page sits right above its code...
pub const USER_STACK: usize = PGSIZE;

/// ...and the stack pointer starts at the top of that page (stacks grow down).
pub const USER_STACK_TOP: usize = USER_STACK + PGSIZE;
