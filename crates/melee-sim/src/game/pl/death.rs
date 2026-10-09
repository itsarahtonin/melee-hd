//! A KO's statistics (`plbonuslib.c`'s `pl_8003D644` and its helpers, `pl_040D.c`'s hit
//! records): how and where the fighter died and what hit it last, for itself and for the
//! player who KOed it.

use super::super::state::Running;
use super::super::structs::_StaticPlayer;
use crate::fpu::reg;
use gekko_fp as fp;

/// The slot that stands for no one.
const NO_SLOT: i32 = 6;

/// `ftCo_MS_DeadDown` .. `ftCo_MS_DeadUp`, and the star and screen KOs' ranges.
const DEAD_DOWN: i32 = 0;
const DEAD_LEFT: i32 = 1;
const DEAD_RIGHT: i32 = 2;
const DEAD_UP: i32 = 3;
const DEAD_UP_STAR: i32 = 4;
const DEAD_UP_STAR_ICE: i32 = 5;
const DEAD_UP_FALL: i32 = 6;
const DEAD_UP_FALL_HIT_CAMERA_ICE: i32 = 10;

/// `xDD1`'s bits, `bit0` the top one.
fn set_dd1(p: &mut _StaticPlayer, n: u8) {
    p.set_ext_u8(0xDD1, p.ext_u8(0xDD1) | (0x80 >> n));
}

fn inc(p: &mut _StaticPlayer, off: usize) {
    p.set_ext_u32(off, p.ext_u32(off).wrapping_add(1));
}

/// `between_A1_D0`.
fn between_a1_d0(x: i32) -> bool {
    (0xA1..0xD0).contains(&x)
}

/// `pl_8003D60C`: the item kinds whose KOs count as an explosion's.
#[doc(alias = "pl_8003D60C")]
fn explosive_kind(x: i32) -> bool {
    x == 0xA0 || (0xEA..0xEE).contains(&x) || x == 0xE1
}

