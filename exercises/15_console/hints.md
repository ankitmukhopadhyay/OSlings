# Hints — 15 Console

## Hint 1
`intr` is the handler called when a device interrupt arrives. It follows the
universal three-step pattern: **claim** the interrupt from the PLIC, **service**
the device, then **complete** it. The helpers are all written for you:
`plic::claim()`, `plic::complete(irq)`, `plic::UART0_IRQ`, `uart::getc()` (reads
one byte if available), and `push(b)` (adds a byte to the input buffer).

If the test says the byte wasn't buffered, you're not reading the UART and
pushing it. Remember: you *must* read the byte, or the UART keeps interrupting
forever.

## Hint 2
The shape:

```rust
let irq = plic::claim();          // which device?
if irq == plic::UART0_IRQ {
    // read every byte the UART has and buffer it
    while let Some(b) = uart::getc() {
        push(b);
    }
}
if irq != 0 {
    plic::complete(irq);          // done — let the PLIC send the next one
}
```

The `while let` loop matters: a UART FIFO can hold several bytes, so drain them
all, not just one.

## Hint 3
Full `intr`:

```rust
pub fn intr() {
    let irq = plic::claim();
    if irq == plic::UART0_IRQ {
        while let Some(b) = uart::getc() {
            push(b);
        }
    }
    if irq != 0 {
        plic::complete(irq);
    }
}
```

Why it works: when a key is pressed, the UART raises its interrupt, the PLIC
delivers a supervisor external interrupt, and `kerneltrap` calls `intr`. You ask
the PLIC who it was (`claim`), read the byte out of the UART (which clears its
"data ready" line so it stops interrupting) and push it into the ring buffer, and
then `complete` so the PLIC will deliver the next one. A reader calling
`console::getc` then pops that byte out of the buffer — which is exactly how the
interactive `cargo run` console echoes your typing.
