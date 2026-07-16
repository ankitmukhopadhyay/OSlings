# 21 · fork / wait / exit

> **Learn -> Understand -> Implement.** So far the kernel ran one user program
> at a time. This exercise adds the three system calls that let a program start
> *another* program and collect its result - `fork`, `exit`, and `wait` - the
> trio at the heart of how every Unix shell launches commands. To run more than
> one process, the kernel needs a **scheduler**, and you finally get to use the
> round-robin policy you wrote back in exercise 06.

## Learn

### fork: one call that returns twice

`fork()` is the strangest and most important system call in Unix. It makes a
near-exact **copy** of the running process - a new **child** process with its
own copy of the parent's memory, registers, and open files - and then *both*
processes return from the same `fork()` call and keep running. They are
distinguished only by fork's return value:

```
        pid = fork();
       /            \
  parent            child
  pid = child's id  pid = 0
```

The parent gets the child's process id (a positive number); the child gets 0.
That one difference is how a program knows which copy it is, and it branches
accordingly:

```
    let pid = fork();
    if pid == 0 {
        // ...this is the child...
    } else {
        // ...this is the parent; pid is the child's id...
    }
```

It feels like magic, but the mechanism is exactly the machinery you already
built. To copy a process you: allocate a new `Proc` (exercise 18's `allocproc`),
give it a fresh page table and copy the parent's user memory into it (a new
helper, `uvmcopy`, built on the page-table walk from exercise 19), and copy the
parent's saved registers - its **trapframe** - so the child resumes at the very
same instruction. Then you set the child's saved `a0` to 0, and *that* is why
`fork` returns 0 in the child.

### exit and wait: finishing, and collecting the result

A child eventually calls `exit(status)`. But a process cannot simply vanish -
its parent may want to know *how* it finished (did it succeed? with what code?).
So `exit` puts the process into a **zombie** state: it stops running and records
its exit status, but its slot in the process table lingers, holding that status,
until someone collects it.

That someone is the parent, calling `wait(&status)`. `wait` looks for one of the
calling process's children that has become a zombie, and when it finds one it
**reaps** it: reads its exit status, frees its slot for good, and returns its
pid. If the parent calls `wait` before any child has exited, `wait` **blocks** -
it gives up the CPU and tries again later, once a child has had a chance to run
and exit.

```
  parent: fork() ─────────────▶ child runs ─────▶ exit(7)  [now a zombie]
  parent: wait(&s) ──blocks──▶ ... ──▶ reaps the zombie ──▶ returns pid, s = 7
```

"Zombie" is the real technical term. A zombie is a finished process kept around
only so its parent can read its result; `wait` is what lays it to rest.

### The scheduler: running more than one process

With `fork`, there can suddenly be several **runnable** processes at once. The
kernel needs a **scheduler**: a loop that repeatedly picks a runnable process,
switches into it, and regains control when that process gives the CPU back -
either by exiting or by blocking in `wait`. Switching in and out is `swtch`,
which you wrote in exercise 05; picking *which* process to run next is the
round-robin policy you wrote in exercise 06. This exercise finally wires them
together to run real user processes:

```
  scheduler loop:
    pick a Runnable process        (RoundRobin::pick_next - your exercise 06!)
    swtch into it                  (your exercise 05 swtch)
    ...it runs until it yields (wait) or exits...
    swtch back to the scheduler
    repeat, until the first process (and its whole tree) has finished
```

A process gives the CPU back in one of two ways, both given:

- **`proc_yield`** (used by `wait`): mark myself Runnable again and `swtch` to
  the scheduler. When the scheduler picks me later, I resume right where I left
  off - this is how a blocked `wait` waits without freezing the machine.
- **`exit_current`** (used by `exit`): mark myself a Zombie and `swtch` to the
  scheduler for good; the scheduler never switches back into a zombie.

All of the scheduler machinery - the loop, `proc_yield`, `exit_current`, the
`forkret` entry point a fresh process starts at, and `uvmcopy` - is **given**.
Your job is the two process-management syscalls that use it: **`fork`** and
**`wait`**. (`exit` is given too, as the model your `wait` reaps.)

### Cooperative scheduling (why nothing is interrupted)

rv6's scheduler is **cooperative**: a process keeps the CPU until it *chooses*
to give it up (by exiting, or by blocking in `wait`). Nothing forcibly preempts
it. That keeps the behavior simple and predictable, which is exactly what you
want while first learning fork/wait. (Real kernels also *preempt* on a timer so
one process cannot hog the CPU; rv6 has the timer from exercise 14, and turning
it into preemption is a natural next step, but not one this exercise takes.)

## Understand

Read these, in order:

1. `rv6/src/proc.rs`: `Proc` gained two fields - `parent` (who forked it) and
   `xstate` (the status it left on exit) - plus a `has_children` helper. These
   are what `wait` uses to find and collect a child.
2. `rv6/src/usermode.rs`: the scheduler. Read `run` (the driver the shell/harness
   calls), then `scheduler` (the loop, using `RoundRobin`), then the three glue
   pieces `ready` / `proc_yield` / `exit_current`, and `forkret`. This is the
   machinery; you do not edit it, but understanding it makes fork/wait obvious.
3. `rv6/src/vm.rs`: `uvmcopy`, which copies a parent's user pages into a child's
   page table - the memory half of `fork`. It walks the page table exactly like
   `free_pt` above it, but copying instead of freeing.
4. `rv6/src/syscall.rs`: the new numbers (`fork` = 1, `wait` = 3), the given
   `exit`, and the two handlers you will write, `sys_fork` and `sys_wait`.
5. `rv6/src/exec.rs`: two new programs - `forktest` (forks one child) and
   `forks2` (forks two) - written in assembly, so you can see real `fork`/`wait`
   calls.

Control flow of `run forktest`:

```
parent: fork()  ──▶ sys_fork: copy me into a child, child's a0 = 0    <- YOU
        (parent gets child's pid; child gets 0 and runs the child branch)
parent: writes "parent", then wait(&s)
        ──▶ sys_wait: no zombie child yet ──▶ proc_yield (block)       <- YOU
scheduler: runs the child ──▶ child writes "child", exit(7) [zombie]
scheduler: runs the parent again ──▶ sys_wait reaps the zombie,
        s = 7, returns the child's pid                                 <- YOU
parent: exit(7 + 10 = 17)
```

## Implement

Two handlers, both in `rv6/src/syscall.rs`:

1. **`sys_fork`** (the star): allocate a child, copy the parent's address space
   into it with `vm::uvmcopy`, copy the parent's trapframe and set the child's
   `a0` to 0, inherit the parent's open files, record the parent, make the child
   runnable, and return the child's pid. The `// IMPLEMENT` comment walks
   through all six steps.
2. **`sys_wait`**: scan the process table for a **zombie** child of the caller;
   when you find one, copy its status out to the user, free it, and return its
   pid. The block-and-retry loop around your scan is given (it uses the given
   `proc_yield`), so you only write the reaping scan.

Check your work:

```sh
oslings run 21_fork_wait
# or
oslings watch
```

The harness runs a plain program (to check the scheduler), then `forktest` (one
child), then `forks2` (two children), checking the exit statuses that only
correct fork + wait + exit can produce.

Then the payoff:

```sh
cd rv6 && cargo run        # boots to the rv6$ prompt
```

Try `run forktest` and watch both "parent" and "child" appear from what began as
one program; then `run forks2`. You now have a kernel that runs a *tree* of
processes - the last big piece before a real userland shell. (Exit QEMU with
Ctrl-A then X.)

Stuck? `oslings hint`.
