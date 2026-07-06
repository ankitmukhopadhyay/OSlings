//! usermode.rs — running a program in user mode, and getting back out.
//!
//! This is the machinery that drops the CPU to its lowest privilege level
//! (U-mode) to run a program, and catches it again when the program traps
//! back in (a system call, an interrupt, or a fault):
//!
//!   run() ── swtch ──> user_entry() ── usertrapret() ── sret ──> USER CODE
//!                                                                   │ ecall
//!   run() <── swtch ── finish() <── sys_exit <── usertrap() <── uservec
//!
//! The pieces:
//!   - `Trapframe`: the parking lot for all 31 user registers (one page per
//!     process, mapped at TRAPFRAME in the user's page table).
//!   - the trampoline (`uservec`/`userret`): the assembly that saves/restores
//!     those registers and switches page tables. Mapped at the SAME virtual
//!     address in the kernel's and every user's page table.
//!   - `usertrap`: the Rust handler for anything a user program does that
//!     needs the kernel. YOUR JOB: the system-call branch.
//!   - `usertrapret`: stages everything and drops back into user mode.

use crate::memlayout::{PGSIZE, TRAMPOLINE, USER_CODE, USER_STACK_TOP};
use crate::proc::{self, Proc, ProcState};
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
//
//  Why does this code get its own page, mapped at the same virtual address
//  in BOTH page tables? Because it changes `satp` (the page table register)
//  while it is running. The instant satp changes, every address means
//  something new — including the address of the NEXT instruction. The only
//  way to survive that is to be standing on a page that both page tables
//  map to the same place. (UNDERSTAND — given; read it slowly, this is the
//  cleverest page of code in the kernel.)
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
    # A trap from user mode lands here (stvec points here while user code
    # runs). We are now in supervisor mode, but satp still holds the USER
    # page table and every user register still holds the user's values.
    # sscratch was parked pointing at the trapframe; swap it with a0 so we
    # have one register to work with.
    csrrw a0, sscratch, a0      # now a0 = TRAPFRAME, sscratch = user a0

    # park every user register in the trapframe
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
    # the user a0 itself is sitting in sscratch; park it too
    csrr t0, sscratch
    sd t0, 112(a0)

    # pick up what usertrapret left for us: a kernel stack to run on, the
    # address of usertrap(), and the kernel's page table
    ld sp, 8(a0)                # kernel_sp
    ld t0, 16(a0)               # kernel_trap = usertrap
    ld t1, 0(a0)                # kernel_satp

    # switch worlds: install the kernel page table
    sfence.vma zero, zero
    csrw satp, t1
    sfence.vma zero, zero

    # jump to usertrap(). It never returns here.
    jr t0

userret:
    # usertrapret() calls here with a0 = the user's satp value.
    # switch worlds: install the USER page table
    sfence.vma zero, zero
    csrw satp, a0
    sfence.vma zero, zero

    # the trapframe is mapped at TRAPFRAME in the user's page table too
    li a0, {trapframe}

    # stash the user's a0 in sscratch for the final swap below
    ld t0, 112(a0)
    csrw sscratch, t0

    # restore every other user register from the trapframe
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

    # last move: a0 = the user's a0, sscratch = TRAPFRAME (ready for the
    # next uservec)
    csrrw a0, sscratch, a0

    # back to user mode, at the address usertrapret put in sepc
    sret
trampoline_end:
"#,
    trapframe = const crate::memlayout::TRAPFRAME,
);

// ========================================================================
//  The user program.
//
//  There is no compiler or loader for user programs yet (exercise 19 builds
//  the loader). So our first user program is written directly in assembly
//  and baked into the kernel image as DATA; `setup()` copies its bytes onto
//  a fresh page that gets mapped — with PTE_U — at address 0.
//
//  What it does, start to finish:
//      write(1, "hello from user mode\n", 21)   # syscall 16
//      pid = getpid()                           # syscall 11
//      exit(pid + 41)                           # syscall 2
//
//  Note the calling convention: the syscall NUMBER goes in a7, the
//  arguments in a0..a2, then `ecall`. The return value comes back in a0.
//  (UNDERSTAND — given.)
// ========================================================================

extern "C" {
    static user_prog_start: u8;
    static user_prog_end: u8;
}

global_asm!(
    r#"
.section .rodata
.globl user_prog_start
.globl user_prog_end
.balign 8
user_prog_start:
.option push
.option norelax
    la   a1, user_msg           # a1 = address of the message (pc-relative,
                                #      so it survives being copied to page 0)
    li   a2, 21                 # a2 = message length (21 bytes, counted below)
    li   a0, 1                  # a0 = "file descriptor" 1: the console
    li   a7, 16                 # a7 = SYS_WRITE
    ecall                       # trap into the kernel; kernel prints the msg

    li   a7, 11                 # a7 = SYS_GETPID
    ecall                       # returns our process id in a0

    addi a0, a0, 41             # exit status = pid + 41 (proves the return
                                #   value really made it back from the kernel)
    li   a7, 2                  # a7 = SYS_EXIT
    ecall                       # never returns

1:  j 1b                        # (not reached)

user_msg:
    .ascii "hello from user mode\n"
.option pop
user_prog_end:
"#
);

// ========================================================================
//  Creating and running the process.
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

