//! The Like Like, `melee/it/kinds/itlikelike.c` (`It_Kind_Likelike`): an adventure enemy the
//! Underground Maze's spawner makes, on the floor or clinging to the ceiling (upside down, its
//! box flipped). It waits, then crawls the way it faces (turning at walls and ledges, and when
//! it's been stuck in place a few frames), now and then pausing to look about; with a fighter
//! near enough ahead it lunges (twice its speed) and, close, swallows it, or from the ceiling
//! drops onto it, mouth open. A swallowed fighter (`ft::co::capture_likelike`) is chewed a bite
//! at a time until it mashes free or the chewing ends, and spat out with the attributes' hit.
//! Hit, the Like Like lets go of what it holds and is knocked down, or past its health beaten
//! and flung away; a fighter can pick it up and throw it.
//!
//! Its variables are the union's bytes ([`zako::bytes`]): the enemies' common words, then its
//! own ([`off`]).

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::item::{article, attr_of, change, change_state, play_sfx};
use super::super::{data, ground, hitbox, item, item_mut, lib2725, maplib, owner, zako};
use crate::fpu::{mem, reg};
use crate::game::camera;
use crate::game::handles::CodeRef;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x802D_9BA8;
pub const DMG_RECEIVED: u32 = 0x802D_9CC4;
pub const PICKED_UP: u32 = 0x802D_BE90;
pub const DROPPED: u32 = 0x802D_BF30;
pub const THROWN: u32 = 0x802D_C000;
pub const DESTROYED: u32 = 0x802D_C480;
/// `it_802DB358`, the swallow's callback on the Like Like.
pub const SWALLOWED: u32 = 0x802D_B358;
/// `ftCo_800C78B0`, the swallow's callback on the fighter.
pub const FT_CAPTURE: u32 = 0x800C_78B0;
pub const ANIM0: u32 = 0x802D_9E38;
pub const PHYS0: u32 = 0x802D_9E80;
pub const COLL0: u32 = 0x802D_9EF4;
pub const ANIM7: u32 = 0x802D_9F9C;
pub const PHYS7: u32 = 0x802D_9FDC;
pub const COLL7: u32 = 0x802D_A05C;
pub const ANIM1: u32 = 0x802D_A194;
pub const PHYS1: u32 = 0x802D_A1DC;
pub const COLL1: u32 = 0x802D_A348;
pub const ANIM2: u32 = 0x802D_A550;
pub const PHYS2: u32 = 0x802D_A598;
pub const COLL2: u32 = 0x802D_A730;
pub const ANIM5: u32 = 0x802D_AAD0;
pub const PHYS5: u32 = 0x802D_AAD8;
pub const COLL4: u32 = 0x802D_AB48;
pub const COLL5: u32 = 0x802D_AB84;
pub const ANIM17: u32 = 0x802D_AC20;
pub const PHYS17: u32 = 0x802D_AC98;
pub const COLL17: u32 = 0x802D_ACE4;
pub const ANIM3: u32 = 0x802D_AE0C;
pub const PHYS3: u32 = 0x802D_AE14;
pub const COLL3: u32 = 0x802D_AE30;
pub const ANIM16: u32 = 0x802D_AEA8;
pub const PHYS16: u32 = 0x802D_AFC8;
pub const COLL16: u32 = 0x802D_AFCC;
pub const ANIM8: u32 = 0x802D_B0F0;
pub const PHYS8: u32 = 0x802D_B138;
pub const COLL8: u32 = 0x802D_B2B0;
pub const ANIM12: u32 = 0x802D_B464;
pub const PHYS12: u32 = 0x802D_B500;
pub const COLL12: u32 = 0x802D_B570;
pub const ANIM9: u32 = 0x802D_B670;
pub const PHYS9: u32 = 0x802D_B70C;
pub const COLL9: u32 = 0x802D_B710;
pub const ANIM10: u32 = 0x802D_B7CC;
pub const PHYS10: u32 = 0x802D_B868;
pub const COLL10: u32 = 0x802D_B86C;
pub const ANIM13: u32 = 0x802D_B914;
pub const PHYS13: u32 = 0x802D_B95C;
pub const COLL13: u32 = 0x802D_B9B8;
pub const ANIM14: u32 = 0x802D_BC68;
pub const PHYS14: u32 = 0x802D_BD3C;
pub const COLL14: u32 = 0x802D_BD40;
pub const ANIM15: u32 = 0x802D_BD7C;
pub const PHYS15: u32 = 0x802D_BE50;
pub const COLL15: u32 = 0x802D_BE54;
pub const ANIM18: u32 = 0x802D_BED4;
pub const PHYS18: u32 = 0x802D_BF2C;
pub const ANIM19: u32 = 0x802D_C050;
pub const PHYS19: u32 = 0x802D_C058;
pub const COLL19: u32 = 0x802D_C088;
pub const ANIM6: u32 = 0x802D_C114;
pub const PHYS6: u32 = 0x802D_C11C;
pub const COLL6: u32 = 0x802D_C268;
pub const ANIM20: u32 = 0x802D_C370;
pub const PHYS20: u32 = 0x802D_C3B8;
pub const COLL20: u32 = 0x802D_C3BC;
pub const ANIM21: u32 = 0x802D_C43C;
pub const PHYS21: u32 = 0x802D_C444;
pub const COLL21: u32 = 0x802D_C460;

/// Its variables' offsets in the union (`itLikelike_ItemVars`).
pub mod off {
    /// `x18` (the common `x14.y`), cleared as it's flung away.
    pub const X18: usize = 0x18;
    /// `x20`: where it stood last frame, for its stuck check.
    pub const LAST_POS: usize = 0x20;
    /// `x38`: 1 while it clings to the ceiling.
    pub const CEILING: usize = 0x38;
    /// `x3C`: the way to its target (-1 or 1), 0 for none.
    pub const AIM: usize = 0x3C;
    /// `x44`: the frames to its next chance of a pause.
    pub const PAUSE: usize = 0x44;
    /// `x48`: frames in a row it hasn't moved.
    pub const STUCK: usize = 0x48;
    /// `x4C`: a state's countdown.
    pub const TIMER: usize = 0x4C;
    /// `x50`: its target, then the fighter it swallowed.
    pub const TARGET: usize = 0x50;
    /// `x54`: its first hurtbox's capsule (its offsets and scale, from `+0x58`).
    pub const HURT: usize = 0x54;
}

/// Its special attributes (`itLikelikeAttributes`), `off` bytes in.
fn attr(r: &Running, gobj: GObjId, off: u32) -> DataRef {
    article(item(r.game, gobj))
        .special(r.data)
        .expect("a Like Like without attributes")
        .at(off)
}

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    r.data.f32(attr(r, gobj, off))
}

fn attr_i32(r: &Running, gobj: GObjId, off: u32) -> i32 {
    r.data.u32(attr(r, gobj, off)) as i32
}

fn attr_s8(r: &Running, gobj: GObjId, off: u32) -> i32 {
    i32::from(r.data.u8(attr(r, gobj, off)) as i8)
}

