# 18 · User mode

> **Learn -> Understand -> Implement.** Phase 2B begins. Until now, every line
> of code you wrote ran *inside the kernel*, with total power over the machine.
> In this exercise rv6 runs its first real **user program**: code that runs at
> the CPU's lowest privilege level, inside its own private address space, and
> that can only talk to the kernel through **system calls**. This wall between
> user programs and the kernel is the defining feature of every real OS.

## Learn

### Why build a wall at all?

Everything in Part 1 and 2A trusted itself completely: the shell, the
filesystem, the drivers are all kernel code, and kernel code can read any
memory, write any register, and halt the machine. That is fine while *you*
write all the code. But an operating system exists to run *other people's
programs*, and those programs crash, scribble on memory, and loop forever.
The OS must be able to say: "run this program, but if it misbehaves, only the
program dies, not the machine."

Two hardware features make that promise enforceable, and you have already met
both halves:

- **privilege levels** (exercise 13): some instructions only work above a
  certain level.
- **the MMU and page tables** (exercises 03 and 09): every memory access goes
  through a translation the *kernel* controls.

This exercise snaps them together.

### The third privilege level: user mode

In exercise 13 the kernel dropped from machine mode (M) to supervisor mode (S)
with an `mret`. RISC-V has one level below that: **user mode (U)**, the
weakest of the three.

| level | who runs there | what it may do |
|---|---|---|
| M (machine) | `start.rs`, `timervec` | everything |
| S (supervisor) | the rv6 kernel | use CSRs, manage page tables, take traps |
| U (user) | user programs | ordinary computation only |

Code in user mode cannot read or write any CSR, cannot change `satp`, cannot
turn interrupts off - it cannot even *ask* what mode it is in. If it tries,
the CPU traps to the kernel. The drop into U-mode works exactly like
exercise 13's drop into S-mode, one level down: the kernel clears `sstatus.SPP`
(bit 8, "where did the last trap come from / where will `sret` go") and
executes `sret`.

There are only two ways back up from user mode, and both are traps into the
kernel: the program *asks* for something (an `ecall`), or the hardware forces
the issue (an interrupt, or a fault when the program does something illegal).

### A private world: the user address space

Privilege alone is not enough - a user program could still *read* the
kernel's memory. So each process gets its **own page table** (you have been
allocating one per process since exercise 04; this is the exercise where it
finally gets used). When the program runs, `satp` points at *its* table, so
the only memory that exists, from its point of view, is what the kernel chose
to map:

```
virtual address       what lives there              who may touch it
---------------       --------------------------    ----------------
0x3F_FFFF_F000        TRAMPOLINE (uservec/userret)   kernel only (R X)
0x3F_FFFF_E000        TRAPFRAME (saved registers)    kernel only (R W)
     ...                    (unmapped)
0x0000_2000           <- initial stack pointer
0x0000_1000           the stack page                 user (R W U)
0x0000_0000           the program's code             user (R X U)
```

Two things to notice:

- **`PTE_U` is the wall.** A page table entry you build with `PTE_U` set can
  be touched from user mode; one without it cannot (the CPU faults). That
  single bit is what makes the kernel's parts of this table invisible to the
  program. It works in both directions, by the way: the kernel is *also* not
  allowed to casually touch `PTE_U` pages, which is why `copyin` (below)
  translates addresses explicitly.
- **The program's code sits at virtual address 0.** In a fresh, private
  address space, 0 is an ordinary address, and it is where xv6 loads programs
  too. (Hosted programs leave address 0 unmapped so stray null pointers
  crash - a luxury we will meet again in exercise 19.)

`MAXVA` is one past the highest usable Sv39 address. Sv39 has 39 address
bits, but we stop at `1 << 38` so no address ever has its top bit set (such
addresses must be sign-extended, a classic source of bugs).

### The trapframe: a parking lot for 31 registers

When the program executes `ecall`, the CPU jumps into the kernel - but every
single register still holds the *user program's* values. The kernel cannot run
one line of Rust without clobbering them, and it must hand every one of them
back, exactly as they were, when the program resumes.

