//! The Pokémon the Poké Ball lets out (`it_279C.c`'s Pokémon part): which one comes out
//! (`it_8027AB64` and the draws before it), and what every Pokémon does as it comes out of the
//! ball, growing to its size in steps, falling to the floor, its root bone's motion read back
//! as the item's own.
//!
//! The Pokémon kinds themselves are in `it::kinds`, a module each.

pub mod vars;

use gekko_fp as fp;

pub use vars::PokemonVars;

use super::item::{article, attr_of, spawn_air};
use super::{data, item, item_mut, jobj_of, lib26b1, lib2725};
use melee_data::archive::DataRef;

use crate::fpu::{mem, reg};
use crate::game::code;
use crate::game::ef::Arg;
use crate::game::gm::{RECORD_CEREBI, RECORD_MEW};
use crate::game::handles::CodeRef;
use crate::game::state::{Game, Running};
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::quat::Quat;
use crate::vec::Vec3;

use super::kinds::{PKIND_START, PKIND_TERMINATE};

/// `It_PKind_Marumine`, Electrode, which grows with no gravity.
const PKIND_MARUMINE: i32 = 0xB1;
/// `It_PKind_Metamon`, Ditto.
const PKIND_METAMON: i32 = 0xB4;
/// `It_PKind_Mew` and `It_PKind_Cerebi`, the rare ones.
const PKIND_MEW: i32 = 0xB7;
const PKIND_CEREBI: i32 = 0xB8;
/// `Pokemon_ID_Metamon` and `Pokemon_ID_Tosakinto` (`PokemonKind`, from Goldeen's 0).
const POKEMON_ID_METAMON: i32 = PKIND_METAMON - PKIND_START;
const POKEMON_ID_TOSAKINTO: i32 = 0;

/// The release's variables (`itPokemonSpawn_ItemVars`), by offset in the union.
pub mod spawn {
    /// The bone motion since the last frame (`x4`) and the bone's last translation (`x10`).
    pub const MOTION: usize = 0x4;
    pub const BONE: usize = 0x10;
    /// The fall out of the ball: its gravity and limit (`x1C`, `x20`) and frames left (`x24`).
    pub const FALL: usize = 0x1C;
    pub const FALL_MAX: usize = 0x20;
    pub const FALL_FRAMES: usize = 0x24;
    /// The growth's frames (`x28`), its sizes (`x2C`, six), frames per size (`x44`), the step
    /// to full size (`x48`), the step to the next size (`x4C`), frames to the next step
    /// (`x50`), how many sizes (`x54`), which size is next (`x58`) and the phase (`x5C`: 0
    /// growing, 1 pulsing through the sizes, 2 done).
    pub const GROW_FRAMES: usize = 0x28;
    pub const SIZES: usize = 0x2C;
    pub const SIZE_FRAMES: usize = 0x44;
    pub const GROW_STEP: usize = 0x48;
    pub const SIZE_STEP: usize = 0x4C;
    pub const SIZE_LEFT: usize = 0x50;
    pub const SIZE_COUNT: usize = 0x54;
    pub const SIZE_NEXT: usize = 0x58;
    pub const PHASE: usize = 0x5C;
}

/// A Pokémon's variables.
pub fn vars(game: &mut Game, gobj: GObjId) -> &mut PokemonVars {
    item_mut(game, gobj).xDD4_itemVar.pokemon()
}

/// The Poké Ball's own attributes (`itPokemonSpawn_DatAttrs`), word `off`.
fn ball_attr(r: &Running, ball: GObjId, off: u32) -> DataRef {
    let a = article(item(r.game, ball))
        .special(r.data)
        .expect("a Poké Ball without attributes");
    a.at(off)
}

