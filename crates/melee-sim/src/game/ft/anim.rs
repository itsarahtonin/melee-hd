//! Fighter animation playback (`ftanim.c`): each frame the fighter's joints step through the
//! current action's animation (a figatree bound to the joints), blending from the previous
//! one over a few frames when the action asked for it, and TransN's motion is taken out of
//! the model so the action's physics can apply it (`x68C_transNPos`).

use gekko_fp as fp;

use super::super::handles::FighterId;
use super::super::state::{Game, Running};
use super::super::structs::{Fighter, FighterBone};
use super::parts::TRANS_N;
use crate::fpu::{mem, reg};
use crate::hsd::aobj;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, INSTANCE, JObjId, USE_QUATERNION};
use crate::lb::anim::{blend, copy_srt, current_frame, framerate, is_animated};
use crate::vec::Vec3;
use melee_data::archive::{Archives, DataRef};

/// `FtPart` 0x35, the part some characters' animations move alongside TransN.
const PART_35: i32 = 0x35;
/// `FtPart_XRotN`.
const X_ROT_N: i32 = 2;

/// The fighter's `x594` bits, most significant first.
pub fn x594_bit(fp: &Fighter, bit: u32) -> bool {
    fp.x594[0] & (0x80 >> bit) != 0
}

/// A part's bone flags (`x8.x0`), most significant first.
pub fn bone_flag(bone: &FighterBone, bit: u32) -> bool {
    bone.x8[0] & (0x80 >> bit) != 0
}

/// Whether a part plays the action's animation: `flags_b1` set, `flags_b0` and `flags_b5`
/// clear.
fn plays_anim(bone: &FighterBone) -> bool {
    bone_flag(bone, 1) && !bone_flag(bone, 0) && !bone_flag(bone, 5)
}

