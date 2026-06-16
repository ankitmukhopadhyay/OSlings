//! proc.rs — the process table and process control blocks (PCBs).
//!
//! A *process* is one running program, plus all the kernel's bookkeeping about
//! it. That bookkeeping lives in a **Process Control Block (PCB)** — here, the
//! `Proc` struct. The kernel keeps a fixed array of them: the process table.

use crate::kalloc;
use crate::memlayout::PGSIZE;
use crate::param::NPROC;
use crate::vm::Pte;
use core::ptr;

/// The lifecycle states a process moves through. This is an `enum`: a type
/// whose value is exactly one of these named variants.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ProcState {
    Unused,   // this table slot is free
    Runnable, // ready to run, waiting for the scheduler to pick it
    Running,  // currently executing on a CPU
    Sleeping, // blocked, waiting for some event
    Zombie,   // finished; waiting for its parent to clean it up
}

/// A Process Control Block: everything the kernel tracks about one process.
/// It grows in later exercises (saved registers, open files, parent, ...).
pub struct Proc {
    pub state: ProcState,
    pub pid: usize,
    /// Root of this process's page table (null when the slot is unused). The
    /// process **owns** this page — freeing the process must free it too.
    pub pagetable: *mut Pte,
    /// Human-readable name; filled in when we load a program (a later exercise).
    pub name: [u8; 16],
}

impl Proc {
    /// A blank, unused slot. `const fn` so the whole table can be built at
    /// compile time (see `PROCS` below) without requiring `Proc: Copy`.
    pub const fn new() -> Proc {
        Proc {
            state: ProcState::Unused,
            pid: 0,
            pagetable: ptr::null_mut(),
            name: [0; 16],
        }
    }
}

/// The process table: a fixed array of PCBs living in static memory. Processes
/// are never heap-allocated — they all live here.
static mut PROCS: [Proc; NPROC] = [const { Proc::new() }; NPROC];

/// Source of unique process ids. Every process gets a fresh, increasing pid.
static mut NEXTPID: usize = 1;

/// Reset the whole table to empty. (UNDERSTAND — given.)
pub unsafe fn init() {
    for i in 0..NPROC {
        let p = ptr::addr_of_mut!(PROCS[i]);
        (*p).state = ProcState::Unused;
        (*p).pid = 0;
        (*p).pagetable = ptr::null_mut();
    }
    NEXTPID = 1;
}

/// Hand out the next unique pid. (UNDERSTAND — given; call it from allocproc.)
unsafe fn alloc_pid() -> usize {
    let pid = NEXTPID;
    NEXTPID += 1;
    pid
}

/// Make an empty page table for a process: one zeroed root page. Returns null
/// if out of memory. (UNDERSTAND — given; reuses your exercise 02/03 code.)
unsafe fn create_pagetable() -> *mut Pte {
    let pt = kalloc::kalloc() as *mut Pte;
    if !pt.is_null() {
        ptr::write_bytes(pt as *mut u8, 0, PGSIZE);
    }
    pt
}

/// Return a process's page table to the allocator. (UNDERSTAND — given; call it
/// from freeproc.)
unsafe fn free_pagetable(pt: *mut Pte) {
    if !pt.is_null() {
        kalloc::kfree(pt as *mut u8);
    }
}

/// Find a free slot and set it up as a new, runnable process.
/// Returns a pointer to the new `Proc`, or null if the table is full.
pub unsafe fn allocproc() -> *mut Proc {
    // IMPLEMENT:
    //   1. Scan the table for the first slot whose state is ProcState::Unused.
    //      Get each slot as a raw pointer WITHOUT making a reference to the
    //      static array:  let p = ptr::addr_of_mut!(PROCS[i]);
    //      Compare with:  if (*p).state == ProcState::Unused { ... }
    //   2. If no slot is free, return ptr::null_mut().
    //   3. Otherwise set the slot up:
    //        (*p).pid = alloc_pid();
    //        (*p).state = ProcState::Runnable;
    //        (*p).pagetable = create_pagetable();
    //      If the page table came back null (out of memory), return
    //      ptr::null_mut().
    //   4. Return p.
    ptr::null_mut()
}

/// Tear a process down and return its slot to the free pool.
///
/// The process **owns** its page table, so we must free that here — otherwise
/// the page leaks. (And we must not free it twice: after this, pagetable is
/// null and the slot is Unused.)
pub unsafe fn freeproc(p: *mut Proc) {
    // IMPLEMENT:
    //   1. free_pagetable((*p).pagetable);   // release the owned resource
    //   2. (*p).pagetable = ptr::null_mut();
    //      (*p).pid = 0;
    //      (*p).state = ProcState::Unused;
    let _ = p; // remove once implemented
}
