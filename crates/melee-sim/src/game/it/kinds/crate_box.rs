//! The box (crate), `melee/it/kinds/itbox.c`: a container that breaks open when thrown hard
//! enough, hit for enough damage or when it hits something, dropping items or nothing. Kongo
//! Jungle's barrels carry floating ones (`it_80286088`).

use gekko_fp as fp;

use super::super::ground::{air, fall, ground_or_fall, ground_skid};
use super::super::item::{article, attr_of, change, change_state, common_data, play_sfx};
use super::super::{drop, effect, hitbox, item, item_mut, jobj_of, lib26b1, lib2725, maplib};
use crate::fpu::{mem, reg};
use crate::game::code;
use crate::game::handles::CodeRef;
use crate::game::state::{Game, Running};
use crate::game::structs::{SpawnItem, itBox_ItemVars};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x8028_61CC;
pub const DESTROYED: u32 = 0x8028_6208;
pub const LANDED: u32 = 0x8028_6480;
pub const ANIM0: u32 = 0x8028_64D8;
pub const PHYS0: u32 = 0x8028_6518;
pub const COLL0: u32 = 0x8028_651C;
pub const FELL: u32 = 0x8028_655C;
pub const ANIM4: u32 = 0x8028_6598;
pub const PHYS1: u32 = 0x8028_65A0;
pub const COLL1: u32 = 0x8028_6754;
pub const PICKED_UP: u32 = 0x8028_6780;
pub const ANIM2: u32 = 0x8028_67A8;
pub const PHYS2: u32 = 0x8028_67B0;
pub const THROWN: u32 = 0x8028_67B4;
pub const PHYS4: u32 = 0x8028_67F0;
pub const COLL3: u32 = 0x8028_683C;
pub const DROPPED: u32 = 0x8028_6964;
pub const COLL4: u32 = 0x8028_69A0;
pub const ANIM6: u32 = 0x8028_6B74;
pub const PHYS6: u32 = 0x8028_6B94;
pub const COLL6: u32 = 0x8028_6B98;
pub const ANIM7: u32 = 0x8028_6C7C;
pub const PHYS7: u32 = 0x8028_6CA8;
pub const COLL7: u32 = 0x8028_6CAC;
pub const DMG_DEALT: u32 = 0x8028_6CB4;
pub const CLANKED: u32 = 0x8028_6D48;
pub const HIT_SHIELD: u32 = 0x8028_6DDC;
pub const REFLECTED: u32 = 0x8028_6E70;
pub const DMG_RECEIVED: u32 = 0x8028_6F04;
pub const ENTERED_AIR: u32 = 0x8028_6FC8;
pub const ANIM5: u32 = 0x8028_7044;
pub const PHYS5: u32 = 0x8028_704C;
pub const COLL5: u32 = 0x8028_7050;
pub const ANIM8: u32 = 0x8028_7104;
pub const PHYS8: u32 = 0x8028_710C;
pub const COLL8: u32 = 0x8028_7334;
pub const FOLLOW: u32 = 0x8028_733C;

/// Runs one of the box's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 => anim0(r, gobj),
        COLL0 => coll0(r, gobj),
        ANIM4 | ANIM2 | ANIM5 | ANIM8 | COLL6 | COLL7 | COLL8 => no(r, gobj),
        COLL1 => coll1(r, gobj),
        COLL3 => coll3(r, gobj),
        COLL4 => coll4(r, gobj),
        ANIM6 => anim6(r, gobj),
        ANIM7 => anim7(r, gobj),
        DMG_DEALT | CLANKED | HIT_SHIELD | REFLECTED => hit(r, gobj),
        DMG_RECEIVED => dmg_received(r, gobj),
        COLL5 => coll5(r, gobj),
        _ => return None,
    })
}

