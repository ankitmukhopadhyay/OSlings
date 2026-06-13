# Hints — 01 Boot

## Hint 1
Two separate places need work:

- `main.rs`: `kmain` already prints "rv6 is booting..." but not the marker the
  harness wants. Add one more `uart::puts(...)` call.
- `entry.rs`: the `asm!` block is empty, so `sp` is never set and the `call`
  never happens — the kernel faults before reaching `kmain`. If the run times
  out with no output at all, this is why.

## Hint 2
For `main.rs`, the harness greps for the exact line `OSLINGS:PASS`. So:

```rust
uart::puts("OSLINGS:PASS\n");
```

For `entry.rs`, you need four instructions and you must enable the two named
operands. The instructions, in order: load the stack address into `sp`, load
the size into a scratch register, add them, then call `kmain`. Don't forget to
uncomment `stack = sym STACK0,` and `size = const STACK_SIZE,`.

## Hint 3
Complete `entry.rs` like this:

```rust
asm!(
    "la sp, {stack}",
    "li t0, {size}",
    "add sp, sp, t0",
    "call kmain",
    stack = sym STACK0,
    size = const STACK_SIZE,
    options(noreturn),
);
```

And in `main.rs`, the body of `kmain` becomes:

```rust
uart::puts("\nrv6 is booting...\n");
uart::puts("OSLINGS:PASS\n");
testdev::exit_success();
```

`la` = load address, `li` = load immediate, and the stack grows downward so we
add the size to reach the top before using it.
