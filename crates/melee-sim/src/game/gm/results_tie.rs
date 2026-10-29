//! What `fn_80169000` reads through a place no player took: the automatic handicaps after a
//! tie (Melee's bugs, the automatic handicaps after a tie).
//!
//! The function keeps its players by place in a stack array (`positions`, `sp+0x20`, at
//! 0x804EEAC0 on VS mode's results' exit) and its handicaps beside it (`sp+0x24`). Tied
//! players share a place, so the places after them stay as the stack held them, and the
//! function indexes its handicaps with those bytes: `handicaps[b]`, read and in some branches
//! moved, is the byte at `sp+0x24+b` for any `b` up to 255, the frames of its callers.
//!
//! The word is what `OSSleepThread` saved there (its `stw r30, 0x10(r1)`) in the scene loop's
//! last wait, `HSD_VIWaitXFBFlush` > `VIWaitForRetrace` > `OSSleepThread`: `VIWaitForRetrace`'s
//! `r30`, the VI's retrace count as that wait began ([`Clock::flush_count`]). Nothing on the
//! main thread writes it again before the call: the results' exit runs no deeper than
//! `gmVsMelee_ExitResults` until then.
//!
//! The bytes above the handicaps (`sp+0x28` up, 0x804EEAC8 to 0x804EEBC3) are the frames of
//! the exit's call chain, `main` > `gm_801A4510` > `runGameMode` > `gm_801A4014` >
//! `onExitResults@801B16C8` > `gmVsMelee_ExitResults` > `gm_80168638` > `fn_80169000`, and the
//! cells between them that the chain's frames leave as earlier calls wrote them
//! ([`VS_EXIT`]). Each was read off ssbm-rs's run of the original code (`WATCH_ADDR` on the
//! stack through the results sessions) and from the functions' prologues. Melee moving a
//! byte of a saved register or a saved return address changes what the caller does after
//! the return, which isn't modelled: the exit stops there.
//!
//! [`Clock::flush_count`]: crate::audio::clock::Clock::flush_count

use super::super::code::unported_call;

/// Whose frames lie above `fn_80169000`'s handicaps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TieStack {
    /// VS mode's results' exit (`onExitResults@801B16C8`): the retrace count the scene
    /// loop's last flush began its last wait with (none when it didn't wait), and the days
    /// from 2000-01-01 to the boot's report of the date (`main`'s `gm_801692E8`, none when a
    /// day may end in the boot).
    VsMelee {
        flush: Option<u32>,
        boot_days: Option<u32>,
    },
    /// A caller whose frames aren't modelled (Tournament's `gm_8019E634`, Camera Mode's
    /// `gm_801B2AF8`, a Special Melee mode's results' exit).
    Other(&'static str),
}

/// A word of [`VS_EXIT`]: what it holds.
#[derive(Clone, Copy, Debug)]
enum Holds {
    Word(u32),
    /// `gm_80168638`'s `spC`: the handicaps it passes (`hb`), which the function's last
    /// loop writes back over whatever it moved there.
    Handicaps,
    /// The boot's `OSCalendarTime`'s `wday` and `yday` (`main` > `gm_801692E8` >
    /// `lbTime_8000B028` > `OSTicksToCalendarTime` > `GetDates`, under `gm_801A4510`'s frame).
    WeekDay,
    YearDay,
}

/// What moving a byte of a word does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Use {
    /// Nothing reads it again: a frame's padding, a callee's save slot no callee uses, a
    /// back chain (every epilogue on the chain pops its frame with `addi r1`).
    Dead,
    /// A saved register or return address the chain restores and uses.
    Live,
    /// `gmVsMelee_ExitResults`'s `r28`, the next state's id: only its low byte reaches
    /// `gm_SetNextGameModeStateId` (`stb`), which a move there changes.
    StateId,
}

use Holds::{Handicaps, Word};
use Use::{Dead, Live, StateId};

