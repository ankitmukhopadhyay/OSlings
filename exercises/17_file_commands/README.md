# 17 · File commands

> **Learn -> Understand -> Implement.** Exercise 16 gave you a shell that moves
> around directories (`pwd`, `ls`, `cd`, `mkdir`). Now you will teach it to work
> with **files**: create them (`touch`), read them (`cat`), and delete them
> (`rm`). After this, rv6 has a genuinely usable little shell and filesystem.

## Learn

A directory command like `mkdir` only makes folders. The interesting part of any
filesystem is the **files** inside those folders: making them, putting bytes in,
reading the bytes back, and removing them. That is the whole life cycle of a
file, and each step is one call into the filesystem you built back in exercise
10.

### The filesystem already gives you everything you need

You are not writing filesystem internals here. The filesystem (`fs.rs`) exposes
a small set of methods, and each new command is just a few of them stitched
together. Here are the ones the file commands use:

| Method | What it does |
|---|---|
| `dirlookup(dir, name)` | find an entry by name in directory `dir`; returns `Ok(inum)` (the file's inode number) or `Err` if there is no such name |
| `dircreate(dir, name, kind)` | create a new entry of a given `kind` (`File` or `Dir`); returns `Ok(inum)` or an `Err` (for example `AlreadyExists`) |
| `read(inum, buf)` | copy a file's bytes into `buf`; returns `Ok(n)` = how many bytes were read |
| `write(inum, data)` | replace a file's contents with `data`; returns `Ok(n)` |
| `unlink(dir, name)` | remove the entry `name` from `dir` and free its inode (the inverse of `dircreate`) |
| `is_dir(inum)` | `true` if that inode is a directory |
| `dir_is_empty(inum)` | `true` if a directory has no entries |

The last three (`unlink`, `is_dir`, `dir_is_empty`) are new in this exercise;
read them in `fs.rs`. They are short.

A few terms, in case they are new:

- **inode**: the filesystem's record for one file or directory (its kind, its
  size, its bytes). Every file has exactly one inode.
- **inode number (`inum`)**: a small integer that names an inode, the way a seat
  number names a seat. `dirlookup` turns a *name* into an `inum`; the other
  methods take an `inum`.
- **directory entry**: the (name -> inum) pair stored inside a directory. `ls`
  walks these; `unlink` removes one.

### How a command becomes filesystem calls

Each command is a tiny recipe:

- **`touch name`**: "make an empty file." That is one call:
  `dircreate(dir, name, InodeKind::File)`. (Compare `mkdir`, which you read in
  exercise 16: it is the same call with `InodeKind::Dir`.)
- **`cat name`**: "show me the bytes." Look the name up with `dirlookup` to get
  its `inum`, then `read` the bytes into a buffer and print them.
- **`rm name`**: "delete this file." Look it up, make sure it is not a directory,
  then `unlink` it.

### Reading bytes and turning them into text

`read` fills a plain byte buffer. You make a buffer the size of one file and ask
`read` how many bytes it actually wrote:

```rust
let mut buf = [0u8; fs::FILESIZE];   // FILESIZE is the max bytes a file holds
let n = fsg.read(inum, &mut buf)?;   // n = bytes actually read
```

Bytes are not automatically text. To print them you convert the first `n` bytes
to a string slice with `core::str::from_utf8`, which returns a `Result` (it fails
only if the bytes are not valid UTF-8, so you can safely ignore that case):

```rust
if let Ok(s) = core::str::from_utf8(&buf[..n]) {
    out.puts(s);
}
```

### Handling "it might not be there": `Result` and `match`

Almost every filesystem call returns a `Result`: `Ok(value)` on success or
`Err(reason)` on failure (no such file, already exists, and so on). A command is
mostly deciding what to do with each `Err`. The pattern you will use over and
over is `match`:

```rust
let inum = match fsg.dirlookup(dir, name.as_bytes()) {
    Ok(i) => i,
    Err(_) => {
        out.puts("cat: no such file\n");
        return;                 // give up early on a missing file
    }
};
// ... from here on, `inum` is a real file
```

This "match, handle the error, return early" shape is exactly how the given
commands (`cmd_cd`, `cmd_echo`, `cmd_rmdir`) are written, so you have models to
copy.

### Two commands are given as worked examples

So that you can focus on the core idea (turning a command into filesystem
calls), two of the trickier commands are written for you to read:

- **`cmd_echo`** handles `echo text > file`. It shows the **write** path:
  find-or-create the file, then `write` the text into it. It also does a little
  parsing to split the line at the `>` redirect. Your `cat` reads back exactly
  what `echo` writes.
- **`cmd_rmdir`** removes an *empty directory*. It is the same shape as the `rm`
  you will write, but it checks the target is a directory and that
  `dir_is_empty` before calling `unlink`. Read it side by side with your `rm`.

### Why `name.as_bytes()`?

Filenames in the filesystem are stored as raw bytes, not Rust strings, so the
filesystem methods take `&[u8]`. A `&str` becomes `&[u8]` with `.as_bytes()`.
You will see this on every `dirlookup`/`dircreate`/`unlink` call.

## Understand

Read these, in order:

1. `rv6/src/fs.rs`: skim the methods in the table above. The new ones are
   `unlink` and `dir_is_empty`, near the bottom. Notice `unlink` frees the inode
   (`Inode::new()` resets it to `Free`) and clears the directory slot.
2. `rv6/src/shell.rs`: the `exec` dispatch already routes `touch`/`cat`/`rm`/
   `rmdir`/`echo` to handlers. Read the two given handlers `cmd_echo` and
   `cmd_rmdir` closely; they are your templates. Then find the three handlers you
   will fill in: `cmd_touch`, `cmd_cat`, `cmd_rm`.
3. `rv6/src/main.rs`: the harness drives the shell with a script
   (`touch`, then `echo`+`cat`, then `rm`, then `rmdir`) and checks the output,
   so you can see exactly what each command is expected to do.

Control flow when you type a line:

```
you type "cat notes.txt"  ->  run() reads the line
                          ->  Shell::exec splits it: cmd="cat", arg="notes.txt"
                          ->  match dispatches to self.cmd_cat("notes.txt", out)
                          ->  cmd_cat: dirlookup -> read -> from_utf8 -> out.puts
```

## Implement

In `rv6/src/shell.rs`, fill in three handlers. Each has step-by-step guidance in
its `// IMPLEMENT` comment.

1. **`cmd_touch`**: create an empty `File` with `dircreate`. Treat
   "already exists" as success (a no-op), like real `touch`.
2. **`cmd_cat`**: `dirlookup` the name, `read` the bytes into a
   `[0u8; fs::FILESIZE]` buffer, convert with `core::str::from_utf8`, and print.
3. **`cmd_rm`**: `dirlookup` the name, refuse it if `is_dir`, then `unlink` it.

You will also need to add `FsError` to the `use crate::fs::{...}` line at the top
(the `cmd_touch` example matches on `FsError::AlreadyExists`).

Check your work:

```sh
oslings run 17_file_commands
# or
oslings watch
```

It passes when `touch`, `cat`, and `rm` all behave (the given `echo` and `rmdir`
are part of the same checks).

Then use it for real, the payoff:

```sh
cd rv6 && cargo run        # boots to a rv6$ prompt
```

Try: `touch notes.txt`, `ls`, `echo hello world > notes.txt`, `cat notes.txt`,
`rm notes.txt`, `cat notes.txt` (now missing), `mkdir d`, `rmdir d`.
(Exit QEMU with Ctrl-A then X.)

Stuck? `oslings hint`.
