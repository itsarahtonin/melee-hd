//! The Polar Bear, `melee/it/kinds/itwhitebea.c` from `it_802E37BC` on (`It_Kind_Whitebea`; the
//! file's start is Topi's, [`super::topi`]): an adventure enemy the adventure's Icicle
//! Mountain's spawner makes. It falls to the floor, stands a while, then walks the way it faces
//! (a step's sound every 15 frames), turning at walls, ledges and the script's turn points;
//! standing, one draw in two (never after a walk) it crouches and jumps, and its landing speeds
//! the climb up (`grIceMt_801FA6D8`). Hit, it's knocked back, or past its health beaten and
//! flung away (toward the screen by chance); a fighter can pick it up and throw it.
//!
//! Its variables are the union's bytes ([`zako::bytes`]): the enemies' common words, then its
//! own ([`off`]).

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::item::{change, change_state, play_sfx};
use super::super::zako::{self, Knock, attr_f32, attr_s16, set_coll_facing, stop};
use super::super::{ground, hitbox, item, item_mut, lib2725, maplib, owner};
use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const SPAWNED: u32 = 0x802E_37BC;
pub const DMG_RECEIVED: u32 = 0x802E_3884;
pub const PICKED_UP: u32 = 0x802E_468C;
pub const DROPPED: u32 = 0x802E_4700;
pub const THROWN: u32 = 0x802E_47C0;
#[doc(alias("EVT_UNK"))]
pub const FORGET: u32 = 0x802E_4A24;
pub const ANIM0: u32 = 0x802E_39C8;
pub const PHYS0: u32 = 0x802E_3A10;
pub const COLL0: u32 = 0x802E_3A8C;
pub const ANIM1: u32 = 0x802E_3BB8;
pub const PHYS1: u32 = 0x802E_3C70;
pub const COLL1: u32 = 0x802E_3D38;
pub const ANIM2: u32 = 0x802E_41D4;
pub const PHYS2: u32 = 0x802E_432C;
pub const COLL2: u32 = 0x802E_4330;
pub const ANIM3: u32 = 0x802E_3E5C;
pub const PHYS3: u32 = 0x802E_3E64;
pub const COLL3: u32 = 0x802E_3E94;
pub const ANIM4: u32 = 0x802E_3FAC;
pub const PHYS4: u32 = 0x802E_4018;
pub const COLL4: u32 = 0x802E_4040;
pub const ANIM5: u32 = 0x802E_43C0;
pub const PHYS5: u32 = 0x802E_4424;
pub const COLL5: u32 = 0x802E_4428;
pub const ANIM6: u32 = 0x802E_44E4;
pub const PHYS6: u32 = 0x802E_44EC;
pub const COLL6: u32 = 0x802E_451C;
pub const ANIM7: u32 = 0x802E_45D0;
pub const PHYS7: u32 = 0x802E_464C;
pub const COLL7: u32 = 0x802E_4650;
pub const ANIM8: u32 = 0x802E_46B4;
pub const PHYS8: u32 = 0x802E_46FC;
pub const ANIM9: u32 = 0x802E_4818;
pub const PHYS9: u32 = 0x802E_4860;
pub const COLL9: u32 = 0x802E_4890;
pub const ANIM10: u32 = 0x802E_4914;
pub const PHYS10: u32 = 0x802E_495C;
pub const COLL10: u32 = 0x802E_4960;
pub const ANIM11: u32 = 0x802E_49E0;
pub const PHYS11: u32 = 0x802E_49E8;
pub const COLL11: u32 = 0x802E_4A04;

/// Its variables' offsets in the union (`itWhiteBea_ItemVars`; its `x18` is the common `x14.y`,
/// [`zako::BOUNCES`]).
pub mod off {
    /// `x3C`: a walk's frames left (1 after a jump or a throw, which a walk redraws).
    pub const WALK: usize = 0x3C;
    /// `x40`: a countdown (standing, turning, crouching).
    pub const TIMER: usize = 0x40;
    /// `x44`: the frames to its next step's sound.
    pub const STEP: usize = 0x44;
}

