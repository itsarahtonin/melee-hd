//! Where the player's output goes. The release build on Windows is a windowed program without
//! a console of its own, so a double-click opens no console window. Started from a terminal,
//! it attaches to the terminal's console and prints there as a console program does; with its
//! output piped or redirected (a test reading it), it writes there. Started with neither (a
//! double-click), its output goes to `melee-hd.log` in the settings folder and fatal errors
//! show in a message box.

use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

/// No console and no redirected output: a double-click on the release build on Windows.
static QUIET: AtomicBool = AtomicBool::new(false);

/// Points the output at the parent's console, or at the log, as the start calls for.
pub fn attach() {
    #[cfg(all(windows, not(debug_assertions)))]
    windows::attach();
}

/// Reports an error that stops the player: printed, and in a message box when no one would
/// see it printed.
pub fn fatal(text: &str) {
    eprintln!("{text}");
    if QUIET.load(Relaxed) {
        crate::settings::error_box(text);
    }
}

#[cfg(all(windows, not(debug_assertions)))]
mod windows {
    use std::os::windows::io::IntoRawHandle;

    use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_TYPE_DISK, FILE_TYPE_PIPE, FILE_TYPE_UNKNOWN, GetFileType,
    };
    use windows_sys::Win32::System::Console::{
        ATTACH_PARENT_PROCESS, AttachConsole, GetStdHandle, STD_ERROR_HANDLE, STD_HANDLE,
        STD_OUTPUT_HANDLE, SetStdHandle,
    };

    /// A handle that writes somewhere: a console, a file or a pipe.
    fn usable(h: HANDLE) -> bool {
        !h.is_null() && h != INVALID_HANDLE_VALUE && unsafe { GetFileType(h) } != FILE_TYPE_UNKNOWN
    }

    /// A handle the parent redirected to a file or a pipe.
    fn redirected(h: HANDLE) -> bool {
        usable(h) && matches!(unsafe { GetFileType(h) }, FILE_TYPE_DISK | FILE_TYPE_PIPE)
    }

    /// Sets each of `ids` that writes nowhere to `open`'s file (std reads the handles anew at
    /// each write).
    fn fill(ids: &[STD_HANDLE], open: impl Fn() -> std::io::Result<std::fs::File>) {
        let mut file: Option<HANDLE> = None;
        for &id in ids {
            if usable(unsafe { GetStdHandle(id) }) {
                continue;
            }
            if file.is_none() {
                file = open().ok().map(|f| f.into_raw_handle() as HANDLE);
            }
            if let Some(h) = file {
                unsafe { SetStdHandle(id, h) };
            }
        }
    }

    pub fn attach() {
        let ids = [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE];
        if ids
            .iter()
            .all(|&id| redirected(unsafe { GetStdHandle(id) }))
        {
            return;
        }
        if unsafe { AttachConsole(ATTACH_PARENT_PROCESS) } != 0 {
            fill(&ids, || {
                std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open("CONOUT$")
            });
            return;
        }
        super::QUIET.store(true, super::Relaxed);
        fill(&ids, || {
            let folder = crate::settings::folder();
            std::fs::create_dir_all(&folder)?;
            std::fs::File::create(folder.join("melee-hd.log"))
        });
    }
}
