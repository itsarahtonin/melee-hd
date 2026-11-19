//! Pikachu's and Pichu's copies (`ftkirbyspecialpikachu.c`): the Thunder Jolt thrown ahead of
//! him when the script raises its flag, on the ground or in the air. Pikachu's own code
//! ([`ft::pikachu::special_n`](super::super::pikachu::special_n)) over his states and his
//! attributes for each copy, the kind his copy's ([`KIRBY`]); only his copies' addresses are
//! here.

use super::super::super::state::Running;
use super::super::pikachu::special_n::ThunderJolt;
use super::{KindFrom, attr, ms};
use crate::hsd::gobj::GObjId;

/// The copies' function addresses.
pub(super) mod addr {
    pub const ENTER: u32 = 0x800F_9FD4;
    pub const AIR_ENTER: u32 = 0x800F_A064;
    pub const N_ANIM: u32 = 0x800F_A0F4;
    pub const AIR_N_ANIM: u32 = 0x800F_A254;
    pub const N_IASA: u32 = 0x800F_A418;
    pub const AIR_N_IASA: u32 = 0x800F_A41C;
    pub const N_PHYS: u32 = 0x800F_A420;
    pub const AIR_N_PHYS: u32 = 0x800F_A440;
    pub const N_COLL: u32 = 0x800F_A460;
    pub const AIR_N_COLL: u32 = 0x800F_A4F0;
}

/// Kirby's Thunder Jolts: his states for each copy and his attributes' two blocks
/// (`specialn_pk_*` and `specialn_pc_*` in `ftKb_DatAttrs`, Pikachu's layout).
pub const KIRBY: ThunderJolt = ThunderJolt {
    kind: KindFrom::Copy,
    pikachu: (ms::PK_SPECIAL_N, ms::PK_SPECIAL_AIR_N),
    pichu: (ms::PC_SPECIAL_N, ms::PC_SPECIAL_AIR_N),
    attrs: (attr::PK_JOLT, attr::PC_JOLT),
};

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let t = &KIRBY;
    match a {
        addr::ENTER => g.pk_special_n_enter(gobj, t),
        addr::AIR_ENTER => g.pk_special_air_n_enter(gobj, t),
        addr::N_ANIM => g.pk_special_n_anim(gobj, t),
        addr::AIR_N_ANIM => g.pk_special_air_n_anim(gobj, t),
        addr::N_IASA | addr::AIR_N_IASA => g.pk_nothing(gobj),
        addr::N_PHYS => g.pk_ground_friction(gobj),
        addr::AIR_N_PHYS => g.pk_special_air_n_phys(gobj),
        addr::N_COLL => g.pk_special_n_coll(gobj, t),
        addr::AIR_N_COLL => g.pk_special_air_n_coll(gobj, t),
        _ => return false,
    }
    true
}
