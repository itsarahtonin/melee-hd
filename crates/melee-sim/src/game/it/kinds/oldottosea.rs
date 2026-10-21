//! Topi, `melee/it/kinds/itoldottosea.c` (`It_Kind_Ottosea`), and the rest of its code, which the
//! decomp keeps at the start of `itwhitebea.c` (`it_802E31F8` to `it_802E37A4`): one of the
//! monsters a container releases (`it_8026F6BC`). It stands, walks a while (pausing now and
//! then), sometimes charges, turns at walls and ledges and when its walk's round ends; hit, it's
//! knocked back or, past its health, flung away as an enemy KO. A fighter can pick it up and
//! throw it. With Freezies on, one Topi in `x28` of its attributes' draws pushes a Freezie
//! along ([`super::freeze::spawn_pushed`]), letting go of it once it gets too far ahead.
//!
//! Its variables are the union's bytes ([`zako::bytes`]): the enemies' common words, then its
//! own ([`off`]).

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::item::{article, attr_of, change, change_state};
use super::super::{data, ground, hitbox, item, item_mut, lib26b1, lib2725, maplib, owner, zako};
use crate::fpu::{mem, reg};
use crate::game::camera;
use crate::game::gm;
use crate::game::gr::stage;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const SPAWNED: u32 = 0x802E_2470;
pub const DMG_RECEIVED: u32 = 0x802E_257C;
pub const DESTROYED: u32 = 0x802E_34DC;
pub const PICKED_UP: u32 = 0x802E_31F8;
pub const DROPPED: u32 = 0x802E_32B4;
pub const THROWN: u32 = 0x802E_3314;
pub const STAND: u32 = 0x802E_269C;
pub const FALL: u32 = 0x802E_2BC0;
pub const TURN: u32 = 0x802E_3098;
pub const LANDED_KNOCKED: u32 = 0x802E_2DF4;
pub const ANIM0: u32 = 0x802E_26F4;
pub const PHYS0: u32 = 0x802E_273C;
pub const COLL0: u32 = 0x802E_2778;
pub const ANIM2: u32 = 0x802E_28E0;
pub const PHYS2: u32 = 0x802E_2A58;
pub const COLL2: u32 = 0x802E_2B38;
pub const ANIM4: u32 = 0x802E_2C0C;
pub const PHYS4: u32 = 0x802E_2C14;
pub const COLL4: u32 = 0x802E_2C44;
pub const ANIM5: u32 = 0x802E_2CFC;
pub const PHYS5: u32 = 0x802E_2D68;
pub const COLL5: u32 = 0x802E_2D90;
pub const ANIM7: u32 = 0x802E_2E90;
pub const PHYS7: u32 = 0x802E_2F08;
pub const COLL7: u32 = 0x802E_3024;
pub const ANIM3: u32 = 0x802E_3110;
pub const PHYS3: u32 = 0x802E_31B8;
pub const COLL3: u32 = 0x802E_31BC;
pub const ANIM8: u32 = 0x802E_324C;
pub const PHYS8: u32 = 0x802E_32B0;
pub const ANIM9: u32 = 0x802E_3364;
pub const PHYS9: u32 = 0x802E_33AC;
pub const COLL9: u32 = 0x802E_33DC;
pub const ANIM10: u32 = 0x802E_3470;
pub const PHYS10: u32 = 0x802E_34B8;
pub const COLL10: u32 = 0x802E_34BC;
pub const ANIM11: u32 = 0x802E_3588;
pub const PHYS11: u32 = 0x802E_3590;
pub const COLL11: u32 = 0x802E_35AC;

/// Its variables' offsets in the union (`itOldottosea_ItemVars`).
pub mod off {
    /// `x18` (`whitebea.x18`, the common `x14.y`), cleared as it's flung away.
    pub const X18: usize = 0x18;
    /// `x20`: the Freezie it pushes.
    pub const FREEZIE: usize = 0x20;
    /// `x24`: its walk's or charge's frame count.
    pub const FRAME: usize = 0x24;
    /// `x28`: a state's timer.
    pub const TIMER: usize = 0x28;
    /// `x2C` (a halfword): whether it makes a Freezie when it next walks.
    pub const MAKES_FREEZIE: usize = 0x2C;
}

