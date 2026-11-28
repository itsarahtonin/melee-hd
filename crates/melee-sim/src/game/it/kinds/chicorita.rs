//! Chikorita (`It_PKind_Chicorita`), `melee/it/kinds/itchicorita.c`: out of the ball it lands
//! facing its owner's nearest opponent and fires Razor Leaf a number of times, each leaf (its
//! script says when) an item flying ahead, then leaves. The way out, the rounds and the leaf's
//! launch are [`shooter`]'s, over [`CHIKORITA`] and [`LEAF`].
//!
//! The unit also holds the leaf's code (`It_Kind_Chicorita_Leaf`): it flies straight until its
//! life runs out or it touches the stage.

use crate::mp::coll::GroundOrAir;
use gekko_fp as fp;

use super::super::item::{attr_of, change, change_state, play_sfx};
use super::super::pokemon::{self, attr_f32};
use super::super::{data, item, item_mut, jobj_of, lib26b1, lib2725};
use super::shooter::{self, Release, Rounds, Shot, ShotSound};
use crate::fpu::{mem, reg};
use crate::game::it::ItKind;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const SPAWNED: u32 = 0x802C_9588;
pub const LANDED_NOTHING: u32 = 0x802C_9618;
#[doc(alias("EVT_UNK"))]
pub const FORGET: u32 = 0x802C_961C;
pub const ANIM0: u32 = 0x802C_96CC;
pub const PHYS0: u32 = 0x802C_9798;
pub const COLL0: u32 = 0x802C_9850;
pub const FIRE_ACCESSORY: u32 = 0x802C_989C;
pub const ANIM1: u32 = 0x802C_990C;
pub const PHYS1: u32 = 0x802C_9970;
pub const COLL1: u32 = 0x802C_9A28;
pub const ANIM2: u32 = 0x802C_9AB4;
pub const PHYS2: u32 = 0x802C_9AD8;
pub const COLL2: u32 = 0x802C_9AF8;
pub const LEAF_SPAWNED: u32 = 0x802C_9C20;
pub const LEAF_KEPT: u32 = 0x802C_9C78;
#[doc(alias("LEAF_EVT_UNK"))]
pub const LEAF_FORGET: u32 = 0x802C_9C80;
pub const LEAF_REFLECTED: u32 = 0x802C_9CA0;
pub const LEAF_ANIM: u32 = 0x802C_9CE8;
pub const LEAF_PHYS: u32 = 0x802C_9D1C;
pub const LEAF_COLL: u32 = 0x802C_9D20;

/// Its own fall speed (`itChicorita_ItemVars`' `x64`, past the leaves left, [`shooter::ROUNDS`]).
const FALL_SPEED: usize = 0x64;

/// Chikorita's way out, its rounds of leaves (state 0, no stand) and leaving (state 1); none of
/// its states pause its effects in hitlag.
pub const CHIKORITA: Rounds = Rounds {
    release: Release {
        out: 2,
        hitlag: false,
        posed: false,
        landed,
        fallen: None,
    },
    stand: None,
    fire: 0,
    leave: 1,
    accessory: FIRE_ACCESSORY,
    first_round: shooter::rounds_from_attrs,
    on_round: None,
    each_frame: Some(bone_motion),
    stops: false,
};

/// Its leaf: `x8` ahead, `xC` up, at `x10`, with its own sound.
pub const LEAF: Shot = Shot {
    kind: ItKind::ChikoritaLeaf,
    ahead: 8,
    up: 0xC,
    speed: 0x10,
    flagged: true,
    depth: None,
    sound: ShotSound::Sfx(0x2710),
};

/// Runs one of Chikorita's (or its leaf's) predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 => shooter::rounds(r, gobj, &CHIKORITA),
        COLL0 | COLL1 => shooter::stays(r, gobj),
        ANIM1 => shooter::leaving(r, gobj, &CHIKORITA),
        ANIM2 => shooter::growing(r, gobj),
        COLL2 => shooter::landing(r, gobj, &CHIKORITA.release),
        LEAF_KEPT => shooter::kept(r, gobj),
        LEAF_REFLECTED => shooter::reflected(r, gobj),
        LEAF_ANIM => shooter::life_ticks(r, gobj),
        LEAF_COLL => shooter::gone_on_touch(r, gobj),
        _ => return None,
    })
}