So each process owns a **trapframe**: one page where, on every trap, all 31
general-purpose registers are parked (and reloaded on the way back out). It
also carries a few notes the kernel leaves for its own trap path: the kernel's
`satp`, the process's kernel stack pointer, the address of `usertrap`, and
`epc` - the user program counter to resume at.

(Why does a process also get its own *kernel stack* - the `kstack` field?
Because when the program traps in, the kernel code that handles the trap needs
a stack, and it cannot trust the user's stack pointer: the program controls
it, and it might point anywhere.)

### The trampoline: the cleverest page in the kernel

Here is the puzzle at the heart of this exercise. Entering or leaving user
mode means changing `satp` - switching page tables - *while the CPU is
executing instructions*. The instant `satp` changes, every address means
something new, **including the address the next instruction will be fetched
from**. Switch tables while standing on a page the new table maps somewhere
else (or not at all), and the CPU is suddenly executing garbage.

The escape: a single page of assembly, the **trampoline**, mapped at the SAME
virtual address (`TRAMPOLINE`, the very top page) in the kernel's page table
and in every user page table. Code standing on that page can flip `satp`
between those tables and nothing moves under its feet. It has two halves:

- **`uservec`** - traps from user mode land here (`stvec` points here while
  user code runs). It parks all the registers in the trapframe, switches to
  the kernel page table and kernel stack, and jumps to `usertrap` (Rust).
- **`userret`** - the road back. It switches to the user page table, reloads
  every register from the trapframe, and `sret`s back into the program.

One page has to be *somewhere*, so rv6 copies the trampoline's instructions
onto their own fresh page at boot (`kvmmake`) and maps that page at
`TRAMPOLINE` in every table it builds. Note the trampoline is mapped
*without* `PTE_U`: it lives inside the user's address space, but the user
cannot touch it - only a trap can land there.

### System calls: a function call across the wall

A user program cannot print, because printing means touching the UART, and
the UART is not in its address space. It has to *ask the kernel*. The asking
convention (the "system call ABI") is a function call across the privilege
wall:

| register | role |
|---|---|
| a7 | WHICH call (the system call number) |
| a0, a1, a2 | the arguments |
| a0 (after) | the return value |

The program loads those registers and executes `ecall`, which traps into the
kernel with `scause = 8` ("environment call from U-mode"). rv6 starts with
three calls, using xv6's numbers so the table can grow in later exercises
without renumbering:

| number | call | meaning |
|---|---|---|
| 2 | exit(status) | the program is finished |
| 11 | getpid() | which process am I? |
| 16 | write(fd, buf, len) | print len bytes from buf (fd 1 = console) |

One sharp edge you will handle in `usertrap`: on an `ecall`, `sepc` points
**at** the `ecall` instruction itself, not after it. Resume there and the
program makes the same system call forever. The fix is one line - add 4 (the
instruction's size in bytes) to the saved `epc` - and forgetting it is a rite
of passage.

### copyin: a user pointer is not a kernel pointer

Look at `write(1, buf, len)`. That `buf` is a virtual address *in the user's
world*. The kernel runs on its own page table, where that number means
something entirely different (at `buf = 0x28`, it is somewhere inside the
kernel image!). And the user's pages can be scattered anywhere in physical
memory, one page at a time.

So the kernel must do the translation by hand: for each page the buffer
touches, look the address up in the *user's* page table (`walkaddr`, which
also refuses anything that is not a `PTE_U` page - a program that passes a
kernel address gets an error, not a data leak), then copy what lives there.
That is `copyin`, one of your four pieces. Its mirror image `copyout` (kernel
memory INTO user memory) is given, right above it - read them side by side.
Exercise 20's `read()` will need `copyout`; nothing uses it yet.

### The first user program

There is no compiler or loader for user programs yet - building one is
exercise 19. So rv6's first user program is a dozen lines of assembly, baked
into the kernel image as *data* and copied onto the code page at setup time:

```
write(1, "hello from user mode\n", 21)     # syscall 16
pid = getpid()                             # syscall 11
exit(pid + 41)                             # syscall 2
```

