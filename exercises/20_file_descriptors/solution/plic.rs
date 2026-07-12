//! plic.rs — the Platform-Level Interrupt Controller. (UNDERSTAND — given.)
//!
//! The timer (exercise 14) is built into the CPU, but most interrupts come from
//! *devices* (the UART, a disk, ...). The PLIC is the traffic cop that collects
//! those device interrupt lines and delivers them to a CPU. To use a device
//! interrupt we: give its source a priority, enable it for our hart's
//! supervisor "context", and set our threshold to accept it. When one fires we
//! `claim` it (asking "which device?"), handle it, then `complete` it.

use crate::memlayout::PLIC;
use core::ptr::{read_volatile, write_volatile};

/// The UART's interrupt source number on the QEMU `virt` machine.
pub const UART0_IRQ: u32 = 10;

// Registers for hart 0's *supervisor* context.
const PLIC_SENABLE: usize = PLIC + 0x2080; // which sources are enabled
const PLIC_STHRESHOLD: usize = PLIC + 0x20_1000; // minimum priority we accept
const PLIC_SCLAIM: usize = PLIC + 0x20_1004; // claim / complete

/// Configure the PLIC to deliver UART interrupts to this hart's S-mode.
pub unsafe fn init() {
    // give the UART source a non-zero priority (0 means "disabled")
    write_volatile((PLIC + UART0_IRQ as usize * 4) as *mut u32, 1);
    // enable the UART source for our supervisor context
    write_volatile(PLIC_SENABLE as *mut u32, 1 << UART0_IRQ);
    // accept any interrupt with priority greater than 0
    write_volatile(PLIC_STHRESHOLD as *mut u32, 0);
}

/// Ask the PLIC which device interrupt is pending (0 = none).
pub fn claim() -> u32 {
    unsafe { read_volatile(PLIC_SCLAIM as *const u32) }
}

/// Tell the PLIC we have finished handling interrupt `irq`.
pub fn complete(irq: u32) {
    unsafe { write_volatile(PLIC_SCLAIM as *mut u32, irq) }
}
