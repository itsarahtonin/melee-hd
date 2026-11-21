//! What the frame's collisions come to for a fighter (`Fighter_procCollResolve`): its shield
//! regrows or wears down, a delayed tip lands, the strongest result of the attack collision
//! (a hit's knockback, a tip, a shield hit, a clank, a hit it dealt) picks its reaction and
//! hitlag, and the frame's damage bookkeeping starts over. A broken shield isn't ported: it
//! stops there.

// NaN takes the original's branches, which the negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::code::run_fighter_cb;
use super::super::handles::FighterId;
use super::super::state::Running;
use crate::fpu::{mem, reg};
use crate::game::ft::FighterKind;
use crate::game::it::ItKind;
use crate::hsd::gobj::GObjId;
use crate::lb::joint::world_position;

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
    let mut shield_broke = false;
    let fp = &mut g.game.fighters[f.0];
    if !fp.shield_was_hit && !(reg(fp.shield_health) >= reg(full)) {
        fp.shield_health = mem(fp::fadds(reg(fp.shield_health), reg(regen)));
        if !(reg(fp.shield_health) <= reg(full)) {
            fp.shield_health = full;
        }
    }
    if fp.shield_was_hit {
        let light = fp::fmadds(
            reg(fp.lightshield_amount),
            fp::fsubs(reg(light_hi), reg(light_lo)),
            reg(light_lo),
        );
        let taken = fp::frsp(f64::from(fp.shield_damage_taken));
        let wear = fp::fmadds(
            reg(wear_mul),
            fp::fmuls(taken, fp::fsubs(1.0, light)),
            reg(wear_add),
        );
        fp.shield_health = mem(fp::fsubs(reg(fp.shield_health), wear));
        if !(reg(fp.shield_health) >= 0.0) {
            fp.shield_health = broken;
            shield_broke = true;
            let (by, slot) = (fp.shield_hit_slot, i32::from(fp.player_idx));
            g.record_shield_broken_by(by, slot);
        }
    }
    // A tip's damage, held back while its hitlag lasted, lands.
    let fp = &mut g.game.fighters[f.0];
    if !(reg(fp.dmg.x189C_unk_num_frames) <= 0.0) {
        fp.dmg.x189C_unk_num_frames = mem(fp::fsubs(reg(fp.dmg.x189C_unk_num_frames), 1.0));
        if reg(fp.dmg.x189C_unk_num_frames) <= 0.0 && reg(fp.dmg.kb_applied) == 0.0 {
            fp.dmg.x189C_unk_num_frames = 0.0;
            g.delayed_tip_damage(gobj);
        }
    }
    let fp = &g.game.fighters[f.0];
    let kb = fp.dmg.kb_applied;
    let mut strength = 0;
    let mut tip = false;
    let mut knocked = false;
    if reg(kb) != 0.0 {
        let ga = fp.ground_or_air;
        g.game.fighters[f.0].dmg.x189C_unk_num_frames = 0.0;
        let dmg = reg(g.game.fighters[f.0].dmg.x1838_percentTemp);
        g.take_damage_coins(f, dmg);
        g.calc_knockback(f);
        g.kb_maybe_lose_copy(gobj);
        if let Some(cb) = g.game.fighters[f.0].take_dmg_2_cb {
            run_fighter_cb(g, gobj, cb);
        }
        if g.game.fighters[f.0].no_kb {
            // A hand takes no reaction: its kind's hook (both hands' are empty), then
            // `ftCo_8008E9D0`; the original asserts on any other kind.
            match g.game.fighters[f.0].fighter_kind() {
                FighterKind::MasterHand | FighterKind::CrazyHand => g.hand_nothing(gobj),
                _ => panic!("the original's assertion failed: ellegal flag fp->no_reaction_always"),
            }
            g.damage_no_reaction(gobj);
        } else {
            match g.game.fighters[f.0].hit_reaction {
                0 => g.damage_pick_reaction(gobj),
                1 => {
                    g.damage_hooks(gobj);
                    g.damage_reaction(gobj, 0.0);
                }
                2 => {
                    g.damage_hooks(gobj);
                    g.damage_no_reaction(gobj);
                }
                3 => {
                    g.damage_hooks(gobj);
                    g.holder_light_hit(gobj);
                }
                _ => {}
            }
            let fp = &g.game.fighters[f.0];
            let (element, applied, mul) = (
                fp.dmg.x1860_element,
                fp.dmg.x183C_applied,
                fp.hitlag_vibrate_mul,
            );
            knocked = true;
            g.start_hit_shake(f, element, applied, msid, ga, mul);
            let dmg = fp::fctiwz(reg(g.game.fighters[f.0].dmg.x1838_percentTemp));
            g.damage_rumble(f, dmg);
            strength = applied;
        }
    } else if reg(fp.dmg.x18a0) != 0.0 {
        strength = fp.dmg.x1840;
        tip = true;
    } else if fp.shield_hit_damage != 0 {
        if shield_broke {
            g.enter_shield_break(gobj);
            g.fighter_play_sfx(f, 0x82, 0x7F, 0x40);
        } else if let Some(cb) = fp.shield_hit_cb {
            run_fighter_cb(g, gobj, cb);
        }
        strength = g.game.fighters[f.0].shield_hit_damage;
    } else if fp.dmg.int_value != 0 {
        if reg(fp.dmg.x191C) != 0.0 && fp.victim_gobj.is_none() && fp.target_item_gobj.is_none() {
            g.damage_hooks(gobj);
            g.enter_rebound(gobj);
        }
        strength = g.game.fighters[f.0].dmg.int_value;
    } else if fp.dmg.x1914 != 0 {
        if let Some(cb) = fp.deal_dmg_cb {
            run_fighter_cb(g, gobj, cb);
        }
        let fp = &g.game.fighters[f.0];
        strength = fp.dmg.x1914;
        if fp.attack_flags[3] == 0x46 {
            g.start_rumble(f, 0xE, 0);
        } else {
            g.hit_rumble(f, strength);
        }
    } else if fp.dmg.x1924 != 0 {
        strength = fp.dmg.x1924;
    } else if fp.reflect.x1A3C_damageOver != 0 {
        // A reflector overpowered breaks like a shield (`ftCo_80098C9C`).
        g.enter_shield_break(gobj);
    } else if reg(fp.reflect.x1A2C_reflectHitDirection) != 0.0 {
        if let Some(cb) = fp.reflect_hit_cb {
            run_fighter_cb(g, gobj, cb);
        }
    } else if reg(fp.absorb.x1A40_absorbHitDirection) != 0.0 {
        // `ftData_OnAbsorb`: Ness's and Mr. Game & Watch's.
        match fp.fighter_kind() {
            FighterKind::Ness => g.ness_on_absorb(gobj),
            FighterKind::GameWatch => g.gw_on_absorb(gobj),
            _ => {}
        }
    } else if fp.hit_by.is_some()
        && let Some(cb) = fp.hurtbox_detect_cb
    {
        run_fighter_cb(g, gobj, cb);
    }
    if reg(kb) == 0.0 && reg(g.game.fighters[f.0].dmg.x1838_percentTemp) != 0.0 {
        let dmg = reg(g.game.fighters[f.0].dmg.x1838_percentTemp);
        g.take_damage_coins(f, dmg);
        g.kb_maybe_lose_copy_no_knockback(gobj);
        g.forget_attacker_grounded(f);
    }
    g.stamina_out(gobj);
    if strength != 0 {
        let mul = g.game.fighters[f.0].hitlag_vibrate_mul;
        let lag = g.hitlag(strength, msid, mul);
        let fp = &mut g.game.fighters[f.0];
        fp.dmg.x195c_hitlag_frames = mem(lag);
        if !(reg(fp.dmg.x195c_hitlag_frames) >= reg(fp.min_hitlag)) {
            fp.dmg.x195c_hitlag_frames = fp.min_hitlag;
        }
        let lag = reg(fp.dmg.x195c_hitlag_frames);
        if !(lag <= 0.0) {
            let max = g.common().f32(0x194);
            let fp = &mut g.game.fighters[f.0];
            if !(lag <= reg(max)) {
                fp.dmg.x195c_hitlag_frames = max;
            }
            fp.allow_sdi = true;
            if knocked {
                fp.in_hitstun_sdi = true;
            }
            if tip {
                fp.dmg.x189C_unk_num_frames = fp.dmg.x195c_hitlag_frames;
            }
            if !fp.in_hitlag {
                g.start_hitlag(gobj);
            }
        }
    } else {
        g.queued_sounds(f);
    }
    let fp = &g.game.fighters[f.0];
    if fp.pending_hitlag || reg(fp.dmg.x1958) != 0.0 {
        if !fp.in_hitlag {
            g.start_hitlag(gobj);
        }
        let fp = &mut g.game.fighters[f.0];
        if fp.pending_hitlag {
            fp.hitlag_held = true;
            fp.pending_hitlag = false;
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
        fp.ground_attacker_shield_kb_vel = mem(if !(reg(fp.dmg.x192c) >= 0.0) {
            push
        } else {
            fp::fneg(push)
        });
        g.shield_push(gobj);
    }
    let fp = &mut g.game.fighters[f.0];
    fp.dmg.x1838_percentTemp = 0.0;
    fp.dmg.x183C_applied = 0;
    fp.hit_reaction = 0;
    fp.dmg.kb_applied = 0.0;
    fp.dmg.x18a0 = 0.0;
    fp.dmg.x1840 = 0;
    fp.dmg.x1914 = 0;
    fp.dmg.int_value = 0;
    fp.dmg.x191C = 0.0;
    fp.hit_by = None;
    fp.hit_this_frame = false;
    fp.dmg.x1924 = 0;
    fp.dmg.x1928 = 0.0;
    fp.shield_damage_taken = 0;
    fp.shield_hit_damage = 0;
    fp.shield_hit_by = None;
    fp.reflect.x1A3C_damageOver = 0;
    fp.reflect.x1A2C_reflectHitDirection = 0.0;
    fp.absorb.x1A40_absorbHitDirection = 0.0;
    fp.absorb.x1A44_damageTaken = 0;
    fp.absorb.x1A48_hitsTaken = 0;
    fp.hitlag_vibrate_mul = 1.0;
    fp.min_hitlag = 0.0;
    fp.dmg.x1950 = 0;
    if !fp.was_in_hitlag || fp.dmg.x18F4 != 0 {
        g.afterimage(f);
    }
    g.cpu_hurt_bounds(f);
}

