//! The player's matches saved as Slippi replays: melee-sim's recorder (`game::record`) runs
//! in every match of the player's own (not a replay's), and when the match is left its
//! recording goes to the replays folder as `Game_YYYYMMDDTHHMMSS.slp`, named by the local time
//! the match started, as Slippi Dolphin names its replays.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use melee_sim::game::record::Recorder;

/// Where a match's recording goes, and when it started.
pub struct Recording {
    dir: PathBuf,
    started: SystemTime,
    local: [u32; 6],
}

impl Recording {
    /// A recording of a match starting now, for `dir`, or the replays folder beside the
    /// executable.
    pub fn new(dir: Option<&str>) -> Recording {
        Recording {
            dir: dir
                .filter(|d| !d.is_empty())
                .map_or_else(default_dir, PathBuf::from),
            started: SystemTime::now(),
            local: local_now(),
        }
    }

    /// Writes the recording, unless nothing was recorded: the file's path.
    pub fn save(&self, r: &Recorder) -> Result<Option<PathBuf>, String> {
        if r.last_frame().is_none() {
            return Ok(None);
        }
        let file = melee_slippi::write::recording(r, self.started);
        std::fs::create_dir_all(&self.dir).map_err(|e| format!("{}: {e}", self.dir.display()))?;
        let name = melee_slippi::write::file_name(self.local);
        let mut path = self.dir.join(&name);
        // Two matches started in the same second keep both.
        let mut n = 1;
        while path.exists() {
            path = self
                .dir
                .join(format!("{}-{n}.slp", name.trim_end_matches(".slp")));
            n += 1;
        }
        std::fs::write(&path, file).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Some(path))
    }
}

/// `replays` beside the executable (the working folder if its path is unknown).
pub fn default_dir() -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_default();
    exe.parent().unwrap_or(Path::new(".")).join("replays")
}

/// The local date and time of day, as Slippi names replays by: on Windows the system's local
/// time, elsewhere UTC.
pub fn local_now() -> [u32; 6] {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::SYSTEMTIME;
        use windows_sys::Win32::System::SystemInformation::GetLocalTime;
        let mut t = SYSTEMTIME {
            wYear: 0,
            wMonth: 0,
            wDayOfWeek: 0,
            wDay: 0,
            wHour: 0,
            wMinute: 0,
            wSecond: 0,
            wMilliseconds: 0,
        };
        // SAFETY: GetLocalTime fills the struct it's given.
        unsafe { GetLocalTime(&mut t) };
        [t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond].map(u32::from)
    }
    #[cfg(not(windows))]
    {
        melee_slippi::write::civil(SystemTime::now())
    }
}
