//! entry.rs — the boot trampoline. (UNDERSTAND, don't edit.)
//!
//! QEMU jumps here first, in machine mode. We set up a stack and then call
//! `start` (machine-mode setup), which drops the kernel into supervisor mode and
//! hands off to `kmain`. (Up to exercise 12 this called `kmain` directly; from
//! exercise 13 on it goes through `start`, so the kernel runs in supervisor
//! mode and the MMU truly takes effect.)

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
        "call start",         // machine-mode setup, then mret into kmain (S-mode)
        stack = sym STACK0,
        size = const STACK_SIZE,
        options(noreturn),
    );
}
