//! The results screen's statistics (`gm_17AD.c`'s getters and clamps, `gm_17BA.c`'s labels)
//! and its per-character tables in `gm_1601.c`: each character's results file and fanfare.

use super::super::state::Running;
use super::super::structs::MatchEnd;

/// `lbl_803D53A8`: each character kind's results file (`gm_80160438`).
const RESULT_FILES: [(u8, &str); 26] = [
    (0x00, "GmRstMCa.dat"),
    (0x15, "GmRstMCl.dat"),
    (0x01, "GmRstMDk.dat"),
    (0x16, "GmRstMDr.dat"),
    (0x14, "GmRstMFc.dat"),
    (0x02, "GmRstMFx.dat"),
    (0x03, "GmRstMGw.dat"),
    (0x19, "GmRstMGn.dat"),
    (0x04, "GmRstMKb.dat"),
    (0x05, "GmRstMKp.dat"),
    (0x06, "GmRstMLk.dat"),
    (0x07, "GmRstMLg.dat"),
    (0x09, "GmRstMMs.dat"),
    (0x08, "GmRstMMr.dat"),
    (0x0A, "GmRstMMt.dat"),
    (0x0B, "GmRstMNs.dat"),
    (0x0E, "GmRstMPn.dat"),
    (0x0C, "GmRstMPe.dat"),
    (0x0D, "GmRstMPk.dat"),
    (0x18, "GmRstMPc.dat"),
    (0x0F, "GmRstMPr.dat"),
    (0x10, "GmRstMSs.dat"),
    (0x13, "GmRstMSk.dat"),
    (0x11, "GmRstMYs.dat"),
    (0x12, "GmRstMZd.dat"),
    (0x17, "GmRstMFe.dat"),
];

/// Every results file, which the frontend reads for the results screen.
pub fn results_files() -> impl Iterator<Item = &'static str> {
    RESULT_FILES.iter().map(|e| e.1)
}

/// A character kind's results file, `gm_80160438` (none for a kind without one).
pub fn results_file(ckind: i32) -> Option<&'static str> {
    RESULT_FILES
        .iter()
        .find(|e| i32::from(e.0) == ckind)
        .map(|e| e.1)
}

/// `ckind_victory_themes`: each character kind's fanfare.
const VICTORY_THEMES: [(u8, i32); 26] = [
    (0x00, 0x11),
    (0x01, 0x0D),
    (0x02, 0x10),
    (0x03, 0x0F),
    (0x04, 0x14),
    (0x05, 0x16),
    (0x06, 0x15),
    (0x07, 0x16),
    (0x08, 0x16),
    (0x09, 0x0E),
    (0x0A, 0x18),
    (0x0B, 0x17),
    (0x0C, 0x16),
    (0x0D, 0x18),
    (0x0E, 0x13),
    (0x0F, 0x18),
    (0x10, 0x19),
    (0x11, 0x1D),
    (0x12, 0x15),
    (0x13, 0x15),
    (0x14, 0x10),
    (0x15, 0x15),
    (0x16, 0x16),
    (0x17, 0x0E),
    (0x18, 0x18),
    (0x19, 0x15),
];

/// A character kind's fanfare, `fn_80160400` (-1 for a kind without one).
#[doc(alias = "fn_80160400")]
pub fn victory_theme(ckind: i32) -> i32 {
    VICTORY_THEMES
        .iter()
        .find(|e| i32::from(e.0) == ckind)
        .map_or(-1, |e| e.1)
}

/// `fn_8017AD04`: a value within `lim` either way.
#[doc(alias = "fn_8017AD04")]
pub fn clamp_to(v: i32, lim: i32) -> i32 {
    if v > lim {
        return lim;
    }
    if v < -lim {
        return -lim;
    }
    v
}

/// `fn_8017AD28`: within 9999.
#[doc(alias = "fn_8017AD28")]
pub fn clamp4(v: i32) -> i32 {
    v.clamp(-9999, 9999)
}

/// `fn_8017AD48`: within 9999999.
#[doc(alias = "fn_8017AD48")]
pub fn clamp7(v: i32) -> i32 {
    v.clamp(-9_999_999, 9_999_999)
}

