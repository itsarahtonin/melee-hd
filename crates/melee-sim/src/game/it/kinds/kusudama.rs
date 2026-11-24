//! The party ball, `melee/it/kinds/itkusudama.c`: when hit, thrown hard or hit for enough
//! damage it floats up, swings open and rains items (a kind in a set number, food, or random
//! ones), or bursts empty. Its rest, falls, pickup, throw and drop, its hits, its burst and its
//! opening are the containers' ([`container`]).

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use gekko_fp as fp;

use super::super::ground::air;
use super::super::item::{change, change_state, play_sfx};
use super::super::vars::ItemVars;
use super::super::{drop, hitbox, item, item_mut, jobj_of, lib26b1, lib2725, maplib, spawn};
use super::container::{self, Container, Opening};
use crate::fpu::{mem, reg};
use crate::game::it::ItKind;
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

/// The party ball: its own fall and stop (stood upright), held in 4, thrown into 5 and dropped
/// into 6.
pub const PARTY_BALL: Container = Container {
    fell,
    landed: stopped,
    tilts: false,
    held: 4,
    thrown: 5,
    dropped: 6,
    let_go_disables_pickup: true,
};

/// The party ball's opening: a hit or `x20` damage opens it or bursts it, a throw into
/// something or a drop past `x2C` the same with a chime (`it_80289B50`); empty (by its
/// weights, `x4`, `x0`, `x8` and `xC`) it bursts in state 7, else it rises and swings open in
/// 8 (0xF8), raining its items as its script says.
pub const OPENING: Opening = Opening {
    name: "party ball",
    vars: shared,
    breaks: open_quietly,
    smashes: open_chimed,
    damage_limit: 0x20,
    drop_limit: 0x2C,
    settles: settle,
    has_items,
    burst_state: 7,
    burst_wind: false,
    open_sfx: 0xF8,
    spills: None,
    open_frames: 0x55,
    open_state: 8,
};

/// Runs one of the party ball's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM2 => upright(r, gobj),
        COLL0 => container::rests(r, gobj, &PARTY_BALL),
        ANIM1 => anim1(r, gobj),
        COLL1 => coll1(r, gobj),
        COLL2 => container::lands(r, gobj, &PARTY_BALL),
        ANIM3 => anim3(r, gobj),
        COLL3 => coll3(r, gobj),
        ANIM4 | ANIM6 | COLL7 | COLL8 => container::still(r, gobj),
        COLL5 => container::flies(r, gobj, &OPENING),
        COLL6 => container::drops(r, gobj, &OPENING),
        ANIM7 => container::blast_lasts(r, gobj),
        ANIM8 => anim8(r, gobj),
        DMG_DEALT | CLANKED | HIT_SHIELD | REFLECTED => container::hit(r, gobj, &OPENING),
        DMG_RECEIVED => container::damaged(r, gobj, &OPENING),
        _ => return None,
    })
}

/// Runs one of the party ball's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        STOPPED => stopped(r, gobj),
        PHYS0 | PHYS1 | PHYS3 | PHYS4 | PHYS7 | PHYS8 => container::idle(r, gobj),
        FELL => fell(r, gobj),
        PHYS2 => container::gravity(r, gobj),
        RISE => rise(r, gobj),
        PICKED_UP => container::picked_up(r, gobj, &PARTY_BALL),
        THROWN => container::thrown(r, gobj, &PARTY_BALL),
        PHYS6 => container::thrown_phys(r, gobj),
        DROPPED => container::dropped(r, gobj, &PARTY_BALL),
        BURST => container::burst(r, gobj, &OPENING),
        OPEN => container::open(r, gobj, &OPENING),
        _ => return false,
    }
    true
}

/// The party ball's variables: `x0` whether it's opened, `x4` a frame count.
fn vars(game: &mut Game, gobj: GObjId) -> &mut itKusudama_ItemVars {
    item_mut(game, gobj).vars.kusudama()
}

/// The fields the box and the barrel have too.
fn shared(v: &mut ItemVars) -> container::Vars<'_> {
    let k = v.kusudama();
    container::Vars {
        opened: &mut k.opened,
        timer: &mut k.timer,
    }
}

/// `itKusudamaAttributes`' integer at `off`.
fn attr_i(r: &Running, gobj: GObjId, off: u32) -> i32 {
    container::attr_i(r, gobj, &OPENING, off)
}

/// `itKusudamaAttributes`' float at `off`.
fn attr_f(r: &Running, gobj: GObjId, off: u32) -> f32 {
    container::attr_f(r, gobj, &OPENING, off)
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
    super::super::set_flag(&mut ip.hurt_flags, 7, false);
    ip.cmd_var0 = 0;
    ip.cmd_var1 = 0;
    ip.cmd_var2 = 0;
    vars(r.game, gobj).opened.set(0);
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
    if (OPENING.has_items)(r, gobj) {
        if chime {
            play_sfx(r, gobj, 0x126, 0x7F, 0x40);
        }
        rise(r, gobj);
    } else {
        container::burst(r, gobj, &OPENING);
    }
}

