//! Fighter parts (`ftparts.c`): the common part names (`FtPart_*`) every character's joints
//! map to, and setting and reading a part's rotation.

use super::super::handles::{DObjListId, FighterId, PartsId};
use super::super::state::{Game, Running};
use super::super::structs::{Fighter, FighterBone};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId, USE_QUATERNION};
use crate::hsd::mobj::MObjClass;
use crate::hsd::tobj::TObjId;

/// A part's index in a fighter's parts table: an unmapped part (`FTPART_INVALID`).
pub const INVALID: u8 = 0xFF;

/// `FtPart_TopN`, the fighter's root.
pub const TOP_N: i32 = 0;
/// `FtPart_TransN`.
pub const TRANS_N: i32 = 1;
/// `FtPart_XRotN`, `FtPart_YRotN`, `FtPart_HipN`, `FtPart_TransN2`.
pub const XROT_N: i32 = 2;
pub const YROT_N: i32 = 3;
pub const HIP_N: i32 = 4;
pub const TRANS_N2: i32 = 0x34;

impl Running<'_> {
    /// The fighter's joint index for a common part, `ftParts_GetBoneIndex`.
    #[doc(alias = "ftParts_GetBoneIndex")]
    pub fn bone_index(&self, fp: &Fighter, part: i32) -> i32 {
        i32::from(self.parts_table(fp.kind).joint_of(part as u32))
    }
}

impl Game {
    /// The joints of bone `part_idx` of the fighter: `joint` and `x4_jobj2`.
    fn part_joints(&self, fp: &Fighter, part_idx: i32) -> (JObjId, Option<JObjId>) {
        let parts = &self.fighter_parts[fp.parts.expect("a fighter without parts").0];
        let bone = &parts[part_idx as usize];
        (bone.joint.expect("a part without a joint"), bone.x4_jobj2)
    }

    /// The joint whose rotation a part's angles set: its own, or for a joint animated by
    /// quaternion, its second joint, which the original asserts isn't one too.
    fn angle_joint(&self, fp: &Fighter, part_idx: i32, what: &str) -> JObjId {
        let (j, j2) = self.part_joints(fp, part_idx);
        if self.objs.jobjs[j].flags & USE_QUATERNION == 0 {
            return j;
        }
        let j2 = j2.expect("a quaternion part without a second joint");
        assert!(
            self.objs.jobjs[j2].flags & USE_QUATERNION == 0,
            "cant {what} fighter rot!"
        );
        j2
    }

    /// `ftPartSetRotY`.
    #[doc(alias = "ftPartSetRotY")]
    pub fn part_set_rot_y(&mut self, f: FighterId, part_idx: i32, v: f32) {
        let j = self.angle_joint(&self.fighters[f.0], part_idx, "set");
        jobj::set_rotation_y(&mut self.objs.jobjs, j, v);
    }

    /// `ftPartGetRotZ`, which reads the joint's y rotation.
    #[doc(alias = "ftPartGetRotZ")]
    pub fn part_rot_y(&self, f: FighterId, part_idx: i32) -> f32 {
        let j = self.angle_joint(&self.fighters[f.0], part_idx, "get");
        self.objs.jobjs[j].rotate.y
    }

    /// `ftPartGetRotX`.
    #[doc(alias = "ftPartGetRotX")]
    pub fn part_rot_x(&self, f: FighterId, part_idx: i32) -> f32 {
        let j = self.angle_joint(&self.fighters[f.0], part_idx, "get");
        self.objs.jobjs[j].rotate.x
    }

    /// `ftPartSetRotX`.
    #[doc(alias = "ftPartSetRotX")]
    pub fn part_set_rot_x(&mut self, f: FighterId, part_idx: i32, v: f32) {
        let j = self.angle_joint(&self.fighters[f.0], part_idx, "set");
        jobj::set_rotation_x(&mut self.objs.jobjs, j, v);
    }

