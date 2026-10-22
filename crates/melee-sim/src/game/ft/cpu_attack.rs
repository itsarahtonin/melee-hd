//! Computer players' attacks (`ftcpuattack.c`): whether to attack and with what, from
//! per-character tables in `PlCo.dat` (`Fighter_804D64FC`, `ftLoadCommonData::x58`) whose
//! entries say where an attack reaches and how often to pick it, weighed against where the
//! target will be.

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::code::unported_call;
use super::super::handles::{FighterId, ItemId};
use super::super::state::Running;
use super::common::GA_AIR;
use crate::fpu::{mem, reg};
use crate::lb::{collision, hitcapsule};
use crate::math;
use crate::vec::Vec3;

/// C's `ABS` on a single difference.
fn abs(x: f64) -> f64 {
    if x < 0.0 { fp::fneg(x) } else { x }
}

/// `ftLoadCommonData::x58`: the computer players' data.
const CPU_DATA: u32 = 0x58;

/// The attack tables in it (`Fighter_804D64FC_t`) the ported checks use: ground, air and
/// edge-guard attacks, one table per character.
mod table {
    pub const GROUND: u32 = 0x4;
    pub const AIR: u32 = 0x8;
    pub const RANGED: u32 = 0xC;
    pub const SHIELD: u32 = 0x10;
    pub const EDGE_GUARD: u32 = 0x1C;
}

/// `ftCo_AttackEntry`'s size, and its weight's offset.
const ENTRY_SIZE: u32 = 0x24;
const ENTRY_WEIGHT: u32 = 0x18;

/// `HSD_GOBJ_PLINK_ITEM`.
const P_LINK_ITEM: u8 = 9;

/// `HitElement_Catch`, `HitElement_Inert`.
const ELEMENT_CATCH: u32 = 8;
const ELEMENT_INERT: u32 = 11;

/// `Ft_Kind_Koopa`, `_GKoops`.
const KIND_KOOPA: i32 = 5;
const KIND_GKOOPS: i32 = 31;

/// `Ft_Kind_Mario`, `_Fox`, `_Captain`, `_Kirby`, `_Peach`, `_Pikachu`, `_Mewtwo`, `_Luigi`,
/// `_Falco`, `_Pichu`, `_Ganon`.
const KIND_MARIO: i32 = 0;
const KIND_FOX: i32 = 1;
const KIND_CAPTAIN: i32 = 2;
const KIND_KIRBY: i32 = 4;
const KIND_PEACH: i32 = 9;
const KIND_PIKACHU: i32 = 12;
const KIND_MEWTWO: i32 = 16;
const KIND_LUIGI: i32 = 17;
const KIND_FALCO: i32 = 22;
const KIND_PICHU: i32 = 23;
const KIND_GANON: i32 = 25;

/// `Ft_Kind_Ness`, `_Yoshi`, `_Samus`, `_Donkey`, `_Zelda`.
const KIND_NESS: i32 = 8;
const KIND_YOSHI: i32 = 14;
const KIND_SAMUS: i32 = 13;
const KIND_DONKEY: i32 = 3;
const KIND_ZELDA: i32 = 19;

/// Commands the attack checks rule out (`CpuFighter::xCC_array`) or pick from
/// (`xA8_array`).
mod cmd {
    pub const FORWARD_THROW: i32 = 10;
    pub const DOWN_THROW: i32 = 14;
    pub const DOWN_SMASH: i32 = 40;
    pub const DOWN_SMASH_2: i32 = 41;
    pub const ANY_ATTACK: i32 = 52;
}

