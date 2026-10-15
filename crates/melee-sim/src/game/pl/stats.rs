//! A player's match statistics (`plstale.c`, `plattack.c`, and the resets in `plbonus.c`
//! and `pl_040D.c`): the stale-move queue, attack counts, and the records bonuses are
//! scored from. The block's tail past the stale-move table (`xDB0`) is the rest of
//! `pl_StaleMoveTableExt_t`, which the original reaches by casting the table.

use super::super::state::Game;
use super::super::structs::{_StaticPlayer, plAttackStats};

/// Where `pl_StaleMoveTableExt_t`'s fields past the stale-move table start, in its own
/// offsets: the block's `xDB0`.
const EXT: usize = 0xCF4;

impl _StaticPlayer {
    /// A word of `pl_StaleMoveTableExt_t` past the stale-move table, by its offset there.
    pub fn ext_u32(&self, off: usize) -> u32 {
        let i = off - EXT;
        u32::from_be_bytes(self.xDB0[i..i + 4].try_into().unwrap())
    }

    pub fn set_ext_u32(&mut self, off: usize, v: u32) {
        let i = off - EXT;
        self.xDB0[i..i + 4].copy_from_slice(&v.to_be_bytes());
    }

    pub fn ext_f32(&self, off: usize) -> f32 {
        f32::from_bits(self.ext_u32(off))
    }

    pub fn set_ext_f32(&mut self, off: usize, v: f32) {
        self.set_ext_u32(off, v.to_bits());
    }

    pub fn ext_u8(&self, off: usize) -> u8 {
        self.xDB0[off - EXT]
    }

    pub fn set_ext_u8(&mut self, off: usize, v: u8) {
        self.xDB0[off - EXT] = v;
    }
}

impl Game {
    /// Adds how far smash DI moved a player to their stats (`xCDC`, `xCE0`), `pl_800401F0`.
    #[doc(alias = "pl_800401F0")]
    pub fn add_sdi_distance(&mut self, slot: i32, _sub: i32, dx: f64, dy: f64) {
        use gekko_fp as fp;
        let abs = |v: f64| if v < fp::frsp(0.0) { fp::fneg(v) } else { v };
        let t = self.player_stale_moves(slot);
        t.xCDC = crate::fpu::mem(fp::fadds(crate::fpu::reg(t.xCDC), abs(dx)));
        t.xCE0 = crate::fpu::mem(fp::fadds(crate::fpu::reg(t.xCE0), abs(dy)));
    }

    /// Clears every slot for a new match, `Player_InitAllPlayers`.
    #[doc(alias = "Player_InitAllPlayers")]
    pub fn init_all_players(&mut self) {
        self.init_attack_instance();
        self.init_attack_records();
        for i in 0..6 {
            self.reset_player(i);
            self.reset_player_records(i);
        }
    }

    /// Clears a slot and its records, `Player_80036CF0`.
    #[doc(alias = "Player_80036CF0")]
    pub fn reinit_player(&mut self, slot: i32) {
        self.reset_player(slot);
        self.reset_player_records(slot);
    }

    /// Clears a slot for a new opponent mid-match, `Player_80036D24`: the others forget it
    /// in their records.
    #[doc(alias = "Player_80036D24")]
    pub fn replace_player(&mut self, slot: i32) {
        self.reset_player(slot);
        self.forget_player_records(slot);
    }

    /// `plStale_InitAttackInstance`.
    #[doc(alias = "plStale_InitAttackInstance")]
    pub fn init_attack_instance(&mut self) {
        self.pl.stale_attack_instance = 1;
    }

    /// Takes the next attack instance, skipping 0, `plStale_IncrementAttackInstance`.
    #[doc(alias = "plStale_IncrementAttackInstance")]
    pub fn next_attack_instance(&mut self) -> u16 {
        let before = self.pl.stale_attack_instance;
        self.pl.stale_attack_instance = before.wrapping_add(1);
        if self.pl.stale_attack_instance == 0 {
            self.pl.stale_attack_instance = 1;
        }
        before
    }

    /// Empties a player's stale-move queue, `plStale_ResetStaleMoveTableForPlayer`.
    #[doc(alias = "plStale_ResetStaleMoveTableForPlayer")]
    pub fn reset_stale_moves(&mut self, slot: i32) {
        let t = self.player_stale_moves(slot);
        t.current_index = 0;
        for m in &mut t.StaleMoves {
            m.move_id = 0;
            m.attack_instance = 0;
        }
    }

