//! The chain (`ftseakspecials.c`): drawing it, swinging it with the stick (the pose blends
//! toward the stick's direction, eased), its hitboxes switched on while it moves fast enough,
//! and retracting it. The chain item's own code (`itseakchain.c`, links of `ItemLink` like
//! Link's hookshot) is another part of the port's: Sheik's code reaches it by address
//! ([`ext`]), and it reaches Sheik's queries ([`call`]).

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::code::Val;
use super::super::super::handles::CodeRef;
use super::super::super::it::item::article;
use super::super::super::state::Running;
use super::super::parts::TRANS_N;
use super::{GA_AIR, PAD_B, addr, attr, ms, mv, u};
use crate::fpu::{mem, reg};
use crate::game::structs::{Fighter, HitCapsule};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::lb::hitcapsule;
use crate::math;
use crate::vec::Vec3;

/// The chain item's functions Sheik's code calls, by address.
pub mod ext {
    pub const IT_SEAK_CHAIN_SPAWN: u32 = 0x802B_B290;
    /// Destroys the chain.
    pub const IT_802BB20C: u32 = 0x802B_B20C;
    /// The chain's hitlag hooks (`it_8026B724`, `it_8026B73C`).
    pub const IT_802BAEEC: u32 = 0x802B_AEEC;
    pub const IT_802BAF0C: u32 = 0x802B_AF0C;
    /// Retracts it.
    pub const IT_802BCF84: u32 = 0x802B_CF84;
    /// Throws it out at a velocity.
    pub const IT_802BCFC4: u32 = 0x802B_CFC4;
}

/// `transition_flags`: what the chain's states keep between the ground and the air.
const TRANSITION_FLAGS: u32 = {
    use super::super::motion::flags::*;
    SKIP_HIT
        | SKIP_MAT_ANIM
        | SKIP_COL_ANIM
        | UPDATE_CMD
        | SKIP_ITEM_VIS
        | UNK_19
        | SKIP_MODEL_PART_VIS
        | SKIP_MODEL_FLAGS
        | UNK_27
};

/// `FtPart_L3rdNa`: the hand the chain hangs from (Sheik's code indexes `fp->parts` by the
/// part directly).
const PART_L3RD_NA: i32 = 26;

/// The swing's sounds: forward or up, and back. (The original passes these as `flags`.)
const SFX_SWING: i32 =
    (1 << 3) | (1 << 6) | (1 << 8) | (1 << 9) | (1 << 10) | (1 << 11) | (1 << 12) | (1 << 18);
const SFX_SWING_BACK: i32 = (1 << 0)
    | (1 << 1)
    | (1 << 3)
    | (1 << 6)
    | (1 << 8)
    | (1 << 9)
    | (1 << 10)
    | (1 << 11)
    | (1 << 12)
    | (1 << 18);

/// `itChainSegment` (the chain's attributes) offsets.
const CHAIN_X48: u32 = 0x48;
const CHAIN_X4C: u32 = 0x4C;
const CHAIN_X50: u32 = 0x50;

/// The common data's easing factor (`COMMON_DATA_F32[275]`).
const COMMON_EASE: u32 = 0x44C;

/// The grounded and aerial swings' animations (passed to `ftSk_SpecialS_80110610`).
const ANIM_GROUND: i32 = 305;
const ANIM_AIR: i32 = 308;

fn u_f32(fp: &Fighter, off: usize) -> f32 {
    f32::from_bits(fp.u_i32(off) as u32)
}

fn set_u_f32(fp: &mut Fighter, off: usize, v: f32) {
    fp.set_u_i32(off, v.to_bits() as i32);
}

fn u_vec(fp: &Fighter, off: usize) -> Vec3 {
    Vec3::new(u_f32(fp, off), u_f32(fp, off + 4), u_f32(fp, off + 8))
}

fn set_u_vec(fp: &mut Fighter, off: usize, v: Vec3) {
    set_u_f32(fp, off, v.x);
    set_u_f32(fp, off + 4, v.y);
    set_u_f32(fp, off + 8, v.z);
}

/// Moves a hitbox to `pos`, off any joint, `ftColl_8007B8A8`.
#[doc(alias = "ftColl_8007B8A8")]
pub fn hit_at(hit: &mut HitCapsule, pos: Vec3) {
    hit.jobj = None;
    hit.b_offset = pos;
}

