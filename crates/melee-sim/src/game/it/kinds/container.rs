//! The containers' shared code: the Capsule, the Crate, the Barrel and the Party Ball
//! (`melee/it/kinds/itcapsule.c`, `itbox.c`, `ittaru.c`, `itkusudama.c`, near-copies of each
//! other here). Each rests (state 0), falls, is held, thrown and dropped; what it holds comes out
//! when it breaks or opens. What they share is the rest, the fall and its landing, the skid, the
//! pickup, throw and drop, the gravity of the falls, the smoke and the blast's lifetime, and for
//! the three that open (the Crate, the Barrel and the Party Ball) the hit that breaks one, the
//! thrown flight and the drop that smash one, the damage that breaks one, the burst empty, the
//! opening, the frames until it's gone and the breaking's splinters, over a [`Container`] and an
//! [`Opening`] per kind. What each does of its own stays its own: the Capsule's explosion and its
//! landing's check, the Crate's tumble with Kongo Jungle's barrels and its upright landing, the
//! Barrel's roll and turn, the Party Ball's tilt, rise, swing and rain.

use gekko_fp as fp;

use super::super::ground::{Event, air, fall, ground_or_fall, ground_skid};
use super::super::item::{article, change, change_state, play_sfx};
use super::super::vars::{ItemVars, Var};
use super::super::{effect, hitbox, item, item_mut, lib26b1, lib2725, maplib, set_flag};
use crate::fpu::reg;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// One container's states and its own fall and stop, which the shared callbacks read (the
/// Capsule, the Crate and the Party Ball; the Barrel rolls its own way).
pub struct Container {
    /// Its fall: the Capsule's (`it_8027D148`, the state alone), [`fell`] (the Crate's), the
    /// Party Ball's (stood upright).
    pub fell: Event,
    /// Its stop on landing: [`landed`] (the Capsule's and the Crate's), the Party Ball's
    /// (`it_8028A190`, stood upright).
    pub landed: Event,
    /// Whether resting, it tilts with the floor (the Crate).
    pub tilts: bool,
    /// Its state held.
    pub held: i32,
    /// Its state thrown.
    pub thrown: i32,
    /// Its state dropped.
    pub dropped: i32,
    /// Whether a throw or a drop stops it being picked up (all but the Capsule).
    pub let_go_disables_pickup: bool,
}

/// The fields of the opening containers' variables all three have, at the same places
/// (`itBox_ItemVars`, `itTaru_ItemVars`, `itKusudama_ItemVars`).
pub struct Vars<'v> {
    /// `x0`: whether it's broken or opened (the Barrel's `hit`).
    pub opened: &'v mut Var<i32>,
    /// `x4`: the frames until it's gone, once open (the Crate's `despawn_timer`, the Barrel's
    /// `open_frames`, the Party Ball's `timer`).
    pub timer: &'v mut Var<i32>,
}

/// One opening container's settings: the Crate, the Barrel, the Party Ball.
pub struct Opening {
    /// Its name, for the stops.
    pub name: &'static str,
    /// Its variables' shared fields.
    pub vars: for<'v> fn(&'v mut ItemVars) -> Vars<'v>,
    /// What a hit does to it unopened: the Crate and the Barrel break, the Party Ball opens or
    /// bursts (`it_80289B50`, unchimed).
    pub breaks: Event,
    /// What smashing it does, thrown into something or dropped hard: the Crate's and the
    /// Barrel's break, the Party Ball's chimed opening.
    pub smashes: Event,
    /// Its attributes' damage that breaks it (`x14`, the Barrel's `xC`, `x20`).
    pub damage_limit: u32,
    /// Its attributes' landing speed that smashes it (`x1C`, the Barrel's `x34`, `x2C`).
    pub drop_limit: u32,
    /// Its stop after a soft landing: [`landed`] (the Crate), [`stop`] (the Barrel), the Party
    /// Ball's stop stood upright (`it_80289910`).
    pub settles: Event,
    /// Whether it breaks with something in it: the Crate's and the Party Ball's draws over their
    /// weights ([`has_items`]), the Barrel's over its two.
    pub has_items: fn(&mut Running, GObjId) -> bool,
    /// Its state bursting empty (6, 6, 7).
    pub burst_state: i32,
    /// Whether its burst blows the wind (the Crate, the Barrel).
    pub burst_wind: bool,
    /// Its opening's sound (0xF6, 0xFB, 0xF8).
    pub open_sfx: i32,
    /// What spills out as it opens, with a small quake, before it's hidden: the Crate's items by
    /// its weights (`it_80286248`), the Barrel's one item; none for the Party Ball, which rains
    /// its items later.
    pub spills: Option<Event>,
    /// Its frames open before it's gone (40, 40, 85).
    pub open_frames: i32,
    /// Its state open (7, 7, 8).
    pub open_state: i32,
}