/// The words from 0x804EEAC8 (`handicaps[4]`) to 0x804EEBC3 (`handicaps[255]`) at
/// `fn_80169000`'s call from VS mode's results' exit, each with whose cell it is.
const VS_EXIT: [(Holds, Use, &str); 63] = [
    // fn_80169000's frame top (sp+0x28): unused.
    (Word(0x804E_EAD8), Dead, "VIWaitForRetrace's back chain"),
    // The console's: OSSleepThread's saved return address into VIWaitForRetrace (ssbm-rs's
    // original code holds 0xFFFFFFF0 here, its interrupt's stand-in return address).
    (
        Word(0x8034_F33C),
        Dead,
        "OSSleepThread's saved return address",
    ),
    // gm_80168638's frame (0x804EEAD0).
    (Word(0x804E_EAF8), Dead, "gm_80168638's back chain"),
    (
        Word(0x804C_1D80),
        Dead,
        "VIWaitForRetrace's saved r31 (HSD_VIData)",
    ),
    (Word(0x804E_EAF8), Dead, "HSD_VIWaitXFBFlush's back chain"),
    (Handicaps, Dead, "gm_80168638's handicaps"),
    (
        Word(0x804D_6730),
        Live,
        "gmVsMelee_ExitResults's r26 (ko_counts)",
    ),
    (
        Word(0x8045_AC50),
        Live,
        "gmVsMelee_ExitResults's r27 (its VS data)",
    ),
    (
        Word(0),
        StateId,
        "gmVsMelee_ExitResults's r28 (the next state)",
    ),
    // r29 and r30: gm_801A4014's, which gmVsMelee_ExitResults sets before it reads them.
    (
        Word(0x8047_9D33),
        Dead,
        "gmVsMelee_ExitResults's r29 (gm_801A4014's)",
    ),
    (
        Word(0x8046_B0F4),
        Dead,
        "gmVsMelee_ExitResults's r30 (gm_801A4014's)",
    ),
    (
        Word(0x8047_9DA4),
        Live,
        "gmVsMelee_ExitResults's r31 (the match's end)",
    ),
    // gmVsMelee_ExitResults's frame (0x804EEAF8).
    (
        Word(0x804E_EB30),
        Dead,
        "gmVsMelee_ExitResults's back chain",
    ),
    (
        Word(0x801A_5FA8),
        Live,
        "gm_80168638's saved return address",
    ),
    (Word(0x803D_ACB8), Dead, "gmVsMelee_ExitResults's padding"),
    (Word(0x8047_9D33), Dead, "gmVsMelee_ExitResults's padding"),
    (Word(0x803D_ACB8), Dead, "gmVsMelee_ExitResults's padding"),
    (Word(0x8047_9D48), Dead, "gmVsMelee_ExitResults's padding"),
    (Word(0x804E_EB18), Dead, "gmVsMelee_ExitResults's padding"),
    (Word(0x803D_DA0C), Live, "gm_801A4014's r25"),
    (Word(0x803D_A984), Live, "gm_801A4014's r26"),
    (Word(0x803D_DA00), Live, "gm_801A4014's r27"),
    (Word(0x803D_ACB8), Live, "gm_801A4014's r28"),
    (Word(0x8047_9D33), Live, "gm_801A4014's r29"),
    (Word(0x8046_B0F4), Live, "gm_801A4014's r30"),
    (
        Word(0x803D_DA00),
        Live,
        "onExitResults's r31 (its game mode state)",
    ),
    // onExitResults@801B16C8's frame (0x804EEB30).
    (Word(0x804E_EB48), Dead, "onExitResults's back chain"),
    (
        Word(0x801B_16F0),
        Live,
        "gmVsMelee_ExitResults's saved return address",
    ),
    (Word(0x803D_ACB8), Dead, "onExitResults's padding"),
    (Word(0x8047_9D33), Dead, "onExitResults's padding"),
    (Word(0x804E_EB48), Dead, "onExitResults's padding"),
    (Word(0x8047_9D30), Live, "gm_801A4014's r31"),
    // gm_801A4014's frame (0x804EEB48).
    (Word(0x804E_EB80), Dead, "gm_801A4014's back chain"),
    (
        Word(0x801A_4148),
        Live,
        "onExitResults's saved return address",
    ),
    (Word(0x804E_EB88), Dead, "gm_801A4014's padding"),
    (Word(1), Dead, "gm_801A4014's padding"),
    (Word(0), Dead, "gm_801A4014's padding"),
    (Word(0), Dead, "gm_801A4014's padding"),
    (Word(0), Dead, "gm_801A4014's padding"),
    (Word(0), Live, "runGameMode's r25"),
    (Word(0), Live, "runGameMode's r26"),
    (Word(0x2D), Live, "runGameMode's r27"),
    (Word(0x8046_B0F0), Live, "runGameMode's r28"),
    (Word(0x8047_9D36), Live, "runGameMode's r29"),
    (Word(0x803D_ACB8), Live, "runGameMode's r30"),
    (Word(0x8047_9D30), Live, "runGameMode's r31"),
    // runGameMode's frame (0x804EEB80).
    (Word(0x804E_EBB0), Dead, "runGameMode's back chain"),
    (
        Word(0x801A_44C4),
        Live,
        "gm_801A4014's saved return address",
    ),
    (Word(0x804E_EBA8), Dead, "runGameMode's padding"),
    (Word(0x8016_7B8C), Dead, "runGameMode's padding"),
    (Word(0x804E_EBD0), Dead, "runGameMode's padding"),
    (Word(0), Dead, "runGameMode's padding"),
    (Word(0x8047_9D30), Dead, "runGameMode's padding"),
    (Word(0), Live, "gm_801A4510's r27"),
    (Word(0), Live, "gm_801A4510's r28"),
    (Word(0x8046_B0F4), Live, "gm_801A4510's r29"),
    (Word(0x8047_9D30), Live, "gm_801A4510's r30"),
    (Word(0), Live, "gm_801A4510's r31"),
    // gm_801A4510's frame (0x804EEBB0).
    (Word(0x804E_EBD0), Dead, "gm_801A4510's back chain"),
    (
        Word(0x801A_45C8),
        Live,
        "runGameMode's saved return address",
    ),
    (
        Holds::WeekDay,
        Dead,
        "gm_801A4510's padding (the boot's calendar's wday)",
    ),
    (
        Holds::YearDay,
        Dead,
        "gm_801A4510's padding (the boot's calendar's yday)",
    ),
    (Word(0x804E_EBD0), Dead, "gm_801A4510's padding"),
];

