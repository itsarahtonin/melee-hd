//! What `fn_80169000` reads through a place no player took: the automatic handicaps after a
//! tie (Melee's bugs, the automatic handicaps after a tie).
//!
//! The function keeps its players by place in a stack array (`positions`, `sp+0x20`) and its
//! handicaps beside it (`sp+0x24`). Tied players share a place, so the places after them stay
//! as the stack held them, and the function indexes its handicaps with those bytes:
//! `handicaps[b]`, read and in some branches moved, is the byte at `sp+0x24+b` for any `b` up
//! to 255, the frames of its callers. Only the last place's entry can be unset: the first
//! place is Sudden Death's one winner (`gm_80166CCC` takes its places from Sudden Death's
//! ranks, which give a single winner or, with several, rank them apart), so place 1 is always
//! taken, and the last place is read and moved up by one or two alone (`fn_80169000`'s
//! branches move places 0 and 1 down, the last up). A byte above 8 never moves.
//!
//! Which word lies under the places depends on the exit's depth:
//!
//! - VS mode's results' exit (`positions` at 0x804EEAC0): what `OSSleepThread` saved there
//!   (its `stw r30, 0x10(r1)`) in the scene loop's last wait, `HSD_VIWaitXFBFlush` >
//!   `VIWaitForRetrace` > `OSSleepThread`: `VIWaitForRetrace`'s `r30`, the VI's retrace count
//!   as that wait began ([`Clock::flush_count`]). Nothing on the main thread writes it again
//!   before the call: the results' exit runs no deeper than `gmVsMelee_ExitResults` until
//!   then. The bytes above the handicaps (0x804EEAC8 to 0x804EEBC3) are the frames of the
//!   exit's call chain, `main` > `gm_801A4510` > `runGameMode` > `gm_801A4014` >
//!   `onExitResults@801B16C8` > `gmVsMelee_ExitResults` > `gm_80168638` > `fn_80169000`, and
//!   the cells between them that the chain's frames leave as earlier calls wrote them
//!   ([`VS_EXIT`]), read off ssbm-rs's run of the original code at its b0abe5e (`WATCH_ADDR`
//!   on the stack through the results and tie sessions) and from the functions' prologues.
//! - A Special Melee mode's results' exit (`gm_801B9154` and the seven like it, each with an
//!   8-byte frame, then `gmVsMelee_ExitResults`) and Tournament's next match-up
//!   (`gm_801B1AD4`'s 0x18 bytes, then `gm_8019E634`'s 0x50) both call `fn_80169000` 0x10
//!   bytes higher (`positions` at 0x804EEAD0): `VIWaitForRetrace`'s save of its caller's `r30`
//!   (`stw r30, 0x8(r1)`), which `HSD_VIWaitXFBFlush` holds as `OSDisableInterrupts`' answer
//!   on its loop, 1 on the main thread with interrupts on. Nothing between the wait and the
//!   call writes it (`gm_801B1AD4`'s `gm_801688AC` and `gm_8016247C`, and `gm_8019E634`'s
//!   calls before it, are leaves or stay above it; ssbm-rs's original code through
//!   `exit-sessions/sd-giant` and `tournament-sessions/winner-next2` has 1 there at the call).
//!   So the last place of three reads `handicaps[0]` and of four `handicaps[1]`: a player's,
//!   never the stack above.
//! - Camera Mode's match exit (`gm_801B2AF8`) has no tie: its mode has no Sudden Death
//!   (`gm_Mode_Camera_States`), and `gm_80166378`'s places break every tie by `x30`, whose
//!   low nibble is `6 - port`, so every place is taken.
//!
//! A byte the move reaches may be one a caller restores: a register `main`'s frames keep
//! through the modes' loop ([`MainRegs`]) is moved for good, and the next exit's frames save
//! it again. Melee moving any other byte of a saved register or a saved return address the
//! callers then use would change what they do after the return, which isn't modelled: the
//! exit stops there, though no such byte is 8 or less ([`VS_EXIT`]).
//!
//! [`Clock::flush_count`]: crate::audio::clock::Clock::flush_count

