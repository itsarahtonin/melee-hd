//! Fighters' dynamic bones (`ftdynamics.c`): chains of joints (hair, tails, capes) that follow
//! physics instead of the animation, each link's state a [`DynamicsData`] in
//! `Game::fighter_dynamics`.
//!
//! [`DynamicsData`]: super::super::structs::DynamicsData

// NaN takes the original's branches, which the negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::super::handles::{DynamicsId, FighterId};
use super::super::mirror::{Cx, Mem, Mirror};
use super::super::state::{Game, Running};
use super::super::structs::DynamicsDesc;
use super::anim::{bone_flag, x594_bit};
use super::common::GA_AIR;
use crate::fpu::{copy, mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId, USE_QUATERNION};
use crate::lb::dynamics::Collider;
use crate::lb::joint::world_position;
use crate::quat::Quat;
use crate::vec::Vec3;

const KIND_KIRBY: i32 = 4;
const KIND_PURIN: i32 = 15;
const KIND_MARS: i32 = 18;
const KIND_EMBLEM: i32 = 26;

/// What a dynamic bone chain's bone id slot holds (`BoneDynamicsDesc::bone_id`): a bone id,
/// 0x100 while its physics is off, or the figatree an animation's blend slot gives it.
#[derive(Clone, Copy, Debug, PartialEq, melee_derive::StateEq)]
pub enum BoneSlot {
    Id(i32),
    Tree(DataRef),
}

impl Default for BoneSlot {
    fn default() -> Self {
        BoneSlot::Id(0)
    }
}

impl Mirror for BoneSlot {
    const SIZE: u32 = 4;

    fn import(m: &Mem, at: u32, cx: &mut dyn Cx) -> Self {
        let w = m.u32(at);
        if w >= 0x8000_0000
            && let Some(d) = cx.import_data(w)
        {
            BoneSlot::Tree(d)
        } else {
            BoneSlot::Id(w as i32)
        }
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        let theirs = m.u32(at);
        match *self {
            BoneSlot::Tree(d) => cx.compare_data(path, Some(d), theirs),
            BoneSlot::Id(v) if v as u32 != theirs => {
                cx.mismatch(path, format!("{v:#x}"), format!("{theirs:#x}"))
            }
            BoneSlot::Id(_) => {}
        }
    }
}

impl Running<'_> {
    /// A part's dynamics toggled, `ftCo_8009E318`: the chain link on the part's joint found,
    /// its bone flag flipped and the chain's slot set past it (physics off from the link on,
    /// the action's animation put back on the part from `frame`) or at it (physics on, the
    /// part's and its animation skeleton twin's own animations removed); -1 with no such link.
    #[doc(alias = "ftCo_8009E318")]
    pub fn toggle_part_dynamics(&mut self, gobj: GObjId, part: i32, frame: f32) -> i32 {
        let f = self.game.fighter_id(gobj);
        let part_j = self.bone(f, part).joint;
        let parts = self.game.fighters[f.0]
            .parts
            .expect("a fighter without parts");
        for i in 0..self.game.fighters[f.0].dynamics_num as usize {
            let mut link = self.game.fighters[f.0].dynamic_bone_sets[i].dyn_desc.data;
            let mut j = 0;
            while let Some(d) = link {
                if self.game.fighter_dynamics[d.0].desc.jobj == part_j {
                    let b = &mut self.game.fighter_parts[parts.0][part as usize];
                    if b.x8[0] & 0x80 != 0 {
                        b.x8[0] &= !0x80;
                        self.game.fighters[f.0].dynamic_bone_sets[i].bone_id = BoneSlot::Id(j + 1);
                        if self.game.fighters[f.0].x590.is_some() {
                            let speed = self.game.fighters[f.0].frame_speed_mul;
                            self.restart_part_anim(f, part, frame, speed);
                        }
                    } else {
                        b.x8[0] |= 0x80;
                        self.game.fighters[f.0].dynamic_bone_sets[i].bone_id = BoneSlot::Id(j);
                        let root = super::super::gr::ground::jobj_of(self.game, gobj);
                        let skel = self.game.fighters[f.0].x8AC_animSkeleton;
                        let twin = twin_joint(&self.game.objs.jobjs, root, skel, part_j);
                        self.remove_anim(part_j);
                        self.remove_anim(twin);
                    }
                    return 0;
                }
                link = self.game.fighter_dynamics[d.0].next;
                j += 1;
            }
        }
        -1
    }
}