/// `fn_80169000`'s frame from its handicaps up, as the function sees it: the handicaps it
/// moves, the bytes above them, and what it has moved there.
pub(crate) struct TieFrame {
    pub handicaps: [u8; 4],
    hb: [u8; 4],
    stack: TieStack,
    moved: Vec<(u8, u8)>,
    /// The next state's id, when a move reached `gmVsMelee_ExitResults`'s `r28`.
    pub next_state: Option<u8>,
}

impl TieFrame {
    pub(crate) fn new(hb: [u8; 4], stack: TieStack) -> Self {
        Self {
            handicaps: hb,
            hb,
            stack,
            moved: Vec::new(),
            next_state: None,
        }
    }

    /// Byte `n` of the word an unset place reads (`positions[n]`).
    pub(crate) fn unset_place(&self, n: usize) -> u8 {
        match self.stack {
            TieStack::VsMelee { flush: Some(c), .. } => c.to_be_bytes()[n],
            TieStack::VsMelee { flush: None, .. } => unported_call(&format!(
                "fn_80169000's unset place {n} after a flush that didn't wait (the stack word isn't OSSleepThread's)"
            )),
            TieStack::Other(who) => unported_call(&format!(
                "fn_80169000's unset place {n} from {who}, whose stack isn't modelled"
            )),
        }
    }

    /// `handicaps[b]`.
    pub(crate) fn get(&self, b: u8) -> u8 {
        if b < 4 {
            return self.handicaps[usize::from(b)];
        }
        if let Some(&(_, v)) = self.moved.iter().rev().find(|(at, _)| *at == b) {
            return v;
        }
        self.cell(b).0
    }

    /// `handicaps[b] = v`.
    pub(crate) fn set(&mut self, b: u8, v: u8) {
        if b < 4 {
            self.handicaps[usize::from(b)] = v;
            return;
        }
        let (_, use_, who) = self.cell(b);
        if use_ == StateId && (b - 4) % 4 == 3 {
            self.next_state = Some(v);
        }
        if use_ == Live {
            unported_call(&format!(
                "fn_80169000 moving handicaps[{b}], a byte of {who}, which its caller then uses"
            ));
        }
        self.moved.push((b, v));
    }

    /// The byte at `handicaps[b]` for `b` from 4, what moving it does, and whose it is.
    fn cell(&self, b: u8) -> (u8, Use, &'static str) {
        let TieStack::VsMelee { boot_days, .. } = self.stack else {
            unreachable!("an unset place from another caller stops first")
        };
        let at = usize::from(b) - 4;
        let (holds, use_, who) = VS_EXIT[at / 4];
        let byte = at % 4;
        let word = match holds {
            Word(w) => w,
            Handicaps => return (self.hb[byte], use_, who),
            Holds::WeekDay | Holds::YearDay => {
                let days = boot_days.unwrap_or_else(|| {
                    unported_call(&format!(
                        "fn_80169000 reading handicaps[{b}], {who}, after a boot at midnight"
                    ))
                });
                if matches!(holds, Holds::WeekDay) {
                    (days + 6) % 7
                } else {
                    year_day(days)
                }
            }
        };
        (word.to_be_bytes()[byte], use_, who)
    }
}