fn get(r: &mut Running, gobj: GObjId, off: usize) -> i32 {
    zako::var(r.game, gobj, off)
}

fn set(r: &mut Running, gobj: GObjId, off: usize, v: i32) {
    zako::set_var(r.game, gobj, off, v);
}

/// The Polar Bear's settings for the enemies' shared code (its attributes are
/// `itWhiteBeaAttributes`, through [`zako::attr`]).
pub const POLAR_BEAR: zako::Enemy = zako::Enemy {
    thrown: 9,
    flung: (10, change::ANIM_UPDATE),
    to_screen: (11, zako::UNK_0X1 | change::ANIM_UPDATE),
    on_hit: None,
    keeps_facing: true,
    on_beaten: None,
    stompable: false,
    clears_hits: true,
    knocked: knocked_back,
    flung_keeps_facing: false,
    flung_upright: false,
    flung_checks_blast_zones: false,
};

/// Runs one of the Polar Bear's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        DMG_RECEIVED => zako::damaged(r, gobj, &POLAR_BEAR),
        ANIM0 => zako::looped(r, gobj, 0, change::ANIM_UPDATE),
        COLL0 | COLL2 => coll_ground(r, gobj),
        ANIM1 => anim1(r, gobj),
        COLL1 => coll1(r, gobj),
        ANIM3 | ANIM6 | ANIM11 => zako::answers_false(r, gobj),
        COLL3 => coll3(r, gobj),
        ANIM4 => zako::knocked_anim(r, gobj, 4, walk),
        COLL4 => zako::knocked_coll(r, gobj, walk, fall),
        ANIM2 => anim2(r, gobj),
        ANIM5 => anim5(r, gobj),
        COLL5 | COLL7 => coll_ground_or(r, gobj),
        COLL6 => coll6(r, gobj),
        ANIM7 => anim7(r, gobj),
        ANIM8 => zako::looped(r, gobj, 8, change::ANIM_UPDATE),
        ANIM9 => zako::looped(r, gobj, 9, change::ANIM_UPDATE),
        COLL9 => zako::flight_lands(r, gobj, None),
        ANIM10 => zako::looped(r, gobj, 10, change::ANIM_UPDATE),
        COLL10 => zako::bounce(r, gobj),
        COLL11 => zako::answers_false(r, gobj),
        _ => return None,
    })
}

/// Runs one of the Polar Bear's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED => dropped(r, gobj),
        THROWN => thrown(r, gobj),
        PHYS0 => phys0(r, gobj),
        PHYS1 => phys1(r, gobj),
        PHYS3 | PHYS6 | PHYS9 => zako::fall_physics(r, gobj),
        PHYS4 => zako::knocked_physics(r, gobj),
        PHYS2 | PHYS5 | PHYS7 | PHYS8 | PHYS10 => zako::still(r, gobj),
        PHYS11 => zako::flight_physics(r, gobj),
        _ => return false,
    }
    true
}

/// The Polar Bear's setup, `it_802E37BC`: the enemies' common variables, facing most fighters,
/// its collision an enemy's, no spin, its walk's frames 1 and no countdown; then it falls.
#[doc(alias = "it_802E37BC")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    zako::reset(r.game, gobj);
    let pos = item(r.game, gobj).pos;
    let facing = owner::facing_for(r.game, r.data, pos);
    item_mut(r.game, gobj).facing_dir = facing;
    zako::face(r, gobj, facing);
    set_coll_facing(r.game, gobj);
    zako::init_collision(r.game, gobj);
    lib2725::stop_spin(r.game, r.data, gobj);
    item_mut(r.game, gobj).xDC8_word.flags.x19 = true;
    set(r, gobj, off::WALK, 1);
    set(r, gobj, off::TIMER, 0);
    fall(r, gobj);
}

