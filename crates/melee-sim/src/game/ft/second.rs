//! The fighter's second animation: a state's own blend animation (the fall's lean, the shield's
//! tilt) loaded into a buffer of its own (`ftData_80085E50`) and bound to the blend skeleton
//! (`ftanim.c`), and blends toward a joint description's pose.

use melee_data::archive::{ArchiveId, DataRef};

use super::super::code::unported_call;
use super::super::handles::FighterId;
use super::super::mirror::{Cx, Mem, Mirror};
use super::super::state::Running;
use super::anim::bone_flag;
use super::anim_buf::{ARCHIVE_HEADER, SIZE};
use super::parts::{TOP_N, TRANS_N};
use crate::hsd::aobj;
use crate::hsd::jobj;
use crate::lb::figatree::{TRACK_SIZE, bind_tracks};

/// The object types `HSD_ForeachAnim` reaches here: all but materials and textures.
const ANIM_MASK: u32 = 0xFB7F;

/// The second animation's figatree (`Fighter::x598`), a pointer into one of the fighter's two
/// animation buffers: the second (`x5A0`), or the main one (`x59C`) when Nana copied the
/// archive from Popo's buffer, which `ftData_80085E50` copies into her main buffer. There the
/// next main animation overwrites it, and the pointer, unchanged, then points into that
/// animation's archive at the same offset (docs/engine.md).
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct SecondTree {
    pub tree: Option<DataRef>,
    /// Whether the figatree is in the main buffer.
    pub in_main: bool,
}

/// Where `addr` is in the fighter's main animation buffer, which the word after `x598`
/// (`x59C`) points to, if it's there.
fn main_buffer_offset(m: &Mem, at: u32, addr: u32) -> Option<u32> {
    let off = addr.wrapping_sub(m.u32(at + 4));
    (addr != 0 && off < SIZE).then_some(off)
}

impl crate::game::mirror::LayOut for SecondTree {
    /// A figatree outside the main buffer by its address; one in it stays unknown (its address
    /// is the buffer's, which the fighter's layout doesn't pass).
    fn lay(&self, l: &mut crate::hsd::export::Lay, at: u32, cx: &dyn crate::game::mirror::LayCx) {
        let w = if self.in_main {
            None
        } else {
            cx.data(self.tree)
        };
        l.put_word(at, w);
    }
}

impl Mirror for SecondTree {
    const SIZE: u32 = 4;

    /// A figatree in the main buffer is taken as the offset it's at in the archive the action's
    /// figatree (`x590`) names, which may be past that archive's data once a main animation
    /// overwrote the copy.
    fn import(m: &Mem, at: u32, cx: &mut dyn Cx) -> Self {
        let w = m.u32(at);
        if let Some(off) = main_buffer_offset(m, at, w)
            && let Some(main) = cx.import_data(m.u32(at - 8))
        {
            return SecondTree {
                tree: Some(DataRef {
                    archive: main.archive,
                    offset: off.wrapping_sub(ARCHIVE_HEADER),
                }),
                in_main: true,
            };
        }
        SecondTree {
            tree: cx.import_data(w),
            in_main: false,
        }
    }

    /// One in the main buffer compares by its offset there (the action's figatree, compared
    /// by itself, names the archive the buffer holds).
    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        let theirs = m.u32(at);
        match (self.in_main, self.tree) {
            (true, Some(t)) => {
                let ours = t.offset.wrapping_add(ARCHIVE_HEADER);
                if main_buffer_offset(m, at, theirs) != Some(ours) {
                    cx.mismatch(
                        path,
                        format!("{t:?} in the main buffer at {ours:#x}"),
                        format!("{theirs:#x}"),
                    );
                }
            }
            _ => {
                if self.tree.is_some() && main_buffer_offset(m, at, theirs).is_some() {
                    cx.mismatch(
                        path,
                        format!("{:?} in the second buffer", self.tree),
                        format!("{theirs:#x} in the main buffer"),
                    );
                } else {
                    cx.compare_data(path, self.tree, theirs);
                }
            }
        }
    }
}

