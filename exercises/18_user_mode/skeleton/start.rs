//! start.rs — machine-mode startup: enter supervisor mode and start the timer.
//! (UNDERSTAND, don't edit.)
//!
//! As in exercise 13, this drops the kernel from machine mode into supervisor
//! mode. New here: it also starts a periodic **timer**. The timer hardware
//! (the CLINT) only speaks machine mode, so we set up a small machine-mode
//! handler, `timervec`, that fires on every timer interrupt, reschedules the
//! next one, and then forwards the tick to supervisor mode as a *software
//! interrupt* that your kernel handles.

use core::arch::{asm, global_asm};

const MSTATUS_MPP_MASK: usize = 0b11 << 11;
const MSTATUS_MPP_SUPERVISOR: usize = 0b01 << 11;

// The CLINT (core-local interruptor) on the QEMU `virt` machine drives time.
const CLINT_MTIME: usize = 0x0200_0000 + 0xBFF8; // the current time
const CLINT_MTIMECMP0: usize = 0x0200_0000 + 0x4000; // hart 0's compare register
const INTERVAL: u64 = 1_000_000; // ticks between interrupts (about 0.1s at 10 MHz)

// Scratch space the machine-mode timer vector uses to save a few registers.
static mut TIMER_SCRATCH: [u64; 5] = [0; 5];

#[no_mangle]
pub unsafe extern "C" fn start() -> ! {
    // mret should land in supervisor mode.
    let mut mstatus: usize;
    asm!("csrr {0}, mstatus", out(reg) mstatus);
    mstatus &= !MSTATUS_MPP_MASK;
    mstatus |= MSTATUS_MPP_SUPERVISOR;
    asm!("csrw mstatus, {0}", in(reg) mstatus);

    // mret target: kmain.
    asm!("la t0, kmain", "csrw mepc, t0", out("t0") _);

    // paging off for now; kmain turns the MMU on once it is in supervisor mode.
    asm!("csrw satp, zero");

    // delegate all traps to supervisor mode.
    asm!("li t0, 0xffff", "csrw medeleg, t0", "csrw mideleg, t0", out("t0") _);

    // give supervisor mode access to all of physical memory.
    asm!("li t0, 0x3fffffffffffff", "csrw pmpaddr0, t0", out("t0") _);
    asm!("li t0, 0xf", "csrw pmpcfg0, t0", out("t0") _);

    // let supervisor mode read the time/cycle counters (the `time` CSR).
    asm!("li t0, 0xffffffff", "csrw mcounteren, t0", out("t0") _);

    // start periodic timer interrupts (handled in machine mode by timervec,
    // then forwarded to supervisor mode).
    timerinit();

    // drop into supervisor mode at kmain.
    asm!("mret", options(noreturn));
}

/// Schedule the first timer interrupt and install the machine-mode timer vector.
/// The machine timer fires even while the CPU runs in supervisor mode.
unsafe fn timerinit() {
    // schedule the first interrupt
    let now = core::ptr::read_volatile(CLINT_MTIME as *const u64);
    core::ptr::write_volatile(CLINT_MTIMECMP0 as *mut u64, now + INTERVAL);

    // scratch layout: [0..2] save area, [3] = &mtimecmp, [4] = interval
    let scratch = core::ptr::addr_of_mut!(TIMER_SCRATCH);
    (*scratch)[3] = CLINT_MTIMECMP0 as u64;
    (*scratch)[4] = INTERVAL;
    asm!("csrw mscratch, {}", in(reg) scratch as usize);

    // machine-mode timer trap vector
    asm!("la t0, timervec", "csrw mtvec, t0", out("t0") _);

    // enable the machine timer interrupt (mie.MTIE, bit 7)
    asm!("csrs mie, {}", in(reg) 1usize << 7);
}

// Machine-mode timer interrupt handler. On each timer interrupt it reschedules
// the next one and raises a supervisor *software* interrupt (sip.SSIP), which
// the supervisor-mode kernel handles as a timer tick. (UNDERSTAND — given.)
global_asm!(
    r#"
.align 4
.globl timervec
timervec:
    # mscratch points at TIMER_SCRATCH; swap it into a0 so we can use it.
    csrrw a0, mscratch, a0
    sd a1, 0(a0)
    sd a2, 8(a0)
    sd a3, 16(a0)

    # reschedule the next timer interrupt: mtimecmp += interval
    ld a1, 24(a0)        # a1 = &mtimecmp
    ld a2, 32(a0)        # a2 = interval
    ld a3, 0(a1)         # a3 = current mtimecmp
    add a3, a3, a2
    sd a3, 0(a1)

    # raise a supervisor software interrupt (sip.SSIP, bit 1)
    li a1, 2
    csrw sip, a1

    ld a3, 16(a0)
    ld a2, 8(a0)
    ld a1, 0(a0)
    csrrw a0, mscratch, a0
    mret
"#
);
