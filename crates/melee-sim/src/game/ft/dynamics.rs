//! Fighters' dynamic bones (`ftdynamics.c`): chains of joints (hair, tails, capes) that follow
//! physics instead of the animation, each link's state a [`DynamicsData`] in
//! `Game::fighter_dynamics`.
//!
//! [`DynamicsData`]: super::super::structs::DynamicsData

use melee_data::archive::DataRef;

use super::super::code::unported_call;
use super::super::handles::FighterId;
use super::super::mirror::{Cx, Mem, Mirror};
use super::super::state::Running;
use super::anim::{bone_flag, x594_bit};
use crate::hsd::jobj::{self, USE_QUATERNION};
use crate::vec::Vec3;

const KIND_KIRBY: i32 = 4;
const KIND_PURIN: i32 = 15;
const KIND_MARS: i32 = 18;
const KIND_EMBLEM: i32 = 26;

/// What a dynamic bone chain's bone id slot holds (`BoneDynamicsDesc::bone_id`): a bone id,
/// 0x100 while its physics is off, or the figatree an animation's blend slot gives it.
#[derive(Clone, Copy, Debug, PartialEq)]
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

    /// Every chain's physics on or off: Kirby's never, Jigglypuff's first only.
    fn set_dynamics(&mut self, f: FighterId, on: bool) {
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
        if matches!(fp.kind, KIND_MARS | KIND_EMBLEM) {
            unported_call("lb_80011ABC");
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