/// Its special attributes (`itOldottoseaAttributes`), `off` bytes in.
fn attr(r: &Running, gobj: GObjId, off: u32) -> DataRef {
    article(item(r.game, gobj))
        .special(r.data)
        .expect("a Topi without attributes")
        .at(off)
}

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    r.data.f32(attr(r, gobj, off))
}

/// `attr->x0`: its health (`x0`) and walking speed (`x4`).
fn health(r: &Running, gobj: GObjId) -> i32 {
    let p = r.data.ptr(attr(r, gobj, 0)).expect("a Topi's health");
    r.data.u32(p) as i32
}

fn walk_speed(r: &Running, gobj: GObjId) -> f32 {
    let p = r.data.ptr(attr(r, gobj, 0)).expect("a Topi's speed");
    r.data.f32(p.at(4))
}

fn get(r: &mut Running, gobj: GObjId, off: usize) -> i32 {
    zako::bytes(r.game, gobj).i32(off)
}

fn set(r: &mut Running, gobj: GObjId, off: usize, v: i32) {
    zako::bytes(r.game, gobj).set_i32(off, v);
}

fn freezie(r: &mut Running, gobj: GObjId) -> Option<GObjId> {
    zako::bytes(r.game, gobj).gobj(off::FREEZIE)
}

/// Lets go of its Freezie, if it pushes one: the Freezie forgets it (`it_8028ECE0`) and it the
/// Freezie (`it_802E37A4`).
fn let_go(r: &mut Running, gobj: GObjId) {
    if let Some(f) = freezie(r, gobj) {
        super::freeze::let_go(r, f);
        forget_freezie(r, gobj);
    }
}

/// The game's language setting (`lbLang_IsSettingJP`, `lbLang_IsSettingUS`).
fn is_jp(_: &Running) -> bool {
    gm::language_setting_is_jp()
}

fn is_us(_: &Running) -> bool {
    gm::language_setting_is_us()
}

/// `GA_Air`.
const GA_AIR: i32 = 1;
/// `QuakeKind_Small`.
const QUAKE_SMALL: i32 = 2;
/// `ITEM_UNK_0x1`, a state change's first flag.
const UNK_0X1: u32 = 1;

/// The collision's facing from the item's (`mpCollSetFacingDir`).
fn set_coll_facing(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    let f = if reg(ip.facing_dir) == fp::fneg(1.0) {
        -1
    } else {
        1
    };
    ip.x378_itemColl.set_facing_dir(f);
}

/// Stops (the inline `itResetVelocity`).
fn stop(r: &mut Running, gobj: GObjId) {
    let v = &mut item_mut(r.game, gobj).x40_vel;
    v.z = 0.0;
    v.y = 0.0;
    v.x = 0.0;
}

/// Runs one of Topi's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        DMG_RECEIVED => damaged(r, gobj),
        ANIM0 => anim0(r, gobj),
        COLL0 | COLL3 => coll_ground(r, gobj),
        ANIM2 => anim2(r, gobj),
        COLL2 => coll2(r, gobj),
        ANIM4 | ANIM11 => answers_false(r, gobj),
        COLL4 => coll4(r, gobj),
        ANIM5 => anim5(r, gobj),
        COLL5 => coll5(r, gobj),
        ANIM7 => anim7(r, gobj),
        COLL7 => coll7(r, gobj),
        ANIM3 => anim3(r, gobj),
        ANIM8 => anim8(r, gobj),
        ANIM9 => anim9(r, gobj),
        COLL9 => coll9(r, gobj),
        ANIM10 => anim10(r, gobj),
        COLL10 => coll10(r, gobj),
        COLL11 => watch(r, gobj),
        _ => return None,
    })
}

/// Runs one of Topi's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        DESTROYED => destroyed(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED => dropped(r, gobj),
        THROWN => thrown(r, gobj),
        STAND => stand(r, gobj),
        FALL => fall(r, gobj),
        TURN => turn(r, gobj),
        LANDED_KNOCKED => landed_knocked(r, gobj),
        PHYS0 => phys0(r, gobj),
        PHYS2 => phys2(r, gobj),
        PHYS4 | PHYS9 => phys_fall(r, gobj),
        PHYS5 => phys5(r, gobj),
        PHYS7 => phys7(r, gobj),
        PHYS3 | PHYS8 | PHYS10 => still(r, gobj),
        PHYS11 => phys_gravity(r, gobj),
        _ => return false,
    }
    true
}

