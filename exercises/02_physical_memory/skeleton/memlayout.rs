//! memlayout.rs — the physical memory layout of the QEMU `virt` machine.
//! (UNDERSTAND — you'll reuse and extend these constants in later exercises.)

/// Bytes per page.
///
/// Hardware doesn't manage memory one byte at a time — it works in fixed-size
/// chunks called **pages**. 4096 bytes (4 KiB) is the standard page size on
/// RISC-V (and x86). Our allocator hands out memory one page at a time.
pub const PGSIZE: usize = 4096;

/// Where RAM begins on the `virt` board (see the memory map in exercise 01).
pub const KERNBASE: usize = 0x8000_0000;

/// One byte past the top of the RAM we gave QEMU (`-m 128M` = 128 MiB).
///
/// So physical RAM is the range `KERNBASE .. PHYSTOP`. The kernel image sits at
/// the bottom; everything above it, up to here, is free memory our allocator
/// gets to manage.
pub const PHYSTOP: usize = KERNBASE + 128 * 1024 * 1024; // 0x8800_0000
