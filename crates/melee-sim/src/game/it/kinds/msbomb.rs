//! The motion-sensor bomb, `melee/it/kinds/itmsbomb.c`: thrown, it sticks to the floor, wall or
//! ceiling it meets and arms itself; anything that touches it then, or a hit, sets it off.

use super::super::ground::{
    fall, fall_until_touch_unstick, ground_or_fall, ground_skid, line_or, on_line,
};
use super::super::item::{article, change, change_state, play_sfx};
use super::super::{hitbox, item, item_mut, jobj_of, lib26b1, lib2725, maplib, set_flag};
use super::falling_phys;
use crate::fpu::{mem, reg};
use crate::game::handles::CodeRef;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use gekko_fp as fp;
use melee_data::archive::DataRef;

pub const DETONATE: u32 = 0x8028_FE90;
pub const SPAWNED: u32 = 0x8028_FED4;
pub const LANDED: u32 = 0x8028_FF1C;
pub const COLL0: u32 = 0x8028_FF60;
pub const FELL: u32 = 0x8028_FF8C;
pub const PHYS1: u32 = 0x8029_0070;
pub const COLL1: u32 = 0x8029_00A0;
pub const PICKED_UP: u32 = 0x8029_00CC;
pub const DROPPED: u32 = 0x8029_00F4;
pub const THROWN: u32 = 0x8029_0114;
pub const PHYS3: u32 = 0x8029_01C0;
pub const COLL3: u32 = 0x8029_020C;
pub const COLL4: u32 = 0x8029_0404;
pub const PHYS5: u32 = 0x8029_0560;
pub const COLL5: u32 = 0x8029_05AC;
pub const ANIM6: u32 = 0x8029_06C4;
pub const DMG_DEALT: u32 = 0x8029_06E4;
pub const DMG_RECEIVED: u32 = 0x8029_0738;
pub const ENTERED_AIR: u32 = 0x8029_0794;
pub const COLL7: u32 = 0x8029_07BC;
pub const CLANKED: u32 = 0x8029_07F0;
pub const REFLECTED: u32 = 0x8029_0830;
pub const HIT_SHIELD: u32 = 0x8029_0850;
pub const SHIELD_BOUNCED: u32 = 0x8029_08A4;
pub const TOUCHED: u32 = 0x8029_08D8;

/// The armed bomb's sound as it sticks, and its blast's attack flags.
const SFX_STICK: i32 = 243;
const ATTACK_FLAGS: u32 = 0x0044_0061;

/// Runs one of the bomb's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        COLL0 => coll0(r, gobj),
        COLL1 => coll1(r, gobj),
        COLL3 | COLL5 => thrown_coll(r, gobj),
        COLL4 => coll4(r, gobj),
        ANIM6 => exploding(r, gobj),
        DMG_DEALT | HIT_SHIELD => bounce_if_thrown(r, gobj),
        DMG_RECEIVED => hit(r, gobj),
        COLL7 => coll7(r, gobj),
        CLANKED => clanked(r, gobj),
        REFLECTED => lib2725::reflected_back(r.game, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        TOUCHED => touched(r, gobj),
        _ => return None,
    })
}

/// Runs one of the bomb's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DETONATE => detonate(r, gobj),
        SPAWNED => spawned(r, gobj),
        LANDED => landed(r, gobj),
        FELL => fell(r, gobj),
        PHYS1 => gravity(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED | THROWN => thrown(r, gobj),
        PHYS3 | PHYS5 => flying(r, gobj),
        ENTERED_AIR => entered_air(r, gobj),
        _ => return false,
    }
    true
}

/// `itMsBomb_Attrs`: the falling bomb's x turn (`x0`), how far a wall it sticks to sets it
/// out (`x4`), and its thrown box (`x8`).
fn attrs(r: &Running, gobj: GObjId) -> DataRef {
    article(item(r.game, gobj))
        .special(r.data)
        .expect("a motion-sensor bomb without attributes")
}

/// Sets off a stuck bomb that isn't held, `it_8028FE90`: a crushed one's burst
/// (`it_8026C368`), which only Mute City's cars set (`x34_flags.b7`).
#[doc(alias = "it_8028FE90")]
pub fn detonate(r: &mut Running, gobj: GObjId) {
    let ip = item(r.game, gobj);
    if !ip.xDC8_word.flags.x13 && (ip.msid == 4 || ip.msid == 5) {
        explode(r, gobj);
    }
}

/// `itMSBomb_Logic19_Spawned`: not armed, falling.
#[doc(alias = "itMSBomb_Logic19_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .msbomb()
        .xDD4_b0
        .set(false);
    hitbox::start_blink(r.game, gobj);
    fell(r, gobj);
}

/// Lands, stopped, `it_8028FF1C`.
#[doc(alias = "it_8028FF1C")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    lib26b1::enable_pickup(r.game, gobj);
    lib2725::clear_velocity(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// `itMsbomb_UnkMotion0_Coll`: falls off ledges.
#[doc(alias = "itMsbomb_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    false
}

