//! Snorlax (`It_PKind_Kabigon`), `melee/it/kinds/itkabigon.c`: out of the ball it lands with a
//! quake, leaps up off the top of the screen when its script says, waits there hidden and grown
//! to its falling size, then drops back down through the stage from the top of the blast zone,
//! gone past the bottom, puffing dust as it goes.

use gekko_fp as fp;

use super::super::ground::{fall, ground_or_fall};
use super::super::item::{change, change_state, play_sfx};
use super::super::pokemon::{self, attr_f32, attr_i32, effect_hitlag};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725, maplib};
use crate::fpu::{mem, reg};
use crate::game::camera;
use crate::game::ef::{self, Arg};
use crate::game::gr::stage;
use crate::game::handles::CodeRef;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x802C_9D40;
pub const LANDED_NOTHING: u32 = 0x802C_9DFC;
pub const DESTROYED: u32 = 0x802C_9E00;
pub const EVT_UNK: u32 = 0x802C_9E24;
pub const OUT_LANDED: u32 = 0x802C_9E44;
pub const ANIM0: u32 = 0x802C_9EF0;
pub const PHYS0: u32 = 0x802C_9F74;
pub const COLL0: u32 = 0x802C_9FBC;
pub const DUST_ACCESSORY: u32 = 0x802C_A014;
pub const ANIM1: u32 = 0x802C_A198;
pub const PHYS1: u32 = 0x802C_A318;
pub const COLL1: u32 = 0x802C_A31C;
pub const ANIM2: u32 = 0x802C_A388;
pub const PHYS2: u32 = 0x802C_A3AC;
pub const COLL2: u32 = 0x802C_A3CC;

/// Its variables past the release's (`itKabigon_ItemVars`): its leap's speed (`timer`, a
/// float), its fall's speed (`x64`), its size up there (`x68`), the frames it waits hidden
/// (`x6C`), the frames to the next dust puff (`xE44`) and its attribute's dust spread as an
/// integer (`xE48`, never read).
const LEAP: usize = 0x60;
const DROP: usize = 0x64;
const SIZE: usize = 0x68;
const WAIT: usize = 0x6C;
const DUST_FRAMES: usize = 0x70;
const SPREAD: usize = 0x74;

/// `QuakeKind_Small` and `QuakeKind_Medium`.
const QUAKE_SMALL: i32 = 2;
const QUAKE_MEDIUM: i32 = 3;

/// Runs one of Snorlax's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 => anim0(r, gobj),
        COLL0 => coll0(r, gobj),
        ANIM1 => anim1(r, gobj),
        COLL1 => false,
        ANIM2 => anim2(r, gobj),
        COLL2 => coll2(r, gobj),
        _ => return None,
    })
}

/// Runs one of Snorlax's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        LANDED_NOTHING => {}
        DESTROYED => destroyed(r, gobj),
        OUT_LANDED => out_landed(r, gobj),
        PHYS0 => phys0(r, gobj),
        DUST_ACCESSORY => dust_when_due(r, gobj),
        PHYS1 => {}
        PHYS2 => phys2(r, gobj),
        _ => return false,
    }
    true
}

/// Snorlax out of the ball, `it_802C9D40`: its leap, fall, size, wait and dust from its
/// attributes, kept in the blast zones, its script's flags clear but the third, at its first
/// size, its sound, and into the air out of the ball.
#[doc(alias = "it_802C9D40")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let (leap, drop, size, wait, dust, spread) = (
        attr_f32(r, gobj, 4),
        attr_f32(r, gobj, 8),
        attr_f32(r, gobj, 0xC),
        attr_i32(r, gobj, 0x10),
        attr_i32(r, gobj, 0x14),
        attr_f32(r, gobj, 0x18),
    );
    let v = pokemon::vars(r.game, gobj);
    v.set_f32(LEAP, leap);
    v.set_f32(DROP, drop);
    v.set_f32(SIZE, size);
    v.set_i32(WAIT, wait);
    v.set_i32(DUST_FRAMES, dust);
    v.set_i32(SPREAD, fp::fctiwz(reg(spread)));
    let ip = item_mut(r.game, gobj);
    ip.xDCC_flag.b3 = true;
    ip.xDAC_itcmd_var0 = 0;
    ip.xDB0_itcmd_var1 = 0;
    ip.xDB4_itcmd_var2 = 1;
    let first = attr_f32(r, gobj, 0);
    pokemon::start(r, gobj, first);
    play_sfx(r, gobj, 0x2723, 0x7F, 0x40);
    enter_air(r, gobj);
}

