//! Charizard (`It_PKind_Lizardon`), `melee/it/kinds/itlizardon.c`: out of the ball it lands
//! and breathes fire a number of rounds, turning when its script says; while its script says
//! it's breathing, a flame item every few frames from its mouth at a random angle, with smoke
//! and four flame effects on their own timers, then it leaves.
//!
//! The unit also holds the flames' code (`It_Kind_Lizardon_Flame1` to `_Flame4`): each starts
//! its effect, flies with its speed shrinking by its attribute every frame, and is gone when
//! its life runs out or it touches the stage.

use gekko_fp as fp;

use super::super::ground::{air_touched_any, fall, ground_or_fall};
use super::super::item::{attr_of, change, change_state, play_sfx, spawn_air};
use super::super::pokemon::{self, attr_f32, attr_i32, effect_hitlag, table_bone};
use super::super::{data, item, item_mut, jobj_of, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::ef::{self, Arg};
use crate::game::handles::CodeRef;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x802C_B8AC;
pub const LANDED_NOTHING: u32 = 0x802C_B93C;
pub const EVT_UNK: u32 = 0x802C_B940;
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
pub const FLAME_EVT_UNK: u32 = 0x802C_C584;
pub const FLAME_REFLECTED: u32 = 0x802C_C5A4;
pub const FLAME_HIT_SHIELD: u32 = 0x802C_C5C4;
pub const FLAME_ABSORBED: u32 = 0x802C_C5CC;
pub const FLAME_FLY: u32 = 0x802C_C5D4;
pub const FLAME_ANIM: u32 = 0x802C_C650;
pub const FLAME_PHYS: u32 = 0x802C_C684;
pub const FLAME_COLL: u32 = 0x802C_C6C4;

/// `It_Kind_Lizardon_Flame1`, the first of its four flames.
pub const FLAME1: i32 = 0xC3;

/// Its variables past the release's (`itLizardon_ItemVars`): the rounds left (`x60`, -1
/// until the first), which way it breathes (`x64`, 1 for its back), the frames to its next
/// flame (`x68`, a float), every other flame's sound (`x6C`), the frames to its next smoke
/// (`x70`) and to each of its four flame effects (`x74`).
const ROUNDS: usize = 0x60;
const BACK: usize = 0x64;
const FLAME_FRAMES: usize = 0x68;
const SOUND: usize = 0x6C;
const SMOKE_FRAMES: usize = 0x70;
const EFFECT_FRAMES: usize = 0x74;

/// Its mouth's and its throat's bones in its table.
const MOUTH: usize = 0x32;
const THROAT: usize = 0x31;

/// `GA_Air`.
const GA_AIR: i32 = 1;

/// Runs one of Charizard's (or its flames') predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM1 => anim1(r, gobj),
        COLL1 | COLL2 | COLL3 => coll1(r, gobj),
        ANIM2 => anim2(r, gobj),
        ANIM3 => anim3(r, gobj),
        ANIM0 => anim0(r, gobj),
        COLL0 => coll0(r, gobj),
        FLAME_REFLECTED => flame_reflected(r, gobj),
        FLAME_HIT_SHIELD => flame_hit_shield(r, gobj),
        FLAME_ABSORBED => flame_absorbed(r, gobj),
        FLAME_ANIM => flame_anim(r, gobj),
        FLAME_COLL => flame_coll(r, gobj),
        _ => return None,
    })
}

/// Runs one of Charizard's (or its flames') events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        LANDED_NOTHING => landed_nothing(r, gobj),
        LANDED => landed(r, gobj),
        STAND => stand(r, gobj),
        PHYS1 | PHYS2 | PHYS3 => phys1(r, gobj),
        BREATHE => breathe(r, gobj),
        ACCESSORY => accessory(r, gobj),
        LEAVE => leave(r, gobj),
        ENTER_AIR => enter_air(r, gobj),
        PHYS0 => phys0(r, gobj),
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
    ip.xDAC_itcmd_var0 = 0;
    ip.xDB0_itcmd_var1 = 0;
    ip.xDB4_itcmd_var2 = 0;
    let v = pokemon::vars(r.game, gobj);
    v.set_i32(ROUNDS, -1);
    v.set_i32(BACK, 0);
    v.set_f32(FLAME_FRAMES, 0.0);
    v.set_i32(SOUND, 0);
    enter_air(r, gobj);
    let size = attr_f32(r, gobj, 0);
    pokemon::start(r, gobj, size);
    play_sfx(r, gobj, 0x2719, 0x7F, 0x40);
}

