# 00 · Rust Kernel Basics

> **Learn → Understand → Implement.** Read this lesson, study the code in
> `rv6/src/main.rs`, then fill in the `IMPLEMENT` markers until the exercise
> passes.

## Learn

When you write an ordinary Rust program, an enormous amount of machinery is
handed to you for free by the **standard library** (`std`): a heap allocator,
threads, file handles, `println!`, and a `main` function that the operating
system's C runtime calls on your behalf.

A kernel has *none* of that, because **the kernel is what provides those
things**. There is no OS beneath us to open files or spawn threads. So the
very first thing we do is tell the compiler: *don't assume any of that exists.*

### `#![no_std]`

This inner attribute removes the standard library. We keep only
[`core`](https://doc.rust-lang.org/core/) — the dependency-free heart of Rust:
types like `Option`, `Result`, slices, iterators, and `PanicInfo`, none of
which need an OS. (Later, once we've written our own memory allocators, we will
opt back into `alloc` for `Box`, `Vec`, etc.)

### `#![no_main]`

Normally the compiler generates a hidden `main` shim that the C runtime
(`crt0`) calls after setting up the process. There is no C runtime here, so we
remove that shim too. Our entry point is whatever the **linker script** names —
for rv6 that is the symbol `_entry` (see `rv6/kernel.ld`).

### The panic handler

`std` defines what happens when code panics (unwind, print, abort). Without
it, the compiler requires us to provide exactly one function marked
`#[panic_handler]`:

```rust
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    loop {}
}
```

The return type `!` is the **never type** — a promise that this function never
returns. That makes sense for a kernel: when something has gone irrecoverably
wrong this early in boot, there is nothing to return to. For now we simply spin.

### `#[no_mangle]` and `extern "C"`

```rust
#[no_mangle]
pub extern "C" fn _entry() -> ! { loop {} }
```

- `#[no_mangle]` keeps the symbol name exactly `_entry` so the linker script
  can find it (Rust normally "mangles" names into unique gibberish).
- `extern "C"` gives the function the C calling convention, which is the
  stable ABI the boot environment expects.

## Understand

Open `rv6/src/main.rs`. Read every line. Notice:

- There is no `use std::...` anywhere — only `core`.
- `_entry` is the only thing the outside world can call into.
- The file as shipped **does not compile**, on purpose.

## Implement

1. Add `#![no_std]` and `#![no_main]` at the top of `rv6/src/main.rs`.
2. Write the `#[panic_handler]` function.

Then check your work:

```sh
oslings run 00_rust_kernel_basics     # one-shot
# or
oslings watch                          # re-runs automatically on save
```

This exercise passes when **the kernel compiles for
`riscv64gc-unknown-none-elf`**. No QEMU yet — booting comes next, in
exercise 01.

Stuck? `oslings hint` reveals progressively more specific help.
