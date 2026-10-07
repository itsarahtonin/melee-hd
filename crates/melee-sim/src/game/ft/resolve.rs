//! What the frame's collisions come to for a fighter (`Fighter_procCollResolve`): its shield
//! regrows or wears down, a delayed tip lands, the strongest result of the attack collision
//! (a hit's knockback, a tip, a shield hit, a clank, a hit it dealt) picks its reaction and
//! hitlag, and the frame's damage bookkeeping starts over. The damage states a hit enters
//! (`ftCo_8008EC90` and its neighbours) and a broken shield aren't ported: those calls stop
//! there.

// NaN takes the original's branches, which the negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::code::{self, Val, run_fighter_cb, unported_call};
use super::super::handles::FighterId;
use super::super::state::Running;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::lb::joint::world_position;

/// `lb_80014574`: queues a rumble on a controller, the rumble's seam.
pub const LB_80014574: u32 = 0x8001_4574;

/// `ftCo_MS_Squat` and the state after it (crouching), which shortens hitlag.
const MS_SQUAT: u32 = 39;

/// `Fighter_procCollResolve`.
#[doc(alias = "Fighter_procCollResolve")]
pub fn proc_coll_resolve(g: &mut Running, gobj: GObjId) {
    let f = g.game.fighter_id(gobj);
    if g.game.fighters[f.0].is_sleeping {
        return;
    }
    let msid = g.game.fighters[f.0].motion_id;
    let c = g.common();
    let (full, regen, broken) = (c.f32(0x260), c.f32(0x27C), c.f32(0x280));
    let (wear_mul, wear_add, light_lo, light_hi) =
        (c.f32(0x284), c.f32(0x288), c.f32(0x2DC), c.f32(0x2E0));
    let fp = &mut g.game.fighters[f.0];
    if !fp.x221A_b7 && !(reg(fp.shield_health) >= reg(full)) {
        fp.shield_health = mem(fp::fadds(reg(fp.shield_health), reg(regen)));
        if !(reg(fp.shield_health) <= reg(full)) {
            fp.shield_health = full;
        }
    }
    if fp.x221A_b7 {
        let light = fp::fmadds(
            reg(fp.lightshield_amount),
            fp::fsubs(reg(light_hi), reg(light_lo)),
            reg(light_lo),
        );
        let taken = fp::frsp(f64::from(fp.x19A0_shieldDamageTaken));
        let wear = fp::fmadds(
            reg(wear_mul),
            fp::fmuls(taken, fp::fsubs(1.0, light)),
            reg(wear_add),
        );
        fp.shield_health = mem(fp::fsubs(reg(fp.shield_health), wear));
        if !(reg(fp.shield_health) >= 0.0) {
            fp.shield_health = broken;
            unported_call("a shield broken by its damage (pl_8003E058, ftCo_80098B20)");
        }
    }
    // A tip's damage, held back while its hitlag lasted, lands.
    let fp = &mut g.game.fighters[f.0];
    if !(reg(fp.dmg.x189C_unk_num_frames) <= 0.0) {
        fp.dmg.x189C_unk_num_frames = mem(fp::fsubs(reg(fp.dmg.x189C_unk_num_frames), 1.0));
        if reg(fp.dmg.x189C_unk_num_frames) <= 0.0 && reg(fp.dmg.kb_applied) == 0.0 {
            fp.dmg.x189C_unk_num_frames = 0.0;
            g.land_tip(gobj);
        }
    }
    let fp = &g.game.fighters[f.0];
    let kb = fp.dmg.kb_applied;
    let mut strength = 0;
    let mut tip = false;
    if reg(kb) != 0.0 {
        unported_call("a hit's knockback (ftCo_Damage_CalcKnockback and the damage states)");
    } else if reg(fp.dmg.x18a0) != 0.0 {
        strength = fp.dmg.x1840;
        tip = true;
    } else if fp.x19A4 != 0 {
        if let Some(cb) = fp.shield_hit_cb {
            run_fighter_cb(g, gobj, cb);
        }
        strength = g.game.fighters[f.0].x19A4;
    } else if fp.dmg.int_value != 0 {
        if reg(fp.dmg.x191C) != 0.0 && fp.victim_gobj.is_none() && fp.target_item_gobj.is_none() {
            unported_call("a clank's recoil (ftCo_80099D9C)");
        }
        strength = fp.dmg.int_value;
    } else if fp.dmg.x1914 != 0 {
        if let Some(cb) = fp.deal_dmg_cb {
            run_fighter_cb(g, gobj, cb);
        }
        let fp = &g.game.fighters[f.0];
        strength = fp.dmg.x1914;
        if fp.x2070[3] == 0x46 {
            g.rumble(f, 0xE, 0);
        } else {
            g.hit_rumble(f, strength);
        }
    } else if fp.dmg.x1924 != 0 {
        strength = fp.dmg.x1924;
    } else if fp.ReflectAttr.x1A3C_damageOver != 0 {
        unported_call("ftCo_80098C9C (a reflector overpowered)");
    } else if reg(fp.ReflectAttr.x1A2C_reflectHitDirection) != 0.0 {
        if let Some(cb) = fp.reflect_hit_cb {
            run_fighter_cb(g, gobj, cb);
        }
    } else if reg(fp.AbsorbAttr.x1A40_absorbHitDirection) != 0.0 {
        unported_call("ftData_OnAbsorb (an absorbed item)");
    } else if fp.unk_gobj.is_some()
        && let Some(cb) = fp.hurtbox_detect_cb
    {
        run_fighter_cb(g, gobj, cb);
    }
    if reg(kb) == 0.0 && reg(g.game.fighters[f.0].dmg.x1838_percentTemp) != 0.0 {
        unported_call("damage without knockback (Fighter_TakeDamage_8006CC7C)");
    }
    g.stamina_out(gobj);
    if strength != 0 {
        let mul = g.game.fighters[f.0].x1960_vibrateMult;
        let lag = g.hitlag(strength, msid, mul);
        let fp = &mut g.game.fighters[f.0];
        fp.dmg.x195c_hitlag_frames = mem(lag);
        if !(reg(fp.dmg.x195c_hitlag_frames) >= reg(fp.x1964)) {
            fp.dmg.x195c_hitlag_frames = fp.x1964;
        }
        let lag = reg(fp.dmg.x195c_hitlag_frames);
        if !(lag <= 0.0) {
            let max = g.common().f32(0x194);
            let fp = &mut g.game.fighters[f.0];
            if !(lag <= reg(max)) {
                fp.dmg.x195c_hitlag_frames = max;
            }
            fp.allow_sdi = true;
            if tip {
                fp.dmg.x189C_unk_num_frames = fp.dmg.x195c_hitlag_frames;
            }
            if !fp.x2219_b5 {
                g.start_hitlag(gobj);
            }
        }
    } else {
        g.queued_sounds(f);
    }
    let fp = &g.game.fighters[f.0];
    if fp.x221A_b0 || reg(fp.dmg.x1958) != 0.0 {
        if !fp.x2219_b5 {
            g.start_hitlag(gobj);
        }
        let fp = &mut g.game.fighters[f.0];
        if fp.x221A_b0 {
            fp.x2219_b7 = true;
            fp.x221A_b0 = false;
        } else {
            if !(reg(fp.dmg.x1958) <= reg(fp.dmg.x1954)) {
                fp.dmg.x1954 = fp.dmg.x1958;
            }
            fp.dmg.x1958 = 0.0;
        }
    }
    let c = g.common();
    let (push_mul, push_add) = (c.f32(0x3E0), c.f32(0x3E4));
    let fp = &mut g.game.fighters[f.0];
    if reg(fp.dmg.x1928) != 0.0 {
        let push = fp::fmadds(reg(fp.dmg.x1928), reg(push_mul), reg(push_add));
        fp.xF4_ground_attacker_shield_kb_vel = mem(if !(reg(fp.dmg.x192c) >= 0.0) {
            push
        } else {
            fp::fneg(push)
        });
        g.shield_push(gobj);
    }
    let fp = &mut g.game.fighters[f.0];
    fp.dmg.x1838_percentTemp = 0.0;
    fp.dmg.x183C_applied = 0;
    fp.x1828 = 0;
    fp.dmg.kb_applied = 0.0;
    fp.dmg.x18a0 = 0.0;
    fp.dmg.x1840 = 0;
    fp.dmg.x1914 = 0;
    fp.dmg.int_value = 0;
    fp.dmg.x191C = 0.0;
    fp.unk_gobj = None;
    fp.x221C_b5 = false;
    fp.dmg.x1924 = 0;
    fp.dmg.x1928 = 0.0;
    fp.x19A0_shieldDamageTaken = 0;
    fp.x19A4 = 0;
    fp.x19A8 = None;
    fp.ReflectAttr.x1A3C_damageOver = 0;
    fp.ReflectAttr.x1A2C_reflectHitDirection = 0.0;
    fp.AbsorbAttr.x1A40_absorbHitDirection = 0.0;
    fp.AbsorbAttr.x1A44_damageTaken = 0;
    fp.AbsorbAttr.x1A48_hitsTaken = 0;
    fp.x1960_vibrateMult = 1.0;
    fp.x1964 = 0.0;
    fp.dmg.x1950 = 0;
    if !fp.x2219_b6 || fp.dmg.x18F4 != 0 {
        g.afterimage(f);
    }
    g.cpu_hurt_bounds(f);
}