/// The total weight of the Pokémon that can come out, the last two left out, plus one,
/// `it_8027A364`.
#[doc(alias = "it_8027A364")]
pub fn total_weight(r: &Running, ball: GObjId) -> i32 {
    let p = r.game.it.pokemon.clone();
    let mut total = 1i32;
    for kind in PKIND_START..PKIND_TERMINATE {
        if p.last_kind != kind && p.previous_kind != kind {
            let w = r.data.u32(ball_attr(r, ball, weight_off(kind))) as i32;
            total = total.wrapping_add(w);
        }
    }
    total
}

/// Where a Pokémon's weight is in the ball's attributes (`pokemon_spawn_weights`).
fn weight_off(kind: i32) -> u32 {
    0x3C + 4 * (kind - PKIND_START) as u32
}

/// A rare Pokémon, if the draws say so, once a match: Celebi (23) when the card records every
/// Pokémon trophy, then Mew (22) when every character is unlocked, each recorded on the card
/// (`gm_80165084`, `gm_80172C04`, `gm_80164ABC`, `gm_80172BC4`).
fn rare(r: &mut Running) -> Option<i32> {
    if r.game.objs.rng.below(251) == 0
        && r.game.it.pokemon.rare_spawned == 0
        && r.game.gm.unlocks.pokemon_trophies()
    {
        r.game.it.pokemon.rare_spawned = 1;
        r.record_rare_pokemon(RECORD_CEREBI);
        return Some(PKIND_CEREBI - PKIND_START);
    }
    if r.game.objs.rng.below(251) == 0
        && r.game.it.pokemon.rare_spawned == 0
        && r.game.gm.unlocks.all_characters()
    {
        r.game.it.pokemon.rare_spawned = 1;
        r.record_rare_pokemon(RECORD_MEW);
        return Some(PKIND_MEW - PKIND_START);
    }
    None
}

/// Draws a Pokémon by its weight, never the last two (`selectPokemonForOpening`'s draw):
/// which one, from Goldeen's 0.
fn draw_weighted(r: &mut Running, ball: GObjId) -> i32 {
    let total = total_weight(r, ball);
    let roll = r.game.objs.rng.below(total);
    let mut sum = 0i32;
    for kind in PKIND_START..PKIND_TERMINATE {
        let p = &r.game.it.pokemon;
        let (prev, last) = (p.previous_kind, p.last_kind);
        if last != kind && prev != kind {
            let w = r.data.u32(ball_attr(r, ball, weight_off(kind))) as i32;
            sum = sum.wrapping_add(w);
            if sum >= roll {
                let p = &mut r.game.it.pokemon;
                p.previous_kind = last;
                p.last_kind = kind;
                return kind - PKIND_START;
            }
        }
    }
    0
}

/// A random Pokémon for a match, `it_8027A4D4`: a rare one, or one by weight.
#[doc(alias = "it_8027A4D4")]
pub fn draw(r: &mut Running, ball: GObjId) -> i32 {
    if let Some(k) = rare(r) {
        return k;
    }
    draw_weighted(r, ball)
}

/// Which Pokémon comes out (`It_PKind_*` less `It_PKind_Start`), as `it_8027AB64` picks it.
fn choose(r: &mut Running, ball: GObjId) -> i32 {
    let forced = r.game.vs.start.x28;
    if forced != 0 {
        if forced == 1 {
            code::unported_call("it_8027A9B8 (an event's Pokémon list)");
        }
        return forced.wrapping_sub(PKIND_START);
    }
    if crate::game::gm::is_training(r.game) {
        code::unported_call("it_8027A780 (training's Pokémon)");
    }
    if r.game.game_mode == crate::game::gm::machine::GM_OPENING_MV {
        return draw_weighted(r, ball);
    }
    let mut kind = r.game.it.db_menu.CurrentlySelectedPokemon;
    if kind == POKEMON_ID_TOSAKINTO {
        kind = draw(r, ball);
    } else {
        kind -= 1;
    }
    if kind == POKEMON_ID_METAMON {
        code::unported_call("ftLib_CountFighters (Ditto's check)");
    }
    kind
}

