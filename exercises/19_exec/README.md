# 19 · exec

> **Learn -> Understand -> Implement.** Exercise 18 ran one program, hard-coded
> onto one page. That is not how an OS works: an OS keeps a whole collection of
> programs and starts whichever one you name, with whatever arguments you type.
> This exercise builds that. You will load a program (of any size) into a fresh
> address space and hand it its command-line arguments, the way `exec` does on
> every Unix-like system.

## Learn

### What `exec` is

On Unix, starting a program is two steps: `fork` makes a copy of the current
process (that is exercise 20), and **`exec`** throws away that copy's memory and
loads a *new* program in its place. This exercise is the `exec` half: given a
program and some arguments, build the address space that program needs and point
the CPU at its first instruction.

Three things make it more than exercise 18's one-page loader:

1. **Programs live in a table, looked up by name.** `run echo` finds the program
   called `echo`. We keep a small table of programs (`hello`, `args`, `echo`,
   `big`), each a flat binary baked into the kernel image.
2. **A program can be bigger than one page.** Exercise 18 copied one page and
   was done. A real loader copies however many pages the program needs. That is
   `load_segment`, your first piece.
3. **A program receives arguments.** `run echo hello world` has to get the words
   `hello` and `world` *into* the program somehow. That is `argv`, and setting
   it up is the meat of `exec`.

### A flat binary, and where it goes

We do not have a compiler for user programs yet (that is exercise 21). So each
program is a short, hand-written, position-independent chunk of machine code,
stored in the kernel image as plain data between two labels
(`prog_echo_start` .. `prog_echo_end`). A **flat binary** just means "the raw
instruction bytes, with no header" - as opposed to a real executable file
format like **ELF**, which wraps the code in metadata describing where each part
should load. Flat binaries are the simplest thing that works, and ours are small
enough to always load at virtual address 0.

**Position-independent** means the code does not care *what* address it runs at:
it never hard-codes an address, only computes them relative to the current
program counter (the `pc`, the register holding the address of the instruction
being executed). That matters because we copy the program to virtual address 0
in the user's world, which is a different address than where it sits in the
kernel image. The `la` (load address) instruction with `.option norelax`
produces pc-relative address math, so the program works wherever it lands. You
do not have to write any of this - the programs are given - but it is worth
knowing why they look the way they do.

### `load_segment`: copy a program across as many pages as it needs

This is the general form of exercise 18's `map_user_pages`. The idea is
identical - allocate a physical page, copy code onto it, map it into the user's
page table with the user bit (`PTE_U`) - but now in a **loop**, one page at a
time, until the whole image is copied:

```
image bytes:  [======== 5000 bytes ========]
                    |                |
              page 0 (4096)     page 1 (904, rest zero-filled)
                    |                |
   mapped at VA 0x0000        mapped at VA 0x1000     (both R + X + U)
```

For each page you allocate a fresh physical page, zero it (so the unused tail of
the final page is clean), copy that slice of the image onto it, and `mappages`
it at the right virtual address. The program's `big` entry in the table is
deliberately padded past 4096 bytes so that loading it *requires* your loop to
run more than once.

### `argv`: how a program receives its arguments

When you type `run echo hello world`, the program needs two things:

- **`argc`** = the argument *count* (here 3: the program name `echo`, plus
  `hello` and `world`).
- **`argv`** = a pointer to an *array of pointers*, one per argument, each
  pointing at a NUL-terminated string. (A **NUL terminator** is a single zero
  byte marking the end of a string - that is how C-style strings know where they
  stop, since they carry no length.)

By our calling convention, a program finds these in two registers on entry:
`a0 = argc` and `a1 = argv`. (This mirrors C's `int main(int argc, char **argv)`.)

The tricky part is *building* argv. The strings and the pointer array have to
live in the **user's** memory (the program cannot read the kernel's), so we lay
them out on the user's stack, from the top downward:

