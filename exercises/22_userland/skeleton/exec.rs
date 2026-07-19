//! exec.rs — load a named program into a fresh address space and give it
//! command-line arguments. This is `exec`, the second half of how Unix
//! starts programs (the first half, `fork`, came in exercise 21).
//!
//! New in exercise 22: `exec` becomes a **system call**. Until now only the
//! kernel called `exec` (from the `run` command). Now a *user* program can ask
//! the kernel "replace my memory with this other program" — which is exactly
//! what a shell does after `fork`. The heart of that is `exec_into` (you write
//! it): it swaps a running process's whole address space for a new one.
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

use crate::kalloc;
use crate::memlayout::{
    MAX_PROG_PAGES, PGSIZE, TRAMPOLINE, TRAPFRAME, USER_CODE, USER_STACK, USER_STACK_TOP,
};
use crate::proc::{self, Proc};
use crate::vm::{self, Pte, PTE_R, PTE_W, PTE_X};
use core::ptr::addr_of;

// ========================================================================
//  The programs.
//
//  With no compiler for user code yet, each program is a hand-written,
//  position-independent flat binary baked into the kernel image as data.
//  `exec` copies the bytes between the `_start` and `_end` labels to the
//  user's address space at virtual address 0 and jumps there.
//
//  New here: `sh`, a tiny user-mode SHELL, and three little exec test
//  programs. `sh` is the payoff of the whole course — a shell that runs
//  in user mode with no special privileges, reading commands and launching
//  other programs with fork + exec + wait, talking to the kernel only
//  through system calls.
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
    static prog_cat_start: u8;
    static prog_cat_end: u8;
    static prog_create_start: u8;
    static prog_create_end: u8;
    static prog_forktest_start: u8;
    static prog_forktest_end: u8;
    static prog_forks2_start: u8;
    static prog_forks2_end: u8;
    static prog_sh_start: u8;
    static prog_sh_end: u8;
    static prog_execself_start: u8;
    static prog_execself_end: u8;
    static prog_exectest_start: u8;
    static prog_exectest_end: u8;
    static prog_execfail_start: u8;
    static prog_execfail_end: u8;
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

# ---- cat FILE: open the file, then loop read -> write(stdout) to EOF.
#      New in exercise 20: it uses open/read/close and a stack buffer. ----
.balign 16
.globl prog_cat_start
.globl prog_cat_end
prog_cat_start:
.option push
.option norelax
    li   t0, 2
    blt  a0, t0, cat_exit     # argc < 2: no filename, just exit
    addi sp, sp, -64          # carve a 64-byte read buffer on the (writable) stack
    mv   s1, sp               # s1 = buffer  (registers survive ecalls)
    ld   a0, 8(a1)            # a0 = argv[1] = the filename pointer
    li   a1, 0                # O_RDONLY
    li   a7, 15               # SYS_open
    ecall
    mv   s0, a0               # s0 = fd
    bltz s0, cat_exit         # open failed
cat_loop:
    mv   a0, s0               # fd
    mv   a1, s1               # buf
    li   a2, 64               # count
    li   a7, 5                # SYS_read
    ecall                     # a0 = bytes read (0 at end of file)
    blez a0, cat_close        # 0 or negative: stop
    mv   a2, a0               # count = bytes read
    li   a0, 1                # fd 1 = stdout
    mv   a1, s1               # buf
    li   a7, 16               # SYS_write
    ecall
    j    cat_loop
cat_close:
    mv   a0, s0
    li   a7, 21               # SYS_close
    ecall
cat_exit:
    li   a0, 0
    li   a7, 2                # SYS_exit(0)
    ecall
.option pop
prog_cat_end:

# ---- create FILE: create the file and write a line into it. ----
.balign 16
.globl prog_create_start
.globl prog_create_end
prog_create_start:
.option push
.option norelax
    li   t0, 2
    blt  a0, t0, create_exit  # argc < 2: no filename
    ld   a0, 8(a1)            # a0 = argv[1] = the filename
    li   a1, 0x201            # O_CREATE | O_WRONLY  (0x200 | 0x001)
    li   a7, 15               # SYS_open
    ecall
    mv   s0, a0               # s0 = fd
    bltz s0, create_exit
    mv   a0, s0               # fd
    la   a1, create_msg       # the message (read-only image data; write reads it)
    li   a2, 12               # length of "saved by fd\n"
    li   a7, 16               # SYS_write
    ecall
    mv   a0, s0
    li   a7, 21               # SYS_close
    ecall
