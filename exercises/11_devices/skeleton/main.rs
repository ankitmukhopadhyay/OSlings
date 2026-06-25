#![no_std]
#![no_main]

// ╔══════════════════════════════════════════════════════════════════════╗
// ║  Exercise 11 — Devices (UART driver)                                   ║
// ║  Goal: turn the blind-write UART into a real polled device driver.      ║
// ╚══════════════════════════════════════════════════════════════════════╝
//
// The work is in `uart.rs`. This file is the test harness.
//
// One wrinkle: this harness can't print through the driver you're building (a
// half-finished driver could hang or stay silent, and then we'd see nothing).
// So the harness reports results through `dbg_*` below — a minimal, always-works
// console that writes the UART transmit register directly (exactly the blind
// approach from exercise 01). Your real driver is what gets *tested*.

mod entry;
mod testdev;
mod uart;
// Carried from earlier exercises; not exercised by this test.
#[allow(dead_code)]
mod fs;
#[allow(dead_code)]
mod kalloc;
#[allow(dead_code)]
mod kheap;
#[allow(dead_code)]
mod memlayout;
#[allow(dead_code)]
mod param;
#[allow(dead_code)]
mod proc;
#[allow(dead_code)]
mod sched;
#[allow(dead_code)]
mod semaphore;
#[allow(dead_code)]
mod spinlock;
#[allow(dead_code)]
mod swtch;
#[allow(dead_code)]
mod vm;

use core::panic::PanicInfo;
use core::ptr::write_volatile;
use memlayout::UART0;

/// Minimal bootstrap console: write a byte straight to the UART's transmit
/// register, no status checks. Used only for the harness's own messages.
fn dbg_putc(c: u8) {
    unsafe { write_volatile(UART0 as *mut u8, c) }
}
fn dbg_puts(s: &str) {
    for b in s.bytes() {
        dbg_putc(b);
    }
}

#[no_mangle]
pub extern "C" fn kmain() -> ! {
    dbg_puts("\nrv6 booting (exercise 11: devices)...\n");
    if run_checks() {
        dbg_puts("OSLINGS:PASS\n");
    } else {
        dbg_puts("OSLINGS:FAIL\n");
    }
    testdev::exit_success();
}

fn run_checks() -> bool {
    uart::init();

    // After init the transmitter should report itself ready.
    if !uart::tx_ready() {
        dbg_puts("  [fail] tx_ready() is false right after init\n");
        return false;
    }

    // With no input arriving, there should be nothing to receive.
    if uart::rx_ready() {
        dbg_puts("  [fail] rx_ready() is true with no input\n");
        return false;
    }
    if uart::getc().is_some() {
        dbg_puts("  [fail] getc() returned a byte when none was available\n");
        return false;
    }

    // Loopback mode wires the transmitter straight back to the receiver, so a
    // byte we send with putc() should come back out of getc(). This exercises
    // the whole driver end-to-end.
    uart::set_loopback(true);
    uart::putc(0x42); // 'B'

    let mut got = None;
    for _ in 0..1_000_000 {
        if let Some(b) = uart::getc() {
            got = Some(b);
            break;
        }
    }
    uart::set_loopback(false);

    match got {
        Some(0x42) => {}
        Some(_) => {
            dbg_puts("  [fail] loopback returned the wrong byte\n");
            return false;
        }
        None => {
            dbg_puts("  [fail] loopback returned nothing (putc/getc not working)\n");
            return false;
        }
    }

    dbg_puts("  [ok] status flags, putc, and getc (via loopback) all correct\n");
    true
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    dbg_puts("OSLINGS:FAIL (panic)\n");
    testdev::exit_failure(1);
}
