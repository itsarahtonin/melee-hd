//! A console session ssbm-rs recorded (`SSBMINP1`, `tools/run/src/inputs.rs` there): the
//! settings its course depends on and the controllers of every `PADRead`, which a
//! [`Session`](super::session::Session) plays from power-on (the oracle's session check, the
//! frames checks' sessions).

use std::path::{Path, PathBuf};

use crate::audio::clock::DiscSpeed;
use crate::audio::dsp::DspModel;
use crate::input::controller::PadData;
use crate::input::pad::{ERR_NO_CONTROLLER, ERR_NONE, PadStatus};

pub const MAGIC: &[u8; 8] = b"SSBMINP1";
pub const SOUND: u8 = 1;
pub const CARD: u8 = 2;
/// The flags' bits 4 to 6: the debug level a `develop.ini` set (ssbm-rs's `--develop`), plus
/// one, 0 for none.
pub const DEVELOP_SHIFT: u8 = 4;
pub const DEVELOP_MASK: u8 = 0x70;
/// The flags' bit 3: the level was raised alone, with no `/develop.ini` (ssbm-rs's
/// `DBLEVEL=N` from its main 1158d12, melee-hd's developer setting).
pub const LEVEL_ALONE: u8 = 0x08;
/// The flags' bit 2: the session ran under the Slippi ruleset from power-on (ssbm-rs's
/// `--ruleset slippi`, [`super::state::Ruleset`]), in Slippi's floating point, with no card
/// and no debug level.
pub const SLIPPI_RULESET: u8 = 0x04;
/// The flags' bit 7: with no disc rate, the disc ran at Slippi Dolphin's fast disc speed
/// ([`DiscSpeed::Fast`], ssbm-rs's `DiscSpeed::Dolphin` since its `05d550a`); without it, at the
/// flat model every earlier recording ran ([`DiscSpeed::FastFlat`]).
pub const DOLPHIN_DISC: u8 = 0x80;
const PAD_BYTES: usize = 11;

/// A session ssbm-rs recorded: the settings its course depends on, and the controllers of
/// every `PADRead`, in order.
///
/// The file is `SSBMINP1`, a version (u32), the disc rate in bytes a second (u64, 0 for none)
/// and flags (u8: 1 sound mixed, 2 a card beside it, 4 the Slippi ruleset, 8 the debug level
/// raised alone, in bits 4 to 6 the debug level plus one, 0 for none, and 0x80 that with no
/// disc rate the disc ran at Dolphin's fast speed, [`DOLPHIN_DISC`]), all little-endian. Version 2 adds the
/// console's settings the boot reads: the clock at power-on (u64, time base ticks from
/// 2000-01-01), the VI's DTV status (u8, 1 a component cable) and the SRAM's language and
/// flags (u8 each); a version 1 file has ssbm-rs's defaults (0, 0, 0 and 0x2C). Then a byte per
/// read: 0 for the controllers of the read before, or 1 and the four controllers, 11 bytes
/// each.
pub struct Recording {
    /// The disc's read rate in bytes a second, none for Slippi's fast disc speed.
    pub disc_rate: Option<u64>,
    /// With no rate, whether the fast speed was Dolphin's ([`DOLPHIN_DISC`]) rather than the
    /// flat model's.
    pub dolphin_disc: bool,
    /// Whether the DSP mixed sound (the sample-accurate model) rather than only answering.
    pub sound: bool,
    /// The memory card the session started with (beside the recording, as `FILE.card`).
    pub card: Option<PathBuf>,
    /// The file's version, 1 or 2.
    pub version: u32,
    /// The console's clock at power-on, in time base ticks from 2000-01-01.
    pub power_on: u64,
    /// Whether the VI reports a component cable (`VIGetDTVStatus`).
    pub dtv: bool,
    /// The SRAM's language, which Melee never reads (its language is the save's).
    pub sram_language: u8,
    /// The SRAM's flags (0x04 stereo, 0x80 progressive scan).
    pub sram_flags: u8,
    /// The debug level the session ran at (0 to 4), none for a retail disc's: as from a disc
    /// with `/develop.ini` (ssbm-rs's `--develop N`), or raised alone ([`LEVEL_ALONE`],
    /// `develop_ini` false: ssbm-rs's `DBLEVEL=N`, the player's developer setting).
    pub develop: Option<i32>,
    pub develop_ini: bool,
    /// The ruleset the session ran ([`SLIPPI_RULESET`]).
    pub ruleset: super::state::Ruleset,
    pub reads: Vec<PadData>,
}

impl Recording {
    pub fn read(path: &Path) -> Result<Recording, String> {
        let data = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Recording::parse(&data, path)
    }

