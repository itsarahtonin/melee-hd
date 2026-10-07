//! Hitboxes' bookkeeping, from `melee/lb/lbcollision.c`: their state, and the victims each
//! has hit (two lists, of hurtboxes and of other hitboxes), which keep a box from hitting the
//! same victim twice.

use crate::game::handles::RawPtr;
use crate::game::structs::HitCapsule;

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
#[doc(alias = "lbColl_80008434")]
pub fn enable(hit: &mut HitCapsule) {
    hit.state = ENABLED;
}

/// Forgets every victim, `lbColl_80008440`.
#[doc(alias = "lbColl_80008440")]
pub fn clear_victims(hit: &mut HitCapsule) {
    for v in &mut hit.victims_1 {
        v.victim = RawPtr(0);
    }
    hit.x44 = 0;
    for v in &mut hit.victims_2 {
        v.victim = RawPtr(0);
    }
    hit.x45 = 0;
}

/// Copies another hitbox's victims (an item's boxes of one group share them),
/// `lbColl_CopyHitCapsule`.
#[doc(alias = "lbColl_CopyHitCapsule")]
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
        if v.victim != RawPtr(0) && v.x4 != 0 {
            v.x4 -= 1;
            if v.x4 == 0 {
                v.victim = RawPtr(0);
            }
        }
    }
}
