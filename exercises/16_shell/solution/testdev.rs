//! testdev.rs — power off the virtual machine. (UNDERSTAND, don't edit.)
//!
//! QEMU's `virt` machine includes a "SiFive test finisher" device at physical
//! address 0x10_0000. Writing a magic value to it makes QEMU exit — which is
//! exactly what an automated test harness needs: a way for the guest kernel to
//! say "I'm done, here's my result" and have the emulator quit.

use core::ptr::write_volatile;

/// MMIO address of the SiFive test finisher on the `virt` machine.
const TEST_FINISHER: *mut u32 = 0x10_0000 as *mut u32;

const FINISHER_PASS: u32 = 0x5555;
const FINISHER_FAIL: u32 = 0x3333;

/// Power off QEMU reporting success (exit status 0).
pub fn exit_success() -> ! {
    unsafe {
        write_volatile(TEST_FINISHER, FINISHER_PASS);
    }
    // If, somehow, QEMU did not exit, don't fall off the end of the world.
    loop {
        core::hint::spin_loop();
    }
}

/// Power off QEMU reporting failure. `code` is folded into QEMU's exit status.
pub fn exit_failure(code: u16) -> ! {
    unsafe {
        write_volatile(TEST_FINISHER, FINISHER_FAIL | ((code as u32) << 16));
    }
    loop {
        core::hint::spin_loop();
    }
}