/// The day of its year (from 0) of the day `days` after 2000-01-01, as `GetDates` counts.
fn year_day(mut days: u32) -> u32 {
    let mut year = 2000;
    loop {
        let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let len = if leap { 366 } else { 365 };
        if days < len {
            return days;
        }
        days -= len;
        year += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::super::results_exit::move_handicaps;
    use super::*;
    use crate::game::structs::MatchEnd;

    /// A match of `places.len()` players at those places.
    fn end(places: &[u8]) -> MatchEnd {
        let mut e = MatchEnd::default();
        for p in &mut e.player_standings {
            p.pkind = 3;
        }
        for (i, &at) in places.iter().enumerate() {
            e.player_standings[i].pkind = 0;
            e.player_standings[i].is_small_loser = at;
        }
        e
    }

    fn vs(flush: u32) -> TieStack {
        TieStack::VsMelee {
            flush: Some(flush),
            boot_days: Some(0),
        }
    }

    #[test]
    fn a_tie_for_second_reads_the_stack_above() {
        // Places 0, 1, 1: the last place (2) reads the count's second byte, 0x1C, the first
        // byte of gmVsMelee_ExitResults's saved r26 (0x80): above 8, so nothing moves.
        let mut h = [5, 5, 5, 5];
        move_handicaps(&end(&[0, 1, 1]), &mut h, vs(0x1C40));
        assert_eq!(h, [5, 5, 5, 5]);
        // 0x19: gm_80168638's handicaps, port 2's 5: the winner down, a byte the copy back
        // overwrites up.
        let mut h = [5, 3, 5, 5];
        move_handicaps(&end(&[0, 1, 1]), &mut h, vs(0x1940));
        assert_eq!(h, [4, 3, 5, 5]);
    }

    #[test]
    fn a_count_whose_byte_is_under_four_reads_a_handicap() {
        // Places 0, 1, 1, 1: the last place (3) reads the count's low byte, 2: port 3's.
        let mut h = [5, 5, 7, 5];
        move_handicaps(&end(&[0, 1, 1, 1]), &mut h, vs(0x2302));
        assert_eq!(h, [4, 5, 8, 5]);
    }

    #[test]
    fn the_boot_calendar_cells() {
        // 0xF4 to 0xFB: wday (2000-01-01 a Saturday, 6) and yday.
        let f = TieFrame::new([5; 4], vs(0));
        assert_eq!(f.get(0xF7), 6);
        assert_eq!(f.get(0xFB), 0);
        let f = TieFrame::new(
            [5; 4],
            TieStack::VsMelee {
                flush: Some(0),
                boot_days: Some(366 + 31),
            },
        );
        assert_eq!(u32::from(f.get(0xF7)), (366 + 31 + 6) % 7);
        assert_eq!(f.get(0xFB), 31);
    }

    #[test]
    fn moving_the_saved_state_id_moves_the_next_state() {
        // 0x27: the saved r28's low byte, 0 (the character select): the winner down, the
        // byte up by one, which gmVsMelee_ExitResults then sets as the next state.
        let mut h = [5, 5, 5, 5];
        let next = move_handicaps(&end(&[0, 1, 1]), &mut h, vs(0x2700));
        assert_eq!((h, next), ([4, 5, 5, 5], Some(1)));
        // 0x24: its high byte, which nothing reads.
        let mut h = [5, 5, 5, 5];
        let next = move_handicaps(&end(&[0, 1, 1]), &mut h, vs(0x2400));
        assert_eq!((h, next), ([4, 5, 5, 5], None));
    }

    #[test]
    #[should_panic(expected = "a byte of runGameMode's r25")]
    fn moving_a_saved_register_stops() {
        // Places 0, 1, 1, 1: the count's low byte, 0xA0, runGameMode's saved r25's first
        // byte, 0.
        let mut h = [5, 5, 5, 5];
        move_handicaps(&end(&[0, 1, 1, 1]), &mut h, vs(0x10A0));
    }

    #[test]
    #[should_panic(expected = "whose stack isn't modelled")]
    fn another_callers_tie_stops() {
        let mut h = [5, 5, 5, 5];
        move_handicaps(&end(&[0, 1, 1]), &mut h, TieStack::Other("a test"));
    }

    #[test]
    fn untied_places_read_no_stack() {
        let mut h = [5, 5, 5, 5];
        move_handicaps(&end(&[2, 0, 1]), &mut h, TieStack::Other("a test"));
        assert_eq!(h, [6, 4, 5, 5]);
    }
}
