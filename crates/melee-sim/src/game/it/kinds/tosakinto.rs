//! Goldeen (`It_PKind_Tosakinto`), `melee/it/kinds/ittosakinto.c`: out of the ball it lands
//! and flops about, each flop a splash, its next animation of three and a sideways push either
//! way, falling off ledges and flopping again where it lands, until its life is up. Falling
//! out of the stage, it splashes a last sound.

use gekko_fp as fp;

use super::super::ground::{air, fall};
use super::super::item::{attr_of, change, change_state, play_sfx};
use super::super::pokemon::{self, attr_f32, take_bone_motion};
use super::super::{data, item, item_mut, jobj_of, lib26b1, lib2725, maplib};
use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::lb::sound_obj::SoundParams;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x802C_8F4C;
pub const EVT_UNK: u32 = 0x802C_8FC4;
pub const LANDED: u32 = 0x802C_8FE4;
pub const FLOP: u32 = 0x802C_90E8;
pub const ANIM2: u32 = 0x802C_9190;
pub const PHYS2: u32 = 0x802C_9204;
pub const COLL2: u32 = 0x802C_9240;
pub const ENTER_AIR: u32 = 0x802C_93BC;
pub const ANIM4: u32 = 0x802C_93FC;
pub const PHYS4: u32 = 0x802C_9420;
pub const COLL4: u32 = 0x802C_9440;
pub const FALL_OFF: u32 = 0x802C_9468;
pub const PHYS3: u32 = 0x802C_94A8;
pub const COLL3: u32 = 0x802C_94D8;
pub const DESTROYED: u32 = 0x802C_9504;

/// Its variables past the release's (`itTosakinto_ItemVars`): its next flop's state
/// (`x60_msid`), its sideways push (`x64_vel_x`) and the sound its landing kept (`x68_sfx_id`,
/// `SFX_NONE` until then).
const MSID: usize = 0x60;
const PUSH: usize = 0x64;
const SFX: usize = 0x68;

/// Runs one of Goldeen's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM2 => anim2(r, gobj),
        COLL2 => coll2(r, gobj),
        ANIM4 => anim4(r, gobj),
        COLL4 => coll4(r, gobj),
        COLL3 => coll3(r, gobj),
        _ => return None,
    })
}

/// Runs one of Goldeen's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        LANDED => landed(r, gobj),
        FLOP => flop(r, gobj),
        PHYS2 => phys2(r, gobj),
        ENTER_AIR => enter_air(r, gobj),
        PHYS4 => phys4(r, gobj),
        FALL_OFF => fall_off(r, gobj),
        PHYS3 => phys3(r, gobj),
        DESTROYED => destroyed(r, gobj),
        _ => return false,
    }
    true
}

/// `it_802C8F4C`, its spawn: its first flop's state, its life, no sound kept, at its first
/// size, its sound, into the air out of the ball.
#[doc(alias = "it_802C8F4C")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let life = attr_f32(r, gobj, 4);
    pokemon::vars(r.game, gobj).set_i32(MSID, 0);
    item_mut(r.game, gobj).xD44_lifeTimer = life;
    pokemon::vars(r.game, gobj).set_i32(SFX, -1);
    let size = attr_f32(r, gobj, 0);
    pokemon::start(r, gobj, size);
    play_sfx(r, gobj, 0x272E, 0x7F, 0x40);
    enter_air(r, gobj);
}

/// Forgets a GObj going away, `it_802C8FC4` (`it_8026B894`).
#[doc(alias = "it_802C8FC4")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// Lands out of the ball, `it_802C8FE4`: its body at the item's scale, still, its first flop,
/// and the sound its spawn played kept for its fall.
#[doc(alias = "it_802C8FE4")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    let child = jobj_of(r.game, gobj).and_then(|j| r.game.objs.jobjs[j].child);
    let s = attr_of(item(r.game, gobj)).f32(r.data, data::attr::SCALE);
    let child = child.expect("HSD_JObjSetScale on no joint");
    jobj::set_scale(&mut r.game.objs.jobjs, child, Vec3::new(s, s, s));
    lib2725::clear_velocity(r.game, gobj);
    flop(r, gobj);
    let sfx = item(r.game, gobj).xD6C;
    pokemon::vars(r.game, gobj).set_i32(SFX, sfx);
}