/// The joint of `b`'s tree in the place `target` holds in `a`'s, `ftCo_8009E1D4`.
#[doc(alias = "ftCo_8009E1D4")]
fn twin_joint(
    jobjs: &crate::hsd::jobj::JObjs,
    a: Option<JObjId>,
    b: Option<JObjId>,
    target: Option<JObjId>,
) -> Option<JObjId> {
    let (a, b) = (a?, b?);
    if Some(a) == target {
        return Some(b);
    }
    if jobjs[a].child.is_some()
        && let Some(t) = twin_joint(jobjs, jobjs[a].child, jobjs[b].child, target)
    {
        return Some(t);
    }
    if jobjs[a].next.is_some()
        && let Some(t) = twin_joint(jobjs, jobjs[a].next, jobjs[b].next, target)
    {
        return Some(t);
    }
    None
}

impl Running<'_> {
    /// The character's dynamics description (`ftData::x2C`).
    fn dynamics_desc(&self, f: FighterId) -> DataRef {
        let fp = &self.game.fighters[f.0];
        self.data
            .ptr(fp.ft_data.expect("a fighter without data").at(0x2C))
            .expect("ftData's dynamics")
    }

    /// Turns a dynamic bone chain's physics on or off (`on`: the joints follow physics, not
    /// the animation), `ftCo_8009CB40`. `arg` is what the animation's blend slot gives the
    /// chain (`Id(0)` for nothing): a count of leading links that do the opposite, or (as the
    /// original reads a pointer as that count, a negative number) a figatree, which none
    /// do. Turning a link's physics on puts its joint back at the link's rest pose.
    #[doc(alias = "ftCo_8009CB40")]
    pub fn set_bone_dynamics(&mut self, f: FighterId, set: usize, on: bool, arg: BoneSlot) {
        let dynamics = self.dynamics_desc(f);
        if self.data.u32(dynamics) == 0 {
            return;
        }
        let bones = self.data.ptr(dynamics.at(4)).expect("dynamic bones");
        let mut bone = self.data.u32(bones.at(0x18 * set as u32)) as i32;
        let mut j = self.bone(f, bone).joint;
        let fp = &mut self.game.fighters[f.0];
        fp.dynamic_bone_sets[set].bone_id = if on { arg } else { BoneSlot::Id(0x100) };
        let limit = match arg {
            BoneSlot::Id(n) => n,
            BoneSlot::Tree(_) => -1,
        };
        let parts = fp.parts.expect("a fighter without parts");
        let mut link = fp.dynamic_bone_sets[set].dyn_desc.data;
        let mut k = 0;
        while let Some(d) = link {
            let b = &self.game.fighter_parts[parts.0][bone as usize];
            let on = if k < limit { !on } else { on };
            if on && !bone_flag(b, 0) {
                // `ftCo_8009CB40_inline`.
                let desc = &mut self.game.fighter_dynamics[d.0].desc;
                let dj = desc.jobj.expect("a dynamic bone without a joint");
                let m = &self.game.objs.jobjs[dj].mtx.m;
                desc.unk_2C = Vec3 {
                    x: m[0][3],
                    y: m[1][3],
                    z: m[2][3],
                };
                desc.unk_44 = 0.0;
                let (t, s) = (desc.translate, desc.scale);
                jobj::set_translate(&mut self.game.objs.jobjs, dj, t);
                jobj::set_scale(&mut self.game.objs.jobjs, dj, s);
            }
            let b = &mut self.game.fighter_parts[parts.0][bone as usize];
            if on {
                b.x8[0] |= 0x80;
                jobj::clear_flags(&mut self.game.objs.jobjs, j, USE_QUATERNION);
            } else {
                b.x8[0] &= !0x80;
            }
            j = j.and_then(|j| self.game.objs.jobjs[j].child);
            link = self.game.fighter_dynamics[d.0].next;
            bone += 1;
            k += 1;
        }
    }

    /// Every chain's physics on or off: Kirby's never, Jigglypuff's first only,
    /// `ftCo_8009E140`.
    #[doc(alias = "ftCo_8009E140")]
    pub fn set_dynamics(&mut self, f: FighterId, on: bool) {
        match self.game.fighters[f.0].kind {
            KIND_KIRBY => {}
            KIND_PURIN => self.set_bone_dynamics(f, 0, on, BoneSlot::Id(0)),
            _ => {
                for i in 0..self.game.fighters[f.0].dynamics_num {
                    self.set_bone_dynamics(f, i as usize, on, BoneSlot::Id(0));
                }
            }
        }
    }

    /// Sets the fighter's bone physics for the action's animation, `ftCo_8009E7B4`: off while
    /// the fighter is held (`x2227_b6`) or the animation moves those bones itself
    /// (`x594_b3`); with `x594_b4`, on with what the animation's blend slot (`pair`'s second
    /// byte) gives each chain (`ftDynamics::x10`); else on.
    #[doc(alias = "ftCo_8009E7B4")]
    pub fn dynamics_for_anim(&mut self, f: FighterId, pair: DataRef) {
        let fp = &self.game.fighters[f.0];
        if fp.anim_id == -1 || fp.x2227_b6 {
            if fp.x2227_b6 {
                self.set_dynamics(f, false);
            }
            return;
        }
        // Marth's and Roy's capes follow the wind while a box's blows.
        if matches!(fp.kind, KIND_MARS | KIND_EMBLEM) && self.game.wind_level() > 0 {
            self.set_dynamics(f, true);
            return;
        }
        if !x594_bit(fp, 4) {
            let on = !x594_bit(fp, 3);
            self.set_dynamics(f, on);
            return;
        }
        let n = fp.dynamics_num;
        let slot = u32::from(self.data.u8(pair.at(1)));
        let slots = self
            .pointer(self.dynamics_desc(f).at(0x10))
            .and_then(|t| self.pointer(t.at(4 * slot)));
        for i in 0..n {
            match slots {
                Some(t) => {
                    let at = t.at(4 * i as u32);
                    let arg = match self.data.ptr(at) {
                        Some(tree) => BoneSlot::Tree(tree),
                        None => BoneSlot::Id(self.data.u32(at) as i32),
                    };
                    self.set_bone_dynamics(f, i as usize, true, arg);
                }
                None => self.set_bone_dynamics(f, i as usize, false, BoneSlot::Id(0)),
            }
        }
    }

    /// The pointer at `at`, which the original follows: a word that isn't one must be null.
    fn pointer(&self, at: DataRef) -> Option<DataRef> {
        let p = self.data.ptr(at);
        assert!(
            p.is_some() || self.data.u32(at) == 0,
            "a pointer the data doesn't relocate at {at:?}"
        );
        p
    }
}

