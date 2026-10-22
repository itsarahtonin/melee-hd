//! Samus's Charge Shot, `melee/it/kinds/itsamuschargeshot.c`: a ball in her hand that grows
//! with her charge (state 0) until she fires it, then a shot of the charge's size and speed
//! (states 1 to 8, by charge) that flies until its life runs out or it meets the stage.
//!
//! Kirby's copy (`It_Kind_Kirby_SamusCharge`) runs the same code with his own checks, which
//! aren't ported.

use gekko_fp as fp;

use super::super::item::{article, change, change_state, pick_up, spawn_air};
use super::super::{ground, item, item_mut, jobj_of, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::ef::{self, Arg};
use crate::game::handles::CodeRef;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::math;
use crate::mp::coll::{
    CEILING_HUG, CEILING_PUSH, FLOOR_HUG, FLOOR_PUSH, LEFT_WALL_MASK, RIGHT_WALL_MASK,
};
use crate::vec::Vec3;

/// `It_Kind_Samus_Charge`.
pub const KIND: i32 = super::SAMUS_CHARGE;
/// `It_Kind_Kirby_SamusCharge`.
const KIRBY_SAMUS_CHARGE: i32 = 151;

pub const HIT_STAGE: u32 = 0x802B_5518;
pub const SPAWN: u32 = 0x802B_55C8;
pub const FIRE: u32 = 0x802B_56E4;
pub const DESTROYED: u32 = 0x802B_58D8;
pub const REMOVE: u32 = 0x802B_5974;
pub const PICKED_UP: u32 = 0x802B_59C4;
pub const ANIM_0: u32 = 0x802B_5A98;
#[doc(alias = "itSamuschargeshot_UnkMotion0_Phys")]
pub const PHYS_0: u32 = 0x802B_5CB0;
#[doc(alias = "itSamuschargeshot_UnkMotion0_Coll")]
pub const COLL_0: u32 = 0x802B_5CB4;
pub const FIRED: u32 = 0x802B_5CBC;
pub const ANIM_8: u32 = 0x802B_5D54;
pub const PHYS_8: u32 = 0x802B_5E50;
#[doc(alias = "itSamuschargeshot_UnkMotion8_Coll")]
pub const COLL_8: u32 = 0x802B_5EA0;
pub const SPARKLE: u32 = 0x802B_5EDC;
#[doc(alias = "itSamusChargeshot_Logic108_DmgDealt")]
pub const DMG_DEALT: u32 = 0x802B_5FD8;
#[doc(alias = "itSamusChargeshot_Logic108_Clanked")]
pub const CLANKED: u32 = 0x802B_5FE0;
pub const ABSORBED: u32 = 0x802B_5FE8;
pub const REFLECTED: u32 = 0x802B_5FF0;
#[doc(alias = "itSamusChargeshot_Logic108_HitShield")]
pub const HIT_SHIELD: u32 = 0x802B_613C;
pub const SHIELD_BOUNCED: u32 = 0x802B_6144;
pub const EVT_UNK: u32 = 0x802B_62B0;

/// `itSamusChargeShot_Attributes`.
mod attr {
    pub const LIFETIME: u32 = 0x0;
    pub const SPEED_MIN: u32 = 0x8;
    pub const SPEED_MAX: u32 = 0xC;
    pub const DAMAGE_MIN: u32 = 0x10;
    pub const DAMAGE_MAX: u32 = 0x14;
    pub const SCALE_MIN: u32 = 0x18;
    pub const SCALE_MAX: u32 = 0x1C;
}

/// The ball's glow in her hand, and the full shot's sparkles.
const GFX_HELD: i32 = 0x47F;
const GFX_FULL: [i32; 2] = [0x480, 0x481];
/// Kirby's shot's glow in his hand and full charge's sparkles.
const GFX_HELD_KIRBY: i32 = 0x4A1;
const GFX_FULL_KIRBY: [i32; 2] = [0x4A2, 0x4A3];
/// `M_PI`, `M_TAU` and `M_PI / 2`.
const PI: f64 = std::f64::consts::PI;
const TAU: f64 = std::f64::consts::TAU;
const HALF_PI: f64 = std::f64::consts::FRAC_PI_2;

/// Runs one of the shot's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM_0 => animate_held(r, gobj),
        COLL_0 => false,
        ANIM_8 => animate_flying(r, gobj),
        COLL_8 => hit_stage(r, gobj),
        DMG_DEALT | CLANKED | ABSORBED | HIT_SHIELD => true,
        REFLECTED => reflected(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        _ => return None,
    })
}

