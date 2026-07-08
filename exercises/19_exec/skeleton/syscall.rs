//! syscall.rs — the system-call table: numbers, dispatch, and handlers.
//!
//! A system call is just a function call that crosses the privilege wall.
//! The user program picks a NUMBER (a7) and up to three ARGUMENTS (a0..a2),
//! executes `ecall`, and gets a RETURN VALUE back in a0. This file is the
//! kernel's side of that contract: given the number and arguments that
//! usertrap fished out of the trapframe, run the right handler.
//!
//! The numbers match xv6's, so this table will keep growing in the next
//! exercises (fork, wait, open, read...) without renumbering anything.

use crate::usermode::{self, RunOutcome};
use crate::vm;

pub const SYS_EXIT: usize = 2; // exit(status): end the program
pub const SYS_GETPID: usize = 11; // getpid() -> pid
pub const SYS_WRITE: usize = 16; // write(fd, buf, len) -> len

/// Route a system call to its handler and hand back the return value.
/// Unknown numbers return -1 (the classic Unix "that failed" value) rather
/// than crashing: a user program must not be able to break the kernel by
/// passing garbage.
pub fn dispatch(num: usize, a0: usize, a1: usize, a2: usize) -> isize {
    // IMPLEMENT: match on `num` and call the right handler below:
    //
    //     SYS_EXIT   -> sys_exit(a0 as isize)
    //     SYS_GETPID -> sys_getpid()
    //     SYS_WRITE  -> sys_write(a0, a1, a2)
    //     anything else -> -1
    //
    // (Each handler already returns an isize, so each match arm can just be
    // the call itself. sys_exit never returns — Rust's `!` type — and that
    // is fine as a match arm too.)
    match num {
        SYS_EXIT => sys_exit(a0 as isize),
        SYS_GETPID => sys_getpid(),
        SYS_WRITE => sys_write(a0, a1, a2),
        _ => -1,
    }
}

/// exit(status): the program is done. Hand its status to usermode::finish,
/// which swtch-es back to the kernel code that launched the program.
/// This one never returns — there is no program left to return to.
/// (UNDERSTAND — given.)
fn sys_exit(status: isize) -> ! {
    unsafe { usermode::finish(RunOutcome::Exited(status)) }
}

/// getpid(): which process am I? (UNDERSTAND — given.)
fn sys_getpid() -> isize {
    unsafe { (*usermode::curproc()).pid as isize }
}

/// write(fd, buf, len): print `len` bytes from the USER's buffer at `buf`.
///
/// The interesting part: `buf` is a virtual address in the USER's address
/// space. The kernel cannot just read it — it must translate it through the
/// user's page table, page by page. That is exactly what your `copyin`
/// (vm.rs) does. A bad pointer makes copyin return Err, and the program
/// gets -1 instead of taking the kernel down. (UNDERSTAND — given.)
fn sys_write(fd: usize, buf: usize, len: usize) -> isize {
    if fd != 1 {
        return -1; // only "file descriptor" 1, the console, exists so far
    }
    let mut kbuf = [0u8; 64]; // a kernel-side bounce buffer
    let mut copied = 0;
    unsafe {
        let table = (*usermode::curproc()).pagetable;
        while copied < len {
            let n = core::cmp::min(kbuf.len(), len - copied);
            if vm::copyin(table, &mut kbuf[..n], buf + copied).is_err() {
                return -1; // the user gave us a bad pointer
            }
            emit(&kbuf[..n]);
            copied += n;
        }
    }
    len as isize
}

/// Where write's bytes go: the UART normally; a capture buffer under the
/// test harness, so the self-check can verify exactly what the user program
/// said. (Given.)
fn emit(bytes: &[u8]) {
    #[cfg(feature = "harness")]
    capture::put(bytes);
    #[cfg(not(feature = "harness"))]
    for &b in bytes {
        crate::uart::putc(b);
    }
}

#[cfg(feature = "harness")]
pub use capture::captured;

#[cfg(feature = "harness")]
mod capture {
    use core::ptr::{addr_of, addr_of_mut};

    static mut BUF: [u8; 256] = [0; 256];
    static mut LEN: usize = 0;

    pub(super) fn put(bytes: &[u8]) {
        unsafe {
            for &b in bytes {
                let len = *addr_of!(LEN);
                if len < 256 {
                    *addr_of_mut!(BUF[len]) = b;
                    *addr_of_mut!(LEN) = len + 1;
                }
            }
        }
    }

    /// Everything user programs have written so far, as text.
    pub fn captured() -> &'static str {
        unsafe {
            let len = *addr_of!(LEN);
            let buf = core::slice::from_raw_parts(addr_of!(BUF) as *const u8, len);
            core::str::from_utf8(buf).unwrap_or("")
        }
    }
}