/// `attr->x0`: its health (`x`) and crawling speed (`y`).
fn health(r: &Running, gobj: GObjId) -> i32 {
    let p = r.data.ptr(attr(r, gobj, 0)).expect("a Like Like's health");
    r.data.u32(p) as i32
}

fn speed(r: &Running, gobj: GObjId) -> f32 {
    let p = r.data.ptr(attr(r, gobj, 0)).expect("a Like Like's speed");
    r.data.f32(p.at(4))
}

fn get(r: &mut Running, gobj: GObjId, off: usize) -> i32 {
    zako::bytes(r.game, gobj).i32(off)
}

fn set(r: &mut Running, gobj: GObjId, off: usize, v: i32) {
    zako::bytes(r.game, gobj).set_i32(off, v);
}

fn target(r: &mut Running, gobj: GObjId) -> Option<GObjId> {
    zako::bytes(r.game, gobj).gobj(off::TARGET)
}

fn set_target(r: &mut Running, gobj: GObjId, g: Option<GObjId>) {
    zako::bytes(r.game, gobj).set_gobj(off::TARGET, g);
}

/// `GA_Air`.
const GA_AIR: i32 = 1;
/// `QuakeKind_Small`.
const QUAKE_SMALL: i32 = 2;
/// `ITEM_UNK_0x1`, a state change's first flag.
const UNK_0X1: u32 = 1;
/// Fighters whose `x1A6A` shares this bit pass under its swallow.
const GRAB_MASK: u8 = 1;

/// The original's `ABS` of a register value.
fn abs(v: f64) -> f64 {
    if v < fp::frsp(0.0) { fp::fneg(v) } else { v }
}

/// Runs one of the Like Like's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        DMG_RECEIVED => damaged(r, gobj),
        ANIM0 => anim0(r, gobj),
        COLL0 | COLL7 | COLL8 | COLL6 => coll_cling_or_ground(r, gobj),
        ANIM7 => anim7(r, gobj),
        ANIM1 => anim1(r, gobj),
        COLL1 => coll1(r, gobj),
        ANIM2 => anim2(r, gobj),
        COLL2 => coll2(r, gobj),
        ANIM5 | ANIM3 | ANIM19 | ANIM6 | ANIM21 => answers_false(r, gobj),
        COLL4 => coll4(r, gobj),
        COLL5 => coll5(r, gobj),
        ANIM17 => anim17(r, gobj),
        COLL17 => coll17(r, gobj),
        COLL3 => coll3(r, gobj),
        ANIM16 => anim16(r, gobj),
        COLL16 => coll16(r, gobj),
        ANIM8 => anim8(r, gobj),
        COLL12 => coll12(r, gobj),
        ANIM9 | ANIM10 | ANIM12 => anim_swallow(r, gobj),
        COLL9 | COLL10 | COLL13 | COLL14 | COLL15 => coll_ground_or_let_go(r, gobj),
        ANIM13 => anim13(r, gobj),
        ANIM14 => anim14(r, gobj),
        ANIM15 => anim15(r, gobj),
        ANIM18 => anim18(r, gobj),
        COLL19 => coll19(r, gobj),
        ANIM20 => anim20(r, gobj),
        COLL20 => coll20(r, gobj),
        COLL21 => coll21(r, gobj),
        _ => return None,
    })
}

/// Runs one of the Like Like's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED => dropped(r, gobj),
        THROWN => thrown(r, gobj),
        DESTROYED => destroyed(r, gobj),
        SWALLOWED => swallowed(r, gobj),
        PHYS0 => phys0(r, gobj),
        PHYS7 => phys7(r, gobj),
        PHYS1 => phys1(r, gobj),
        PHYS2 => phys2(r, gobj),
        PHYS5 => phys5(r, gobj),
        PHYS17 => phys17(r, gobj),
        PHYS3 => phys3(r, gobj),
        PHYS16 | PHYS9 | PHYS10 | PHYS14 | PHYS15 | PHYS18 | PHYS20 => still(r, gobj),
        PHYS8 => phys8(r, gobj),
        PHYS12 => phys12(r, gobj),
        PHYS13 => phys13(r, gobj),
        PHYS19 => phys19(r, gobj),
        PHYS6 => phys6(r, gobj),
        PHYS21 => phys21(r, gobj),
        _ => return false,
    }
    true
}

/// Whether a fighter is in reach to lunge at, `it_802D9A2C`: the nearest fighter, if no enemy
/// holds it, within `x1C` across (`x20` in its pause, 7) and `x24` up or down; its way (-1 left,
/// 1 right) kept in `x3C` and the fighter as its target, else 0.
#[doc(alias = "it_802D9A2C")]
pub fn in_reach(r: &mut Running, gobj: GObjId) -> i32 {
    set(r, gobj, off::AIM, 0);
    let msid = item(r.game, gobj).msid;
    let range = fp::fctiwz(reg(attr_f32(r, gobj, if msid == 7 { 0x20 } else { 0x1C })));
    let pos = item(r.game, gobj).pos;
    let Some(f) = owner::nearest_opponent(r.game, r.data, pos, None) else {
        return get(r, gobj, off::AIM);
    };
    if zako::held_by_enemy(r.game, f) {
        return get(r, gobj, off::AIM);
    }
    let at = r.game.ft_pos(f);
    let dx = abs(fp::fsubs(reg(at.x), reg(pos.x)));
    if dx < fp::frsp(f64::from(range)) {
        let mut dy = fp::fsubs(reg(at.y), reg(pos.y));
        if dy < 0.0 {
            dy = fp::fneg(dy);
        }
        if dy < reg(attr_f32(r, gobj, 0x24)) {
            set_target(r, gobj, Some(f));
            let aim = if at.x < pos.x { -1 } else { 1 };
            set(r, gobj, off::AIM, aim);
        }
    }
    get(r, gobj, off::AIM)
}

/// Turns back at its speed (the inline `itSwapVelocity`, and `it_802D9B78`, the ledge's
/// callback).
// Ports it_802D9B78; aliased once a check reaches it.
pub fn turn_back(r: &mut Running, gobj: GObjId) {
    let s = reg(speed(r, gobj));
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
    ip.x40_vel.x = mem(fp::fmuls(reg(ip.facing_dir), s));
}

