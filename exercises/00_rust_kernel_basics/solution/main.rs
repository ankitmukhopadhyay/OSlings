#![no_std]
#![no_main]

// Exercise 00 — reference solution.
//
// `#![no_std]` drops the standard library (we keep only `core`).
// `#![no_main]` drops the compiler's `main` entry shim — our entry point is
// `_entry`, named by kernel.ld.

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

// Placeholder entry point. Exercise 01 replaces this with a real boot
// sequence that sets up a stack and jumps into Rust.
#[no_mangle]
pub extern "C" fn _entry() -> ! {
    loop {}
}
