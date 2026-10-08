//! Hitboxes' bookkeeping, from `melee/lb/lbcollision.c`: their state, and the victims each
//! has hit (two lists, of hurtboxes and of other hitboxes), which keep a box from hitting the
//! same victim twice.

use gekko_fp as fp;
use melee_data::archive::Archives;

use crate::fpu::{mem, reg};
use crate::game::handles::{FighterId, ItemId};
use crate::game::mirror::{Cx, Mem, Mirror};
use crate::game::structs::{HitCapsule, HitResult, HurtCapsule};
use crate::hsd::Objects;
use crate::hsd::jobj::{self, JObjId};
use crate::lb::collision::{HitShape, hit_hurt};
use crate::lb::joint::world_position;
use crate::mtx::Mtx34;
use crate::vec::Vec3;

/// What a hitbox remembers hitting (`HitVictim::victim`): the user data of the fighter or item
/// it hit, which the original keeps as a pointer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Victim {
    #[default]
    None,
    Fighter(FighterId),
    Item(ItemId),
    /// A pointer to something no longer live (a freed item a box still remembers), kept by its
    /// address: nothing follows it, and it is never another live object's.
    Stale(u32),
}

impl Mirror for Victim {
    const SIZE: u32 = 4;

    fn import(m: &Mem, at: u32, cx: &mut dyn Cx) -> Self {
        let a = m.u32(at);
        if a == 0 {
            Victim::None
        } else if let Some(f) = cx.import_handle("VictimFighter", a) {
            Victim::Fighter(FighterId(f))
        } else if let Some(i) = cx.import_handle("VictimItem", a) {
            Victim::Item(ItemId(i))
        } else {
            Victim::Stale(a)
        }
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        let theirs = m.u32(at);
        match *self {
            Victim::Fighter(f) => cx.compare_handle(path, "VictimFighter", Some(f.0), theirs),
            Victim::Item(i) => cx.compare_handle(path, "VictimItem", Some(i.0), theirs),
            Victim::None | Victim::Stale(_) => {
                let ours = match *self {
                    Victim::Stale(a) => a,
                    _ => 0,
                };
                if ours != theirs {
                    cx.mismatch(path, format!("{self:?}"), format!("{theirs:#x}"));
                }
            }
        }
    }
}

/// `HitCapsuleState`.
pub const DISABLED: i32 = 0;
pub const ENABLED: i32 = 1;
pub const UNK2: i32 = 2;
pub const UNK3: i32 = 3;
pub const UNK4: i32 = 4;

/// Turns a hitbox off, `lbColl_80008428`.
#[doc(alias = "lbColl_80008428")]
pub fn disable(hit: &mut HitCapsule) {
    hit.state = DISABLED;
}

/// Turns a hitbox on, `lbColl_80008434`.
pub fn enable(hit: &mut HitCapsule) {
    hit.state = ENABLED;
}

/// Forgets every victim, `lbColl_80008440`.
#[doc(alias = "lbColl_80008440")]
pub fn clear_victims(hit: &mut HitCapsule) {
    for v in &mut hit.victims_1 {
        v.victim = Victim::None;
    }
    hit.x44 = 0;
    for v in &mut hit.victims_2 {
        v.victim = Victim::None;
    }
    hit.x45 = 0;
}

/// Copies another hitbox's victims (an item's boxes of one group share them),
/// `lbColl_CopyHitCapsule`.
pub fn copy_victims(src: &HitCapsule, dst: &mut HitCapsule) {
    dst.victims_1.clone_from(&src.victims_1);
    dst.victims_2.clone_from(&src.victims_2);
    dst.x44 = src.x44;
    dst.x45 = src.x45;
}

/// Counts down how long each victim stays remembered, forgetting it at zero,
/// `lbColl_80008A5C`.
#[doc(alias = "lbColl_80008A5C")]
pub fn age_victims(hit: &mut HitCapsule) {
    if hit.state == DISABLED {
        return;
    }
    for v in hit.victims_1.iter_mut().chain(hit.victims_2.iter_mut()) {
        if v.victim != Victim::None && v.x4 != 0 {
            v.x4 -= 1;
            if v.x4 == 0 {
                v.victim = Victim::None;
            }
        }
    }
}

/// How many victims each of a hitbox's lists holds.
const VICTIMS: usize = 12;

/// Whether `lbColl_80008688` (`list2` false) or `lbColl_80008820` remembers a victim of this
/// kind for the box's rehit time (`x40_b4`) rather than for good.
fn timed(kind: i32, list2: bool) -> bool {
    matches!(kind, 2 | 5 | 7 | 8) || (!list2 && kind == 4)
}

