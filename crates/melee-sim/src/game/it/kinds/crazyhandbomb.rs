//! Crazy Hand's bombs, `melee/it/kinds/itcrazyhandbomb.c`: dropped from his fingers, they
//! fall tumbling (state 0) and burst when they land or hit (state 1), hidden while the blast
//! plays out.

use gekko_fp as fp;

use super::super::item::{change, change_state, debug_owner_bubbles, spawn_air};
use super::super::{hitbox, item, item_mut, jobj_of, lib26b1, lib2725, maplib};
use crate::fpu::{mem, reg};
use crate::game::handles::CodeRef;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

/// `It_Kind_CrazyHand_Bomb`.
pub const KIND: i32 = 129;

#[doc(alias = "itCrazyhandbomb_UnkMotion0_Anim")]
pub const ANIM_0: u32 = 0x802F_1168;
#[doc(alias = "itCrazyhandbomb_UnkMotion0_Phys")]
pub const PHYS_0: u32 = 0x802F_1170;
#[doc(alias = "itCrazyhandbomb_UnkMotion0_Coll")]
pub const COLL_0: u32 = 0x802F_1304;
#[doc(alias = "it_802F1340")]
pub const ACCESSORY_0: u32 = 0x802F_1340;
#[doc(alias = "itCrazyhandbomb_UnkMotion1_Anim")]
pub const ANIM_1: u32 = 0x802F_1384;
#[doc(alias = "itCrazyhandbomb_UnkMotion1_Phys")]
pub const PHYS_1: u32 = 0x802F_13A4;
#[doc(alias = "itCrazyhandbomb_UnkMotion1_Coll")]
pub const COLL_1: u32 = 0x802F_13A8;
#[doc(alias = "it_802F13B0")]
pub const ACCESSORY_1: u32 = 0x802F_13B0;
#[doc(alias = "itCrazyHandBomb_Logic86_DmgDealt")]
pub const DMG_DEALT: u32 = 0x802F_10B4;
// `itCrazyHandBomb_Logic86_Reflected`: dead, the bomb's boxes are never reflected
// (ledger/dead.txt).
pub const REFLECTED: u32 = 0x802F_10D8;

/// `itCrazyHandBombAttributes`: the drop's first speed (`x0`), and its fall's acceleration
/// and top speed (`x4`, `x8`).
mod attr {
    pub const SPEED: u32 = 0x0;
    pub const FALL_ACCEL: u32 = 0x4;
    pub const FALL_MAX: u32 = 0x8;
}

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let a = super::super::item::article(item(r.game, gobj))
        .special(r.data)
        .expect("a bomb without attributes");
    r.data.f32(a.at(off))
}

/// A bomb from Crazy Hand (`owner`) at `pos`, `it_802F0F6C`, with a first collision from
/// `prev_pos`.
#[doc(alias = "it_802F0F6C")]
pub fn spawn(
    r: &mut Running,
    owner: GObjId,
    prev_pos: Vec3,
    pos: Vec3,
    kind: i32,
    facing_dir: f32,
) {
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    spawn.prev_pos = prev_pos;
    spawn.pos = pos;
    // `Item_InitSpawnCommonFields`, with a first collision.
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(owner);
    spawn.x4_parent_gobj2 = Some(owner);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let g = spawn_air(r, &mut spawn).expect("a bomb's spawn reads a null item");
    start(r, g);
    debug_owner_bubbles(r.game, g, Some(owner));
    lib2725::first_frame(r, g);
}

/// Falling, `it_802F10F8`: in the air, tumbling, its first speed up.
#[doc(alias = "it_802F10F8")]
pub fn start(r: &mut Running, gobj: GObjId) {
    let speed = attr_f32(r, gobj, attr::SPEED);
    maplib::enter_air(item_mut(r.game, gobj));
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    let ip = item_mut(r.game, gobj);
    ip.on_accessory = Some(CodeRef(ACCESSORY_0));
    ip.x40_vel.y = speed;
}

/// `itCrazyhandbomb_UnkMotion0_Phys`: the fall, and a tumble about one axis at random by ten
/// degrees for each unit of its fall's speed.
fn phys_0(r: &mut Running, gobj: GObjId) {
    let (accel, max) = (
        attr_f32(r, gobj, attr::FALL_ACCEL),
        attr_f32(r, gobj, attr::FALL_MAX),
    );
    lib2725::fall(r.game, gobj, accel, max);
    let j = jobj_of(r.game, gobj).expect("a bomb without its model");
    let mut q = r.game.objs.jobjs[j].rotate;
    let vy = item(r.game, gobj).x40_vel.y;
    let turn = |v: f32| {
        mem(fp::fmadds(
            f64::from(0.017_453_292_f32),
            fp::fmuls(10.0, reg(vy)),
            reg(v),
        ))
    };
    match r.game.objs.rng.below(3) {
        0 => q.x = turn(q.x),
        1 => q.y = turn(q.y),
        2 => q.z = turn(q.z),
        _ => {}
    }
    jobj::set_rotation(&mut r.game.objs.jobjs, j, q);
}

/// The burst once it lands, `it_802F1030`: hidden, not pickable, vanishing, still, the
/// blast, blinking.
#[doc(alias = "it_802F1030")]
pub fn land(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    lib26b1::set_xdd0_b3(r.game, gobj);
    lib2725::start_vanishing(r.game, r.data, gobj);
    lib2725::clear_velocity(r.game, gobj);
    lib2725::blast_effect(r, gobj);
    hitbox::start_blink(r.game, gobj);
    burst(r, gobj);
}

/// Bursting, `it_802F1344` (and on a hit, `itCrazyHandBomb_Logic86_DmgDealt`).
#[doc(alias = "it_802F1344")]
pub fn burst(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(ACCESSORY_1));
}

pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM_0 | COLL_1 => false,
        COLL_0 => {
            if item(r.game, gobj).ground_or_air == 1 {
                super::super::ground::fall_until_touch_unstick(r, gobj, land);
            }
            false
        }
        ANIM_1 => lib2725::tick_life(r.game, gobj),
        DMG_DEALT => {
            burst(r, gobj);
            true
        }
        REFLECTED => lib2725::reflected_back(r.game, gobj),
        _ => return None,
    })
}

pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PHYS_0 => phys_0(r, gobj),
        PHYS_1 | ACCESSORY_0 | ACCESSORY_1 => {}
        _ => return false,
    }
    true
}
