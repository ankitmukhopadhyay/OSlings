//! syscall.rs — the system-call table: numbers, dispatch, and handlers.
//!
//! Exercise 21 added the process calls fork/wait/exit. This exercise adds the
//! last piece of the trio that starts programs: **exec** — replace the calling
//! process's memory with a different program. Together, `fork` + `exec` + `wait`
//! are how every Unix shell runs a command, and now that exec is a system call,
//! the shell itself can run in user mode (see `sh` in exec.rs).
//!
//! `sys_exec` here is given (it just unpacks the user's request); the mechanism
//! it calls — `exec_into` — is the one you implement, over in exec.rs.
//!
//! The numbers still match xv6's, so nothing has to be renumbered.

use crate::file::{File, FileKind, NOFILE, O_CREATE, O_RDWR, O_TRUNC, O_WRONLY};
use crate::fs::{InodeKind, FS, ROOT};
use crate::param::NPROC;
use crate::proc::{self, Proc, ProcState};
use crate::usermode;
use crate::vm;

pub const SYS_FORK: usize = 1; // fork() -> child pid (parent) / 0 (child)
pub const SYS_EXIT: usize = 2; // exit(status): end the program
pub const SYS_WAIT: usize = 3; // wait(&status) -> pid of a finished child
pub const SYS_READ: usize = 5; // read(fd, buf, len) -> bytes read
pub const SYS_EXEC: usize = 7; // exec(path, argv): run a new program in place
pub const SYS_GETPID: usize = 11; // getpid() -> pid
pub const SYS_OPEN: usize = 15; // open(path, flags) -> fd
pub const SYS_WRITE: usize = 16; // write(fd, buf, len) -> bytes written
pub const SYS_CLOSE: usize = 21; // close(fd) -> 0

/// Route a system call to its handler and hand back the return value.
/// Unknown numbers return -1. (Given — it has just grown fork and wait.)
pub fn dispatch(num: usize, a0: usize, a1: usize, a2: usize) -> isize {
    match num {
        SYS_FORK => sys_fork(),
        SYS_EXIT => sys_exit(a0 as isize),
        SYS_WAIT => sys_wait(a0),
        SYS_EXEC => sys_exec(a0, a1),
        SYS_GETPID => sys_getpid(),
        SYS_READ => sys_read(a0, a1, a2),
        SYS_WRITE => sys_write(a0, a1, a2),
        SYS_OPEN => sys_open(a0, a1),
        SYS_CLOSE => sys_close(a0),
        _ => -1,
    }
}

// ========================================================================
//  The process calls: fork, exit, wait.
// ========================================================================

/// fork(): create a near-exact copy of the calling process — the **child** —
/// and return the child's pid to the parent, but 0 to the child. That single
/// call returning *two* different values, in two now-separate processes, is the
/// whole trick, and it is how every Unix program launches another.
///
/// IMPLEMENT, step by step. `parent` is the caller; you build `child`:
///
///  1. The parent is the current process:
///         let parent = usermode::curproc();
///
///  2. Allocate the child (pid, page table, trapframe, kstack, console fds):
///         let child = proc::allocproc();
///         if child.is_null() { return -1; }        // out of processes
///
///  3. Give the child the kernel-side mappings (trampoline + trapframe), then
///     COPY the parent's user memory into it (given: vm::uvmcopy). On failure,
///     free the half-built child and bail:
///         if proc::proc_pagetable(child).is_err()
///             || vm::uvmcopy((*parent).pagetable, (*child).pagetable).is_err()
///         {
///             proc::freeproc(child);
///             return -1;
///         }
///
///  4. Copy the parent's saved user registers so the child resumes at the SAME
///     instruction — then override the child's a0 to 0, so `fork` returns 0 in
///     the child:
///         *(*child).trapframe = core::ptr::read((*parent).trapframe);
///         (*(*child).trapframe).a0 = 0;
///
///  5. The child inherits the parent's open files, and remembers its parent so
///     the parent's `wait` can find it:
///         (*child).ofile = (*parent).ofile;
///         (*child).parent = parent;
///
///  6. Make the child schedulable and runnable, and return its pid to the
///     parent (a0 in the PARENT is set from this return value by usertrap):
///         usermode::ready(child);
///         (*child).state = ProcState::Runnable;
///         (*child).pid as isize
fn sys_fork() -> isize {
    unsafe {
        let parent = usermode::curproc();
        let child = proc::allocproc();
        if child.is_null() {
            return -1;
        }
        if proc::proc_pagetable(child).is_err()
            || vm::uvmcopy((*parent).pagetable, (*child).pagetable).is_err()
        {
            proc::freeproc(child);
            return -1;
        }
        *(*child).trapframe = core::ptr::read((*parent).trapframe);
        (*(*child).trapframe).a0 = 0; // the child's fork() returns 0
        (*child).ofile = (*parent).ofile;
        (*child).parent = parent;
        usermode::ready(child);
        (*child).state = ProcState::Runnable;
        (*child).pid as isize
    }
}