/// The Like Like's setup, `it_802D9BA8`: the enemies' common variables, facing most fighters,
/// its model facing the screen, its collision an enemy's, a pause's chance next frame, no
/// target, its hurtbox's capsule kept, where it stands, on the floor or the ceiling at random,
/// 40 lower, a first wait drawn between `x8` and `xC`; then it falls, or rises to the ceiling.
#[doc(alias = "it_802D9BA8")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    zako::reset(r.game, gobj);
    let pos = item(r.game, gobj).pos;
    let facing = owner::facing_for(r.game, r.data, pos);
    item_mut(r.game, gobj).facing_dir = facing;
    zako::face(r, gobj, 0.0);
    let ip = item_mut(r.game, gobj);
    ip.xD5C = 0;
    ip.xDC8_word.flags.x15 = false;
    hitbox::clear_dcd_b7(r.game, gobj);
    zako::set_dd0_b4(r.game, gobj);
    set(r, gobj, off::PAUSE, 1);
    set(r, gobj, off::TIMER, 0);
    set_target(r, gobj, None);
    let (a, b, scale) = super::super::coll::hurt_capsule(r.game, r.data, gobj, 0);
    let pos = item(r.game, gobj).pos;
    let v = zako::bytes(r.game, gobj);
    v.set_vec(off::HURT + 4, a);
    v.set_vec(off::HURT + 0x10, b);
    v.set_f32(off::HURT + 0x1C, scale);
    v.set_vec(off::LAST_POS, pos);
    let ceiling = r.game.objs.rng.below(2);
    set(r, gobj, off::CEILING, ceiling);
    let ip = item_mut(r.game, gobj);
    ip.pos.y = mem(fp::fsubs(reg(ip.pos.y), 40.0));
    let (lo, hi) = (attr_i32(r, gobj, 8), attr_i32(r, gobj, 0xC));
    let n = r.game.objs.rng.below(hi.wrapping_sub(lo));
    set(r, gobj, off::TIMER, lo.wrapping_add(n));
    if get(r, gobj, off::CEILING) == 0 {
        fall(r, gobj);
        return;
    }
    rise(r, gobj);
}

/// Takes a hit, `it_2725_Logic5_DmgReceived`: whoever it swallowed spat out, its facing kept,
/// the hit's element adds to the damage; past its health (or hit while thrown, 0x13) it's
/// beaten (`Item_ZakoDefeat`) and flung away (toward the screen by the enemies' common odds),
/// else knocked down.
#[doc(alias = "it_2725_Logic5_DmgReceived")]
pub fn damaged(r: &mut Running, gobj: GObjId) -> bool {
    release(r, gobj, 0, true);
    let ip = item_mut(r.game, gobj);
    ip.init_facing_dir = ip.facing_dir;
    let extra = reg(zako::element_damage(r, gobj));
    let ip = item_mut(r.game, gobj);
    ip.xC9C = fp::fctiwz(fp::fadds(fp::frsp(f64::from(ip.xC9C)), extra));
    let (damage, msid) = (ip.xC9C, ip.msid);
    if damage > health(r, gobj) || msid == 0x13 {
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
        knocked_down(r, gobj);
    }
    false
}

/// Waits on the floor, `it_802D9DDC`: settled, along the floor, its collision an enemy's,
/// state 0.
#[doc(alias = "it_802D9DDC")]
pub fn wait(r: &mut Running, gobj: GObjId) {
    zako::settle(r, gobj);
    ground::ground_probe(r, gobj);
    zako::align_floor(r, gobj, 0.0, 5.0);
    zako::set_dd0_b4(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// Waiting's animation, `itLikelike_UnkMotion0_Anim`: looped.
#[doc(alias = "itLikelike_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 0, change::ANIM_UPDATE);
    }
    false
}

/// Waiting's physics, `itLikelike_UnkMotion0_Phys`: its wait done it crawls (on the ceiling if
/// it clings there); along the floor.
#[doc(alias = "itLikelike_UnkMotion0_Phys")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    let t = get(r, gobj, off::TIMER);
    if t == 0 {
        if get(r, gobj, off::CEILING) == 1 {
            crawl_ceiling(r, gobj);
        } else {
            crawl(r, gobj);
        }
    } else {
        set(r, gobj, off::TIMER, t.wrapping_sub(1));
    }
    zako::align_floor(r, gobj, 0.0, 5.0);
}

/// The pass clinging to the ceiling (`it_80276214`, `mpColl_8004C750`): it keeps where the pass
/// leaves it and the ceiling it found; whether it's at a ledge (`it_802762D8`).
fn cling(r: &mut Running, gobj: GObjId) -> bool {
    maplib::prep_coll(r.game, r.data, gobj);
    let mut cd = item(r.game, gobj).x378_itemColl.clone();
    let hit = maplib::with_stage_coll(r.game, r.data, |c| c.ceiling_frame(&mut cd));
    let ip = item_mut(r.game, gobj);
    ip.x378_itemColl = cd;
    ip.pos = ip.x378_itemColl.cur_pos;
    if hit {
        ip.xC30 = ip.x378_itemColl.ceiling.index;
    }
    maplib::at_edge(r.game, gobj)
}

/// The collision of the states it can hold on the ceiling in (`itLikelike_UnkMotion0_Coll`,
/// `_6_Coll`, `_7_Coll`, `_8_Coll`): clinging, the ceiling's pass; else off the floor it lets
/// go and falls.
#[doc(alias = "itLikelike_UnkMotion0_Coll")]
#[doc(alias = "itLikelike_UnkMotion6_Coll")]
#[doc(alias = "itLikelike_UnkMotion7_Coll")]
#[doc(alias = "itLikelike_UnkMotion8_Coll")]
pub fn coll_cling_or_ground(r: &mut Running, gobj: GObjId) -> bool {
    if get(r, gobj, off::CEILING) == 1 {
        let _ = cling(r, gobj);
    } else {
        ground::ground_or(r, gobj, let_go_fall);
    }
    zako::answers_false(r, gobj)
}

/// The pause's animation, `itLikelike_UnkMotion7_Anim`: when it ends it crawls.
#[doc(alias = "itLikelike_UnkMotion7_Anim")]
pub fn anim7(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        crawl(r, gobj);
    }
    false
}

/// The pause's physics, `itLikelike_UnkMotion7_Phys`: each countdown's end a fighter in reach
/// is lunged at, else the countdown starts again from `x18`.
#[doc(alias = "itLikelike_UnkMotion7_Phys")]
pub fn phys7(r: &mut Running, gobj: GObjId) {
    let t = get(r, gobj, off::TIMER);
    if t <= 0 {
        if in_reach(r, gobj) != 0 {
            lunge(r, gobj);
            return;
        }
        let n = attr_i32(r, gobj, 0x18);
        set(r, gobj, off::TIMER, n);
        return;
    }
    set(r, gobj, off::TIMER, t.wrapping_sub(1));
}

/// Crawls on the floor, `it_802DA104` (and pausing's end, `itLikelike_UnkMotion16_Anim`'s):
/// at its speed its way, no blink, a countdown of `x18`, a pause's chance in 60 frames, not
/// stuck, state 1.
#[doc(alias = "it_802DA104")]
pub fn crawl(r: &mut Running, gobj: GObjId) {
    start_crawl(r, gobj, 1);
}

/// Crawls on the ceiling, `it_802DA4C0`: as [`crawl`], state 2.
#[doc(alias = "it_802DA4C0")]
pub fn crawl_ceiling(r: &mut Running, gobj: GObjId) {
    start_crawl(r, gobj, 2);
}

fn start_crawl(r: &mut Running, gobj: GObjId, msid: i32) {
    let s = reg(speed(r, gobj));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(ip.facing_dir), s));
    if msid == 1 {
        ip.x40_vel.z = 0.0;
        ip.x40_vel.y = 0.0;
    } else {
        ip.x40_vel.y = 0.0;
        ip.x40_vel.z = 0.0;
    }
    hitbox::end_blink(r.game, gobj);
    let n = attr_i32(r, gobj, 0x18);
    set(r, gobj, off::TIMER, n);
    set(r, gobj, off::PAUSE, 0x3C);
    set(r, gobj, off::STUCK, 0);
    change_state(r, gobj, msid, UNK_0X1);
}

