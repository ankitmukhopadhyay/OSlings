//! start.rs — machine-mode startup. (UNDERSTAND, don't edit.)
//!
//! QEMU starts our kernel in **machine mode (M-mode)**, the most privileged
//! level. A real OS kernel runs one level down, in **supervisor mode (S-mode)**,
//! because that is where the MMU actually translates addresses and where we can
//! safely drop further to **user mode (U-mode)** for programs later.
//!
//! So this tiny machine-mode routine prepares the switch and then `mret`s into
//! `kmain` running in supervisor mode. It is the reason that, from this exercise
//! on, the page table you built in exercises 09 and 12 genuinely takes effect,
//! and that the supervisor trap registers (`stvec`, `scause`, `sret`) work.

use core::arch::asm;

// mstatus.MPP (bits 12..11) selects the privilege `mret` returns to.
const MSTATUS_MPP_MASK: usize = 0b11 << 11;
const MSTATUS_MPP_SUPERVISOR: usize = 0b01 << 11;

#[no_mangle]
pub unsafe extern "C" fn start() -> ! {
    // 1. Make `mret` land in supervisor mode (MPP = Supervisor).
    let mut mstatus: usize;
    asm!("csrr {0}, mstatus", out(reg) mstatus);
    mstatus &= !MSTATUS_MPP_MASK;
    mstatus |= MSTATUS_MPP_SUPERVISOR;
    asm!("csrw mstatus, {0}", in(reg) mstatus);

    // 2. Tell `mret` where to go: the kernel's `kmain`.
    asm!("la t0, kmain", "csrw mepc, t0", out("t0") _);

    // 3. Paging off for now; kmain turns the MMU on once it is in S-mode.
    asm!("csrw satp, zero");

    // 4. Delegate all exceptions and interrupts to supervisor mode, so traps go
    //    to our `stvec` handler instead of staying in machine mode.
    asm!("li t0, 0xffff", "csrw medeleg, t0", "csrw mideleg, t0", out("t0") _);

    // 5. Give supervisor mode access to all of physical memory (PMP entry 0
    //    covers everything, with read/write/execute).
    asm!("li t0, 0x3fffffffffffff", "csrw pmpaddr0, t0", out("t0") _);
    asm!("li t0, 0xf", "csrw pmpcfg0, t0", out("t0") _);

    // 6. Drop into supervisor mode at kmain.
    asm!("mret", options(noreturn));
}
