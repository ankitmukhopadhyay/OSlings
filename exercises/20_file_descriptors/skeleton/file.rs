//! file.rs — the "open file" abstraction and the per-process file table.
//! (UNDERSTAND — given; you use these from syscall.rs.)
//!
//! A **file descriptor** (fd) is just a small integer a program uses to name
//! something it has open. The kernel keeps, per process, an array of open
//! files (`ofile` in `Proc`); the fd is an index into that array. By
//! convention every process starts with three already open:
//!
//!     fd 0 = standard input    (the console)
//!     fd 1 = standard output   (the console)
//!     fd 2 = standard error    (the console)
//!
//! `open` hands back the next free fd; `close` frees one; `read`/`write` use
//! one. What sits behind an fd can be the console or a file on disk — the
//! program reads and writes the same way regardless. That uniform interface
//! ("everything is a file") is one of Unix's oldest good ideas.

/// The most files a single process may have open at once.
pub const NOFILE: usize = 16;

/// What kind of thing a file descriptor refers to.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    /// A free slot in the file table (nothing open here).
    None,
    /// The console (keyboard in, screen out).
    Console,
    /// A file in the filesystem, identified by its inode number.
    Inode,
}

/// One open file. Small and `Copy`, so the file table is a plain array.
///
/// The important stateful field is `off`: the current read/write **offset**.
/// Each `read` starts where the last one stopped and then advances `off`, so
/// successive reads walk through the file. This is what makes a file
/// descriptor different from the shell's one-shot `read(inum)` in exercise 17:
/// an fd *remembers its place*.
#[derive(Clone, Copy)]
pub struct File {
    pub kind: FileKind,
    /// Which inode, when `kind == Inode`.
    pub inum: usize,
    /// The read/write cursor: the byte offset the next read/write starts at.
    pub off: usize,
    /// May the program read from this fd?
    pub readable: bool,
    /// May the program write to this fd?
    pub writable: bool,
}

impl File {
    /// A free (closed) slot.
    pub const fn none() -> File {
        File {
            kind: FileKind::None,
            inum: 0,
            off: 0,
            readable: false,
            writable: false,
        }
    }

    /// The console, opened for both reading and writing.
    pub const fn console() -> File {
        File {
            kind: FileKind::Console,
            inum: 0,
            off: 0,
            readable: true,
            writable: true,
        }
    }
}

// ========================================================================
//  Flags for open(path, flags). These match xv6's values.
// ========================================================================

/// Open for reading only (the default: value 0).
pub const O_RDONLY: usize = 0x000;
/// Open for writing only.
pub const O_WRONLY: usize = 0x001;
/// Open for both reading and writing.
pub const O_RDWR: usize = 0x002;
/// Create the file if it does not exist.
pub const O_CREATE: usize = 0x200;
/// Truncate the file to zero length when opening it.
pub const O_TRUNC: usize = 0x400;