/// Lands, `fn_802E398C`: settled, state 0.
#[doc(alias = "fn_802E398C")]
pub fn land(r: &mut Running, gobj: GObjId) {
    zako::settle(r, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// Standing's physics, `itWhitebea_UnkMotion0_Phys`: its countdown done, a draw in two; after
/// a walk (frames left) or on 1 it walks, else it crouches to jump.
#[doc(alias = "itWhitebea_UnkMotion0_Phys")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    let t = get(r, gobj, off::TIMER);
    if t == 0 {
        let rand = r.game.objs.rng.below(2);
        if get(r, gobj, off::WALK) != 0 || rand != 0 {
            walk(r, gobj);
            return;
        }
        crouch(r, gobj);
    } else {
        set(r, gobj, off::TIMER, t.wrapping_sub(1));
    }
}

/// Standing's and turning's collision (`itWhitebea_UnkMotion0_Coll`, `_2_Coll`): off the floor
/// it falls.
#[doc(alias = "itWhitebea_UnkMotion0_Coll")]
#[doc(alias = "itWhitebea_UnkMotion2_Coll")]
pub fn coll_ground(r: &mut Running, gobj: GObjId) -> bool {
    ground::ground_or_fall(r, gobj, fall);
    zako::answers_false(r, gobj)
}

/// Walks (`it_802E3AC8`, and its copies `it_802E40A4` and turning's end): at its speed its way,
/// its model and collision facing it, no blink; a walk's frames drawn between `xC` and `xE`
/// unless it has some left within them (1 and under always redraws); its step's sound in 4,
/// state 1.
#[doc(alias = "it_802E3AC8")]
#[doc(alias = "it_802E40A4")]
pub fn walk(r: &mut Running, gobj: GObjId) {
    zako::walk_velocity(r, gobj);
    let facing = item(r.game, gobj).facing_dir;
    zako::face(r, gobj, facing);
    set_coll_facing(r.game, gobj);
    hitbox::end_blink(r.game, gobj);
    let timer = get(r, gobj, off::WALK);
    let (lo, hi) = (attr_s16(r, gobj, 0xC), attr_s16(r, gobj, 0xE));
    if timer < 2 || timer > hi {
        let range = hi.wrapping_sub(lo).wrapping_abs();
        let n = lo.wrapping_add(r.game.objs.rng.below(range));
        set(r, gobj, off::WALK, n);
    }
    set(r, gobj, off::STEP, 4);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// Walking's animation, `itWhitebea_UnkMotion1_Anim`: looped keeping its hits; every 15 frames
/// a step's sound (0x13C) and the floor's own step sound, if it has one.
#[doc(alias = "itWhitebea_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 1, change::ANIM_UPDATE | change::HIT_PRESERVE);
    }
    let step = get(r, gobj, off::STEP);
    if step <= 0 {
        let line = item(r.game, gobj).xC30;
        let flags = r
            .game
            .geometry
            .as_ref()
            .expect("no collision geometry")
            .line_surface(line);
        let sfx = crate::mp::terrain_sounds(r.game.stage_info.grkind, flags)
            .sounds(0)
            .0[0];
        set(r, gobj, off::STEP, 14);
        play_sfx(r, gobj, 0x13C, 127, 64);
        if sfx != -1 {
            play_sfx(r, gobj, sfx, 127, 64);
        }
    } else {
        set(r, gobj, off::STEP, step.wrapping_sub(1));
    }
    false
}

/// Walking's physics, `itWhitebea_UnkMotion1_Phys`: its walk done, it stands for a countdown
/// drawn between `x8` and `xA`; at the script's turn point it turns; along the floor.
#[doc(alias = "itWhitebea_UnkMotion1_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    let w = get(r, gobj, off::WALK);
    if w == 0 {
        let (lo, hi) = (attr_s16(r, gobj, 8), attr_s16(r, gobj, 0xA));
        let range = hi.wrapping_sub(lo).wrapping_abs();
        let n = lo.wrapping_add(r.game.objs.rng.below(range));
        set(r, gobj, off::TIMER, n);
        zako::settle(r, gobj);
        change_state(r, gobj, 0, change::ANIM_UPDATE);
    } else {
        set(r, gobj, off::WALK, w.wrapping_sub(1));
    }
    if lib2725::script_flags_have(r.game, gobj, 2) {
        turn(r, gobj);
    }
    let ip = item_mut(r.game, gobj);
    let (normal, facing) = (ip.x378_itemColl.floor.normal, ip.facing_dir);
    zako::along_surface(&mut ip.x40_vel, normal, facing);
}

