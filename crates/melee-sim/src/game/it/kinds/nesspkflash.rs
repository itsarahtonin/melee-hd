//! PK Flash, `melee/it/kinds/itnesspkflash.c`: a flash that floats up from Ness's hand and
//! grows while he holds the special (he steers it with the stick), then waits a moment and
//! bursts ([`super::nesspkflashexplode`]); meeting the stage it fizzles (state 2).
//!
//! Kirby's copy (`It_Kind_Kirby_NessPKFlush`) runs the same code with his own checks, which
//! aren't ported.

use gekko_fp as fp;

use super::super::item::{
    article, change, change_state, debug_owner_bubbles, play_sfx, spawn_air, stop_sfx2,
};
use super::super::{ground, item, item_mut, jobj_of, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::code::unported_call;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::math;
use crate::mp::coll::{
    CEILING_HUG, CEILING_PUSH, FLOOR_HUG, FLOOR_PUSH, LEFT_WALL_MASK, RIGHT_WALL_MASK,
};
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
pub const PHYS_1: u32 = 0x802A_B128;
pub const PHYS_2: u32 = 0x802A_B140;
pub const COLL_0: u32 = 0x802A_B158;
pub const COLL_1: u32 = 0x802A_B29C;
pub const COLL_2: u32 = 0x802A_B2A4;
pub const REFLECTED: u32 = 0x802A_B2AC;
pub const CLANKED: u32 = 0x802A_B3C0;
pub const ABSORBED: u32 = 0x802A_B3C8;
pub const EVT_UNK: u32 = 0x802A_B3D0;

/// `itFlashAttributes`.
mod attr {
    pub const LIFETIMER: u32 = 0x0;
    pub const HITBOX_SIZE_MUL: u32 = 0x4;
    pub const GRAPHIC_SIZE_INIT_MUL: u32 = 0x8;
    pub const GRAPHIC_SIZE_GROWTH_MUL: u32 = 0xC;
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

/// Runs one of the flash's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM_0 => animate_0(r, gobj),
        ANIM_1 => animate_1(r, gobj),
        ANIM_2 => animate_2(r, gobj),
        COLL_0 => collide_0(r, gobj),
        COLL_1 | COLL_2 => false,
        REFLECTED => reflected(r, gobj),
        CLANKED | ABSORBED => true,
        _ => return None,
    })
}

/// Runs one of the flash's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => destroyed(r, gobj),
        PHYS_0 => physics_0(r, gobj),
        PHYS_1 | PHYS_2 => item_mut(r.game, gobj).x40_vel = Vec3::new(0.0, 0.0, 0.0),
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

/// The flash's owner, `it_802AA7E4`.
#[doc(alias = "it_802AA7E4")]
pub fn owner(r: &Running, gobj: GObjId) -> Option<GObjId> {
    item(r.game, gobj).owner
}

/// Whether the flash fizzled, `it_802AA7F0`: state 2.
#[doc(alias = "it_802AA7F0")]
pub fn released(r: &Running, gobj: GObjId) -> bool {
    item(r.game, gobj).msid == 2
}

/// Whether the flash met the stage head on, `it_802AA810`.
#[doc(alias = "it_802AA810")]
pub fn hit_stage(r: &mut Running, gobj: GObjId) -> bool {
    ground::air(r, gobj);
    let ip = item(r.game, gobj);
    let env = ip.x378_itemColl.env_flags;
    let mut hit = false;
    if ip.x40_vel.y > 0.0 {
        if env & (CEILING_PUSH | CEILING_HUG) != 0 {
            hit = true;
        }
    } else if env & (FLOOR_PUSH | FLOOR_HUG) != 0 {
        hit = true;
    }
    if ip.x40_vel.x > 0.0 {
        if env & LEFT_WALL_MASK != 0 {
            hit = true;
        }
    } else if env & RIGHT_WALL_MASK != 0 {
        hit = true;
    }
    hit
}