/// Flops, `it_802C90E8`: a splash, into the air at the next of its three flops, pushed
/// sideways by its attribute's speed, either way at random.
#[doc(alias = "it_802C90E8")]
pub fn flop(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    pokemon::effect_scaled(r, 0x46E, gobj, j, 2.0);
    maplib::enter_air(item_mut(r.game, gobj));
    let v = pokemon::vars(r.game, gobj);
    let mut msid = v.i32(MSID);
    if msid >= 3 {
        msid = 0;
    }
    v.set_i32(MSID, msid);
    let msid = v.i32(MSID);
    change_state(r, gobj, msid, change::ANIM_UPDATE);
    let push = attr_f32(r, gobj, 8);
    pokemon::vars(r.game, gobj).set_f32(PUSH, push);
    if r.game.objs.rng.below(2) != 0 {
        let v = pokemon::vars(r.game, gobj);
        let p = v.f32(PUSH);
        v.set_f32(PUSH, mem(fp::fneg(reg(p))));
    }
}

/// `itTosakinto_UnkMotion2_Anim` (its three flops): its body's motion taken as its own, gone
/// once its life is up (`Item_TickLifetime`).
#[doc(alias = "itTosakinto_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    let child = jobj_of(r.game, gobj).and_then(|j| r.game.objs.jobjs[j].child);
    take_bone_motion(r, child, gobj);
    let ip = item_mut(r.game, gobj);
    if reg(ip.xD44_lifeTimer) <= 0.0 {
        return true;
    }
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    false
}

/// `itTosakinto_UnkMotion2_Phys`: moved by its body, plus its push.
#[doc(alias = "itTosakinto_UnkMotion2_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId) {
    pokemon::move_by_bone(r.game, gobj);
    let push = pokemon::vars(r.game, gobj).f32(PUSH);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fadds(reg(ip.x40_vel.x), reg(push)));
}

/// `itTosakinto_UnkMotion2_Coll`: touching the stage flops again; at its animation's end
/// without touching it, it falls.
#[doc(alias = "itTosakinto_UnkMotion2_Coll")]
pub fn coll2(r: &mut Running, gobj: GObjId) -> bool {
    let touched = air(r, gobj);
    let next = |r: &mut Running| {
        let v = pokemon::vars(r.game, gobj);
        let m = v.i32(MSID).wrapping_add(1);
        v.set_i32(MSID, m);
    };
    if !lib2725::is_animating(r.game, gobj) {
        next(r);
        if touched {
            flop(r, gobj);
        } else {
            fall_off(r, gobj);
        }
    } else if touched {
        next(r);
        flop(r, gobj);
    }
    false
}

/// Into the air out of the ball, `it_802C93BC`: its growing state.
#[doc(alias = "it_802C93BC")]
pub fn enter_air(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    change_state(r, gobj, 4, change::ANIM_UPDATE);
}

/// `itTosakinto_UnkMotion4_Anim`: growing out of the ball.
#[doc(alias = "itTosakinto_UnkMotion4_Anim")]
pub fn anim4(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::grow(r, gobj);
    false
}

/// `itTosakinto_UnkMotion4_Phys`: falling out of the ball.
#[doc(alias = "itTosakinto_UnkMotion4_Phys")]
pub fn phys4(r: &mut Running, gobj: GObjId) {
    pokemon::fall(r, gobj);
}

/// `itTosakinto_UnkMotion4_Coll`: lands out of the ball.
#[doc(alias = "itTosakinto_UnkMotion4_Coll")]
pub fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::land(r, gobj, landed)
}

/// Falls, `it_802C9468`: into the air in its falling state.
#[doc(alias = "it_802C9468")]
pub fn fall_off(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    change_state(r, gobj, 3, change::ANIM_UPDATE);
}

/// `itTosakinto_UnkMotion3_Phys`: its gravity.
#[doc(alias = "itTosakinto_UnkMotion3_Phys")]
pub fn phys3(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
}

/// `itTosakinto_UnkMotion3_Coll`: flops where it lands.
#[doc(alias = "itTosakinto_UnkMotion3_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, flop);
    false
}

/// `itTosakinto_Logic0_Destroyed`: gone below the bottom blast zone (with 50 to spare), its
/// fall's sound, on the voice its landing kept.
#[doc(alias = "itTosakinto_Logic0_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    let bottom = reg(crate::game::gr::stage::blast_bottom(r.game));
    let y = item(r.game, gobj).pos.y;
    if fp::fsubs(reg(y), 50.0) < bottom {
        let voice = pokemon::vars(r.game, gobj).i32(SFX);
        r.lb_create_sound_object(&SoundParams {
            owner: gobj,
            behavior: 9,
            sfx_id: 0x272E,
            start_vol: 0x7F,
            end_vol: 0x7F,
            pan_left: 0x7F,
            pan_right: 0,
            end_frame: 0x3C,
            direction: 3.0,
            track: 0,
            voice,
        });
    }
}
