#![no_std]
#![no_main]

// ╔══════════════════════════════════════════════════════════════════════╗
// ║  Exercise 19 — exec                                          PART 2    ║
// ║  Goal: load a named program (of any size) into a fresh address        ║
// ║        space, hand it command-line arguments, and run it.             ║
// ╚══════════════════════════════════════════════════════════════════════╝
//
// Exercise 18 could run one hard-coded program on one page. This exercise
// builds a real loader: a table of programs, a `load_segment` that copies an
// image of ANY size into the user's address space, and `argv` setup so a
// program can read the arguments it was started with. You implement two
// pieces:
//
//   vm.rs    load_segment  — copy a program image across as many pages as it
//                            needs (the general form of ex18's map_user_pages)
//   exec.rs  build_process — the exec recipe: load image, make a stack, push
//                            argv, and point the CPU at the program
//
// Try it live:  `cd rv6 && cargo run`  then type:
//   progs            (list the programs)
//   run echo hello world
//   run args a b c

extern crate alloc;

#[allow(dead_code)]
mod console;
#[allow(dead_code)]
mod entry;
#[allow(dead_code)]
mod exec;
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
        uart::puts("rv6: starting shell. Try: progs, then `run echo hello world`. (Ctrl-A X to quit)\n\n");
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

/// Check one leaf mapping of a user page table: present, with (at least) the
/// given flags, and the right PTE_U setting. (Given.)
#[cfg(feature = "harness")]
unsafe fn check_mapping(
    table: *mut vm::Pte,
    va: usize,
    want_flags: usize,
    want_user: bool,
    what: &str,
) -> bool {
    let pte = vm::walk(table, va, false);
    if pte.is_null() || !(*pte).is_valid() {
        uart::puts("  [fail] ");
        uart::puts(what);
        uart::puts(" is not mapped (va ");
        put_hex(va);
        uart::puts(")\n");
        return false;
    }
    let flags = (*pte).flags();
    if flags & want_flags != want_flags {
        uart::puts("  [fail] ");
        uart::puts(what);
        uart::puts(" is missing permission bits (has ");
        put_hex(flags);
        uart::puts(", needs ");
        put_hex(want_flags);
        uart::puts(")\n");
        return false;
    }
    if want_user && flags & vm::PTE_U == 0 {
        uart::puts("  [fail] ");
        uart::puts(what);
        uart::puts(" has no PTE_U — user mode cannot touch it\n");
        return false;
    }
    true
}

/// Build a process with exec, verify its address space, run it, and hand back
/// the outcome — or None if exec/verification failed (message already
/// printed). (Given.)
#[cfg(feature = "harness")]
unsafe fn exec_and_run(
    name: &str,
    args: &[&str],
    extra_code_pages: usize,
) -> Option<usermode::RunOutcome> {
    use memlayout::{PGSIZE, USER_CODE, USER_STACK};

    let p = match exec::exec(name, args) {
        Ok(p) => p,
        Err(_) => {
            uart::puts("  [fail] exec(\"");
            uart::puts(name);
            uart::puts("\") failed\n");
            return None;
        }
    };
    let pt = (*p).pagetable;

    // the program's first page and its stack must be user R/X and R/W
    if !check_mapping(pt, USER_CODE, vm::PTE_R | vm::PTE_X, true, "the program's code page") {
        proc::freeproc(p);
        return None;
    }
    // a multi-page image (like `big`) must have its later pages mapped too
    for i in 1..=extra_code_pages {
        if !check_mapping(
            pt,
            USER_CODE + i * PGSIZE,
            vm::PTE_R | vm::PTE_X,
            true,
            "a later page of the program image",
        ) {
            proc::freeproc(p);
            return None;
        }
    }
    if !check_mapping(pt, USER_STACK, vm::PTE_R | vm::PTE_W, true, "the stack page") {
        proc::freeproc(p);
        return None;
    }

    let outcome = usermode::run(p);
    proc::freeproc(p);
    Some(outcome)
}

#[cfg(feature = "harness")]
fn exec_self_check() -> bool {
    unsafe {
        // ---- 1. arguments arrive: `args` exits with its argc ----
        // argv = ["args", "one", "two"] -> argc = 3.
        uart::puts("  exec(\"args\", [one, two]) — arguments reach the program...\n");
        match exec_and_run("args", &["one", "two"], 0) {
            Some(usermode::RunOutcome::Exited(3)) => {
                uart::puts("  [ok] the program saw argc = 3\n");
            }
            Some(usermode::RunOutcome::Exited(n)) => {
                uart::puts("  [fail] argc came through as ");
                put_num(n);
                uart::puts(", expected 3 (check a0/a1 in build_process, and push_argv)\n");
                return false;
            }
            Some(usermode::RunOutcome::TimedOut) => {
                uart::puts("  [fail] the program never finished (did load_segment map its code?)\n");
                return false;
            }
            Some(usermode::RunOutcome::Faulted(c)) => {
                uart::puts("  [fail] it faulted (scause ");
                put_hex(c);
                uart::puts(") — is the code page mapped R+X+U?\n");
                return false;
            }
            None => return false,
        }

        // ---- 2. the argument *strings* arrive intact: `echo` prints them ----
        uart::puts("  exec(\"echo\", [hello, world]) — argument strings survive...\n");
        match exec_and_run("echo", &["hello", "world"], 0) {
            Some(usermode::RunOutcome::Exited(_)) => {}
            _ => {
                uart::puts("  [fail] echo did not run to completion\n");
                return false;
            }
        }
        let said = syscall::captured();
        if !said.contains("hello world\n") {
            uart::puts("  [fail] echo printed \"");
            uart::puts(said);
            uart::puts("\", expected it to contain \"hello world\"\n");
            uart::puts("         (the argv pointers must be USER addresses — see push_argv)\n");
            return false;
        }
        uart::puts("  [ok] echo printed its arguments: \"hello world\"\n");

        // ---- 3. a multi-page image loads: `big` spans two pages, exits 99 ----
        uart::puts("  exec(\"big\") — an image larger than one page...\n");
        match exec_and_run("big", &[], 1) {
            Some(usermode::RunOutcome::Exited(99)) => {
                uart::puts("  [ok] the two-page program loaded and ran (exit 99)\n");
            }
            Some(usermode::RunOutcome::Exited(n)) => {
                uart::puts("  [fail] big exited with ");
                put_num(n);
                uart::puts(", expected 99\n");
                return false;
            }
            _ => {
                uart::puts("  [fail] the two-page program did not load/run — does\n");
                uart::puts("         load_segment loop over EVERY page of the image?\n");
                return false;
            }
        }

        // ---- 4. an unknown program name is a clean error, not a crash ----
        if exec::exec("does_not_exist", &[]).is_ok() {
            uart::puts("  [fail] exec of a missing program should fail, but it succeeded\n");
            return false;
        }
        uart::puts("  [ok] exec of an unknown program name fails cleanly\n");
    }

    uart::puts("  [ok] exec loads programs of any size and passes them arguments\n");
    true
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    uart::puts("OSLINGS:FAIL (panic)\n");
    testdev::exit_failure(1);
}