impl Running<'_> {
    /// Whether the fighter's target hangs from (1) or holds (2) a ledge, `ftCo_800A3200`.
    // Ports `ftCo_800A3200`; aliased once a check reaches it.
    pub(super) fn cpu_on_ledge(&self, f: FighterId) -> i32 {
        match self.game.fighters[f.0].motion_id {
            // `ftCo_MS_CliffCatch`, `ftCo_MS_CliffWait`.
            252 => 1,
            253 => 2,
            _ => 0,
        }
    }

    /// Whether the fighter lies on the ground face up (1) or down (2), `ftCo_800A3134`.
    // Ports `ftCo_800A3134`; aliased once a check reaches it.
    pub(super) fn cpu_lying(&self, f: FighterId) -> i32 {
        match self.game.fighters[f.0].motion_id {
            191 | 183 => 1,
            192 | 184 => 2,
            _ => 0,
        }
    }

    /// Rules out an attack command for this decision (`xCC_array`), up to eight.
    fn cpu_rule_out(&mut self, f: FighterId, c: i32) {
        let d = &mut self.game.fighters[f.0].cpu;
        if d.xEC < 8 {
            d.xCC_array[usize::from(d.xEC)] = c;
            d.xEC += 1;
        }
    }

    /// Restricts the attack to a command (`xA8_array`), up to eight.
    fn cpu_only(&mut self, f: FighterId, c: i32) {
        let d = &mut self.game.fighters[f.0].cpu;
        if d.xC8 < 8 {
            d.xA8_array[usize::from(d.xC8)] = c;
            d.xC8 += 1;
        }
    }

    /// The character's table of a kind in the computer players' data.
    fn cpu_table(&self, which: u32, kind: i32) -> Option<DataRef> {
        let data = self.common_ptr(CPU_DATA);
        let tables = self.data.ptr(data.at(which))?;
        self.data.ptr(tables.at(4 * kind as u32))
    }

    /// What the fighter does from range with the item it holds, `ftCo_800B9A04`: nothing
    /// with a healing item or mid-swing or shot; a bomb about to go off is thrown (2); an
    /// item it shoots is thrown away empty (0x28), else shot, toward a target it faces (2,
    /// the Star Rod's 0xF, straight at one more than 30 away, else 0x2A); a container is
    /// thrown (2), a thrown item up (0xD), down (0xE) or ahead (0xF; a Poké Ball down);
    /// any other but a swung one is thrown ahead (0x2A) at a target more than 30 away
    /// within 15 degrees.
    #[doc(alias = "ftCo_800B9A04")]
    fn cpu_held_item_attack(&mut self, f: FighterId, it: ItemId, angle: f64, dist: f64) -> bool {
        use super::super::it::kinds as k;
        if self.cpu_item_heals(it) || self.cpu_item_attacking(f) {
            return false;
        }
        let g = self.game.fighters[f.0].item_gobj.expect("a held item");
        let fuse = reg(super::super::it::lib26b1::fuse_left(self.game, g));
        if fuse < 60.0 && fuse > 0.0 {
            self.game.fighters[f.0].cpu.xA4 = 2;
            return true;
        }
        let kind = self.game.items[it.0].kind;
        // `ftCo_800A59C0`: an item the fighter shoots.
        if super::super::it::lib26b1::use_kind(self.game, self.data, g) == 3 {
            let left = fp::frsp(f64::from(super::super::it::lib26b1::uses_left(
                self.game, g,
            )));
            if left < 9.999_999_747_378_752e-6 && left > -9.999_999_747_378_752e-6 {
                self.game.fighters[f.0].cpu.xA4 = 0x28;
                return true;
            }
            if !self.cpu_target_ahead(f) {
                return false;
            }
            let a = if angle < 0.087_266_461_923_718_45
                && angle > -0.087_266_461_923_718_45
                && dist > 30.0
            {
                if kind == k::STARROD { 0xF } else { 2 }
            } else {
                0x2A
            };
            self.game.fighters[f.0].cpu.xA4 = a;
            return true;
        }
        // `ftCo_800A5A5C`: a container.
        if matches!(kind, k::BOX | k::TARU | k::KUSUDAMA | k::TARU_CANN) {
            self.game.fighters[f.0].cpu.xA4 = 2;
            return true;
        }
        // `ftCo_800A5A28`: an item thrown at the target.
        if matches!(
            kind,
            k::CAPSULE
                | k::BOMBHEI
                | k::DOSEI
                | k::G_SHELL
                | k::R_SHELL
                | k::FREEZE
                | k::MSBOMB
                | k::FLIPPER
                | k::M_BALL
        ) {
            let a = if angle > 1.308_996_928_855_776_8 {
                0xD
            } else if angle < -1.308_996_928_855_776_8 {
                0xE
            } else if !self.cpu_target_ahead(f) {
                return false;
            } else if angle < 0.261_799_385_771_155_36 && angle > -0.261_799_385_771_155_36 {
                0xF
            } else if kind == k::M_BALL {
                0xE
            } else {
                return false;
            };
            self.game.fighters[f.0].cpu.xA4 = a;
            return true;
        }
        if self.cpu_item_swung(it) {
            return false;
        }
        if dist > 30.0 && angle < 0.261_799_385_771_155_36 && angle > -0.261_799_385_771_155_36 {
            self.game.fighters[f.0].cpu.xA4 = 0x2A;
            return true;
        }
        false
    }

    /// Whether an item is swung, `ftCo_800A59E4`: the fan, Star Rod, Lip's Stick, bat, Beam
    /// Sword or parasol.
    #[doc(alias = "ftCo_800A59E4")]
    pub(super) fn cpu_item_swung(&self, it: ItemId) -> bool {
        use super::super::it::kinds as k;
        matches!(
            self.game.items[it.0].kind,
            k::HARISEN | k::STARROD | k::LIPSTICK | k::BAT | k::SWORD | k::PARASOL
        )
    }

    /// Whether a fighter is raising or holding its shield, `ftCo_800B9F6C`.
    #[doc(alias = "ftCo_800B9F6C")]
    fn cpu_shielding(&self, f: FighterId) -> bool {
        // `ftCo_MS_GuardOn`, `ftCo_MS_Guard`.
        matches!(self.game.fighters[f.0].motion_id, 178 | 179)
    }

    /// Whether the fighter uses a ranged attack (a projectile), `ftCo_800B9CBC`: only with a
    /// target, if it may (`xF9_b5`), once its delay (`x34`) runs out.
    #[doc(alias = "ftCo_800B9CBC")]
    pub(super) fn cpu_ranged_attack(&mut self, f: FighterId) -> bool {
        let c = &mut self.game.fighters[f.0].cpu;
        c.xA4 = 0;
        if c.x44.is_none() || !c.xF9_b5 {
            return false;
        }
        if c.x34 > 0 {
            c.x34 -= 1;
            return false;
        }
        self.cpu_attack_timer(f);
        if self.cpu_wall_before_target(f) {
            return false;
        }
        let fpp = &self.game.fighters[f.0];
        let t = fpp.cpu.x44.expect("checked above");
        let dist = self.cpu_distance(f, t);
        let (me, tp) = (fpp.cur_pos, self.game.fighters[t.0].cur_pos);
        let dx = abs(fp::fsubs(reg(tp.x), reg(me.x)));
        let angle = super::co::guard::stick_angle(fp::fsubs(reg(tp.y), reg(me.y)), dx);
        if let Some(g) = fpp.item_gobj {
            let it = super::super::it::item_of(self.game, g);
            return self.cpu_held_item_attack(f, it, angle, dist);
        }
        if reg(self.game.objs.rng.draw_f32()) > 0.75 {
            return false;
        }
        if dist < 30.0 {
            return false;
        }
        let fpp = &self.game.fighters[f.0];
        let picked = match fpp.kind {
            KIND_NESS => self.cpu_ness_ranged(f, angle, dist),
            KIND_YOSHI => self.cpu_yoshi_ranged(f, angle, dist),
            KIND_SAMUS => {
                if fpp.ground_or_air == super::common::GA_GROUND {
                    false
                } else if angle < -0.785_398_157_313_466_1 {
                    self.game.fighters[f.0].cpu.xA4 = 0x26;
                    true
                } else {
                    false
                }
            }
            _ => false,
        };
        if picked {
            return true;
        }
        if !self.cpu_target_ahead(f) {
            return false;
        }
        if angle < 0.087_266_461_923_718_45 && angle > -0.087_266_461_923_718_45 {
            let kind = self.game.fighters[f.0].kind;
            let a = self.cpu_pick_entry(table::RANGED, kind);
            self.game.fighters[f.0].cpu.xA4 = a;
            return a != 0;
        }
        false
    }

    /// Whether a wall stands between the fighter and its target, the way it faces,
    /// `ftCo_800A1BA8`.
    #[doc(alias = "ftCo_800A1BA8")]
    fn cpu_wall_before_target(&mut self, f: FighterId) -> bool {
        let fpp = &self.game.fighters[f.0];
        let Some(t) = fpp.cpu.x44 else { return false };
        let (me, tp) = (fpp.cur_pos, self.game.fighters[t.0].cur_pos);
        let right = fpp.facing_dir > 0.0;
        let geo = self.game.geometry.as_mut().expect("no collision geometry");
        if right {
            geo.check_right_wall(me.x, me.y, tp.x, tp.y, -1, -1)
                .is_some()
        } else {
            geo.check_left_wall(me.x, me.y, tp.x, tp.y, -1, -1)
                .is_some()
        }
    }

    /// Whether two fighters stand on the same island, `ftCo_800A2170`.
    #[doc(alias = "ftCo_800A2170")]
    pub(super) fn cpu_same_island(&self, a: FighterId, b: FighterId) -> bool {
        let (fa, fb) = (&self.game.fighters[a.0], &self.game.fighters[b.0]);
        if fa.ground_or_air == GA_AIR || fb.ground_or_air == GA_AIR {
            return false;
        }
        let geo = self.game.geometry.as_ref().expect("no collision geometry");
        geo.island_of_line(fa.coll_data.floor.index)
            .is_some_and(|i| geo.island_of_line(fb.coll_data.floor.index) == Some(i))
    }

    /// Ness's ranged attacks, `ftCo_800B9790`: PK Fire (27) at a target level with him (or
    /// below him diagonally in the air) within 53 and ahead; PK Thunder upward (20) or, a
    /// tenth of the time, PK Flash (31) at a target above him on the ground.
    #[doc(alias = "ftCo_800B9790")]
    fn cpu_ness_ranged(&mut self, f: FighterId, angle: f64, dist: f64) -> bool {
        let fpp = &self.game.fighters[f.0];
        if fpp.ground_or_air == GA_AIR {
            if angle < -0.698_131_695_389_747_6
                && angle > -0.872_664_619_237_184_5
                && dist < 53.0
                && self.cpu_target_ahead(f)
            {
                self.game.fighters[f.0].cpu.xA4 = 0x1B;
                return true;
            }
            return false;
        }
        if angle < 0.087_266_461_923_718_45
            && angle > -0.087_266_461_923_718_45
            && dist < 53.0
            && self.cpu_target_ahead(f)
        {
            self.game.fighters[f.0].cpu.xA4 = 0x1B;
            return true;
        }
        let fpp = &self.game.fighters[f.0];
        let t = fpp.cpu.x44.expect("ftCo_800B9790 without a target");
        if fpp.cur_pos.y >= self.game.fighters[t.0].cur_pos.y {
            return false;
        }
        if angle > 1.047_197_543_084_621_4 {
            self.game.fighters[f.0].cpu.xA4 = 0x14;
        } else if reg(self.game.objs.rng.draw_f32()) < 0.1 {
            self.game.fighters[f.0].cpu.xA4 = 0x1F;
        }
        true
    }

    /// Yoshi's ranged attacks, `ftCo_800B98C8`, on the ground at a target ahead within 80:
    /// the egg roll (27) at a target on his island, the egg throw upward by the angle (49 to
    /// 51).
    #[doc(alias = "ftCo_800B98C8")]
    fn cpu_yoshi_ranged(&mut self, f: FighterId, angle: f64, dist: f64) -> bool {
        if self.game.fighters[f.0].ground_or_air == GA_AIR {
            return false;
        }
        if !self.cpu_target_ahead(f) {
            return false;
        }
        if dist > 80.0 {
            return false;
        }
        let t = self.game.fighters[f.0]
            .cpu
            .x44
            .expect("ftCo_800B98C8 without a target");
        let a = if angle < 0.087_266_461_923_718_45 && angle > -0.087_266_461_923_718_45 {
            if self.game.fighters[t.0].ground_or_air == GA_AIR || !self.cpu_same_island(f, t) {
                return false;
            }
            0x1B
        } else if angle < 0.785_398_157_313_466_1 {
            return false;
        } else if angle < 1.047_197_543_084_621_4 {
            0x31
        } else if angle < 1.308_996_928_855_776_8 {
            0x32
        } else {
            0x33
        };
        self.game.fighters[f.0].cpu.xA4 = a;
        true
    }

    /// A random command from a character's table, weighed by each entry's weight,
    /// `ftCo_800B6208`; 0 for an empty table or weights summing to zero.
    #[doc(alias = "ftCo_800B6208")]
    fn cpu_pick_entry(&mut self, which: u32, kind: i32) -> i32 {
        let r = reg(self.game.objs.rng.draw_f32());
        let arr = self
            .cpu_table(which, kind)
            .expect("a computer attack table");
        let mut sum = 0.0;
        let mut n = 0;
        while self.data.u32(arr.at(n * ENTRY_SIZE)) != 0 {
            sum = fp::fadds(
                sum,
                reg(self.data.f32(arr.at(n * ENTRY_SIZE + ENTRY_WEIGHT))),
            );
            n += 1;
        }
        if n == 0 {
            return 0;
        }
        if sum < reg(1e-5) && sum > fp::fneg(reg(1e-5)) {
            return 0;
        }
        let inv = fp::frsp(fp::fdiv(1.0, sum));
        let mut acc = 0.0;
        for i in 0..n {
            acc = fp::fadds(
                acc,
                reg(self.data.f32(arr.at(i * ENTRY_SIZE + ENTRY_WEIGHT))),
            );
            if fp::fmuls(acc, inv) >= r {
                return self.data.u32(arr.at(i * ENTRY_SIZE)) as i32;
            }
        }
        panic!("ftcpuattack.c:650: no attack picked (ftCo_800B6208)");
    }

    /// Whether the fighter's skill keeps it from attacking now, `ftCo_800B885C`: below level
    /// 5, for a stretch of each period of its script count, longer the lower its level.
    // Ports `ftCo_800B885C`; aliased once a check reaches it.
    fn cpu_holds_back(&self, f: FighterId) -> bool {
        let c = &self.game.fighters[f.0].cpu;
        let (period, limit) = match c.level {
            0 => (0x12C, 240),
            1 => (0x12C, 180),
            2 => (0x12C, 120),
            3 => (240, 120),
            4 => (240, 60),
            _ => return false,
        };
        c.x80 % period <= limit
    }

    /// The character's own rules on attacks for this decision, `ftCo_800B77E8`: an out-of-
    /// action or untargetable fighter rules out every attack; the characters whose specials
    /// travel rule out the one that would carry them off the stage or past a wall
    /// ([`Running::cpu_special_blocked`]); Kirby rules out his copy's special unless it's one
    /// that charges, and his inhale at a bigger target.
    #[doc(alias = "ftCo_800B77E8")]
    fn cpu_character_rules(&mut self, f: FighterId) {
        if self.game.fighters[f.0].cpu.kind == 7 || self.cpu_untargetable(f) {
            self.cpu_rule_out(f, cmd::ANY_ATTACK);
        }
        let fpp = &self.game.fighters[f.0];
        let ground = fpp.ground_or_air == super::common::GA_GROUND;
        let off_stage = !fpp.cpu.xFA_b5;
        // Each rule: the command it rules out and the reach it tests.
        match fpp.kind {
            KIND_MARIO => {
                if self.cpu_special_blocked(f, 22.0) {
                    self.cpu_rule_out(f, 0x1F);
                }
            }
            KIND_FOX | KIND_FALCO => {
                if self.cpu_special_blocked(f, 62.0) {
                    self.cpu_rule_out(f, 0x1B);
                }
            }
            KIND_CAPTAIN | KIND_GANON => {
                if self.cpu_special_blocked(f, 102.0) {
                    self.cpu_rule_out(f, 0x26);
                }
                if self.cpu_special_blocked(f, 58.0) {
                    self.cpu_rule_out(f, 0x1B);
                    self.cpu_rule_out(f, 0x37);
                    self.cpu_rule_out(f, 0x38);
                }
            }
            KIND_PIKACHU | KIND_PICHU => {
                if self.cpu_special_blocked(f, 57.0) {
                    self.cpu_rule_out(f, 0x1B);
                }
            }
            KIND_KOOPA | KIND_GKOOPS | KIND_YOSHI => {
                let reach = match fpp.kind {
                    KIND_KOOPA => 27.0,
                    KIND_GKOOPS => 53.0,
                    _ => 16.0,
                };
                if ground {
                    if self.cpu_special_blocked(f, reach) {
                        self.cpu_rule_out(f, 0x26);
                    }
                } else if off_stage {
                    self.cpu_rule_out(f, 0x26);
                }
            }
            KIND_LUIGI | KIND_PEACH => {
                let reach = if fpp.kind == KIND_LUIGI { 60.0 } else { 32.0 };
                if self.cpu_special_blocked(f, reach) {
                    self.cpu_rule_out(f, 0x26);
                }
            }
            KIND_KIRBY => {
                if !ground && off_stage {
                    self.cpu_rule_out(f, 0x26);
                }
                let fpp = &self.game.fighters[f.0];
                // His copy's kind (`u.kb.hat.kind`): Donkey Kong, Samus or Mewtwo charge.
                let copy = fpp.u_i32(super::kirby::u::HAT_KIND);
                if matches!(copy, KIND_DONKEY | KIND_SAMUS | KIND_MEWTWO) {
                    self.cpu_rule_out(f, 0x11);
                } else {
                    let (scale, t) = (fpp.x34_scale.y, fpp.cpu.x44);
                    self.cpu_rule_out(f, 0x12);
                    let t = t.expect("ftCo_800B77E8 for Kirby without a target");
                    if scale < self.game.fighters[t.0].x34_scale.y {
                        self.cpu_rule_out(f, 0x11);
                    }
                }
            }
            _ => {}
        }
    }

    /// Whether a traveling special would carry the fighter somewhere it shouldn't, as
    /// `ftCo_CpuCanUseRangedAttack` asks it: a surface between it and its target, or no floor
    /// within 1000 below the point `reach` ahead of it.
    fn cpu_special_blocked(&mut self, f: FighterId, reach: f32) -> bool {
        let fpp = &self.game.fighters[f.0];
        let me = fpp.cur_pos;
        let right = fpp.facing_dir > 0.0;
        if let Some(t) = fpp.cpu.x44 {
            let tp = self.game.fighters[t.0].cur_pos;
            let geo = self.game.geometry.as_mut().expect("no collision geometry");
            if geo.check_all(-1, -1, me.x, me.y, tp.x, tp.y).is_some() {
                return true;
            }
        }
        let x = mem(if right {
            fp::fadds(reg(me.x), reg(reach))
        } else {
            fp::fsubs(reg(me.x), reg(reach))
        });
        let above = mem(fp::frsp(fp::fadd(5.0, reg(me.y))));
        let below = mem(fp::frsp(fp::fsub(reg(me.y), 1000.0)));
        self.cpu_floor(x, above, x, below).is_none()
    }

    /// Whether the fighter attacks in the air now, `ftCo_800B89CC`: always against a target
    /// on a ledge; else if it may (`xF9_b4`), at level 4 and up (Bowser only over ground),
    /// not past Kongo Jungle's middle.
    // Ports `ftCo_800B89CC`; aliased once a check reaches it.
    fn cpu_air_attack_ok(&self, f: FighterId) -> bool {
        let fpp = &self.game.fighters[f.0];
        // Gr_Kind_Kongo.
        if self.game.stage_info.grkind == 4 && (fpp.cur_pos.x > 25.0 || fpp.cur_pos.x < -25.0) {
            return false;
        }
        let target = fpp.cpu.x44.expect("ftCo_800B89CC without a target");
        if self.cpu_on_ledge(target) != 0 {
            return true;
        }
        let c = &fpp.cpu;
        if !c.xF9_b4 {
            return false;
        }
        // Ft_Kind_Koopa.
        if !c.xFA_b5 && fpp.kind == 5 {
            return false;
        }
        c.level >= 4
    }

    /// Whether the fighter attacks its target now and with what, `ftCo_800B8A9C`: by the
    /// character's tables, ruling out what doesn't fit (a grab on a target already grabbing
    /// or bigger, the down smash), from the air, against a target lying down or on a ledge,
    /// with a weapon, a smash, an edge guard, a ground attack or a special on an enemy item.
    /// The chosen command goes in `xA4`.
    // Ports `ftCo_800B8A9C`; aliased once a check reaches it.
    pub(super) fn cpu_attack(&mut self, f: FighterId) -> bool {
        if !self.game.fighters[f.0].cpu.xF9_b2 {
            return false;
        }
        let c = &mut self.game.fighters[f.0].cpu;
        let target = c.x44;
        c.xEC = 0;
        c.xC8 = 0;
        // `ftCo_MS_Pass`.
        let out = match target {
            None => true,
            Some(t) => {
                self.game.fighters[f.0].motion_id == 244
                    || self.cpu_untargetable(t)
                    || self.cpu_holds_back(f)
                    || self.game.fighters[f.0].x2168 != 0
            }
        };
        if out {
            self.game.fighters[f.0].cpu.xA4 = 0;
            return false;
        }
        let target = target.unwrap();
        let tms = self.game.fighters[target.0].motion_id;
        // `ftCo_MS_Catch` to `ftCo_MS_EscapeAir`.
        if (212..=236).contains(&tms) {
            self.cpu_rule_out(f, cmd::DOWN_SMASH);
            self.cpu_rule_out(f, cmd::DOWN_SMASH_2);
        } else if self.game.fighters[target.0].x34_scale.y > self.game.fighters[f.0].x34_scale.y {
            self.cpu_rule_out(f, cmd::DOWN_SMASH);
            self.cpu_rule_out(f, cmd::DOWN_SMASH_2);
            // Ft_Kind_Kirby.
            if self.game.fighters[f.0].kind == 4 {
                self.cpu_rule_out(f, 17);
            }
        }
        self.cpu_character_rules(f);
        let kind = self.game.fighters[f.0].kind;
        let result = 'done: {
            if self.game.fighters[f.0].ground_or_air == GA_AIR {
                if self.cpu_air_attack_ok(f) {
                    let r = self.cpu_pick_attack(f, target, self.cpu_table(table::AIR, kind));
                    if r != 0 {
                        break 'done r;
                    }
                }
                break 'done 0;
            }
            if self.cpu_lying(target) != 0 || self.cpu_on_ledge(target) != 0 {
                self.cpu_only(f, cmd::FORWARD_THROW);
                self.cpu_only(f, cmd::DOWN_THROW);
                let r = self.cpu_pick_attack(f, target, self.cpu_table(table::GROUND, kind));
                break 'done r;
            }
            // A swung item's attacks (`ftCo_800B52AC` with the item's reach); the fighters'
            // own items aren't swung.
            if let Some(g) = self.game.fighters[f.0].item_gobj
                && self.cpu_item_swung(super::super::it::item_of(self.game, g))
            {
                unported_call("ftCo_800B8A9C with a swung item (ftCo_800B52AC)");
            }
            if self.game.fighters[f.0].cpu.level > 5 && self.cpu_shielding(target) {
                let r = self.cpu_pick_attack(f, target, self.cpu_table(table::SHIELD, kind));
                if r != 0 {
                    break 'done r;
                }
            }
            // An edge guard on a target over no floor.
            let tp = self.game.fighters[target.0].cur_pos;
            let edge =
                if self.game.fighters[f.0].item_gobj.is_some() || self.cpu_on_ledge(target) != 0 {
                    false
                } else {
                    // `ftCo_800A0FB0`.
                    let above = mem(fp::frsp(fp::fadd(5.0, reg(tp.y))));
                    let below = mem(fp::frsp(fp::fsub(reg(tp.y), 1000.0)));
                    self.cpu_floor(tp.x, above, tp.x, below).is_none()
                };
            if edge {
                let r = self.cpu_pick_attack(f, target, self.cpu_table(table::EDGE_GUARD, kind));
                if r != 0 {
                    self.game.fighters[f.0].cpu.xF8_b7 = true;
                    break 'done r;
                }
            }
            let r = self.cpu_pick_attack(f, target, self.cpu_table(table::GROUND, kind));
            if r != 0 {
                break 'done r;
            }
            if self.game.fighters[f.0].cpu.x50.is_some() {
                unported_call("ftCo_800B5AB0");
            }
            0
        };
        self.game.fighters[f.0].cpu.xA4 = result;
        result != 0
    }

    /// Picks an attack from a table, `ftCo_800B4AB0`: each entry the fighter's level allows
    /// and the decision hasn't ruled out, whose reach (scaled by the fighter's size and its
    /// recovery scale) covers where the target will be relative to the fighter after the
    /// entry's frames (falling under gravity to terminal speed in the air, the target's hurt
    /// box around it), joins the candidates when its period comes up (or it's one the
    /// decision restricts to); one is drawn by weight. Its reach goes in `x6C` and `x74`.
    // Ports `ftCo_800B4AB0`; aliased once a check reaches it.
    fn cpu_pick_attack(&mut self, f: FighterId, target: FighterId, list: Option<DataRef>) -> i32 {
        let c = &mut self.game.fighters[f.0].cpu;
        c.x74.y = 0.0;
        c.x74.x = 0.0;
        c.x6C.y = 0.0;
        c.x6C.x = 0.0;
        let half_range = fp::frsp(fp::fmadd(0.5, reg(c.x570), 0.5));
        let Some(mut list) = list else { return 0 };
        let me = &self.game.fighters[f.0];
        let tg = &self.game.fighters[target.0];
        let fp_term_neg = fp::fneg(reg(me.co_attrs.terminal_velocity));
        let tgt_term_neg = fp::fneg(reg(tg.co_attrs.terminal_velocity));
        let (fp_x, fp_y) = (reg(me.cur_pos.x), reg(me.cur_pos.y));
        let (fp_vx, fp_vy) = (reg(me.pos_delta.x), reg(me.pos_delta.y));
        let fp_grav = reg(me.co_attrs.gravity);
        let (tgt_x, tgt_y) = (reg(tg.cur_pos.x), reg(tg.cur_pos.y));
        let (tgt_vx, tgt_vy) = (reg(tg.pos_delta.x), reg(tg.pos_delta.y));
        let tgt_grav = reg(tg.co_attrs.gravity);
        let (range_f, range_b) = if f64::from(tg.facing_dir) > 0.0 {
            (reg(tg.cpu.x55C), reg(tg.cpu.x560))
        } else {
            (reg(tg.cpu.x560), reg(tg.cpu.x55C))
        };
        let x568 = reg(tg.cpu.x568);
        let target_air = tg.ground_or_air == GA_AIR;
        let scale = reg(me.x34_scale.y);
        let facing_right = f64::from(me.facing_dir) > 0.0;
        let tiny = reg(1e-5);
        let mut picks: [Option<DataRef>; 32] = [None; 32];
        let mut count = 0usize;
        loop {
            let entry = list;
            let ecmd = self.data.u32(entry) as i32;
            if ecmd == 0 {
                break;
            }
            list = list.at(ENTRY_SIZE);
            let c = &self.game.fighters[f.0].cpu;
            if (self.data.u32(entry.at(0x20)) as i32) > c.level {
                continue;
            }
            if c.xCC_array[..usize::from(c.xEC)].contains(&ecmd) {
                continue;
            }
            let t = fp::frsp(f64::from(self.data.u32(entry.at(4)) as i32));
            let relx = fp::fsubs(fp::fmadds(tgt_vx, t, tgt_x), fp::fmadds(fp_vx, t, fp_x));
            // The original predicts the fighter's own fall only when the target is airborne.
            let fp_pred_y = if target_air {
                let v = if fp_grav < tiny && fp_grav > fp::fneg(tiny) {
                    1000.0
                } else {
                    fp::fdivs(fp::fneg(fp::fsubs(fp_term_neg, fp_y)), fp_grav)
                };
                if v <= 0.0 {
                    fp::fmadds(fp_vy, t, fp_y)
                } else if t < v {
                    let sq = math::sqrt_reg(t);
                    fp::frsp(fp::fadd(
                        fp_y,
                        fp::fnmsub(0.5, fp::fmuls(fp_grav, sq), fp::fmuls(fp_vy, t)),
                    ))
                } else {
                    let sq = math::sqrt_reg(v);
                    fp::frsp(fp::fadd(
                        fp_y,
                        fp::fadd(
                            fp::fmuls(fp_term_neg, fp::fsubs(t, v)),
                            fp::fnmsub(0.5, fp::fmuls(fp_grav, sq), fp::fmuls(fp_vy, t)),
                        ),
                    ))
                }
            } else {
                fp::fmadds(fp_vy, t, fp_y)
            };
            let rel_pred_y = if target_air {
                let v = if tgt_grav < tiny && tgt_grav > fp::fneg(tiny) {
                    1000.0
                } else {
                    fp::fdivs(fp::fneg(fp::fsubs(tgt_term_neg, tgt_vy)), tgt_grav)
                };
                if v <= 0.0 {
                    fp::fsubs(fp::fmadds(tgt_vy, t, tgt_y), fp_pred_y)
                } else if t < v {
                    let sq = math::sqrt_reg(t);
                    fp::frsp(fp::fsub(
                        fp::fnmsub(0.5, fp::fmuls(tgt_grav, sq), fp::fmadds(tgt_vy, t, tgt_y)),
                        fp_pred_y,
                    ))
                } else {
                    let sq = math::sqrt_reg(v);
                    fp::frsp(fp::fsub(
                        fp::fadd(
                            fp::fmuls(tgt_term_neg, fp::fsubs(t, v)),
                            fp::fnmsub(0.5, fp::fmuls(tgt_grav, sq), fp::fmadds(tgt_vy, t, tgt_y)),
                        ),
                        fp_pred_y,
                    ))
                }
            } else {
                fp::fsubs(fp::fmadds(tgt_vy, t, tgt_y), fp_pred_y)
            };
            let (x08, x0c) = (
                reg(self.data.f32(entry.at(8))),
                reg(self.data.f32(entry.at(0xC))),
            );
            let (mut dirx, mut diry) = if facing_right {
                (fp::fmuls(x08, scale), fp::fmuls(x0c, scale))
            } else {
                (
                    fp::fmuls(fp::fneg(x0c), scale),
                    fp::fmuls(fp::fneg(x08), scale),
                )
            };
            dirx = fp::fmuls(dirx, half_range);
            diry = fp::fmuls(diry, half_range);
            let upper = fp::fmuls(
                fp::fmuls(reg(self.data.f32(entry.at(0x14))), scale),
                half_range,
            );
            let lower = fp::fmuls(
                fp::fmuls(reg(self.data.f32(entry.at(0x10))), scale),
                half_range,
            );
            if upper > rel_pred_y
                && lower < fp::fadds(rel_pred_y, x568)
                && dirx < fp::fadds(relx, range_f)
                && diry > fp::fsubs(relx, range_b)
            {
                let c = &self.game.fighters[f.0].cpu;
                if c.xC8 != 0 {
                    for j in 0..usize::from(c.xC8) {
                        if c.xA8_array[j] == ecmd {
                            picks[count] = Some(entry);
                            count += 1;
                        }
                    }
                } else if c.x80 % (self.data.u32(entry.at(0x1C)) as i32) == 0 {
                    picks[count] = Some(entry);
                    count += 1;
                }
            }
        }
        if count == 0 {
            return 0;
        }
        let r = reg(self.game.objs.rng.draw_f32());
        let weight = |e: Option<DataRef>| reg(self.data.f32(e.unwrap().at(0x18)));
        let mut sum = 0.0;
        for &p in &picks[..count] {
            sum = fp::fadds(sum, weight(p));
        }
        if sum < tiny && sum > fp::fneg(tiny) {
            return 0;
        }
        let inv = fp::frsp(fp::fdiv(1.0, sum));
        let mut acc = 0.0;
        for &p in &picks[..count] {
            acc = fp::fadds(acc, weight(p));
            if fp::fmuls(acc, inv) >= r {
                // `ftCo_CpuSelectAttack`.
                let e = p.unwrap();
                let scale = reg(self.game.fighters[f.0].x34_scale.y);
                let (x08, x0c) = (self.data.f32(e.at(8)), self.data.f32(e.at(0xC)));
                let (x10, x14) = (self.data.f32(e.at(0x10)), self.data.f32(e.at(0x14)));
                let c = &mut self.game.fighters[f.0].cpu;
                c.x6C.x = mem(fp::fmuls(scale, reg(x08)));
                c.x6C.y = mem(fp::fmuls(scale, reg(x10)));
                c.x74.x = mem(fp::fmuls(scale, reg(x0c)));
                c.x74.y = mem(fp::fmuls(scale, reg(x14)));
                return self.data.u32(e) as i32;
            }
        }
        panic!("the original's assertion failed: ftcpuattack.c:250");
    }

    /// Whether the fighter is swinging or shooting an item, `ftCo_800A5A90`:
    /// `ftCo_MS_SwordSwing1` to `ftCo_MS_StarRodSwingDash`, `ftCo_MS_LGunShoot` to
    /// `ftCo_MS_ItemScopeAirEndEmpty`.
    #[doc(alias = "ftCo_800A5A90")]
    fn cpu_item_attacking(&self, f: FighterId) -> bool {
        let ms = self.game.fighters[f.0].motion_id;
        (120..=139).contains(&ms) || (148..=173).contains(&ms)
    }

    /// Whether the fighter is mid-attack, `ftCo_800B630C`: in a common attack or an item's
    /// swing or shot ([`Running::cpu_item_attacking`]), or in one of its character's own
    /// states.
    #[doc(alias = "ftCo_800B630C")]
    fn cpu_attacking(&self, f: FighterId) -> bool {
        let fpp = &self.game.fighters[f.0];
        let ms = fpp.motion_id;
        // `ftCo_MS_Attack11` to `ftCo_MS_AttackAirLw`.
        if (44..=69).contains(&ms) || self.cpu_item_attacking(f) {
            return true;
        }
        // Donkey Kong's, Kirby's and Peach's first own states aren't attacks: the carry and
        // its throws (`ftDk_MS_ThrowAirFLw`), the air jumps and dash attack in the air
        // (`ftKb_MS_AttackDashAir`), the float (`ftPe_MS_FloatFallB`).
        match fpp.kind {
            KIND_DONKEY => ms > 368,
            KIND_KIRBY => ms > super::kirby::ms::ATTACK_DASH_AIR,
            KIND_PEACH => ms > super::peach::ms::FLOAT_FALL_B,
            _ => ms >= 341,
        }
    }

    /// The pause before an attack by level and a random draw (`ftCo_800B63D8`).
    // Ports `ftCo_800B63D8`; aliased once a check reaches it.
    fn cpu_attack_delay(&mut self, f: FighterId) {
        let r = reg(self.game.objs.rng.draw_f32());
        let (scale, add) = match self.game.fighters[f.0].cpu.level {
            0 => (40, 5),
            1 => (35, 5),
            2 => (30, 5),
            3 => (25, 5),
            4 => (25, 0),
            5 => (20, 0),
            6 => (15, 0),
            7 => (10, 0),
            8 => (5, 0),
            _ => return,
        };
        let wait = fp::fctiwz(fp::fmuls(fp::frsp(f64::from(scale)), r)).wrapping_add(add);
        self.cpu_cmd1(f, super::cpu_script::cmd::WAIT_FOR, wait as u8);
    }

    /// Forgets the target and goes back to the usual behavior
    /// (`ftCo_CpuClearTargetAndFinish`).
    fn cpu_drop_target(&mut self, f: FighterId) {
        let c = &mut self.game.fighters[f.0].cpu;
        c.x44 = None;
        c.x18 = c.x1C;
        self.cpu_cmd(f, super::cpu_script::cmd::DONE);
    }

    /// Attacking, `ftCo_800B658C` (behavior 2): with a target, after a possible jump at it,
    /// the character's own follow-ups, then (not mid-attack) the chosen command (choosing one
    /// if none, facing the target first on the ground) after a pause.
    #[doc(alias = "ftCo_800B658C")]
    pub(super) fn cpu_attack_behavior(&mut self, f: FighterId) {
        let Some(target) = self.game.fighters[f.0].cpu.x44 else {
            self.cpu_drop_target(f);
            return;
        };
        let fpp = &self.game.fighters[f.0];
        if fpp.cpu.xF8_b6 && fpp.ground_or_air == super::common::GA_GROUND {
            let r = reg(self.game.objs.rng.draw_f32());
            let level = fp::frsp(f64::from(self.game.fighters[f.0].cpu.level));
            if r < fp::fmuls(0.050_000_000_745_058_06, level) {
                self.game.fighters[f.0].cpu.xF8_b6 = false;
                let _ = target;
                unported_call("ftCo_800BA2E8");
            }
        }
        if self.cpu_follow_up(f, false) {
            return;
        }
        if self.cpu_attacking(f) {
            self.cpu_cmd(f, super::cpu_script::cmd::DONE);
            self.game.fighters[f.0].cpu.xA4 = 0;
            return;
        }
        if self.game.fighters[f.0].cpu.xA4 == 0 {
            if self.game.fighters[f.0].ground_or_air == super::common::GA_GROUND
                && !self.cpu_target_ahead(f)
            {
                self.cpu_turn_around(f);
                return;
            }
            if !self.cpu_attack(f) {
                self.cpu_drop_target(f);
                return;
            }
        }
        self.cpu_attack_delay(f);
        let cmd = self.game.fighters[f.0].cpu.xA4;
        self.cpu_data_script(f, cmd as u32);
        self.game.fighters[f.0].cpu.xA4 = 0;
    }

    /// The characters' follow-ups to a special under way, shared by the attack
    /// ([`Running::cpu_attack_behavior`]) and the ranged attack ([`Running::cpu_ranged`]):
    /// Ness steers PK Flash and PK Thunder, Yoshi his egg roll, Zelda Din's Fire, each
    /// ending the decision (true), and the ranged attack counts it (`x90`); in the attack,
    /// Samus and Donkey Kong let a charge go and go on.
    fn cpu_follow_up(&mut self, f: FighterId, ranged: bool) -> bool {
        let fpp = &self.game.fighters[f.0];
        let ms = fpp.motion_id;
        let counted = match fpp.kind {
            KIND_NESS => {
                use super::ness::ms;
                let m = fpp.motion_id;
                if (ms::SPECIAL_N_START..=ms::SPECIAL_N_END).contains(&m) {
                    self.cpu_steer_pk_flash(f);
                    true
                } else if (ms::SPECIAL_HI_START..=ms::SPECIAL_AIR_HI_REBOUND).contains(&m) {
                    self.cpu_steer_pk_thunder(f);
                    true
                } else {
                    return false;
                }
            }
            KIND_YOSHI => {
                // `ftYs_MS_SpecialAirSStart_0` to `_SpecialAirSEnd`.
                if !(0x164..=0x167).contains(&ms) {
                    return false;
                }
                self.cpu_steer_egg_roll(f);
                true
            }
            KIND_ZELDA => {
                // `ftZd_MS_SpecialSLoop` to `_SpecialAirSEnd`; Din's Fire isn't counted.
                if !(0x158..=0x15C).contains(&ms) {
                    return false;
                }
                self.cpu_steer_din_fire(f);
                return true;
            }
            KIND_SAMUS | KIND_DONKEY if !ranged => {
                let charging = if fpp.kind == KIND_SAMUS {
                    ms == super::samus::ms::SPECIAL_N_HOLD
                } else {
                    ms == super::donkey::ms::SPECIAL_N_LOOP
                };
                if charging {
                    // `ftCo_CpuTapRAndWaitFiveFrames`.
                    use super::cpu_script::cmd as c;
                    self.cpu_cmd(f, c::PRESS_R);
                    self.cpu_cmd1(f, c::WAIT_FOR, 1);
                    self.cpu_cmd(f, c::RELEASE_R);
                    self.cpu_cmd1(f, c::WAIT_FOR, 5);
                }
                return false;
            }
            _ => return false,
        };
        if ranged && counted {
            let c = &mut self.game.fighters[f.0].cpu;
            c.x90 = c.x90.wrapping_add(1);
        }
        true
    }

    /// PK Flash steered toward its target until level with it (within 25), then let go,
    /// `ftCo_800B920C`.
    #[doc(alias = "ftCo_800B920C")]
    fn cpu_steer_pk_flash(&mut self, f: FighterId) {
        use super::cpu_script::cmd as c;
        let fpp = &self.game.fighters[f.0];
        let Some(t) = fpp.cpu.x44 else {
            self.cpu_drop_target(f);
            return;
        };
        let dy = abs(fp::fsubs(
            reg(self.game.fighters[t.0].cur_pos.y),
            reg(fpp.cur_pos.y),
        ));
        if dy < 25.0 {
            self.cpu_cmd(f, c::RELEASE_B);
            self.cpu_cmd(f, c::DONE);
            self.cpu_drop_target(f);
        } else {
            self.cpu_cmd1(f, c::LSTICK_TOWARD_FIGHTER, 0x7F);
            self.cpu_cmd(f, c::DONE);
        }
    }

    /// PK Thunder steered at the target, `ftCo_800B92D4`.
    #[doc(alias = "ftCo_800B92D4")]
    fn cpu_steer_pk_thunder(&mut self, f: FighterId) {
        use super::cpu_script::cmd as c;
        if self.game.fighters[f.0].cpu.x44.is_none() {
            self.cpu_drop_target(f);
        } else {
            self.cpu_cmd1(f, c::LSTICK_TOWARD_FIGHTER, 0x7F);
            self.cpu_cmd(f, c::DONE);
        }
    }

    /// Yoshi's egg roll steered along his island, `ftCo_800B9340`: off it from his target's
    /// (or with no target) he jumps out of it and drops the target; near either end (10) he
    /// turns back; else toward the target.
    #[doc(alias = "ftCo_800B9340")]
    fn cpu_steer_egg_roll(&mut self, f: FighterId) {
        use super::cpu_script::cmd as c;
        let fpp = &self.game.fighters[f.0];
        let (pos, target) = (fpp.cur_pos, fpp.cpu.x44);
        let geo = self.game.geometry.as_mut().expect("no collision geometry");
        let island = geo.island_below(pos, -100.0).filter(|&i| {
            target.is_some_and(|t| {
                let tp = self.game.fighters[t.0].cur_pos;
                let geo = self.game.geometry.as_mut().expect("no collision geometry");
                geo.island_below(tp, -100.0) == Some(i)
            })
        });
        let Some(island) = island else {
            let cpu = &mut self.game.fighters[f.0].cpu;
            cpu.x44 = None;
            cpu.x18 = cpu.x1C;
            self.cpu_cmd(f, c::RELEASE_B);
            self.cpu_cmd1(f, c::WAIT_FOR, 1);
            self.cpu_cmd(f, c::PRESS_B);
            self.cpu_cmd1(f, c::WAIT_FOR, 5);
            self.cpu_cmd(f, c::RELEASE_B);
            self.cpu_cmd1(f, c::WAIT_FOR, 1);
            self.cpu_cmd(f, c::DONE);
            return;
        };
        let isl = *self
            .game
            .geometry
            .as_ref()
            .expect("no collision geometry")
            .islands
            .get(island);
        self.cpu_cmd1(f, c::SET_LSTICK_Y, 0);
        if abs(fp::fsubs(reg(isl.right.x), reg(pos.x))) < 10.0 {
            self.cpu_cmd1(f, c::SET_LSTICK_X, 0x81);
        } else if abs(fp::fsubs(reg(isl.left.x), reg(pos.x))) < 10.0 {
            self.cpu_cmd1(f, c::SET_LSTICK_X, 0x7F);
        } else {
            self.cpu_cmd1(f, c::LSTICK_X_TOWARD_FIGHTER, 0x7F);
        }
        self.cpu_cmd(f, c::DONE);
    }

    /// Zelda's Din's Fire steered, `ftCo_800B9504`: with no fire it waits; toward the enemy
    /// nearest the fire; let go once the fire is past that enemy, or within 15 of a point 15
    /// above it; else up or down toward that point.
    #[doc(alias = "ftCo_800B9504")]
    fn cpu_steer_din_fire(&mut self, f: FighterId) {
        use super::cpu_script::cmd as c;
        let gobj = self.game.fighters[f.0]
            .gobj
            .expect("a fighter without a GObj");
        let Some(fire) = self.zelda_din_fire(gobj) else {
            self.cpu_cmd(f, c::DONE);
            return;
        };
        let j = super::super::it::jobj_of(self.game, fire).expect("jobj");
        let at = self.game.objs.jobjs[j].translate;
        let Some(t) = self.cpu_nearest_enemy_to(f, at) else {
            self.cpu_drop_target(f);
            return;
        };
        let (me, tp) = (
            self.game.fighters[f.0].cur_pos,
            self.game.fighters[t.0].cur_pos,
        );
        let to_enemy = abs(fp::fsubs(reg(tp.x), reg(me.x)));
        let to_fire = abs(fp::fsubs(reg(at.x), reg(me.x)));
        let release = |g: &mut Self| {
            g.cpu_cmd(f, c::RELEASE_B);
            g.cpu_cmd1(f, c::SET_LSTICK_X, 0);
            g.cpu_cmd1(f, c::SET_LSTICK_Y, 0);
            g.cpu_cmd1(f, c::WAIT_FOR, 1);
        };
        if to_fire > to_enemy {
            release(self);
        } else {
            let above = fp::fadd(15.0, reg(tp.y));
            if abs(fp::fsubs(reg(at.x), reg(tp.x))) < 15.0 && abs(fp::fsub(above, reg(at.y))) < 15.0
            {
                release(self);
            } else if fp::fadds(15.0, reg(tp.y)) > reg(at.y) {
                self.cpu_cmd1(f, c::SET_LSTICK_Y, 0x7F);
            } else {
                self.cpu_cmd1(f, c::SET_LSTICK_Y, 0x81);
            }
        }
        self.cpu_cmd(f, c::DONE);
    }

    /// The enemy nearest a point among those in reach, targetable, awake, visible and alive,
    /// `ftCo_800A4E8C`: the first such on the fighters' list, then any strictly nearer.
    #[doc(alias = "ftCo_800A4E8C")]
    fn cpu_nearest_enemy_to(&self, f: FighterId, at: Vec3) -> Option<FighterId> {
        let me = self.game.fighters[f.0].gobj;
        let mut best: Option<(FighterId, f64)> = None;
        let mut cur = self.game.world.first_in(super::cpu_ai::P_LINK_FIGHTER);
        while let Some(g) = cur {
            cur = self.game.world.gobjs[g.0].next;
            if me == Some(g) {
                continue;
            }
            let o = self.game.fighter_id(g);
            let pos = self.game.fighters[o.0].cur_pos;
            if self.cpu_out_of_bounds(f, pos.x, pos.y)
                || self.cpu_is_ally(f, o)
                || self.cpu_untargetable(o)
            {
                continue;
            }
            let other = &self.game.fighters[o.0];
            if other.is_sleeping
                || other.stamina_dead
                || self.cpu_cloaked(o)
                || self.game.ft_is_dead(g)
            {
                continue;
            }
            let dy = fp::fsubs(reg(pos.y), reg(at.y));
            let dx = fp::fsubs(reg(pos.x), reg(at.x));
            let dz = fp::fsubs(reg(pos.z), reg(at.z));
            let d2 = fp::fmadds(dz, dz, fp::fmadds(dx, dx, fp::fmuls(dy, dy)));
            let d = reg(math::sqrt(mem(d2)));
            match best {
                None => best = Some((o, d)),
                Some((_, bd)) if bd > d => best = Some((o, d)),
                _ => {}
            }
        }
        best.map(|(o, _)| o)
    }

    /// A ranged attack, `ftCo_800B9020` (behavior 3): with a target, a special under way
    /// steered ([`Running::cpu_follow_up`]); mid-attack it waits; else the command chosen
    /// (or chosen again now, [`Running::cpu_ranged_attack`]), each counted (`x90`); with
    /// none it drops the target.
    #[doc(alias = "ftCo_800B9020")]
    pub(super) fn cpu_ranged(&mut self, f: FighterId) {
        use super::cpu_script::cmd as c;
        if self.game.fighters[f.0].cpu.x44.is_none() {
            self.cpu_drop_target(f);
            return;
        }
        if self.cpu_follow_up(f, true) {
            return;
        }
        if self.cpu_attacking(f) {
            self.cpu_cmd1(f, c::SET_LSTICK_X, 0);
            self.cpu_cmd1(f, c::SET_LSTICK_Y, 0);
            self.cpu_cmd(f, c::DONE);
            self.game.fighters[f.0].cpu.xA4 = 0;
        } else if self.game.fighters[f.0].cpu.xA4 != 0 {
            let a = self.game.fighters[f.0].cpu.xA4;
            self.cpu_data_script(f, a as u32);
            self.game.fighters[f.0].cpu.xA4 = 0;
        } else if self.cpu_ranged_attack(f) && self.game.fighters[f.0].cpu.xA4 != 0 {
            let a = self.game.fighters[f.0].cpu.xA4;
            self.cpu_data_script(f, a as u32);
            let cpu = &mut self.game.fighters[f.0].cpu;
            cpu.xA4 = 0;
            cpu.x90 = cpu.x90.wrapping_add(1);
        } else {
            self.cpu_drop_target(f);
        }
    }

    /// What threatens the fighter enough to shield or dodge, `ftCo_800BB9B4` (`xF8_b12`): an
    /// enemy's live hitbox about to reach its middle (1, the enemy in `xF0`), an enemy's move
    /// that comes down on it (1), or an item's hitbox on its way (2, the item in `xF4`); none
    /// (0) when it can't be targeted, or for Giga Bowser.
    #[doc(alias = "ftCo_800BB9B4")]
    pub(super) fn cpu_shield_threat(&mut self, f: FighterId) -> u8 {
        let c = &mut self.game.fighters[f.0].cpu;
        c.xF8_b12 = 0;
        c.xF0 = None;
        c.xF4 = None;
        if self.cpu_untargetable(f) || self.game.fighters[f.0].kind == KIND_GKOOPS {
            return 0;
        }
        let fpp = &self.game.fighters[f.0];
        let radius = mem(fp::fmuls(0.5, reg(fpp.cpu.x568)));
        let middle = Vec3::new(
            fpp.cur_pos.x,
            mem(fp::fadds(reg(fpp.cur_pos.y), reg(radius))),
            fpp.cur_pos.z,
        );
        let me = fpp.gobj;
        let mut cur = self.game.world.first_in(super::cpu_ai::P_LINK_FIGHTER);
        while let Some(g) = cur {
            cur = self.game.world.gobjs[g.0].next;
            if me == Some(g) {
                continue;
            }
            let o = self.game.fighter_id(g);
            if self.cpu_is_ally(f, o) {
                continue;
            }
            let threat = u8::from(self.cpu_hitbox_coming(f, o, middle, radius));
            self.game.fighters[f.0].cpu.xF8_b12 = threat;
            if threat != 0 {
                self.game.fighters[f.0].cpu.xF0 = Some(o);
                return threat;
            }
            let threat = u8::from(self.cpu_move_coming(f, o));
            self.game.fighters[f.0].cpu.xF8_b12 = threat;
            if threat != 0 {
                self.game.fighters[f.0].cpu.xF0 = Some(o);
                return threat;
            }
        }
        let mut cur = self.game.world.first_in(P_LINK_ITEM);
        while let Some(g) = cur {
            cur = self.game.world.gobjs[g.0].next;
            let it = super::super::it::item_of(self.game, g);
            // `it_8026C1B4`: a hitbox on.
            let live = self.game.items[it.0]
                .x5D4_hitboxes
                .iter()
                .take(4)
                .any(|h| h.hit.state != hitcapsule::DISABLED);
            if !live {
                continue;
            }
            let threat = self.cpu_item_coming(f, it, middle, radius);
            self.game.fighters[f.0].cpu.xF8_b12 = threat;
            if threat != 0 {
                self.game.fighters[f.0].cpu.xF4 = Some(it);
                return threat;
            }
        }
        self.game.fighters[f.0].cpu.xF8_b12
    }

    /// Whether one of an enemy's hitboxes, live and not a grab's, that hasn't hit the
    /// fighter yet, sweeps within `radius` of `middle` over its next three frames'
    /// movement, `ftCo_800BB104`.
    #[doc(alias = "ftCo_800BB104")]
    fn cpu_hitbox_coming(&self, f: FighterId, o: FighterId, middle: Vec3, radius: f32) -> bool {
        self.game.fighters[o.0].x914.iter().any(|h| {
            if h.state == hitcapsule::DISABLED
                || h.state == hitcapsule::ENABLED
                || h.x43_b2
                || h.element == ELEMENT_CATCH
                || hitcapsule::has_victim(h, hitcapsule::Victim::Fighter(f))
            {
                return false;
            }
            let ahead = |now: f32, before: f32| {
                mem(fp::fmadds(3.0, fp::fsubs(reg(now), reg(before)), reg(now)))
            };
            let end = Vec3::new(
                ahead(h.x4C.x, h.x58.x),
                ahead(h.x4C.y, h.x58.y),
                ahead(h.x4C.z, h.x58.z),
            );
            collision::capsules(h.x4C, end, middle, middle, h.scale, radius)
                .is_some_and(|c| c.touching)
        })
    }

    /// Whether an enemy's move comes down on the fighter, `ftCo_800BB768`: Bowser's (and Giga
    /// Bowser's) Bowser Bomb or up smash, Yoshi's Yoshi Bomb, from above within both their
    /// half sizes across (the bombs with no floor between), or Samus's Screw Attack from
    /// below within 30.
    #[doc(alias = "ftCo_800BB768")]
    fn cpu_move_coming(&mut self, f: FighterId, o: FighterId) -> bool {
        let (me, other) = (&self.game.fighters[f.0], &self.game.fighters[o.0]);
        let (mp, op, ms) = (me.cur_pos, other.cur_pos, other.motion_id);
        let within = |g: &Self| {
            let (x, w) = (reg(op.x), reg(g.game.fighters[o.0].cpu.x564));
            let mine = reg(g.game.fighters[f.0].cpu.x564);
            fp::fadds(mine, fp::fadds(x, w)) > reg(mp.x)
                && fp::fsubs(fp::fsubs(x, w), mine) < reg(mp.x)
        };
        let bomb = |g: &mut Self| {
            let above = mem(fp::frsp(fp::fadd(5.0, reg(mp.y))));
            g.cpu_floor(op.x, op.y, mp.x, above).is_none() && within(g)
        };
        match other.kind {
            KIND_GKOOPS | KIND_KOOPA => {
                use super::koopa::ms as kp;
                if ms == kp::SPECIAL_LW || ms == kp::SPECIAL_AIR_LW {
                    bomb(self)
                } else if ms == super::co::ids::ms::ATTACK_HI4 {
                    within(self) && op.y < mp.y && fp::fsubs(reg(mp.y), reg(op.y)) < 35.0
                } else {
                    false
                }
            }
            KIND_YOSHI => {
                use super::yoshi::ms as ys;
                (ms == ys::SPECIAL_LW || ms == ys::SPECIAL_AIR_LW) && bomb(self)
            }
            KIND_SAMUS => {
                ms == super::samus::ms::SPECIAL_HI && op.y < mp.y && self.cpu_distance(f, o) < 30.0
            }
            _ => false,
        }
    }

    /// Whether an item's hitbox, not the fighter's own or an ally's item's, nor a star's or
    /// an item `ftCo_800A5980` passes, will reach the fighter's middle as both move on, over a
    /// span that shortens with its level (a random 10 to 29 frames below level 3, down to 1 at
    /// level 9), `ftCo_800BB220`: 2 when it does. Ness above level 3 tells an absorbable
    /// hitbox (3) only in spans of 21 or more, which his level never draws, so his test is
    /// everyone's.
    #[doc(alias = "ftCo_800BB220")]
    fn cpu_item_coming(&mut self, f: FighterId, it: ItemId, middle: Vec3, radius: f32) -> u8 {
        let ip = &self.game.items[it.0];
        if ip.owner == self.game.fighters[f.0].gobj {
            return 0;
        }
        if super::super::it::item::is_fighter(self.game, ip.owner) {
            let owner = self.game.fighter_id(ip.owner.expect("is_fighter"));
            if self.cpu_is_ally(f, owner) {
                return 0;
            }
        }
        // `It_Kind_Star`, `It_Kind_Unk4`, and `ftCo_800A5980` on the item read as a fighter.
        let kind = self.game.items[it.0].kind;
        if kind == 10 || kind == 158 || (43..47).contains(&kind) || matches!(kind, 211..=213 | 217)
        {
            return 0;
        }
        let level = self.game.fighters[f.0].cpu.level;
        let span = |g: &mut Self, scale: f64, base: i32| {
            fp::fctiwz(fp::fmuls(scale, reg(g.game.objs.rng.draw_f32()))).wrapping_add(base)
        };
        let count = if level < 3 {
            span(self, 20.0, 10)
        } else if level < 6 {
            span(self, 10.0, 5)
        } else if level < 9 {
            span(self, 5.0, 3)
        } else {
            1
        };
        let fpp = &self.game.fighters[f.0];
        if fpp.kind == KIND_NESS && level > 3 && count >= 21 {
            unported_call("ftCo_800BB220 for Ness spanning 21 frames or more");
        }
        let n = fp::frsp(f64::from(count));
        let along = |v: f32, base: f32| mem(fp::fmadds(reg(v), n, reg(base)));
        let dst = Vec3::new(
            along(fpp.pos_delta.x, middle.x),
            along(fpp.pos_delta.y, middle.y),
            along(fpp.pos_delta.z, middle.z),
        );
        let hit = self.game.items[it.0].x5D4_hitboxes.iter().take(4).any(|b| {
            let h = &b.hit;
            if h.state == hitcapsule::DISABLED
                || h.state == hitcapsule::ENABLED
                || h.x43_b2
                || h.element == ELEMENT_INERT
                || hitcapsule::has_victim(h, hitcapsule::Victim::Fighter(f))
            {
                return false;
            }
            // `ftCo_CpuPredictHitboxPosition`.
            let ahead = |now: f32, before: f32| {
                mem(fp::fmadds(fp::fsubs(reg(now), reg(before)), n, reg(now)))
            };
            let end = Vec3::new(
                ahead(h.x4C.x, h.x58.x),
                ahead(h.x4C.y, h.x58.y),
                ahead(h.x4C.z, h.x58.z),
            );
            collision::capsules(h.x4C, end, middle, dst, h.scale, radius)
                .is_some_and(|c| c.touching)
        });
        if hit { 2 } else { 0 }
    }

    /// Shielding or dodging, `ftCo_800BA9A0` (behavior 7): mid-dodge it waits; raising its
    /// shield it holds it ([`Running::cpu_hold_shield`]); with no threat it lets go and goes
    /// back to its usual behavior. In the air, an air dodge (Fox and Falco fire a blaster at
    /// an item); on the ground, from an item a roll away from it or a shield (Fox and Falco
    /// fire half the time), from an enemy a roll by how near it is.
    #[doc(alias = "ftCo_800BA9A0")]
    pub(super) fn cpu_shield(&mut self, f: FighterId) {
        use super::co::ids::ms;
        use super::cpu_script::cmd as c;
        let fpp = &self.game.fighters[f.0];
        let m = fpp.motion_id;
        if matches!(
            m,
            ms::ESCAPE_F | ms::ESCAPE_B | ms::ESCAPE_AIR | ms::ESCAPE_N
        ) {
            self.cpu_cmd(f, c::DONE);
            return;
        }
        if m == ms::GUARD_ON {
            self.cpu_hold_shield(f);
            return;
        }
        let threat = fpp.cpu.xF8_b12;
        if threat == 0 {
            let cpu = &mut self.game.fighters[f.0].cpu;
            cpu.x18 = cpu.x1C;
            self.cpu_cmd1(f, c::SET_LSTICK_X, 0);
            self.cpu_cmd1(f, c::SET_LSTICK_Y, 0);
            self.cpu_cmd(f, c::RELEASE_R);
            self.cpu_cmd(f, c::RELEASE_B);
            self.cpu_cmd(f, c::DONE);
            return;
        }
        // A threat of 3 (an absorbable hitbox to Ness) would run `ftCo_800BA080`, but
        // [`Running::cpu_item_coming`] never finds one.
        let blaster = matches!(fpp.kind, 1 | 22);
        if fpp.ground_or_air == GA_AIR {
            if blaster && threat == 2 {
                self.cpu_fire_blaster(f);
            } else {
                self.cpu_retap_r(f);
            }
        } else if threat == 2 {
            let item = fpp.cpu.xF4.expect("an item threat without its item");
            use super::super::it::kinds as k;
            match self.game.items[item.0].kind {
                k::FLIPPER | k::TARU_CANN | k::DKINOKO => {
                    unported_call("ftCo_800BA9A0 for a flipper, barrel cannon or mushroom")
                }
                _ => {
                    if blaster && reg(self.game.objs.rng.draw_f32()) > 0.5 {
                        self.cpu_fire_blaster(f);
                    } else {
                        self.cpu_hold_shield(f);
                    }
                }
            }
        } else {
            match fpp.cpu.xF0 {
                None => self.cpu_hold_shield(f),
                Some(o) => {
                    // `inline2`: away from a far enemy, through a near one.
                    let d = fp::fsubs(
                        reg(self.game.fighters[f.0].cur_pos.x),
                        reg(self.game.fighters[o.0].cur_pos.x),
                    );
                    let far = abs(d) > 30.0;
                    if (d > 0.0) == far {
                        self.cpu_roll_right(f);
                    } else {
                        self.cpu_roll_left(f);
                    }
                }
            }
        }
    }

    /// Holds the shield, `ftCo_800B9F90`: held already, for a while by level (none at 9);
    /// else a press.
    #[doc(alias = "ftCo_800B9F90")]
    fn cpu_hold_shield(&mut self, f: FighterId) {
        use super::cpu_script::cmd as c;
        let fpp = &self.game.fighters[f.0];
        let held = fpp.motion_id == super::co::ids::ms::GUARD;
        let wait = 9 - fpp.cpu.level;
        self.cpu_cmd1(f, c::SET_LSTICK_X, 0);
        self.cpu_cmd1(f, c::SET_LSTICK_Y, 0);
        self.cpu_cmd(f, c::PRESS_R);
        if held && wait != 0 {
            let r = reg(self.game.objs.rng.draw_f32());
            let n = fp::fctiwz(fp::fmuls(4.0, r)).wrapping_add(4);
            self.cpu_cmd1(f, c::WAIT_FOR, wait.wrapping_mul(n) as u8);
        }
        self.cpu_cmd(f, c::DONE);
    }

    /// A roll to the right, `ftCo_800BA160`.
    #[doc(alias = "ftCo_800BA160")]
    fn cpu_roll_right(&mut self, f: FighterId) {
        self.cpu_roll(f, 0x7F);
    }

    /// A roll to the left, `ftCo_800BA224`.
    #[doc(alias = "ftCo_800BA224")]
    fn cpu_roll_left(&mut self, f: FighterId) {
        self.cpu_roll(f, 0x81);
    }

    /// A shield's roll with the stick at `x`: R let go, then pressed for 2 frames with the
    /// stick held sideways for 7.
    fn cpu_roll(&mut self, f: FighterId, x: u8) {
        use super::cpu_script::cmd as c;
        self.cpu_cmd1(f, c::SET_LSTICK_X, 0);
        self.cpu_cmd1(f, c::SET_LSTICK_Y, 0);
        self.cpu_cmd(f, c::RELEASE_R);
        self.cpu_cmd1(f, c::WAIT_FOR, 1);
        self.cpu_cmd1(f, c::SET_LSTICK_X, x);
        self.cpu_cmd(f, c::PRESS_R);
        self.cpu_cmd1(f, c::WAIT_FOR, 2);
        self.cpu_cmd(f, c::RELEASE_R);
        self.cpu_cmd1(f, c::WAIT_FOR, 5);
        self.cpu_cmd1(f, c::SET_LSTICK_X, 0);
        self.cpu_cmd(f, c::DONE);
    }

    /// `ftCo_CpuFireBlaster`: a down special's reflector (stick down, B), held a while by level
    /// (none at 9).
    fn cpu_fire_blaster(&mut self, f: FighterId) {
        use super::cpu_script::cmd as c;
        let wait = 9 - self.game.fighters[f.0].cpu.level;
        self.cpu_cmd1(f, c::SET_LSTICK_X, 0);
        self.cpu_cmd1(f, c::SET_LSTICK_Y, (-0x50i8) as u8);
        self.cpu_cmd(f, c::PRESS_B);
        self.cpu_cmd1(f, c::WAIT_FOR, 1);
        self.cpu_cmd1(f, c::SET_LSTICK_Y, 0);
        if wait != 0 {
            let r = reg(self.game.objs.rng.draw_f32());
            let n = fp::fctiwz(fp::fmuls(4.0, r)).wrapping_add(4);
            self.cpu_cmd1(f, c::WAIT_FOR, wait.wrapping_mul(n) as u8);
        }
        self.cpu_cmd(f, c::DONE);
    }

    /// Whether the fighter jumps at its target, `ftCo_800B732C`: 1 at a target on a ledge
    /// steeply above it and in clear view; else, if it may (`xF9_b3`), by the character's
    /// rules. On the ground only.
    #[doc(alias = "ftCo_800B732C")]
    pub(super) fn cpu_jump_attack(&mut self, f: FighterId) -> i32 {
        let fpp = &self.game.fighters[f.0];
        let Some(t) = fpp.cpu.x44 else { return 0 };
        if fpp.ground_or_air == GA_AIR {
            return 0;
        }
        if self.cpu_on_ledge(t) != 0 {
            let (me, tp) = (fpp.cur_pos, self.game.fighters[t.0].cur_pos);
            if tp.y < me.y {
                return 0;
            }
            let geo = self.game.geometry.as_mut().expect("no collision geometry");
            if geo.check_ceiling(me.x, me.y, tp.x, tp.y, -1, -1).is_some() {
                return 0;
            }
            let _ = self.cpu_distance(f, t);
            let dy = fp::fsubs(reg(tp.y), reg(me.y));
            let dx = fp::fsubs(reg(tp.x), reg(me.x));
            let dx = if dx < 0.0 { fp::fneg(dx) } else { dx };
            return i32::from(super::co::guard::stick_angle(dy, dx) > 1.308_996_915_817_260_7);
        }
        if !self.game.fighters[f.0].cpu.xF9_b3 {
            return 0;
        }
        if self.game.fighters[f.0].cpu.kind == 0x10 {
            unported_call("ftCo_800B7638");
        }
        if !self.cpu_facing_destination(f) {
            return 0;
        }
        if self.game.fighters[f.0].cpu.level < 4 {
            return 0;
        }
        if self.cpu_near_edge(f, 4.0) {
            return 0;
        }
        let (me, tp) = (
            self.game.fighters[f.0].cur_pos,
            self.game.fighters[t.0].cur_pos,
        );
        let angle = || {
            let dy = fp::fsubs(reg(tp.y), reg(me.y));
            let dx = abs(fp::fsubs(reg(tp.x), reg(me.x)));
            super::co::guard::stick_angle(dy, dx)
        };
        if self.game.fighters[t.0].ground_or_air == GA_AIR {
            if tp.y < me.y {
                return 0;
            }
            let geo = self.game.geometry.as_mut().expect("no collision geometry");
            if geo.check_ceiling(me.x, me.y, tp.x, tp.y, -1, -1).is_some() {
                return 0;
            }
            let d = self.cpu_distance(f, t);
            return i32::from(angle() > 0.785_398_157_313_466_1 && d < 30.0);
        }
        if self.game.fighters[f.0].cpu.x7C % 60 != 0 {
            return 0;
        }
        if reg(self.game.objs.rng.draw_f32()) > 0.1 {
            return 0;
        }
        let d = self.cpu_distance(f, t);
        let a = angle();
        if d > 30.0 && d < 40.0 && a > -0.087_266_461_923_718_45 && a < 0.087_266_461_923_718_45 {
            2
        } else {
            0
        }
    }

    /// Holding a grabbed fighter, `ftCo_800B683C` (behavior 9): with no victim it drops
    /// the target. Donkey Kong carrying one walks to the nearer end of its island and throws
    /// there; Kirby with a swallowed Kirby spits, else with a swallowed fighter crouches;
    /// Popo, Fox and Captain Falcon above level 4 and Mario below 5 sometimes run their own
    /// throw scripts first. Holding, it pummels (Bowser's koopa klaw bites), more rarely
    /// each time, else near an end throws off it, else throws a random way.
    // Ports `ftCo_800B683C`; aliased once a check reaches it.
    pub(super) fn cpu_hold(&mut self, f: FighterId) {
        use super::cpu_script::cmd as c;
        let Some(victim) = self.game.fighters[f.0].victim_gobj else {
            self.cpu_drop_target(f);
            return;
        };
        let fpp = &self.game.fighters[f.0];
        let (kind, ms) = (fpp.kind, fpp.motion_id);
        let mut klaw = false;
        let chance = |g: &mut Self, f: FighterId| -> bool {
            let level = g.game.fighters[f.0].cpu.level;
            reg(g.game.objs.rng.draw_f32()) < fp::fmuls(reg(0.05), fp::frsp(f64::from(level)))
        };
        match kind {
            // Ft_Kind_Donkey's cargo states.
            3 if (0x15F..=0x164).contains(&ms) => {
                let pos = self.game.fighters[f.0].cur_pos;
                let geo = self.game.geometry.as_mut().expect("no collision geometry");
                let Some(island) = geo.island_below(pos, -10.0) else {
                    self.cpu_cmd(f, c::DONE);
                    return;
                };
                let (mut left, mut right) = (Vec3::default(), Vec3::default());
                geo.island_ends(island, Some(&mut left), Some(&mut right));
                let x = reg(pos.x);
                let to_right = abs(fp::fsubs(x, reg(right.x)));
                let to_left = abs(fp::fsubs(x, reg(left.x)));
                let (dist, dir) = if to_right < to_left {
                    (to_right, 0x7F)
                } else {
                    (to_left, 0x81)
                };
                if dist > 10.0 {
                    self.cpu_cmd1(f, c::SET_LSTICK_Y, 0);
                    self.cpu_cmd1(f, c::SET_LSTICK_X, dir);
                    self.cpu_cmd(f, c::DONE);
                } else {
                    // `ftCo_CpuPressAWithLstickX`.
                    self.cpu_cmd1(f, c::SET_LSTICK_Y, 0);
                    self.cpu_cmd1(f, c::SET_LSTICK_X, 0);
                    self.cpu_cmd1(f, c::WAIT_FOR, 1);
                    self.cpu_cmd1(f, c::SET_LSTICK_Y, 0);
                    self.cpu_cmd1(f, c::SET_LSTICK_X, dir);
                    self.cpu_cmd(f, c::PRESS_A);
                    self.cpu_cmd1(f, c::WAIT_FOR, 1);
                    self.cpu_cmd1(f, c::SET_LSTICK_X, 0);
                    self.cpu_cmd1(f, c::SET_LSTICK_Y, 0);
                    self.cpu_cmd(f, c::RELEASE_A);
                    self.cpu_cmd(f, c::DONE);
                }
                return;
            }
            // Ft_Kind_Koopa, Ft_Kind_GKoops in the koopa klaw.
            5 | 31 if ms == 0x15E => klaw = true,
            // Ft_Kind_Kirby with a swallowed fighter.
            4 if (0x164..=0x17E).contains(&ms) => {
                if self.game.fighter(victim).kind == 4 {
                    self.cpu_cmd1(f, c::SET_LSTICK_Y, 0);
                    self.cpu_cmd1(f, c::SET_LSTICK_X, 0);
                    self.cpu_cmd1(f, c::WAIT_FOR, 1);
                    self.cpu_cmd(f, c::PRESS_A);
                    self.cpu_cmd1(f, c::WAIT_FOR, 1);
                    self.cpu_cmd(f, c::RELEASE_A);
                    self.cpu_cmd(f, c::DONE);
                } else {
                    self.cpu_neutral_stick(f);
                    self.cpu_cmd1(f, c::WAIT_FOR, 1);
                    self.cpu_cmd1(f, c::SET_LSTICK_Y, 0x81);
                    self.cpu_cmd1(f, c::WAIT_FOR, 1);
                    self.cpu_cmd1(f, c::SET_LSTICK_Y, 0);
                    self.cpu_cmd(f, c::DONE);
                }
                return;
            }
            // Ft_Kind_Popo, Ft_Kind_Fox, Ft_Kind_Captain above level 4.
            10 | 1 | 2 => {
                let cpu = &self.game.fighters[f.0].cpu;
                if cpu.x94 == 0 && cpu.level > 4 && chance(self, f) {
                    let script = match kind {
                        10 => 0x3B,
                        1 => 0x3D,
                        _ if reg(self.game.fighter(victim).dmg.x1830_percent) < 100.0 => 0x39,
                        _ => 0x3A,
                    };
                    self.cpu_data_script(f, script);
                    return;
                }
            }
            // Ft_Kind_Mario below level 5.
            0 => {
                let cpu = &self.game.fighters[f.0].cpu;
                if cpu.x94 == 0 && cpu.level < 5 && chance(self, f) {
                    self.cpu_data_script(f, 0x35);
                    return;
                }
            }
            _ => {}
        }
        if !self.cpu_is_grabbing(f) {
            // `ftCo_800A0C8C`.
            self.cpu_cmd(f, c::DONE);
            return;
        }
        let r = reg(self.game.objs.rng.draw_f32());
        let x94 = self.game.fighters[f.0].cpu.x94;
        if fp::frsp(fp::fmul(r, fp::fadd(1.0, f64::from(x94)))) < 0.5 {
            self.cpu_data_script(f, if klaw { 0x11 } else { 2 });
            let cpu = &mut self.game.fighters[f.0].cpu;
            cpu.x94 = cpu.x94.wrapping_add(1);
            return;
        }
        let pos = self.game.fighters[f.0].cur_pos;
        let geo = self.game.geometry.as_mut().expect("no collision geometry");
        if let Some(island) = geo.island_below(pos, -10.0) {
            let (mut left, mut right) = (Vec3::default(), Vec3::default());
            geo.island_ends(island, Some(&mut left), Some(&mut right));
            let x = reg(pos.x);
            let to_right = abs(fp::fsubs(x, reg(right.x)));
            let to_left = abs(fp::fsubs(x, reg(left.x)));
            let flick = if to_right > to_left {
                (to_left < 30.0).then_some((0xB0, 0x81))
            } else {
                (to_right < 30.0).then_some((0x50, 0x7F))
            };
            if let Some((half, full)) = flick {
                // `ftCo_CpuFlickLstickX`.
                self.cpu_neutral_stick(f);
                self.cpu_cmd1(f, c::WAIT_FOR, 1);
                self.cpu_cmd1(f, c::SET_LSTICK_X, half);
                self.cpu_cmd1(f, c::WAIT_FOR, 1);
                self.cpu_cmd1(f, c::SET_LSTICK_X, full);
                self.cpu_cmd1(f, c::WAIT_FOR, 1);
                self.cpu_cmd1(f, c::SET_LSTICK_X, 0);
                self.cpu_cmd(f, c::DONE);
                return;
            }
        }
        let r = reg(self.game.objs.rng.draw_f32());
        self.cpu_neutral_stick(f);
        self.cpu_cmd1(f, c::WAIT_FOR, 1);
        let (which, half, full) = if r < 0.25 {
            (c::SET_LSTICK_Y, 0x50, 0x7F)
        } else if r < 0.5 {
            (c::SET_LSTICK_Y, 0xB0, 0x81)
        } else if r < 0.75 {
            (c::LSTICK_X_FORWARD, 0x50, 0x7F)
        } else {
            (c::LSTICK_X_FORWARD, 0xB0, 0x81)
        };
        self.cpu_cmd1(f, which, half);
        self.cpu_cmd1(f, c::WAIT_FOR, 1);
        self.cpu_cmd1(f, which, full);
        self.cpu_cmd1(f, c::WAIT_FOR, 1);
        self.cpu_neutral_stick(f);
        self.cpu_cmd(f, c::DONE);
    }
}
