//! fs.rs — a tiny in-memory filesystem: inodes and directories.
//!
//! A real filesystem lives on a disk; we don't have a disk driver yet (that's
//! exercise 11), so this one lives in RAM. But the *shapes* are the real ones:
//!   - an **inode** ("index node") is the kernel's record of one file or
//!     directory — its kind, its size, and its contents;
//!   - a **directory** is just an inode whose contents are a list of
//!     (name → inode number) entries;
//!   - every operation returns a **`Result`**, because filesystem calls fail in
//!     ordinary, expected ways (not found, already exists, ...) that the caller
//!     must handle — there is no `std` to throw for us.

use crate::spinlock::SpinLock;

pub const NINODE: usize = 64; // total inodes
pub const NDIRENT: usize = 16; // entries per directory
pub const NAMELEN: usize = 14; // max filename length
pub const FILESIZE: usize = 128; // max bytes per file
pub const ROOT: usize = 1; // inode number of the root directory (0 is unused)

/// What an inode is. (An `enum` — exactly one of these.)
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InodeKind {
    Free, // this inode slot is unused
    File, // a regular file (contents in `data`)
    Dir,  // a directory (contents in `entries`)
}

/// The ways a filesystem operation can fail. Returning these in a `Result` is
/// how a `no_std` kernel does error handling: explicit, checked values — no
/// exceptions, no `std::io::Error`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FsError {
    NotFound,
    AlreadyExists,
    NotADirectory,
    IsADirectory,
    NoFreeInode,
    DirFull,
    NameTooLong,
    FileTooBig,
}

/// One directory entry: a name mapped to an inode number.
#[derive(Clone, Copy)]
struct DirEnt {
    name: [u8; NAMELEN],
    len: usize, // actual name length (<= NAMELEN)
    inum: usize,
    used: bool,
}

impl DirEnt {
    const fn new() -> DirEnt {
        DirEnt {
            name: [0; NAMELEN],
            len: 0,
            inum: 0,
            used: false,
        }
    }
}

/// One inode. (For simplicity, every inode carries both a `data` array and an
/// `entries` array; a File uses `data`, a Dir uses `entries`.)
#[derive(Clone, Copy)]
pub struct Inode {
    kind: InodeKind,
    size: usize,
    data: [u8; FILESIZE],
    entries: [DirEnt; NDIRENT],
}

impl Inode {
    const fn new() -> Inode {
        Inode {
            kind: InodeKind::Free,
            size: 0,
            data: [0; FILESIZE],
            entries: [const { DirEnt::new() }; NDIRENT],
        }
    }
}

/// The whole filesystem: just the table of inodes. It lives behind a `SpinLock`
/// (exercise 07), so the kernel can share it safely.
pub struct FileSystem {
    inodes: [Inode; NINODE],
}

impl FileSystem {
    pub const fn new() -> FileSystem {
        FileSystem {
            inodes: [const { Inode::new() }; NINODE],
        }
    }

    /// Reset the filesystem and create an empty root directory. (UNDERSTAND.)
    pub fn init(&mut self) {
        for i in 0..NINODE {
            self.inodes[i] = Inode::new();
        }
        self.inodes[ROOT].kind = InodeKind::Dir;
    }

    /// Find a free inode, mark it `kind`, and return its number. (UNDERSTAND.)
    fn alloc(&mut self, kind: InodeKind) -> Result<usize, FsError> {
        for i in ROOT..NINODE {
            if self.inodes[i].kind == InodeKind::Free {
                self.inodes[i] = Inode::new();
                self.inodes[i].kind = kind;
                return Ok(i);
            }
        }
        Err(FsError::NoFreeInode)
    }

    /// Read a file's contents into `buf`; returns the number of bytes read.
    /// (UNDERSTAND — given; the mirror of `write`.)
    pub fn read(&self, inum: usize, buf: &mut [u8]) -> Result<usize, FsError> {
        let node = &self.inodes[inum];
        match node.kind {
            InodeKind::Free => return Err(FsError::NotFound),
            InodeKind::Dir => return Err(FsError::IsADirectory),
            InodeKind::File => {}
        }
        let n = core::cmp::min(node.size, buf.len());
        buf[..n].copy_from_slice(&node.data[..n]);
        Ok(n)
    }

    /// Look up `name` in directory `dir`; return its inode number.
    pub fn dirlookup(&self, dir: usize, name: &[u8]) -> Result<usize, FsError> {
        // IMPLEMENT:
        //   1. If self.inodes[dir].kind is not Dir, return Err(NotADirectory).
        //   2. Scan self.inodes[dir].entries. For each entry `e` that is `used`
        //      and whose name matches (e.len == name.len() and
        //      &e.name[..e.len] == name), return Ok(e.inum).
        //   3. If nothing matches, return Err(NotFound).
        let _ = (dir, name); // remove once implemented
        Err(FsError::NotFound)
    }

    /// Create a new inode of `kind` named `name` inside directory `dir`; return
    /// its inode number.
    pub fn dircreate(&mut self, dir: usize, name: &[u8], kind: InodeKind) -> Result<usize, FsError> {
        // IMPLEMENT:
        //   1. If name.len() > NAMELEN, return Err(NameTooLong).
        //   2. Make sure it doesn't already exist. Calling self.dirlookup(dir,
        //      name) also validates `dir` is a directory:
        //        match self.dirlookup(dir, name) {
        //            Ok(_) => return Err(FsError::AlreadyExists),
        //            Err(FsError::NotFound) => {}      // good — free to create
        //            Err(e) => return Err(e),          // e.g. NotADirectory
        //        }
        //   3. Find a free entry slot in self.inodes[dir].entries (one whose
        //      `used` is false). If there is none, return Err(DirFull).
        //   4. Allocate the inode with the `?` operator:
        //        let inum = self.alloc(kind)?;
        //   5. Fill the entry: copy the name bytes into a [0u8; NAMELEN], set
        //      len/inum/used, and store it in the slot you found.
        //   6. Return Ok(inum).
        let _ = (dir, name, kind); // remove once implemented
        Err(FsError::NoFreeInode)
    }

    /// Write `data` as the entire contents of file `inum`; return bytes written.
    pub fn write(&mut self, inum: usize, data: &[u8]) -> Result<usize, FsError> {
        // IMPLEMENT:
        //   1. Match self.inodes[inum].kind: Free → Err(NotFound),
        //      Dir → Err(IsADirectory), File → proceed.
        //   2. If data.len() > FILESIZE, return Err(FileTooBig).
        //   3. Copy data into self.inodes[inum].data[..data.len()], set
        //      self.inodes[inum].size = data.len(), and return Ok(data.len()).
        let _ = (inum, data); // remove once implemented
        Err(FsError::FileTooBig)
    }
}

/// The kernel's single filesystem instance, shareable behind a lock.
pub static FS: SpinLock<FileSystem> = SpinLock::new(FileSystem::new());
