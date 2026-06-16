//! swtch.rs — saving and restoring CPU execution context (the heart of
//! multitasking).
//!
//! A "context switch" is how one thread of execution is paused and another
//! resumed on the same CPU. To pause something we save the CPU registers that
//! define "where it is and what it's doing"; to resume something else we load
//! *its* saved registers. The actual register shuffling can only be done in
//! assembly, because Rust has no way to name `sp`, `ra`, `s0`, etc. directly.

use core::arch::global_asm;

/// The saved registers of a paused execution context.
///
/// We only save the **callee-saved** registers (the RISC-V C ABI guarantees a
/// function preserves these across a call): the return address `ra`, the stack
/// pointer `sp`, and `s0`–`s11`. The caller-saved registers were already spilled
/// to the stack by the compiler around the `swtch` call, so we don't touch them.
///
/// `#[repr(C)]` is essential: it forces the fields to live in memory in exactly
/// this declared order, with no reordering, so the assembly below can reach them
/// by fixed byte offsets (ra = 0, sp = 8, s0 = 16, ...).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Context {
    pub ra: usize,  // 0   return address — where `ret` will jump
    pub sp: usize,  // 8   stack pointer
    pub s0: usize,  // 16
    pub s1: usize,  // 24
    pub s2: usize,  // 32
    pub s3: usize,  // 40
    pub s4: usize,  // 48
    pub s5: usize,  // 56
    pub s6: usize,  // 64
    pub s7: usize,  // 72
    pub s8: usize,  // 80
    pub s9: usize,  // 88
    pub s10: usize, // 96
    pub s11: usize, // 104
}

impl Context {
    /// An all-zero context. `const` so it can initialize a `static`.
    pub const fn zero() -> Context {
        Context {
            ra: 0, sp: 0,
            s0: 0, s1: 0, s2: 0, s3: 0, s4: 0, s5: 0,
            s6: 0, s7: 0, s8: 0, s9: 0, s10: 0, s11: 0,
        }
    }
}

// The assembly routine `swtch` is defined below and declared to Rust here.
extern "C" {
    /// Save the current registers into `*old`, then load registers from `*new`.
    /// The `ret` at the end jumps to `(*new).ra`, so control resumes wherever
    /// `new` was last paused. Returns (to the caller) only when some other
    /// context later switches back into `old`.
    pub fn swtch(old: *mut Context, new: *mut Context);
}

/// Prepare a fresh context so that the *first* time something switches into it,
/// execution begins at `entry` running on the stack whose top is `stack_top`.
/// (UNDERSTAND — given. Note how it just sets the `ra` and `sp` fields.)
pub fn init_context(ctx: *mut Context, entry: usize, stack_top: usize) {
    unsafe {
        *ctx = Context::zero();
        (*ctx).ra = entry; // swtch's final `ret` will jump here
        (*ctx).sp = stack_top; // ...running on this stack
    }
}

// ---- the context switch, in assembly -----------------------------------
//
// Arguments arrive in registers a0 (old) and a1 (new), per the C ABI.
global_asm!(
    r#"
.globl swtch
swtch:
    # IMPLEMENT: the context switch.
    #
    #   1. SAVE the current callee-saved registers into the OLD context, whose
    #      address is in a0. Store each register at its offset:
    #          sd ra,  0(a0)
    #          sd sp,  8(a0)
    #          sd s0,  16(a0)
    #          sd s1,  24(a0)
    #          ... s2..s11 at 32, 40, 48, 56, 64, 72, 80, 88, 96, 104 ...
    #
    #   2. LOAD the new context's registers from NEW, whose address is in a1,
    #      using the SAME offsets but with `ld` instead of `sd`, and a1:
    #          ld ra,  0(a1)
    #          ld sp,  8(a1)
    #          ld s0,  16(a1)
    #          ... and so on through s11 ...
    #
    #   3. ret    # jumps to the freshly-loaded ra (i.e. into `new`)
    #
    # Until you implement it, this just returns without switching, so the
    # test's task never runs.
    ret
"#
);