impl Opening {
    /// Its variables' shared fields.
    fn vars<'v>(&self, r: &'v mut Running, gobj: GObjId) -> Vars<'v> {
        (self.vars)(&mut item_mut(r.game, gobj).vars)
    }

    /// Whether it's broken or opened (`x0`).
    fn opened(&self, r: &mut Running, gobj: GObjId) -> bool {
        self.vars(r, gobj).opened.get() != 0
    }
}

/// Its attributes' word at `off`.
pub fn attr(r: &Running, gobj: GObjId, o: &Opening, off: u32) -> melee_data::archive::DataRef {
    article(item(r.game, gobj))
        .special(r.data)
        .unwrap_or_else(|| panic!("a {} without attributes", o.name))
        .at(off)
}

/// Its attributes' integer at `off`.
pub fn attr_i(r: &Running, gobj: GObjId, o: &Opening, off: u32) -> i32 {
    r.data.u32(attr(r, gobj, o, off)) as i32
}

/// Its attributes' float at `off`.
pub fn attr_f(r: &Running, gobj: GObjId, o: &Opening, off: u32) -> f32 {
    r.data.f32(attr(r, gobj, o, off))
}

/// The callbacks that do nothing and answer false.
#[doc(alias = "itCapsule_UnkMotion0_Anim")]
#[doc(alias = "itCapsule_UnkMotion2_Anim")]
#[doc(alias = "itCapsule_UnkMotion4_Anim")]
#[doc(alias = "itCapsule_UnkMotion6_Anim")]
#[doc(alias = "itCapsule_UnkMotion5_Coll")]
#[doc(alias = "itBox_UnkMotion4_Anim")]
#[doc(alias = "itBox_UnkMotion2_Anim")]
#[doc(alias = "itBox_UnkMotion5_Anim")]
#[doc(alias = "itBox_UnkMotion8_Anim")]
#[doc(alias = "itBox_UnkMotion6_Coll")]
#[doc(alias = "itBox_UnkMotion7_Coll")]
#[doc(alias = "itBox_UnkMotion8_Coll")]
#[doc(alias = "itTaru_UnkMotion1_Anim")]
#[doc(alias = "itTaru_UnkMotion2_Anim")]
#[doc(alias = "itTaru_UnkMotion6_Coll")]
#[doc(alias = "itTaru_UnkMotion7_Coll")]
#[doc(alias = "itKusudama_UnkMotion4_Anim")]
#[doc(alias = "itKusudama_UnkMotion6_Anim")]
#[doc(alias = "itKusudama_UnkMotion7_Coll")]
#[doc(alias = "itKusudama_UnkMotion8_Coll")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states that move nothing.
#[doc(alias = "itCapsule_UnkMotion0_Phys")]
#[doc(alias = "itCapsule_UnkMotion2_Phys")]
#[doc(alias = "itCapsule_UnkMotion5_Phys")]
#[doc(alias = "itCapsule_UnkMotion6_Phys")]
#[doc(alias = "itBox_UnkMotion0_Phys")]
#[doc(alias = "itBox_UnkMotion2_Phys")]
#[doc(alias = "itBox_UnkMotion6_Phys")]
#[doc(alias = "itBox_UnkMotion7_Phys")]
#[doc(alias = "itBox_UnkMotion5_Phys")]
#[doc(alias = "itTaru_UnkMotion0_Phys")]
#[doc(alias = "itTaru_UnkMotion2_Phys")]
#[doc(alias = "itTaru_UnkMotion6_Phys")]
#[doc(alias = "itTaru_UnkMotion7_Phys")]
#[doc(alias = "itKusudama_UnkMotion0_Phys")]
#[doc(alias = "itKusudama_UnkMotion1_Phys")]
#[doc(alias = "itKusudama_UnkMotion3_Phys")]
#[doc(alias = "itKusudama_UnkMotion4_Phys")]
#[doc(alias = "itKusudama_UnkMotion7_Phys")]
#[doc(alias = "itKusudama_UnkMotion8_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// Gravity from the attributes, the falls.
#[doc(alias = "itCapsule_UnkMotion4_Phys")]
#[doc(alias = "itTaru_UnkMotion1_Phys")]
#[doc(alias = "itKusudama_UnkMotion2_Phys")]
pub fn gravity(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
}