/// Makes the flash at `pos` from `parent`'s hand, `it_802AA8C0`.
#[doc(alias = "it_802AA8C0")]
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
    // `Item_InitSpawnPositionFromParent`.
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = lib26b1::parent_ecb_center(r.game, parent);
    // `Item_InitSpawnCommonFields(&spawn, gobj, facing_dir, true)`.
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(parent);
    spawn.x4_parent_gobj2 = spawn.x0_parent_gobj;
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let flash = spawn_air(r, &mut spawn)?;
    let life = attr_f32(r, flash, attr::LIFETIMER);
    let ip = item_mut(r.game, flash);
    // `Item_ClearCmdVars`.
    ip.xDB8_itcmd_var3 = 0;
    ip.xDB4_itcmd_var2 = 0;
    ip.xDB0_itcmd_var1 = 0;
    ip.xDAC_itcmd_var0 = 0;
    lib2725::set_lifetime(r.game, r.data, flash, life);
    let v = item_mut(r.game, flash).xDD4_itemVar.pk_flush();
    v.xDD8_PKFlash.set(0.0);
    v.xDDC_PKFlash.set(0);
    v.xDE0_PKFlash_Owner.set(Some(parent));
    v.xDE4_PKFlash.set(0);
    start(r, flash);
    Some(flash)
}

/// The flash's end, `it_2725_Logic102_Destroyed`: its hitboxes off, and its owner told
/// (`ftNs_SpecialN_SetNULL`) if it's still his.
#[doc(alias = "it_2725_Logic102_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    lib2725::clear_hitboxes(r.game, r.data, gobj);
    let ip = item_mut(r.game, gobj);
    let (owner, kind) = (ip.owner, ip.kind);
    let field = ip.xDD4_itemVar.pk_flush().xDE0_PKFlash_Owner.get();
    if let Some(ness) = field {
        if owner == Some(ness) {
            match kind {
                super::NESS_PK_FLUSH => r.ness_special_n_set_null(ness),
                super::KIRBY_NESS_PK_FLUSH => unported_call("ftKb_SpecialNNs_800FECBC"),
                _ => {}
            }
        }
        item_mut(r.game, gobj)
            .xDD4_itemVar
            .pk_flush()
            .xDE0_PKFlash_Owner
            .set(None);
    }
    item_mut(r.game, gobj).xDC8_word.flags.x13 = false;
}

/// The flash loses Ness, `it_802AAA50`.
#[doc(alias = "it_802AAA50")]
pub fn let_go(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.owner = None;
    ip.xDD4_itemVar.pk_flush().xDE0_PKFlash_Owner.set(None);
    ip.xDC8_word.flags.x13 = false;
}