/// Crawling's animation, `itLikelike_UnkMotion1_Anim`: looped keeping its hits.
#[doc(alias = "itLikelike_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 1, change::ANIM_UPDATE | change::HIT_PRESERVE);
    }
    false
}

/// Turned away from a nudge against its facing (the crawls' physics).
fn turn_from_nudge(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    let (facing, nudge) = (reg(ip.facing_dir), reg(ip.x70_nudge.x));
    if facing > 0.0 && nudge < 0.0 || facing < 0.0 && nudge > 0.0 {
        ip.facing_dir = mem(fp::fneg(facing));
        ip.x40_vel.x = mem(fp::fmuls(reg(ip.x40_vel.x), fp::fneg(1.0)));
    }
}

/// Crawling's physics, `itLikelike_UnkMotion1_Phys`: every 60 frames a draw in `x3C` pauses it
/// (state 7, a countdown of `x18`); each countdown's end a fighter in reach is lunged at, else
/// one draw in three pauses it to look about, else the countdown starts again; turned from a
/// nudge, along the floor.
#[doc(alias = "itLikelike_UnkMotion1_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    let p = get(r, gobj, off::PAUSE);
    if p == 0 {
        let n = attr_s8(r, gobj, 0x3C);
        if r.game.objs.rng.below(n) != 0 {
            set(r, gobj, off::PAUSE, 0x3C);
        } else {
            lib2725::clear_velocity(r.game, gobj);
            let n = attr_i32(r, gobj, 0x18);
            set(r, gobj, off::TIMER, n);
            change_state(r, gobj, 7, change::ANIM_UPDATE);
            return;
        }
    } else {
        set(r, gobj, off::PAUSE, p.wrapping_sub(1));
    }
    let t = get(r, gobj, off::TIMER);
    if t == 0 {
        if in_reach(r, gobj) != 0 {
            lunge(r, gobj);
            return;
        }
        if r.game.objs.rng.below(3) != 0 {
            let n = attr_i32(r, gobj, 0x18);
            set(r, gobj, off::TIMER, n);
        } else {
            look_about(r, gobj);
            return;
        }
    } else {
        set(r, gobj, off::TIMER, t.wrapping_sub(1));
    }
    turn_from_nudge(r, gobj);
    let ip = item_mut(r.game, gobj);
    let (normal, facing) = (ip.x378_itemColl.floor.normal, ip.facing_dir);
    zako::along_surface(&mut ip.x40_vel, normal, facing);
    zako::align_floor(r, gobj, 0.0, 5.0);
}

/// Whether it hasn't moved half its speed since last frame (the inline `likelikeVelocity`):
/// five frames in a row and it turns back.
fn stuck_check(r: &mut Running, gobj: GObjId) {
    let s = fp::fmuls(reg(speed(r, gobj)), 0.5);
    let pos = item(r.game, gobj).pos;
    let last = zako::bytes(r.game, gobj).vec(off::LAST_POS);
    if abs(fp::fsubs(reg(last.x), reg(pos.x))) < s && abs(fp::fsubs(reg(last.y), reg(pos.y))) < s {
        let n = get(r, gobj, off::STUCK);
        if n >= 4 {
            turn_back(r, gobj);
            set(r, gobj, off::STUCK, 0);
        } else {
            set(r, gobj, off::STUCK, n.wrapping_add(1));
        }
    } else {
        set(r, gobj, off::STUCK, 0);
    }
}

fn keep_pos(r: &mut Running, gobj: GObjId) {
    let pos = item(r.game, gobj).pos;
    zako::bytes(r.game, gobj).set_vec(off::LAST_POS, pos);
}

/// Crawling's collision, `itLikelike_UnkMotion1_Coll`: at a ledge it turns back; into a wall it
/// turns back, else stuck in place it turns back; off the floor it lets go and falls.
#[doc(alias = "itLikelike_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    if ground::ground_to_ledge(r, gobj, turn_back) {
        if maplib::touched_wall(r.game, gobj) != 0 {
            turn_back(r, gobj);
        } else {
            stuck_check(r, gobj);
        }
    } else {
        let_go_fall(r, gobj);
    }
    keep_pos(r, gobj);
    zako::answers_false(r, gobj)
}

/// Crawling on the ceiling's animation, `itLikelike_UnkMotion2_Anim`: looped keeping its hits.
#[doc(alias = "itLikelike_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 2, change::ANIM_UPDATE | change::HIT_PRESERVE);
    }
    false
}

/// Crawling on the ceiling's physics, `itLikelike_UnkMotion2_Phys`: each countdown's end, the
/// nearest fighter within `x28` across and `x2C` up or down makes it drop (its way 1), else one
/// draw in three pauses it to look about, else the countdown starts again; at its speed its
/// way, turned from a nudge, along the ceiling.
#[doc(alias = "itLikelike_UnkMotion2_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId) {
    let t = get(r, gobj, off::TIMER);
    if t == 0 {
        let mut near = false;
        let pos = item(r.game, gobj).pos;
        if let Some(f) = owner::nearest_opponent(r.game, r.data, pos, None) {
            let at = r.game.ft_pos(f);
            let dx = abs(fp::fsubs(reg(at.x), reg(pos.x)));
            if dx < reg(attr_f32(r, gobj, 0x28)) {
                let dy = abs(fp::fsubs(reg(at.y), reg(pos.y)));
                if dy < reg(attr_f32(r, gobj, 0x2C)) {
                    near = true;
                }
            }
        }
        if near {
            set(r, gobj, off::AIM, 1);
            let_go_fall(r, gobj);
            return;
        }
        if r.game.objs.rng.below(3) != 0 {
            let n = attr_i32(r, gobj, 0x18);
            set(r, gobj, off::TIMER, n);
        } else {
            look_about(r, gobj);
            return;
        }
    } else {
        set(r, gobj, off::TIMER, t.wrapping_sub(1));
    }
    let s = reg(speed(r, gobj));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(ip.facing_dir), s));
    turn_from_nudge(r, gobj);
    let ip = item_mut(r.game, gobj);
    let (normal, facing) = (ip.x378_itemColl.floor.normal, ip.facing_dir);
    zako::along_surface(&mut ip.x40_vel, normal, facing);
    zako::align_ceiling(r, gobj, 0.0, 5.0);
}

/// Crawling on the ceiling's collision, `itLikelike_UnkMotion2_Coll`: the ceiling's pass; at a
/// ledge or into a wall it turns back, else stuck in place it turns back.
#[doc(alias = "itLikelike_UnkMotion2_Coll")]
pub fn coll2(r: &mut Running, gobj: GObjId) -> bool {
    let reverse = |r: &mut Running| {
        let ip = item_mut(r.game, gobj);
        ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
        ip.x40_vel.x = mem(fp::fmuls(reg(ip.x40_vel.x), fp::fneg(1.0)));
    };
    if cling(r, gobj) || maplib::touched_wall(r.game, gobj) != 0 {
        reverse(r);
    } else {
        stuck_check(r, gobj);
    }
    keep_pos(r, gobj);
    zako::answers_false(r, gobj)
}

