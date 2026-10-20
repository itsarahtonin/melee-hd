//! Whispy's apples, `melee/it/kinds/itwhispyapple.c`: the apples Green Greens' Whispy Woods
//! drops (`gr::greens`), plain (`It_Kind_WhispyApple`) or healing (`It_Kind_WhispyHealApple`)
//! when healing items are on. Each falls from behind the tree toward the stage, bounces with a
//! sound and lies there until its life runs out; some can't be picked up and burst where they
//! land (state 7). Picked up, a healing apple heals (the fighter's side, `ftpickupitem`).

use gekko_fp as fp;

use super::super::Var;
use super::super::ground::{air_touches, fall, ground_or_fall, ground_skid, try_land};
use super::super::item::{article, attr_of, change, change_state, common_data, play_sfx};
use super::super::{data, hitbox, item, item_mut, jobj_of, lib26b1, lib2725, maplib, release};
use crate::fpu::{mem, reg};
use crate::game::mirror::{Cx, Mem, Mirror};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

use super::{WHISPY_APPLE, WHISPY_HEAL_APPLE};

pub const ANIM0: u32 = 0x802E_E434;
pub const PHYS0: u32 = 0x802E_E544;
pub const COLL0: u32 = 0x802E_E5A0;
pub const LANDED: u32 = 0x802E_E6A0;
pub const ANIM1: u32 = 0x802E_E6DC;
pub const PHYS1: u32 = 0x802E_E7CC;
pub const COLL1: u32 = 0x802E_E7D0;
pub const FELL: u32 = 0x802E_E7FC;
pub const ANIM5: u32 = 0x802E_E824;
pub const PHYS5: u32 = 0x802E_E914;
pub const COLL5: u32 = 0x802E_E944;
pub const PICKED_UP: u32 = 0x802E_EA08;
pub const ANIM3: u32 = 0x802E_EA64;
pub const PHYS3: u32 = 0x802E_EA6C;
pub const DROPPED: u32 = 0x802E_EA70;
pub const THROWN: u32 = 0x802E_EB28;
pub const ENTERED_AIR: u32 = 0x802E_EBB0;
pub const ANIM6: u32 = 0x802E_EBD8;
pub const PHYS6: u32 = 0x802E_ECC8;
pub const COLL6: u32 = 0x802E_ECCC;
pub const BURST: u32 = 0x802E_ED00;
pub const ANIM7: u32 = 0x802E_EDD4;
pub const PHYS7: u32 = 0x802E_EEC4;
pub const COLL7: u32 = 0x802E_EEC8;
pub const DMG_DEALT: u32 = 0x802E_EED0;
pub const CLANKED: u32 = 0x802E_EED8;
pub const REFLECTED: u32 = 0x802E_EF10;
pub const HIT_SHIELD: u32 = 0x802E_EF30;
pub const SHIELD_BOUNCED: u32 = 0x802E_EF68;

/// The bounce's sounds, one drawn at random (`it_803F9250`).
const BOUNCE_SFX: [i32; 3] = [0x68FB4, 0x68FB5, 0x68FB6];

/// Runs one of the apples' predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 => anim0(r, gobj),
        ANIM1 | ANIM5 | ANIM6 | ANIM7 => anim_age(r, gobj),
        ANIM3 => held(r, gobj),
        COLL0 => coll0(r, gobj),
        COLL1 => coll1(r, gobj),
        COLL5 => coll5(r, gobj),
        COLL6 => coll6(r, gobj),
        COLL7 => coll7(r, gobj),
        DMG_DEALT => dealt(r, gobj),
        CLANKED | HIT_SHIELD => bounce_off(r, gobj),
        REFLECTED => reflected(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        _ => return None,
    })
}

/// Runs one of the apples' events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PHYS0 => phys0(r, gobj),
        PHYS5 => gravity(r, gobj),
        PHYS1 | PHYS3 | PHYS6 | PHYS7 => idle(r, gobj),
        LANDED => landed(r, gobj),
        FELL => fell(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED => dropped(r, gobj),
        THROWN => thrown(r, gobj),
        ENTERED_AIR => entered_air(r, gobj),
        BURST => burst(r, gobj),
        _ => return false,
    }
    true
}

/// `itWhispyApple_ItemVars`: the heal (`xDD4`) and whether the apple can't be picked up and
/// bursts where it lands (`xDD8`, 1).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct WhispyAppleVars {
    pub heal: Var<i32>,
    pub xdd8: Var<i32>,
}