/// Whether the hitbox has already hit `victim` (its first list), `lbColl_8000ACFC`.
#[doc(alias = "lbColl_8000ACFC")]
pub fn has_victim(hit: &HitCapsule, victim: Victim) -> bool {
    hit.victims_1.iter().any(|v| v.victim == victim)
}

/// Whether the hitbox has already hit `victim` with a phantom hit (its second list).
pub fn has_victim2(hit: &HitCapsule, victim: Victim) -> bool {
    hit.victims_2.iter().any(|v| v.victim == victim)
}

/// Remembers a victim in one of a hitbox's lists: a known victim's timer restarts for the
/// timed kinds and nothing is added; a new one takes the first free entry, or else the one at
/// the ring index, which moves on. Returns whether the victim was new.
fn add_to(hit: &mut HitCapsule, kind: i32, victim: Victim, list2: bool) -> bool {
    let rehit = u32::from(hit.x40_b4);
    let timed = timed(kind, list2);
    let (list, ring) = if list2 {
        (&mut hit.victims_2, &mut hit.x45)
    } else {
        (&mut hit.victims_1, &mut hit.x44)
    };
    if let Some(v) = list.iter_mut().find(|v| v.victim == victim) {
        if timed {
            v.x4 = rehit;
        }
        return false;
    }
    let free = list.iter().position(|v| v.victim == Victim::None);
    let at = free.unwrap_or(usize::from(*ring));
    list[at].victim = victim;
    list[at].x4 = if timed { rehit } else { 0 };
    if free.is_none() {
        *ring = ring.wrapping_add(1);
        if usize::from(*ring) >= VICTIMS {
            *ring = 0;
        }
    }
    true
}

/// Remembers a victim the hitbox hit, `lbColl_80008688`.
#[doc(alias = "lbColl_80008688")]
pub fn add_victim(hit: &mut HitCapsule, kind: i32, victim: Victim) -> bool {
    add_to(hit, kind, victim, false)
}

/// Remembers a victim of a phantom hit, `lbColl_80008820`.
#[doc(alias = "lbColl_80008820")]
pub fn add_victim2(hit: &mut HitCapsule, kind: i32, victim: Victim) -> bool {
    add_to(hit, kind, victim, true)
}

/// Forgets a victim the hitbox hit, `lbColl_800089B8`.
#[doc(alias = "lbColl_800089B8")]
pub fn remove_victim(hit: &mut HitCapsule, victim: Victim) {
    for v in &mut hit.victims_1 {
        if v.victim == victim {
            v.victim = Victim::None;
        }
    }
}

/// `HurtCapsuleState`: an intangible hurtbox can't be hit.
pub const HURT_INTANGIBLE: i32 = 2;
/// `HurtCapsule_Enabled`.
pub const HURT_ENABLED: i32 = 0;

/// The radius a hitbox tests with: its own, or scaled by its owner's size unless fixed.
fn hit_radius(hit: &HitCapsule, owner_scale: f32) -> f32 {
    if hit.x43_b1 {
        hit.scale
    } else {
        mem(fp::fmuls(reg(hit.scale), reg(owner_scale)))
    }
}

/// A bone's matrix brought up to date (`HSD_JObjGetMtxPtr`), after `mtx` when the owner has a
/// matrix of its own (a flattened fighter's).
fn bone_mtx(
    objs: &mut Objects,
    data: &Archives,
    bone: Option<JObjId>,
    mtx: Option<&Mtx34>,
) -> Mtx34 {
    let bone = bone.expect("a collision bubble without a bone");
    jobj::setup_matrix(objs, data, Some(bone));
    let m = objs.jobjs[bone].mtx;
    match mtx {
        Some(own) => own.concat(&m),
        None => m,
    }
}