/// Landing or leaving the floor in its first state, `it_802C9DFC`: nothing.
#[doc(alias = "it_802C9DFC")]
pub fn landed_nothing(_: &mut Running, _: GObjId) {}

/// `itKabigon_Logic2_Destroyed`: the small quake its drop started stops.
#[doc(alias = "itKabigon_Logic2_Destroyed")]
pub fn destroyed(r: &mut Running, _gobj: GObjId) {
    camera::stop_quake(&mut r.game.camera.cam, QUAKE_SMALL);
}

/// Forgets a GObj going away, `it_802C9E24` (`it_8026B894`).
#[doc(alias = "it_802C9E24")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// Lands out of the ball, `it_802C9E44`: kept in the blast zones but the top, stopped, and
/// standing.
#[doc(alias = "it_802C9E44")]
pub fn out_landed(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).xDCC_flag.b4567 = 0xE;
    lib2725::clear_velocity(r.game, gobj);
    stand(r, gobj);
}

/// Stands, `it_802C9E8C`: puffing dust once it leaps, with a medium quake.
#[doc(alias = "it_802C9E8C")]
pub fn stand(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(DUST_ACCESSORY));
    camera::request_quake(r, QUAKE_MEDIUM, None);
}

/// `itKabigon_UnkMotion0_Anim`: once its script says, it leaps at its leap speed into the air;
/// past the top of the blast zone it goes up there.
#[doc(alias = "itKabigon_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).xDB0_itcmd_var1 != 0 {
        let leap = pokemon::vars(r.game, gobj).f32(LEAP);
        let ip = item_mut(r.game, gobj);
        ip.x40_vel.y = leap;
        maplib::enter_air(ip);
        if ip.xDB4_itcmd_var2 != 0 {
            ip.xDB4_itcmd_var2 = 0;
        }
    }
    let top = stage::blast_top(r.game);
    if reg(item(r.game, gobj).pos.y) > reg(top) {
        go_up(r, gobj);
    }
    false
}

/// `itKabigon_UnkMotion0_Phys`: until it leaps, gravity in the air.
#[doc(alias = "itKabigon_UnkMotion0_Phys")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    let ip = item(r.game, gobj);
    if ip.xDB0_itcmd_var1 == 0 && ip.ground_or_air == 1 {
        super::gravity(r, gobj);
    }
}

/// `itKabigon_UnkMotion0_Coll`: until it leaps, lands in the air or walks off the floor,
/// doing nothing either way.
#[doc(alias = "itKabigon_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item(r.game, gobj);
    if ip.xDB0_itcmd_var1 == 0 {
        if ip.ground_or_air == 1 {
            fall(r, gobj, landed_nothing);
        } else {
            ground_or_fall(r, gobj, landed_nothing);
        }
    }
    false
}

/// Its `on_accessory`, `it_802CA014`: once it leaps, a dust puff every so many frames (the
/// count from `itPokemonAttributes`' `x14`).
#[doc(alias = "it_802CA014")]
pub fn dust_when_due(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).xDB0_itcmd_var1 == 0 {
        return;
    }
    let v = pokemon::vars(r.game, gobj);
    let left = v.i32(DUST_FRAMES).wrapping_sub(1);
    v.set_i32(DUST_FRAMES, left);
    if left == 0 {
        dust(r, gobj);
        let frames = attr_i32(r, gobj, 0x14);
        pokemon::vars(r.game, gobj).set_i32(DUST_FRAMES, frames);
    }
}