/// Topi's setup, `it_802E2470`: the enemies' common variables, facing most fighters, upright,
/// its collision an enemy's, its timer 1, no Freezie, and with Freezies on a draw for making one;
/// then standing.
#[doc(alias = "it_802E2470")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    zako::reset(r.game, gobj);
    let pos = item(r.game, gobj).pos;
    let facing = owner::facing_for(r.game, r.data, pos);
    item_mut(r.game, gobj).facing_dir = facing;
    zako::face(r, gobj, facing);
    set_coll_facing(r, gobj);
    let ip = item_mut(r.game, gobj);
    ip.xD5C = 0;
    ip.xDCC_flag.b3 = false;
    ip.xDC8_word.flags.x15 = false;
    hitbox::clear_dcd_b7(r.game, gobj);
    zako::set_dd0_b4(r.game, gobj);
    item_mut(r.game, gobj).xDC8_word.flags.x19 = true;
    set(r, gobj, off::TIMER, 1);
    zako::bytes(r.game, gobj).set_gobj(off::FREEZIE, None);
    let makes = if r.game.vs_item_freq() == -1 {
        0
    } else if super::super::spawn::kind_on(r.game, super::FREEZE) && {
        let odds = i32::from(r.data.u8(attr(r, gobj, 0x28)) as i8);
        r.game.objs.rng.below(odds) == 0
    } {
        1
    } else {
        0
    };
    zako::bytes(r.game, gobj).set_i16(off::MAKES_FREEZIE, makes);
    stand(r, gobj);
}

/// Takes a hit, `it_2725_Logic8_DmgReceived`: the hit's element adds to the damage; past its
/// health (or hit while thrown, 9) it lets its Freezie go, is beaten (`Item_ZakoDefeat`) and
/// flung away (toward the screen at random); else knocked back.
#[doc(alias = "it_2725_Logic8_DmgReceived")]
pub fn damaged(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    ip.init_facing_dir = ip.facing_dir;
    let extra = reg(zako::element_damage(r, gobj));
    let ip = item_mut(r.game, gobj);
    ip.xC9C = fp::fctiwz(fp::fadds(fp::frsp(f64::from(ip.xC9C)), extra));
    let (damage, msid) = (ip.xC9C, ip.msid);
    if damage > health(r, gobj) || msid == 9 {
        let_go(r, gobj);
        // `Item_ZakoDefeat`.
        zako::hurt_sound(r, gobj);
        hitbox::start_blink(r.game, gobj);
        hitbox::clear_hit_ids(r.game, gobj);
        zako::ko(r, gobj);
        let pos = item(r.game, gobj).pos;
        camera::request_quake(r, QUAKE_SMALL, Some(pos));
        let f = reg(r.game.objs.rng.draw_f32());
        let z = r.game.it.zako.expect("no enemies' common data");
        if f < reg(r.data.f32(z.at(8))) {
            fling_to_screen(r, gobj);
        } else {
            fling(r, gobj);
        }
    } else {
        knocked_back(r, gobj);
    }
    false
}

/// Stands, `it_802E269C`: settled, its model turned to the floor, state 0.
#[doc(alias = "it_802E269C")]
pub fn stand(r: &mut Running, gobj: GObjId) {
    zako::settle(r, gobj);
    let facing = item(r.game, gobj).facing_dir;
    zako::align_floor(r, gobj, facing, 1.0);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// Standing's animation, `itOldottosea_UnkMotion0_Anim`: looped.
#[doc(alias = "itOldottosea_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 0, change::ANIM_UPDATE);
    }
    false
}

/// Standing's physics, `itOldottosea_UnkMotion0_Phys`: its timer done, it walks.
#[doc(alias = "itOldottosea_UnkMotion0_Phys")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    let t = get(r, gobj, off::TIMER);
    if t == 0 {
        walk(r, gobj);
        return;
    }
    set(r, gobj, off::TIMER, t.wrapping_sub(1));
}

