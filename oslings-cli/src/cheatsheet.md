# OSlings cheatsheet

A single-page reference for everything rv6 is built from: what every bit means,
what every magic number is, and the concepts behind them. Skim it to jog your
memory, or read a section in full to understand a piece better.

> How to read the bit tables: bit 0 is the **least significant** bit (value 1),
> bit 1 is value 2, bit 2 is value 4, and so on. `1 << n` is "a 1 in bit n".
> `0x` means hexadecimal (base 16); each hex digit is 4 bits.

---

## 1 · Privilege levels

The CPU runs at one of three privilege levels. A trap moves *up*; a special
return instruction moves *down*.

| Level | Who runs there | May do |
|---|---|---|
| **M** machine | `start.rs`, `timervec` | everything; the level QEMU boots into |
| **S** supervisor | the rv6 kernel | CSRs, page tables, take traps |
| **U** user | user programs | ordinary computation only |

- **M → S**: `start` sets `mstatus.MPP = S`, `mepc = kmain`, then `mret`.
- **S → U**: `usertrapret` sets `sstatus.SPP = U`, `sepc = entry`, then `sret`.
- **U/S → higher**: any trap (ecall, interrupt, fault) enters the handler.

---

## 2 · Registers & the calling convention

RISC-V has 32 integer registers, `x0`..`x31`, each with an ABI name and a role.
Who is responsible for preserving a register across a function call is the key
distinction.

| Reg | ABI | Role | Saved by |
|---|---|---|---|
| x0 | zero | always 0 | — |
| x1 | ra | return address | caller |
| x2 | sp | stack pointer | callee |
| x3 | gp | global pointer | — |
| x4 | tp | thread pointer | — |
| x5–x7 | t0–t2 | temporaries | caller |
| x8 | s0/fp | saved / frame ptr | callee |
| x9 | s1 | saved | callee |
| x10–x11 | a0–a1 | args / return values | caller |
| x12–x17 | a2–a7 | args | caller |
| x18–x27 | s2–s11 | saved | callee |
| x28–x31 | t3–t6 | temporaries | caller |

- **Callee-saved** (`sp`, `s0`–`s11`): a function must leave them as it found
  them. These 14 registers are exactly the `Context` that `swtch` saves (ex05).
- **Caller-saved** (`ra`, `t0`–`t6`, `a0`–`a7`): a trap can clobber them, so
  `kernelvec` parks all of them before calling `kerneltrap` (ex13).
- **Program entry** (ex19 `exec`): `a0 = argc`, `a1 = argv`.

---

## 3 · Sv39 virtual memory

Sv39 = 39-bit virtual addresses through a 3-level page table. Pages are 4096
(`0x1000`) bytes.

**A virtual address splits into four fields:**

```
 38      30 29      21 20      12 11         0
+----------+----------+----------+-----------+
| VPN[2]   | VPN[1]   | VPN[0]   |  offset   |
| 9 bits   | 9 bits   | 9 bits   |  12 bits  |
+----------+----------+----------+-----------+
     |          |          |           |
   L2 index   L1 index   L0 index    byte in page
```

`walk()` uses VPN[2] to index the root table, VPN[1] the next, VPN[0] the leaf.
Each table has 512 entries (2⁹) and is itself one page.

**A page table entry (PTE) is 64 bits:**

```
 63    54 53                10 9   8 7 6 5 4 3 2 1 0
+--------+--------------------+-----+-+-+-+-+-+-+-+-+
| unused |   PPN (44 bits)    | RSW |D|A|G|U|X|W|R|V|
+--------+--------------------+-----+-+-+-+-+-+-+-+-+
```

| Bit | Name | Meaning |
|---|---|---|
| 0 | V | **Valid** — this entry is in use |
| 1 | R | **Read** allowed |
| 2 | W | **Write** allowed |
| 3 | X | e**X**ecute allowed |
| 4 | U | **User** mode may access (the wall, ex18) |
| 5 | G | Global mapping |
| 6 | A | Accessed |
| 7 | D | Dirty |
| 8–9 | RSW | reserved for software |
| 10–53 | PPN | physical page number |

- A PTE with R/W/X all 0 is an **interior node** (points at the next table). Any
  of R/W/X set makes it a **leaf**.
- `Pte::new(pa, flags)` = `((pa >> 12) << 10) | flags` — the PPN sits 10 bits up.
- `Pte::pa()` = `(pte >> 10) << 12` — recover the physical address.
- Typical flags: kernel code `R|W|X`, user code `R|X|U`, user stack `R|W|U`,
  trampoline/trapframe `R|X` / `R|W` (no U — kernel-only).

**`satp` — the register that turns on paging:**

