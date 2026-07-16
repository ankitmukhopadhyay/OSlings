# Hints - 21 fork / wait / exit

## Hint 1
Two handlers, both in `syscall.rs`, and everything hard is already given.

- `sys_fork` builds a child that is a copy of the parent. The parent is
  `usermode::curproc()`. Every piece you need is a given helper: `proc::allocproc`
  (a fresh process, from exercise 18), `proc::proc_pagetable` (map its trampoline
  + trapframe), `vm::uvmcopy` (copy the parent's memory into the child - new
  here), and `usermode::ready` (make it schedulable). The one clever line is
  setting the child's trapframe `a0` to 0, which is *why fork returns 0 in the
  child*. The six steps are spelled out in the `// IMPLEMENT` comment.
- `sys_wait` scans the process table for a **zombie** child and reaps it. The
  loop that blocks and retries (using the given `proc_yield`) is already written;
  you only fill in the scan: walk `0..NPROC`, and for a slot that is your child
  (`(*q).parent == p`) and a zombie (`(*q).state == ProcState::Zombie`), copy out
  its status, `freeproc` it, and return its pid.

If the child never runs, the bug is in fork (does it create the child and return
0 in it?). If the child runs but `wait` never returns, the bug is in the wait
scan (does it match a Zombie child of `p`?).

## Hint 2
The shapes.

`sys_wait` - the reaping scan goes where the `// IMPLEMENT` comment is:

```rust
for i in 0..NPROC {
    let q = proc::proc_at(i);
    if (*q).parent == p && (*q).state == ProcState::Zombie {
        let pid = (*q).pid;
        if status_addr != 0 {
            let st = (*q).xstate as i32;
            let _ = vm::copyout((*p).pagetable, status_addr, &st.to_le_bytes());
        }
        proc::freeproc(q);   // reap it: free the slot for reuse
        return pid as isize;
    }
}
// (given loop tail: return -1 if no children, else proc_yield and retry)
```

`sys_fork` - build the child, following the six steps:

```rust
unsafe {
    let parent = usermode::curproc();
    let child = proc::allocproc();
    if child.is_null() { return -1; }
    // give the child a page table + a copy of the parent's memory
    if proc::proc_pagetable(child).is_err()
        || vm::uvmcopy((*parent).pagetable, (*child).pagetable).is_err()
    {
        proc::freeproc(child);
        return -1;
    }
    // resume the child at the same instruction, but with fork() == 0
    *(*child).trapframe = core::ptr::read((*parent).trapframe);
    (*(*child).trapframe).a0 = 0;
    // inherit the parent's open files; remember the parent for wait()
    (*child).ofile = (*parent).ofile;
    (*child).parent = parent;
    // make it schedulable and runnable; the parent gets its pid
    usermode::ready(child);
    (*child).state = ProcState::Runnable;
    (*child).pid as isize
}
```

## Hint 3
Both handlers in full, with the reasoning.

```rust
fn sys_fork() -> isize {
    unsafe {
        let parent = usermode::curproc();

        // 1. a blank child: pid, page table, trapframe, kstack, console fds.
        let child = proc::allocproc();
        if child.is_null() {
            return -1; // no free process slot
        }

        // 2. map the child's kernel pages, then COPY the parent's user memory
        //    into it, so the child has its own private duplicate.
        if proc::proc_pagetable(child).is_err()
            || vm::uvmcopy((*parent).pagetable, (*child).pagetable).is_err()
        {
            proc::freeproc(child);
            return -1;
        }

        // 3. copy the parent's saved registers so the child resumes at the SAME
        //    instruction after fork - then make the child's return value 0.
        *(*child).trapframe = core::ptr::read((*parent).trapframe);
        (*(*child).trapframe).a0 = 0;

        // 4. the child inherits the open files and knows who its parent is.
        (*child).ofile = (*parent).ofile;
        (*child).parent = parent;

        // 5. schedulable + runnable; return the child's pid to the PARENT
        //    (usertrap puts this in the parent's a0).
        usermode::ready(child);
        (*child).state = ProcState::Runnable;
        (*child).pid as isize
    }
}

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
            if !proc::has_children(p) {
                return -1;             // no children: nothing to wait for
            }
            usermode::proc_yield(p);   // a child exists but hasn't exited; block
        }
    }
}
```

Why fork works: a process *is* its memory + its registers + its open files, so
duplicating those three is duplicating the process. `uvmcopy` clones the memory;
copying the trapframe clones the registers (so the child picks up exactly where
the parent was); `ofile` clones the file table. The single asymmetry - the
child's `a0 = 0` versus the parent's `a0 = child pid` - is the entire mechanism
by which one `fork()` returns two different values in two processes.

Why wait works: `exit` (given) leaves a finished process as a Zombie carrying its
status, and records who its parent is. So `wait` just searches for a zombie whose
parent is itself, hands back its status, and frees the slot. If a child exists
but has not exited yet, `proc_yield` gives the CPU to the scheduler so the child
can run; when the parent is scheduled again, it loops and checks once more. That
loop is how a parent "waits" without freezing the kernel.
