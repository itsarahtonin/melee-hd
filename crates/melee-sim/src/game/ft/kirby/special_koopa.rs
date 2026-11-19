//! Bowser's and Giga Bowser's copies (`ftkirbyspecialkoopa.c`): the fire breath as Kirby
//! breathes it. Bowser's own code ([`ft::koopa::special_n`](super::super::koopa::special_n))
//! over his states (Giga Bowser's copy's 95 on), Bowser's attributes 0x114 bytes on in his,
//! the flame's speed and size in his `u` (`x84`, `x88`), a point on his mouth, the copy's own
//! flame kind and first angle group, and three branches of his own ([`KIRBY`]); the breath's
//! variables are laid out as Bowser's (`mv.kb.specialn_kp`). Only his copy's addresses are
//! here.

use super::super::super::code::Val;
use super::super::super::it::kinds::KIRBY_KOOPA_FLAME;
use super::super::super::state::Running;
use super::super::koopa::ms as kp;
use super::super::koopa::special_n::{FireBreath, Phase};
use super::copy::KindFrom;
use super::ms;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// The copy's function addresses.
pub(super) mod addr {
    pub const FLAME: u32 = 0x800F_A588;
    pub const REFUEL: u32 = 0x800F_A7D4;
    pub const MAX_SPEED: u32 = 0x800F_A83C;
    pub const MAX_SIZE: u32 = 0x800F_A878;
    pub const ENTER: u32 = 0x800F_A8B4;
    pub const AIR_ENTER: u32 = 0x800F_A958;
    pub const N_START_ANIM: u32 = 0x800F_A9FC;
    pub const N_ANIM: u32 = 0x800F_AA74;
    pub const N_END_ANIM: u32 = 0x800F_AB18;
    pub const AIR_N_START_ANIM: u32 = 0x800F_AB54;
    pub const AIR_N_ANIM: u32 = 0x800F_ABCC;
    pub const AIR_N_END_ANIM: u32 = 0x800F_AC70;
    pub const N_START_IASA: u32 = 0x800F_ACAC;
    pub const N_IASA: u32 = 0x800F_ACB0;
    pub const N_END_IASA: u32 = 0x800F_AE0C;
    pub const AIR_N_START_IASA: u32 = 0x800F_AE10;
    pub const AIR_N_IASA: u32 = 0x800F_AE14;
    pub const AIR_N_END_IASA: u32 = 0x800F_AF70;
    pub const N_START_PHYS: u32 = 0x800F_AF74;
    pub const N_PHYS: u32 = 0x800F_AF94;
    pub const N_END_PHYS: u32 = 0x800F_AFB4;
    pub const AIR_N_START_PHYS: u32 = 0x800F_AFD4;
    pub const AIR_N_PHYS: u32 = 0x800F_AFF4;
    pub const AIR_N_END_PHYS: u32 = 0x800F_B014;
    pub const N_START_COLL: u32 = 0x800F_B034;
    pub const N_COLL: u32 = 0x800F_B0C4;
    pub const N_END_COLL: u32 = 0x800F_B154;
    pub const AIR_N_START_COLL: u32 = 0x800F_B1E4;
    pub const AIR_N_COLL: u32 = 0x800F_B274;
    pub const AIR_N_END_COLL: u32 = 0x800F_B304;
}

/// Kirby's breath. Its states by his copy (Giga Bowser's 95 on from Bowser's), the flames
/// from a point on his mouth (`fp->parts[12]`), starting from angle group -1, always with
/// Bowser's sounds, and the aerial start landing in the grounded loop (in Melee's bugs).
pub const KIRBY: FireBreath = FireBreath {
    states: ms::KP_SPECIAL_N_START - kp::SPECIAL_N_START,
    kind: KindFrom::Copy,
    giga_states: ms::GK_SPECIAL_N_START - ms::KP_SPECIAL_N_START,
    attrs: 0x114,
    u: 0x84,
    mouth_bone: 12,
    mouth: Some(Vec3 {
        x: 0.0,
        y: 0.5,
        z: 3.0,
    }),
    flame_kind: KIRBY_KOOPA_FLAME,
    first_dir: -1,
    sfx_by_kind: false,
    air_start_lands_in_loop: true,
};

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use addr::*;
    let p = &KIRBY;
    match a {
        FLAME => g.kp_flame(gobj, p),
        REFUEL => g.kp_refuel(gobj, p),
        ENTER => g.kp_special_n_enter(gobj, p),
        AIR_ENTER => g.kp_special_air_n_enter(gobj, p),
        N_START_ANIM => g.kp_special_n_start_anim(gobj, p, false),
        AIR_N_START_ANIM => g.kp_special_n_start_anim(gobj, p, true),
        N_ANIM | AIR_N_ANIM => g.kp_special_n_anim(gobj, p),
        N_END_ANIM => g.kp_special_n_end_anim(gobj),
        AIR_N_END_ANIM => g.kp_special_air_n_end_anim(gobj),
        N_START_IASA | N_END_IASA | AIR_N_START_IASA | AIR_N_END_IASA => {
            g.kp_special_n_no_iasa(gobj)
        }
        N_IASA => g.kp_special_n_iasa(gobj, p, false),
        AIR_N_IASA => g.kp_special_n_iasa(gobj, p, true),
        N_START_PHYS | N_PHYS | N_END_PHYS => g.kp_special_n_phys(gobj),
        AIR_N_START_PHYS | AIR_N_PHYS | AIR_N_END_PHYS => g.kp_special_air_n_phys(gobj),
        N_START_COLL => g.kp_special_n_coll(gobj, p, Phase::Start),
        N_COLL => g.kp_special_n_coll(gobj, p, Phase::Loop),
        N_END_COLL => g.kp_special_n_coll(gobj, p, Phase::End),
        AIR_N_START_COLL => g.kp_special_air_n_coll(gobj, p, Phase::Start),
        AIR_N_COLL => g.kp_special_air_n_coll(gobj, p, Phase::Loop),
        AIR_N_END_COLL => g.kp_special_air_n_coll(gobj, p, Phase::End),
        _ => return false,
    }
    true
}

#[melee_callbacks::callbacks(Call)]
pub(super) fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    Some(match f {
        addr::MAX_SPEED => Val::Int(r.kp_flame_max_speed(args[0].gobj(), &KIRBY)),
        addr::MAX_SIZE => Val::Int(r.kp_flame_max_size(args[0].gobj(), &KIRBY)),
        _ => return None,
    })
}
