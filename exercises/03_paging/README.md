# 03 · Paging

> **Learn → Understand → Implement.** You'll build the page-table data structure
> the hardware uses to translate addresses — and meet Rust's newtype structs,
> bit manipulation, and `const fn`.

## Learn

So far, every address in our kernel has been a real, physical RAM address. This
exercise introduces the single most important idea in modern operating systems:
**virtual memory**.

### Why virtual memory exists

Imagine many programs running at once. Each one wants to believe it has the
whole machine's memory to itself, starting at a tidy address. But they can't all
*actually* use the same physical RAM — they'd stomp on each other. We also want
the kernel to be protected from buggy programs, and programs protected from each
other.

The solution: give each program its own private map of addresses. The addresses
a program uses are **virtual addresses**; the real RAM locations are **physical
addresses**. A hardware unit called the **MMU** (Memory Management Unit) sits
between the CPU and RAM and **translates** every virtual address into a physical
one, on every single memory access, using a lookup structure we build: the
**page table**.

This exercise builds and verifies that page table. (We don't switch the MMU
*on* yet — that's a later exercise. Here we focus on getting the structure
exactly right, which is the hard and interesting part.)

### How translation works: pages again

Translation happens at **page** granularity (our familiar 4096-byte chunks). The
bottom 12 bits of an address are the **offset** within a page (4096 = 2¹²), and
they're never translated — only the page *number* is. So translating an address
means: "which physical page does this virtual page map to?" then "keep the same
offset within it."

```text
  virtual addr  =  [ virtual page number ] [ 12-bit offset ]
                            │ translate           │ copied unchanged
  physical addr =  [ physical page number ] [ 12-bit offset ]
```

### Sv39: a three-level page table

RISC-V's scheme is called **Sv39**: virtual addresses are 39 bits wide. A single
flat table mapping every page would be enormous, so Sv39 uses a **tree of
tables, three levels deep.** The 27-bit virtual page number is sliced into three
9-bit indices, one per level:

```text
  bits: [38..30] VPN[2] | [29..21] VPN[1] | [20..12] VPN[0] | [11..0] offset
```

Each table is exactly one page (4096 bytes) holding **512 entries** of 8 bytes
each (512 × 8 = 4096; and 2⁹ = 512 — that's why the indices are 9 bits). To
translate an address you **walk** the tree:

1. Use VPN[2] to index the top (root) table → that entry points to a level-1
   table.
2. Use VPN[1] to index that table → points to a level-0 table.
3. Use VPN[0] to index that → the **leaf** entry, which finally gives the
   physical page.

That descent is the `walk` function you'll write. Where a table doesn't exist
yet, `walk` (in `alloc` mode) calls your `kalloc` from exercise 02 to make one —
this is where paging builds on physical memory.

### The page-table entry (PTE)

Each 8-byte entry packs two things into one 64-bit number:

```text
  bits [53..10]  physical page number (PPN)      bits [9..0]  flags
```

- The **PPN** is a physical address with its low 12 bits dropped (`pa >> 12`),
  stored starting at bit 10. So to build an entry: `(pa >> 12) << 10`. To read
  the address back: `(pte >> 10) << 12`. This shifting is the bit manipulation
  at the heart of the exercise.
- The **flags** (low 10 bits) describe the mapping:
  - `V` valid (the entry is in use),
  - `R` / `W` / `X` readable / writable / executable,
  - `U` user-mode accessible.

  An important rule: a PTE that points to a *next-level table* has **only `V`
  set** (no R/W/X). A PTE with any of R/W/X set is a **leaf** that maps real
  memory. That's how the hardware tells "go deeper" from "you've arrived."

### The Rust you need

- **Newtype struct** — `struct Pte(pub usize)` wraps a plain integer in a named
  type. It's a real type the compiler checks (you can't mix up a `Pte` with a
  bare number), but at runtime it's just the integer. `#[repr(transparent)]`
  guarantees it has the *exact* memory layout of that `usize`, so an array of
  512 `Pte`s really is a hardware page table.
- **Methods with bit operations** — `<<` / `>>` shift bits; `|` sets bits; `&`
  masks bits. `self.0` reaches the wrapped integer inside the newtype.
- **`const fn`** — a function the compiler can evaluate at compile time. Marking
  the PTE helpers `const fn` means constant page-table values can be computed
  before the program even runs (and it's good discipline — these are pure bit
  math).
- **`Result<(), ()>`** — `mappages` returns `Ok(())` on success or `Err(())` if
  it runs out of memory. `Result` is Rust's standard "this might fail" type;
  `.is_err()` checks for failure. (We'll use richer error types later.)
- **Raw pointers (recap from 02)** — a page table is reached through a `*mut
  Pte`; `table.add(i)` is the `i`-th entry; `(*pte)` reads/writes it.

## Understand

Read, in order:

- `rv6/src/vm.rs` — the flag constants, the `Pte` newtype, the given `px`
  (index extraction), `pgrounddown`, and `mappages`. Note how `mappages` leans
  on `walk`.
- `rv6/src/main.rs` — `run_checks`: it round-trips a PTE, identity-maps the
  UART page, maps a code page at a high virtual address (forcing new
  intermediate tables to be allocated), translates addresses through the table,
  and confirms an unmapped address does *not* translate.

## Implement

In `rv6/src/vm.rs`:

1. **`Pte::new`** — pack `pa` and `flags` into the entry.
2. **`Pte::pa`** — extract the physical address back out.
3. **`walk`** — descend levels 2 and 1 (following or allocating tables), then
   return the level-0 leaf entry.

Check your work:

```sh
oslings run 03_paging
# or
oslings watch
```

It passes when the page-table self-test prints `OSLINGS:PASS`. Each check prints
which step failed, so a wrong shift or a missing level shows up precisely.

Stuck? `oslings hint`.