/// The grounded states' collision (`itOldottosea_UnkMotion0_Coll`, `_3_Coll`): off the floor it
/// falls; then the blast zones and its Freezie.
#[doc(alias = "itOldottosea_UnkMotion0_Coll")]
#[doc(alias = "itOldottosea_UnkMotion3_Coll")]
pub fn coll_ground(r: &mut Running, gobj: GObjId) -> bool {
    ground::ground_or_fall(r, gobj, fall);
    watch(r, gobj)
}

/// Walks, `it_802E27B4`: stopped, its count reset, no blink; if it's to make a Freezie, one
/// `x14` ahead of it, slowing by `x24`, handed its (stopped) speed scaled by `x1C`; state 2 with
/// a Freezie, else 1.
///
/// ssbm-rs transliterates it from its machine code, as the Freezie's spawn takes its facing
/// and deceleration in the registers this function loads them into.
#[doc(alias = "it_802E27B4")]
pub fn walk(r: &mut Running, gobj: GObjId) {
    stop(r, gobj);
    set(r, gobj, off::FRAME, 0);
    set_coll_facing(r, gobj);
    hitbox::end_blink(r.game, gobj);
    if zako::bytes(r.game, gobj).i16(off::MAKES_FREEZIE) != 0 {
        let ip = item(r.game, gobj);
        let (mut pos, facing) = (ip.pos, ip.facing_dir);
        pos.x = mem(fp::fmadds(
            reg(attr_f32(r, gobj, 0x14)),
            reg(facing),
            reg(pos.x),
        ));
        let decel = attr_f32(r, gobj, 0x24);
        let f = super::freeze::spawn_pushed(r, gobj, pos, facing, decel);
        zako::bytes(r.game, gobj).set_gobj(off::FREEZIE, f);
        if let Some(f) = f {
            let v = fp::fmuls(
                reg(item(r.game, gobj).x40_vel.x),
                reg(attr_f32(r, gobj, 0x1C)),
            );
            super::freeze::hand_speed(r, f, mem(v));
        }
        zako::bytes(r.game, gobj).set_i16(off::MAKES_FREEZIE, 0);
    }
    if freezie(r, gobj).is_some() {
        change_state(r, gobj, 2, change::ANIM_UPDATE);
    } else {
        change_state(r, gobj, 1, change::ANIM_UPDATE);
    }
}

/// Walking's animation, `itOldottosea_UnkMotion2_Anim`: after its walk's frames (80, 42 in
/// Japanese) one draw in `x10` charges, else it stops and walks again.
#[doc(alias = "itOldottosea_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    let threshold = if is_jp(r) { 0x2A } else { 0x50 };
    if get(r, gobj, off::FRAME) < threshold {
        return false;
    }
    let odds = i32::from(r.data.u8(attr(r, gobj, 0x10)) as i8);
    if r.game.objs.rng.below(odds) == 0 {
        charge(r, gobj);
        return false;
    }
    item_mut(r.game, gobj).x40_vel.x = 0.0;
    set(r, gobj, off::TIMER, 0);
    set(r, gobj, off::FRAME, 0);
    let has = freezie(r, gobj).is_some();
    if is_us(r) {
        if !lib2725::is_animating(r.game, gobj) {
            change_state(r, gobj, if has { 2 } else { 1 }, change::ANIM_UPDATE);
        }
    } else if has {
        change_state(r, gobj, 2, change::ANIM_UPDATE);
    } else {
        change_state(r, gobj, 1, change::ANIM_UPDATE | change::HIT_PRESERVE);
    }
    false
}

/// Walking's physics, `itOldottosea_UnkMotion2_Phys`: its speed its way on its first frame (the
/// tenth in Japanese), then at the script's turn point (without a Freezie) it turns; along the
/// floor.
#[doc(alias = "itOldottosea_UnkMotion2_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId) {
    let start = if is_jp(r) { 10 } else { 0 };
    if get(r, gobj, off::FRAME) == start {
        let s = reg(walk_speed(r, gobj));
        let ip = item_mut(r.game, gobj);
        ip.x40_vel.x = mem(fp::fmuls(reg(ip.facing_dir), s));
    }
    let n = get(r, gobj, off::FRAME).wrapping_add(1);
    set(r, gobj, off::FRAME, n);
    if item(r.game, gobj).x40_vel.x != 0.0
        && freezie(r, gobj).is_none()
        && lib2725::xdc0_has(r.game, gobj, 2)
    {
        turn(r, gobj);
    }
    along_floor(r, gobj);
}

