#![no_std]
#![no_main]

// ╔══════════════════════════════════════════════════════════════════════╗
// ║  Exercise 22 — userland: exec, and a shell in user mode      PART 2    ║
// ║  Goal: add the `exec` system call, so the shell itself can run as a    ║
// ║        user program — fork + exec + wait, with no kernel privileges.   ║
// ╚══════════════════════════════════════════════════════════════════════╝
//
// You have fork and wait (exercise 21). The missing piece is `exec`: replace a
// running process's memory with a DIFFERENT program. With it, a user-mode shell
// can do what every Unix shell does — fork a child, have the child exec the
// command you typed, and wait for it. This is the capstone: even the shell is
// now just a user program, talking to the kernel only through system calls.
// You implement:
//
//   exec.rs  exec_into  — swap a running process's whole address space for a
//                         new program (the heart of the exec system call)
//
// Try it live:  `cd rv6 && cargo run`  then, at the kernel prompt:
//   run sh            (drop into the USER-MODE shell; the prompt becomes `$ `)
//     hello           (run user programs by fork + exec + wait)
//     echo hi there
//     exit            (leave sh, back to the kernel `rv6$` shell)

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
        if exec_self_check() {
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
        uart::puts("rv6: starting shell. Try `run sh` for the user-mode shell. (Ctrl-A X to quit)\n\n");
        shell::run();
    }
}

// ========================================================================
//  Harness.
// ========================================================================

#[cfg(feature = "harness")]
fn put_hex(n: usize) {
    uart::puts("0x");
    let mut started = false;
    for i in (0..16).rev() {
        let d = (n >> (i * 4)) & 0xf;
        if d != 0 || started || i == 0 {
            started = true;
            uart::putc(b"0123456789abcdef"[d]);
        }
    }
}

#[cfg(feature = "harness")]
fn put_num(n: isize) {
    if n < 0 {
        uart::puts("-");
    }
    let mut v = n.unsigned_abs();
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    for &b in &buf[i..] {
        uart::putc(b);
    }
}

/// Exec a program and run its whole process tree, returning how the root ended
/// (or None if exec failed). (Given.)
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

#[cfg(feature = "harness")]
fn exec_self_check() -> bool {
    unsafe {
        // ---- 1. exec replaces the CURRENT image (and does not return) ----
        // execself execs `args x` (two arguments), so it should exit with
        // argc = 2. If exec RETURNED instead of replacing the image, execself
        // would fall through to its own exit(88) — the tell-tale of a swap that
        // did not happen.
        uart::puts("  run execself — exec replaces the running program...\n");
        match run_prog("execself", &[]) {
            Some(usermode::RunOutcome::Exited(2)) => {
                uart::puts("  [ok] exec replaced the image and did not return\n");
            }
            Some(usermode::RunOutcome::Exited(88)) => {
                uart::puts("  [fail] exec RETURNED to the caller — on success it must replace\n");
                uart::puts("         the image and resume as the new program. Does exec_into\n");
                uart::puts("         install the new page table and repoint the trapframe?\n");
                return false;
            }
            Some(usermode::RunOutcome::Exited(n)) => {
                uart::puts("  [fail] execself exited ");
                put_num(n);
                uart::puts(", expected 2 (= argc of `args x`)\n");
                return false;
            }
            Some(usermode::RunOutcome::TimedOut) => {
                uart::puts("  [fail] execself never finished — does exec_into build and\n");
                uart::puts("         install a valid new address space?\n");
                return false;
            }
            _ => {
                uart::puts("  [fail] execself faulted or did not run\n");
                return false;
            }
        }

        // ---- 2. fork + exec + wait: exactly what a shell does ----
        // exectest forks; the child execs `echo hi` (printing "hi"); the parent
        // waits, then exits 42 — proving exec in the CHILD leaves the parent
        // untouched.
        uart::puts("  run exectest — fork a child, exec `echo hi` in it, then wait...\n");
        syscall::clear_capture();
        let outcome = run_prog("exectest", &[]);
        let said = syscall::captured();
        if !said.contains("hi") {
            uart::puts("  [fail] the child's exec did not run the new program — `echo hi`\n");
            uart::puts("         should have printed \"hi\". Got: \"");
            uart::puts(said);
            uart::puts("\"\n");
            return false;
        }
        match outcome {
            Some(usermode::RunOutcome::Exited(42)) => {
                uart::puts("  [ok] the child exec'd a new program; the parent ran on unharmed\n");
            }
            Some(usermode::RunOutcome::Exited(n)) => {
                uart::puts("  [fail] exectest exited ");
                put_num(n);
                uart::puts(", expected 42 — exec in the CHILD must not disturb the parent\n");
                return false;
            }
            Some(usermode::RunOutcome::TimedOut) => {
                uart::puts("  [fail] exectest never finished — does the exec'd child exit,\n");
                uart::puts("         and does the parent's wait reap it?\n");
                return false;
            }
            Some(usermode::RunOutcome::Faulted(c)) => {
                uart::puts("  [fail] exectest faulted (scause ");
                put_hex(c);
                uart::puts(")\n");
                return false;
            }
            None => {
                uart::puts("  [fail] exectest did not run\n");
                return false;
            }
        }

        // ---- 3. a failed exec returns -1 and leaves the caller running ----
        // execfail execs a program that does not exist, then exits 7. If exec
        // did not return the error, execfail could not reach that exit.
        uart::puts("  run execfail — exec a missing program (must fail cleanly)...\n");
        match run_prog("execfail", &[]) {
            Some(usermode::RunOutcome::Exited(7)) => {
                uart::puts("  [ok] a failed exec returned -1; the caller kept running\n");
            }
            Some(usermode::RunOutcome::Exited(n)) => {
                uart::puts("  [fail] execfail exited ");
                put_num(n);
                uart::puts(", expected 7 — a failed exec must return -1, not run anything\n");
                return false;
            }
            _ => {
                uart::puts("  [fail] execfail did not finish cleanly\n");
                return false;
            }
        }
    }

    uart::puts("  [ok] exec works — fork + exec + wait, the whole shell pattern\n");
    true
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    uart::puts("OSLINGS:FAIL (panic)\n");
    testdev::exit_failure(1);
}