/// Lands (or leaves the floor) and nothing happens, `it_802CB93C`.
#[doc(alias = "it_802CB93C")]
pub fn landed_nothing(_: &mut Running, _: GObjId) {}

/// Forgets a GObj going away, `it_802CB940` and the flames' `itLizardon_Logic37_EvtUnk`
/// (`it_8026B894`).
#[doc(alias = "it_802CB940")]
#[doc(alias = "itLizardon_Logic37_EvtUnk")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// Lands out of the ball, `it_802CB960`: still, standing.
#[doc(alias = "it_802CB960")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    lib2725::clear_velocity(r.game, gobj);
    stand(r, gobj);
}

/// Stands, `it_802CB994`.
#[doc(alias = "it_802CB994")]
pub fn stand(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
}

/// `itLizardon_UnkMotion1_Anim`: breathes fire when its standing animation ends.
#[doc(alias = "itLizardon_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        breathe(r, gobj);
    }
    false
}

/// `itLizardon_UnkMotion1_Phys` (and its copies in states 2 and 3): its gravity in the air.
#[doc(alias = "itLizardon_UnkMotion1_Phys")]
#[doc(alias = "itLizardon_UnkMotion2_Phys")]
#[doc(alias = "itLizardon_UnkMotion3_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).ground_or_air == GA_AIR {
        super::gravity(r, gobj);
    }
}

/// `itLizardon_UnkMotion1_Coll` (and its copies in states 2 and 3): lands in the air, falls
/// off the floor on it, doing nothing either way.
#[doc(alias = "itLizardon_UnkMotion1_Coll")]
#[doc(alias = "itLizardon_UnkMotion2_Coll")]
#[doc(alias = "itLizardon_UnkMotion3_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).ground_or_air == GA_AIR {
        fall(r, gobj, landed_nothing);
    } else {
        ground_or_fall(r, gobj, landed_nothing);
    }
    false
}

/// The frames to its next smoke, from its attributes' range.
fn next_smoke(r: &mut Running, gobj: GObjId) {
    let (lo, hi) = (attr_i32(r, gobj, 0x28), attr_i32(r, gobj, 0x2C));
    let d = r.game.objs.rng.below(hi.wrapping_sub(lo));
    pokemon::vars(r.game, gobj).set_i32(SMOKE_FRAMES, lo.wrapping_add(d));
}

/// Breathes a round of fire, `it_802CBAA8` (and its inline copy in
/// `itLizardon_UnkMotion2_Anim`): the first time, its rounds from its attributes (the float's
/// bits as a count); its smoke's and flame effects' timers, and breathing as its script says.
#[doc(alias = "it_802CBAA8")]
pub fn breathe(r: &mut Running, gobj: GObjId) {
    if pokemon::vars(r.game, gobj).i32(ROUNDS) == -1 {
        let rounds = attr_i32(r, gobj, 4);
        pokemon::vars(r.game, gobj).set_i32(ROUNDS, rounds);
    }
    change_state(r, gobj, 2, change::ANIM_UPDATE);
    next_smoke(r, gobj);
    for i in 0..4 {
        let n = attr_i32(r, gobj, 0x18 + 4 * i as u32);
        pokemon::vars(r.game, gobj).set_i32(EFFECT_FRAMES + 4 * i, n);
    }
    effect_hitlag(r.game, gobj);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(ACCESSORY));
}

/// `itLizardon_UnkMotion2_Anim`: at each round's end, another, or with none left it leaves.
#[doc(alias = "itLizardon_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        let v = pokemon::vars(r.game, gobj);
        let rounds = v.i32(ROUNDS);
        if rounds <= 0 {
            v.set_i32(ROUNDS, 0);
            leave(r, gobj);
        } else {
            v.set_i32(ROUNDS, rounds.wrapping_sub(1));
            breathe(r, gobj);
        }
    }
    false
}

