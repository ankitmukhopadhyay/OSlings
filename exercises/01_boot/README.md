# 01 · Boot

> **Learn → Understand → Implement.** After this exercise, your kernel boots
> on (virtual) hardware and prints to a real serial port.

## Learn

In exercise 00 the crate compiled, but nothing ran. Now we boot.

### What "boot" means here

We start QEMU with `-bios none`, which means **there is no firmware** — no
SBI, no bootloader. QEMU loads our kernel ELF into RAM and jumps directly to
the entry symbol our linker script declares (`_entry`) at the RAM base address
`0x8000_0000`. We are the very first software on the machine.

### The stack problem

The first instruction runs with the CPU in a near-pristine reset state. In
particular, **the stack pointer `sp` does not point at anything usable.** Rust
(like C) needs a stack for local variables, function calls, and saved
registers. So before we can call a single Rust function, we must, in raw
assembly:

1. point `sp` at memory we reserved for a stack, and
2. `call` into Rust.

That tiny assembly shim is the **boot trampoline**, in `entry.rs`.

```rust
#[no_mangle]
#[link_section = ".entry"]
pub unsafe extern "C" fn _entry() -> ! {
    asm!(
        "la sp, {stack}",
        "li t0, {size}",
        "add sp, sp, t0",
        "call kmain",
        stack = sym STACK0,
        size = const STACK_SIZE,
        options(noreturn),
    );
}
```

New Rust pieces:

- **`asm!`** — inline assembly. `{stack}` / `{size}` are *named operands*
  filled in by the operands listed at the end. `sym STACK0` substitutes a
  symbol's address; `const STACK_SIZE` substitutes a compile-time constant.
- **`options(noreturn)`** — promises control never falls out of the `asm!`
  block (we hand off to `kmain`, which itself returns `!`). Required because
  the function's return type is `!`.
- **`#[link_section = ".entry"]`** — places this function in the `.entry`
  section, which `kernel.ld` puts *first*, guaranteeing `_entry` sits exactly
  at `0x8000_0000` where QEMU jumps.
- **`static mut STACK0`** — the stack itself, a big byte array living in
  `.bss`. We point `sp` at its top because the stack grows **downward**.

### Talking to hardware: MMIO

Once in `kmain`, how do we print? There is no `println!`. Devices are
controlled through **memory-mapped I/O (MMIO)**: specific physical addresses
are wired to device registers. Writing a byte to the UART's transmit register
at `0x1000_0000` sends that byte out the serial line. See `uart.rs` — the
whole driver is one `write_volatile`.

`volatile` matters: it forbids the compiler from optimizing away or reordering
the store, because the "memory" here is actually a device with side effects.

### Ending the test

`testdev.rs` writes a magic value to QEMU's SiFive *test finisher* at
`0x10_0000`, which makes QEMU power off. That is how the OSlings harness knows
the run is over and can read the captured serial output.

## Understand

Read, in order: `rv6/src/entry.rs`, `rv6/src/uart.rs`, `rv6/src/testdev.rs`,
`rv6/src/main.rs`. Trace the control flow:

```
QEMU → _entry (asm: set sp) → kmain (Rust) → uart::puts → testdev::exit_success → QEMU exits
```

## Implement

1. In `rv6/src/entry.rs`, fill in the four assembly instructions in the
   `asm!` block, and uncomment the two operand lines (`stack = sym STACK0,`
   and `size = const STACK_SIZE,`).
2. In `rv6/src/main.rs`, make `kmain` print the line `OSLINGS:PASS`.

Check your work:

```sh
oslings run 01_boot
# or
oslings watch
```

This exercise boots the kernel in QEMU and passes when it sees `OSLINGS:PASS`
on the serial console. If the trampoline is wrong, the kernel typically
faults silently and the run times out — that is your signal that `sp` never
got set up correctly.

Stuck? `oslings hint`.
