//! Joints: moving, hiding, enabling and linking the groups of lines that make up a stage's
//! parts.

use gekko_fp as fp;

use super::{
    CEILING, FLOOR, GROUP_DYNAMIC, GROUP_FLOOR, Geometry, JOINT_ALWAYS_NEAR, JOINT_ENABLED,
    JOINT_HIDDEN, JOINT_MOVED, JOINT_NO_ISLANDS, JOINT_ROTATED, KIND_MASK, Kinds, LEFT_WALL,
    LINE_DYNAMIC_PLATFORM, LINE_ENABLED, LINE_HIDDEN, LINE_PLATFORM, RIGHT_WALL, Vec2,
};
use crate::fpu::{copy, mem, reg};
use crate::mtx::Mtx34;
use crate::vec::Vec3;

/// The model joint a collision joint follows, as `mpLib_80055E9C` reads it: hidden
/// (`JOBJ_HIDDEN`), or shown with its world matrix after `HSD_JObjSetupMatrix`.
#[derive(Clone, Copy, Debug, PartialEq, melee_derive::StateEq)]
pub enum JointPose {
    Hidden,
    Visible(Mtx34),
}

/// Walking a model's joint tree, for `mpLib_800552B0`.
pub trait JObjTree<J> {
    fn child(&self, j: J) -> Option<J>;
    fn next(&self, j: J) -> Option<J>;
    fn parent(&self, j: J) -> Option<J>;
    fn flags(&self, j: J) -> u32;
}

/// `JOBJ_INSTANCE`: a joint whose children the walk doesn't enter.
const JOBJ_INSTANCE: u32 = 1 << 12;

impl<K: Kinds> Geometry<K> {
    /// Sets or clears `LINE_HIDDEN` on every line of a joint.
    fn set_lines_hidden(&mut self, joint: usize, hidden: bool) {
        for group in [GROUP_FLOOR, 1, 3, 2, GROUP_DYNAMIC] {
            for l in self.group(joint, group) {
                if hidden {
                    self.lines[l].flags |= LINE_HIDDEN;
                } else {
                    self.lines[l].flags &= !LINE_HIDDEN;
                }
            }
        }
    }

    /// Sets or clears `LINE_ENABLED` on every line of a joint.
    fn set_lines_enabled(&mut self, joint: usize, enabled: bool) {
        for group in [GROUP_FLOOR, 1, 3, 2, GROUP_DYNAMIC] {
            for l in self.group(joint, group) {
                if enabled {
                    self.lines[l].flags |= LINE_ENABLED;
                } else {
                    self.lines[l].flags &= !LINE_ENABLED;
                }
            }
        }
    }

    /// The vertex range of a joint, as the islands take it.
    fn vtx_range(&self, joint: usize) -> (i32, i32) {
        let d = &self.joints[joint].desc;
        (i32::from(d.vtx_start), i32::from(d.vtx_count))
    }

    /// Updates the islands for a joint, live as its flags say.
    fn refresh_islands(&mut self, joint: usize) {
        let live = self.islands_live(joint);
        let (start, count) = self.vtx_range(joint);
        self.update_islands(joint as i32, start, count, live);
    }

    /// Binds a joint to the `z`th joint below `root` in its model, depth first, not entering
    /// instances' children, `mpLib_800552B0`. Nothing changes if the model has fewer.
    #[doc(alias = "mpLib_800552B0")]
    pub fn bind_jobj<T: JObjTree<K::JObj>>(
        &mut self,
        joint_id: i32,
        tree: &T,
        root: Option<K::JObj>,
        z: i32,
    ) {
        let mut cur = root.and_then(|r| tree.child(r));
        let mut i = 0;
        while let Some(j) = cur {
            if i == z {
                break;
            }
            i += 1;
            if tree.flags(j) & JOBJ_INSTANCE == 0
                && let Some(c) = tree.child(j)
            {
                cur = Some(c);
                continue;
            }
            if let Some(n) = tree.next(j) {
                cur = Some(n);
                continue;
            }
            let mut up = j;
            cur = loop {
                match tree.parent(up) {
                    None => break None,
                    Some(p) => match tree.next(p) {
                        Some(n) => break Some(n),
                        None => up = p,
                    },
                }
            };
        }
        if let Some(j) = cur {
            self.joints[joint_id as usize].jobj = Some(j);
        }
    }