/// Runs one of the box's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        DESTROYED => destroyed(r, gobj),
        LANDED => landed(r, gobj),
        PHYS0 | PHYS2 | PHYS6 | PHYS7 | PHYS5 => idle(r, gobj),
        FELL => fell(r, gobj),
        PHYS1 => phys1(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        THROWN => thrown(r, gobj),
        PHYS4 => phys4(r, gobj),
        DROPPED => dropped(r, gobj),
        ENTERED_AIR => entered_air(r, gobj),
        PHYS8 => phys8(r, gobj),
        FOLLOW => follow(r, gobj),
        _ => return false,
    }
    true
}

/// The box's variables.
fn vars(game: &mut Game, gobj: GObjId) -> &mut itBox_ItemVars {
    item_mut(game, gobj).xDD4_itemVar.crate_box()
}

/// `itBoxAttributes`' word at `off`.
fn attr(r: &Running, gobj: GObjId, off: u32) -> melee_data::archive::DataRef {
    article(item(r.game, gobj))
        .special(r.data)
        .expect("a box without attributes")
        .at(off)
}

fn attr_i(r: &Running, gobj: GObjId, off: u32) -> i32 {
    r.data.u32(attr(r, gobj, off)) as i32
}

fn attr_f(r: &Running, gobj: GObjId, off: u32) -> f32 {
    r.data.f32(attr(r, gobj, off))
}

/// Makes a box floating with a barrel (`parent`), `it_80286088`: it spins at a random rate
/// and follows the barrel.
#[doc(alias = "it_80286088")]
pub fn spawn_floating(r: &mut Running, parent: Option<GObjId>) -> Option<GObjId> {
    let pj = parent.and_then(|p| jobj_of(r.game, p))?;
    let pos = crate::lb::joint::world_position(&mut r.game.objs, r.data, Some(pj), None);
    let mut spawn = SpawnItem {
        kind: super::BOX,
        ..Default::default()
    };
    // `Item_InitSpawnPosition`.
    spawn.prev_pos = pos;
    spawn.pos = pos;
    spawn.facing_dir = 1.0;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = None;
    spawn.x4_parent_gobj2 = None;
    spawn.x44_flag[0] &= !0x80;
    spawn.x40 = 0;
    let gobj = super::super::item::spawn_air(r, &mut spawn)?;
    vars(r.game, gobj).spawned_gobj.set(parent);
    let scale = 0.034_906_584_769_487_38;
    for i in 0..2 {
        let f = r.game.objs.rng.draw_f32();
        let v = mem(fp::frsp(fp::fmul(fp::fsubs(reg(f), 0.5), scale)));
        if i == 0 {
            vars(r.game, gobj).rot_vel_x.set(v);
        } else {
            vars(r.game, gobj).rot_vel_y.set(v);
        }
    }
    item_mut(r.game, gobj).xDC8_word.flags.x19 = false;
    lib26b1::stop_keeping_clear(r.game, gobj);
    float(r, gobj);
    Some(gobj)
}

/// `itBox_Logic1_Spawned`.
#[doc(alias = "itBox_Logic1_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    super::super::set_flag(&mut item_mut(r.game, gobj).xDCE_flag, 7, false);
    let v = vars(r.game, gobj);
    v.opened.set(0);
    v.spawned_gobj.set(None);
    fell(r, gobj);
}

/// `itBox_Logic1_Destroyed`: a barrel's box lets go of it.
#[doc(alias = "itBox_Logic1_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    if vars(r.game, gobj).spawned_gobj.get().is_some() {
        code::unported_call("grKongo_801D8058 (a Kongo Jungle barrel's box gone)");
    }
}

/// Drops what's in the box, `it_80286248`: coins in a coin match, else a monster if one comes
/// out, else 1, 2 or 3 items by the weights (all one kind on a 1 in `special` draw).
#[doc(alias = "it_80286248")]
pub fn drop_items(r: &mut Running, gobj: GObjId, w1: i32, w2: i32, w3: i32, special: i32) {
    let pos = item(r.game, gobj).pos;
    if drop::drop_coins(r, gobj, pos, None) {
        return;
    }
    if drop::drop_monster(r, Some(gobj), pos, None).is_some() {
        return;
    }
    let same = r.game.objs.rng.below(special) == 0;
    let sum = w1.wrapping_add(w2);
    let roll = r.game.objs.rng.below(w3.wrapping_add(sum));
    let n = if roll < w1 {
        1
    } else if roll < sum {
        2
    } else {
        3
    };
    drop::drop_contents(r, gobj, n, same);
}

