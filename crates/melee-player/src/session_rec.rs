//! The player's session recordings: every `PADRead` the game takes from power-on, as ssbm-rs's
//! runner records a session (`SSBMINP1` version 2, melee-sim's `game::inputs`), with the card the
//! session started with beside it (`FILE.card`), so a session played here plays again in
//! ssbm-rs (`--inputs FILE`) and in the oracle.
//!
//! The file is `SSBMINP1`, the version (2), the disc rate in bytes a second (0 for Slippi's
//! fast disc speed), flags (1 sound mixed, 2 a card beside it), the clock at power-on (time
//! base ticks from 2000-01-01), the DTV status, the SRAM's language and flags; then a byte per
//! read: 0 for the controllers of the read before, else 1 and the four controllers, 11 bytes
//! each. Written as the session runs, flushed every couple of seconds.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use melee_sim::audio::clock::{DRIVE_RATE, DiscSpeed};
use melee_sim::audio::dsp::DspModel;
use melee_sim::game::inputs::{CARD, MAGIC, SOUND};
use melee_sim::game::session::Setup;
use melee_sim::input::controller::PadData;
use melee_sim::input::pad::{ERR_NONE, PadStatus};

const PAD_BYTES: usize = 11;

/// A read's controllers as ssbm-rs writes them: connected, the buttons big-endian, the
/// sticks, the triggers and the analog buttons; a controller not connected all zeros.
fn encode(pads: &PadData) -> [u8; 4 * PAD_BYTES] {
    let mut out = [0; 4 * PAD_BYTES];
    for (p, b) in pads.iter().zip(out.chunks_mut(PAD_BYTES)) {
        if p.err != ERR_NONE {
            continue;
        }
        let PadStatus {
            button,
            stick_x,
            stick_y,
            substick_x,
            substick_y,
            trigger_left,
            trigger_right,
            analog_a,
            analog_b,
            ..
        } = *p;
        let [b0, b1] = button.to_be_bytes();
        b.copy_from_slice(&[
            1,
            b0,
            b1,
            stick_x as u8,
            stick_y as u8,
            substick_x as u8,
            substick_y as u8,
            trigger_left,
            trigger_right,
            analog_a,
            analog_b,
        ]);
    }
    out
}

/// The header for a session with `setup`'s console, sound mixed or not, with a card beside it
/// or not.
pub fn header(setup: &Setup, card: bool) -> Vec<u8> {
    let mut h = MAGIC.to_vec();
    h.extend_from_slice(&2u32.to_le_bytes());
    let rate = match setup.disc {
        DiscSpeed::Drive => u64::from(DRIVE_RATE),
        _ => 0,
    };
    h.extend_from_slice(&rate.to_le_bytes());
    let sound = if setup.dsp == DspModel::Samples {
        SOUND
    } else {
        0
    };
    // A raised debug level, as ssbm-rs's `--develop` marks the file (level + 1, 0 for none).
    let develop = setup.develop.map_or(0, |d| {
        ((d.level + 1) as u8) << melee_sim::game::inputs::DEVELOP_SHIFT
            & melee_sim::game::inputs::DEVELOP_MASK
    });
    h.push(sound | if card { CARD } else { 0 } | develop);
    h.extend_from_slice(&setup.power_on.to_le_bytes());
    // The SRAM's language: Melee never reads it (its language is the save's).
    h.extend_from_slice(&[u8::from(setup.dtv), 0, setup.sram_flags]);
    h
}

/// A session's recording as it's written.
pub struct Writer {
    out: Option<BufWriter<File>>,
    last: Option<[u8; 4 * PAD_BYTES]>,
    reads: u64,
}

