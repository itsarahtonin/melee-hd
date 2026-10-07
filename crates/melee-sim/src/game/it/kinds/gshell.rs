//! The green shell, `melee/it/kinds/itgshell.c`: kicked, jumped on or hit, it slides along the
//! ground hitting what it meets, bouncing off walls, until it slows to a stop or its time runs
//! out.

use gekko_fp as fp;

use super::super::ground::{air, fall_to_floor, fall_unbreakable, ground_or_fall, ground_skid};
use super::super::item::{article, attr_of, change, change_state, common_data, play_sfx};
use super::super::{coll, hitbox, item, item_mut, jobj_of, lib26b1, lib2725, maplib};
use crate::fpu::{mem, reg};
use crate::game::code;
use crate::game::handles::CodeRef;
use crate::game::state::{Game, Running};
use crate::game::structs::itGShell_ItemVars;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

pub const TRAIL: u32 = 0x8028_B8D8;
pub const SPAWNED: u32 = 0x8028_BDAC;
pub const STOPPED: u32 = 0x8028_BE54;
pub const ANIM0: u32 = 0x8028_BEE8;
pub const PHYS0: u32 = 0x8028_BEF0;
pub const COLL0: u32 = 0x8028_BEF4;
pub const FELL: u32 = 0x8028_C018;
pub const ANIM1: u32 = 0x8028_C054;
pub const PHYS1: u32 = 0x8028_C0D0;
pub const COLL1: u32 = 0x8028_C100;
pub const PICKED_UP: u32 = 0x8028_C148;
pub const ANIM2: u32 = 0x8028_C1A0;
pub const PHYS2: u32 = 0x8028_C1A8;
pub const THROWN: u32 = 0x8028_C1AC;
pub const ANIM3: u32 = 0x8028_C1FC;
pub const PHYS3: u32 = 0x8028_C2A4;
pub const COLL3: u32 = 0x8028_C2D4;
pub const DROPPED: u32 = 0x8028_C300;
pub const ANIM4: u32 = 0x8028_C328;
pub const PHYS4: u32 = 0x8028_C34C;
pub const COLL4: u32 = 0x8028_C37C;
pub const SLIDE: u32 = 0x8028_C3A8;
pub const ANIM6: u32 = 0x8028_C52C;
pub const PHYS6: u32 = 0x8028_C6E4;
pub const COLL6: u32 = 0x8028_C75C;
pub const SLIDE_AIR: u32 = 0x8028_C898;
pub const ANIM8: u32 = 0x8028_CA14;
pub const PHYS8: u32 = 0x8028_CAC8;
pub const COLL8: u32 = 0x8028_CAF8;
pub const ENTERED_AIR: u32 = 0x8028_CB24;
pub const ANIM9: u32 = 0x8028_CB70;
pub const PHYS9: u32 = 0x8028_CB78;
pub const COLL9: u32 = 0x8028_CB7C;
pub const DMG_DEALT: u32 = 0x8028_CCA8;
pub const DMG_RECEIVED: u32 = 0x8028_CCCC;
pub const REFLECTED: u32 = 0x8028_CD04;
pub const CLANKED: u32 = 0x8028_CD3C;
pub const HIT_SHIELD: u32 = 0x8028_CE1C;
pub const SHIELD_BOUNCED: u32 = 0x8028_CF2C;
pub const JUMPED_ON: u32 = 0x8028_CF68;

/// Runs one of the shell's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM2 | ANIM9 => still(r, gobj),
        COLL0 => coll0(r, gobj),
        ANIM1 => anim1(r, gobj),
        COLL1 => coll1(r, gobj),
        ANIM3 | ANIM4 => anim3(r, gobj),
        COLL3 | COLL4 | COLL8 => coll3(r, gobj),
        ANIM6 => anim6(r, gobj),
        COLL6 => coll6(r, gobj),
        ANIM8 => anim8(r, gobj),
        COLL9 => coll9(r, gobj),
        DMG_DEALT => dmg_dealt(r, gobj),
        DMG_RECEIVED => dmg_received(r, gobj),
        REFLECTED => reflected(r, gobj),
        CLANKED => clanked(r, gobj),
        HIT_SHIELD => hit_shield(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        JUMPED_ON => jumped_on(r, gobj),
        _ => return None,
    })
}