/// Along the floor, its model turned to it.
fn along_floor(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    let (normal, facing) = (ip.x378_itemColl.floor.normal, ip.facing_dir);
    zako::along_surface(&mut ip.x40_vel, normal, facing);
    zako::align_floor(r, gobj, facing, 5.0);
}

/// Walking's collision, `itOldottosea_UnkMotion2_Coll`: at a ledge it turns, and moving into a
/// wall; off the floor it falls; then the blast zones and its Freezie.
#[doc(alias = "itOldottosea_UnkMotion2_Coll")]
pub fn coll2(r: &mut Running, gobj: GObjId) -> bool {
    if ground::ground_to_ledge(r, gobj, turn) {
        if item(r.game, gobj).x40_vel.x != 0.0 && maplib::touched_wall(r.game, gobj) != 0 {
            turn(r, gobj);
        }
    } else {
        fall(r, gobj);
    }
    watch(r, gobj)
}

/// Falls, `it_802E2BC0`: its timer cleared, slowed by `x4`, state 4.
#[doc(alias = "it_802E2BC0")]
pub fn fall(r: &mut Running, gobj: GObjId) {
    set(r, gobj, off::TIMER, 0);
    let k = reg(attr_f32(r, gobj, 4));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(ip.x40_vel.x), k));
    change_state(r, gobj, 4, change::ANIM_UPDATE);
}

/// The states' animation callbacks that answer false (`itOldottosea_UnkMotion4_Anim`, `_11_Anim`).
#[doc(alias = "itOldottosea_UnkMotion4_Anim")]
#[doc(alias = "itOldottosea_UnkMotion11_Anim")]
pub fn answers_false(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states' physics that do nothing (`itOldottosea_UnkMotion3_Phys` and its like).
#[doc(alias = "itOldottosea_UnkMotion3_Phys")]
#[doc(alias = "itOldottosea_UnkMotion8_Phys")]
#[doc(alias = "itOldottosea_UnkMotion10_Phys")]
pub fn still(_: &mut Running, _: GObjId) {}

/// The falling states' physics (`itOldottosea_UnkMotion4_Phys`, `_9_Phys`): gravity.
#[doc(alias = "itOldottosea_UnkMotion4_Phys")]
#[doc(alias = "itOldottosea_UnkMotion9_Phys")]
pub fn phys_fall(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, data::attr::FALL_SPEED);
    let max = a.f32(r.data, data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
}

/// Falling's collision, `itOldottosea_UnkMotion4_Coll`: landing it stands; then the blast zones
/// and its Freezie.
#[doc(alias = "itOldottosea_UnkMotion4_Coll")]
pub fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    ground::fall_to_floor(r, gobj, stand);
    watch(r, gobj)
}

/// Knocked back, `it_802E2C80`: along the hit's knockback (in the air if it points off the
/// floor), facing as before the hit, state 5.
#[doc(alias = "it_802E2C80")]
pub fn knocked_back(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    let mut v = item(r.game, gobj).x40_vel;
    let up = zako::knockback_vel(r, gobj, &mut v);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel = v;
    if up {
        maplib::enter_air(ip);
    } else {
        maplib::enter_ground(ip);
    }
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = ip.init_facing_dir;
    change_state(r, gobj, 5, change::ANIM_UPDATE);
}

/// Knocked back's animation, `itOldottosea_UnkMotion5_Anim`: when it ends, in the air again, on
/// the ground [`landed_knocked`].
#[doc(alias = "itOldottosea_UnkMotion5_Anim")]
pub fn anim5(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        if item(r.game, gobj).ground_or_air == GA_AIR {
            change_state(r, gobj, 5, change::ANIM_UPDATE);
        } else {
            landed_knocked(r, gobj);
        }
    }
    false
}