/// Walking's collision, `itWhitebea_UnkMotion1_Coll`: at a ledge or into a wall it turns; off
/// the floor it falls.
#[doc(alias = "itWhitebea_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    if ground::ground_to_ledge(r, gobj, turn) {
        if maplib::touched_wall(r.game, gobj) != 0 {
            turn(r, gobj);
        }
    } else {
        fall(r, gobj);
    }
    zako::answers_false(r, gobj)
}

/// Falls, `it_802E3DA0`: no countdown, its sideways speed slowed by `x4`, turned away from a
/// nudge against its facing, state 3.
#[doc(alias = "it_802E3DA0")]
pub fn fall(r: &mut Running, gobj: GObjId) {
    set(r, gobj, off::TIMER, 0);
    let k = reg(attr_f32(r, gobj, 4));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(ip.x40_vel.x), k));
    let (facing, nudge) = (reg(ip.facing_dir), reg(ip.x70_nudge.x));
    if facing > 0.0 && nudge < 0.0 || facing < 0.0 && nudge > 0.0 {
        item_mut(r.game, gobj).facing_dir = mem(fp::fneg(facing));
        set_coll_facing(r.game, gobj);
    }
    change_state(r, gobj, 3, change::ANIM_UPDATE);
}

/// Falling's collision, `itWhitebea_UnkMotion3_Coll`: landing, it stands.
#[doc(alias = "itWhitebea_UnkMotion3_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    ground::fall_to_floor(r, gobj, land);
    zako::answers_false(r, gobj)
}

/// Knocked back, `it_802E3ED0`: along the hit's knockback (up at least 0.2, else a fifth of
/// it, in the air) or, if it keeps to the floor, a fifth of its sideways speed on the ground;
/// facing as before the hit, state 4. It enters the air before the knockback is read, so the
/// knockback never points off the floor (Melee's bugs: [`Knock::Fifth`]).
#[doc(alias = "it_802E3ED0")]
pub fn knocked_back(r: &mut Running, gobj: GObjId) {
    zako::knock_back(r, gobj, Knock::Fifth);
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = ip.init_facing_dir;
    set_coll_facing(r.game, gobj);
    change_state(r, gobj, 4, zako::UNK_0X1 | change::ANIM_UPDATE);
}

/// Turns, `it_802E4190`: stopped, a 15-frame countdown, state 2.
#[doc(alias = "it_802E4190")]
pub fn turn(r: &mut Running, gobj: GObjId) {
    stop(r.game, gobj);
    set(r, gobj, off::TIMER, 0xF);
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// Turning's animation, `itWhitebea_UnkMotion2_Anim`: its facing flips when the countdown
/// reaches 0; when the animation ends, no countdown, and it walks.
#[doc(alias = "itWhitebea_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    if zako::turning(r, gobj, off::TIMER) {
        set(r, gobj, off::TIMER, 0);
        walk(r, gobj);
    }
    false
}

/// Crouches to jump, `it_802E436C`: on the ground, state 5, a 20-frame countdown.
#[doc(alias = "it_802E436C")]
pub fn crouch(r: &mut Running, gobj: GObjId) {
    maplib::enter_ground(item_mut(r.game, gobj));
    change_state(r, gobj, 5, change::ANIM_UPDATE);
    set(r, gobj, off::TIMER, 0x14);
}

