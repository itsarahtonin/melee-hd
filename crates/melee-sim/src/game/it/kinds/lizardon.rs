//! Charizard (`It_PKind_Lizardon`), `melee/it/kinds/itlizardon.c`: out of the ball it lands
//! and breathes fire a number of rounds, turning when its script says; while its script says
//! it's breathing, a flame item every few frames from its mouth at a random angle, with smoke
//! and four flame effects on their own timers, then it leaves. The way out and the rounds are
//! [`shooter`]'s, over [`CHARIZARD`].
//!
//! The unit also holds the flames' code (`It_Kind_Lizardon_Flame1` to `_Flame4`): each starts
//! its effect, flies with its speed shrinking by its attribute every frame, and is gone when
//! its life runs out or it touches the stage.

use gekko_fp as fp;

use super::super::item::{attr_of, change, change_state, play_sfx, spawn_air};
use super::super::pokemon::{self, attr_f32, attr_i32, effect_hitlag, table_bone};
use super::super::{data, item, item_mut, jobj_of, lib26b1, lib2725};
use super::shooter::{self, ROUNDS, Release, Rounds};
use crate::fpu::{mem, reg};
use crate::game::ef::{self, Arg};
use crate::game::it::ItKind;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x802C_B8AC;
pub const LANDED_NOTHING: u32 = 0x802C_B93C;
#[doc(alias("EVT_UNK"))]
pub const FORGET: u32 = 0x802C_B940;
pub const LANDED: u32 = 0x802C_B960;
pub const STAND: u32 = 0x802C_B994;
pub const ANIM1: u32 = 0x802C_B9E0;
pub const PHYS1: u32 = 0x802C_BA20;
pub const COLL1: u32 = 0x802C_BA5C;
pub const BREATHE: u32 = 0x802C_BAA8;
pub const ANIM2: u32 = 0x802C_BB84;
pub const PHYS2: u32 = 0x802C_BC9C;
pub const COLL2: u32 = 0x802C_BCD8;
pub const ACCESSORY: u32 = 0x802C_BD24;
pub const LEAVE: u32 = 0x802C_BFE4;
pub const ANIM3: u32 = 0x802C_C030;
pub const PHYS3: u32 = 0x802C_C064;
pub const COLL3: u32 = 0x802C_C0A0;
pub const ENTER_AIR: u32 = 0x802C_C0EC;
pub const ANIM0: u32 = 0x802C_C160;
pub const PHYS0: u32 = 0x802C_C184;
pub const COLL0: u32 = 0x802C_C1A4;
pub const FLAME1_SPAWNED: u32 = 0x802C_C3D4;
pub const FLAME2_SPAWNED: u32 = 0x802C_C440;
pub const FLAME3_SPAWNED: u32 = 0x802C_C4AC;
pub const FLAME4_SPAWNED: u32 = 0x802C_C518;
#[doc(alias("FLAME_EVT_UNK"))]
pub const FLAME_FORGET: u32 = 0x802C_C584;
pub const FLAME_REFLECTED: u32 = 0x802C_C5A4;
pub const FLAME_HIT_SHIELD: u32 = 0x802C_C5C4;
pub const FLAME_ABSORBED: u32 = 0x802C_C5CC;
pub const FLAME_FLY: u32 = 0x802C_C5D4;
pub const FLAME_ANIM: u32 = 0x802C_C650;
pub const FLAME_PHYS: u32 = 0x802C_C684;
pub const FLAME_COLL: u32 = 0x802C_C6C4;

/// Its variables past the rounds left (`itLizardon_ItemVars`, [`ROUNDS`]): which way it
/// breathes (`x64`, 1 for its back), the frames to its next flame (`x68`, a float), every other
/// flame's sound (`x6C`), the frames to its next smoke (`x70`) and to each of its four flame
/// effects (`x74`).
const BACK: usize = 0x64;
const FLAME_FRAMES: usize = 0x68;
const SOUND: usize = 0x6C;
const SMOKE_FRAMES: usize = 0x70;
const EFFECT_FRAMES: usize = 0x74;

/// Its mouth's and its throat's bones in its table.
const MOUTH: usize = 0x32;
const THROAT: usize = 0x31;

