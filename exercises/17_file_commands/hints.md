# Hints - 17 File commands

## Hint 1
Each command is just a few filesystem calls. The pieces you need already exist in
`fs.rs`:

- `touch`: create an empty file with `dircreate(dir, name, InodeKind::File)`.
  It is the same call `mkdir` uses, but with `InodeKind::File` instead of `Dir`.
- `cat`: `dirlookup` the name to get its `inum`, then `read` its bytes and print
  them.
- `rm`: `dirlookup` the name, make sure it is not a directory (`is_dir`), then
  `unlink` it.

The two given handlers are your templates: `cmd_echo` shows the find-or-create
and `write` pattern; `cmd_rmdir` is `rm` with an extra directory check. Read them
first.

Remember filesystem methods take bytes, so pass `name.as_bytes()`. You will need
to add `FsError` to the `use crate::fs::{...}` line for `cmd_touch`.

## Hint 2
The shapes, step by step.

`cmd_touch` (treat "already exists" as success):

```rust
if name.is_empty() {
    out.puts("touch: missing operand\n");
    return;
}
let dir = self.cwd();
let mut fsg = FS.lock();
match fsg.dircreate(dir, name.as_bytes(), InodeKind::File) {
    Ok(_) => {}
    Err(FsError::AlreadyExists) => {}   // already there: fine
    Err(_) => out.puts("touch: cannot create file\n"),
}
```

`cmd_cat` (look up, read into a buffer, print as text):

```rust
let dir = self.cwd();
let fsg = FS.lock();
let inum = match fsg.dirlookup(dir, name.as_bytes()) {
    Ok(i) => i,
    Err(_) => { out.puts("cat: no such file\n"); return; }
};
let mut buf = [0u8; fs::FILESIZE];
match fsg.read(inum, &mut buf) {
    Ok(n) => {
        if let Ok(s) = core::str::from_utf8(&buf[..n]) {
            out.puts(s);
        }
    }
    Err(_) => out.puts("cat: is a directory\n"),
}
```

`cmd_rm` (look up, refuse directories, unlink):

```rust
if name.is_empty() {
    out.puts("rm: missing operand\n");
    return;
}
let dir = self.cwd();
let mut fsg = FS.lock();
let inum = match fsg.dirlookup(dir, name.as_bytes()) {
    Ok(i) => i,
    Err(_) => { out.puts("rm: no such file\n"); return; }
};
if fsg.is_dir(inum) {
    out.puts("rm: is a directory\n");
    return;
}
let _ = fsg.unlink(dir, name.as_bytes());
```

Do not forget to add `FsError` to the imports:
`use crate::fs::{self, FsError, InodeKind, FS};`

## Hint 3
Full handlers, with the reasoning.

```rust
/// `touch NAME` - create an empty file.
fn cmd_touch(&mut self, name: &str, out: &mut dyn Out) {
    if name.is_empty() {
        out.puts("touch: missing operand\n");
        return;
    }
    let dir = self.cwd();
    let mut fsg = FS.lock();
    match fsg.dircreate(dir, name.as_bytes(), InodeKind::File) {
        Ok(_) => {}
        Err(FsError::AlreadyExists) => {} // already there: fine
        Err(_) => out.puts("touch: cannot create file\n"),
    }
}

/// `cat NAME` - print a file's contents.
fn cmd_cat(&self, name: &str, out: &mut dyn Out) {
    let dir = self.cwd();
    let fsg = FS.lock();
    let inum = match fsg.dirlookup(dir, name.as_bytes()) {
        Ok(i) => i,
        Err(_) => { out.puts("cat: no such file\n"); return; }
    };
    let mut buf = [0u8; fs::FILESIZE];
    match fsg.read(inum, &mut buf) {
        Ok(n) => {
            if let Ok(s) = core::str::from_utf8(&buf[..n]) {
                out.puts(s);
            }
        }
        Err(_) => out.puts("cat: is a directory\n"),
    }
}

/// `rm NAME` - delete a file (refuses directories).
fn cmd_rm(&mut self, name: &str, out: &mut dyn Out) {
    if name.is_empty() {
        out.puts("rm: missing operand\n");
        return;
    }
    let dir = self.cwd();
    let mut fsg = FS.lock();
    let inum = match fsg.dirlookup(dir, name.as_bytes()) {
        Ok(i) => i,
        Err(_) => { out.puts("rm: no such file\n"); return; }
    };
    if fsg.is_dir(inum) {
        out.puts("rm: is a directory\n");
        return;
    }
    let _ = fsg.unlink(dir, name.as_bytes());
}
```

Why it works: `dirlookup` turns a name into an inode number (or tells you it is
missing). `dircreate` makes a new inode and links a name to it; `unlink` does the
reverse, freeing the inode and the directory slot. `read` copies the stored bytes
into your buffer and tells you how many there were, and `from_utf8` turns those
bytes into printable text. The given `cmd_echo` writes "hello\n" into a file, so
once `cat` is done, `echo hello > f` followed by `cat f` prints `hello`.

Top of the file, the import line must include `FsError`:

```rust
use crate::fs::{self, FsError, InodeKind, FS};
```