/// Knocked back's physics, `itOldottosea_UnkMotion5_Phys`: in the air its fall speed off the
/// vertical velocity.
#[doc(alias = "itOldottosea_UnkMotion5_Phys")]
pub fn phys5(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).ground_or_air == GA_AIR {
        phys_gravity(r, gobj);
    }
}

/// Knocked back's collision, `itOldottosea_UnkMotion5_Coll`: in the air it lands
/// ([`landed_knocked`]), on the ground off the floor it falls; then the blast zones and its
/// Freezie.
#[doc(alias = "itOldottosea_UnkMotion5_Coll")]
pub fn coll5(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).ground_or_air == GA_AIR {
        ground::fall_until_touch_no_ceiling(r, gobj, landed_knocked);
    } else {
        ground::ground_or_fall(r, gobj, fall);
    }
    watch(r, gobj)
}

/// Down after a knockback, `it_802E2DF4`: hit from the way it faces it turns, else it walks.
#[doc(alias = "it_802E2DF4")]
pub fn landed_knocked(r: &mut Running, gobj: GObjId) {
    let ip = item(r.game, gobj);
    if ip.xCCC_incDamageDirection == ip.facing_dir {
        turn(r, gobj);
        return;
    }
    walk(r, gobj);
}

/// Charges, `it_802E2E30`: stopped, its count reset, state 7 with a Freezie, else 6.
#[doc(alias = "it_802E2E30")]
pub fn charge(r: &mut Running, gobj: GObjId) {
    stop(r, gobj);
    set(r, gobj, off::FRAME, 0);
    if freezie(r, gobj).is_some() {
        change_state(r, gobj, 7, change::ANIM_UPDATE);
    } else {
        change_state(r, gobj, 6, change::ANIM_UPDATE);
    }
}

/// Charging's animation, `itOldottosea_UnkMotion7_Anim`: when it ends, it stands (its timer
/// cleared).
#[doc(alias = "itOldottosea_UnkMotion7_Anim")]
pub fn anim7(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        set(r, gobj, off::TIMER, 0);
        stand(r, gobj);
    }
    false
}

/// Charging's physics, `itOldottosea_UnkMotion7_Phys`: at frame 25 it dashes at `x8` its way,
/// slowing by `xC` a frame after; at the script's turn point (without a Freezie) it turns;
/// along the floor.
#[doc(alias = "itOldottosea_UnkMotion7_Phys")]
pub fn phys7(r: &mut Running, gobj: GObjId) {
    let n = get(r, gobj, off::FRAME);
    if n == 0x19 {
        let s = reg(attr_f32(r, gobj, 8));
        let ip = item_mut(r.game, gobj);
        ip.x40_vel.x = mem(fp::fmuls(reg(ip.facing_dir), s));
    } else if n > 0x19 {
        let k = reg(attr_f32(r, gobj, 0xC));
        let ip = item_mut(r.game, gobj);
        ip.x40_vel.x = mem(fp::fmadds(
            fp::fneg(reg(ip.facing_dir)),
            k,
            reg(ip.x40_vel.x),
        ));
    }
    set(r, gobj, off::FRAME, n.wrapping_add(1));
    if item(r.game, gobj).x40_vel.x != 0.0
        && freezie(r, gobj).is_none()
        && lib2725::xdc0_has(r.game, gobj, 2)
    {
        turn(r, gobj);
    }
    along_floor(r, gobj);
}

/// Charging's collision, `itOldottosea_UnkMotion7_Coll`: off the floor it falls; moving into a
/// wall it turns; then the blast zones and its Freezie.
#[doc(alias = "itOldottosea_UnkMotion7_Coll")]
pub fn coll7(r: &mut Running, gobj: GObjId) -> bool {
    ground::ground_or_fall(r, gobj, fall);
    if item(r.game, gobj).x40_vel.x != 0.0 && maplib::touched_wall(r.game, gobj) != 0 {
        turn(r, gobj);
    }
    watch(r, gobj)
}

/// Turns, `it_802E3098`: stopped for 40 frames (55 in Japanese), state 3, kept clear of the
/// stage's pushes (`it_8026BDB4`).
#[doc(alias = "it_802E3098")]
pub fn turn(r: &mut Running, gobj: GObjId) {
    stop(r, gobj);
    let t = if is_jp(r) { 0x37 } else { 0x28 };
    set(r, gobj, off::TIMER, t);
    change_state(r, gobj, 3, change::ANIM_UPDATE);
    lib26b1::stop_keeping_clear(r.game, gobj);
}

