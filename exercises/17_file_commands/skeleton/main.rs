#![no_std]
#![no_main]

// ╔══════════════════════════════════════════════════════════════════════╗
// ║  Exercise 17 — File commands                                 PART 2    ║
// ║  Goal: make the shell *do* things to files: touch, cat, rm            ║
// ║        (with echo and rmdir given to read).                            ║
// ╚══════════════════════════════════════════════════════════════════════╝
//
// Exercise 16 gave us a shell that navigates directories (pwd/ls/cd/mkdir).
// Now we add the commands that create, read, and delete *files*. The parsing
// and dispatch are already wired up in `shell.rs`; your job is three command
// handlers that call the filesystem: `cmd_touch`, `cmd_cat`, and `cmd_rm`.
// (`echo … > file` and `rmdir` are given — read them as models.)
//
// Try it live:  `cd rv6 && cargo run`  then:
//   touch notes.txt   ls   echo hello > notes.txt   cat notes.txt   rm notes.txt

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
        if file_commands_self_check() {
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
        uart::puts("rv6: starting shell. Try: touch, cat, echo, rm, ls. (Ctrl-A X to quit)\n\n");
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

/// Drive the shell with a script of file commands and check each one.
#[cfg(feature = "harness")]
fn file_commands_self_check() -> bool {
    let mut sh = shell::Shell::new();
    let mut out = BufOut::new();

    // touch creates an empty file; ls should then list it.
    sh.exec("touch a.txt", &mut out);
    sh.exec("ls", &mut out);
    if !out.as_str().contains("a.txt") {
        uart::puts("  [fail] 'touch a.txt' then 'ls' did not list a.txt\n");
        return false;
    }

    // echo writes into the file; cat reads exactly that back.
    out.clear();
    sh.exec("echo hello > a.txt", &mut out);
    sh.exec("cat a.txt", &mut out);
    if out.as_str() != "hello\n" {
        uart::puts("  [fail] 'echo hello > a.txt' then 'cat a.txt' did not print hello\n");
        return false;
    }

    // rm deletes the file; ls should no longer list it.
    out.clear();
    sh.exec("rm a.txt", &mut out);
    sh.exec("ls", &mut out);
    if out.as_str().contains("a.txt") {
        uart::puts("  [fail] 'rm a.txt' did not remove the file\n");
        return false;
    }

    // mkdir + rmdir on an empty directory removes it.
    out.clear();
    sh.exec("mkdir d", &mut out);
    sh.exec("rmdir d", &mut out);
    sh.exec("ls", &mut out);
    if out.as_str().contains("d/") {
        uart::puts("  [fail] 'rmdir d' did not remove the empty directory\n");
        return false;
    }

    // rmdir must refuse a directory that still has something in it.
    sh.exec("mkdir box", &mut out);
    sh.exec("cd box", &mut out);
    sh.exec("touch item", &mut out); // put a file inside, so box is not empty
    sh.exec("cd ..", &mut out);
    out.clear();
    sh.exec("rmdir box", &mut out);
    if !out.as_str().contains("not empty") {
        uart::puts("  [fail] 'rmdir box' should refuse a non-empty directory\n");
        return false;
    }

    uart::puts("  [ok] touch / cat / rm / rmdir all work on the filesystem\n");
    true
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    uart::puts("OSLINGS:FAIL (panic)\n");
    testdev::exit_failure(1);
}