    /// Hides a joint and its lines, `mpJointHide`.
    #[doc(alias = "mpJointHide")]
    pub fn hide_joint(&mut self, joint_id: i32) {
        let j = joint_id as usize;
        self.joints[j].flags |= JOINT_HIDDEN;
        self.set_lines_hidden(j, true);
    }

    /// Shows a joint and its lines again, `mpJointUnhide`; its vertices don't remap from
    /// where they were while it was hidden.
    #[doc(alias = "mpJointUnhide")]
    pub fn unhide_joint(&mut self, joint_id: i32) {
        let j = joint_id as usize;
        self.joints[j].flags &= !JOINT_HIDDEN;
        self.set_lines_hidden(j, false);
        self.settle_vertices(joint_id);
    }

    /// Reclassifies a joint's dynamic lines by their slope, `mpJointUpdateDynamics`: steeper
    /// than 60° from a floor or 30° from a ceiling is a wall. Dynamic platforms are floors
    /// (and platforms) only while they face up.
    #[doc(alias = "mpJointUpdateDynamics")]
    pub fn update_dynamics(&mut self, joint_id: i32) {
        const TAN30: f64 = 0.577350295784245;
        const TAN60: f64 = 1.7320508368950045;
        let j = joint_id as usize;
        let joint_flags = self.joints[j].flags;
        for l in self.group(j, GROUP_DYNAMIC) {
            let line = self.lines[l];
            let (v0, v1) = (
                self.vertices[line.v0 as usize].pos,
                self.vertices[line.v1 as usize].pos,
            );
            let dx = fp::fsubs(reg(v1.x), reg(v0.x));
            let dy = fp::fsubs(reg(v1.y), reg(v0.y));
            let kind = if dx > 0.0 {
                if fp::fdivs(dy, dx) > TAN60 {
                    LEFT_WALL
                } else if fp::fdivs(dy, dx) < -TAN60 {
                    RIGHT_WALL
                } else {
                    FLOOR
                }
            } else if dx < 0.0 {
                if fp::fdivs(dy, dx) > TAN30 {
                    RIGHT_WALL
                } else if fp::fdivs(dy, dx) < -TAN30 {
                    LEFT_WALL
                } else {
                    CEILING
                }
            } else if dy > 0.0 {
                LEFT_WALL
            } else if dy < 0.0 {
                RIGHT_WALL
            } else {
                panic!("mplib.c: dynamic line {l} has no length");
            };
            let line = &mut self.lines[l];
            line.flags = (line.flags & !KIND_MASK) | kind;
            if joint_flags & JOINT_ENABLED != 0
                && u32::from(line.lo_flags) & LINE_DYNAMIC_PLATFORM != 0
            {
                if kind & FLOOR != 0 {
                    line.flags |= LINE_ENABLED | LINE_PLATFORM;
                    line.lo_flags |= LINE_PLATFORM as u16;
                } else {
                    line.flags &= !LINE_ENABLED;
                }
            }
        }
    }

    /// Reclassifies a joint's dynamic lines and updates its islands, `mpLib_80055E24`.
    #[doc(alias = "mpLib_80055E24")]
    pub fn update_dynamics_and_islands(&mut self, joint_id: i32) {
        self.update_dynamics(joint_id);
        self.refresh_islands(joint_id as usize);
    }

