# 11 · Devices

> **Learn → Understand → Implement.** The capstone: turn the blind-write UART
> into a *real* device driver that talks to the hardware through its registers.
> You consolidate everything about MMIO, status flags, and `volatile`.

## Learn

Way back in exercise 01 we "printed" by storing a byte to the UART's transmit
address and assuming the chip was ready. That works in QEMU at boot, but it
isn't how you drive real hardware. A proper **device driver** communicates with
a device through its **registers** and checks the device's *status* before each
transfer. This final exercise builds that driver — and the technique is the same
for *any* memory-mapped device.

### Device registers

A hardware device exposes a set of small control/status slots called
**registers**, mapped into the physical address space (MMIO — memory-mapped I/O,
from exercise 01). The UART (a "Universal Asynchronous Receiver/Transmitter" —
the serial port) is an **NS16550A**, and several of its one-byte registers sit at
consecutive addresses starting at `UART0` (`0x1000_0000`):

| Offset | Register | Meaning |
|---|---|---|
| 0 | **RBR** / **THR** | received byte (read) / byte to transmit (write) |
| 1 | IER | interrupt enable |
| 2 | FCR | FIFO control |
| 3 | LCR | line control (word length, etc.) |
| 4 | MCR | modem control (incl. loopback) |
| 5 | **LSR** | **line status** — the chip's current state |

Reading offset 0 gives you an *incoming* byte; writing offset 0 *sends* one. The
same address means different things for read vs. write — normal for hardware.

### The status register and "flags"

The key to a correct driver is the **Line Status Register (LSR)**. Its
individual **bits** ("flags") report what the chip is doing. Two matter here:

- **`THRE`** (bit 5, "Transmit Holding Register Empty") — set when the
  transmitter has room for another byte. You must wait for this before writing
  `THR`, or you'd overwrite a byte still being sent.
- **`DR`** (bit 0, "Data Ready") — set when a received byte is waiting in `RBR`.
  You check this before reading, so you don't read stale/garbage data.

Testing a flag is a bitwise AND with a mask: `lsr & THRE != 0`. This is the
universal pattern for reading device state — a status register full of
single-bit flags. (We used the same idea for the PTE flags in exercise 03 and
the lock bit in exercise 07.)

So the driver becomes:

- **`tx_ready()`** = is `LSR & THRE` set?
- **`rx_ready()`** = is `LSR & DR` set?
- **`putc(c)`** = spin until `tx_ready()`, then write `c` to `THR`.
- **`getc()`** = if `rx_ready()`, read and return `RBR`, else `None` (returning
  an `Option` because there may be nothing to read — `Result`/`Option` error
  habits from exercise 10).

`init()` (given) sets 8-bit words, turns interrupts off (we *poll* the status
flags instead), and enables the FIFOs.

### `volatile`, one more time

Every register access uses `read_volatile`/`write_volatile`. As in exercise 01,
this forbids the compiler from caching, reordering, or eliminating the access —
essential when the "memory" is a device whose value changes on its own and whose
reads/writes have side effects.

### Testing without a keyboard: loopback

There's no one typing into the automated test, so how do we test `getc`? The
UART has a **loopback** mode (a bit in MCR): while it's on, every byte the
transmitter sends is wired straight back into the receiver. So the test turns
loopback on, `putc`s a byte, and expects `getc` to hand the *same* byte back —
exercising `tx_ready`, `putc`, `rx_ready`, and `getc` together, end-to-end.

> Heads-up on this harness: it reports its own results through a tiny
> blind-write console (`dbg_*` in `main.rs`), *not* through the driver you're
> writing — because a half-finished driver might hang or print nothing. Your
> driver is the thing under test, not the thing doing the reporting.

## Understand

Read `rv6/src/uart.rs`: the register offsets and LSR/MCR bit constants, the
`reg_read`/`reg_write` MMIO helpers, and the given `init`/`set_loopback`/`puts`.
Then read `rv6/src/main.rs`: it inits the UART, checks `tx_ready` is set and
`rx_ready`/`getc` report "nothing yet", then uses loopback to confirm a byte sent
with `putc` comes back through `getc`.

## Implement

In `rv6/src/uart.rs`:

1. **`tx_ready`** — `LSR & THRE` is set.
2. **`rx_ready`** — `LSR & DR` is set.
3. **`putc`** — spin until `tx_ready()`, then write the byte to `THR`.
4. **`getc`** — if `rx_ready()`, return `Some(RBR)`, else `None`.

Check your work:

```sh
oslings run 11_devices
# or
oslings watch
```

Passes when the status flags read correctly and a byte survives the loopback
round-trip — printing `OSLINGS:PASS`.

## 🎉 That's the whole kernel

If this passes, you've built rv6 end to end: it boots on bare metal, manages
physical memory, walks page tables and runs with the MMU on, creates and
schedules processes with real context switches, synchronizes with spinlocks and
semaphores over a heap, keeps a filesystem of inodes and directories, and drives
a hardware device. Every line had a reason. Nicely done.

Stuck? `oslings hint`.