    /// `ftPartSetRotZ`.
    #[doc(alias = "ftPartSetRotZ")]
    pub fn part_set_rot_z(&mut self, f: FighterId, part_idx: i32, v: f32) {
        let j = self.angle_joint(&self.fighters[f.0], part_idx, "set");
        jobj::set_rotation_z(&mut self.objs.jobjs, j, v);
    }
}

/// `MAX_FT_PARTS`: the bones a parts table has room for.
const MAX_PARTS: u32 = 140;
/// The DObjs a fighter's model and second model have room for.
pub const MAX_DOBJS: usize = 124;
pub const MAX_SECOND_DOBJS: usize = 32;

/// Sets or clears the bit of a part's flags that the decomp names `x8.x0.flags_b{bit}`
/// (`byte` 0) or `flags2_b{bit}` (`byte` 1), bits counted from the most significant.
pub fn set_bone_bit(b: &mut FighterBone, byte: usize, bit: u32, v: bool) {
    let m = 0x80u8 >> bit;
    if v {
        b.x8[byte] |= m;
    } else {
        b.x8[byte] &= !m;
    }
}

/// The part's flags from its joint's (lighting, texture generation, specular and three
/// others) and its depth in the tree, the head of `ftParts_80074194`.
pub fn bone_from_joint(bone: &mut FighterBone, flags: u32, depth: u32) {
    set_bone_bit(bone, 0, 1, true);
    set_bone_bit(bone, 0, 7, flags & (1 << 7) != 0);
    set_bone_bit(bone, 1, 0, flags & (1 << 8) != 0);
    set_bone_bit(bone, 1, 1, flags & (1 << 16) != 0);
    set_bone_bit(bone, 1, 2, flags & (1 << 18) != 0);
    set_bone_bit(bone, 1, 3, flags & (1 << 20) != 0);
    set_bone_bit(bone, 1, 4, flags & (1 << 19) != 0);
    bone.xC_u[0] = depth as u8;
}

/// Sets a part's last DObj index, the seven-bit `xC_u.x0.xD` (the top bits of its byte).
pub fn set_bone_xd(b: &mut FighterBone, v: u8) {
    b.xC_u[1] = (b.xC_u[1] & 1) | (v << 1);
}

/// The walk the parts take over a joint tree: the next joint after `j` depth first (an
/// instance's child not entered), and how the depth changes.
fn next_in_tree(jobjs: &crate::hsd::jobj::JObjs, j: JObjId) -> (Option<JObjId>, i32) {
    let o = &jobjs[j];
    if o.flags & jobj::INSTANCE == 0
        && let Some(c) = o.child
    {
        return (Some(c), 1);
    }
    if let Some(n) = o.next {
        return (Some(n), 0);
    }
    let mut cur = j;
    let mut depth = 0;
    loop {
        let Some(p) = jobjs[cur].parent else {
            return (None, depth);
        };
        depth -= 1;
        if let Some(n) = jobjs[p].next {
            return (Some(n), depth);
        }
        cur = p;
    }
}

