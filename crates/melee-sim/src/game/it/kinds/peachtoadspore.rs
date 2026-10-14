//! Toad's spores, `melee/it/kinds/itpeachtoadspore.c`: released when an attack meets Toad, each
//! sent up at a random speed and angle in its spread, slowing as it goes, gone when it hits or
//! its life runs out.

use gekko_fp as fp;

use super::super::item::{article, change, change_state, debug_owner_bubbles, spawn_air};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::ef::{self, Arg};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::math;
use crate::vec::Vec3;

pub const ANIM: u32 = 0x802B_E408;
pub const PHYS: u32 = 0x802B_E458;
pub const DMG_DEALT: u32 = 0x802B_E488;
pub const CLANKED: u32 = 0x802B_E4AC;
pub const HIT_SHIELD: u32 = 0x802B_E4D0;
pub const ABSORBED: u32 = 0x802B_E4F4;
pub const SHIELD_BOUNCED: u32 = 0x802B_E518;
pub const REFLECTED: u32 = 0x802B_E538;
pub const EVT_UNK: u32 = 0x802B_E578;

/// `It_Kind_Peach_ToadSpore`.
pub const KIND: i32 = 111;

/// The spores' effect.
const GFX_SPORE: i32 = 0x4D3;

/// `itPeachToadSporeAttributes` offsets: the least speed, the most added to it, the slowing
/// each frame, and the angle's spread.
mod attr {
    pub const MIN_SPEED: u32 = 0x0;
    pub const MAX_SPEED_OFFSET: u32 = 0x4;
    pub const SPEED_DECAY: u32 = 0x8;
    pub const ANGLE: u32 = 0xC;
}

/// Runs one of the spore's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => tick(r, gobj),
        DMG_DEALT | CLANKED | HIT_SHIELD | ABSORBED => gone(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        REFLECTED => reflected(r, gobj),
        _ => return None,
    })
}

/// Runs one of the spore's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PHYS => slow(r, gobj),
        _ => return false,
    }
    true
}

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let attrs = article(item(r.game, gobj))
        .special(r.data)
        .expect("a spore without attributes");
    r.data.f32(attrs.at(off))
}

/// Releases a spore of `kind` from `parent` at `pos`, `it_802BE214`.
#[doc(alias = "it_802BE214")]
pub fn spawn(
    r: &mut Running,
    parent: GObjId,
    pos: Vec3,
    kind: i32,
    facing_dir: f32,
) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    // `Item_InitSpawnPosition`, keeping its depth.
    spawn.prev_pos = pos;
    spawn.pos = spawn.prev_pos;
    // `Item_InitSpawnCommonFields`, with a first collision.
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(parent);
    spawn.x4_parent_gobj2 = Some(parent);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    launch(r, gobj, parent);
    debug_owner_bubbles(r.game, gobj, Some(parent));
    Some(gobj)
}

/// Sends the spore off for `owner`, `it_802BE2E8`: a life of 60 frames, a random speed, and a
/// random angle in its spread around straight up (mirrored facing left); hidden, not to be
/// picked up, with its effect.
#[doc(alias = "it_802BE2E8")]
pub fn launch(r: &mut Running, gobj: GObjId, owner: GObjId) {
    let j = jobj_of(r.game, gobj);
    item_mut(r.game, gobj).owner = Some(owner);
    lib2725::set_lifetime(r.game, r.data, gobj, 60.0);
    item_mut(r.game, gobj).xDAC_itcmd_var0 = 0;
    let (min, offset, spread) = (
        reg(attr_f32(r, gobj, attr::MIN_SPEED)),
        reg(attr_f32(r, gobj, attr::MAX_SPEED_OFFSET)),
        reg(attr_f32(r, gobj, attr::ANGLE)),
    );
    let rand = reg(r.game.objs.rng.draw_f32());
    let speed = fp::fmadds(offset, rand, min);
    let rand = reg(r.game.objs.rng.draw_f32());
    let double_angle = fp::fsubs(fp::frsp(std::f64::consts::PI), spread);
    let angle_offset = fp::fmuls(spread, rand);
    let angle = fp::fmadds(0.5, double_angle, angle_offset);
    let sin = reg(math::sin(mem(angle)));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(ip.facing_dir), fp::fmuls(speed, sin)));
    let cos = reg(math::cos(mem(angle)));
    item_mut(r.game, gobj).x40_vel.y = mem(fp::fmuls(speed, cos));
    lib2725::hide_tree(r.game, j);
    lib26b1::disable_pickup(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    ef::spawn(r, GFX_SPORE, Some(gobj), &[Arg::JObj(j)]);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// `itPeachtoadspore_UnkMotion0_Anim`: gone when its script says, or once its life runs out.
#[doc(alias = "itPeachtoadspore_UnkMotion0_Anim")]
fn tick(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    if ip.xDAC_itcmd_var0 != 0 {
        return true;
    }
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    if reg(ip.xD44_lifeTimer) <= 0.0 {
        ip.xD44_lifeTimer = 0.0;
        return true;
    }
    false
}

/// `itPeachtoadspore_UnkMotion0_Phys`: slowed by its attributes' rate.
#[doc(alias = "itPeachtoadspore_UnkMotion0_Phys")]
fn slow(r: &mut Running, gobj: GObjId) {
    let decay = reg(attr_f32(r, gobj, attr::SPEED_DECAY));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(ip.x40_vel.x), decay));
    ip.x40_vel.y = mem(fp::fmuls(reg(ip.x40_vel.y), decay));
}

/// The spore's hit, clank, shield hit and absorption (`itPeachToadSpore_Logic92_DmgDealt`,
/// `itPeachToadSpore_Logic68_Clanked`, `_HitShield`, `_Absorbed`): its effects gone, and it
/// goes.
#[doc(alias = "itPeachToadSpore_Logic92_DmgDealt")]
#[doc(alias = "itPeachToadSpore_Logic68_Clanked")]
#[doc(alias = "itPeachToadSpore_Logic68_HitShield")]
#[doc(alias = "itPeachToadSpore_Logic68_Absorbed")]
fn gone(r: &mut Running, gobj: GObjId) -> bool {
    ef::destroy_all(r, gobj);
    true
}

/// `itPeachToadSpore_Logic68_ShieldBounced`: off the shield.
#[doc(alias = "itPeachToadSpore_Logic68_ShieldBounced")]
fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_shield(r.game, gobj)
}

/// `itPeachToadSpore_Logic68_Reflected`: sent back, keeping its life.
#[doc(alias = "itPeachToadSpore_Logic68_Reflected")]
fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    let life = item(r.game, gobj).xD44_lifeTimer;
    lib2725::reflected_back(r.game, gobj);
    item_mut(r.game, gobj).xD44_lifeTimer = life;
    false
}

/// `itPeachToadSpore_Logic92_EvtUnk`: forgets `other`.
#[doc(alias = "itPeachToadSpore_Logic92_EvtUnk")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
