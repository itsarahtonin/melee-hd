//! Ho-oh (`It_PKind_Houou`), `melee/it/kinds/ithouou.c`: out of the ball it flies up off the
//! top of the screen, comes back into the background towards the camera, sinks to where it
//! came out, and calls down Sacred Fire on the stage below (or above) where it came out, its
//! script saying when.
//!
//! The unit also holds the Sacred Fire's code (`It_Kind_Houou_SacredFire`): it burns where it
//! was made, riding the stage line it was made on, until its life runs out or it touches the
//! stage.

use gekko_fp as fp;

use super::super::item::{change, change_state, spawn_air};
use super::super::pokemon::{self, attr_f32, attr_i32, effect_hitlag, enter_first_state};
use super::super::{ground, item, item_mut, jobj_of, lib26b1, lib2725, set_flag};
use crate::fpu::{mem, reg};
use crate::game::gr::stage;
use crate::game::handles::CodeRef;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x802D_251C;
pub const LANDED_NOTHING: u32 = 0x802D_25B8;
pub const EVT_UNK: u32 = 0x802D_25BC;
pub const ANIM1: u32 = 0x802D_25DC;
pub const PHYS1: u32 = 0x802D_2640;
pub const COLL1: u32 = 0x802D_2660;
pub const ANIM2: u32 = 0x802D_26BC;
pub const PHYS2: u32 = 0x802D_26FC;
pub const COLL2: u32 = 0x802D_27A8;
pub const ANIM3: u32 = 0x802D_27FC;
pub const PHYS3: u32 = 0x802D_288C;
pub const COLL3: u32 = 0x802D_2904;
pub const ANIM4: u32 = 0x802D_2970;
pub const PHYS4: u32 = 0x802D_29D4;
pub const COLL4: u32 = 0x802D_2A50;
pub const ANIM5: u32 = 0x802D_2AC0;
pub const PHYS5: u32 = 0x802D_2B24;
pub const COLL5: u32 = 0x802D_2B44;
pub const FIRE_ACCESSORY: u32 = 0x802D_2B4C;
pub const ANIM0: u32 = 0x802D_2C54;
pub const PHYS0: u32 = 0x802D_2C78;
pub const COLL0: u32 = 0x802D_2D04;
pub const FIRE_SPAWNED: u32 = 0x802D_2E80;
pub const FIRE_EVT_UNK: u32 = 0x802D_2ED0;
pub const FIRE_ANIM: u32 = 0x802D_2F3C;
pub const FIRE_PHYS: u32 = 0x802D_2F70;
pub const FIRE_COLL: u32 = 0x802D_2FE8;

/// `It_Kind_Houou_SacredFire`.
pub const SACRED_FIRE: i32 = 0xCB;

/// Its variables past the release's (`itHouou_ItemVars`): the frames it comes towards the
/// camera (`timer`; the Sacred Fire keeps its stage line there), where it came out
/// (`start_pos`) and its climb's or sink's speed (`vel_accum`).
const TIMER: usize = 0x60;
const HOME: usize = 0x64;
const CLIMB: usize = 0x70;

/// Runs one of Ho-oh's (or its Sacred Fire's) predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM1 => anim1(r, gobj),
        ANIM2 => anim2(r, gobj),
        ANIM3 => anim3(r, gobj),
        ANIM4 => anim4(r, gobj),
        ANIM5 => anim5(r, gobj),
        COLL1 | COLL2 | COLL3 | COLL4 | COLL5 => coll_nothing(r, gobj),
        ANIM0 => anim0(r, gobj),
        COLL0 => coll0(r, gobj),
        FIRE_ANIM => fire_anim(r, gobj),
        FIRE_COLL => fire_coll(r, gobj),
        _ => return None,
    })
}

/// Runs one of Ho-oh's (or its Sacred Fire's) events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        LANDED_NOTHING => {}
        PHYS1 | PHYS5 => phys1(r, gobj),
        PHYS2 => phys2(r, gobj),
        PHYS3 => phys3(r, gobj),
        PHYS4 => phys4(r, gobj),
        FIRE_ACCESSORY => call_fire_when_told(r, gobj),
        PHYS0 => phys0(r, gobj),
        FIRE_SPAWNED => fire_spawned(r, gobj),
        FIRE_PHYS => fire_phys(r, gobj),
        _ => return false,
    }
    true
}

/// Its root joint's child's motion taken as its own.
fn bone_motion(r: &mut Running, gobj: GObjId) {
    let bone = jobj_of(r.game, gobj).and_then(|j| r.game.objs.jobjs[j].child);
    pokemon::take_bone_motion(r, bone, gobj);
}

