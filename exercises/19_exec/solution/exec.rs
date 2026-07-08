//! exec.rs — load a named program into a fresh address space and give it
//! command-line arguments. This is `exec`, the second half of how Unix
//! starts programs (the first half, `fork`, comes in exercise 20).
//!
//! Exercise 18 could run exactly ONE program, hard-coded onto a single
//! page. Here we build a real loader:
//!
//!   - a TABLE of programs (`hello`, `args`, `echo`, `big`), each an
//!     embedded flat binary, looked up by name;
//!   - `load_segment` (in vm.rs) copies a program of ANY size into the
//!     user's address space, page by page;
//!   - `push_argv` lays the command-line arguments out on the user's stack
//!     the way the C calling convention expects, so a program can read its
//!     own `argc`/`argv`;
//!   - `exec` ties it together into a ready-to-run process.

use crate::memlayout::{MAX_PROG_PAGES, PGSIZE, USER_CODE, USER_STACK, USER_STACK_TOP};
use crate::proc::{self, Proc};
use crate::vm::{self, Pte};
use core::ptr::addr_of;

// ========================================================================
//  The programs.
//
//  With no compiler for user code yet (that is exercise 21's user crate),
//  each program is a hand-written, position-independent flat binary baked
//  into the kernel image as data. `exec` copies the bytes between the
//  `_start` and `_end` labels to the user's address space at virtual
//  address 0 and jumps there.
//
//  On entry every program gets, by our calling convention:
//      a0 = argc          (how many arguments, including the program name)
//      a1 = argv          (a user pointer to an array of `argc` pointers,
//                          each pointing at a NUL-terminated argument string)
//  and the usual syscall convention (a7 = number, a0..a2 = args) to talk to
//  the kernel. (UNDERSTAND — given.)
// ========================================================================

extern "C" {
    static prog_hello_start: u8;
    static prog_hello_end: u8;
    static prog_args_start: u8;
    static prog_args_end: u8;
    static prog_echo_start: u8;
    static prog_echo_end: u8;
    static prog_big_start: u8;
    static prog_big_end: u8;
}

core::arch::global_asm!(
    r#"
.section .rodata

# ---- hello: print a fixed message, then exit(getpid() + 41) ----
.balign 16
.globl prog_hello_start
.globl prog_hello_end
prog_hello_start:
.option push
.option norelax
    la   a1, hello_msg
    li   a2, 21              # length of hello_msg
    li   a0, 1               # fd 1 = console
    li   a7, 16              # SYS_WRITE
    ecall
    li   a7, 11              # SYS_GETPID
    ecall
    addi a0, a0, 41
    li   a7, 2               # SYS_EXIT
    ecall
hello_msg:
    .ascii "hello from user mode\n"
.option pop
prog_hello_end:

# ---- args: exit(argc). a0 already holds argc on entry. ----
.balign 16
.globl prog_args_start
.globl prog_args_end
prog_args_start:
.option push
.option norelax
    li   a7, 2               # SYS_EXIT, status already in a0 = argc
    ecall
.option pop
prog_args_end:

# ---- echo: write argv[1..], space-separated, then a newline. ----
.balign 16
.globl prog_echo_start
.globl prog_echo_end
prog_echo_start:
.option push
.option norelax
    mv   s0, a0              # s0 = argc
    mv   s1, a1              # s1 = argv
    li   s2, 1               # s2 = i, starting at 1 (skip the program name)
echo_loop:
    bge  s2, s0, echo_nl     # i >= argc: nothing left, print the newline
    slli t0, s2, 3           # t0 = i * 8  (each argv entry is an 8-byte pointer)
    add  t0, s1, t0
    ld   a1, 0(t0)           # a1 = argv[i]  (pointer to the string)
    mv   t1, a1              # measure its length
echo_slen:
    lb   t2, 0(t1)
    beqz t2, echo_slen_done
    addi t1, t1, 1
    j    echo_slen
echo_slen_done:
    sub  a2, t1, a1          # a2 = length
    li   a0, 1
    li   a7, 16              # write(1, argv[i], len)
    ecall
    addi s2, s2, 1
    bge  s2, s0, echo_nl     # that was the last one: no trailing space
    la   a1, echo_space
    li   a2, 1
    li   a0, 1
    li   a7, 16              # write a single space between arguments
    ecall
    j    echo_loop
echo_nl:
    la   a1, echo_newline
    li   a2, 1
    li   a0, 1
    li   a7, 16
    ecall
    li   a0, 0
    li   a7, 2               # exit(0)
    ecall
echo_space:
    .ascii " "
echo_newline:
    .ascii "\n"
.option pop
prog_echo_end:

# ---- big: exit(99), but padded past one page so the loader must map
#      more than a single page for it. ----
.balign 16
.globl prog_big_start
.globl prog_big_end
prog_big_start:
.option push
.option norelax
    li   a0, 99
    li   a7, 2               # exit(99)
    ecall
    .skip 4096               # padding: forces the image over one page
.option pop
prog_big_end:
"#
);

