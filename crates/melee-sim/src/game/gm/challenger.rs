//! What of the challenger approach's mode (`gm_1736.c`, `gm_1BFA.c`) the special messages'
//! interface needs: the mode's load (straight to the prize state when no challenger's fighter
//! waits), and the prize state's entry, which queues the messages and trophies waiting for
//! their notices, and its exit back to the mode before. The approach and its match are
//! milestone 5's task 17.

use super::super::hud::prize::{EntryRef, PrizeEntry};
use super::super::state::Running;

/// `TY_TROPHY_COUNT`.
const TROPHIES: usize = 293;
/// The trophies' notice's message (`0x3E`).
const TROPHY_MESSAGE: u16 = 0x3E;

impl Running<'_> {
    /// `gm_Mode_ChallengerApproach_OnLoad`: the mode before kept; no challenger's fighter
    /// (only notices waiting) starts at the prize state (2), else at the approach.
    #[doc(alias = "gm_Mode_ChallengerApproach_OnLoad")]
    pub fn challenger_mode_on_load(&mut self) {
        self.game.gm.challenger.prev_mode = self.game.machine.routing.prev_mode;
        let id = if self.game.gm.challenger.human_ckind == 0x21 {
            2
        } else {
            0
        };
        self.set_game_mode_state_id(id);
    }

    /// `gm_801BFC60`: the `n`-th notice queued: the first into the prize's enter data
    /// (`if_Scene_Prize_EnterData`), each later one into a block of the heap linked from the
    /// one before (`prev`); the entry whose link the next fills.
    #[doc(alias = "gm_801BFC60")]
    fn queue_notice(
        &mut self,
        kind: u16,
        n: i32,
        trophy: u16,
        time: u32,
        prev: Option<EntryRef>,
    ) -> Option<EntryRef> {
        let e = PrizeEntry {
            kind,
            trophy,
            time,
            next: None,
        };
        let p = &mut self.game.prize;
        if n == 0 {
            p.first = PrizeEntry {
                next: p.first.next,
                ..e
            };
            return Some(EntryRef::Static);
        }
        let addr = crate::hsd::heap::mem_alloc(0xC);
        if addr == 0 {
            return prev;
        }
        let p = &mut self.game.prize;
        p.heap.insert(addr, e);
        if let Some(r) = prev {
            *p.entry_mut(r).next_mut() = Some(EntryRef::Heap(addr));
        }
        Some(EntryRef::Heap(addr))
    }

    /// A trophy's notice taken up: its time and the save's mark when new, the trophy
    /// given (`Toy_SetUnlockState`).
    fn take_trophy_notice(&mut self, t: usize) {
        let s = &self.game.game_data.thing.save_data;
        if s.x1B58[t / 32] & (1 << (t % 32)) == 0 {
            let now = self.clock_seconds();
            self.game.game_data.unk_44.times[t] = now;
            self.game.game_data.thing.save_data.x1B58[t / 32] |= 1 << (t % 32);
            self.add_trophy(t as i32, 1);
        }
    }

    fn trophy_notice_waits(&self, t: usize) -> bool {
        self.game.game_data.unk_44.flags[t / 32] & (1 << (t % 32)) != 0
    }

    /// `gm_ModeState_Prize_OnEnter`: the notices waiting queued for the prize interface: each
    /// special message announced and waiting, each trophy whose notice waits (taken up, its
    /// count kept); then the trophies and messages the save has earned since, those queued
    /// too; the trophies' notices cleared, the trophy sets' checks, the queue ended, and the
    /// save's archive and mark.
    #[doc(alias = "gm_ModeState_Prize_OnEnter")]
    pub fn prize_state_on_enter(&mut self) {
        let mut last: Option<EntryRef> = None;
        let mut n = 0;
        let p = &mut self.game.prize;
        p.queued[..0x42].fill(0);
        p.queued[0x44..0x44 + TROPHIES].fill(0);
        self.mark_class_two_trophies();
        for i in 0..0x42u32 {
            if self.game.message_waits(i) {
                let time = self.game.game_data.thing.save_data.x1B80[i as usize];
                last = self.queue_notice(i as u16, n, 0, time, last);
                self.game.prize.queued[i as usize] = 1;
                n += 1;
            }
        }
        for t in 0..TROPHIES {
            if self.trophy_notice_waits(t) {
                self.take_trophy_notice(t);
                self.game.prize.queued[0x44 + t] = self.game.trophy_count(t) as u8;
                let time = self.game.game_data.unk_44.times[t];
                last = self.queue_notice(TROPHY_MESSAGE, n, t as u16, time, last);
                n += 1;
            }
        }
        self.trophy_unlocks();
        self.special_unlocks(0xFFFF);
        for i in 0..0x42u32 {
            if self.game.prize.queued[i as usize] == 0 && self.game.message_waits(i) {
                let time = self.game.game_data.thing.save_data.x1B80[i as usize];
                last = self.queue_notice(i as u16, n, 0, time, last);
                n += 1;
            }
        }
        for t in 0..TROPHIES {
            let had = i32::from(self.game.prize.queued[0x44 + t]);
            let now = self.game.trophy_count(t);
            if (had < now || now == 0) && self.trophy_notice_waits(t) {
                self.take_trophy_notice(t);
                let time = self.game.game_data.unk_44.times[t];
                last = self.queue_notice(TROPHY_MESSAGE, n, t as u16, time, last);
                n += 1;
            }
        }
        self.game.clear_trophy_notices();
        self.trophy_set_checks();
        if let Some(r) = last {
            *self.game.prize.entry_mut(r).next_mut() = None;
        }
        self.card_alloc_work_area();
        self.card_game_load_archive(0);
        self.card_game_save_changes();
    }

    /// `onExitPrize`: the mode before the notices next.
    #[doc(alias = "onExitPrize@801BFF7C")]
    pub fn prize_state_on_exit(&mut self) {
        let m = self.game.gm.challenger.curr_mode;
        self.set_pending_game_mode(m);
        self.set_new_game_mode_pending();
    }

    /// `gm_80174180`: the trophy sets the records earn: the 1P modes' (Classic and
    /// Adventure cleared), 200 VS matches', and every character's (`Toy_80305918`).
    #[doc(alias = "gm_80174180")]
    fn trophy_set_checks(&mut self) {
        if self.message_condition(0x1E) == Some(true) && self.message_condition(0x1F) == Some(true)
        {
            self.toy_mark_class(4, false, false);
        }
        if self.vs_matches_played() >= 0xC8 {
            self.toy_mark_class(5, false, false);
        }
        if self.game.gm.unlocks.all_characters() {
            self.toy_mark_class(6, false, false);
        }
    }
}
