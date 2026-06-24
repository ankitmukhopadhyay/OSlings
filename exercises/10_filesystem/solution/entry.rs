//! entry.rs — the boot trampoline. (Exercise 01 reference solution.)

use core::arch::asm;

const STACK_SIZE: usize = 4096 * 4;

#[no_mangle]
static mut STACK0: [u8; STACK_SIZE] = [0; STACK_SIZE];

#[no_mangle]
#[link_section = ".entry"]
pub unsafe extern "C" fn _entry() -> ! {
    asm!(
        "la sp, {stack}",     // sp = bottom of our stack
        "li t0, {size}",      // t0 = stack size
        "add sp, sp, t0",     // sp = top of stack (it grows downward)
        "call kmain",         // enter Rust; never returns
        stack = sym STACK0,
        size = const STACK_SIZE,
        options(noreturn),
    );
}
