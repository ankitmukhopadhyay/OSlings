# Hints - 18 User mode

## Hint 1
Four pieces, and each one has a worked model sitting next to it:

- `map_user_pages` (vm.rs) is two `mappages` calls - the exact same helper
  you used to build the kernel's page table in exercise 09 (`kvmmake`, just
  above it, is full of examples). The only new idea is the `PTE_U` flag: user
  mode may only touch pages that have it. Code wants R + X, stack wants R + W,
  both want U.
- the `scause == 8` branch of `usertrap` (usermode.rs) is three lines, and
  the `// IMPLEMENT` comment walks through all three. The one people forget:
  `sepc` points AT the `ecall`, so without `epc += 4` the program re-executes
  the same call forever (the harness watchdog catches this and says the
  syscalls were "never answered").
- `dispatch` (syscall.rs) is a plain `match num { ... }` like the shell's
  command dispatch in exercise 16, routing to the three handlers that already
  exist below it.
- `copyin` (vm.rs) is the mirror image of `copyout`, which is given directly
  above it with every step commented. Same loop, arrows reversed.

## Hint 2
The shapes, piece by piece.

`map_user_pages` (vm.rs):

```rust
mappages(table, USER_CODE, PGSIZE, code_page, PTE_R | PTE_X | PTE_U)?;
mappages(table, USER_STACK, PGSIZE, stack_page, PTE_R | PTE_W | PTE_U)?;
Ok(())
```

`usertrap`'s ecall branch (usermode.rs) - everything the user put in
registers is parked in the trapframe by now, so read a7 and a0..a2 from
there, and write the result back there:

```rust
(*tf).epc += 4; // step over the ecall, or it runs again forever
let ret = crate::syscall::dispatch(
    (*tf).a7 as usize,
    (*tf).a0 as usize,
    (*tf).a1 as usize,
    (*tf).a2 as usize,
);
(*tf).a0 = ret as u64;
```

`dispatch` (syscall.rs):

```rust
match num {
    SYS_EXIT => sys_exit(a0 as isize),
    SYS_GETPID => sys_getpid(),
    SYS_WRITE => sys_write(a0, a1, a2),
    _ => -1,
}
```

`copyin` (vm.rs): first make the last parameter mutable - change
`srcva: usize` to `mut srcva: usize` (compare `copyout`'s `mut dstva`). Then
loop: round `srcva` down to its page, translate that page with `walkaddr`
(0 means a bad pointer: `return Err(())`), copy as many bytes as fit on that
page (or as remain), advance.

## Hint 3
Full code for the two vm.rs pieces, with the reasoning.

```rust
pub unsafe fn map_user_pages(
    table: *mut Pte,
    code_page: usize,
    stack_page: usize,
) -> Result<(), ()> {
    mappages(table, USER_CODE, PGSIZE, code_page, PTE_R | PTE_X | PTE_U)?;
    mappages(table, USER_STACK, PGSIZE, stack_page, PTE_R | PTE_W | PTE_U)?;
    Ok(())
}
```

Why those flags: the CPU fetches instructions from the code page (X) and
reads the message stored in it (R), but nothing ever writes it, so no W. The
stack is data: R + W but never executed. And without U on both, the very
first instruction fetch at address 0 faults - user mode may only touch pages
whose leaf PTE has PTE_U set.

```rust
pub unsafe fn copyin(table: *mut Pte, dst: &mut [u8], mut srcva: usize) -> Result<(), ()> {
    let mut copied = 0;
    while copied < dst.len() {
        let va0 = pgrounddown(srcva);          // the user page this address is on
        let pa0 = walkaddr(table, va0);        // where that page really is
        if pa0 == 0 {
            return Err(());                    // unmapped, or not a user page
        }
        let off = srcva - va0;                 // where on the page we start
        let mut n = PGSIZE - off;              // bytes left on this page
        if n > dst.len() - copied {
            n = dst.len() - copied;            // do not copy more than we need
        }
        ptr::copy_nonoverlapping((pa0 + off) as *const u8, dst.as_mut_ptr().add(copied), n);
        copied += n;
        srcva = va0 + PGSIZE;                  // continue on the next page
    }
    Ok(())
}
```

Why the loop: the user's buffer is contiguous in *virtual* addresses, but its
pages can be anywhere in physical memory, so every page needs its own
`walkaddr` translation. `walkaddr` is also the security check: it returns 0
for anything that is not a valid, user-accessible (`PTE_U`) page, so a
program that passes the kernel a hostile pointer gets `Err` (the syscall
returns -1) instead of a peek at kernel memory.

For the other two pieces, the code in Hint 2 is already complete: the ecall
branch goes right after the `// IMPLEMENT` comment in `usertrap` (usermode.rs),
and the `match` replaces the `-1` stub in `dispatch` (syscall.rs). If the
run still times out, check the `epc += 4` line is really there - resuming AT
the ecall is an infinite loop the watchdog reports as "system calls never
answered".