impl WhispyAppleVars {
    /// Marks every field stale, as a new item finds its variables.
    pub fn mark_stale(&mut self) {
        self.heal.mark_stale();
        self.xdd8.mark_stale();
    }
}

impl Mirror for WhispyAppleVars {
    const SIZE: u32 = 8;

    fn import(m: &Mem, at: u32, _: &mut dyn Cx) -> Self {
        WhispyAppleVars {
            heal: Var::new(m.u32(at) as i32),
            xdd8: Var::new(m.u32(at + 4) as i32),
        }
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        for (name, v, off) in [("xDD4_heal", &self.heal, 0), ("xDD8", &self.xdd8, 4)] {
            let p = format!("{path}.{name}");
            if v.live(&p, cx) && *v.peek() != m.u32(at + off) as i32 {
                cx.mismatch(
                    &p,
                    format!("{}", v.peek()),
                    format!("{}", m.u32(at + off) as i32),
                );
            }
        }
    }
}

fn vars<'r>(r: &'r mut Running, gobj: GObjId) -> &'r mut WhispyAppleVars {
    item_mut(r.game, gobj).xDD4_itemVar.whispy_apple()
}

/// Whether the apple bursts where it lands (`xDD8`, 1), which also keeps it from being picked
/// up.
fn bursts(r: &mut Running, gobj: GObjId) -> bool {
    vars(r, gobj).xdd8.get() == 1
}

/// An apple from Whispy at `pos`, `it_802EE200`: facing either way at random; a healing one
/// when the draw falls under `heal` (only if healing items are on), and one that bursts where
/// it lands unless the draw falls under `heal` plus `keep`.
#[doc(alias = "it_802EE200")]
pub fn spawn(
    r: &mut Running,
    _parent: Option<GObjId>,
    pos: Vec3,
    keep: f32,
    heal: f32,
) -> Option<GObjId> {
    let mut rand = reg(r.game.objs.rng.draw_f32());
    let mut spawn = SpawnItem {
        pos,
        prev_pos: pos,
        ..Default::default()
    };
    spawn.facing_dir = if r.game.objs.rng.below(2) != 0 {
        1.0
    } else {
        -1.0
    };
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = None;
    spawn.x44_flag[0] &= !0x80;
    spawn.x40 = 0;
    let mut heal = reg(heal);
    if !super::super::spawn::healing_on(r.game) {
        rand = fp::fsubs(rand, heal);
        if rand < 0.0 {
            rand = 0.0;
        }
        heal = 0.0;
        spawn.kind = WHISPY_APPLE;
    } else if rand < heal {
        spawn.kind = WHISPY_HEAL_APPLE;
    } else {
        spawn.kind = WHISPY_APPLE;
    }
    let gobj = super::super::item::spawn_air(r, &mut spawn)?;
    lib2725::count_spawned(r.game);
    start(r, gobj);
    if rand < fp::fadds(reg(keep), heal) {
        vars(r, gobj).xdd8.set(0);
    } else {
        vars(r, gobj).xdd8.set(1);
        lib26b1::disable_pickup(r.game, gobj);
    }
    lib2725::first_frame(r, gobj);
    Some(gobj)
}

/// `itWhispyAppleAttributes`' word at `off`: the heal (`x4`), the life (`x8`), the fall's
/// rise (`x14`) and its sideways spread (`x18`).
fn attr(r: &Running, gobj: GObjId, off: u32) -> u32 {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("an apple without attributes");
    r.data.u32(a.at(off))
}

/// Falls, `it_802EE374`: its life and heal, a random sideways speed within the spread, the
/// attributes' rise, and toward the stage at a twentieth of its depth a tick.
#[doc(alias = "it_802EE374")]
pub fn start(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    let life = fp::frsp(f64::from(attr(r, gobj, 8) as i32)) as f32;
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    let heal = attr(r, gobj, 4) as i32;
    vars(r, gobj).heal.set(heal);
    let rand = reg(r.game.objs.rng.draw_f32());
    let spread = reg(f32::from_bits(attr(r, gobj, 0x18)));
    let rise = f32::from_bits(attr(r, gobj, 0x14));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmsubs(fp::fmuls(2.0, spread), rand, spread));
    ip.x40_vel.y = rise;
    let z = fp::fdivs(reg(ip.pos.z), 20.0);
    ip.x40_vel.z = mem(if z < 0.0 { fp::fneg(z) } else { z });
}