/// The ball lets its Pokémon out, `it_8027AB64`: one is chosen, made above the ball facing the
/// ball's owner's opponents, given the release's variables, counted for the owner's player,
/// and its attack records begun. Whether one came out.
#[doc(alias = "it_8027AB64")]
pub fn release(r: &mut Running, ball: GObjId) -> bool {
    let kind = choose(r, ball);
    let ip = item(r.game, ball);
    let (pos, owner) = (ip.pos, ip.owner);
    let up = r.data.f32(ball_attr(r, ball, 8));
    let prev_pos = Vec3::new(pos.x, pos.y, 0.0);
    let center = lib26b1::ecb_center(r.game, ball);
    let facing = super::owner::opponents_dir(r.game, r.data, prev_pos, owner);
    let mut spawn = SpawnItem {
        vel: Vec3::new(0.0, up, 0.0),
        prev_pos,
        pos: center,
        x3C_damage: 0,
        facing_dir: facing,
        kind: kind.wrapping_add(PKIND_START),
        x0_parent_gobj: owner,
        x4_parent_gobj2: Some(ball),
        x40: 0,
        ..Default::default()
    };
    spawn.x44_flag[0] |= 0x80;
    if spawn.kind == PKIND_CEREBI {
        r.record_rare_pokemon(RECORD_CEREBI);
    }
    if spawn.kind == PKIND_MEW {
        r.record_rare_pokemon(RECORD_MEW);
    }
    let Some(gobj) = spawn_air(r, &mut spawn) else {
        return false;
    };
    set_release_vars(r, ball, gobj, spawn.kind);
    count_for_owner(r, gobj);
    super::item::debug_owner_bubbles(r.game, gobj, owner);
    super::kinds::wstar::set_attack_flags(r, gobj, 0x0044_0060);
    count_attack(r, gobj);
    true
}

/// Counts a Pokémon (`It_PKind_*`) in its owner's player's records, `pl_8003E70C`.
#[doc(alias = "pl_8003E70C")]
pub fn count_for_owner(r: &mut Running, gobj: GObjId) {
    let ip = item(r.game, gobj);
    let (kind, owner) = (ip.kind, ip.owner);
    assert!(
        (PKIND_START..PKIND_TERMINATE).contains(&kind),
        "It_PKind_Start <= itGetKind(igobj) && itGetKind(igobj) < It_PKind_Terminate"
    );
    if !super::item::is_fighter(r.game, owner) {
        return;
    }
    let slot = r.game.fighter(owner.expect("an owner")).player_idx as usize;
    let n = &mut r.game.players[slot].stale_moves.x848[(kind - PKIND_START) as usize];
    *n = n.wrapping_add(1);
}

/// Counts the item's attack in its fighter owner's player's statistics, `it_8027B564`
/// (`pl_80037DF4`).
#[doc(alias = "it_8027B564")]
#[doc(alias = "pl_80037DF4")]
pub fn count_attack(r: &mut Running, gobj: GObjId) {
    let ip = item(r.game, gobj);
    let (owner, ev) = (ip.owner, ip.xD90);
    if !super::item::is_fighter(r.game, owner) {
        return;
    }
    let slot = r.game.fighter(owner.expect("an owner")).player_idx as usize;
    let a = &mut r.game.players[slot]
        .stale_moves
        .total_attack_count_struct
        .attacks;
    a.total = a.total.wrapping_add(1);
    let id = ev[3] as usize;
    a.by_attack_counts[id] = a.by_attack_counts[id].wrapping_add(1);
    for (mask, n) in [
        (0x08, &mut a.thrown_item_count),
        (0x04, &mut a.aerials_count),
        (0x01, &mut a.specials_count),
        (0x02, &mut a.x1A0_count),
        (0x10, &mut a.x1A4_count),
    ] {
        if ev[2] & mask != 0 {
            *n = n.wrapping_add(1);
        }
    }
}