/// Runs one of the shell's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        TRAIL => trail(r, gobj),
        SPAWNED => spawned(r, gobj),
        STOPPED => stopped(r, gobj),
        PHYS0 | PHYS2 | PHYS9 => idle(r, gobj),
        FELL => fell(r, gobj),
        PHYS1 | PHYS3 | PHYS4 | PHYS8 => phys_fall(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        THROWN => thrown(r, gobj),
        DROPPED => dropped(r, gobj),
        SLIDE => slide(r, gobj),
        PHYS6 => phys6(r, gobj),
        SLIDE_AIR => slide_air(r, gobj),
        ENTERED_AIR => entered_air(r, gobj),
        _ => return false,
    }
    true
}

/// The shell's variables.
fn vars(game: &mut Game, gobj: GObjId) -> &mut itGShell_ItemVars {
    item_mut(game, gobj).xDD4_itemVar.gshell()
}

/// `itGShell_Attrs`' float at `off`.
fn attr(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a shell without attributes");
    r.data.f32(a.at(off))
}

/// Starts the dust trail every `x30` frames, `it_8028B8D8`.
#[doc(alias = "it_8028B8D8")]
pub fn trail(r: &mut Running, gobj: GObjId) {
    let v = vars(r.game, gobj);
    if reg(v.xDDC) <= 0.0 {
        code::unported_call("efAsync_Spawn (a shell's trail, effect 1029)");
    }
    v.xDDC = mem(fp::fsubs(reg(v.xDDC), 1.0));
}

/// Counts down how long a hit shell can't hurt its owner, `it_8028B988`.
#[doc(alias = "it_8028B988")]
pub fn count_owner_grace(r: &mut Running, gobj: GObjId) {
    let x28 = attr(r, gobj, 0x28);
    let v = vars(r.game, gobj);
    if !v.xDEC_b1 {
        return;
    }
    v.xDE0 = mem(fp::fsubs(reg(v.xDE0), 1.0));
    if reg(v.xDE0) <= 0.0 {
        v.xDEC_b1 = false;
        v.xDE0 = x28;
        let blinking = v.xDEC_b2;
        hitbox::set_hit_flag0(r.game, gobj, 0, true);
        if blinking {
            hitbox::end_blink(r.game, gobj);
        }
    }
}

/// Lands from a throw or a hit, `it_8028BA2C`: stops when slow (or slides off), else
/// slides on.
#[doc(alias = "it_8028BA2C")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    let x8 = attr(r, gobj, 8);
    let b = item(r.game, gobj).xC0C.clone();
    maplib::set_box(r.game, gobj, b);
    if reg(item(r.game, gobj).x40_vel.x).abs() < reg(x8) {
        item_mut(r.game, gobj).x40_vel = Vec3::new(0.0, 0.0, 0.0);
        if !maplib::slides(r.game, r.data, gobj) {
            stopped(r, gobj);
        } else {
            entered_air(r, gobj);
        }
    } else {
        slide(r, gobj);
    }
}

/// `(f32) i`, as the original converts it (exact, then rounded to single).
fn to_f32(i: i32) -> f64 {
    fp::frsp(f64::from(i))
}

/// Sent sliding the way the hit pushed it, `it_8028BAD8`.
#[doc(alias = "it_8028BAD8")]
pub fn knocked(r: &mut Running, gobj: GObjId) {
    let x14 = attr(r, gobj, 0x14);
    let ip = item_mut(r.game, gobj);
    let push = fp::fmuls(to_f32(ip.xCA0), reg(x14));
    let away = -reg(ip.xCCC_incDamageDirection);
    match ip.msid {
        0 | 1 | 3 | 4 | 9 => {
            ip.x40_vel.x = mem(fp::fmuls(away, push));
            vars(r.game, gobj).xDEC_b2 = true;
        }
        5..=8 => {
            ip.x40_vel.x = mem(fp::fmadds(away, push, reg(ip.x40_vel.x)));
            vars(r.game, gobj).xDEC_b2 = true;
        }
        _ => {}
    }
    coll::take_attacker(r.game, gobj);
    hitbox::start_blink(r.game, gobj);
    if item(r.game, gobj).ground_or_air == 1 {
        slide_air(r, gobj);
    } else {
        slide(r, gobj);
    }
    vars(r.game, gobj).xDEC_b1 = true;
    hitbox::set_hit_flag0(r.game, gobj, 0, false);
}

