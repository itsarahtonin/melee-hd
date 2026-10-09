//! The few Windows calls the Dolphin harness needs: running a program on a desktop of its own
//! (as ssbm-rs's `tools/gx/hidden.ps1` does), and reading another process's memory.

#![allow(non_snake_case, clippy::upper_case_acronyms)]

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::ptr::null_mut;

type HANDLE = *mut c_void;
type BOOL = i32;

#[repr(C)]
struct STARTUPINFOW {
    cb: u32,
    lpReserved: *mut u16,
    lpDesktop: *mut u16,
    lpTitle: *mut u16,
    dwX: u32,
    dwY: u32,
    dwXSize: u32,
    dwYSize: u32,
    dwXCountChars: u32,
    dwYCountChars: u32,
    dwFillAttribute: u32,
    dwFlags: u32,
    wShowWindow: u16,
    cbReserved2: u16,
    lpReserved2: *mut u8,
    hStdInput: HANDLE,
    hStdOutput: HANDLE,
    hStdError: HANDLE,
}

#[repr(C)]
struct PROCESS_INFORMATION {
    hProcess: HANDLE,
    hThread: HANDLE,
    dwProcessId: u32,
    dwThreadId: u32,
}

#[repr(C)]
struct MEMORY_BASIC_INFORMATION {
    BaseAddress: *mut c_void,
    AllocationBase: *mut c_void,
    AllocationProtect: u32,
    PartitionId: u16,
    RegionSize: usize,
    State: u32,
    Protect: u32,
    Type: u32,
}

const MEM_COMMIT: u32 = 0x1000;
const MEM_MAPPED: u32 = 0x40000;
const PROCESS_VM_READ: u32 = 0x10;
const PROCESS_QUERY_INFORMATION: u32 = 0x400;
const GENERIC_ALL: u32 = 0x1000_0000;
const WAIT_OBJECT_0: u32 = 0;
const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: i32 = 9;
const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x2000;

#[repr(C)]
#[derive(Default)]
struct JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
    PerProcessUserTimeLimit: i64,
    PerJobUserTimeLimit: i64,
    LimitFlags: u32,
    MinimumWorkingSetSize: usize,
    MaximumWorkingSetSize: usize,
    ActiveProcessLimit: u32,
    Affinity: usize,
    PriorityClass: u32,
    SchedulingClass: u32,
    IoCounters: [u64; 6],
    ProcessMemoryLimit: usize,
    JobMemoryLimit: usize,
    PeakProcessMemoryUsed: usize,
    PeakJobMemoryUsed: usize,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateProcessW(
        app: *const u16,
        cmd: *mut u16,
        pa: *mut c_void,
        ta: *mut c_void,
        inherit: BOOL,
        flags: u32,
        env: *mut c_void,
        dir: *const u16,
        si: *mut STARTUPINFOW,
        pi: *mut PROCESS_INFORMATION,
    ) -> BOOL;
    fn WaitForSingleObject(h: HANDLE, ms: u32) -> u32;
    fn TerminateProcess(h: HANDLE, code: u32) -> BOOL;
    fn GetExitCodeProcess(h: HANDLE, code: *mut u32) -> BOOL;
    fn CloseHandle(h: HANDLE) -> BOOL;
    fn GetCurrentThread() -> HANDLE;
    fn SetThreadPriority(thread: HANDLE, priority: i32) -> BOOL;
    fn CreateJobObjectW(sa: *mut c_void, name: *const u16) -> HANDLE;
    fn SetInformationJobObject(job: HANDLE, class: i32, info: *mut c_void, len: u32) -> BOOL;
    fn AssignProcessToJobObject(job: HANDLE, process: HANDLE) -> BOOL;
    fn OpenProcess(access: u32, inherit: BOOL, pid: u32) -> HANDLE;
    fn ReadProcessMemory(
        h: HANDLE,
        base: *const c_void,
        buf: *mut c_void,
        size: usize,
        read: *mut usize,
    ) -> BOOL;
    fn VirtualQueryEx(
        h: HANDLE,
        addr: *const c_void,
        info: *mut MEMORY_BASIC_INFORMATION,
        len: usize,
    ) -> usize;
}

#[link(name = "user32")]
unsafe extern "system" {
    fn CreateDesktopW(
        name: *const u16,
        device: *const u16,
        devmode: *mut c_void,
        flags: u32,
        access: u32,
        sa: *mut c_void,
    ) -> HANDLE;
    fn CloseDesktop(desk: HANDLE) -> BOOL;
}

fn wide(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s).encode_wide().chain([0]).collect()
}

