//! Birdo's eggs, `melee/it/kinds/itkyasarinegg.c` (`It_Kind_Kyasarin_Egg`): spat out ahead
//! of her (`it::kinds::kyasarin`), an egg rolls along (state 0), bouncing back up off what it
//! meets (1). It can be picked up (2) and thrown (3); a thrown egg that lands or hits something
//! breaks with an effect and a sound, dropping items or, one time in its attributes' `x10`,
//! fading away empty (4).
//!
//! Its variables share the union's first words with the enemies' common member
//! (`itZako_ItemVars`, which `it::zako`'s spawn writes), so [`KyasarinEggVars`] holds both.

use gekko_fp as fp;

use super::super::ground::{air, air_touched_any, air_touches};
use super::super::item::{article, change, change_state, play_sfx};
use super::super::{drop, hitbox, item, item_mut, lib26b1, lib2725, set_flag};
use super::falling_phys;
use crate::fpu::{mem, reg};
use crate::game::ef;
use crate::game::mirror::{Cx, Mem, Mirror};
use crate::game::state::{Game, Running};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;
use crate::vec::Vec3;

use super::super::Var;

pub const PICKED_UP: u32 = 0x802E_FBC0;
pub const DROPPED: u32 = 0x802E_FBE8;
pub const THROWN: u32 = 0x802E_FC08;
pub const DMG_DEALT: u32 = 0x802E_FE28;
pub const CLANKED: u32 = 0x802E_FF1C;
pub const HIT_SHIELD: u32 = 0x802F_0010;
pub const REFLECTED: u32 = 0x802F_0104;
pub const SHIELD_BOUNCED: u32 = 0x802F_01F8;
pub const DMG_RECEIVED: u32 = 0x802F_022C;
pub const COLL0: u32 = 0x802E_FB80;
pub const PHYS1: u32 = 0x802E_FD18;
pub const COLL1: u32 = 0x802E_FD64;
pub const PHYS3: u32 = 0x802E_FC30;
pub const COLL3: u32 = 0x802E_FC7C;
pub const ANIM4: u32 = 0x802E_FE08;

/// `itKyasarinEggAttributes`' fields, by offset.
mod a {
    /// Its rolling speed, and its bounce's upward speed.
    pub const SPEED: u32 = 0x4;
    pub const BOUNCE: u32 = 0x8;
    /// The items a broken egg drops, and the chance (one in it) it holds none (integers).
    pub const ITEMS: u32 = 0xC;
    pub const EMPTY: u32 = 0x10;
}

/// `itKyasarinEgg_ItemVars` over `itZako_ItemVars`: the enemies' common words first, then
/// the egg's own. Offsets are from `xDD4`.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct KyasarinEggVars {
    /// +0x0, `zako.jobj`.
    pub jobj: Var<Option<JObjId>>,
    /// +0x4, `zako.idx`.
    pub idx: Var<i32>,
    /// +0x8, `zako.x8`.
    pub x8: Var<Vec3>,
    /// +0x14, `zako.x14`.
    pub x14: [Var<i32>; 3],
    /// +0x20: whether it is broken empty.
    pub x20: Var<i32>,
}

impl KyasarinEggVars {
    /// Marks every field stale, as a new item finds its variables.
    pub fn mark_stale(&mut self) {
        self.jobj.mark_stale();
        self.idx.mark_stale();
        self.x8.mark_stale();
        for v in &mut self.x14 {
            v.mark_stale();
        }
        self.x20.mark_stale();
    }
}

impl Mirror for KyasarinEggVars {
    const SIZE: u32 = 0x24;

