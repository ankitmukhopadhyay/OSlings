# 20 · File descriptors

> **Learn -> Understand -> Implement.** In exercise 18 a user program could
> only `write` to the console. In exercise 19 it could take arguments. But it
> still could not touch a file. This exercise gives user programs the last
> piece: they can `open`, `read`, `write`, and `close` files, all through small
> integer handles called **file descriptors**. After this, a user-mode `cat`
> reads a real file through system calls, exactly the way `cat` does on a real
> Unix system.

## Learn

### What a file descriptor is

When a program wants to work with a file, it does not carry the file around.
Instead it asks the kernel to `open` the file, and the kernel hands back a
small integer: a **file descriptor** (fd). From then on the program says
"read from fd 3" or "write to fd 3", and the kernel looks up what fd 3 refers
to. `close` gives the descriptor back.

Why a plain integer? Because it is the simplest possible handle: cheap to pass
in a register, cheap for the kernel to look up (it is just an array index), and
it hides *what* is behind it. That last point is the powerful one. Behind an fd
can be a file on disk, the console, or (later) a pipe or a network socket - and
the program reads and writes all of them the same way. "Everything is a file"
is one of Unix's oldest and best ideas, and the file descriptor is what makes
it work.

Every process starts with three descriptors already open, by convention:

| fd | name | usually |
|---|---|---|
| 0 | standard input (stdin) | the keyboard |
| 1 | standard output (stdout) | the screen |
| 2 | standard error (stderr) | the screen |

That is why `write(1, ...)` printed to the console in exercises 18 and 19: fd 1
was the console all along. In `allocproc` (proc.rs), rv6 now sets those three up
for every process.

### The per-process file table

Each process owns an array of open files, `ofile` in `Proc` (proc.rs). The fd
*is* the index into that array: fd 3 is `ofile[3]`. Each entry is a `File`
(file.rs):

```rust
pub struct File {
    pub kind: FileKind,   // None (free), Console, or Inode
    pub inum: usize,      // which inode, when kind == Inode
    pub off: usize,       // the read/write cursor  <-- the interesting part
    pub readable: bool,
    pub writable: bool,
}
```

Because the table is per-process, fd 3 in one program has nothing to do with
fd 3 in another - descriptors are private, like the address space.

### The offset: what makes a descriptor stateful

Back in exercise 17 the shell's `cat` read a whole file in one call:
`read(inum, buf)` always started at the beginning. A file descriptor is
different: it **remembers where you are**. Each `File` carries an offset `off`,
the byte position the next read or write starts at. Every `read` returns the
next chunk and then advances `off` by however many bytes it returned:

```
file:  [ h e l l o   f i l e s . . . ]
         ^
read(fd, buf, 64)  -> returns "hello files...", off jumps to 64
read(fd, buf, 64)  -> returns the NEXT 64 bytes, off jumps to 128
read(fd, buf, 64)  -> returns 0  (nothing left: end of file)
```

That returned `0` is how a reader knows it has hit the end. A program like
`cat` just loops "read a chunk, write it out" until read returns 0. **If you
forget to advance the offset, every read returns the same first chunk forever**
and the loop never ends - a mistake worth remembering, because the test catches
it as a timeout.

### open, and its flags

`open(path, flags)` finds a file by name and returns a fresh descriptor for it.
`flags` is a bitmask (file.rs) - you combine the bits with `|`:

| Flag | Value | Meaning |
|---|---|---|
| `O_RDONLY` | 0x000 | open for reading (the default) |
| `O_WRONLY` | 0x001 | open for writing |
| `O_RDWR`   | 0x002 | open for reading and writing |
| `O_CREATE` | 0x200 | create the file if it does not exist |
| `O_TRUNC`  | 0x400 | empty the file when opening it |

The low bits pick the access mode; the high bits are extras. So
`open("out", O_CREATE | O_WRONLY)` means "make out if needed, open it for
writing." rv6 resolves every path in the **root directory** (a per-process
current directory comes later, with the userland shell).

### Reading a user pointer that is a *string*

`open`'s first argument is a pointer into the user's memory to the filename -
and it is a NUL-terminated string, whose length the kernel does not know in
advance. So instead of `copyin` (which needs a length), open uses the given
`vm::copyinstr`, which copies bytes out of user memory until it hits the zero
byte that ends the string. It is `copyin` with an early stop; you do not write
it, just call it.

### The uniform read/write path

The neat part of the whole design: `read` and `write` do not care whether the
fd is the console or a file. They look at `file.kind` and branch - console
bytes go to/from the UART, inode bytes go to/from the filesystem - but the
program calling `read(fd, ...)` is identical either way. You will see this in
`sys_write` (given) and build the matching `sys_read`.

## Understand

Read these, in order:

1. `rv6/src/file.rs`: the `File` struct, `FileKind`, `NOFILE`, and the `open`
   flags. Short - read all of it.
2. `rv6/src/proc.rs`: `Proc` now has an `ofile` table; `allocproc` opens fds
   0/1/2 on the console, `freeproc` closes everything.
3. `rv6/src/fs.rs`: the new offset-based primitives near the bottom -
   `read_at`, `write_at`, `truncate`, `size`. These are what the fd layer sits
   on.
4. `rv6/src/syscall.rs`: the new numbers, `dispatch`, the given `getfile`
   helper, the given `sys_write` (your model for read), and `sys_close`. Then
   the three you will fill in: `fdalloc`, `sys_open`, `sys_read`.
5. `rv6/src/exec.rs`: two new user programs, `cat` and `create`, written in
   assembly - see how a real program calls open/read/write/close.

Control flow of `run cat notes.txt`:

```
cat: open("notes.txt", O_RDONLY)
   -> sys_open: copyinstr the name, dirlookup, fdalloc -> fd     <- YOU
cat: loop { read(fd, buf, 64); write(1, buf, n) } until read == 0
   -> sys_read: read_at(off) -> copyout -> off += n -> return n  <- YOU
   -> sys_write: copyin -> emit to console (given)
cat: close(fd); exit(0)
```

## Implement

Three pieces, all in `rv6/src/syscall.rs`:

1. **`fdalloc`**: scan the current process's `ofile` table for a free slot
   (`FileKind::None`), store the given `File` there, and return the slot index
   as the fd. Return -1 if the table is full.
2. **`sys_open`**: fetch the path with `vm::copyinstr`, work out readable/
   writable from the flags, find the inode (creating it if `O_CREATE`),
   truncate if `O_TRUNC`, build the `File`, and hand it to `fdalloc`. The
   `// IMPLEMENT` comment lays out all five steps.
3. **`sys_read`**: the mirror of the given **`sys_write`** (read that first).
   Look up the file, then for an inode `read_at` the current offset, `copyout`
   the bytes to the user, and **advance the offset**. The `// IMPLEMENT`
   comment has it step by step.

Check your work:

```sh
oslings run 20_file_descriptors
# or
oslings watch
```

The harness seeds a file, then runs the `cat` and `create` programs and checks
what they read and wrote - one idea per step (read a file, create+write a file,
open a missing file).

Then the payoff:

```sh
cd rv6 && cargo run        # boots to the rv6$ prompt
```

Try:

```
run create notes.txt        (a user program creates and writes the file)
run cat notes.txt           (a user program reads it back, through syscalls)
```

Both are unprivileged user programs doing real file I/O through the system-call
wall - not the kernel touching the filesystem directly. (The shell still has
its own built-in `cat` from exercise 17 for comparison; `run cat` launches the
user-mode one.) Exit QEMU with Ctrl-A then X.

Stuck? `oslings hint`.
