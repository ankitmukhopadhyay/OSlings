#![no_std]
#![no_main]

// ╔══════════════════════════════════════════════════════════════════════╗
// ║  Exercise 10 — Filesystem                                              ║
// ║  Goal: implement inode/directory operations with a Result-based API.   ║
// ╚══════════════════════════════════════════════════════════════════════╝
//
// The work is in `fs.rs` (`dirlookup`, `dircreate`, `write`). This file is the
// test harness — read it (UNDERSTAND). It locks the filesystem, then creates
// files and directories, looks them up, reads/writes, and checks that the
// *errors* (NotFound, AlreadyExists, IsADirectory) come back as the right
// `Result` values.

mod entry;
mod fs;
mod testdev;
mod uart;
// Carried from earlier exercises; not exercised by this test.
#[allow(dead_code)]
mod kalloc;
#[allow(dead_code)]
mod kheap;
#[allow(dead_code)]
mod memlayout;
#[allow(dead_code)]
mod param;
#[allow(dead_code)]
mod proc;
#[allow(dead_code)]
mod sched;
#[allow(dead_code)]
mod semaphore;
#[allow(dead_code)]
mod spinlock;
#[allow(dead_code)]
mod swtch;
#[allow(dead_code)]
mod vm;

use core::panic::PanicInfo;
use fs::{FsError, InodeKind, ROOT, FS};

#[no_mangle]
pub extern "C" fn kmain() -> ! {
    uart::puts("\nrv6 booting (exercise 10: filesystem)...\n");
    if run_checks() {
        uart::puts("OSLINGS:PASS\n");
    } else {
        uart::puts("OSLINGS:FAIL\n");
    }
    testdev::exit_success();
}

fn run_checks() -> bool {
    let mut fs = FS.lock(); // lock the filesystem for the whole test
    fs.init();

    // 1) create a regular file in the root directory
    let hello = match fs.dircreate(ROOT, b"hello", InodeKind::File) {
        Ok(i) => i,
        Err(_) => {
            uart::puts("  [fail] creating /hello failed\n");
            return false;
        }
    };

    // 2) look it up again — same inode
    match fs.dirlookup(ROOT, b"hello") {
        Ok(i) if i == hello => {}
        _ => {
            uart::puts("  [fail] lookup of /hello returned the wrong inode\n");
            return false;
        }
    }

    // 3) a name that isn't there comes back as NotFound
    match fs.dirlookup(ROOT, b"missing") {
        Err(FsError::NotFound) => {}
        _ => {
            uart::puts("  [fail] a missing name should be NotFound\n");
            return false;
        }
    }

    // 4) creating the same name twice is AlreadyExists
    match fs.dircreate(ROOT, b"hello", InodeKind::File) {
        Err(FsError::AlreadyExists) => {}
        _ => {
            uart::puts("  [fail] a duplicate create should be AlreadyExists\n");
            return false;
        }
    }

    // 5) write the file, then read it back byte-for-byte
    let msg = b"world!";
    match fs.write(hello, msg) {
        Ok(n) if n == msg.len() => {}
        _ => {
            uart::puts("  [fail] write to /hello failed\n");
            return false;
        }
    }
    let mut buf = [0u8; 64];
    match fs.read(hello, &mut buf) {
        Ok(n) if n == msg.len() && &buf[..n] == msg => {}
        _ => {
            uart::puts("  [fail] read did not return what we wrote\n");
            return false;
        }
    }

    // 6) directories nest: make /sub, then /sub/inner inside it
    let sub = match fs.dircreate(ROOT, b"sub", InodeKind::Dir) {
        Ok(i) => i,
        Err(_) => {
            uart::puts("  [fail] creating directory /sub failed\n");
            return false;
        }
    };
    let inner = match fs.dircreate(sub, b"inner", InodeKind::File) {
        Ok(i) => i,
        Err(_) => {
            uart::puts("  [fail] creating /sub/inner failed\n");
            return false;
        }
    };
    match fs.dirlookup(sub, b"inner") {
        Ok(i) if i == inner => {}
        _ => {
            uart::puts("  [fail] lookup of /sub/inner failed\n");
            return false;
        }
    }

    // 7) you can't write file data into a directory
    match fs.write(sub, b"x") {
        Err(FsError::IsADirectory) => {}
        _ => {
            uart::puts("  [fail] writing to a directory should be IsADirectory\n");
            return false;
        }
    }

    uart::puts("  [ok] inodes, directories, files, and Result errors all correct\n");
    true
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    uart::puts("OSLINGS:FAIL (panic)\n");
    testdev::exit_failure(1);
}