```
 63    60 59        44 43              0
+--------+------------+----------------+
| MODE=8 |    ASID    |  root PPN      |
+--------+------------+----------------+
```

`make_satp(root)` = `(8 << 60) | (root >> 12)`. MODE 8 = Sv39. Writing `satp`
then `sfence.vma` installs a page table.

---

## 4 · `scause` — why a trap happened

The top bit says interrupt vs exception; the low bits say which one.

```
 63          62                    0
+---+----------------------------+
| I |         exception code      |
+---+----------------------------+
  I = 1: interrupt   I = 0: exception
```

**Interrupts** (`scause >> 63 == 1`, code = `scause & 0xff`):

| Code | Meaning | rv6 uses it for |
|---|---|---|
| 1 | supervisor **software** | the forwarded timer tick (ex14) |
| 5 | supervisor **timer** | (we forward via software instead) |
| 9 | supervisor **external** | a device via the PLIC (ex15) |

**Exceptions** (`scause >> 63 == 0`):

| Code | Meaning | rv6 uses it for |
|---|---|---|
| 2 | illegal instruction | a faulting user program |
| 3 | **breakpoint** (`ebreak`) | the ex13 trap test |
| 8 | **ecall from U-mode** | every system call (ex18) |
| 12/13/15 | instr/load/store page fault | bad memory access |

On an `ecall`, `sepc` points **at** the ecall, so the handler does `epc += 4`
(instructions are 4 bytes) before returning, or it loops forever.

---

## 5 · Supervisor CSRs (the kernel's control registers)

`sstatus` — supervisor status:

| Bit | Name | Meaning |
|---|---|---|
| 1 | SIE | interrupts enabled **now** (in S-mode) |
| 5 | SPIE | interrupts were enabled before the trap |
| 8 | SPP | previous privilege: 0 = user, 1 = supervisor |

`sret` restores SIE from SPIE and returns to the SPP level. Setting `SPP = 0`
before `sret` is how the kernel drops to user mode.

`sie` / `sip` — interrupt **enable** / **pending**, same bit layout:

| Bit | Name | Source |
|---|---|---|
| 1 | SSIE / SSIP | software (our timer tick) |
| 5 | STIE / STIP | timer |
| 9 | SEIE / SEIP | external (PLIC / devices) |

- `intr_on()` = set `sie.SSIE` (bit 1) + `sstatus.SIE` (bit 1).
- Clear a pending software interrupt: `sip &= ~2` (ex14).

Other S-CSRs: `stvec` (trap vector address), `sepc` (trap PC), `scause`,
`sscratch` (uservec parks the trapframe pointer here), `satp` (§3).

---

## 6 · Machine CSRs (`start.rs`, ex13–14)

Set up once in machine mode, then we live in S-mode.

| CSR | Value we write | Why |
|---|---|---|
| `mstatus.MPP` | `01` (bits 12:11) | `mret` lands in Supervisor |
| `mepc` | `kmain` | where `mret` jumps |
| `medeleg` / `mideleg` | `0xffff` | delegate all traps/interrupts to S |
| `mie.MTIE` | bit 7 | enable the machine timer interrupt |
| `mcounteren` | `0xffffffff` | let S-mode read the `time` CSR |
| `mtvec` | `timervec` | machine timer trap vector |
| `mscratch` | `&TIMER_SCRATCH` | scratch area for timervec |
| `pmpaddr0` | `0x3f_ffff_ffff_ffff` | cover all of physical memory |
| `pmpcfg0` | `0xf` | give S-mode R+W+X to it |

**`mstatus.MPP`** (bits 12:11): `00` = U, `01` = S, `11` = M.

**PMP config byte** (`pmpcfg0` low 8 bits):

| Bit | Name | Meaning |
|---|---|---|
| 0 | R | read |
| 1 | W | write |
| 2 | X | execute |
| 3–4 | A | match mode: 0 off, 1 TOR, 2 NA4, 3 NAPOT |
| 7 | L | lock |

`0xf` = R|W|X + A=1 (TOR: "everything below `pmpaddr0`").

---

## 7 · The trap machinery

```
             kernel code                     user code
                 │                               │
  trap ──▶ stvec = kernelvec            trap ──▶ stvec = uservec (trampoline)
                 │                               │
           save caller regs               park 31 regs in TRAPFRAME
                 │                         switch to kernel satp + stack
           kerneltrap()                          │
           decode scause                     usertrap()
                 │                          decode scause, dispatch syscall
           restore, sret                          │
                                            usertrapret → userret → sret
```

- **kernelvec / kerneltrap** (ex13–15): traps that happen *in* the kernel.
- **uservec / usertrap / userret** (ex18): traps *from* user mode, via the
  **trampoline** — one page mapped at the same VA (`TRAMPOLINE`) in every
  address space, so `satp` can change without the running code vanishing.

