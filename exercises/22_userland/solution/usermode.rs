//! usermode.rs — running user processes, now with a real scheduler.
//!
//! Exercises 18–20 could run only ONE user process at a time: `run` swtch-ed
//! into it, and its `exit` swtch-ed straight back. Fork changes everything —
//! now there can be *several* runnable processes, so we need a **scheduler**: a
//! loop that repeatedly picks a runnable process, `swtch`-es into it, and gets
//! control back when it yields or exits.
//!
//!   run(root) ──> scheduler loop
//!                    │  swtch in
//!                    ▼
//!                 a process runs (forkret ─> usertrapret ─> user code)
//!                    │  it yields (wait) / exits            │ ecall / trap
//!                    ▼  swtch back                          ▼
//!                 scheduler loop  <── proc_yield / exit_current <── usertrap
//!
//! The scheduler, the swtch glue (`forkret`, `proc_yield`, `exit_current`,
//! `ready`), and the trampoline are all given. Your job (in syscall.rs) is the
//! process-management logic on top: `fork` and `wait`.

use crate::memlayout::{PGSIZE, TRAMPOLINE};
use crate::proc::{self, Proc, ProcState};
use crate::sched::{RoundRobin, Scheduler};
use crate::swtch::{self, Context};
use crate::vm;
use core::arch::{asm, global_asm};
use core::ptr;

// ========================================================================
//  The trapframe. (UNDERSTAND — given; unchanged since exercise 18.)
// ========================================================================

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
//  The scheduler.
// ========================================================================

/// How one top-level run ended.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RunOutcome {
    /// The root process (and its whole tree) finished; this is its exit status.
    Exited(isize),
    /// A process did something illegal; scause says what.
    Faulted(usize),
    /// We gave up — a process never made progress (a bug caught by a watchdog).
    TimedOut,
}

/// The scheduler's own saved context. A process `swtch`-es HERE to give the CPU
/// back to the scheduler (see `proc_yield` / `exit_current`).
static mut SCHED_CTX: Context = Context::zero();
/// The process the CPU is currently running (or is about to).
static mut CURPROC: *mut Proc = ptr::null_mut();
/// A process faulted; the scheduler turns this into `Faulted`.
static mut FAULTED: bool = false;
static mut FAULT_CAUSE: usize = 0;
/// A watchdog tripped; the scheduler turns this into `TimedOut`.
static mut ABORT: bool = false;
#[cfg(feature = "harness")]
static mut U_TICKS: usize = 0;

/// (harness only) Wall-clock budget for one run, measured with the `time` CSR
/// (as in exercise 14 — a CPU-speed-independent bound). A correct program tree
/// finishes in milliseconds; a stuck fork/wait ping-pongs in the scheduler, so
/// we cut it off after ~3 seconds with a clean TimedOut instead of hanging.
/// The `time` CSR ticks at 10 MHz on QEMU `virt`, so 3s = 30_000_000 ticks.
#[cfg(feature = "harness")]
const SCHED_TIMEOUT_TICKS: u64 = 30_000_000;
/// (harness only) Timer ticks a process may take in USER mode before we assume
/// it is stuck in a loop of its own (a second watchdog, for the rare case a
/// process spins without ever trapping back to the scheduler).
#[cfg(feature = "harness")]
const MAX_U_TICKS: usize = 50;

/// Read the `time` CSR (a free-running counter; see exercise 14). (harness only.)
#[cfg(feature = "harness")]
unsafe fn rdtime() -> u64 {
    let t: u64;
    asm!("csrr {}, time", out(reg) t);
    t
}

/// The process currently running. Syscalls (fork/wait/exit/read/write/…) act on
/// this one.
pub fn curproc() -> *mut Proc {
    unsafe { CURPROC }
}

/// Make a freshly-built process schedulable: when the scheduler first `swtch`-es
/// into it, execution starts at `forkret` on the process's own kernel stack.
/// Both `exec` (in exec.rs) and `fork` call this. (UNDERSTAND — given.)
pub unsafe fn ready(p: *mut Proc) {
    (*p).context = Context::zero();
    (*p).context.ra = forkret as *const () as usize;
    (*p).context.sp = (*p).kstack + PGSIZE;
}

/// Run process `root` and everything it forks, and return how `root` ended.
/// This drives the scheduler loop until the root process becomes a Zombie.
/// (UNDERSTAND — given; the caller frees `root` afterward, as before.)
pub unsafe fn run(root: *mut Proc) -> RunOutcome {
    FAULTED = false;
    ABORT = false;
    #[cfg(feature = "harness")]
    {
        U_TICKS = 0;
    }

    // let the timer's forwarded ticks reach processes while they run in user
    // mode; the KERNEL keeps interrupts off (sstatus.SIE) so scheduling stays
    // deterministic.
    asm!("csrs sie, {}", in(reg) 1usize << 1);

    let outcome = scheduler(root);

    // hand the kernel its interrupts back for the interactive console.
    #[cfg(not(feature = "harness"))]
    crate::trap::intr_on();
    outcome
}

