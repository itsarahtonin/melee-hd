//! Pikachu's and Pichu's Thunder bolt, `melee/it/kinds/itpikachuthunder.c`: a chain of links
//! that drop from the cloud one after another; the first stops at the ground (or at
//! Pikachu, who stops it), and each link after it stretches to reach where the one before it
//! stopped.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::item::spawn_air;
use super::super::item::{article, change, change_state, debug_owner_bubbles, next_attack_id};
use super::super::{ground, hitbox, item, item_mut, jobj_of, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

pub const ANIM0: u32 = 0x802B_20DC;
pub const ANIM1: u32 = 0x802B_21A8;
pub const COLL1: u32 = 0x802B_21E0;
pub const ANIM2: u32 = 0x802B_2340;
pub const DESTROYED: u32 = 0x802B_202C;
#[doc(alias = "itPikachuThunder_Logic39_DmgDealt")]
pub const DMG_DEALT: u32 = 0x802B_2508;
#[doc(alias = "itPikachuThunder_Logic39_HitShield")]
pub const HIT_SHIELD: u32 = 0x802B_2510;
pub const CLANKED: u32 = 0x802B_2518;
#[doc(alias = "itPikachuThunder_Logic39_Absorbed")]
pub const ABSORBED: u32 = 0x802B_2520;
pub const EVT_UNK: u32 = 0x802B_2528;

/// `itPikachuthunderAttributes`: the lifetime, a link's length, and how far below a link
/// its tip is.
mod attr {
    pub const LIFETIME: u32 = 0x0;
    pub const LENGTH: u32 = 0x4;
    pub const TIP: u32 = 0x8;
}

/// Runs one of the bolt's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 => anim0(r, gobj),
        ANIM1 => anim1(r, gobj),
        COLL1 => coll1(r, gobj),
        ANIM2 => anim2(r, gobj),
        DMG_DEALT | HIT_SHIELD | CLANKED | ABSORBED => false,
        _ => return None,
    })
}

/// Runs one of the bolt's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => destroyed(r, gobj),
        _ => return false,
    }
    true
}

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let attrs = article(item(r.game, gobj))
        .special(r.data)
        .expect("a bolt without attributes");
    r.data.f32(attrs.at(off))
}

/// Whether the bolt has stopped (`it_802B1DEC`, `x4`).
#[doc(alias = "it_802B1DEC")]
pub fn has_hit(r: &mut Running, gobj: GObjId) -> bool {
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .pikachu_thunder()
        .x4
        .get()
        != 0
}

/// Drops a bolt of `count` links from `pos` for `owner`, `it_802B1DF8`: link `i` waits
/// `i * delay` frames and falls at `vel`; all share one attack. The first link comes back.
#[doc(alias = "it_802B1DF8")]
pub fn spawn(
    r: &mut Running,
    owner: GObjId,
    pos: Vec3,
    vel: Vec3,
    count: i32,
    delay: i32,
    kind: i32,
) -> Option<GObjId> {
    let attack = next_attack_id(r.game);
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    // `Item_InitSpawnPosition`, on the plane.
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = spawn.prev_pos;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.facing_dir = -1.0;
    spawn.x3C_damage = 0;
    spawn.x0_parent_gobj = Some(owner);
    spawn.x4_parent_gobj2 = Some(owner);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = attack as i32;
    let mut first = None;
    let mut prev: Option<GObjId> = None;
    let mut wait = 0;
    for i in 0..count {
        let gobj = spawn_air(r, &mut spawn);
        if i == 0 {
            first = gobj;
        }
        if let Some(gobj) = gobj {
            let length = reg(attr_f32(r, gobj, attr::LENGTH));
            if let Some(prev) = prev {
                item_mut(r.game, prev)
                    .xDD4_itemVar
                    .pikachu_thunder()
                    .x34
                    .set(Some(gobj));
            }
            hitbox::start_blink(r.game, gobj);
            item_mut(r.game, gobj).xDC8_word.flags.xD = false;
            debug_owner_bubbles(r.game, gobj, Some(owner));
            start(r, gobj);
            let ip = item_mut(r.game, gobj);
            ip.xDCC_flag.b3 = false;
            ip.xDCC_flag.b4567 = 14;
            let v = ip.xDD4_itemVar.pikachu_thunder();
            v.x0.set(i);
            v.x38.set(Some(owner));
            v.x8.set(wait);
            let frames = fp::fdivs(length, reg(vel.y));
            v.x14.set(mem(if frames < 0.0 {
                fp::fneg(frames)
            } else {
                frames
            }));
            v.x1C.set(vel);
            v.x34.set(None);
            v.x4.set(0);
            v.xC.set(mem(length));
            v.x18.set(1.0);
        }
        prev = gobj;
        wait = wait.wrapping_add(delay);
    }
    first
}