/// Runs one of the shot's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => destroyed(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        PHYS_0 => {}
        PHYS_8 => physics_flying(r, gobj),
        SPARKLE => sparkle(r, gobj),
        _ => return false,
    }
    true
}

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let attrs = article(item(r.game, gobj))
        .special(r.data)
        .expect("the Charge Shot without attributes");
    r.data.f32(attrs.at(off))
}

/// The model's grandchild, which the shot scales (`itGetJObjGrandchild`).
fn grandchild(r: &Running, gobj: GObjId) -> JObjId {
    let root = jobj_of(r.game, gobj).expect("the Charge Shot without a model");
    let child = r.game.objs.jobjs[root]
        .child
        .expect("the shot's model's child");
    r.game.objs.jobjs[child]
        .child
        .expect("the shot's model's grandchild")
}

/// The shot's size for `charge` of `max`: from the attributes' `x18` to `x1C`.
fn charged_scale(r: &Running, gobj: GObjId, charge: i32, max: i32) -> f64 {
    let lo = reg(attr_f32(r, gobj, attr::SCALE_MIN));
    let hi = reg(attr_f32(r, gobj, attr::SCALE_MAX));
    fp::fmadds(
        fp::frsp(f64::from(charge)),
        fp::fdivs(fp::fsubs(hi, lo), fp::frsp(f64::from(max))),
        lo,
    )
}

/// Whether the shot flew into the stage, `it_802B5518`: a ceiling rising, a floor falling,
/// the wall it flies toward.
#[doc(alias = "it_802B5518")]
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

/// Makes the ball in `parent`'s hand at `pos`, `it_802B55C8`: its lifetime, not fired, her
/// model's scale kept, and held at her part `part`.
#[doc(alias = "it_802B55C8")]
pub fn spawn(
    r: &mut Running,
    parent: GObjId,
    pos: Vec3,
    part: i32,
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
    let shot = spawn_air(r, &mut spawn)?;
    let life = attr_f32(r, shot, attr::LIFETIME);
    let ip = item_mut(r.game, shot);
    // `Item_ClearCmdVars`.
    ip.xDB8_itcmd_var3 = 0;
    ip.xDB4_itcmd_var2 = 0;
    ip.xDB0_itcmd_var1 = 0;
    ip.xDAC_itcmd_var0 = 0;
    lib2725::set_lifetime(r.game, r.data, shot, life);
    let scale = mem(Running::model_scale(r.game.fighter(parent)));
    let v = item_mut(r.game, shot).xDD4_itemVar.samus_chargeshot();
    v.xDE8.set(0);
    v.xDE0.set(0.0);
    v.xDFC.set(0);
    v.xE00.set(Some(parent));
    v.xE04.set(scale);
    v.xE2C.set(0.0);
    v.xE30.set(0.0);
    v.xE34.set(0.0);
    pick_up(r, shot, parent, part);
    Some(shot)
}