/// Whether a box breaks with items in it, `it_80286340`: a draw over the weights and the
/// empty weight lands below the three item weights.
#[doc(alias = "it_80286340")]
pub fn has_items(r: &mut Running, w1: i32, w2: i32, w3: i32, empty: i32) -> bool {
    let sum1 = w1.wrapping_add(w2);
    let sum2 = w3.wrapping_add(sum1);
    let roll = r.game.objs.rng.below(empty.wrapping_add(sum2));
    roll < w1 || roll < sum1 || roll < sum2
}

/// Whether it landed on a floor near enough upright to stop, `it_802863BC`, stopping its
/// tilting if so.
#[doc(alias = "it_802863BC")]
pub fn lands_upright(r: &mut Running, gobj: GObjId) -> bool {
    let limit = attr_f(r, gobj, 0x18);
    let ip = item_mut(r.game, gobj);
    if ip.x378_itemColl.env_flags & (crate::mp::coll::FLOOR_PUSH | crate::mp::coll::FLOOR_HUG) == 0
    {
        return false;
    }
    let dir = maplib::surface_normal(&ip.x378_itemColl).expect("a floor's normal");
    if reg(dir.angle_xy(Vec3::new(0.0, 1.0, 0.0))) < reg(limit) {
        let z = Vec3::new(0.0, 0.0, 0.0);
        ip.x7C = z;
        ip.x88 = z;
        ip.x94 = z;
        ip.xA0 = z;
        ip.xD5C = 1;
        return true;
    }
    false
}

/// Lands, stopped, `fn_80286480`.
#[doc(alias = "fn_80286480")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    lib26b1::enable_pickup(r.game, gobj);
    item_mut(r.game, gobj).x40_vel = Vec3::new(0.0, 0.0, 0.0);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// `itBox_UnkMotion0_Anim`: smokes as its time runs out.
#[doc(alias = "itBox_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    if reg(item(r.game, gobj).xD44_lifeTimer) <= 1.0 {
        effect::spawn_simple(r, gobj, 0x421);
    }
    false
}

/// The callbacks that do nothing and answer false.
#[doc(alias = "itBox_UnkMotion4_Anim")]
#[doc(alias = "itBox_UnkMotion2_Anim")]
#[doc(alias = "itBox_UnkMotion5_Anim")]
#[doc(alias = "itBox_UnkMotion8_Anim")]
#[doc(alias = "itBox_UnkMotion6_Coll")]
#[doc(alias = "itBox_UnkMotion7_Coll")]
#[doc(alias = "itBox_UnkMotion8_Coll")]
pub fn no(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states that move nothing.
#[doc(alias = "itBox_UnkMotion0_Phys")]
#[doc(alias = "itBox_UnkMotion2_Phys")]
#[doc(alias = "itBox_UnkMotion6_Phys")]
#[doc(alias = "itBox_UnkMotion7_Phys")]
#[doc(alias = "itBox_UnkMotion5_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// `itBox_UnkMotion0_Coll`.
#[doc(alias = "itBox_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    maplib::tilt_on_floor(r.game, r.data, gobj);
    false
}

/// Falls (state 1), `it_8028655C`.
#[doc(alias = "it_8028655C")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    lib26b1::disable_pickup(r.game, gobj);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// Wraps a rotation past a turn either way.
fn wrap_turn(v: f32) -> f32 {
    let tau = std::f64::consts::TAU;
    if reg(v) > tau {
        mem(fp::frsp(fp::fsub(reg(v), tau)))
    } else if reg(v) < -tau {
        mem(fp::frsp(fp::fadd(reg(v), tau)))
    } else {
        v
    }
}