const KIND_KOOPA: i32 = 5;
const KIND_PEACH: i32 = 9;
const KIND_MEWTWO: i32 = 16;
const KIND_GKOOPS: i32 = 31;

/// `Gr_Kind_Flatzone`, where fighters are flat.
const GR_FLATZONE: i32 = 0x1B;

impl Running<'_> {
    /// Wind blown from a point `(x, y)` off a bone, `ftCo_8009E714`: `frames` long (120 when
    /// negative), with its strength, decay and angle step (`lb_800119DC`).
    #[doc(alias = "ftCo_8009E714")]
    #[allow(clippy::too_many_arguments)]
    pub fn bone_wind(
        &mut self,
        gobj: GObjId,
        bone: usize,
        frames: i32,
        x: f32,
        y: f32,
        scale: f32,
        decay: f32,
        angle: f32,
    ) {
        let parts = self
            .game
            .fighter(gobj)
            .parts
            .expect("a fighter without parts");
        let joint = self.game.fighter_parts[parts.0][bone].joint;
        let at = crate::vec::Vec3::new(x, y, 0.0);
        let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, joint, Some(at));
        let frames = if frames < 0 { 120 } else { frames };
        crate::lb::wind::blast(&mut self.game.gr.winds, pos, frames, scale, decay, angle);
    }
    /// Swings the fighter's dynamic bone chains a frame, `ftCo_8009E0A8` (`ftCo_8009DD94`
    /// with `x2228_b1`, whose chains take a flat floor at the fighter's height instead of the
    /// stage's): the colliders follow their joints, Marth's and Roy's capes follow the wind,
    /// and each chain swings, against the floor while the fighter stands at its normal size
    /// (Bowser's and Giga Bowser's never). On Flat Zone the model is flattened (its x scale
    /// the fighter's) only while the chains swing.
    #[doc(alias = "ftCo_8009E0A8")]
    #[doc(alias = "ftCo_8009DD94")]
    pub fn swing_chains(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let n = fp.dynamics_num;
        let flat_floor = fp.x2228_b1;
        let flatzone = n != 0 && self.game.stage_info.grkind == GR_FLATZONE;
        let root = self.root_jobj(gobj);
        let mut scale_x = 0.0;
        if flatzone {
            let sy = self.game.fighters[f.0].x34_scale.y;
            let o = &mut self.game.objs.jobjs[root];
            scale_x = o.scale.x;
            o.scale.x = sy;
            if !o.mtx_is_dirty() {
                jobj::set_mtx_dirty_sub(&mut self.game.objs.jobjs, root);
            }
        }
        let fp = &self.game.fighters[f.0];
        // Only at exactly normal size (a NaN size takes the original's other branch).
        let ground = if reg(fp.x34_scale.y) == 1.0 {
            if fp.kind == KIND_MEWTWO && (0xDF..=0xE8).contains(&fp.motion_id) {
                true
            } else {
                !(fp.ground_or_air == GA_AIR || fp.kind == KIND_KOOPA || fp.kind == KIND_GKOOPS)
            }
        } else {
            false
        };
        self.place_colliders(gobj);
        self.capes_in_wind(f);
        let fp = &self.game.fighters[f.0];
        let count = usize::from(fp.x166C);
        assert!(
            count <= fp.x1670.len(),
            "more dynamics colliders than a fighter holds"
        );
        // On the stack: a fighter holds at most `x1670.len()` colliders.
        let mut held = [Collider {
            radius: 0.0,
            center: crate::vec::Vec3::ZERO,
        }; 1];
        for (dst, c) in held.iter_mut().zip(&fp.x1670[..count]) {
            *dst = Collider {
                radius: c.v2,
                center: c.x18,
            };
        }
        let colliders = &held[..count];
        let flat = flat_floor.then_some(fp.cur_pos.y);
        let kind = fp.kind;
        let last = (n - 1).max(0);
        // Each set and the wind its first link takes: Peach's all but the last 3, Mewtwo's 8.
        let sets = if kind == KIND_PEACH { last + 1 } else { n };
        for i in 0..sets.max(0) {
            let first_wind = match kind {
                KIND_PEACH if i < last => 3,
                KIND_PEACH => 0,
                KIND_MEWTWO => 8,
                _ => 0,
            };
            let set = &self.game.fighters[f.0].dynamic_bone_sets[i as usize];
            let (desc, bone) = (set.dyn_desc.clone(), set.bone_id);
            self.game
                .swing_chain(self.data, &desc, colliders, flat, bone, first_wind, ground);
        }
        if flatzone {
            let o = &mut self.game.objs.jobjs[root];
            o.scale.x = scale_x;
            if !o.mtx_is_dirty() {
                jobj::set_mtx_dirty_sub(&mut self.game.objs.jobjs, root);
            }
            for i in 0..n as usize {
                let set = &self.game.fighters[f.0].dynamic_bone_sets[i];
                let d = set.dyn_desc.data.expect("a chain of no links");
                let mut j = self.game.fighter_dynamics[d.0]
                    .desc
                    .jobj
                    .expect("a link without a joint");
                while let Some(c) = self.game.objs.jobjs[j].child {
                    j = c;
                }
                jobj::setup_matrix(&mut self.game.objs, self.data, Some(j));
            }
        }
        if self.game.playback.is_some() {
            // Playback's codes set up every dirty matrix of the model here.
            self.setup_dirty_matrices(Some(root));
        }
    }

    /// Slippi's playback code at `ftCo_8009DD94+0x2FC`: every joint of the model whose matrix
    /// is dirty and not the game's own sets it up, children before siblings.
    fn setup_dirty_matrices(&mut self, j: Option<JObjId>) {
        let Some(j) = j else { return };
        if self.game.objs.jobjs[j].mtx_is_dirty() {
            jobj::setup_matrix_sub(&mut self.game.objs, self.data, j);
        }
        let (child, next) = (self.game.objs.jobjs[j].child, self.game.objs.jobjs[j].next);
        self.setup_dirty_matrices(child);
        self.setup_dirty_matrices(next);
    }

    /// The colliders the chains swing around follow their joints, `ftColl_8007AF60`.
    #[doc(alias = "ftColl_8007AF60")]
    fn place_colliders(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        for i in 0..usize::from(self.game.fighters[f.0].x166C) {
            let c = &self.game.fighters[f.0].x1670[i];
            let (j, off) = (c.jobj, c.v1);
            let at = world_position(&mut self.game.objs, self.data, j, Some(off));
            self.game.fighters[f.0].x1670[i].x18 = at;
        }
    }

    /// Marth's and Roy's capes in the wind, `ftCo_8009E614`: physics back on for every chain
    /// when a box's wind starts, under an animation that moves them itself.
    #[doc(alias = "ftCo_8009E614")]
    fn capes_in_wind(&mut self, f: FighterId) {
        let fp = &self.game.fighters[f.0];
        if !matches!(fp.kind, KIND_MARS | KIND_EMBLEM) || fp.x2227_b6 {
            return;
        }
        match self.game.wind_level() {
            -1 => self.capes_out_of_wind(f),
            1 => {
                let fp = &self.game.fighters[f.0];
                if x594_bit(fp, 4) || x594_bit(fp, 3) {
                    let (frame, speed) = (fp.cur_anim_frame, fp.frame_speed_mul);
                    let n = self.data.u32(self.dynamics_desc(f));
                    for i in 0..n {
                        self.set_bone_dynamics(f, i as usize, true, BoneSlot::Id(0));
                    }
                    if self.game.fighters[f.0].x590.is_some() {
                        let gobj = self.game.fighters[f.0].gobj.expect("a fighter's GObj");
                        self.start_anim(gobj, reg(frame), reg(speed), 0.0);
                        self.anim_joints(gobj);
                    }
                }
            }
            _ => {}
        }
    }
}

