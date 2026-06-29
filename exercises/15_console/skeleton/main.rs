#![no_std]
#![no_main]

// ╔══════════════════════════════════════════════════════════════════════╗
// ║  Exercise 15 — Console                                        PART 2    ║
// ║  Goal: read keyboard input via UART interrupts through the PLIC.        ║
// ╚══════════════════════════════════════════════════════════════════════╝
//
// The timer (exercise 14) was a CPU-internal interrupt. Now we handle a *device*
// interrupt: the UART, routed through the PLIC. With this, the OS can finally
// READ what you type. The work is in `console.rs` (the `intr` handler); the PLIC
// setup is given in `plic.rs`.
//
// Try it live:  `cd rv6 && cargo run`  boots a console that echoes your typing.

#[allow(dead_code)]
mod console;
#[allow(dead_code)]
mod entry;
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
mod plic;
#[allow(dead_code)]
mod proc;
#[allow(dead_code)]
mod sched;
#[allow(dead_code)]
mod semaphore;
#[allow(dead_code)]
mod spinlock;
#[allow(dead_code)]
mod start;
#[allow(dead_code)]
mod swtch;
#[allow(dead_code)]
mod testdev;
#[allow(dead_code)]
mod trap;
#[allow(dead_code)]
mod uart;
#[allow(dead_code)]
mod vm;

use core::panic::PanicInfo;

const BANNER: &str = r#"
                  __
 _ __            / /_
| '__|  \ \ / /  | '_ \
| |      \ V /   | (_) |
|_|       \_/     \___/

  A tiny RISC-V OS
"#;

unsafe fn kinit() {
    uart::init();
    kalloc::init();
    vm::kvminithart(vm::kvmmake());
    proc::init();
    trap::init();
}

#[no_mangle]
pub extern "C" fn kmain() -> ! {
    unsafe {
        kinit();
    }

    uart::puts("\n");
    uart::puts(BANNER);
    uart::puts("\nrv6: kernel booted.\n");

    #[cfg(feature = "harness")]
    {
        if unsafe { console_self_check() } {
            uart::puts("OSLINGS:PASS\n");
        } else {
            uart::puts("OSLINGS:FAIL\n");
        }
        testdev::exit_success();
    }

    #[cfg(not(feature = "harness"))]
    {
        unsafe {
            console::init();
            trap::intr_on();
        }
        uart::puts("rv6: console ready — type something (it echoes). Ctrl-A X to quit.\n> ");
        loop {
            let c = console::getc();
            uart::putc(c);
            if c == b'\r' {
                uart::puts("\n> ");
            }
        }
    }
}

/// Confirm the interrupt handler does its job. We loop the UART back to itself
/// and "type" bytes; each one makes the UART raise its interrupt, and we invoke
/// `console::intr` exactly as the trap path would, then read the byte back out
/// of the console's buffer. (We drive `intr` directly rather than enabling
/// interrupts globally, so a half-finished handler fails cleanly instead of
/// storming.)
#[cfg(feature = "harness")]
unsafe fn console_self_check() -> bool {
    console::init();
    uart::set_loopback(true);

    let msg = b"hi";
    let mut got = [0u8; 2];
    for i in 0..msg.len() {
        uart::putc(msg[i]); // loop a byte to the receiver; the UART raises its IRQ
        console::intr(); // handle it as the trap path would
        match console::try_getc() {
            Some(b) => got[i] = b,
            None => {
                uart::set_loopback(false);
                uart::puts("  [fail] the interrupt handler did not buffer the byte\n");
                return false;
            }
        }
    }
    uart::set_loopback(false);

    if got == *msg {
        uart::puts("  [ok] a UART interrupt is claimed, read, and buffered by the handler\n");
        true
    } else {
        uart::puts("  [fail] received the wrong bytes\n");
        false
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    uart::puts("OSLINGS:FAIL (panic)\n");
    testdev::exit_failure(1);
}
