//! fs.rs — a tiny in-memory filesystem. (Exercise 10 reference solution.)

use crate::spinlock::SpinLock;

pub const NINODE: usize = 64;
pub const NDIRENT: usize = 16;
pub const NAMELEN: usize = 14;
pub const FILESIZE: usize = 128;
pub const ROOT: usize = 1;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InodeKind {
    Free,
    File,
    Dir,
}

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

#[derive(Clone, Copy)]
struct DirEnt {
    name: [u8; NAMELEN],
    len: usize,
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

pub struct FileSystem {
    inodes: [Inode; NINODE],
}

impl FileSystem {
    pub const fn new() -> FileSystem {
        FileSystem {
            inodes: [const { Inode::new() }; NINODE],
        }
    }

    pub fn init(&mut self) {
        for i in 0..NINODE {
            self.inodes[i] = Inode::new();
        }
        self.inodes[ROOT].kind = InodeKind::Dir;
    }

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

    pub fn dirlookup(&self, dir: usize, name: &[u8]) -> Result<usize, FsError> {
        if self.inodes[dir].kind != InodeKind::Dir {
            return Err(FsError::NotADirectory);
        }
        for e in &self.inodes[dir].entries {
            if e.used && e.len == name.len() && &e.name[..e.len] == name {
                return Ok(e.inum);
            }
        }
        Err(FsError::NotFound)
    }

    pub fn dircreate(&mut self, dir: usize, name: &[u8], kind: InodeKind) -> Result<usize, FsError> {
        if name.len() > NAMELEN {
            return Err(FsError::NameTooLong);
        }
        // Reject duplicates (also validates that `dir` is a directory).
        match self.dirlookup(dir, name) {
            Ok(_) => return Err(FsError::AlreadyExists),
            Err(FsError::NotFound) => {}
            Err(e) => return Err(e),
        }
        // Find a free directory slot.
        let mut slot = None;
        for i in 0..NDIRENT {
            if !self.inodes[dir].entries[i].used {
                slot = Some(i);
                break;
            }
        }
        let slot = slot.ok_or(FsError::DirFull)?;
        // Allocate the inode (the `?` propagates NoFreeInode if the table is full).
        let inum = self.alloc(kind)?;
        // Fill in the directory entry.
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

    pub fn write(&mut self, inum: usize, data: &[u8]) -> Result<usize, FsError> {
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
    }

    /// Is this inode a directory? (Used by the shell's `cd`.) (Given.)
    pub fn is_dir(&self, inum: usize) -> bool {
        self.inodes[inum].kind == InodeKind::Dir
    }

    /// Call `f(name, kind)` for every entry in directory `dir`. (Used by the
    /// shell's `ls`.) (Given.)
    pub fn for_each_entry(&self, dir: usize, mut f: impl FnMut(&[u8], InodeKind)) {
        if self.inodes[dir].kind != InodeKind::Dir {
            return;
        }
        for e in &self.inodes[dir].entries {
            if e.used {
                let kind = self.inodes[e.inum].kind;
                f(&e.name[..e.len], kind);
            }
        }
    }
}

pub static FS: SpinLock<FileSystem> = SpinLock::new(FileSystem::new());
