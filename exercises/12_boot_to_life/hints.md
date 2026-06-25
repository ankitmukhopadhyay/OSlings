# Hints — 12 Boot to life

## Hint 1
`kinit` doesn't contain any new logic — it just *calls* the `init` functions of
the subsystems you already built, one per line, in the right order. There are
four of them: the console (uart), the page allocator (kalloc), virtual memory
(vm), and the process table (proc).

If the self-check says "kalloc is not initialized", you haven't called
`kalloc::init()` (or `kinit` is still empty).

## Hint 2
The four calls, and the order they must go in:

1. `uart::init();`
2. `kalloc::init();`
3. turn the MMU on — build the page table then load it:
   `vm::kvminithart(vm::kvmmake());`
4. `proc::init();`

`kvmmake()` allocates page-table pages with `kalloc`, so `kalloc::init()` must
come *before* it. If you turn the MMU on before the allocator is ready, the page
table is empty/garbage and the kernel hangs the instant paging switches on.

## Hint 3
Full `kinit`:

```rust
unsafe fn kinit() {
    uart::init();                       // 1. console — so we can print
    kalloc::init();                     // 2. physical page allocator
    vm::kvminithart(vm::kvmmake());     // 3. build kernel page table + MMU on
    proc::init();                       // 4. process table
}
```

Why each check then passes: `kalloc::init()` makes `kalloc()` hand out a real
page; `vm::kvminithart(...)` writes `satp` with Sv39 mode (so `satp >> 60 == 8`);
`proc::init()` makes `allocproc()` succeed. Run `oslings run 12` to grade, and
`cargo run` (from `rv6/`) to watch the banner print and the kernel idle — your
OS booting for the first time.
