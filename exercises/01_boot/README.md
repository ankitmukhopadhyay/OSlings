# 01 · Boot

> **Learn → Understand → Implement.** After this exercise, your kernel boots
> on (virtual) hardware and prints to a real serial port.

## Learn

In exercise 00 the crate compiled, but nothing ran. Now we make it *boot* —
that is, we get our code running on the (virtual) machine from the very first
instruction. If you've never done OS work before, this section starts from the
absolute basics. Take it slowly; nothing here is assumed.

### A 60-second model of the machine

- The **CPU** is the chip that executes **instructions** (tiny commands like
  "add these two numbers" or "jump to this location"). It runs them one after
  another, forever.
- Inside the CPU are **registers**: a handful of ultra-fast storage slots, each
  holding one number. RISC-V (our CPU architecture) has registers with names
  like `sp`, `t0`, `a0`. Think of them as the CPU's scratch variables.
- **Memory (RAM)** is a giant array of bytes. Each byte has a numbered position
  called its **address**. "Address `0x8000_0000`" just means "byte number
  `0x8000_0000` in that array".

#### Reading those addresses

Addresses are written in **hexadecimal** (base 16), marked by the `0x` prefix.
Hex is used because it lines up neatly with how hardware groups bits. The
underscores (`0x8000_0000`) are just digit separators for readability, like
commas in `2,147,483,648`. In fact `0x8000_0000` *is* 2,147,483,648 — i.e. the
2 GiB mark. You don't need to do the math; just know that each such address is a
specific, fixed location.

### The memory map: every address has a meaning

Here is the crucial idea for this exercise. On this machine, **RAM and hardware
devices live in the *same* address space.** Some address ranges are real memory;
others are wired directly to a device. Reading or writing those device addresses
talks to the device instead of to memory. Which address means what is fixed by
the board's design — for us, QEMU's `virt` ("virtual") machine. It's the
equivalent of a datasheet:

| Address        | What lives there                          | We use it for          |
|----------------|-------------------------------------------|------------------------|
| `0x0010_0000`  | "test finisher" device                    | power off when done    |
| `0x1000_0000`  | UART (serial port) registers              | printing text          |
| `0x8000_0000`  | the start of normal RAM                   | our kernel + its stack |

So when you see a "magic" address later, it isn't magic — it's just an entry in
this map. `0x8000_0000` matters because **that's where usable RAM begins** on
this board (everything below it is reserved for devices and boot ROM).
`0x1000_0000` matters because **that's where the serial port is wired**.

### What "boot" means here