/// Bounces up off a fighter who hit a sliding shell (the file's inline `shellHit`, and the
/// sliding half of `it_8028BC2C`), up to `fell` (state 1).
fn pop_up(r: &mut Running, gobj: GObjId, fell: fn(&mut Running, GObjId)) {
    let (xc, x10, x2c) = (attr(r, gobj, 0xC), attr(r, gobj, 0x10), attr(r, gobj, 0x2C));
    hitbox::start_blink(r.game, gobj);
    hitbox::set_hit_ids(r.game, gobj);
    let f = r.game.objs.rng.draw_f32();
    let ip = item_mut(r.game, gobj);
    let vx = fp::fmuls(-reg(ip.x40_vel.x), reg(xc));
    ip.x40_vel.x = mem(fp::fmuls(vx, reg(f)));
    ip.x40_vel.y = x10;
    if vars(r.game, gobj).xDEC_b0 {
        let max = r
            .data
            .u8(common_data(r.game).0.at(super::super::data::common::X48))
            & 0xF;
        let n = r.game.objs.rng.below(i32::from(max));
        vars(r.game, gobj).xDE8 = n;
    } else {
        vars(r.game, gobj).xDEC_b0 = true;
    }
    maplib::enter_air(item_mut(r.game, gobj));
    fell(r, gobj);
    let v = vars(r.game, gobj);
    v.xDEC_b3 = true;
    v.xDE4 = x2c;
}

/// Jumped on: a sliding shell pops up, a still one is kicked away from the jumper,
/// `it_8028BC2C`.
#[doc(alias = "it_8028BC2C")]
pub fn stomped(r: &mut Running, gobj: GObjId) {
    match item(r.game, gobj).msid {
        5..=8 => {
            pop_up(r, gobj, fell);
            vars(r.game, gobj).xDEC_b2 = true;
        }
        0 | 1 | 3 | 4 | 9 => {
            let x1c = attr(r, gobj, 0x1C);
            let ip = item_mut(r.game, gobj);
            ip.x40_vel.x = mem(fp::fmuls(reg(x1c), -reg(ip.xCD0)));
            coll::take_stomper(r.game, gobj);
            vars(r.game, gobj).xDEC_b2 = true;
            if item(r.game, gobj).ground_or_air == 1 {
                slide_air(r, gobj);
            } else {
                slide(r, gobj);
            }
        }
        _ => {}
    }
    hitbox::start_blink(r.game, gobj);
    vars(r.game, gobj).xDEC_b1 = true;
    hitbox::set_hit_flag0(r.game, gobj, 0, false);
}

/// `itGShell_Logic14_Spawned`.
#[doc(alias = "itGShell_Logic14_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let (x0, x28, x2c) = (attr(r, gobj, 0), attr(r, gobj, 0x28), attr(r, gobj, 0x2C));
    vars(r.game, gobj).xDD4 = x0;
    lib2725::set_half_life(r.game, r.data, gobj, x0);
    let v = vars(r.game, gobj);
    v.xDE8 = 1;
    v.xDEC_b0 = false;
    v.xDEC_b1 = false;
    v.xDE0 = x28;
    v.xDEC_b3 = false;
    v.xDE4 = x2c;
    v.xDEC_b2 = true;
    fell(r, gobj);
}