impl Running<'_> {
    /// A delayed tip's damage lands, `ftColl_8007BE3C`: its damage counts (armor first), its
    /// attacker records the hit, and its effect spawns.
    #[doc(alias = "ftColl_8007BE3C")]
    fn land_tip(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if !self.take_damage_temp(f) {
            return;
        }
        let fp = &self.game.fighters[f.0];
        let (source, dmg) = (fp.dmg.x1894, fp.dmg.x1898);
        match source.map(|g| self.game.world.gobjs[g.0].classifier) {
            Some(4) => self.record_fighter_hit(source.unwrap(), gobj, reg(dmg)),
            Some(6) => unported_call("ftColl_80078998 (an item's delayed tip)"),
            _ => {}
        }
        let fp = &self.game.fighters[f.0];
        let d = &fp.dmg;
        let effect = super::attack::hit_effect_id(d.x188c as u32);
        let (pos, severity, kb) = (d.x1880, d.x1890 as u32, d.x187c);
        let dmg = crate::ppc::cvt_fp2unsigned(reg(d.x1898));
        self.hit_effect(f, effect, pos, severity, dmg, reg(kb));
    }

    /// Adds the held-back damage (`dmg.x1898`) to the frame's, through any armor (`x1834`),
    /// `ftColl_80076640`; false while the armor holds.
    #[doc(alias = "ftColl_80076640")]
    fn take_damage_temp(&mut self, f: FighterId) -> bool {
        let fp = &mut self.game.fighters[f.0];
        let whole = super::attack::whole_damage(reg(fp.dmg.x1898));
        if fp.x221C_b4 {
            fp.dmg.x1834 = mem(fp::fsubs(reg(fp.dmg.x1834), reg(fp.dmg.x1898)));
            if fp.dmg.x1834 < 0.0 {
                fp.dmg.x1898 = mem(fp::fneg(reg(fp.dmg.x1834)));
                fp.x221C_b4 = false;
            }
        }
        if fp.x221C_b4 {
            return false;
        }
        assert!(!(fp.dmg.x1898 > 500.0), "attack power over 500!!");
        fp.dmg.x1838_percentTemp = mem(fp::fadds(reg(fp.dmg.x1838_percentTemp), reg(fp.dmg.x1898)));
        if whole > fp.dmg.x183C_applied {
            fp.dmg.x183C_applied = whole;
        }
        true
    }

    /// Hitlag frames for a result of `strength`, `ftCommon_CalcHitlag`: scaled by the
    /// electric multiplier, and shorter crouching.
    #[doc(alias = "ftCommon_CalcHitlag")]
    pub fn hitlag(&self, strength: i32, msid: i32, mul: f32) -> f64 {
        let c = self.common();
        let base = fp::fctiwz(fp::fmadds(
            fp::frsp(f64::from(strength)),
            reg(c.f32(0x198)),
            reg(c.f32(0x19C)),
        ));
        let mut lag = fp::frsp(f64::from(fp::fctiwz(fp::fmuls(
            fp::frsp(f64::from(base)),
            reg(mul),
        ))));
        if (msid as u32).wrapping_sub(MS_SQUAT) <= 1 {
            lag = fp::frsp(f64::from(fp::fctiwz(fp::fmuls(lag, reg(c.f32(0x1A0))))));
        }
        lag
    }

    /// Hitlag starts, `Fighter_UnkRecursiveFunc_8006D044`: the state's hook, and the fighter
    /// it holds or is held by freezes too.
    #[doc(alias = "Fighter_UnkRecursiveFunc_8006D044")]
    pub fn start_hitlag(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if let Some(cb) = self.game.fighters[f.0].pre_hitlag_cb {
            run_fighter_cb(self, gobj, cb);
        }
        let fp = &mut self.game.fighters[f.0];
        fp.x2219_b5 = true;
        if let Some(partner) = fp.x1A5C
            && !fp.x2219_b7
        {
            self.game.fighter_mut(partner).x2219_b7 = true;
            self.start_hitlag(partner);
        }
    }

    /// The sounds a hit queued for when hitlag ends, played now there's none,
    /// `ftCo_80090718`.
    #[doc(alias = "ftCo_80090718")]
    fn queued_sounds(&mut self, f: FighterId) {
        let id = self.game.fighters[f.0].dmg.x1908;
        if id != -1 {
            self.fighter_play_sfx(f, id, 0x7F, 0x40);
            self.game.fighters[f.0].dmg.x1908 = -1;
        }
        if self.game.fighters[f.0].dmg.x190C != super::super::handles::RawPtr(0) {
            unported_call("ft_800889F4 from ftCo_80090718 (a queued random voice)");
        }
    }

    /// A stamina fighter out of stamina falls (`ftCo_800C8D00`); not ported.
    #[doc(alias = "ftCo_800C8D00")]
    fn stamina_out(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).x2224_b3 {
            unported_call("ftCo_800C8D00 (a stamina fighter out)");
        }
    }

    /// A shield hit pushes the attacker back along the floor, `ftCommon_8007E2A4`.
    #[doc(alias = "ftCommon_8007E2A4")]
    fn shield_push(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let (n, v) = (
            fp.coll_data.floor.normal,
            reg(fp.xF4_ground_attacker_shield_kb_vel),
        );
        fp.x98_atk_shield_kb.x = mem(fp::fmuls(reg(n.y), v));
        fp.x98_atk_shield_kb.y = mem(fp::fmuls(fp::fneg(reg(n.x)), v));
    }

    /// A sword's or item's afterimage follows it (`ftCo_800C2FD8`): nothing without one; an
    /// afterimage isn't ported.
    #[doc(alias = "ftCo_800C2FD8")]
    fn afterimage(&mut self, f: FighterId) {
        if self.game.fighters[f.0].x2100 != -1 {
            unported_call("ftCo_800C2FD8 (an afterimage)");
        }
    }

    /// The extents of the fighter's hurtboxes around it, for the computer players
    /// (`ftCo_800A0DA4`): behind and ahead (`cpu.x55C`, `x560`), their middle, and the top.
    #[doc(alias = "ftCo_800A0DA4")]
    fn cpu_hurt_bounds(&mut self, f: FighterId) {
        let (mut back, mut front, mut top) = (0.0, 0.0, 0.0);
        let n = usize::from(self.game.fighters[f.0].hurt_capsules_len);
        for i in 0..n {
            self.update_hurtbox(f, i);
            let fp = &self.game.fighters[f.0];
            let h = &fp.hurt_capsules[i].capsule;
            let scale = fp::fmuls(reg(h.scale), reg(fp.x34_scale.y));
            for end in [h.a_pos, h.b_pos] {
                let dx = fp::fsubs(reg(end.x), reg(fp.cur_pos.x));
                let dy = fp::fsubs(reg(end.y), reg(fp.cur_pos.y));
                if back > fp::fsubs(dx, scale) {
                    back = fp::fsubs(dx, scale);
                }
                if front < fp::fadds(dx, scale) {
                    front = fp::fadds(dx, scale);
                }
                if top < fp::fadds(dy, scale) {
                    top = fp::fadds(dy, scale);
                }
            }
        }
        let fp = &mut self.game.fighters[f.0];
        let c = &mut fp.cpu;
        if reg(fp.facing_dir) > 0.0 {
            c.x55C = mem(front);
            c.x560 = mem(fp::fneg(back));
        } else {
            c.x55C = mem(fp::fneg(back));
            c.x560 = mem(front);
        }
        c.x564 = mem(fp::frsp(fp::fmul(0.5, fp::fadds(reg(c.x55C), reg(c.x560)))));
        c.x568 = mem(top);
    }

    /// Brings a hurtbox's ends up to date once a frame, `lbColl_800083C4`.
    #[doc(alias = "lbColl_800083C4")]
    fn update_hurtbox(&mut self, f: FighterId, i: usize) {
        let h = &self.game.fighters[f.0].hurt_capsules[i].capsule;
        if h.skip_update_pos {
            return;
        }
        let (bone, a, b) = (h.bone, h.a_offset, h.b_offset);
        let pa = world_position(&mut self.game.objs, self.data, bone, Some(a));
        let pb = world_position(&mut self.game.objs, self.data, bone, Some(b));
        let h = &mut self.game.fighters[f.0].hurt_capsules[i].capsule;
        h.a_pos = pa;
        h.b_pos = pb;
        h.skip_update_pos = true;
    }

    /// Rumbles for a hit the fighter dealt, by its strength, `ftCommon_8007EE0C`.
    #[doc(alias = "ftCommon_8007EE0C")]
    fn hit_rumble(&mut self, f: FighterId, strength: i32) {
        let c = self.common();
        let t = fp::fmadds(
            fp::frsp(f64::from(strength)),
            reg(c.f32(0xEC)),
            reg(c.f32(0xF0)),
        );
        let frames = fp::fctiwz(t);
        if frames < 1 {
            return;
        }
        self.rumble(f, 10, frames);
    }

    /// Rumbles the fighter's controller when its player has rumble on, `ftCommon_8007EBAC`
    /// (`lb_80014574`, the seam).
    #[doc(alias = "ftCommon_8007EBAC")]
    fn rumble(&mut self, f: FighterId, kind: i32, frames: i32) {
        let fp = &self.game.fighters[f.0];
        let (slot, sub, port) = (
            i32::from(fp.player_idx),
            i32::from(fp.is_sub_fighter),
            i32::from(fp.pad_port),
        );
        if self.game.rumble_enabled(slot, sub) && !fp.is_sleeping && !fp.stamina_dead {
            code::call(
                self,
                LB_80014574,
                &[
                    Val::Int(port),
                    Val::Int(kind.wrapping_add(2)),
                    Val::Int(kind),
                    Val::Int(frames),
                ],
            );
        }
    }
}
