# 02 · Physical Memory

> **Learn → Understand → Implement.** You'll write the kernel's physical page
> allocator — the service that hands out and reclaims raw RAM. Along the way you
> meet Rust's `unsafe` and raw pointers.

## Learn

Your kernel can now boot and print (exercise 01). The next thing every kernel
needs is a way to *manage memory* — to keep track of which parts of RAM are in
use and which are free, and to hand out chunks on request. This exercise builds
exactly that, at the most basic level.

If you've never thought about memory management before, start here.

### What problem are we solving?

When the machine starts, RAM is just one big undifferentiated array of bytes
(from `0x8000_0000` up to `0x8800_0000` on our board — that's the 128 MiB we
gave QEMU). Our kernel's code and data sit at the bottom of that range. The rest
is empty and unclaimed.

The trouble is that lots of things will soon need memory: page tables, stacks
for each process, buffers for the disk, and so on. Something has to keep track
of which bytes are spoken for, so two different users don't accidentally grab the
same memory. That "something" is an **allocator**.

### Pages: the unit of allocation

Hardware doesn't manage memory one byte at a time — it works in fixed-size
blocks called **pages**. On RISC-V a page is **4096 bytes (4 KiB)**, the constant
`PGSIZE`. From now on, the kernel thinks of physical RAM as a grid of
4096-byte pages, and our allocator's job is to hand out *whole pages*, one at a
time.

A few terms you'll see:

- **Physical address** — an actual byte position in RAM (e.g. `0x8000_5000`).
  This is the raw, real address the hardware uses. (Later, "virtual addresses"
  will be a translation layer on top; not yet.)
- **Page-aligned** — an address that is an exact multiple of `PGSIZE`. Page
  `0x8000_1000` is aligned; `0x8000_1234` is not. Allocators always return
  page-aligned addresses, because the hardware expects pages to start on those
  boundaries.

### The design: a free list

How do we remember which pages are free? We keep a **free list**: a linked list
where each node is one free page.

Here's the elegant trick that real kernels (xv6, Linux's early allocators, ours)
use. A *free* page, by definition, contains nothing important — so we store the
list's "next pointer" *inside the free page itself*, in its first 8 bytes. No
separate table of bookkeeping is needed; the list lives in the very memory it
describes. This is called an **intrusive linked list**.

```
 FREELIST ─▶ [page A | next ─]─▶ [page B | next ─]─▶ [page C | next = null]
```

- **`kfree(page)`** — mark a page free: write the current list head into the
  page's first bytes, then make the page the new head. (Push onto the front.)
- **`kalloc()`** — grab a free page: take the head off the front of the list and
  return it. (Pop from the front.)
- **`init()`** — at boot, walk every page from the end of the kernel up to
  `PHYSTOP` and `kfree` each one, building the initial list. (Given to you.)

Because we always push and pop at the front, the most recently freed page is the
next one allocated — a **LIFO** (last-in, first-out) order. The test relies on
this.

### Where does the free memory start? The `end` symbol

We must not hand out the RAM the kernel itself is sitting in. How do we know
where the kernel ends? The **linker script** (`kernel.ld`) defines a symbol
called `end` at the very tail of the kernel image. (Recall from exercise 01: a
*symbol* is a name the linker attaches to an address.) So everything from `end`
up to `PHYSTOP` is fair game. The given `init()` starts there.

### The Rust you need: `unsafe` and raw pointers

Everything above means touching memory directly, by numeric address. Rust's
normal references (`&T`, `&mut T`) come with guarantees the compiler enforces —
but the compiler can't possibly know that `0x8000_5000` is a valid place to
write. So we drop to **raw pointers** and **`unsafe`**.

- **Raw pointers** — `*mut T` (mutable) and `*const T` (read-only) are just
  addresses with a type attached. Unlike references, they can be null, can
  dangle, and carry no borrow-checking. Creating one is safe; *using* one
  (dereferencing it) is not.

- **`unsafe`** — a block or function where you may do things the compiler can't
  verify, such as dereferencing a raw pointer. `unsafe` does **not** turn off
  Rust's other rules; it's you telling the compiler "I've personally checked
  that this is sound." Our allocator functions are `unsafe fn` for this reason.

- **Casting** — `pa as *mut Run` reinterprets a raw pointer as pointing to a
  different type. That's how we treat a free page's bytes *as* a list node.

- **Dereferencing** — `(*r).next` reads or writes the field a raw pointer points
  at. This is the line that actually pokes physical memory.

- **`ptr::null_mut()`** — a null `*mut T`, our "empty list / out of memory"
  value. Check it with `r.is_null()`.

- **`.add(i)`** — pointer arithmetic: `a.add(i)` is the address `i` bytes past
  `a`. The test uses it to walk across a whole page.

- **`static mut`** — a single global mutable variable (`FREELIST`). Reading or
  writing it needs `unsafe`, because nothing prevents two threads from racing on
  it. (We'll fix that with locking in a later exercise; for now we run on one
  CPU.)

Don't worry if `unsafe` feels uncomfortable — that discomfort is the point. The
goal is to confine these address-level operations to a tiny, carefully written
module so the *rest* of the kernel can stay safe.

## Understand

Open and read, in order:

- `rv6/src/memlayout.rs` — the constants (`PGSIZE`, `KERNBASE`, `PHYSTOP`).
- `rv6/src/kalloc.rs` — the allocator. Study `Run`, `FREELIST`, the `end`
  symbol, and the given `pgroundup` / `init` / `free_range`.
- `rv6/src/main.rs` — the test harness `run_checks`. It allocates, writes a
  pattern across a full page, checks two allocations differ, and checks that
  free-then-alloc recycles the same page. This is the contract your code meets.

Control flow:

```
kmain → kalloc::init → (free_range kfrees every page) → run_checks → kalloc/kfree → PASS
```

## Implement

In `rv6/src/kalloc.rs`, fill in the two `IMPLEMENT` functions:

1. **`kfree`** — push the page onto the front of the free list (write the old
   head into the page, then make the page the new head).
2. **`kalloc`** — pop the front page off the list and return it (or null).

Check your work:

```sh
oslings run 02_physical_memory
# or
oslings watch
```

This boots the kernel in QEMU; it passes when the allocator self-test prints
`OSLINGS:PASS`. If a specific check fails, the kernel prints which one — read
that line, it points straight at the bug.

Stuck? `oslings hint`.