    /// Moves a joint's vertices to follow its model joint, `mpLib_80055E9C`. The vertices'
    /// positions become last frame's; an unbound joint stops there. A hidden model joint hides
    /// the collision joint. Otherwise the vertices take the model joint's transform (just a
    /// scale and translation when the scales match), its bounds follow with a 30-unit margin,
    /// its dynamic lines are reclassified, and it is shown. `pose` reads the bound model
    /// joint; it is called only for a bound joint.
    #[doc(alias = "mpLib_80055E9C")]
    pub fn update_joint(&mut self, joint_id: i32, pose: impl FnOnce(K::JObj) -> JointPose) {
        self.generation = self.generation.wrapping_add(1);
        let j = joint_id as usize;
        let (vtx_start, vtx_count) = self.vtx_range(j);
        let verts = vtx_start as usize..(vtx_start + vtx_count.max(0)) as usize;
        for v in &mut self.vertices[verts.clone()] {
            v.prev = Vec2 {
                x: copy(v.pos.x),
                y: copy(v.pos.y),
            };
        }
        let Some(jobj) = self.joints[j].jobj else {
            return;
        };
        let mtx = match pose(jobj) {
            JointPose::Hidden => {
                if self.joints[j].flags & JOINT_HIDDEN == 0 {
                    self.hide_joint(joint_id);
                    self.refresh_islands(j);
                }
                return;
            }
            JointPose::Visible(m) => m,
        };
        self.joints[j].dirty = true;
        let m = &mtx.m;
        let m00 = reg(m[0][0]);
        if m00 == reg(m[1][1]) && m00 == reg(m[2][2]) {
            let (tx, ty) = (reg(m[0][3]), reg(m[1][3]));
            for v in &mut self.vertices[verts] {
                v.pos.x = mem(fp::fmadds(reg(v.base.x), m00, tx));
                v.pos.y = mem(fp::fmadds(reg(v.base.y), m00, ty));
            }
            let joint = &mut self.joints[j];
            let d = joint.desc;
            joint.bounding_min.x = mem(fp::fsubs(fp::fmadds(reg(d.left), m00, tx), 30.0));
            joint.bounding_min.y = mem(fp::fsubs(fp::fmadds(reg(d.bottom), m00, ty), 30.0));
            joint.bounding_max.x = mem(fp::fadds(30.0, fp::fmadds(reg(d.right), m00, tx)));
            joint.bounding_max.y = mem(fp::fadds(30.0, fp::fmadds(reg(d.top), m00, ty)));
            joint.flags |= JOINT_MOVED;
        } else {
            for v in &mut self.vertices[verts] {
                let p = mtx.transform(Vec3::new(copy(v.base.x), copy(v.base.y), 0.0));
                v.pos.x = copy(p.x);
                v.pos.y = copy(p.y);
            }
            let joint = &mut self.joints[j];
            let off_diagonal = [m[0][1], m[0][2], m[1][0], m[1][2], m[2][0], m[2][1]];
            if off_diagonal.iter().any(|&x| reg(x) != 0.0) {
                joint.flags |= JOINT_ROTATED;
            }
            joint.flags |= JOINT_MOVED;
            if joint.flags & JOINT_ALWAYS_NEAR == 0 {
                let d = joint.desc;
                let at = |x: f32, y: f32| mtx.transform(Vec3::new(copy(x), copy(y), 0.0));
                let p = at(d.left, d.bottom);
                joint.bounding_min = Vec2 {
                    x: copy(p.x),
                    y: copy(p.y),
                };
                let p = at(d.right, d.top);
                joint.bounding_max = Vec2 {
                    x: copy(p.x),
                    y: copy(p.y),
                };
                if joint.flags & JOINT_ROTATED != 0 {
                    // The other two corners.
                    let c = at(d.right, d.bottom);
                    let (x30, y31) = (reg(c.x), reg(c.y));
                    let c = at(d.left, d.top);
                    let (x2, y3) = (reg(c.x), reg(c.y));
                    for (min, max, a, b) in [
                        (
                            &mut joint.bounding_min.x,
                            &mut joint.bounding_max.x,
                            x30,
                            x2,
                        ),
                        (
                            &mut joint.bounding_min.y,
                            &mut joint.bounding_max.y,
                            y31,
                            y3,
                        ),
                    ] {
                        let (lo, hi) = (reg(*min), reg(*max));
                        if lo > hi {
                            *min = mem(hi);
                        }
                        for x in [a, b] {
                            if reg(*min) > x {
                                *min = mem(x);
                            }
                        }
                        for x in [lo, a, b] {
                            if reg(*max) < x {
                                *max = mem(x);
                            }
                        }
                    }
                }
                joint.bounding_min.x = mem(fp::fsubs(reg(joint.bounding_min.x), 30.0));
                joint.bounding_max.x = mem(fp::fadds(reg(joint.bounding_max.x), 30.0));
                joint.bounding_min.y = mem(fp::fsubs(reg(joint.bounding_min.y), 30.0));
                joint.bounding_max.y = mem(fp::fadds(reg(joint.bounding_max.y), 30.0));
            }
        }
        // The original can stop early when a vertex didn't move (`mpLib_804D64CC`), but that
        // switch is never set.
        self.update_dynamics(joint_id);
        if self.joints[j].flags & JOINT_HIDDEN != 0 {
            self.unhide_joint(joint_id);
        }
        self.refresh_islands(j);
    }