/// Falls turned over (the attributes' x turn), `it_8028FF8C`.
#[doc(alias = "it_8028FF8C")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    let turn = r.data.f32(attrs(r, gobj));
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    let j = jobj_of(r.game, gobj).expect("a bomb without a model");
    lib2725::set_rotation_x(r.game, j, turn);
}

/// Gravity, `itMsbomb_UnkMotion1_Phys`.
#[doc(alias = "itMsbomb_UnkMotion1_Phys")]
pub fn gravity(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
}

/// `itMsbomb_UnkMotion1_Coll`: lands.
#[doc(alias = "itMsbomb_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, landed);
    false
}

/// Held, `itMSBomb_Logic19_PickedUp`.
#[doc(alias = "itMSBomb_Logic19_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// Let go of or thrown, it flies with its thrown box, `itMSBomb_Logic19_Thrown` (and
/// `itMSBomb_Logic19_Dropped`).
#[doc(alias = "itMSBomb_Logic19_Thrown")]
#[doc(alias = "itMSBomb_Logic19_Dropped")]
pub fn thrown(r: &mut Running, gobj: GObjId) {
    let a = attrs(r, gobj).at(8);
    change_state(r, gobj, 3, change::ANIM_UPDATE | change::DROP_UPDATE);
    let b = crate::game::structs::_itECB {
        top: r.data.f32(a),
        bottom: r.data.f32(a.at(4)),
        right: r.data.f32(a.at(8)),
        left: r.data.f32(a.at(12)),
    };
    item_mut(r.game, gobj).xBFC = b.clone();
    maplib::set_box(r.game, gobj, b);
    set_flag(&mut item_mut(r.game, gobj).xDCE_flag, 7, false);
}

/// Flying, `itMsbomb_UnkMotion3_Phys` and `itMsbomb_UnkMotion5_Phys`.
#[doc(alias = "itMsbomb_UnkMotion3_Phys")]
#[doc(alias = "itMsbomb_UnkMotion5_Phys")]
pub fn flying(r: &mut Running, gobj: GObjId) {
    falling_phys(r, gobj);
}

/// `itMsbomb_UnkMotion3_Coll` and `itMsbomb_UnkMotion5_Coll`: sticks to what it touches.
#[doc(alias = "itMsbomb_UnkMotion3_Coll")]
#[doc(alias = "itMsbomb_UnkMotion5_Coll")]
pub fn thrown_coll(r: &mut Running, gobj: GObjId) -> bool {
    fall_until_touch_unstick(r, gobj, stick);
    false
}

/// Sticks on the ground (moved off a ceiling by its height, off a wall by the attributes'
/// `x4`) with a sound, `it_80290238`.
#[doc(alias = "it_80290238")]
pub fn settle(r: &mut Running, gobj: GObjId) {
    let a = attrs(r, gobj);
    set_flag(&mut item_mut(r.game, gobj).xDCE_flag, 3, true);
    if !maplib::touched_floor(r.game, gobj) {
        if maplib::touched_ceiling(r.game, gobj) == 2 {
            let (top, bottom) = (r.data.f32(a.at(8)), r.data.f32(a.at(12)));
            let ip = item_mut(r.game, gobj);
            ip.pos.y = mem(fp::fmadds(
                reg(ip.scl),
                fp::fsubs(reg(top), reg(bottom)),
                reg(ip.pos.y),
            ));
        } else {
            let wall = maplib::touched_wall(r.game, gobj);
            let x4 = reg(r.data.f32(a.at(4)));
            let ip = item_mut(r.game, gobj);
            if wall == 8 {
                ip.pos.x = mem(fp::fadds(reg(ip.pos.x), x4));
            }
            if wall == 4 {
                ip.pos.x = mem(fp::fsubs(reg(ip.pos.x), x4));
            }
        }
    }
    play_sfx(r, gobj, SFX_STICK, 127, 64);
}

/// Stuck where it touched, armed, `it_80290314`.
#[doc(alias = "it_80290314")]
pub fn stick(r: &mut Running, gobj: GObjId) {
    let flags = if item_mut(r.game, gobj).xDD4_itemVar.msbomb().xDD4_b0.get() {
        17
    } else {
        change::ANIM_UPDATE
    };
    change_state(r, gobj, 4, flags);
    maplib::enter_ground(item_mut(r.game, gobj));
    arm(r, gobj);
}

/// What sticking does in both sticking states (`it_80290314`'s inline): stopped, tilted to a
/// floor and settled there, not to be picked up, set off by a touch.
fn arm(r: &mut Running, gobj: GObjId) {
    lib2725::clear_velocity(r.game, gobj);
    if item(r.game, gobj).ground_or_air == 0 {
        lib2725::stop_spin(r.game, r.data, gobj);
        maplib::tilt_to_slope(r.game, r.data, gobj, 0);
        settle(r, gobj);
    }
    lib26b1::disable_pickup(r.game, gobj);
    let ip = item_mut(r.game, gobj);
    ip.touched = Some(CodeRef(TOUCHED));
    ip.xDC8_word.flags.x1A = false;
    hitbox::end_blink(r.game, gobj);
    hitbox::set_hit_ids(r.game, gobj);
    hitbox::set_hit_ids2(r.game, gobj);
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .msbomb()
        .xDD4_b0
        .set(true);
}

