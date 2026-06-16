# 04 · Processes

> **Learn → Understand → Implement.** You'll build the kernel's process table —
> the list of running programs — and meet Rust's `enum` and the idea of
> *ownership*.

## Learn

We can now manage RAM (exercise 02) and build address maps (exercise 03). The
next big kernel concept is the **process**: one running program, together with
all the state the kernel keeps about it.

### What is a process?

When you run a program, the operating system wraps it in a **process** — a
container that holds everything that program needs and everything the kernel
must remember about it: which memory it can touch (its page table), whether it's
currently running or waiting, its unique id, and (later) its saved CPU
registers, open files, and parent. Running ten programs means ten processes, and
the kernel has to keep them straight.

The kernel's record for one process is called a **Process Control Block (PCB)**.
In our kernel that's the `Proc` struct. This exercise builds the table of PCBs
and the two operations that manage it: hand out a free slot (`allocproc`) and
give one back (`freeproc`). We are *not* running or switching between processes
yet — that's the next exercise. Here we build the bookkeeping.

### Process states: a lifecycle

A process is always in exactly one **state**, and it moves between them over its
life:

- **Unused** — this table slot is empty/free.
- **Runnable** — ready to run, waiting for the scheduler to choose it.
- **Running** — currently executing on a CPU.
- **Sleeping** — blocked, waiting for something (a key press, disk, a lock).
- **Zombie** — finished, but its slot isn't cleaned up yet.

"Exactly one of a fixed set of named values" is *precisely* what a Rust **enum**
is for, so `ProcState` is an enum. A freshly allocated process starts
**Runnable**; a freed one goes back to **Unused**.

### The process table

Where do PCBs live? In a **fixed-size array**, `PROCS`, of length `NPROC` (64),
in static memory. This is deliberate: kernels avoid growing core data structures
on a heap, because the heap itself is something the kernel provides and because a
fixed table gives predictable memory use and simple, fast lookup. The cost is a
hard limit — at most `NPROC` processes — which is exactly what check #3 in the
test verifies.

Each process also gets a unique **pid** (process id), a simple increasing
counter, so the rest of the system can refer to it.

### The Rust you need

**`enum`** — a type that is one of several named **variants**:

```rust
enum ProcState { Unused, Runnable, Running, Sleeping, Zombie }
```

We `#[derive(PartialEq, Eq)]` on it so we can compare with `==`/`!=`
(`state == ProcState::Unused`), and `#[derive(Clone, Copy)]` so a state is a
cheap value you can copy around.

**`const fn` and the static table** — `Proc::new()` is a `const fn` (computable
at compile time). That lets us build the whole array at compile time with

```rust
static mut PROCS: [Proc; NPROC] = [const { Proc::new() }; NPROC];
```

so we don't need `Proc` to be `Copy` (and it shouldn't be — see ownership).

**Ownership** — this is one of Rust's central ideas. Every resource has exactly
one **owner** responsible for releasing it; leak it and memory is lost forever,
release it twice and you corrupt things. In *safe* Rust the compiler enforces
this automatically (values are dropped when their owner goes out of scope). In
the kernel's process table we're below that safety net — the table is a `static
mut` reached through raw pointers — so we uphold the same *discipline by hand*:

- A `Proc` **owns** its page table page. `allocproc` creates it (the process
  becomes the owner).
- `freeproc` must give it back (`free_pagetable`) and null the field. Forget,
  and the page leaks; free twice, and you have a bug.

That is exactly why `Proc` is **not** `Copy` — copying a PCB would duplicate the
owner of a page table, which ownership forbids. (Exercise 09 brings the borrow
checker back to help with in-kernel data.)

**Raw pointers and `addr_of_mut!`** — to touch a slot we use
`ptr::addr_of_mut!(PROCS[i])`, which produces a `*mut Proc` *without* first
creating a Rust reference (`&mut`) to the static. Creating references to a
`static mut` is error-prone (two references could alias), so we deliberately
stay on raw pointers, consistent with the previous two exercises.

## Understand

Read `rv6/src/proc.rs`:

- the `ProcState` enum and the `Proc` PCB struct;
- the `PROCS` table and `NEXTPID` counter;
- the **given** helpers: `init`, `alloc_pid`, `create_pagetable`,
  `free_pagetable` (your `allocproc`/`freeproc` call these).

Then read `rv6/src/main.rs` → `run_checks`: it allocates one process, checks a
second is distinct with a unique pid, fills the table to exactly `NPROC`,
confirms a full table refuses more, frees a slot (checking the state resets and
the page table is dropped), and confirms exactly one more allocation then
succeeds.

## Implement

In `rv6/src/proc.rs`:

1. **`allocproc`** — find the first `Unused` slot, give it a pid, mark it
   `Runnable`, and create its page table (return null if the table is full or
   memory runs out).
2. **`freeproc`** — free the owned page table, then reset the slot to `Unused`.

Check your work:

```sh
oslings run 04_processes
# or
oslings watch
```

Passes when the process-table self-test prints `OSLINGS:PASS`. Each check tells
you exactly what went wrong.

Stuck? `oslings hint`.
