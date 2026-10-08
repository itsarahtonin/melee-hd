//! The party ball, `melee/it/kinds/itkusudama.c`: when hit, thrown hard or hit for enough
//! damage it floats up, swings open and rains items (a kind in a set number, food, or random
//! ones), or bursts empty.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use gekko_fp as fp;

use super::super::ground::{air, fall, ground_or_fall};
use super::super::item::{article, attr_of, change, change_state, common_data, play_sfx};
use super::super::{drop, hitbox, item, item_mut, jobj_of, lib26b1, lib2725, maplib, spawn};
use crate::fpu::{mem, reg};
use crate::game::code;
use crate::game::state::{Game, Running};
use crate::game::structs::itKusudama_ItemVars;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x8028_9784;
pub const STOPPED: u32 = 0x8028_A190;
pub const ANIM0: u32 = 0x8028_A298;
pub const PHYS0: u32 = 0x8028_A2C0;
pub const COLL0: u32 = 0x8028_A2C4;
pub const ANIM1: u32 = 0x8028_A2F0;
pub const PHYS1: u32 = 0x8028_A3A4;
pub const COLL1: u32 = 0x8028_A3A8;
pub const FELL: u32 = 0x8028_A3CC;
pub const ANIM2: u32 = 0x8028_A4C0;
pub const PHYS2: u32 = 0x8028_A4E8;
pub const COLL2: u32 = 0x8028_A518;
pub const RISE: u32 = 0x8028_A544;
pub const ANIM3: u32 = 0x8028_A690;
pub const PHYS3: u32 = 0x8028_A7F0;
pub const COLL3: u32 = 0x8028_A7F4;
pub const PICKED_UP: u32 = 0x8028_A93C;
pub const ANIM4: u32 = 0x8028_A964;
pub const PHYS4: u32 = 0x8028_A96C;
pub const THROWN: u32 = 0x8028_A970;
pub const ANIM6: u32 = 0x8028_A9AC;
pub const PHYS6: u32 = 0x8028_A9B4;
pub const COLL5: u32 = 0x8028_AA00;
pub const DROPPED: u32 = 0x8028_AB28;
pub const COLL6: u32 = 0x8028_AB64;
pub const BURST: u32 = 0x8028_AC74;
pub const ANIM7: u32 = 0x8028_AD18;
pub const PHYS7: u32 = 0x8028_AD38;
pub const COLL7: u32 = 0x8028_AD3C;
pub const OPEN: u32 = 0x8028_AD44;
pub const ANIM8: u32 = 0x8028_ADD4;
pub const PHYS8: u32 = 0x8028_AE98;
pub const COLL8: u32 = 0x8028_AE9C;
pub const DMG_DEALT: u32 = 0x8028_AEA4;
pub const CLANKED: u32 = 0x8028_AF1C;
pub const HIT_SHIELD: u32 = 0x8028_AF54;
pub const REFLECTED: u32 = 0x8028_AF8C;
pub const DMG_RECEIVED: u32 = 0x8028_AFC4;

/// Runs one of the party ball's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM2 => upright(r, gobj),
        COLL0 => coll0(r, gobj),
        ANIM1 => anim1(r, gobj),
        COLL1 => coll1(r, gobj),
        COLL2 => coll2(r, gobj),
        ANIM3 => anim3(r, gobj),
        COLL3 => coll3(r, gobj),
        ANIM4 | ANIM6 | COLL7 | COLL8 => no(r, gobj),
        COLL5 => coll5(r, gobj),
        COLL6 => coll6(r, gobj),
        ANIM7 => anim7(r, gobj),
        ANIM8 => anim8(r, gobj),
        DMG_DEALT => dmg_dealt(r, gobj),
        CLANKED | HIT_SHIELD | REFLECTED => clanked(r, gobj),
        DMG_RECEIVED => dmg_received(r, gobj),
        _ => return None,
    })
}