/// Charizard's way out (state 0, posed), its stand (1), its rounds of breath (2) and leaving
/// (3).
pub const CHARIZARD: Rounds = Rounds {
    release: Release {
        out: 0,
        hitlag: true,
        posed: true,
        landed,
        fallen: None,
    },
    stand: Some(1),
    fire: 2,
    leave: 3,
    accessory: ACCESSORY,
    first_round: shooter::rounds_from_attrs,
    on_round: Some(breath_timers),
    each_frame: None,
    stops: false,
};

/// Runs one of Charizard's (or its flames') predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM1 => shooter::standing(r, gobj, &CHARIZARD),
        COLL1 | COLL2 | COLL3 => shooter::stays(r, gobj),
        ANIM2 => shooter::rounds(r, gobj, &CHARIZARD),
        ANIM3 => shooter::leaving(r, gobj, &CHARIZARD),
        ANIM0 => shooter::growing(r, gobj),
        COLL0 => shooter::landing(r, gobj, &CHARIZARD.release),
        FLAME_REFLECTED => shooter::reflected(r, gobj),
        FLAME_HIT_SHIELD | FLAME_ABSORBED => shooter::gone(r, gobj),
        FLAME_ANIM => shooter::life_ticks(r, gobj),
        FLAME_COLL => shooter::gone_on_touch(r, gobj),
        _ => return None,
    })
}

/// Runs one of Charizard's (or its flames') events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        LANDED_NOTHING => shooter::nothing(r, gobj),
        LANDED => shooter::landed(r, gobj, &CHARIZARD),
        STAND => shooter::stand(r, gobj, &CHARIZARD),
        PHYS1 | PHYS2 | PHYS3 => shooter::gravity_in_air(r, gobj),
        BREATHE => shooter::fire(r, gobj, &CHARIZARD),
        ACCESSORY => accessory(r, gobj),
        LEAVE => shooter::leave(r, gobj, &CHARIZARD),
        ENTER_AIR => shooter::enter_air(r, gobj, &CHARIZARD.release),
        PHYS0 => shooter::falling(r, gobj, &CHARIZARD.release),
        FLAME1_SPAWNED => flame1_spawned(r, gobj),
        FLAME2_SPAWNED => flame2_spawned(r, gobj),
        FLAME3_SPAWNED => flame3_spawned(r, gobj),
        FLAME4_SPAWNED => flame4_spawned(r, gobj),
        FLAME_FLY => flame_fly(r, gobj),
        FLAME_PHYS => flame_phys(r, gobj),
        _ => return false,
    }
    true
}

/// `it_802CB8AC`, its spawn: its script's variables clear, no rounds yet, breathing ahead,
/// into the air at its first size out of the ball, its sound.
#[doc(alias = "it_802CB8AC")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.cmd_var0 = 0;
    ip.cmd_var1 = 0;
    ip.cmd_var2 = 0;
    let v = pokemon::vars(r.game, gobj);
    v.set_i32(ROUNDS, -1);
    v.set_i32(BACK, 0);
    v.set_f32(FLAME_FRAMES, 0.0);
    v.set_i32(SOUND, 0);
    shooter::enter_air(r, gobj, &CHARIZARD.release);
    let size = attr_f32(r, gobj, 0);
    pokemon::start(r, gobj, size);
    play_sfx(r, gobj, 0x2719, 0x7F, 0x40);
}

/// Forgets a GObj going away, `it_802CB940` and the flames' `itLizardon_Logic37_EvtUnk`
/// (`it_8026B894`).
#[doc(alias = "it_802CB940")]
#[doc(alias = "itLizardon_Logic37_EvtUnk")]
#[doc(alias("evt_unk"))]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// The frames to its next smoke, from its attributes' range.
fn next_smoke(r: &mut Running, gobj: GObjId) {
    let (lo, hi) = (attr_i32(r, gobj, 0x28), attr_i32(r, gobj, 0x2C));
    let d = r.game.objs.rng.below(hi.wrapping_sub(lo));
    pokemon::vars(r.game, gobj).set_i32(SMOKE_FRAMES, lo.wrapping_add(d));
}

/// Lands out of the ball ([`shooter::landed`]).
fn landed(r: &mut Running, gobj: GObjId) {
    shooter::landed(r, gobj, &CHARIZARD);
}