/// `fn_8017AD78`: within 99999.
#[doc(alias = "fn_8017AD78")]
pub fn clamp5(v: i32) -> i32 {
    v.clamp(-99_999, 99_999)
}

/// `gmClampResultStat`: a statistic within 999999.
fn stat(v: i32) -> i32 {
    clamp_to(v, 999_999)
}

/// The KOs `killer` scored on `victim` (-1 for a player on itself, or an empty `slot`),
/// `getKOs`.
fn kos(me: &MatchEnd, slot: usize, killer: usize, victim: usize) -> i32 {
    if killer == victim {
        return -1;
    }
    if me.player_standings[slot].pkind != 3 {
        let k = if victim < 4 {
            me.player_standings[killer].kills[victim]
        } else {
            me.player_standings[killer].x18
        };
        return stat(i32::from(k));
    }
    -1
}

/// One of the results' statistics a stats page lists, by its getter's address
/// (`StatsEntry::check`): a count clamped (KOs, falls, self-destructs, damage, distances and
/// the like), or a player's KOs on another.
pub fn stat_value(me: &MatchEnd, us: bool, getter: u32, slot: usize) -> i32 {
    let p = &me.player_standings[slot];
    let f = |v: u32| stat(v as i32);
    match getter {
        // `fn_8017ADA8`: coins.
        0x8017_ADA8 => stat(p.x1C),
        0x8017_AE0C => f(p.x20),
        0x8017_AE70 => kos(me, 0, slot, 0),
        0x8017_AED8 => kos(me, 1, slot, 1),
        0x8017_AF40 => kos(me, 2, slot, 2),
        0x8017_AFA8 => kos(me, 3, slot, 3),
        0x8017_B010 => stat(p.x24.wrapping_sub(i32::from(p.self_destructs))),
        0x8017_B07C => kos(me, 0, 0, slot),
        0x8017_B0E4 => kos(me, 1, 1, slot),
        0x8017_B14C => kos(me, 2, 2, slot),
        0x8017_B1B4 => kos(me, 3, 3, slot),
        0x8017_B21C => stat(i32::from(p.self_destructs)),
        0x8017_B280 => f(p.x40),
        0x8017_B2E4 => f(p.x44),
        0x8017_B348 => f(p.x48),
        0x8017_B3AC => f(p.x4C),
        0x8017_B410 => {
            // The peak height: in feet (a 30.4788 cm one) in English, else meters.
            if us {
                let v = gekko_fp::fdivs(gekko_fp::frsp(f64::from(p.x5C)), f64::from(30.4788_f32));
                f(crate::ppc::cvt_fp2unsigned(v))
            } else {
                f(p.x5C / 100)
            }
        }
        0x8017_B4D0 => f(p.x6C),
        0x8017_B534 => f(p.x70),
        0x8017_B598 => stat(i32::from(p.x34)),
        0x8017_B5FC => f(p.x74),
        0x8017_B660 => f(p.x78),
        0x8017_B6C4 => f(p.x7C),
        0x8017_B728 => f(p.x84),
        0x8017_B78C => f(p.x88),
        0x8017_B7F0 => f(p.x8C),
        0x8017_B854 => f(p.x80),
        0x8017_B8B8 => f(p.x90),
        0x8017_B91C | 0x8017_B9F4 => {
            let raw = if getter == 0x8017_B91C { p.x94 } else { p.x98 };
            let v = gekko_fp::frsp(f64::from(raw));
            let v = if us {
                gekko_fp::frsp(v / 30.4788)
            } else {
                gekko_fp::fdivs(v, 100.0)
            };
            f(crate::ppc::cvt_fp2unsigned(v))
        }
        other => {
            super::super::code::unported_call(&format!("the results' statistic {other:#010x}"))
        }
    }
}

impl Running<'_> {
    /// The saved language is English (`lbLang_IsSavedLanguageUS`, on the save).
    pub fn saved_us(&self) -> bool {
        self.game.game_data.saved_language() == super::main_lib::LANG_US
    }

    /// Whether files named for the saved language take English names: always outside a
    /// console's session (a replay's match has no save).
    pub fn files_us(&self) -> bool {
        self.game.audio.dvd.is_none() || self.saved_us()
    }
}
