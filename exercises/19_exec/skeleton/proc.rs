//! proc.rs — the process table and process control blocks (PCBs).
//!
//! Extended for user mode: a process that runs user code needs two more
//! pieces of per-process memory, so `Proc` grows two fields and
//! `allocproc`/`freeproc` manage them (UNDERSTAND — given):
//!
//!   - `trapframe`: one page where the trampoline parks all 31 user
//!     registers every time the process traps into the kernel.
//!   - `kstack`: one page of *kernel* stack. When this process traps in, the
//!     kernel code that handles the trap needs a stack of its own — it
//!     cannot trust or reuse the user's stack.

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
/// (empty) page table, a zeroed trapframe page, and a kernel stack page.
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
            return p;
        }
    }
    ptr::null_mut()
}

/// Return everything a process owns. Tolerates half-built processes (any
/// field may still be null/0), so `allocproc` can use it to clean up.
pub unsafe fn freeproc(p: *mut Proc) {
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
