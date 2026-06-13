// ╔══════════════════════════════════════════════════════════════════════╗
// ║  Exercise 00 — Rust Kernel Basics                                      ║
// ║  Goal: make this crate COMPILE for a bare-metal RISC-V target.         ║
// ╚══════════════════════════════════════════════════════════════════════╝
//
// A normal Rust program links against `std`, which assumes an operating
// system underneath it (files, threads, a heap, a `main` that the C runtime
// calls). We ARE the operating system, so none of that exists yet. We must
// opt out of it.
//
// This file will not compile until you complete the two IMPLEMENT steps.
// Run `oslings run 00_rust_kernel_basics` (or `oslings watch`) to check.

// IMPLEMENT (1): Add the two inner attributes that opt out of the standard
//   library and the standard `main` entry shim. They go at the VERY TOP of
//   this file (above everything, even this comment is fine to keep below
//   them). Both start with `#![no_`.
//
//     - one disables the standard library (we only get `core`)
//     - one disables the compiler-generated `main` wrapper

use core::panic::PanicInfo;

// IMPLEMENT (2): Define the panic handler.
//   `std` normally provides one; without it the compiler demands we supply
//   exactly one function marked `#[panic_handler]`. Signature:
//
//       #[panic_handler]
//       fn panic(info: &PanicInfo) -> ! {
//           loop {}
//       }
//
//   UNDERSTAND: the return type `!` is the "never" type. It promises this
//   function never returns — there is nowhere to return *to* in a kernel.

// UNDERSTAND: With `#![no_main]` there is no Rust `main`. Instead the linker
//   script (kernel.ld) names `_entry` as the program's entry point, and that
//   is the symbol QEMU jumps to. For now it just spins forever; in exercise
//   01 you turn it into a real boot sequence.
#[no_mangle]
pub extern "C" fn _entry() -> ! {
    loop {}
}