impl Running<'_> {
    /// A float of the chain's attributes (`itChainSegment`).
    fn seak_chain_attr(&self, chain: GObjId, off: u32) -> f32 {
        let ip = super::super::super::it::item(self.game, chain);
        let attrs = article(ip)
            .special(self.data)
            .expect("a chain without attributes");
        self.data.f32(attrs.at(off))
    }

    /// The stick's direction and push, eased toward (`ftSk_SpecialS_80110490`): `x18` the
    /// angle in degrees, `x14` the push.
    #[doc(alias = "ftSk_SpecialS_80110490")]
    pub fn seak_special_s_ease_stick(&mut self, gobj: GObjId) {
        let ease = reg(self.common().f32(COMMON_EASE));
        let fp = self.game.fighter_mut(gobj);
        let s = fp.input.lstick[0].clone();
        let mut a = reg(math::atan2(
            s.y,
            mem(fp::fmuls(reg(s.x), reg(fp.facing_dir))),
        ));
        if a < fp::frsp(0.0) {
            a = fp::fadds(a, fp::frsp(std::f64::consts::TAU));
        }
        let mut deg = fp::fmuls(57.295_780_181_884_766, a);
        if deg < fp::frsp(0.0) {
            deg = fp::frsp(0.0);
        }
        if deg > fp::frsp(359.0) {
            deg = fp::frsp(359.0);
        }
        let x18 = reg(fp.mv_f32(mv::S_X18));
        let mut d = fp::fsubs(deg, x18);
        if d > fp::frsp(180.0) {
            d = fp::fsubs(d, fp::frsp(360.0));
        } else if d < fp::frsp(-180.0) {
            d = fp::fadds(d, fp::frsp(360.0));
        }
        let mut v = fp::fmadds(d, ease, x18);
        if v > fp::frsp(360.0) {
            v = fp::fsubs(v, fp::frsp(360.0));
        } else if v < fp::frsp(0.0) {
            v = fp::fadds(v, fp::frsp(360.0));
        }
        fp.set_mv_f32(mv::S_X18, v);
        let sq = fp::fadds(fp::fmuls(reg(s.x), reg(s.x)), fp::fmuls(reg(s.y), reg(s.y)));
        let mut push = math::sqrt_reg(sq);
        if push > fp::frsp(1.0) {
            push = fp::frsp(1.0);
        }
        let x14 = reg(fp.mv_f32(mv::S_X14));
        fp.set_mv_f32(mv::S_X14, fp::fmadds(ease, fp::fsubs(push, x14), x14));
    }

    /// The swing's pose, `ftSk_SpecialS_80110610`: the swing animation at the eased angle's
    /// frame, blended toward the hold pose by how far the stick is pushed, then toward the
    /// state's own by `t`.
    #[doc(alias = "ftSk_SpecialS_80110610")]
    pub fn seak_special_s_pose(&mut self, gobj: GObjId, anim: i32, t: f32) {
        let f = self.game.fighter_id(gobj);
        // `ft_data->x48_items[4]` (the grounded swing) or `[5]`, its third joint.
        let ft_data = self.game.fighters[f.0]
            .ft_data
            .expect("a fighter without data");
        let items = self.data.ptr(ft_data.at(0x48)).expect("ftData x48_items");
        let slot = if anim == ANIM_GROUND { 4 } else { 5 };
        let item = self.data.ptr(items.at(4 * slot)).expect("x48_items[n]");
        let hold = self.data.ptr(item.at(8));
        self.seak_special_s_ease_stick(gobj);
        let fp = &self.game.fighters[f.0];
        let frame = mem(fp::fmadds(
            0.055_599_998_682_737_35,
            reg(fp.mv_f32(mv::S_X18)),
            4.0,
        ));
        let t = reg(t);
        if reg(fp.mv_f32(mv::S_X14)) != 0.0 {
            let skel = fp.x8AC_animSkeleton;
            let tree = self.second_figatree(f, anim);
            let tree = tree.expect("ftAnim_8006F4C8 reads a missing figatree at address 0");
            self.bind_own_figatree(f, true, tree);
            self.request_anim(skel, frame);
            let costume = self.game.fighters[f.0].x108_costume_joint;
            let child = costume.and_then(|c| self.data.ptr(c.at(8)));
            self.reset_parts(f, TRANS_N, child, true);
            jobj::anim_all(
                &mut self.game.objs,
                self.data,
                &mut crate::game::code::ParticleHooks,
                skel,
            );
            let push = reg(self.game.fighters[f.0].mv_f32(mv::S_X14));
            if push < fp::frsp(1.0) {
                self.blend_parts_to_desc(f, TRANS_N, fp::fsubs(1.0, push), push, hold, true);
            }
            if t < fp::frsp(1.0) {
                self.blend_parts(f, TRANS_N, t, fp::fsubs(1.0, t));
                return;
            }
            self.copy_parts(f, TRANS_N);
            return;
        }
        if t < fp::frsp(1.0) {
            self.blend_parts_to_desc(f, TRANS_N, t, fp::fsubs(1.0, t), hold, false);
            return;
        }
        self.reset_parts(f, TRANS_N, hold, false);
    }

    /// The swing's input, `ftSk_SpecialS_80110788`: the stick's motion since last frame
    /// (`lstick_delta`), a swing's sound for a flick forward or up and for one back, and the
    /// motion halved while the stick rests within the chain's dead zone.
    #[doc(alias = "ftSk_SpecialS_80110788")]
    pub fn seak_special_s_input(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        let (s0, s1) = (fp.input.lstick[0].clone(), fp.input.lstick[1].clone());
        let dx = mem(fp::fsubs(reg(s0.x), reg(s1.x)));
        let dy = mem(fp::fsubs(reg(s0.y), reg(s1.y)));
        set_u_f32(fp, u::LSTICK_DELTA, dx);
        set_u_f32(fp, u::LSTICK_DELTA + 4, dy);
        let facing = reg(fp.facing_dir);
        let (dx, dy) = (reg(dx), reg(dy));
        let x8 = fp.mv_i32(mv::S_X8);
        if x8 > 0 {
            fp.set_mv_i32(mv::S_X8, x8 - 1);
        } else if (facing == 1.0 && dx > reg(0.3)) || (facing == -1.0 && dx < reg(-0.3)) {
            self.fighter_play_sfx(f, SFX_SWING, 127, 64);
            self.game.fighters[f.0].set_mv_i32(mv::S_X8, 6);
        } else if dy > reg(0.5) {
            self.fighter_play_sfx(f, SFX_SWING, 127, 64);
            self.game.fighters[f.0].set_mv_i32(mv::S_X8, 12);
        }
        let fp = &mut self.game.fighters[f.0];
        let xc = fp.mv_i32(mv::S_XC);
        if xc > 0 {
            fp.set_mv_i32(mv::S_XC, xc - 1);
        } else {
            let sx = reg(fp.input.lstick[0].x);
            if (facing == 1.0 && dx < reg(-0.3) && sx < 0.0)
                || (facing == -1.0 && dx > reg(0.3) && sx > 0.0)
            {
                self.fighter_play_sfx(f, SFX_SWING_BACK, 127, 64);
                self.game.fighters[f.0].set_mv_i32(mv::S_XC, 6);
            }
        }
        let Some(chain) = self.seak_chain(gobj) else {
            return;
        };
        let dead = reg(self.seak_chain_attr(chain, CHAIN_X48));
        let fp = &mut self.game.fighters[f.0];
        let s = fp.input.lstick[0].clone();
        let ax = if reg(s.x) < 0.0 {
            fp::fneg(reg(s.x))
        } else {
            reg(s.x)
        };
        if ax < dead {
            let ay = if reg(s.y) < 0.0 {
                fp::fneg(reg(s.y))
            } else {
                reg(s.y)
            };
            if ay < dead {
                let dx = reg(u_f32(fp, u::LSTICK_DELTA));
                set_u_f32(fp, u::LSTICK_DELTA, mem(fp::fmuls(dx, 0.5)));
                let dy = reg(u_f32(fp, u::LSTICK_DELTA + 4));
                set_u_f32(fp, u::LSTICK_DELTA + 4, mem(fp::fmuls(dy, 0.5)));
            }
        }
    }

    /// The chain states' accessory, `ftSk_SpecialS_8011097C`: nothing.
    #[doc(alias = "ftSk_SpecialS_8011097C")]
    pub fn seak_special_s_nothing(&mut self, _gobj: GObjId) {}

    /// Puts the chain's hitbox `id` at `pos`, while the script has them on (`cmd_vars[0]`),
    /// `ftSk_SpecialS_UpdateHitboxes` (the chain item calls it).
    #[doc(alias = "ftSk_SpecialS_UpdateHitboxes")]
    pub fn seak_special_s_update_hitbox(&mut self, gobj: Option<GObjId>, pos: Vec3, id: i32) {
        let Some(gobj) = gobj else { return };
        let fp = self.game.fighter_mut(gobj);
        if fp.cmd_vars[0] == 0 {
            return;
        }
        let id = id as usize;
        set_u_vec(fp, u::HIT_POS + 12 * id, pos);
        if reg(pos.x) != 0.0 || reg(pos.y) != 0.0 {
            let hit = &mut fp.x914[id];
            hit_at(hit, pos);
            hit.x4C = pos;
            if reg(hit.x58.x) == 0.0 && reg(hit.x58.y) == 0.0 && reg(hit.x58.z) == 0.0 {
                hit.x58 = pos;
            }
        }
    }

    /// `ftSk_SpecialS_ZeroHitboxPositions`.
    #[doc(alias = "ftSk_SpecialS_ZeroHitboxPositions")]
    pub fn seak_special_s_zero_hitbox_positions(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        for hit in &mut fp.x914 {
            hit.x58 = Vec3::new(0.0, 0.0, 0.0);
            hit.x4C = Vec3::new(0.0, 0.0, 0.0);
        }
    }

    /// Switches the chain's hitboxes on, `ftSk_SpecialS_80110AEC`.
    #[doc(alias = "ftSk_SpecialS_80110AEC")]
    pub fn seak_special_s_hitboxes_on(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        for hit in &mut fp.x914 {
            hitcapsule::enable(hit);
        }
        self.seak_special_s_zero_hitbox_positions(gobj);
        self.game.fighter_mut(gobj).x2219_b3 = true;
    }

    /// `ftSeakSpecialS_LoopChainHitCollisions`: the hitboxes off, their victims forgotten.
    fn seak_special_s_hitboxes_off(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        for hit in &mut fp.x914 {
            hitcapsule::clear_victims(hit);
            hitcapsule::disable(hit);
        }
        self.seak_special_s_zero_hitbox_positions(gobj);
    }

    /// The chain's hitboxes by its speed, `ftSk_SpecialS_80110BCC`: on for `x18` frames once
    /// any of its points moves farther than the chain's `x4C` in a frame, off once none does
    /// (after `x20`'s grace frames).
    #[doc(alias = "ftSk_SpecialS_80110BCC")]
    pub fn seak_special_s_chain_hits(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let Some(chain) = self.seak_chain(gobj) else {
            return;
        };
        let fp = &mut self.game.fighters[f.0];
        let mut sums = [0.0f64; 4];
        for (i, sum) in sums.iter_mut().enumerate() {
            let cur = u_vec(fp, u::HIT_POS + 12 * i);
            let prev = u_vec(fp, u::HIT_POS_PREV + 12 * i);
            let x = fp::fsubs(reg(cur.x), reg(prev.x));
            let y = fp::fsubs(reg(cur.y), reg(prev.y));
            *sum = fp::fadds(fp::fmuls(x, x), fp::fmuls(y, y));
            set_u_f32(fp, u::HIT_POS_PREV + 12 * i, cur.x);
            set_u_f32(fp, u::HIT_POS_PREV + 12 * i + 4, cur.y);
        }
        let x1c = fp.mv_i32(mv::S_X1C);
        if x1c > 0 {
            fp.set_mv_i32(mv::S_X1C, x1c - 1);
            if x1c - 1 == 0 {
                self.seak_special_s_hitboxes_off(gobj);
            }
        }
        let v = reg(self.seak_chain_attr(chain, CHAIN_X4C));
        let v2 = fp::fmuls(v, v);
        if sums.iter().any(|&s| s > v2) {
            if self.game.fighters[f.0].mv_i32(mv::S_X1C) <= 0 {
                let frames = fp::fctiwz(reg(self.seak_attr_f32(f, attr::X18)));
                self.game.fighters[f.0].set_mv_i32(mv::S_X1C, frames);
                self.seak_special_s_hitboxes_on(gobj);
            }
        } else {
            let fp = &mut self.game.fighters[f.0];
            let x20 = fp.mv_i32(mv::S_X20);
            if x20 > 0 {
                fp.set_mv_i32(mv::S_X20, x20 - 1);
            } else {
                fp.set_mv_i32(mv::S_X1C, 0);
                self.seak_special_s_hitboxes_off(gobj);
            }
        }
    }

    /// The chain's own goodbye, `ftSk_SpecialS_80110E4C` (the chain item calls it as it goes):
    /// no chain, no damage hook.
    #[doc(alias = "ftSk_SpecialS_80110E4C")]
    pub fn seak_special_s_chain_gone(&mut self, gobj: GObjId) {
        self.seak_special_s_chain_resume(gobj);
        self.set_seak_chain(gobj, None);
        let fp = self.game.fighter_mut(gobj);
        fp.death2_cb = None;
        fp.take_dmg_cb = None;
    }

    /// Destroys the chain if Sheik has it out, `ftSk_SpecialS_CheckAndDestroyChain`.
    #[doc(alias = "ftSk_SpecialS_CheckAndDestroyChain")]
    pub fn seak_check_and_destroy_chain(&mut self, gobj: GObjId) {
        let Some(chain) = self.seak_chain(gobj) else {
            return;
        };
        self.seak_ext(ext::IT_802BB20C, &[Val::GObj(Some(chain))]);
        self.seak_special_s_chain_resume(gobj);
        self.set_seak_chain(gobj, None);
        let fp = self.game.fighter_mut(gobj);
        fp.death2_cb = None;
        fp.take_dmg_cb = None;
    }

    /// Hitlag starts: the chain pauses, `ftSk_SpecialS_80110EE8`.
    #[doc(alias = "ftSk_SpecialS_80110EE8")]
    pub fn seak_special_s_chain_pause(&mut self, gobj: GObjId) {
        if let Some(chain) = self.seak_chain(gobj) {
            self.seak_ext(ext::IT_802BAEEC, &[Val::GObj(Some(chain))]);
        }
    }

    /// Hitlag ends: the chain resumes, with two frames' grace for its hitboxes,
    /// `ftSk_SpecialS_ChainSomething`.
    #[doc(alias = "ftSk_SpecialS_ChainSomething")]
    pub fn seak_special_s_chain_resume(&mut self, gobj: GObjId) {
        if let Some(chain) = self.seak_chain(gobj) {
            self.seak_ext(ext::IT_802BAF0C, &[Val::GObj(Some(chain))]);
            self.game.fighter_mut(gobj).set_mv_i32(mv::S_X20, 2);
        }
    }

    /// The stick's x, for the chain, `ftSk_SpecialS_80110F58`.
    #[doc(alias = "ftSk_SpecialS_80110F58")]
    pub fn seak_stick_x(&self, gobj: GObjId) -> f32 {
        self.game.fighter(gobj).input.lstick[0].x
    }

    /// The stick's y, for the chain, `ftSk_SpecialS_80110F64`.
    #[doc(alias = "ftSk_SpecialS_80110F64")]
    pub fn seak_stick_y(&self, gobj: GObjId) -> f32 {
        self.game.fighter(gobj).input.lstick[0].y
    }

    /// The chain's variables cleared, `ftSk_SpecialS_80110F70`.
    #[doc(alias = "ftSk_SpecialS_80110F70")]
    pub fn seak_special_s_init(&mut self, gobj: GObjId) {
        self.set_seak_chain(gobj, None);
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.set_mv_i32(mv::S_X0, 0);
        fp.set_mv_i32(mv::S_X4, 0);
        fp.set_mv_i32(mv::S_X8, 0);
        fp.set_mv_i32(mv::S_XC, 0);
        fp.set_mv_f32(mv::S_X10, 0.0);
        fp.set_mv_f32(mv::S_X18, 4.0);
        fp.set_mv_f32(mv::S_X14, 0.0);
        fp.set_mv_i32(mv::S_X1C, 0);
        fp.set_mv_i32(mv::S_X20, 0);
        for i in 0..4 {
            set_u_vec(fp, u::HIT_POS + 12 * i, Vec3::new(0.0, 0.0, 0.0));
            set_u_vec(fp, u::HIT_POS_PREV + 12 * i, Vec3::new(0.0, 0.0, 0.0));
        }
        set_u_vec(fp, u::LSTICK_DELTA, Vec3::new(0.0, 0.0, 0.0));
        fp.x2222_b2 = true;
        fp.accessory4_cb = Some(CodeRef(addr::S_8011097C));
    }

    /// The grounded side special, `ftSk_SpecialS_Enter`.
    #[doc(alias = "ftSk_SpecialS_Enter")]
    pub fn seak_special_s_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_S_START, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.seak_special_s_init(gobj);
    }

    /// The aerial side special, `ftSk_SpecialAirS_Enter`: the fall stopped.
    #[doc(alias = "ftSk_SpecialAirS_Enter")]
    pub fn seak_special_air_s_enter(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).self_vel.y = 0.0;
        self.change_motion_state(gobj, ms::SPECIAL_AIR_S_START, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.seak_special_s_init(gobj);
    }

    /// The damage hook and hitlag hooks the chain's states hold while it is out.
    fn seak_special_s_hooks(&mut self, gobj: GObjId) {
        let has_chain = self.seak_chain(gobj).is_some();
        let fp = self.game.fighter_mut(gobj);
        if has_chain {
            fp.death2_cb = Some(CodeRef(addr::DAMAGE_CB));
            fp.take_dmg_cb = Some(CodeRef(addr::DAMAGE_CB));
        }
        fp.accessory4_cb = Some(CodeRef(addr::S_8011097C));
        fp.pre_hitlag_cb = Some(CodeRef(addr::S_80110EE8));
        fp.post_hitlag_cb = Some(CodeRef(addr::S_CHAIN_SOMETHING));
    }

    /// `ftSk_SpecialS_SpawnChain`: the chain from the hand, held as Sheik's special item.
    fn seak_special_s_spawn_chain(&mut self, gobj: GObjId) {
        let j = self.bone_joint(gobj, PART_L3RD_NA);
        let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
        let facing = self.game.fighter(gobj).facing_dir;
        let v = self.seak_ext(
            ext::IT_SEAK_CHAIN_SPAWN,
            &[
                Val::GObj(Some(gobj)),
                Val::Vec(pos),
                Val::Float(reg(facing)),
            ],
        );
        let chain = super::super::fox::ret_gobj(v);
        self.set_seak_chain(gobj, chain);
        let fp = self.game.fighter_mut(gobj);
        fp.x1984_heldItemSpec = chain;
        if chain.is_some() {
            fp.death2_cb = Some(CodeRef(addr::DAMAGE_CB));
            fp.take_dmg_cb = Some(CodeRef(addr::DAMAGE_CB));
        }
        fp.pre_hitlag_cb = Some(CodeRef(addr::S_80110EE8));
        fp.post_hitlag_cb = Some(CodeRef(addr::S_CHAIN_SOMETHING));
    }

    /// The draw's frames, `ftSk_SpecialS_CheckInitChain`: the chain out on frame `x1C` (or
    /// the move ends without one), thrown out the next at its speed; whether the draw is over
    /// (past `x20`).
    #[doc(alias = "ftSk_SpecialS_CheckInitChain")]
    pub fn seak_special_s_check_init_chain(&mut self, gobj: GObjId) -> bool {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::S_X0).wrapping_add(1);
        fp.set_mv_i32(mv::S_X0, n);
        let x1c = reg(self.seak_attr_f32(f, attr::X1C));
        if fp::frsp(f64::from(n)) == x1c {
            self.seak_special_s_spawn_chain(gobj);
            let frames = fp::fctiwz(reg(self.seak_attr_f32(f, attr::X18)));
            self.game.fighters[f.0].set_mv_i32(mv::S_X1C, frames);
            if self.seak_chain(gobj).is_none() {
                if self.game.fighters[f.0].ground_or_air == GA_AIR {
                    self.enter_fall(gobj);
                } else {
                    self.enter_wait(gobj);
                }
            }
        }
        let n = self.game.fighters[f.0].mv_i32(mv::S_X0);
        if fp::frsp(f64::from(n)) == fp::fadds(x1c, 1.0) {
            let chain = self
                .seak_chain(gobj)
                .expect("a thrown chain without a chain");
            let speed = reg(self.seak_chain_attr(chain, CHAIN_X50));
            let facing = reg(super::super::super::it::item(self.game, chain).facing_dir);
            let vel = Vec3::new(mem(fp::fmuls(speed, facing)), 0.0, 0.0);
            self.seak_ext(ext::IT_802BCFC4, &[Val::GObj(Some(chain)), Val::Vec(vel)]);
        }
        let n = self.game.fighters[f.0].mv_i32(mv::S_X0);
        fp::frsp(f64::from(n)) > reg(self.seak_attr_f32(f, attr::X20))
    }

    /// `ftSk_SpecialSStart_Anim`: the swing once the draw ends.
    #[doc(alias = "ftSk_SpecialSStart_Anim")]
    pub fn seak_special_s_start_anim(&mut self, gobj: GObjId) {
        if self.seak_special_s_check_init_chain(gobj) {
            self.seak_special_s_to_swing(gobj);
        }
    }

    /// `ftSk_SpecialAirSStart_Anim`.
    #[doc(alias = "ftSk_SpecialAirSStart_Anim")]
    pub fn seak_special_air_s_start_anim(&mut self, gobj: GObjId) {
        if self.seak_special_s_check_init_chain(gobj) {
            self.seak_special_air_s_to_swing(gobj);
        }
    }

    /// The states' empty IASA callbacks.
    #[doc(alias = "ftSk_SpecialSStart_IASA")]
    #[doc(alias = "ftSk_SpecialAirSStart_IASA")]
    #[doc(alias = "ftSk_SpecialSEnd_IASA")]
    #[doc(alias = "ftSk_SpecialAirSEnd_IASA")]
    pub fn seak_special_s_no_iasa(&mut self, _gobj: GObjId) {}

    /// `ftSk_SpecialAirSStart_Phys`: falling once the script says (`cmd_vars[0]`), slowing.
    #[doc(alias = "ftSk_SpecialAirSStart_Phys")]
    pub fn seak_special_air_s_start_phys(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.cmd_vars[0] != 0 {
            let (g, t) = (reg(fp.co_attrs.gravity), reg(fp.co_attrs.terminal_velocity));
            Self::fall(fp, g, t);
        }
        let fr = reg(fp.co_attrs.aerial_friction);
        Self::self_accel_deaccel(fp, fr);
    }

    /// `ftSk_SpecialSStart_Coll`.
    #[doc(alias = "ftSk_SpecialSStart_Coll")]
    pub fn seak_special_s_start_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.seak_special_s_start_to_air(gobj);
        }
    }

    /// `ftSk_SpecialAirSStart_Coll`.
    #[doc(alias = "ftSk_SpecialAirSStart_Coll")]
    pub fn seak_special_air_s_start_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.seak_special_s_start_to_ground(gobj);
        }
    }

    /// `ftSk_SpecialS_80111440`.
    #[doc(alias = "ftSk_SpecialS_80111440")]
    pub fn seak_special_s_start_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_S_START, TRANSITION_FLAGS);
        self.seak_special_s_hooks(gobj);
    }

    /// `ftSk_SpecialS_801114E4`.
    #[doc(alias = "ftSk_SpecialS_801114E4")]
    pub fn seak_special_s_start_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_S_START, TRANSITION_FLAGS);
        self.seak_special_s_hooks(gobj);
    }

    /// The swing's frames: past `x14` with B let go, the retraction.
    fn seak_special_s_swing_tick(&mut self, gobj: GObjId) -> bool {
        self.seak_special_s_chain_hits(gobj);
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::S_X0).wrapping_add(1);
        fp.set_mv_i32(mv::S_X0, n);
        let x14 = reg(self.seak_attr_f32(f, attr::X14));
        let fp = &mut self.game.fighters[f.0];
        if fp::frsp(f64::from(n)) > x14 && fp.mv_i32(mv::S_X4) != 0 {
            fp.set_mv_i32(mv::S_X4, 0);
            return true;
        }
        false
    }

    /// `ftSk_SpecialS_Anim`.
    #[doc(alias = "ftSk_SpecialS_Anim")]
    pub fn seak_special_s_anim(&mut self, gobj: GObjId) {
        if self.seak_special_s_swing_tick(gobj) {
            self.seak_special_s_to_end(gobj);
            return;
        }
        self.seak_special_s_pose(gobj, ANIM_GROUND, 1.0);
    }

    /// `ftSk_SpecialAirS_Anim`.
    #[doc(alias = "ftSk_SpecialAirS_Anim")]
    pub fn seak_special_air_s_anim(&mut self, gobj: GObjId) {
        if self.seak_special_s_swing_tick(gobj) {
            self.seak_special_air_s_to_end(gobj);
            return;
        }
        self.seak_special_s_pose(gobj, ANIM_AIR, 1.0);
    }

    /// The swing's IASA, `ftSk_SpecialS_IASA` and `ftSk_SpecialAirS_IASA`: B let go marks the
    /// retraction, and the stick swings.
    #[doc(alias = "ftSk_SpecialS_IASA")]
    #[doc(alias = "ftSk_SpecialAirS_IASA")]
    pub fn seak_special_s_iasa(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.input.held_buttons[0] & PAD_B == 0 {
            fp.set_mv_i32(mv::S_X4, 1);
        }
        self.seak_special_s_input(gobj);
    }

    /// `ftSk_SpecialS_Coll`.
    #[doc(alias = "ftSk_SpecialS_Coll")]
    pub fn seak_special_s_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.seak_special_s_to_end(gobj);
        }
    }

    /// `ftSk_SpecialAirS_Coll`.
    #[doc(alias = "ftSk_SpecialAirS_Coll")]
    pub fn seak_special_air_s_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.seak_special_air_s_to_end(gobj);
        }
    }

    /// Into the swing on the ground, `ftSk_SpecialS_80111830`: the hitboxes on, and the
    /// retraction at once if the stage lies between Sheik's middle and her hand.
    #[doc(alias = "ftSk_SpecialS_80111830")]
    pub fn seak_special_s_to_swing(&mut self, gobj: GObjId) {
        self.change_motion_state(
            gobj,
            ms::SPECIAL_S,
            super::super::motion::flags::SKIP_HIT,
            0.0,
            1.0,
            0.0,
            None,
        );
        self.seak_special_s_pose(gobj, ANIM_GROUND, 0.0);
        self.seak_special_s_swing_common(gobj);
        let fp = self.game.fighter(gobj);
        let top = reg(fp.coll_data.ecb.top.y);
        let bottom = reg(fp.coll_data.ecb.bottom.y);
        let mid = Vec3::new(
            mem(fp::fadds(0.0, reg(fp.cur_pos.x))),
            mem(fp::fadds(
                fp::fmuls(0.5, fp::fadds(top, bottom)),
                reg(fp.cur_pos.y),
            )),
            mem(fp::fadds(0.0, reg(fp.cur_pos.z))),
        );
        let j = self.bone_joint(gobj, PART_L3RD_NA);
        let hand = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
        let flags = self
            .seak_ext(
                super::ext::FTNS_YOYO_CHECK_ENV_COLL,
                &[
                    Val::GObj(Some(gobj)),
                    Val::Vec(mid),
                    Val::Vec(hand),
                    Val::Float(0.5),
                ],
            )
            .int();
        if flags != 0 {
            self.seak_special_s_to_end(gobj);
        }
    }

    /// Into the swing in the air, `ftSk_SpecialS_80111988`.
    #[doc(alias = "ftSk_SpecialS_80111988")]
    pub fn seak_special_air_s_to_swing(&mut self, gobj: GObjId) {
        self.change_motion_state(
            gobj,
            ms::SPECIAL_AIR_S,
            super::super::motion::flags::SKIP_HIT,
            0.0,
            1.0,
            0.0,
            None,
        );
        self.seak_special_s_pose(gobj, ANIM_AIR, 0.0);
        self.seak_special_s_swing_common(gobj);
    }

    /// The swing's start: its frames from zero, the hitboxes on, the hooks.
    fn seak_special_s_swing_common(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).set_mv_i32(mv::S_X0, 0);
        self.seak_special_s_hitboxes_on(gobj);
        self.seak_special_s_hooks(gobj);
        self.game.fighter_mut(gobj).x2222_b2 = true;
    }

    /// The retraction's frames, `ftSk_SpecialSEnd_Anim` and `ftSk_SpecialAirSEnd_Anim`: the
    /// chain pulled in on frame `x24`, gone on `x28`, its hitboxes by its speed meanwhile.
    fn seak_special_s_end_tick(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::S_X0).wrapping_add(1);
        fp.set_mv_i32(mv::S_X0, n);
        let n = fp::frsp(f64::from(n));
        let x28 = reg(self.seak_attr_f32(f, attr::X28));
        if n < x28 {
            let chain = self.seak_chain(gobj);
            if n == reg(self.seak_attr_f32(f, attr::X24)) {
                self.seak_ext(ext::IT_802BCF84, &[Val::GObj(chain)]);
            }
            self.seak_special_s_chain_hits(gobj);
        } else if n == x28 {
            let chain = self.seak_chain(gobj);
            self.seak_ext(ext::IT_802BB20C, &[Val::GObj(chain)]);
        } else {
            self.seak_special_s_chain_hits(gobj);
        }
    }

    /// `ftSk_SpecialSEnd_Anim`.
    #[doc(alias = "ftSk_SpecialSEnd_Anim")]
    pub fn seak_special_s_end_anim(&mut self, gobj: GObjId) {
        self.seak_special_s_end_tick(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftSk_SpecialAirSEnd_Anim`.
    #[doc(alias = "ftSk_SpecialAirSEnd_Anim")]
    pub fn seak_special_air_s_end_anim(&mut self, gobj: GObjId) {
        self.seak_special_s_end_tick(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftSk_SpecialSEnd_Coll`.
    #[doc(alias = "ftSk_SpecialSEnd_Coll")]
    pub fn seak_special_s_end_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.seak_special_s_end_to_air(gobj);
        }
    }

    /// `ftSk_SpecialAirSEnd_Coll`.
    #[doc(alias = "ftSk_SpecialAirSEnd_Coll")]
    pub fn seak_special_air_s_end_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.seak_special_s_end_to_ground(gobj);
        }
    }

    /// `ftSk_SpecialS_80111CB0`.
    #[doc(alias = "ftSk_SpecialS_80111CB0")]
    pub fn seak_special_s_end_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_S_END, TRANSITION_FLAGS);
        self.seak_special_s_hooks(gobj);
    }

    /// `ftSk_SpecialS_80111D54`.
    #[doc(alias = "ftSk_SpecialS_80111D54")]
    pub fn seak_special_s_end_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_S_END, TRANSITION_FLAGS);
        self.seak_special_s_hooks(gobj);
    }

    /// Into the retraction, `ftSk_SpecialS_80111DF8` and `ftSk_SpecialS_80111EB4`: the
    /// hitboxes on again if they were.
    fn seak_special_s_to_end_as(&mut self, gobj: GObjId, msid: i32) {
        use super::super::motion::flags::SKIP_HIT;
        self.change_motion_state(gobj, msid, SKIP_HIT, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(mv::S_X0, 0);
        if fp.mv_i32(mv::S_X1C) != 0 {
            self.seak_special_s_hitboxes_on(gobj);
        }
        self.seak_special_s_hooks(gobj);
        self.game.fighter_mut(gobj).x2222_b2 = true;
    }

    #[doc(alias = "ftSk_SpecialS_80111DF8")]
    pub fn seak_special_s_to_end(&mut self, gobj: GObjId) {
        self.seak_special_s_to_end_as(gobj, ms::SPECIAL_S_END);
    }

    #[doc(alias = "ftSk_SpecialS_80111EB4")]
    pub fn seak_special_air_s_to_end(&mut self, gobj: GObjId) {
        self.seak_special_s_to_end_as(gobj, ms::SPECIAL_AIR_S_END);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::S_8011097C => g.seak_special_s_nothing(gobj),
        addr::S_ZERO_HITBOX_POSITIONS => g.seak_special_s_zero_hitbox_positions(gobj),
        0x8011_0AEC => g.seak_special_s_hitboxes_on(gobj),
        0x8011_0BCC => g.seak_special_s_chain_hits(gobj),
        0x8011_0788 => g.seak_special_s_input(gobj),
        addr::S_80110E4C => g.seak_special_s_chain_gone(gobj),
        addr::S_CHECK_AND_DESTROY_CHAIN => g.seak_check_and_destroy_chain(gobj),
        addr::S_80110EE8 => g.seak_special_s_chain_pause(gobj),
        addr::S_CHAIN_SOMETHING => g.seak_special_s_chain_resume(gobj),
        addr::S_80110F70 => g.seak_special_s_init(gobj),
        addr::SPECIAL_S_ENTER => g.seak_special_s_enter(gobj),
        addr::SPECIAL_AIR_S_ENTER => g.seak_special_air_s_enter(gobj),
        0x8011_12CC => g.seak_special_s_start_anim(gobj),
        0x8011_1308 => g.seak_special_air_s_start_anim(gobj),
        0x8011_1344 | 0x8011_1348 | 0x8011_1BF0 | 0x8011_1BF4 => g.seak_special_s_no_iasa(gobj),
        0x8011_134C | 0x8011_1778 | 0x8011_1BF8 => g.seak_ground_phys(gobj),
        0x8011_136C => g.seak_special_air_s_start_phys(gobj),
        0x8011_1798 | 0x8011_1C18 => g.seak_air_phys(gobj),
        0x8011_13C8 => g.seak_special_s_start_coll(gobj),
        0x8011_1404 => g.seak_special_air_s_start_coll(gobj),
        0x8011_1440 => g.seak_special_s_start_to_air(gobj),
        0x8011_14E4 => g.seak_special_s_start_to_ground(gobj),
        0x8011_1588 => g.seak_special_s_anim(gobj),
        0x8011_1648 => g.seak_special_air_s_anim(gobj),
        0x8011_1708 | 0x8011_1740 => g.seak_special_s_iasa(gobj),
        0x8011_17B8 => g.seak_special_s_coll(gobj),
        0x8011_17F4 => g.seak_special_air_s_coll(gobj),
        0x8011_1830 => g.seak_special_s_to_swing(gobj),
        0x8011_1988 => g.seak_special_air_s_to_swing(gobj),
        0x8011_1A48 => g.seak_special_s_end_anim(gobj),
        0x8011_1B1C => g.seak_special_air_s_end_anim(gobj),
        0x8011_1C38 => g.seak_special_s_end_coll(gobj),
        0x8011_1C74 => g.seak_special_air_s_end_coll(gobj),
        0x8011_1CB0 => g.seak_special_s_end_to_air(gobj),
        0x8011_1D54 => g.seak_special_s_end_to_ground(gobj),
        0x8011_1DF8 => g.seak_special_s_to_end(gobj),
        0x8011_1EB4 => g.seak_special_air_s_to_end(gobj),
        _ => return false,
    }
    true
}

