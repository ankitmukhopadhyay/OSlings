//! uart.rs — a polled NS16550A UART driver.
//!
//! Back in exercise 01 we "printed" by blindly storing a byte to the UART's
//! transmit register and hoping the chip was ready. A real driver doesn't hope:
//! it talks to the device through its **registers**, checking a *status*
//! register before each transfer. That's what you build here — the same way you
//! drive any memory-mapped device.

use crate::memlayout::UART0;
use core::ptr::{read_volatile, write_volatile};

// The NS16550A exposes several one-byte registers at consecutive addresses
// starting at UART0. We name them by their offset.
const RBR: usize = 0; // Receive Buffer Register (read): an incoming byte
const THR: usize = 0; // Transmit Holding Register (write): an outgoing byte
const IER: usize = 1; // Interrupt Enable Register
const FCR: usize = 2; // FIFO Control Register
const LCR: usize = 3; // Line Control Register
const MCR: usize = 4; // Modem Control Register
const LSR: usize = 5; // Line Status Register: tells us the chip's state

// Bits within the Line Status Register.
const LSR_DR: u8 = 1 << 0; // Data Ready:        a received byte is waiting in RBR
const LSR_THRE: u8 = 1 << 5; // Tx Holding Empty:  it's safe to write a byte to THR

// Bit within the Modem Control Register.
const MCR_LOOP: u8 = 1 << 4; // loopback: transmitted bytes loop back to the receiver

/// Read one of the UART's registers. (UNDERSTAND — MMIO read.)
unsafe fn reg_read(off: usize) -> u8 {
    read_volatile((UART0 + off) as *const u8)
}

/// Write one of the UART's registers. (UNDERSTAND — MMIO write.)
unsafe fn reg_write(off: usize, val: u8) {
    write_volatile((UART0 + off) as *mut u8, val);
}

/// Configure the UART: 8 data bits, no interrupts (we poll), FIFOs on.
/// (UNDERSTAND — given.)
pub fn init() {
    unsafe {
        reg_write(IER, 0x00); // polling, so disable interrupts
        reg_write(LCR, 0x03); // 8 bits, no parity, 1 stop bit
        reg_write(FCR, 0x07); // enable FIFO + clear receive/transmit FIFOs
    }
}

/// True when the transmitter can accept another byte (THR is empty).
pub fn tx_ready() -> bool {
    // IMPLEMENT: read the Line Status Register (offset LSR) and test the
    //   LSR_THRE bit. Return true if it is set.
    //     unsafe { reg_read(LSR) & LSR_THRE != 0 }
    false
}

/// True when a received byte is waiting to be read.
pub fn rx_ready() -> bool {
    // IMPLEMENT: read LSR and test the LSR_DR ("data ready") bit.
    false
}

/// Send one byte, waiting until the transmitter is ready first.
pub fn putc(c: u8) {
    // IMPLEMENT: spin until tx_ready() is true, then write the byte to THR:
    //     while !tx_ready() {}
    //     unsafe { reg_write(THR, c) }
    let _ = c; // remove once implemented
}

/// Receive one byte if one is available, else None.
pub fn getc() -> Option<u8> {
    // IMPLEMENT: if rx_ready(), read and return the byte from RBR wrapped in
    //   Some(...); otherwise return None.
    //     if rx_ready() { Some(unsafe { reg_read(RBR) }) } else { None }
    None
}

#[allow(dead_code)] // the kernel prints through this; this exercise's harness
                    // uses its own bootstrap console, so it's unused here.
pub fn puts(s: &str) {
    for b in s.bytes() {
        putc(b);
    }
}

/// Turn loopback mode on/off: in loopback, bytes written to the transmitter are
/// fed straight back to the receiver. Handy for testing without real input.
/// (UNDERSTAND — given.)
pub fn set_loopback(on: bool) {
    unsafe { reg_write(MCR, if on { MCR_LOOP } else { 0 }) }
}
