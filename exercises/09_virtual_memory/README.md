# 09 · Virtual Memory

> **Learn → Understand → Implement.** The payoff: you build the kernel's page
> table and actually **turn the MMU on**, so every address the CPU uses is now
> translated. You meet the `satp` register, identity mapping, and why kernels
> live below the borrow checker.

## Learn

In exercise 03 you built page tables and verified translations by hand — but the
hardware never used them. This exercise flips the switch: after it, the CPU's
**MMU** (Memory Management Unit) translates *every* memory access through a page
table you provide. This is the moment rv6 gets real virtual memory.

### The bootstrap problem, and identity mapping

Here's the scary part. The instant you turn translation on, the *very next
instruction the CPU fetches* is itself a memory access — and it gets translated.
If the page holding your running code isn't mapped, the CPU faults immediately
and the kernel dies before it can do anything.

The way every kernel solves this is **identity mapping**: build the kernel's page
table so that each virtual address maps to *the same* physical address
(`va == pa`). Then turning the MMU on changes nothing about where things appear
to be — the program counter, the stack pointer, every pointer you hold keeps
working, because they all translate to themselves. Translation is "on," but
transparent. (Later, user processes get *non*-identity maps; the kernel keeps its
identity map.)

So `kvmmake` builds an identity map of everything the kernel must keep touching:

- **the UART page** (`0x1000_0000`) — or printing breaks the moment paging is on;
- **the test-finisher page** (`0x10_0000`) — or the kernel can't exit QEMU;
- **all of RAM** (`KERNBASE`..`PHYSTOP`) — this single region covers the kernel's
  code, its data, every stack, *and* the page tables themselves (they live in
  RAM too, and must stay reachable).

We map RAM as read+write+execute for simplicity. (A hardened kernel maps code as
read+execute and data as read+write — "W^X" — but getting the split exactly right
is fiddly, and one mistake means an instant fault, so we keep it permissive here.)

### `satp`: the register that turns it on

A special CPU register, **`satp`** (Supervisor Address Translation and
Protection), tells the hardware which page table to use and which scheme. Its
bits are packed:

```text
  bits 63..60 : MODE   (8 = Sv39, our 3-level 39-bit scheme; 0 = off/"Bare")
  bits 59..44 : ASID   (address-space id; 0 for us)
  bits 43..0  : PPN    (physical page number of the ROOT table = root_pa >> 12)
```

So activating page table `root` means computing
`satp = (8 << 60) | (root_pa >> 12)` and writing it. That packing is `make_satp`.
Writing `satp` with `MODE = 0` would mean "no translation," which is why a wrong
`make_satp` must be caught *before* we rely on it.

### `sfence.vma`: flush stale translations

CPUs cache recent address translations in a **TLB** (Translation Lookaside
Buffer). After changing the page table you must run the **`sfence.vma`**
instruction to flush that cache, so the CPU doesn't use stale entries. `csrw
satp, …` followed by `sfence.vma` is the standard "switch on / switch tables"
sequence — that's all `kvminithart` does.

### Doing this safely

Because a wrong table hangs the kernel with no error message, the test here
**verifies the table with `walk` while the MMU is still off** — checking each
region is mapped, identity-mapped, and has the right permissions, and that
`make_satp` is well-formed — and only calls `kvminithart` once everything checks
out. So a bug shows up as a precise message instead of a hang.

### The Rust angle: below the borrow checker

You'll notice this whole subsystem is raw pointers (`*mut Pte`) and `unsafe`, not
Rust references (`&mut Pte`). That's deliberate. A reference carries a
**lifetime** — a compiler-tracked promise about how long it stays valid and that
nothing else mutates the same data meanwhile. But a page table is interpreted by
*hardware*; the MMU can change what an address means out from under you, and the
same memory is reachable through many addresses at once. The borrow checker can't
model any of that, so we step outside it and take responsibility ourselves with
`unsafe`. Lifetimes and the borrow checker come back the moment we build *safe*
abstractions on top of this raw layer — which is exactly the point of confining
`unsafe` to small, audited modules like `vm.rs`.

## Understand

Read `rv6/src/vm.rs`: the carried `Pte`/`walk`/`mappages` (from exercise 03), then
the new `SATP_SV39`, `make_satp`, `kvmmake`, and the given `kvminithart` (the
actual `csrw satp` + `sfence.vma`). Then read `rv6/src/main.rs`: it builds the
table, verifies each mapping with `walk` (MMU off), checks the `satp` value, and
only then turns paging on.

## Implement

In `rv6/src/vm.rs`:

1. **`make_satp`** — pack the Sv39 mode with the root table's PPN:
   `SATP_SV39 | ((root as usize) >> 12)`.
2. **`kvmmake`** — after the root table is allocated and zeroed (given),
   identity-map the three regions with `mappages` (UART → R+W, test finisher →
   R+W, `KERNBASE..PHYSTOP` → R+W+X), returning null if any mapping fails.

Check your work:

```sh
oslings run 09_virtual_memory
# or
oslings watch
```

Passes when the table verifies and the MMU switches on cleanly — printing
`OSLINGS:PASS`. If a region is missing you'll get a precise `[fail]` (e.g.
`UART page not identity-mapped read+write`) *before* anything dangerous happens.

Stuck? `oslings hint`.