    /// Records a hit by attack `id`, instance `instance`, in `slot`'s stale-move queue unless
    /// that hit is already there (the tail of `plStale_UpdateStaleMovesFromFighter` and
    /// `plStale_UpdateStaleMovesFromItem`).
    pub fn record_stale_move(&mut self, slot: i32, id: i32, instance: u16) {
        if id == 1 {
            return;
        }
        let t = self.player_stale_moves(slot);
        if t.StaleMoves
            .iter()
            .any(|m| id == i32::from(m.move_id) && instance == m.attack_instance)
        {
            return;
        }
        let i = t.current_index as usize;
        t.StaleMoves[i].move_id = id as u16;
        t.StaleMoves[i].attack_instance = instance;
        t.current_index = if t.current_index == 9 {
            0
        } else {
            t.current_index + 1
        };
    }

    /// `plAttack_80037590`.
    #[doc(alias = "plAttack_80037590")]
    pub fn init_attack_records(&mut self) {
        self.pl.attack_instance = 1;
    }

    /// Takes the next attack record number, skipping 0, `plAttack_80037B08`.
    #[doc(alias = "plAttack_80037B08")]
    pub fn next_attack_record(&mut self) -> u16 {
        let before = self.pl.attack_instance;
        self.pl.attack_instance = before.wrapping_add(1);
        if self.pl.attack_instance == 0 {
            self.pl.attack_instance = 1;
        }
        before
    }

    /// Clears a player's attack statistics, `plAttack_8003759C`.
    #[doc(alias = "plAttack_8003759C")]
    pub fn reset_action_stats(&mut self, slot: i32) {
        let s = self.player_action_stats(slot);
        clear_attack_stats(&mut s.attacks);
        clear_attack_stats(&mut s.hits);
        // `x358.x358_hits`, a third `plAttackStats` the mirror keeps as bytes; its first
        // 0x194 bytes are the total and counts, then five counts and `x1A8`.
        s.x358.fill(0);
        s.x504.fill(0);
        s.x568 = 0;
        s.x570 = 0;
        s.x56C = 0;
        s.x574 = 0;
        s.x578 = 0;
        s.x57C = 0;
        s.x580 = 0;
        s.x584 = 0;
        s.x588 = 0;
        s.x58C = 0;
        s.x590 = 0;
        s.x594 = 0;
        s.x598 = [0; 8];
        s.x5BC_b0 = false;
        s.x5BC_b1 = false;
        s.x5BC_b2 = false;
        s.x5BC_b3 = false;
    }

    /// Clears a player's bonus records, `pl_8003891C`: the stale moves, the attack counts,
    /// the per-frame counters, the item logs and every record the bonuses read.
    #[doc(alias = "pl_8003891C")]
    pub fn reset_player_records(&mut self, slot: i32) {
        self.reset_stale_moves(slot);
        self.reset_action_stats(slot);
        self.reset_frame_counters(slot);
        self.reset_x8c0(slot);
        self.reset_x8d8(slot);
        let p = self.player_mut(slot);
        let t = &mut p.stale_moves;
        t.x674 = [0; 39];
        t.x710 = [0; 39];
        t.x7AC = [0; 39];
        t.x848 = [0; 30];
        t.x904 = [0; 215];
        t.xC60 = 0.0;
        t.xC64 = 0.0;
        t.xC68 = 0;
        t.xC6C = 0.0;
        t.xC70 = 0.0;
        t.xC74 = 0.0;
        t.xC78 = [0.0; 6];
        t.xC90 = 0;
        t.xC94 = 0;
        t.xC98 = 0.0;
        t.xC9C = 0.0;
        t.xCA0 = 0.0;
        t.xCA4 = 0;
        t.xCAC = 6;
        t.xCB4 = 0;
        t.xCB8 = 6;
        t.xCBC = [0; 4];
        let c = &mut t.xCC0;
        c.x0 = 0;
        c.x4 = 0;
        c.kb_applied1 = 0.0;
        c.xC = 6;
        c.x10_b0 = false;
        c.x11_b3 = false;
        c.x10_b1 = false;
        c.x10_b2 = false;
        c.x10_b3 = false;
        c.x10_b4 = false;
        c.x10_b5 = false;
        c.x10_b6 = false;
        c.x10_b7 = false;
        c.x11_b0 = false;
        c.x11_b1 = false;
        c.x11_b2 = false;
        c.x11_b4 = false;
        t.xCD4 = 0;
        t.xCD8 = 0;
        t.xCE0 = 0.0;
        t.xCDC = 0.0;
        t.xCE4 = 0;
        t.xCE8 = 0;
        t.xCEC = 0;
        t.xCF0 = 0;
        p.set_ext_u32(0xCF4, 0);
        p.set_ext_f32(0xCF8, 0.0);
        p.stale_moves.xCA8 = 0;
        for off in (0xD04..=0xD54).step_by(4) {
            p.set_ext_u32(off, 0);
        }
        p.set_ext_f32(0xDC0, 0.0);
        p.set_ext_u32(0xD58, 0);
        p.set_ext_u32(0xD5C, u32::MAX);
        p.set_ext_u32(0xD60, 0);
        p.set_ext_u32(0xD64, 6);
        p.set_ext_u32(0xD68, 0);
        p.set_ext_u32(0xD6C, 6);
        p.set_ext_u32(0xCFC, 0);
        p.set_ext_u32(0xD00, 0);
        p.set_ext_u32(0xD70, 0);
        for off in [0xD74, 0xD78, 0xD7C, 0xD80, 0xD84, 0xD88, 0xD94] {
            p.set_ext_f32(off, 0.0);
        }
        p.set_ext_u32(0xD98, 0);
        p.set_ext_f32(0xD8C, 0.0);
        p.set_ext_u32(0xD90, 0);
        for off in [
            0xD9C, 0xDA0, 0xDA4, 0xDA8, 0xDAC, 0xDB4, 0xDBC, 0xDC4, 0xDC8, 0xDCC,
        ] {
            p.set_ext_u32(off, 0);
        }
        // `xDD0`'s and `xDD1`'s bits, one by one.
        p.set_ext_u8(0xDD0, 0);
        p.set_ext_u8(0xDD1, 0);
        p.set_ext_u32(0xDB8, 0);
    }

