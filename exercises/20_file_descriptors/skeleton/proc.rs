//! proc.rs — the process table and process control blocks (PCBs).
//!
//! Extended for file descriptors: each `Proc` now carries an `ofile` array —
//! its open-file table. `allocproc` starts every process with fds 0, 1, 2
//! already pointing at the console (stdin/stdout/stderr), and `freeproc`
//! closes everything when the process is torn down. (UNDERSTAND — given; the
//! interesting fd logic is in syscall.rs.)

use crate::file::{File, NOFILE};
use crate::kalloc;
use crate::memlayout::{PGSIZE, TRAMPOLINE, TRAPFRAME};
use crate::param::NPROC;
use crate::swtch::Context;
use crate::usermode::Trapframe;
use crate::vm::{self, Pte, PTE_R, PTE_W, PTE_X};
use core::ptr;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ProcState {
    Unused,
    Runnable,
    Running,
    Sleeping,
    Zombie,
}

pub struct Proc {
    pub state: ProcState,
    pub pid: usize,
    pub pagetable: *mut Pte,
    /// Saved registers for when this process isn't running. The scheduler
    /// `swtch`-es into this to resume the process.
    pub context: Context,
    /// This process's trapframe page (see usermode.rs for the layout).
    pub trapframe: *mut Trapframe,
    /// The top-most byte of this process's kernel stack is kstack + PGSIZE.
    pub kstack: usize,
    /// The open-file table: fd `n` is `ofile[n]`. New in exercise 20.
    pub ofile: [File; NOFILE],
    pub name: [u8; 16],
}

impl Proc {
    pub const fn new() -> Proc {
        Proc {
            state: ProcState::Unused,
            pid: 0,
            pagetable: ptr::null_mut(),
            context: Context::zero(),
            trapframe: ptr::null_mut(),
            kstack: 0,
            ofile: [const { File::none() }; NOFILE],
            name: [0; 16],
        }
    }
}

static mut PROCS: [Proc; NPROC] = [const { Proc::new() }; NPROC];
static mut NEXTPID: usize = 1;

/// Raw pointer to process slot `i`. Lets other modules reach the table without
/// creating references into a `static mut`. (UNDERSTAND — given.)
pub unsafe fn proc_at(i: usize) -> *mut Proc {
    ptr::addr_of_mut!(PROCS[i])
}

pub unsafe fn init() {
    for i in 0..NPROC {
        let p = ptr::addr_of_mut!(PROCS[i]);
        (*p).state = ProcState::Unused;
        (*p).pid = 0;
        (*p).pagetable = ptr::null_mut();
        (*p).trapframe = ptr::null_mut();
        (*p).kstack = 0;
        (*p).ofile = [const { File::none() }; NOFILE];
    }
    NEXTPID = 1;
}

unsafe fn alloc_pid() -> usize {
    let pid = NEXTPID;
    NEXTPID += 1;
    pid
}

unsafe fn create_pagetable() -> *mut Pte {
    let pt = kalloc::kalloc() as *mut Pte;
    if !pt.is_null() {
        ptr::write_bytes(pt as *mut u8, 0, PGSIZE);
    }
    pt
}

/// Claim a free slot and give the new process everything it owns: a pid, an
/// (empty) page table, a zeroed trapframe page, a kernel stack page, and an
/// open-file table with the console already at fds 0, 1, 2.
/// On any allocation failure, undo with `freeproc` and hand back null.
pub unsafe fn allocproc() -> *mut Proc {
    for i in 0..NPROC {
        let p = ptr::addr_of_mut!(PROCS[i]);
        if (*p).state == ProcState::Unused {
            (*p).pid = alloc_pid();
            (*p).state = ProcState::Runnable;

            (*p).pagetable = create_pagetable();
            (*p).trapframe = kalloc::kalloc() as *mut Trapframe;
            (*p).kstack = kalloc::kalloc() as usize;
            if (*p).pagetable.is_null() || (*p).trapframe.is_null() || (*p).kstack == 0 {
                freeproc(p);
                return ptr::null_mut();
            }
            ptr::write_bytes((*p).trapframe as *mut u8, 0, PGSIZE);

            // Every process starts with the three standard fds open on the
            // console: 0 = stdin, 1 = stdout, 2 = stderr.
            (*p).ofile = [const { File::none() }; NOFILE];
            (*p).ofile[0] = File::console();
            (*p).ofile[1] = File::console();
            (*p).ofile[2] = File::console();
            return p;
        }
    }
    ptr::null_mut()
}

/// Return everything a process owns. Tolerates half-built processes (any
/// field may still be null/0), so `allocproc` can use it to clean up.
pub unsafe fn freeproc(p: *mut Proc) {
    // close every open file (here just forgetting them: our files hold no
    // resource of their own beyond a slot in this table).
    (*p).ofile = [const { File::none() }; NOFILE];

    if !(*p).trapframe.is_null() {
        kalloc::kfree((*p).trapframe as *mut u8);
        (*p).trapframe = ptr::null_mut();
    }
    if (*p).kstack != 0 {
        kalloc::kfree((*p).kstack as *mut u8);
        (*p).kstack = 0;
    }
    if !(*p).pagetable.is_null() {
        vm::free_user_pagetable((*p).pagetable);
        (*p).pagetable = ptr::null_mut();
    }
    (*p).pid = 0;
    (*p).state = ProcState::Unused;
}

/// Put the two kernel-side mappings into a process's page table: the shared
/// trampoline page and this process's own trapframe page. Note there is no
/// PTE_U on either — they live inside the user's address space, but user
/// code can never touch them. (UNDERSTAND — given.)
pub unsafe fn proc_pagetable(p: *mut Proc) -> Result<(), ()> {
    let pt = (*p).pagetable;
    vm::mappages(pt, TRAMPOLINE, PGSIZE, vm::trampoline_page(), PTE_R | PTE_X)?;
    vm::mappages(pt, TRAPFRAME, PGSIZE, (*p).trapframe as usize, PTE_R | PTE_W)?;
    Ok(())
}
