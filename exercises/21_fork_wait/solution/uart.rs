//! uart.rs — a polled NS16550A UART driver. (Exercise 11 reference solution.)

use crate::memlayout::UART0;
use core::ptr::{read_volatile, write_volatile};

const RBR: usize = 0; // Receive Buffer Register (read)
const THR: usize = 0; // Transmit Holding Register (write)
const IER: usize = 1; // Interrupt Enable Register
const FCR: usize = 2; // FIFO Control Register
const LCR: usize = 3; // Line Control Register
const MCR: usize = 4; // Modem Control Register
const LSR: usize = 5; // Line Status Register

const LSR_DR: u8 = 1 << 0; // Data Ready: a byte is waiting in RBR
const LSR_THRE: u8 = 1 << 5; // Tx Holding Empty: ok to write THR
const MCR_LOOP: u8 = 1 << 4; // loopback mode

unsafe fn reg_read(off: usize) -> u8 {
    read_volatile((UART0 + off) as *const u8)
}

unsafe fn reg_write(off: usize, val: u8) {
    write_volatile((UART0 + off) as *mut u8, val);
}

pub fn init() {
    unsafe {
        reg_write(IER, 0x00); // polling, so disable interrupts
        reg_write(LCR, 0x03); // 8 bits, no parity, 1 stop bit
        reg_write(FCR, 0x07); // enable FIFO + clear receive/transmit FIFOs
    }
}

/// Turn on the UART's "a byte arrived" interrupt (IER bit 0). After this, the
/// UART raises an interrupt whenever a received byte is waiting. (Given.)
pub fn enable_rx_interrupt() {
    unsafe { reg_write(IER, 0x01) }
}

pub fn tx_ready() -> bool {
    unsafe { reg_read(LSR) & LSR_THRE != 0 }
}

pub fn rx_ready() -> bool {
    unsafe { reg_read(LSR) & LSR_DR != 0 }
}

pub fn putc(c: u8) {
    while !tx_ready() {}
    unsafe { reg_write(THR, c) }
}

pub fn getc() -> Option<u8> {
    if rx_ready() {
        Some(unsafe { reg_read(RBR) })
    } else {
        None
    }
}

#[allow(dead_code)] // the kernel prints through this; this exercise's harness
                    // uses its own bootstrap console, so it's unused here.
pub fn puts(s: &str) {
    for b in s.bytes() {
        putc(b);
    }
}

pub fn set_loopback(on: bool) {
    unsafe { reg_write(MCR, if on { MCR_LOOP } else { 0 }) }
}