/// Whether it opens with items: [`container::has_items`] over the attributes' weights `x4`,
/// `x0` and `x8` and the empty one `xC`.
fn has_items(r: &mut Running, gobj: GObjId) -> bool {
    let w = [4, 0, 8, 0xC].map(|o| attr_i(r, gobj, o));
    container::has_items(r, w[0], w[1], w[2], w[3])
}

/// A hit's opening or burst (unchimed).
fn open_quietly(r: &mut Running, gobj: GObjId) {
    open_or_burst(r, gobj, false);
}

/// A smash's opening or burst (chimed).
fn open_chimed(r: &mut Running, gobj: GObjId) {
    open_or_burst(r, gobj, true);
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
    let food_w = if spawn::kind_on(r.game, ItKind::Food.id()) {
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
            let kind = attr_i(r, gobj, 0x10);
            let n = attr_i(r, gobj, 0x14);
            let refused = (kind == ItKind::PokeBall && spawn::poke_balls_full(r.game))
                || !spawn::kind_on(r.game, kind);
            if !refused {
                let mut i = 0;
                while i < n {
                    if kind == ItKind::PokeBall && spawn::poke_balls_full(r.game) {
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
                if let Some(k) = throw_out(r, gobj, ItKind::Food.id(), 1.8) {
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
    if let Some(o) = owner.filter(|_| super::super::item::is_fighter(r.game, owner)) {
        r.record_items_got(o, &out[..count as usize]);
    }
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

/// A party ball standing at `pos`, `it_802896CC`.
#[doc(alias = "it_802896CC")]
pub fn spawn_at(r: &mut Running, pos: Vec3) -> Option<GObjId> {
    let mut spawn = super::super::item::drop_spawn(ItKind::PartyBall.id(), pos, false);
    let gobj = super::super::item::spawn_ground(r, &mut spawn)?;
    stopped(r, gobj);
    Some(gobj)
}

/// Stops, standing, `it_8028A190`.
#[doc(alias = "it_8028A190")]
pub fn stopped(r: &mut Running, gobj: GObjId) {
    container::stop(r, gobj);
    stand(r, gobj);
}

/// Dropped softly, it stops, tipped upright (`it_80289910`).
fn settle(r: &mut Running, gobj: GObjId) {
    container::stop(r, gobj);
    tip_to(r, gobj, FRAC_PI_2);
}

/// Tips it back upright, `itKusudama_UnkMotion0_Anim` and 2.
#[doc(alias = "itKusudama_UnkMotion0_Anim")]
#[doc(alias = "itKusudama_UnkMotion2_Anim")]
pub fn upright(r: &mut Running, gobj: GObjId) -> bool {
    tip_towards(r, gobj, FRAC_PI_2);
    false
}

/// `itKusudama_UnkMotion1_Anim`: floating open, it cheers once (when the script says) and
/// opens when the script says, facing front.
#[doc(alias = "itKusudama_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).cmd_var2 == 1 {
        item_mut(r.game, gobj).cmd_var2 = 2;
        let sfx = if r.game.objs.rng.below(2) != 0 {
            0x127
        } else {
            0x128
        };
        play_sfx(r, gobj, sfx, 0x7F, 0x40);
    }
    if item(r.game, gobj).cmd_var1 != 0 {
        container::open(r, gobj, &OPENING);
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

/// Floats up to open (state 3), `it_8028A544`.
#[doc(alias = "it_8028A544")]
pub fn rise(r: &mut Running, gobj: GObjId) {
    let (x18, x1c) = (attr_f(r, gobj, 0x18), attr_f(r, gobj, 0x1C));
    let ip = item_mut(r.game, gobj);
    ip.vel.z = 0.0;
    ip.vel.x = 0.0;
    ip.vel.y = x18;
    let v = ip.vars.kusudama();
    v.timer.set(fp::fctiwz(reg(x1c)));
    v.opened.set(1);
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
    ip.vel = Vec3::new(0.0, 0.0, 0.0);
    ip.vars.kusudama().timer.set(0x5A);
    super::super::set_flag(&mut ip.phys_flags, 1, true);
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
    v.timer.update(|old| old.wrapping_sub(1));
    if v.timer.get() <= 0 {
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
    if item(r.game, gobj).coll.env_flags & 0x6000 != 0 {
        hang(r, gobj);
    }
    false
}

/// `itKusudama_UnkMotion8_Anim`: open, it rains its items when the script says (confetti from
/// bones 3 and 5), and is gone after 85 frames.
#[doc(alias = "itKusudama_UnkMotion8_Anim")]
pub fn anim8(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).cmd_var0 != 0 {
        item_mut(r.game, gobj).cmd_var0 = 0;
        let w = [4, 0, 8].map(|o| attr_i(r, gobj, o));
        rain(r, gobj, w[0], w[1], w[2]);
        for b in [3, 5] {
            let t = item(r.game, gobj)
                .bone_table
                .expect("a party ball without a bone table");
            let j = r.game.it.bone_tables[t.0].bones[b];
            crate::game::ef::spawn(r, 0x433, Some(gobj), &[crate::game::ef::Arg::JObj(j)]);
        }
    }
    container::gone_after(r, gobj, &OPENING)
}
