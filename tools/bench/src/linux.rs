//! The few Linux calls the Dolphin harness needs, as `win.rs` has them on Windows: running
//! Dolphin so that it ends with the harness, and reading its memory.
//!
//! Dolphin is the harness's own child, so the harness may read its memory where ptrace's scope
//! is 1 (only descendants): through `/proc/PID/maps` and `/proc/PID/mem`, no ptrace attach.

use std::fs::File;
use std::io;
use std::os::fd::AsFd as _;
use std::os::unix::fs::FileExt as _;
use std::os::unix::process::{CommandExt as _, ExitStatusExt as _};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

unsafe extern "C" {
    fn prctl(option: i32, ...) -> i32;
    fn getppid() -> i32;
}

const PR_SET_PDEATHSIG: i32 = 1;
const SIGKILL: u64 = 9;
const ESRCH: i32 = 3;

/// A program the harness started (no window: Dolphin runs on its headless platform). The kernel
/// kills it if the thread that started it ends, so a harness that is killed, panics or times out
/// never leaves Dolphin running.
pub struct Hidden {
    child: Mutex<Child>,
    pub pid: u32,
}

impl Hidden {
    /// Starts `exe` with `args` in `dir`. Its output goes to the harness's standard error, so the
    /// harness's tables stay alone on standard output.
    pub fn spawn(exe: &str, args: &[String], dir: &str) -> io::Result<Hidden> {
        let parent = std::process::id();
        let stderr = io::stderr().as_fd().try_clone_to_owned()?;
        let mut cmd = Command::new(exe);
        cmd.args(args)
            .current_dir(dir)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stderr));
        // SAFETY: between fork and exec the closure makes only async-signal-safe calls (prctl,
        // getppid) and allocates nothing.
        unsafe {
            cmd.pre_exec(move || {
                if prctl(PR_SET_PDEATHSIG, SIGKILL) != 0 {
                    return Err(io::Error::last_os_error());
                }
                // The harness ended before the signal was set up: nothing would kill the child.
                if getppid() as u32 != parent {
                    return Err(io::Error::from_raw_os_error(ESRCH));
                }
                Ok(())
            });
        }
        let child = cmd.spawn()?;
        Ok(Hidden {
            pid: child.id(),
            child: Mutex::new(child),
        })
    }

    /// Whether it has exited, waiting up to `ms` milliseconds.
    pub fn wait(&self, ms: u32) -> bool {
        let end = Instant::now() + Duration::from_millis(u64::from(ms));
        loop {
            if self.status().is_some() {
                return true;
            }
            if Instant::now() >= end {
                return false;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn status(&self) -> Option<std::process::ExitStatus> {
        // `try_wait` keeps the status once it has it.
        self.child.lock().unwrap().try_wait().ok().flatten()
    }

    /// Its exit code, once it has exited (128 plus the signal if a signal ended it).
    pub fn exit_code(&self) -> Option<u32> {
        let s = self.status()?;
        s.code()
            .map(|c| c as u32)
            .or_else(|| s.signal().map(|n| 128 + n as u32))
    }

    /// Stops it and waits until it has gone; true if it has.
    pub fn kill(&self) -> bool {
        if self.status().is_none() {
            let _ = self.child.lock().unwrap().kill();
        }
        self.wait(10_000)
    }
}

impl Drop for Hidden {
    fn drop(&mut self) {
        if !self.wait(0) {
            self.kill();
        }
    }
}

/// A child process's memory, opened for reading.
pub struct Process {
    pid: u32,
    mem: File,
}

impl Process {
    pub fn open(pid: u32) -> io::Result<Process> {
        Ok(Process {
            pid,
            mem: File::open(format!("/proc/{pid}/mem"))?,
        })
    }

    /// Reads `buf.len()` bytes at `addr`; false if they can't all be read.
    pub fn read(&self, addr: usize, buf: &mut [u8]) -> bool {
        self.mem.read_exact_at(buf, addr as u64).is_ok()
    }

    /// Its readable shared mappings (Dolphin's emulated memory is a shared memory file mapped
    /// into it) at least `min` bytes long: (base, size).
    pub fn mapped_regions(&self, min: usize) -> Vec<(usize, usize)> {
        std::fs::read_to_string(format!("/proc/{}/maps", self.pid))
            .map(|maps| shared_regions(&maps, min))
            .unwrap_or_default()
    }
}

/// The readable shared mappings at least `min` bytes long in a `/proc/PID/maps` listing.
fn shared_regions(maps: &str, min: usize) -> Vec<(usize, usize)> {
    maps.lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let (start, end) = fields.next()?.split_once('-')?;
            let perms = fields.next()?.as_bytes();
            let start = usize::from_str_radix(start, 16).ok()?;
            let size = usize::from_str_radix(end, 16).ok()?.checked_sub(start)?;
            (perms.first() == Some(&b'r') && perms.get(3) == Some(&b's') && size >= min)
                .then_some((start, size))
        })
        .collect()
}