---

## 8 · UART — the 16550 serial port (ex11, ex15)

Base `0x1000_0000`. Registers are one byte, at these offsets from the base:

| Off | Read | Write | Name |
|---|---|---|---|
| 0 | RBR | THR | Receive Buffer / Transmit Holding |
| 1 | IER | IER | Interrupt Enable |
| 2 | — | FCR | FIFO Control |
| 3 | LCR | LCR | Line Control |
| 4 | MCR | MCR | Modem Control |
| 5 | LSR | — | Line Status |

**LSR — line status** (poll before reading/writing data):

| Bit | Name | Meaning |
|---|---|---|
| 0 | DR | **Data Ready** — a byte is waiting in RBR |
| 5 | THRE | **Tx Holding Empty** — ok to write THR |

`getc` waits for `LSR & 1` (DR); `putc` waits for `LSR & 0x20` (THRE).

**IER — interrupt enable:** bit 0 = "interrupt when a byte arrives".
`enable_rx_interrupt()` writes `0x01`.

**FCR — FIFO control:** `0x07` = enable FIFO (bit0) + clear Rx FIFO (bit1) +
clear Tx FIFO (bit2).

**LCR — line control:** `0x03` = 8 data bits (bits 1:0 = 11), no parity, 1 stop.

**MCR — modem control:** bit 4 = **loopback** (Tx wired to Rx, used to test the
driver deterministically in ex15).

---

## 9 · PLIC — routing device interrupts (ex15)

Base `0x0c00_0000`. The PLIC collects device IRQs and delivers one to the CPU as
a supervisor **external** interrupt (scause code 9).

| Register | Address | Purpose |
|---|---|---|
| priority | `PLIC + irq*4` | set an IRQ's priority (0 = off) |
| S-enable | `PLIC + 0x2080` | bitmask of enabled sources |
| S-threshold | `PLIC + 0x20_1000` | ignore priorities ≤ this |
| S-claim/complete | `PLIC + 0x20_1004` | read = claim, write = complete |

- **UART0_IRQ = 10.** Enable = set bit 10 of S-enable; priority = write 1 to
  `PLIC + 10*4`.
- **claim** returns the pending IRQ number (0 = none); **complete** writes it
  back when done, or the interrupt re-fires forever.

---

## 10 · CLINT — the timer (ex14)

Base `0x0200_0000`. Drives time in machine mode.

| Register | Address | Meaning |
|---|---|---|
| `mtime` | `0x0200_bff8` | current time, ever-increasing |
| `mtimecmp0` | `0x0200_4000` | hart 0's alarm: interrupt when mtime ≥ this |

`timervec` (M-mode) fires on each timer interrupt, sets `mtimecmp += interval`
to schedule the next, and raises `sip.SSIP` to forward a tick to S-mode. The
`time` CSR (readable in S-mode thanks to `mcounteren`) gives wall-clock bounds
for deterministic tests.

---

## 11 · System calls (ex18–19)

A syscall is a function call across the privilege wall, via `ecall`.

| Register | Role |
|---|---|
| a7 | which call (the number) |
| a0, a1, a2 | arguments |
| a0 (after) | return value (−1 = error) |

xv6 numbers (rv6 grows into these):

| # | Call | Status |
|---|---|---|
| 2 | exit(status) | ex18 |
| 11 | getpid() | ex18 |
| 16 | write(fd, buf, len) | ex18 |
| 1 | fork() | ex20 (planned) |
| 3 | wait() | ex20 (planned) |
| 5 | read(fd, buf, len) | ex20 (planned) |
| 15 | open(path, flags) | ex20 (planned) |

