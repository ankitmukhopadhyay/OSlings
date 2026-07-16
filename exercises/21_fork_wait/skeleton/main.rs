#![no_std]
#![no_main]

// ╔══════════════════════════════════════════════════════════════════════╗
// ║  Exercise 21 — fork / wait / exit                            PART 2    ║
// ║  Goal: let a program start another with fork(), and collect its       ║
// ║        result with wait() — driven by a real scheduler.               ║
// ╚══════════════════════════════════════════════════════════════════════╝
//
// Until now the kernel ran ONE user process at a time. This exercise adds the
// three system calls that make a process tree possible — fork (duplicate the
// running process), exit (finish, leaving a status), and wait (collect a
// finished child) — plus the scheduler that runs several processes at once
// (built on your round-robin policy from exercise 06). You implement:
//
//   syscall.rs  sys_fork  — duplicate the caller; child returns 0, parent the pid
//   syscall.rs  sys_wait  — reap a finished child and return its pid/status
//
// Try it live:  `cd rv6 && cargo run`  then type:
//   run forktest      (a parent forks a child; watch both print)
//   run forks2        (a parent forks two children and sums their statuses)

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
        if fork_self_check() {
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
        uart::puts("rv6: starting shell. Try: run forktest, run forks2. (Ctrl-A X to quit)\n\n");
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
fn fork_self_check() -> bool {
    unsafe {
        // ---- 1. a single process still runs correctly under the scheduler ----
        // (`hello` never forks; this checks exec + forkret + exit on the new
        // scheduler path before we bring fork into it.)
        uart::puts("  run hello — one process under the new scheduler...\n");
        syscall::clear_capture();
        match run_prog("hello", &[]) {
            Some(usermode::RunOutcome::Exited(_)) => {}
            _ => {
                uart::puts("  [fail] a plain program did not run to completion\n");
                return false;
            }
        }
        if !syscall::captured().contains("hello from user mode") {
            uart::puts("  [fail] the scheduler did not run a single process correctly\n");
            return false;
        }
        uart::puts("  [ok] the scheduler runs one process\n");

        // ---- 2. fork creates a second process; wait collects its status ----
        // forktest: parent writes "parent", child writes "child" and exits(7);
        // parent waits and exits(7 + 10 = 17).
        uart::puts("  run forktest — fork a child, then wait for it...\n");
        syscall::clear_capture();
        let outcome = run_prog("forktest", &[]);
        let said = syscall::captured();

        // First: did fork create a child that actually ran? The child prints
        // "child"; if that is missing, fork is the problem.
        if !said.contains("child") {
            uart::puts("  [fail] only the parent ran — fork() must create a second\n");
            uart::puts("         process, and must return 0 in the child (so the child\n");
            uart::puts("         branch runs). Got output: \"");
            uart::puts(said);
            uart::puts("\"\n");
            return false;
        }
        // The child ran. Now: did wait block for it and deliver its status?
        match outcome {
            Some(usermode::RunOutcome::Exited(17)) => {}
            Some(usermode::RunOutcome::Exited(n)) => {
                uart::puts("  [fail] the child ran, but forktest exited ");
                put_num(n);
                uart::puts(", expected 17\n         (= child's exit 7 + 10). Does wait() return the CHILD's status?\n");
                return false;
            }
            Some(usermode::RunOutcome::TimedOut) => {
                uart::puts("  [fail] the child ran but wait() never returned — does wait\n");
                uart::puts("         find and reap a ZOMBIE child (and free it)?\n");
                return false;
            }
            _ => {
                uart::puts("  [fail] forktest did not finish cleanly\n");
                return false;
            }
        }
        uart::puts("  [ok] fork ran a child, and wait collected its exit status (17)\n");

        // ---- 3. two children: wait reaps both, scheduler juggles three ----
        // forks2: children exit 3 and 4; parent waits twice and exits 3+4 = 7.
        uart::puts("  run forks2 — fork two children and reap both...\n");
        match run_prog("forks2", &[]) {
            Some(usermode::RunOutcome::Exited(7)) => {
                uart::puts("  [ok] both children were reaped; their statuses summed to 7\n");
            }
            Some(usermode::RunOutcome::Exited(n)) => {
                uart::puts("  [fail] forks2 exited ");
                put_num(n);
                uart::puts(", expected 7 (children exit 3 and 4). Does wait reap EACH child?\n");
                return false;
            }
            Some(usermode::RunOutcome::TimedOut) => {
                uart::puts("  [fail] forks2 never finished — can the scheduler run several\n");
                uart::puts("         processes, and does wait return once per child?\n");
                return false;
            }
            Some(usermode::RunOutcome::Faulted(c)) => {
                uart::puts("  [fail] forks2 faulted (scause ");
                put_hex(c);
                uart::puts(")\n");
                return false;
            }
            None => {
                uart::puts("  [fail] forks2 did not run\n");
                return false;
            }
        }
    }

    uart::puts("  [ok] fork / wait / exit and the scheduler all work\n");
    true
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    uart::puts("OSLINGS:FAIL (panic)\n");
    testdev::exit_failure(1);
}