create_exit:
    li   a0, 0
    li   a7, 2                # SYS_exit(0)
    ecall
create_msg:
    .ascii "saved by fd\n"
.option pop
prog_create_end:

# ---- forktest: fork one child. The child writes "child\n" and exits(7);
#      the parent writes "parent\n", waits for the child, and exits with the
#      child's status + 10 (= 17), proving wait delivered the status. ----
.balign 16
.globl prog_forktest_start
.globl prog_forktest_end
prog_forktest_start:
.option push
.option norelax
    li   a7, 1                # SYS_FORK
    ecall                     # a0 = child pid (parent) or 0 (child)
    bnez a0, ft_parent
ft_child:
    la   a1, ft_child_msg
    li   a2, 6                # "child\n"
    li   a0, 1
    li   a7, 16               # write(1, "child\n", 6)
    ecall
    li   a0, 7
    li   a7, 2                # exit(7)
    ecall
ft_parent:
    la   a1, ft_parent_msg
    li   a2, 7                # "parent\n"
    li   a0, 1
    li   a7, 16               # write(1, "parent\n", 7)
    ecall
    addi sp, sp, -16          # make room for the status int on the stack
    mv   a0, sp               # a0 = &status
    li   a7, 3                # SYS_WAIT
    ecall                     # blocks until the child exits; *sp = its status
    lw   a1, 0(sp)            # a1 = child's exit status (7)
    addi a1, a1, 10           # 17
    mv   a0, a1
    li   a7, 2                # exit(17)
    ecall
ft_child_msg:
    .ascii "child\n"
ft_parent_msg:
    .ascii "parent\n"
.option pop
prog_forktest_end:

# ---- forks2: fork TWO children (A exits 3, B exits 4); the parent waits for
#      both and exits with the sum of their statuses (= 7). Exercises reaping
#      more than one child and the scheduler juggling three processes. ----
.balign 16
.globl prog_forks2_start
.globl prog_forks2_end
prog_forks2_start:
.option push
.option norelax
    li   a7, 1                # fork child A
    ecall
    bnez a0, fs_parent1
    li   a0, 3
    li   a7, 2                # child A: exit(3)
    ecall
fs_parent1:
    li   a7, 1                # fork child B
    ecall
    bnez a0, fs_parent2
    li   a0, 4
    li   a7, 2                # child B: exit(4)
    ecall
fs_parent2:
    li   s0, 0                # s0 = running sum (saved reg survives ecalls)
    addi sp, sp, -16
    mv   a0, sp               # wait #1
    li   a7, 3
    ecall
    lw   a1, 0(sp)
    add  s0, s0, a1
    mv   a0, sp               # wait #2
    li   a7, 3
    ecall
    lw   a1, 0(sp)
    add  s0, s0, a1
    mv   a0, s0
    li   a7, 2                # exit(3 + 4 = 7)
    ecall
.option pop
prog_forks2_end:

# ---- sh: THE USERLAND SHELL. A user program (no kernel privileges) that
#      loops forever: print a prompt, read a line, split it into words, then
#      fork a child, have the child `exec` the command, and wait for it.
#      Built-in `exit` leaves the shell. This is the payoff of the course:
#      a shell running in user mode, using only system calls.
#
#      Registers held across the loop (the trampoline saves/restores them on
#      every trap, so they survive `ecall`s):
#        s0 = line buffer (256 bytes)      s1 = argv array (pointers)
#        s2 = read cursor   s3 = buffer end   s4 = argc
.balign 16
.globl prog_sh_start
.globl prog_sh_end
prog_sh_start:
.option push
.option norelax
    addi sp, sp, -512         # one frame: [0..256) line buffer, [256..) argv[]
    mv   s0, sp               # s0 = line buffer
    addi s1, sp, 256          # s1 = argv array
sh_loop:
    la   a1, sh_prompt        # write(1, "$ ", 2)
    li   a2, 2
    li   a0, 1
    li   a7, 16
    ecall
    # --- read one line, echoing each key, until Enter ---
    mv   s2, s0               # write cursor = start of buffer
    addi s3, s0, 255          # leave room for the NUL terminator
sh_readc:
    li   a0, 0                # fd 0 = console
    mv   a1, s2
    li   a2, 1
    li   a7, 5                # read(0, cursor, 1)
    ecall
    blez a0, sh_line_end      # end of input: finish the line
    li   a0, 1                # echo the character back
    mv   a1, s2
    li   a2, 1
    li   a7, 16
    ecall
    lb   t0, 0(s2)
    li   t1, 10               # '\n'
    beq  t0, t1, sh_line_end
    li   t1, 13               # '\r'
    beq  t0, t1, sh_line_end
    addi s2, s2, 1
    blt  s2, s3, sh_readc     # more room: keep reading