impl Running<'_> {
    /// Loads animation `msid`'s figatree into the fighter's second buffer unless it's there
    /// already, `ftData_80085E50`: `x598` then holds the figatree, `x5A8` where it came from.
    /// None past the character's animations.
    #[doc(alias = "ftData_80085E50")]
    pub fn second_figatree(&mut self, f: FighterId, msid: i32) -> Option<DataRef> {
        let fp = &self.game.fighters[f.0];
        if msid as u32 >= fp.x58C {
            return None;
        }
        let entry = self.anim_entry(f, msid);
        let aram = self.anim_aram(fp, entry);
        if aram != fp.x5A8 {
            let second = if aram != 0 {
                // Nana copies the archive from Popo's animation buffer when it holds it
                // (untimed), into her main buffer.
                let kind = self.anim_entry_kind(fp.kind, entry);
                let shared = self
                    .anim_share_source(f)
                    .is_some_and(|p| self.game.fighters[p.0].x5A4 == aram);
                let size = self.data.u32(entry.at(8));
                let tree = self.anim_tree(kind, entry);
                let fp = &self.game.fighters[f.0];
                if shared {
                    // `memcpy` copies the file's own size, into the main buffer.
                    self.anim_buf_load(fp.x59C, tree.archive, size);
                    self.overwrite_main_buffer(f, tree.archive);
                } else {
                    if aram >= 0x8000_0000 {
                        unported_call("ftData_80085E50 from main memory");
                    }
                    // `lbArq_80014BD0`'s copy out of ARAM, rounded up to 32 bytes, takes time
                    // on ssbm-rs's timeline.
                    let len = (size + 31) & !31;
                    self.anim_buf_load(fp.x5A0, tree.archive, len);
                    self.game.audio.aram_load(self.data, len);
                }
                SecondTree {
                    tree: Some(tree),
                    in_main: shared,
                }
            } else {
                SecondTree::default()
            };
            let fp = &mut self.game.fighters[f.0];
            fp.x598 = second;
            fp.x5A8 = aram;
        }
        self.game.fighters[f.0].x598.tree
    }

    /// The fighter's main animation buffer now holds `archive`: the figatrees that point into
    /// it, the action's (`x590`) and a second one copied there, point into `archive` at the
    /// same offsets.
    pub(super) fn overwrite_main_buffer(&mut self, f: FighterId, archive: ArchiveId) {
        let fp = &mut self.game.fighters[f.0];
        if let Some(t) = &mut fp.x590 {
            t.archive = archive;
        }
        if fp.x598.in_main
            && let Some(t) = &mut fp.x598.tree
        {
            t.archive = archive;
        }
    }

    /// Binds a figatree to the blend skeleton from part `part` on, `ftAnim_80070A10`: the
    /// tracks before the part are skipped, and binding stops at the first part outside the
    /// part's subtree.
    #[doc(alias = "ftAnim_80070A10")]
    pub fn bind_second_figatree(&mut self, f: FighterId, part: i32, tree: DataRef) {
        let data = self.data;
        let fp = &self.game.fighters[f.0];
        let kind = fp.kind;
        let flags = (super::bind::x594_word(fp.x594) >> 9) & 0x1FFF;
        let mut node = data.ptr(tree.at(0xC)).expect("figatree nodes");
        let mut track = data.ptr(tree.at(0x10)).expect("figatree tracks");
        let depth = self.bone(f, part).xC_u[0];
        let buf = if fp.x598.in_main { fp.x59C } else { fp.x5A0 };
        let mut i = 0;
        // As in `ftAnim_8006F7C8`, past the node list's end the skip reads on in the buffer.
        while i < part {
            let m = self.part_mask(kind, i);
            if m == 0 || m & flags != 0 {
                track = track.at(self.anim_buf_u8(buf, node) as i8 as u32 * TRACK_SIZE);
                node = node.at(1);
            }
            i += 1;
        }
        let mut b = i;
        loop {
            let frames = self.anim_buf_u8(buf, node) as i8;
            if frames == -1 {
                break;
            }
            while !bone_flag(&self.bone(f, b), 1) {
                i += 1;
                b += 1;
            }
            let mut r = i;
            loop {
                if !bone_flag(&self.bone(f, r), 2) {
                    break;
                }
                let m = self.part_mask(kind, i);
                if m == 0 || m & flags != 0 {
                    break;
                }
                r += 1;
                i += 1;
                b += 1;
            }
            let bone = self.bone(f, b);
            if bone.xC_u[0] <= depth && i != part {
                break;
            }
            if !bone_flag(&bone, 0) && !bone_flag(&bone, 5) {
                bind_tracks(
                    &mut self.game.objs,
                    data,
                    bone.x4_jobj2,
                    tree,
                    track,
                    frames,
                );
            }
            track = track.at(frames as u32 * TRACK_SIZE);
            i += 1;
            b += 1;
            node = node.at(1);
        }
    }

    /// Plays animation `msid` on the blend skeleton from frame `frame` at `rate`,
    /// `ftAnim_8006EDD0`: the skeleton's own animation removed, TransN reset to the model's,
    /// the figatree bound from TopN on, looping if the animation says.
    #[doc(alias = "ftAnim_8006EDD0")]
    pub fn play_second_anim(&mut self, f: FighterId, msid: i32, frame: f32, rate: f32) {
        let skel = self.game.fighters[f.0].x8AC_animSkeleton;
        self.remove_anim(skel);
        let costume = self.game.fighters[f.0].x108_costume_joint;
        let child = costume.and_then(|c| self.data.ptr(c.at(0x8)));
        self.reset_parts(f, TRANS_N, child, true);
        let tree = self
            .second_figatree(f, msid)
            .expect("ftAnim_80070A10 with no figatree");
        self.bind_second_figatree(f, TOP_N, tree);
        self.request_anim(skel, frame);
        let entry = self.anim_entry(f, msid);
        let flags = self.data.u32(entry.at(0x10));
        if let Some(j) = skel {
            if flags & 0x4000_0000 != 0 {
                jobj::foreach_anim(&mut self.game.objs, j, ANIM_MASK, &mut |objs, a| {
                    aobj::set_flags(&mut objs.aobjs, Some(a), aobj::LOOP)
                });
            }
            jobj::foreach_anim(&mut self.game.objs, j, ANIM_MASK, &mut |objs, a| {
                aobj::set_rate(&mut objs.aobjs, Some(a), rate)
            });
        }
    }

    /// Blends parts toward a joint description's pose, `ftAnim_80070010` (the parts' joints)
    /// and `ftAnim_80070108` (their blend joints): a part that doesn't animate rotation takes
    /// the description's pose outright (`lb_8000B4FC`), the others blend by `t`
    /// (`lb_8000C868`).
    #[doc(alias = "ftAnim_80070010")]
    #[doc(alias = "ftAnim_80070108")]
    pub fn blend_parts_to_desc(
        &mut self,
        f: FighterId,
        start: i32,
        t: f64,
        t_inv: f64,
        joint: Option<DataRef>,
        blend: bool,
    ) {
        let kind = self.game.fighters[f.0].kind;
        let mut stack = super::bind::JointStack::new();
        let mut i = start;
        let mut joint = joint;
        while let Some(desc) = joint {
            while self.part_mask(kind, i) != 0 {
                i += 1;
            }
            let bone = self.bone(f, i);
            if !bone_flag(&bone, 0) && !bone_flag(&bone, 5) {
                let j = if blend { bone.x4_jobj2 } else { bone.joint };
                let jobjs = &mut self.game.objs.jobjs;
                if bone_flag(&bone, 4) {
                    crate::lb::anim::reset_pose(jobjs, self.data, j, Some(desc));
                } else if let Some(j) = j {
                    crate::lb::anim::blend_from_desc(jobjs, self.data, desc, j, j, t, t_inv);
                }
            }
            i += 1;
            joint = super::bind::next_joint(self.data, desc, &mut stack);
        }
    }
}