/// The queries the chain and the needles ask of Sheik, and the side special's functions that
/// take more than the GObj.
pub(super) fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    let gobj = |i: usize| args[i].gobj();
    Some(match f {
        addr::S_80110F58 => Val::Float(reg(r.seak_stick_x(gobj(0).expect("a null fighter")))),
        addr::S_80110F64 => Val::Float(reg(r.seak_stick_y(gobj(0).expect("a null fighter")))),
        addr::S_80111F70 => Val::Int(i32::from(r.seak_lets_go(gobj(0)))),
        addr::S_80111FA0 => Val::Int(r.seak_held_count(gobj(0))),
        addr::S_CHECK_INIT_CHAIN => Val::Int(i32::from(
            r.seak_special_s_check_init_chain(gobj(0).expect("a null fighter")),
        )),
        addr::S_UPDATE_HITBOXES => {
            let Val::Vec(pos) = args[1] else {
                panic!(
                    "ftSk_SpecialS_UpdateHitboxes takes a position, not {:?}",
                    args[1]
                );
            };
            r.seak_special_s_update_hitbox(gobj(0), pos, args[2].int());
            Val::None
        }
        0x8011_0610 => {
            let Val::Float(t) = args[2] else {
                panic!("ftSk_SpecialS_80110610 takes a float, not {:?}", args[2]);
            };
            r.seak_special_s_pose(gobj(0).expect("a null fighter"), args[1].int(), mem(t));
            Val::None
        }
        _ => return None,
    })
}