    fn import(m: &Mem, at: u32, cx: &mut dyn Cx) -> Self {
        let f = |a: u32| f32::from_bits(m.u32(a));
        let i = |a: u32| Var::new(m.u32(a) as i32);
        KyasarinEggVars {
            jobj: Var::new(cx.import_handle("JObjId", m.u32(at)).map(JObjId)),
            idx: i(at + 4),
            x8: Var::new(Vec3::new(f(at + 8), f(at + 0xC), f(at + 0x10))),
            x14: std::array::from_fn(|k| i(at + 0x14 + 4 * k as u32)),
            x20: i(at + 0x20),
        }
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        let p = |n: &str| format!("{path}.{n}");
        if self.jobj.live(&p("jobj"), cx) {
            let ours = self.jobj.peek().map(|j| j.0);
            cx.compare_handle(&p("jobj"), "JObjId", ours, m.u32(at));
        }
        let ints: [(&str, &Var<i32>, u32); 5] = [
            ("idx", &self.idx, 4),
            ("x14.x", &self.x14[0], 0x14),
            ("x14.y", &self.x14[1], 0x18),
            ("x14.z", &self.x14[2], 0x1C),
            ("x20", &self.x20, 0x20),
        ];
        for (name, v, off) in ints {
            if v.live(&p(name), cx) && *v.peek() != m.u32(at + off) as i32 {
                cx.mismatch(
                    &p(name),
                    format!("{:?}", v.peek()),
                    format!("{:?}", m.u32(at + off) as i32),
                );
            }
        }
        self.x8.compare(m, at + 8, &p("x8"), cx);
    }
}

fn vars(game: &mut Game, gobj: GObjId) -> &mut KyasarinEggVars {
    item_mut(game, gobj).xDD4_itemVar.kyasarin_egg()
}

fn attr(r: &Running, gobj: GObjId, off: u32) -> melee_data::archive::DataRef {
    article(item(r.game, gobj))
        .special(r.data)
        .expect("Birdo's egg without attributes")
        .at(off)
}

/// Runs one of the egg's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        COLL0 => coll0(r, gobj),
        COLL1 => coll1(r, gobj),
        COLL3 => coll3(r, gobj),
        ANIM4 => anim4(r, gobj),
        DMG_DEALT | CLANKED | HIT_SHIELD | REFLECTED | DMG_RECEIVED => broken(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        _ => return None,
    })
}

/// Runs one of the egg's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PICKED_UP => picked_up(r, gobj),
        DROPPED | THROWN => thrown(r, gobj),
        PHYS1 | PHYS3 => falling(r, gobj),
        _ => return false,
    }
    true
}

/// An egg from Birdo `parent`, `it_802EFA44`: an enemy at `pos` in the air facing `dir`, its
/// collision facing the same way, not broken, its color overlay cleared, its hurtbox on
/// unless it must blink, rolling.
#[doc(alias = "it_802EFA44")]
pub fn spawn(r: &mut Running, parent: Option<GObjId>, pos: Vec3, dir: f32) {
    if parent.is_none() {
        return;
    }
    let Some(egg) = super::super::zako::spawn(r, super::KYASARIN_EGG, Some(pos), None, None, true)
    else {
        return;
    };
    let ip = item_mut(r.game, egg);
    ip.facing_dir = dir;
    let facing = if ip.facing_dir == -1.0 { -1 } else { 1 };
    ip.x378_itemColl.set_facing_dir(facing);
    set_flag(&mut ip.xDD0_flag, 6, false);
    ip.xD40 = 0.0;
    vars(r.game, egg).x20.set(0);
    let ip = item_mut(r.game, egg);
    super::super::anim::clear_overlay(ip);
    if !super::super::flag(ip.xDD0_flag, 7) {
        hitbox::end_blink(r.game, egg);
    }
    roll(r, egg);
}

/// Rolling, `it_802EFB0C`: able to be picked up, at its speed as it faces, state 0.
#[doc(alias = "it_802EFB0C")]
pub fn roll(r: &mut Running, gobj: GObjId) {
    lib26b1::enable_pickup(r.game, gobj);
    let speed = reg(r.data.f32(attr(r, gobj, a::SPEED)));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(speed, reg(ip.facing_dir)));
    ip.x40_vel.z = 0.0;
    ip.x40_vel.y = 0.0;
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// Rolling's collision, `itKyasarinegg_UnkMotion0_Coll`: touching anything, it bounces.
#[doc(alias = "itKyasarinegg_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    if air_touches(r, gobj) & 0xF != 0 {
        bounce(r, gobj);
    }
    false
}