/// Thrown or dropped, gravity and the spin it slows (`Item_ApplyFallingPhysics`).
#[doc(alias = "itCapsule_UnkMotion3_Phys")]
#[doc(alias = "itBox_UnkMotion4_Phys")]
#[doc(alias = "itKusudama_UnkMotion6_Phys")]
pub fn thrown_phys(r: &mut Running, gobj: GObjId) {
    super::falling_phys(r, gobj);
}

/// Stops and rests, to be picked up: `it_8027D0B8` (the Capsule), `fn_80286480` (the Crate).
#[doc(alias = "it_8027D0B8")]
#[doc(alias = "fn_80286480")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    lib26b1::enable_pickup(r.game, gobj);
    item_mut(r.game, gobj).vel = Vec3::new(0.0, 0.0, 0.0);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// Stops and rests the other way round, its speed first (the inline `Item_StopAndEnterState`:
/// the Barrel's soft landing, the Party Ball's stops).
pub fn stop(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).vel = Vec3::new(0.0, 0.0, 0.0);
    lib26b1::enable_pickup(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// Falls (state 1), no longer to be picked up: `it_8028655C` (the Crate), `it_80287D0C` (the
/// Barrel).
#[doc(alias = "it_8028655C")]
#[doc(alias = "it_80287D0C")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    lib26b1::disable_pickup(r.game, gobj);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// Resting, it falls off ledges (with [`Container::tilts`], tilting with the floor).
#[doc(alias = "itCapsule_UnkMotion0_Coll")]
#[doc(alias = "itBox_UnkMotion0_Coll")]
#[doc(alias = "itKusudama_UnkMotion0_Coll")]
pub fn rests(r: &mut Running, gobj: GObjId, c: &Container) -> bool {
    ground_or_fall(r, gobj, c.fell);
    if c.tilts {
        maplib::tilt_on_floor(r.game, r.data, gobj);
    }
    false
}

/// Falling, it lands and stops.
#[doc(alias = "itCapsule_UnkMotion4_Coll")]
#[doc(alias = "itBox_UnkMotion1_Coll")]
#[doc(alias = "itKusudama_UnkMotion2_Coll")]
pub fn lands(r: &mut Running, gobj: GObjId, c: &Container) -> bool {
    fall(r, gobj, c.landed);
    false
}

/// Skidding off a slope, it stops or falls.
#[doc(alias = "itCapsule_UnkMotion6_Coll")]
#[doc(alias = "itBox_UnkMotion5_Coll")]
pub fn skids(r: &mut Running, gobj: GObjId, c: &Container) -> bool {
    ground_skid(r, gobj, c.landed, c.fell);
    false
}

/// Picked up, held.
#[doc(alias = "itCapsule_Logic0_PickedUp")]
#[doc(alias = "itBox_Logic1_PickedUp")]
#[doc(alias = "itKusudama_Logic4_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId, c: &Container) {
    change_state(r, gobj, c.held, change::ANIM_UPDATE);
}

