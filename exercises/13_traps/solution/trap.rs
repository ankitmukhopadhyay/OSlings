//! trap.rs — supervisor trap handling.
//!
//! A *trap* is the CPU's way of stopping what it's doing and jumping to the
//! kernel when something needs attention: an **exception** (the running code did
//! something that needs handling — a breakpoint, a bad memory access, an
//! `ecall`) or an **interrupt** (a device or the timer wants the CPU; next
//! exercise). When a trap happens in supervisor mode, the hardware:
//!   * records why, in the `scause` register,
//!   * records where it happened, in `sepc` (the faulting instruction's address),
//!   * and jumps to the address in the `stvec` register.
//!
//! So we point `stvec` at an assembly entry (`kernelvec`), which saves
//! registers and calls our Rust handler (`kerneltrap`). When the handler
//! returns, `kernelvec` restores the registers and runs `sret`, which resumes
//! at `sepc`.

use core::arch::{asm, global_asm};

/// How many traps we've handled — used by the exercise's self-check.
static mut TRAP_COUNT: usize = 0;

/// Read the handled-trap counter. (UNDERSTAND — given.)
pub fn trap_count() -> usize {
    unsafe { core::ptr::read_volatile(core::ptr::addr_of!(TRAP_COUNT)) }
}

extern "C" {
    /// The assembly trap vector, defined in the `global_asm!` block below.
    fn kernelvec();
}

/// The address `stvec` should hold: our trap vector. (UNDERSTAND — given.)
pub fn vector_addr() -> usize {
    let v: unsafe extern "C" fn() = kernelvec;
    v as usize
}

/// Install supervisor trap handling: point `stvec` at our trap vector.
pub unsafe fn init() {
    let addr = vector_addr();
    asm!("csrw stvec, {}", in(reg) addr);
}

/// The Rust trap handler, called by `kernelvec` after it has saved registers.
#[no_mangle]
pub extern "C" fn kerneltrap() {
    unsafe {
        let scause: usize;
        let sepc: usize;
        asm!("csrr {}, scause", out(reg) scause);
        asm!("csrr {}, sepc", out(reg) sepc);

        if scause == 3 {
            // breakpoint: count it and resume past the 4-byte ebreak
            TRAP_COUNT += 1;
            asm!("csrw sepc, {}", in(reg) sepc + 4);
        }
    }
}

// The assembly trap vector. On a trap the hardware leaves all general-purpose
// registers untouched, so before we can run Rust we must save the ones our C
// handler may clobber (the caller-saved registers), call `kerneltrap`, then
// restore them and `sret` — making the whole trap invisible to the interrupted
// code. (UNDERSTAND — given; read it, but you don't edit assembly here.)
global_asm!(
    r#"
.globl kernelvec
.align 4
kernelvec:
    addi sp, sp, -128
    sd ra,   0(sp)
    sd t0,   8(sp)
    sd t1,  16(sp)
    sd t2,  24(sp)
    sd a0,  32(sp)
    sd a1,  40(sp)
    sd a2,  48(sp)
    sd a3,  56(sp)
    sd a4,  64(sp)
    sd a5,  72(sp)
    sd a6,  80(sp)
    sd a7,  88(sp)
    sd t3,  96(sp)
    sd t4, 104(sp)
    sd t5, 112(sp)
    sd t6, 120(sp)

    call kerneltrap

    ld ra,   0(sp)
    ld t0,   8(sp)
    ld t1,  16(sp)
    ld t2,  24(sp)
    ld a0,  32(sp)
    ld a1,  40(sp)
    ld a2,  48(sp)
    ld a3,  56(sp)
    ld a4,  64(sp)
    ld a5,  72(sp)
    ld a6,  80(sp)
    ld a7,  88(sp)
    ld t3,  96(sp)
    ld t4, 104(sp)
    ld t5, 112(sp)
    ld t6, 120(sp)
    addi sp, sp, 128

    sret
"#
);
