//! memlayout.rs — the physical memory layout of the QEMU `virt` machine.
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
