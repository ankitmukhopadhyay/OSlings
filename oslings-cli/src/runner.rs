//! The test harness: compiles the rv6 kernel and, for QEMU exercises, boots
//! it and inspects the serial output for the pass marker.

use crate::model::{Exercise, Mode, Project};
use anyhow::{Context, Result};
use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const TARGET: &str = "riscv64gc-unknown-none-elf";
// A correct kernel powers off in well under a second; only a kernel that
// faulted before reaching its exit path will run out this clock.
const QEMU_TIMEOUT: Duration = Duration::from_secs(10);

/// Result of running one exercise's test.
pub struct Outcome {
    pub passed: bool,
    /// Human-facing explanation of what happened.
    pub summary: String,
    /// Captured compiler / QEMU output, for the learner to read.
    pub detail: String,
}

pub fn run(project: &Project, ex: &Exercise) -> Result<Outcome> {
    match ex.mode {
        Mode::Build => run_build(project, ex),
        Mode::Qemu => run_qemu(project, ex),
    }
}

/// Compile rv6 for the bare-metal target. Success = it builds.
fn run_build(project: &Project, ex: &Exercise) -> Result<Outcome> {
    let output = cargo_build(project, ex)?;
    let detail = combine(&output.stdout, &output.stderr);
    if output.status.success() {
        Ok(Outcome {
            passed: true,
            summary: format!("kernel compiles for {TARGET}"),
            detail,
        })
    } else {
        Ok(Outcome {
            passed: false,
            summary: "kernel failed to compile".into(),
            detail,
        })
    }
}

/// Compile, then boot in QEMU and look for the pass marker on the serial line.
fn run_qemu(project: &Project, ex: &Exercise) -> Result<Outcome> {
    let build = cargo_build(project, ex)?;
    if !build.status.success() {
        return Ok(Outcome {
            passed: false,
            summary: "kernel failed to compile".into(),
            detail: combine(&build.stdout, &build.stderr),
        });
    }

    let kernel = project
        .rv6_dir()
        .join("target")
        .join(TARGET)
        .join("debug")
        .join("rv6");
    if !kernel.exists() {
        return Ok(Outcome {
            passed: false,
            summary: format!("kernel binary not found at {}", kernel.display()),
            detail: combine(&build.stdout, &build.stderr),
        });
    }

    let (serial, timed_out) = boot_qemu(&kernel)?;

    let pass = &project.info.meta.pass_marker;
    let fail = &project.info.meta.fail_marker;

    if serial.contains(pass) {
        Ok(Outcome {
            passed: true,
            summary: format!("kernel booted and printed `{pass}`"),
            detail: serial,
        })
    } else if serial.contains(fail) {
        Ok(Outcome {
            passed: false,
            summary: format!("kernel booted but reported `{fail}`"),
            detail: serial,
        })
    } else if timed_out {
        Ok(Outcome {
            passed: false,
            summary: format!(
                "QEMU timed out without printing `{pass}` \
                 (kernel likely faulted before reaching kmain — check the stack setup)"
            ),
            detail: serial,
        })
    } else {
        Ok(Outcome {
            passed: false,
            summary: format!("kernel exited without printing `{pass}`"),
            detail: serial,
        })
    }
}

fn cargo_build(project: &Project, ex: &Exercise) -> Result<std::process::Output> {
    let mut cmd = Command::new("cargo");
    cmd.arg("build").current_dir(project.rv6_dir());
    // Part 2 exercises build with `--features harness` so the kernel runs its
    // boot self-check (OSLINGS:PASS) instead of dropping into the interactive OS.
    if !ex.features.is_empty() {
        cmd.arg("--features").arg(ex.features.join(","));
    }
    cmd.output()
        .context("failed to run `cargo build` — is cargo on your PATH?")
}

/// Boot the kernel in QEMU, capturing serial output. Returns the captured
/// text and whether we had to kill QEMU due to timeout.
fn boot_qemu(kernel: &std::path::Path) -> Result<(String, bool)> {
    let mut child = Command::new("qemu-system-riscv64")
        .args([
            "-machine", "virt",
            "-bios", "none",
            "-m", "128M",
            "-smp", "1",
            "-nographic",
            "-serial", "mon:stdio",
            "-kernel",
        ])
        .arg(kernel)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to start qemu-system-riscv64 — is QEMU installed?")?;

    // Read stdout on a background thread so a chatty kernel can't deadlock us.
    let mut stdout = child.stdout.take().unwrap();
    let reader = thread::spawn(move || {
        let mut buf = String::new();
        let _ = stdout.read_to_string(&mut buf);
        buf
    });

    // Poll for exit; kill if it overruns the timeout.
    let start = Instant::now();
    let mut timed_out = false;
    loop {
        match child.try_wait()? {
            Some(_) => break,
            None => {
                if start.elapsed() >= QEMU_TIMEOUT {
                    let _ = child.kill();
                    let _ = child.wait();
                    timed_out = true;
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }
        }
    }

    let mut serial = reader.join().unwrap_or_default();
    if let Some(mut err) = child.stderr.take() {
        let mut e = String::new();
        let _ = err.read_to_string(&mut e);
        if !e.trim().is_empty() {
            serial.push_str("\n[qemu stderr]\n");
            serial.push_str(&e);
        }
    }
    Ok((serial, timed_out))
}

fn combine(stdout: &[u8], stderr: &[u8]) -> String {
    let mut s = String::new();
    s.push_str(&String::from_utf8_lossy(stdout));
    s.push_str(&String::from_utf8_lossy(stderr));
    s
}
