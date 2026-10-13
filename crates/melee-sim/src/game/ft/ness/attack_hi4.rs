//! The yo-yo's up smash (`ftnessattackhi4.c`) and the yo-yo helpers both smashes share: the
//! yo-yo (`it::kinds::nessyoyo`) comes out of his hand, its hitbox follows it, it can be
//! charged by holding A once it's out, and it hits again on a timer after a hit.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::nessyoyo;
use super::super::super::state::Running;
use super::super::common::GA_AIR;
use super::super::motion::flags::SKIP_ITEM_VIS;
use super::super::sound::Track;
use super::{PART_R2ND_NA, addr, attr, ms, mv, u};
use crate::fpu::{mem, reg};
use crate::game::structs::{CollData, Vec2, ftCollisionBox};
use crate::hsd::gobj::GObjId;
use crate::lb::hitcapsule;
use crate::math;
use crate::mp::coll::{
    CEILING_HUG, CEILING_PUSH, FLOOR_HUG, FLOOR_PUSH, LEFT_WALL_MASK, RIGHT_WALL_MASK,
};
use crate::mtx::Axis;
use crate::vec::Vec3;

/// `FtPart_XRotN`, and the part the yo-yo's string hangs from (`parts[61]`).
const PART_X_ROT_N: i32 = 2;
const PART_YOYO: i32 = 61;

/// `HSD_PAD_A`.
const PAD_A: u32 = 1 << 8;

/// The smash charge's color animation, and the charge's sounds: its start, its release, the
/// yo-yo's stop, and its spin (up and down).
const COLANIM_CHARGE: i32 = 119;
const SFX_CHARGE: i32 = 210_087;
const SFX_RELEASE: i32 = 210_090;
const SFX_STOP: i32 = 210_096;
const SFX_SPIN_UP: i32 = 210_099;
const SFX_SPIN_DOWN: i32 = 210_093;

/// `0.0039059999` (about 1/256) as the original's float.
const DAMAGE_SCALE: f32 = 0.003_906;

