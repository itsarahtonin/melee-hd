//! Computer players' attacks (`ftcpuattack.c`): whether to attack and with what, from
//! per-character tables in `PlCo.dat` (`Fighter_804D64FC`, `ftLoadCommonData::x58`) whose
//! entries say where an attack reaches and how often to pick it, weighed against where the
//! target will be.

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::code::unported_call;
use super::super::handles::FighterId;
use super::super::state::Running;
use super::common::GA_AIR;
use crate::fpu::{mem, reg};
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
    pub const SHIELD: u32 = 0x10;
    pub const EDGE_GUARD: u32 = 0x1C;
}

/// `ftCo_AttackEntry`'s size.
const ENTRY_SIZE: u32 = 0x24;

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

    /// Whether a fighter is raising or holding its shield, `ftCo_800B9F6C`.
    #[doc(alias = "ftCo_800B9F6C")]
    fn cpu_shielding(&self, f: FighterId) -> bool {
        // `ftCo_MS_GuardOn`, `ftCo_MS_Guard`.
        matches!(self.game.fighters[f.0].motion_id, 178 | 179)
    }

    /// Whether the fighter uses a ranged attack (a projectile), `ftCo_800B9CBC`: only with a
    /// target, if it may (`xF9_b5`), once its delay (`x34`) runs out.
    // Ports `ftCo_800B9CBC`; aliased once a check reaches it.
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
        unported_call("ftCo_800B9CBC past its delay");
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
    /// action or untargetable fighter rules out every attack; the characters with ranged
    /// attacks check their reach. Only the characters without are ported.
    // Ports `ftCo_800B77E8`; aliased once a check reaches it.
    fn cpu_character_rules(&mut self, f: FighterId) {
        if self.game.fighters[f.0].cpu.kind == 7 || self.cpu_untargetable(f) {
            self.cpu_rule_out(f, cmd::ANY_ATTACK);
        }
        match self.game.fighters[f.0].kind {
            0 | 1 | 22 | 2 | 25 | 12 | 23 | 5 | 31 | 14 | 17 | 9 | 4 => {
                unported_call("ftCo_800B77E8 for a character with ranged attacks")
            }
            _ => {}
        }
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
            if self.game.fighters[f.0].item_gobj.is_some() {
                unported_call("ftCo_800B8A9C with an item held (ftCo_800A59E4)");
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

    /// Whether the fighter is mid-attack, `ftCo_800B630C`: in a common attack or an item's
    /// swing or shot (`ftCo_800A5A90`), or in one of its character's own states.
    // Ports `ftCo_800B630C`; aliased once a check reaches it.
    // Ports `ftCo_800A5A90`; aliased once a check reaches it.
    fn cpu_attacking(&self, f: FighterId) -> bool {
        let fpp = &self.game.fighters[f.0];
        let ms = fpp.motion_id;
        // `ftCo_MS_Attack11` to `ftCo_MS_AttackAirLw`; `ftCo_MS_SwordSwing1` to
        // `ftCo_MS_StarRodSwingDash`, `ftCo_MS_LGunShoot` to `ftCo_MS_ItemScopeAirEndEmpty`.
        if (44..=69).contains(&ms) || (120..=139).contains(&ms) || (148..=173).contains(&ms) {
            return true;
        }
        // Ft_Kind_Donkey, Ft_Kind_Kirby, Ft_Kind_Peach have states past the common ones
        // that aren't attacks.
        if matches!(fpp.kind, 3 | 4 | 9) {
            unported_call("ftCo_800B630C for Donkey Kong, Kirby or Peach");
        }
        ms >= 341
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
    // Ports `ftCo_800B658C`; aliased once a check reaches it.
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
        // Ness, Yoshi, Samus, Donkey Kong and Zelda have follow-ups.
        if matches!(self.game.fighters[f.0].kind, 8 | 14 | 13 | 3 | 19) {
            unported_call("ftCo_800B658C for a character with follow-ups");
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

    /// Whether the fighter jumps at its target, `ftCo_800B732C`: 1 at a target on a ledge
    /// steeply above it and in clear view; else, if it may (`xF9_b3`), by the character's
    /// rules. On the ground only.
    // Ports `ftCo_800B732C`; aliased once a check reaches it.
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
        unported_call("ftCo_800B732C past xF9_b3");
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