    /// Widens a joint's bounds to its vertices with a 30-unit margin, `mpJointUpdateBounding`.
    #[doc(alias = "mpJointUpdateBounding")]
    pub fn update_bounding(&mut self, joint_id: i32) {
        let j = joint_id as usize;
        let (start, count) = self.vtx_range(j);
        for v in start..start + count.max(0) {
            let p = self.vertices[v as usize].pos;
            let joint = &mut self.joints[j];
            let (x, y) = (reg(p.x), reg(p.y));
            if reg(joint.bounding_min.x) > fp::fsubs(x, 30.0) {
                joint.bounding_min.x = mem(fp::fsubs(x, 30.0));
            }
            if reg(joint.bounding_max.x) < fp::fadds(30.0, x) {
                joint.bounding_max.x = mem(fp::fadds(30.0, x));
            }
            if reg(joint.bounding_min.y) > fp::fsubs(y, 30.0) {
                joint.bounding_min.y = mem(fp::fsubs(y, 30.0));
            }
            if reg(joint.bounding_max.y) < fp::fadds(30.0, y) {
                joint.bounding_max.y = mem(fp::fadds(30.0, y));
            }
        }
    }

    /// Updates a joint's islands, `mpLib_8005667C`.
    #[doc(alias = "mpLib_8005667C")]
    pub fn update_joint_islands(&mut self, joint_id: i32) {
        self.refresh_islands(joint_id as usize);
    }

    /// A vertex's position, `mpVtxGetPos`.
    #[doc(alias = "mpVtxGetPos")]
    pub fn vertex_pos(&self, vtx_id: i32) -> (f32, f32) {
        let p = self.vertices[vtx_id as usize].pos;
        (copy(p.x), copy(p.y))
    }

    /// Moves a vertex, `mpVtxSetPos`.
    #[doc(alias = "mpVtxSetPos")]
    pub fn set_vertex_pos(&mut self, vtx_id: i32, x: f32, y: f32) {
        let v = &mut self.vertices[vtx_id as usize];
        v.pos = Vec2 {
            x: copy(x),
            y: copy(y),
        };
    }

    /// Moves a line's two vertices, `mpLineSetPos`.
    #[doc(alias = "mpLineSetPos")]
    pub fn set_line_pos(&mut self, line_id: i32, x0: f32, y0: f32, x1: f32, y1: f32) {
        let line = self.lines[line_id as usize];
        self.set_vertex_pos(i32::from(line.v0), x0, y0);
        self.set_vertex_pos(i32::from(line.v1), x1, y1);
    }

    /// Moves a line's two vertices to offsets from their data positions, `mpLib_80056758`.
    #[doc(alias = "mpLib_80056758")]
    pub fn offset_line(&mut self, line_id: i32, x0: f32, y0: f32, x1: f32, y1: f32) {
        let line = self.lines[line_id as usize];
        for (v, x, y) in [(line.v0, x0, y0), (line.v1, x1, y1)] {
            let v = &mut self.vertices[v as usize];
            v.pos.x = mem(fp::fadds(reg(v.base.x), reg(x)));
            v.pos.y = mem(fp::fadds(reg(v.base.y), reg(y)));
        }
    }

    /// Makes a joint's vertices' positions last frame's too, so they don't remap,
    /// `mpLib_80057424`.
    #[doc(alias = "mpLib_80057424")]
    pub fn settle_vertices(&mut self, joint_id: i32) {
        let (start, count) = self.vtx_range(joint_id as usize);
        for v in &mut self.vertices[start as usize..(start + count.max(0)) as usize] {
            v.prev = Vec2 {
                x: copy(v.pos.x),
                y: copy(v.pos.y),
            };
        }
    }