/// Fires the shot from `pos` at `angle`, `it_802B56E4`: its charge (`charge` within 0 and
/// `max`) sets its speed, damage and size; while its owner is the one who made it, it leaves
/// her hand ([`fired`], `it_8027429C`), flies her way, and is fired (`xDE8`).
#[doc(alias = "it_802B56E4")]
pub fn fire(r: &mut Running, gobj: GObjId, pos: Vec3, angle: f32, charge: f32, max: f32) {
    let life = attr_f32(r, gobj, attr::LIFETIME);
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .samus_chargeshot()
        .xDD8
        .set(angle);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    let mut charge = reg(charge);
    if charge < 0.0 {
        charge = 0.0;
    }
    let max = reg(max);
    if charge > max {
        charge = max;
    }
    let v = item_mut(r.game, gobj).xDD4_itemVar.samus_chargeshot();
    v.xDEC.set(fp::fctiwz(charge));
    v.xDF0.set(fp::fctiwz(max));
    let ip = item_mut(r.game, gobj);
    let owner = ip.owner;
    let maker = ip.xDD4_itemVar.samus_chargeshot().xE00.get();
    if maker.is_none() || owner != maker {
        return;
    }
    let maker = maker.unwrap();
    fired(r, gobj);
    super::super::release::let_go(r, gobj, Vec3::new(0.0, 0.0, 0.0));
    item_mut(r.game, gobj).xDC8_word.flags.x14 = false;
    lib26b1::disable_pickup(r.game, gobj);
    let lerp = |r: &Running, lo, hi| {
        let (lo, hi) = (reg(attr_f32(r, gobj, lo)), reg(attr_f32(r, gobj, hi)));
        fp::fmadds(charge, fp::fdivs(fp::fsubs(hi, lo), max), lo)
    };
    let speed = lerp(r, attr::SPEED_MIN, attr::SPEED_MAX);
    let damage = lerp(r, attr::DAMAGE_MIN, attr::DAMAGE_MAX);
    let v = item_mut(r.game, gobj).xDD4_itemVar.samus_chargeshot();
    v.xDDC.set(mem(speed));
    v.xDF8.set(cvt_unsigned(damage) as i32);
    v.xDE0.set(0.0);
    let (level, levels) = (v.xDEC.get(), v.xDF0.get());
    let scale = charged_scale(r, gobj, level, levels);
    let facing = r.game.fighter(maker).facing_dir;
    let ip = item_mut(r.game, gobj);
    let v = ip.xDD4_itemVar.samus_chargeshot();
    v.xDE4.set(mem(scale));
    v.xDF4.set(0);
    ip.facing_dir = facing;
    ip.pos = pos;
    let v = ip.xDD4_itemVar.samus_chargeshot();
    v.xDFC.set(0);
    let (speed, angle) = (reg(v.xDDC.get()), v.xDD8.get());
    let c = reg(math::cos(angle));
    let s = reg(math::sin(angle));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(speed, c));
    ip.x40_vel.y = mem(fp::fmuls(speed, s));
    ip.x40_vel.z = 0.0;
    ip.xDD4_itemVar.samus_chargeshot().xDE8.set(1);
}

/// `__cvt_fp2unsigned`: a double to an unsigned word, 0 below zero and saturating above.
fn cvt_unsigned(v: f64) -> u32 {
    if v <= 0.0 {
        0
    } else if v >= 4_294_967_296.0 {
        u32::MAX
    } else {
        v as u32
    }
}

/// The shot's end, `it_2725_Logic108_Destroyed`: its effects gone; unfired, its maker lets it
/// go (`ftSs_SpecialN_801291F0`) if she still owns it; then no owner.
#[doc(alias = "it_2725_Logic108_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    ef::eflib::destroy_all(r, gobj);
    let ip = item_mut(r.game, gobj);
    let (owner, kind) = (ip.owner, ip.kind);
    let v = ip.xDD4_itemVar.samus_chargeshot();
    if v.xDE8.get() == 0 {
        if let Some(maker) = v.xE00.get()
            && owner == Some(maker)
        {
            match kind {
                KIND => r.samus_shot_released(maker),
                KIRBY_SAMUS_CHARGE => r.kb_ss_shot_released(maker),
                _ => {}
            }
        }
        item_mut(r.game, gobj)
            .xDD4_itemVar
            .samus_chargeshot()
            .xE00
            .set(None);
    }
    let ip = item_mut(r.game, gobj);
    ip.owner = None;
    ip.xDC8_word.flags.x13 = false;
}

/// The ball taken away from her hand, `it_802B5974`: its effects gone, and the item.
#[doc(alias = "it_802B5974")]
pub fn remove(r: &mut Running, gobj: GObjId) {
    ef::eflib::destroy_all(r, gobj);
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .samus_chargeshot()
        .xDFC
        .set(0);
    super::super::item::destroy(r, gobj);
}

