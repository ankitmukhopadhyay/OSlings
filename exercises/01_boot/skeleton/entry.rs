//! entry.rs — the boot trampoline.
//!
//! When QEMU's `virt` machine starts with `-bios none`, it loads our kernel
//! and jumps straight to the `_entry` symbol (named by kernel.ld) at machine
//! reset. At that moment there is *no usable stack pointer*. Rust code cannot
//! run without a stack, so the very first job — written in raw assembly — is
//! to point `sp` at some memory we own, then `call` into Rust (`kmain`).

use core::arch::asm;

// A 16 KiB stack for the kernel, reserved in .bss. `STACK0` is the BOTTOM of
// the stack; the stack grows downward, so we make `sp` point at the TOP
// (STACK0 + STACK_SIZE).
const STACK_SIZE: usize = 4096 * 4;

#[no_mangle]
static mut STACK0: [u8; STACK_SIZE] = [0; STACK_SIZE];

#[no_mangle]
#[link_section = ".entry"] // kernel.ld places this first, at 0x8000_0000
pub unsafe extern "C" fn _entry() -> ! {
    // IMPLEMENT: write the assembly that bootstraps Rust.
    //
    //   1. Load the address of STACK0 into `sp`.
    //   2. Add STACK_SIZE to `sp`, so it points at the TOP of the stack.
    //   3. `call kmain`.
    //
    // Fill in the four instruction lines below. The `asm!` operands at the
    // bottom (`stack = sym STACK0`, `size = const STACK_SIZE`) are already
    // wired up for you — use `{stack}` and `{size}` to refer to them, and use
    // a scratch register such as `t0` for the size.
    //
    // Example shapes (RISC-V):
    //     "la sp, {stack}"       load address of a symbol
    //     "li t0, {size}"        load an immediate constant
    //     "add sp, sp, t0"       sp = sp + t0
    //     "call kmain"           jump-and-link into Rust
    asm!(
        // IMPLEMENT: la sp, {stack}
        // IMPLEMENT: li t0, {size}
        // IMPLEMENT: add sp, sp, t0
        // IMPLEMENT: call kmain
        "",
        // stack = sym STACK0,     // <- uncomment once you reference {stack}
        // size  = const STACK_SIZE, // <- uncomment once you reference {size}
        options(noreturn),
    );
}