impl Running<'_> {
    /// `ftCommon_GetModelScale`, in a register.
    #[doc(alias = "ftCommon_GetModelScale")]
    pub fn model_scale(fp: &Fighter) -> f64 {
        fp::fmuls(reg(fp.x34_scale.y), reg(fp.co_attrs.model_scaling))
    }

    pub(super) fn bones(&self, f: FighterId) -> &[FighterBone] {
        let parts = self.game.fighters[f.0]
            .parts
            .expect("a fighter without parts");
        &self.game.fighter_parts[parts.0]
    }

    pub(super) fn bone(&self, f: FighterId, i: i32) -> FighterBone {
        self.bones(f)[i as usize].clone()
    }

    fn parts_num(&self, f: FighterId) -> u32 {
        self.parts_table(self.game.fighters[f.0].kind).len()
    }

    /// The fighter's animation for the frame, `ftAnim_8006EBA4`: the joints, the action's
    /// command script, the part animations and the throw offset.
    #[doc(alias = "ftAnim_8006EBA4")]
    pub fn anim_update(&mut self, gobj: GObjId) {
        self.anim_joints(gobj);
        self.run_commands(gobj);
        self.blend_part_anims(gobj);
        self.throw_offset_anim(gobj);
    }

    /// Steps the joints' animation, `ftAnim_8006E9B4`, and notes the frame it reached.
    #[doc(alias = "ftAnim_8006E9B4")]
    pub fn anim_joints(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].anim_id == -1 {
            return;
        }
        let root = self.root_jobj(gobj);
        // `lfs f1, @422`: the register the frame below falls back on starts at 0.
        self.game.objs.aobj_f1 = Some(0.0);
        if self.game.fighters[f.0].x8A4_animBlendFrames == 0.0 {
            jobj::clear_flags_all(&mut self.game.objs.jobjs, Some(root), USE_QUATERNION);
            if x594_bit(&self.game.fighters[f.0], 0) {
                let fp = &self.game.fighters[f.0];
                let p35 = self.bone(f, self.bone_index(fp, PART_35)).joint;
                let trans = self.bone(f, self.bone_index(fp, TRANS_N)).joint;
                self.anim_tree_trans(f, Some(root), trans, p35);
            } else {
                self.anim_part_tree(f, 0);
            }
        } else {
            let skel = self.game.fighters[f.0].x8AC_animSkeleton;
            let rate = reg(framerate(&self.game.objs, skel));
            let fp = &mut self.game.fighters[f.0];
            fp.x8A8_anim_frame = mem(fp::fadds(reg(fp.x8A8_anim_frame), rate));
            let (t, t_inv);
            if fp.x8A4_animBlendFrames <= fp.x8A8_anim_frame {
                fp.x8A8_anim_frame = fp.x8A4_animBlendFrames;
                t = 1.0;
                t_inv = 0.0;
            } else {
                let remaining = fp::fsubs(reg(fp.x8A4_animBlendFrames), reg(fp.x8A8_anim_frame));
                t = fp::fdivs(rate, fp::fadds(rate, remaining));
                t_inv = fp::fsubs(1.0, t);
            }
            self.anim_part_tree(f, 0);
            if x594_bit(&self.game.fighters[f.0], 0) {
                let fp = &self.game.fighters[f.0];
                let p35 = self.bone(f, self.bone_index(fp, PART_35)).joint;
                let trans = self.bone(f, self.bone_index(fp, TRANS_N)).x4_jobj2;
                self.anim_tree_trans(f, skel, trans, p35);
            } else {
                jobj::anim_all(
                    &mut self.game.objs,
                    self.data,
                    &mut crate::game::code::ParticleHooks,
                    skel,
                );
            }
            if t_inv != 0.0 {
                self.blend_parts(f, TRANS_N, t, t_inv);
            } else {
                self.copy_parts(f, TRANS_N);
            }
        }
        if x594_bit(&self.game.fighters[f.0], 2) {
            match self.anim_frame(gobj) {
                Some(frame) => {
                    let fp = &mut self.game.fighters[f.0];
                    if frame < fp.cur_anim_frame {
                        fp.x898_unk = mem(fp::fadds(
                            reg(fp.x898_unk),
                            fp::fadds(reg(fp.cur_anim_frame), reg(fp.frame_speed_mul)),
                        ));
                    }
                }
                // No part has an animation: the original compares the register `f1` the
                // walk left (docs/engine.md), and taking the branch loads `x898` into it.
                None => match self.game.objs.aobj_f1 {
                    Some(v) => {
                        let fp = &mut self.game.fighters[f.0];
                        if v < fp.cur_anim_frame {
                            let old = fp.x898_unk;
                            fp.x898_unk = mem(fp::fadds(
                                reg(old),
                                fp::fadds(reg(fp.cur_anim_frame), reg(fp.frame_speed_mul)),
                            ));
                            self.game.objs.aobj_f1 = Some(old);
                        }
                    }
                    None => self.game.objs.stale_reads += 1,
                },
            }
        }
        match self.anim_frame(gobj) {
            Some(frame) => self.game.fighters[f.0].cur_anim_frame = frame,
            // As above: the original stores the register `f1` as it stands; unknown, it's
            // counted and the frame left as it was.
            None => match self.game.objs.aobj_f1 {
                Some(v) => self.game.fighters[f.0].cur_anim_frame = v,
                None => self.game.objs.stale_reads += 1,
            },
        }
    }

    /// The GObj's root joint.
    pub fn root_jobj(&self, gobj: GObjId) -> JObjId {
        match self.game.world.gobjs[gobj.0].obj {
            Some(super::super::state::Obj::JObj(j)) => j,
            other => panic!("GObj {gobj:?} holds {other:?}, not a joint"),
        }
    }

    /// Animates a tree from `jobj` with TransN (`trans`) and part 0x35 (`p35`, when `x594_b5`)
    /// held at the origin, their animated translation kept, scaled, on the fighter as the
    /// motion the action applies, `ftAnim_8006E054`.
    #[doc(alias = "ftAnim_8006E054")]
    fn anim_tree_trans(
        &mut self,
        f: FighterId,
        mut jobj: Option<JObjId>,
        trans: Option<JObjId>,
        p35: Option<JObjId>,
    ) {
        aobj::init_end_callback(&mut self.game.objs.aobjs);
        let scale_of = |fp: &Fighter| {
            if !x594_bit(fp, 6) {
                Self::model_scale(fp)
            } else {
                reg(fp.co_attrs.model_scaling)
            }
        };
        let scaled = |v: Vec3, s: f64| {
            Vec3::new(
                mem(fp::fmuls(reg(v.x), s)),
                mem(fp::fmuls(reg(v.y), s)),
                mem(fp::fmuls(reg(v.z), s)),
            )
        };
        while let Some(j) = jobj {
            if jobj == trans {
                jobj::anim(
                    &mut self.game.objs,
                    self.data,
                    &mut crate::game::code::ParticleHooks,
                    Some(j),
                );
                let t = self.game.objs.jobjs[j].translate;
                let fp = &mut self.game.fighters[f.0];
                fp.x698 = fp.x68C_transNPos;
                fp.x68C_transNPos = t;
                let s = scale_of(fp);
                fp.x68C_transNPos = scaled(fp.x68C_transNPos, s);
                fp.x6B0 = fp.x6A4_transNOffset;
                fp.x6A4_transNOffset = fp.x68C_transNPos - fp.x698;
                jobj::set_translate(&mut self.game.objs.jobjs, j, Vec3::default());
            } else if x594_bit(&self.game.fighters[f.0], 5) && jobj == p35 {
                jobj::anim(
                    &mut self.game.objs,
                    self.data,
                    &mut crate::game::code::ParticleHooks,
                    Some(j),
                );
                let t = self.game.objs.jobjs[j].translate;
                let fp = &mut self.game.fighters[f.0];
                fp.x6CC = fp.x6C0;
                fp.x6C0 = t;
                let s = scale_of(fp);
                fp.x6C0 = scaled(fp.x6C0, s);
                fp.x6E4 = fp.x6D8;
                fp.x6D8 = fp.x6C0 - fp.x6CC;
                jobj::set_translate(&mut self.game.objs.jobjs, j, Vec3::default());
            } else {
                jobj::anim(
                    &mut self.game.objs,
                    self.data,
                    &mut crate::game::code::ParticleHooks,
                    Some(j),
                );
            }
            jobj = next_in_tree(&self.game.objs.jobjs, j, None);
        }
        let fp = &mut self.game.fighters[f.0];
        if x594_bit(fp, 5) {
            let d = fp.x68C_transNPos - fp.x6C0;
            fp.x68C_transNPos = fp.x6C0;
            fp.x698 = fp.x6CC;
            fp.x6A4_transNOffset = fp.x6D8;
            fp.x6B0 = fp.x6E4;
            jobj::set_translate(
                &mut self.game.objs.jobjs,
                trans.expect("HSD_JObjSetTranslate on a null joint"),
                d,
            );
        }
        let fp = &self.game.fighters[f.0];
        if !fp.x2226_b2 && fp.x2221_b2 {
            let bone = self.data.u8(self.ft_data_x8(fp).at(0x10));
            let j = self.bone(f, i32::from(bone)).joint.expect("a joint");
            let t = self.game.objs.jobjs[j].translate;
            let s = scale_of(fp);
            let inv = fp::fdivs(1.0, s);
            let p = scaled(fp.x68C_transNPos, inv);
            jobj::set_translate(&mut self.game.objs.jobjs, j, t - p);
        }
        aobj::invoke_end_callbacks(&self.game.objs.aobjs);
    }

    /// `ft_data->x8`, the character's model description.
    pub fn ft_data_x8(&self, fp: &Fighter) -> melee_data::archive::DataRef {
        let d = fp.ft_data.expect("a fighter without data");
        self.data.ptr(d.at(8)).expect("ftData x8")
    }

    /// Animates the parts in `part`'s subtree that play the action's animation,
    /// `ftAnim_8006E7B8`.
    #[doc(alias = "ftAnim_8006E7B8")]
    pub(super) fn anim_part_tree(&mut self, f: FighterId, part: i32) {
        aobj::init_end_callback(&mut self.game.objs.aobjs);
        let mut i = part;
        let top_bone = self.bone(f, part);
        let top = top_bone.joint;
        let depth = top_bone.xC_u[0];
        let mut jobj = top;
        while let Some(j) = jobj {
            while !bone_flag(&self.bones(f)[i as usize], 1) {
                i += 1;
            }
            let bone = self.bone(f, i);
            if bone.xC_u[0] <= depth && i != part {
                break;
            }
            if !bone_flag(&bone, 0) && !bone_flag(&bone, 5) {
                jobj::anim(
                    &mut self.game.objs,
                    self.data,
                    &mut crate::game::code::ParticleHooks,
                    Some(j),
                );
            }
            i += 1;
            jobj = next_in_tree(&self.game.objs.jobjs, j, top);
        }
        aobj::invoke_end_callbacks(&self.game.objs.aobjs);
    }

    /// The animation's current frame, `ftAnim_8006F3DC`: the first animated part's, or the
    /// blend skeleton's while blending. None where no part has an animation, when the
    /// original returns a register it never set.
    #[doc(alias = "ftAnim_8006F3DC")]
    pub fn anim_frame(&self, gobj: GObjId) -> Option<f32> {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        if fp.x8A4_animBlendFrames != 0.0 {
            return Some(current_frame(&self.game.objs, fp.x8AC_animSkeleton));
        }
        let n = self.parts_num(f) as usize;
        self.bones(f)[..n]
            .iter()
            .filter(|b| plays_anim(b))
            .find_map(|b| {
                let j = b.joint.expect("a part without a joint");
                self.game.objs.jobjs[j]
                    .aobj
                    .map(|a| self.game.objs.aobjs[a].curr_frame)
            })
    }

    /// Whether a part's animation plays, `ftAnim_8006F368`.
    #[doc(alias = "ftAnim_8006F368")]
    pub fn part_is_animated(&self, f: FighterId, part: i32) -> bool {
        let bone = self.bone(f, part);
        let j = if self.game.fighters[f.0].x8A4_animBlendFrames == 0.0 {
            bone.joint
        } else {
            bone.x4_jobj2
        };
        is_animated(&self.game.objs, j.expect("a part without a joint"))
    }

    /// Whether any part's animation still plays, `ftAnim_IsFramesRemaining`.
    #[doc(alias = "ftAnim_IsFramesRemaining")]
    pub fn anim_frames_remaining(&self, gobj: GObjId) -> bool {
        let f = self.game.fighter_id(gobj);
        let blending = self.game.fighters[f.0].x8A4_animBlendFrames != 0.0;
        let n = self.parts_num(f) as usize;
        self.bones(f)[..n]
            .iter()
            .filter(|b| plays_anim(b))
            .any(|b| {
                let j = if blending { b.x4_jobj2 } else { b.joint };
                is_animated(&self.game.objs, j.expect("a part without a joint"))
            })
    }

    /// Blends the animated parts from `start` on toward the blend skeleton's pose,
    /// `ftAnim_8006FE9C`.
    #[doc(alias = "ftAnim_8006FE9C")]
    pub fn blend_parts(&mut self, f: FighterId, start: i32, t: f64, t_inv: f64) {
        for i in start..self.parts_num(f) as i32 {
            let b = self.bone(f, i);
            if !plays_anim(&b) {
                continue;
            }
            let (j2, j) = (b.x4_jobj2.expect("a joint"), b.joint.expect("a joint"));
            if bone_flag(&b, 4) {
                copy_srt(&mut self.game.objs.jobjs, j2, j);
            } else {
                blend(&mut self.game.objs.jobjs, j2, j, j, t, t_inv);
            }
        }
    }

    /// Copies the blend skeleton's pose to the animated parts from `start` on,
    /// `ftAnim_8006FF74`.
    #[doc(alias = "ftAnim_8006FF74")]
    pub fn copy_parts(&mut self, f: FighterId, start: i32) {
        for i in start..self.parts_num(f) as i32 {
            let b = self.bone(f, i);
            if plays_anim(&b) {
                copy_srt(
                    &mut self.game.objs.jobjs,
                    b.x4_jobj2.expect("a joint"),
                    b.joint.expect("a joint"),
                );
            }
        }
    }

    /// Plays animation `anim` of the character's part animation set `slot` (`ftData::x1C`),
    /// blending into it over `frames` frames (0 at once), `ftAnim_ApplyPartAnim`.
    #[doc(alias = "ftAnim_ApplyPartAnim")]
    pub fn apply_part_anim(&mut self, gobj: GObjId, slot: i32, anim: i32, frames: f32) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let ft_data = fp.ft_data.expect("a fighter without data");
        let list = self.data.ptr(ft_data.at(0x1C)).expect("ftData x1C");
        let set = self
            .data
            .ptr(list.at(4 * slot as u32))
            .expect("ftData x1C entry");
        let start = i32::from(self.data.u16(set));
        let anims = self.data.ptr(set.at(8)).expect("the set's animations");
        let aj = self.data.ptr(anims.at(4 * anim as u32));
        let blend = if frames != 0.0 {
            mem(fp::fdivs(reg(first_end_frame(self.data, aj)), reg(frames)))
        } else {
            0.0
        };
        let e = &mut self.game.fighters[f.0].x8B0[slot as usize];
        e.x11 = anim as i8;
        e.x4 = frames;
        e.x8 = 0.0;
        e.xC = blend;
        self.add_part_anims(f, start, aj);
    }

    /// Gives the parts from `start` on an AnimJoint tree's animations, `ftAnim_80070904`:
    /// joint by joint in the tree's order, skipping the character's optional parts; each
    /// part not held (`flags_b0`) whose joint animates plays it on its blend joint from frame
    /// 0, marked as playing a part animation (`flags_b5`).
    #[doc(alias = "ftAnim_80070904")]
    fn add_part_anims(&mut self, f: FighterId, start: i32, aj: Option<DataRef>) {
        let kind = self.game.fighters[f.0].kind;
        let parts = self.game.fighters[f.0]
            .parts
            .expect("a fighter without parts");
        let mut stack = super::bind::DataStack::<MAX_ANIM_JOINT_DEPTH>::new();
        let (mut i, mut cur) = (start, aj);
        while let Some(a) = cur {
            while self.part_mask(kind, i) != 0 {
                i += 1;
            }
            let b = self.bone(f, i);
            if !bone_flag(&b, 0) && self.data.ptr(a.at(8)).is_some() {
                let j = b.x4_jobj2;
                let objs = &mut self.game.objs;
                jobj::add_anim(objs, self.data, j, Some(a), None, None);
                jobj::clear_flags(&mut objs.jobjs, j, 0x20000);
                jobj::req_anim_by_flags(objs, j, 1, 0.0);
                jobj::anim(objs, self.data, &mut crate::game::code::ParticleHooks, j);
                self.game.fighter_parts[parts.0][i as usize].x8[0] |= 0x04;
            }
            i += 1;
            cur = next_anim_joint(self.data, a, &mut stack);
        }
    }

    /// The pose part animation set `slot` takes when shown (-1 none), `ftAnim_80070FB4`: a
    /// hand's hold pose for an item.
    #[doc(alias = "ftAnim_80070FB4")]
    pub fn set_part_anim_pose(&mut self, gobj: GObjId, slot: i32, anim: i32) {
        self.game.fighter_mut(gobj).x8B0[slot as usize].x10 = anim as i8;
    }

    /// Plays set `slot`'s pose at once, if it has one, `ftAnim_80070C48`.
    #[doc(alias = "ftAnim_80070C48")]
    pub fn show_part_anim_pose(&mut self, gobj: GObjId, slot: i32) {
        let anim = self.game.fighter(gobj).x8B0[slot as usize].x10;
        if anim != -1 {
            self.apply_part_anim(gobj, slot, i32::from(anim), 0.0);
        }
    }

    /// Ends set `slot`'s part animation, `ftAnim_80070CC4`: its parts stop playing it, and
    /// they go back to the action's animation (`ftAnim_8006EED4`) or, with none, to the
    /// costume's pose (`ftAnim_8006FA58`).
    #[doc(alias = "ftAnim_80070CC4")]
    pub fn end_part_anim(&mut self, gobj: GObjId, slot: i32) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let anim = fp.x8B0[slot as usize].x11;
        if anim == -1 {
            return;
        }
        let kind = fp.kind;
        let ft_data = fp.ft_data.expect("a fighter without data");
        let list = self.data.ptr(ft_data.at(0x1C)).expect("ftData x1C");
        let set = self
            .data
            .ptr(list.at(4 * slot as u32))
            .expect("ftData x1C entry");
        let start = i32::from(self.data.u16(set));
        let anims = self.data.ptr(set.at(8)).expect("the set's animations");
        let parts = fp.parts.expect("a fighter without parts");
        let mut stack = super::bind::DataStack::<MAX_ANIM_JOINT_DEPTH>::new();
        let (mut i, mut cur) = (start, self.data.ptr(anims.at(4 * anim as u32)));
        while let Some(a) = cur {
            while self.part_mask(kind, i) != 0 {
                i += 1;
            }
            self.game.fighter_parts[parts.0][i as usize].x8[0] &= !0x04;
            i += 1;
            cur = next_anim_joint(self.data, a, &mut stack);
        }
        let fp = &mut self.game.fighters[f.0];
        fp.x8B0[slot as usize].x11 = -1;
        if fp.x590.is_some() {
            let (frame, speed) = (fp.cur_anim_frame, fp.frame_speed_mul);
            self.restart_part_anim(f, start, frame, speed);
            return;
        }
        let j = self.bone(f, start).joint;
        let costume = self.game.fighters[f.0].x108_costume_joint;
        let desc = self.part_joint_desc(f, j, costume);
        self.reset_parts(f, start, desc, false);
    }

    /// Blends each of the five part animations (`x8B0`) in progress, `ftAnim_800707B0`.
    #[doc(alias = "ftAnim_800707B0")]
    pub fn blend_part_anims(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        for i in 0..5 {
            let fp = &self.game.fighters[f.0];
            if fp.x8B0[i].x11 == -1 {
                continue;
            }
            let ft_data = fp.ft_data.expect("a fighter without data");
            let list = self.data.ptr(ft_data.at(0x1C)).expect("ftData x1C");
            let entry = self
                .data
                .ptr(list.at(4 * i as u32))
                .expect("ftData x1C entry");
            let count = self.data.u16(entry.at(2));
            let bones = self.data.ptr(entry.at(4));
            let fp = &mut self.game.fighters[f.0];
            let e = &mut fp.x8B0[i];
            e.x8 = mem(fp::fadds(reg(e.x8), reg(e.xC)));
            let (t, t_inv);
            if e.x4 <= e.x8 {
                e.x8 = e.x4;
                t = 1.0;
                t_inv = 0.0;
            } else {
                let d = fp::fdivs(
                    reg(e.xC),
                    fp::fadds(reg(e.xC), fp::fsubs(reg(e.x4), reg(e.x8))),
                );
                t = d;
                t_inv = fp::fsubs(1.0, d);
            }
            for j in 0..u32::from(count) {
                let bone = self.data.u8(bones.expect("a bone list").at(j));
                let b = self.bone(f, i32::from(bone));
                if !bone_flag(&b, 5) {
                    continue;
                }
                let (j2, j1) = (b.x4_jobj2.expect("a joint"), b.joint.expect("a joint"));
                if t_inv != 0.0 {
                    blend(&mut self.game.objs.jobjs, j2, j1, j1, t, t_inv);
                } else {
                    copy_srt(&mut self.game.objs.jobjs, j2, j1);
                }
            }
        }
    }

    /// `ftCo_800DB500`: while thrown (`x2226_b2`), keeps XRotN's animated translation and
    /// offsets the model by it.
    #[doc(alias = "ftCo_800DB500")]
    pub fn throw_offset_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].x2226_b2 {
            let part = self.bone_index(&self.game.fighters[f.0], X_ROT_N);
            if self.part_is_animated(f, part) {
                let j = self.bone(f, part).joint.expect("XRotN");
                self.game.fighters[f.0].x2174 = self.game.objs.jobjs[j].translate;
            }
        }
        if self.game.fighters[f.0].x2226_b2 {
            self.hip_to_root(f);
        }
    }

    /// `ftAnim_8006DF0C`: with `x2221_b2`, the hip's offset point brought into the model's
    /// space for the character's bone (not ported); nothing otherwise.
    #[doc(alias = "ftAnim_8006DF0C")]
    fn hip_to_root(&mut self, f: FighterId) {
        if self.game.fighters[f.0].x2221_b2 {
            super::super::code::unported_call("ftAnim_8006DF0C with x2221_b2");
        }
    }
}

