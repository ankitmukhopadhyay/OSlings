#![no_std]
#![no_main]

// ╔══════════════════════════════════════════════════════════════════════╗
// ║  Exercise 01 — Boot                                                    ║
// ║  Goal: take control from QEMU, set up a stack, reach Rust, and print.  ║
// ╚══════════════════════════════════════════════════════════════════════╝
//
// You completed the bare-metal setup in exercise 00. Now we make the kernel
// actually *boot*: QEMU jumps to `_entry` (in entry.rs), which hands off to
// the Rust function `kmain` below.
//
// New files this exercise (read them all):
//   - entry.rs    the assembly trampoline — you IMPLEMENT this
//   - uart.rs     a tiny serial-port driver (provided, UNDERSTAND it)
//   - testdev.rs  asks QEMU to power off (provided, UNDERSTAND it)

mod entry;
mod testdev;
mod uart;

use core::panic::PanicInfo;

// `kmain` is the first Rust code that runs with a valid stack. `entry.rs`
// calls it. It must never return — there is nothing to return to.
#[no_mangle]
pub extern "C" fn kmain() -> ! {
    uart::puts("\nrv6 is booting...\n");

    // IMPLEMENT: print the success marker the test harness watches for.
    //   The harness passes this exercise when it sees this EXACT line on the
    //   serial console:
    //
    //       OSLINGS:PASS
    //
    //   Use `uart::puts(...)`. Remember the trailing newline ('\n').

    // UNDERSTAND: cleanly power off the virtual machine so the test ends.
    testdev::exit_success();
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    uart::puts("OSLINGS:FAIL (panic)\n");
    testdev::exit_failure(1);
}