/// Build a ready-to-run user process: allocate it, wire up its page table,
/// and load the program. Returns null if out of memory. (UNDERSTAND — given.)
pub unsafe fn setup() -> *mut Proc {
    let p = proc::allocproc(); // pid + empty page table + trapframe + kstack
    if p.is_null() {
        return p;
    }
    // map the trampoline + trapframe into the new page table (kernel-only)
    if proc::proc_pagetable(p).is_err() {
        proc::freeproc(p);
        return ptr::null_mut();
    }

    // copy the program's instructions onto a fresh page...
    let code = kalloc_zeroed();
    let stack = kalloc_zeroed();
    if code.is_null() || stack.is_null() {
        proc::freeproc(p);
        return ptr::null_mut();
    }
    let src = ptr::addr_of!(user_prog_start) as usize;
    let len = ptr::addr_of!(user_prog_end) as usize - src;
    ptr::copy_nonoverlapping(src as *const u8, code, len);
    asm!("fence.i"); // we wrote instructions: flush the instruction fetch path

    // ...and hand both pages to the user's page table (this is YOUR
    // map_user_pages, in vm.rs)
    if vm::map_user_pages((*p).pagetable, code as usize, stack as usize).is_err() {
        proc::freeproc(p);
        return ptr::null_mut();
    }

    // where the program starts: pc = 0, stack pointer = top of its stack page
    (*(*p).trapframe).epc = USER_CODE as u64;
    (*(*p).trapframe).sp = USER_STACK_TOP as u64;
    p
}

unsafe fn kalloc_zeroed() -> *mut u8 {
    let page = crate::kalloc::kalloc();
    if !page.is_null() {
        ptr::write_bytes(page, 0, PGSIZE);
    }
    page
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

        // sstatus.SPP (bit 8) tells where the trap came from: 0 = user mode.
        // This is the proof that the CPU really was at user privilege.
        if sstatus & (1 << 8) == 0 {
            CAME_FROM_USER = true;
        }

        let p = CURPROC;
        let tf = (*p).trapframe;

        // remember where the user program was, so we can resume it
        let sepc: usize;
        asm!("csrr {}, sepc", out(reg) sepc);
        (*tf).epc = sepc as u64;

        if scause == 8 {
            // ECALL from user mode: the program asked the kernel to do
            // something — a system call.
            //
            // IMPLEMENT: the system-call path, three steps.
            //
            //  1. Step OVER the ecall instruction: it is 4 bytes, and sepc
            //     points AT it. If you skip this, sret lands on the ecall
            //     again and the program calls the same syscall forever:
            //         (*tf).epc += 4;
            //
            //  2. The user put the syscall number in a7 and the arguments in
            //     a0..a2 — they are all parked in the trapframe now. Hand
            //     them to the dispatcher (your other half of this exercise,
            //     in syscall.rs):
            //         let ret = crate::syscall::dispatch(
            //             (*tf).a7 as usize,
            //             (*tf).a0 as usize,
            //             (*tf).a1 as usize,
            //             (*tf).a2 as usize,
            //         );
            //
            //  3. The return value travels back in a0. Store it in the
            //     trapframe's a0, and userret will hand it to the program:
            //         (*tf).a0 = ret as u64;
        } else if (scause >> 63) == 1 {
            // An interrupt arrived while the user program ran; handle it
            // like kerneltrap does, then resume the program. (Given.)
            match scause & 0xff {
                1 => {
                    // forwarded timer tick: clear the pending bit
                    let sip: usize;
                    asm!("csrr {}, sip", out(reg) sip);
                    asm!("csrw sip, {}", in(reg) sip & !2);
                    #[cfg(feature = "harness")]
                    {
                        // watchdog: if the program's syscalls are never
                        // answered it runs forever; give up after ~3 seconds
                        U_TICKS += 1;
                        if U_TICKS > 30 {
                            finish(RunOutcome::TimedOut);
                        }
                    }
                }
                9 => crate::console::intr(), // a device (the UART)
                _ => {}
            }
        } else {
            // The program did something illegal (touched memory it does not
            // own, ran a privileged instruction...). In a real OS this kills
            // the process; here we end the run and report it. (Given.)
            finish(RunOutcome::Faulted(scause));
        }

        // back to user mode
        usertrapret(p)
    }
}

/// Return to user mode: stage everything uservec will need for the NEXT
/// trap, aim sret at the user program, and jump through the trampoline.
/// (UNDERSTAND — given.)
pub unsafe fn usertrapret(p: *mut Proc) -> ! {
    let tf = (*p).trapframe;

    // While user code runs, traps must land in uservec — the trampoline
    // copy of it, at its high virtual address.
    let tramp_uservec = TRAMPOLINE + (uservec as *const () as usize - trampoline as *const () as usize);
    asm!("csrw stvec, {}", in(reg) tramp_uservec);

    // leave notes for uservec: the kernel's page table, this process's
    // kernel stack, and where the Rust handler lives
    let kernel_satp: usize;
    asm!("csrr {}, satp", out(reg) kernel_satp);
    (*tf).kernel_satp = kernel_satp as u64;
    (*tf).kernel_sp = ((*p).kstack + PGSIZE) as u64;
    (*tf).kernel_trap = usertrap as *const () as usize as u64;

    // sret consults sstatus: SPP (bit 8) = 0 means "return to USER mode",
    // SPIE (bit 5) = 1 means "run with interrupts on once there"
    let mut sstatus: usize;
    asm!("csrr {}, sstatus", out(reg) sstatus);
    sstatus &= !(1 << 8);
    sstatus |= 1 << 5;
    asm!("csrw sstatus, {}", in(reg) sstatus);

    // sret will jump to sepc: the user program's resume point
    asm!("csrw sepc, {}", in(reg) (*tf).epc as usize);

    // the satp value for the USER's page table; userret installs it
    let user_satp = vm::make_satp((*p).pagetable);

    // jump to userret — through its TRAMPOLINE address, since the page
    // table is about to change under our feet
    let tramp_userret = TRAMPOLINE + (userret as *const () as usize - trampoline as *const () as usize);
    let f: extern "C" fn(usize) -> ! = core::mem::transmute(tramp_userret);
    f(user_satp)
}
