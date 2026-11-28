//! Ho-oh (`It_PKind_Houou`), `melee/it/kinds/ithouou.c`: out of the ball it flies up off the
//! top of the screen, comes back into the background towards the camera, sinks to where it
//! came out, and calls down Sacred Fire on the stage below (or above) where it came out, its
//! script saying when. Lugia's code with its own sink and attack ([`super::legendary`]).
//!
//! The unit also holds the Sacred Fire's code (`It_Kind_Houou_SacredFire`): it burns where it
//! was made, riding the stage line it was made on, until its life runs out or it touches the
//! stage.

use gekko_fp as fp;

use super::super::item::spawn_air;
use super::super::pokemon::{self, attr_f32};
use super::super::{item, item_mut};
use super::legendary::{self as lg, COME_FRAMES, HOME, Legendary};
use crate::fpu::{mem, reg};
use crate::game::gr::stage;
use crate::game::handles::CodeRef;
use crate::game::it::ItKind;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x802D_251C;
pub const LANDED_NOTHING: u32 = 0x802D_25B8;
#[doc(alias("EVT_UNK"))]
pub const FORGET: u32 = 0x802D_25BC;
pub const ANIM1: u32 = 0x802D_25DC;
pub const PHYS1: u32 = 0x802D_2640;
pub const COLL1: u32 = 0x802D_2660;
pub const ANIM2: u32 = 0x802D_26BC;
pub const PHYS2: u32 = 0x802D_26FC;
pub const COLL2: u32 = 0x802D_27A8;
pub const ANIM3: u32 = 0x802D_27FC;
pub const PHYS3: u32 = 0x802D_288C;
pub const COLL3: u32 = 0x802D_2904;
pub const ANIM4: u32 = 0x802D_2970;
pub const PHYS4: u32 = 0x802D_29D4;
pub const COLL4: u32 = 0x802D_2A50;
pub const ANIM5: u32 = 0x802D_2AC0;
pub const PHYS5: u32 = 0x802D_2B24;
pub const COLL5: u32 = 0x802D_2B44;
pub const FIRE_ACCESSORY: u32 = 0x802D_2B4C;
pub const ANIM0: u32 = 0x802D_2C54;
pub const PHYS0: u32 = 0x802D_2C78;
pub const COLL0: u32 = 0x802D_2D04;
pub const FIRE_SPAWNED: u32 = 0x802D_2E80;
#[doc(alias("FIRE_EVT_UNK"))]
pub const FIRE_FORGET: u32 = 0x802D_2ED0;
pub const FIRE_ANIM: u32 = 0x802D_2F3C;
pub const FIRE_PHYS: u32 = 0x802D_2F70;
pub const FIRE_COLL: u32 = 0x802D_2FE8;

/// Its climb's or sink's speed in its variables (`itHouou_ItemVars::vel_accum`); the frames
/// towards the camera and where it came out are [`lg::COME_FRAMES`] and [`lg::HOME`] (the
/// Sacred Fire keeps its stage line at the first).
const CLIMB: usize = 0x70;

/// Ho-oh: two script words (its fire's effects and the fire), off from a standstill, its own
/// sink, and its fire called when the sink's animation ends.
pub const HO_OH: Legendary = Legendary {
    climb: CLIMB,
    second_script_word: true,
    take_off_speed: None,
    sink,
    sink_ended: call_fire,
};

/// Runs one of Ho-oh's (or its Sacred Fire's) predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM1 => lg::anim1(r, gobj, &HO_OH),
        ANIM2 => lg::anim2(r, gobj),
        ANIM3 => lg::anim3(r, gobj),
        ANIM4 => lg::anim4(r, gobj, &HO_OH),
        ANIM5 => lg::anim5(r, gobj),
        COLL1 | COLL2 | COLL3 | COLL4 | COLL5 => lg::coll_nothing(r, gobj),
        ANIM0 => lg::anim0(r, gobj),
        COLL0 => lg::coll0(r, gobj),
        FIRE_ANIM => lg::shot_anim(r, gobj),
        FIRE_COLL => lg::shot_coll(r, gobj),
        _ => return None,
    })
}

/// Runs one of Ho-oh's (or its Sacred Fire's) events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => lg::spawned(r, gobj, &HO_OH),
        LANDED_NOTHING => {}
        PHYS1 | PHYS5 => lg::phys1(r, gobj),
        PHYS2 => lg::phys2(r, gobj, &HO_OH),
        PHYS3 => lg::phys3(r, gobj, &HO_OH),
        PHYS4 => phys4(r, gobj),
        FIRE_ACCESSORY => call_fire_when_told(r, gobj),
        PHYS0 => lg::phys0(r, gobj),
        FIRE_SPAWNED => lg::shot_out(r, gobj),
        FIRE_PHYS => fire_phys(r, gobj),
        _ => return false,
    }
    true
}

/// Sinks, `it_802D290C`: from its attribute's speed (`x18`).
#[doc(alias = "it_802D290C")]
pub fn sink(r: &mut Running, gobj: GObjId) {
    lg::enter(r, gobj, 4);
    let speed = attr_f32(r, gobj, 0x18);
    pokemon::vars(r.game, gobj).set_f32(CLIMB, speed);
}