/// A no-op: raising a thread's priority needs privileges the harness doesn't have here.
pub fn raise_thread_priority() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_readable_shared_mappings() {
        let maps = "\
5e2a4b000000-5e2a4b2c0000 r--p 00000000 103:02 1234 /usr/bin/dolphin-emu-nogui
7f0000000000-7f0001800000 rw-s 00000000 00:01 77 /memfd:dolphin-emu.4242 (deleted)
7f0002000000-7f0002400000 rw-s 01800000 00:01 77 /memfd:dolphin-emu.4242 (deleted)
7f1000000000-7f1100000000 ---p 00000000 00:00 0
7f1180000000-7f1181800000 rw-s 00000000 00:01 77 /memfd:dolphin-emu.4242 (deleted)
7f2000000000-7f2004000000 rw-p 00000000 00:00 0
7f3000000000-7f3001800000 -w-s 00000000 00:01 77 /memfd:dolphin-emu.4242 (deleted)
";
        assert_eq!(
            shared_regions(maps, 0x180_0000),
            vec![
                (0x7f00_0000_0000, 0x180_0000),
                (0x7f11_8000_0000, 0x180_0000)
            ]
        );
        assert_eq!(shared_regions(maps, 0).len(), 3);
        assert!(shared_regions("garbage\n\n", 0).is_empty());
    }

    /// A harness that ends however it ends (here the thread that started the child, without
    /// dropping it) takes Dolphin with it.
    #[test]
    fn a_child_ends_with_the_thread_that_started_it() {
        let pid = std::thread::spawn(|| {
            let child = Hidden::spawn("sleep", &["30".into()], "/").unwrap();
            let pid = child.pid;
            std::mem::forget(child);
            pid
        })
        .join()
        .unwrap();
        // Killed, it stays a zombie of this process (nothing reaps it).
        let state = || {
            let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
            stat.rsplit_once(") ").unwrap().1.chars().next().unwrap()
        };
        let end = Instant::now() + Duration::from_secs(5);
        while state() != 'Z' && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(state(), 'Z');
    }

    #[test]
    fn a_child_ends_and_its_memory_reads() {
        let child = Hidden::spawn("sleep", &["30".into()], "/").unwrap();
        let process = Process::open(child.pid).unwrap();
        // The child's stack is mapped and readable, as Dolphin's memory will be.
        let maps = std::fs::read_to_string(format!("/proc/{}/maps", child.pid)).unwrap();
        let stack = maps.lines().find(|l| l.ends_with("[stack]")).unwrap();
        let range = stack.split_whitespace().next().unwrap();
        let end = usize::from_str_radix(range.split_once('-').unwrap().1, 16).unwrap();
        let mut b = [0u8; 16];
        assert!(process.read(end - 16, &mut b));
        assert!(!process.read(0, &mut b));
        assert!(!child.wait(0));
        assert!(child.kill());
        assert_eq!(child.exit_code(), Some(128 + 9));
    }
}