/// Runs one of the party ball's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        STOPPED => stopped(r, gobj),
        PHYS0 | PHYS1 | PHYS3 | PHYS4 | PHYS7 | PHYS8 => idle(r, gobj),
        FELL => fell(r, gobj),
        PHYS2 => phys2(r, gobj),
        RISE => rise(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        THROWN => thrown(r, gobj),
        PHYS6 => phys6(r, gobj),
        DROPPED => dropped(r, gobj),
        BURST => burst(r, gobj),
        OPEN => open(r, gobj),
        _ => return false,
    }
    true
}

/// The party ball's variables: `x0` whether it's opened, `x4` a frame count.
fn vars(game: &mut Game, gobj: GObjId) -> &mut itKusudama_ItemVars {
    item_mut(game, gobj).xDD4_itemVar.kusudama()
}

/// `itKusudamaAttributes`' word at `off`.
fn attr(r: &Running, gobj: GObjId, off: u32) -> melee_data::archive::DataRef {
    article(item(r.game, gobj))
        .special(r.data)
        .expect("a party ball without attributes")
        .at(off)
}

fn attr_i(r: &Running, gobj: GObjId, off: u32) -> i32 {
    r.data.u32(attr(r, gobj, off)) as i32
}

fn attr_f(r: &Running, gobj: GObjId, off: u32) -> f32 {
    r.data.f32(attr(r, gobj, off))
}

/// The model's root, whose rotation the party ball turns.
fn root(r: &Running, gobj: GObjId) -> JObjId {
    jobj_of(r.game, gobj).expect("a party ball without a model")
}

/// A rotation of the root; the original asserts it has no quaternion.
fn rotation(r: &Running, j: JObjId) -> crate::quat::Quat {
    let o = &r.game.objs.jobjs[j];
    assert!(
        o.flags & jobj::USE_QUATERNION == 0,
        "!(jobj->flags & JOBJ_USE_QUATERNION)"
    );
    o.rotate
}

/// `itKusudama_Logic4_Spawned`.
#[doc(alias = "itKusudama_Logic4_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    super::super::set_flag(&mut ip.xDCE_flag, 7, false);
    ip.xDAC_itcmd_var0 = 0;
    ip.xDB0_itcmd_var1 = 0;
    ip.xDB4_itcmd_var2 = 0;
    vars(r.game, gobj).x0 = 0;
    fell(r, gobj);
}

/// `angle` mirrored with the facing (after wrapping past half a turn).
fn facing_angle(r: &Running, gobj: GObjId, angle: f32) -> f64 {
    let mut a = reg(angle);
    if a > reg(PI) {
        a = fp::fsubs(a, reg(TAU));
    }
    if reg(item(r.game, gobj).facing_dir) == -1.0 {
        a = fp::fmuls(a, -1.0);
    }
    a
}

/// Tips the ball towards `angle` about x by up to 5 degrees a frame, `it_802897C8`.
#[doc(alias = "it_802897C8")]
pub fn tip_towards(r: &mut Running, gobj: GObjId, angle: f32) {
    let mut a = facing_angle(r, gobj, angle);
    let j = root(r, gobj);
    let x = reg(rotation(r, j).x);
    let d = fp::fsubs(x, a);
    if (if d < 0.0 { -d } else { d }) > f64::from(0.174_532_92_f32) {
        let step = f64::from(0.087_266_46_f32);
        a = if x < a {
            fp::fadds(x, step)
        } else {
            fp::fsubs(x, step)
        };
    }
    jobj::set_angle(&mut r.game.objs.jobjs, j, |q| q.x = mem(a));
}

/// Tips the ball to `angle` about x, `it_80289910`.
#[doc(alias = "it_80289910")]
pub fn tip_to(r: &mut Running, gobj: GObjId, angle: f32) {
    let a = facing_angle(r, gobj, angle);
    let j = root(r, gobj);
    jobj::set_angle(&mut r.game.objs.jobjs, j, |q| q.x = mem(a));
}