/// Held, `itKyasarinEgg_Logic28_PickedUp`: state 2.
#[doc(alias = "itKyasarinEgg_Logic28_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// Let go of or thrown, `itKyasarinEgg_Logic28_Dropped` and `itKyasarinEgg_Logic28_Thrown`:
/// state 3.
#[doc(alias = "itKyasarinEgg_Logic28_Dropped")]
#[doc(alias = "itKyasarinEgg_Logic28_Thrown")]
pub fn thrown(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 3, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// Bouncing's and thrown's physics, `itKyasarinegg_UnkMotion1_Phys` and
/// `itKyasarinegg_UnkMotion3_Phys`: gravity (`Item_ApplyFallingPhysics`).
#[doc(alias = "itKyasarinegg_UnkMotion1_Phys")]
#[doc(alias = "itKyasarinegg_UnkMotion3_Phys")]
pub fn falling(r: &mut Running, gobj: GObjId) {
    falling_phys(r, gobj);
}

/// Thrown's collision, `itKyasarinegg_UnkMotion3_Coll`: touching the stage, it breaks.
#[doc(alias = "itKyasarinegg_UnkMotion3_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    if air(r, gobj) {
        return broken(r, gobj);
    }
    false
}

/// Bouncing, `it_802EFCC0`: state 1, turned back, going up at its bounce's speed.
#[doc(alias = "it_802EFCC0")]
pub fn bounce(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    let up = r.data.f32(attr(r, gobj, a::BOUNCE));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fneg(reg(ip.x40_vel.x)));
    ip.x40_vel.y = up;
}

/// Bouncing's collision, `itKyasarinegg_UnkMotion1_Coll`: whether it touched anything (then
/// it's gone).
#[doc(alias = "itKyasarinegg_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    air_touched_any(r, gobj)
}

/// Broken empty, `it_802EFD84`: hidden with a blast effect, its hurtbox blinking, no longer
/// to be picked up, fading out, still, state 4.
#[doc(alias = "it_802EFD84")]
pub fn break_empty(r: &mut Running, gobj: GObjId) {
    lib26b1::hide(r.game, gobj);
    lib2725::blast_effect(r, gobj);
    hitbox::start_blink(r.game, gobj);
    lib26b1::disable_pickup(r.game, gobj);
    lib26b1::set_xdd0_b3(r.game, gobj);
    lib2725::start_vanishing(r.game, r.data, gobj);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = 0.0;
    ip.x40_vel.y = 0.0;
    vars(r.game, gobj).x20.set(1);
    change_state(r, gobj, 4, change::ANIM_UPDATE);
}

/// Fading's animation, `itKyasarinegg_UnkMotion4_Anim`: its life counts down (`it_802751D8`).
/// The C returns nothing; the machine code leaves the count's answer (whether its life ran
/// out) in r3, which the caller takes as its own.
#[doc(alias = "itKyasarinegg_UnkMotion4_Anim")]
pub fn anim4(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::tick_life(r.game, gobj)
}

/// A hit dealt, a clank, a shield hit, a reflection or a hit taken (`it_damage_inline` in
/// each): unless broken already, a rolling or bouncing egg bounces; a thrown one breaks with
/// its effect and sound, empty one time in its attributes' `x10` (staying), else dropping
/// its items (and going).
#[doc(alias = "it_2725_Logic28_DmgDealt")]
#[doc(alias = "it_2725_Logic28_Clanked")]
#[doc(alias = "it_2725_Logic28_HitShield")]
#[doc(alias = "it_2725_Logic28_Reflected")]
#[doc(alias = "it_802F022C")]
pub fn broken(r: &mut Running, gobj: GObjId) -> bool {
    if vars(r.game, gobj).x20.get() != 0 {
        return false;
    }
    if item(r.game, gobj).msid != 3 {
        bounce(r, gobj);
        return false;
    }
    let pos = item(r.game, gobj).pos;
    ef::spawn(r, 0x4D0, Some(gobj), &[ef::Arg::Vec(pos)]);
    play_sfx(r, gobj, 244, 127, 64);
    let empty = r.data.u32(attr(r, gobj, a::EMPTY)) as i32;
    if r.game.objs.rng.below(empty) == 0 {
        break_empty(r, gobj);
        return false;
    }
    let n = r.data.u32(attr(r, gobj, a::ITEMS)) as i32;
    drop::drop_contents(r, gobj, n, false);
    true
}

/// Bounced off a shield, `itKyasarinEgg_Logic28_ShieldBounced`: a thrown egg bounces off it
/// (`itColl_BounceOffShield`).
#[doc(alias = "itKyasarinEgg_Logic28_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).msid == 3 {
        lib2725::bounce_off_shield(r.game, gobj);
    }
    false
}
