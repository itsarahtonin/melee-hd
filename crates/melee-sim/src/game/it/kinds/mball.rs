//! The Poké Ball, `melee/it/kinds/itmball.c`: it falls and lies about; thrown, it opens where
//! it lands and, at its attributes' moment, lets a Pokémon out (`it_8027AB64`, not ported).
//! Out of the count of Poké Balls (`Item_804A0C64.x1C`) once gone, unless its Pokémon came
//! out. The held items' callbacks ([`super::held`]), but for its fall and its opening.

use gekko_fp as fp;

use super::super::effect;
use super::super::ground::{fall, fall_to_floor, ground_or_fall};
use super::super::item::{article, change, change_state, play_sfx};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725, maplib};
use super::held::{self, Held};
use crate::fpu::reg;
use crate::game::ef::SpawnKind;
use crate::game::handles::CodeRef;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const DESTROYED: u32 = 0x8029_78DC;
pub const SPAWNED: u32 = 0x8029_7904;
pub const LANDED: u32 = 0x8029_7944;
pub const ANIM0: u32 = 0x8029_799C;
pub const PHYS0: u32 = 0x8029_79A4;
pub const COLL0: u32 = 0x8029_79A8;
pub const FELL: u32 = 0x8029_79D4;
pub const ANIM1: u32 = 0x8029_7A1C;
pub const PHYS1: u32 = 0x8029_7A24;
pub const COLL1: u32 = 0x8029_7A54;
pub const PICKED_UP: u32 = 0x8029_7ABC;
pub const ANIM2: u32 = 0x8029_7AE4;
pub const PHYS2: u32 = 0x8029_7AEC;
pub const DROPPED: u32 = 0x8029_7AF0;
pub const THROWN: u32 = 0x8029_7B18;
pub const PHYS3: u32 = 0x8029_7B40;
pub const COLL3: u32 = 0x8029_7B8C;
pub const DMG_DEALT: u32 = 0x8029_7BB8;
pub const ENTERED_AIR: u32 = 0x8029_7BF8;
pub const ANIM4: u32 = 0x8029_7C84;
pub const PHYS4: u32 = 0x8029_7C8C;
pub const COLL4: u32 = 0x8029_7C90;
pub const OPEN: u32 = 0x8029_7CC4;
pub const ON_ACCESSORY: u32 = 0x8029_7DD8;
pub const ANIM5: u32 = 0x8029_7E28;
pub const PHYS5: u32 = 0x8029_7E5C;
pub const COLL5: u32 = 0x8029_7E60;
pub const OPEN_GROUNDED: u32 = 0x8029_7E8C;
pub const ANIM6: u32 = 0x8029_7F94;
pub const PHYS6: u32 = 0x8029_7FC8;
pub const COLL6: u32 = 0x8029_7FF8;
pub const CLANKED: u32 = 0x8029_8060;
pub const REFLECTED: u32 = 0x8029_80A0;
pub const HIT_SHIELD: u32 = 0x8029_80D4;
pub const SHIELD_BOUNCED: u32 = 0x8029_8114;

/// The Poké Ball: closed as it spawns, its own fall, let go of or thrown into state 3,
/// skidding in 4 facing its motion; a hit, a reflection or a shield disarms it.
pub const POKE_BALL: Held = Held {
    name: "Poké Ball",
    charges: false,
    on_spawn: Some(closed),
    fell,
    dropped: 3,
    thrown: 3,
    drop_update: true,
    skid: 4,
    bounces_only_let_go: false,
    hits_disarm: true,
    skid_faces_motion: true,
};

/// Runs one of the Poké Ball's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM1 | ANIM2 | ANIM4 => held::still(r, gobj),
        ANIM5 | ANIM6 => opening(r, gobj),
        COLL0 => held::grounded(r, gobj, &POKE_BALL),
        COLL1 => coll1(r, gobj),
        COLL3 => coll3(r, gobj),
        COLL4 => held::skid(r, gobj, &POKE_BALL),
        COLL5 => coll5(r, gobj),
        COLL6 => coll6(r, gobj),
        DMG_DEALT | CLANKED | HIT_SHIELD => held::bounce(r, gobj, &POKE_BALL),
        REFLECTED => held::reflected(r, gobj, &POKE_BALL),
        SHIELD_BOUNCED => held::shield_bounced(r, gobj, &POKE_BALL),
        _ => return None,
    })
}

/// Runs one of the Poké Ball's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => destroyed(r, gobj),
        SPAWNED => held::spawned(r, gobj, &POKE_BALL),
        LANDED => held::landed(r, gobj),
        FELL => fell(r, gobj),
        PHYS0 | PHYS2 | PHYS4 | PHYS5 => held::idle(r, gobj),
        PHYS1 | PHYS6 => held::gravity(r, gobj),
        PICKED_UP => held::picked_up(r, gobj),
        DROPPED | THROWN => held::dropped(r, gobj, &POKE_BALL),
        PHYS3 => held::thrown_phys(r, gobj),
        ENTERED_AIR => held::entered_air(r, gobj, &POKE_BALL),
        OPEN => open(r, gobj),
        ON_ACCESSORY => on_accessory(r, gobj),
        OPEN_GROUNDED => open_grounded(r, gobj),
        _ => return false,
    }
    true
}