/// Its `on_accessory`, `it_802CBD24`: turns when its script says (its flame timer reset);
/// while its script says it's breathing, a flame of a random kind whenever its timer runs
/// out, smoke at its throat on its own timer, and each of its four flame effects at its
/// mouth, each a step smaller than the last one this frame.
#[doc(alias = "it_802CBD24")]
pub fn accessory(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).xDB0_itcmd_var1 != 0 {
        let every = attr_i32(r, gobj, 0x14);
        let v = pokemon::vars(r.game, gobj);
        let back = v.i32(BACK);
        v.set_i32(BACK, back ^ 1);
        v.set_f32(FLAME_FRAMES, mem(fp::frsp(f64::from(every))));
        item_mut(r.game, gobj).xDB0_itcmd_var1 = 0;
    }
    if item(r.game, gobj).xDAC_itcmd_var0 == 0 {
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
        let kind = r.game.objs.rng.below(4).wrapping_add(FLAME1);
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

/// Leaves, `it_802CBFE4`.
#[doc(alias = "it_802CBFE4")]
pub fn leave(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 3, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
}

/// `itLizardon_UnkMotion3_Anim`: gone when its animation ends.
#[doc(alias = "itLizardon_UnkMotion3_Anim")]
pub fn anim3(r: &mut Running, gobj: GObjId) -> bool {
    !lib2725::is_animating(r.game, gobj)
}

/// Into the air out of the ball, `it_802CC0EC` (`Item_EnterAirStateWithHitlagAndStateDesc`).
#[doc(alias = "it_802CC0EC")]
pub fn enter_air(r: &mut Running, gobj: GObjId) {
    pokemon::enter_first_state(r, gobj);
}

/// `it_802CC160`: growing out of the ball.
#[doc(alias = "it_802CC160")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::grow(r, gobj);
    false
}

/// `it_802CC184`: falling out of the ball.
#[doc(alias = "it_802CC184")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    pokemon::fall(r, gobj);
}

/// `it_802CC1A4`: lands out of the ball.
#[doc(alias = "it_802CC1A4")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::land(r, gobj, landed)
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
    spawn.x3C_damage = 0;
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
    spawn.kind = kind;
    spawn.x0_parent_gobj = item(r.game, gobj).owner;
    spawn.x4_parent_gobj2 = Some(gobj);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
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

/// A flame reflected, `itLizardon_Logic37_Reflected` (`it_80273030`).
#[doc(alias = "itLizardon_Logic37_Reflected")]
pub fn flame_reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj)
}

/// A flame hitting a shield, `itLizardon_Logic37_HitShield`: gone.
#[doc(alias = "itLizardon_Logic37_HitShield")]
pub fn flame_hit_shield(_: &mut Running, _: GObjId) -> bool {
    true
}

/// A flame absorbed, `itLizardon_Logic37_Absorbed`: gone.
#[doc(alias = "itLizardon_Logic37_Absorbed")]
pub fn flame_absorbed(_: &mut Running, _: GObjId) -> bool {
    true
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

/// A flame's `it_802CC650`: gone once its life is out, else one less (`Item_TickLifetime`).
#[doc(alias = "it_802CC650")]
pub fn flame_anim(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    if reg(ip.xD44_lifeTimer) <= 0.0 {
        return true;
    }
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    false
}

/// A flame's `it_802CC684`: its speed shrinks by its attribute.
#[doc(alias = "it_802CC684")]
pub fn flame_phys(r: &mut Running, gobj: GObjId) {
    let k = reg(attr_f32(r, gobj, 4));
    let ip = item_mut(r.game, gobj);
    let v = ip.x40_vel;
    ip.x40_vel = Vec3::new(
        mem(fp::fmuls(reg(v.x), k)),
        mem(fp::fmuls(reg(v.y), k)),
        mem(fp::fmuls(reg(v.z), k)),
    );
}

/// A flame's `it_802CC6C4`: gone when it touches the stage.
#[doc(alias = "it_802CC6C4")]
pub fn flame_coll(r: &mut Running, gobj: GObjId) -> bool {
    air_touched_any(r, gobj)
}