/// In her hand, `it_2725_Logic108_PickedUp`: the held state and its glow.
#[doc(alias = "it_2725_Logic108_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    let j = grandchild(r, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    match item(r.game, gobj).kind {
        KIND => {
            ef::spawn(r, GFX_HELD, Some(gobj), &[Arg::JObj(Some(j))]);
            item_mut(r.game, gobj)
                .xDD4_itemVar
                .samus_chargeshot()
                .xDFC
                .set(1);
        }
        KIRBY_SAMUS_CHARGE => {
            ef::spawn(r, GFX_HELD_KIRBY, Some(gobj), &[Arg::JObj(Some(j))]);
            item_mut(r.game, gobj)
                .xDD4_itemVar
                .samus_chargeshot()
                .xDFC
                .set(1);
        }
        _ => {}
    }
}

/// `itSamuschargeshot_UnkMotion0_Anim`: while its maker holds it, the ball goes once she
/// loses it (hit, out of the charge's states, or holding none) and otherwise takes her charge;
/// it grows with the charge.
#[doc(alias = "itSamuschargeshot_UnkMotion0_Anim")]
pub fn animate_held(r: &mut Running, gobj: GObjId) -> bool {
    let j = grandchild(r, gobj);
    let ip = item_mut(r.game, gobj);
    let (owner, kind) = (ip.owner, ip.kind);
    let maker = ip.xDD4_itemVar.samus_chargeshot().xE00.get();
    if maker.is_some() && owner == maker {
        match kind {
            KIND => {
                if r.samus_shot_lost(maker) || r.samus_left_charge(maker) {
                    return true;
                }
                let Some((charge, max)) = r.samus_charge_of(maker) else {
                    return true;
                };
                let v = item_mut(r.game, gobj).xDD4_itemVar.samus_chargeshot();
                v.xDEC.set(charge);
                v.xDF0.set(max);
            }
            KIRBY_SAMUS_CHARGE => {
                if r.kb_ss_shot_lost(maker) || r.kb_ss_left_charge(maker) {
                    return true;
                }
                let Some((charge, max)) = r.kb_ss_charge_of(maker) else {
                    return true;
                };
                let v = item_mut(r.game, gobj).xDD4_itemVar.samus_chargeshot();
                v.xDEC.set(charge);
                v.xDF0.set(max);
            }
            _ => {}
        }
    }
    let v = item_mut(r.game, gobj).xDD4_itemVar.samus_chargeshot();
    let (charge, max) = (v.xDEC.get(), v.xDF0.get());
    let s = mem(charged_scale(r, gobj, charge, max));
    jobj::set_scale(&mut r.game.objs.jobjs, j, Vec3::new(s, s, s));
    false
}

/// Fired, `it_802B5CBC`: its lifetime, its effects gone, the state for its charge (1 to 8),
/// and the sparkles hooked.
#[doc(alias = "it_802B5CBC")]
pub fn fired(r: &mut Running, gobj: GObjId) {
    let life = attr_f32(r, gobj, attr::LIFETIME);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    ef::eflib::destroy_all(r, gobj);
    let v = item_mut(r.game, gobj).xDD4_itemVar.samus_chargeshot();
    v.xDFC.set(0);
    if v.xDEC.get() < 0 {
        v.xDEC.set(0);
    }
    if v.xDEC.get() >= 8 {
        v.xDEC.set(7);
    }
    let level = v.xDEC.get();
    change_state(r, gobj, level.wrapping_add(1), change::ANIM_UPDATE);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(SPARKLE));
}

/// `itSamuschargeshot_UnkMotion8_Anim`: its size, and its life counts down.
#[doc(alias = "itSamuschargeshot_UnkMotion8_Anim")]
pub fn animate_flying(r: &mut Running, gobj: GObjId) -> bool {
    let j = grandchild(r, gobj);
    let s = item_mut(r.game, gobj)
        .xDD4_itemVar
        .samus_chargeshot()
        .xDE4
        .get();
    jobj::set_scale(&mut r.game.objs.jobjs, j, Vec3::new(s, s, s));
    lib2725::tick_lifetime(r.game, gobj)
}