/// Lands on the ceiling, `it_802DA8D8`: on the ground (the ceiling), its box flipped, state 0.
#[doc(alias = "it_802DA8D8")]
pub fn clung(r: &mut Running, gobj: GObjId) {
    maplib::enter_ground(item_mut(r.game, gobj));
    flip_box(r, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// Its box upside down (bottom and top swapped).
fn flip_box(r: &mut Running, gobj: GObjId) {
    let ip = item(r.game, gobj);
    let mut e = ip.xC1C.clone();
    e.bottom = e.top;
    e.top = ip.xC1C.bottom;
    maplib::set_box(r.game, gobj, e);
}

/// Falls, `it_802DA960` (and its copy, `it_2725_Logic5_Dropped`'s): in the air, off the
/// ceiling (its box flipped back, its model upright), stopped, state 4.
#[doc(alias = "it_802DA960")]
pub fn fall(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    if get(r, gobj, off::CEILING) == 1 {
        flip_box(r, gobj);
        zako::face(r, gobj, 0.0);
    }
    set(r, gobj, off::CEILING, 0);
    lib2725::clear_velocity(r.game, gobj);
    change_state(r, gobj, 4, change::ANIM_UPDATE);
}

/// Lets go and falls mouth open, `it_802DAA10` (and the inline `it_802DC4BC_ptr`, and the
/// knocked down's end on the ceiling): whoever it swallowed spat out, in the air, off the
/// ceiling (its box flipped back), its model upright, no countdown, stopped, state 5.
#[doc(alias = "it_802DAA10")]
pub fn let_go_fall(r: &mut Running, gobj: GObjId) {
    release(r, gobj, 1, true);
    maplib::enter_air(item_mut(r.game, gobj));
    if get(r, gobj, off::CEILING) == 1 {
        flip_box(r, gobj);
    }
    zako::face(r, gobj, 0.0);
    set(r, gobj, off::CEILING, 0);
    set(r, gobj, off::TIMER, 0);
    lib2725::clear_velocity(r.game, gobj);
    change_state(r, gobj, 5, change::ANIM_UPDATE);
}

/// The falls' animation and the states' that answer false (`itLikelike_UnkMotion5_Anim` and
/// its like).
#[doc(alias = "itLikelike_UnkMotion5_Anim")]
#[doc(alias = "itLikelike_UnkMotion3_Anim")]
#[doc(alias = "itLikelike_UnkMotion6_Anim")]
// Ports itLikelike_UnkMotion19_Anim; aliased once a check reaches it.
#[doc(alias = "itLikelike_UnkMotion21_Anim")]
pub fn answers_false(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The falls' physics, `itLikelike_UnkMotion5_Phys`: falling mouth open (5) a fighter in reach
/// is swallowed on the way; gravity.
#[doc(alias = "itLikelike_UnkMotion5_Phys")]
pub fn phys5(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).msid == 5 && in_reach(r, gobj) != 0 {
        swallow_falling(r, gobj);
        return;
    }
    phys19(r, gobj);
}

/// The fall's collision, `itLikelike_UnkMotion4_Coll`: landing, it waits.
#[doc(alias = "itLikelike_UnkMotion4_Coll")]
pub fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    ground::fall_to_floor(r, gobj, wait);
    zako::answers_false(r, gobj)
}

/// The fall mouth open's collision, `itLikelike_UnkMotion5_Coll`: landing, [`landed`].
#[doc(alias = "itLikelike_UnkMotion5_Coll")]
pub fn coll5(r: &mut Running, gobj: GObjId) -> bool {
    ground::fall_to_floor(r, gobj, landed);
    zako::answers_false(r, gobj)
}

/// Lands mouth open, `it_802DABC0`: off the ceiling, no countdown or target way, stopped, on the
/// ground, state 17.
#[doc(alias = "it_802DABC0")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    set(r, gobj, off::CEILING, 0);
    set(r, gobj, off::TIMER, 0);
    set(r, gobj, off::AIM, 0);
    lib2725::clear_velocity(r.game, gobj);
    maplib::enter_ground(item_mut(r.game, gobj));
    change_state(r, gobj, 0x11, change::ANIM_UPDATE);
}

/// Landing's animation, `itLikelike_UnkMotion17_Anim`: when it ends, it waits.
#[doc(alias = "itLikelike_UnkMotion17_Anim")]
pub fn anim17(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        ground::ground_probe(r, gobj);
        wait(r, gobj);
    }
    false
}

/// Landing's physics, `itLikelike_UnkMotion17_Phys`: along the floor.
#[doc(alias = "itLikelike_UnkMotion17_Phys")]
pub fn phys17(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    let (normal, facing) = (ip.x378_itemColl.floor.normal, ip.facing_dir);
    zako::along_surface(&mut ip.x40_vel, normal, facing);
    zako::align_floor(r, gobj, 0.0, 5.0);
}

/// Landing's collision, `itLikelike_UnkMotion17_Coll`: the grounded pass.
#[doc(alias = "itLikelike_UnkMotion17_Coll")]
pub fn coll17(r: &mut Running, gobj: GObjId) -> bool {
    ground::ground(r, gobj);
    zako::answers_false(r, gobj)
}

/// Rises to the ceiling, `it_802DAD18`: upside down (half a turn about z), its box turning with
/// its model, state 3.
#[doc(alias = "it_802DAD18")]
pub fn rise(r: &mut Running, gobj: GObjId) {
    let j = super::super::jobj_of(r.game, gobj).expect("a Like Like without a model");
    jobj::set_rotation_z(
        &mut r.game.objs.jobjs,
        j,
        mem(fp::frsp(std::f64::consts::PI)),
    );
    super::super::set_flag(&mut item_mut(r.game, gobj).xDCE_flag, 7, true);
    change_state(r, gobj, 3, change::ANIM_UPDATE);
}

/// Rising's physics, `itLikelike_UnkMotion3_Phys`: its fall speed up.
#[doc(alias = "itLikelike_UnkMotion3_Phys")]
pub fn phys3(r: &mut Running, gobj: GObjId) {
    let accel = reg(attr_of(item(r.game, gobj)).f32(r.data, data::attr::FALL_SPEED));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = mem(fp::fadds(reg(ip.x40_vel.y), accel));
}

/// Rising's collision, `itLikelike_UnkMotion3_Coll`: touching the ceiling, it clings there.
#[doc(alias = "itLikelike_UnkMotion3_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    ground::fall_until_touch_unstick(r, gobj, clung);
    zako::answers_false(r, gobj)
}

/// Pauses to look about, `it_802DAE6C`: stopped, state 16.
#[doc(alias = "it_802DAE6C")]
pub fn look_about(r: &mut Running, gobj: GObjId) {
    lib2725::clear_velocity(r.game, gobj);
    change_state(r, gobj, 0x10, change::ANIM_UPDATE);
}