/// Turning's animation, `itOldottosea_UnkMotion3_Anim`: at its timer's end it faces the other
/// way; when the animation ends it walks again.
#[doc(alias = "itOldottosea_UnkMotion3_Anim")]
pub fn anim3(r: &mut Running, gobj: GObjId) -> bool {
    if get(r, gobj, off::TIMER) == 0 {
        let ip = item_mut(r.game, gobj);
        ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
        set_coll_facing(r, gobj);
    }
    let t = get(r, gobj, off::TIMER).wrapping_sub(1);
    set(r, gobj, off::TIMER, t);
    if !lib2725::is_animating(r.game, gobj) {
        set(r, gobj, off::TIMER, 0);
        lib26b1::keep_clear(r.game, gobj);
        walk(r, gobj);
    }
    false
}

/// Picked up, `it_802E31F8`: its Freezie let go, state 8.
#[doc(alias = "it_802E31F8")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    let_go(r, gobj);
    change_state(r, gobj, 8, change::ANIM_UPDATE);
}

/// Held's animation, `itOldottosea_UnkMotion8_Anim`: looped as when picked up.
#[doc(alias = "itOldottosea_UnkMotion8_Anim")]
pub fn anim8(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        picked_up(r, gobj);
    }
    false
}

/// Dropped, `it_802E32B4`: the enemies' drop, its timer cleared, slowed by `x4`, falling.
// Ports `it_802E32B4`; aliased once a check reaches it (no oracle match has a fighter pick up a
// monster).
pub fn dropped(r: &mut Running, gobj: GObjId) {
    zako::dropped(r, gobj);
    fall(r, gobj);
}

/// Thrown, `it_802E3314`: the enemies' throw, its timer cleared, state 9.
// Ports `it_802E3314`; aliased once a check reaches it (no oracle match has a fighter pick up a
// monster).
pub fn thrown(r: &mut Running, gobj: GObjId) {
    zako::thrown(r, gobj);
    set(r, gobj, off::TIMER, 0);
    change_state(r, gobj, 9, change::ANIM_UPDATE);
}

/// Thrown's animation, `itOldottosea_UnkMotion9_Anim`: looped.
#[doc(alias = "itOldottosea_UnkMotion9_Anim")]
pub fn anim9(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 9, change::ANIM_UPDATE);
    }
    false
}

/// Thrown's collision, `itOldottosea_UnkMotion9_Coll`: touching the stage ends the flight as a
/// KO.
#[doc(alias = "itOldottosea_UnkMotion9_Coll")]
pub fn coll9(r: &mut Running, gobj: GObjId) -> bool {
    zako::flight_lands(r, gobj, None)
}

/// Flung away's collision, `itOldottosea_UnkMotion10_Coll`: bounces off the stage.
#[doc(alias = "itOldottosea_UnkMotion10_Coll")]
pub fn coll10(r: &mut Running, gobj: GObjId) -> bool {
    zako::bounce(r, gobj)
}

/// Flung away, `it_802E3400`: gone past the blast zones from now (`xDCC_flag.b3`), in the air,
/// launched along its knockback, `x18` cleared, state 10.
#[doc(alias = "it_802E3400")]
pub fn fling(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.xDCC_flag.b3 = true;
    maplib::enter_air(ip);
    zako::launch(r, gobj, false);
    set(r, gobj, off::X18, 0);
    change_state(r, gobj, 10, change::ANIM_UPDATE);
}

/// Flung away's animation, `itOldottosea_UnkMotion10_Anim`: looped.
#[doc(alias = "itOldottosea_UnkMotion10_Anim")]
pub fn anim10(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 10, change::ANIM_UPDATE);
    }
    false
}

/// Its destroy callback, `it_2725_Logic3_Destroyed`: its Freezie let go and forgotten
/// (`it_802E37A4`), then the enemies' destroy.
#[doc(alias = "it_2725_Logic3_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    if let Some(f) = freezie(r, gobj) {
        super::freeze::let_go(r, f);
    }
    forget_freezie(r, gobj);
    zako::destroyed(r, gobj);
}