When a real computer powers on, it doesn't run your program first. It runs
**firmware** — built-in startup software (on a PC this is the **BIOS/UEFI**; on
RISC-V there's a standard firmware interface called **SBI**). Firmware does
early hardware setup and then loads a **bootloader**, which finally loads the
operating system. That's a lot of layers.

We skip all of them. We launch QEMU with the flag `-bios none`, which means
**there is no firmware and no bootloader — our kernel *is* the first software
that runs.** QEMU loads our compiled kernel into RAM and jumps straight to it.

A couple of supporting terms:

- **ELF** — the standard file format for a compiled program (our kernel binary
  is an ELF file). It contains the machine code plus a map of where each piece
  should go in memory.
- **Linker script** (`kernel.ld`) — a small file that tells the linker *exactly*
  where in memory to place our code and data. We use it to place our code at
  `0x8000_0000` and to name the **entry point**.
- **Entry point / entry symbol** — the single instruction the machine jumps to
  first. Ours is the function named `_entry`. (A **symbol** is just a name the
  linker attaches to an address, like a labeled bookmark.)
- **Section** — compiled programs are split into named chunks called sections
  (code, read-only data, zeroed data, …). The linker script arranges them. We
  put `_entry` into a section called `.entry` and tell the linker to place that
  section *first*, so `_entry` lands exactly at `0x8000_0000` — the address QEMU
  jumps to.

Put together: **QEMU jumps to `0x8000_0000`; our linker script guarantees
`_entry` is sitting there; so `_entry` is the very first thing that runs.**

### The stack problem (and what a "trampoline" is)

There's a catch. When the CPU first starts, it's in a bare **reset state** —
most registers hold garbage. One of them, the **stack pointer** (`sp`), is
especially important.

- The **stack** is a region of memory that running code uses for its temporary
  bookkeeping: local variables, the return addresses of function calls, saved
  registers. Practically *every* function needs it.
- `sp` is the register that holds the address of the top of that stack. Rust
  (like C) generates code that constantly reads and updates `sp`.

At reset, `sp` points at nothing usable. So if we jumped straight into a Rust
function, it would immediately try to use a broken stack and crash. We have to
fix `sp` *before* any Rust code runs — and the only thing that runs before Rust
is a few hand-written assembly instructions.

That little assembly stub is called a **trampoline**. The name is a metaphor:
it's a tiny piece of code whose only purpose is to *bounce* execution from one
environment into another. Here it bounces us from "bare CPU at reset" into
"proper Rust with a working stack". You land on it for an instant and
immediately spring into the real code. The trampoline lives in `entry.rs`:

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

Reading those four assembly instructions in plain English:

- `la sp, {stack}` — **l**oad **a**ddress: put the address of our reserved stack
  memory (`STACK0`) into `sp`.
- `li t0, {size}` — **l**oad **i**mmediate: put the constant stack size into the
  scratch register `t0`. ("Immediate" just means a fixed number baked into the
  instruction.)
- `add sp, sp, t0` — set `sp = sp + t0`. This moves `sp` from the *bottom* of
  our stack memory to the *top*. We do this because **the stack grows downward**:
  as code uses the stack, `sp` moves to lower addresses, so it must *start* at
  the highest address of the region.
- `call kmain` — jump into our first real Rust function, `kmain`, leaving a
  return address behind. From here on, Rust takes over.

New Rust pieces used above:

- **`asm!`** — inline assembly: it lets us write raw CPU instructions inside
  Rust. `{stack}` / `{size}` are *named operands* (placeholders) filled in by
  the lines at the end. `sym STACK0` substitutes a symbol's address; `const
  STACK_SIZE` substitutes a compile-time constant.
- **`options(noreturn)`** — promises that control never falls out the bottom of
  the `asm!` block (we hand off to `kmain`, which never returns). Required
  because the function's return type is `!` (the "never returns" type from
  exercise 00).
- **`#[link_section = ".entry"]`** — places this function in the `.entry`
  section, which `kernel.ld` puts *first*, guaranteeing `_entry` sits exactly at
  `0x8000_0000` where QEMU jumps (the mechanism described above).
- **`static mut STACK0`** — the stack memory itself: a large byte array reserved
  in the kernel. `sp` is pointed at its top.

### Talking to hardware: MMIO

Once we're in `kmain`, we want to print. But there's no `println!` — that's a
standard-library feature that depends on an operating system, and *we are the
operating system*. So how does printing physically work?

Through **memory-mapped I/O (MMIO)**. Recall the memory map: certain addresses
aren't RAM, they're wired to a device's control knobs, called **registers**.
Writing a value to one of those addresses doesn't store data — it *operates the
device*.

The device we use is the **UART** (a serial port — the classic chip that sends
text out one bit at a time over a wire). On the `virt` machine its registers
start at `0x1000_0000`. The very first register is the **transmit register**:
**write one byte there and the UART sends that byte out the serial line.** Under
QEMU's `-nographic -serial mon:stdio` flags, "the serial line" is simply your
terminal — so that byte appears as a character on your screen. That's the entire
trick behind printing in a kernel. Look at `uart.rs`: the whole driver is one
write to `0x1000_0000`.

One subtlety: we use a **`volatile`** write (`write_volatile`). Normally the
compiler is free to optimize away or reorder memory writes it thinks are
pointless (e.g. "you wrote here but never read it back — I'll delete that").
For a device that would be a disaster: the write *is* the point. `volatile`
tells the compiler "this access has a side effect — perform it exactly as
written, don't remove or reorder it."

### Ending the test

Finally, how does the program stop? `testdev.rs` writes a special value to the
**test finisher** device at `0x10_0000` (again, just an entry in the memory
map). That device tells QEMU to power off. This is how the OSlings test harness
knows the run is over, so it can read the serial output it captured and decide
whether you passed.

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