impl Running<'_> {
    /// `ftNs_AttackHi4_YoyoUpdateHitPos` (accessory 4): the first hitbox at the yo-yo once it
    /// has a position.
    #[doc(alias = "ftNs_AttackHi4_YoyoUpdateHitPos")]
    pub fn ness_yoyo_update_hit_pos(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.x914[0].state != hitcapsule::DISABLED {
            let p = u_vec(fp, u::YOYO_HITBOX_POS);
            if p.x != 0.0 || p.y != 0.0 {
                super::super::seak::special_s::hit_at(&mut fp.x914[0], p);
            }
        }
    }

    /// `ftNs_AttackHi4_YoyoCheckTimedRehit`: until the script says, the rehit timer runs out
    /// and the hitbox forgets its victims and hits again.
    #[doc(alias = "ftNs_AttackHi4_YoyoCheckTimedRehit")]
    pub fn ness_yoyo_check_timed_rehit(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.cmd_vars[0] == 0 {
            let t = fp.mv_i32(mv::YOYO_REHIT_TIMER);
            if t > 0 {
                fp.set_mv_i32(mv::YOYO_REHIT_TIMER, t - 1);
                if t - 1 == 0 {
                    hitcapsule::clear_victims(&mut fp.x914[0]);
                    hitcapsule::enable(&mut fp.x914[0]);
                }
            }
        }
    }

    /// The charged smash's damage (`ftNs_AttackHi4_YoyoApplyDamage`, inline).
    fn ness_yoyo_apply_damage(&mut self, gobj: GObjId, charge: f32) {
        if charge == 0.0 {
            return;
        }
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].x914[0].state != hitcapsule::ENABLED {
            return;
        }
        let duration = reg(self.ness_attr_f32(f, attr::YOYO_CHARGE_DURATION));
        let mul = reg(self.ness_attr_f32(f, attr::YOYO_DAMAGE_MUL));
        let t = fp::fdivs(reg(charge), duration);
        let m = fp::fmsubs(mul, reg(DAMAGE_SCALE), 1.0);
        let dmg = fp::fmuls(
            reg(self.game.fighters[f.0].x914[0].damage),
            fp::fmadds(m, t, 1.0),
        );
        self.hitbox_damage(f, 0, false, crate::ppc::cvt_fp2unsigned(dmg));
    }

    /// Whether the stage lies between `from` and `to`, `ftNs_AttackHi4_YoyoCheckEnvColl`: a
    /// diamond `size` times the fighter's height swept from one point to the other through
    /// the airborne pass on collision data of its own (on the original's stack). The result
    /// is the touch flags met: `0x8000` a floor, `1` a left wall, `0x40` a right wall,
    /// `0x2000` a ceiling.
    #[doc(alias = "ftNs_AttackHi4_YoyoCheckEnvColl")]
    pub fn ness_yoyo_check_env_coll(
        &mut self,
        gobj: GObjId,
        from: Vec3,
        to: Vec3,
        size: f64,
    ) -> i32 {
        let y = mem(fp::fmuls(size, reg(self.game.fighter(gobj).x34_scale.y)));
        let ny = mem(fp::fneg(reg(y)));
        let ecb = ftCollisionBox {
            top: y,
            bottom: ny,
            left: Vec2 { x: ny, y: 0.0 },
            right: Vec2 { x: y, y: 0.0 },
        };
        // The stack's collision data: `mpColl_80041EE4` copies its never-set position into
        // `prev_pos`, which the substeps write before they read it, and the two pushes
        // (`last_pos`, then `cur_pos`) set the move.
        let mut cd = CollData::default();
        let env = crate::game::it::maplib::with_stage_coll(self.game, self.data, |c| {
            c.init(&mut cd);
            cd.last_pos = cd.cur_pos;
            cd.cur_pos = from;
            cd.last_pos = cd.cur_pos;
            cd.cur_pos = to;
            cd.x34_flags.b1234 = 5;
            c.air_frame_box(&mut cd, &ecb, 0);
            cd.env_flags
        });
        let mut ret = 0;
        if env & (FLOOR_PUSH | FLOOR_HUG) != 0 {
            ret |= 0x8000;
        }
        if env & LEFT_WALL_MASK != 0 {
            ret |= 1;
        }
        if env & RIGHT_WALL_MASK != 0 {
            ret |= 0x40;
        }
        if env & (CEILING_HUG | CEILING_PUSH) != 0 {
            ret |= 0x2000;
        }
        ret
    }

    /// Where the yo-yo's hand is, `ftNs_AttackHi4_YoyoSetUnkPos` (and the position
    /// `ftNs_AttackHi4_YoyoSetHitPos` and `ftNs_AttackHi4_YoyoSetHitPosUnk` take): the string's
    /// joint, turned about the hip's rotation joint by the floor's slope while `isPosUpdateMod`.
    #[doc(alias = "ftNs_AttackHi4_YoyoSetUnkPos")]
    pub fn ness_yoyo_hand_pos(&mut self, gobj: GObjId) -> Vec3 {
        let j = self.bone_joint(gobj, PART_YOYO);
        let at = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
        if self.game.fighter(gobj).mv_i32(mv::YOYO_POS_UPDATE_MOD) == 0 {
            return at;
        }
        let j = self.bone_joint(gobj, PART_X_ROT_N);
        let pivot = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
        let mut pos = at - pivot;
        let n = self.game.fighter(gobj).coll_data.floor.normal;
        let angle = mem(fp::fneg(reg(math::atan2(n.x, n.y))));
        pos.rotate(Axis::Z, angle);
        pos + pivot
    }

    /// `ftNs_AttackHi4_YoyoSetHitPos`: the hitbox's position at the hand.
    #[doc(alias = "ftNs_AttackHi4_YoyoSetHitPos")]
    pub fn ness_yoyo_set_hit_pos(&mut self, gobj: GObjId) {
        let p = self.ness_yoyo_hand_pos(gobj);
        set_u_vec(self.game.fighter_mut(gobj), u::YOYO_HITBOX_POS, p);
    }

    /// `ftNs_AttackHi4_YoyoSetHitPosUnk`: the hitbox's position `t` of the way to the hand.
    #[doc(alias = "ftNs_AttackHi4_YoyoSetHitPosUnk")]
    pub fn ness_yoyo_lerp_hit_pos(&mut self, gobj: GObjId, t: f32) {
        let p = self.ness_yoyo_hand_pos(gobj);
        let fp = self.game.fighter_mut(gobj);
        let prev = u_vec(fp, u::YOYO_HITBOX_POS);
        let t = reg(t);
        let u = fp::fsubs(1.0, t);
        let lerp = |a: f32, b: f32| mem(fp::fmadds(reg(a), t, fp::fmuls(reg(b), u)));
        let v = Vec3::new(lerp(p.x, prev.x), lerp(p.y, prev.y), lerp(p.z, prev.z));
        set_u_vec(fp, u::YOYO_HITBOX_POS, v);
    }

    /// Whether the yo-yo is out in the open, `ftNs_AttackHi4_YoyoCheckNoObstruct`: nothing
    /// between Ness's middle and the yo-yo, and a floor right under the yo-yo.
    #[doc(alias = "ftNs_AttackHi4_YoyoCheckNoObstruct")]
    pub fn ness_yoyo_check_no_obstruct(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        let e = &fp.coll_data.ecb;
        let mid = Vec3::new(
            mem(fp::fadds(0.0, reg(fp.cur_pos.x))),
            mem(fp::fadds(
                fp::fmuls(0.5, fp::fadds(reg(e.top.y), reg(e.bottom.y))),
                reg(fp.cur_pos.y),
            )),
            mem(fp::fadds(0.0, reg(fp.cur_pos.z))),
        );
        let yoyo = u_vec(fp, u::YOYO_HITBOX_POS);
        if self.ness_yoyo_check_env_coll(gobj, mid, yoyo, 0.5) != 0 {
            return false;
        }
        let fp = self.game.fighter(gobj);
        let mut above = u_vec(fp, u::YOYO_HITBOX_POS);
        let mut below = above;
        above.y = mem(fp::fadds(reg(above.y), reg(fp.x34_scale.y)));
        below.y = mem(fp::fmadds(fp::fneg(1.0), reg(fp.x34_scale.y), reg(below.y)));
        self.ness_yoyo_check_env_coll(gobj, above, below, 1.5) & 0x8000 != 0
    }

    /// The smashes' variables at their start, `ftNs_AttackHi4_YoyoSetVarAll`.
    #[doc(alias = "ftNs_AttackHi4_YoyoSetVarAll")]
    pub fn ness_yoyo_set_var_all(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[1] = 0;
        fp.cmd_vars[0] = 0;
        fp.set_mv_i32(mv::YOYO_CURRENT_FRAME, 1);
        fp.set_mv_i32(mv::YOYO_REHIT_TIMER, 0);
        fp.set_mv_i32(mv::YOYO_POS_UPDATE_MOD, 1);
        set_u_vec(fp, u::YOYO_HITBOX_POS, Vec3::new(0.0, 0.0, 0.0));
        fp.set_u_i32(u::X223C, 0);
    }

    /// The charge's start, `ftNs_AttackHi4_YoyoApplySmash`: the yo-yo let go forward, and the
    /// smash charge's state, color, rumble and sound.
    #[doc(alias = "ftNs_AttackHi4_YoyoApplySmash")]
    pub fn ness_yoyo_apply_smash(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].set_mv_i32(mv::YOYO_CURRENT_FRAME, 0);
        if let Some(yoyo) = self.game.fighters[f.0].u.gobj_at(u::YOYO) {
            let vx = nessyoyo::attr_f32(self, yoyo, nessyoyo::attr::RELEASE_VEL_X);
            let vy = nessyoyo::attr_f32(self, yoyo, nessyoyo::attr::RELEASE_VEL_Y);
            let facing = reg(self.game.fighters[f.0].facing_dir);
            let v = Vec3::new(mem(fp::fmuls(reg(vx), facing)), vy, 0.0);
            nessyoyo::release(self, yoyo, v);
        }
        let rows = self.data.u32(self.load_common(0x2C).at(4)) as u8;
        let fp = &mut self.game.fighters[f.0];
        let s = &mut fp.smash_attrs;
        s.state = 4;
        s.x2124_frameSpeedMul = fp.frame_speed_mul;
        let s = &mut fp.smash_attrs;
        s.x2118_frames = 0.0;
        s.x211C_holdFrame = 60.0;
        s.x212C = 0;
        s.x212D = rows;
        s.x2128 = COLANIM_CHARGE;
        if s.x2128 != 123 {
            self.start_color_anim(gobj, COLANIM_CHARGE, 0);
        }
        self.start_rumble(f, 36, 0);
        self.fighter_play_track(f, Track::Effect2, SFX_CHARGE, 127, 64);
    }

    /// The release, `ftNs_AttackHi4_YoyoSetChargeDamage`: the yo-yo spinning out again and
    /// the hitbox's damage by the charge.
    #[doc(alias = "ftNs_AttackHi4_YoyoSetChargeDamage")]
    pub fn ness_yoyo_set_charge_damage(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].set_mv_i32(mv::YOYO_CURRENT_FRAME, 14);
        if let Some(yoyo) = self.game.fighters[f.0].u.gobj_at(u::YOYO) {
            nessyoyo::spin_out(self, yoyo, Vec3::new(0.0, 0.0, 0.0));
            let spin = nessyoyo::attr_f32(self, yoyo, nessyoyo::attr::SPIN_TEXANIM_SPEED);
            self.game.fighters[f.0].set_u_i32(u::X223C, spin.to_bits() as i32);
        }
        self.fighter_play_track(f, Track::Effect2, SFX_RELEASE, 127, 64);
        let charge = self.game.fighters[f.0].smash_attrs.x2118_frames;
        self.ness_yoyo_apply_damage(gobj, charge);
    }

    /// The yo-yo's frame, `ftNs_AttackHi4_YoyoThink_IsRemove`: it comes out on frame 2, spins
    /// or stops as the script says, turns and is nudged on its frames, and goes on the last
    /// (49 up, 60 down). Whether the state ended (no yo-yo came out).
    #[doc(alias = "ftNs_AttackHi4_YoyoThink_IsRemove")]
    pub fn ness_yoyo_think(&mut self, gobj: GObjId) -> bool {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[0] != 0 {
            self.game.fighters[f.0].set_mv_i32(mv::YOYO_POS_UPDATE_MOD, 0);
        }
        let msid = self.game.fighters[f.0].motion_id;
        let is_up = (ms::ATTACK_HI4..=ms::ATTACK_HI4_RELEASE).contains(&msid);
        let (spawn, despawn) = (2, if is_up { 49 } else { 60 });
        let frame = self.game.fighters[f.0].mv_i32(mv::YOYO_CURRENT_FRAME);
        if frame > spawn && frame <= despawn {
            let yoyo = self.game.fighters[f.0]
                .u
                .gobj_at(u::YOYO)
                .expect("the yo-yo's frames without a yo-yo");
            if self.game.fighters[f.0].cmd_vars[1] == 1 {
                self.game.fighters[f.0].cmd_vars[1] = 0;
                nessyoyo::set_spin(self, yoyo, 1.0);
                let spin = nessyoyo::attr_f32(self, yoyo, nessyoyo::attr::SPIN_TEXANIM_SPEED);
                self.game.fighters[f.0].set_u_i32(u::X223C, spin.to_bits() as i32);
            }
            if self.game.fighters[f.0].cmd_vars[1] == 2 {
                self.game.fighters[f.0].cmd_vars[1] = 0;
                nessyoyo::set_spin(self, yoyo, 0.0);
                self.game.fighters[f.0].set_u_i32(u::X223C, 0);
                self.fighter_play_track(f, Track::Effect2, SFX_STOP, 127, 64);
            }
            let msid = self.game.fighters[f.0].motion_id;
            let is_up = (ms::ATTACK_HI4..=ms::ATTACK_HI4_RELEASE).contains(&msid);
            let (rot, nudge) = if is_up {
                (
                    nessyoyo::attr_i32(self, yoyo, nessyoyo::attr::UPSMASH_YOYO_ROT_FRAME),
                    nessyoyo::attr_i32(self, yoyo, nessyoyo::attr::UPSMASH_YOYO_NUDGE_FRAME),
                )
            } else {
                (
                    nessyoyo::attr_i32(self, yoyo, nessyoyo::attr::DOWNSMASH_YOYO_ROT_FRAME),
                    nessyoyo::attr_i32(self, yoyo, nessyoyo::attr::DOWNSMASH_YOYO_NUDGE_FRAME),
                )
            };
            let frame = self.game.fighters[f.0].mv_i32(mv::YOYO_CURRENT_FRAME);
            if frame == rot {
                nessyoyo::spin_out(self, yoyo, Vec3::new(0.0, 0.0, 0.0));
                let msid = self.game.fighters[f.0].motion_id;
                let sfx = if (ms::ATTACK_HI4..=ms::ATTACK_HI4_RELEASE).contains(&msid) {
                    SFX_SPIN_UP
                } else {
                    SFX_SPIN_DOWN
                };
                self.fighter_play_track(f, Track::Effect2, sfx, 127, 64);
            } else if frame == nudge {
                nessyoyo::nudge(self, yoyo);
            }
            if self.game.fighters[f.0].mv_i32(mv::YOYO_CURRENT_FRAME) == despawn {
                let y = self.game.fighters[f.0]
                    .u
                    .gobj_at(u::YOYO)
                    .expect("no yo-yo to remove");
                nessyoyo::remove(self, y);
            }
        } else if frame == spawn {
            self.ness_yoyo_create_item(gobj);
            if self.game.fighters[f.0].u.gobj_at(u::YOYO).is_none() {
                self.enter_wait(gobj);
                return true;
            }
        }
        false
    }

    /// The yo-yo's spin rate through the charge, `ftNs_AttackHi4_YoyoSetUnkRate`: from the
    /// texture animation's base toward its speed.
    #[doc(alias = "ftNs_AttackHi4_YoyoSetUnkRate")]
    pub fn ness_yoyo_set_unk_rate(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let Some(yoyo) = self.game.fighters[f.0].u.gobj_at(u::YOYO) else {
            return;
        };
        let duration = reg(self.ness_attr_f32(f, attr::YOYO_CHARGE_DURATION));
        let modv = reg(nessyoyo::attr_f32(
            self,
            yoyo,
            nessyoyo::attr::UNK_TEXANIM_MOD,
        ));
        let speed = reg(nessyoyo::attr_f32(
            self,
            yoyo,
            nessyoyo::attr::UNK_TEXANIM_SPEED,
        ));
        let frame = fp::frsp(f64::from(
            self.game.fighters[f.0].mv_i32(mv::YOYO_CURRENT_FRAME),
        ));
        let x = fp::fmuls(fp::fsubs(modv, speed), fp::fdivs(frame, duration));
        let v = mem(fp::fsubs(modv, x));
        self.game.fighters[f.0].set_u_i32(u::X223C, v.to_bits() as i32);
    }

    /// The yo-yo out of his hand, `ftNs_AttackHi4_YoyoCreateItem`: held as his special item,
    /// his damage hooks letting go of it, and the hitlag hooks pausing it.
    #[doc(alias = "ftNs_AttackHi4_YoyoCreateItem")]
    pub fn ness_yoyo_create_item(&mut self, gobj: GObjId) {
        let j = self.bone_joint(gobj, PART_R2ND_NA);
        let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
        let fp = self.game.fighter(gobj);
        let (facing, msid) = (fp.facing_dir, fp.motion_id);
        let yoyo = nessyoyo::spawn(self, gobj, facing, pos, msid);
        let fp = self.game.fighter_mut(gobj);
        fp.u.set_gobj_at(u::YOYO, yoyo);
        fp.x1984_heldItemSpec = yoyo;
        if yoyo.is_some() {
            if fp.death2_cb.is_none() {
                fp.death2_cb = Some(CodeRef(addr::ON_DAMAGE));
            }
            if fp.take_dmg_cb.is_none() {
                fp.take_dmg_cb = Some(CodeRef(addr::ON_DAMAGE));
            }
        }
        fp.pre_hitlag_cb = Some(CodeRef(addr::ATTACK_HI4_YOYO_ITEM_SET_UNK));
        fp.post_hitlag_cb = Some(CodeRef(addr::ATTACK_HI4_YOYO_ITEM_SET_UNK2));
    }

    /// The yo-yo gone on its own, `ftNs_AttackHi4_YoyoItemSetFlag`.
    #[doc(alias = "ftNs_AttackHi4_YoyoItemSetFlag")]
    pub fn ness_yoyo_item_set_flag(&mut self, gobj: GObjId) {
        self.ness_yoyo_hitlag_end(gobj);
        self.game.fighter_mut(gobj).u.set_gobj_at(u::YOYO, None);
    }

    /// The yo-yo taken away, `ftNs_AttackHi4_YoyoItemDespawn`.
    #[doc(alias = "ftNs_AttackHi4_YoyoItemDespawn")]
    pub fn ness_yoyo_item_despawn(&mut self, gobj: GObjId) {
        if let Some(yoyo) = self.game.fighter(gobj).u.gobj_at(u::YOYO) {
            nessyoyo::remove(self, yoyo);
            self.ness_yoyo_hitlag_end(gobj);
            self.game.fighter_mut(gobj).u.set_gobj_at(u::YOYO, None);
        }
    }

    /// Hitlag's start for the yo-yo, `ftNs_AttackHi4_YoyoItemSetUnk`.
    #[doc(alias = "ftNs_AttackHi4_YoyoItemSetUnk")]
    pub fn ness_yoyo_hitlag_start(&mut self, gobj: GObjId) {
        if let Some(yoyo) = self.game.fighter(gobj).u.gobj_at(u::YOYO) {
            nessyoyo::hitlag_start(self, yoyo);
        }
    }

    /// Hitlag's end for the yo-yo, `ftNs_AttackHi4_YoyoItemSetUnk2`.
    #[doc(alias = "ftNs_AttackHi4_YoyoItemSetUnk2")]
    pub fn ness_yoyo_hitlag_end(&mut self, gobj: GObjId) {
        if let Some(yoyo) = self.game.fighter(gobj).u.gobj_at(u::YOYO) {
            nessyoyo::hitlag_end(self, yoyo);
        }
    }

    /// The smashes' hooks: the timed rehit after a hit, the hitbox following the yo-yo.
    fn ness_yoyo_hooks(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.x2222_b2 = true;
        fp.deal_dmg_cb = Some(CodeRef(addr::ATTACK_HI4_YOYO_START_TIMED_REHIT));
        fp.accessory4_cb = Some(CodeRef(addr::ATTACK_HI4_YOYO_UPDATE_HIT_POS));
    }

    /// `ftNs_AttackHi4_Enter`.
    #[doc(alias = "ftNs_AttackHi4_Enter")]
    pub fn ness_attack_hi4_enter(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.allow_interrupt = false;
        fp.set_mv_i32(mv::YOYO_CHARGE_DISABLE, 0);
        self.ness_yoyo_set_var_all(gobj);
        self.change_motion_state(gobj, ms::ATTACK_HI4, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.ness_yoyo_hooks(gobj);
    }

    /// After a hit, the rehit timer, `ftNs_AttackHi4_YoyoStartTimedRehit` (the damage dealt
    /// hook).
    #[doc(alias = "ftNs_AttackHi4_YoyoStartTimedRehit")]
    pub fn ness_yoyo_start_timed_rehit(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let rate = fp::fctiwz(reg(self.ness_attr_f32(f, attr::YOYO_REHIT_RATE)));
        self.game.fighters[f.0].set_mv_i32(mv::YOYO_REHIT_TIMER, rate);
    }

    /// The smashes' frames (`ftNs_AttackHi4_Anim`, `ftNs_AttackLw4_Anim`): the yo-yo's frame,
    /// the rehit, and on frame 13 the charge if A is still held and the yo-yo is out in the
    /// open; at the end, waiting.
    pub(super) fn ness_yoyo_smash_anim(&mut self, gobj: GObjId, charge: fn(&mut Self, GObjId)) {
        let fp = self.game.fighter_mut(gobj);
        let n = fp.mv_i32(mv::YOYO_CURRENT_FRAME).wrapping_add(1);
        fp.set_mv_i32(mv::YOYO_CURRENT_FRAME, n);
        if self.ness_yoyo_think(gobj) {
            return;
        }
        self.ness_yoyo_check_timed_rehit(gobj);
        let fp = self.game.fighter(gobj);
        if fp.mv_i32(mv::YOYO_CURRENT_FRAME) == 13
            && fp.mv_i32(mv::YOYO_CHARGE_DISABLE) == 0
            && self.ness_yoyo_check_no_obstruct(gobj)
        {
            charge(self, gobj);
        }
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftNs_AttackHi4_Anim`.
    #[doc(alias = "ftNs_AttackHi4_Anim")]
    pub fn ness_attack_hi4_anim(&mut self, gobj: GObjId) {
        self.ness_yoyo_smash_anim(gobj, Self::ness_attack_hi4_charge_enter);
    }

    /// The smashes' interrupt (`ftNs_AttackHi4_IASA`, `ftNs_AttackLw4_IASA`): letting go of A
    /// rules out the charge.
    pub(super) fn ness_yoyo_smash_iasa(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.input.held_buttons[0] & PAD_A == 0 {
            fp.set_mv_i32(mv::YOYO_CHARGE_DISABLE, 1);
        }
        if fp.allow_interrupt {
            self.co_wait_iasa(gobj);
        }
    }

    /// `ftNs_AttackHi4_Phys` (the hitbox at the hand, which `updateYoyoHitPos` inlines).
    #[doc(alias = "ftNs_AttackHi4_Phys")]
    pub fn ness_attack_hi4_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
        self.ness_yoyo_set_hit_pos(gobj);
    }

    /// The up smash's collision (`ftNs_AttackHi4_Coll` and its charge's and release's): in the
    /// air the yo-yo goes.
    fn ness_attack_hi4_coll(&mut self, gobj: GObjId) {
        self.ground_coll(gobj);
        if self.game.fighter(gobj).ground_or_air == GA_AIR
            && let Some(yoyo) = self.game.fighter(gobj).u.gobj_at(u::YOYO)
        {
            nessyoyo::remove(self, yoyo);
            self.ness_yoyo_hitlag_end(gobj);
            self.game.fighter_mut(gobj).u.set_gobj_at(u::YOYO, None);
        }
    }

    /// `ftNs_AttackHi4Charge_Anim`: the spin speeding up, the rehit, and at the charge's
    /// length the release.
    #[doc(alias = "ftNs_AttackHi4Charge_Anim")]
    pub fn ness_attack_hi4_charge_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::YOYO_CURRENT_FRAME).wrapping_add(1);
        fp.set_mv_i32(mv::YOYO_CURRENT_FRAME, n);
        self.ness_yoyo_set_unk_rate(gobj);
        self.ness_yoyo_check_timed_rehit(gobj);
        let duration = reg(self.ness_attr_f32(f, attr::YOYO_CHARGE_DURATION));
        let frame = fp::frsp(f64::from(
            self.game.fighters[f.0].mv_i32(mv::YOYO_CURRENT_FRAME),
        ));
        if frame >= duration {
            self.ness_attack_hi4_release_enter(gobj);
        }
    }

    /// The charge's interrupt (`ftNs_AttackHi4Charge_IASA`, `ftNs_AttackLw4Charge_IASA`):
    /// letting go of A releases it.
    pub(super) fn ness_yoyo_charge_iasa(&mut self, gobj: GObjId, release: fn(&mut Self, GObjId)) {
        if self.game.fighter(gobj).input.held_buttons[0] & PAD_A == 0 {
            release(self, gobj);
        }
    }

    /// `ftNs_AttackHi4Charge_Enter`: the animation held on frame 12 and the yo-yo let go.
    #[doc(alias = "ftNs_AttackHi4Charge_Enter")]
    pub fn ness_attack_hi4_charge_enter(&mut self, gobj: GObjId) {
        self.ness_yoyo_charge_enter(gobj, ms::ATTACK_HI4_CHARGE);
    }

    /// The charges' entries.
    pub(super) fn ness_yoyo_charge_enter(&mut self, gobj: GObjId, msid: i32) {
        self.change_motion_state(gobj, msid, SKIP_ITEM_VIS, 12.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.set_anim_rate(gobj, 0.0);
        self.ness_yoyo_apply_smash(gobj);
        self.ness_yoyo_hooks(gobj);
    }

    /// The releases' frames (`ftNs_AttackHi4Release_Anim`, `ftNs_AttackLw4Release_Anim`).
    pub(super) fn ness_yoyo_release_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let n = fp.mv_i32(mv::YOYO_CURRENT_FRAME).wrapping_add(1);
        fp.set_mv_i32(mv::YOYO_CURRENT_FRAME, n);
        if self.ness_yoyo_think(gobj) {
            return;
        }
        self.ness_yoyo_check_timed_rehit(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// The releases' interrupt (`ftNs_AttackHi4Release_IASA`, `ftNs_AttackLw4Release_IASA`).
    pub(super) fn ness_yoyo_release_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).allow_interrupt {
            self.co_wait_iasa(gobj);
        }
    }

    /// `ftNs_AttackHi4Release_Phys`: the hitbox eases back to the hand over ten frames from
    /// frame 14, then follows it.
    #[doc(alias = "ftNs_AttackHi4Release_Phys")]
    pub fn ness_attack_hi4_release_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
        let frame = self.game.fighter(gobj).mv_i32(mv::YOYO_CURRENT_FRAME);
        if frame < 24 {
            let t = fp::fmuls(
                fp::fsubs(fp::frsp(f64::from(frame)), 14.0),
                f64::from(0.1_f32),
            );
            // Not `clamp`: a negative zero comes out positive here.
            #[allow(clippy::manual_clamp)]
            let t = if t >= 1.0 {
                1.0
            } else if t <= 0.0 {
                0.0
            } else {
                t
            };
            self.ness_yoyo_lerp_hit_pos(gobj, mem(t));
            return;
        }
        self.ness_yoyo_set_hit_pos(gobj);
    }

    /// `ftNs_AttackHi4Release_Enter`: the animation from frame 13 and the charged damage.
    #[doc(alias = "ftNs_AttackHi4Release_Enter")]
    pub fn ness_attack_hi4_release_enter(&mut self, gobj: GObjId) {
        self.ness_yoyo_release_enter(gobj, ms::ATTACK_HI4_RELEASE);
    }

    /// The releases' entries.
    pub(super) fn ness_yoyo_release_enter(&mut self, gobj: GObjId, msid: i32) {
        self.change_motion_state(gobj, msid, SKIP_ITEM_VIS, 13.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.ness_yoyo_set_charge_damage(gobj);
        self.ness_yoyo_hooks(gobj);
    }
}

fn u_vec(fp: &crate::game::structs::Fighter, off: usize) -> Vec3 {
    let f = |o: usize| f32::from_bits(fp.u_i32(o) as u32);
    Vec3::new(f(off), f(off + 4), f(off + 8))
}

fn set_u_vec(fp: &mut crate::game::structs::Fighter, off: usize, v: Vec3) {
    fp.set_u_i32(off, v.x.to_bits() as i32);
    fp.set_u_i32(off + 4, v.y.to_bits() as i32);
    fp.set_u_i32(off + 8, v.z.to_bits() as i32);
}

/// The yo-yo's hitbox position, for the yo-yo's own code.
pub(crate) fn hitbox_pos(fp: &crate::game::structs::Fighter) -> Vec3 {
    u_vec(fp, u::YOYO_HITBOX_POS)
}

pub(crate) fn set_hitbox_pos(fp: &mut crate::game::structs::Fighter, v: Vec3) {
    set_u_vec(fp, u::YOYO_HITBOX_POS, v);
}

/// The yo-yo's spin per frame (`x223C`).
pub(crate) fn spin_rate(fp: &crate::game::structs::Fighter) -> f32 {
    f32::from_bits(fp.u_i32(u::X223C) as u32)
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ATTACK_HI4_YOYO_UPDATE_HIT_POS => g.ness_yoyo_update_hit_pos(gobj),
        addr::ATTACK_HI4_YOYO_CHECK_TIMED_REHIT => g.ness_yoyo_check_timed_rehit(gobj),
        addr::ATTACK_HI4_YOYO_SET_HIT_POS => g.ness_yoyo_set_hit_pos(gobj),
        addr::ATTACK_HI4_YOYO_SET_VAR_ALL => g.ness_yoyo_set_var_all(gobj),
        addr::ATTACK_HI4_YOYO_APPLY_SMASH => g.ness_yoyo_apply_smash(gobj),
        addr::ATTACK_HI4_YOYO_SET_CHARGE_DAMAGE => g.ness_yoyo_set_charge_damage(gobj),
        addr::ATTACK_HI4_YOYO_SET_UNK_RATE => g.ness_yoyo_set_unk_rate(gobj),
        addr::ATTACK_HI4_YOYO_CREATE_ITEM => g.ness_yoyo_create_item(gobj),
        addr::ATTACK_HI4_YOYO_ITEM_SET_FLAG => g.ness_yoyo_item_set_flag(gobj),
        addr::ATTACK_HI4_YOYO_ITEM_DESPAWN => g.ness_yoyo_item_despawn(gobj),
        addr::ATTACK_HI4_YOYO_ITEM_SET_UNK => g.ness_yoyo_hitlag_start(gobj),
        addr::ATTACK_HI4_YOYO_ITEM_SET_UNK2 => g.ness_yoyo_hitlag_end(gobj),
        addr::ATTACK_HI4_ENTER => g.ness_attack_hi4_enter(gobj),
        addr::ATTACK_HI4_YOYO_START_TIMED_REHIT => g.ness_yoyo_start_timed_rehit(gobj),
        addr::ATTACK_HI4_ANIM => g.ness_attack_hi4_anim(gobj),
        addr::ATTACK_HI4_IASA => g.ness_yoyo_smash_iasa(gobj),
        addr::ATTACK_HI4_PHYS => g.ness_attack_hi4_phys(gobj),
        addr::ATTACK_HI4_COLL | addr::ATTACK_HI4_CHARGE_COLL | addr::ATTACK_HI4_RELEASE_COLL => {
            g.ness_attack_hi4_coll(gobj)
        }
        addr::ATTACK_HI4_CHARGE_ANIM => g.ness_attack_hi4_charge_anim(gobj),
        addr::ATTACK_HI4_CHARGE_IASA => {
            g.ness_yoyo_charge_iasa(gobj, Running::ness_attack_hi4_release_enter)
        }
        addr::ATTACK_HI4_CHARGE_PHYS => g.ground_friction(gobj),
        addr::ATTACK_HI4_CHARGE_ENTER => g.ness_attack_hi4_charge_enter(gobj),
        addr::ATTACK_HI4_RELEASE_ANIM => g.ness_yoyo_release_anim(gobj),
        addr::ATTACK_HI4_RELEASE_IASA => g.ness_yoyo_release_iasa(gobj),
        addr::ATTACK_HI4_RELEASE_PHYS => g.ness_attack_hi4_release_phys(gobj),
        addr::ATTACK_HI4_RELEASE_ENTER => g.ness_attack_hi4_release_enter(gobj),
        _ => return false,
    }
    true
}
