//! Master Hand's finger gun's bullets, `melee/it/kinds/itmasterhandbullet.c`: shots that fly
//! straight (state 0, or state 1 when he's hurt enough to shoot faster), sparking each frame,
//! until their lifetime runs out or they hit.

use gekko_fp as fp;

use super::super::item::{
    article, change, change_state, debug_owner_bubbles, next_attack_id, spawn_air,
};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::ef::Arg;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::quat::Quat;
use crate::vec::Vec3;

/// `It_Kind_MasterHand_Bullet`.
pub const KIND: i32 = 126;

#[doc(alias = "itMasterhandbullet_UnkMotion1_Anim")]
pub const ANIM: u32 = 0x802F_0E70;
#[doc(alias = "itMasterhandbullet_UnkMotion1_Phys")]
pub const PHYS: u32 = 0x802F_0ED4;
#[doc(alias = "itMasterhandbullet_UnkMotion1_Coll")]
pub const COLL: u32 = 0x802F_0ED8;
#[doc(alias = "it_802F0F04")]
pub const ACCESSORY: u32 = 0x802F_0F04;
#[doc(alias = "it_802F0F08")]
pub const LANDED: u32 = 0x802F_0F08;
#[doc(alias = "itMasterHandBullet_Logic83_DmgDealt")]
pub const DMG_DEALT: u32 = 0x802F_0F0C;
#[doc(alias = "itMasterHandBullet_Logic83_Reflected")]
pub const REFLECTED: u32 = 0x802F_0F14;
#[doc(alias = "itMasterHandBullet_Logic83_Clanked")]
pub const CLANKED: u32 = 0x802F_0F34;
#[doc(alias = "itMasterHandBullet_Logic83_Absorbed")]
pub const ABSORBED: u32 = 0x802F_0F3C;
#[doc(alias = "itMasterHandBullet_Logic83_ShieldBounced")]
pub const SHIELD_BOUNCED: u32 = 0x802F_0F44;
#[doc(alias = "itMasterHandBullet_Logic83_HitShield")]
pub const HIT_SHIELD: u32 = 0x802F_0F64;

/// The spark each frame.
const GFX_SPARK: i32 = 0x405;

/// A bullet from Master Hand (`owner`) at `pos`, `it_802F0AE0`: `speed_x` his way and
/// `speed_y` up, in state 1 when `fast`.
#[doc(alias = "it_802F0AE0")]
#[allow(clippy::too_many_arguments)]
pub fn spawn(
    r: &mut Running,
    owner: GObjId,
    prev_pos: Vec3,
    pos: Vec3,
    kind: i32,
    fast: bool,
    facing_dir: f32,
    speed_x: f32,
    speed_y: f32,
) {
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    spawn.prev_pos = prev_pos;
    spawn.pos = pos;
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(mem(fp::fmuls(reg(speed_x), reg(facing_dir))), speed_y, 0.0);
    spawn.x0_parent_gobj = Some(owner);
    spawn.x4_parent_gobj2 = Some(owner);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = next_attack_id(r.game) as i32;
    let g = spawn_air(r, &mut spawn).expect("a bullet's spawn reads a null item");
    start(r, g, i32::from(fast));
    debug_owner_bubbles(r.game, g, Some(owner));
    lib2725::first_frame(r, g);
}

/// Forgets a GObj the bullet points to, `itMasterHandBullet_Logic85_EvtUnk` (`it_8026B894`).
// Ports `itMasterHandBullet_Logic85_EvtUnk`; aliased once a check reaches it.
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// Flying, `it_802F0BE8` (state 0) and `it_802F0D2C` (state 1): its accessory, its lifetime
/// (`x0`), its hits uncounted, turned a quarter turn its way.
#[doc(alias = "it_802F0BE8")]
#[doc(alias = "it_802F0D2C")]
pub fn start(r: &mut Running, gobj: GObjId, state: i32) {
    let life = {
        let a = article(item(r.game, gobj))
            .special(r.data)
            .expect("a bullet without attributes");
        r.data.f32(a)
    };
    change_state(r, gobj, state, change::ANIM_UPDATE);
    let ip = item_mut(r.game, gobj);
    ip.on_accessory = Some(crate::game::handles::CodeRef(ACCESSORY));
    ip.xDD4_itemVar.masterhand_bullet().x0.set(life);
    lib2725::uncount_hits(r.game, gobj);
    let facing = item(r.game, gobj).facing_dir;
    let y = mem(fp::frsp(fp::fmul(
        core::f64::consts::FRAC_PI_2,
        reg(facing),
    )));
    let j = jobj_of(r.game, gobj).expect("a bullet without its model");
    let q = Quat {
        x: 0.0,
        y,
        z: 0.0,
        w: 0.0,
    };
    crate::hsd::jobj::set_rotation(&mut r.game.objs.jobjs, j, q);
}

/// `itMasterhandbullet_UnkMotion1_Anim`: a spark; gone once its lifetime runs out.
fn anim(r: &mut Running, gobj: GObjId) -> bool {
    let pos = item(r.game, gobj).pos;
    crate::game::ef::spawn(r, GFX_SPARK, Some(gobj), &[Arg::Vec(pos)]);
    let v = item_mut(r.game, gobj).xDD4_itemVar.masterhand_bullet();
    let t = mem(fp::fsubs(reg(v.x0.get()), 1.0));
    v.x0.set(t);
    reg(t) < 0.0
}

pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => anim(r, gobj),
        COLL => {
            super::super::ground::fall(r, gobj, |_, _| {});
            false
        }
        DMG_DEALT | CLANKED | ABSORBED | HIT_SHIELD => true,
        REFLECTED => super::super::pokemon::bounce_off_wall(r, gobj),
        SHIELD_BOUNCED => super::super::pokemon::bounce_off_surface(r, gobj),
        _ => return None,
    })
}

pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    let _ = r;
    let _ = gobj;
    matches!(addr, PHYS | ACCESSORY | LANDED)
}