/// Looking about's animation, `itLikelike_UnkMotion16_Anim`: when it ends, facing most fighters,
/// it crawls on.
#[doc(alias = "itLikelike_UnkMotion16_Anim")]
pub fn anim16(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        let pos = item(r.game, gobj).pos;
        let facing = owner::facing_for(r.game, r.data, pos);
        item_mut(r.game, gobj).facing_dir = facing;
        if get(r, gobj, off::CEILING) == 0 {
            start_crawl(r, gobj, 1);
        } else {
            start_crawl(r, gobj, 2);
        }
    }
    false
}

/// The states' physics that do nothing (`itLikelike_UnkMotion16_Phys` and its like).
#[doc(alias = "itLikelike_UnkMotion16_Phys")]
#[doc(alias = "itLikelike_UnkMotion9_Phys")]
#[doc(alias = "itLikelike_UnkMotion10_Phys")]
#[doc(alias = "itLikelike_UnkMotion14_Phys")]
#[doc(alias = "itLikelike_UnkMotion15_Phys")]
// Ports itLikelike_UnkMotion18_Phys; aliased once a check reaches it.
#[doc(alias = "itLikelike_UnkMotion20_Phys")]
pub fn still(_: &mut Running, _: GObjId) {}

/// Looking about's collision, `itLikelike_UnkMotion16_Coll`: on the floor, off it it lets go and
/// falls; on the ceiling, the ceiling's pass.
#[doc(alias = "itLikelike_UnkMotion16_Coll")]
pub fn coll16(r: &mut Running, gobj: GObjId) -> bool {
    if get(r, gobj, off::CEILING) == 0 {
        ground::ground_or(r, gobj, let_go_fall);
    } else {
        let _ = cling(r, gobj);
    }
    zako::answers_false(r, gobj)
}

/// Lunges at its target, `it_802DB074`: facing it, at twice its speed, for 30 frames, state 8.
#[doc(alias = "it_802DB074")]
pub fn lunge(r: &mut Running, gobj: GObjId) {
    let aim = get(r, gobj, off::AIM);
    let s = reg(speed(r, gobj));
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = mem(fp::frsp(f64::from(aim)));
    ip.x40_vel.x = mem(fp::fmuls(2.0, fp::fmuls(reg(ip.facing_dir), s)));
    set(r, gobj, off::TIMER, 0x1E);
    change_state(r, gobj, 8, change::ANIM_UPDATE);
}

/// Lunging's animation, `itLikelike_UnkMotion8_Anim`: looped.
#[doc(alias = "itLikelike_UnkMotion8_Anim")]
pub fn anim8(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 8, change::ANIM_UPDATE);
    }
    false
}

/// Lunging's physics, `itLikelike_UnkMotion8_Phys`: its 30 frames done (or without a target, a
/// 120-frame wait) it waits, its target forgotten; within 15 across of its target, its
/// collision no longer an enemy's, it swallows forward (to the right) or the other way; along
/// the ceiling.
#[doc(alias = "itLikelike_UnkMotion8_Phys")]
pub fn phys8(r: &mut Running, gobj: GObjId) {
    let t = get(r, gobj, off::TIMER);
    if t == 0 {
        set_target(r, gobj, None);
        wait(r, gobj);
        return;
    }
    set(r, gobj, off::TIMER, t.wrapping_sub(1));
    match target(r, gobj) {
        Some(f) => {
            let at = r.game.ft_pos(f);
            let pos = item(r.game, gobj).pos;
            if abs(fp::fsubs(reg(pos.x), reg(at.x))) < 15.0 {
                zako::clear_dd0_b4(r.game, gobj);
                if fp::frsp(f64::from(get(r, gobj, off::AIM))) == 1.0 {
                    swallow_right(r, gobj);
                    return;
                }
                swallow_left(r, gobj);
                return;
            }
        }
        None => {
            set(r, gobj, off::TIMER, 0x78);
            wait(r, gobj);
            return;
        }
    }
    let ip = item_mut(r.game, gobj);
    let (normal, facing) = (ip.x378_itemColl.floor.normal, ip.facing_dir);
    zako::along_surface(&mut ip.x40_vel, normal, facing);
    zako::align_ceiling(r, gobj, 0.0, 5.0);
}

/// The swallow takes its victim, `it_802DB358` (the grab's callback on the Like Like): its
/// target the fighter it grabbed, done looking, the fighter hidden behind it (`xD09`).
#[doc(alias = "it_802DB358")]
pub fn swallowed(r: &mut Running, gobj: GObjId) {
    let victim = item(r.game, gobj).grab_victim;
    set_target(r, gobj, victim);
    super::super::set_flag(&mut item_mut(r.game, gobj).xDD0_flag, 5, false);
    lib2725::set_xd09(r.game, gobj, true);
}

/// Arms the swallow (`it_80274F28`): fighters with `x1A6A`'s first bit pass under it.
fn arm(r: &mut Running, gobj: GObjId) {
    lib2725::arm_grab(
        item_mut(r.game, gobj),
        GRAB_MASK,
        CodeRef(SWALLOWED),
        CodeRef(FT_CAPTURE),
    );
}

/// Swallows falling, `it_802DB398`: facing its target, no target, its swallow's sound (0x139),
/// its collision no longer an enemy's, state 11 (facing right) or 12, armed.
// Ports it_802DB398; aliased once a check reaches it.
pub fn swallow_falling(r: &mut Running, gobj: GObjId) {
    let aim = get(r, gobj, off::AIM);
    item_mut(r.game, gobj).facing_dir = mem(fp::frsp(f64::from(aim)));
    set_target(r, gobj, None);
    play_sfx(r, gobj, 0x139, 0x7F, 0x40);
    zako::clear_dd0_b4(r.game, gobj);
    if reg(item(r.game, gobj).facing_dir) == 1.0 {
        change_state(r, gobj, 0xB, change::ANIM_UPDATE);
    } else {
        change_state(r, gobj, 0xC, change::ANIM_UPDATE);
    }
    arm(r, gobj);
}

/// The swallows' end (`itLikelike_UnkMotion12_Anim`, `_9_Anim`, `_10_Anim`): when the animation
/// ends, with a fighter inside it chews, else it waits.
// Ports itLikelike_UnkMotion12_Anim; aliased once a check reaches it.
#[doc(alias = "itLikelike_UnkMotion9_Anim")]
#[doc(alias = "itLikelike_UnkMotion10_Anim")]
pub fn anim_swallow(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        ground::ground_probe(r, gobj);
        if target(r, gobj).is_some() {
            chew(r, gobj);
        } else {
            wait(r, gobj);
        }
    }
    false
}

/// The falling swallow's physics, `itLikelike_UnkMotion12_Phys`: in the air gravity, on the
/// ground along the floor.
// Ports itLikelike_UnkMotion12_Phys; aliased once a check reaches it.
pub fn phys12(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).ground_or_air == GA_AIR {
        phys19(r, gobj);
        return;
    }
    phys17(r, gobj);
}

/// The falling swallow's collision, `itLikelike_UnkMotion12_Coll`: in the air, landing it stops
/// on the ground; on the ground, off it it lets go and falls.
// Ports itLikelike_UnkMotion12_Coll; aliased once a check reaches it.
pub fn coll12(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).ground_or_air == GA_AIR {
        if ground::air(r, gobj) {
            maplib::enter_ground(item_mut(r.game, gobj));
            lib2725::clear_velocity(r.game, gobj);
        }
    } else {
        ground::ground_or(r, gobj, let_go_fall);
    }
    zako::answers_false(r, gobj)
}