use super::super::code::unported_call;

/// The registers `main`'s frames keep through the modes' loop, as a tie's automatic handicaps
/// may have moved them (`main`'s `r25` to `r28`, which `gm_801A4510` and `runGameMode` pass
/// on unused, and `gm_801A4510`'s loop's `r31`, 0). No code reads them for a value:
/// `gm_801A4510` never returns, `runGameMode` and `gm_801A4014` save them and set their own,
/// and `gm_801A4510` stores its `r31` only over a reset's flag (`gmMainLib_8046B0F0.x4`),
/// which the console's reset button sets, and no session or player presses it. So a move
/// stays in the register, and every exit after saves it where the next tie can read it
/// (`runGameMode`'s save of `r25` to `r28` in `gm_801A4014`'s frame, `gm_801A4510`'s in
/// `runGameMode`'s).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct MainRegs {
    pub r25: u32,
    pub r26: u32,
    pub r27: u32,
    pub r28: u32,
    /// `gm_801A4510`'s `r31`.
    pub loop_r31: u32,
}

/// One of [`MainRegs`]' registers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MainReg {
    R25,
    R26,
    R27,
    R28,
    LoopR31,
}

impl MainRegs {
    fn value(mut self, r: MainReg) -> u32 {
        *self.reg(r)
    }

    fn reg(&mut self, r: MainReg) -> &mut u32 {
        match r {
            MainReg::R25 => &mut self.r25,
            MainReg::R26 => &mut self.r26,
            MainReg::R27 => &mut self.r27,
            MainReg::R28 => &mut self.r28,
            MainReg::LoopR31 => &mut self.loop_r31,
        }
    }
}

/// Whose frames lie above `fn_80169000`'s handicaps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TieStack {
    /// VS mode's results' exit (`onExitResults@801B16C8`): the retrace count the scene
    /// loop's last flush began its last wait with (none when it didn't wait), the days from
    /// 2000-01-01 to the boot's report of the date (`main`'s `gm_801692E8`, none without a
    /// boot from power-on), and `main`'s kept registers.
    VsMelee {
        flush: Option<u32>,
        boot_days: Option<u32>,
        regs: MainRegs,
    },
    /// A Special Melee mode's results' exit or Tournament's next match-up, whose places lie
    /// on `VIWaitForRetrace`'s save of 1: whether the scene loop's last flush waited (the
    /// save is that wait's), and the caller's name.
    Shallow { waited: bool, who: &'static str },
    /// Camera Mode's match exit, whose places all differ.
    Camera,
}

impl TieStack {
    /// `main`'s kept registers after the call ([`MainRegs`]).
    pub(crate) fn regs(&self) -> Option<MainRegs> {
        match *self {
            TieStack::VsMelee { regs, .. } => Some(regs),
            _ => None,
        }
    }
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
    /// A save of one of `main`'s kept registers ([`MainRegs`]).
    Main(MainReg),
}

/// What moving a byte of a word does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Use {
    /// Nothing reads it again: a frame's padding, a callee's save slot no callee uses, a
    /// back chain (every epilogue on the chain pops its frame with `addi r1`).
    Dead,
    /// A saved register or return address the chain restores and uses.
    Live,
    /// A save of one of `main`'s kept registers: a move stays in the register.
    Kept,
    /// `gmVsMelee_ExitResults`'s `r28`, the next state's id: only its low byte reaches
    /// `gm_SetNextGameModeStateId` (`stb`), which a move there changes.
    StateId,
}

use Holds::{Handicaps, Word};
use Use::{Dead, Kept, Live, StateId};

