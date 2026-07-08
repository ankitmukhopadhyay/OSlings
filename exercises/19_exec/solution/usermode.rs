//! usermode.rs — running a program in user mode, and getting back out.
//!
//! Exercise 18 built this: the trampoline that switches between the kernel
//! and user worlds, the `Trapframe` that parks the user's registers, and
//! `run`/`finish`/`usertrap` that launch a process and catch it when it
//! traps back in. That machinery is unchanged here.
//!
//! What moved OUT: exercise 18 also had a `setup()` that hard-coded one
//! embedded program onto one page. In this exercise a real loader (exec.rs)
//! takes over building processes, so `setup` and the baked-in program are
//! gone. `run` now takes any process exec.rs hands it.
//!
//!   run() ── swtch ──> user_entry() ── usertrapret() ── sret ──> USER CODE
//!                                                                   │ ecall
//!   run() <── swtch ── finish() <── sys_exit <── usertrap() <── uservec

use crate::memlayout::{PGSIZE, TRAMPOLINE};
use crate::proc::{Proc, ProcState};
use crate::swtch::{self, Context};
use crate::vm;
use core::arch::{asm, global_asm};
use core::ptr;

// ========================================================================
//  The trapframe.
// ========================================================================

/// Everything the kernel must remember about a user program's CPU state to
/// pause it and later resume it exactly where it was. `uservec` (below)
/// stores registers at these exact byte offsets, so the field order here is
/// load-bearing — do not reorder. (UNDERSTAND — given.)
#[repr(C)]
pub struct Trapframe {
    pub kernel_satp: u64,   //   0: the kernel's page table, for uservec
    pub kernel_sp: u64,     //   8: this process's kernel stack top
    pub kernel_trap: u64,   //  16: address of usertrap()
    pub epc: u64,           //  24: user program counter (where to resume)
    pub kernel_hartid: u64, //  32: unused here; keeps the classic xv6 layout
    pub ra: u64,            //  40
    pub sp: u64,            //  48
    pub gp: u64,            //  56
    pub tp: u64,            //  64
    pub t0: u64,            //  72
    pub t1: u64,            //  80
    pub t2: u64,            //  88
    pub s0: u64,            //  96
    pub s1: u64,            // 104
    pub a0: u64,            // 112
    pub a1: u64,            // 120
    pub a2: u64,            // 128
    pub a3: u64,            // 136
    pub a4: u64,            // 144
    pub a5: u64,            // 152
    pub a6: u64,            // 160
    pub a7: u64,            // 168
    pub s2: u64,            // 176
    pub s3: u64,            // 184
    pub s4: u64,            // 192
    pub s5: u64,            // 200
    pub s6: u64,            // 208
    pub s7: u64,            // 216
    pub s8: u64,            // 224
    pub s9: u64,            // 232
    pub s10: u64,           // 240
    pub s11: u64,           // 248
    pub t3: u64,            // 256
    pub t4: u64,            // 264
    pub t5: u64,            // 272
    pub t6: u64,            // 280
}

// ========================================================================
//  The trampoline: uservec (user -> kernel) and userret (kernel -> user).
//  (UNDERSTAND — given; read it slowly in exercise 18. Unchanged here.)
// ========================================================================

extern "C" {
    fn trampoline(); // first byte of the trampoline code
    fn uservec(); // where traps from user mode land
    fn userret(); // the road back to user mode
    fn trampoline_end(); // one past the last byte
}