/// `itMBallAttributes`: how long it stays open (0), and the moment its Pokémon comes out (4).
fn attr(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a Poké Ball without attributes");
    r.data.f32(a.at(off))
}

/// `itMball_Destroyed`: out of the count of Poké Balls, unless its Pokémon came out.
#[doc(alias = "itMball_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    if !item_mut(r.game, gobj).vars.mball().out.get() {
        let c = &mut r.game.it.counts;
        c.x1C = c.x1C.wrapping_sub(1);
    }
}

/// Its spawn's own part: closed, its Pokémon not out.
fn closed(r: &mut Running, gobj: GObjId) {
    let v = item_mut(r.game, gobj).vars.mball();
    v.open.set(false);
    v.out.set(false);
}

/// Falls, `itMball_802979D4`: a closed ball's state keeps its animation from where it was.
#[doc(alias = "itMball_802979D4")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    let open = item_mut(r.game, gobj).vars.mball().open.get();
    let flags = if open { change::ANIM_UPDATE } else { 1 };
    change_state(r, gobj, 1, flags);
}

/// A fall that turns to face its motion when it bounces, the falling states' collision.
fn fall_facing(r: &mut Running, gobj: GObjId, landed: fn(&mut Running, GObjId)) {
    let before = item(r.game, gobj).vel.x;
    fall(r, gobj, landed);
    if item(r.game, gobj).vel.x != before {
        lib2725::face_motion(r.game, gobj);
    }
}

/// `itMball_Motion1_Coll`: lands, facing its motion after a bounce.
#[doc(alias = "itMball_Motion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    fall_facing(r, gobj, held::landed);
    false
}

/// `itMball_Motion3_Coll`: thrown, it opens where it lands.
#[doc(alias = "itMball_Motion3_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    fall_to_floor(r, gobj, open);
    false
}

/// Opens, the first time with its flash, sound and the time it stays open; then into
/// `state`, its animation starting over (the first time) or kept.
fn open_into(r: &mut Running, gobj: GObjId, state: i32) {
    let life = attr(r, gobj, 0);
    let j = jobj_of(r.game, gobj);
    let ip = item_mut(r.game, gobj);
    ip.flags.flags.keeps_clear = false;
    ip.skid = 0;
    lib2725::clear_velocity(r.game, gobj);
    lib2725::stop_spin(r.game, r.data, gobj);
    if !item_mut(r.game, gobj).vars.mball().open.get() {
        let ip = item_mut(r.game, gobj);
        ip.life_timer = life;
        ip.vars.mball().open.set(true);
        effect::queue(r, gobj, SpawnKind::Attach, 0x449, j, &[]);
        play_sfx(r, gobj, 0x10C, 0x7F, 0x40);
        change_state(r, gobj, state, change::ANIM_UPDATE);
    } else {
        change_state(r, gobj, state, 1);
    }
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(ON_ACCESSORY));
}

/// Opens where it landed, `itMball_80297CC4`: its open collision box.
#[doc(alias = "itMball_80297CC4")]
pub fn open(r: &mut Running, gobj: GObjId) {
    lib26b1::disable_pickup(r.game, gobj);
    let b = item(r.game, gobj).kind_coll_box.clone();
    maplib::set_box(r.game, gobj, b);
    let ip = item_mut(r.game, gobj);
    ip.vel = Vec3::new(0.0, 0.0, 0.0);
    open_into(r, gobj, 5);
}

/// `itMball_OnAccessory`: at its attributes' moment, its Pokémon comes out.
#[doc(alias = "itMball_OnAccessory")]
pub fn on_accessory(r: &mut Running, gobj: GObjId) {
    let at = attr(r, gobj, 4);
    if item(r.game, gobj).life_timer == at {
        let out = super::super::pokemon::release(r, gobj);
        item_mut(r.game, gobj).vars.mball().out.set(out);
    }
}

/// Open until its time runs out, `itMball_Motion5_Anim` and `itMball_Motion6_Anim`
/// (`Item_TickLifetime`).
#[doc(alias = "itMball_Motion5_Anim")]
#[doc(alias = "itMball_Motion6_Anim")]
pub fn opening(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    if reg(ip.life_timer) <= 0.0 {
        return true;
    }
    ip.life_timer = crate::fpu::mem(fp::fsubs(reg(ip.life_timer), 1.0));
    false
}

/// `itMball_Motion5_Coll`: open, off a ledge it falls open.
#[doc(alias = "itMball_Motion5_Coll")]
pub fn coll5(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, open_grounded);
    false
}

/// Opens falling, `itMball_80297E8C`.
#[doc(alias = "itMball_80297E8C")]
pub fn open_grounded(r: &mut Running, gobj: GObjId) {
    lib26b1::disable_pickup(r.game, gobj);
    open_into(r, gobj, 6);
}

/// `itMball_Motion6_Coll`: falling open, it opens where it lands, facing its motion after
/// a bounce.
#[doc(alias = "itMball_Motion6_Coll")]
pub fn coll6(r: &mut Running, gobj: GObjId) -> bool {
    fall_facing(r, gobj, open);
    false
}
