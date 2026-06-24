# Hints — 10 Filesystem

## Hint 1
All three functions are safe Rust over `self.inodes` (an array). The new idea is
*returning errors as values*: each returns `Result<usize, FsError>`, so success
is `Ok(x)` and every failure is `Err(FsError::Something)`.

- `dirlookup`: check the kind, then scan `self.inodes[dir].entries`.
- `dircreate`: a few checks, then `alloc` an inode and record an entry.
- `write`: check the kind and size, then copy bytes in.

If the test says "creating /hello failed", `dircreate` is still returning its
placeholder `Err`.

## Hint 2
`dirlookup`:

```rust
if self.inodes[dir].kind != InodeKind::Dir {
    return Err(FsError::NotADirectory);
}
for e in &self.inodes[dir].entries {
    if e.used && e.len == name.len() && &e.name[..e.len] == name {
        return Ok(e.inum);
    }
}
Err(FsError::NotFound)
```

`write`:

```rust
match self.inodes[inum].kind {
    InodeKind::Free => return Err(FsError::NotFound),
    InodeKind::Dir => return Err(FsError::IsADirectory),
    InodeKind::File => {}
}
if data.len() > FILESIZE {
    return Err(FsError::FileTooBig);
}
self.inodes[inum].data[..data.len()].copy_from_slice(data);
self.inodes[inum].size = data.len();
Ok(data.len())
```

For `dircreate`, the shape is: length check → duplicate check (match on
`dirlookup`) → find a free slot → `let inum = self.alloc(kind)?;` → fill the
entry.

## Hint 3
Full `dircreate`:

```rust
pub fn dircreate(&mut self, dir: usize, name: &[u8], kind: InodeKind) -> Result<usize, FsError> {
    if name.len() > NAMELEN {
        return Err(FsError::NameTooLong);
    }
    match self.dirlookup(dir, name) {
        Ok(_) => return Err(FsError::AlreadyExists),
        Err(FsError::NotFound) => {}
        Err(e) => return Err(e),
    }
    let mut slot = None;
    for i in 0..NDIRENT {
        if !self.inodes[dir].entries[i].used {
            slot = Some(i);
            break;
        }
    }
    let slot = slot.ok_or(FsError::DirFull)?;
    let inum = self.alloc(kind)?;
    let mut nm = [0u8; NAMELEN];
    nm[..name.len()].copy_from_slice(name);
    self.inodes[dir].entries[slot] = DirEnt {
        name: nm,
        len: name.len(),
        inum,
        used: true,
    };
    Ok(inum)
}
```

Why this passes: `dirlookup` reuse gives both the duplicate check *and* the
"is it a directory?" check for free; `?` on `alloc` cleanly bails if inodes run
out; and recording the entry is what makes a later `dirlookup` find the file.
The error variants (`NotFound`, `AlreadyExists`, `IsADirectory`) are returned as
plain values, which is the whole point of `Result` in a `no_std` kernel.