/// `itSamuschargeshot_UnkMotion8_Phys`: its speed along its angle.
#[doc(alias = "itSamuschargeshot_UnkMotion8_Phys")]
pub fn physics_flying(r: &mut Running, gobj: GObjId) {
    let v = item_mut(r.game, gobj).xDD4_itemVar.samus_chargeshot();
    let (speed, angle) = (reg(v.xDDC.get()), v.xDD8.get());
    let c = reg(math::cos(angle));
    let s = reg(math::sin(angle));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(speed, c));
    ip.x40_vel.y = mem(fp::fmuls(speed, s));
}

/// A fully charged shot's sparkles (its accessory), `it_802B5EDC`: every third frame.
#[doc(alias = "it_802B5EDC")]
pub fn sparkle(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    let ip = item_mut(r.game, gobj);
    let kind = ip.kind;
    let v = ip.xDD4_itemVar.samus_chargeshot();
    if v.xDEC.get() == v.xDF0.get() && v.xDF4.get() == 0 {
        match kind {
            KIND => {
                for gfx in GFX_FULL {
                    ef::spawn(r, gfx, Some(gobj), &[Arg::JObj(j)]);
                }
            }
            KIRBY_SAMUS_CHARGE => {
                for gfx in GFX_FULL_KIRBY {
                    ef::spawn(r, gfx, Some(gobj), &[Arg::JObj(j)]);
                }
            }
            _ => {}
        }
    }
    let v = item_mut(r.game, gobj).xDD4_itemVar.samus_chargeshot();
    let n = v.xDF4.get().wrapping_add(1);
    v.xDF4.set(n);
    let n = v.xDF4.get() % 3;
    v.xDF4.set(n);
}

/// The angle kept within a turn.
fn wrap(a: f32) -> f32 {
    let mut a = a;
    while a < 0.0 {
        a = mem(fp::frsp(fp::fadd(reg(a), TAU)));
    }
    while reg(a) > TAU {
        a = mem(fp::frsp(fp::fsub(reg(a), TAU)));
    }
    a
}

/// Reflected, `it_2725_Logic108_Reflected`: it faces and flies back.
#[doc(alias = "it_2725_Logic108_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    let j = jobj_of(r.game, gobj).expect("the Charge Shot without a model");
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
    let yaw = mem(fp::frsp(fp::fmul(HALF_PI, reg(ip.facing_dir))));
    jobj::set_rotation_y(&mut r.game.objs.jobjs, j, yaw);
    let v = item_mut(r.game, gobj).xDD4_itemVar.samus_chargeshot();
    let a = mem(fp::frsp(fp::fadd(reg(v.xDD8.get()), PI)));
    v.xDD8.set(wrap(a));
    false
}

/// Bouncing off a shield, `it_2725_Logic108_ShieldBounced`: its speed mirrored off the shield,
/// its angle and facing from the new speed.
#[doc(alias = "it_2725_Logic108_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    let j = jobj_of(r.game, gobj).expect("the Charge Shot without a model");
    let ip = item_mut(r.game, gobj);
    let normal = ip.xC58;
    ip.x40_vel.mirror_xy(normal);
    let a = math::atan2(ip.x40_vel.y, ip.x40_vel.x);
    let ip = item_mut(r.game, gobj);
    ip.xDD4_itemVar.samus_chargeshot().xDD8.set(wrap(a));
    ip.facing_dir = if ip.x40_vel.x >= 0.0 { 1.0 } else { -1.0 };
    let yaw = mem(fp::frsp(fp::fmul(HALF_PI, reg(ip.facing_dir))));
    jobj::set_rotation_y(&mut r.game.objs.jobjs, j, yaw);
    false
}

/// Forgets a GObj the shot points to, `itSamusChargeshot_Logic108_EvtUnk` (`it_8026B894`).
// Ports `itSamusChargeshot_Logic108_EvtUnk`; aliased once a check reaches it.
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
