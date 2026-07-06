#![no_std]
#![no_main]

// ╔══════════════════════════════════════════════════════════════════════╗
// ║  Exercise 18 — User mode                                     PART 2    ║
// ║  Goal: run a program at USER privilege, in its own address space,     ║
// ║        and serve its system calls (write / getpid / exit).            ║
// ╚══════════════════════════════════════════════════════════════════════╝
//
// Until now, everything ran inside the kernel, with full power over the
// machine. This exercise builds the wall every real OS is built around: a
// user program runs at the CPU's LOWEST privilege, inside its own private
// address space, and the ONLY door through the wall is `ecall` — a system
// call. You implement four pieces of that door:
//
//   vm.rs       map_user_pages — the program's memory, with the PTE_U bit
//   usermode.rs usertrap       — the ecall branch: epc+4, dispatch, return
//   syscall.rs  dispatch       — number -> handler routing
//   vm.rs       copyin         — safely reading the user's buffer
//
// Try it live:  `cd rv6 && cargo run`  then type:  run

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
        if user_mode_self_check() {
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
        uart::puts("rv6: starting shell. Try: run  (launches a user program). (Ctrl-A X to quit)\n\n");
        shell::run();
    }
}

/// Print a usize in hex (for diagnostics). (Given, harness only.)
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

/// Check one leaf mapping of the user page table: present, pointing at the
/// right physical page (if `want_pa` is nonzero), with (at least) the given
/// flags set and, where it matters, PTE_U present or absent. (Given.)
#[cfg(feature = "harness")]
unsafe fn check_mapping(
    table: *mut vm::Pte,
    va: usize,
    want_pa: usize,
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
        uart::puts(") — check map_user_pages in vm.rs\n");
        return false;
    }
    if want_pa != 0 && (*pte).pa() != want_pa {
        uart::puts("  [fail] ");
        uart::puts(what);
        uart::puts(" is mapped to the wrong physical page\n");
        return false;
    }
    let flags = (*pte).flags();
    if flags & want_flags != want_flags {
        uart::puts("  [fail] ");
        uart::puts(what);
        uart::puts(" is missing permission bits (has ");
        put_hex(flags);
        uart::puts(", needs at least ");
        put_hex(want_flags | vm::PTE_V);
        uart::puts(")\n");
        return false;
    }
    if want_user && flags & vm::PTE_U == 0 {
        uart::puts("  [fail] ");
        uart::puts(what);
        uart::puts(" has no PTE_U — user mode is not allowed to touch it\n");
        return false;
    }
    if !want_user && flags & vm::PTE_U != 0 {
        uart::puts("  [fail] ");
        uart::puts(what);
        uart::puts(" must NOT have PTE_U (it belongs to the kernel)\n");
        return false;
    }
    true
}

/// Build the user process, verify its address space BEFORE running it (a
/// wrong page table would fault or hang, which is miserable to debug), then
/// run it and check everything it did.
#[cfg(feature = "harness")]
fn user_mode_self_check() -> bool {
    use memlayout::{TRAMPOLINE, TRAPFRAME, USER_CODE, USER_STACK};

    unsafe {
        let p = usermode::setup();
        if p.is_null() {
            uart::puts("  [fail] could not allocate the user process\n");
            return false;
        }
        let pt = (*p).pagetable;

        // ---- stage 1: the user address space, checked while the MMU is
        //      not looking (same verify-before-use idea as exercise 09) ----
        uart::puts("  checking the user address space...\n");
        if !check_mapping(pt, USER_CODE, 0, vm::PTE_R | vm::PTE_X, true, "the code page") {
            return false;
        }
        if !check_mapping(pt, USER_STACK, 0, vm::PTE_R | vm::PTE_W, true, "the stack page") {
            return false;
        }
        if !check_mapping(
            pt,
            TRAMPOLINE,
            vm::trampoline_page(),
            vm::PTE_R | vm::PTE_X,
            false,
            "the trampoline",
        ) {
            return false;
        }
        if !check_mapping(
            pt,
            TRAPFRAME,
            (*p).trapframe as usize,
            vm::PTE_R | vm::PTE_W,
            false,
            "the trapframe",
        ) {
            return false;
        }
        uart::puts("  [ok] code + stack mapped for user, trampoline + trapframe for the kernel\n");

        // ---- stage 2: drop to user mode and let the program run ----
        uart::puts("  running the user program...\n");
        let outcome = usermode::run(p);
        proc::freeproc(p);

        if !usermode::came_from_user() {
            uart::puts("  [fail] no trap ever arrived from user mode (sstatus.SPP was never 0)\n");
            return false;
        }

        let status = match outcome {
            usermode::RunOutcome::TimedOut => {
                uart::puts("  [fail] the program ran, but its system calls were never answered\n");
                uart::puts("         (did usertrap's ecall branch and syscall::dispatch run?)\n");
                return false;
            }
            usermode::RunOutcome::Faulted(scause) => {
                uart::puts("  [fail] the program faulted (scause ");
                put_hex(scause);
                uart::puts(") instead of finishing\n");
                return false;
            }
            usermode::RunOutcome::Exited(status) => status,
        };

        let said = syscall::captured();
        if said != "hello from user mode\n" {
            uart::puts("  [fail] the write syscall did not deliver the program's message —\n");
            uart::puts("         expected \"hello from user mode\", got \"");
            uart::puts(said);
            uart::puts("\" (check copyin in vm.rs)\n");
            return false;
        }
        uart::puts("  [ok] the program's write() arrived: \"hello from user mode\"\n");

        if status != 42 {
            uart::puts("  [fail] exit status was ");
            put_num(status);
            uart::puts(", expected 42 (= pid 1 + 41; do return values reach a0?)\n");
            return false;
        }
        uart::puts("  [ok] getpid() returned 1 and exit(42) came back through the wall\n");
    }

    uart::puts("  [ok] a user program ran at user privilege and the kernel served it\n");
    true
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    uart::puts("OSLINGS:FAIL (panic)\n");
    testdev::exit_failure(1);
}