/// The life every state counts down (the inline `itWhispyapple_UnkMotion0_Anim_inline`):
/// hidden once burst (state 7), flickering near its end or while still behind the stage, and
/// gone at 0.
fn age(r: &mut Running, gobj: GObjId) -> bool {
    let child = jobj_of(r.game, gobj).and_then(|j| r.game.objs.jobjs[j].child);
    let flicker = common_data(r.game).i32(r.data, data::common::X34);
    let ip = item_mut(r.game, gobj);
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    let (msid, life, z) = (ip.msid, reg(ip.xD44_lifeTimer), reg(ip.pos.z));
    if msid == 7 {
        jobj::set_flags_all(&mut r.game.objs.jobjs, child, jobj::HIDDEN);
    } else if life <= fp::frsp(f64::from(flicker)) || (if z < 0.0 { fp::fneg(z) } else { z }) > 1.0
    {
        lib2725::flicker(r.game, r.data, gobj);
    } else {
        jobj::clear_flags_all(&mut r.game.objs.jobjs, child, jobj::HIDDEN);
    }
    reg(item(r.game, gobj).xD44_lifeTimer) <= 0.0
}

/// `itWhispyapple_UnkMotion0_Anim`: falling, an apple that bursts stays out of reach.
#[doc(alias = "itWhispyapple_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    if bursts(r, gobj) {
        lib26b1::disable_pickup(r.game, gobj);
    }
    age(r, gobj)
}

/// `itWhispyapple_UnkMotion0_Phys`: gravity, out of reach if it bursts.
#[doc(alias = "itWhispyapple_UnkMotion0_Phys")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    if bursts(r, gobj) {
        lib26b1::disable_pickup(r.game, gobj);
    }
    gravity(r, gobj);
}

/// `itWhispyapple_UnkMotion0_Coll`: once it reaches the stage's plane, it bursts on the
/// floor or bounces.
#[doc(alias = "itWhispyapple_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    if bursts(r, gobj) {
        lib26b1::disable_pickup(r.game, gobj);
    }
    if reg(item(r.game, gobj).pos.z) >= 0.0 {
        let ip = item_mut(r.game, gobj);
        ip.pos.z = 0.0;
        ip.x40_vel.z = 0.0;
        if bursts(r, gobj) {
            fall(r, gobj, burst);
        } else {
            bounce(r, gobj);
        }
    }
    false
}

/// A bounce off what it touched, with a sound, landing if it met a floor it can stand on
/// (`itWhispyapple_UnkMotion5_Coll`'s body).
fn bounce(r: &mut Running, gobj: GObjId) {
    let touched = air_touches(r, gobj);
    if touched & 0xF != 0 {
        let sfx = BOUNCE_SFX[r.game.objs.rng.below(3) as usize];
        play_sfx(r, gobj, sfx, 0x7F, 0x40);
        maplib::bounce(r, gobj, touched);
        if touched & 1 != 0 && try_land(r, gobj) {
            landed(r, gobj);
        }
    }
}

/// Lands, stopped, `it_802EE6A0`.
#[doc(alias = "it_802EE6A0")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).x40_vel = Vec3::new(0.0, 0.0, 0.0);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// The states that age only, `itWhispyapple_UnkMotion1_Anim` (lying), `_UnkMotion5_Anim`
/// (falling again, dropped or thrown), `_UnkMotion6_Anim` (skidding) and `_UnkMotion7_Anim`
/// (burst).
#[doc(alias = "itWhispyapple_UnkMotion1_Anim")]
#[doc(alias = "itWhispyapple_UnkMotion5_Anim")]
#[doc(alias = "itWhispyapple_UnkMotion6_Anim")]
#[doc(alias = "itWhispyapple_UnkMotion7_Anim")]
pub fn anim_age(r: &mut Running, gobj: GObjId) -> bool {
    age(r, gobj)
}

/// The states that move nothing, `itWhispyapple_UnkMotion1_Phys` (3, 6, 7).
#[doc(alias = "itWhispyapple_UnkMotion1_Phys")]
#[doc(alias = "itWhispyapple_UnkMotion3_Phys")]
#[doc(alias = "itWhispyapple_UnkMotion6_Phys")]
#[doc(alias = "itWhispyapple_UnkMotion7_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// `itWhispyapple_UnkMotion1_Coll`: lying, it falls off ledges.
#[doc(alias = "itWhispyapple_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    false
}

/// Falls again, `fn_802EE7FC`.
#[doc(alias = "fn_802EE7FC")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// Gravity, `itWhispyapple_UnkMotion5_Phys`.
#[doc(alias = "itWhispyapple_UnkMotion5_Phys")]
pub fn gravity(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, data::attr::FALL_SPEED);
    let max = a.f32(r.data, data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
}