/// Runs one of Chikorita's (or its leaf's) events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        LANDED_NOTHING | LEAF_PHYS => shooter::nothing(r, gobj),
        PHYS0 | PHYS1 => phys(r, gobj),
        FIRE_ACCESSORY => shooter::fire_when_told(r, gobj, shoot, None),
        PHYS2 => shooter::falling(r, gobj, &CHIKORITA.release),
        LEAF_SPAWNED => shooter::shot_out(r, gobj, leaf_fly, uncounted),
        _ => return false,
    }
    true
}

/// `it_802C9588`, its spawn: facing its owner's nearest opponent, no leaves yet, its script's
/// flag clear, at its first size, not falling, into the air out of the ball, its sound.
#[doc(alias = "it_802C9588")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    pokemon::face_nearest_opponent(r, gobj);
    pokemon::vars(r.game, gobj).set_i32(shooter::ROUNDS, -1);
    item_mut(r.game, gobj).cmd_flags[0] &= !0x80;
    let size = attr_f32(r, gobj, 0);
    pokemon::start(r, gobj, size);
    pokemon::vars(r.game, gobj).set_f32(FALL_SPEED, 0.0);
    shooter::enter_air(r, gobj, &CHIKORITA.release);
    play_sfx(r, gobj, 0x2711, 0x7F, 0x40);
}

/// Forgets a GObj going away, `itChicorita_Logic1_EvtUnk` and the leaf's
/// `itChicoritaLeaf_Logic30_EvtUnk` (`it_8026B894`).
#[doc(alias = "itChicorita_Logic1_EvtUnk")]
#[doc(alias = "itChicoritaLeaf_Logic30_EvtUnk")]
#[doc(alias("evt_unk"))]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// Lands out of the ball ([`shooter::landed`]).
fn landed(r: &mut Running, gobj: GObjId) {
    shooter::landed(r, gobj, &CHIKORITA);
}

/// Fires a leaf ([`shooter::shoot`]).
fn shoot(r: &mut Running, gobj: GObjId) {
    shooter::shoot(r, gobj, &LEAF);
}

/// Its root joint's child's motion taken as its own.
fn bone_motion(r: &mut Running, gobj: GObjId) {
    let bone = jobj_of(r.game, gobj).and_then(|j| r.game.objs.jobjs[j].child);
    pokemon::take_bone_motion(r, bone, gobj);
}

/// `itChicorita_UnkMotion0_Phys` and `_1_Phys` (the inline `itChicorita_Phys`): the bone's
/// motion; in the air it falls by its own speed, sped up by its gravity unless already past
/// the limit against it; on the floor it doesn't fall.
#[doc(alias = "itChicorita_UnkMotion0_Phys")]
#[doc(alias = "itChicorita_UnkMotion1_Phys")]
pub fn phys(r: &mut Running, gobj: GObjId) {
    pokemon::move_by_bone(r.game, gobj);
    if item(r.game, gobj).ground_or_air != GroundOrAir::Air {
        pokemon::vars(r.game, gobj).set_f32(FALL_SPEED, 0.0);
        return;
    }
    let a = attr_of(item(r.game, gobj));
    let (g, max) = (
        a.f32(r.data, data::attr::FALL_SPEED),
        a.f32(r.data, data::attr::FALL_SPEED_MAX),
    );
    let g_dir = if reg(g) < 0.0 { -1 } else { 1 };
    let v = pokemon::vars(r.game, gobj);
    let mut s = reg(v.f32(FALL_SPEED));
    let s_dir = if s < 0.0 { -1 } else { 1 };
    let speed_up = if s_dir != g_dir {
        if s < 0.0 {
            s = fp::fneg(s);
        }
        s < reg(max)
    } else {
        true
    };
    if speed_up {
        let s = v.f32(FALL_SPEED);
        v.set_f32(FALL_SPEED, mem(fp::fsubs(reg(s), reg(g))));
    }
    let s = v.f32(FALL_SPEED);
    item_mut(r.game, gobj).vel.y = s;
}

/// The leaf's hits not counted.
fn uncounted(r: &mut Running, gobj: GObjId) {
    lib2725::uncount_hits(r.game, gobj);
}

/// The leaf flies, `it_802C9CC0`.
#[doc(alias = "it_802C9CC0")]
pub fn leaf_fly(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}