/// Into state `msid`, its effects pausing in hitlag.
fn enter(r: &mut Running, gobj: GObjId, msid: i32) {
    change_state(r, gobj, msid, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
}

/// `it_2725_Logic18_Spawned`: where it came out kept, facing nowhere, its script's flags clear,
/// its frames towards the camera from its attributes, not kept in the blast zones, into the air
/// out of the ball at its first size, not climbing.
#[doc(alias = "it_2725_Logic18_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let frames = attr_i32(r, gobj, 0x14);
    let pos = item(r.game, gobj).pos;
    pokemon::vars(r.game, gobj).set_vec(HOME, pos);
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = 0.0;
    ip.xDAC_itcmd_var0 = 0;
    ip.xDB0_itcmd_var1 = 0;
    pokemon::vars(r.game, gobj).set_i32(TIMER, frames);
    item_mut(r.game, gobj).xDCC_flag.b3 = false;
    enter_air(r, gobj);
    let size = attr_f32(r, gobj, 0);
    pokemon::start(r, gobj, size);
    pokemon::vars(r.game, gobj).set_f32(CLIMB, 0.0);
}

/// Landing out of the ball, `it_802D25B8`: nothing.
#[doc(alias = "it_802D25B8")]
pub fn landed_nothing(_: &mut Running, _: GObjId) {}

/// Forgets a GObj going away, `it_802D25BC` and the Sacred Fire's `it_802D2ED0`
/// (`it_8026B894`).
#[doc(alias = "it_802D25BC")]
#[doc(alias = "it_802D2ED0")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// `itHouou_UnkMotion1_Anim`: out of the ball, its bone's motion; it takes off when its
/// animation ends.
#[doc(alias = "itHouou_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    bone_motion(r, gobj);
    if !lib2725::is_animating(r.game, gobj) {
        take_off(r, gobj);
    }
    false
}

/// `itHouou_UnkMotion1_Phys` and `itHouou_UnkMotion5_Phys`: the bone's motion.
#[doc(alias = "itHouou_UnkMotion1_Phys")]
#[doc(alias = "itHouou_UnkMotion5_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    pokemon::move_by_bone(r.game, gobj);
}

/// `itHouou_UnkMotion1_Coll` to `itHouou_UnkMotion5_Coll`: nothing.
#[doc(alias = "itHouou_UnkMotion1_Coll")]
#[doc(alias = "itHouou_UnkMotion2_Coll")]
#[doc(alias = "itHouou_UnkMotion3_Coll")]
#[doc(alias = "itHouou_UnkMotion4_Coll")]
#[doc(alias = "itHouou_UnkMotion5_Coll")]
pub fn coll_nothing(_: &mut Running, _: GObjId) -> bool {
    false
}

/// Takes off, `it_802D2668`: from a standstill.
#[doc(alias = "it_802D2668")]
pub fn take_off(r: &mut Running, gobj: GObjId) {
    pokemon::vars(r.game, gobj).set_f32(CLIMB, 0.0);
    enter(r, gobj, 2);
}

/// `itHouou_UnkMotion2_Anim`: its bone's motion.
#[doc(alias = "itHouou_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    bone_motion(r, gobj);
    false
}

/// `itHouou_UnkMotion2_Phys`: the bone's motion and its climb, faster by `x8` a frame while its
/// animation plays and by `xC` after; past the top of the blast zone it stops and comes back.
#[doc(alias = "itHouou_UnkMotion2_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId) {
    pokemon::move_by_bone(r.game, gobj);
    let accel = if lib2725::is_animating(r.game, gobj) {
        attr_f32(r, gobj, 8)
    } else {
        attr_f32(r, gobj, 0xC)
    };
    let v = pokemon::vars(r.game, gobj);
    let climb = mem(fp::fadds(reg(v.f32(CLIMB)), reg(accel)));
    v.set_f32(CLIMB, climb);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = mem(fp::fadds(reg(ip.x40_vel.y), reg(climb)));
    let top = stage::blast_top(r.game);
    if reg(item(r.game, gobj).pos.y) > reg(top) {
        item_mut(r.game, gobj).x40_vel.y = 0.0;
        come_back(r, gobj);
    }
}

/// Comes back towards the camera, `it_802D27B0` (and its inline copy in
/// `itHouou_UnkMotion3_Anim`).
#[doc(alias = "it_802D27B0")]
pub fn come_back(r: &mut Running, gobj: GObjId) {
    enter(r, gobj, 3);
}