/// `itHouou_UnkMotion4_Phys`: the bone's motion and its sink, by `x1C` a frame more; stopped
/// once more than 10 below where it came out.
#[doc(alias = "itHouou_UnkMotion4_Phys")]
pub fn phys4(r: &mut Running, gobj: GObjId) {
    pokemon::move_by_bone(r.game, gobj);
    let accel = attr_f32(r, gobj, 0x1C);
    let v = pokemon::vars(r.game, gobj);
    let climb = mem(fp::fadds(reg(v.f32(CLIMB)), reg(accel)));
    v.set_f32(CLIMB, climb);
    let home_y = v.f32(HOME + 4);
    let ip = item_mut(r.game, gobj);
    ip.vel.y = mem(fp::fadds(reg(ip.vel.y), reg(climb)));
    if reg(ip.pos.y) < fp::fsubs(reg(home_y), 10.0) {
        ip.vel.y = 0.0;
    }
}

/// Calls its fire, `it_802D2A58`: on its script's word, kept in the blast zones.
#[doc(alias = "it_802D2A58")]
pub fn call_fire(r: &mut Running, gobj: GObjId) {
    lg::enter(r, gobj, 5);
    let ip = item_mut(r.game, gobj);
    ip.on_accessory = Some(CodeRef(FIRE_ACCESSORY));
    ip.state_flags.checks_blast_zones = true;
}

/// Its `on_accessory`, `it_802D2B4C`: on its script's first word an effect on each of its
/// bones 16 and 17, on its second the Sacred Fire.
#[doc(alias = "it_802D2B4C")]
pub fn call_fire_when_told(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).cmd_var0 != 0 {
        item_mut(r.game, gobj).cmd_var0 = 0;
        let b16 = pokemon::table_bone(r.game, gobj, 16);
        pokemon::effect_scaled(r, 0x46F, gobj, b16, 1.3);
        let b17 = pokemon::table_bone(r.game, gobj, 17);
        pokemon::effect_scaled(r, 0x46F, gobj, b17, 1.3);
    }
    if item(r.game, gobj).cmd_var1 != 0 {
        item_mut(r.game, gobj).cmd_var1 = 0;
        sacred_fire(r, gobj);
    }
}

/// The Sacred Fire, `it_802D2D2C`: on the stage straight below where Ho-oh came out (to the
/// bottom of the blast zone), else straight above it (from the top), still, keeping the line it
/// was made on; none if neither way meets the stage.
#[doc(alias = "it_802D2D2C")]
pub fn sacred_fire(r: &mut Running, gobj: GObjId) {
    let home = pokemon::vars(r.game, gobj).vec(HOME);
    let (bottom, top) = (stage::blast_bottom(r.game), stage::blast_top(r.game));
    let g = r.game.geometry.as_mut().expect("no collision geometry");
    let Some(hit) = g
        .check_all_remap(-1, -1, home.x, home.y, home.x, bottom)
        .or_else(|| g.check_all_remap(-1, -1, home.x, top, home.x, home.y))
    else {
        return;
    };
    let ip = item(r.game, gobj);
    let (facing, owner) = (ip.facing_dir, ip.owner);
    let mut spawn = SpawnItem {
        kind: ItKind::HoOhSacredFire,
        ..Default::default()
    };
    spawn.prev_pos = hit.pos;
    spawn.pos = home;
    spawn.facing_dir = facing;
    spawn.damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.owner = owner;
    spawn.maker = Some(gobj);
    spawn.flags[0] |= 0x80;
    spawn.ignore_id = 0;
    let fire = spawn_air(r, &mut spawn).expect("Item_80268B18 made no Sacred Fire");
    pokemon::vars(r.game, fire).set_i32(COME_FRAMES, hit.line);
}

/// The Sacred Fire burns, `it_802D2EF0` (inline in its spawn, [`lg::shot_out`]).
#[doc(alias = "it_802D2EF0")]
pub fn burn(r: &mut Running, gobj: GObjId) {
    lg::enter(r, gobj, 0);
}

/// The Sacred Fire's `it_802D2F70`: carried by its line's motion while the line is live.
#[doc(alias = "it_802D2F70")]
pub fn fire_phys(r: &mut Running, gobj: GObjId) {
    let line = pokemon::vars(r.game, gobj).i32(COME_FRAMES);
    let pos = item(r.game, gobj).pos;
    let g = r.game.geometry.as_ref().expect("no collision geometry");
    if !g.line_live(line) {
        return;
    }
    let d = g
        .line_speed(line, pos)
        .expect("a live line without a speed");
    let ip = item_mut(r.game, gobj);
    ip.pos.x = mem(fp::fadds(reg(ip.pos.x), reg(d.x)));
    ip.pos.y = mem(fp::fadds(reg(ip.pos.y), reg(d.y)));
    ip.pos.z = mem(fp::fadds(reg(ip.pos.z), reg(d.z)));
}