impl Running<'_> {
    /// Marth's and Roy's capes as a box's wind ends, `ftCo_8009E4A8`: each chain back on the
    /// trees its animation's slot gives (`x594` bit 4) or off (bit 3), and with an
    /// animation of their own (`x590`) the chains' bones restarted where it plays.
    #[doc(alias = "ftCo_8009E4A8")]
    pub fn capes_out_of_wind(&mut self, f: FighterId) {
        let fp = &self.game.fighters[f.0];
        let (frame, speed, n) = (fp.cur_anim_frame, fp.frame_speed_mul, fp.dynamics_num);
        let desc = self.dynamics_desc(f);
        let bones = |r: &Self, i: i32| {
            let array = r.data.ptr(desc.at(4)).expect("dynamic bones");
            r.data.u32(array.at(0x18 * i as u32)) as i32
        };
        let restart = |r: &mut Self, i: i32| {
            if r.game.fighters[f.0].x590.is_some() {
                let bone = bones(r, i);
                r.restart_part_anim(f, bone, frame, speed);
            }
        };
        if x594_bit(fp, 4) {
            let anim = u32::try_from(fp.anim_id).expect("an animation's trees without one");
            let pair = fp
                .x28
                .expect("a fighter without animation pairs")
                .at(2 * anim);
            let slot = u32::from(self.data.u8(pair.at(1)));
            let Some(trees) = self
                .pointer(desc.at(0x10))
                .and_then(|t| self.pointer(t.at(4 * slot)))
            else {
                return;
            };
            for i in 0..n {
                let at = trees.at(4 * i as u32);
                let arg = match self.data.ptr(at) {
                    Some(tree) => BoneSlot::Tree(tree),
                    None => BoneSlot::Id(self.data.u32(at) as i32),
                };
                self.set_bone_dynamics(f, i as usize, true, arg);
                restart(self, i);
            }
        } else if x594_bit(fp, 3) {
            for i in 0..n {
                self.set_bone_dynamics(f, i as usize, false, BoneSlot::Id(0));
                restart(self, i);
            }
        }
    }
}