/// exit(status): the program is done. Hand the status to `exit_current`, which
/// records it, marks this process a Zombie so its parent's `wait` can find it,
/// and gives the CPU back to the scheduler for good. Never returns.
/// (UNDERSTAND — given; this is the model your `wait` reaps.)
fn sys_exit(status: isize) -> ! {
    unsafe { usermode::exit_current(status) }
}

/// wait(status_addr): block until one of this process's children has exited,
/// then reap it — free its slot and return its pid. If `status_addr` is
/// non-zero, the child's exit status is written there (as a 4-byte int) for the
/// parent to read.
///
/// IMPLEMENT the reaping scan (the block-and-retry loop around it is given):
///
///   Walk the whole process table with `proc::proc_at(i)` for `i in 0..NPROC`.
///   For a slot `q` that is a child of `p` (`(*q).parent == p`) AND is a
///   Zombie (`(*q).state == ProcState::Zombie`):
///     - remember its pid: `let pid = (*q).pid;`
///     - if `status_addr != 0`, copy its status out to the user as 4 bytes:
///           let st = (*q).xstate as i32;
///           let _ = vm::copyout((*p).pagetable, status_addr, &st.to_le_bytes());
///     - reap it: `proc::freeproc(q);`
///     - return its pid: `return pid as isize;`
///   If you scan the whole table without finding a Zombie child, fall through
///   to the given code below, which either reports "no children" or blocks.
fn sys_wait(status_addr: usize) -> isize {
    unsafe {
        let p = usermode::curproc();
        loop {
            for i in 0..NPROC {
                let q = proc::proc_at(i);
                if (*q).parent == p && (*q).state == ProcState::Zombie {
                    let pid = (*q).pid;
                    if status_addr != 0 {
                        let st = (*q).xstate as i32;
                        let _ = vm::copyout((*p).pagetable, status_addr, &st.to_le_bytes());
                    }
                    proc::freeproc(q);
                    return pid as isize;
                }
            }

            // No child has exited yet. (Given.) If this process has no children
            // at all, there is nothing to wait for.
            if !proc::has_children(p) {
                return -1;
            }
            // Otherwise, give up the CPU so a child can run, and try again when
            // the scheduler picks us back up.
            usermode::proc_yield(p);
        }
    }
}

/// getpid(): which process am I? (UNDERSTAND — given.)
fn sys_getpid() -> isize {
    unsafe { (*usermode::curproc()).pid as isize }
}

// ========================================================================
//  exec: run a different program in place of this one.
// ========================================================================

const MAXPATH: usize = 32; // longest program name we accept
const MAXARGV: usize = 8; // most arguments (including argv[0])
const MAXARGLEN: usize = 32; // longest single argument (including its NUL)