sh_line_end:
    sb   zero, 0(s2)          # NUL-terminate the line
    # --- split the line into words (argv), NUL-terminating each in place ---
    mv   t0, s0               # scan cursor
    li   s4, 0                # argc = 0
sh_skip:
    lb   t1, 0(t0)
    beqz t1, sh_parsed        # end of line
    li   t2, 32               # ' '
    bne  t1, t2, sh_tok
    addi t0, t0, 1            # skip a space
    j    sh_skip
sh_tok:
    slli t3, s4, 3            # argv[argc] = &word
    add  t3, s1, t3
    sd   t0, 0(t3)
    addi s4, s4, 1
sh_scan:
    lb   t1, 0(t0)
    beqz t1, sh_parsed
    li   t2, 32
    beq  t1, t2, sh_endtok
    addi t0, t0, 1
    j    sh_scan
sh_endtok:
    sb   zero, 0(t0)          # terminate this word
    addi t0, t0, 1
    li   t4, 18               # stop before we fill the argv array
    blt  s4, t4, sh_skip
sh_parsed:
    slli t3, s4, 3            # argv[argc] = NULL
    add  t3, s1, t3
    sd   zero, 0(t3)
    beqz s4, sh_loop          # blank line: prompt again
    # --- built-in: "exit" leaves the shell ---
    ld   t0, 0(s1)            # t0 = argv[0]
    la   t1, sh_exit          # t1 = "exit"
sh_cmp:
    lb   t2, 0(t0)
    lb   t3, 0(t1)
    bne  t2, t3, sh_run       # differ: not the exit builtin
    beqz t2, sh_doexit        # both reached NUL: it IS "exit"
    addi t0, t0, 1
    addi t1, t1, 1
    j    sh_cmp
sh_doexit:
    li   a0, 0
    li   a7, 2                # exit(0): return to whoever ran the shell
    ecall
sh_run:
    # --- fork; child execs the command, parent waits ---
    li   a7, 1                # fork()
    ecall
    bnez a0, sh_wait          # parent branch
    ld   a0, 0(s1)            # child: exec(argv[0], argv)
    mv   a1, s1
    li   a7, 7                # SYS_EXEC
    ecall
    la   a1, sh_nf            # exec returned -> the command was not found
    li   a2, 16               # length of "exec: not found\n"
    li   a0, 1
    li   a7, 16
    ecall
    li   a0, 1
    li   a7, 2                # exit(1)
    ecall
sh_wait:
    li   a0, 0               # wait(0): reap the child, ignore its status
    li   a7, 3
    ecall
    j    sh_loop
sh_prompt:
    .ascii "$ "
sh_exit:
    .asciz "exit"
sh_nf:
    .ascii "exec: not found\n"
.option pop
prog_sh_end:

# ---- execself: replace THIS process with `args` (given two arguments, so it
#      exits with argc = 2). Proves exec replaces the running image and does
#      NOT return on success — if it did, we would reach the `exit(88)` below. ----
.balign 16
.globl prog_execself_start
.globl prog_execself_end
prog_execself_start:
.option push
.option norelax
    la   t0, es_name          # "args"
    la   t1, es_argx          # "x"
    addi sp, sp, -32          # build argv = ["args", "x", NULL] on the stack
    sd   t0, 0(sp)            #   (the pointers are USER addresses, from `la`)
    sd   t1, 8(sp)
    sd   zero, 16(sp)
    mv   a0, t0               # a0 = path = "args"
    mv   a1, sp               # a1 = argv
    li   a7, 7                # SYS_EXEC
    ecall
    li   a0, 88               # only reached if exec FAILED
    li   a7, 2
    ecall
es_name:
    .asciz "args"
es_argx:
    .asciz "x"
.option pop
prog_execself_end:

# ---- exectest: the universal Unix pattern. fork(); the child execs `echo hi`
#      (printing "hi"); the parent waits, then exits 42. Proves exec runs in a
#      child while the PARENT is untouched by it. ----
.balign 16
.globl prog_exectest_start
.globl prog_exectest_end
prog_exectest_start:
.option push
.option norelax
    li   a7, 1                # fork()
    ecall
    bnez a0, et_parent
    la   t0, et_echo          # child: exec("echo", ["echo", "hi"])
    la   t1, et_hi
    addi sp, sp, -32
    sd   t0, 0(sp)
    sd   t1, 8(sp)
    sd   zero, 16(sp)
    mv   a0, t0
    mv   a1, sp
    li   a7, 7                # SYS_EXEC
    ecall
    li   a0, 1                # exec failed
    li   a7, 2
    ecall
