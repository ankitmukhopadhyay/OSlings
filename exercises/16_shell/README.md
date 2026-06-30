# 16 · Shell

> **Learn → Understand → Implement.** You'll build a **shell**: a loop that reads
> a line, figures out which command it is, and runs it. After this, rv6 boots to
> a prompt you can actually type commands into.

## Learn

Two pieces are now in place: the kernel can **read input** (exercise 15) and it
has a **filesystem** (exercise 10). A shell ties them together. A shell is
surprisingly simple — it's a loop, often called a **REPL** (read, evaluate,
print, loop):

1. **Read** a line the user typed.
2. **Evaluate** it: figure out the command and run it.
3. **Print** any output.
4. **Loop** back for the next line.

The reading loop is given (`run` in `shell.rs`): it prints a `rv6$` prompt,
collects characters via `console::getc` (echoing them, handling backspace), and
when you press Enter it hands the finished line to `exec`. Your job is **`exec`**:
the "evaluate" step.

### Parsing a command line

A command line is just words separated by spaces, like `mkdir docs`. The first
word is the **command** (`mkdir`); the rest are **arguments** (`docs`). Rust's
`str` gives you `split_whitespace()`, an iterator over the words — no manual
character fiddling needed:

```rust
let mut words = line.split_whitespace();
let cmd = words.next();          // Option<&str>: the command (None if blank)
let arg = words.next().unwrap_or("");  // the first argument, or "" if none
```

### Dispatching

Once you know the command, you **dispatch**: a `match` that calls the right
handler. The handlers are written for you — read them, they show how a command
uses the filesystem:

* **`cmd_pwd`** — prints the current directory path, e.g. `/docs`.
* **`cmd_ls`** — lists the entries in the current directory (using the
  `for_each_entry` helper you saw added to `fs.rs`).
* **`cmd_cd`** — changes directory: `cd name` goes into a subdirectory, `cd ..`
  goes up, `cd /` goes to the root.
* **`cmd_mkdir`** — creates a directory with `dircreate`.

So `exec` is the heart of the shell: parse, then dispatch.

### The current directory

The shell remembers where you are with a small **stack** of path components (a
`Vec<(String, usize)>` — name and inode number). `pwd` walks it to print the
path; `cd ..` pops it; `cd name` pushes a new component. This uses the heap
(`Vec`, `String`) that came online in exercise 08 — which is why a shell needs an
allocator.

### Where output goes: the `Out` trait

Commands don't call the UART directly; they write to an **`Out`** (a trait, like
the `Scheduler` trait in exercise 06). The real shell uses an `Out` that writes
to the console; the automatic test uses one that writes to a buffer it can
inspect. Same commands, two destinations — that's the point of a trait.

### The Rust you need

* **`str::split_whitespace()`** to break the line into words.
* **`match`** on the command string to dispatch.
* the command handlers and `Out` are given; you call them.

## Understand

Read `rv6/src/shell.rs`: the `Out` trait, the `Shell` struct and its `cwd`
helper, the given command handlers (`cmd_pwd`/`cmd_ls`/`cmd_cd`/`cmd_mkdir`), the
`run` REPL, and the one function you write, `exec`. Note the new `fs.rs` helpers
`is_dir` and `for_each_entry`. Then read `rv6/src/main.rs`: the harness drives the
shell with a script of commands and checks the output.

## Implement

In `rv6/src/shell.rs`, fill in **`exec`**: split the line into words, take the
command (returning on a blank line) and the optional argument, then `match` the
command to call `cmd_pwd` / `cmd_ls` / `cmd_cd` / `cmd_mkdir` (or report an
unknown command).

Check your work:

```sh
oslings run 16_shell
# or
oslings watch
```

It passes when `mkdir`, `ls`, `cd`, and `pwd` all work through your dispatch.

Then use it for real — this is the payoff:

```sh
cd rv6 && cargo run        # boots to a rv6$ prompt
```

Try: `mkdir docs`, `ls`, `cd docs`, `pwd`, `mkdir notes`, `ls`, `cd ..`, `pwd`.
(Exit QEMU with Ctrl-A then X.)

Stuck? `oslings hint`.