/// Scratch storage for the argument strings we copy out of user memory, so we
/// can hand them to `exec_into` as ordinary `&str`s. (Given.)
struct ArgvStore {
    bufs: [[u8; MAXARGLEN]; MAXARGV],
    lens: [usize; MAXARGV],
    n: usize,
}

/// The argv scratch lives in a `static`, NOT on the kernel stack. A kernel
/// stack is a single 4 KiB page (allocproc gives each process one page), and
/// copying a dozen argument strings onto it — on top of the exec call chain —
/// would risk overflowing it and corrupting the page below. exec runs one at a
/// time on our single hart, so one shared scratch buffer is safe. (Given.)
static mut ARGV_STORE: ArgvStore = ArgvStore {
    bufs: [[0; MAXARGLEN]; MAXARGV],
    lens: [0; MAXARGV],
    n: 0,
};

/// Copy an argv out of user memory into `store`. `uargv` is a user address of
/// an array of user string pointers, ending in a NULL; for each pointer we
/// copy the string it names into `store`. (Given — it is `copyin` + `copyinstr`,
/// which you wrote/used earlier, applied in a loop.)
unsafe fn fetch_argv(p: *mut Proc, uargv: usize, store: &mut ArgvStore) -> Result<(), ()> {
    let pt = (*p).pagetable;
    let mut i = 0;
    loop {
        if i >= MAXARGV {
            return Err(()); // too many arguments
        }
        // read the i-th 8-byte pointer from the user's argv array.
        let mut ptrbuf = [0u8; 8];
        vm::copyin(pt, &mut ptrbuf, uargv + i * 8)?;
        let uptr = usize::from_le_bytes(ptrbuf);
        if uptr == 0 {
            break; // the NULL terminator: end of the argument list
        }
        // copy the string it points at into our scratch buffer.
        let len = vm::copyinstr(pt, &mut store.bufs[i], uptr)?;
        store.lens[i] = len;
        i += 1;
    }
    store.n = i;
    Ok(())
}

/// exec(path, argv): replace this process's memory with the program named by
/// `path`, passing it the argument vector `argv`. Returns argc on success —
/// though "returning" here means the process resumes as the NEW program — or
/// -1 on failure, in which case THIS program keeps running.
///
/// This wrapper just reads the request out of user memory (the path string and
/// the argv strings) and calls exec.rs's `exec_into`, which does the real work.
/// (UNDERSTAND — given; `exec_into` is the part you implement.)
fn sys_exec(path: usize, uargv: usize) -> isize {
    unsafe {
        let p = usermode::curproc();

        // 1. the program name, copied out of user memory as a string.
        let mut pathbuf = [0u8; MAXPATH];
        let plen = match vm::copyinstr((*p).pagetable, &mut pathbuf, path) {
            Ok(n) => n,
            Err(_) => return -1,
        };
        let name = match core::str::from_utf8(&pathbuf[..plen]) {
            Ok(s) => s,
            Err(_) => return -1,
        };

        // 2. the argument strings, copied out of user memory into the shared
        //    scratch (see ARGV_STORE above for why it is not on the stack).
        let store = &mut *core::ptr::addr_of_mut!(ARGV_STORE);
        if fetch_argv(p, uargv, store).is_err() {
            return -1;
        }
        let mut argv: [&str; MAXARGV] = [""; MAXARGV];
        for i in 0..store.n {
            argv[i] = core::str::from_utf8(&store.bufs[i][..store.lens[i]]).unwrap_or("");
        }
        // argv[0] is conventionally the program name (the same as `path`), and
        // exec re-adds it as argv[0], so hand it the arguments AFTER argv[0].
        let rest: &[&str] = if store.n > 1 { &argv[1..store.n] } else { &[] };

        // 3. do the exec. On success we return INTO the new program.
        match crate::exec::exec_into(p, name, rest) {
            Ok(argc) => argc as isize,
            Err(_) => -1,
        }
    }
}

