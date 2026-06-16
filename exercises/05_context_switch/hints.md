# Hints — 05 Context Switch

## Hint 1
`swtch` does three things, in order: **save** the 14 callee-saved registers into
the old context, **load** them from the new context, then `ret`. The `ret` is
what actually performs the switch — it jumps to the `ra` you just loaded from
`new`.

The offsets are fixed by the `#[repr(C)]` field order in `Context`:
`ra`=0, `sp`=8, then `s0..s11` at 16, 24, 32, …, 104 (each 8 bytes apart).

If the run says "task never ran", your `swtch` returned without switching (the
skeleton's placeholder `ret`). If it times out instead, an offset or register
name is off and the kernel faulted.

## Hint 2
Use `sd` (store doubleword, 8 bytes) to save into the OLD context whose pointer
is in `a0`, and `ld` (load doubleword) to load from the NEW context whose
pointer is in `a1`. The pattern for one register:

```asm
sd ra, 0(a0)      # save:  memory[a0 + 0] = ra
...
ld ra, 0(a1)      # load:  ra = memory[a1 + 0]
```

Do all 14 stores first (ra, sp, s0…s11), then all 14 loads (same order/offsets),
then `ret`. Don't forget `sp` at offset 8 — without restoring the new stack, the
task has no stack to run on.

## Hint 3
Full body of `swtch`:

```asm
.globl swtch
swtch:
    sd ra,  0(a0)
    sd sp,  8(a0)
    sd s0,  16(a0)
    sd s1,  24(a0)
    sd s2,  32(a0)
    sd s3,  40(a0)
    sd s4,  48(a0)
    sd s5,  56(a0)
    sd s6,  64(a0)
    sd s7,  72(a0)
    sd s8,  80(a0)
    sd s9,  88(a0)
    sd s10, 96(a0)
    sd s11, 104(a0)

    ld ra,  0(a1)
    ld sp,  8(a1)
    ld s0,  16(a1)
    ld s1,  24(a1)
    ld s2,  32(a1)
    ld s3,  40(a1)
    ld s4,  48(a1)
    ld s5,  56(a1)
    ld s6,  64(a1)
    ld s7,  72(a1)
    ld s8,  80(a1)
    ld s9,  88(a1)
    ld s10, 96(a1)
    ld s11, 104(a1)

    ret
```

Why it round-trips: the first `swtch(SCHED→TASK)` saves kmain's registers into
`SCHED_CTX` and loads `TASK_CTX`, whose `ra` is `task_entry` and whose `sp` is
the task stack — so `ret` lands in the task. The task then calls
`swtch(TASK→SCHED)`, which reloads `SCHED_CTX`, so its `ret` returns into `kmain`
right after the first call.