/// `itHouou_UnkMotion3_Anim`: its bone's motion, its animation over and over.
#[doc(alias = "itHouou_UnkMotion3_Anim")]
pub fn anim3(r: &mut Running, gobj: GObjId) -> bool {
    bone_motion(r, gobj);
    if !lib2725::is_animating(r.game, gobj) {
        come_back(r, gobj);
    }
    false
}

/// `itHouou_UnkMotion3_Phys`: the bone's motion; towards the camera at `x10` for its frames,
/// then it stops and sinks.
#[doc(alias = "itHouou_UnkMotion3_Phys")]
pub fn phys3(r: &mut Running, gobj: GObjId) {
    pokemon::move_by_bone(r.game, gobj);
    let v = pokemon::vars(r.game, gobj);
    let left = v.i32(TIMER);
    v.set_i32(TIMER, left.wrapping_sub(1));
    if left != 0 {
        let speed = attr_f32(r, gobj, 0x10);
        item_mut(r.game, gobj).x40_vel.z = speed;
        return;
    }
    item_mut(r.game, gobj).x40_vel.z = 0.0;
    sink(r, gobj);
}

/// Sinks, `it_802D290C`: from its attribute's speed (`x18`).
#[doc(alias = "it_802D290C")]
pub fn sink(r: &mut Running, gobj: GObjId) {
    enter(r, gobj, 4);
    let speed = attr_f32(r, gobj, 0x18);
    pokemon::vars(r.game, gobj).set_f32(CLIMB, speed);
}

/// `itHouou_UnkMotion4_Anim`: its bone's motion; it calls its fire when its animation ends.
#[doc(alias = "itHouou_UnkMotion4_Anim")]
pub fn anim4(r: &mut Running, gobj: GObjId) -> bool {
    bone_motion(r, gobj);
    if !lib2725::is_animating(r.game, gobj) {
        call_fire(r, gobj);
    }
    false
}

/// `itHouou_UnkMotion4_Phys`: the bone's motion and its sink, by `x1C` a frame more; stopped
/// once more than 10 below where it came out.
#[doc(alias = "itHouou_UnkMotion4_Phys")]
pub fn phys4(r: &mut Running, gobj: GObjId) {
    pokemon::move_by_bone(r.game, gobj);
    let accel = attr_f32(r, gobj, 0x1C);
    let v = pokemon::vars(r.game, gobj);
    let climb = mem(fp::fadds(reg(v.f32(CLIMB)), reg(accel)));
    v.set_f32(CLIMB, climb);
    let home_y = v.f32(HOME + 4);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = mem(fp::fadds(reg(ip.x40_vel.y), reg(climb)));
    if reg(ip.pos.y) < fp::fsubs(reg(home_y), 10.0) {
        ip.x40_vel.y = 0.0;
    }
}

/// Calls its fire, `it_802D2A58`: on its script's word, kept in the blast zones.
#[doc(alias = "it_802D2A58")]
pub fn call_fire(r: &mut Running, gobj: GObjId) {
    enter(r, gobj, 5);
    let ip = item_mut(r.game, gobj);
    ip.on_accessory = Some(CodeRef(FIRE_ACCESSORY));
    ip.xDCC_flag.b3 = true;
}

/// `itHouou_UnkMotion5_Anim`: calling its fire; gone when its animation ends.
#[doc(alias = "itHouou_UnkMotion5_Anim")]
pub fn anim5(r: &mut Running, gobj: GObjId) -> bool {
    bone_motion(r, gobj);
    !lib2725::is_animating(r.game, gobj)
}

/// Its `on_accessory`, `it_802D2B4C`: on its script's first word an effect on each of its
/// bones 16 and 17, on its second the Sacred Fire.
#[doc(alias = "it_802D2B4C")]
pub fn call_fire_when_told(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).xDAC_itcmd_var0 != 0 {
        item_mut(r.game, gobj).xDAC_itcmd_var0 = 0;
        let b16 = pokemon::table_bone(r.game, gobj, 16);
        pokemon::effect_scaled(r, 0x46F, gobj, b16, 1.3);
        let b17 = pokemon::table_bone(r.game, gobj, 17);
        pokemon::effect_scaled(r, 0x46F, gobj, b17, 1.3);
    }
    if item(r.game, gobj).xDB0_itcmd_var1 != 0 {
        item_mut(r.game, gobj).xDB0_itcmd_var1 = 0;
        sacred_fire(r, gobj);
    }
}

