# 22 · userland: exec, and a shell in user mode

> **Learn -> Understand -> Implement.** You can already start a copy of a
> process (`fork`, exercise 21) and wait for it (`wait`). The last missing
> piece is `exec`: make a running process become a *different* program. With
> it, the `fork` + `exec` + `wait` trio is complete, and something remarkable
> becomes possible - the **shell itself can run in user mode**, as just another
> program with no special privileges. That is the capstone of this whole
> course: an operating system you built from nothing, booting to a user-mode
> shell that runs real programs.

## Learn

### What `exec` does

`fork` makes a *copy* of a process. But a copy of the shell is still the shell -
useless on its own. To run `ls`, the copy has to *become* `ls`. That is `exec`:

> **exec** throws away a process's current memory and loads a **different
> program** in its place, then starts that program from its first instruction.
> The process keeps its identity - same process id, same open files - but its
> code, data, and stack are all replaced.

The two calls are partners. Every Unix shell runs a command the same way:

```
    let pid = fork();          // 1. make a child (a copy of the shell)
    if pid == 0 {
        exec("ls", argv);      // 2. the child BECOMES `ls`
        // exec never returns on success: the child is `ls` now
    } else {
        wait(&status);         // 3. the shell waits for `ls` to finish
    }
```

`fork` answers "make another process"; `exec` answers "run this program in it";
`wait` answers "tell me when it is done". Together they are how *every* program
on a Unix system is launched - including the shell, which is launched by the
very first user process.

### exec does not return (on success)

This is the strangest thing about `exec`, and the key to understanding it. When
`exec` succeeds, **there is no code to return to** - the memory holding the
instruction after the `exec` call was just thrown away and replaced. So a
successful `exec` never comes back; the process simply continues as the new
program, at *its* first instruction. `exec` only returns if it *fails* (say, no
such program), and then it returns -1 so the caller - still its old self - can
react.

```
   program A calls exec("B", ...)
        |
        |  success: A's memory is replaced by B; we resume in B at its start.
        |           A's next instruction is gone; exec does not return.
        v
   program B runs   ────────────▶   (A is gone; only B remains, same pid)

   ...OR, on failure (no such program B):
        exec returns -1, and A keeps running its own code.
```

### The mechanism: swap the address space

An **address space** is everything a process can see through its page table:
its code, its stack, its arguments. In rv6 each process owns a **page table**
(the `Sv39` tree from exercise 03) that maps its virtual addresses to physical
pages. So "replace the program" really means: **build a brand-new page table
holding the new program, and swap it in for the old one.**

You have already built every piece of this:

- `load_segment` (exercise 19) copies a program image into a page table.
- `map_user_stack` gives it a fresh stack.
- `push_argv` lays the command-line arguments on that stack.
- `free_user_pagetable` (exercise 19) tears an old address space down.

So `exec` is: build a new address space with those tools, then **swap**: point
the process at the new page table, repoint its saved registers (the
**trapframe**) at the new program's first instruction and stack, and free the
old page table. That swap is the one function you write, `exec_into`.

Why is it safe to free the old memory out from under a running program? Because
a system call runs on the **kernel's** page table, not the user's. While
`exec_into` runs, the CPU is executing kernel code through the kernel page
table; the old *user* page table is just data we can free. The trampoline and
trapframe pages are shared or owned elsewhere, so tearing down the old page
table leaves them untouched.

### The shell becomes a user program

Until now, rv6's shell has lived **in the kernel** (`shell.rs`, since exercise
16). It could call filesystem functions directly because it *was* the kernel.
That is a shortcut real systems do not take: in Unix, the shell is an ordinary
**user program**, with no more power than any other. It can only ask the kernel
for things through system calls.

Now that `exec` is a system call, we can finally write the shell that way. This
exercise adds `sh`, a small user-mode shell (a hand-written program in the exec
table). It does exactly what the diagram above shows: print a `$ ` prompt, read
a line, split it into words, `fork`, have the child `exec` the command, and
`wait`. It talks to the kernel only through `read`, `write`, `fork`, `exec`,
`wait`, and `exit` - nothing else. Run it with `run sh` and you are typing at a
shell that has no special privileges at all.