/// Turns the model by the spin rates about x and y.
fn spin(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj).expect("a box without a model");
    let (vx, vy) = {
        let v = vars(r.game, gobj);
        (v.rot_vel_x.get(), v.rot_vel_y.get())
    };
    let mut q = r.game.objs.jobjs[j].rotate;
    q.x = wrap_turn(mem(fp::fadds(reg(q.x), reg(vx))));
    q.y = wrap_turn(mem(fp::fadds(reg(q.y), reg(vy))));
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.rotate = q);
}

/// `itBox_UnkMotion1_Phys`: gravity, and a barrel's box tumbles.
#[doc(alias = "itBox_UnkMotion1_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, super::super::data::attr::FALL_SPEED);
    let max = a.f32(r.data, super::super::data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
    if vars(r.game, gobj).spawned_gobj.get().is_some() {
        let ip = item_mut(r.game, gobj);
        ip.x40_vel.z = mem(fp::fmuls(reg(ip.x40_vel.z), f64::from(0.95_f32)));
        spin(r, gobj);
    }
}

/// `itBox_UnkMotion1_Coll`.
#[doc(alias = "itBox_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, landed);
    false
}

/// `itBox_Logic1_PickedUp`.
#[doc(alias = "itBox_Logic1_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// `itBox_Logic1_Thrown`.
#[doc(alias = "itBox_Logic1_Thrown")]
pub fn thrown(r: &mut Running, gobj: GObjId) {
    lib26b1::disable_pickup(r.game, gobj);
    change_state(r, gobj, 3, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// Gravity and spin, `itBox_UnkMotion4_Phys` (`Item_ApplyFallingPhysics`).
#[doc(alias = "itBox_UnkMotion4_Phys")]
pub fn phys4(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, super::super::data::attr::FALL_SPEED);
    let max = a.f32(r.data, super::super::data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
    let x68 = common_data(r.game).f32(r.data, super::super::data::common::X68);
    lib2725::clamp_fall(r.game, gobj, x68);
}

/// Breaks: its splinters (effect 0x427), then opens with items or empty (the file's inline
/// `itBox_TryOpen_inline`).
fn break_open(r: &mut Running, gobj: GObjId) {
    let pos = item(r.game, gobj).pos;
    crate::game::ef::spawn(r, 0x427, Some(gobj), &[crate::game::ef::Arg::Vec(pos)]);
    let w: [i32; 4] = std::array::from_fn(|i| attr_i(r, gobj, 4 * i as u32));
    if has_items(r, w[0], w[1], w[2], w[3]) {
        open(r, gobj);
    } else {
        open_empty(r, gobj);
    }
}

/// `itBox_UnkMotion3_Coll`: thrown, it breaks on what it hits, and stops rising at ceilings.
#[doc(alias = "itBox_UnkMotion3_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    if air(r, gobj) {
        break_open(r, gobj);
    } else if maplib::touched_ceiling(r.game, gobj) != 0 {
        let ip = item_mut(r.game, gobj);
        if reg(ip.x40_vel.y) > 0.0 {
            ip.x40_vel.y = 0.0;
        }
    } else if maplib::touched_wall(r.game, gobj) != 0 {
        break_open(r, gobj);
    }
    false
}