global_asm!(
    r#"
.globl trampoline
.globl uservec
.globl userret
.globl trampoline_end
.align 4
trampoline:
uservec:
    csrrw a0, sscratch, a0      # a0 = TRAPFRAME, sscratch = user a0

    sd ra, 40(a0)
    sd sp, 48(a0)
    sd gp, 56(a0)
    sd tp, 64(a0)
    sd t0, 72(a0)
    sd t1, 80(a0)
    sd t2, 88(a0)
    sd s0, 96(a0)
    sd s1, 104(a0)
    sd a1, 120(a0)
    sd a2, 128(a0)
    sd a3, 136(a0)
    sd a4, 144(a0)
    sd a5, 152(a0)
    sd a6, 160(a0)
    sd a7, 168(a0)
    sd s2, 176(a0)
    sd s3, 184(a0)
    sd s4, 192(a0)
    sd s5, 200(a0)
    sd s6, 208(a0)
    sd s7, 216(a0)
    sd s8, 224(a0)
    sd s9, 232(a0)
    sd s10, 240(a0)
    sd s11, 248(a0)
    sd t3, 256(a0)
    sd t4, 264(a0)
    sd t5, 272(a0)
    sd t6, 280(a0)
    csrr t0, sscratch
    sd t0, 112(a0)

    ld sp, 8(a0)                # kernel_sp
    ld t0, 16(a0)               # kernel_trap = usertrap
    ld t1, 0(a0)                # kernel_satp

    sfence.vma zero, zero
    csrw satp, t1
    sfence.vma zero, zero

    jr t0

userret:
    sfence.vma zero, zero
    csrw satp, a0
    sfence.vma zero, zero

    li a0, {trapframe}

    ld t0, 112(a0)
    csrw sscratch, t0

    ld ra, 40(a0)
    ld sp, 48(a0)
    ld gp, 56(a0)
    ld tp, 64(a0)
    ld t0, 72(a0)
    ld t1, 80(a0)
    ld t2, 88(a0)
    ld s0, 96(a0)
    ld s1, 104(a0)
    ld a1, 120(a0)
    ld a2, 128(a0)
    ld a3, 136(a0)
    ld a4, 144(a0)
    ld a5, 152(a0)
    ld a6, 160(a0)
    ld a7, 168(a0)
    ld s2, 176(a0)
    ld s3, 184(a0)
    ld s4, 192(a0)
    ld s5, 200(a0)
    ld s6, 208(a0)
    ld s7, 216(a0)
    ld s8, 224(a0)
    ld s9, 232(a0)
    ld s10, 240(a0)
    ld s11, 248(a0)
    ld t3, 256(a0)
    ld t4, 264(a0)
    ld t5, 272(a0)
    ld t6, 280(a0)

    csrrw a0, sscratch, a0
    sret
trampoline_end:
"#,
    trapframe = const crate::memlayout::TRAPFRAME,
);

// ========================================================================
//  Running a process.
// ========================================================================

/// How one user-program run ended.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RunOutcome {
    /// It called exit(status).
    Exited(isize),
    /// It did something illegal (bad memory access etc.); scause says what.
    Faulted(usize),
    /// (harness only) Its system calls were never answered, so we gave up.
    TimedOut,
}

// The kernel parks here (RUN_CTX) while the user program runs; finish()
// swtch-es back. DISCARD_CTX is a write-only scratch save area for the side
// we are abandoning. (Exercise 05's Context, reused.)
static mut RUN_CTX: Context = Context::zero();
static mut USER_CTX: Context = Context::zero();
static mut DISCARD_CTX: Context = Context::zero();
static mut CURPROC: *mut Proc = ptr::null_mut();
static mut OUTCOME: RunOutcome = RunOutcome::TimedOut;
static mut CAME_FROM_USER: bool = false;
#[cfg(feature = "harness")]
static mut U_TICKS: usize = 0;

/// The process currently in (or headed for) user mode.
pub fn curproc() -> *mut Proc {
    unsafe { CURPROC }
}

/// Did we observe a trap that really arrived from user mode (sstatus.SPP=0)?
pub fn came_from_user() -> bool {
    unsafe { CAME_FROM_USER }
}

/// Run a set-up process in user mode and wait for it to finish. This is a
/// miniature of what a scheduler does: swtch away to start the program, and
/// the program's exit swtch-es back here. (UNDERSTAND — given.)
pub unsafe fn run(p: *mut Proc) -> RunOutcome {
    CURPROC = p;
    OUTCOME = RunOutcome::TimedOut;
    CAME_FROM_USER = false;
    #[cfg(feature = "harness")]
    {
        U_TICKS = 0;
    }

    // let the timer's forwarded tick reach us while in user mode (sie.SSIE);
    // whether it can interrupt the KERNEL is still governed by sstatus.SIE
    asm!("csrs sie, {}", in(reg) 1usize << 1);

    (*p).state = ProcState::Running;
    swtch::init_context(
        ptr::addr_of_mut!(USER_CTX),
        user_entry as *const () as usize,
        (*p).kstack + PGSIZE,
    );
    // swtch saves US into RUN_CTX and starts user_entry on the process's
    // kernel stack. We wake up back here when finish() swtch-es to RUN_CTX.
    swtch::swtch(ptr::addr_of_mut!(RUN_CTX), ptr::addr_of_mut!(USER_CTX));

    (*p).state = ProcState::Zombie;
    CURPROC = ptr::null_mut();
    // give the kernel its interrupts back (taking the trap turned them off)
    #[cfg(not(feature = "harness"))]
    crate::trap::intr_on();
    OUTCOME
}