// ========================================================================
//  The file-descriptor helpers.
// ========================================================================

/// Install `file` in the current process's open-file table and return the
/// file descriptor (the slot's index), or -1 if the table is full.
///
/// IMPLEMENT: the current process is `usermode::curproc()` — a `*mut Proc`.
/// Its open-file table is `(*p).ofile`, an array of `NOFILE` `File`s. Scan it
/// for the first slot whose `kind` is `FileKind::None` (a free slot); when you
/// find one, store `file` there and return that index as an `isize`. If every
/// slot is taken, return -1.
///
///     let p = usermode::curproc();
///     for fd in 0..NOFILE {
///         if (*p).ofile[fd].kind == FileKind::None {
///             (*p).ofile[fd] = file;
///             return fd as isize;
///         }
///     }
///     -1
fn fdalloc(file: File) -> isize {
    unsafe {
        let p = usermode::curproc();
        for fd in 0..NOFILE {
            if (*p).ofile[fd].kind == FileKind::None {
                (*p).ofile[fd] = file;
                return fd as isize;
            }
        }
        -1
    }
}

/// Fetch the `File` behind descriptor `fd` in the current process, or `None`
/// if `fd` is out of range or not open. (Given — a small helper the handlers
/// share. `File` is `Copy`, so this hands back a copy; to change the stored
/// file, index `(*p).ofile[fd]` directly, as `sys_read` does for the offset.)
unsafe fn getfile(p: *mut Proc, fd: usize) -> Option<File> {
    if fd >= NOFILE {
        return None;
    }
    let f = (*p).ofile[fd];
    if f.kind == FileKind::None {
        None
    } else {
        Some(f)
    }
}

/// open(path, flags): open (or create) a file and return a new descriptor.
///
/// Paths are resolved in the root directory (rv6 has no per-process working
/// directory yet — that arrives with the userland shell). `flags` is a
/// bitmask from file.rs: O_RDONLY/O_WRONLY/O_RDWR pick the access mode,
/// O_CREATE makes the file if it is missing, O_TRUNC empties an existing one.
///
/// IMPLEMENT, in five steps:
///
///  1. Fetch the path string from user memory. It is a NUL-terminated string
///     at user address `path`, so use the given `vm::copyinstr`:
///         let p = usermode::curproc();
///         let mut namebuf = [0u8; 32];
///         let len = match vm::copyinstr((*p).pagetable, &mut namebuf, path) {
///             Ok(n) => n,
///             Err(_) => return -1,
///         };
///         let name = &namebuf[..len];
///
///  2. Work out the access mode from the flags. By convention:
///         let writable = flags & O_WRONLY != 0 || flags & O_RDWR != 0;
///         let readable = flags & O_WRONLY == 0; // RDONLY and RDWR can read
///
///  3. Find the file's inode — creating it first if O_CREATE was asked for
///     and it does not exist. Everything lives in the root directory `ROOT`:
///         let mut fsg = FS.lock();
///         let inum = if flags & O_CREATE != 0 {
///             match fsg.dircreate(ROOT, name, InodeKind::File) {
///                 Ok(i) => i,
///                 Err(_) => match fsg.dirlookup(ROOT, name) {
///                     Ok(i) => i,             // already existed: use it
///                     Err(_) => return -1,
///                 },
///             }
///         } else {
///             match fsg.dirlookup(ROOT, name) {
///                 Ok(i) => i,
///                 Err(_) => return -1,        // no such file
///             }
///         };
///
///  4. If O_TRUNC was set, empty the file, then drop the fs lock:
///         if flags & O_TRUNC != 0 {
///             let _ = fsg.truncate(inum);
///         }
///         drop(fsg);
///
///  5. Build the `File` and hand it to `fdalloc`, returning the fd it gives:
///         let file = File {
///             kind: FileKind::Inode,
///             inum,
///             off: 0,
///             readable,
///             writable,
///         };
///         fdalloc(file)
fn sys_open(path: usize, flags: usize) -> isize {
    unsafe {
        let p = usermode::curproc();
        let mut namebuf = [0u8; 32];
        let len = match vm::copyinstr((*p).pagetable, &mut namebuf, path) {
            Ok(n) => n,
            Err(_) => return -1,
        };
        let name = &namebuf[..len];

        let writable = flags & O_WRONLY != 0 || flags & O_RDWR != 0;
        let readable = flags & O_WRONLY == 0;

        let mut fsg = FS.lock();
        let inum = if flags & O_CREATE != 0 {
            match fsg.dircreate(ROOT, name, InodeKind::File) {
                Ok(i) => i,
                Err(_) => match fsg.dirlookup(ROOT, name) {
                    Ok(i) => i,
                    Err(_) => return -1,
                },
            }
        } else {
            match fsg.dirlookup(ROOT, name) {
                Ok(i) => i,
                Err(_) => return -1,
            }
        };
        if flags & O_TRUNC != 0 {
            let _ = fsg.truncate(inum);
        }
        drop(fsg);

        let file = File {
            kind: FileKind::Inode,
            inum,
            off: 0,
            readable,
            writable,
        };
        fdalloc(file)
    }
}

