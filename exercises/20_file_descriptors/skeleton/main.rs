#![no_std]
#![no_main]

// ╔══════════════════════════════════════════════════════════════════════╗
// ║  Exercise 20 — File descriptors                              PART 2    ║
// ║  Goal: let user programs open, read, write, and close files through   ║
// ║        small integer file descriptors.                                ║
// ╚══════════════════════════════════════════════════════════════════════╝
//
// So far a user program could only write to the console (fd 1). Real programs
// open files, read them a piece at a time, and write them — all through
// per-process file descriptors. This exercise builds that. You implement:
//
//   syscall.rs  fdalloc   — hand out the next free descriptor
//   syscall.rs  sys_open  — open (or create) a file, return a descriptor
//   syscall.rs  sys_read  — read at the fd's offset, then advance the offset
//
// Try it live:  `cd rv6 && cargo run`  then type:
//   run create notes.txt      (a user PROGRAM creates + writes the file)
//   run cat notes.txt         (a user PROGRAM reads it back, via syscalls)

extern crate alloc;

#[allow(dead_code)]
mod console;
#[allow(dead_code)]
mod entry;
#[allow(dead_code)]
mod exec;
#[allow(dead_code)]
mod file;
#[allow(dead_code)]
mod fs;
#[allow(dead_code)]
mod kalloc;
#[allow(dead_code)]
mod kheap;
#[allow(dead_code)]
mod memlayout;
#[allow(dead_code)]
mod param;
#[allow(dead_code)]
mod plic;
#[allow(dead_code)]
mod proc;
#[allow(dead_code)]
mod sched;
#[allow(dead_code)]
mod semaphore;
#[allow(dead_code)]
mod shell;
#[allow(dead_code)]
mod spinlock;
#[allow(dead_code)]
mod start;
#[allow(dead_code)]
mod swtch;
#[allow(dead_code)]
mod syscall;
#[allow(dead_code)]
mod testdev;
#[allow(dead_code)]
mod trap;
#[allow(dead_code)]
mod uart;
#[allow(dead_code)]
mod usermode;
#[allow(dead_code)]
mod vm;

use core::panic::PanicInfo;

const BANNER: &str = r#"
                  __
 _ __            / /_
| '__|  \ \ / /  | '_ \
| |      \ V /   | (_) |
|_|       \_/     \___/

  A tiny RISC-V OS
"#;

unsafe fn kinit() {
    uart::init();
    kalloc::init();
    vm::kvminithart(vm::kvmmake());
    proc::init();
    trap::init();
    fs::FS.lock().init(); // create the root directory
}

#[no_mangle]
pub extern "C" fn kmain() -> ! {
    unsafe {
        kinit();
    }

    uart::puts("\n");
    uart::puts(BANNER);
    uart::puts("\nrv6: kernel booted.\n");

    #[cfg(feature = "harness")]
    {
        if fd_self_check() {
            uart::puts("OSLINGS:PASS\n");
        } else {
            uart::puts("OSLINGS:FAIL\n");
        }
        testdev::exit_success();
    }

    #[cfg(not(feature = "harness"))]
    {
        unsafe {
            console::init();
            trap::intr_on();
        }
        uart::puts("rv6: starting shell. Try: run create notes.txt, then run cat notes.txt. (Ctrl-A X to quit)\n\n");
        shell::run();
    }
}

// ========================================================================
//  Harness.
// ========================================================================

/// The contents we seed into a file for the read test. It is longer than
/// cat's 64-byte buffer on purpose, so cat must read TWICE — which only works
/// if `sys_read` advances the offset between reads.
#[cfg(feature = "harness")]
const POEM: &str = "two roads diverged in a yellow wood,\nand sorry i could not travel both\n";

/// Exec a program and run it to completion, returning how it ended (or None if
/// exec failed). (Given.)
#[cfg(feature = "harness")]
unsafe fn run_prog(name: &str, args: &[&str]) -> Option<usermode::RunOutcome> {
    let p = match exec::exec(name, args) {
        Ok(p) => p,
        Err(_) => return None,
    };
    let outcome = usermode::run(p);
    proc::freeproc(p);
    Some(outcome)
}