    pub fn parse(data: &[u8], path: &Path) -> Result<Recording, String> {
        if data.len() < 21 || &data[..8] != MAGIC {
            return Err(format!("{}: not a controller recording", path.display()));
        }
        let version = u32::from_le_bytes(data[8..12].try_into().unwrap());
        let header = match version {
            1 => 21,
            2 => 32,
            _ => return Err(format!("{}: recording version {version}", path.display())),
        };
        if data.len() < header {
            return Err(format!("{}: a cut-off header", path.display()));
        }
        let rate = u64::from_le_bytes(data[12..20].try_into().unwrap());
        let flags = data[20];
        let ruleset = if flags & SLIPPI_RULESET != 0 {
            if flags & CARD != 0 {
                return Err(format!(
                    "{}: the Slippi ruleset with a memory card",
                    path.display()
                ));
            }
            if flags & DEVELOP_MASK != 0 {
                return Err(format!(
                    "{}: the Slippi ruleset with a debug level",
                    path.display()
                ));
            }
            super::state::Ruleset::Slippi
        } else {
            super::state::Ruleset::Vanilla
        };
        let (power_on, dtv, sram_language, sram_flags) = if version >= 2 {
            (
                u64::from_le_bytes(data[21..29].try_into().unwrap()),
                data[29] != 0,
                data[30],
                data[31],
            )
        } else {
            (0, false, 0, super::session::SRAM_FLAGS_DEFAULT)
        };
        let mut reads = Vec::new();
        let mut last = [PadStatus::error(ERR_NO_CONTROLLER); 4];
        let mut at = header;
        while at < data.len() {
            match data[at] {
                0 => at += 1,
                1 if at + 1 + 4 * PAD_BYTES <= data.len() => {
                    for (i, pad) in last.iter_mut().enumerate() {
                        *pad = decode(&data[at + 1 + i * PAD_BYTES..][..PAD_BYTES]);
                    }
                    at += 1 + 4 * PAD_BYTES;
                }
                // A session cut short mid-write leaves a partial last read, as ssbm-rs reads it.
                1 => break,
                b => {
                    return Err(format!(
                        "{}: bad read marker {b} at byte {at}",
                        path.display()
                    ));
                }
            }
            reads.push(last);
        }
        Ok(Recording {
            disc_rate: (rate > 0).then_some(rate),
            dolphin_disc: flags & DOLPHIN_DISC != 0,
            sound: flags & SOUND != 0,
            card: (flags & CARD != 0).then(|| path.with_extension("card")),
            version,
            power_on,
            dtv,
            sram_language,
            sram_flags,
            develop: match (flags & DEVELOP_MASK) >> DEVELOP_SHIFT {
                0 => None,
                n => Some(i32::from(n) - 1),
            },
            develop_ini: flags & LEVEL_ALONE == 0,
            ruleset,
            reads,
        })
    }

    /// The developer setting the session ran with, as its flags mark it.
    pub fn developer(&self) -> Option<super::session::Develop> {
        self.develop.map(|level| super::session::Develop {
            level,
            ini: self.develop_ini,
        })
    }

    /// The disc's pace, as melee-sim's timeline models it: with no rate, Dolphin's fast speed
    /// where the flags mark it and the flat model where they don't (every recording before
    /// ssbm-rs's `05d550a`), as ssbm-rs plays them back.
    pub fn disc(&self) -> Result<DiscSpeed, String> {
        match self.disc_rate {
            None if self.dolphin_disc => Ok(DiscSpeed::Fast),
            None => Ok(DiscSpeed::FastFlat),
            Some(r) if r == u64::from(crate::audio::clock::DRIVE_RATE) => Ok(DiscSpeed::Drive),
            Some(r) => Err(format!(
                "a disc read at {r} bytes a second: melee-sim models Slippi's fast disc speed and a drive's {}",
                crate::audio::clock::DRIVE_RATE
            )),
        }
    }

    pub fn dsp(&self) -> DspModel {
        if self.sound {
            DspModel::Samples
        } else {
            DspModel::Silent
        }
    }
}