/// The next joint after `j` in a depth-first walk that stays below `top` (or the whole
/// tree, for None), not entering instances.
/// How deep `ftAnim_GetNextAnimJointInTree` follows a tree (`ftAnim_804590D8`).
const MAX_ANIM_JOINT_DEPTH: usize = 30;

/// The AnimJoint after `aj` in a depth-first walk, `ftAnim_GetNextAnimJointInTree`: its child,
/// else its next sibling, else the nearest ancestor's next sibling.
#[doc(alias = "ftAnim_GetNextAnimJointInTree")]
fn next_anim_joint(
    data: &Archives,
    aj: DataRef,
    stack: &mut super::bind::DataStack<MAX_ANIM_JOINT_DEPTH>,
) -> Option<DataRef> {
    if let Some(child) = data.ptr(aj) {
        stack.push(aj);
        assert!(
            stack.len() < MAX_ANIM_JOINT_DEPTH,
            "fighter joint depth num over!"
        );
        return Some(child);
    }
    if let Some(next) = data.ptr(aj.at(4)) {
        return Some(next);
    }
    while let Some(parent) = stack.pop() {
        if let Some(next) = data.ptr(parent.at(4)) {
            return Some(next);
        }
    }
    None
}

/// The end frame of the first AnimJoint in the tree with an animation, or 0,
/// `lb_8000BFF0` (`lb_8000BECC` finds it).
// Ports `lb_8000BFF0`; aliased once a check reaches it.
// Ports `lb_8000BECC`; aliased once a check reaches it.
fn first_end_frame(data: &Archives, aj: Option<DataRef>) -> f32 {
    fn find(data: &Archives, aj: Option<DataRef>) -> Option<DataRef> {
        let aj = aj?;
        if data.ptr(aj.at(8)).is_some() {
            return Some(aj);
        }
        find(data, data.ptr(aj)).or_else(|| find(data, data.ptr(aj.at(4))))
    }
    match find(data, aj) {
        Some(a) => data.f32(data.ptr(a.at(8)).expect("aobjdesc").at(4)),
        None => 0.0,
    }
}