/// The release's variables from the ball's attributes, `it_8027AAA0`: the fall (none for
/// Electrode), the growth to the Pokémon's size and the sizes it pulses through.
#[doc(alias = "it_8027AAA0")]
pub fn set_release_vars(r: &mut Running, ball: GObjId, gobj: GObjId, kind: i32) {
    let a = |off| r.data.f32(ball_attr(r, ball, off));
    let (fall, fall_max) = if kind == PKIND_MARUMINE {
        (0.0, 0.0)
    } else {
        (a(0xC), a(0x10))
    };
    let (frames, grow) = (a(0x14), a(0x18));
    let (size_frames, sizes_n) = (a(0x34), r.data.u32(ball_attr(r, ball, 0x38)) as i32);
    let sizes: Vec<f32> = (0..sizes_n.max(0) as u32)
        .map(|i| a(0x1C + 4 * i))
        .collect();
    let scale = attr_of(item(r.game, gobj)).f32(r.data, data::attr::SCALE);
    let v = vars(r.game, gobj);
    v.set_f32(spawn::FALL, fall);
    v.set_f32(spawn::FALL_MAX, fall_max);
    v.set_f32(spawn::FALL_FRAMES, frames);
    v.set_f32(spawn::GROW_FRAMES, grow);
    let step = fp::fdivs(
        fp::fsubs(reg(scale), reg(v.f32(spawn::SIZE_STEP))),
        reg(v.f32(spawn::GROW_FRAMES)),
    );
    v.set_f32(spawn::GROW_STEP, mem(step));
    v.set_f32(spawn::SIZE_FRAMES, size_frames);
    v.set_i32(spawn::SIZE_COUNT, sizes_n);
    v.set_f32(spawn::PHASE, 0.0);
    v.set_f32(spawn::SIZE_NEXT, 0.0);
    v.set_f32(spawn::SIZE_LEFT, 0.0);
    for (i, s) in sizes.into_iter().enumerate() {
        v.set_f32(spawn::SIZES + 4 * i, mem(fp::fmuls(reg(s), reg(scale))));
    }
}

/// Faces the nearest opponent of its owner, or a random way with none, `it_80279C48`.
#[doc(alias = "it_80279C48")]
pub fn face_nearest_opponent(r: &mut Running, gobj: GObjId) {
    let ip = item(r.game, gobj);
    let (pos, owner) = (ip.pos, ip.owner);
    if let Some(g) = super::owner::nearest_opponent(r.game, r.data, pos, owner) {
        let v = r.game.ft_pos(g);
        let d = fp::fsubs(reg(item(r.game, gobj).pos.x), reg(v.x));
        item_mut(r.game, gobj).facing_dir = if d > 0.0 { -1.0 } else { 1.0 };
        return;
    }
    let dir = if r.game.objs.rng.below(2) != 0 {
        1.0
    } else {
        -1.0
    };
    item_mut(r.game, gobj).facing_dir = dir;
}

/// A Pokémon's start out of the ball, `it_80279CDC`: no bone motion, its first size, no
/// pickup, blinking, no longer kept clear of.
#[doc(alias = "it_80279CDC")]
pub fn start(r: &mut Running, gobj: GObjId, scale: f32) {
    clear_motion(r.game, gobj);
    set_size(r, gobj, scale);
    lib26b1::disable_pickup(r.game, gobj);
    super::hitbox::start_blink(r.game, gobj);
    lib26b1::stop_keeping_clear(r.game, gobj);
}

/// A hit Pokémon's launch along its knockback, `it_80279D38` (`it_8027B964`).
#[doc(alias = "it_80279D38")]
pub fn launch(r: &mut Running, gobj: GObjId) {
    super::zako::launch(r, gobj, false);
}

/// Its first size, `it_80279D5C`: the model scaled, and growing (phase 0).
#[doc(alias = "it_80279D5C")]
pub fn set_size(r: &mut Running, gobj: GObjId, scale: f32) {
    let j = jobj_of(r.game, gobj).expect("a Pokémon without a model");
    let v = vars(r.game, gobj);
    v.set_f32(spawn::SIZE_STEP, scale);
    v.set_i32(spawn::PHASE, 0);
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| {
        o.scale = Vec3::new(scale, scale, scale)
    });
}