/// Put a file called `name` in the root directory holding `contents`.
/// (Given — the harness seeds the filesystem so the read test has something
/// to read.)
#[cfg(feature = "harness")]
fn seed_file(name: &[u8], contents: &[u8]) -> bool {
    let mut fsg = fs::FS.lock();
    let inum = match fsg.dircreate(fs::ROOT, name, fs::InodeKind::File) {
        Ok(i) => i,
        Err(_) => return false,
    };
    fsg.write(inum, contents).is_ok()
}

#[cfg(feature = "harness")]
fn fd_self_check() -> bool {
    unsafe {
        // ---- 1. read an existing file (open + read + offset advance) ----
        // The `cat` program opens "poem", reads it 64 bytes at a time, and
        // writes each chunk to stdout. POEM is 71 bytes, so cat reads twice;
        // the second read only reaches new bytes if sys_read advanced the
        // offset. A stuck offset would loop forever (caught as a timeout).
        if !seed_file(b"poem", POEM.as_bytes()) {
            uart::puts("  [fail] harness could not seed the test file\n");
            return false;
        }
        uart::puts("  cat poem — open + read a file back...\n");
        syscall::clear_capture();
        match run_prog("cat", &["poem"]) {
            Some(usermode::RunOutcome::Exited(_)) => {}
            Some(usermode::RunOutcome::TimedOut) => {
                uart::puts("  [fail] cat never finished — does sys_read ADVANCE the offset?\n");
                uart::puts("         (a stuck offset re-reads the same bytes forever)\n");
                return false;
            }
            _ => {
                uart::puts("  [fail] cat did not run to completion\n");
                return false;
            }
        }
        if syscall::captured() != POEM {
            uart::puts("  [fail] cat printed:\n\"");
            uart::puts(syscall::captured());
            uart::puts("\"\n         expected the file's contents. Check sys_open (does it\n");
            uart::puts("         return a valid fd?) and sys_read (does it read + copy out?).\n");
            return false;
        }
        uart::puts("  [ok] cat read the whole file back, across two reads\n");

        // ---- 2. create a file, write to it, read it back (open O_CREATE +
        //         write to an inode fd + read it) ----
        uart::puts("  create out.txt — open(O_CREATE) + write through an fd...\n");
        match run_prog("create", &["out.txt"]) {
            Some(usermode::RunOutcome::Exited(_)) => {}
            _ => {
                uart::puts("  [fail] create did not run to completion\n");
                return false;
            }
        }
        syscall::clear_capture();
        match run_prog("cat", &["out.txt"]) {
            Some(usermode::RunOutcome::Exited(_)) => {}
            _ => {
                uart::puts("  [fail] cat of the created file did not finish\n");
                return false;
            }
        }
        if syscall::captured() != "saved by fd\n" {
            uart::puts("  [fail] the created file read back as \"");
            uart::puts(syscall::captured());
            uart::puts("\", expected \"saved by fd\" (open O_CREATE / write path)\n");
            return false;
        }
        uart::puts("  [ok] a file created and written through an fd read back correctly\n");

        // ---- 3. opening a missing file fails cleanly (no crash, no hang) ----
        uart::puts("  cat nope — opening a missing file must fail, not crash...\n");
        syscall::clear_capture();
        match run_prog("cat", &["nope"]) {
            Some(usermode::RunOutcome::Exited(_)) => {}
            _ => {
                uart::puts("  [fail] cat of a missing file did not exit cleanly\n");
                return false;
            }
        }
        if !syscall::captured().is_empty() {
            uart::puts("  [fail] cat of a missing file printed something; open should return -1\n");
            return false;
        }
        uart::puts("  [ok] opening a missing file returned an error, handled cleanly\n");
    }

    uart::puts("  [ok] user programs can open, read, write, and close files\n");
    true
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    uart::puts("OSLINGS:FAIL (panic)\n");
    testdev::exit_failure(1);
}