impl Game {
    /// Takes the first free dynamics link (`cur_data`), or none when they're all in use.
    fn pop_dynamics_link(&mut self) -> Option<DynamicsId> {
        let link = self.ft.dynamics_free?;
        self.ft.dynamics_free = self.fighter_dynamics[link.0].next;
        self.fighter_dynamics[link.0].next = None;
        Some(link)
    }

    /// Builds a chain of up to `n` links down `root` and its first children, taking each from
    /// the free links, and sizes each link from the next one's position, `lb_8000FD48`. Links
    /// run out silently: the chain ends early and isn't sized.
    #[doc(alias = "lb_8000FD48")]
    pub fn load_dynamics_chain(
        &mut self,
        data: &Archives,
        root: Option<JObjId>,
        desc: &mut DynamicsDesc,
        n: u32,
    ) {
        let Some(mut j) = root else {
            desc.data = None;
            return;
        };
        desc.count = 0;
        let mut cur: Option<DynamicsId> = None;
        while (desc.count as i32) < n as i32 {
            let link = self.pop_dynamics_link();
            match cur {
                None => desc.data = link,
                Some(c) => self.fighter_dynamics[c.0].next = link,
            }
            cur = link;
            let Some(l) = link else { return };
            if !self.objs.jobjs[j].mtx_is_dirty() {
                jobj::set_mtx_dirty_sub(&mut self.objs.jobjs, j);
            }
            if self.objs.jobjs[j].mtx_is_dirty() {
                jobj::setup_matrix_sub(&mut self.objs, data, j);
            }
            let o = &self.objs.jobjs[j];
            let m = &o.mtx.m;
            let d = &mut self.fighter_dynamics[l.0].desc;
            d.jobj = Some(j);
            if o.stale_w {
                // The link keeps the joint's rotation, w and all.
                self.objs.stale_reads += 1;
            }
            let o = &self.objs.jobjs[j];
            let d = &mut self.fighter_dynamics[l.0].desc;
            d.rotate = o.rotate;
            d.scale = o.scale;
            d.translate = o.translate;
            d.unk_2C = Vec3::new(copy(m[0][3]), copy(m[1][3]), copy(m[2][3]));
            d.unk_58 = d.rotate;
            d.unk_38.x = 1.0;
            d.unk_38.y = 0.0;
            d.unk_38.z = 0.0;
            d.unk_44 = 0.0;
            d.unk_8C = 0.0;
            d.unk_48 = 0.0;
            self.fighter_dynamics[l.0].next = None;
            self.fighter_dynamics[l.0].unk_94 = 0;
            desc.count += 1;
            match self.objs.jobjs[j].child {
                Some(c) => j = c,
                None if (desc.count as i32) < n as i32 => {
                    panic!("lb_8000FD48: a chain longer than its joints faults on a null joint")
                }
                None => {}
            }
        }
        let mut cur = desc.data.expect("lb_8000FD48: a chain of no links");
        while let Some(next) = self.fighter_dynamics[cur.0].next {
            let (a, b) = (
                self.fighter_dynamics[cur.0].desc.unk_2C,
                self.fighter_dynamics[next.0].desc.unk_2C,
            );
            let dy = fp::fsubs(reg(a.y), reg(b.y));
            let dx = fp::fsubs(reg(a.x), reg(b.x));
            let dz = fp::fsubs(reg(a.z), reg(b.z));
            let sq = fp::fmadds(dz, dz, fp::fmadds(dx, dx, fp::fmuls(dy, dy)));
            // The inline `sqrtf`, taken for anything not at most zero (a NaN too).
            let len = if !(sq <= 0.0) { newton_sqrt(sq) } else { sq };
            self.fighter_dynamics[cur.0].desc.unk_48 = mem(len);
            let t = self.fighter_dynamics[next.0].desc.translate;
            // `fabsf` as the branches compile it: anything not at least zero is negated.
            let abs = |v: f32| {
                let r = reg(v);
                if r >= 0.0 { r } else { fp::fneg(r) }
            };
            let (x, y, z) = (abs(t.x), abs(t.y), abs(t.z));
            self.fighter_dynamics[cur.0].desc.unk_54 = if !(z <= y) {
                if !(z <= x) { -1 } else { 0 }
            } else if !(y <= x) {
                1
            } else {
                0
            };
            cur = next;
        }
    }

