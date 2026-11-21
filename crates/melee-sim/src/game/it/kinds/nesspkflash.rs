//! PK Flash, `melee/it/kinds/itnesspkflash.c`: a flash that floats up from Ness's hand and
//! grows while he holds the special (he steers it with the stick), then waits a moment and
//! bursts ([`super::nesspkflashexplode`]); meeting the stage it fizzles (state 2). Din's Fire's
//! code ([`super::steeredshot`]) with its own rise, steering, wait and fizzle.
//!
//! Kirby's copy (`It_Kind_Kirby_NessPKFlush`) runs the same code with his own checks
//! (`ft::kirby::special_ness`).

use gekko_fp as fp;

use super::super::item::{article, change, change_state, play_sfx, stop_sfx2};
use super::super::vars::ItemVars;
use super::super::{item, item_mut, lib26b1, lib2725};
use super::nesspkflashexplode::PK_FLASH_BLAST;
use super::steeredshot::{self, SteeredShot, Vars};
use crate::fpu::{mem, reg};
use crate::game::code::unported_call;
use crate::game::ft::kirby::special_ness::KIRBY as NS_KIRBY;
use crate::game::ft::ness::special_n::NESS;
use crate::game::it::ItKind;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::math;
use crate::vec::Vec3;

pub const OWNER: u32 = 0x802A_A7E4;
pub const RELEASED: u32 = 0x802A_A7F0;
pub const HIT_STAGE: u32 = 0x802A_A810;
pub const SPAWN: u32 = 0x802A_A8C0;
pub const DESTROYED: u32 = 0x802A_A9CC;
pub const LET_GO: u32 = 0x802A_AA50;
pub const START: u32 = 0x802A_AA80;
pub const ANIM_0: u32 = 0x802A_AB70;
pub const ANIM_1: u32 = 0x802A_AD48;
pub const ANIM_2: u32 = 0x802A_AEE4;
pub const PHYS_0: u32 = 0x802A_AFDC;
#[doc(alias = "itNesspkflash_UnkMotion1_Phys")]
pub const PHYS_1: u32 = 0x802A_B128;
#[doc(alias = "itNesspkflash_UnkMotion2_Phys")]
pub const PHYS_2: u32 = 0x802A_B140;
pub const COLL_0: u32 = 0x802A_B158;
#[doc(alias = "itNesspkflash_UnkMotion1_Coll")]
pub const COLL_1: u32 = 0x802A_B29C;
#[doc(alias = "itNesspkflash_UnkMotion2_Coll")]
pub const COLL_2: u32 = 0x802A_B2A4;
pub const REFLECTED: u32 = 0x802A_B2AC;
// `itNesspkflash_Logic102_Clanked`: dead, no script of the kind opens a hitbox (ledger/dead.txt).
pub const CLANKED: u32 = 0x802A_B3C0;
// `itNesspkflash_Logic102_Absorbed`: dead, no script of the kind opens a hitbox (ledger/dead.txt).
pub const ABSORBED: u32 = 0x802A_B3C8;
#[doc(alias("EVT_UNK"))]
pub const FORGET: u32 = 0x802A_B3D0;

/// `itFlashAttributes`, those only the flash reads ([`steeredshot`] reads the lifetime, the
/// full size and the scales).
mod attr {
    pub const UNK1: u32 = 0x10;
    pub const PEAK_RISE_HEIGHT: u32 = 0x14;
    pub const CONTROL: u32 = 0x18;
    pub const GRAVITY: u32 = 0x1C;
    pub const UNK2: u32 = 0x20;
    pub const UNK3: u32 = 0x24;
    pub const EXPLOSION_DELAY: u32 = 0x28;
}

/// The fizzle's sound.
const SFX_FIZZLE: i32 = 0x86;

/// PK Flash (and Kirby's): its rise, its owner charging it, his special told at its end.
pub const PK_FLASH: SteeredShot = SteeredShot {
    name: "PK Flash",
    vars: shared,
    launch: rise,
    let_go_of: not_charged,
    owner_done,
    end_effects: None,
};

/// The flash's variables' shared fields.
fn shared(v: &mut ItemVars) -> Vars<'_> {
    let v = v.pk_flush();
    Vars {
        grown: &mut v.xDD8_PKFlash,
        reflected: &mut v.xDDC_PKFlash,
        owner: &mut v.xDE0_PKFlash_Owner,
        xde4: &mut v.xDE4_PKFlash,
    }
}

/// Runs one of the flash's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM_0 => animate_0(r, gobj),
        ANIM_1 => animate_1(r, gobj),
        ANIM_2 => animate_2(r, gobj),
        COLL_0 => collide_0(r, gobj),
        COLL_1 | COLL_2 => false,
        REFLECTED => steeredshot::reflected(r, gobj, &PK_FLASH),
        CLANKED | ABSORBED => true,
        _ => return None,
    })
}