    /// Clears a player's per-frame counters, `pl_80040DDC`.
    /// Counts damage healed toward the slot's statistics (`xC68`), `pl_80040B8C`: not for a
    /// sub-fighter.
    #[doc(alias = "pl_80040B8C")]
    pub fn add_healed(&mut self, slot: i32, sub: i32, amount: i32) {
        if sub != 0 {
            return;
        }
        let t = self.player_stale_moves(slot);
        t.xC68 = t.xC68.wrapping_add(amount);
    }

    #[doc(alias = "pl_80040DDC")]
    pub fn reset_frame_counters(&mut self, slot: i32) {
        let t = self.player_unk6a8(slot);
        t.x0 = 0;
        t.x4 = 0.0;
        t.x8 = 0;
        t.xC = 0;
        for e in &mut t.x10 {
            e.x0 = 0.0;
            e.x4 = 0;
            e.x8 = 0;
            e.xC = 0;
            e.x10 = 0;
            e.x12_b0 = false;
        }
    }

    /// `pl_80041524`.
    #[doc(alias = "pl_80041524")]
    pub fn reset_x8c0(&mut self, slot: i32) {
        let t = &mut self.player_stale_moves(slot).x8C0;
        t.x0 = 0;
        for e in &mut t.x4 {
            e.x0 = 0;
            e.x2 = 0;
            e.x3_b0 = false;
        }
    }

    /// `pl_800418F4`.
    #[doc(alias = "pl_800418F4")]
    pub fn reset_x8d8(&mut self, slot: i32) {
        let t = &mut self.player_stale_moves(slot).x8D8;
        t.x0 = 0;
        for e in &mut t.x4 {
            e.x0 = 0;
            e.x4_b0 = false;
            e.x4_b1 = false;
        }
    }

    /// A Bob-omb's hit on the player, by its spawn number (`Item::x1C`): the Bob-omb's entry
    /// among the five the player last threw (`x8D8`) is marked as having hit, `pl_80041B08`.
    #[doc(alias = "pl_80041B08")]
    pub fn count_bombhei_hit(&mut self, slot: i32, spawn: u16) {
        for e in &mut self.player_stale_moves(slot).x8D8.x4 {
            if u32::from(spawn) == e.x0 {
                e.x4_b0 = true;
            }
        }
    }

    /// Clears a player's bonus counts (`x904`), `fn_80038FB8`.
    #[doc(alias = "fn_80038FB8")]
    pub fn reset_bonus_counts(&mut self, slot: i32) {
        self.player_stale_moves(slot).x904 = [0; 215];
    }

    /// Clears the records that follow who KO'd whom when a slot is replaced, `pl_80038F10`.
    #[doc(alias = "pl_80038F10")]
    pub fn forget_player_records(&mut self, slot: i32) {
        let p = self.player_mut(slot);
        p.set_ext_u32(0xD50, 0);
        p.set_ext_u32(0xD54, 0);
        p.set_ext_u32(0xD60, 0);
        p.set_ext_u32(0xD64, 6);
        for i in 0..6 {
            let o = self.player_mut(i);
            if slot == o.ext_u32(0xD64) as i32 {
                o.set_ext_u32(0xD60, 0);
                o.set_ext_u32(0xD64, 6);
            }
            if slot == o.stale_moves.xCAC as i32 {
                o.stale_moves.xCAC = 6;
            }
            if slot == o.ext_u32(0xD6C) as i32 {
                o.set_ext_u32(0xD6C, 6);
            }
        }
    }
}

/// `clearAttackStats`.
fn clear_attack_stats(s: &mut plAttackStats) {
    s.by_attack_counts = [0; 100];
    s.total = 0;
    s.thrown_item_count = 0;
    s.aerials_count = 0;
    s.specials_count = 0;
    s.x1A0_count = 0;
    s.x1A4_count = 0;
    s.x1A8 = 0;
}
