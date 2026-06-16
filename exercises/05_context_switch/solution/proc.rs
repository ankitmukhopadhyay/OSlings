//! proc.rs — the process table and process control blocks. (Ex 04 solution.)

use crate::kalloc;
use crate::memlayout::PGSIZE;
use crate::param::NPROC;
use crate::vm::Pte;
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
    pub name: [u8; 16],
}

impl Proc {
    pub const fn new() -> Proc {
        Proc {
            state: ProcState::Unused,
            pid: 0,
            pagetable: ptr::null_mut(),
            name: [0; 16],
        }
    }
}

static mut PROCS: [Proc; NPROC] = [const { Proc::new() }; NPROC];
static mut NEXTPID: usize = 1;

pub unsafe fn init() {
    for i in 0..NPROC {
        let p = ptr::addr_of_mut!(PROCS[i]);
        (*p).state = ProcState::Unused;
        (*p).pid = 0;
        (*p).pagetable = ptr::null_mut();
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

unsafe fn free_pagetable(pt: *mut Pte) {
    if !pt.is_null() {
        kalloc::kfree(pt as *mut u8);
    }
}

pub unsafe fn allocproc() -> *mut Proc {
    for i in 0..NPROC {
        let p = ptr::addr_of_mut!(PROCS[i]);
        if (*p).state == ProcState::Unused {
            (*p).pid = alloc_pid();
            (*p).state = ProcState::Runnable;
            (*p).pagetable = create_pagetable();
            if (*p).pagetable.is_null() {
                return ptr::null_mut();
            }
            return p;
        }
    }
    ptr::null_mut()
}

pub unsafe fn freeproc(p: *mut Proc) {
    free_pagetable((*p).pagetable);
    (*p).pagetable = ptr::null_mut();
    (*p).pid = 0;
    (*p).state = ProcState::Unused;
}
