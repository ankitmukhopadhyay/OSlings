#![no_std]
#![no_main]

// ╔══════════════════════════════════════════════════════════════════════╗
// ║  Exercise 16 — Shell                                          PART 2    ║
// ║  Goal: a read-eval-print loop with commands: pwd, ls, cd, mkdir.       ║
// ╚══════════════════════════════════════════════════════════════════════╝
//
// We can read input (exercise 15) and we have a filesystem (exercise 10). Put
// them together: a *shell* that reads a line, figures out which command it is,
// and runs it on the live filesystem. The work is in `shell.rs` (the `exec`
// dispatch); the command handlers and the read loop are given to read.
//
// Try it live:  `cd rv6 && cargo run`  boots to a `rv6$` prompt you can use.

extern crate alloc;

#[allow(dead_code)]
mod console;
#[allow(dead_code)]
mod entry;
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
mod testdev;
#[allow(dead_code)]
mod trap;
#[allow(dead_code)]
mod uart;
#[allow(dead_code)]
mod vm;

use core::panic::PanicInfo;

const BANNER: &str = r#"
                  __
 _ __            / /_
| '__|  \ \ / /  | '_ \
| |      \ V /   | (_) |
|_|       \_/     \___/

  A tiny interesting RISC-V OS
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
        if shell_self_check() {
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
        uart::puts("rv6: starting shell. Try: mkdir, ls, cd, pwd. (Ctrl-A X to quit)\n\n");
        shell::run();
    }
}

/// A capture buffer used to check the shell's output in the harness.
#[cfg(feature = "harness")]
struct BufOut {
    buf: [u8; 512],
    len: usize,
}

#[cfg(feature = "harness")]
impl BufOut {
    fn new() -> BufOut {
        BufOut { buf: [0; 512], len: 0 }
    }
    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }
    fn clear(&mut self) {
        self.len = 0;
    }
}

#[cfg(feature = "harness")]
impl shell::Out for BufOut {
    fn puts(&mut self, s: &str) {
        for &b in s.as_bytes() {
            if self.len < self.buf.len() {
                self.buf[self.len] = b;
                self.len += 1;
            }
        }
    }
}

/// Drive the shell with a script of commands and check it behaves.
#[cfg(feature = "harness")]
fn shell_self_check() -> bool {
    let mut sh = shell::Shell::new();
    let mut out = BufOut::new();

    // mkdir then ls should list the new directory
    sh.exec("mkdir docs", &mut out);
    sh.exec("ls", &mut out);
    if !out.as_str().contains("docs") {
        uart::puts("  [fail] 'mkdir docs' then 'ls' did not list docs\n");
        return false;
    }

    // cd into it; pwd should report the path
    out.clear();
    sh.exec("cd docs", &mut out);
    sh.exec("pwd", &mut out);
    if out.as_str() != "/docs\n" {
        uart::puts("  [fail] 'cd docs; pwd' did not print /docs\n");
        return false;
    }

    // a nested mkdir + ls
    out.clear();
    sh.exec("mkdir sub", &mut out);
    sh.exec("ls", &mut out);
    if !out.as_str().contains("sub") {
        uart::puts("  [fail] nested 'mkdir sub; ls' failed\n");
        return false;
    }

    // cd .. returns to the root
    out.clear();
    sh.exec("cd ..", &mut out);
    sh.exec("pwd", &mut out);
    if out.as_str() != "/\n" {
        uart::puts("  [fail] 'cd ..; pwd' did not return to /\n");
        return false;
    }

    uart::puts("  [ok] the shell parses and runs pwd/ls/cd/mkdir on the filesystem\n");
    true
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    uart::puts("OSLINGS:FAIL (panic)\n");
    testdev::exit_failure(1);
}
