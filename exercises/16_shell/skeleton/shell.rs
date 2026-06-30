//! shell.rs — a tiny interactive shell (REPL).

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

    /// Parse one input line and run the command it names.
    pub fn exec(&mut self, line: &str, out: &mut dyn Out) {
        // IMPLEMENT: parse the line and dispatch to the right command handler.
        //
        //   1. Split `line` into whitespace-separated words. `str` has a handy
        //      method for this: `line.split_whitespace()` gives an iterator.
        //   2. The first word is the command. If there is no first word (a blank
        //      line), just `return`.
        //   3. The second word, if any, is the argument (use "" if absent):
        //        let arg = words.next().unwrap_or("");
        //   4. `match` on the command and call the matching handler (these are
        //      already written for you):
        //        "pwd"   => self.cmd_pwd(out),
        //        "ls"    => self.cmd_ls(out),
        //        "cd"    => self.cmd_cd(arg, out),
        //        "mkdir" => self.cmd_mkdir(arg, out),
        //        anything else => print "<cmd>: command not found\n" to `out`.
        let _ = (line, out); // remove once implemented
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
