# Hints — 04 Processes

## Hint 1
Both functions are short and use the given helpers — you don't allocate memory
or assign pids by hand.

- `allocproc` is a loop over the table looking for a slot whose `state` is
  `ProcState::Unused`. The `enum` comparison is just `==`. When you find one,
  fill it in using `alloc_pid()` and `create_pagetable()`.
- `freeproc` is the reverse: release the page table with `free_pagetable(...)`,
  then set the slot back to `Unused`.

If check #1 says "allocproc returned null on an empty table", your loop isn't
finding the free slots (every slot starts `Unused` after `init`).

## Hint 2
Walk the table with raw pointers so you don't make references to the `static`:

```rust
for i in 0..NPROC {
    let p = ptr::addr_of_mut!(PROCS[i]);
    if (*p).state == ProcState::Unused {
        // set up *p and return it
    }
}
ptr::null_mut()  // nothing free
```

Setting a slot up: assign `(*p).pid`, `(*p).state`, `(*p).pagetable` in turn.
Remember `create_pagetable()` can fail (null) — if it does, return null.

For `freeproc`, the order matters for the ownership rule: free the page table
*first*, then null the field, then set the state to `Unused`.

## Hint 3
Full bodies:

```rust
pub unsafe fn allocproc() -> *mut Proc {
    for i in 0..NPROC {
        let p = ptr::addr_of_mut!(PROCS[i]);
        if (*p).state == ProcState::Unused {
            (*p).pid = alloc_pid();
            (*p).state = ProcState::Runnable;
            (*p).pagetable = create_pagetable();
            if (*p).pagetable.is_null() {
                return ptr::null_mut();
            }
            return p;
        }
    }
    ptr::null_mut()
}

pub unsafe fn freeproc(p: *mut Proc) {
    free_pagetable((*p).pagetable);
    (*p).pagetable = ptr::null_mut();
    (*p).pid = 0;
    (*p).state = ProcState::Unused;
}
```

Why this satisfies the test: each `Unused` slot becomes `Runnable` with a fresh
pid (uniqueness via `alloc_pid`), the loop naturally stops handing out slots once
all `NPROC` are taken, and `freeproc` returns a slot to `Unused` with its page
table released — so exactly one more `allocproc` then succeeds.