Why `pid + 41`? It proves the whole round trip: the kernel's return value
(`getpid`) had to travel back through the wall into the program's `a0`, and
the program's argument (`pid + 41`) had to travel into the kernel again. The
first process gets pid 1, so a healthy run exits with status 42. Run it twice
from the shell and the status climbs with the pid.

## Understand

Read these, in order:

1. `rv6/src/memlayout.rs`: the new constants (`MAXVA`, `TRAMPOLINE`,
   `TRAPFRAME`, `USER_CODE`, `USER_STACK`) and the address-space picture.
2. `rv6/src/usermode.rs`: top to bottom - the `Trapframe` struct (the field
   offsets match the assembly, do not reorder), the trampoline assembly
   (`uservec`/`userret`, heavily commented), the user program, then
   `setup`/`run`/`finish` (how the kernel launches a program and gets control
   back - it is exercise 05's `swtch`, reused), `usertrap` (your ecall
   branch), and `usertrapret`.
3. `rv6/src/proc.rs`: `Proc` grew `trapframe` and `kstack`;
   `proc_pagetable` maps the trampoline + trapframe (no `PTE_U`).
4. `rv6/src/vm.rs`: `kvmmake` now installs the trampoline; then `walkaddr`,
   `copyout` (your model), and the homes of two of your pieces.
5. `rv6/src/syscall.rs`: the numbers and the three handlers; `sys_write` is
   where your `copyin` gets used.
6. `rv6/src/main.rs`: the self-check verifies your page-table work BEFORE
   running the program (exercise 09's verify-before-use trick), then runs it
   and checks what came out.

Control flow of one system call, the full round trip:

```
user code:  ecall
   -> CPU: trap! mode = S, pc = stvec = TRAMPOLINE (uservec)
   -> uservec:  park 31 registers in TRAPFRAME, switch to kernel
                page table + kernel stack, jump to usertrap
   -> usertrap: scause == 8, so: epc += 4, dispatch(a7, a0, a1, a2),
                return value into trapframe a0        <- YOU
   -> dispatch: match the number, call the handler    <- YOU
   -> usertrapret: aim stvec back at uservec, sstatus.SPP = User,
                sepc = epc, jump to userret
   -> userret: switch to user page table, reload the 31 registers,
                sret
user code:  ...continues after the ecall, result in a0
```

## Implement

Four pieces, in the order the self-check meets them:

1. **`vm.rs` - `map_user_pages`**: two `mappages` calls - the code page
   (`USER_CODE`, `PTE_R | PTE_X | PTE_U`) and the stack page (`USER_STACK`,
   `PTE_R | PTE_W | PTE_U`). Without `PTE_U` the program cannot even fetch
   its first instruction.
2. **`usermode.rs` - `usertrap`, the `scause == 8` branch**: step over the
   `ecall` (`epc += 4`), pull a7 and a0..a2 out of the trapframe and hand
   them to `crate::syscall::dispatch`, put the return value back in the
   trapframe's a0.
3. **`syscall.rs` - `dispatch`**: a `match` on the number, routing to
   `sys_exit` / `sys_getpid` / `sys_write`. Unknown numbers return -1 (a
   user program must never be able to crash the kernel with a bad number).
4. **`vm.rs` - `copyin`**: the page-by-page copy OUT of the user's address
   space, mirroring the given `copyout`. (You will advance `srcva`, so make
   the parameter `mut srcva: usize`, just like `copyout`'s `mut dstva`.)

Check your work:

```sh
oslings run 18_user_mode
# or
oslings watch
```

The self-check tells you which piece it got stuck on, in the same order.

Then the payoff:

```sh
cd rv6 && cargo run        # boots to the rv6$ prompt
```

Type `run`. That is your kernel launching a user program, the program
printing through a system call, and the shell getting the CPU back when it
exits. Run it a few times and watch the pid (and the exit status) climb. The
file commands from exercise 17 all still work alongside it.
(Exit QEMU with Ctrl-A then X.)

Stuck? `oslings hint`.