/// First (and only) stop on the new process's kernel stack: dive into user
/// mode. Every later return to user mode also goes through usertrapret.
unsafe extern "C" fn user_entry() {
    usertrapret(CURPROC);
}

/// The way OUT of a finished user program, called by sys_exit (and by the
/// fault/timeout paths): record how it ended, then swtch back to run().
pub unsafe fn finish(outcome: RunOutcome) -> ! {
    OUTCOME = outcome;
    swtch::swtch(ptr::addr_of_mut!(DISCARD_CTX), ptr::addr_of_mut!(RUN_CTX));
    unreachable!() // nothing ever swtch-es back into DISCARD_CTX
}

// ========================================================================
//  usertrap: every trap from user mode lands here (via uservec).
//  (UNDERSTAND — given; you wrote the ecall branch in exercise 18.)
// ========================================================================

#[no_mangle]
pub extern "C" fn usertrap() -> ! {
    unsafe {
        // we are back in the kernel: kernel traps go to kernelvec again
        asm!("csrw stvec, {}", in(reg) crate::trap::vector_addr());

        let scause: usize;
        let sstatus: usize;
        asm!("csrr {}, scause", out(reg) scause);
        asm!("csrr {}, sstatus", out(reg) sstatus);

        if sstatus & (1 << 8) == 0 {
            CAME_FROM_USER = true;
        }

        let p = CURPROC;
        let tf = (*p).trapframe;

        let sepc: usize;
        asm!("csrr {}, sepc", out(reg) sepc);
        (*tf).epc = sepc as u64;

        if scause == 8 {
            // ECALL from user mode: a system call.
            (*tf).epc += 4;
            let ret = crate::syscall::dispatch(
                (*tf).a7 as usize,
                (*tf).a0 as usize,
                (*tf).a1 as usize,
                (*tf).a2 as usize,
            );
            (*tf).a0 = ret as u64;
        } else if (scause >> 63) == 1 {
            match scause & 0xff {
                1 => {
                    // forwarded timer tick: clear the pending bit
                    let sip: usize;
                    asm!("csrr {}, sip", out(reg) sip);
                    asm!("csrw sip, {}", in(reg) sip & !2);
                    #[cfg(feature = "harness")]
                    {
                        U_TICKS += 1;
                        if U_TICKS > 30 {
                            finish(RunOutcome::TimedOut);
                        }
                    }
                }
                9 => crate::console::intr(),
                _ => {}
            }
        } else {
            // The program did something illegal. End the run and report it.
            finish(RunOutcome::Faulted(scause));
        }

        usertrapret(p)
    }
}

/// Return to user mode: stage everything uservec will need for the NEXT
/// trap, aim sret at the user program, and jump through the trampoline.
/// (UNDERSTAND — given.)
pub unsafe fn usertrapret(p: *mut Proc) -> ! {
    let tf = (*p).trapframe;

    let tramp_uservec =
        TRAMPOLINE + (uservec as *const () as usize - trampoline as *const () as usize);
    asm!("csrw stvec, {}", in(reg) tramp_uservec);

    let kernel_satp: usize;
    asm!("csrr {}, satp", out(reg) kernel_satp);
    (*tf).kernel_satp = kernel_satp as u64;
    (*tf).kernel_sp = ((*p).kstack + PGSIZE) as u64;
    (*tf).kernel_trap = usertrap as *const () as usize as u64;

    let mut sstatus: usize;
    asm!("csrr {}, sstatus", out(reg) sstatus);
    sstatus &= !(1 << 8); // SPP = 0: return to user mode
    sstatus |= 1 << 5; // SPIE = 1: interrupts on once there
    asm!("csrw sstatus, {}", in(reg) sstatus);

    asm!("csrw sepc, {}", in(reg) (*tf).epc as usize);

    let user_satp = vm::make_satp((*p).pagetable);

    let tramp_userret =
        TRAMPOLINE + (userret as *const () as usize - trampoline as *const () as usize);
    let f: extern "C" fn(usize) -> ! = core::mem::transmute(tramp_userret);
    f(user_satp)
}