/// Comes to a stop, stompable, `it_8028BE54`.
#[doc(alias = "it_8028BE54")]
pub fn stopped(r: &mut Running, gobj: GObjId) {
    lib26b1::enable_pickup(r.game, gobj);
    hitbox::set_hit_ids(r.game, gobj);
    hitbox::set_hit_ids2(r.game, gobj);
    hitbox::end_blink(r.game, gobj);
    item_mut(r.game, gobj).x40_vel = Vec3::new(0.0, 0.0, 0.0);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    vars(r.game, gobj).xDEC_b3 = false;
    lib2725::make_stompable(r.game, gobj);
    item_mut(r.game, gobj).jumped_on = Some(CodeRef(JUMPED_ON));
}

/// The states that animate nothing, `itGshell_UnkMotion0_Anim` (2, 9).
#[doc(alias = "itGshell_UnkMotion0_Anim")]
#[doc(alias = "itGshell_UnkMotion2_Anim")]
#[doc(alias = "itGshell_UnkMotion9_Anim")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states that move nothing, `itGshell_UnkMotion0_Phys` (2, 9).
#[doc(alias = "itGshell_UnkMotion0_Phys")]
#[doc(alias = "itGshell_UnkMotion2_Phys")]
#[doc(alias = "itGshell_UnkMotion9_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// Rolls the model with the speed on the ground (`Item_UpdateRollingShellRotation`).
fn roll(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).ground_or_air != 0 {
        return;
    }
    let rate = attr(r, gobj, 0x20);
    maplib::tilt_on_floor(r.game, r.data, gobj);
    let child = jobj_of(r.game, gobj).and_then(|j| r.game.objs.jobjs[j].child);
    let turn = fp::fmuls(reg(rate), reg(item(r.game, gobj).x40_vel.x).abs());
    let j = child.expect("a shell without a model child");
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| {
        o.rotate.y = mem(fp::fadds(reg(o.rotate.y), turn));
    });
}

/// `itGshell_UnkMotion0_Coll`.
#[doc(alias = "itGshell_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    roll(r, gobj);
    false
}

/// Falls (state 1), not stompable, `it_8028C018`.
#[doc(alias = "it_8028C018")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    lib2725::clear_stomp(r.game, gobj);
}

/// `itGshell_UnkMotion1_Anim`: a popped-up shell turns stompable again after `xDE4` frames.
#[doc(alias = "itGshell_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    let stompable = super::super::flag(item(r.game, gobj).xDD0_flag, 0);
    let v = vars(r.game, gobj);
    if v.xDEC_b3 && !stompable {
        if reg(v.xDE4) <= 0.0 {
            lib2725::make_stompable(r.game, gobj);
            item_mut(r.game, gobj).jumped_on = Some(CodeRef(JUMPED_ON));
        } else {
            v.xDE4 = mem(fp::fsubs(reg(v.xDE4), 1.0));
        }
    }
    false
}

/// Gravity from the attributes, `itGshell_UnkMotion1_Phys` (3, 4, 8).
#[doc(alias = "itGshell_UnkMotion1_Phys")]
#[doc(alias = "itGshell_UnkMotion3_Phys")]
#[doc(alias = "itGshell_UnkMotion4_Phys")]
#[doc(alias = "itGshell_UnkMotion8_Phys")]
pub fn phys_fall(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, super::super::data::attr::FALL_SPEED);
    let max = a.f32(r.data, super::super::data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
}

/// `itGshell_UnkMotion1_Coll`: lands stopped while it has landings left (`xDE8`).
#[doc(alias = "itGshell_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    if vars(r.game, gobj).xDE8 != 0 {
        fall_unbreakable(r, gobj, stopped);
        return false;
    }
    air(r, gobj)
}

/// `itGShell_Logic14_PickedUp`.
#[doc(alias = "itGShell_Logic14_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    hitbox::clear_hit_ids(r.game, gobj);
    vars(r.game, gobj).xDEC_b3 = false;
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// `itGShell_Logic14_Thrown`.
#[doc(alias = "itGShell_Logic14_Thrown")]
pub fn thrown(r: &mut Running, gobj: GObjId) {
    vars(r.game, gobj).xDE8 = 1;
    change_state(r, gobj, 3, change::ANIM_UPDATE | change::DROP_UPDATE);
    play_sfx(r, gobj, 242, 127, 64);
}

