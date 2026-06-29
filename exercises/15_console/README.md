# 15 · Console

> **Learn → Understand → Implement.** You'll handle a **device interrupt** — the
> UART announcing that a key was pressed — routed through the PLIC. After this,
> the OS can finally *read what you type*.

## Learn

Exercise 14's timer was an interrupt built into the CPU. This exercise handles a
*device* interrupt: the **UART** (the serial port) telling the kernel "a byte
just arrived." Getting that working is what lets the kernel read keyboard input
without constantly checking — and it's the same pattern you'd use for any
device (a disk, a network card, ...).

### Polling vs. interrupts

Back in exercise 11 we *polled* the UART: to read a byte, we sat in a loop
checking the "data ready" bit. That wastes the CPU and means the kernel can only
read when it happens to be looking. The better way is **interrupt-driven** input:
the UART raises an interrupt the moment a byte arrives, the kernel handles it
right then (stashing the byte in a buffer), and the rest of the time the CPU is
free to do other work (or sleep).

### The PLIC: routing device interrupts

Device interrupts don't go straight to the CPU. They pass through the **PLIC**
(Platform-Level Interrupt Controller), a piece of hardware that collects every
device's interrupt line and decides which one to deliver to which CPU. To use a
device's interrupt you configure the PLIC (give the source a priority, enable it
for your hart, set your acceptance threshold) — all done for you in `plic.rs`.

When a device interrupt is delivered, it arrives as a **supervisor external
interrupt** (`scause` low bits = `9`), through the same trap path you built
earlier. The given `kerneltrap` recognizes it and calls `console::intr`.

### Handling a device interrupt: claim → read → complete

Inside the handler, the standard three-step dance is:

1. **Claim** — ask the PLIC *which* device interrupted: `plic::claim()` returns
   the source number (or `0` for none).
2. **Service** — if it's the UART (`plic::UART0_IRQ`), read the waiting byte(s)
   out of the UART and stash them somewhere. **You must actually read the
   byte.** The UART keeps its interrupt line raised as long as a byte sits
   unread, so if you don't read it, the interrupt fires again and again — an
   **interrupt storm** that locks the kernel up.
3. **Complete** — tell the PLIC you're done (`plic::complete(irq)`), so it can
   deliver the next interrupt.

### Where the bytes go: a ring buffer

Received bytes are stashed in a small **ring buffer** (a fixed array with a
*head* index for the reader and a *tail* index for the writer; both wrap around
the end). The interrupt handler is the *producer* (it pushes bytes in); code that
wants input is the *consumer* (it pops bytes out). With one producer and one
consumer on a single CPU, separate head and tail indices make this safe without
a lock. The buffer and its `push`/`try_getc`/`getc` are given; a blocking `getc`
simply waits (with `wfi`) until the interrupt delivers a byte.

### The Rust you need

* **MMIO** through `plic`/`uart` helpers (already written) — you call them.
* `while let Some(b) = uart::getc() { ... }` to drain all available bytes.
* the given `push(b)` to add a byte to the ring buffer.

This handler is short; the lesson is the *shape* — claim, service the device,
complete — which is how every device driver's interrupt handler is built.

## Understand

Read `rv6/src/plic.rs` (the PLIC setup and `claim`/`complete`), the new bits of
`rv6/src/uart.rs` (`enable_rx_interrupt`) and `rv6/src/vm.rs` (the PLIC region is
now mapped so the kernel can reach it with paging on), and `rv6/src/trap.rs`
(the external-interrupt case calling `console::intr`). Then `rv6/src/console.rs`:
the ring buffer, the given `init`/`try_getc`/`getc`, and the one function you
write, `intr`. Finally `rv6/src/main.rs`: the harness loops the UART back to
itself, "types" a byte, invokes your handler, and checks the byte was buffered.

## Implement

In `rv6/src/console.rs`, fill in **`intr`**:

1. `let irq = plic::claim();`
2. if `irq == plic::UART0_IRQ`, drain the UART:
   `while let Some(b) = uart::getc() { push(b); }`
3. if `irq != 0`, `plic::complete(irq);`

Check your work:

```sh
oslings run 15_console
# or
oslings watch
```

It passes when a UART interrupt is claimed, its byte is read, and the byte lands
in the buffer. If it reports the byte wasn't buffered, your handler isn't reading
the UART and pushing it.

And try it live — this is the payoff:

```sh
cd rv6 && cargo run        # boots a console that echoes your typing
```

Type, and watch your keystrokes come back. (Exit QEMU with Ctrl-A then X.)

Stuck? `oslings hint`.