```
   high addresses
      "echo\0"  "hello\0"  "world\0"      <- the argument strings
      argv[0]  argv[1]  argv[2]  NULL      <- pointers to those strings, + a NULL
   low addresses   <- the stack pointer ends here; this address IS argv
```

Every pointer stored in the argv array is a **user virtual address** - where the
string sits in the *program's* address space, not the kernel's. We write it all
into the user's memory with `copyout`, the function you wrote in exercise 18 as
the mirror of `copyin`, which sat unused until now. Read `push_argv` closely: it
is given, and it is the clearest picture you will get of what argv really is.

### Putting it together: `build_process`

`exec` allocates a process, then calls `build_process` (your second piece) to
fill it in. `build_process` is a recipe with six steps, and every step is a
function that already exists:

1. look the program up by name,
2. map the trampoline + trapframe (exercise 18's `proc_pagetable`),
3. load the image (`load_segment`),
4. give it a stack (`map_user_stack`, given),
5. push the arguments (`push_argv`, given), which hands back `(argc, argv, sp)`,
6. set the trapframe so the program starts at its first instruction, on that
   stack, with `a0 = argc` and `a1 = argv`.

Each fallible step returns a `Result`, so you chain them with the `?` operator
(exercise 10). If any step fails, `exec` frees the half-built process for you.

## Understand

Read these, in order:

1. `rv6/src/exec.rs`: the whole file. The four programs (skim the assembly - the
   comments say what each does), the `Program` table, `lookup`, then `exec` and
   the `build_process` you will write, and finally `push_argv` (read this one
   slowly - it is the argv layout made real).
2. `rv6/src/vm.rs`: find `load_segment` (your first piece) with `map_user_stack`
   right below it as a one-page model, and re-read `copyout`, which `push_argv`
   now uses.
3. `rv6/src/memlayout.rs`: the user address space picture - the stack moved to a
   fixed address above the largest possible image (`USER_STACK`), and
   `MAX_PROG_PAGES` bounds how big a program may be.
4. `rv6/src/main.rs`: the harness runs `args` (checks argc), `echo` (checks the
   argument strings arrive), and `big` (checks a two-page image loads).

Control flow of `run echo hello world`:

```
shell: exec("echo", ["hello","world"])
   -> allocproc                          a blank process
   -> build_process:                                            <- YOU
        lookup("echo")                   find the program
        proc_pagetable                   map trampoline+trapframe
        load_segment(image)              copy the program in     <- YOU
        map_user_stack                   give it a stack
        push_argv(name,args)             lay out argv -> (argc,argv,sp)
        trapframe: epc=0, sp, a0=argc, a1=argv
   -> run(p)                             drop to user mode (exercise 18)
        echo reads a0/a1, writes each argument, exit(0)
   -> "hello world"
```

## Implement

Two pieces:

1. **`vm.rs` - `load_segment`**: loop over the image one `PGSIZE` chunk at a
   time; for each chunk allocate a zeroed page, copy the chunk onto it, and
   `mappages` it at `USER_CODE + offset` with `PTE_R | PTE_X | PTE_U`. The
   `// IMPLEMENT` comment walks through it; `map_user_stack` just below is the
   single-page version to copy from.
2. **`exec.rs` - `build_process`**: the six-step recipe above, using `?` on each
   step. The `// IMPLEMENT` comment gives the exact call for every line.

Check your work:

```sh
oslings run 19_exec
# or
oslings watch
```

The harness checks each idea in turn: arguments reach the program, the argument
*strings* arrive intact, and a two-page program loads.

Then the payoff:

```sh
cd rv6 && cargo run        # boots to the rv6$ prompt
```

Try `progs` to list the programs, then:

```
run echo hello world
run args a b c             (exits with status = argc = 4)
run hello
run big
```

You are now typing a command, and the kernel is finding that program, building
it a private world, handing it your words, and running it - the same loop a real
shell runs. (Exit QEMU with Ctrl-A then X.)

Stuck? `oslings hint`.