/// `itWhispyapple_UnkMotion5_Coll`: falling again, it bounces.
#[doc(alias = "itWhispyapple_UnkMotion5_Coll")]
pub fn coll5(r: &mut Running, gobj: GObjId) -> bool {
    bounce(r, gobj);
    false
}

/// Shown and held, `it_802EEA08`.
#[doc(alias = "it_802EEA08")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    show(r, gobj);
    change_state(r, gobj, 3, change::ANIM_UPDATE);
}

fn show(r: &mut Running, gobj: GObjId) {
    let child = jobj_of(r.game, gobj).and_then(|j| r.game.objs.jobjs[j].child);
    jobj::clear_flags_all(&mut r.game.objs.jobjs, child, jobj::HIDDEN);
}

/// Held, `itWhispyapple_UnkMotion3_Anim`.
#[doc(alias = "itWhispyapple_UnkMotion3_Anim")]
pub fn held(_: &mut Running, _: GObjId) -> bool {
    false
}

/// Let go of, it falls, `it_802EEA70`: in the stage's plane, let go with no speed.
#[doc(alias = "it_802EEA70")]
pub fn dropped(r: &mut Running, gobj: GObjId) {
    lib26b1::enable_pickup(r.game, gobj);
    show(r, gobj);
    change_state(r, gobj, 4, change::ANIM_UPDATE);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.z = 0.0;
    ip.x40_vel.y = 0.0;
    ip.pos.z = 0.0;
    release::let_go(r, gobj, Vec3::new(0.0, 0.0, 0.0));
}

/// Thrown, `it_802EEB28`: in the stage's plane.
#[doc(alias = "it_802EEB28")]
pub fn thrown(r: &mut Running, gobj: GObjId) {
    lib26b1::enable_pickup(r.game, gobj);
    show(r, gobj);
    change_state(r, gobj, 5, change::ANIM_UPDATE);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.z = 0.0;
    ip.pos.z = 0.0;
}

/// Skidding off a slope, `itWhispyApple_Logic18_EnteredAir`.
#[doc(alias = "itWhispyApple_Logic18_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 6, change::ANIM_UPDATE);
}

/// `itWhispyapple_UnkMotion6_Coll`: skidding, it stops or falls.
#[doc(alias = "itWhispyapple_UnkMotion6_Coll")]
pub fn coll6(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, landed, fell);
    false
}

/// Bursts where it lands, `it_802EED00`: out of reach and hidden, 20 more ticks of life, no
/// speed, its blast's effect, its hurtbox blinking, its hits new, and a gust that sways
/// jointed parts around it.
#[doc(alias = "it_802EED00")]
pub fn burst(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    lib26b1::disable_pickup(r.game, gobj);
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    lib26b1::set_xdd0_b3(r.game, gobj);
    lib2725::set_lifetime(r.game, r.data, gobj, 20.0);
    lib26b1::stop_keeping_clear(r.game, gobj);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.z = 0.0;
    ip.x40_vel.y = 0.0;
    ip.x40_vel.x = 0.0;
    lib2725::blast_effect(r, gobj);
    hitbox::start_blink(r.game, gobj);
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
    change_state(r, gobj, 7, change::ANIM_UPDATE | change::HIT_PRESERVE);
}

/// `itWhispyapple_UnkMotion7_Coll`: burst, it touches nothing.
#[doc(alias = "itWhispyapple_UnkMotion7_Coll")]
pub fn coll7(_: &mut Running, _: GObjId) -> bool {
    false
}

/// A hit it dealt ends it, `it_802EEED0`.
#[doc(alias = "it_802EEED0")]
pub fn dealt(_: &mut Running, _: GObjId) -> bool {
    true
}

/// Clanked or hitting a shield, it bounces back off the victim, `it_802EEED8` and
/// `it_802EEF30`.
#[doc(alias = "it_802EEED8")]
#[doc(alias = "it_802EEF30")]
pub fn bounce_off(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::clear_hitboxes(r.game, r.data, gobj);
    lib2725::bounce_off_victim(r.game, r.data, gobj);
    false
}

/// Reflected, `it_802EEF10`.
#[doc(alias = "it_802EEF10")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj)
}

/// Bounced off a shield, `it_802EEF68`.
#[doc(alias = "it_802EEF68")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_shield(r.game, gobj)
}

/// Forgets a GObj the apple points to, `it_802EEF88` (`it_8026B894`).
#[doc(alias = "it_802EEF88")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