/// Into `state`, let go of.
fn let_go(r: &mut Running, gobj: GObjId, c: &Container, state: i32) {
    if c.let_go_disables_pickup {
        lib26b1::disable_pickup(r.game, gobj);
    }
    change_state(r, gobj, state, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// Thrown.
#[doc(alias = "itCapsule_Logic0_Thrown")]
#[doc(alias = "itBox_Logic1_Thrown")]
#[doc(alias = "itKusudama_Logic4_Thrown")]
pub fn thrown(r: &mut Running, gobj: GObjId, c: &Container) {
    let_go(r, gobj, c, c.thrown);
}

/// Let go of.
#[doc(alias = "itCapsule_Logic0_Dropped")]
#[doc(alias = "itBox_Logic1_Dropped")]
#[doc(alias = "itKusudama_Logic4_Dropped")]
pub fn dropped(r: &mut Running, gobj: GObjId, c: &Container) {
    let_go(r, gobj, c, c.dropped);
}

/// Smokes as its time runs out.
#[doc(alias = "itBox_UnkMotion0_Anim")]
#[doc(alias = "itTaru_UnkMotion0_Anim")]
pub fn smokes(r: &mut Running, gobj: GObjId) -> bool {
    if reg(item(r.game, gobj).life_timer) <= 1.0 {
        effect::spawn_simple(r, gobj, 0x421);
    }
    false
}

/// The blast lasts its lifetime: the Capsule's explosion, the bursts empty.
#[doc(alias = "itCapsule_UnkMotion5_Anim")]
#[doc(alias = "itBox_UnkMotion6_Anim")]
#[doc(alias = "itTaru_UnkMotion6_Anim")]
#[doc(alias = "itKusudama_UnkMotion7_Anim")]
pub fn blast_lasts(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::tick_life(r.game, gobj)
}

/// Open, gone once its frames run out (the Party Ball's after its rain).
#[doc(alias = "itBox_UnkMotion7_Anim")]
#[doc(alias = "itTaru_UnkMotion7_Anim")]
pub fn gone_after(r: &mut Running, gobj: GObjId, o: &Opening) -> bool {
    let v = o.vars(r, gobj);
    v.timer.update(|old| old.wrapping_sub(1));
    v.timer.get() <= 0
}

/// Hitting something, clanking, on a shield or reflected, an unopened one breaks (its
/// [`Opening::breaks`]).
#[doc(alias = "itBox_Logic1_DmgDealt")]
#[doc(alias = "itBox_Logic1_Clanked")]
#[doc(alias = "itBox_Logic1_HitShield")]
#[doc(alias = "itBox_Logic1_Reflected")]
#[doc(alias = "it_3F14_Logic2_DmgDealt")]
#[doc(alias = "it_3F14_Logic2_Clanked")]
#[doc(alias = "it_3F14_Logic2_HitShield")]
#[doc(alias = "it_3F14_Logic2_Reflected")]
#[doc(alias = "itKusudama_Logic4_DmgDealt")]
#[doc(alias = "itKusudama_Logic4_Clanked")]
#[doc(alias = "itKusudama_Logic4_HitShield")]
#[doc(alias = "itKusudama_Logic4_Reflected")]
pub fn hit(r: &mut Running, gobj: GObjId, o: &Opening) -> bool {
    if !o.opened(r, gobj) {
        (o.breaks)(r, gobj);
    }
    false
}

/// Hit, an unopened one breaks once its damage reaches its attributes' limit
/// ([`Opening::damage_limit`]). The Barrel's is its own.
#[doc(alias = "itBox_Logic1_DmgReceived")]
#[doc(alias = "itKusudama_Logic4_DmgReceived")]
pub fn damaged(r: &mut Running, gobj: GObjId, o: &Opening) -> bool {
    let limit = attr_f(r, gobj, o, o.damage_limit);
    if !o.opened(r, gobj) && fp::frsp(f64::from(item(r.game, gobj).damage_taken)) >= reg(limit) {
        (o.breaks)(r, gobj);
    }
    false
}

/// Thrown, it smashes on what it hits ([`Opening::smashes`]), and stops rising at ceilings.
#[doc(alias = "itBox_UnkMotion3_Coll")]
#[doc(alias = "itKusudama_UnkMotion5_Coll")]
pub fn flies(r: &mut Running, gobj: GObjId, o: &Opening) -> bool {
    if air(r, gobj) {
        (o.smashes)(r, gobj);
    } else if maplib::touched_ceiling(r.game, gobj) != 0 {
        let ip = item_mut(r.game, gobj);
        if reg(ip.vel.y) > 0.0 {
            ip.vel.y = 0.0;
        }
    } else if maplib::touched_wall(r.game, gobj) != 0 {
        (o.smashes)(r, gobj);
    }
    false
}

/// Dropped, it smashes landing faster than its [`Opening::drop_limit`], else stops
/// ([`Opening::settles`]).
#[doc(alias = "itBox_UnkMotion4_Coll")]
#[doc(alias = "itKusudama_UnkMotion6_Coll")]
pub fn drops(r: &mut Running, gobj: GObjId, o: &Opening) -> bool {
    let limit = attr_f(r, gobj, o, o.drop_limit);
    if air(r, gobj) {
        let vy = reg(item(r.game, gobj).vel.y);
        if (if vy < 0.0 { -vy } else { vy }) > reg(limit) {
            (o.smashes)(r, gobj);
        } else {
            maplib::enter_ground(item_mut(r.game, gobj));
            (o.settles)(r, gobj);
        }
    }
    false
}

/// Bursts empty: hidden, it blasts a moment (with [`Opening::burst_wind`], pushing the wind):
/// `it_80286AA4` (the Crate), `it_802885C8` (the Barrel), `it_8028AC74` (the Party Ball).
#[doc(alias = "it_80286AA4")]
#[doc(alias = "it_802885C8")]
#[doc(alias = "it_8028AC74")]
pub fn burst(r: &mut Running, gobj: GObjId, o: &Opening) {
    lib26b1::hide(r.game, gobj);
    lib2725::blast_effect(r, gobj);
    hitbox::start_blink(r.game, gobj);
    lib26b1::disable_pickup(r.game, gobj);
    lib26b1::freeze_lifetime(r.game, gobj);
    lib2725::start_vanishing(r.game, r.data, gobj);
    let ip = item_mut(r.game, gobj);
    ip.vel.x = 0.0;
    ip.vel.y = 0.0;
    set_flag(&mut ip.draw_flags, 2, true);
    let v = o.vars(r, gobj);
    v.opened.set(1);
    v.timer.set(40);
    hitbox::set_hit_ids(r.game, gobj);
    if o.burst_wind {
        let pos = item(r.game, gobj).pos;
        crate::lb::wind::blast(
            &mut r.game.gr.winds,
            pos,
            0x78,
            1.0,
            0.02,
            std::f32::consts::FRAC_PI_3,
        );
    }
    change_state(r, gobj, o.burst_state, change::ANIM_UPDATE);
}

/// Opens: its sound, what spills out ([`Opening::spills`], with a small quake, then hidden),
/// gone after its frames: `it_80286BA0` (the Crate), `it_802886C4` (the Barrel), `it_8028AD44`
/// (the Party Ball, which swings open).
#[doc(alias = "it_80286BA0")]
#[doc(alias = "it_802886C4")]
#[doc(alias = "it_8028AD44")]
pub fn open(r: &mut Running, gobj: GObjId, o: &Opening) {
    play_sfx(r, gobj, o.open_sfx, 0x7F, 0x40);
    if let Some(spill) = o.spills {
        let pos = item(r.game, gobj).pos;
        crate::game::camera::request_quake(r, 2, Some(pos));
        spill(r, gobj);
        lib26b1::hide(r.game, gobj);
    }
    hitbox::start_blink(r.game, gobj);
    let ip = item_mut(r.game, gobj);
    ip.vel.x = 0.0;
    ip.vel.y = 0.0;
    set_flag(&mut ip.draw_flags, 2, true);
    let v = o.vars(r, gobj);
    v.opened.set(1);
    v.timer.set(o.open_frames);
    lib26b1::disable_pickup(r.game, gobj);
    change_state(r, gobj, o.open_state, change::ANIM_UPDATE);
}

/// Breaks: its splinters (effect 0x427), then opens if something's in it
/// ([`Opening::has_items`]), else bursts empty (the Crate's inline `itBox_TryOpen_inline`, the
/// Barrel's own).
pub fn break_open(r: &mut Running, gobj: GObjId, o: &Opening) {
    let pos = item(r.game, gobj).pos;
    crate::game::ef::spawn(r, 0x427, Some(gobj), &[crate::game::ef::Arg::Vec(pos)]);
    if (o.has_items)(r, gobj) {
        open(r, gobj, o);
    } else {
        burst(r, gobj, o);
    }
}

/// Whether it breaks with items in it, `it_80286340` (the Crate), `it_8028A114` (the Party
/// Ball): a draw over the weights and the empty weight lands below the three item weights.
#[doc(alias = "it_80286340")]
#[doc(alias = "it_8028A114")]
pub fn has_items(r: &mut Running, w1: i32, w2: i32, w3: i32, empty: i32) -> bool {
    let sum1 = w1.wrapping_add(w2);
    let sum2 = w3.wrapping_add(sum1);
    let roll = r.game.objs.rng.below(empty.wrapping_add(sum2));
    roll < w1 || roll < sum1 || roll < sum2
}

/// Forgets a GObj it points to (`it_8026B894`).
#[doc(alias = "itBox_Logic1_EvtUnk")]
#[doc(alias = "itTaru_Logic2_EvtUnk")]
#[doc(alias = "itKusudama_Logic4_EvtUnk")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