/// Runs one of the flash's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => steeredshot::destroyed(r, gobj, &PK_FLASH),
        PHYS_0 => physics_0(r, gobj),
        PHYS_1 | PHYS_2 => steeredshot::stopped(r, gobj),
        _ => return false,
    }
    true
}

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let attrs = article(item(r.game, gobj))
        .special(r.data)
        .expect("PK Flash without attributes");
    r.data.f32(attrs.at(off))
}

/// The flash's owner ([`steeredshot::owner`]).
pub fn owner(r: &Running, gobj: GObjId) -> Option<GObjId> {
    steeredshot::owner(r, gobj)
}

/// Whether the flash fizzled, `it_802AA7F0`: state 2.
#[doc(alias = "it_802AA7F0")]
pub fn released(r: &Running, gobj: GObjId) -> bool {
    item(r.game, gobj).msid == 2
}

/// Makes the flash of `kind` at `pos` from `parent`'s hand ([`steeredshot::spawn`]).
pub fn spawn(
    r: &mut Running,
    parent: GObjId,
    pos: Vec3,
    kind: i32,
    facing_dir: f32,
) -> Option<GObjId> {
    steeredshot::spawn(r, parent, pos, kind, facing_dir, &PK_FLASH)
}

/// Its end's word to its owner, Ness's or Kirby's special (`ftNs_SpecialN_SetNULL`).
fn owner_done(r: &mut Running, gobj: GObjId, ness: GObjId) {
    match ItKind::of(item(r.game, gobj).kind) {
        ItKind::NessPKFlash => r.ness_special_n_set_null(ness, &NESS),
        ItKind::KirbyNessPKFlash => r.ness_special_n_set_null(ness, &NS_KIRBY),
        _ => {}
    }
}

/// The flash loses Ness ([`steeredshot::let_go`]).
pub fn let_go(r: &mut Running, gobj: GObjId) {
    steeredshot::let_go(r, gobj, &PK_FLASH);
}

/// The flash's own start: up and away at its angle.
fn rise(r: &mut Running, gobj: GObjId) {
    let unk1 = reg(attr_f32(r, gobj, attr::UNK1));
    let rise = reg(attr_f32(r, gobj, attr::PEAK_RISE_HEIGHT));
    let ip = item_mut(r.game, gobj);
    let angle = mem(fp::fmadds(
        fp::fmuls(f64::from(0.017_453_292_f32), unk1),
        reg(ip.facing_dir),
        fp::frsp(fp::fdiv(std::f64::consts::PI, 2.0)),
    ));
    ip.x40_vel.x = mem(fp::fmuls(fp::fneg(rise), reg(math::cos(angle))));
    ip.x40_vel.y = mem(fp::fmuls(rise, reg(math::sin(angle))));
    ip.x40_vel.z = 0.0;
}

/// Whether the flash's owner (`xDE0`) still charges it.
fn owner_holding(r: &mut Running, gobj: GObjId, owner: GObjId) -> bool {
    match ItKind::of(item(r.game, gobj).kind) {
        ItKind::NessPKFlash => r.ness_special_n_holding(owner, &NESS),
        ItKind::KirbyNessPKFlash => r.ness_special_n_holding(owner, &NS_KIRBY),
        // The original reads a register it never set.
        _ => unported_call("itNesspkflash_UnkMotion0_Anim on another kind"),
    }
}

/// Its owner no longer charges it.
fn not_charged(r: &mut Running, gobj: GObjId, owner: GObjId) -> bool {
    !owner_holding(r, gobj, owner)
}

/// Into the burst's wait: state 1, its delay as its life.
fn to_burst(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    let delay = attr_f32(r, gobj, attr::EXPLOSION_DELAY);
    let ip = item_mut(r.game, gobj);
    ip.xD44_lifeTimer = delay;
    ip.xDAC_itcmd_var0 = 0;
}

/// `itNesspkflash_UnkMotion0_Anim`: its animation looping; unreflected, it grows to its full
/// size and, once its owner stops charging, waits to burst; so it does when its life runs out.
#[doc(alias = "itNesspkflash_UnkMotion0_Anim")]
pub fn animate_0(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 0, change::ANIM_UPDATE);
    }
    if steeredshot::grows(r, gobj, &PK_FLASH) {
        to_burst(r, gobj);
    }
    if lib2725::tick_lifetime(r.game, gobj) {
        to_burst(r, gobj);
    }
    steeredshot::scale(r, gobj, &PK_FLASH);
    false
}

