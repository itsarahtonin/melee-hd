//! Helpers the processes share, from the common fighter units (`ftcommon.c`, `ft_*.c`,
//! `ftCo_*.c`).

use gekko_fp as fp;

use super::super::code::{run_fighter_cb, unported_call};
use super::super::handles::FighterId;
use super::super::state::Running;
use super::super::structs::{Fighter, Vec2};
use super::parts::{TOP_N, TRANS_N};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::lb::joint::world_position;
use crate::vec::Vec3;

/// `GA_Ground`.
pub const GA_GROUND: i32 = 0;

/// `GA_Air`.
pub const GA_AIR: i32 = 1;

/// The process link fighters' GObjs are on.
pub const FIGHTER_P_LINK: u8 = 8;

/// `M_PI` as the original spells it, a double.
const PI: f64 = std::f64::consts::PI;

impl Running<'_> {
    /// The held item taken in hand again, `ftpickupitem_80094818`: the character's pickup
    /// hook (`ftData_OnItemPickupExt`) and the player's item record (`pl_8003E17C`). Nothing
    /// without an item; holding one isn't ported.
    #[doc(alias = "ftpickupitem_80094818")]
    pub fn retake_held_item(&mut self, gobj: GObjId, _flag: bool) {
        let fp = self.game.fighter(gobj);
        if fp.x1978.is_some() || fp.item_gobj.is_some() {
            unported_call("ftpickupitem_80094818 with an item held");
        }
    }

    /// `ftCo_800C37A0`: a turn that spins the model about TransN over `ftCommonData` x648
    /// frames (`dmg.x18F4` left); at its end the fighter faces the other way.
    #[doc(alias = "ftCo_800C37A0")]
    pub fn turn_around_tick(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].dmg.x18F4 == 0 {
            return;
        }
        let part = self.bone_index(&self.game.fighters[f.0], TRANS_N);
        let parts = self.game.fighters[f.0].parts.expect("parts");
        let jobj = self.game.fighter_parts[parts.0][part as usize]
            .x4_jobj2
            .expect("TransN's second joint");
        let total = self.common().i32(0x648);
        let fp = &mut self.game.fighters[f.0];
        let rot_y = fp::frsp(fp::fdiv(
            fp::fmul(PI, fp::frsp(f64::from(total.wrapping_sub(fp.dmg.x18F4)))),
            fp::frsp(f64::from(total)),
        ));
        fp.dmg.x18F4 = fp.dmg.x18F4.wrapping_sub(1);
        let rot_y = mem(rot_y);
        self.game.part_set_rot_y(f, part, rot_y);
        jobj::set_rotation_y(&mut self.game.objs.jobjs, jobj, rot_y);
        let fp = &mut self.game.fighters[f.0];
        if fp.dmg.x18F4 == 0 {
            fp.x2220_b4 = false;
            fp.facing_dir = mem(fp::fneg(reg(fp.facing_dir)));
            let top = mem(fp::frsp(fp::fmul(fp::fdiv(PI, 2.0), reg(fp.facing_dir))));
            self.game.part_set_rot_y(f, TOP_N, top);
            self.game.part_set_rot_y(f, part, 0.0);
            jobj::set_rotation_y(&mut self.game.objs.jobjs, jobj, 0.0);
            if let Some(cb) = self.game.fighters[f.0].x21F8 {
                run_fighter_cb(self, gobj, cb);
            }
        }
    }

    /// `ft_800819A8`: the box items test for being picked up, from the ECB's sides and the
    /// character's pickup height.
    #[doc(alias = "ft_800819A8")]
    pub fn update_item_pickup_bounds(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let pos = fp.coll_data.cur_pos;
        let lx = reg(fp.coll_data.ecb.left.x);
        let rx = reg(fp.coll_data.ecb.right.x);
        fp.dmg.x1930.x0.x = mem(fp::fadds(lx, reg(pos.x)));
        fp.dmg.x1930.x0.y = mem(fp::fadds(rx, reg(pos.x)));
        fp.dmg.x1930.x0.z = mem(fp::fadds(reg(fp.cur_pos.y), reg(fp.co_attrs.x17C)));
    }

    /// Whether the magnifier shows the player off screen, `ifMagnify_802FC998`.
    #[doc(alias = "ifMagnify_802FC998")]
    pub fn magnify_offscreen(&self, slot: i32) -> bool {
        self.game.magnify.player[slot as usize].state.is_offscreen
    }

    /// The fighter takes `amount` damage, `Fighter_TakeDamage_8006CC7C`, unless it's armored
    /// (`x2226_b4` without `x2226_b3`): the percent (capped at 999) and the metal box's and
    /// `x2038`'s health, its player block's, the player's damage records (`pl_8003EC9C`), and
    /// a stamina fighter's knockout (`ftCo_800C8C84`).
    #[doc(alias = "Fighter_TakeDamage_8006CC7C")]
    pub fn take_damage(&mut self, f: FighterId, amount: f64) {
        let fp = &mut self.game.fighters[f.0];
        if fp.x2226_b4 && !fp.x2226_b3 {
            return;
        }
        fp.dmg.x1830_percent = mem(fp::fadds(reg(fp.dmg.x1830_percent), amount));
        if fp.metal_timer != 0 {
            fp.metal_health = fp::fctiwz(fp::fsubs(fp::frsp(f64::from(fp.metal_health)), amount));
        }
        if fp.x2034 != 0 {
            fp.x2038 = fp::fctiwz(fp::fsubs(fp::frsp(f64::from(fp.x2038)), amount));
        }
        if reg(fp.dmg.x1830_percent) > 999.0 {
            fp.dmg.x1830_percent = 999.0;
        }
        let (slot, sub, percent) = (
            i32::from(fp.player_idx),
            i32::from(fp.is_sub_fighter),
            fp.dmg.x1830_percent,
        );
        self.game
            .set_player_damage_at(slot, sub, fp::fctiwz(reg(percent)));
        // `pl_8003EC9C`: the damage taken and the highest percent.
        if sub == 0 {
            let t = self.game.player_stale_moves(slot);
            t.xC60 = mem(fp::fadds(reg(t.xC60), amount));
            if reg(t.xC64) < reg(percent) {
                t.xC64 = percent;
            }
            self.game.player_mut(slot).set_ext_u32(0xD10, 0);
        }
        // `ftCo_800C8C84`.
        let fp = &self.game.fighters[f.0];
        if !fp.stamina_dead && fp.x2225_b7 && fp.dmg.x18F0 == 0 {
            unported_call("ftCo_800C8C84 (a stamina fighter's knockout)");
        }
    }

    /// A grounded fighter forgets who last hit it, `ftCommon_800804FC`.
    #[doc(alias = "ftCommon_800804FC")]
    pub fn forget_attacker_grounded(&mut self, f: FighterId) {
        let fp = &mut self.game.fighters[f.0];
        if fp.ground_or_air == GA_GROUND {
            fp.dmg.x18c4_source_ply = 6;
            fp.dmg.x18C8 = -1;
        }
    }

    /// Damage taken outside a knockback (`Fighter_UnkTakeDamage_8006CC30`): the damage, then in
    /// a coin match the coins it knocks out (`ftCommon_8007EA90`).
    #[doc(alias = "Fighter_UnkTakeDamage_8006CC30")]
    pub fn take_damage_coins(&mut self, f: FighterId, amount: f64) {
        self.take_damage(f, amount);
        if self.game.vs_is_coins() {
            unported_call("ftCommon_8007EA90 (a coin match's coins)");
        }
    }

    /// `ftCo_800D71D8`: a delayed item pickup's window, an item caught in reach while it
    /// lasts.
    #[doc(alias = "ftCo_800D71D8")]
    pub fn item_pickup_tick(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).x209C == 0 {
            return;
        }
        let item = self.item_in_reach(gobj, 1);
        let fp = self.game.fighter_mut(gobj);
        fp.x209C -= 1;
        if fp.x209C == 0 {
            fp.x2224_b1 = true;
        }
        if item.is_some() {
            unported_call("fn_800D6F58 (catching an item in the air)");
        }
    }

    /// `ftColl_800764DC`: the combo counter's window, and forgetting an attacker whose
    /// combo ended.
    #[doc(alias = "ftColl_800764DC")]
    pub fn combo_tick(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.x2098 != 0 {
            fp.x2098 -= 1;
        }
        if let Some(other) = fp.x2094 {
            let o = self.game.fighter(other);
            if !o.x221C_b6 && o.x2098 == 0 {
                self.game.fighter_mut(gobj).x2094 = None;
            }
        }
    }

    /// `ftCo_800DEF38`: a smash attack's charge.
    #[doc(alias = "ftCo_800DEF38")]
    pub fn smash_charge_tick(&mut self, gobj: GObjId) {
        let sfx_frame = self.common().f32(0x7C8);
        let attr = &mut self.game.fighter_mut(gobj).smash_attrs;
        let charging = attr.state == 2;
        if !charging && attr.state != 4 {
            return;
        }
        attr.x2118_frames = mem(fp::fadds(reg(attr.x2118_frames), 1.0));
        attr.x212C = attr.x212C.wrapping_add(1);
        if attr.x212C >= attr.x212D {
            attr.x212C = 0;
        }
        if charging && attr.x2118_frames >= attr.x211C_holdFrame {
            if attr.x2118_frames > attr.x211C_holdFrame {
                attr.x2118_frames = attr.x211C_holdFrame;
            }
            attr.state = 3;
            let (rate, color) = (attr.x2124_frameSpeedMul, attr.x2128);
            self.set_anim_rate(gobj, rate);
            self.end_color_anim(gobj, color);
            let f = self.game.fighter_id(gobj);
            self.stop_rumble(f, 0x24);
        }
        let attr = &self.game.fighter(gobj).smash_attrs;
        if attr.x2130_sfxBool == 0 && attr.x2118_frames >= sfx_frame {
            let f = self.game.fighter_id(gobj);
            self.fighter_play_sfx(f, 0x7B, 0x7F, 0x40);
            self.game.fighter_mut(gobj).smash_attrs.x2130_sfxBool = 1;
        }
    }

    /// `ftCommon_8007F8B4`: where the fighter will be once its pending offset applies.
    #[doc(alias = "ftCommon_8007F8B4")]
    pub fn pending_pos(fp: &Fighter) -> Vec3 {
        Vec3::new(
            mem(fp::fadds(reg(fp.cur_pos.x), reg(fp.xD4_unk_vel.x))),
            mem(fp::fadds(reg(fp.cur_pos.y), reg(fp.xD4_unk_vel.y))),
            mem(fp::fadds(reg(fp.cur_pos.z), reg(fp.xD4_unk_vel.z))),
        )
    }

    /// `ftCommon_8007E0E4`: grounded fighters push apart, and drift back toward depth 0.
    #[doc(alias = "ftCommon_8007E0E4")]
    pub fn nudge(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.xF8_playerNudgeVel.y = 0.0;
        fp.xF8_playerNudgeVel.x = 0.0;
        if fp.x2219_b1 || fp.x2219_b5 || fp.ground_or_air != GA_GROUND {
            return;
        }
        let c = self.common();
        let fp = self.game.fighter(gobj);
        let (f30, f31) = if !fp.is_sub_fighter {
            (reg(c.f32(0x454)), reg(c.f32(0x458)))
        } else {
            (reg(c.f32(0x45C)), reg(c.f32(0x460)))
        };
        let mut v = Self::pending_pos(fp);
        if !fp.x221D_b5 {
            if !fp.is_sub_fighter {
                self.nudge_from_others(gobj, v);
            } else {
                self.nudge_sub(gobj, v);
            }
        }
        let fp = self.game.fighter_mut(gobj);
        let nv = &mut fp.xF8_playerNudgeVel;
        if nv.y == 0.0 && v.z != 0.0 {
            nv.y = mem(if v.z < 0.0 { f30 } else { fp::fneg(f30) });
        }
        let sum = |nv: &Vec2, z: f32| fp::fadds(reg(z), reg(nv.y));
        if (nv.y > 0.0 && v.z < 0.0 && sum(nv, v.z) >= 0.0)
            || (nv.y < 0.0 && v.z > 0.0 && sum(nv, v.z) <= 0.0)
        {
            nv.y = mem(fp::fneg(reg(v.z)));
            v.z = 0.0;
        }
        if sum(nv, v.z) > f31 {
            nv.y = mem(fp::fsubs(f31, reg(v.z)));
        } else if sum(nv, v.z) < fp::fneg(f31) {
            nv.y = mem(fp::fsubs(fp::fneg(f31), reg(v.z)));
        }
        if fp.is_sub_fighter {
            fp.xF8_playerNudgeVel.x = 0.0;
        }
    }

    /// Whether two floor lines are the same or neighbours.
    fn floors_meet(&self, a: i32, b: i32) -> bool {
        let geometry = self.game.geometry.as_ref().expect("no collision geometry");
        a == b || geometry.line_next(a) == b || geometry.line_prev(a) == b
    }

    /// Whether `x` is within the crowd's margin of the stage's sides (`gCrowdConfig`'s
    /// `horiz_margin` inside the enabled floors' bounds), `un_80322258`.
    #[doc(alias = "un_80322258")]
    pub fn near_side_blast_zone(&self, x: f32) -> bool {
        let crowd = self.load_common(0x54);
        let margin = reg(self.data.f32(crowd.at(0x2C)));
        let b = self
            .game
            .geometry
            .as_ref()
            .expect("no collision geometry")
            .bounds[1];
        reg(x) < fp::fadds(margin, reg(b.left)) || reg(x) > fp::fsubs(reg(b.right), margin)
    }

    /// `ftCommon_8007DD7C`: pushes from the other grounded fighters on the same floor.
    #[doc(alias = "ftCommon_8007DD7C")]
    fn nudge_from_others(&mut self, gobj: GObjId, v: Vec3) {
        let c = self.common();
        let (x450, x454) = (reg(c.f32(0x450)), reg(c.f32(0x454)));
        let mut seen_self = false;
        let mut cur = self.game.world.first_in(FIGHTER_P_LINK);
        while let Some(o) = cur {
            cur = self.game.world.gobjs[o.0].next;
            if o == gobj || self.is_same_player(Some(o), Some(gobj)) {
                seen_self = true;
                continue;
            }
            let other = self.game.fighter(o);
            if other.is_sleeping
                || other.ground_or_air != GA_GROUND
                || other.victim_gobj.is_some()
                || other.is_sub_fighter
            {
                continue;
            }
            let me = self.game.fighter(gobj);
            if !self.floors_meet(me.coll_data.floor.index, other.coll_data.floor.index) {
                continue;
            }
            let p = Self::pending_pos(other);
            let (mine, theirs) = (me.x2C4.clone(), other.x2C4.clone());
            let d = fp::fsubs(
                fp::fmadds(reg(mine.x), reg(me.facing_dir), reg(v.x)),
                fp::fmadds(reg(other.facing_dir), reg(theirs.x), reg(p.x)),
            );
            let abs = if d < 0.0 { fp::fneg(d) } else { d };
            #[allow(clippy::neg_cmp_op_on_partial_ord)]
            let apart = !(abs < fp::fadds(reg(mine.y), reg(theirs.y)));
            if apart {
                continue;
            }
            let signed = |neg: bool, v: f64| if neg { fp::fneg(v) } else { v };
            let nv = &mut self.game.fighter_mut(gobj).xF8_playerNudgeVel;
            let dx = if d != 0.0 {
                signed(d < 0.0, x450)
            } else {
                signed(seen_self, x450)
            };
            nv.x = mem(fp::fadds(reg(nv.x), dx));
            let dz = fp::fsubs(reg(v.z), reg(p.z));
            let dy = if dz != 0.0 {
                signed(dz < 0.0, x454)
            } else if d != 0.0 {
                signed(d < 0.0, x454)
            } else {
                signed(seen_self, x454)
            };
            nv.y = mem(fp::fadds(reg(nv.y), dy));
        }
    }

    /// `ftCommon_8007DFD0` (a sub-fighter keeps off its leader), not ported yet.
    fn nudge_sub(&mut self, _gobj: GObjId, _v: Vec3) {
        unported_call("ftCommon_8007DFD0");
    }

    /// `ftCo_800C0A98`: a burial's timers.
    #[doc(alias = "ftCo_800C0A98")]
    pub fn bury_tick(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.bury_timer_1 != 0 {
            fp.bury_timer_1 -= 1;
        }
        if matches!(fp.bury_grkind, 30 | 4 | 32 | 39 | 8) && fp.bury_timer_2 != 0 {
            fp.bury_timer_2 -= 1;
        }
    }

    /// Whether part 0x35's X rotation changed since it was last noted (`x100`),
    /// `ftAnim_80070FD0`.
    #[doc(alias = "ftAnim_80070FD0")]
    pub fn x100_rot_changed(&mut self, f: FighterId) -> bool {
        let part = self.bone_index(&self.game.fighters[f.0], 0x35);
        let rot = self.game.part_rot_x(f, part);
        let fp = &mut self.game.fighters[f.0];
        if fp.x100 != rot {
            fp.x100 = rot;
            return true;
        }
        false
    }

    /// The stage's wind on the fighter, `ftColl_GetWindOffsetVec`. No stage that blows is
    /// ported.
    #[doc(alias = "ftColl_GetWindOffsetVec")]
    pub fn wind_offset(&mut self, gobj: GObjId) -> Vec3 {
        if !self.game.fighter(gobj).x2224_b4 && self.game.ft.wind_sources != 0 {
            unported_call("ftCo_800C0A28");
        }
        Vec3::default()
    }

    /// `ftColl_80076528`: a grounded fighter that was hit (`x2092` frames ago) slides along
    /// the floor with the combo's push.
    #[doc(alias = "ftColl_80076528")]
    pub fn combo_push(&mut self, gobj: GObjId) {
        let c = self.common();
        let (limit, near, far) = (c.i32(0x4C8), reg(c.f32(0x4D0)), reg(c.f32(0x4D4)));
        let fp = self.game.fighter_mut(gobj);
        if fp.x2092 == 0 {
            return;
        }
        fp.x2092 -= 1;
        if fp.victim_gobj.is_none() && fp.ground_or_air == GA_GROUND {
            let n = fp.coll_data.floor.normal;
            let push = if i32::from(fp.x2090) < limit {
                near
            } else {
                far
            };
            let d = fp::fmuls(reg(fp.facing_dir), push);
            fp.cur_pos.x = mem(fp::fnmsubs(reg(n.y), d, reg(fp.cur_pos.x)));
            fp.cur_pos.y = mem(fp::fnmsubs(fp::fneg(reg(n.x)), d, reg(fp.cur_pos.y)));
        }
    }

    /// `ftCo_800D3158`: past a blast zone, the fighter dies; whether it did.
    #[doc(alias = "ftCo_800D3158")]
    pub fn blast_zone_check(&mut self, gobj: GObjId) -> bool {
        let kb_limit = reg(self.common().f32(0x4F0));
        let si = &self.game.stage_info;
        // `Stage_GetBlastZone*Offset`, inline.
        let (x_off, y_off) = (reg(si.cam_info.cam_x_offset), reg(si.cam_info.cam_y_offset));
        let b = &si.blast_zone;
        let right = fp::fadds(reg(b.right), x_off);
        let left = fp::fadds(reg(b.left), x_off);
        let top = fp::fadds(reg(b.top), y_off);
        let bottom = fp::fadds(reg(b.bottom), y_off);
        let fp = self.game.fighter(gobj);
        if fp.x222A_b1 || fp.x2228_b5 || fp.is_sandbag || fp.x2219_b2 || fp.x2219_b1 {
            return false;
        }
        let (x, y) = (reg(fp.cur_pos.x), reg(fp.cur_pos.y));
        if x > right {
            self.enter_dead_right(gobj);
            return true;
        }
        if x < left {
            self.enter_dead_left(gobj);
            return true;
        }
        if y > top
            && (fp.ground_or_air == GA_GROUND || fp.x2222_b3 || reg(fp.x8c_kb_vel.y) > kb_limit)
        {
            let (slot, ice) = (
                i32::from(fp.player_idx),
                fp.motion_id == super::co::ids::ms::DAMAGE_ICE,
            );
            if self.game.player_more_flags_bit5(slot) {
                self.enter_dead_up(gobj);
            } else {
                let roll = self.game.objs.rng.below(100) + 1;
                let star_chance = self.common().i32(0x520);
                let free = super::super::camera::is_free(&self.game.camera.cam);
                match (!free && star_chance >= roll, ice) {
                    (true, true) => self.enter_dead_up_fall_ice(gobj),
                    (true, false) => self.enter_dead_up_fall(gobj),
                    (false, true) => self.enter_dead_up_star_ice(gobj),
                    (false, false) => self.enter_dead_up_star(gobj),
                }
            }
            return true;
        }
        if y < bottom {
            self.enter_dead_down(gobj);
            return true;
        }
        false
    }

    /// `ftColl_8007AF28`: the hurtboxes follow their bones again.
    #[doc(alias = "ftColl_8007AF28")]
    pub fn hurt_capsules_follow(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let n = fp.hurt_capsules_len as usize;
        for h in &mut fp.hurt_capsules[..n] {
            h.capsule.skip_update_pos = false;
        }
    }

    /// Which screen edge the magnifier shows the player past (0 when on screen),
    /// `ifMagnify_802FB6E8`.
    #[doc(alias = "ifMagnify_802FB6E8")]
    pub fn magnify_edge(&self, slot: i32) -> i32 {
        if self.magnify_offscreen(slot) {
            i32::from(self.game.magnify.player[slot as usize].state.edge)
        } else {
            0
        }
    }

    /// The fighter's matrix for a depth scale other than 1 (`x44_mtx`), from its root
    /// joint's transform with the model's scale, `Fighter_UnkApplyTransformation_8006C0F0`.
    #[doc(alias = "Fighter_UnkApplyTransformation_8006C0F0")]
    pub fn depth_scale_matrix(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).x34_scale.z == 1.0 {
            return;
        }
        let root = self.root_jobj(gobj);
        jobj::setup_matrix(&mut self.game.objs, self.data, Some(root));
        let j = &self.game.objs.jobjs[root];
        let inv = j.mtx.hsd_inverse();
        let mut scale = j.scale;
        let (rot, trans) = (j.rotate, j.translate);
        scale.x = mem(Self::model_scale(self.game.fighter(gobj)));
        let srt = crate::mtx::Mtx34::srt(scale, Vec3::new(rot.x, rot.y, rot.z), trans, None);
        self.game.fighter_mut(gobj).x44_mtx = srt.concat(&inv).m;
    }

    /// `ftColl_8007AE80`: each hitbox follows its bone.
    #[doc(alias = "ftColl_8007AE80")]
    pub fn hitboxes_follow(&mut self, gobj: GObjId) {
        for i in 0..4 {
            self.hitbox_follow(gobj, i);
        }
    }

    /// `ftColl_8007AD18`: a new hitbox takes its bone's position, a live one keeps where it
    /// was (for the swept capsule) and moves.
    #[doc(alias = "ftColl_8007AD18")]
    pub fn hitbox_follow(&mut self, gobj: GObjId, i: usize) {
        let fp = self.game.fighter(gobj);
        let h = &fp.x914[i];
        let state = h.state;
        if !(1..=3).contains(&state) {
            return;
        }
        let offset = if h.x43_b1 {
            let inv = fp::fdivs(1.0, reg(fp.x34_scale.y));
            let b = h.b_offset;
            Vec3::new(
                mem(fp::fmuls(reg(b.x), inv)),
                mem(fp::fmuls(reg(b.y), inv)),
                mem(fp::fmuls(reg(b.z), inv)),
            )
        } else {
            h.b_offset
        };
        let j = h.jobj;
        if state == 1 {
            let p = world_position(&mut self.game.objs, self.data, j, Some(offset));
            let h = &mut self.game.fighter_mut(gobj).x914[i];
            h.x4C = p;
            h.x58 = p;
            h.state = 2;
            return;
        }
        let h = &mut self.game.fighter_mut(gobj).x914[i];
        if state == 2 {
            h.state = 3;
        }
        h.x58 = h.x4C;
        let p = world_position(&mut self.game.objs, self.data, j, Some(offset));
        self.game.fighter_mut(gobj).x914[i].x4C = p;
    }

    /// `ft_8007C224`: the thrown fighter's hitbox follows its bone, as `ftColl_8007AD18`
    /// does without the scale correction.
    #[doc(alias = "ft_8007C224")]
    pub fn thrown_hitbox_follow(&mut self, gobj: GObjId) {
        let h = &self.game.fighter(gobj).x1064_thrownHitbox;
        let (state, j, offset) = (h.state, h.jobj, h.b_offset);
        match state {
            1 => {
                let p = world_position(&mut self.game.objs, self.data, j, Some(offset));
                let h = &mut self.game.fighter_mut(gobj).x1064_thrownHitbox;
                h.x4C = p;
                h.x58 = p;
                h.state = 2;
            }
            2 | 3 => {
                let h = &mut self.game.fighter_mut(gobj).x1064_thrownHitbox;
                if state == 2 {
                    h.state = 3;
                }
                h.x58 = h.x4C;
                let p = world_position(&mut self.game.objs, self.data, j, Some(offset));
                self.game.fighter_mut(gobj).x1064_thrownHitbox.x4C = p;
            }
            _ => {}
        }
    }

    /// `ft_8007C6DC`: in team-less stamina matches (`gm_8016B0B4`), the two stamina markers
    /// (`x1614`) follow their bones in the plane.
    #[doc(alias = "ft_8007C6DC")]
    pub fn x1614_follow(&mut self, gobj: GObjId) {
        if self.game.vs.start.match_kind != 2 {
            return;
        }
        let fp = self.game.fighter(gobj);
        let table = self
            .data
            .ptr(fp.ft_data.expect("ftData").at(0x38))
            .expect("ftData x38");
        for i in 0..2 {
            let off = crate::game::mirror::read_vec3(self.data, table.at(0x14 * i as u32 + 4));
            let fp = self.game.fighter_mut(gobj);
            let e = &mut fp.x1614[i];
            e.x14 = e.x8;
            let j = e.x4;
            let p = world_position(&mut self.game.objs, self.data, j, Some(off));
            let e = &mut self.game.fighter_mut(gobj).x1614[i];
            e.x8 = p;
            e.x8.z = 0.0;
        }
    }
}