/// Stops the bolt (`it_802B1FC8`).
#[doc(alias = "it_802B1FC8")]
pub fn hit(r: &mut Running, gobj: GObjId) {
    stop(r, gobj);
}

/// Where the bolt's tip is, `it_802B1FE8`: `attr::TIP` below it, scaled with its length.
#[doc(alias = "it_802B1FE8")]
pub fn tip(r: &mut Running, gobj: GObjId) -> Vec3 {
    let below = attr_f32(r, gobj, attr::TIP);
    let ip = item_mut(r.game, gobj);
    let scale = reg(ip.xDD4_itemVar.pikachu_thunder().x18.get());
    let offset = fp::fmuls(fp::fneg(reg(below)), scale);
    let mut p = item(r.game, gobj).pos;
    p.y = mem(fp::fadds(reg(p.y), offset));
    p
}

/// `it_2725_Logic39_Destroyed`: the first link gone, Pikachu's Thunder knows (unless he's
/// in a state that ignores it).
#[doc(alias = "it_2725_Logic39_Destroyed")]
fn destroyed(r: &mut Running, gobj: GObjId) {
    let v = item_mut(r.game, gobj).xDD4_itemVar.pikachu_thunder();
    let (i, owner) = (v.x0.get(), v.x38.get());
    if i == 0
        && let Some(owner) = owner
        && !r.pk_special_lw_check_property(owner)
    {
        r.pk_special_lw_set_hit(owner);
    }
}

/// A link's start, `it_802B2080`: waiting, hidden.
#[doc(alias = "it_802B2080")]
fn start(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    let lifetime = attr_f32(r, gobj, attr::LIFETIME);
    lib2725::set_lifetime(r.game, r.data, gobj, lifetime);
    lib26b1::hide(r.game, gobj);
}

/// `itPikachuthunder_UnkMotion0_Anim`: the link waits its frames, then falls.
#[doc(alias = "itPikachuthunder_UnkMotion0_Anim")]
fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    let v = item_mut(r.game, gobj).xDD4_itemVar.pikachu_thunder();
    let wait = v.x8.get();
    if wait <= 0 {
        fall(r, gobj);
    } else {
        v.x8.set(wait.wrapping_sub(1));
    }
    false
}

/// The link falls, shown, `it_802B211C`.
#[doc(alias = "it_802B211C")]
fn fall(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    let lifetime = attr_f32(r, gobj, attr::LIFETIME);
    lib2725::set_lifetime(r.game, r.data, gobj, lifetime);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel = ip.xDD4_itemVar.pikachu_thunder().x1C.get();
    ip.xDCC_flag.b3 = true;
    lib26b1::show(r.game, gobj);
}

/// One frame of the item's lifetime, ending it at 0.
fn tick(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    reg(ip.xD44_lifeTimer) <= 0.0
}

/// `itPikachuthunder_UnkMotion1_Anim`.
#[doc(alias = "itPikachuthunder_UnkMotion1_Anim")]
fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    tick(r, gobj)
}