/// Turns the ball about y to face front, by `x28` a frame, `it_80289A00`.
#[doc(alias = "it_80289A00")]
pub fn face_front(r: &mut Running, gobj: GObjId) {
    let j = root(r, gobj);
    let step = reg(attr_f(r, gobj, 0x28));
    let mut y = reg(rotation(r, j).y);
    let half = reg(FRAC_PI_2);
    if reg(item(r.game, gobj).facing_dir) == 1.0 {
        y = if y > fp::fadds(half, step) {
            fp::fsubs(y, step)
        } else {
            half
        };
    } else {
        y = if y < fp::fsubs(-half, step) {
            fp::fadds(y, step)
        } else {
            -half
        };
    }
    jobj::set_angle(&mut r.game.objs.jobjs, j, |q| q.y = mem(y));
}

/// Opens (floating up, chimed when `chime`) or bursts empty by the attributes' weights,
/// `it_80289B50`.
#[doc(alias = "it_80289B50")]
pub fn open_or_burst(r: &mut Running, gobj: GObjId, chime: bool) {
    let w = [4, 0, 8, 0xC].map(|o| attr_i(r, gobj, o));
    if has_items(r, w[0], w[1], w[2], w[3]) {
        if chime {
            play_sfx(r, gobj, 0x126, 0x7F, 0x40);
        }
        rise(r, gobj);
    } else {
        burst(r, gobj);
    }
}

/// Throws one item of `kind` out of the ball at a random speed up to `speed`; whether one
/// came out (the file's inline `it_80289BE8_spawn`).
fn throw_out(r: &mut Running, gobj: GObjId, kind: i32, speed: f32) -> Option<i32> {
    let (pos, vel) = scatter(r, gobj, speed);
    let g = drop::spawn_at(r, Some(gobj), kind, pos)?;
    drop::pop_up(r.game, r.data, g, Some(vel));
    lib2725::count_spawned(r.game);
    Some(kind)
}

/// Below the ball, and a random speed up to `speed` across and half that up or down (the
/// file's inline `it_80289BE8_inline`).
fn scatter(r: &mut Running, gobj: GObjId, speed: f32) -> (Vec3, Vec3) {
    let mut pos = item(r.game, gobj).pos;
    pos.y = mem(fp::fsubs(reg(pos.y), 5.0));
    // The halves and quarters are the compiler's constants (exact).
    let s = reg(speed);
    let f = reg(r.game.objs.rng.draw_f32());
    let x = mem(fp::fmsubs(s, f, fp::fmuls(s, 0.5)));
    let f = reg(r.game.objs.rng.draw_f32());
    let y = mem(fp::fmsubs(fp::fmuls(s, 0.5), f, fp::fmuls(s, 0.25)));
    (pos, Vec3::new(x, y, 0.0))
}

/// Throws out a random kind, if one is drawn (the file's inline
/// `it_80289BE8_spawn_random`, whose missing return leaves the drawn -1).
fn throw_random(r: &mut Running, gobj: GObjId, speed: f32) -> i32 {
    let kind = drop::pick(r.game);
    if kind == -1 {
        return -1;
    }
    throw_out(r, gobj, kind, speed).unwrap_or(-1)
}

