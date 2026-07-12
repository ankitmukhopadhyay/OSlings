# Hints - 20 File descriptors

## Hint 1
Three pieces, all in `syscall.rs`, and each has a model nearby.

- `fdalloc` is a loop over the current process's file table looking for an
  empty slot. The current process is `usermode::curproc()` (a `*mut Proc`); its
  table is `(*p).ofile`, an array of `NOFILE` `File`s; a free slot has
  `kind == FileKind::None`.
- `sys_open` is the longest, but every step is spelled out in its `// IMPLEMENT`
  comment. It fetches the filename with the given `vm::copyinstr`, finds (or
  creates) the inode with the filesystem calls you already know from exercise
  17 (`dirlookup`, `dircreate`), builds a `File`, and calls your `fdalloc`.
- `sys_read` is the mirror of the given `sys_write` right below it - read that
  first. The one idea to get right is the **offset**: after reading `n` bytes,
  do `(*p).ofile[fd].off += n`. Forget it and `cat` loops forever (the test
  reports a timeout and tells you why).

Everything is `unsafe` because it dereferences the `*mut Proc` - wrap each body
in an `unsafe { ... }` block, like the given handlers do.

## Hint 2
The shapes.

`fdalloc` - find a free slot, fill it, return its index:

```rust
fn fdalloc(file: File) -> isize {
    unsafe {
        let p = usermode::curproc();
        for fd in 0..NOFILE {
            if (*p).ofile[fd].kind == FileKind::None {
                (*p).ofile[fd] = file;
                return fd as isize;
            }
        }
        -1
    }
}
```

`sys_read` - look up the file, read at the offset, copy out, advance:

```rust
fn sys_read(fd: usize, buf: usize, len: usize) -> isize {
    unsafe {
        let p = usermode::curproc();
        let file = match getfile(p, fd) {
            Some(f) if f.readable => f,
            _ => return -1,
        };
        match file.kind {
            FileKind::Console => {
                let b = [crate::console::getc()];
                if vm::copyout((*p).pagetable, buf, &b).is_err() { return -1; }
                1
            }
            FileKind::Inode => {
                let mut kbuf = [0u8; 128];
                let want = core::cmp::min(len, kbuf.len());
                let n = match FS.lock().read_at(file.inum, file.off, &mut kbuf[..want]) {
                    Ok(n) => n,
                    Err(_) => return -1,
                };
                if n > 0 && vm::copyout((*p).pagetable, buf, &kbuf[..n]).is_err() { return -1; }
                (*p).ofile[fd].off += n;   // <-- advance the cursor!
                n as isize
            }
            FileKind::None => -1,
        }
    }
}
```

For `sys_open`, follow the five numbered steps in its `// IMPLEMENT` comment
verbatim - every line is given there. The two access-mode lines are the only
bit of cleverness:

```rust
let writable = flags & O_WRONLY != 0 || flags & O_RDWR != 0;
let readable = flags & O_WRONLY == 0; // RDONLY (0) and RDWR both readable
```

## Hint 3
Full `sys_open`, with the reasoning.

```rust
fn sys_open(path: usize, flags: usize) -> isize {
    unsafe {
        let p = usermode::curproc();

        // 1. the path is a NUL-terminated STRING in user memory; copyinstr
        //    copies it out and tells us its length.
        let mut namebuf = [0u8; 32];
        let len = match vm::copyinstr((*p).pagetable, &mut namebuf, path) {
            Ok(n) => n,
            Err(_) => return -1,
        };
        let name = &namebuf[..len];

        // 2. access mode from the flags.
        let writable = flags & O_WRONLY != 0 || flags & O_RDWR != 0;
        let readable = flags & O_WRONLY == 0;

        // 3. find the inode, creating it if O_CREATE was asked for. Everything
        //    lives in the root directory.
        let mut fsg = FS.lock();
        let inum = if flags & O_CREATE != 0 {
            match fsg.dircreate(ROOT, name, InodeKind::File) {
                Ok(i) => i,
                Err(_) => match fsg.dirlookup(ROOT, name) {
                    Ok(i) => i,          // it already existed: just open it
                    Err(_) => return -1,
                },
            }
        } else {
            match fsg.dirlookup(ROOT, name) {
                Ok(i) => i,
                Err(_) => return -1,     // no such file
            }
        };

        // 4. O_TRUNC empties an existing file; then release the fs lock.
        if flags & O_TRUNC != 0 {
            let _ = fsg.truncate(inum);
        }
        drop(fsg);

        // 5. build the open file and hand it to fdalloc, which returns the fd.
        let file = File { kind: FileKind::Inode, inum, off: 0, readable, writable };
        fdalloc(file)
    }
}
```

Why it is shaped this way: `open` is where a *name* becomes a *descriptor*.
The name lookup is pure exercise-17 filesystem work (`dirlookup`/`dircreate`);
the new part is wrapping the result in a `File` - with `off: 0` so reading
starts at the beginning - and registering it in the process's table via
`fdalloc`. The `O_CREATE` branch tries to create first and falls back to
looking up, so opening a file that already exists still works. And returning
-1 on every failure (bad path, missing file, full table) means a user program
can never crash the kernel by opening something silly; it just gets an error
back, the way real `open` does.
