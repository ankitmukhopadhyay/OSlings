//! shell.rs — a tiny interactive shell with file commands.

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::console;
use crate::fs::{self, InodeKind, FS};
use crate::uart;

/// Somewhere a command can write its output. The interactive shell writes to the
/// UART; the test writes to a buffer it can check. (A trait, like exercise 06.)
pub trait Out {
    fn puts(&mut self, s: &str);
}

/// Shell state: the current directory, as a path from the root. Each component
/// is (name, inode number).
pub struct Shell {
    stack: Vec<(String, usize)>,
}

impl Shell {
    pub fn new() -> Shell {
        Shell { stack: Vec::new() }
    }

    /// The inode number of the current directory.
    fn cwd(&self) -> usize {
        self.stack.last().map(|(_, inum)| *inum).unwrap_or(fs::ROOT)
    }

    /// Parse one input line and run the command it names. (Given — extended from
    /// exercise 16 with the new file commands. `echo` gets the whole line because
    /// it has to look for the `>` redirect itself.)
    pub fn exec(&mut self, line: &str, out: &mut dyn Out) {
        let mut words = line.split_whitespace();
        let cmd = match words.next() {
            Some(c) => c,
            None => return, // a blank line: do nothing
        };
        let arg = words.next().unwrap_or("");

        match cmd {
            "pwd" => self.cmd_pwd(out),
            "ls" => self.cmd_ls(out),
            "cd" => self.cmd_cd(arg, out),
            "mkdir" => self.cmd_mkdir(arg, out),
            "touch" => self.cmd_touch(arg, out),
            "cat" => self.cmd_cat(arg, out),
            "rm" => self.cmd_rm(arg, out),
            "rmdir" => self.cmd_rmdir(arg, out),
            "echo" => self.cmd_echo(line, out),
            _ => {
                out.puts(cmd);
                out.puts(": command not found\n");
            }
        }
    }

    fn cmd_pwd(&self, out: &mut dyn Out) {
        out.puts("/");
        for (i, (name, _)) in self.stack.iter().enumerate() {
            if i > 0 {
                out.puts("/");
            }
            out.puts(name);
        }
        out.puts("\n");
    }

    fn cmd_ls(&self, out: &mut dyn Out) {
        let dir = self.cwd();
        let fsg = FS.lock();
        fsg.for_each_entry(dir, |name, kind| {
            if let Ok(s) = core::str::from_utf8(name) {
                out.puts(s);
            }
            if kind == InodeKind::Dir {
                out.puts("/");
            }
            out.puts("\n");
        });
    }

    fn cmd_cd(&mut self, arg: &str, out: &mut dyn Out) {
        match arg {
            "" | "/" => self.stack.clear(),
            ".." => {
                self.stack.pop();
            }
            name => {
                let dir = self.cwd();
                let fsg = FS.lock();
                match fsg.dirlookup(dir, name.as_bytes()) {
                    Ok(inum) if fsg.is_dir(inum) => {
                        drop(fsg);
                        self.stack.push((String::from(name), inum));
                    }
                    Ok(_) => out.puts("cd: not a directory\n"),
                    Err(_) => out.puts("cd: no such directory\n"),
                }
            }
        }
    }

    fn cmd_mkdir(&mut self, arg: &str, out: &mut dyn Out) {
        if arg.is_empty() {
            out.puts("mkdir: missing operand\n");
            return;
        }
        let dir = self.cwd();
        let mut fsg = FS.lock();
        if fsg.dircreate(dir, arg.as_bytes(), InodeKind::Dir).is_err() {
            out.puts("mkdir: cannot create directory\n");
        }
    }

    /// `touch NAME` — create an empty file.
    fn cmd_touch(&mut self, name: &str, out: &mut dyn Out) {
        // IMPLEMENT: create an empty file called `name` in the current directory.
        //
        //   1. If `name` is empty, print "touch: missing operand\n" and return.
        //   2. Get the current directory inode: `let dir = self.cwd();`
        //   3. Lock the filesystem:           `let mut fsg = FS.lock();`
        //   4. Create the file with `dircreate`, asking for a *File* (not a Dir):
        //        fsg.dircreate(dir, name.as_bytes(), InodeKind::File)
        //      `dircreate` returns Err(FsError::AlreadyExists) if the name is
        //      taken — for `touch` that is fine (a no-op), so only report *other*
        //      errors, e.g. print "touch: cannot create file\n".
        //   Tip: `match` on the result, with an arm for the AlreadyExists case.
        //        (You will need to add `FsError` to the `use crate::fs::{...}`
        //        line at the top of this file.)
        let _ = (name, out); // remove once implemented
    }