/// The words from 0x804EEAC8 (`handicaps[4]`) to 0x804EEBC3 (`handicaps[255]`) at
/// `fn_80169000`'s call from VS mode's results' exit, each with whose cell it is.
const VS_EXIT: [(Holds, Use, &str); 63] = [
    // fn_80169000's frame top (sp+0x28): unused.
    (Word(0x804E_EAD8), Dead, "VIWaitForRetrace's back chain"),
    // OSSleepThread's saved return address into VIWaitForRetrace (ssbm-rs before its b0abe5e
    // held 0xFFFFFFF0 here, its interrupt's stand-in return address).
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
    // gm_801A4014's r25 to r27: once the exit returns it reads only r28 to r31 before its
    // epilogue restores runGameMode's.
    (Word(0x803D_DA0C), Dead, "gm_801A4014's r25"),
    (Word(0x803D_A984), Dead, "gm_801A4014's r26"),
    (Word(0x803D_DA00), Dead, "gm_801A4014's r27"),
    (Word(0x803D_ACB8), Live, "gm_801A4014's r28"),
    (Word(0x8047_9D33), Live, "gm_801A4014's r29"),
    (Word(0x8046_B0F4), Live, "gm_801A4014's r30"),
    // onExitResults sets r31 again before it reads it.
    (
        Word(0x803D_DA00),
        Dead,
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
    // An ARAM interrupt's handler in the boot (`__ARHandler`'s save of the dispatcher's r31,
    // the handler's address), taken while the main thread waited this high on the stack.
    (Word(0x8035_0F98), Dead, "gm_801A4014's padding"),
    (Word(0), Dead, "gm_801A4014's padding"),
    (Word(0), Dead, "gm_801A4014's padding"),
    (Word(0), Dead, "gm_801A4014's padding"),
    (
        Holds::Main(MainReg::R25),
        Kept,
        "runGameMode's r25 (main's)",
    ),
    (
        Holds::Main(MainReg::R26),
        Kept,
        "runGameMode's r26 (main's)",
    ),
    // runGameMode's r27, the override's answer (lbCardGame_DecideGameMode, installed at the
    // boot for good), which it asks again before its next call.
    (Word(0x2D), Dead, "runGameMode's r27"),
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
    (
        Holds::Main(MainReg::R27),
        Kept,
        "gm_801A4510's r27 (main's)",
    ),
    (
        Holds::Main(MainReg::R28),
        Kept,
        "gm_801A4510's r28 (main's)",
    ),
    (Word(0x8046_B0F4), Live, "gm_801A4510's r29"),
    (Word(0x8047_9D30), Live, "gm_801A4510's r30"),
    (Holds::Main(MainReg::LoopR31), Kept, "gm_801A4510's r31"),
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
    pub stack: TieStack,
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
        let word = match self.stack {
            TieStack::VsMelee { flush: Some(c), .. } => c,
            TieStack::Shallow { waited: true, .. } => 1,
            TieStack::VsMelee { flush: None, .. } => unported_call(&format!(
                "fn_80169000's unset place {n} after a flush that didn't wait (the stack word isn't OSSleepThread's)"
            )),
            TieStack::Shallow { waited: false, who } => unported_call(&format!(
                "fn_80169000's unset place {n} from {who} after a flush that didn't wait (the stack word isn't VIWaitForRetrace's)"
            )),
            TieStack::Camera => unported_call(&format!(
                "fn_80169000's unset place {n} from Camera Mode's match exit, whose places all differ"
            )),
        };
        word.to_be_bytes()[n]
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
        let at = usize::from(b) - 4;
        match use_ {
            StateId if at % 4 == 3 => self.next_state = Some(v),
            Live => unported_call(&format!(
                "fn_80169000 moving handicaps[{b}], a byte of {who}, which its caller then uses"
            )),
            Kept => {
                let Holds::Main(r) = VS_EXIT[at / 4].0 else {
                    unreachable!("a kept register's cell holds it")
                };
                let TieStack::VsMelee { regs, .. } = &mut self.stack else {
                    unreachable!("only VS mode's exit reads the stack")
                };
                let w = regs.reg(r);
                let mut bytes = w.to_be_bytes();
                bytes[at % 4] = v;
                *w = u32::from_be_bytes(bytes);
            }
            _ => {}
        }
        self.moved.push((b, v));
    }

    /// The byte at `handicaps[b]` for `b` from 4, what moving it does, and whose it is.
    fn cell(&self, b: u8) -> (u8, Use, &'static str) {
        let TieStack::VsMelee {
            boot_days, regs, ..
        } = self.stack
        else {
            unreachable!("only VS mode's exit's places index past the handicaps")
        };
        let at = usize::from(b) - 4;
        let (holds, use_, who) = VS_EXIT[at / 4];
        let byte = at % 4;
        let word = match holds {
            Word(w) => w,
            Handicaps => return (self.hb[byte], use_, who),
            Holds::Main(r) => regs.value(r),
            Holds::WeekDay | Holds::YearDay => {
                let days = boot_days.unwrap_or_else(|| {
                    unported_call(&format!(
                        "fn_80169000 reading handicaps[{b}], {who}, with no boot's report"
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
            regs: MainRegs::default(),
        }
    }

    fn moved(places: &[u8], h: &mut [u8; 4], stack: TieStack) -> (Option<u8>, TieStack) {
        let mut stack = stack;
        let next = move_handicaps(&end(places), h, &mut stack);
        (next, stack)
    }

    #[test]
    fn a_tie_for_second_reads_the_stack_above() {
        // Places 0, 1, 1: the last place (2) reads the count's second byte, 0x1C, the first
        // byte of gmVsMelee_ExitResults's saved r26 (0x80): above 8, so nothing moves.
        let mut h = [5, 5, 5, 5];
        moved(&[0, 1, 1], &mut h, vs(0x1C40));
        assert_eq!(h, [5, 5, 5, 5]);
        // 0x19: gm_80168638's handicaps, port 2's 5: the winner down, a byte the copy back
        // overwrites up.
        let mut h = [5, 3, 5, 5];
        moved(&[0, 1, 1], &mut h, vs(0x1940));
        assert_eq!(h, [4, 3, 5, 5]);
    }

    #[test]
    fn a_count_whose_byte_is_under_four_reads_a_handicap() {
        // Places 0, 1, 1, 1: the last place (3) reads the count's low byte, 2: port 3's.
        let mut h = [5, 5, 7, 5];
        moved(&[0, 1, 1, 1], &mut h, vs(0x2302));
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
                regs: MainRegs::default(),
            },
        );
        assert_eq!(u32::from(f.get(0xF7)), (366 + 31 + 6) % 7);
        assert_eq!(f.get(0xFB), 31);
    }

    #[test]
    fn the_tenth_of_january_moves_the_winner_by_two() {
        // A three-way tie for second of four whose count's low byte is 0xFB, yday's: on
        // 2000-01-09 (8) the winner down by one and the padding up; on the 10th (9) the
        // winner down by two.
        let at = |days| TieStack::VsMelee {
            flush: Some(0x23FB),
            boot_days: Some(days),
            regs: MainRegs::default(),
        };
        let mut h = [5, 5, 5, 5];
        moved(&[0, 1, 1, 1], &mut h, at(8));
        assert_eq!(h, [4, 5, 5, 5]);
        let mut h = [5, 5, 5, 5];
        moved(&[0, 1, 1, 1], &mut h, at(9));
        assert_eq!(h, [3, 5, 5, 5]);
    }

    #[test]
    fn moving_the_saved_state_id_moves_the_next_state() {
        // 0x27: the saved r28's low byte, 0 (the character select): the winner down, the
        // byte up by one, which gmVsMelee_ExitResults then sets as the next state.
        let mut h = [5, 5, 5, 5];
        let (next, _) = moved(&[0, 1, 1], &mut h, vs(0x2700));
        assert_eq!((h, next), ([4, 5, 5, 5], Some(1)));
        // 0x24: its high byte, which nothing reads.
        let mut h = [5, 5, 5, 5];
        let (next, _) = moved(&[0, 1, 1], &mut h, vs(0x2400));
        assert_eq!((h, next), ([4, 5, 5, 5], None));
    }

    #[test]
    fn moving_a_kept_register_keeps_it() {
        // Places 0, 1, 1, 1: the count's low byte, 0xA3, runGameMode's saved r25's low byte,
        // main's 0: the winner down, r25 up by one; the next exit's frame saves 1 there.
        let mut h = [5, 5, 5, 5];
        let (_, after) = moved(&[0, 1, 1, 1], &mut h, vs(0x10A3));
        assert_eq!(h, [4, 5, 5, 5]);
        let regs = after.regs().unwrap();
        assert_eq!(
            regs,
            MainRegs {
                r25: 1,
                ..MainRegs::default()
            }
        );
        let next = TieStack::VsMelee {
            flush: Some(0x10A3),
            boot_days: Some(0),
            regs,
        };
        assert_eq!(TieFrame::new([5; 4], next).get(0xA3), 1);
        // With the winner at 1, gm_801A4510's r28's second byte (0xDD) up by two.
        let mut h = [1, 5, 5, 5];
        let (_, after) = moved(&[0, 1, 1, 1], &mut h, vs(0x10DD));
        assert_eq!(h, [1, 5, 5, 5]);
        assert_eq!(after.regs().unwrap().r28, 0x0002_0000);
        // A kept register at 9 moves the winner by two.
        let mut h = [5, 5, 5, 5];
        let nine = TieStack::VsMelee {
            flush: Some(0x10A3),
            boot_days: Some(0),
            regs: MainRegs {
                r25: 9,
                ..MainRegs::default()
            },
        };
        moved(&[0, 1, 1, 1], &mut h, nine);
        assert_eq!(h, [3, 5, 5, 5]);
    }

    #[test]
    fn a_dead_register_moves_nothing_after() {
        // 0xA7: runGameMode's r27's low byte, 0x2D: above 8. 0x5B: gm_801A4014's r27's low
        // byte (0x803DDA00), which it doesn't read after the exit: moved, nothing kept.
        let mut h = [5, 5, 5, 5];
        let (_, after) = moved(&[0, 1, 1, 1], &mut h, vs(0x105B));
        assert_eq!(h, [4, 5, 5, 5]);
        assert_eq!(after.regs().unwrap(), MainRegs::default());
    }

    #[test]
    fn no_live_byte_can_move() {
        // Only the last place reaches the stack, and only a byte of 8 or less moves.
        for (k, &(holds, use_, who)) in VS_EXIT.iter().enumerate() {
            if use_ != Live {
                continue;
            }
            let Word(w) = holds else {
                panic!("{who} isn't a word")
            };
            for (i, b) in w.to_be_bytes().iter().enumerate() {
                assert!(
                    *b > 8,
                    "{who}'s byte {i} ({b}, handicaps[{}])",
                    4 + 4 * k + i
                );
            }
        }
    }

    #[test]
    fn the_shallow_exits_read_a_handicap() {
        let shallow = TieStack::Shallow {
            waited: true,
            who: "a test",
        };
        // Places 0, 1, 1: the third place reads byte 2 of 1, the winner's own handicap,
        // which moves down and back up.
        let mut h = [5, 6, 7, 5];
        moved(&[0, 1, 1], &mut h, shallow);
        assert_eq!(h, [5, 6, 7, 5]);
        // Places 0, 1, 2, 2: the fourth reads byte 3, port 2's.
        let mut h = [5, 6, 7, 5];
        moved(&[0, 1, 2, 2], &mut h, shallow);
        assert_eq!(h, [4, 7, 7, 5]);
    }

    #[test]
    #[should_panic(expected = "whose places all differ")]
    fn a_camera_tie_stops() {
        let mut h = [5, 5, 5, 5];
        moved(&[0, 1, 1], &mut h, TieStack::Camera);
    }

    #[test]
    #[should_panic(expected = "with no boot's report")]
    fn the_calendar_without_a_boot_stops() {
        let stack = TieStack::VsMelee {
            flush: Some(0x10FB),
            boot_days: None,
            regs: MainRegs::default(),
        };
        let mut h = [5, 5, 5, 5];
        moved(&[0, 1, 1, 1], &mut h, stack);
    }

    #[test]
    fn untied_places_read_no_stack() {
        let mut h = [5, 5, 5, 5];
        moved(&[2, 0, 1], &mut h, TieStack::Camera);
        assert_eq!(h, [6, 4, 5, 5]);
    }
}