/// `itBox_Logic1_Dropped`.
#[doc(alias = "itBox_Logic1_Dropped")]
pub fn dropped(r: &mut Running, gobj: GObjId) {
    lib26b1::disable_pickup(r.game, gobj);
    change_state(r, gobj, 4, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// `itBox_UnkMotion4_Coll`: dropped, it breaks on a hard landing, else stops.
#[doc(alias = "itBox_UnkMotion4_Coll")]
pub fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    let limit = attr_f(r, gobj, 0x1C);
    if air(r, gobj) {
        if reg(item(r.game, gobj).x40_vel.y).abs() > reg(limit) {
            break_open(r, gobj);
        } else {
            maplib::enter_ground(item_mut(r.game, gobj));
            lib26b1::enable_pickup(r.game, gobj);
            item_mut(r.game, gobj).x40_vel = Vec3::new(0.0, 0.0, 0.0);
            change_state(r, gobj, 0, change::ANIM_UPDATE);
        }
    }
    false
}

/// Breaks open empty, `it_80286AA4`: hidden, it blasts a moment, pushing the wind.
#[doc(alias = "it_80286AA4")]
pub fn open_empty(r: &mut Running, gobj: GObjId) {
    lib26b1::hide(r.game, gobj);
    lib2725::blast_effect(r, gobj);
    hitbox::start_blink(r.game, gobj);
    lib26b1::disable_pickup(r.game, gobj);
    lib26b1::set_xdd0_b3(r.game, gobj);
    lib2725::start_vanishing(r.game, r.data, gobj);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = 0.0;
    ip.x40_vel.y = 0.0;
    super::super::set_flag(&mut ip.xDCF_flag, 2, true);
    let v = vars(r.game, gobj);
    v.opened.set(1);
    v.despawn_timer.set(40);
    hitbox::set_hit_ids(r.game, gobj);
    let pos = item(r.game, gobj).pos;
    crate::lb::wind::blast(
        &mut r.game.gr.winds,
        pos,
        0x78,
        1.0,
        0.02,
        std::f32::consts::FRAC_PI_3,
    );
    change_state(r, gobj, 6, 2);
}

/// `itBox_UnkMotion6_Anim`: the blast lasts its lifetime.
#[doc(alias = "itBox_UnkMotion6_Anim")]
pub fn anim6(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::tick_life(r.game, gobj)
}

/// Breaks open with items, `it_80286BA0`: a crash, a small quake, the items, and gone.
#[doc(alias = "it_80286BA0")]
pub fn open(r: &mut Running, gobj: GObjId) {
    play_sfx(r, gobj, 0xF6, 0x7F, 0x40);
    let pos = item(r.game, gobj).pos;
    crate::game::camera::request_quake(r, 2, Some(pos));
    let w = [0, 4, 8, 0x10].map(|o| attr_i(r, gobj, o));
    drop_items(r, gobj, w[0], w[1], w[2], w[3]);
    let j = jobj_of(r.game, gobj);
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    hitbox::start_blink(r.game, gobj);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = 0.0;
    ip.x40_vel.y = 0.0;
    super::super::set_flag(&mut ip.xDCF_flag, 2, true);
    let v = vars(r.game, gobj);
    v.opened.set(1);
    v.despawn_timer.set(40);
    lib26b1::disable_pickup(r.game, gobj);
    change_state(r, gobj, 7, 2);
}

/// `itBox_UnkMotion7_Anim`: gone after 40 frames.
#[doc(alias = "itBox_UnkMotion7_Anim")]
pub fn anim7(r: &mut Running, gobj: GObjId) -> bool {
    let v = vars(r.game, gobj);
    v.despawn_timer.update(|old| old.wrapping_sub(1));
    v.despawn_timer.get() <= 0
}

/// Hitting something (or being reflected), an unopened box breaks, `itBox_Logic1_DmgDealt`
/// and the rest.
#[doc(alias = "itBox_Logic1_DmgDealt")]
#[doc(alias = "itBox_Logic1_Clanked")]
#[doc(alias = "itBox_Logic1_HitShield")]
#[doc(alias = "itBox_Logic1_Reflected")]
pub fn hit(r: &mut Running, gobj: GObjId) -> bool {
    if vars(r.game, gobj).opened.get() == 0 {
        break_open(r, gobj);
    }
    false
}

/// `itBox_Logic1_DmgReceived`: breaks once its damage reaches the threshold.
#[doc(alias = "itBox_Logic1_DmgReceived")]
pub fn dmg_received(r: &mut Running, gobj: GObjId) -> bool {
    let limit = attr_f(r, gobj, 0x14);
    if vars(r.game, gobj).opened.get() == 0
        && fp::frsp(f64::from(item(r.game, gobj).xC9C)) >= reg(limit)
    {
        break_open(r, gobj);
    }
    false
}

/// `itBox_Logic1_EnteredAir`: stops if it lands upright, else skids (state 5).
#[doc(alias = "itBox_Logic1_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    if lands_upright(r, gobj) {
        lib26b1::enable_pickup(r.game, gobj);
        item_mut(r.game, gobj).x40_vel = Vec3::new(0.0, 0.0, 0.0);
        change_state(r, gobj, 0, 2);
    } else {
        change_state(r, gobj, 5, 2);
    }
}