/// The desktop nobody sees that hidden programs run on.
const DESKTOP: &str = "melee-bench-hidden";

/// A program running on a desktop of its own: its windows never show or take the focus.
pub struct Hidden {
    process: HANDLE,
    thread: HANDLE,
    desktop: HANDLE,
    /// A job that ends the program when the harness's handle to it closes, even if the harness
    /// itself is killed, so a stopped harness never leaves Dolphin running unseen.
    job: HANDLE,
    pub pid: u32,
}

impl Hidden {
    /// Starts `exe` with the command line `args` (already quoted) in `dir`.
    pub fn spawn(exe: &str, args: &str, dir: &str) -> std::io::Result<Hidden> {
        let name = wide(DESKTOP);
        // SAFETY: plain Win32 calls with valid, NUL-terminated strings and zeroed structures.
        unsafe {
            let desktop = CreateDesktopW(
                name.as_ptr(),
                std::ptr::null(),
                null_mut(),
                0,
                GENERIC_ALL,
                null_mut(),
            );
            if desktop.is_null() {
                return Err(std::io::Error::last_os_error());
            }
            let mut desk_name = wide(DESKTOP);
            let mut si: STARTUPINFOW = std::mem::zeroed();
            si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
            si.lpDesktop = desk_name.as_mut_ptr();
            let mut pi: PROCESS_INFORMATION = std::mem::zeroed();
            let mut cmd = wide(&format!("\"{exe}\" {args}"));
            let exe_w = wide(exe);
            let dir_w = wide(dir);
            if CreateProcessW(
                exe_w.as_ptr(),
                cmd.as_mut_ptr(),
                null_mut(),
                null_mut(),
                0,
                0,
                null_mut(),
                dir_w.as_ptr(),
                &mut si,
                &mut pi,
            ) == 0
            {
                let err = std::io::Error::last_os_error();
                CloseDesktop(desktop);
                return Err(err);
            }
            let job = CreateJobObjectW(null_mut(), std::ptr::null());
            if !job.is_null() {
                let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
                    LimitFlags: JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                    ..Default::default()
                };
                SetInformationJobObject(
                    job,
                    JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
                    (&mut info as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                AssignProcessToJobObject(job, pi.hProcess);
            }
            Ok(Hidden {
                process: pi.hProcess,
                thread: pi.hThread,
                desktop,
                job,
                pid: pi.dwProcessId,
            })
        }
    }

    /// Whether it has exited, waiting up to `ms` milliseconds.
    pub fn wait(&self, ms: u32) -> bool {
        // SAFETY: the process handle stays open until drop.
        unsafe { WaitForSingleObject(self.process, ms) == WAIT_OBJECT_0 }
    }

    /// Its exit code, once it has exited.
    pub fn exit_code(&self) -> Option<u32> {
        let mut code = 0;
        // SAFETY: as above.
        let ok = unsafe { GetExitCodeProcess(self.process, &mut code) } != 0;
        (ok && self.wait(0)).then_some(code)
    }

    /// Stops it and waits until it has gone; true if it has.
    pub fn kill(&self) -> bool {
        // SAFETY: as above.
        unsafe {
            TerminateProcess(self.process, 1);
        }
        self.wait(10_000)
    }
}

impl Drop for Hidden {
    fn drop(&mut self) {
        if !self.wait(0) {
            self.kill();
        }
        // SAFETY: handles opened by `spawn`, closed once.
        unsafe {
            CloseHandle(self.thread);
            CloseHandle(self.process);
            CloseDesktop(self.desktop);
            if !self.job.is_null() {
                CloseHandle(self.job);
            }
        }
    }
}

/// Another process's memory, opened for reading.
pub struct Process(HANDLE);

impl Process {
    pub fn open(pid: u32) -> std::io::Result<Process> {
        // SAFETY: a plain Win32 call; the handle is closed on drop.
        let h = unsafe { OpenProcess(PROCESS_VM_READ | PROCESS_QUERY_INFORMATION, 0, pid) };
        if h.is_null() {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(Process(h))
        }
    }

    /// Reads `buf.len()` bytes at `addr`; false if they can't all be read.
    pub fn read(&self, addr: usize, buf: &mut [u8]) -> bool {
        let mut n = 0;
        // SAFETY: `buf` is valid for writes of its length.
        let ok = unsafe {
            ReadProcessMemory(
                self.0,
                addr as *const c_void,
                buf.as_mut_ptr().cast(),
                buf.len(),
                &mut n,
            )
        };
        ok != 0 && n == buf.len()
    }