/// Adds to a joint's scale in all three axes, `it_80272DE4`.
#[doc(alias = "it_80272DE4")]
pub fn add_scale(game: &mut Game, j: JObjId, d: f32) {
    let jobjs = &mut game.objs.jobjs;
    jobj::set_srt(jobjs, j, |o| {
        o.scale.x = mem(fp::fadds(reg(o.scale.x), reg(d)))
    });
    jobj::set_srt(jobjs, j, |o| {
        o.scale.y = mem(fp::fadds(reg(o.scale.y), reg(d)))
    });
    jobj::set_srt(jobjs, j, |o| {
        o.scale.z = mem(fp::fadds(reg(o.scale.z), reg(d)))
    });
}

/// Pulses through the sizes, `it_80279E24`: each size reached sets the next step, and after
/// the last the growth is done.
#[doc(alias = "it_80279E24")]
pub fn pulse(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj).expect("a Pokémon without a model");
    let scale = r.game.objs.jobjs[j].scale.x;
    let v = vars(r.game, gobj);
    if reg(v.f32(spawn::SIZE_LEFT)) <= 0.0 {
        let f = v.f32(spawn::SIZE_FRAMES);
        v.set_f32(spawn::SIZE_LEFT, f);
        let next = v.i32(spawn::SIZE_NEXT);
        let size = reg(v.f32(spawn::SIZES + 4 * next as usize));
        let reached = if next & 1 != 0 {
            size <= reg(scale)
        } else {
            size >= reg(scale)
        };
        if reached {
            // The inline: the size taken exactly, the step to the one after.
            let now = r.game.objs.jobjs[j].scale.x;
            let target = vars(r.game, gobj).f32(spawn::SIZES + 4 * next as usize);
            lib2725::set_uniform_scale(r.game, j, target);
            let v = vars(r.game, gobj);
            let next = next.wrapping_add(1);
            v.set_i32(spawn::SIZE_NEXT, next);
            let after = reg(v.f32(spawn::SIZES + 4 * next as usize));
            let step = fp::fdivs(fp::fsubs(after, reg(now)), reg(v.f32(spawn::SIZE_FRAMES)));
            v.set_f32(spawn::SIZE_STEP, mem(step));
        }
        let v = vars(r.game, gobj);
        if v.i32(spawn::SIZE_COUNT) <= v.i32(spawn::SIZE_NEXT) {
            v.set_i32(spawn::PHASE, 2);
            return;
        }
    }
    let v = vars(r.game, gobj);
    let left = v.f32(spawn::SIZE_LEFT);
    v.set_f32(spawn::SIZE_LEFT, mem(fp::fsubs(reg(left), 1.0)));
    let step = v.f32(spawn::SIZE_STEP);
    add_scale(r.game, j, step);
}

/// Grows out of the ball, `it_80279FF8`: to full size by its step, then through the sizes.
#[doc(alias = "it_80279FF8")]
pub fn grow(r: &mut Running, gobj: GObjId) {
    let phase = vars(r.game, gobj).i32(spawn::PHASE);
    if phase == 0 {
        let j = jobj_of(r.game, gobj).expect("a Pokémon without a model");
        let full = attr_of(item(r.game, gobj)).f32(r.data, data::attr::SCALE);
        let step = vars(r.game, gobj).f32(spawn::GROW_STEP);
        let scale = r.game.objs.jobjs[j].scale.x;
        if reg(full) > fp::fadds(reg(step), reg(scale)) {
            add_scale(r.game, j, step);
            return;
        }
        lib2725::set_uniform_scale(r.game, j, full);
        vars(r.game, gobj).set_i32(spawn::PHASE, 1);
        return;
    }
    if phase != 2 {
        pulse(r, gobj);
    }
}

