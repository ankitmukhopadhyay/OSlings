#![no_std]
#![no_main]

// Exercise 01 — reference solution.

mod entry;
mod testdev;
mod uart;

use core::panic::PanicInfo;

#[no_mangle]
pub extern "C" fn kmain() -> ! {
    uart::puts("\nrv6 is booting...\n");
    uart::puts("OSLINGS:PASS\n");
    testdev::exit_success();
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    uart::puts("OSLINGS:FAIL (panic)\n");
    testdev::exit_failure(1);
}