/// Rains the ball's items, `it_80289BE8`: coins in a coin match, else a monster if one comes
/// out, else by the weights `kind_w` (the attributes' kind `x10`, `x14` of them), `food_w`
/// (10 to 14 foods, if foods are on) and `random_w` (3 or 4 random items).
#[doc(alias = "it_80289BE8")]
pub fn rain(r: &mut Running, gobj: GObjId, kind_w: i32, food_w: i32, random_w: i32) {
    let mut pos = item(r.game, gobj).pos;
    pos.y = mem(fp::fsubs(reg(pos.y), 5.0));
    if drop::drop_coins(r, gobj, pos, None) {
        return;
    }
    let (mut pos, vel) = scatter(r, gobj, 0.1);
    pos.y = mem(fp::fsubs(reg(pos.y), 5.0));
    if drop::drop_monster(r, Some(gobj), pos, Some(vel)).is_some() {
        return;
    }
    let food_w = if spawn::kind_on(r.game, super::FOODS) {
        food_w
    } else {
        0
    };
    let mut out = [-1i32; 15];
    let mut roll = r
        .game
        .objs
        .rng
        .below(kind_w.wrapping_add(food_w).wrapping_add(random_w));
    let count;
    'pick: {
        if roll < kind_w {
            let kind = r.data.u32(attr(r, gobj, 0x10)) as i32;
            let n = attr_i(r, gobj, 0x14);
            let refused = (kind == super::M_BALL && spawn::poke_balls_full(r.game))
                || !spawn::kind_on(r.game, kind);
            if !refused {
                let mut i = 0;
                while i < n {
                    if kind == super::M_BALL && spawn::poke_balls_full(r.game) {
                        break;
                    }
                    if let Some(k) = throw_out(r, gobj, kind, 1.4) {
                        out[i as usize] = k;
                    }
                    i += 1;
                }
                while i < n {
                    out[i as usize] = throw_random(r, gobj, 1.2);
                    i += 1;
                }
                count = i;
                break 'pick;
            }
            roll = r.game.objs.rng.below(food_w.wrapping_add(random_w));
        } else {
            roll -= kind_w;
        }
        if roll < food_w {
            let n = r.game.objs.rng.below(5) + 10;
            for i in 0..n {
                if let Some(k) = throw_out(r, gobj, super::FOODS, 1.8) {
                    out[i as usize] = k;
                }
            }
            count = n;
        } else {
            let n = r.game.objs.rng.below(2) + 3;
            for slot in out.iter_mut().take(n as usize) {
                *slot = throw_random(r, gobj, 1.2);
            }
            count = n;
        }
    }
    let owner = item(r.game, gobj).owner;
    if super::super::item::is_fighter(r.game, owner) {
        let _ = (out, count);
        code::unported_call("ftLib_800874CC (a fighter's record of the items it got)");
    }
}

/// Whether the ball opens with items, `it_8028A114`: a draw over the weights and the empty
/// weight lands below the three item weights.
#[doc(alias = "it_8028A114")]
pub fn has_items(r: &mut Running, w1: i32, w2: i32, w3: i32, empty: i32) -> bool {
    let sum1 = w1.wrapping_add(w2);
    let sum2 = w3.wrapping_add(sum1);
    let roll = r.game.objs.rng.below(empty.wrapping_add(sum2));
    roll < w1 || roll < sum1 || roll < sum2
}

/// Stands it upright about x (the file's `it_8028A190_inline`).
fn stand(r: &mut Running, gobj: GObjId) {
    let mut x = reg(FRAC_PI_2);
    if reg(item(r.game, gobj).facing_dir) == -1.0 {
        x = fp::fmuls(x, -1.0);
    }
    let j = root(r, gobj);
    jobj::set_angle(&mut r.game.objs.jobjs, j, |q| q.x = mem(x));
}