/// Swallows to the right, `it_802DB5F0`: stopped, state 9, its swallow's sound, armed, no
/// target.
#[doc(alias = "it_802DB5F0")]
pub fn swallow_right(r: &mut Running, gobj: GObjId) {
    swallow_with(r, gobj, 9);
}

/// Swallows to the left, `it_802DB74C`: as [`swallow_right`], state 10.
#[doc(alias = "it_802DB74C")]
pub fn swallow_left(r: &mut Running, gobj: GObjId) {
    swallow_with(r, gobj, 0xA);
}

fn swallow_with(r: &mut Running, gobj: GObjId, msid: i32) {
    lib2725::clear_velocity(r.game, gobj);
    change_state(r, gobj, msid, change::ANIM_UPDATE);
    play_sfx(r, gobj, 0x139, 0x7F, 0x40);
    arm(r, gobj);
    set_target(r, gobj, None);
}

/// The collision of the states it stands on the floor in (`itLikelike_UnkMotion9_Coll`, `_10`,
/// `_13`, `_14`, `_15`): off the floor it lets go and falls.
#[doc(alias = "itLikelike_UnkMotion9_Coll")]
#[doc(alias = "itLikelike_UnkMotion10_Coll")]
#[doc(alias = "itLikelike_UnkMotion13_Coll")]
#[doc(alias = "itLikelike_UnkMotion14_Coll")]
#[doc(alias = "itLikelike_UnkMotion15_Coll")]
pub fn coll_ground_or_let_go(r: &mut Running, gobj: GObjId) -> bool {
    ground::ground_or(r, gobj, let_go_fall);
    zako::answers_false(r, gobj)
}

/// Chews, `it_802DB8A8`: stopped, no countdown, its chewing's sound (0x13B), its collision no
/// longer an enemy's, state 13.
#[doc(alias = "it_802DB8A8")]
pub fn chew(r: &mut Running, gobj: GObjId) {
    lib2725::clear_velocity(r.game, gobj);
    set(r, gobj, off::TIMER, fp::fctiwz(0.0));
    play_sfx(r, gobj, 0x13B, 0x7F, 0x40);
    zako::clear_dd0_b4(r.game, gobj);
    change_state(r, gobj, 0xD, change::ANIM_UPDATE);
}

/// Chewing's animation, `itLikelike_UnkMotion13_Anim`: looped.
#[doc(alias = "itLikelike_UnkMotion13_Anim")]
pub fn anim13(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 0xD, change::ANIM_UPDATE);
    }
    false
}

/// Chewing's physics, `itLikelike_UnkMotion13_Phys`: every `x3D` frames a bite of `x3E`
/// (`ftCo_800C7C60`).
#[doc(alias = "itLikelike_UnkMotion13_Phys")]
pub fn phys13(r: &mut Running, gobj: GObjId) {
    let t = get(r, gobj, off::TIMER);
    if t == 0 {
        let n = attr_s8(r, gobj, 0x3D);
        set(r, gobj, off::TIMER, n);
        let victim = item(r.game, gobj)
            .grab_victim
            .expect("a Like Like chewing on no fighter");
        let bite = attr_s8(r, gobj, 0x3E);
        r.likelike_bite(victim, bite);
    } else {
        set(r, gobj, off::TIMER, t.wrapping_sub(1));
    }
}

/// Spits its fighter out, `it_802DB9F4` (the mash timer's end): stopped, no countdown, state 14
/// or 15 by a draw.
#[doc(alias = "it_802DB9F4")]
pub fn spit(r: &mut Running, gobj: GObjId) {
    lib2725::clear_velocity(r.game, gobj);
    set(r, gobj, off::TIMER, fp::fctiwz(0.0));
    if r.game.objs.rng.below(2) != 0 {
        change_state(r, gobj, 0xE, change::ANIM_UPDATE);
        return;
    }
    change_state(r, gobj, 0xF, change::ANIM_UPDATE);
}

/// Lets go of its fighter (hit or KO'd), `it_802DBA68`: stopped, its fighter spat out straight,
/// a 120-frame countdown, and it waits.
// Ports it_802DBA68; aliased once a check reaches it.
pub fn let_go(r: &mut Running, gobj: GObjId) {
    lib2725::clear_velocity(r.game, gobj);
    release(r, gobj, 0, true);
    set(r, gobj, off::TIMER, 0x78);
    wait(r, gobj);
}

/// Spits out the fighter it swallowed, `it_802DBAF0`, unless lunging (8): if it holds its
/// target, the fighter moved from its middle to its mouth (bone 8) and given the attributes'
/// hit (`x40[1]` with `hit_1`, else `x40[0]`) at an angle of 0 (`way` 0), `x30` (-1) or `x34`
/// (`ftCo_800C7B0C`), then let go, its spit's sound (0x13A).
#[doc(alias = "it_802DBAF0")]
pub fn release(r: &mut Running, gobj: GObjId, way: i32, hit_1: bool) {
    let ip = item(r.game, gobj);
    if ip.msid == 8 {
        return;
    }
    let Some(victim) = ip.grab_victim else {
        return;
    };
    if Some(victim) != target(r, gobj) {
        return;
    }
    let ip = item(r.game, gobj);
    let cd = &ip.x378_itemColl;
    let mid = mem(fp::fmuls(
        0.5,
        fp::fadds(reg(cd.ecb.top.y), reg(cd.ecb.bottom.y)),
    ));
    let pos = ip.pos;
    let from = Vec3::new(
        mem(fp::fadds(0.0, reg(pos.x))),
        mem(fp::fadds(reg(mid), reg(pos.y))),
        mem(fp::fadds(0.0, reg(pos.z))),
    );
    let bone = lib2725::bone(r.game, r.data, gobj, 8);
    let at = crate::lb::joint::world_position(&mut r.game.objs, r.data, bone, None);
    let angle = if way == 0 {
        0.0
    } else if fp::frsp(f64::from(way)) == fp::fneg(1.0) {
        attr_f32(r, gobj, 0x30)
    } else {
        attr_f32(r, gobj, 0x34)
    };
    let desc = attr(r, gobj, if hit_1 { 0x64 } else { 0x40 });
    r.likelike_spat_out(victim, at, from, desc, angle);
    set_target(r, gobj, None);
    let ip = item_mut(r.game, gobj);
    ip.atk_victim = None;
    ip.grab_victim = None;
    play_sfx(r, gobj, 0x13A, 0x7F, 0x40);
}

/// The spits' animation (`itLikelike_UnkMotion14_Anim`, `_15_Anim`): at the countdown's `x38`th
/// frame the fighter is spat out (to the side, `way` 1 or -1, with `x40[0]`'s hit); at 40 a
/// 120-frame countdown, and it waits.
fn anim_spit(r: &mut Running, gobj: GObjId, way: i32) -> bool {
    let at = attr_f32(r, gobj, 0x38);
    if fp::frsp(f64::from(get(r, gobj, off::TIMER))) == reg(at) {
        release(r, gobj, way, false);
    }
    let t = get(r, gobj, off::TIMER);
    if t >= 0x28 {
        set(r, gobj, off::TIMER, 0x78);
        wait(r, gobj);
    } else {
        set(r, gobj, off::TIMER, t.wrapping_add(1));
    }
    false
}