/// Crouching's animation, `itWhitebea_UnkMotion5_Anim`: at the animation's end or the
/// countdown's it jumps.
#[doc(alias = "itWhitebea_UnkMotion5_Anim")]
pub fn anim5(r: &mut Running, gobj: GObjId) -> bool {
    let t = get(r, gobj, off::TIMER);
    if !lib2725::is_animating(r.game, gobj) || t == 0 {
        jump(r, gobj);
    } else {
        set(r, gobj, off::TIMER, t.wrapping_sub(1));
    }
    false
}

/// Crouching's and landing's collision (`itWhitebea_UnkMotion5_Coll`, `_7_Coll`): off the
/// floor it falls (`it_8026D6F4`).
#[doc(alias = "itWhitebea_UnkMotion5_Coll")]
#[doc(alias = "itWhitebea_UnkMotion7_Coll")]
pub fn coll_ground_or(r: &mut Running, gobj: GObjId) -> bool {
    ground::ground_or(r, gobj, fall);
    zako::answers_false(r, gobj)
}

/// Jumps, `it_802E4464`: in the air at `x10` up, its collision a falling one, a sound (0x4A),
/// state 6.
#[doc(alias = "it_802E4464")]
pub fn jump(r: &mut Running, gobj: GObjId) {
    let up = attr_f32(r, gobj, 0x10);
    let ip = item_mut(r.game, gobj);
    maplib::enter_air(ip);
    ip.x40_vel.y = up;
    zako::clear_dd0_b4(r.game, gobj);
    play_sfx(r, gobj, 0x4A, 127, 64);
    change_state(r, gobj, 6, change::ANIM_UPDATE);
}

/// Jumping's collision, `itWhitebea_UnkMotion6_Coll`: landing, [`landed`].
#[doc(alias = "itWhitebea_UnkMotion6_Coll")]
pub fn coll6(r: &mut Running, gobj: GObjId) -> bool {
    ground::fall_to_floor(r, gobj, landed);
    zako::answers_false(r, gobj)
}

/// Lands from its jump, `it_802E4558`: on the ground, two sounds (0x46, 0x13C), state 7, and
/// the adventure's climb's burst (`grIceMt_801FA6D8`).
#[doc(alias = "it_802E4558")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    maplib::enter_ground(item_mut(r.game, gobj));
    play_sfx(r, gobj, 0x46, 127, 64);
    play_sfx(r, gobj, 0x13C, 127, 64);
    change_state(r, gobj, 7, change::ANIM_UPDATE);
    crate::game::gr::icemt::bear_landed(r.game, r.data);
}

/// Landing's animation, `itWhitebea_UnkMotion7_Anim`: when it ends, a walk's frame left, a
/// countdown of `x14`, settled, standing.
#[doc(alias = "itWhitebea_UnkMotion7_Anim")]
pub fn anim7(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        set(r, gobj, off::WALK, 1);
        let t = attr_s16(r, gobj, 0x14);
        set(r, gobj, off::TIMER, t);
        zako::settle(r, gobj);
        change_state(r, gobj, 0, change::ANIM_UPDATE);
    }
    false
}

/// Picked up, `itWhiteBea_Logic9_PickedUp`: state 8.
// Ports itWhiteBea_Logic9_PickedUp; aliased once a check reaches it.
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 8, change::ANIM_UPDATE);
}

/// Dropped, `it_2725_Logic9_Dropped`: the enemies' drop, then it falls.
// Ports it_2725_Logic9_Dropped; aliased once a check reaches it.
pub fn dropped(r: &mut Running, gobj: GObjId) {
    zako::dropped(r, gobj);
    fall(r, gobj);
}

/// Thrown, `it_2725_Logic9_Thrown`: the enemies' throw, no countdown, a walk's frame left,
/// state 9.
// Ports it_2725_Logic9_Thrown; aliased once a check reaches it.
pub fn thrown(r: &mut Running, gobj: GObjId) {
    zako::thrown(r, gobj);
    set(r, gobj, off::TIMER, 0);
    set(r, gobj, off::WALK, 1);
    change_state(r, gobj, 9, change::ANIM_UPDATE);
}
