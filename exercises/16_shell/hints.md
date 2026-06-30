# Hints — 16 Shell

## Hint 1
`exec` does two things: **parse** the line into a command and an argument, then
**dispatch** to the matching handler. The handlers (`cmd_pwd`, `cmd_ls`,
`cmd_cd`, `cmd_mkdir`) are already written — you just need to call the right one.

Use `line.split_whitespace()` to get the words. The first is the command; the
second (if present) is the argument.

If the test says "'mkdir docs' then 'ls' did not list docs", your `exec` isn't
dispatching (commands never run).

## Hint 2
Get the command and argument:

```rust
let mut words = line.split_whitespace();
let cmd = match words.next() {
    Some(c) => c,
    None => return,            // blank line
};
let arg = words.next().unwrap_or("");
```

Then dispatch with a `match` on `cmd`:

```rust
match cmd {
    "pwd"   => self.cmd_pwd(out),
    "ls"    => self.cmd_ls(out),
    "cd"    => self.cmd_cd(arg, out),
    "mkdir" => self.cmd_mkdir(arg, out),
    _ => { out.puts(cmd); out.puts(": command not found\n"); }
}
```

## Hint 3
Full `exec`:

```rust
pub fn exec(&mut self, line: &str, out: &mut dyn Out) {
    let mut words = line.split_whitespace();
    let cmd = match words.next() {
        Some(c) => c,
        None => return,
    };
    let arg = words.next().unwrap_or("");

    match cmd {
        "pwd" => self.cmd_pwd(out),
        "ls" => self.cmd_ls(out),
        "cd" => self.cmd_cd(arg, out),
        "mkdir" => self.cmd_mkdir(arg, out),
        _ => {
            out.puts(cmd);
            out.puts(": command not found\n");
        }
    }
}
```

Why it works: `split_whitespace` turns `"mkdir docs"` into the words `mkdir` and
`docs`; the `match` sends `mkdir` to `cmd_mkdir("docs", out)`, which creates the
directory; later `ls` runs `cmd_ls`, which lists it. The given `run` loop feeds
each line you type into this `exec`, so once it's written, `cargo run` gives you
a working `rv6$` prompt.
