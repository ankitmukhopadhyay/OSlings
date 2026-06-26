# Hints — 13 Traps

## Hint 1
Two small functions, both using CSR instructions through `asm!`:

* `init` installs the trap vector: write the address `vector_addr()` into the
  `stvec` register. After this, every trap jumps to `kernelvec`.
* `kerneltrap` runs on each trap. Read `scause` to see why you trapped. A
  breakpoint is cause `3`. Handle it by counting it and moving `sepc` past the
  instruction.

If the check says "stvec is not pointing at the trap vector", `init` isn't
writing `stvec`. If the run **times out**, `kerneltrap` isn't advancing `sepc`,
so the same `ebreak` traps over and over.

## Hint 2
`init`:

```rust
let addr = vector_addr();
asm!("csrw stvec, {}", in(reg) addr);
```

`kerneltrap`: read the cause and the faulting address, then, for a breakpoint,
count it and step past the (4-byte) `ebreak`:

```rust
let scause: usize;
let sepc: usize;
asm!("csrr {}, scause", out(reg) scause);
asm!("csrr {}, sepc",   out(reg) sepc);

if scause == 3 {
    TRAP_COUNT += 1;
    asm!("csrw sepc, {}", in(reg) sepc + 4);
}
```

## Hint 3
Full bodies:

```rust
pub unsafe fn init() {
    let addr = vector_addr();
    asm!("csrw stvec, {}", in(reg) addr);
}

#[no_mangle]
pub extern "C" fn kerneltrap() {
    unsafe {
        let scause: usize;
        let sepc: usize;
        asm!("csrr {}, scause", out(reg) scause);
        asm!("csrr {}, sepc", out(reg) sepc);

        if scause == 3 {
            TRAP_COUNT += 1;
            asm!("csrw sepc, {}", in(reg) sepc + 4);
        }
    }
}
```

Why it works: `init` makes the hardware jump to `kernelvec` on any trap;
`kernelvec` (given) saves registers and calls `kerneltrap`. Your handler sees
`scause == 3` (breakpoint), records it, and sets `sepc` to the instruction
*after* the `ebreak`. When `kernelvec` runs `sret`, execution resumes there, so
the test's counter goes from `before` to `before + 1` and the kernel keeps
running.
