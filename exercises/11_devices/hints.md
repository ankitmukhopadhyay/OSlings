# Hints — 11 Devices

## Hint 1
All four functions read or write the UART's registers with the given
`reg_read(offset)` / `reg_write(offset, value)` helpers. The constants are
already defined for you: `LSR`, `THR`, `RBR`, and the bit masks `LSR_THRE`
(transmitter ready) and `LSR_DR` (a byte is waiting).

- `tx_ready` / `rx_ready` just test a bit in the Line Status Register.
- `putc` waits for `tx_ready`, then writes.
- `getc` checks `rx_ready`, then reads.

If the test says "tx_ready() is false right after init", you're still returning
the placeholder `false`.

## Hint 2
Testing a flag is a bitwise AND against its mask:

```rust
pub fn tx_ready() -> bool {
    unsafe { reg_read(LSR) & LSR_THRE != 0 }
}
pub fn rx_ready() -> bool {
    unsafe { reg_read(LSR) & LSR_DR != 0 }
}
```

`putc` must not write until the transmitter is empty, and `getc` must not read
unless a byte is there:

```rust
pub fn putc(c: u8) {
    while !tx_ready() {}
    unsafe { reg_write(THR, c) }
}
```

## Hint 3
Full implementations:

```rust
pub fn tx_ready() -> bool {
    unsafe { reg_read(LSR) & LSR_THRE != 0 }
}

pub fn rx_ready() -> bool {
    unsafe { reg_read(LSR) & LSR_DR != 0 }
}

pub fn putc(c: u8) {
    while !tx_ready() {}
    unsafe { reg_write(THR, c) }
}

pub fn getc() -> Option<u8> {
    if rx_ready() {
        Some(unsafe { reg_read(RBR) })
    } else {
        None
    }
}
```

Why it passes: after `init`, the transmitter is empty so `tx_ready()` is true and
`rx_ready()` is false (nothing received yet). With loopback on, `putc(0x42)`
writes `THR`, the chip feeds it back into `RBR` and sets `DR`, so the next
`getc()` sees `rx_ready()` and returns `Some(0x42)`. The `while !tx_ready()` spin
is what makes `putc` safe on real hardware — it never clobbers a byte that's
still going out.