/// Flung toward the screen, `it_802E3528` (the inline `Item_UpdateZakoVelocity`): state 11.
#[doc(alias = "it_802E3528")]
pub fn fling_to_screen(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    let v = zako::toward_camera(r, gobj);
    item_mut(r.game, gobj).x40_vel = v;
    maplib::enter_air(item_mut(r.game, gobj));
    change_state(r, gobj, 11, UNK_0X1 | change::ANIM_UPDATE);
}

/// Flung toward the screen's physics, `itOldottosea_UnkMotion11_Phys` (and the knockback's in
/// the air): its fall speed off the vertical velocity.
#[doc(alias = "itOldottosea_UnkMotion11_Phys")]
pub fn phys_gravity(r: &mut Running, gobj: GObjId) {
    let accel = reg(attr_of(item(r.game, gobj)).f32(r.data, data::attr::FALL_SPEED));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = mem(fp::fsubs(reg(ip.x40_vel.y), accel));
}

/// The collisions' end, `it_802E35CC` (and flung toward the screen's collision,
/// `itOldottosea_UnkMotion11_Coll`): past a blast zone it's gone (its Freezie let go), true;
/// with a Freezie, it lets go of one farther than `x18` and hands one within `x14` its speed
/// (scaled by `x20` charging, else `x1C`).
#[doc(alias = "it_802E35CC")]
#[doc(alias = "itOldottosea_UnkMotion11_Coll")]
pub fn watch(r: &mut Running, gobj: GObjId) -> bool {
    let pos = item(r.game, gobj).pos;
    let (x, y) = (reg(pos.x), reg(pos.y));
    if x > reg(stage::blast_right(r.game)) {
        item_mut(r.game, gobj).xDCC_flag.b3 = true;
    }
    if x < reg(stage::blast_left(r.game)) {
        item_mut(r.game, gobj).xDCC_flag.b3 = true;
    }
    if y > reg(stage::blast_top(r.game)) {
        item_mut(r.game, gobj).xDCC_flag.b3 = true;
    }
    if y < reg(stage::blast_bottom(r.game)) {
        item_mut(r.game, gobj).xDCC_flag.b3 = true;
    }
    if item(r.game, gobj).xDCC_flag.b3 {
        if let Some(f) = freezie(r, gobj) {
            super::freeze::let_go(r, f);
        }
        zako::destroyed(r, gobj);
        return true;
    }
    if let Some(f) = freezie(r, gobj) {
        let at = super::freeze::position(r, f);
        let pos = item(r.game, gobj).pos;
        let dx = fp::fsubs(reg(pos.x), reg(at.x));
        let dy = fp::fsubs(reg(pos.y), reg(at.y));
        let dz = fp::fsubs(reg(pos.z), reg(at.z));
        let sq = fp::fmadds(dz, dz, fp::fmadds(dx, dx, fp::fmuls(dy, dy)));
        let dist = reg(crate::math::sqrt(mem(sq)));
        if dist > reg(attr_f32(r, gobj, 0x18)) {
            super::freeze::let_go(r, f);
            forget_freezie(r, gobj);
        } else if dist <= reg(attr_f32(r, gobj, 0x14)) {
            let k = if item(r.game, gobj).msid == 7 {
                0x20
            } else {
                0x1C
            };
            let v = fp::fmuls(reg(item(r.game, gobj).x40_vel.x), reg(attr_f32(r, gobj, k)));
            super::freeze::hand_speed(r, f, mem(v));
        }
    }
    false
}

/// Forgets the Freezie it pushed, `it_802E37A4` (the word at `x20`, which a Freezie's own
/// variables call `freeze.x20`).
#[doc(alias = "it_802E37A4")]
pub fn forget_freezie(r: &mut Running, gobj: GObjId) {
    zako::bytes(r.game, gobj).set_gobj(off::FREEZIE, None);
}

/// Forgets a GObj going away, `it_802E3784` (`it_8026B894`).
#[doc(alias = "it_802E3784")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    let _ = lib26b1::forget(r.game, gobj, other);
}