/// A round's start (`it_802CBAA8`'s, [`shooter::fire`]; its rounds' count is the float's bits):
/// its smoke's and flame effects' timers.
fn breath_timers(r: &mut Running, gobj: GObjId) {
    next_smoke(r, gobj);
    for i in 0..4 {
        let n = attr_i32(r, gobj, 0x18 + 4 * i as u32);
        pokemon::vars(r.game, gobj).set_i32(EFFECT_FRAMES + 4 * i, n);
    }
}

/// Its `on_accessory`, `it_802CBD24`: turns when its script says (its flame timer reset);
/// while its script says it's breathing, a flame of a random kind whenever its timer runs
/// out, smoke at its throat on its own timer, and each of its four flame effects at its
/// mouth, each a step smaller than the last one this frame.
#[doc(alias = "it_802CBD24")]
pub fn accessory(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).cmd_var1 != 0 {
        let every = attr_i32(r, gobj, 0x14);
        let v = pokemon::vars(r.game, gobj);
        let back = v.i32(BACK);
        v.set_i32(BACK, back ^ 1);
        v.set_f32(FLAME_FRAMES, mem(fp::frsp(f64::from(every))));
        item_mut(r.game, gobj).cmd_var1 = 0;
    }
    if item(r.game, gobj).cmd_var0 == 0 {
        return;
    }
    let mut scale = 1.0f32;
    let mouth = table_bone(r.game, gobj, MOUTH);
    crate::lb::joint::world_position(&mut r.game.objs, r.data, mouth, None);
    let facing = if pokemon::vars(r.game, gobj).i32(BACK) != 0 {
        -1.0f32
    } else {
        1.0
    };
    let v = pokemon::vars(r.game, gobj);
    let left = fp::fsubs(reg(v.f32(FLAME_FRAMES)), 1.0);
    v.set_f32(FLAME_FRAMES, mem(left));
    if left == 0.0 {
        let every = attr_i32(r, gobj, 0x14);
        pokemon::vars(r.game, gobj).set_f32(FLAME_FRAMES, mem(fp::frsp(f64::from(every))));
        let kind = r
            .game
            .objs
            .rng
            .below(4)
            .wrapping_add(ItKind::CharizardFlame1.id());
        flame(r, gobj, kind);
    }
    let v = pokemon::vars(r.game, gobj);
    let smoke = v.i32(SMOKE_FRAMES).wrapping_sub(1);
    v.set_i32(SMOKE_FRAMES, smoke);
    if smoke < 0 {
        let throat = table_bone(r.game, gobj, THROAT);
        pokemon::effect_scaled(r, 0x452, gobj, throat, 1.4);
        next_smoke(r, gobj);
    }
    for i in 0..4 {
        let v = pokemon::vars(r.game, gobj);
        let n = v.i32(EFFECT_FRAMES + 4 * i).wrapping_sub(1);
        v.set_i32(EFFECT_FRAMES + 4 * i, n);
        if n < 0 {
            let size = attr_of(item(r.game, gobj)).f32(r.data, data::attr::SCALE);
            scale = mem(fp::fmuls(fp::fdivs(reg(scale), reg(1.4f32)), reg(size)));
            let mouth = table_bone(r.game, gobj, MOUTH);
            ef::spawn(
                r,
                0x453 + i as i32,
                Some(gobj),
                &[Arg::JObj(mouth), Arg::Float(facing), Arg::Float(scale)],
            );
            let n = attr_i32(r, gobj, 0x18 + 4 * i as u32);
            pokemon::vars(r.game, gobj).set_i32(EFFECT_FRAMES + 4 * i, n);
        }
    }
}

