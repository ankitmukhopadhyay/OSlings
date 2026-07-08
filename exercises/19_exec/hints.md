# Hints - 19 exec

## Hint 1
Two pieces, and both have a worked model right next to them.

- `load_segment` (vm.rs) is exercise 18's `map_user_pages` turned into a loop.
  `map_user_stack`, just below it, is the one-page version: allocate a page,
  zero it, `mappages` it. `load_segment` does that in a loop - one page per
  `PGSIZE` chunk of the image - and adds a copy step (the stack had nothing to
  copy; a program does). The image's `big` entry is padded past one page, so
  your loop has to run more than once.
- `build_process` (exec.rs) is just calling six things in order, each of which
  already exists. The `// IMPLEMENT` comment gives you the exact line for every
  step. The only real idea is the last one: a program receives its arguments in
  registers `a0` (argc) and `a1` (argv), so you set those in the trapframe.

If nothing runs at all, it is almost always `build_process` returning early -
check every step uses `?` and that the final step returns `Ok(())`.

## Hint 2
The shapes.

`load_segment` (vm.rs) - loop until the whole image is copied:

```rust
let mut off = 0;
while off < image.len() {
    let page = kalloc::kalloc();
    if page.is_null() {
        return Err(());
    }
    ptr::write_bytes(page, 0, PGSIZE);                 // zero the page first
    let n = core::cmp::min(PGSIZE, image.len() - off); // this chunk's size
    ptr::copy_nonoverlapping(image.as_ptr().add(off), page, n);
    mappages(table, USER_CODE + off, PGSIZE, page as usize, PTE_R | PTE_X | PTE_U)?;
    off += PGSIZE;
}
asm!("fence.i"); // we wrote instructions; flush the fetch path (see kvmmake)
Ok(())
```

`build_process` (exec.rs) - the six steps, each with `?`:

```rust
let prog = lookup(name).ok_or(ExecError::NotFound)?;
proc::proc_pagetable(p).map_err(|_| ExecError::NoMem)?;
vm::load_segment((*p).pagetable, prog.image).map_err(|_| ExecError::NoMem)?;
vm::map_user_stack((*p).pagetable).map_err(|_| ExecError::NoMem)?;
let (argc, argv, sp) = push_argv((*p).pagetable, name, args)?;
let tf = (*p).trapframe;
(*tf).epc = USER_CODE as u64;   // start at the program's first instruction
(*tf).sp  = sp as u64;          // on the stack push_argv built
(*tf).a0  = argc as u64;        // a0 = argc
(*tf).a1  = argv as u64;        // a1 = argv
Ok(())
```

Note the two ways of turning a foreign error into an `ExecError`: `lookup`
returns an `Option`, so `.ok_or(ExecError::NotFound)?`; the `vm`/`proc`
functions return `Result<_, ()>`, so `.map_err(|_| ExecError::NoMem)?`.

## Hint 3
Full code for both, with the reasoning.

```rust
// vm.rs
pub unsafe fn load_segment(table: *mut Pte, image: &[u8]) -> Result<(), ()> {
    let mut off = 0;
    while off < image.len() {
        let page = kalloc::kalloc();
        if page.is_null() {
            return Err(());
        }
        ptr::write_bytes(page, 0, PGSIZE);
        let n = core::cmp::min(PGSIZE, image.len() - off);
        ptr::copy_nonoverlapping(image.as_ptr().add(off), page, n);
        mappages(table, USER_CODE + off, PGSIZE, page as usize, PTE_R | PTE_X | PTE_U)?;
        off += PGSIZE;
    }
    asm!("fence.i");
    Ok(())
}
```

Why each part: we zero the page before copying so that if the last chunk is
shorter than a full page (it usually is), the leftover tail is clean zeros, not
garbage. `n` is the smaller of "a full page" and "what is left of the image", so
the final short chunk copies only what exists. We map at `USER_CODE + off` so
page 0 of the image lands at VA 0, page 1 at VA 0x1000, and so on. `PTE_U` lets
user mode run it; without it the very first instruction fetch faults. And
`fence.i` once at the end tells the CPU we wrote new *instructions* into data
memory, so it must not run stale prefetched bytes.

```rust
// exec.rs
unsafe fn build_process(p: *mut Proc, name: &str, args: &[&str]) -> Result<(), ExecError> {
    let prog = lookup(name).ok_or(ExecError::NotFound)?;
    proc::proc_pagetable(p).map_err(|_| ExecError::NoMem)?;
    vm::load_segment((*p).pagetable, prog.image).map_err(|_| ExecError::NoMem)?;
    vm::map_user_stack((*p).pagetable).map_err(|_| ExecError::NoMem)?;
    let (argc, argv, sp) = push_argv((*p).pagetable, name, args)?;
    let tf = (*p).trapframe;
    (*tf).epc = USER_CODE as u64;
    (*tf).sp = sp as u64;
    (*tf).a0 = argc as u64;
    (*tf).a1 = argv as u64;
    Ok(())
}
```

Why it works: `build_process` is the recipe for turning a blank process into a
runnable program. Steps 2-4 build the address space (kernel pages, the program's
code, a stack); step 5 (`push_argv`, given) writes the arguments into that stack
and tells you where argv ended up and where the stack pointer now is; step 6
programs the trapframe, which is the set of register values the trampoline will
load when it drops into user mode - so setting `epc`, `sp`, `a0`, and `a1` there
is exactly how the program starts at instruction 0, on its new stack, already
holding `argc` and `argv`. Because every step returns via `?`, a failure at any
point bubbles up to `exec`, which frees the half-built process - so you never
have to clean up here yourself.