impl Writer {
    /// Starts the recording at `path` with the card the session starts with beside it.
    pub fn create(path: &Path, setup: &Setup) -> std::io::Result<Writer> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        if let Some(card) = &setup.card {
            std::fs::write(path.with_extension("card"), card)?;
        }
        let mut out = BufWriter::new(File::create(path)?);
        out.write_all(&header(setup, setup.card.is_some()))?;
        Ok(Writer {
            out: Some(out),
            last: None,
            reads: 0,
        })
    }

    /// A recording kept in memory only, for tests.
    #[cfg(test)]
    pub fn memory() -> Writer {
        Writer {
            out: None,
            last: None,
            reads: 0,
        }
    }

    /// One read the game took, its bytes appended to `sink` too when given.
    pub fn read(&mut self, pads: &PadData, sink: Option<&mut Vec<u8>>) {
        let bytes = encode(pads);
        let mut rec = Vec::with_capacity(1 + bytes.len());
        if self.last == Some(bytes) {
            rec.push(0);
        } else {
            self.last = Some(bytes);
            rec.push(1);
            rec.extend_from_slice(&bytes);
        }
        if let Some(s) = sink {
            s.extend_from_slice(&rec);
        }
        self.reads += 1;
        if let Some(out) = &mut self.out
            && let Err(e) = out.write_all(&rec)
        {
            eprintln!("the session's recording stopped: {e}");
            self.out = None;
        }
        // About two seconds' worth at a time, so a session cut short keeps nearly all of it.
        if self.reads.is_multiple_of(120)
            && let Some(out) = &mut self.out
        {
            let _ = out.flush();
        }
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        if let Some(out) = &mut self.out {
            let _ = out.flush();
        }
    }
}

/// The folder sessions are recorded in: `sessions` in the settings folder.
pub fn dir() -> PathBuf {
    crate::settings::folder().join("sessions")
}

/// How many recorded sessions the folder keeps: the oldest go as new ones start.
pub const KEEP: usize = 20;

/// A new session's recording path, `Session_YYYYMMDDTHHMMSS.inputs` by the local time, the
/// folder's oldest sessions past [`KEEP`] removed.
pub fn new_path(dir: &Path, local: [u32; 6]) -> PathBuf {
    let mut old: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|r| {
            r.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "inputs"))
                .collect()
        })
        .unwrap_or_default();
    old.sort();
    while old.len() >= KEEP {
        let p = old.remove(0);
        let _ = std::fs::remove_file(p.with_extension("card"));
        let _ = std::fs::remove_file(&p);
    }
    let [y, mo, d, h, mi, s] = local;
    let base = format!("Session_{y:04}{mo:02}{d:02}T{h:02}{mi:02}{s:02}");
    let mut path = dir.join(format!("{base}.inputs"));
    let mut n = 1;
    while path.exists() {
        path = dir.join(format!("{base}-{n}.inputs"));
        n += 1;
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use melee_sim::game::inputs::Recording;
    use melee_sim::input::pad::ERR_NO_CONTROLLER;

    /// What the writer writes, melee-sim's reader (ssbm-rs's format) reads back: the console's
    /// settings and every read, a repeated read as one byte.
    #[test]
    fn a_recording_reads_back() {
        let setup = Setup {
            disc: DiscSpeed::Drive,
            dsp: DspModel::Samples,
            card: None,
            power_on: 0x0123_4567_89AB,
            dtv: false,
            sram_flags: 0x2C,
            card_pull: None,
            develop: None,
        };
        let mut bytes = header(&setup, true);
        let mut w = Writer::memory();
        let none = PadStatus::error(ERR_NO_CONTROLLER);
        let a = PadStatus {
            button: 0x0100,
            stick_x: -5,
            trigger_left: 30,
            ..PadStatus::default()
        };
        let reads = [[a, none, none, none], [a, none, none, none], [none; 4]];
        for r in &reads {
            w.read(r, Some(&mut bytes));
        }
        assert_eq!(bytes.len(), 32 + 45 + 1 + 45);
        let rec = Recording::parse(&bytes, Path::new("s.inputs")).unwrap();
        assert_eq!(rec.version, 2);
        assert_eq!(rec.disc_rate, Some(u64::from(DRIVE_RATE)));
        assert!(rec.sound);
        assert_eq!(rec.card.as_deref(), Some(Path::new("s.card")));
        assert_eq!(
            (rec.power_on, rec.dtv, rec.sram_flags),
            (0x0123_4567_89AB, false, 0x2C)
        );
        assert_eq!(rec.reads, reads);
    }
}