/// `itMsbomb_UnkMotion4_Coll`: stuck to a moving line, it rides it; when the line goes, it
/// falls.
#[doc(alias = "itMsbomb_UnkMotion4_Coll")]
pub fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    if on_line(r.game, gobj) {
        let line = item(r.game, gobj).xC30;
        let mut cd = item(r.game, gobj).x378_itemColl.clone();
        maplib::with_stage_coll(r.game, r.data, |c| c.line_callback(&mut cd, line));
        item_mut(r.game, gobj).x378_itemColl = cd;
        maplib::tilt_to_line(r.game, r.data, gobj, 0);
    }
    line_or(r, gobj, unstuck);
    false
}

/// Its line gone, it falls armed, `it_8029047C`.
#[doc(alias = "it_8029047C")]
pub fn unstuck(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 5, 17);
    maplib::enter_air(item_mut(r.game, gobj));
    arm(r, gobj);
    set_flag(&mut item_mut(r.game, gobj).xDCE_flag, 3, false);
}

/// Explodes, `it_802905D8`.
#[doc(alias = "it_802905D8")]
pub fn explode(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 6, change::ANIM_UPDATE);
    blast(r, gobj);
}

/// The blast, `it_80290614`: hidden, its hitboxes out until its life runs out, its attack
/// counted, a gust of wind.
#[doc(alias = "it_80290614")]
pub fn blast(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    lib26b1::set_xdd0_b3(r.game, gobj);
    lib2725::start_vanishing(r.game, r.data, gobj);
    super::wstar::set_attack_flags(r, gobj, ATTACK_FLAGS);
    super::super::lib279c::count_owner_attack(r, gobj);
    let pos = item(r.game, gobj).pos;
    let angle = mem(fp::frsp(1.0471975803375244));
    crate::lb::wind::blast(&mut r.game.gr.winds, pos, 120, 1.0, 0.02, angle);
    lib2725::blast_effect(r, gobj);
    hitbox::set_hit_ids(r.game, gobj);
    hitbox::start_blink(r.game, gobj);
}

/// Exploding, until its life runs out, `itMsbomb_UnkMotion6_Anim`.
#[doc(alias = "itMsbomb_UnkMotion6_Anim")]
pub fn exploding(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::tick_life(r.game, gobj)
}

/// Bounces off what it hit while thrown, `itMSBomb_Logic19_DmgDealt` and
/// `itMSBomb_Logic19_HitShield`.
#[doc(alias = "itMSBomb_Logic19_DmgDealt")]
#[doc(alias = "itMSBomb_Logic19_HitShield")]
pub fn bounce_if_thrown(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).msid == 3 {
        clanked(r, gobj);
    }
    false
}

/// Bounces off, not to be picked up, its hitboxes gone, `itMSBomb_Logic19_Clanked`.
#[doc(alias = "itMSBomb_Logic19_Clanked")]
pub fn clanked(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_victim(r.game, r.data, gobj);
    lib26b1::disable_pickup(r.game, gobj);
    lib2725::clear_hitboxes(r.game, r.data, gobj);
    false
}

/// A hit sets it off once it's stuck, `itMSBomb_Logic19_DmgReceived`.
#[doc(alias = "itMSBomb_Logic19_DmgReceived")]
pub fn hit(r: &mut Running, gobj: GObjId) -> bool {
    let msid = item(r.game, gobj).msid;
    if msid == 4 || msid == 5 {
        explode(r, gobj);
    }
    false
}

/// Skidding off a slope, `itMSBomb_Logic19_EnteredAir`.
#[doc(alias = "itMSBomb_Logic19_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 7, change::ANIM_UPDATE);
}

/// `itMsbomb_UnkMotion7_Coll`: skidding, it stops or falls.
#[doc(alias = "itMsbomb_UnkMotion7_Coll")]
pub fn coll7(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, landed, fell);
    false
}

/// Bounces off a shield while thrown, `itMSBomb_Logic19_ShieldBounced`.
#[doc(alias = "itMSBomb_Logic19_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).msid == 3 {
        lib2725::bounce_off_shield(r.game, gobj);
    }
    false
}

/// Something touched the armed bomb: it explodes, `it_802908D8`.
#[doc(alias = "it_802908D8")]
pub fn touched(r: &mut Running, gobj: GObjId) -> bool {
    explode(r, gobj);
    false
}

/// Forgets a GObj the bomb points to, `itMSBomb_Logic19_EvtUnk` (`it_8026B894`).
// Ports `itMSBomb_Logic19_EvtUnk`; aliased once a check reaches it (nothing dispatches the
// items' `evt_unk` yet).
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
