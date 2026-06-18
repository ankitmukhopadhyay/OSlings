//! memlayout.rs — the physical memory layout of the QEMU `virt` machine.
//! (UNDERSTAND — you reuse and extend these constants across exercises.)

/// Bytes per page. Hardware manages memory in fixed-size 4096-byte chunks
/// called pages; this is also the size of one page table.
pub const PGSIZE: usize = 4096;

/// Where RAM begins on the `virt` board (see the memory map in exercise 01).
pub const KERNBASE: usize = 0x8000_0000;

/// One byte past the top of RAM (`-m 128M`). Physical RAM is KERNBASE..PHYSTOP.
pub const PHYSTOP: usize = KERNBASE + 128 * 1024 * 1024; // 0x8800_0000

/// MMIO base of the UART (serial port) — from exercise 01's memory map. We map
/// this page in the paging exercise.
pub const UART0: usize = 0x1000_0000;