/// `itNesspkflash_UnkMotion1_Anim`: unreflected and still its owner's, two frames before the
/// delay ends the burst takes its place; else it goes when its life runs out.
#[doc(alias = "itNesspkflash_UnkMotion1_Anim")]
pub fn animate_1(r: &mut Running, gobj: GObjId) -> bool {
    let delay = attr_f32(r, gobj, attr::EXPLOSION_DELAY);
    let ip = item_mut(r.game, gobj);
    ip.xDAC_itcmd_var0 = ip.xDAC_itcmd_var0.wrapping_add(1);
    let (owner, n, pos, facing, kind) = (
        ip.owner,
        ip.xDAC_itcmd_var0 as i32,
        ip.pos,
        ip.facing_dir,
        ip.kind,
    );
    let v = steeredshot::vars(r, gobj, &PK_FLASH);
    if v.reflected.get() == 0
        && let Some(ness) = v.owner.get()
        && owner == Some(ness)
        && n >= fp::fctiwz(reg(delay)).wrapping_sub(2)
    {
        let size = v.grown.get();
        let blast = match ItKind::of(kind) {
            ItKind::NessPKFlash => Some(ItKind::NessPKFlashExplosion),
            ItKind::KirbyNessPKFlash => Some(ItKind::KirbyNessPKFlashExplosion),
            _ => None,
        };
        if let Some(blast) = blast {
            steeredshot::blast_spawn(
                r,
                ness,
                gobj,
                pos,
                blast.id(),
                facing,
                size,
                &PK_FLASH_BLAST,
            );
        }
        return true;
    }
    steeredshot::scale(r, gobj, &PK_FLASH);
    lib2725::tick_lifetime(r.game, gobj)
}

/// `itNesspkflash_UnkMotion2_Anim`: the fizzle, until its life runs out.
#[doc(alias = "itNesspkflash_UnkMotion2_Anim")]
pub fn animate_2(r: &mut Running, gobj: GObjId) -> bool {
    steeredshot::scale(r, gobj, &PK_FLASH);
    lib2725::tick_lifetime(r.game, gobj)
}

/// `itNesspkflash_UnkMotion0_Phys`: while its owner charges it, the stick past 0.2 steers it
/// sideways up to `x20`; it falls by `x1C` to at most `x24`.
#[doc(alias = "itNesspkflash_UnkMotion0_Phys")]
pub fn physics_0(r: &mut Running, gobj: GObjId) {
    let control = reg(attr_f32(r, gobj, attr::CONTROL));
    let max_x = reg(attr_f32(r, gobj, attr::UNK2));
    let gravity = reg(attr_f32(r, gobj, attr::GRAVITY));
    let max_fall = reg(attr_f32(r, gobj, attr::UNK3));
    let owner = item(r.game, gobj).owner;
    let v = steeredshot::vars(r, gobj, &PK_FLASH);
    if v.reflected.get() == 0
        && let Some(ness) = v.owner.get()
        && owner == Some(ness)
        && owner_holding(r, gobj, ness)
    {
        let (sx, _) = r.game.ft_lstick(ness);
        let x = reg(sx);
        let abs = |v: f64| if v < 0.0 { fp::fneg(v) } else { v };
        if abs(x) > f64::from(0.2_f32) {
            let ip = item_mut(r.game, gobj);
            ip.x40_vel.x = mem(fp::fmadds(x, control, reg(ip.x40_vel.x)));
            let vx = reg(ip.x40_vel.x);
            if abs(vx) > max_x {
                ip.x40_vel.x = mem(if vx > 0.0 { max_x } else { fp::fneg(max_x) });
            }
        }
    }
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = mem(fp::fsubs(reg(ip.x40_vel.y), gravity));
    let lim = fp::fneg(max_fall);
    if reg(ip.x40_vel.y) < lim {
        ip.x40_vel.y = mem(lim);
    }
    ip.x40_vel.z = 0.0;
}

/// `itNesspkflash_UnkMotion0_Coll`: meeting the stage head on, it fizzles with a sound.
#[doc(alias = "itNesspkflash_UnkMotion0_Coll")]
pub fn collide_0(r: &mut Running, gobj: GObjId) -> bool {
    if steeredshot::hit_stage(r, gobj) {
        change_state(r, gobj, 2, change::ANIM_UPDATE);
        let delay = attr_f32(r, gobj, attr::EXPLOSION_DELAY);
        item_mut(r.game, gobj).xD44_lifeTimer = delay;
        stop_sfx2(r, gobj);
        play_sfx(r, gobj, SFX_FIZZLE, 0x7F, 0x40);
    }
    steeredshot::scale(r, gobj, &PK_FLASH);
    false
}

/// Forgets a GObj the flash points to, `itNesspkflash_Logic102_EvtUnk` (`it_8026B894`).
// Ports `itNesspkflash_Logic102_EvtUnk`; aliased once a check reaches it.
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
