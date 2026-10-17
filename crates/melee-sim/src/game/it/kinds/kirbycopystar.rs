//! The star a hit knocks Kirby's copy out as, `melee/it/kinds/it_2ADA.c`: thrown up and ahead
//! of him with no owner, it falls until its lifetime runs out or it touches the stage.

use super::super::item::{article, attr_of, change, change_state, spawn_air};
use super::super::{data, item, item_mut, lib26b1, lib2725};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `It_Kind_Unk1`.
pub const KIND: i32 = super::KIRBY_COPY_STAR;

pub const ANIM: u32 = 0x802A_DB48;
pub const PHYS: u32 = 0x802A_DB80;
pub const COLL: u32 = 0x802A_DBB0;

/// Runs one of the star's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => anim(r, gobj),
        COLL => coll(r, gobj),
        _ => return None,
    })
}

/// Runs one of the star's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PHYS => phys(r, gobj),
        _ => return false,
    }
    true
}

/// Throws the star from `pos` at `vel`, `it_802ADA1C`: its lifetime from its attributes.
#[doc(alias = "it_802ADA1C")]
pub fn spawn(r: &mut Running, pos: Vec3, vel: Vec3, facing_dir: f32) {
    let mut spawn = SpawnItem {
        kind: KIND,
        ..Default::default()
    };
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = spawn.prev_pos;
    // `Item_InitSpawnCommonFields`.
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = vel;
    spawn.x0_parent_gobj = None;
    spawn.x4_parent_gobj2 = None;
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let Some(gobj) = spawn_air(r, &mut spawn) else {
        return;
    };
    let attrs = article(item(r.game, gobj))
        .special(r.data)
        .expect("a copy star without attributes");
    let lifetime = r.data.f32(attrs.at(0));
    lib2725::set_lifetime(r.game, r.data, gobj, lifetime);
    setup(r, gobj);
}

/// Its only state, `it_802ADAF0`: out of reach of a grab.
#[doc(alias = "it_802ADAF0")]
fn setup(r: &mut Running, gobj: GObjId) {
    lib26b1::disable_pickup(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    super::super::set_flag(&mut item_mut(r.game, gobj).xDCE_flag, 7, false);
}

/// `it_2ADA_UnkMotion0_Anim`: gone when its lifetime runs out.
#[doc(alias = "it_2ADA_UnkMotion0_Anim")]
fn anim(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::tick_lifetime(r.game, gobj)
}

/// `it_2ADA_UnkMotion0_Phys`: gravity.
#[doc(alias = "it_2ADA_UnkMotion0_Phys")]
fn phys(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, data::attr::FALL_SPEED);
    let max = a.f32(r.data, data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
}

/// `it_2ADA_UnkMotion0_Coll`: gone when it touches the stage.
#[doc(alias = "it_2ADA_UnkMotion0_Coll")]
fn coll(r: &mut Running, gobj: GObjId) -> bool {
    super::super::ground::air_touched_any(r, gobj)
}

// `it_802ADBE4`, its `evt_unk` (`it_8026B894`), isn't ported: nothing dispatches the items'
// `evt_unk` yet.