/// One entry in the program table.
pub struct Program {
    pub name: &'static str,
    pub image: &'static [u8],
}

unsafe fn image(start: *const u8, end: *const u8) -> &'static [u8] {
    core::slice::from_raw_parts(start, end as usize - start as usize)
}

/// The table of programs `run` and the harness can launch. (Given.)
pub fn programs() -> [Program; 4] {
    unsafe {
        [
            Program { name: "hello", image: image(addr_of!(prog_hello_start), addr_of!(prog_hello_end)) },
            Program { name: "args", image: image(addr_of!(prog_args_start), addr_of!(prog_args_end)) },
            Program { name: "echo", image: image(addr_of!(prog_echo_start), addr_of!(prog_echo_end)) },
            Program { name: "big", image: image(addr_of!(prog_big_start), addr_of!(prog_big_end)) },
        ]
    }
}

/// Find a program by name. (Given.)
pub fn lookup(name: &str) -> Option<Program> {
    programs().into_iter().find(|p| p.name == name)
}

// ========================================================================
//  exec: build a runnable process from a program name + arguments.
// ========================================================================

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ExecError {
    /// No program by that name is in the table.
    NotFound,
    /// Out of memory while building the address space.
    NoMem,
    /// Too many arguments, or one is too long.
    BadArgs,
}

const MAXARG: usize = 16; // most arguments a program may receive
const MAXARGLEN: usize = 64; // longest a single argument may be (incl. NUL)

/// Build a runnable process for program `name` with the given `args`.
/// Allocates the process, and on ANY failure frees it again so a failed
/// exec never leaks a half-built process. (Given — the cleanup wrapper; the
/// interesting work is in `build_process`, which you write.)
pub unsafe fn exec(name: &str, args: &[&str]) -> Result<*mut Proc, ExecError> {
    let p = proc::allocproc();
    if p.is_null() {
        return Err(ExecError::NoMem);
    }
    match build_process(p, name, args) {
        Ok(()) => Ok(p),
        Err(e) => {
            proc::freeproc(p); // undo the half-built process
            Err(e)
        }
    }
}

/// Turn the freshly allocated process `p` into a ready-to-run program: give
/// it an address space holding the program image, a stack, and its
/// arguments, then point the CPU at the program's first instruction.
unsafe fn build_process(p: *mut Proc, name: &str, args: &[&str]) -> Result<(), ExecError> {
    // IMPLEMENT: the exec recipe, six steps. Every fallible step returns a
    // Result, so use `?` (exercise 10) to bail out early — `exec` above will
    // free the process for you if you do.
    //
    //  1. Find the program in the table (returns None if there is no such
    //     name — turn that into an error):
    //         let prog = lookup(name).ok_or(ExecError::NotFound)?;
    //
    //  2. Map the kernel's two pages (trampoline + trapframe) into the new
    //     page table, exactly as in exercise 18. `proc_pagetable` returns
    //     Result<(), ()>, so convert its error:
    //         proc::proc_pagetable(p).map_err(|_| ExecError::NoMem)?;
    //
    //  3. Load the program image into the address space (this is YOUR
    //     load_segment, in vm.rs):
    //         vm::load_segment((*p).pagetable, prog.image).map_err(|_| ExecError::NoMem)?;
    //
    //  4. Give the program a stack:
    //         vm::map_user_stack((*p).pagetable).map_err(|_| ExecError::NoMem)?;
    //
    //  5. Push the arguments onto that stack (given, below). It hands back
    //     (argc, argv, sp): the argument count, the user pointer to the argv
    //     array, and the resulting stack pointer:
    //         let (argc, argv, sp) = push_argv((*p).pagetable, name, args)?;
    //
    //  6. Point the CPU at the program: instruction pointer = program start
    //     (USER_CODE, i.e. 0), stack pointer = sp, and hand it its arguments
    //     in a0/a1 the way the calling convention says:
    //         let tf = (*p).trapframe;
    //         (*tf).epc = USER_CODE as u64;
    //         (*tf).sp  = sp as u64;
    //         (*tf).a0  = argc as u64;   // a0 = argc
    //         (*tf).a1  = argv as u64;   // a1 = argv
    //     Then `Ok(())`.
    let prog = lookup(name).ok_or(ExecError::NotFound)?;
    proc::proc_pagetable(p).map_err(|_| ExecError::NoMem)?;
    vm::load_segment((*p).pagetable, prog.image).map_err(|_| ExecError::NoMem)?;
    vm::map_user_stack((*p).pagetable).map_err(|_| ExecError::NoMem)?;
    let (argc, argv, sp) = push_argv((*p).pagetable, name, args)?;
    let tf = (*p).trapframe;
    (*tf).epc = USER_CODE as u64;
    (*tf).sp = sp as u64;
    (*tf).a0 = argc as u64;
    (*tf).a1 = argv as u64;
    Ok(())
}