/// `itBox_UnkMotion5_Coll`.
#[doc(alias = "itBox_UnkMotion5_Coll")]
pub fn coll5(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, landed, fell);
    false
}

/// Forgets a GObj the box points to, `itBox_Logic1_EvtUnk` (`it_8026B894`).
#[doc(alias = "itBox_Logic1_EvtUnk")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// Floats with its barrel (state 8), `it_802870A4`.
#[doc(alias = "it_802870A4")]
pub fn float(r: &mut Running, gobj: GObjId) {
    maplib::enter_ground(item_mut(r.game, gobj));
    lib26b1::disable_pickup(r.game, gobj);
    change_state(r, gobj, 8, change::ANIM_UPDATE);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(FOLLOW));
}

/// Nudges a spin rate at random, capped at 3 degrees a frame.
fn nudge(r: &mut Running, v: f32) -> f32 {
    let deg = 0.017_453_292_384_743_69;
    let max = 0.052_359_877_154_231_07;
    let f = r.game.objs.rng.draw_f32();
    let n = mem(fp::frsp(fp::fmadd(
        deg,
        fp::fsubs(reg(f), f64::from(0.2_f32)),
        reg(v),
    )));
    if reg(n) > max {
        mem(fp::frsp(max))
    } else if reg(n) < -max {
        mem(fp::frsp(-max))
    } else {
        n
    }
}

/// `itBox_UnkMotion8_Phys`: floating, it tumbles at randomly changing rates.
#[doc(alias = "itBox_UnkMotion8_Phys")]
pub fn phys8(r: &mut Running, gobj: GObjId) {
    let vx = vars(r.game, gobj).rot_vel_x.get();
    let vx = nudge(r, vx);
    vars(r.game, gobj).rot_vel_x.set(vx);
    let vy = vars(r.game, gobj).rot_vel_y.get();
    let vy = nudge(r, vy);
    vars(r.game, gobj).rot_vel_y.set(vy);
    spin(r, gobj);
}

/// Follows its barrel, `it_8028733C`, falling once the barrel comes out in front (or is gone).
#[doc(alias = "it_8028733C")]
// NaN keeps following, as the original's `>= 0` test has it.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn follow(r: &mut Running, gobj: GObjId) {
    let parent = vars(r.game, gobj).spawned_gobj.get();
    if let Some(p) = parent {
        let j = jobj_of(r.game, p).expect("a barrel without a model");
        let target = r.game.objs.jobjs[j].translate;
        let ip = item_mut(r.game, gobj);
        let vel = Vec3::new(
            mem(fp::fsubs(reg(target.x), reg(ip.pos.x))),
            mem(fp::fsubs(reg(target.y), reg(ip.pos.y))),
            mem(fp::fsubs(reg(target.z), reg(ip.pos.z))),
        );
        ip.pos = target;
        if !(reg(target.z) >= 0.0) {
            return;
        }
        ip.x40_vel = vel;
    }
    let ip = item_mut(r.game, gobj);
    let pos = ip.pos;
    ip.x378_itemColl.set_position(pos);
    lib26b1::disable_pickup(r.game, gobj);
    change_state(r, gobj, 1, 2);
}