(The kernel-mode shell from exercises 16-17 is still here too, so you can
compare them side by side: the kernel shell calls the filesystem directly; the
user shell can only launch *programs* that make system calls. Directory
built-ins like `cd`/`ls`/`mkdir` would return as user programs once the kernel
grows `chdir`/`mkdir`/`readdir` system calls - a natural next step.)

### One new wrinkle: a blocking read needs interrupts

The user shell's `read` **blocks** - it waits until you press a key. That key
arrives as a **device interrupt** from the UART (exercise 15). But when the CPU
enters the kernel for a system call, it turns interrupts *off*. So if we just
waited, the keypress would never be noticed and the shell would hang forever.
The fix (given, in `sys_read`): at the one system call that blocks on the
console, turn supervisor interrupts back **on** so the UART interrupt can be
delivered and wake the read. We do it there, at the blocking call, rather than
for every system call - which keeps the deeper calls like `exec` running on a
quiet, shallow kernel stack. (A kernel stack is a single 4 KiB page, so we are
careful about what we put on it.)

## Understand

Read these, in order:

1. `rv6/src/exec.rs`: the top half is the program **table**, now with four new
   entries - `sh` (the user shell) and `execself`/`exectest`/`execfail` (little
   programs that test `exec`). You do not need to trace `sh` instruction by
   instruction; read its comments to see the shape: prompt -> read a line ->
   split into words -> fork -> child execs -> parent waits. Then read
   `build_addrspace` (the reusable "build a new address space" helper),
   `build_process` (which uses it for a fresh process), and finally `exec_into`,
   the one you will write.
2. `rv6/src/syscall.rs`: the new system call number (`exec` = 7), and the given
   `sys_exec` - the thin wrapper that copies the path and the argument strings
   out of user memory (with `copyinstr`/`copyin` from earlier exercises) and
   then calls your `exec_into`. Also read the note on `ARGV_STORE` (why the argv
   scratch lives in a `static`, not on the small kernel stack) and the interrupt
   note in `sys_read`.
3. `rv6/src/usermode.rs`: unchanged, but recall how a system call flows -
   `usertrap` -> `dispatch` -> a handler, then back to user mode via
   `usertrapret`, which reads the trapframe's `epc`/`sp`. That is *why* pointing
   the trapframe at the new program is what makes `exec` "jump" there.

Control flow of `run sh`, then typing `hello`:

```
kernel shell: run sh  ──▶ exec("sh")  ──▶ the user shell starts
  sh: write "$ "                                     (a user program!)
  sh: read a line  ──▶ blocks; a keypress interrupt wakes it
  sh: you type "hello", press Enter
  sh: fork()            ──▶ a child copy of the shell
      child: exec("hello", argv)   ──▶ sys_exec ──▶ exec_into      <- YOU
             the child's memory is REPLACED by the `hello` program
             child resumes as `hello`, prints, exits
      parent (sh): wait()  ──▶ reaps the child, loops, prints "$ " again
```

## Implement

One function, in `rv6/src/exec.rs`: **`exec_into`** - the swap that replaces a
running process's address space with a new program. The `// IMPLEMENT` comment
walks through all six steps:

1. Build the new address space with the given `build_addrspace` (use `?` so a
   failure leaves the old program untouched).
2. Remember the old page table.
3. Install the new page table on the process.
4. Point the trapframe at the new program: start address (`USER_CODE`), the new
   stack pointer, and `a0` = argc / `a1` = argv.
5. Free the old page table (safe: we are on the kernel page table now).
6. Return the new argc.

Check your work:

```sh
oslings run 22_userland
# or
oslings watch
```

The harness runs three tiny programs: `execself` (exec replaces the current
image and does not return), `exectest` (fork + exec + wait, the whole shell
pattern), and `execfail` (a failed exec returns -1 and the caller lives on).

Then the payoff:

```sh
cd rv6 && cargo run        # boots to the rv6$ kernel shell
```

At the `rv6$` prompt, type `run sh` to drop into your **user-mode shell**. The
prompt becomes `$ `. Try:

```
hello
echo hi there
forktest
exit            (leaves sh, back to the rv6$ kernel shell)
```

Every one of those ran as a real user program, launched by a shell that is
itself a user program. You now have an operating system that boots to a
userland shell - the finish line of the course.

Stuck? `oslings hint`.
