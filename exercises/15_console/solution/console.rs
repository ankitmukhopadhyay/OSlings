//! console.rs — interrupt-driven console input. (Exercise 15 reference solution.)

use crate::plic;
use crate::uart;
use core::arch::asm;
use core::ptr::{addr_of, addr_of_mut};

const BUF_LEN: usize = 256;

// A single-producer (the interrupt handler) / single-consumer (the reader)
// ring buffer of input bytes. Separate head and tail make it safe without a
// lock on one CPU.
static mut BUF: [u8; BUF_LEN] = [0; BUF_LEN];
static mut HEAD: usize = 0; // next index the consumer will read
static mut TAIL: usize = 0; // next index the producer will write

/// Push a received byte (called from the interrupt handler).
fn push(b: u8) {
    unsafe {
        let tail = *addr_of!(TAIL);
        let head = *addr_of!(HEAD);
        if tail.wrapping_sub(head) < BUF_LEN {
            *addr_of_mut!(BUF[tail % BUF_LEN]) = b;
            *addr_of_mut!(TAIL) = tail.wrapping_add(1);
        }
        // (if full, drop the byte)
    }
}

/// Pop one byte if the buffer has any; otherwise None. (Non-blocking.)
pub fn try_getc() -> Option<u8> {
    unsafe {
        let head = *addr_of!(HEAD);
        let tail = *addr_of!(TAIL);
        if head == tail {
            None
        } else {
            let b = *addr_of!(BUF[head % BUF_LEN]);
            *addr_of_mut!(HEAD) = head.wrapping_add(1);
            Some(b)
        }
    }
}

/// Read one byte, waiting for input to arrive. Sleeps the CPU with `wfi` until
/// an interrupt (the UART) delivers a byte.
pub fn getc() -> u8 {
    loop {
        if let Some(b) = try_getc() {
            return b;
        }
        unsafe { asm!("wfi") };
    }
}

/// Set up the console: configure the UART to interrupt on input, route that
/// interrupt through the PLIC, and enable the supervisor external interrupt.
pub unsafe fn init() {
    uart::init();
    uart::enable_rx_interrupt();
    plic::init();
    // enable the supervisor *external* interrupt source (sie.SEIE, bit 9)
    asm!("csrs sie, {}", in(reg) 1usize << 9);
}

/// Handle a device interrupt (called from the trap handler on a supervisor
/// external interrupt).
pub fn intr() {
    // ask the PLIC which device interrupted
    let irq = plic::claim();
    if irq == plic::UART0_IRQ {
        // drain every byte the UART has into the input buffer
        while let Some(b) = uart::getc() {
            push(b);
        }
    }
    // tell the PLIC we are done (skip irq 0, which means "nothing")
    if irq != 0 {
        plic::complete(irq);
    }
}
