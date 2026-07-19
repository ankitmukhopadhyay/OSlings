# Hints - 22 userland: exec

## Hint 1
One function to write: `exec_into` in `exec.rs`. Everything hard is already
given.

`exec` means "replace this running process with a different program". The only
genuinely new idea is the **swap**: build a fresh address space for the new
program, then point the process at it instead of its old one.

- Building the new address space is the given `build_addrspace(trapframe, name,
  args)`. It hands back a `Built` with a new `pagetable`, plus `argc`, `argv`,
  and `sp` (where the new program should start). It is the exact recipe you
  wrote in exercise 19, gathered into one helper.
- Your job is to install it: remember the old page table, set `(*p).pagetable`
  to the new one, repoint the trapframe (`epc`/`sp`/`a0`/`a1`) at the new
  program, then free the old page table.

Two things to get right:
- Use `?` on `build_addrspace` so that if it fails, you return early WITHOUT
  having touched the old address space (a failed exec must leave the caller
  running its old self).
- On success, `exec` does not "return" to the caller - it resumes as the new
  program. You make that happen by setting the trapframe's `epc` to the new
  program's entry (`USER_CODE`) and its `sp` to the new stack: when the syscall
  returns to user mode, `usertrapret` reads those and lands in the new program.

If `execself` exits 88 instead of 2, your swap did not happen (exec fell through
and returned). If it exits 2, the swap worked.

## Hint 2
The shape. Read the given `build_process` right above `exec_into` first - it
does almost the same thing, for a fresh process. Your `exec_into` does it for
the *current* process and frees the old memory afterward.

```rust
pub unsafe fn exec_into(p: *mut Proc, name: &str, args: &[&str]) -> Result<usize, ExecError> {
    // 1. build the new address space (bail out early on failure).
    let built = build_addrspace((*p).trapframe as usize, name, args)?;

    // 2-3. remember the old page table, install the new one.
    let old = (*p).pagetable;
    (*p).pagetable = built.pagetable;

    // 4. point the trapframe at the new program.
    let tf = (*p).trapframe;
    (*tf).epc = USER_CODE as u64;   // start at the program's first instruction
    (*tf).sp  = built.sp as u64;    // on its fresh stack
    (*tf).a0  = built.argc as u64;  // main(argc, argv): a0 = argc
    (*tf).a1  = built.argv as u64;  //                   a1 = argv

    // 5. free the OLD address space (safe: a syscall runs on the kernel page table).
    vm::free_user_pagetable(old);

    // 6. hand back argc (it becomes a0 as we resume in the new program).
    Ok(built.argc)
}
```

## Hint 3
The whole function, with the reasoning.

```rust
pub unsafe fn exec_into(p: *mut Proc, name: &str, args: &[&str]) -> Result<usize, ExecError> {
    // Build the new image FIRST, before we disturb anything. If this fails
    // (no such program, out of memory), `?` returns the error now, while the
    // process is still its old self - so a failed exec never destroys the
    // caller. `build_addrspace` gives back the new page table plus where the
    // program should begin: argc, the argv array's user address, and the sp.
    let built = build_addrspace((*p).trapframe as usize, name, args)?;

    // Now the swap. Keep a handle on the old page table so we can free it.
    let old = (*p).pagetable;
    (*p).pagetable = built.pagetable;

    // The trapframe holds the registers we will restore when we go back to user
    // mode. Overwriting epc/sp/a0/a1 is what makes the process "become" the new
    // program: usertrapret will resume at epc, on stack sp, with a0=argc and
    // a1=argv - exactly the C `main(argc, argv)` convention.
    let tf = (*p).trapframe;
    (*tf).epc = USER_CODE as u64;
    (*tf).sp = built.sp as u64;
    (*tf).a0 = built.argc as u64;
    (*tf).a1 = built.argv as u64;

    // Free the OLD user address space. This is safe here because a system call
    // runs on the KERNEL page table: we are not executing out of the memory we
    // are freeing. free_user_pagetable leaves the shared trampoline and the
    // trapframe pages alone, so only the old program's own pages go back.
    vm::free_user_pagetable(old);

    // Return argc. We do not really "return" to the caller - the caller's code
    // is gone. When the syscall unwinds and usertrapret runs, it restores the
    // trapframe we just wrote, and the CPU lands in the new program instead.
    Ok(built.argc)
}
```

Why exec is exactly this: a process is its address space (memory) plus its saved
registers (where it will resume). Swapping the page table swaps the memory;
repointing the trapframe swaps where it resumes. The open files (`ofile`) are
deliberately left alone, which is why a program's `stdout` survives across an
`exec` - the shell can redirect a child's output before exec'ing it. And because
the old code is gone, a successful exec cannot return; only a *failed* one does,
which is why `sys_exec` turns an `Err` into -1 for the caller to see.