    /// Enables or disables one line and updates its joint's islands.
    fn set_line_enabled(&mut self, line_id: i32, enabled: bool) {
        let joint = self.joint_from_line(line_id);
        if joint == -1 {
            return;
        }
        let j = joint as usize;
        if enabled {
            self.lines[line_id as usize].flags |= LINE_ENABLED;
        } else {
            self.lines[line_id as usize].flags &= !LINE_ENABLED;
        }
        let live = self.joints[j].flags & JOINT_NO_ISLANDS == 0;
        let (start, count) = self.vtx_range(j);
        self.update_islands(joint, start, count, live);
        self.joints[j].dirty = true;
    }

    /// Enables a line, `mpLib_80057528`.
    #[doc(alias = "mpLib_80057528")]
    pub fn enable_line(&mut self, line_id: i32) {
        self.set_line_enabled(line_id, true);
    }

    /// Disables a line, `mpLib_800575B0`.
    #[doc(alias = "mpLib_800575B0")]
    pub fn disable_line(&mut self, line_id: i32) {
        self.set_line_enabled(line_id, false);
    }

    /// Enables a joint and its lines and adds it to the end of the search list,
    /// `mpJointListAdd`.
    #[doc(alias = "mpJointListAdd")]
    pub fn enable_joint(&mut self, joint_id: i32) {
        let j = joint_id as usize;
        if self.joints[j].flags & JOINT_ENABLED != 0 {
            return;
        }
        self.joints[j].flags |= JOINT_ENABLED;
        if self.list_start.is_none() {
            self.list_start = Some(j as u16);
        } else {
            let end = self.list_end.expect("a joint list with a start has an end");
            self.joints[end as usize].next = Some(j as u16);
        }
        self.list_end = Some(j as u16);
        self.joints[j].next = None;
        self.set_lines_enabled(j, true);
        self.settle_vertices(joint_id);
        let live = self.joints[j].flags & JOINT_NO_ISLANDS == 0;
        let (start, count) = self.vtx_range(j);
        self.update_islands(joint_id, start, count, live);
        self.joints[j].dirty = true;
    }

    /// Removes a joint from the search list, `mpJointListUnlink`. The joint keeps its `next`.
    #[doc(alias = "mpJointListUnlink")]
    pub fn unlink_joint(&mut self, joint_id: i32) {
        let j = Some(joint_id as u16);
        if j == self.list_start {
            if j == self.list_end {
                self.list_end = None;
                self.list_start = None;
            } else {
                self.list_start = self.joints[self.list_start.unwrap() as usize].next;
            }
            return;
        }
        let mut cur = self.list_start;
        while let Some(c) = cur {
            let next = self.joints[c as usize].next;
            if next == j {
                if j == self.list_end {
                    self.list_end = Some(c);
                }
                self.joints[c as usize].next = self.joints[joint_id as usize].next;
                return;
            }
            cur = next;
        }
    }

    /// Disables a joint and its lines and takes it out of the search list, `mpLib_80057BC0`.
    #[doc(alias = "mpLib_80057BC0")]
    pub fn disable_joint(&mut self, joint_id: i32) {
        let j = joint_id as usize;
        if self.joints[j].flags & JOINT_ENABLED == 0 {
            return;
        }
        self.joints[j].flags &= !JOINT_ENABLED;
        self.unlink_joint(joint_id);
        self.set_lines_enabled(j, false);
        let (start, count) = self.vtx_range(j);
        self.update_islands(joint_id, start, count, false);
        self.joints[j].dirty = true;
    }

    /// Lets a joint's islands count again, `mpLib_80057FDC`.
    #[doc(alias = "mpLib_80057FDC")]
    pub fn allow_islands(&mut self, joint_id: i32) {
        self.joints[joint_id as usize].flags &= !JOINT_NO_ISLANDS;
        self.refresh_islands(joint_id as usize);
    }