/// read(fd, buf, len): read up to `len` bytes from `fd` into the user buffer
/// `buf`. Returns the number of bytes read — 0 means end of file — or -1 on
/// error. It may legally return FEWER than `len` (the caller loops).
///
/// This is the mirror of `sys_write` below — read that first as your model.
/// The new idea here is the **offset**: an inode read starts at the file's
/// current cursor `off` and, after reading `n` bytes, advances the cursor by
/// `n` so the next read continues where this one stopped.
///
/// IMPLEMENT:
///
///  1. Look up the file and reject a read we're not allowed to do:
///         let p = usermode::curproc();
///         let file = match getfile(p, fd) {
///             Some(f) if f.readable => f,
///             _ => return -1,
///         };
///
///  2. Split on what the fd refers to:
///     - `FileKind::Console`: read one byte with `crate::console::getc()` and
///       copy it out to the user (blocking until a key is pressed):
///           FileKind::Console => {
///               let b = [crate::console::getc()];
///               if vm::copyout((*p).pagetable, buf, &b).is_err() {
///                   return -1;
///               }
///               1
///           }
///     - `FileKind::Inode`: read at the file's offset into a kernel buffer,
///       copy it out to the user, then ADVANCE the stored offset by `n`:
///           FileKind::Inode => {
///               let mut kbuf = [0u8; 128];
///               let want = core::cmp::min(len, kbuf.len());
///               let n = match FS.lock().read_at(file.inum, file.off, &mut kbuf[..want]) {
///                   Ok(n) => n,
///                   Err(_) => return -1,
///               };
///               if n > 0 && vm::copyout((*p).pagetable, buf, &kbuf[..n]).is_err() {
///                   return -1;
///               }
///               (*p).ofile[fd].off += n; // advance the cursor
///               n as isize
///           }
///     - `FileKind::None`: unreachable (getfile already rejected it) — return -1.
fn sys_read(fd: usize, buf: usize, len: usize) -> isize {
    unsafe {
        let p = usermode::curproc();
        let file = match getfile(p, fd) {
            Some(f) if f.readable => f,
            _ => return -1,
        };
        match file.kind {
            FileKind::Console => {
                // A console read blocks until a key is pressed, and that key
                // arrives as a device interrupt — so we must let supervisor
                // interrupts through while we wait, or the keypress is never
                // noticed and the read hangs forever. This is the first time a
                // USER program blocks on the console (the shell's `read`); we
                // enable interrupts here, at the one syscall that actually
                // blocks, rather than for every syscall — so the deeper calls
                // like exec keep running on a quiet, shallow kernel stack.
                // (The harness never reads the console and keeps interrupts off
                // for deterministic timing.)
                #[cfg(not(feature = "harness"))]
                crate::trap::intr_on();
                let b = [crate::console::getc()];
                if vm::copyout((*p).pagetable, buf, &b).is_err() {
                    return -1;
                }
                1
            }
            FileKind::Inode => {
                let mut kbuf = [0u8; 128];
                let want = core::cmp::min(len, kbuf.len());
                let n = match FS.lock().read_at(file.inum, file.off, &mut kbuf[..want]) {
                    Ok(n) => n,
                    Err(_) => return -1,
                };
                if n > 0 && vm::copyout((*p).pagetable, buf, &kbuf[..n]).is_err() {
                    return -1;
                }
                (*p).ofile[fd].off += n; // advance the cursor
                n as isize
            }
            FileKind::None => -1,
        }
    }
}

