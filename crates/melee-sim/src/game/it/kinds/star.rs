//! The Starman, `melee/it/kinds/itstar.c`: it bounces along, turning at walls; a fighter it
//! touches turns invincible (`ftColl_80077C60`).

use gekko_fp as fp;

use super::super::item::{article, change, change_state, play_sfx};
use super::super::{ground, item, item_mut, lib26b1, maplib};
use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const SPAWNED: u32 = 0x8028_4644;
pub const ANIM0: u32 = 0x8028_46FC;
pub const PHYS0: u32 = 0x8028_4704;
pub const COLL0: u32 = 0x8028_4734;
pub const DMG_DEALT: u32 = 0x8028_482C;

/// Runs one of the Starman's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 => still(r, gobj),
        COLL0 => coll0(r, gobj),
        DMG_DEALT => dmg_dealt(r, gobj),
        _ => return None,
    })
}

/// Runs one of the Starman's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        PHYS0 => super::gravity(r, gobj),
        _ => return false,
    }
    true
}

/// The bounce's speeds, from its attributes (`itStar_ItemVars` laid over them: `xvel`, `yvel`).
fn bounce_speeds(r: &Running, gobj: GObjId) -> (f32, f32) {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a Starman without attributes");
    (r.data.f32(a.at(0x10)), r.data.f32(a.at(0x14)))
}

/// `itStar_Logic10_Spawned`: off to a random side, bouncing.
#[doc(alias = "itStar_Logic10_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let (vx, vy) = bounce_speeds(r, gobj);
    lib26b1::enable_pickup(r.game, gobj);
    let dir = if r.game.objs.rng.below(2) != 0 {
        1.0
    } else {
        -1.0
    };
    let ip = item_mut(r.game, gobj);
    ip.xDD4_itemVar.star().dir.set(dir);
    let dir = ip.xDD4_itemVar.star().dir.get();
    ip.x40_vel.x = mem(fp::fmuls(reg(vx), reg(dir)));
    ip.x40_vel.y = vy;
    ip.x40_vel.z = 0.0;
    bounce_state(r, gobj);
}

/// Bounces, `it_802846D4`.
#[doc(alias = "it_802846D4")]
pub fn bounce_state(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// `itStar_UnkMotion0_Anim`: nothing to animate.
#[doc(alias = "itStar_UnkMotion0_Anim")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// `itStar_UnkMotion0_Coll`: bounces off the floor anew, turns back at walls, stops rising at
/// a ceiling, and sounds when it touched any.
#[doc(alias = "itStar_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    let mut touched = false;
    if ground::air(r, gobj) {
        let (vx, vy) = bounce_speeds(r, gobj);
        let ip = item_mut(r.game, gobj);
        let dir = ip.xDD4_itemVar.star().dir.get();
        ip.x40_vel.x = mem(fp::fmuls(reg(vx), reg(dir)));
        ip.x40_vel.y = vy;
        change_state(r, gobj, 0, 0x1 | change::HIT_PRESERVE);
        touched = true;
    }
    if maplib::touched_wall(r.game, gobj) != 0 {
        let ip = item_mut(r.game, gobj);
        let v = ip.xDD4_itemVar.star();
        let dir = v.dir.get();
        v.dir.set(mem(fp::fneg(reg(dir))));
        ip.x40_vel.x = mem(fp::fneg(reg(ip.x40_vel.x)));
        touched = true;
    }
    if maplib::touched_ceiling(r.game, gobj) != 0 {
        let ip = item_mut(r.game, gobj);
        if reg(ip.x40_vel.y) > 0.0 {
            ip.x40_vel.y = 0.0;
        }
        touched = true;
    }
    if touched {
        play_sfx(r, gobj, 0xFA, 0x7F, 0x40);
    }
    false
}

/// `itStar_Logic10_DmgDealt`: gone once it touched a fighter.
#[doc(alias = "itStar_Logic10_DmgDealt")]
pub fn dmg_dealt(_: &mut Running, _: GObjId) -> bool {
    true
}

/// Forgets a GObj the Starman points to, `itStar_Logic10_EvtUnk` (`it_8026B894`).
#[doc(alias = "itStar_Logic10_EvtUnk")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
