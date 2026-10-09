//! Din's Fire's explosion, `melee/it/kinds/itzeldadinfireexplode.c`: a blast as big and as
//! strong as the fire had grown, for 60 frames, with a rumble and, for the bigger ones, a quake.

use gekko_fp as fp;

use super::super::item::{article, change, change_state, debug_owner_bubbles, spawn_air};
use super::super::{coll, item, item_mut, jobj_of, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::camera::request_quake;
use crate::game::ef::{self, Arg};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::lb::hitcapsule;
use crate::vec::Vec3;

pub const DESTROYED: u32 = 0x802C_4670;
pub const ANIM: u32 = 0x802C_4820;
pub const PHYS: u32 = 0x802C_4980;
pub const COLL: u32 = 0x802C_4998;
pub const CLANKED: u32 = 0x802C_49A0;
pub const ABSORBED: u32 = 0x802C_49A8;
pub const SHIELD_BOUNCED: u32 = 0x802C_49B0;
pub const HIT_SHIELD: u32 = 0x802C_49B8;
pub const EVT_UNK: u32 = 0x802C_49C0;

/// `itZeldaDinFireExplodeAttributes`: the fire's size it scales by, the smallest and largest
/// scale, and the damage's base and growth.
mod attr {
    pub const SIZE: u32 = 0x0;
    pub const SCALE_MIN: u32 = 0x4;
    pub const SCALE_MAX: u32 = 0x8;
    pub const DAMAGE: u32 = 0xC;
    pub const DAMAGE_GROWTH: u32 = 0x10;
}

/// The explosion's effect.
const GFX: i32 = 0x4FA;

/// `QuakeKind_Small` and `QuakeKind_Medium`.
const QUAKE_SMALL: i32 = 2;
const QUAKE_MEDIUM: i32 = 3;

/// Runs one of the explosion's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => animate(r, gobj),
        COLL => false,
        CLANKED | ABSORBED | SHIELD_BOUNCED | HIT_SHIELD => true,
        _ => return None,
    })
}

/// Runs one of the explosion's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => destroyed(r, gobj),
        PHYS => physics(r, gobj),
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

/// Makes the explosion at `pos` of a fire grown to `size`, `it_802C4580`.
#[doc(alias = "it_802C4580")]
pub fn spawn(
    r: &mut Running,
    parent: GObjId,
    fire: GObjId,
    pos: Vec3,
    facing_dir: f32,
    size: f32,
) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind: super::ZELDA_DIN_FIRE_EXPLODE,
        ..Default::default()
    };
    // `Item_InitSpawnPosition`, on the plane.
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = spawn.prev_pos;
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(parent);
    spawn.x4_parent_gobj2 = Some(fire);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    let ip = item_mut(r.game, gobj);
    // `Item_ClearCmdVars`.
    ip.xDB8_itcmd_var3 = 0;
    ip.xDB4_itcmd_var2 = 0;
    ip.xDB0_itcmd_var1 = 0;
    ip.xDAC_itcmd_var0 = 0;
    let v = ip.xDD4_itemVar.zelda_din_fire_explode();
    v.xDD4.set(size);
    v.xDDC.set(Some(parent));
    start(r, gobj);
    Some(gobj)
}

/// The explosion's end, `itZeldaDinFireExplode_Logic66_Destroyed`.
#[doc(alias = "itZeldaDinFireExplode_Logic66_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    lib2725::clear_hitboxes(r.game, r.data, gobj);
    item_mut(r.game, gobj).owner = None;
    crate::game::ef::eflib::destroy_all(r, gobj);
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .zelda_din_fire_explode()
        .xDE0
        .set(0);
}

/// The explosion's start, `it_802C46C4`: 60 frames, its effect, its first frame, and by the
/// fire's size a rumble and a quake.
#[doc(alias = "it_802C46C4")]
pub fn start(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    // `Item_ClearFlagsAndEnterState(gobj, ip, 0)`.
    lib26b1::disable_pickup(r.game, gobj);
    item_mut(r.game, gobj).xDC8_word.flags.x13 = false;
    lib2725::show(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    lib2725::set_lifetime(r.game, r.data, gobj, 60.0);
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .zelda_din_fire_explode()
        .xDD8
        .set(0.0);
    ef::spawn(r, GFX, Some(gobj), &[Arg::JObj(j)]);
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .zelda_din_fire_explode()
        .xDE0
        .set(1);
    // `Item_802694CC`.
    super::super::item::animate(r, gobj);
    animate(r, gobj);
    let full = reg(attr_f32(r, gobj, attr::SIZE));
    let size = reg(item_mut(r.game, gobj)
        .xDD4_itemVar
        .zelda_din_fire_explode()
        .xDD4
        .get());
    let t = fp::fdivs(size, full);
    let pos = item(r.game, gobj).pos;
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
    let owner = item(r.game, gobj).owner;
    debug_owner_bubbles(r.game, gobj, owner);
}

/// The explosion's frame, `itZeldadinfireexplode_UnkMotion0_Anim`: scaled by the fire's size,
/// the hitbox's damage and size with it; whether its life ran out.
#[doc(alias = "itZeldadinfireexplode_UnkMotion0_Anim")]
pub fn animate(r: &mut Running, gobj: GObjId) -> bool {
    let (full, min, max) = (
        reg(attr_f32(r, gobj, attr::SIZE)),
        reg(attr_f32(r, gobj, attr::SCALE_MIN)),
        reg(attr_f32(r, gobj, attr::SCALE_MAX)),
    );
    let (base, growth) = (
        reg(attr_f32(r, gobj, attr::DAMAGE)),
        reg(attr_f32(r, gobj, attr::DAMAGE_GROWTH)),
    );
    let size = reg(item_mut(r.game, gobj)
        .xDD4_itemVar
        .zelda_din_fire_explode()
        .xDD4
        .get());
    let t = fp::fdivs(fp::fsubs(max, min), full);
    let s = mem(fp::fmadds(size, t, min));
    let j = jobj_of(r.game, gobj).expect("Din's Fire's explosion without a model");
    jobj::set_scale(&mut r.game.objs.jobjs, j, Vec3::new(s, s, s));
    if item(r.game, gobj).x5D4_hitboxes[0].hit.state != hitcapsule::DISABLED {
        let dmg = mem(fp::fmadds(size, growth, base));
        coll::set_hit_damage(
            r.game,
            r.data,
            gobj,
            0,
            crate::ppc::cvt_fp2unsigned(reg(dmg)),
        );
        let ip = item_mut(r.game, gobj);
        let first = ip.x5D4_hitboxes[0].hit.scale;
        let v = ip.xDD4_itemVar.zelda_din_fire_explode();
        if v.xDD8.get() == 0.0 {
            v.xDD8.set(first);
        }
        let k = v.xDD8.get();
        ip.x5D4_hitboxes[0].hit.scale = mem(fp::fmuls(reg(k), reg(s)));
    }
    lib2725::tick_lifetime(r.game, gobj)
}

/// `itZeldadinfireexplode_UnkMotion0_Phys`: the vertical and depth speeds stopped (the
/// original clears z twice and leaves x).
#[doc(alias = "itZeldadinfireexplode_UnkMotion0_Phys")]
pub fn physics(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.z = 0.0;
    ip.x40_vel.y = 0.0;
}

/// Forgets a GObj the explosion points to, `itZeldaDinFireExplode_Logic66_EvtUnk`.
#[doc(alias = "itZeldaDinFireExplode_Logic66_EvtUnk")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
