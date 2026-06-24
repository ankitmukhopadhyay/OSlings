# 10 · Filesystem

> **Learn → Understand → Implement.** You'll build the core of a filesystem —
> inodes and directories — with an API that returns `Result`. The Rust focus is
> error handling in `no_std`: no exceptions, just values you must check.

## Learn

A **filesystem** turns raw storage into named files and folders. This exercise
builds its heart: **inodes** and **directories**. (A real filesystem keeps these
on a disk; we don't have a disk driver yet — that's exercise 11 — so ours lives
in RAM. The structures and logic are the real ones; only the storage differs.)

### Inodes

An **inode** ("index node") is the kernel's record of one file or directory. It
holds the *metadata and contents*, but **not** the name — that lives in the
directory that points to it. Each inode in our table has:

- a **kind**: `Free` (unused slot), `File`, or `Dir`;
- a **size**;
- **contents**: file bytes (for a `File`) or directory entries (for a `Dir`).

Inodes are referred to by number — the **inode number** (`inum`), an index into
the inode table. Inode number `ROOT` is the **root directory**, created at init;
that's the top of the tree (the "/" you know from a shell).

### Directories are just inodes

Here's the elegant part: a **directory is simply an inode whose contents are a
list of entries**, each mapping a **name → inode number**. Looking up `"hello"`
in a directory means scanning its entries for that name and returning the inode
number it points to. Creating `"hello"` means allocating a new inode and adding
an entry that points to it. Because directories can contain other directories,
this builds the whole tree: `/sub/inner` is "look up `sub` in the root to get a
directory inode, then look up `inner` in *that*."

### Error handling without `std`: `Result`

Filesystem calls fail in ordinary, *expected* ways: the name isn't there, it
already exists, you tried to write into a directory. These aren't bugs — the
caller must handle them. In `std` land you might get an exception or a
`std::io::Error`; in a `no_std` kernel there's none of that. Rust's answer is the
**`Result<T, E>`** type: a value that is either `Ok(T)` (success, carrying the
result) or `Err(E)` (failure, carrying an error). The caller *cannot ignore it* —
they have to look inside to get the value, which is exactly what makes kernel
error handling robust.

We define our own error type, an `enum`:

```rust
pub enum FsError { NotFound, AlreadyExists, NotADirectory, IsADirectory, /* ... */ }
```

and every operation returns `Result<…, FsError>`. Three idioms you'll use:

- **Return an error**: `return Err(FsError::AlreadyExists);`
- **Match on a result** to react to specific errors:
  ```rust
  match self.dirlookup(dir, name) {
      Ok(_) => return Err(FsError::AlreadyExists), // it exists → creating is an error
      Err(FsError::NotFound) => {}                 // good — go ahead and create
      Err(e) => return Err(e),                     // some other failure → pass it up
  }
  ```
- **The `?` operator** — "do this; if it's an `Err`, return that error from *me*
  immediately; otherwise give me the `Ok` value":
  ```rust
  let inum = self.alloc(kind)?;   // if alloc fails, dircreate returns its error
  ```
  `?` is the clean way to chain fallible steps. `Option` has a sibling helper,
  `.ok_or(err)`, which turns `Some(x)`/`None` into `Ok(x)`/`Err(err)` so you can
  `?` it too.

### Shared safely

The whole filesystem lives behind a `SpinLock` (your exercise 07 lock):
`static FS: SpinLock<FileSystem>`. Locking it hands you `&mut FileSystem`, so the
operations are ordinary *safe* Rust — the interesting part is the `Result` logic,
not pointers.

## Understand

Read `rv6/src/fs.rs`: the `InodeKind` and `FsError` enums, the `Inode`/`DirEnt`
structs, the given `new`/`init`/`alloc`/`read`, and the `FS` lock. Then read
`rv6/src/main.rs`: it locks `FS`, creates `/hello`, looks it up, checks a missing
name gives `NotFound`, checks a duplicate gives `AlreadyExists`, writes and reads
the file, nests `/sub/inner`, and checks that writing to a directory gives
`IsADirectory`.

## Implement

In `rv6/src/fs.rs`:

1. **`dirlookup`** — ensure `dir` is a directory (`NotADirectory` if not), scan
   its entries for a matching name, return its `inum`, or `NotFound`.
2. **`dircreate`** — reject names that are too long; reject duplicates
   (`AlreadyExists`); find a free entry slot (`DirFull` if none); allocate the
   inode with `?`; fill in the entry; return the new `inum`.
3. **`write`** — `NotFound` if the inode is free, `IsADirectory` if it's a
   directory, `FileTooBig` if the data won't fit; otherwise copy the data, set
   the size, and return the byte count.

Check your work:

```sh
oslings run 10_filesystem
# or
oslings watch
```

Passes when create/lookup/read/write and all the error cases behave — printing
`OSLINGS:PASS`. Each failure message names the property that broke.

Stuck? `oslings hint`.