impl Running<'_> {
    /// `pl_CheckIfSameTeam`.
    fn same_team(&self, a: i32, b: i32) -> bool {
        self.game.vs_is_teams() && self.game.player_team(b) == self.game.player_team(a)
    }

    /// `unk_cond`: no one, the player itself or a teammate.
    fn no_rival(&self, slot: i32, other: i32) -> bool {
        other == NO_SLOT || other == slot || self.same_team(slot, other)
    }

    /// A player's point bonus set (`pl_80038788`, `setPointValue`).
    #[doc(alias = "pl_80038788")]
    fn set_point(&mut self, slot: i32, kind: usize, v: u32) {
        assert_ne!(
            slot, NO_SLOT,
            "the original's assertion failed: player != Gm_Player_Other"
        );
        self.game.player_mut(slot).stale_moves.x904[kind] = v;
    }

    /// A KO's statistics, `pl_8003D644`, for the fighter in `slot` (`sub` for Nana, who
    /// keeps none), which died in state `motion` hit by attack `attack`.
    #[doc(alias = "pl_8003D644")]
    #[doc(alias = "pl_8003FED0")]
    #[doc(alias = "pl_80041744")]
    #[doc(alias = "pl_80041720")]
    #[doc(alias = "ft_80087858")]
    #[doc(alias = "ft_80087878")]
    pub fn record_death(&mut self, slot: i32, sub: bool, motion: i32, attack: i32) {
        let gobj = self
            .game
            .player_entity_at(slot, i32::from(sub))
            .expect("a slot without its fighter");
        let fp = self.game.fighter(gobj);
        let msid = fp.motion_id;
        let (source_type, source_kind) = (fp.dmg.x18CC, fp.dmg.x18D0);
        let attack_bits = fp.dmg.x18d4;
        let instance = fp.dmg.x18ec_instancehitby;
        let hit = fp.dmg.x18d8.clone();
        let last = self.game.ft_last_attacker(gobj);
        let last_sub = fp.x221F_b5;
        // `ft_80087858`, `ft_80087878`.
        let x2227_b1 = fp.x2227_b1;
        let grounded_jump =
            fp.x1968_jumpsUsed <= 1 && fp.motion_id == 0 && !matches!(motion, 0x23..=0x25);
        if sub {
            return;
        }
        let p = self.game.player_mut(slot);
        let t = &mut p.stale_moves;
        t.xCB8 = last;
        t.xCBC = attack_bits;
        t.xCC0 = hit.clone();
        t.xCD4 = instance;
        // `pl_8003FED0`: the first KO of the match.
        if (0..6).all(|i| self.game.player_mut(i).stale_moves.x904[0x82] == 0) {
            self.set_point(slot, 0x82, 1);
        }
        if (DEAD_UP_FALL..=DEAD_UP_FALL_HIT_CAMERA_ICE).contains(&msid) {
            self.count_bonus(slot, 0x81);
        }
        let p = self.game.player_mut(slot);
        if (DEAD_UP_STAR..=DEAD_UP_STAR_ICE).contains(&msid) {
            p.stale_moves.xC94 = p.stale_moves.xC94.wrapping_add(1);
        }
        if (DEAD_UP..=DEAD_UP_FALL_HIT_CAMERA_ICE).contains(&msid) {
            set_dd1(p, 2);
        }
        match msid {
            DEAD_DOWN => set_dd1(p, 3),
            DEAD_RIGHT => set_dd1(p, 0),
            DEAD_LEFT => set_dd1(p, 1),
            _ => {}
        }
        if x2227_b1 {
            self.set_point(slot, 0x83, 1);
        }
        if grounded_jump {
            self.count_bonus(slot, 0x84);
        }
        self.mark_ko_positions(slot);
        if attack as u32 == 0x5F {
            self.set_point(slot, 0x9C, 1);
        }
        match source_type {
            1 | 2 => {
                if hit.x4 <= self.bonus_param(0x128) as i32
                    && hit.x0 >= self.bonus_param(0x12C) as i32
                {
                    self.count_bonus(slot, 0x85);
                }
                if source_type == 2 {
                    if explosive_kind(source_kind) {
                        self.set_point(slot, 0x88, 1);
                    } else if last == slot {
                        self.set_point(slot, 0x9C, 1);
                    }
                    if source_kind == 0xEA {
                        inc(self.game.player_mut(slot), 0xD00);
                    }
                }
            }
            3 => {
                let k = if source_kind == 1 { 0x87 } else { 0x88 };
                self.set_point(slot, k, 1);
            }
            _ => {}
        }
        if last == NO_SLOT && source_type == 2 && source_kind == 6 {
            self.set_point(slot, 0xC7, 1);
        }
        if source_type != 2 || !between_a1_d0(source_kind) {
            if self.no_rival(slot, last) {
                if last == slot && !last_sub {
                    self.record_hit(last, instance, true);
                }
            } else {
                self.record_hit(last, instance, false);
            }
        }
        if self.no_rival(slot, last) {
            let p = self.game.player_mut(slot);
            p.set_ext_u32(0xD60, 0);
            p.set_ext_u32(0xD64, NO_SLOT as u32);
        } else {
            let lp = self.game.player_mut(last);
            if lp.ext_u32(0xD60) != 0 && lp.ext_u32(0xD64) as i32 == slot {
                self.count_bonus(last, 0x79);
                let lp = self.game.player_mut(last);
                lp.set_ext_u32(0xD60, 0);
                lp.set_ext_u32(0xD64, NO_SLOT as u32);
            }
            let p = self.game.player_mut(slot);
            p.set_ext_u32(0xD60, 0);
            p.set_ext_u32(0xD64, last as u32);
        }
        if self.no_rival(slot, last) {
            return;
        }
        let x2073 = attack_bits[3];
        let (count_x1a4, count_specials) = (attack_bits[2] & 0x10 != 0, attack_bits[2] & 1 != 0);
        if (DEAD_UP_STAR..=DEAD_UP_STAR_ICE).contains(&msid) {
            self.count_bonus(last, 0x6D);
        }
        if msid == DEAD_UP {
            let lp = &mut self.game.player_mut(last).stale_moves;
            lp.xCB4 = lp.xCB4.wrapping_add(1);
        }
        if self.bonus_param(0x120) as i32 == self.game.player_damage(slot) {
            self.count_bonus(last, 0x76);
        }
        match source_type {
            1 | 2 => {
                if (1..=3).contains(&x2073) {
                    self.count_bonus(last, 0x6E);
                }
                if (0x40..=0x43).contains(&x2073) {
                    self.count_bonus(last, 0xC0);
                }
                if x2073 == 0x46 {
                    self.count_bonus(last, 0xC1);
                }
                if x2073 == 0x5F {
                    self.count_bonus(last, 0xAD);
                }
                if x2073 == 0x63 {
                    self.count_bonus(last, 0x70);
                }
                if x2073 == 0x61 {
                    self.count_bonus(last, 0xC6);
                }
                if count_specials {
                    self.count_bonus(last, 0x6F);
                }
                if source_type == 2 && count_x1a4 {
                    self.count_bonus(last, 0x72);
                }
                for (on, k) in [
                    (hit.x11_b0, 0x71),
                    (hit.x10_b0, 6),
                    (
                        reg(hit.kb_applied1) >= reg(self.bonus_param_f32(0x11C)),
                        0x74,
                    ),
                    (hit.x11_b2, 0x75),
                    (hit.x11_b1, 0x77),
                    (hit.x10_b1, 0xB0),
                    (hit.x10_b2, 0xBB),
                    (hit.x10_b3, 0xBF),
                    (hit.x10_b4, 0xBA),
                    (hit.x10_b5, 0xB7),
                    (hit.x10_b6, 0xB8),
                    (hit.x11_b3, 0xB),
                    (hit.x11_b4, 0x7E),
                ] {
                    if on {
                        self.count_bonus(last, k);
                    }
                }
                if !self.game.vs_untimed()
                    && !self.game.vs_timer_counts_up()
                    && self.game.vs_timer_seconds().wrapping_add(1) <= self.bonus_param(0x118)
                {
                    self.count_bonus(last, 0x73);
                }
                if source_type == 2 {
                    if x2073 == 0x62 {
                        match source_kind {
                            0 | 3 => self.count_bonus(last, 0x97),
                            1 | 2 | 4 => self.count_bonus(last, 0x98),
                            30 => self.count_bonus(last, 0xAC),
                            20 => self.count_bonus(last, 0xB2),
                            17 => self.count_bonus(last, 0xB1),
                            _ => {}
                        }
                    }
                    if between_a1_d0(source_kind) {
                        self.count_bonus(last, 0xCB);
                    }
                    if explosive_kind(source_kind) {
                        self.count_bonus(last, 0x80);
                    }
                    if source_kind == 0xEA {
                        inc(self.game.player_mut(last), 0xCFC);
                    }
                }
            }
            3 => {
                let k = if source_kind == 1 { 0x7F } else { 0x80 };
                self.count_bonus(last, k);
            }
            _ => {}
        }
    }

    /// A ledge caught after a hit, `pl_80040048`: a strong enough hit counts a recovery
    /// (`xCE8`), and a stronger one with a jump or less used scores bonus 0x48 (Nana's don't
    /// count, nor a fighter no one hit).
    #[doc(alias = "pl_80040048")]
    #[doc(alias = "ft_80087AA4")]
    #[doc(alias = "ft_80087AB4")]
    pub fn record_ledge_catch(&mut self, slot: i32, sub: bool) {
        let gobj = self
            .game
            .player_entity_at(slot, i32::from(sub))
            .expect("a slot without its fighter");
        let last = self.game.ft_last_attacker(gobj);
        if sub || last == NO_SLOT {
            return;
        }
        let fp = self.game.fighter(gobj);
        let (kb, special_fall, jumps) = (fp.dmg.x18d8.kb_applied1, fp.x2227_b5, fp.x1968_jumpsUsed);
        if reg(kb) >= reg(self.bonus_param_f32(0x98)) {
            let t = &mut self.game.player_mut(slot).stale_moves;
            t.xCE8 = t.xCE8.wrapping_add(1);
        }
        if reg(kb) >= reg(self.bonus_param_f32(0xA0)) && !special_fall && jumps <= 1 {
            self.count_bonus(slot, 0x48);
        }
    }

    /// Where each rival stood at a KO that wasn't off the top, `plBonusLib_8003D514`: every
    /// other player in play notes it saw one, and whether it faced away from the KO.
    #[doc(alias = "plBonusLib_8003D514")]
    fn mark_ko_positions(&mut self, slot: i32) {
        let me = self
            .game
            .player_entity(slot)
            .expect("a slot without its fighter");
        if (DEAD_UP..=DEAD_UP_FALL_HIT_CAMERA_ICE).contains(&self.game.fighter(me).motion_id) {
            return;
        }
        let at = self.game.ft_pos(me);
        for i in 0..6 {
            if i == slot {
                continue;
            }
            let other = self.game.player_entity(i);
            if !self.game.player_awake(i) || other.is_none_or(|o| self.game.ft_is_dead(o)) {
                continue;
            }
            let o = other.expect("an awake slot without its fighter");
            let pos = self.game.ft_pos(o);
            let facing = reg(self.game.fighter(o).facing_dir);
            let p = self.game.player_mut(i);
            set_dd1(p, 6);
            if fp::fmuls(facing, reg(at.x)) > fp::fmuls(facing, reg(pos.x)) {
                set_dd1(p, 7);
            }
        }
    }

    /// The hit that KOed, in the attacker's ring of recent attack instances,
    /// `fn_800415B0`: an instance already there counts one more KO (or with `self_ko`, is
    /// marked); a new one takes the oldest entry, whose counts score first (`fn_80041770`).
    #[doc(alias = "fn_800415B0")]
    #[doc(alias = "fn_80041770")]
    fn record_hit(&mut self, slot: i32, instance: u16, self_ko: bool) {
        if instance == 0 {
            return;
        }
        let ring = &mut self.game.player_mut(slot).stale_moves.x8C0;
        for e in &mut ring.x4 {
            if e.x0 == instance {
                if self_ko {
                    e.x3_b0 = true;
                } else {
                    e.x2 = e.x2.wrapping_add(1);
                }
                return;
            }
        }
        let i = ring.x0 as usize;
        let (n, marked) = (ring.x4[i].x2, ring.x4[i].x3_b0);
        let k = match n {
            5.. => Some(0x7D),
            4 => Some(0x7C),
            3 => Some(0x7B),
            2 => Some(0x7A),
            _ => None,
        };
        if let Some(k) = k {
            self.count_bonus(slot, k);
        }
        if marked && n != 0 {
            self.count_bonus(slot, 0x78);
        }
        let ring = &mut self.game.player_mut(slot).stale_moves.x8C0;
        let e = &mut ring.x4[i];
        e.x0 = instance;
        e.x2 = 0;
        e.x3_b0 = false;
        if self_ko {
            e.x3_b0 = true;
        } else {
            e.x2 = 1;
        }
        ring.x0 = if ring.x0 == 4 { 0 } else { ring.x0 + 1 };
    }
}