/// Breathes a flame of `kind`, `it_802CC1CC`: from its mouth, at a random angle in its
/// attributes' range, its way; every other flame that comes out makes one of three sounds.
#[doc(alias = "it_802CC1CC")]
pub fn flame(r: &mut Running, gobj: GObjId, kind: i32) {
    let mouth = table_bone(r.game, gobj, MOUTH);
    let from = crate::lb::joint::world_position(&mut r.game.objs, r.data, mouth, None);
    let mut spawn = SpawnItem {
        prev_pos: from,
        ..Default::default()
    };
    spawn.pos = lib26b1::ecb_center(r.game, gobj);
    spawn.damage = 0;
    let (top, low, speed) = (
        attr_f32(r, gobj, 8),
        attr_f32(r, gobj, 0xC),
        attr_f32(r, gobj, 0x10),
    );
    let d = r
        .game
        .objs
        .rng
        .below(fp::fctiwz(fp::fsubs(reg(top), reg(low))));
    let angle = fp::fmuls(
        reg(0.017_453_292f32),
        fp::fsubs(reg(top), fp::frsp(f64::from(d))),
    );
    let cos = reg(crate::math::cos(mem(angle)));
    if pokemon::vars(r.game, gobj).i32(BACK) != 0 {
        spawn.vel.x = mem(fp::fmuls(fp::fneg(1.0), fp::fmuls(reg(speed), cos)));
        spawn.facing_dir = -1.0;
    } else {
        spawn.vel.x = mem(fp::fmuls(reg(speed), cos));
        spawn.facing_dir = 1.0;
    }
    let sin = reg(crate::math::sin(mem(angle)));
    spawn.vel.y = mem(fp::fmuls(reg(speed), sin));
    spawn.vel.z = 0.0;
    spawn.kind = ItKind::from_raw(kind);
    spawn.owner = item(r.game, gobj).owner;
    spawn.maker = Some(gobj);
    spawn.flags[0] |= 0x80;
    spawn.ignore_id = 0;
    if spawn_air(r, &mut spawn).is_some() {
        let v = pokemon::vars(r.game, gobj);
        let s = v.i32(SOUND) ^ 1;
        v.set_i32(SOUND, s);
        if s != 0 {
            let sfx = match r.game.objs.rng.below(3) {
                0 => 0x2716,
                1 => 0x2717,
                2 => 0x2718,
                _ => return,
            };
            play_sfx(r, gobj, sfx, 0x7F, 0x40);
        }
    }
}

/// A flame's effect `id` on its model, facing its way at five sevenths of its scale, then it
/// flies.
fn flame_spawned(r: &mut Running, gobj: GObjId, id: i32) {
    let j = jobj_of(r.game, gobj);
    let ip = item(r.game, gobj);
    let facing = ip.facing_dir;
    let size = attr_of(ip).f32(r.data, data::attr::SCALE);
    let s = mem(fp::fmuls(reg(0.714_285_73f32), reg(size)));
    ef::spawn(
        r,
        id,
        Some(gobj),
        &[Arg::JObj(j), Arg::Float(facing), Arg::Float(s)],
    );
    flame_fly(r, gobj);
}

/// The first flame's spawn, `itLizardon_Logic34_Spawned`: effect `0x44E`.
#[doc(alias = "itLizardon_Logic34_Spawned")]
pub fn flame1_spawned(r: &mut Running, gobj: GObjId) {
    flame_spawned(r, gobj, 0x44E);
}

/// The second flame's spawn, `itLizardon_Logic35_Spawned`: effect `0x44F`.
#[doc(alias = "itLizardon_Logic35_Spawned")]
pub fn flame2_spawned(r: &mut Running, gobj: GObjId) {
    flame_spawned(r, gobj, 0x44F);
}

/// The third flame's spawn, `itLizardon_Logic36_Spawned`: effect `0x450`.
#[doc(alias = "itLizardon_Logic36_Spawned")]
pub fn flame3_spawned(r: &mut Running, gobj: GObjId) {
    flame_spawned(r, gobj, 0x450);
}

/// The fourth flame's spawn, `itLizardon_Logic37_Spawned`: effect `0x451`.
#[doc(alias = "itLizardon_Logic37_Spawned")]
pub fn flame4_spawned(r: &mut Running, gobj: GObjId) {
    flame_spawned(r, gobj, 0x451);
}

/// A flame flies, `it_802CC5D4`: its life from its attributes, not spinning, not pickable.
#[doc(alias = "it_802CC5D4")]
pub fn flame_fly(r: &mut Running, gobj: GObjId) {
    let life = attr_f32(r, gobj, 0);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    lib2725::stop_spin(r.game, r.data, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
    lib26b1::disable_pickup(r.game, gobj);
}

/// A flame's `it_802CC684`: its speed shrinks by its attribute.
#[doc(alias = "it_802CC684")]
pub fn flame_phys(r: &mut Running, gobj: GObjId) {
    let k = reg(attr_f32(r, gobj, 4));
    let ip = item_mut(r.game, gobj);
    let v = ip.vel;
    ip.vel = Vec3::new(
        mem(fp::fmuls(reg(v.x), k)),
        mem(fp::fmuls(reg(v.y), k)),
        mem(fp::fmuls(reg(v.z), k)),
    );
}