/// Stops, standing, `it_8028A190`.
#[doc(alias = "it_8028A190")]
pub fn stopped(r: &mut Running, gobj: GObjId) {
    // `Item_StopAndEnterState`.
    item_mut(r.game, gobj).x40_vel = Vec3::new(0.0, 0.0, 0.0);
    lib26b1::enable_pickup(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    stand(r, gobj);
}

/// Tips it back upright, `itKusudama_UnkMotion0_Anim` and 2.
#[doc(alias = "itKusudama_UnkMotion0_Anim")]
#[doc(alias = "itKusudama_UnkMotion2_Anim")]
pub fn upright(r: &mut Running, gobj: GObjId) -> bool {
    tip_towards(r, gobj, FRAC_PI_2);
    false
}

/// The callbacks that do nothing and answer false.
#[doc(alias = "itKusudama_UnkMotion4_Anim")]
#[doc(alias = "itKusudama_UnkMotion6_Anim")]
#[doc(alias = "itKusudama_UnkMotion7_Coll")]
#[doc(alias = "itKusudama_UnkMotion8_Coll")]
pub fn no(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states that move nothing.
#[doc(alias = "itKusudama_UnkMotion0_Phys")]
#[doc(alias = "itKusudama_UnkMotion1_Phys")]
#[doc(alias = "itKusudama_UnkMotion3_Phys")]
#[doc(alias = "itKusudama_UnkMotion4_Phys")]
#[doc(alias = "itKusudama_UnkMotion7_Phys")]
#[doc(alias = "itKusudama_UnkMotion8_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// `itKusudama_UnkMotion0_Coll`.
#[doc(alias = "itKusudama_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    false
}

/// `itKusudama_UnkMotion1_Anim`: floating open, it cheers once (when the script says) and
/// opens when the script says, facing front.
#[doc(alias = "itKusudama_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).xDB4_itcmd_var2 == 1 {
        item_mut(r.game, gobj).xDB4_itcmd_var2 = 2;
        let sfx = if r.game.objs.rng.below(2) != 0 {
            0x127
        } else {
            0x128
        };
        play_sfx(r, gobj, sfx, 0x7F, 0x40);
    }
    if item(r.game, gobj).xDB0_itcmd_var1 != 0 {
        open(r, gobj);
    }
    tip_towards(r, gobj, 0.0);
    face_front(r, gobj);
    false
}

/// `itKusudama_UnkMotion1_Coll`.
#[doc(alias = "itKusudama_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    air(r, gobj);
    false
}

/// Falls (state 2), standing, `it_8028A3CC`.
#[doc(alias = "it_8028A3CC")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    lib26b1::disable_pickup(r.game, gobj);
    change_state(r, gobj, 2, change::ANIM_UPDATE);
    stand(r, gobj);
}

/// Gravity from the attributes.
#[doc(alias = "itKusudama_UnkMotion2_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, super::super::data::attr::FALL_SPEED);
    let max = a.f32(r.data, super::super::data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
}

/// `itKusudama_UnkMotion2_Coll`.
#[doc(alias = "itKusudama_UnkMotion2_Coll")]
pub fn coll2(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, stopped);
    false
}

/// Floats up to open (state 3), `it_8028A544`.
#[doc(alias = "it_8028A544")]
pub fn rise(r: &mut Running, gobj: GObjId) {
    let (x18, x1c) = (attr_f(r, gobj, 0x18), attr_f(r, gobj, 0x1C));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.z = 0.0;
    ip.x40_vel.x = 0.0;
    ip.x40_vel.y = x18;
    let v = ip.xDD4_itemVar.kusudama();
    v.x4 = fp::fctiwz(reg(x1c));
    v.x0 = 1;
    maplib::enter_air(item_mut(r.game, gobj));
    hitbox::set_hit_ids(r.game, gobj);
    lib26b1::show(r.game, gobj);
    lib26b1::disable_pickup(r.game, gobj);
    change_state(r, gobj, 3, change::ANIM_UPDATE);
    let y = if reg(item(r.game, gobj).facing_dir) == 1.0 {
        4.694_936
    } else {
        -4.694_936
    };
    let j = root(r, gobj);
    jobj::set_angle(&mut r.game.objs.jobjs, j, |q| q.y = y);
}

/// Stops in the air to swing open (state 1, 90 frames), keeping its turn.
fn hang(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.x40_vel = Vec3::new(0.0, 0.0, 0.0);
    ip.xDD4_itemVar.kusudama().x4 = 0x5A;
    super::super::set_flag(&mut ip.xDD1_flag, 1, true);
    lib26b1::disable_pickup(r.game, gobj);
    let j = root(r, gobj);
    let y = rotation(r, j).y;
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    jobj::set_angle(&mut r.game.objs.jobjs, j, |q| q.y = y);
}