/// `itPikachuthunder_UnkMotion1_Coll`: the first link stops at the ground, with its dust,
/// and Pikachu's quake (unless he's in a state that ignores it); each later link stops
/// where the one before it did.
#[doc(alias = "itPikachuthunder_UnkMotion1_Coll")]
fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    let v = item_mut(r.game, gobj).xDD4_itemVar.pikachu_thunder();
    if v.x0.get() == 0 {
        if ground::air_probe(r, gobj) {
            let below = attr_f32(r, gobj, attr::TIP);
            let pos = Vec3::new(0.0, mem(fp::fneg(reg(below))), 0.0);
            lib2725::landing_dust_at(r, gobj, pos);
            stop(r, gobj);
            let owner = item_mut(r.game, gobj)
                .xDD4_itemVar
                .pikachu_thunder()
                .x38
                .get();
            if let Some(owner) = owner
                && !r.pk_special_lw_check_property(owner)
            {
                r.pk_special_lw_bolt_landed(owner);
            }
        }
    } else {
        let ip = item_mut(r.game, gobj);
        let y = ip.pos.y;
        let v = ip.xDD4_itemVar.pikachu_thunder();
        if v.x4.get() != 0 && reg(y) <= reg(v.x28.get().y) {
            stop(r, gobj);
        }
    }
    false
}

/// The link stops, `it_802B22B8`: stretching for its frames, and the next link learns where.
#[doc(alias = "it_802B22B8")]
fn stop(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, 0x1 | change::HIT_PRESERVE);
    let ip = item_mut(r.game, gobj);
    let pos = ip.pos;
    let v = ip.xDD4_itemVar.pikachu_thunder();
    let frames = v.x14.get();
    v.x4.set(1);
    let next = v.x34.get();
    ip.xD44_lifeTimer = frames;
    if let Some(next) = next {
        let v = item_mut(r.game, next).xDD4_itemVar.pikachu_thunder();
        v.x28.set(pos);
        v.x4.set(1);
    }
}

/// `itPikachuthunder_UnkMotion2_Anim`: the stopped link's lifetime, and it stretches
/// (`x10`, its length plus a frame's fall) to scale its model and hitbox.
#[doc(alias = "itPikachuthunder_UnkMotion2_Anim")]
fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    if tick(r, gobj) {
        return true;
    }
    let length = reg(attr_f32(r, gobj, attr::LENGTH));
    let v = item_mut(r.game, gobj).xDD4_itemVar.pikachu_thunder();
    let x10 = fp::fadds(reg(v.xC.get()), reg(v.x1C.get().y));
    v.x10.set(mem(x10));
    if reg(v.x10.get()) > 0.0 {
        v.x18.set(mem(stretch(reg(v.x10.get()), length)));
        if reg(v.x18.get()) <= 0.0 {
            v.x18.set(0.01);
        }
        let s = stretch(reg(v.x10.get()), reg(v.xC.get()));
        hitbox::scale_hitbox(r.game, gobj, 0, s);
        let v = item_mut(r.game, gobj).xDD4_itemVar.pikachu_thunder();
        let x10 = v.x10.get();
        v.xC.set(x10);
    }
    let j = jobj_of(r.game, gobj).expect("a bolt without a model");
    let s = item_mut(r.game, gobj)
        .xDD4_itemVar
        .pikachu_thunder()
        .x18
        .get();
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.scale.y = s);
    false
}

/// `a / b` rounded to four places (`(int)(1 + 10000 a / b) / 10000`).
fn stretch(a: f64, b: f64) -> f64 {
    let f = fp::fdivs(fp::fmuls(10000.0, a), b);
    fp::fdivs(fp::frsp(f64::from(fp::fctiwz(fp::fadds(1.0, f)))), 10000.0)
}

/// `itPikachuThunder_Logic39_EvtUnk`: forgets its owner if that's the GObj that goes, then
/// any GObj it points to (`it_8026B894`).
// Ports `itPikachuThunder_Logic39_EvtUnk`; aliased once a check reaches it.
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    let v = item_mut(r.game, gobj).xDD4_itemVar.pikachu_thunder();
    if v.x38.get() == other {
        v.x38.set(None);
    }
    lib26b1::forget(r.game, gobj, other);
}