/// A controller as ssbm-rs's `PADRead` writes it: a disconnected one is all zeros but its error.
fn decode(b: &[u8]) -> PadStatus {
    if b[0] == 0 {
        return PadStatus::error(ERR_NO_CONTROLLER);
    }
    PadStatus {
        button: u16::from_be_bytes([b[1], b[2]]),
        stick_x: b[3] as i8,
        stick_y: b[4] as i8,
        substick_x: b[5] as i8,
        substick_y: b[6] as i8,
        trigger_left: b[7],
        trigger_right: b[8],
        analog_a: b[9],
        analog_b: b[10],
        err: ERR_NONE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(version: u32) -> Vec<u8> {
        let mut d = MAGIC.to_vec();
        d.extend_from_slice(&version.to_le_bytes());
        d.extend_from_slice(&3_000_000u64.to_le_bytes());
        d.push(SOUND | CARD);
        d
    }

    /// Three reads: the four controllers, then the same twice.
    fn reads() -> Vec<u8> {
        let mut d = vec![1];
        for i in 0..4u8 {
            d.extend_from_slice(&[1, 0x01, 0x00, i, 0, 0, 0, 0, 0, 0, 0]);
        }
        d.extend_from_slice(&[0, 0]);
        d
    }

    #[test]
    fn version_1_has_ssbm_rs_s_console() {
        let mut data = header(1);
        data.extend(reads());
        let rec = Recording::parse(&data, Path::new("v1.inputs")).unwrap();
        assert_eq!(rec.version, 1);
        assert_eq!(rec.disc_rate, Some(3_000_000));
        assert!(rec.sound);
        assert_eq!(rec.card.as_deref(), Some(Path::new("v1.card")));
        assert_eq!(
            (rec.power_on, rec.dtv, rec.sram_language, rec.sram_flags),
            (0, false, 0, 0x2C)
        );
        assert_eq!(rec.reads.len(), 3);
        assert_eq!(rec.reads[2][3].stick_x, 3);
    }

    #[test]
    fn version_2_keeps_the_console_s_settings() {
        let mut data = header(2);
        data.extend_from_slice(&0x1234_5678_9ABCu64.to_le_bytes());
        data.extend_from_slice(&[1, 1, 0xA4]);
        data.extend(reads());
        let rec = Recording::parse(&data, Path::new("v2.inputs")).unwrap();
        assert_eq!(rec.version, 2);
        assert_eq!(
            (rec.power_on, rec.dtv, rec.sram_language, rec.sram_flags),
            (0x1234_5678_9ABC, true, 1, 0xA4)
        );
        assert_eq!(rec.reads.len(), 3);
        assert_eq!(rec.reads[0][1].button, 0x0100);
    }

    #[test]
    fn the_flags_keep_a_develop_level() {
        assert_eq!(
            Recording::parse(&[header(1), reads()].concat(), Path::new("r"))
                .unwrap()
                .develop,
            None
        );
        for level in 0..=4u8 {
            let mut data = header(1);
            data[20] |= (level + 1) << DEVELOP_SHIFT;
            data.extend(reads());
            let rec = Recording::parse(&data, Path::new("develop.inputs")).unwrap();
            assert_eq!(rec.develop, Some(i32::from(level)));
            assert!(rec.sound && rec.card.is_some());
            assert_eq!(
                rec.developer(),
                Some(super::super::session::Develop {
                    level: i32::from(level),
                    ini: true
                })
            );
            data[20] |= LEVEL_ALONE;
            let rec = Recording::parse(&data, Path::new("level.inputs")).unwrap();
            assert_eq!(
                rec.developer(),
                Some(super::super::session::Develop {
                    level: i32::from(level),
                    ini: false
                })
            );
        }
    }

    /// The ruleset's flag (0x04, agreed with ssbm-rs's `--ruleset slippi`): Slippi's with it,
    /// Vanilla's without, and refused beside a card or a debug level, which the ruleset
    /// doesn't run with.
    #[test]
    fn the_flags_keep_the_ruleset() {
        use super::super::state::Ruleset;
        let mut data = header(2);
        data[20] = SOUND;
        data.extend_from_slice(&[0; 11]);
        data.extend(reads());
        assert_eq!(
            Recording::parse(&data, Path::new("v.inputs"))
                .unwrap()
                .ruleset,
            Ruleset::Vanilla
        );
        data[20] = SOUND | SLIPPI_RULESET;
        let rec = Recording::parse(&data, Path::new("s.inputs")).unwrap();
        assert_eq!(rec.ruleset, Ruleset::Slippi);
        assert!(rec.sound && rec.card.is_none() && rec.develop.is_none());
        data[20] = SLIPPI_RULESET | CARD;
        let e = Recording::parse(&data, Path::new("c.inputs"))
            .err()
            .unwrap();
        assert!(e.contains("with a memory card"), "{e}");
        data[20] = SLIPPI_RULESET | LEVEL_ALONE | (4 << DEVELOP_SHIFT);
        let e = Recording::parse(&data, Path::new("d.inputs"))
            .err()
            .unwrap();
        assert!(e.contains("with a debug level"), "{e}");
    }

    #[test]
    fn other_versions_and_cut_headers_are_refused() {
        let mut data = header(3);
        data.extend(reads());
        let e = Recording::parse(&data, Path::new("v3.inputs"))
            .err()
            .unwrap();
        assert!(e.contains("recording version 3"), "{e}");
        let e = Recording::parse(&header(2), Path::new("v2.inputs"))
            .err()
            .unwrap();
        assert!(e.contains("a cut-off header"), "{e}");
    }
}
