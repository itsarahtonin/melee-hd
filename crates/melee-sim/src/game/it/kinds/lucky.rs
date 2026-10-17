//! Chansey (`It_PKind_Lucky`), `melee/it/kinds/itlucky.c`: out of the ball it lands and lays
//! eggs a random number of times, each one its script times (an `on_accessory` callback): a
//! Chansey egg (`It_Kind_Lucky_Egg`, healing) when healing items are on and a draw says so,
//! else a common egg (`itegg.c`); a hard enough hit knocks it away.
//!
//! The unit also holds the Chansey egg's code: it lies about until its life runs out, and can
//! be picked up and thrown.

use gekko_fp as fp;

use super::super::ground::{fall, ground_or_fall, ground_skid};
use super::super::item::{change, change_state, play_sfx, spawn_air};
use super::super::pokemon::{self, attr_f32, attr_i32, effect_hitlag};
use super::super::{hitbox, item, item_mut, lib26b1, lib2725, maplib, spawn};
use super::drmariopill::set_pose;
use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x802D_5050;
pub const LANDED: u32 = 0x802D_50F0;
pub const LAY_ACCESSORY: u32 = 0x802D_52E4;
pub const ANIM2: u32 = 0x802D_53AC;
pub const PHYS2: u32 = 0x802D_53F0;
pub const COLL2: u32 = 0x802D_53F4;
pub const OUT: u32 = 0x802D_5420;
pub const FELL: u32 = 0x802D_546C;
pub const ANIM5: u32 = 0x802D_54C4;
pub const PHYS5: u32 = 0x802D_54CC;
pub const COLL5: u32 = 0x802D_54FC;
pub const ANIM6: u32 = 0x802D_5528;
pub const PHYS6: u32 = 0x802D_5530;
pub const ENTER_AIR: u32 = 0x802D_5560;
pub const ANIM0: u32 = 0x802D_55DC;
pub const PHYS0: u32 = 0x802D_5600;
pub const COLL0: u32 = 0x802D_5620;
pub const DMG_RECEIVED: u32 = 0x802D_5648;
pub const EVT_UNK: u32 = 0x802D_56F0;
pub const EGG_SPAWNED: u32 = 0x802D_57D0;
pub const EGG_LANDED: u32 = 0x802D_582C;
pub const EGG_ANIM0: u32 = 0x802D_5884;
pub const EGG_PHYS0: u32 = 0x802D_58BC;
pub const EGG_COLL0: u32 = 0x802D_58C0;
pub const EGG_FELL: u32 = 0x802D_58EC;
pub const EGG_ANIM1: u32 = 0x802D_5914;
pub const EGG_PHYS1: u32 = 0x802D_594C;
pub const EGG_COLL1: u32 = 0x802D_597C;
pub const EGG_PICKED_UP: u32 = 0x802D_59A8;
pub const EGG_ANIM2: u32 = 0x802D_59D0;
pub const EGG_PHYS2: u32 = 0x802D_59D8;
pub const EGG_DROPPED: u32 = 0x802D_59DC;
pub const EGG_ENTERED_AIR: u32 = 0x802D_5A04;
pub const EGG_ANIM4: u32 = 0x802D_5A2C;
pub const EGG_PHYS4: u32 = 0x802D_5A64;
pub const EGG_COLL4: u32 = 0x802D_5A68;
pub const EGG_EVT_UNK: u32 = 0x802D_5A9C;

/// `It_Kind_Lucky_Egg`.
pub const LUCKY_EGG: i32 = 0xCD;

/// Its variables past the release's (`itLucky_ItemVars`): the state it's in or goes to next
/// (`x60`: 2 starting to lay, 3 laying, 4 done), the eggs left to lay (`x64`), and whether
/// healing items are on (`x68`). The egg keeps its `x4` attribute in `x60`.
const NEXT: usize = 0x60;
const EGGS: usize = 0x64;
const HEALING: usize = 0x68;

/// Runs one of Chansey's (or its egg's) predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM2 => anim2(r, gobj),
        COLL2 => coll2(r, gobj),
        ANIM5 | ANIM6 | EGG_ANIM2 => false,
        COLL5 => coll5(r, gobj),
        ANIM0 => anim0(r, gobj),
        COLL0 => coll0(r, gobj),
        DMG_RECEIVED => dmg_received(r, gobj),
        EGG_ANIM0 | EGG_ANIM1 | EGG_ANIM4 => egg_anim(r, gobj),
        EGG_COLL0 => egg_coll0(r, gobj),
        EGG_COLL1 => egg_coll1(r, gobj),
        EGG_COLL4 => egg_coll4(r, gobj),
        _ => return None,
    })
}