/// The flash's start, `it_802AAA80`: state 0, its lifetime, and up and away at its angle.
#[doc(alias = "it_802AAA80")]
pub fn start(r: &mut Running, gobj: GObjId) {
    // `Item_ClearFlagsAndEnterState(gobj, ip, 0)`.
    lib26b1::disable_pickup(r.game, gobj);
    item_mut(r.game, gobj).xDC8_word.flags.x13 = false;
    lib2725::show(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    let life = attr_f32(r, gobj, attr::LIFETIMER);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    let v = item_mut(r.game, gobj).xDD4_itemVar.pk_flush();
    v.xDD8_PKFlash.set(0.0);
    v.xDDC_PKFlash.set(0);
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
    let owner = ip.owner;
    debug_owner_bubbles(r.game, gobj, owner);
}

/// Whether the flash's owner (`xDE0`) still charges it.
fn owner_holding(r: &mut Running, gobj: GObjId, owner: GObjId) -> bool {
    match item(r.game, gobj).kind {
        super::NESS_PK_FLUSH => r.ness_special_n_holding(owner),
        super::KIRBY_NESS_PK_FLUSH => unported_call("ftKb_SpecialNNs_800FEC78"),
        // The original reads a register it never set.
        _ => unported_call("itNesspkflash_UnkMotion0_Anim on another kind"),
    }
}

/// Into the burst's wait: state 1, its delay as its life.
fn to_burst(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    let delay = attr_f32(r, gobj, attr::EXPLOSION_DELAY);
    let ip = item_mut(r.game, gobj);
    ip.xD44_lifeTimer = delay;
    ip.xDAC_itcmd_var0 = 0;
}

/// The flash's scale by its growth.
fn set_scale(r: &mut Running, gobj: GObjId) {
    let (init, growth, full) = (
        reg(attr_f32(r, gobj, attr::GRAPHIC_SIZE_INIT_MUL)),
        reg(attr_f32(r, gobj, attr::GRAPHIC_SIZE_GROWTH_MUL)),
        reg(attr_f32(r, gobj, attr::HITBOX_SIZE_MUL)),
    );
    let grown = reg(item_mut(r.game, gobj)
        .xDD4_itemVar
        .pk_flush()
        .xDD8_PKFlash
        .get());
    let s = mem(fp::fmadds(
        grown,
        fp::fdivs(fp::fsubs(growth, init), full),
        init,
    ));
    let j = jobj_of(r.game, gobj).expect("PK Flash without a model");
    jobj::set_scale(&mut r.game.objs.jobjs, j, Vec3::new(s, s, s));
}

/// `itNesspkflash_UnkMotion0_Anim`: its animation looping; unreflected, it grows to its full
/// size and, once its owner stops charging, waits to burst; so it does when its life runs out.
#[doc(alias = "itNesspkflash_UnkMotion0_Anim")]
pub fn animate_0(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 0, change::ANIM_UPDATE);
    }
    let full = attr_f32(r, gobj, attr::HITBOX_SIZE_MUL);
    let ip = item_mut(r.game, gobj);
    let owner = ip.owner;
    let v = ip.xDD4_itemVar.pk_flush();
    if v.xDDC_PKFlash.get() == 0 {
        v.xDD8_PKFlash.update(|g| mem(fp::fadds(reg(g), 1.0)));
        if reg(v.xDD8_PKFlash.get()) >= reg(full) {
            v.xDD8_PKFlash.set(full);
        }
        if let Some(ness) = v.xDE0_PKFlash_Owner.get()
            && owner == Some(ness)
            && !owner_holding(r, gobj, ness)
        {
            to_burst(r, gobj);
        }
    }
    if lib2725::tick_lifetime(r.game, gobj) {
        to_burst(r, gobj);
    }
    set_scale(r, gobj);
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
    let v = ip.xDD4_itemVar.pk_flush();
    if v.xDDC_PKFlash.get() == 0
        && let Some(ness) = v.xDE0_PKFlash_Owner.get()
        && owner == Some(ness)
        && n >= fp::fctiwz(reg(delay)).wrapping_sub(2)
    {
        let size = v.xDD8_PKFlash.get();
        match kind {
            super::NESS_PK_FLUSH => {
                super::nesspkflashexplode::spawn(
                    r,
                    ness,
                    gobj,
                    pos,
                    super::NESS_PK_FLUSH_EXPLODE,
                    facing,
                    size,
                );
            }
            super::KIRBY_NESS_PK_FLUSH => {
                super::nesspkflashexplode::spawn(
                    r,
                    ness,
                    gobj,
                    pos,
                    super::KIRBY_NESS_PK_FLUSH_EXPLODE,
                    facing,
                    size,
                );
            }
            _ => {}
        }
        return true;
    }
    set_scale(r, gobj);
    lib2725::tick_lifetime(r.game, gobj)
}

/// `itNesspkflash_UnkMotion2_Anim`: the fizzle, until its life runs out.
#[doc(alias = "itNesspkflash_UnkMotion2_Anim")]
pub fn animate_2(r: &mut Running, gobj: GObjId) -> bool {
    set_scale(r, gobj);
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
    let ip = item_mut(r.game, gobj);
    let owner = ip.owner;
    let v = ip.xDD4_itemVar.pk_flush();
    if v.xDDC_PKFlash.get() == 0
        && let Some(ness) = v.xDE0_PKFlash_Owner.get()
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
    if hit_stage(r, gobj) {
        change_state(r, gobj, 2, change::ANIM_UPDATE);
        let delay = attr_f32(r, gobj, attr::EXPLOSION_DELAY);
        item_mut(r.game, gobj).xD44_lifeTimer = delay;
        stop_sfx2(r, gobj);
        play_sfx(r, gobj, SFX_FIZZLE, 0x7F, 0x40);
    }
    set_scale(r, gobj);
    false
}

/// Reflected, `itNesspkflash_Logic102_Reflected`: no longer steered or grown, turned and flying
/// back.
#[doc(alias = "itNesspkflash_Logic102_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    let j = jobj_of(r.game, gobj).expect("PK Flash without a model");
    let ip = item_mut(r.game, gobj);
    ip.xDD4_itemVar.pk_flush().xDDC_PKFlash.set(1);
    ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
    let y = mem(fp::frsp(fp::fmul(
        std::f64::consts::FRAC_PI_2,
        reg(ip.facing_dir),
    )));
    lib2725::set_rotation_y(r.game, j, y);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fneg(reg(ip.x40_vel.x)));
    ip.x40_vel.y = mem(fp::fneg(reg(ip.x40_vel.y)));
    ip.x40_vel.z = 0.0;
    false
}

/// Forgets a GObj the flash points to, `itNesspkflash_Logic102_EvtUnk` (`it_8026B894`).
// Ports `itNesspkflash_Logic102_EvtUnk`; aliased once a check reaches it.
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