/// Falls out of the ball, `it_8027A09C`: true once its frames are up, at full size.
#[doc(alias = "it_8027A09C")]
pub fn fall(r: &mut Running, gobj: GObjId) -> bool {
    let v = vars(r.game, gobj);
    let (g, max) = (v.f32(spawn::FALL), v.f32(spawn::FALL_MAX));
    lib2725::fall(r.game, gobj, g, max);
    let v = vars(r.game, gobj);
    let left = v.f32(spawn::FALL_FRAMES);
    if reg(left) <= 0.0 {
        let j = jobj_of(r.game, gobj).expect("a Pokémon without a model");
        let full = attr_of(item(r.game, gobj)).f32(r.data, data::attr::SCALE);
        lib2725::set_uniform_scale(r.game, j, full);
        return true;
    }
    v.set_f32(spawn::FALL_FRAMES, mem(fp::fsubs(reg(left), 1.0)));
    false
}

/// Lands out of the ball into `landed`, `it_8027A118`.
#[doc(alias = "it_8027A118")]
pub fn land(r: &mut Running, gobj: GObjId, landed: super::ground::Event) -> bool {
    super::ground::fall_to_floor_rescaled(r, gobj, landed);
    false
}

/// No bone motion yet, `it_8027A13C`.
#[doc(alias = "it_8027A13C")]
pub fn clear_motion(game: &mut Game, gobj: GObjId) {
    let v = vars(game, gobj);
    v.set_vec(spawn::BONE, Vec3::new(0.0, 0.0, 0.0));
    v.set_vec(spawn::MOTION, Vec3::new(0.0, 0.0, 0.0));
}

/// Takes the root bone's translation as motion, `it_8027A160`: its change since the last
/// frame, at the item's scale and with what's under a thousandth dropped, and the bone put back
/// at the origin.
#[doc(alias = "it_8027A160")]
pub fn take_bone_motion(r: &mut Running, bone: Option<JObjId>, gobj: GObjId) {
    let Some(bone) = bone else {
        return;
    };
    let scale = reg(attr_of(item(r.game, gobj)).f32(r.data, data::attr::SCALE));
    let t = r.game.objs.jobjs[bone].translate;
    let snap = |c: f32| {
        let s = fp::fmuls(reg(c), scale);
        let a = if s < 0.0 { fp::fneg(s) } else { s };
        if a < reg(0.001f32) { 0.0 } else { mem(s) }
    };
    let now = Vec3::new(snap(t.x), snap(t.y), snap(t.z));
    let v = vars(r.game, gobj);
    let last = v.vec(spawn::BONE);
    let d = Vec3::new(
        mem(fp::fsubs(reg(now.x), reg(last.x))),
        mem(fp::fsubs(reg(now.y), reg(last.y))),
        mem(fp::fsubs(reg(now.z), reg(last.z))),
    );
    v.set_vec(spawn::MOTION, d);
    v.set_vec(spawn::BONE, now);
    jobj::set_translate(&mut r.game.objs.jobjs, bone, Vec3::new(0.0, 0.0, 0.0));
}

/// Moves by the bone's motion, `it_8027A344`: its z ahead, its y up.
#[doc(alias = "it_8027A344")]
pub fn move_by_bone(game: &mut Game, gobj: GObjId) {
    let m = vars(game, gobj).vec(spawn::MOTION);
    let ip = item_mut(game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(m.z), reg(ip.facing_dir)));
    ip.x40_vel.y = m.y;
}

/// An effect on `j` at the item's size over `size`, `it_8027ADEC`.
#[doc(alias = "it_8027ADEC")]
pub fn effect_scaled(r: &mut Running, id: i32, gobj: GObjId, j: Option<JObjId>, size: f32) {
    let scale = attr_of(item(r.game, gobj)).f32(r.data, data::attr::SCALE);
    let s = mem(fp::fmuls(reg(scale), fp::fdivs(1.0, reg(size))));
    crate::game::ef::spawn(r, id, Some(gobj), &[Arg::JObj(j), Arg::Float(s)]);
}