/// The scheduler loop: pick a runnable process (with the round-robin policy you
/// wrote in exercise 06!), run it until it yields or exits, and repeat until
/// the root process has finished. (UNDERSTAND — given.)
unsafe fn scheduler(root: *mut Proc) -> RunOutcome {
    let mut policy = RoundRobin::new();
    #[cfg(feature = "harness")]
    let deadline = rdtime() + SCHED_TIMEOUT_TICKS;

    loop {
        // gather every process's state for the policy to choose from.
        let mut states = [ProcState::Unused; crate::param::NPROC];
        for i in 0..crate::param::NPROC {
            states[i] = (*proc::proc_at(i)).state;
        }

        match policy.pick_next(&states) {
            Some(i) => {
                let p = proc::proc_at(i);
                (*p).state = ProcState::Running;
                CURPROC = p;
                // run it: control returns here when it yields (proc_yield) or
                // exits (exit_current).
                swtch::swtch(ptr::addr_of_mut!(SCHED_CTX), ptr::addr_of_mut!((*p).context));
                CURPROC = ptr::null_mut();
            }
            None => {
                // nothing runnable. Either the root finished, or we deadlocked.
                if (*root).state == ProcState::Zombie {
                    return done(root);
                }
                cleanup_except(root);
                return RunOutcome::TimedOut;
            }
        }

        // a process just gave the CPU back. See how things stand.
        if FAULTED {
            cleanup_except(root);
            return RunOutcome::Faulted(FAULT_CAUSE);
        }
        if ABORT {
            cleanup_except(root);
            return RunOutcome::TimedOut;
        }
        if (*root).state == ProcState::Zombie {
            return done(root);
        }

        // Watchdog: if we have been scheduling for too long without the root
        // finishing, something is stuck (e.g. a wait that never reaps). Give up
        // cleanly rather than spin forever.
        #[cfg(feature = "harness")]
        if rdtime() > deadline {
            cleanup_except(root);
            return RunOutcome::TimedOut;
        }
    }
}

/// The root finished: capture its exit status, free any leftover children, and
/// leave the root itself for the caller to free. (Given.)
unsafe fn done(root: *mut Proc) -> RunOutcome {
    let status = (*root).xstate;
    cleanup_except(root);
    RunOutcome::Exited(status)
}

/// Free every live process except `root` — used to tidy up children/orphans
/// when a run ends. (Given.)
unsafe fn cleanup_except(root: *mut Proc) {
    for i in 0..crate::param::NPROC {
        let q = proc::proc_at(i);
        if q != root && (*q).state != ProcState::Unused {
            proc::freeproc(q);
        }
    }
}

/// The first thing a freshly-scheduled process does: drop into user mode. For a
/// forked child, its trapframe was copied from the parent (with a0 = 0), so it
/// resumes right after the `fork` that created it. (Given.)
unsafe extern "C" fn forkret() {
    usertrapret(CURPROC);
}

/// Give the CPU back to the scheduler, staying Runnable so we run again later.
/// `wait` calls this to block until a child exits. When the scheduler picks us
/// again, execution resumes right where the `swtch` left off. (Given.)
pub unsafe fn proc_yield(p: *mut Proc) {
    (*p).state = ProcState::Runnable;
    swtch::swtch(ptr::addr_of_mut!((*p).context), ptr::addr_of_mut!(SCHED_CTX));
}

/// End the current process: record its exit status, mark it a Zombie (so a
/// parent's `wait` can find it), and hand the CPU back to the scheduler for
/// good. Never returns. (Given — this is the model your `wait` reaps.)
pub unsafe fn exit_current(status: isize) -> ! {
    let p = CURPROC;
    (*p).xstate = status;
    (*p).state = ProcState::Zombie;
    swtch::swtch(ptr::addr_of_mut!((*p).context), ptr::addr_of_mut!(SCHED_CTX));
    unreachable!() // the scheduler never swtch-es back into a Zombie
}

// ========================================================================
//  usertrap: every trap from user mode lands here (via uservec).
//  (UNDERSTAND — given; you wrote the ecall branch in exercise 18.)
// ========================================================================

#[no_mangle]
pub extern "C" fn usertrap() -> ! {
    unsafe {
        asm!("csrw stvec, {}", in(reg) crate::trap::vector_addr());

        let scause: usize;
        asm!("csrr {}, scause", out(reg) scause);

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
                    // forwarded timer tick: clear the pending bit.
                    let sip: usize;
                    asm!("csrr {}, sip", out(reg) sip);
                    asm!("csrw sip, {}", in(reg) sip & !2);
                    #[cfg(feature = "harness")]
                    {
                        U_TICKS += 1;
                        if U_TICKS > MAX_U_TICKS {
                            ABORT = true;
                            exit_current(-1); // bail out to the scheduler
                        }
                    }
                }
                9 => crate::console::intr(),
                _ => {}
            }
        } else {
            // The program did something illegal: record it and end the process.
            FAULTED = true;
            FAULT_CAUSE = scause;
            exit_current(-1);
        }

        usertrapret(p)
    }
}

/// Return to user mode. (UNDERSTAND — given.)
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