/// Its model at its size up there, as one value in all three axes.
fn set_size(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj).expect("Snorlax without a model");
    let s = pokemon::vars(r.game, gobj).f32(SIZE);
    jobj::set_scale(&mut r.game.objs.jobjs, j, Vec3::new(s, s, s));
}

/// Goes up off the screen, `it_802CA074`: hidden, waiting, still, at its size up there, its
/// sound.
#[doc(alias = "it_802CA074")]
pub fn go_up(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
    item_mut(r.game, gobj).x40_vel.y = 0.0;
    set_size(r, gobj);
    play_sfx(r, gobj, 0x2724, 0x7F, 0x40);
}

/// `itKabigon_UnkMotion1_Anim`: waiting up there its frames, then shown at the top of the
/// blast zone dropping at its fall speed, with a small quake; gone past the bottom. Its
/// animation loops at its size up there (`itKabigon_UnkMotion1_Anim_inline`).
#[doc(alias = "itKabigon_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).xDAC_itcmd_var0 == 0 {
        let v = pokemon::vars(r.game, gobj);
        let wait = v.i32(WAIT);
        if wait <= 0 {
            let drop = v.f32(DROP);
            item_mut(r.game, gobj).xDAC_itcmd_var0 = 1;
            let j = jobj_of(r.game, gobj);
            jobj::clear_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
            let top = stage::blast_top(r.game);
            let ip = item_mut(r.game, gobj);
            ip.pos.y = top;
            ip.x40_vel.y = drop;
            camera::request_quake(r, QUAKE_SMALL, None);
        } else {
            v.set_i32(WAIT, wait.wrapping_sub(1));
        }
    } else if reg(item(r.game, gobj).pos.y) < reg(stage::blast_bottom(r.game)) {
        return true;
    }
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 1, change::ANIM_UPDATE);
        effect_hitlag(r.game, gobj);
        set_size(r, gobj);
    }
    false
}

/// `itKabigon_UnkMotion1_Phys`: nothing.
#[doc(alias = "itKabigon_UnkMotion1_Phys")]
pub fn phys1(_: &mut Running, _: GObjId) {}

/// `itKabigon_UnkMotion1_Coll`: nothing.
#[doc(alias = "itKabigon_UnkMotion1_Coll")]
pub fn coll1(_: &mut Running, _: GObjId) -> bool {
    false
}

/// Into the air out of the ball, `it_802CA324` (`Item_EnterAirStateWithHitlag`, state 2).
#[doc(alias = "it_802CA324")]
pub fn enter_air(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    change_state(r, gobj, 2, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
}

/// `itKabigon_UnkMotion2_Anim`: growing out of the ball.
#[doc(alias = "itKabigon_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::grow(r, gobj);
    false
}

/// `itKabigon_UnkMotion2_Phys`: falling out of the ball.
#[doc(alias = "itKabigon_UnkMotion2_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId) {
    pokemon::fall(r, gobj);
}

/// `itKabigon_UnkMotion2_Coll`: lands out of the ball.
#[doc(alias = "itKabigon_UnkMotion2_Coll")]
pub fn coll2(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::land(r, gobj, out_landed)
}

/// A dust puff, `it_802CA3F4`: at a random point within its spread (`x18`) of it in x and y.
#[doc(alias = "it_802CA3F4")]
pub fn dust(r: &mut Running, gobj: GObjId) {
    let spread = reg(attr_f32(r, gobj, 0x18));
    let mut pos = item(r.game, gobj).pos;
    let rnd = reg(r.game.objs.rng.draw_f32());
    pos.x = mem(fp::fadds(
        reg(pos.x),
        fp::fmsubs(fp::fmuls(2.0, spread), rnd, spread),
    ));
    let rnd = reg(r.game.objs.rng.draw_f32());
    pos.y = mem(fp::fadds(
        reg(pos.y),
        fp::fmsubs(fp::fmuls(2.0, spread), rnd, spread),
    ));
    ef::spawn(r, 0x405, Some(gobj), &[Arg::Vec(pos)]);
}
