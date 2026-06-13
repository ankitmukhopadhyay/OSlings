# Hints — 00 Rust Kernel Basics

## Hint 1
The compiler error mentions `std` and/or a missing `#[panic_handler]`. Both
problems come from the same root cause: this file still assumes the standard
library exists. You need two *inner* attributes (`#![...]`, note the `!`) at
the very top of the file, and you need to supply the panic handler yourself.

## Hint 2
The two attributes are `#![no_std]` and `#![no_main]`. They must be the first
non-comment lines in the file — inner attributes apply to the whole crate and
have to appear before any items.

For the panic handler, you already have `use core::panic::PanicInfo;`. Write a
function that takes `&PanicInfo` and returns `!`.

## Hint 3
Put these at the very top:

```rust
#![no_std]
#![no_main]
```

And add this function (anywhere below the `use`):

```rust
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    loop {}
}
```

The parameter can be named `info` or `_info` (the leading underscore silences
the "unused variable" warning). The body just needs to never return — an empty
`loop {}` is enough for now.