    /// Stops a joint's islands counting, `mpLib_80058044`.
    #[doc(alias = "mpLib_80058044")]
    pub fn forbid_islands(&mut self, joint_id: i32) {
        self.joints[joint_id as usize].flags |= JOINT_NO_ISLANDS;
        self.refresh_islands(joint_id as usize);
    }

    /// Keeps a joint in every search whatever its bounds, `mpJointSetB10`.
    #[doc(alias = "mpJointSetB10")]
    pub fn set_always_near(&mut self, joint_id: i32) {
        self.joints[joint_id as usize].flags |= JOINT_ALWAYS_NEAR;
    }

    /// Sets a joint's ledge callback, `mpJointSetCb1`.
    #[doc(alias = "mpJointSetCb1")]
    pub fn set_ledge_callback(
        &mut self,
        joint_id: i32,
        user_data: Option<K::UserData>,
        callback: Option<K::Callback>,
    ) {
        let joint = &mut self.joints[joint_id as usize];
        joint.ledge_callback = callback;
        joint.ledge_user_data = user_data;
    }

    /// Clears it, `mpJointClearCb1`.
    #[doc(alias = "mpJointClearCb1")]
    pub fn clear_ledge_callback(&mut self, joint_id: i32) {
        self.set_ledge_callback(joint_id, None, None);
    }

    /// Reads it, `mpJointGetCb1`.
    #[doc(alias = "mpJointGetCb1")]
    pub fn ledge_callback(&self, joint_id: i32) -> (Option<K::Callback>, Option<K::UserData>) {
        let joint = &self.joints[joint_id as usize];
        (joint.ledge_callback, joint.ledge_user_data)
    }

    /// The ledge callback to run when a character grabs ledge `ledge_id`, `mpLib_8005811C`:
    /// with its user data and joint. The caller calls it with the character's collision data,
    /// its `x50` truncated to an integer (`fctiwz`), ground kind 3 and 0.
    #[doc(alias = "mpLib_8005811C")]
    pub fn grab_callback(&self, ledge_id: i32) -> Option<(K::Callback, Option<K::UserData>, i32)> {
        if ledge_id == -1 {
            return None;
        }
        let joint = self.joint_from_line(ledge_id);
        if joint == -1 {
            return None;
        }
        let j = &self.joints[joint as usize];
        j.ledge_callback.map(|cb| (cb, j.ledge_user_data, joint))
    }

    /// Sets a joint's second callback, `mpJointSetCb2`.
    #[doc(alias = "mpJointSetCb2")]
    pub fn set_callback(
        &mut self,
        joint_id: i32,
        user_data: Option<K::UserData>,
        callback: Option<K::Callback>,
    ) {
        let joint = &mut self.joints[joint_id as usize];
        joint.callback = callback;
        joint.user_data = user_data;
    }

    /// Reads it, `mpJointGetCb2`.
    #[doc(alias = "mpJointGetCb2")]
    pub fn callback(&self, joint_id: i32) -> (Option<K::Callback>, Option<K::UserData>) {
        let joint = &self.joints[joint_id as usize];
        (joint.callback, joint.user_data)
    }

