//! trap.rs — supervisor trap handling, now including interrupts.
//!
//! Exercise 13 handled *exceptions* (a breakpoint). This exercise adds
//! *interrupts*: asynchronous events that arrive while other code runs. We use
//! the periodic **timer** (set up in `start.rs`), which is forwarded to
//! supervisor mode as a software interrupt. Handling it on every tick is what
//! lets an OS take the CPU back from a running task — the basis of preemptive
//! multitasking.

use core::arch::{asm, global_asm};

/// Count of breakpoints handled (from exercise 13).
static mut TRAP_COUNT: usize = 0;
/// Count of timer ticks handled.
static mut TICKS: usize = 0;

/// Read the timer-tick counter. (UNDERSTAND — given.)
pub fn ticks() -> usize {
    unsafe { core::ptr::read_volatile(core::ptr::addr_of!(TICKS)) }
}

extern "C" {
    fn kernelvec();
}

/// The address `stvec` should hold. (UNDERSTAND — given.)
pub fn vector_addr() -> usize {
    let v: unsafe extern "C" fn() = kernelvec;
    v as usize
}

/// Point `stvec` at our trap vector. (Given — you wrote this in exercise 13.)
pub unsafe fn init() {
    let addr = vector_addr();
    asm!("csrw stvec, {}", in(reg) addr);
}

/// Turn supervisor interrupts on, so the forwarded timer ticks actually fire.
pub unsafe fn intr_on() {
    asm!("csrs sie, {}", in(reg) 1usize << 1); // SSIE: supervisor software interrupt
    asm!("csrs sstatus, {}", in(reg) 1usize << 1); // SIE: global supervisor interrupt enable
}

/// The Rust trap handler, called by `kernelvec`.
#[no_mangle]
pub extern "C" fn kerneltrap() {
    unsafe {
        let scause: usize;
        let sepc: usize;
        asm!("csrr {}, scause", out(reg) scause);
        asm!("csrr {}, sepc", out(reg) sepc);

        // The top bit of `scause` distinguishes an interrupt (1) from an
        // exception (0).
        if (scause >> 63) == 1 {
            // an interrupt. The forwarded timer is a supervisor software
            // interrupt (cause code 1).
            if scause & 0xff == 1 {
                // clear the pending bit so it doesn't re-fire forever
                let sip: usize;
                asm!("csrr {}, sip", out(reg) sip);
                asm!("csrw sip, {}", in(reg) sip & !2);
                TICKS += 1;
            }
        } else {
            // An exception. Handle a breakpoint, as in exercise 13.
            if scause == 3 {
                TRAP_COUNT += 1;
                asm!("csrw sepc, {}", in(reg) sepc + 4);
            }
        }
    }
}

// The supervisor trap vector: save the caller-saved registers, call
// `kerneltrap`, restore them, and `sret`. Works for both exceptions and
// interrupts. (UNDERSTAND — given.)
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