/// `itGshell_UnkMotion3_Anim` and 4.
#[doc(alias = "itGshell_UnkMotion3_Anim")]
#[doc(alias = "itGshell_UnkMotion4_Anim")]
pub fn anim3(r: &mut Running, gobj: GObjId) -> bool {
    count_owner_grace(r, gobj);
    false
}

/// `itGshell_UnkMotion3_Coll` (4, 8).
#[doc(alias = "itGshell_UnkMotion3_Coll")]
#[doc(alias = "itGshell_UnkMotion4_Coll")]
#[doc(alias = "itGshell_UnkMotion8_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    fall_to_floor(r, gobj, landed);
    false
}

/// `itGShell_Logic14_Dropped`.
#[doc(alias = "itGShell_Logic14_Dropped")]
pub fn dropped(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 4, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// Starts sliding: the speed capped, its time and hit delays set, then sliding (`slow` when
/// all but stopped, `fast` else).
fn start_slide(r: &mut Running, gobj: GObjId, fast: i32, slow: i32, ground: bool) {
    let (x4, x8, x24, x30) = (
        attr(r, gobj, 4),
        attr(r, gobj, 8),
        attr(r, gobj, 0x24),
        attr(r, gobj, 0x30),
    );
    lib26b1::disable_pickup(r.game, gobj);
    hitbox::clear_hit_ids(r.game, gobj);
    hitbox::let_own_items_hit(r.game, gobj);
    let life = common_data(r.game).u32(r.data, super::super::data::common::LIFETIME);
    lib2725::set_lifetime(r.game, r.data, gobj, mem(fp::frsp(f64::from(life))));
    item_mut(r.game, gobj).xD5C = 0;
    let v = vars(r.game, gobj);
    v.xDD8 = x24;
    v.xDDC = x30;
    v.xDEC_b3 = false;
    let ip = item_mut(r.game, gobj);
    if reg(ip.x40_vel.x).abs() > reg(x4) {
        let dir = if reg(ip.x40_vel.x) < 0.0 { -1.0 } else { 1.0 };
        ip.x40_vel.x = mem(fp::fmuls(dir, reg(x4)));
    }
    lib2725::face_motion(r.game, gobj);
    if reg(item(r.game, gobj).x40_vel.x).abs() < reg(x8) {
        change_state(r, gobj, slow, change::ANIM_UPDATE);
        lib26b1::enable_pickup(r.game, gobj);
        if ground {
            hitbox::set_hit_ids2(r.game, gobj);
        }
    } else {
        change_state(r, gobj, fast, change::ANIM_UPDATE);
    }
    lib2725::make_stompable(r.game, gobj);
    item_mut(r.game, gobj).jumped_on = Some(CodeRef(JUMPED_ON));
}

/// Slides on the ground, `it_8028C3A8`.
#[doc(alias = "it_8028C3A8")]
pub fn slide(r: &mut Running, gobj: GObjId) {
    start_slide(r, gobj, 5, 6, true);
}

/// `itGshell_UnkMotion6_Anim` (5, 7, 8): its hits come back after `xDD8` frames, it trails
/// dust on the ground, and its time runs out.
#[doc(alias = "itGshell_UnkMotion6_Anim")]
pub fn anim6(r: &mut Running, gobj: GObjId) -> bool {
    let v = vars(r.game, gobj);
    if reg(v.xDD8) <= 0.0 {
        if !item(r.game, gobj).xDCD_flag.x0.b5 {
            hitbox::set_hit_ids(r.game, gobj);
        }
    } else {
        v.xDD8 = mem(fp::fsubs(reg(v.xDD8), 1.0));
    }
    count_owner_grace(r, gobj);
    if matches!(item(r.game, gobj).msid, 5 | 6) {
        trail(r, gobj);
    }
    let v = vars(r.game, gobj);
    if reg(v.xDD4) <= 0.0 {
        return true;
    }
    v.xDD4 = mem(fp::fsubs(reg(v.xDD4), 1.0));
    false
}

/// `itGshell_UnkMotion6_Phys`: when all but stopped (and not held), it can be picked up.
#[doc(alias = "itGshell_UnkMotion6_Phys")]
pub fn phys6(r: &mut Running, gobj: GObjId) {
    let x8 = attr(r, gobj, 8);
    let ip = item(r.game, gobj);
    if reg(ip.x40_vel.x).abs() < reg(x8) && !ip.xDC8_word.flags.x15 {
        lib26b1::enable_pickup(r.game, gobj);
        lib2725::clear_hitboxes(r.game, r.data, gobj);
        hitbox::set_hit_ids2(r.game, gobj);
    }
}

/// `itGshell_UnkMotion6_Coll`: rolls, and turns around off walls.
#[doc(alias = "itGshell_UnkMotion6_Coll")]
pub fn coll6(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, slide_air);
    roll(r, gobj);
    if maplib::bounce_off_wall(r.game, r.data, gobj) {
        lib2725::face_motion(r.game, gobj);
    }
    false
}