/// Tests a hitbox against a hurtbox, `lbColl_8000805C`: the hurtbox's ends brought up to date
/// once a frame (pinned to `z` when its owner has a matrix of its own); then a phantom box
/// (`phantom`) always touches at the hurtbox's middle, and any other tests the capsules, the
/// hurtbox's broad phase grown by three times its owner's scale. The hitbox keeps where they
/// met and by how much.
#[doc(alias = "lbColl_8000805C")]
#[allow(clippy::too_many_arguments)]
pub fn hits_hurtbox(
    objs: &mut Objects,
    data: &Archives,
    hit: &mut HitCapsule,
    hurt: &mut HurtCapsule,
    mtx: Option<&Mtx34>,
    phantom: bool,
    hit_owner_scale: f32,
    hurt_owner_scale: f32,
    z: f32,
) -> bool {
    if hurt.state == HURT_INTANGIBLE {
        return false;
    }
    if !hurt.skip_update_pos {
        hurt.a_pos = world_position(objs, data, hurt.bone, Some(hurt.a_offset));
        hurt.b_pos = world_position(objs, data, hurt.bone, Some(hurt.b_offset));
        if mtx.is_some() {
            hurt.b_pos.z = z;
            hurt.a_pos.z = z;
        }
        hurt.skip_update_pos = true;
    }
    if phantom {
        let mid = |a: f32, b: f32| mem(fp::fmuls(0.5, fp::fadds(reg(a), reg(b))));
        hit.hurt_coll_pos = Vec3::new(
            mid(hurt.a_pos.x, hurt.b_pos.x),
            mid(hurt.a_pos.y, hurt.b_pos.y),
            mid(hurt.a_pos.z, hurt.b_pos.z),
        );
        hit.coll_distance = 5.0;
        return true;
    }
    let m = bone_mtx(objs, data, hurt.bone, mtx);
    let met = hit_hurt(
        hit.x58,
        hit.x4C,
        hurt.a_pos,
        hurt.b_pos,
        &m,
        hit_radius(hit, hit_owner_scale),
        hurt.scale,
        mem(fp::fmuls(3.0, reg(hurt_owner_scale))),
    );
    met.is_some_and(|h| {
        hit.hurt_coll_pos = h.contact;
        hit.coll_distance = h.overlap;
        h.touching
    })
}

/// Tests a grab box against a hurtbox, `lbColl_80007ECC`: [`hits_hurtbox`] on an enabled
/// hurtbox, never phantom.
#[doc(alias = "lbColl_80007ECC")]
#[allow(clippy::too_many_arguments)]
pub fn grabs_hurtbox(
    objs: &mut Objects,
    data: &Archives,
    hit: &mut HitCapsule,
    hurt: &mut HurtCapsule,
    mtx: Option<&Mtx34>,
    hit_owner_scale: f32,
    hurt_owner_scale: f32,
    z: f32,
) -> bool {
    if hurt.state != HURT_ENABLED {
        return false;
    }
    hits_hurtbox(
        objs,
        data,
        hit,
        hurt,
        mtx,
        false,
        hit_owner_scale,
        hurt_owner_scale,
        z,
    )
}

/// Tests a hitbox against a shield, reflector or absorber bubble, `lbColl_80007BCC`: as
/// [`hits_hurtbox`], with the bubble a sphere whose broad phase grows by twenty times its
/// owner's scale, and a phantom box touching at its center.
#[doc(alias = "lbColl_80007BCC")]
#[allow(clippy::too_many_arguments)]
pub fn hits_bubble(
    objs: &mut Objects,
    data: &Archives,
    hit: &mut HitCapsule,
    bubble: &mut HitResult,
    mtx: Option<&Mtx34>,
    phantom: bool,
    hit_owner_scale: f32,
    bubble_owner_scale: f32,
    z: f32,
) -> bool {
    if !bubble.skip_update_pos {
        bubble.pos = world_position(objs, data, bubble.bone, Some(bubble.offset));
        if mtx.is_some() {
            bubble.pos.z = z;
        }
        bubble.skip_update_pos = true;
    }
    if phantom {
        hit.hurt_coll_pos = bubble.pos;
        hit.coll_distance = 0.0;
        return true;
    }
    let m = bone_mtx(objs, data, bubble.bone, mtx);
    let met = hit_hurt(
        hit.x58,
        hit.x4C,
        bubble.pos,
        bubble.pos,
        &m,
        hit_radius(hit, hit_owner_scale),
        bubble.size,
        mem(fp::fmuls(20.0, reg(bubble_owner_scale))),
    );
    met.is_some_and(|h| {
        hit.hurt_coll_pos = h.contact;
        hit.coll_distance = h.overlap;
        h.touching
    })
}

/// A hitbox's shape for [`crate::lb::collision::hitboxes_touch`].
pub fn shape(hit: &HitCapsule) -> HitShape {
    HitShape {
        radius: hit.scale,
        fixed_radius: hit.x43_b1,
        pos: hit.x4C,
        prev_pos: hit.x58,
        contact: hit.hurt_coll_pos,
    }
}