et_parent:
    addi sp, sp, -16
    mv   a0, sp               # wait(&status)
    li   a7, 3
    ecall
    li   a0, 42               # exit(42): the parent survives the child's exec
    li   a7, 2
    ecall
et_echo:
    .asciz "echo"
et_hi:
    .asciz "hi"
.option pop
prog_exectest_end:

# ---- execfail: exec a program that does not exist. exec must return -1 and
#      leave THIS program running, so we go on to exit(7). ----
.balign 16
.globl prog_execfail_start
.globl prog_execfail_end
prog_execfail_start:
.option push
.option norelax
    la   t0, ef_name          # "nosuchprog"
    addi sp, sp, -16
    sd   t0, 0(sp)            # argv = ["nosuchprog", NULL]
    sd   zero, 8(sp)
    mv   a0, t0
    mv   a1, sp
    li   a7, 7                # SYS_EXEC (will fail: no such program)
    ecall
    li   a0, 7                # exec returned -1: carry on and exit(7)
    li   a7, 2
    ecall
ef_name:
    .asciz "nosuchprog"
.option pop
prog_execfail_end:
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
pub fn programs() -> [Program; 12] {
    unsafe {
        [
            Program { name: "hello", image: image(addr_of!(prog_hello_start), addr_of!(prog_hello_end)) },
            Program { name: "args", image: image(addr_of!(prog_args_start), addr_of!(prog_args_end)) },
            Program { name: "echo", image: image(addr_of!(prog_echo_start), addr_of!(prog_echo_end)) },
            Program { name: "big", image: image(addr_of!(prog_big_start), addr_of!(prog_big_end)) },
            Program { name: "cat", image: image(addr_of!(prog_cat_start), addr_of!(prog_cat_end)) },
            Program { name: "create", image: image(addr_of!(prog_create_start), addr_of!(prog_create_end)) },
            Program { name: "forktest", image: image(addr_of!(prog_forktest_start), addr_of!(prog_forktest_end)) },
            Program { name: "forks2", image: image(addr_of!(prog_forks2_start), addr_of!(prog_forks2_end)) },
            Program { name: "sh", image: image(addr_of!(prog_sh_start), addr_of!(prog_sh_end)) },
            Program { name: "execself", image: image(addr_of!(prog_execself_start), addr_of!(prog_execself_end)) },
            Program { name: "exectest", image: image(addr_of!(prog_exectest_start), addr_of!(prog_exectest_end)) },
            Program { name: "execfail", image: image(addr_of!(prog_execfail_start), addr_of!(prog_execfail_end)) },
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

const MAXARG: usize = 8; // most arguments a program may receive
const MAXARGLEN: usize = 32; // longest a single argument may be (incl. NUL)

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

/// A freshly built user address space, ready to be handed to a process: the
/// new page table, plus where the program should begin (argc / argv / sp).
struct Built {
    pagetable: *mut Pte,
    argc: usize,
    argv: usize, // user address of the argv array
    sp: usize,   // user stack pointer to start with
}

/// Build a brand-new address space for program `name` with `args`: a fresh
/// page table mapping the trampoline + this process's trapframe, the loaded
/// program image, a stack, and the arguments laid out on that stack. On ANY
/// failure the half-built page table is freed. Both `build_process` and your
/// `exec_into` call this. (UNDERSTAND — given; it is exercise 19's recipe,
/// gathered into one reusable helper.)
unsafe fn build_addrspace(trapframe: usize, name: &str, args: &[&str]) -> Result<Built, ExecError> {
    let prog = lookup(name).ok_or(ExecError::NotFound)?;

    // a fresh, empty root page table.
    let pt = kalloc::kalloc() as *mut Pte;
    if pt.is_null() {
        return Err(ExecError::NoMem);
    }
    core::ptr::write_bytes(pt as *mut u8, 0, PGSIZE);

    // fill it in; on any error, free the page table and bail.
    match fill_addrspace(pt, trapframe, prog.image, name, args) {
        Ok((argc, argv, sp)) => Ok(Built { pagetable: pt, argc, argv, sp }),
        Err(e) => {
            vm::free_user_pagetable(pt);
            Err(e)
        }
    }
}

/// The fallible middle of `build_addrspace`: map the kernel pages, load the
/// image, map a stack, and push argv. Split out so `build_addrspace` can free
/// the page table if any step fails. (Given.)
unsafe fn fill_addrspace(
    pt: *mut Pte,
    trapframe: usize,
    image: &[u8],
    name: &str,
    args: &[&str],
) -> Result<(usize, usize, usize), ExecError> {
    // the trampoline (shared) and this process's trapframe — the two kernel
    // pages every user page table must contain (as in exercise 18).
    vm::mappages(pt, TRAMPOLINE, PGSIZE, vm::trampoline_page(), PTE_R | PTE_X)
        .map_err(|_| ExecError::NoMem)?;
    vm::mappages(pt, TRAPFRAME, PGSIZE, trapframe, PTE_R | PTE_W).map_err(|_| ExecError::NoMem)?;
    // the program image, then a stack, then the command-line arguments.
    vm::load_segment(pt, image).map_err(|_| ExecError::NoMem)?;
    vm::map_user_stack(pt).map_err(|_| ExecError::NoMem)?;
    push_argv(pt, name, args)
}

/// Turn the freshly allocated process `p` into a ready-to-run program: build
/// it an address space (image + stack + arguments), adopt it, and point the
/// CPU at the program's first instruction. Used by `exec` (for `run` and the
/// harness) and by `fork`'s initial program. (UNDERSTAND — given; this was
/// your exercise 19 `build_process`, now built on `build_addrspace`.)
unsafe fn build_process(p: *mut Proc, name: &str, args: &[&str]) -> Result<(), ExecError> {
    let built = build_addrspace((*p).trapframe as usize, name, args)?;

    // `allocproc` gave us an empty page table; drop it and adopt the new one.
    vm::free_user_pagetable((*p).pagetable);
    (*p).pagetable = built.pagetable;

    // point the CPU at the program: start address, stack, and a0=argc / a1=argv.
    let tf = (*p).trapframe;
    (*tf).epc = USER_CODE as u64;
    (*tf).sp = built.sp as u64;
    (*tf).a0 = built.argc as u64;
    (*tf).a1 = built.argv as u64;

    // schedulable: the scheduler's first swtch into it lands at forkret, which
    // dives into user mode. (From exercise 21.)
    crate::usermode::ready(p);
    Ok(())
}

/// exec, as a **system call** sees it: REPLACE the running process `p`'s
/// memory with program `name` and arguments `args`. On success the process's
/// very next step is the new program's first instruction — this call does not
/// "return" to the old program at all. On failure the old program keeps
/// running and we hand back an error. This is what a shell calls (in the
/// child, after `fork`) to run a command; `sys_exec` in syscall.rs is the
/// thin wrapper that unpacks the user's request and calls this.
///
/// IMPLEMENT the swap. The hard part — building the new address space — is the
/// given `build_addrspace`; your job is to install it in place of the old one:
///
///  1. Build the new address space (image + stack + argv). Use this process's
///     own trapframe page, and `?` to bail out on failure (which leaves the
///     old image untouched — a failed exec must not destroy the caller):
///         let built = build_addrspace((*p).trapframe as usize, name, args)?;
///
///  2. Remember the old page table so you can free it in a moment:
///         let old = (*p).pagetable;
///
///  3. Install the new address space:
///         (*p).pagetable = built.pagetable;
///
///  4. Point the trapframe at the new program — start address (USER_CODE),
///     the new stack pointer, and a0 = argc / a1 = argv:
///         let tf = (*p).trapframe;
///         (*tf).epc = USER_CODE as u64;
///         (*tf).sp  = built.sp as u64;
///         (*tf).a0  = built.argc as u64;
///         (*tf).a1  = built.argv as u64;
///
///  5. Free the OLD address space. This is safe here: a system call runs on
///     the KERNEL page table, so we are not executing out of the user memory
///     we are freeing. (The trampoline/trapframe pages are shared or owned
///     elsewhere, so freeing the old page table leaves them alone.)
///         vm::free_user_pagetable(old);
///
///  6. Return the new argc; when we return to user mode it lands in a0, the
///     way `main(argc, argv)` expects:
///         Ok(built.argc)
pub unsafe fn exec_into(p: *mut Proc, name: &str, args: &[&str]) -> Result<usize, ExecError> {
    // IMPLEMENT the six steps above. Until you do, exec always fails, so a user
    // program that calls it (like the shell, or `execself`) cannot replace its
    // image. The `let _` below just silences "unused" warnings on the stub; you
    // will use all three once you write the real thing.
    let _ = (p, name, args);
    Err(ExecError::NoMem)
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