/// `itKusudama_UnkMotion3_Anim`: rising, it stops after its frames.
#[doc(alias = "itKusudama_UnkMotion3_Anim")]
pub fn anim3(r: &mut Running, gobj: GObjId) -> bool {
    let v = vars(r.game, gobj);
    v.x4 = v.x4.wrapping_sub(1);
    if v.x4 <= 0 {
        hang(r, gobj);
    }
    tip_towards(r, gobj, 0.0);
    face_front(r, gobj);
    false
}

/// `itKusudama_UnkMotion3_Coll`: rising, it stops at a ceiling.
#[doc(alias = "itKusudama_UnkMotion3_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    air(r, gobj);
    if item(r.game, gobj).x378_itemColl.env_flags & 0x6000 != 0 {
        hang(r, gobj);
    }
    false
}

/// `itKusudama_Logic4_PickedUp`.
#[doc(alias = "itKusudama_Logic4_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 4, change::ANIM_UPDATE);
}

/// `itKusudama_Logic4_Thrown`.
#[doc(alias = "itKusudama_Logic4_Thrown")]
pub fn thrown(r: &mut Running, gobj: GObjId) {
    lib26b1::disable_pickup(r.game, gobj);
    change_state(r, gobj, 5, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// Gravity and spin, `itKusudama_UnkMotion6_Phys` (`Item_ApplyFallingPhysics`).
#[doc(alias = "itKusudama_UnkMotion6_Phys")]
pub fn phys6(r: &mut Running, gobj: GObjId) {
    phys2(r, gobj);
    let x68 = common_data(r.game).f32(r.data, super::super::data::common::X68);
    lib2725::clamp_fall(r.game, gobj, x68);
}

/// `itKusudama_UnkMotion5_Coll`: thrown, it opens (chimed) on what it hits, and stops rising
/// at ceilings.
#[doc(alias = "itKusudama_UnkMotion5_Coll")]
pub fn coll5(r: &mut Running, gobj: GObjId) -> bool {
    if air(r, gobj) {
        open_or_burst(r, gobj, true);
    } else if maplib::touched_ceiling(r.game, gobj) != 0 {
        let ip = item_mut(r.game, gobj);
        if reg(ip.x40_vel.y) > 0.0 {
            ip.x40_vel.y = 0.0;
        }
    } else if maplib::touched_wall(r.game, gobj) != 0 {
        open_or_burst(r, gobj, true);
    }
    false
}

/// `itKusudama_Logic4_Dropped`.
#[doc(alias = "itKusudama_Logic4_Dropped")]
pub fn dropped(r: &mut Running, gobj: GObjId) {
    lib26b1::disable_pickup(r.game, gobj);
    change_state(r, gobj, 6, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// `itKusudama_UnkMotion6_Coll`: dropped, it opens on a hard landing, else stands.
#[doc(alias = "itKusudama_UnkMotion6_Coll")]
pub fn coll6(r: &mut Running, gobj: GObjId) -> bool {
    let limit = attr_f(r, gobj, 0x2C);
    if air(r, gobj) {
        let vy = reg(item(r.game, gobj).x40_vel.y);
        if (if vy < 0.0 { -vy } else { vy }) > reg(limit) {
            open_or_burst(r, gobj, true);
        } else {
            maplib::enter_ground(item_mut(r.game, gobj));
            item_mut(r.game, gobj).x40_vel = Vec3::new(0.0, 0.0, 0.0);
            lib26b1::enable_pickup(r.game, gobj);
            change_state(r, gobj, 0, change::ANIM_UPDATE);
            tip_to(r, gobj, FRAC_PI_2);
        }
    }
    false
}

/// Bursts empty, `it_8028AC74`: hidden, it blasts a moment.
#[doc(alias = "it_8028AC74")]
pub fn burst(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    lib2725::hide_tree(r.game, j);
    lib2725::blast_effect(r, gobj);
    hitbox::start_blink(r.game, gobj);
    lib26b1::disable_pickup(r.game, gobj);
    lib26b1::set_xdd0_b3(r.game, gobj);
    lib2725::start_vanishing(r.game, r.data, gobj);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = 0.0;
    ip.x40_vel.y = 0.0;
    super::super::set_flag(&mut ip.xDCF_flag, 2, true);
    let v = ip.xDD4_itemVar.kusudama();
    v.x0 = 1;
    v.x4 = 0x28;
    hitbox::set_hit_ids(r.game, gobj);
    change_state(r, gobj, 7, change::ANIM_UPDATE);
}

/// `itKusudama_UnkMotion7_Anim`: the blast lasts its lifetime.
#[doc(alias = "itKusudama_UnkMotion7_Anim")]
pub fn anim7(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::tick_life(r.game, gobj)
}

/// Swings open (state 8), `it_8028AD44`.
#[doc(alias = "it_8028AD44")]
pub fn open(r: &mut Running, gobj: GObjId) {
    play_sfx(r, gobj, 0xF8, 0x7F, 0x40);
    hitbox::start_blink(r.game, gobj);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = 0.0;
    ip.x40_vel.y = 0.0;
    super::super::set_flag(&mut ip.xDCF_flag, 2, true);
    let v = ip.xDD4_itemVar.kusudama();
    v.x0 = 1;
    v.x4 = 0x55;
    lib26b1::disable_pickup(r.game, gobj);
    change_state(r, gobj, 8, change::ANIM_UPDATE);
}

/// `itKusudama_UnkMotion8_Anim`: open, it rains its items when the script says (confetti from
/// bones 3 and 5), and is gone after 85 frames.
#[doc(alias = "itKusudama_UnkMotion8_Anim")]
pub fn anim8(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).xDAC_itcmd_var0 != 0 {
        item_mut(r.game, gobj).xDAC_itcmd_var0 = 0;
        let w = [4, 0, 8].map(|o| attr_i(r, gobj, o));
        rain(r, gobj, w[0], w[1], w[2]);
        for b in [3, 5] {
            let t = item(r.game, gobj)
                .xBBC_dynamicBoneTable
                .expect("a party ball without a bone table");
            let j = r.game.it.bone_tables[t.0].bones[b];
            crate::game::ef::spawn(r, 0x433, Some(gobj), &[crate::game::ef::Arg::JObj(j)]);
        }
    }
    let v = vars(r.game, gobj);
    v.x4 = v.x4.wrapping_sub(1);
    v.x4 <= 0
}

/// `itKusudama_Logic4_DmgDealt`: an unopened ball opens or bursts.
#[doc(alias = "itKusudama_Logic4_DmgDealt")]
pub fn dmg_dealt(r: &mut Running, gobj: GObjId) -> bool {
    if vars(r.game, gobj).x0 == 0 {
        open_or_burst(r, gobj, false);
    }
    false
}

/// `itKusudama_Logic4_Clanked` (and hitting a shield, reflected).
#[doc(alias = "itKusudama_Logic4_Clanked")]
#[doc(alias = "itKusudama_Logic4_HitShield")]
#[doc(alias = "itKusudama_Logic4_Reflected")]
pub fn clanked(r: &mut Running, gobj: GObjId) -> bool {
    dmg_dealt(r, gobj)
}

/// `itKusudama_Logic4_DmgReceived`: enough damage opens it.
#[doc(alias = "itKusudama_Logic4_DmgReceived")]
pub fn dmg_received(r: &mut Running, gobj: GObjId) -> bool {
    let limit = attr_f(r, gobj, 0x20);
    if vars(r.game, gobj).x0 == 0 && fp::frsp(f64::from(item(r.game, gobj).xC9C)) >= reg(limit) {
        open_or_burst(r, gobj, false);
    }
    false
}

/// Forgets a GObj the ball points to, `itKusudama_Logic4_EvtUnk` (`it_8026B894`).
#[doc(alias = "itKusudama_Logic4_EvtUnk")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