/// write(fd, buf, len): write `len` bytes from the user buffer `buf` to `fd`.
/// Returns the number of bytes written, or -1 on error. This is the worked
/// model for `sys_read` above: same shape, opposite direction. (UNDERSTAND —
/// given; it generalizes exercise 18's console-only write.)
fn sys_write(fd: usize, buf: usize, len: usize) -> isize {
    unsafe {
        let p = usermode::curproc();
        let file = match getfile(p, fd) {
            Some(f) if f.writable => f,
            _ => return -1,
        };
        match file.kind {
            FileKind::Console => {
                // copy the user's bytes into the kernel a chunk at a time and
                // emit them to the console (exactly exercise 18's write).
                let mut kbuf = [0u8; 64];
                let mut done = 0;
                while done < len {
                    let n = core::cmp::min(kbuf.len(), len - done);
                    if vm::copyin((*p).pagetable, &mut kbuf[..n], buf + done).is_err() {
                        return -1;
                    }
                    emit(&kbuf[..n]);
                    done += n;
                }
                len as isize
            }
            FileKind::Inode => {
                // copy the user's bytes in and write them to the file at the
                // current offset, advancing the cursor by what we wrote.
                let mut kbuf = [0u8; 128];
                let mut done = 0;
                while done < len {
                    let n = core::cmp::min(kbuf.len(), len - done);
                    if vm::copyin((*p).pagetable, &mut kbuf[..n], buf + done).is_err() {
                        return -1;
                    }
                    let off = (*p).ofile[fd].off;
                    match FS.lock().write_at(file.inum, off, &kbuf[..n]) {
                        Ok(w) => {
                            (*p).ofile[fd].off += w; // advance the cursor
                            done += w;
                        }
                        Err(_) => return -1,
                    }
                }
                len as isize
            }
            FileKind::None => -1,
        }
    }
}

/// close(fd): let go of a descriptor. (UNDERSTAND — given.)
fn sys_close(fd: usize) -> isize {
    unsafe {
        let p = usermode::curproc();
        if getfile(p, fd).is_none() {
            return -1;
        }
        (*p).ofile[fd] = File::none();
        0
    }
}

/// Where console writes go: the UART normally; a capture buffer under the
/// test harness, so the self-check can verify exactly what a program said.
/// (Given.)
fn emit(bytes: &[u8]) {
    #[cfg(feature = "harness")]
    capture::put(bytes);
    #[cfg(not(feature = "harness"))]
    for &b in bytes {
        crate::uart::putc(b);
    }
}

#[cfg(feature = "harness")]
pub use capture::{captured, clear_capture};

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

    /// Everything written to the console since the last clear, as text.
    pub fn captured() -> &'static str {
        unsafe {
            let len = *addr_of!(LEN);
            let buf = core::slice::from_raw_parts(addr_of!(BUF) as *const u8, len);
            core::str::from_utf8(buf).unwrap_or("")
        }
    }

    /// Reset the capture buffer (so each self-check step sees only its own
    /// program's output).
    pub fn clear_capture() {
        unsafe {
            *addr_of_mut!(LEN) = 0;
        }
    }
}
