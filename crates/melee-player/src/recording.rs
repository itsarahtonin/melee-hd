//! The player's matches saved as Slippi replays: melee-sim's recorder (`game::record`) runs
//! in every match of the player's own (not a replay's), and when the match is left its
//! recording goes to the replays folder as `Game_YYYYMMDDTHHMMSS.slp`, named by the local time
//! the match started, as Slippi Dolphin names its replays, or as `.slpz` with Compress replays
//! on (design call 12 of docs/milestone-7.md: off by default, as Slippi's own tools read only
//! `.slp`).

use std::path::PathBuf;
use std::time::SystemTime;

use melee_sim::game::record::Recorder;

/// Where a match's recording goes, and when it started.
pub struct Recording {
    dir: PathBuf,
    /// Saved compressed, as `.slpz` (`melee_slippi::slpz`).
    compress: bool,
    started: SystemTime,
    local: [u32; 6],
}

impl Recording {
    /// A recording of a match starting now, for `dir`, or the settings folder's replays folder,
    /// compressed or not.
    pub fn new(dir: Option<&str>, compress: bool) -> Recording {
        Recording {
            compress,
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
        let mut file = melee_slippi::write::recording(r, self.started);
        let mut ext = "slp";
        if self.compress {
            match melee_slippi::slpz::compress(&file) {
                Ok(z) => (file, ext) = (z, melee_slippi::slpz::EXTENSION),
                // Never expected of the recorder's own file: kept uncompressed rather than lost.
                Err(e) => eprintln!("the replay is saved uncompressed: {e}"),
            }
        }
        std::fs::create_dir_all(&self.dir).map_err(|e| format!("{}: {e}", self.dir.display()))?;
        let name = melee_slippi::write::file_name(self.local);
        let stem = name.trim_end_matches(".slp");
        // Two matches started in the same second keep both, compressed or not.
        let taken = |s: &str| {
            ["slp", melee_slippi::slpz::EXTENSION]
                .iter()
                .any(|x| self.dir.join(format!("{s}.{x}")).exists())
        };
        let mut base = stem.to_owned();
        let mut n = 1;
        while taken(&base) {
            base = format!("{stem}-{n}");
            n += 1;
        }
        let path = self.dir.join(format!("{base}.{ext}"));
        std::fs::write(&path, file).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Some(path))
    }
}

/// The settings folder's `replays` (before 2026-11-14, `replays` beside the executable, which
/// stays as it was: written nowhere, nothing moved).
pub fn default_dir() -> PathBuf {
    crate::settings::folder().join("replays")
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