impl Running<'_> {
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
        fp.in_hitlag = true;
        if let Some(partner) = fp.partner
            && !fp.hitlag_held
        {
            self.game.fighter_mut(partner).hitlag_held = true;
            self.start_hitlag(partner);
        }
    }

    /// The sounds a hit queued for when hitlag ends, played now there's none,
    /// `ftCo_80090718`.
    #[doc(alias = "ftCo_80090718")]
    pub(crate) fn queued_sounds(&mut self, f: FighterId) {
        let id = self.game.fighters[f.0].dmg.x1908;
        if id != -1 {
            self.fighter_play_sfx(f, id, 0x7F, 0x40);
            self.game.fighters[f.0].dmg.x1908 = -1;
        }
        if let Some(arr) = self.game.fighters[f.0].dmg.x190C {
            // `FtSFXArr`: a count and its ids.
            let n = self.data.u32(arr) as i32;
            let at = self.data.ptr(arr.at(4)).expect("a voice set's ids");
            self.fighter_random_voice_in(f, at, n);
            self.game.fighters[f.0].dmg.x190C = None;
        }
    }

    /// A shield hit pushes the attacker back along the floor, `ftCommon_8007E2A4`.
    #[doc(alias = "ftCommon_8007E2A4")]
    fn shield_push(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let (n, v) = (
            fp.coll_data.floor.normal,
            reg(fp.ground_attacker_shield_kb_vel),
        );
        fp.atk_shield_kb_vel.x = mem(fp::fmuls(reg(n.y), v));
        fp.atk_shield_kb_vel.y = mem(fp::fmuls(fp::fneg(reg(n.x)), v));
    }

    /// A sword's afterimage follows it, `ftCo_800C2FD8`: while one is on (`x2100` not -1),
    /// each frame records where the sword's joint is and which way its blade points (its
    /// matrix's first axis), in a ring of three for the trail's drawing, with the trail's two
    /// widths from the character's sword attributes. A held Beam Sword's trail (`x2101_bits_8`)
    /// follows its bone 5 and its blade (the matrix's second axis), with the widths its length
    /// gives; with nothing in hand the trail ends.
    #[doc(alias = "ftCo_800C2FD8")]
    fn afterimage(&mut self, f: FighterId) {
        let fp = &self.game.fighters[f.0];
        if fp.afterimage_mode == -1 {
            return;
        }
        let at = usize::from(fp.afterimage_slot);
        let (joint, axis) = if fp.item_afterimage {
            let Some(item) = fp.item_gobj else {
                self.game.fighters[f.0].afterimage_mode = -1;
                return;
            };
            let kind = crate::game::it::lib26b1::kind(self.game, item);
            assert!(
                kind == ItKind::BeamSword,
                "ftCo_800C2FD8: no afterimage item!"
            );
            let (w0, w1) = crate::game::it::kinds::sword::trail_widths(self.game, item);
            let fp = &mut self.game.fighters[f.0];
            fp.afterimage_width_a = w0;
            fp.afterimage_width_b = w1;
            (
                crate::game::it::kinds::sword::trail_joint(self.game, self.data, item),
                1,
            )
        } else {
            // The sword's attributes in the character's (Link's and Young Link's at x64,
            // Marth's and Roy's at x78); any other character's are whatever the stack held.
            let base = match fp.fighter_kind() {
                FighterKind::Link | FighterKind::Clink => 0x64,
                FighterKind::Mars | FighterKind::Emblem => 0x78,
                k => panic!("ftCo_800C2FD8: character {k:?} has no sword attributes to read"),
            };
            let attrs =
                &self.game.fighter_attrs[fp.dat_attrs.expect("a fighter without attributes").0];
            let word =
                |o: usize| u32::from_be_bytes(attrs[base + o..base + o + 4].try_into().unwrap());
            let (bone, width0, width1) = (
                word(0x14),
                f32::from_bits(word(0x18)),
                f32::from_bits(word(0x1C)),
            );
            let parts = fp.parts.expect("a fighter without parts");
            let joint = self.game.fighter_parts[parts.0][bone as usize].joint;
            let fp = &mut self.game.fighters[f.0];
            fp.afterimage_width_a = width0;
            fp.afterimage_width_b = width1;
            (joint, 0)
        };
        let pos = world_position(&mut self.game.objs, self.data, joint, None);
        let m = self.game.objs.jobjs[joint.expect("the sword's joint")]
            .mtx
            .m;
        let fp = &mut self.game.fighters[f.0];
        fp.afterimage_ring[at].x0 = pos;
        fp.afterimage_ring[at].xC = crate::vec::Vec3::new(m[0][axis], m[1][axis], m[2][axis]);
        fp.afterimage_slot = if at == 2 { 0 } else { at as u8 + 1 };
        if fp.afterimage_mode < 3 {
            fp.afterimage_mode += 1;
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
            let scale = fp::fmuls(reg(h.scale), reg(fp.scale.y));
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
            c.hurt_behind = mem(front);
            c.hurt_ahead = mem(fp::fneg(back));
        } else {
            c.hurt_behind = mem(fp::fneg(back));
            c.hurt_ahead = mem(front);
        }
        c.hurt_middle = mem(fp::frsp(fp::fmul(
            0.5,
            fp::fadds(reg(c.hurt_behind), reg(c.hurt_ahead)),
        )));
        c.hurt_top = mem(top);
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

    /// Rumbles for a hit the fighter dealt, by its strength (`ftCommon_8007EE0C`): a
    /// presentation event, which the checks don't see.
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
        self.start_rumble(f, 10, frames as u32);
    }
}