fn next_in_tree(jobjs: &jobj::JObjs, j: JObjId, top: Option<JObjId>) -> Option<JObjId> {
    if jobjs[j].flags & INSTANCE == 0
        && let Some(c) = jobjs[j].child
    {
        return Some(c);
    }
    if let Some(n) = jobjs[j].next {
        return Some(n);
    }
    let mut j = j;
    loop {
        let p = jobjs[j].parent;
        if p.is_none() || p == top {
            return None;
        }
        let p = p.unwrap();
        if let Some(n) = jobjs[p].next {
            return Some(n);
        }
        j = p;
    }
}

impl Running<'_> {
    /// Gives the fighter's model its costume's material animations, at frame 0, and finds
    /// the costume's animated textures, stopped, `ftAnim_80070308` (with `ftAnim_80070200`).
    #[doc(alias = "ftAnim_80070308")]
    #[doc(alias = "ftAnim_80070200")]
    pub fn costume_anims(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let root = self.root_jobj(gobj);
        let fp = &self.game.fighters[f.0];
        let matanim =
            self.game.ft.data.costumes[fp.kind as usize][usize::from(fp.costume_id)].matanim;
        jobj::add_anim_all(
            &mut self.game.objs,
            self.data,
            Some(root),
            None,
            matanim,
            None,
        );
        jobj::req_anim_all(&mut self.game.objs, Some(root), 0.0);
        // `ftAnim_80070200` on `ftData::x8->x8`.
        let fp = &self.game.fighters[f.0];
        let data = fp.ft_data.expect("a fighter without data");
        let x8 = self.data.ptr(data.at(8)).expect("ftData::x8").at(8);
        let n = self.data.u32(x8);
        assert!(
            n <= 5,
            "the original's assertion failed: fighter tobj num over!"
        );
        let table = self.data.ptr(x8.at(4)).expect("ftData_x8_x8::xC");
        let costume = u32::from(fp.costume_id);
        // A character without texture animations (Samus, Captain Falcon) has none for any
        // costume: the list is null, and so is what the fighter keeps.
        let indices = self
            .data
            .ptr(table.at(4 * costume))
            .or_else(|| self.data.ptr(table));
        let (list, count) = (fp.dobj_list.data, fp.dobj_list.count);
        self.game.fighters[f.0].tobj_list.n_costume_tobjs = n;
        self.game.fighters[f.0].tobj_list.x5D0 = indices;
        for i in 0..n {
            let indices = indices.expect("texture animations without their list");
            let index = i32::from(self.data.u16(indices.at(2 * i)));
            let t = self.nth_tobj(list, count, index);
            self.game.fighters[f.0].tobj_list.costume_tobjs[i as usize] = Some(t);
            let a = self.game.objs.tobjs[t].aobj;
            assert!(
                a.is_some(),
                "the original's assertion failed: can't find fighter texture anim!"
            );
            crate::hsd::aobj::set_rate(&mut self.game.objs.aobjs, a, 0.0);
        }
    }

    /// Loads the fighter's blend skeleton, the costume's joints without their display
    /// objects, and points the parts' second joints at it, `ftAnim_8006FE48` (with
    /// `ftParts_8007482C`).
    #[doc(alias = "ftAnim_8006FE48")]
    #[doc(alias = "ftParts_8007482C")]
    pub fn load_blend_skeleton(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let joint = self.game.fighters[f.0].x108_costume_joint;
        jobj::set_default_class(&mut self.game.objs, jobj::JObjClass::FighterIntp);
        let skeleton = jobj::load_joint(&mut self.game.objs, self.data, joint);
        jobj::set_default_class(&mut self.game.objs, jobj::JObjClass::Hsd);
        let fp = &mut self.game.fighters[f.0];
        fp.x8AC_animSkeleton = skeleton;
        fp.x8A4_animBlendFrames = 0.0;
        fp.x8A8_anim_frame = 0.0;
        self.setup_blend_parts(gobj);
    }
}

impl Game {
    /// Clears the fighter's part animations' slots, `ftAnim_8007077C`.
    #[doc(alias = "ftAnim_8007077C")]
    pub fn reset_part_anims(&mut self, f: FighterId) {
        for a in &mut self.fighters[f.0].x8B0 {
            a.x11 = -1;
            a.x10 = -1;
        }
    }
}