    /// Links the lines of two joints where their ends meet (within 2 units in x and y),
    /// `mpLib_800581DC`: stale links that aren't returned are cut first.
    #[doc(alias = "mpLib_800581DC")]
    pub fn link_joints(&mut self, joint0: i32, joint1: i32) {
        let (j0, j1) = (joint0 as usize, joint1 as usize);
        for group in 0..5 {
            for j in [j0, j1] {
                for idx in self.group(j, group) {
                    let id = idx as i32;
                    let prev = i32::from(self.lines[idx].prev_link);
                    if prev != -1 {
                        let back = i32::from(self.lines[prev as usize].next_link);
                        if back != -1 && id != back {
                            self.lines[idx].prev_link = -1;
                        }
                    }
                    let next = i32::from(self.lines[idx].next_link);
                    if next != -1 {
                        let back = i32::from(self.lines[next as usize].prev_link);
                        if back != -1 && id != back {
                            self.lines[idx].next_link = -1;
                        }
                    }
                }
            }
        }
        let (start0, count0) = self.vtx_range(j0);
        for vid0 in start0..start0 + count0.max(0) {
            let (start1, count1) = self.vtx_range(j1);
            for vid1 in start1..start1 + count1.max(0) {
                let (a, b) = (
                    self.vertices[vid0 as usize].pos,
                    self.vertices[vid1 as usize].pos,
                );
                let dx = fp::fsubs(reg(a.x), reg(b.x));
                let dy = fp::fsubs(reg(a.y), reg(b.y));
                // `!(|d| < 2)`: a NaN never links.
                if !(fp::fabs(dx) < 2.0) || !(fp::fabs(dy) < 2.0) {
                    continue;
                }
                for g0 in 0..5 {
                    for l0 in self.group(j0, g0) {
                        if vid0 == i32::from(self.lines[l0].v0) {
                            for g1 in 0..5 {
                                for l1 in self.group(j1, g1) {
                                    if vid1 == i32::from(self.lines[l1].v1) {
                                        self.lines[l0].prev_link = l1 as i16;
                                        self.lines[l1].next_link = l0 as i16;
                                    }
                                }
                            }
                        } else if vid0 == i32::from(self.lines[l0].v1) {
                            for g1 in 0..5 {
                                for l1 in self.group(j1, g1) {
                                    if vid1 == i32::from(self.lines[l1].v0) {
                                        self.lines[l0].next_link = l1 as i16;
                                        self.lines[l1].prev_link = l0 as i16;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Links every pair of enabled, shown joints, `mpLib_80058560`.
    #[doc(alias = "mpLib_80058560")]
    pub fn link_all_joints(&mut self) {
        let n = self.joints.len() as i32;
        let shown = |f: u32| f & JOINT_ENABLED != 0 && f & JOINT_HIDDEN == 0;
        for i in 0..n - 1 {
            for j in i + 1..n {
                if shown(self.joints[i as usize].flags) && shown(self.joints[j as usize].flags) {
                    self.link_joints(i, j);
                }
            }
        }
    }

    /// Measures the bounds of the enabled floors when a joint changed since last time,
    /// `mpLib_80058614_Floor`, into `bounds[1]`.
    #[doc(alias = "mpLib_80058614_Floor")]
    pub fn measure_floors(&mut self) {
        if !self.joints.iter().any(|j| j.dirty) {
            return;
        }
        let mut b = super::Bounds {
            top: -f32::MAX,
            bottom: f32::MAX,
            left: f32::MAX,
            right: -f32::MAX,
        };
        for j in 0..self.joints.len() {
            self.joints[j].dirty = false;
            let f = self.joints[j].flags;
            if f & JOINT_ENABLED == 0 || f & JOINT_HIDDEN != 0 {
                continue;
            }
            for group in [GROUP_FLOOR, GROUP_DYNAMIC] {
                for l in self.group(j, group) {
                    let line = &self.lines[l];
                    if line.flags & FLOOR == 0 || line.flags & LINE_ENABLED == 0 {
                        continue;
                    }
                    let (p0, p1) = (
                        self.vertices[line.v0 as usize].pos,
                        self.vertices[line.v1 as usize].pos,
                    );
                    for p in [p0, p1] {
                        let (x, y) = (reg(p.x), reg(p.y));
                        if reg(b.top) < y {
                            b.top = copy(p.y);
                        }
                        if reg(b.bottom) > y {
                            b.bottom = copy(p.y);
                        }
                        if reg(b.right) < x {
                            b.right = copy(p.x);
                        }
                        if reg(b.left) > x {
                            b.left = copy(p.x);
                        }
                    }
                }
            }
        }
        self.bounds[1] = b;
    }

    /// Resets the stage's boxes to ±10000, as `mpLib_80058820` does after creating the
    /// stage's collision GObj (whose process, `mpLib_800587FC`, runs [`Self::measure_floors`]
    /// and the dynamic attribute areas' update each tick).
    pub fn reset_bounds(&mut self) {
        let b = super::Bounds {
            top: 10000.0,
            bottom: -10000.0,
            left: -10000.0,
            right: 10000.0,
        };
        self.bounds = [b, b];
    }
}
