//! Din's Fire's explosion, `melee/it/kinds/itzeldadinfireexplode.c`: a blast as big and as
//! strong as the fire had grown, for 60 frames, with a rumble and, for the bigger ones, a quake.
//! PK Flash's blast's code ([`super::steeredshot`]) with its own effect, damage and rumbles.

use gekko_fp as fp;

use super::super::item::article;
use super::super::vars::ItemVars;
use super::super::{coll, item, item_mut, jobj_of, lib26b1, lib2725};
use super::steeredshot::{self, Blast, BlastVars};
use crate::fpu::{mem, reg};
use crate::game::camera::request_quake;
use crate::game::ef::{self, Arg};
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::lb::hitcapsule;
use crate::vec::Vec3;

pub const DESTROYED: u32 = 0x802C_4670;
pub const ANIM: u32 = 0x802C_4820;
pub const PHYS: u32 = 0x802C_4980;
#[doc(alias = "itZeldadinfireexplode_UnkMotion0_Coll")]
pub const COLL: u32 = 0x802C_4998;
// `itZeldaDinFireExplode_Logic66_Clanked`: dead, every hitbox its scripts open has the clank bit clear (ledger/dead.txt).
pub const CLANKED: u32 = 0x802C_49A0;
#[doc(alias = "itZeldaDinFireExplode_Logic66_Absorbed")]
pub const ABSORBED: u32 = 0x802C_49A8;
#[doc(alias = "itZeldaDinFireExplode_Logic66_ShieldBounced")]
pub const SHIELD_BOUNCED: u32 = 0x802C_49B0;
#[doc(alias = "itZeldaDinFireExplode_Logic66_HitShield")]
pub const HIT_SHIELD: u32 = 0x802C_49B8;
#[doc(alias("EVT_UNK"))]
pub const FORGET: u32 = 0x802C_49C0;

/// `itZeldaDinFireExplodeAttributes`, those only the explosion reads ([`steeredshot`] reads
/// the fire's size and the scales): the damage's base and growth.
mod attr {
    pub const DAMAGE: u32 = 0xC;
    pub const DAMAGE_GROWTH: u32 = 0x10;
}

/// The explosion's effect.
const GFX: i32 = 0x4FA;

/// `QuakeKind_Small` and `QuakeKind_Medium`.
const QUAKE_SMALL: i32 = 2;
const QUAKE_MEDIUM: i32 = 3;

/// Din's Fire's explosion: 60 frames, its effect, ended at its end.
pub const DINS_FIRE_BLAST: Blast = Blast {
    name: "Din's Fire's explosion",
    vars: shared,
    life: 60.0,
    on_start: Some(effect),
    animate,
    rumble,
    end_effects: Some(end_effects),
};

/// The explosion's variables' shared fields.
fn shared(v: &mut ItemVars) -> BlastVars<'_> {
    let v = v.zelda_din_fire_explode();
    BlastVars {
        charge: &mut v.xDD4,
        hit_scale: &mut v.xDD8,
        owner: &mut v.xDDC,
    }
}

/// Runs one of the explosion's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => animate(r, gobj),
        COLL => false,
        CLANKED | ABSORBED | SHIELD_BOUNCED | HIT_SHIELD => true,
        _ => return None,
    })
}

/// Runs one of the explosion's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => steeredshot::blast_destroyed(r, gobj, &DINS_FIRE_BLAST),
        PHYS => steeredshot::blast_stopped(r, gobj),
        _ => return false,
    }
    true
}

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let attrs = article(item(r.game, gobj))
        .special(r.data)
        .expect("Din's Fire's explosion without attributes");
    r.data.f32(attrs.at(off))
}

/// Its start's own part: its effect, on.
fn effect(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    ef::spawn(r, GFX, Some(gobj), &[Arg::JObj(j)]);
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .zelda_din_fire_explode()
        .xDE0
        .set(1);
}

/// Its end's own part: its effects gone.
fn end_effects(r: &mut Running, gobj: GObjId) {
    crate::game::ef::eflib::destroy_all(r, gobj);
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .zelda_din_fire_explode()
        .xDE0
        .set(0);
}

/// The explosion's rumble by the fire's share of the full size, and a quake for the bigger
/// ones.
fn rumble(r: &mut Running, gobj: GObjId, t: f64, pos: Vec3) {
    if t < 0.4375 {
        lib2725::rumble_owner(r, gobj, 0xB, 0xA);
    } else if t < 0.625 {
        lib2725::rumble_owner(r, gobj, 0xC, 0xA);
    } else if t < 0.8125 {
        lib2725::rumble_owner(r, gobj, 0xD, 0xF);
        request_quake(r, QUAKE_SMALL, Some(pos));
    } else {
        lib2725::rumble_owner(r, gobj, 0xE, 0x14);
        request_quake(r, QUAKE_MEDIUM, Some(pos));
    }
}

/// The explosion's frame, `itZeldadinfireexplode_UnkMotion0_Anim`: scaled by the fire's size,
/// the hitbox's damage (rounded to single, unlike the flash's, and set every frame) and size
/// with it while it's on; whether its life ran out.
#[doc(alias = "itZeldadinfireexplode_UnkMotion0_Anim")]
pub fn animate(r: &mut Running, gobj: GObjId) -> bool {
    let (base, growth) = (
        reg(attr_f32(r, gobj, attr::DAMAGE)),
        reg(attr_f32(r, gobj, attr::DAMAGE_GROWTH)),
    );
    let s = steeredshot::blast_scale(r, gobj, &DINS_FIRE_BLAST);
    if item(r.game, gobj).x5D4_hitboxes[0].hit.state != hitcapsule::DISABLED {
        let size = reg(steeredshot::blast_vars(r, gobj, &DINS_FIRE_BLAST)
            .charge
            .get());
        let dmg = mem(fp::fmadds(size, growth, base));
        coll::set_hit_damage(
            r.game,
            r.data,
            gobj,
            0,
            crate::ppc::cvt_fp2unsigned(reg(dmg)),
        );
        steeredshot::blast_hit_scale(r, gobj, &DINS_FIRE_BLAST, s);
    }
    lib2725::tick_lifetime(r.game, gobj)
}

/// Forgets a GObj the explosion points to, `itZeldaDinFireExplode_Logic66_EvtUnk`.
#[doc(alias = "itZeldaDinFireExplode_Logic66_EvtUnk")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