/// Slides through the air, `it_8028C898`.
#[doc(alias = "it_8028C898")]
pub fn slide_air(r: &mut Running, gobj: GObjId) {
    start_slide(r, gobj, 7, 8, false);
}

/// `itGshell_UnkMotion8_Anim`.
#[doc(alias = "itGshell_UnkMotion8_Anim")]
pub fn anim8(r: &mut Running, gobj: GObjId) -> bool {
    anim6(r, gobj);
    false
}

/// `itGShell_Logic14_EnteredAir`.
#[doc(alias = "itGShell_Logic14_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).xDC8_word.flags.x1F = true;
    vars(r.game, gobj).xDEC_b3 = false;
    change_state(r, gobj, 9, change::ANIM_UPDATE);
}

/// `itGshell_UnkMotion9_Coll`.
#[doc(alias = "itGshell_UnkMotion9_Coll")]
pub fn coll9(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, stopped, fell);
    roll(r, gobj);
    false
}

/// `itGShell_Logic14_DmgDealt`.
#[doc(alias = "itGShell_Logic14_DmgDealt")]
pub fn dmg_dealt(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::show(r.game, gobj);
    false
}

/// `itGShell_Logic14_DmgReceived`.
#[doc(alias = "itGShell_Logic14_DmgReceived")]
pub fn dmg_received(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::show(r.game, gobj);
    knocked(r, gobj);
    false
}

/// `itGShell_Logic14_Reflected`: its time starts over at the half lifetime.
#[doc(alias = "itGShell_Logic14_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj);
    let half = item(r.game, gobj).xD48_halfLifeTimer;
    vars(r.game, gobj).xDD4 = half;
    false
}

/// `itGShell_Logic14_Clanked`.
#[doc(alias = "itGShell_Logic14_Clanked")]
pub fn clanked(r: &mut Running, gobj: GObjId) -> bool {
    pop_up(r, gobj, fell);
    false
}

/// `itGShell_Logic14_HitShield`: a thrown shell bounces off, a sliding one pops up.
#[doc(alias = "itGShell_Logic14_HitShield")]
pub fn hit_shield(r: &mut Running, gobj: GObjId) -> bool {
    match item(r.game, gobj).msid {
        3 | 4 => lib2725::bounce_off_victim(r.game, r.data, gobj),
        5..=8 => pop_up(r, gobj, fell),
        _ => {}
    }
    false
}

/// `itGShell_Logic14_ShieldBounced`.
#[doc(alias = "itGShell_Logic14_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    if matches!(item(r.game, gobj).msid, 3 | 4) {
        lib2725::bounce_off_shield(r.game, gobj);
    }
    false
}

/// Jumped on, `it_8028CF68`.
#[doc(alias = "it_8028CF68")]
pub fn jumped_on(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::show(r.game, gobj);
    stomped(r, gobj);
    play_sfx(r, gobj, 241, 127, 64);
    false
}

/// Forgets a GObj the shell points to, `itGShell_Logic14_EvtUnk` (`it_8026B894`).
#[doc(alias = "itGShell_Logic14_EvtUnk")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
