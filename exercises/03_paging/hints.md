# Hints — 03 Paging

## Hint 1
Three pieces, building on each other:

- `Pte::new` and `Pte::pa` are pure bit math and are inverses of each other. A
  PTE stores the physical *page number* (the address shifted right by 12) up at
  bit 10, with flags in the low bits. If check #0 ("Pte::pa did not recover the
  address") fails, these two are where the bug is.
- `walk` descends the tree. If #0 passes but mapping/translation checks fail,
  look there. The structure mirrors the `kalloc` free-list loop from exercise
  02, but instead of one list you step through levels 2 → 1 → 0.

The given `px(level, va)` already extracts the 9-bit index for a level — use it.

## Hint 2
The two PTE helpers:

```rust
pub const fn new(pa: usize, flags: usize) -> Pte {
    Pte(((pa >> 12) << 10) | flags)
}
pub const fn pa(self) -> usize {
    (self.0 >> 10) << 12
}
```

For `walk`, loop over the upper levels (2, then 1). At each level look at
`table.add(px(level, va))`:

- if it's already valid, follow it: `table = (*pte).pa() as *mut Pte;`
- otherwise, if `alloc`, make a new table: `kalloc::kalloc()`, bail out null if
  that fails, **zero it** with `ptr::write_bytes(page, 0, PGSIZE)`, link it with
  `*pte = Pte::new(page as usize, PTE_V);` (valid bit only — no R/W/X on a
  table-pointing entry), then `table = page as *mut Pte;`
- if not valid and not `alloc`, return `ptr::null_mut()`.

Finally return the leaf: `table.add(px(0, va))`.

## Hint 3
Full `walk`:

```rust
pub unsafe fn walk(mut table: *mut Pte, va: usize, alloc: bool) -> *mut Pte {
    let mut level = 2;
    while level > 0 {
        let pte = table.add(px(level, va));
        if (*pte).is_valid() {
            table = (*pte).pa() as *mut Pte;
        } else {
            if !alloc {
                return ptr::null_mut();
            }
            let page = kalloc::kalloc();
            if page.is_null() {
                return ptr::null_mut();
            }
            ptr::write_bytes(page, 0, PGSIZE);
            *pte = Pte::new(page as usize, PTE_V);
            table = page as *mut Pte;
        }
        level -= 1;
    }
    table.add(px(0, va))
}
```

Why it works: each iteration moves `table` down one level, creating zeroed
tables on demand. After two steps `table` is the leaf-level table, and
`px(0, va)` indexes the final entry. Forgetting to zero a new table is the most
common bug — leftover bytes look like bogus "valid" entries on the next walk.
