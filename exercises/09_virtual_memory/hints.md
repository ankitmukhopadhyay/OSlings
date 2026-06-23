# Hints — 09 Virtual Memory

## Hint 1
Two functions:

- `make_satp` is pure bit math: combine the Sv39 mode bits (`SATP_SV39`, already
  defined) with the root table's *physical page number* (its address divided by
  the page size, i.e. `>> 12`).
- `kvmmake` builds the identity map. The root table is already allocated and
  zeroed for you — you just add three `mappages` calls, each mapping a region to
  *itself* (`va == pa`).

If you see `make_satp: mode field is not Sv39`, your `make_satp` is returning 0
(the placeholder). If you see `UART page not identity-mapped`, `kvmmake` isn't
adding the mappings.

## Hint 2
`make_satp`:

```rust
SATP_SV39 | ((root as usize) >> 12)
```

`kvmmake` — three identity mappings (note `va` and `pa` are the same value):

```rust
if mappages(root, UART0, PGSIZE, UART0, PTE_R | PTE_W).is_err() {
    return ptr::null_mut();
}
// ...same for TEST_FINISHER (R+W)...
// ...and for all of RAM: KERNBASE, size PHYSTOP - KERNBASE, perms R+W+X...
```

The RAM mapping is one `mappages` call covering the whole `KERNBASE..PHYSTOP`
range — `mappages` loops over every page in it for you.

## Hint 3
Complete:

```rust
pub fn make_satp(root: *mut Pte) -> usize {
    SATP_SV39 | ((root as usize) >> 12)
}

pub unsafe fn kvmmake() -> *mut Pte {
    let root = kalloc::kalloc() as *mut Pte;
    if root.is_null() {
        return ptr::null_mut();
    }
    ptr::write_bytes(root as *mut u8, 0, PGSIZE);

    if mappages(root, UART0, PGSIZE, UART0, PTE_R | PTE_W).is_err() {
        return ptr::null_mut();
    }
    if mappages(root, TEST_FINISHER, PGSIZE, TEST_FINISHER, PTE_R | PTE_W).is_err() {
        return ptr::null_mut();
    }
    if mappages(root, KERNBASE, PHYSTOP - KERNBASE, KERNBASE, PTE_R | PTE_W | PTE_X).is_err() {
        return ptr::null_mut();
    }
    root
}
```

Why it works: every region is mapped `va == pa`, so when `kvminithart` writes
`satp` and runs `sfence.vma`, the program counter, stack, UART, and test device
all translate to the same addresses they had before — the kernel keeps running
without noticing, except that translation is now on. Mapping the whole
`KERNBASE..PHYSTOP` range in one call covers the code, data, stacks, and the page
tables themselves.