    /// `cat NAME` — print a file's contents.
    fn cmd_cat(&self, name: &str, out: &mut dyn Out) {
        // IMPLEMENT: look up `name` in the current directory and print its bytes.
        //
        //   1. `let dir = self.cwd();` then `let fsg = FS.lock();`
        //   2. Find the file's inode number:
        //        match fsg.dirlookup(dir, name.as_bytes()) { ... }
        //      On Err, print "cat: no such file\n" and return.
        //   3. Read into a fixed buffer the size of a file:
        //        let mut buf = [0u8; fs::FILESIZE];
        //        match fsg.read(inum, &mut buf) { ... }
        //      `read` gives Ok(n) = number of bytes read, or Err if it is a
        //      directory (print "cat: is a directory\n").
        //   4. Turn the first `n` bytes into text and print them:
        //        if let Ok(s) = core::str::from_utf8(&buf[..n]) { out.puts(s); }
        let _ = (name, out); // remove once implemented
    }

    /// `rm NAME` — delete a file (refuses directories; use `rmdir` for those).
    fn cmd_rm(&mut self, name: &str, out: &mut dyn Out) {
        // IMPLEMENT: delete the file `name` from the current directory.
        //
        //   1. If `name` is empty, print "rm: missing operand\n" and return.
        //   2. `let dir = self.cwd();` then `let mut fsg = FS.lock();`
        //   3. Find it with `dirlookup`; on Err print "rm: no such file\n", return.
        //   4. Refuse to remove a directory:
        //        if fsg.is_dir(inum) { out.puts("rm: is a directory\n"); return; }
        //   5. Remove it with `fsg.unlink(dir, name.as_bytes())` (ignore the
        //      result with `let _ = ...;` — the checks above already handled the
        //      error cases).
        //   Compare with `cmd_rmdir` below: it is the same shape, but it removes
        //   an *empty directory* instead.
        let _ = (name, out); // remove once implemented
    }

    /// `rmdir NAME` — delete an empty directory. (Given — read it as the model
    /// for `rm`: same shape, but it checks the target *is* an (empty) directory.)
    fn cmd_rmdir(&mut self, name: &str, out: &mut dyn Out) {
        if name.is_empty() {
            out.puts("rmdir: missing operand\n");
            return;
        }
        let dir = self.cwd();
        let mut fsg = FS.lock();
        let inum = match fsg.dirlookup(dir, name.as_bytes()) {
            Ok(i) => i,
            Err(_) => {
                out.puts("rmdir: no such directory\n");
                return;
            }
        };
        if !fsg.is_dir(inum) {
            out.puts("rmdir: not a directory\n");
            return;
        }
        if !fsg.dir_is_empty(inum) {
            out.puts("rmdir: directory not empty\n");
            return;
        }
        let _ = fsg.unlink(dir, name.as_bytes());
    }

    /// `echo TEXT > FILE` — write TEXT (plus a newline) into FILE, creating it if
    /// needed. With no `>`, just print TEXT. (Given — shows the write path and a
    /// little redirect parsing; your `cat` reads back what this writes.)
    fn cmd_echo(&mut self, line: &str, out: &mut dyn Out) {
        // `line` is the whole command, e.g. "echo hello world > notes.txt".
        let rest = line.strip_prefix("echo").unwrap_or(line).trim_start();
        match rest.split_once('>') {
            // No redirect: print the text to the console.
            None => {
                out.puts(rest);
                out.puts("\n");
            }
            // "echo TEXT > FILE": write TEXT + '\n' into FILE.
            Some((text, file)) => {
                let file = file.trim();
                if file.is_empty() {
                    out.puts("echo: missing file name after >\n");
                    return;
                }
                let mut contents = String::from(text.trim());
                contents.push('\n');

                let dir = self.cwd();
                let mut fsg = FS.lock();
                // Find the file, or create it if it does not exist yet.
                let inum = match fsg.dirlookup(dir, file.as_bytes()) {
                    Ok(i) => i,
                    Err(_) => match fsg.dircreate(dir, file.as_bytes(), InodeKind::File) {
                        Ok(i) => i,
                        Err(_) => {
                            out.puts("echo: cannot create file\n");
                            return;
                        }
                    },
                };
                if fsg.write(inum, contents.as_bytes()).is_err() {
                    out.puts("echo: write failed\n");
                }
            }
        }
    }
}

/// Output sink that writes to the UART console.
struct ConsoleOut;
impl Out for ConsoleOut {
    fn puts(&mut self, s: &str) {
        uart::puts(s);
    }
}

/// The interactive read-eval-print loop: print a prompt, read a line (echoing
/// each keystroke), run it, repeat. (Given — this is what `cargo run` uses.)
pub fn run() -> ! {
    let mut sh = Shell::new();
    let mut out = ConsoleOut;
    let mut line = String::new();
    out.puts("rv6$ ");
    loop {
        let c = console::getc();
        match c {
            b'\r' | b'\n' => {
                out.puts("\n");
                sh.exec(&line, &mut out);
                line.clear();
                out.puts("rv6$ ");
            }
            0x7f | 0x08 => {
                // backspace: erase one character on screen
                if line.pop().is_some() {
                    out.puts("\x08 \x08");
                }
            }
            c if c.is_ascii_graphic() || c == b' ' => {
                line.push(c as char);
                let one = [c];
                if let Ok(s) = core::str::from_utf8(&one) {
                    out.puts(s); // echo
                }
            }
            _ => {}
        }
    }
}