/// Runs one of Chansey's (or its egg's) events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        LANDED => landed(r, gobj),
        LAY_ACCESSORY => lay_when_told(r, gobj),
        PHYS2 | EGG_PHYS0 | EGG_PHYS2 | EGG_PHYS4 => {}
        OUT => out(r, gobj),
        FELL => fell(r, gobj),
        PHYS5 | PHYS6 | EGG_PHYS1 => super::gravity(r, gobj),
        ENTER_AIR => enter_air(r, gobj),
        PHYS0 => phys0(r, gobj),
        EGG_SPAWNED => egg_spawned(r, gobj),
        EGG_LANDED => egg_landed(r, gobj),
        EGG_FELL => egg_fell(r, gobj),
        EGG_PICKED_UP => egg_picked_up(r, gobj),
        EGG_DROPPED => egg_dropped(r, gobj),
        EGG_ENTERED_AIR => egg_entered_air(r, gobj),
        _ => return false,
    }
    true
}

/// `it_802D5050`, its spawn: facing nowhere, to start laying, its eggs drawn, at its first
/// size, whether healing items are on, its sound, into the air out of the ball.
#[doc(alias = "it_802D5050")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).facing_dir = 0.0;
    pokemon::vars(r.game, gobj).set_i32(NEXT, 2);
    let (base, range) = (attr_i32(r, gobj, 0x10), attr_i32(r, gobj, 0x14));
    let n = r.game.objs.rng.below(range);
    pokemon::vars(r.game, gobj).set_i32(EGGS, base.wrapping_add(n));
    let size = attr_f32(r, gobj, 0);
    pokemon::start(r, gobj, size);
    let healing = spawn::healing_on(r.game);
    pokemon::vars(r.game, gobj).set_i32(HEALING, i32::from(healing));
    play_sfx(r, gobj, 0x273B, 0x7F, 0x40);
    item_mut(r.game, gobj).xDAC_itcmd_var0 = 0;
    enter_air(r, gobj);
}

/// Lands and lays on, `it_802D50F0`.
#[doc(alias = "it_802D50F0")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    lib2725::clear_velocity(r.game, gobj);
    enter_next(r, gobj);
}

/// The next state of laying, `it_802D5124`: starting goes to laying, each laying counts an egg
/// down until the last, then done; whether it's done (and gone).
#[doc(alias = "it_802D5124")]
pub fn next(r: &mut Running, gobj: GObjId) -> bool {
    let v = pokemon::vars(r.game, gobj);
    let mut done = false;
    match v.i32(NEXT) {
        2 => v.set_i32(NEXT, 3),
        3 => {
            let n = v.i32(EGGS).wrapping_sub(1);
            v.set_i32(EGGS, n);
            let next = if v.i32(EGGS) != 0 { 3 } else { 4 };
            v.set_i32(NEXT, next);
        }
        4 => done = true,
        _ => {}
    }
    if !done {
        enter_next(r, gobj);
    }
    done
}

/// Lays an egg, `it_802D51C8`: from its egg bone, thrown up and a random way; a Chansey egg
/// when healing items are on and the draw isn't 0, else a common egg.
#[doc(alias = "it_802D51C8")]
pub fn lay(r: &mut Running, gobj: GObjId) {
    let (spread, up, odds) = (
        attr_f32(r, gobj, 4),
        attr_f32(r, gobj, 8),
        attr_i32(r, gobj, 0x18),
    );
    let f = r.game.objs.rng.draw_f32();
    let vx = mem(fp::fmuls(
        reg(spread),
        fp::fmuls(2.0, fp::fsubs(reg(f), 0.5)),
    ));
    let vel = Vec3::new(vx, up, 0.0);
    let facing = if reg(vel.x) >= 0.0 { 1.0 } else { -1.0 };
    let bone = pokemon::table_bone(r.game, gobj, 32);
    let mut pos = crate::lb::joint::world_position(&mut r.game.objs, r.data, bone, None);
    pos.z = 0.0;
    let mut draw = 0;
    if pokemon::vars(r.game, gobj).i32(HEALING) != 0 {
        draw = r.game.objs.rng.below(odds);
    }
    let laid = if draw != 0 {
        spawn_egg(r, Some(gobj), pos, vel, facing)
    } else {
        super::egg::lay(r, Some(gobj), pos, vel, facing)
    };
    if laid.is_some() {
        lib2725::count_spawned(r.game);
    }
}