A user pointer (like `write`'s `buf`) is a **user** virtual address — the kernel
must translate it page by page with `copyin` / `copyout`, never dereference it
directly.

---

## 12 · Physical memory map (QEMU `virt`)

| Address | What |
|---|---|
| `0x0000_0000` | user code (base of every user address space) |
| `0x0010_0000` | SiFive test finisher (power off / exit QEMU) |
| `0x0200_0000` | CLINT (timer) |
| `0x0c00_0000` | PLIC |
| `0x1000_0000` | UART0 |
| `0x8000_0000` | KERNBASE — RAM begins; the kernel loads here |
| `0x8800_0000` | PHYSTOP — end of 128 MiB RAM |

---

## 13 · User address space (ex18–19)

Each process has its own page table; from user code, only these exist:

| Virtual address | Contents | Perms |
|---|---|---|
| `MAXVA − 0x1000` = TRAMPOLINE | uservec / userret | R X (no U) |
| TRAMPOLINE − 0x1000 = TRAPFRAME | saved user registers | R W (no U) |
| `0x1_1000` | initial stack pointer | — |
| `0x1_0000` USER_STACK | the stack page | R W U |
| `0x0` USER_CODE .. | the program image (1..16 pages) | R X U |

`MAXVA = 1 << 38` (one bit short of Sv39's 39, so no address is sign-extended).

**argv on the stack** (ex19): strings pushed at the top, then an array of user
pointers to them (null-terminated) below; `sp`/`a1` point at the array,
`a0 = argc`.

---

## 14 · Key kernel data structures

| Struct | File | What |
|---|---|---|
| `Pte` | vm.rs | one page-table entry (§3) |
| `Context` | swtch.rs | 14 callee-saved regs for `swtch` (ra, sp, s0–s11) |
| `Trapframe` | usermode.rs | all 31 user regs + kernel notes, offsets 0..280 |
| `Proc` | proc.rs | a process: state, pid, pagetable, context, trapframe, kstack |
| `Inode` | fs.rs | one file/dir: kind, size, data, entries |

**ProcState:** Unused → Runnable → Running → Sleeping / Zombie.

---

## 15 · In-memory filesystem (ex10, ex16–17)

| Constant | Value | Meaning |
|---|---|---|
| ROOT | 1 | inode number of `/` |
| NINODE | 64 | total inodes |
| NDIRENT | 16 | entries per directory |
| NAMELEN | 14 | max filename length |
| FILESIZE | 128 | max bytes per file |

- **inode**: the record for one file or directory (kind, size, bytes).
- **inum**: the integer that names an inode; `dirlookup` turns a name into one.
- **directory entry**: a (name → inum) pair stored inside a directory.
- Methods: `dirlookup`, `dircreate`, `read`, `write`, `unlink`, `is_dir`,
  `dir_is_empty`, `for_each_entry`.

---

## 16 · Rust in a kernel (`no_std`)

Concepts introduced, and where:

| Concept | First seen | The idea |
|---|---|---|
| `#![no_std]` / `no_main` | ex00 | no OS underneath; provide your own entry + panic |
| raw pointers, `unsafe` | ex02 | you promise the compiler a deref is valid |
| `#[repr(C)]` / `transparent` | ex03/05 | a struct's layout matches hardware exactly |
| `const fn` | ex03 | compute at compile time (bit-packing a PTE) |
| enums | ex04 | one type, a fixed set of variants (`ProcState`) |
| ownership "by hand" | ex04 | each `Proc` owns its page table |
| `global_asm!` / `asm!` | ex05 | drop to assembly for `swtch`, the trampoline |
| `AtomicBool`, `UnsafeCell` | ex07 | a `SpinLock` from the ground up |
| traits | ex06/16 | shared behaviour (`Scheduler`, `Out`) |
| `GlobalAlloc` | ex08 | turns on `Box`/`Vec`/`Arc` (the heap) |
| `Result` + `?` | ex10 | recoverable errors, propagated concisely |
| `addr_of!` / `addr_of_mut!` | ex04+ | a pointer to a `static mut` without a reference |
| `&str` / `&[u8]` / `from_utf8` | ex16–17 | text is bytes; convert deliberately |

**`static mut` rule:** reading/writing a scalar is fine; taking a reference
(`&`/`&mut`) trips the `static_mut_refs` lint — use `addr_of!` instead.

---

## 17 · What each exercise built

**Part 1 — build the kernel**

| # | Name | Landmark |
|---|---|---|
| 00 | rust_kernel_basics | a bare-metal binary that compiles |
| 01 | boot | boot to `_entry`, print over the UART |
| 02 | physical_memory | `kalloc` page allocator (free list) |
| 03 | paging | Sv39 page tables, MMU off |
| 04 | processes | the `Proc` table |
| 05 | context_switch | `swtch` between contexts |
| 06 | scheduling | round-robin scheduler |
| 07 | spinlocks | `SpinLock<T>` |
| 08 | semaphores | the heap comes online + a semaphore |
| 09 | virtual_memory | the MMU turned **on** |
| 10 | filesystem | in-memory inodes + directories |
| 11 | devices | the polled UART driver |

**Part 2 — boot it & build a shell**

| # | Name | Landmark |
|---|---|---|
| 12 | boot_to_life | a real `kmain`; dual-mode harness |
| 13 | traps | M→S transition; kernel trap handling |
| 14 | interrupts | timer ticks via CLINT |
| 15 | console | UART RX interrupts through the PLIC |
| 16 | shell | a REPL: pwd/ls/cd/mkdir |
| 17 | file_commands | touch/cat/rm/echo/rmdir |
| 18 | user_mode | first U-mode program + syscalls |
| 19 | exec | load any program + argv |

_Scroll with ↑↓ / PgUp / PgDn. Press m for the menu._