/// `itLikelike_UnkMotion14_Anim`: [`anim_spit`] one way.
#[doc(alias = "itLikelike_UnkMotion14_Anim")]
pub fn anim14(r: &mut Running, gobj: GObjId) -> bool {
    anim_spit(r, gobj, 1)
}

/// `itLikelike_UnkMotion15_Anim`: [`anim_spit`] the other.
#[doc(alias = "itLikelike_UnkMotion15_Anim")]
pub fn anim15(r: &mut Running, gobj: GObjId) -> bool {
    anim_spit(r, gobj, -1)
}

/// Picked up, `itLikeLike_Logic5_PickedUp`: its fighter spat out, state 18.
// Ports itLikeLike_Logic5_PickedUp; aliased once a check reaches it.
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    release(r, gobj, 0, true);
    change_state(r, gobj, 0x12, change::ANIM_UPDATE);
}

/// Held's animation, `itLikelike_UnkMotion18_Anim`: looped, spitting its fighter out.
// Ports itLikelike_UnkMotion18_Anim; aliased once a check reaches it.
pub fn anim18(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        picked_up(r, gobj);
    }
    false
}

/// Dropped, `it_2725_Logic5_Dropped`: the enemies' drop, a countdown of `x4`, then it falls.
// Ports it_2725_Logic5_Dropped; aliased once a check reaches it.
pub fn dropped(r: &mut Running, gobj: GObjId) {
    zako::dropped(r, gobj);
    let n = attr_i32(r, gobj, 4);
    set(r, gobj, off::TIMER, n);
    fall(r, gobj);
}

/// Thrown, `itLikeLike_Logic5_Thrown`: the enemies' throw, no countdown, state 19.
// Ports itLikeLike_Logic5_Thrown; aliased once a check reaches it.
pub fn thrown(r: &mut Running, gobj: GObjId) {
    zako::thrown(r, gobj);
    set(r, gobj, off::TIMER, 0);
    change_state(r, gobj, 0x13, change::ANIM_UPDATE);
}

/// The falls' gravity (`itLikelike_UnkMotion19_Phys`, and the falls' and the airborne swallow's).
// Ports itLikelike_UnkMotion19_Phys; aliased once a check reaches it.
pub fn phys19(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, data::attr::FALL_SPEED);
    let max = a.f32(r.data, data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
}

/// Thrown's collision, `itLikelike_UnkMotion19_Coll`: touching the stage ends the flight as a
/// KO.
// Ports itLikelike_UnkMotion19_Coll; aliased once a check reaches it.
pub fn coll19(r: &mut Running, gobj: GObjId) -> bool {
    zako::flight_lands(r, gobj, None)
}

/// Knocked down, `it_802DC0AC`: stopped, a countdown of `x4`, no target way, state 6.
#[doc(alias = "it_802DC0AC")]
pub fn knocked_down(r: &mut Running, gobj: GObjId) {
    lib2725::clear_velocity(r.game, gobj);
    let n = attr_i32(r, gobj, 4);
    set(r, gobj, off::TIMER, n);
    set(r, gobj, off::AIM, 0);
    change_state(r, gobj, 6, change::ANIM_UPDATE);
}

/// Knocked down's physics, `itLikelike_UnkMotion6_Phys`: the countdown done it crawls, or on the
/// ceiling lets go and falls (the inline `it_802DAA10`).
#[doc(alias = "itLikelike_UnkMotion6_Phys")]
pub fn phys6(r: &mut Running, gobj: GObjId) {
    let t = get(r, gobj, off::TIMER);
    if t == 0 {
        if get(r, gobj, off::CEILING) == 0 {
            crawl(r, gobj);
            return;
        }
        let_go_fall(r, gobj);
        return;
    }
    set(r, gobj, off::TIMER, t.wrapping_sub(1));
}

/// Flung away, `it_802DC310`: in the air, launched along its knockback, `x18` cleared, state 20.
#[doc(alias = "it_802DC310")]
pub fn fling(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    zako::launch(r, gobj, false);
    set(r, gobj, off::X18, 0);
    change_state(r, gobj, 0x14, change::ANIM_UPDATE);
}

/// Flung away's animation, `itLikelike_UnkMotion20_Anim`: looped.
#[doc(alias = "itLikelike_UnkMotion20_Anim")]
pub fn anim20(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 0x14, change::ANIM_UPDATE);
    }
    false
}

/// Flung away's collision, `itLikelike_UnkMotion20_Coll`: bounces off the stage.
#[doc(alias = "itLikelike_UnkMotion20_Coll")]
pub fn coll20(r: &mut Running, gobj: GObjId) -> bool {
    zako::bounce(r, gobj)
}

/// Flung toward the screen, `it_802DC3DC` (the inline `Item_UpdateZakoVelocity`): state 21.
#[doc(alias = "it_802DC3DC")]
pub fn fling_to_screen(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    let v = zako::toward_camera(r, gobj);
    item_mut(r.game, gobj).x40_vel = v;
    maplib::enter_air(item_mut(r.game, gobj));
    change_state(r, gobj, 0x15, UNK_0X1 | change::ANIM_UPDATE);
}

/// Flung toward the screen's physics, `itLikelike_UnkMotion21_Phys`: its fall speed off the
/// vertical velocity.
#[doc(alias = "itLikelike_UnkMotion21_Phys")]
pub fn phys21(r: &mut Running, gobj: GObjId) {
    let accel = reg(attr_of(item(r.game, gobj)).f32(r.data, data::attr::FALL_SPEED));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = mem(fp::fsubs(reg(ip.x40_vel.y), accel));
}

/// Flung toward the screen's collision, `itLikelike_UnkMotion21_Coll`: nothing.
#[doc(alias = "itLikelike_UnkMotion21_Coll")]
pub fn coll21(r: &mut Running, gobj: GObjId) -> bool {
    zako::answers_false(r, gobj)
}

/// Destroyed, `itLikeLike_Logic5_Destroyed`: its fighter spat out, then the enemies' end.
#[doc(alias = "itLikeLike_Logic5_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    release(r, gobj, 0, true);
    zako::destroyed(r, gobj);
}

/// A Like Like an enemy spawner makes at `pos`, `it_802DC4BC`: on the ceiling for `ceiling` 1
/// (rising to it), else falling mouth open.
#[doc(alias = "it_802DC4BC")]
pub fn spawn(r: &mut Running, ceiling: i32, pos: Vec3) -> Option<GObjId> {
    let zero = Vec3::new(0.0, 0.0, 0.0);
    let gobj = zako::spawn(r, super::LIKELIKE, Some(pos), None, Some(zero), true)?;
    set(r, gobj, off::CEILING, ceiling);
    maplib::enter_air(item_mut(r.game, gobj));
    if ceiling == 0 {
        let_go_fall(r, gobj);
    } else {
        rise(r, gobj);
    }
    Some(gobj)
}