/// Its `on_accessory`, `it_802D52E4`: lays an egg when its script says, with a sound.
#[doc(alias = "it_802D52E4")]
pub fn lay_when_told(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).xDAC_itcmd_var0 != 0 {
        lay(r, gobj);
        play_sfx(r, gobj, 0x273C, 0x7F, 0x40);
        item_mut(r.game, gobj).xDAC_itcmd_var0 = 0;
    }
}

/// Into its next state (`x60`), `it_802D533C`: its script's flag clear, laying when told.
#[doc(alias = "it_802D533C")]
pub fn enter_next(r: &mut Running, gobj: GObjId) {
    let msid = pokemon::vars(r.game, gobj).i32(NEXT);
    change_state(r, gobj, msid, change::ANIM_UPDATE);
    item_mut(r.game, gobj).xDAC_itcmd_var0 = 0;
    effect_hitlag(r.game, gobj);
    item_mut(r.game, gobj).on_accessory = Some(crate::game::handles::CodeRef(LAY_ACCESSORY));
}

/// `it_802D53AC` (states 2 to 4): the next state at its animation's end; whether it's done.
#[doc(alias = "it_802D53AC")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        return next(r, gobj);
    }
    false
}

/// `it_802D53F0` (states 2 to 4): nothing.
#[doc(alias = "it_802D53F0")]
pub fn phys2(_: &mut Running, _: GObjId) {}

/// `it_802D53F4` (states 2 to 4): falls off the floor.
#[doc(alias = "it_802D53F4")]
pub fn coll2(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    false
}

/// Out of the ball, falls, `it_802D5420`: its hitbox on.
#[doc(alias = "it_802D5420")]
pub fn out(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    hitbox::end_blink(r.game, gobj);
    hitbox::set_hit_ids2(r.game, gobj);
    hitbox::clear_xdce_b2(r.game, gobj);
}

/// Falls off the floor, `it_802D546C`.
#[doc(alias = "it_802D546C")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 5, 1);
    item_mut(r.game, gobj).xDAC_itcmd_var0 = 0;
    effect_hitlag(r.game, gobj);
}

/// `itLucky_UnkMotion5_Anim`, `_6_Anim`, and the egg's `itLucky_UnkMotion2_Anim`: nothing.
#[doc(alias = "itLucky_UnkMotion5_Anim")]
#[doc(alias = "itLucky_UnkMotion6_Anim")]
#[doc(alias = "itLucky_UnkMotion2_Anim")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// `itLucky_UnkMotion5_Phys`, `_6_Phys`, and the egg's `itLucky_UnkMotion3_Phys`: falling by
/// its gravity.
#[doc(alias = "itLucky_UnkMotion5_Phys")]
#[doc(alias = "itLucky_UnkMotion6_Phys")]
#[doc(alias = "itLucky_UnkMotion3_Phys")]
pub fn phys5(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
}

/// `itLucky_UnkMotion5_Coll`: lands and lays on.
#[doc(alias = "itLucky_UnkMotion5_Coll")]
pub fn coll5(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, landed);
    false
}

/// Into the air out of the ball, `it_802D5560`: its first state at its first pose, no longer
/// kept clear of.
#[doc(alias = "it_802D5560")]
pub fn enter_air(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
    set_pose(r, gobj, 0, 0.0);
    lib26b1::stop_keeping_clear(r.game, gobj);
}

/// `it_802D55DC`: growing out of the ball.
#[doc(alias = "it_802D55DC")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::grow(r, gobj);
    false
}

/// `it_802D5600`: falling out of the ball.
#[doc(alias = "it_802D5600")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    pokemon::fall(r, gobj);
}

/// `it_802D5620`: lands out of the ball.
#[doc(alias = "it_802D5620")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::land(r, gobj, out)
}

/// `it_802D5648`, its damage taken: enough knocks it away, flying off blinking.
#[doc(alias = "it_802D5648")]
pub fn dmg_received(r: &mut Running, gobj: GObjId) -> bool {
    let limit = attr_f32(r, gobj, 0xC);
    let damage = fp::frsp(f64::from(item(r.game, gobj).xC9C));
    if damage >= reg(limit) {
        pokemon::launch(r, gobj);
        let ip = item_mut(r.game, gobj);
        ip.facing_dir = 0.0;
        maplib::enter_air(ip);
        hitbox::start_blink(r.game, gobj);
        change_state(r, gobj, 6, change::ANIM_UPDATE);
        set_pose(r, gobj, 0, 0.0);
    }
    false
}

