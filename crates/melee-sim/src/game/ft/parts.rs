//! Fighter parts (`ftparts.c`): the common part names (`FtPart_*`) every character's joints
//! map to, and setting and reading a part's rotation.

use super::super::state::{Game, Running};
use super::super::structs::Fighter;
use crate::hsd::jobj::{self, JObjId, USE_QUATERNION};

/// A part's index in a fighter's parts table: an unmapped part (`FTPART_INVALID`).
pub const INVALID: u8 = 0xFF;

/// `FtPart_TopN`, the fighter's root.
pub const TOP_N: i32 = 0;
/// `FtPart_TransN`.
pub const TRANS_N: i32 = 1;

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
    pub fn part_set_rot_y(&mut self, f: super::FighterId, part_idx: i32, v: f32) {
        let j = self.angle_joint(&self.fighters[f.0], part_idx, "set");
        jobj::set_rotation_y(&mut self.objs.jobjs, j, v);
    }

    /// `ftPartGetRotX`.
    #[doc(alias = "ftPartGetRotX")]
    pub fn part_rot_x(&self, f: super::FighterId, part_idx: i32) -> f32 {
        let j = self.angle_joint(&self.fighters[f.0], part_idx, "get");
        self.objs.jobjs[j].rotate.x
    }

    /// `ftPartSetRotX`.
    #[doc(alias = "ftPartSetRotX")]
    pub fn part_set_rot_x(&mut self, f: super::FighterId, part_idx: i32, v: f32) {
        let j = self.angle_joint(&self.fighters[f.0], part_idx, "set");
        jobj::set_rotation_x(&mut self.objs.jobjs, j, v);
    }

    /// `ftPartSetRotZ`.
    #[doc(alias = "ftPartSetRotZ")]
    pub fn part_set_rot_z(&mut self, f: super::FighterId, part_idx: i32, v: f32) {
        let j = self.angle_joint(&self.fighters[f.0], part_idx, "set");
        jobj::set_rotation_z(&mut self.objs.jobjs, j, v);
    }
}
