//! swtch.rs — saving and restoring CPU execution context. (Ex 05 solution.)

use core::arch::global_asm;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Context {
    pub ra: usize,
    pub sp: usize,
    pub s0: usize,
    pub s1: usize,
    pub s2: usize,
    pub s3: usize,
    pub s4: usize,
    pub s5: usize,
    pub s6: usize,
    pub s7: usize,
    pub s8: usize,
    pub s9: usize,
    pub s10: usize,
    pub s11: usize,
}

impl Context {
    pub const fn zero() -> Context {
        Context {
            ra: 0, sp: 0,
            s0: 0, s1: 0, s2: 0, s3: 0, s4: 0, s5: 0,
            s6: 0, s7: 0, s8: 0, s9: 0, s10: 0, s11: 0,
        }
    }
}

extern "C" {
    pub fn swtch(old: *mut Context, new: *mut Context);
}

pub fn init_context(ctx: *mut Context, entry: usize, stack_top: usize) {
    unsafe {
        *ctx = Context::zero();
        (*ctx).ra = entry;
        (*ctx).sp = stack_top;
    }
}

global_asm!(
    r#"
.globl swtch
swtch:
    sd ra,  0(a0)
    sd sp,  8(a0)
    sd s0,  16(a0)
    sd s1,  24(a0)
    sd s2,  32(a0)
    sd s3,  40(a0)
    sd s4,  48(a0)
    sd s5,  56(a0)
    sd s6,  64(a0)
    sd s7,  72(a0)
    sd s8,  80(a0)
    sd s9,  88(a0)
    sd s10, 96(a0)
    sd s11, 104(a0)

    ld ra,  0(a1)
    ld sp,  8(a1)
    ld s0,  16(a1)
    ld s1,  24(a1)
    ld s2,  32(a1)
    ld s3,  40(a1)
    ld s4,  48(a1)
    ld s5,  56(a1)
    ld s6,  64(a1)
    ld s7,  72(a1)
    ld s8,  80(a1)
    ld s9,  88(a1)
    ld s10, 96(a1)
    ld s11, 104(a1)

    ret
"#
);
