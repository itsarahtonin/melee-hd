//! PK Flash's blast, `melee/it/kinds/itnesspkflashexplode.c`: as big and as strong as the flash
//! had grown, for its animation's length, with a rumble and, for the bigger ones, a quake. Din's
//! Fire's blast's code ([`super::steeredshot`]) with its own damage, end and rumbles.

use crate::lb::hitcapsule::HitCapsuleState;
use gekko_fp as fp;

use super::super::item::article;
use super::super::vars::ItemVars;
use super::super::{coll, item, item_mut, lib26b1, lib2725};
use super::steeredshot::{self, Blast, BlastVars};
use crate::fpu::reg;
use crate::game::camera::request_quake;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const SPAWN: u32 = 0x802A_F940;
pub const DESTROYED: u32 = 0x802A_FA2C;
pub const START: u32 = 0x802A_FA70;
pub const ANIM: u32 = 0x802A_FBA4;
#[doc(alias = "itNessPKFlashExplode_UnkMotion0_Phys")]
pub const PHYS: u32 = 0x802A_FD2C;
#[doc(alias = "itNessPKFlashExplode_UnkMotion0_Coll")]
pub const COLL: u32 = 0x802A_FD44;
// `itNessPKFlashExplode_Logic103_Clanked`: dead, every hitbox its scripts open has the clank bit clear (ledger/dead.txt).
pub const CLANKED: u32 = 0x802A_FD4C;
#[doc(alias = "itNessPKFlashExplode_Logic103_Absorbed")]
pub const ABSORBED: u32 = 0x802A_FD54;
#[doc(alias = "itNessPKFlashExplode_Logic103_ShieldBounced")]
pub const SHIELD_BOUNCED: u32 = 0x802A_FD5C;
#[doc(alias = "itNessPKFlashExplode_Logic103_HitShield")]
pub const HIT_SHIELD: u32 = 0x802A_FD64;
#[doc(alias("EVT_UNK"))]
pub const FORGET: u32 = 0x802A_FD6C;

/// `itFlashExplAttributes`, those only the blast reads ([`steeredshot`] reads the flash's size
/// and the scales): the damage's base and growth.
mod attr {
    pub const BASE_DAMAGE: u32 = 0xC;
    pub const DAMAGE_MUL: u32 = 0x10;
}

/// `QuakeKind_Small`, `QuakeKind_Medium`.
const QUAKE_SMALL: i32 = 2;
const QUAKE_MEDIUM: i32 = 3;

/// PK Flash's blast (and Kirby's): 1,024 frames, which its animation's end cuts short.
pub const PK_FLASH_BLAST: Blast = Blast {
    name: "PK Flash's blast",
    vars: shared,
    life: 1024.0,
    on_start: None,
    animate,
    rumble,
    end_effects: None,
};

/// The blast's variables' shared fields.
fn shared(v: &mut ItemVars) -> BlastVars<'_> {
    let v = v.pk_flush_explode();
    BlastVars {
        charge: &mut v.charge,
        hit_scale: &mut v.first_scale,
        owner: &mut v.unread_owner,
    }
}

/// Runs one of the blast's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => animate(r, gobj),
        COLL => false,
        CLANKED | ABSORBED | SHIELD_BOUNCED | HIT_SHIELD => true,
        _ => return None,
    })
}

/// Runs one of the blast's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => steeredshot::blast_destroyed(r, gobj, &PK_FLASH_BLAST),
        PHYS => steeredshot::blast_stopped(r, gobj),
        _ => return false,
    }
    true
}

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let attrs = article(item(r.game, gobj))
        .special(r.data)
        .expect("PK Flash's blast without attributes");
    r.data.f32(attrs.at(off))
}

/// The blast's rumble by the charge's share of the full size, and a quake for the bigger ones.
fn rumble(r: &mut Running, gobj: GObjId, ratio: f64, pos: Vec3) {
    if ratio > f64::from(0.849_999_96_f32) {
        request_quake(r, QUAKE_MEDIUM, Some(pos));
        lib2725::rumble_owner(r, gobj, 0xE, 0x14);
    } else if ratio > f64::from(0.65_f32) {
        request_quake(r, QUAKE_SMALL, Some(pos));
        lib2725::rumble_owner(r, gobj, 0xD, 0xF);
    } else if ratio > f64::from(0.45_f32) {
        lib2725::rumble_owner(r, gobj, 0xC, 0xA);
    } else {
        lib2725::rumble_owner(r, gobj, 0xB, 5);
    }
}

/// `itNessPKFlashExplode_UnkMotion0_Anim`: scaled by the charge, its damage set once (unrounded,
/// unlike the fire's), its hitbox grown with it; it goes when its animation ends.
#[doc(alias = "itNessPKFlashExplode_UnkMotion0_Anim")]
pub fn animate(r: &mut Running, gobj: GObjId) -> bool {
    let (base, mul) = (
        reg(attr_f32(r, gobj, attr::BASE_DAMAGE)),
        reg(attr_f32(r, gobj, attr::DAMAGE_MUL)),
    );
    let s = steeredshot::blast_scale(r, gobj, &PK_FLASH_BLAST);
    let ip = item(r.game, gobj);
    if ip.cmd_var2 == 0 && ip.hitboxes[0].hit.state != HitCapsuleState::Disabled {
        let charge = reg(steeredshot::blast_vars(r, gobj, &PK_FLASH_BLAST)
            .charge
            .get());
        let dmg = fp::fmadds(charge, mul, base);
        coll::set_hit_damage(r.game, r.data, gobj, 0, crate::ppc::cvt_fp2unsigned(dmg));
        item_mut(r.game, gobj).cmd_var2 = 1;
    }
    steeredshot::blast_hit_scale(r, gobj, &PK_FLASH_BLAST, s);
    !lib2725::is_animating(r.game, gobj)
}

/// Forgets a GObj the blast points to, `itNessPKFlashExplode_Logic103_EvtUnk`.
// Ports `itNessPKFlashExplode_Logic103_EvtUnk`; aliased once a check reaches it.
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