/// Bounces off a wall, `it_8027AE34`: its x speed turned back by its bounce, facing the other
/// way, its life back to its half life, turned to face its way.
#[doc(alias = "it_8027AE34")]
pub fn bounce_off_wall(r: &mut Running, gobj: GObjId) -> bool {
    let j = jobj_of(r.game, gobj).expect("a Pokémon without a model");
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(fp::fneg(reg(ip.x40_vel.x)), reg(ip.xC70)));
    ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
    ip.xD44_lifeTimer = ip.xD48_halfLifeTimer;
    let facing = ip.facing_dir;
    set_facing_rotation(r.game, j, facing);
    false
}

/// Bounces off a surface, `it_8027AF50`: its speed mirrored about the surface's normal, facing
/// its motion, turned to face its way.
#[doc(alias = "it_8027AF50")]
pub fn bounce_off_surface(r: &mut Running, gobj: GObjId) -> bool {
    let j = jobj_of(r.game, gobj).expect("a Pokémon without a model");
    let ip = item_mut(r.game, gobj);
    let n = ip.xC58;
    ip.x40_vel.mirror_xy(n);
    lib2725::face_motion(r.game, gobj);
    let facing = item(r.game, gobj).facing_dir;
    set_facing_rotation(r.game, j, facing);
    false
}

/// The zero rotation (`it_803B8588`, `it_803B8598`) with its y a quarter turn per unit of
/// facing, in double precision.
fn set_facing_rotation(game: &mut Game, j: JObjId, facing: f32) {
    let y = mem(fp::frsp(fp::fmul(std::f64::consts::FRAC_PI_2, reg(facing))));
    let q = Quat {
        x: 0.0,
        y,
        z: 0.0,
        w: 0.0,
    };
    jobj::set_rotation(&mut game.objs.jobjs, j, q);
}

/// Joint `i` of the item's bone table (`xBBC_dynamicBoneTable->bones[i]`).
pub fn table_bone(game: &Game, gobj: GObjId, i: usize) -> Option<JObjId> {
    let t = item(game, gobj)
        .xBBC_dynamicBoneTable
        .expect("a Pokémon without its bone table");
    game.it.bone_tables[t.0].bones[i]
}

/// The kind's own attributes (`xC4_article_data->x4_specialAttributes`), word `off` as a
/// float.
pub fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a Pokémon without attributes");
    r.data.f32(a.at(off))
}

/// The kind's own attributes, word `off` as an integer.
pub fn attr_i32(r: &Running, gobj: GObjId, off: u32) -> i32 {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a Pokémon without attributes");
    r.data.u32(a.at(off)) as i32
}

/// Its effects pause in hitlag and resume after (`Item_SetEffectHitlagCallbacks`).
pub fn effect_hitlag(game: &mut Game, gobj: GObjId) {
    let ip = item_mut(game, gobj);
    ip.entered_hitlag = Some(CodeRef(code::EF_PAUSE_ALL));
    ip.exited_hitlag = Some(CodeRef(code::EF_RESUME_ALL));
}

/// Into its first state in the air, posed at its first frame
/// (`Item_EnterAirStateWithHitlagAndStateDesc`).
pub fn enter_first_state(r: &mut Running, gobj: GObjId) {
    super::maplib::enter_air(item_mut(r.game, gobj));
    super::item::change_state(r, gobj, 0, super::item::change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
    super::kinds::drmariopill::set_pose(r, gobj, 0, 0.0);
}

/// Runs an item callback the Pokémon set, if `addr` is one: their effects' hitlag hooks.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        code::EF_PAUSE_ALL => crate::game::ef::pause_all(r.game, Some(gobj)),
        code::EF_RESUME_ALL => crate::game::ef::resume_all(r.game, Some(gobj)),
        _ => return false,
    }
    true
}