/// Lay the command-line arguments out on the user's stack and return
/// `(argc, argv, sp)`. This is the fiddly heart of `exec`, so it is given —
/// but it is worth reading closely, because it shows what `argv` really *is*.
///
/// The layout we build, from the top of the stack downward:
///
///     high addresses
///        "echo\0"  "hello\0"  "world\0"     <- the argument strings
///        argv[0] argv[1] argv[2] NULL       <- an array of pointers to them
///     low addresses  (sp points here; this is also `argv`)
///
/// Every pointer we store in the argv array is a USER virtual address (where
/// the string will live in the program's world), and we write it all into
/// the user's address space with `copyout` — the function that sat unused in
/// exercise 18 finally earns its keep. (UNDERSTAND — given.)
unsafe fn push_argv(
    pt: *mut Pte,
    name: &str,
    args: &[&str],
) -> Result<(usize, usize, usize), ExecError> {
    let argc = 1 + args.len(); // argv[0] is the program name
    if argc > MAXARG {
        return Err(ExecError::BadArgs);
    }

    let mut sp = USER_STACK_TOP; // stacks grow downward from the top
    let mut uargv = [0usize; MAXARG + 1]; // the user addresses of the strings

    // Push each argument string (argv[0] = the program name, then the rest).
    for i in 0..argc {
        let s: &[u8] = if i == 0 {
            name.as_bytes()
        } else {
            args[i - 1].as_bytes()
        };
        if s.len() + 1 > MAXARGLEN {
            return Err(ExecError::BadArgs);
        }
        sp -= s.len() + 1; // room for the string plus its NUL terminator
        sp &= !7; // keep the stack 8-byte aligned
        if sp < USER_STACK {
            return Err(ExecError::BadArgs); // ran off the bottom of the stack page
        }
        let mut buf = [0u8; MAXARGLEN];
        buf[..s.len()].copy_from_slice(s);
        // buf[s.len()] is already 0: that is the NUL terminator.
        if vm::copyout(pt, sp, &buf[..s.len() + 1]).is_err() {
            return Err(ExecError::NoMem);
        }
        uargv[i] = sp; // remember where this string ended up
    }
    uargv[argc] = 0; // the argv array is null-terminated

    // Push the argv array itself: argc + 1 eight-byte pointers.
    let count = argc + 1;
    sp -= count * 8;
    sp &= !15; // 16-byte align the array (and the final stack pointer)
    if sp < USER_STACK {
        return Err(ExecError::BadArgs);
    }
    let mut pbuf = [0u8; (MAXARG + 1) * 8];
    for i in 0..count {
        pbuf[i * 8..i * 8 + 8].copy_from_slice(&(uargv[i] as u64).to_le_bytes());
    }
    if vm::copyout(pt, sp, &pbuf[..count * 8]).is_err() {
        return Err(ExecError::NoMem);
    }

    // a1 (argv) points at the array; the stack pointer sits at the same spot.
    Ok((argc, sp, sp))
}

/// A generous upper bound on how big a program image may be. (Given — used
/// by the harness; the loader itself just keeps allocating pages until the
/// image is fully copied.)
pub const fn max_image_bytes() -> usize {
    MAX_PROG_PAGES * PGSIZE
}