    /// Gives a chain's links their physics parameters from the character's data, and the
    /// chain its position, `lb_80011710`.
    #[doc(alias = "lb_80011710")]
    pub fn set_dynamics_params(&mut self, data: &Archives, src: DataRef, desc: &mut DynamicsDesc) {
        let pos = crate::game::mirror::read_vec3(data, src.at(8));
        desc.pos = Vec3::new(copy(pos.x), copy(pos.y), copy(pos.z));
        let inner = data.ptr(src).expect("a dynamics description without links");
        let raw = |at: DataRef| f32::from_bits(data.u32(at));
        let raw3 = |at: DataRef| Vec3::new(raw(at), raw(at.at(4)), raw(at.at(8)));
        let mut link = desc.data;
        for i in 0..data.u32(src.at(4)) as i32 {
            let l = link.expect("lb_80011710: more parameters than links faults");
            let e = inner.at(0x3C * i as u32);
            let d = &mut self.fighter_dynamics[l.0].desc;
            d.unk_4C = copy(data.f32(e));
            d.unk_50 = copy(data.f32(e.at(4)));
            d.unk_58 = Quat {
                x: raw(e.at(8)),
                y: raw(e.at(0xC)),
                z: raw(e.at(0x10)),
                w: raw(e.at(0x14)),
            };
            d.unk_68 = copy(data.f32(e.at(0x18)));
            d.unk_6C = raw3(e.at(0x1C));
            d.unk_78 = raw3(e.at(0x28));
            d.unk_84 = copy(data.f32(e.at(0x34)));
            d.unk_88 = copy(data.f32(e.at(0x38)));
            d.unk_8C = if reg(d.unk_48) != 0.0 {
                mem(fp::fdivs(reg(pos.z), reg(d.unk_48)))
            } else {
                0.0
            };
            link = self.fighter_dynamics[l.0].next;
        }
    }
}

/// The inline `sqrtf`'s three Newton steps on `frsqrte`, for a value already known positive.
fn newton_sqrt(x: f64) -> f64 {
    let mut g = fp::frsqrte(x);
    for _ in 0..3 {
        let gg = fp::fmul(g, g);
        let half = fp::fmul(0.5, g);
        g = fp::fmul(half, fp::fnmsub(x, gg, 3.0));
    }
    fp::frsp(fp::fmul(x, g))
}