impl Running<'_> {
    /// Gives the fighter its parts table and DObj list, flags cleared, and marks the parts
    /// that move it as a whole, `ftParts_80074E58`.
    #[doc(alias = "ftParts_80074E58")]
    pub fn init_parts(&mut self, f: FighterId) {
        let n = self.parts_table(self.game.fighters[f.0].kind).len();
        let bones = vec![FighterBone::default(); n as usize];
        let parts = PartsId(self.game.fighter_parts.alloc(bones));
        let list = DObjListId(self.game.fighter_dobj_lists.alloc(vec![None; MAX_DOBJS]));
        let fp = &mut self.game.fighters[f.0];
        fp.parts = Some(parts);
        fp.dobj_list.data = Some(list);
        let fp = &self.game.fighters[f.0];
        let idx = |part| self.bone_index(fp, part) as usize;
        let marked = [
            (0, 3),
            (idx(TRANS_N), 3),
            (idx(XROT_N), 3),
            (idx(YROT_N), 3),
            (idx(HIP_N), 3),
            (idx(TRANS_N2), 3),
            (idx(TRANS_N), 4),
            (idx(0x35), 4),
        ];
        let bones = &mut self.game.fighter_parts[parts.0];
        for (i, bit) in marked {
            set_bone_bit(&mut bones[i], 0, bit, true);
        }
    }

    /// Fills the fighter's parts from its model's joints, depth first, and lists its DObjs
    /// (each material of the fighters' class), `ftParts_SetupParts` (with
    /// `ftParts_80074194`). Parts the character leaves out (`ftParts_8007506C`) get no joint.
    #[doc(alias = "ftParts_SetupParts")]
    #[doc(alias = "ftParts_80074194")]
    pub fn setup_parts(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let kind = self.game.fighters[f.0].kind;
        let n = self.parts_table(kind).len();
        assert!(
            n <= MAX_PARTS,
            "the original's assertion failed: fighter parts num over!"
        );
        let fp = &self.game.fighters[f.0];
        let parts = fp.parts.expect("a fighter without parts").0;
        let list = fp.dobj_list.data.expect("a fighter without a DObj list").0;
        let mut j = Some(self.root_jobj(gobj));
        let (mut part, mut depth, mut count) = (0u32, 0i32, 0usize);
        while let Some(cur) = j {
            if self.part_mask(kind, part as i32) != 0 {
                self.game.fighter_parts[parts][part as usize].joint = None;
                part += 1;
                continue;
            }
            // `ftParts_80074194`.
            let flags = self.game.objs.jobjs[cur].flags;
            let bone = &mut self.game.fighter_parts[parts][part as usize];
            bone.joint = Some(cur);
            bone_from_joint(bone, flags, depth as u32);
            let mut d = self.game.objs.jobjs[cur].dobj();
            let mut own = 0;
            while let Some(dobj) = d {
                assert!(
                    count < MAX_DOBJS,
                    "the original's assertion failed: fighter parts model dobj num over!"
                );
                self.game.fighter_dobj_lists[list][count] = Some(dobj);
                if let Some(m) = self.game.objs.dobjs[dobj].mobj {
                    self.game.objs.mobjs[m].class = MObjClass::Fighter;
                }
                d = self.game.objs.dobjs[dobj].next;
                own += 1;
                count += 1;
            }
            let bone = &mut self.game.fighter_parts[parts][part as usize];
            let xd = if count != 0 { (count - 1) as u8 } else { 0 };
            set_bone_xd(bone, xd);
            set_bone_bit(bone, 1, 6, own != 0);
            part += 1;
            let (next, dd) = next_in_tree(&self.game.objs.jobjs, cur);
            j = next;
            depth += dd;
        }
        self.game.fighters[f.0].dobj_list.count = count as u32;
        assert!(
            part == n,
            "the original's assertion failed: fighter parts num not match!"
        );
    }

    /// Points each part's second joint at the blend skeleton's joint for it,
    /// `ftParts_8007462C`.
    #[doc(alias = "ftParts_8007462C")]
    pub fn setup_blend_parts(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let kind = self.game.fighters[f.0].kind;
        let parts = self.game.fighters[f.0]
            .parts
            .expect("a fighter without parts")
            .0;
        let mut j = self.game.fighters[f.0].x8AC_animSkeleton;
        let mut i = 0u32;
        while let Some(cur) = j {
            if self.part_mask(kind, i as i32) != 0 {
                self.game.fighter_parts[parts][i as usize].x4_jobj2 = None;
                i += 1;
                continue;
            }
            self.game.fighter_parts[parts][i as usize].x4_jobj2 = Some(cur);
            i += 1;
            j = next_in_tree(&self.game.objs.jobjs, cur).0;
        }
        assert!(
            i == self.parts_table(kind).len(),
            "the original's assertion failed: fighter parts num not match!"
        );
    }

    /// The fighter's model variants (`ftData::x8->x0`) for its costume, every variant's DObjs
    /// hidden, and none chosen, `ftParts_800749CC` (with `ftParts_8007487C`).
    #[doc(alias = "ftParts_800749CC")]
    #[doc(alias = "ftParts_8007487C")]
    pub fn init_parts_vis(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let data = fp.ft_data.expect("a fighter without data");
        let desc = self.data.ptr(data.at(8)).expect("ftData::x8");
        let costume = u32::from(fp.costume_id);
        let model_num = self.data.u32(desc);
        assert!(
            model_num <= 11,
            "the original's assertion failed: fighter parts model num over!"
        );
        let table = self.data.ptr(desc.at(4)).expect("FtPartsDesc::vis_table");
        let mut xc = [None; 5];
        for k in 0..4u32 {
            let mine = self.data.ptr(table.at(0x10 * costume + 4 * k));
            xc[k as usize] = mine.or_else(|| self.data.ptr(table.at(4 * k)));
        }
        let fp = &mut self.game.fighters[f.0];
        fp.x5AC.model_num = model_num;
        fp.x5AC.xC = xc;
        fp.x5AC.cleared = [1; 5];
        let (main, second) = (fp.dobj_list.data, fp.x203C.data);
        for (idx, list) in [(0, main), (1, main), (2, second), (3, main), (4, main)] {
            self.hide_parts_vis(f, idx, list);
        }
        let fp = &mut self.game.fighters[f.0];
        for i in 0..fp.x5AC.model_num as usize {
            fp.x5F4_arr[i].prev = -1;
        }
        self.game.clear_part_vis(gobj);
    }

    /// Shows the DObjs of each model group's current choice and hides the rest, unless the
    /// lookup has been shown, `ftParts_80074B6C`. The original marks the lookup shown inside
    /// the group loop, so a lookup with no groups stays unshown.
    #[doc(alias = "ftParts_80074B6C")]
    pub fn show_parts_vis(&mut self, f: FighterId, idx: usize, list: Option<DObjListId>) {
        let vis = &self.game.fighters[f.0].x5AC;
        let Some(lookup) = vis.xC[idx] else { return };
        if vis.cleared[idx] != 0 {
            return;
        }
        let model_num = vis.model_num;
        for i in 0..model_num {
            let chosen = i32::from(self.game.fighters[f.0].x5F4_arr[i as usize].idx);
            let entry = lookup.at(8 * i);
            let n = self.data.u32(entry) as i32;
            for j in 0..n {
                let t = self
                    .data
                    .ptr(entry.at(4))
                    .expect("FtPartsVisLookup::x4")
                    .at(8 * j as u32);
                let count = self.data.u32(t) as i32;
                let bytes = self.data.ptr(t.at(4));
                for k in 0..count {
                    let b = bytes.expect("TempS::x4").at(k as u32);
                    let index = usize::from(self.data.u8(b));
                    let l = list.expect("a fighter without a DObj list");
                    let d = self.game.fighter_dobj_lists[l.0][index];
                    let dobjs = &mut self.game.objs.dobjs;
                    if j == chosen {
                        crate::hsd::dobj::clear_flags(dobjs, d, 1);
                    } else {
                        crate::hsd::dobj::set_flags(dobjs, d, 1);
                    }
                }
            }
            self.game.fighters[f.0].x5AC.cleared[idx] = 1;
        }
    }

    /// Shows (`show`) or hides a model group, `ftParts_800750C8`: group 2 (the low-poly
    /// models) covers lookups 2 (on the second DObj list) and 3, each followed by the kind's
    /// model event (Jigglypuff's hat; Kirby's no port covers yet).
    #[doc(alias = "ftParts_800750C8")]
    pub fn set_parts_vis(&mut self, f: FighterId, group: usize, show: bool) {
        let (main, second) = {
            let fp = &self.game.fighters[f.0];
            (fp.dobj_list.data, fp.x203C.data)
        };
        let lists: &[(usize, Option<DObjListId>)] = if group == 2 {
            &[(2, second), (3, main)]
        } else {
            &[(group, main)]
        };
        for &(idx, list) in lists {
            if show {
                self.show_parts_vis(f, idx, list);
            } else {
                self.hide_parts_vis(f, idx, list);
            }
            // The kind's model event (`ftData_UnkIntBoolFunc0`) after each lookup.
            match self.game.fighters[f.0].kind {
                4 => crate::game::code::unported_call("ftKb_UnkIntBoolFunc0"),
                15 => self.purin_hat_vis(f, idx, show),
                _ => {}
            }
        }
    }

    /// Hides every DObj a visibility lookup names unless it has, `ftParts_80074D7C`.
    #[doc(alias = "ftParts_80074D7C")]
    pub fn hide_parts_vis(&mut self, f: FighterId, idx: usize, list: Option<DObjListId>) {
        let vis = &self.game.fighters[f.0].x5AC;
        let Some(lookup) = vis.xC[idx] else { return };
        if vis.cleared[idx] == 0 {
            return;
        }
        let model_num = vis.model_num;
        for i in 0..model_num {
            let entry = lookup.at(8 * i);
            let n = self.data.u32(entry) as i32;
            for j in 0..n {
                let t = self
                    .data
                    .ptr(entry.at(4))
                    .expect("FtPartsVisLookup::x4")
                    .at(8 * j as u32);
                let count = self.data.u32(t) as i32;
                let bytes = self.data.ptr(t.at(4));
                for k in 0..count {
                    let b = bytes.expect("TempS::x4").at(k as u32);
                    let index = usize::from(self.data.u8(b));
                    let l = list.expect("a fighter without a DObj list");
                    let d = self.game.fighter_dobj_lists[l.0][index];
                    crate::hsd::dobj::set_flags(&mut self.game.objs.dobjs, d, 1);
                }
            }
        }
        self.game.fighters[f.0].x5AC.cleared[idx] = 0;
    }

    /// The TObj `n` (counted across the DObjs' materials in order) of a DObj list,
    /// `ftParts_80075240`; the original asserts there is one.
    #[doc(alias = "ftParts_80075240")]
    pub fn nth_tobj(&self, list: Option<DObjListId>, count: u32, n: i32) -> TObjId {
        let l = list.expect("a fighter without a DObj list");
        let mut i = 0;
        for k in 0..count as usize {
            let Some(d) = self.game.fighter_dobj_lists[l.0][k] else {
                continue;
            };
            let Some(m) = self.game.objs.dobjs[d].mobj else {
                continue;
            };
            let mut t = crate::hsd::mobj::tobj(&self.game.objs.mobjs, Some(m));
            while let Some(c) = t {
                if i == n {
                    return c;
                }
                i += 1;
                t = crate::hsd::tobj::next(&self.game.objs.tobjs, Some(c));
            }
        }
        panic!("the original's assertion failed: can't find fighter texture!");
    }
}

impl Game {
    /// Chooses a model variant, `ftParts_80074A4C`.
    #[doc(alias = "ftParts_80074A4C")]
    pub fn set_part_vis(&mut self, gobj: GObjId, model: usize, val: i32) {
        let fp = self.fighter_mut(gobj);
        fp.x5F4_arr[model].prev = val as i8;
        fp.x221D_b2 = true;
    }

    /// Chooses no model variant, `ftParts_80074ACC`.
    #[doc(alias = "ftParts_80074ACC")]
    pub fn clear_part_vis(&mut self, gobj: GObjId) {
        let fp = self.fighter_mut(gobj);
        for i in 0..fp.x5AC.model_num as usize {
            fp.x5F4_arr[i].idx = -1;
        }
        fp.x221D_b2 = false;
    }
}
