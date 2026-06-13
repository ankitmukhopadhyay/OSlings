# Hints — 02 Physical Memory

## Hint 1
Both functions work on the same singly linked list whose head is the global
`FREELIST`. Think of the two basic linked-list operations:

- `kfree` = **push** a node onto the front of the list.
- `kalloc` = **pop** a node off the front of the list.

The "node" *is* the page: cast the page pointer to `*mut Run` and use its
`next` field as the link. You read and write that field with `(*r).next`.

If `run_checks` says "kalloc returned null", your free list is empty — most
likely `kfree` isn't actually linking pages in, so `init` built nothing.

## Hint 2
`kfree(pa)` in three steps:

1. `let r = pa as *mut Run;` — view the page as a list node.
2. `(*r).next = FREELIST;` — this node now points at the old head.
3. `FREELIST = r;` — this node becomes the new head.

Do them in that order. If you overwrite `FREELIST` first, you lose the rest of
the list.

`kalloc()` is the mirror image:

1. `let r = FREELIST;` — the current head (may be null).
2. if `!r.is_null()` then `FREELIST = (*r).next;` — advance the head.
3. return `r as *mut u8`.

## Hint 3
Complete bodies:

```rust
pub unsafe fn kfree(pa: *mut u8) {
    let r = pa as *mut Run;
    (*r).next = FREELIST;
    FREELIST = r;
}

pub unsafe fn kalloc() -> *mut u8 {
    let r = FREELIST;
    if !r.is_null() {
        FREELIST = (*r).next;
    }
    r as *mut u8
}
```

Why this passes the checks: `kfree` followed by `kalloc` returns the page you
just freed (it went on the front and comes right back off the front — LIFO),
which is exactly what check #5 verifies. And because `init` already `kfree`d
every page in RAM, the first `kalloc` succeeds with a real, page-aligned
address.