/// Into the air out of the ball, `it_802D2BE0` (`Item_EnterAirStateWithHitlagAndStateDesc`).
#[doc(alias = "it_802D2BE0")]
pub fn enter_air(r: &mut Running, gobj: GObjId) {
    enter_first_state(r, gobj);
}

/// `it_802D2C54`: growing out of the ball.
#[doc(alias = "it_802D2C54")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::grow(r, gobj);
    false
}

/// `it_802D2C78`: falling out of the ball; once its frames are up it stops and poses.
#[doc(alias = "it_802D2C78")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    if pokemon::fall(r, gobj) {
        lib2725::clear_velocity(r.game, gobj);
        enter(r, gobj, 1);
        set_flag(&mut item_mut(r.game, gobj).xDD1_flag, 1, true);
    }
}

/// `it_802D2D04`: lands out of the ball, doing nothing.
#[doc(alias = "it_802D2D04")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::land(r, gobj, landed_nothing)
}

/// The Sacred Fire, `it_802D2D2C`: on the stage straight below where Ho-oh came out (to the
/// bottom of the blast zone), else straight above it (from the top), still, keeping the line it
/// was made on; none if neither way meets the stage.
#[doc(alias = "it_802D2D2C")]
pub fn sacred_fire(r: &mut Running, gobj: GObjId) {
    let home = pokemon::vars(r.game, gobj).vec(HOME);
    let (bottom, top) = (stage::blast_bottom(r.game), stage::blast_top(r.game));
    let g = r.game.geometry.as_mut().expect("no collision geometry");
    let Some(hit) = g
        .check_all_remap(-1, -1, home.x, home.y, home.x, bottom)
        .or_else(|| g.check_all_remap(-1, -1, home.x, top, home.x, home.y))
    else {
        return;
    };
    let ip = item(r.game, gobj);
    let (facing, owner) = (ip.facing_dir, ip.owner);
    let mut spawn = SpawnItem {
        kind: SACRED_FIRE,
        ..Default::default()
    };
    spawn.prev_pos = hit.pos;
    spawn.pos = home;
    spawn.facing_dir = facing;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = owner;
    spawn.x4_parent_gobj2 = Some(gobj);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let fire = spawn_air(r, &mut spawn).expect("Item_80268B18 made no Sacred Fire");
    pokemon::vars(r.game, fire).set_i32(TIMER, hit.line);
}

/// The Sacred Fire's `it_2725_Logic42_Spawned`: its life, not spinning, burning, not pickable.
#[doc(alias = "it_2725_Logic42_Spawned")]
pub fn fire_spawned(r: &mut Running, gobj: GObjId) {
    let life = attr_f32(r, gobj, 0);
    item_mut(r.game, gobj).xD44_lifeTimer = life;
    lib2725::stop_spin(r.game, r.data, gobj);
    burn(r, gobj);
    lib26b1::disable_pickup(r.game, gobj);
}

/// The Sacred Fire burns, `it_802D2EF0`.
#[doc(alias = "it_802D2EF0")]
pub fn burn(r: &mut Running, gobj: GObjId) {
    enter(r, gobj, 0);
}

/// The Sacred Fire's `it_802D2F3C` (`Item_TickLifetime`): gone once its life has run out, else
/// a frame less.
#[doc(alias = "it_802D2F3C")]
pub fn fire_anim(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    if reg(ip.xD44_lifeTimer) <= 0.0 {
        return true;
    }
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    false
}

/// The Sacred Fire's `it_802D2F70`: carried by its line's motion while the line is live.
#[doc(alias = "it_802D2F70")]
pub fn fire_phys(r: &mut Running, gobj: GObjId) {
    let line = pokemon::vars(r.game, gobj).i32(TIMER);
    let pos = item(r.game, gobj).pos;
    let g = r.game.geometry.as_ref().expect("no collision geometry");
    if !g.line_live(line) {
        return;
    }
    let d = g
        .line_speed(line, pos)
        .expect("a live line without a speed");
    let ip = item_mut(r.game, gobj);
    ip.pos.x = mem(fp::fadds(reg(ip.pos.x), reg(d.x)));
    ip.pos.y = mem(fp::fadds(reg(ip.pos.y), reg(d.y)));
    ip.pos.z = mem(fp::fadds(reg(ip.pos.z), reg(d.z)));
}

/// The Sacred Fire's `it_802D2FE8`: gone when it touches the stage (`it_8026DA08`).
#[doc(alias = "it_802D2FE8")]
pub fn fire_coll(r: &mut Running, gobj: GObjId) -> bool {
    ground::air(r, gobj)
}
