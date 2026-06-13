//! uart.rs — the world's smallest serial driver. (UNDERSTAND, don't edit.)
//!
//! QEMU's `virt` machine emulates an NS16550A UART (a classic PC serial chip)
//! with its registers mapped into physical memory at 0x1000_0000. Writing a
//! byte to the Transmit Holding Register (the first byte of that region) sends
//! that byte out the serial line — which, under `-nographic -serial mon:stdio`,
//! is your terminal.
//!
//! That is the entire trick behind "print" in a kernel: store a byte to a
//! magic address. No syscalls, no libc — just a `volatile` write to MMIO.

use core::ptr::write_volatile;

/// Base address of the UART's memory-mapped registers on the `virt` machine.
const UART0: *mut u8 = 0x1000_0000 as *mut u8;

/// Send a single byte out the serial port.
pub fn putc(c: u8) {
    // `write_volatile` tells the compiler this store has a side effect and
    // must not be optimized away or reordered — essential for device I/O.
    // It is `unsafe` because we are dereferencing an arbitrary raw pointer;
    // we know UART0 is valid because the `virt` machine guarantees it.
    unsafe {
        write_volatile(UART0, c);
    }
}

/// Send a string out the serial port, one byte at a time.
pub fn puts(s: &str) {
    for byte in s.bytes() {
        putc(byte);
    }
}
