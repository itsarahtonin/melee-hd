//! Starting a fighter's animation (`ftanim.c`): its joints go back to the costume's pose,
//! the action's figatree is bound to them track by track, and the animations are requested
//! from the starting frame at the action's rate.

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::handles::FighterId;
use super::super::state::Running;
use super::anim::bone_flag;
use super::parts::{INVALID, TRANS_N};
use crate::fpu::{mem, reg};
use crate::hsd::aobj;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId, USE_QUATERNION};
use crate::lb::anim::{
    reset_pose, reset_rotation_translation, reset_scale_translation, reset_translation,
};
use crate::lb::figatree::{TRACK_SIZE, bind_tracks, bind_tracks_no_translation};
use crate::vec::Vec3;

/// The object types the fighter's animation requests reach: all but materials and textures.
const ANIM_MASK: u32 = 0xFB7F;

/// How deep `ftAnim_GetNextJointInTree` follows a joint tree (`ftAnim_804590D8_F0`).
const MAX_JOINT_DEPTH: usize = 30;

/// The fighter's `x594` as a word.
pub fn x594_word(x594: [u8; 4]) -> u32 {
    u32::from_be_bytes(x594)
}

/// The next joint descriptor after `joint` in a depth-first walk, `ftAnim_GetNextJointInTree`:
/// its child, else its next sibling, else the nearest ancestor's next sibling.
#[doc(alias = "ftAnim_GetNextJointInTree")]
fn next_joint(
    data: &melee_data::archive::Archives,
    joint: DataRef,
    stack: &mut Vec<DataRef>,
) -> Option<DataRef> {
    if let Some(child) = data.ptr(joint.at(8)) {
        stack.push(joint);
        assert!(
            stack.len() < MAX_JOINT_DEPTH,
            "fighter joint depth num over!"
        );
        return Some(child);
    }
    if let Some(next) = data.ptr(joint.at(0xC)) {
        return Some(next);
    }
    while let Some(&parent) = stack.last() {
        stack.pop();
        if let Some(next) = data.ptr(parent.at(0xC)) {
            return Some(next);
        }
    }
    None
}