/// Forgets a GObj going away, `it_802D56F0` and the egg's `itLucky_Logic44_EvtUnk`
/// (`it_8026B894`).
#[doc(alias = "it_802D56F0")]
#[doc(alias = "itLucky_Logic44_EvtUnk")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// Lays a Chansey egg from `parent`, `it_802D5710`: at `pos` on the plane, moving at `vel`,
/// facing `facing`, owned by the parent's owner.
#[doc(alias = "it_802D5710")]
pub fn spawn_egg(
    r: &mut Running,
    parent: Option<GObjId>,
    pos: Vec3,
    vel: Vec3,
    facing: f32,
) -> Option<GObjId> {
    let parent = parent?;
    let owner = item(r.game, parent).owner;
    let mut spawn = SpawnItem {
        kind: LUCKY_EGG,
        ..Default::default()
    };
    spawn.prev_pos = Vec3::new(pos.x, pos.y, 0.0);
    spawn.pos = spawn.prev_pos;
    spawn.facing_dir = facing;
    spawn.x3C_damage = 0;
    spawn.vel = vel;
    spawn.x0_parent_gobj = owner;
    spawn.x4_parent_gobj2 = Some(parent);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    spawn_air(r, &mut spawn)
}

/// The egg's `itLucky_Logic44_Spawned`: its life and `x4` from its attributes, falling.
#[doc(alias = "itLucky_Logic44_Spawned")]
pub fn egg_spawned(r: &mut Running, gobj: GObjId) {
    let life = attr_f32(r, gobj, 0);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    let x4 = attr_i32(r, gobj, 4);
    pokemon::vars(r.game, gobj).set_i32(NEXT, x4);
    egg_fell(r, gobj);
}

/// The egg lands, `it_802D582C`: pickable, still, lying.
#[doc(alias = "it_802D582C")]
pub fn egg_landed(r: &mut Running, gobj: GObjId) {
    lib26b1::enable_pickup(r.game, gobj);
    let ip = item_mut(r.game, gobj);
    // `itResetVelocity`: z, then y, then x.
    ip.x40_vel.z = 0.0;
    ip.x40_vel.y = 0.0;
    ip.x40_vel.x = 0.0;
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// The egg's `it_802D5884` (and `itLucky_UnkMotion3_Anim`, `it_802D5A2C`): gone when its life
/// runs out.
#[doc(alias = "it_802D5884")]
#[doc(alias = "itLucky_UnkMotion3_Anim")]
#[doc(alias = "it_802D5A2C")]
pub fn egg_anim(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    reg(ip.xD44_lifeTimer) <= 0.0
}

/// The egg's `it_802D58BC`, `itLucky_UnkMotion2_Phys` and `it_802D5A64`: nothing.
#[doc(alias = "it_802D58BC")]
#[doc(alias = "itLucky_UnkMotion2_Phys")]
#[doc(alias = "it_802D5A64")]
pub fn egg_idle(_: &mut Running, _: GObjId) {}

/// The egg's `it_802D58C0`: falls off the floor.
#[doc(alias = "it_802D58C0")]
pub fn egg_coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, egg_fell);
    false
}

/// The egg falls, `it_802D58EC`.
#[doc(alias = "it_802D58EC")]
pub fn egg_fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// The egg's `itLucky_UnkMotion3_Coll` (falling or dropped): lands.
#[doc(alias = "itLucky_UnkMotion3_Coll")]
pub fn egg_coll1(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, egg_landed);
    false
}

/// The egg's `itLucky_Logic44_PickedUp`: held.
#[doc(alias = "itLucky_Logic44_PickedUp")]
pub fn egg_picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// The egg's `itLucky_Logic44_Dropped`.
#[doc(alias = "itLucky_Logic44_Dropped")]
pub fn egg_dropped(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 3, change::ANIM_UPDATE);
}

/// The egg's `itLucky_Logic44_EnteredAir` (thrown).
#[doc(alias = "itLucky_Logic44_EnteredAir")]
pub fn egg_entered_air(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 4, change::ANIM_UPDATE);
}

/// The egg's `it_802D5A68` (thrown): lands, or skids and falls off.
#[doc(alias = "it_802D5A68")]
pub fn egg_coll4(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, egg_landed, egg_fell);
    false
}