    /// Committed mapped regions at least `min` bytes long: (base, size).
    pub fn mapped_regions(&self, min: usize) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        let mut addr = 0usize;
        loop {
            // SAFETY: `info` is a valid out-structure of the size passed.
            let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
            let got = unsafe {
                VirtualQueryEx(
                    self.0,
                    addr as *const c_void,
                    &mut info,
                    std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
                )
            };
            if got == 0 {
                break;
            }
            let base = info.BaseAddress as usize;
            if info.State == MEM_COMMIT && info.Type == MEM_MAPPED && info.RegionSize >= min {
                out.push((base, info.RegionSize));
            }
            let next = base.wrapping_add(info.RegionSize);
            if next <= addr {
                break;
            }
            addr = next;
        }
        out
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        // SAFETY: opened by `open`, closed once.
        unsafe {
            CloseHandle(self.0);
        }
    }
}

/// Raises the calling thread's priority a step, so the harness's sampler wakes on time on a busy
/// machine.
pub fn raise_thread_priority() {
    const THREAD_PRIORITY_ABOVE_NORMAL: i32 = 1;
    // SAFETY: plain Win32 calls on the current thread's pseudo-handle.
    unsafe {
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_ABOVE_NORMAL);
    }
}

#[repr(C)]
struct SYMBOL_INFOW {
    SizeOfStruct: u32,
    TypeIndex: u32,
    Reserved: [u64; 2],
    Index: u32,
    Size: u32,
    ModBase: u64,
    Flags: u32,
    Value: u64,
    Address: u64,
    Register: u32,
    Scope: u32,
    Tag: u32,
    NameLen: u32,
    MaxNameLen: u32,
    Name: [u16; 1024],
}

#[repr(C)]
struct IMAGEHLP_LINEW64 {
    SizeOfStruct: u32,
    Key: *mut c_void,
    LineNumber: u32,
    FileName: *mut u16,
    Address: u64,
}

#[link(name = "dbghelp")]
unsafe extern "system" {
    fn SymInitializeW(process: HANDLE, path: *const u16, invade: BOOL) -> BOOL;
    fn SymFromAddrW(process: HANDLE, addr: u64, disp: *mut u64, sym: *mut SYMBOL_INFOW) -> BOOL;
    fn SymGetLineFromAddrW64(
        process: HANDLE,
        addr: u64,
        disp: *mut u32,
        line: *mut IMAGEHLP_LINEW64,
    ) -> BOOL;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> HANDLE;
    fn GetModuleHandleW(name: *const u16) -> HANDLE;
}

/// The names of functions at offsets (RVAs) in this executable, from its PDB: for profiles
/// whose symbols the profiler couldn't find.
pub fn symbolize(rvas: &[u64]) -> Vec<Option<String>> {
    // SAFETY: plain dbghelp calls on this process; the symbol buffer outlives each call.
    unsafe {
        let process = GetCurrentProcess();
        let base = GetModuleHandleW(null_mut()) as u64;
        // The executable names its PDB without a directory: search beside it.
        let dir = std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|d| d.as_os_str().to_owned()))
            .unwrap_or_default();
        let dir: Vec<u16> = dir.encode_wide().chain(Some(0)).collect();
        SymInitializeW(process, dir.as_ptr(), 1);
        rvas.iter()
            .map(|&rva| {
                let mut sym: SYMBOL_INFOW = std::mem::zeroed();
                sym.SizeOfStruct = 88;
                sym.MaxNameLen = 1024;
                let mut disp = 0u64;
                if SymFromAddrW(process, base + rva, &mut disp, &mut sym) == 0 {
                    return None;
                }
                let name = String::from_utf16_lossy(&sym.Name[..sym.NameLen.min(1024) as usize]);
                // The source line, which folded functions and missing symbols don't confuse.
                let mut line: IMAGEHLP_LINEW64 = std::mem::zeroed();
                line.SizeOfStruct = std::mem::size_of::<IMAGEHLP_LINEW64>() as u32;
                let mut d = 0u32;
                if SymGetLineFromAddrW64(process, base + rva, &mut d, &mut line) == 0 {
                    return Some(name);
                }
                let mut n = 0;
                while *line.FileName.add(n) != 0 {
                    n += 1;
                }
                let file = String::from_utf16_lossy(std::slice::from_raw_parts(line.FileName, n));
                let short: Vec<&str> = file.rsplit(['\\', '/']).take(2).collect();
                Some(format!(
                    "{name} ({}/{}:{})",
                    short[short.len() - 1],
                    short[0],
                    line.LineNumber
                ))
            })
            .collect()
    }
}