impl Running<'_> {
    /// The bit of a part among the character's optional parts (`Fighter_804D6540`), or 0 for
    /// a part every animation moves, `ftParts_8007506C`.
    #[doc(alias = "ftParts_8007506C")]
    pub fn part_mask(&self, kind: i32, part: i32) -> u32 {
        let table = self.load_common(0x14);
        let Some(entry) = self.data.ptr(table.at(4 * kind as u32)) else {
            return 0;
        };
        let n = self.data.u32(entry.at(4)) as i32;
        if n == 0 {
            return 0;
        }
        let list = self.data.ptr(entry).expect("Fighter_804D6540 entries");
        (0..n)
            .find(|&i| i32::from(self.data.u8(list.at(4 * i as u32))) == part)
            .map_or(0, |i| 1 << i)
    }

    /// The joint of character `to` matching joint `joint` of character `from`, through their
    /// common parts, or [`INVALID`], `ftPartsRemap`.
    #[doc(alias = "ftPartsRemap")]
    pub fn parts_remap(&self, to: i32, from: i32, joint: u32) -> u8 {
        let from = self.parts_table(from);
        if joint >= from.len() {
            return INVALID;
        }
        match from.part_of(joint) {
            INVALID => INVALID,
            part => self.parts_table(to).joint_of(u32::from(part)),
        }
    }

    /// Scales a joint by the inverse of the character's model scale, `ftCommon_8007F6A4`.
    #[doc(alias = "ftCommon_8007F6A4")]
    fn unscale_joint(&mut self, f: FighterId, j: Option<JObjId>) {
        let fp = &self.game.fighters[f.0];
        let attrs = self
            .data
            .ptr(fp.ft_data.expect("a fighter without data"))
            .expect("ftData's attributes");
        let v = mem(fp::fdivs(1.0, reg(self.data.f32(attrs.at(0x8C)))));
        let j = j.expect("a part without a joint");
        jobj::set_scale(&mut self.game.objs.jobjs, j, Vec3 { x: v, y: v, z: v });
    }

    /// Puts the parts from `part` on back in the costume's pose, walking its joint tree from
    /// `joint`: `ftAnim_8006FA58` for the joints themselves, `ftAnim_8006FB88` (`blend`) for
    /// the second joints an animation blends into. Parts that follow dynamics keep their
    /// angles and scale; the fighter's root keeps its rotation and scale.
    #[doc(alias = "ftAnim_8006FA58")]
    #[doc(alias = "ftAnim_8006FB88")]
    fn reset_parts(&mut self, f: FighterId, part: i32, joint: Option<DataRef>, blend: bool) {
        let kind = self.game.fighters[f.0].kind;
        let root_part = i32::from(
            self.data
                .u8(self.ft_data_x8(&self.game.fighters[f.0]).at(0x10)),
        );
        let mut stack = Vec::new();
        let mut i = part;
        let mut joint = joint;
        while let Some(desc) = joint {
            while self.part_mask(kind, i) != 0 {
                i += 1;
            }
            let bone = self.bone(f, i);
            let j = if blend { bone.x4_jobj2 } else { bone.joint };
            let jobjs = &mut self.game.objs.jobjs;
            if i == root_part {
                if bone_flag(&bone, 0) {
                    reset_translation(jobjs, self.data, j, Some(desc));
                    if blend {
                        jobj::clear_flags(jobjs, j, USE_QUATERNION);
                    }
                } else if !bone_flag(&bone, 5) {
                    reset_rotation_translation(jobjs, self.data, j, Some(desc));
                }
                self.unscale_joint(f, j);
            } else if bone_flag(&bone, 0) {
                reset_scale_translation(jobjs, self.data, j, Some(desc));
                if blend {
                    jobj::clear_flags(jobjs, j, USE_QUATERNION);
                }
            } else if !bone_flag(&bone, 5) {
                reset_pose(jobjs, self.data, j, Some(desc));
            }
            i += 1;
            joint = next_joint(self.data, desc, &mut stack);
        }
    }

    /// The joint of bone `i` an animation binds to: the second joint when blending.
    fn anim_joint(&self, f: FighterId, i: i32, blend: bool) -> Option<JObjId> {
        let bone = self.bone(f, i);
        if blend { bone.x4_jobj2 } else { bone.joint }
    }

    /// Binds the action's figatree to the parts, `ftAnim_8006FE08`: through the parts of the
    /// character it was made for when that's another (a copied ability's).
    #[doc(alias = "ftAnim_8006FE08")]
    fn bind_figatree(&mut self, f: FighterId, blend: bool) {
        let fp = &self.game.fighters[f.0];
        if fp.kind as u32 != x594_word(fp.x594) & 0x3F {
            self.bind_figatree_remapped(f, blend);
        } else {
            let tree = fp.x590.expect("no figatree");
            self.bind_own_figatree(f, blend, tree);
        }
    }

    /// Binds a figatree made for the fighter's own character, `ftAnim_8006F4C8`: one node per
    /// animated bone, skipping optional parts the animation doesn't name.
    #[doc(alias = "ftAnim_8006F4C8")]
    pub fn bind_own_figatree(&mut self, f: FighterId, blend: bool, tree: DataRef) {
        let data = self.data;
        let kind = self.game.fighters[f.0].kind;
        let flags = (x594_word(self.game.fighters[f.0].x594) >> 9) & 0x1FFF;
        let mut node = data.ptr(tree.at(0xC)).expect("figatree nodes");
        let mut track = data.ptr(tree.at(0x10)).expect("figatree tracks");
        let mut i = 0;
        loop {
            let frames = data.u8(node) as i8;
            if frames == -1 {
                break;
            }
            while !bone_flag(&self.bone(f, i), 1) {
                i += 1;
            }
            while bone_flag(&self.bone(f, i), 2) {
                let m = self.part_mask(kind, i);
                if m == 0 || m & flags != 0 {
                    break;
                }
                i += 1;
            }
            assert!(
                i < 140,
                "fighter animation joint over! player {}",
                self.game.fighters[f.0].player_idx
            );
            let bone = self.bone(f, i);
            if !bone_flag(&bone, 0) && !bone_flag(&bone, 5) {
                let j = self.anim_joint(f, i, blend);
                bind_tracks(&mut self.game.objs, data, j, tree, track, frames);
            }
            node = node.at(1);
            i += 1;
            track = track.at(frames as u32 * TRACK_SIZE);
        }
    }

    /// Binds a figatree made for another character (`x597_bits`), mapping its joints to the
    /// fighter's through their common parts, `ftAnim_8006FCE4`.
    #[doc(alias = "ftAnim_8006FCE4")]
    pub fn bind_figatree_remapped(&mut self, f: FighterId, blend: bool) {
        let data = self.data;
        let fp = &self.game.fighters[f.0];
        let tree = fp.x590.expect("no figatree");
        let word = x594_word(fp.x594);
        let (bits, from) = ((word >> 9) & 0x1FFF, (word & 0x3F) as i32);
        let to = fp.kind;
        let mut node = data.ptr(tree.at(0xC)).expect("figatree nodes");
        let mut track = data.ptr(tree.at(0x10)).expect("figatree tracks");
        let mut i = 0;
        loop {
            let frames = data.u8(node) as i8;
            if frames == -1 {
                break;
            }
            loop {
                let m = self.part_mask(from, i);
                if m == 0 || m & bits != 0 {
                    break;
                }
                i += 1;
            }
            let part = self.parts_remap(to, from, i as u32);
            if part != INVALID {
                let part = i32::from(part);
                let bone = self.bone(f, part);
                if bone_flag(&bone, 1) && !bone_flag(&bone, 0) && !bone_flag(&bone, 5) {
                    let j = self.anim_joint(f, part, blend);
                    if bone_flag(&bone, 3) {
                        bind_tracks(&mut self.game.objs, data, j, tree, track, frames);
                    } else {
                        bind_tracks_no_translation(
                            &mut self.game.objs,
                            data,
                            j,
                            tree,
                            track,
                            frames,
                        );
                    }
                }
            }
            track = track.at(frames as u32 * TRACK_SIZE);
            i += 1;
            node = node.at(1);
        }
    }

    /// Requests a joint tree's own animations from `frame`, `ftAnim_80070710`.
    #[doc(alias = "ftAnim_80070710")]
    fn request_anim(&mut self, j: Option<JObjId>, frame: f32) {
        jobj::req_anim_all_by_flags(&mut self.game.objs, j, 1, frame);
    }

    /// Removes a joint tree's own animations, `ftAnim_80070758`.
    #[doc(alias = "ftAnim_80070758")]
    pub fn remove_anim(&mut self, j: Option<JObjId>) {
        jobj::remove_anim_all_by_flags(&mut self.game.objs, j, 1);
    }

    /// Makes a tree's animations loop (`HSD_AObjSetFlags` with `AOBJ_LOOP`) or play at `rate`
    /// (`HSD_AObjSetRate`), through `HSD_ForeachAnim`.
    fn tree_anims(&mut self, j: Option<JObjId>, set: impl Fn(&mut aobj::AObjs, aobj::AObjId)) {
        let Some(j) = j else { return };
        jobj::foreach_anim(&mut self.game.objs, j, ANIM_MASK, &mut |objs, a| {
            set(&mut objs.aobjs, a)
        });
    }

    /// Starts the action's animation from `start` at `rate`, blending into it over `blend`
    /// frames (0: none), `ftAnim_8006EBE8`.
    #[doc(alias = "ftAnim_8006EBE8")]
    pub fn start_anim(&mut self, gobj: GObjId, start: f64, rate: f64, blend: f64) {
        let f = self.game.fighter_id(gobj);
        let root = Some(self.root_jobj(gobj));
        let skel = self.game.fighters[f.0].x8AC_animSkeleton;
        self.remove_anim(root);
        self.remove_anim(skel);
        let costume = self.game.fighters[f.0]
            .x108_costume_joint
            .expect("a fighter without a costume");
        let first = self.data.ptr(costume.at(8));
        let (start, rate32) = (mem(start), mem(rate));
        let looping = self.game.fighters[f.0].x594[0] & 0x40 != 0;

        let trees: &[Option<JObjId>] = if blend == 0.0 {
            self.reset_parts(f, TRANS_N, first, false);
            self.bind_figatree(f, false);
            &[root]
        } else {
            self.reset_parts(f, TRANS_N, first, true);
            self.bind_figatree(f, true);
            &[skel, root]
        };
        for &t in trees {
            self.request_anim(t, start);
        }
        if looping {
            for &t in trees {
                self.tree_anims(t, |aobjs, a| aobj::set_flags(aobjs, Some(a), aobj::LOOP));
            }
        }
        for &t in trees {
            self.tree_anims(t, |aobjs, a| aobj::set_rate(aobjs, Some(a), rate32));
        }
        let fp = &mut self.game.fighters[f.0];
        fp.x8A4_animBlendFrames = mem(blend);
        fp.x8A8_anim_frame = 0.0;
    }
}
