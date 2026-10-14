//! Samus's missiles, `melee/it/kinds/itsamusmissile.c`: a homing missile (state 0) that turns
//! toward the nearest opponent or target for its first frames, or a super missile (state 1)
//! that waits, then speeds up straight ahead; both burst (states 2 and 3) on what they hit,
//! on the stage, at their life's end, or when she fires two more.

use gekko_fp as fp;

use super::super::item::{article, change, change_state, debug_owner_bubbles, spawn_air};
use super::super::{ground, item, item_mut, jobj_of, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::ef::{self, Arg};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::math;
use crate::mtx::Axis;
use crate::vec::Vec3;

/// `It_Kind_Samus_Missile`.
pub const KIND: i32 = super::SAMUS_MISSILE;

pub const SPAWN: u32 = 0x802B_62D0;
pub const LIFE: u32 = 0x802B_63F8;
pub const HOME: u32 = 0x802B_64FC;
pub const START_HOMING: u32 = 0x802B_66A8;
pub const ANIM_0: u32 = 0x802B_6774;
pub const PHYS_0: u32 = 0x802B_67E4;
pub const COLL_0: u32 = 0x802B_6A34;
pub const START_SUPER: u32 = 0x802B_6A60;
pub const ANIM_1: u32 = 0x802B_6B1C;
pub const PHYS_1: u32 = 0x802B_6B8C;
pub const COLL_1: u32 = 0x802B_6BFC;
pub const DMG_DEALT: u32 = 0x802B_6C28;
pub const CLANKED: u32 = 0x802B_6C7C;
pub const HIT_SHIELD: u32 = 0x802B_6CD0;
pub const SHIELD_BOUNCED: u32 = 0x802B_6D24;
pub const REFLECTED: u32 = 0x802B_6E4C;
pub const BURST_HOMING: u32 = 0x802B_701C;
pub const ANIM_BURST: u32 = 0x802B_7080;
pub const BURST_SUPER: u32 = 0x802B_70A0;
pub const EVT_UNK: u32 = 0x802B_7104;

/// `itSamusMissileAttributes`.
mod attr {
    pub const HOMING_LIFE: u32 = 0x4;
    pub const HOMING_TIME: u32 = 0x8;
    pub const HOMING_SPEED: u32 = 0xC;
    pub const HOMING_DECEL: u32 = 0x10;
    pub const HOMING_SPEED_MIN: u32 = 0x14;
    pub const TURN_STEP: u32 = 0x18;
    pub const TURN_MAX: u32 = 0x1C;
    pub const TURN_ANGLE_MIN: u32 = 0x20;
    pub const SUPER_LIFE: u32 = 0x24;
    pub const SUPER_WAIT: u32 = 0x28;
    pub const SUPER_SPEED: u32 = 0x2C;
    pub const SUPER_ACCEL: u32 = 0x30;
    pub const SUPER_SPEED_MAX: u32 = 0x34;
}

/// The homing missile's and the super missile's trails.
const GFX_HOMING: i32 = 0x485;
const GFX_SUPER: i32 = 0x484;
/// `M_PI / 2` in double precision, and `M_PI` as a float.
const HALF_PI: f64 = std::f64::consts::FRAC_PI_2;
const PI_F: f64 = 3.141_592_741_012_573_2;
/// `0.01745329238474369`: degrees to radians.
const DEG_TO_RAD: f64 = 0.017_453_292_384_743_69;

/// Runs one of the missile's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM_0 | ANIM_1 => animate(r, gobj),
        COLL_0 => {
            ground::fall_until_touch_unstick(r, gobj, burst_homing);
            false
        }
        COLL_1 => {
            ground::fall_until_touch_unstick(r, gobj, burst_super);
            false
        }
        DMG_DEALT | CLANKED | HIT_SHIELD => hit(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        REFLECTED => reflected(r, gobj),
        ANIM_BURST => lib2725::tick_life(r.game, gobj),
        _ => return None,
    })
}

/// Runs one of the missile's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PHYS_0 => physics_homing(r, gobj),
        PHYS_1 => physics_super(r, gobj),
        BURST_HOMING => burst_homing(r, gobj),
        BURST_SUPER => burst_super(r, gobj),
        _ => return false,
    }
    true
}

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let attrs = article(item(r.game, gobj))
        .special(r.data)
        .expect("a missile without attributes");
    r.data.f32(attrs.at(off))
}

/// The model's grandchild, which carries the trail (`itGetJObjGrandchild`).
fn grandchild(r: &Running, gobj: GObjId) -> JObjId {
    let root = jobj_of(r.game, gobj).expect("a missile without a model");
    let child = r.game.objs.jobjs[root]
        .child
        .expect("the missile's model's child");
    r.game.objs.jobjs[child]
        .child
        .expect("the missile's model's grandchild")
}

/// Fires a missile from `parent` at `pos`, `it_802B62D0`: a super missile if `smash`, each
/// knowing its owner and how many missiles she had fired.
#[doc(alias = "it_802B62D0")]
pub fn spawn(r: &mut Running, parent: GObjId, pos: Vec3, smash: bool, facing_dir: f32) {
    let mut spawn = SpawnItem {
        kind: KIND,
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
    let Some(missile) = spawn_air(r, &mut spawn) else {
        return;
    };
    let ip = item_mut(r.game, missile);
    let owner = ip.owner;
    let v = ip.xDD4_itemVar.samus_missile();
    v.is_smash_missile.set(i32::from(smash));
    v.owner.set(owner);
    if let Some(o) = v.owner.get() {
        let fired = r.samus_missiles(o);
        item_mut(r.game, missile)
            .xDD4_itemVar
            .samus_missile()
            .x4
            .set(fired);
    }
    if item_mut(r.game, missile)
        .xDD4_itemVar
        .samus_missile()
        .is_smash_missile
        .get()
        == 0
    {
        start_homing(r, missile);
    } else {
        start_super(r, missile);
    }
    debug_owner_bubbles(r.game, missile, Some(parent));
}

/// The missile's life, `it_802B63F8`: it homes (`xDAC`) for the attributes' first frames,
/// a homing missile's trail goes when the homing ends, a super missile speeds up (`xDB0`)
/// after its wait, and at the end of its life it bursts.
#[doc(alias = "it_802B63F8")]
pub fn life(r: &mut Running, gobj: GObjId) {
    let (life, time) = (
        reg(attr_f32(r, gobj, attr::HOMING_LIFE)),
        reg(attr_f32(r, gobj, attr::HOMING_TIME)),
    );
    let (slife, wait) = (
        reg(attr_f32(r, gobj, attr::SUPER_LIFE)),
        reg(attr_f32(r, gobj, attr::SUPER_WAIT)),
    );
    let homing_end = fp::fsubs(life, time);
    let ip = item_mut(r.game, gobj);
    ip.xDAC_itcmd_var0 = u32::from(reg(ip.xD44_lifeTimer) > homing_end);
    let smash = ip.xDD4_itemVar.samus_missile().is_smash_missile.get() != 0;
    if reg(ip.xD44_lifeTimer) == homing_end && !smash {
        ef::eflib::destroy_all(r, gobj);
    }
    let ip = item_mut(r.game, gobj);
    if reg(ip.xD44_lifeTimer) <= fp::fsubs(slife, wait) {
        ip.xDB0_itcmd_var1 = 1;
    }
    if ip.xD44_lifeTimer <= 0.0 {
        if !smash {
            burst_homing(r, gobj);
        } else {
            burst_super(r, gobj);
        }
    } else {
        ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), fp::frsp(1.0)));
    }
}

/// Turns the homing missile toward its target, `it_802B64FC`: the nearest opponent ahead (by
/// its camera bone), else the nearest target item ahead; once the target is more than the
/// attributes' `x20` off its course, its turn (`x8`) grows by `x18` toward it, up to `x1C`.
#[doc(alias = "it_802B64FC")]
pub fn home(r: &mut Running, gobj: GObjId) {
    let (pos, owner, facing) = {
        let ip = item(r.game, gobj);
        (ip.pos, ip.owner, ip.facing_dir)
    };
    let target =
        match super::super::owner::nearest_opponent_ahead(r.game, r.data, pos, owner, facing) {
            Some((_, at)) => at,
            None => match lib26b1::nearest_target_item(r.game, pos, facing) {
                Some(t) => lib26b1::ecb_center(r.game, t),
                None => return,
            },
        };
    if target.x == 0.0 && target.y == 0.0 {
        return;
    }
    let ip = item(r.game, gobj);
    let mut course = Vec3::new(ip.x40_vel.x, ip.x40_vel.y, 0.0);
    course.normalize_xy();
    let mut to = Vec3::new(
        mem(fp::fsubs(reg(target.x), reg(ip.pos.x))),
        mem(fp::fsubs(reg(target.y), reg(ip.pos.y))),
        0.0,
    );
    to.normalize_xy();
    let off = course.angle_xy(to);
    if off < attr_f32(r, gobj, attr::TURN_ANGLE_MIN) {
        return;
    }
    let d = to - course;
    let step = reg(attr_f32(r, gobj, attr::TURN_STEP));
    let max = attr_f32(r, gobj, attr::TURN_MAX);
    let v = item_mut(r.game, gobj).xDD4_itemVar.samus_missile();
    if reg(d.y) > 0.001_000_000_047_497_451_3 {
        let t = reg(v.x8.get());
        v.x8.set(mem(fp::fsubs(t, step)));
    } else if reg(d.y) < 0.001_000_000_047_497_451_3 {
        let t = reg(v.x8.get());
        v.x8.set(mem(fp::fadds(t, step)));
    }
    // `itSamusMissile_ClampTurn`.
    let turn = v.x8.get();
    let mag = if turn < 0.0 {
        mem(fp::fneg(reg(turn)))
    } else {
        turn
    };
    if mag > max {
        v.x8.set(if turn > 0.0 {
            max
        } else {
            mem(fp::fneg(reg(max)))
        });
    }
}

/// The homing missile's start, `it_802B66A8`: flying ahead at the attributes' `xC`, its
/// script's variables cleared, its life, no turn, the homing state and its trail.
#[doc(alias = "it_802B66A8")]
pub fn start_homing(r: &mut Running, gobj: GObjId) {
    let speed = reg(attr_f32(r, gobj, attr::HOMING_SPEED));
    let life = attr_f32(r, gobj, attr::HOMING_LIFE);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(speed, reg(ip.facing_dir)));
    ip.x40_vel.y = 0.0;
    ip.xDB8_itcmd_var3 = 0;
    ip.xDB4_itcmd_var2 = 0;
    ip.xDAC_itcmd_var0 = 0;
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .samus_missile()
        .x8
        .set(0.0);
    lib26b1::disable_pickup(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    let j = grandchild(r, gobj);
    ef::spawn(r, GFX_HOMING, Some(gobj), &[Arg::JObj(Some(j))]);
}

/// `itSamusmissile_UnkMotion0_Anim` and `itSamusmissile_UnkMotion1_Anim`: its life; unless
/// reflected, it bursts once its owner has fired two missiles since.
#[doc(alias = "itSamusmissile_UnkMotion0_Anim")]
#[doc(alias = "itSamusmissile_UnkMotion1_Anim")]
pub fn animate(r: &mut Running, gobj: GObjId) -> bool {
    life(r, gobj);
    let ip = item_mut(r.game, gobj);
    if ip.xDB8_itcmd_var3 == 0 {
        let v = ip.xDD4_itemVar.samus_missile();
        if let Some(owner) = v.owner.get() {
            let at = v.x4.get();
            if (r.samus_missiles(owner).wrapping_sub(at) as u32) >= 2 {
                hit(r, gobj);
            }
        }
    }
    false
}

/// The missile's model turned to its turn (`x8`) about x, on its child.
fn turn_model(r: &mut Running, gobj: GObjId) {
    let root = jobj_of(r.game, gobj).expect("a missile without a model");
    let child = r.game.objs.jobjs[root]
        .child
        .expect("the missile's model's child");
    let turn = item_mut(r.game, gobj).xDD4_itemVar.samus_missile().x8.get();
    jobj::set_rotation_x(&mut r.game.objs.jobjs, child, turn);
}

/// `itSamusmissile_UnkMotion0_Phys`: homing, it turns toward its target and flies its
/// speed along the turn; then it slows by the attributes' `x10` down to `x14`.
#[doc(alias = "itSamusmissile_UnkMotion0_Phys")]
pub fn physics_homing(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).xDAC_itcmd_var0 != 0 {
        home(r, gobj);
        let speed = reg(attr_f32(r, gobj, attr::HOMING_SPEED));
        let ip = item_mut(r.game, gobj);
        let mut v = Vec3::new(mem(fp::fmuls(speed, reg(ip.facing_dir))), 0.0, 0.0);
        let turn = ip.xDD4_itemVar.samus_missile().x8.get();
        let facing = reg(item(r.game, gobj).facing_dir);
        v.rotate(Axis::Z, mem(fp::fmuls(reg(turn), fp::fneg(facing))));
        let ip = item_mut(r.game, gobj);
        ip.x40_vel.x = v.x;
        ip.x40_vel.y = v.y;
    } else {
        let decel = reg(attr_f32(r, gobj, attr::HOMING_DECEL));
        let min = reg(attr_f32(r, gobj, attr::HOMING_SPEED_MIN));
        let ip = item_mut(r.game, gobj);
        let (x, y) = (reg(ip.x40_vel.x), reg(ip.x40_vel.y));
        if fp::fmadds(x, x, fp::fmuls(y, y)) > fp::fmuls(min, min) {
            ip.x40_vel.x = mem(fp::fmuls(x, decel));
            ip.x40_vel.y = mem(fp::fmuls(reg(ip.x40_vel.y), decel));
        }
    }
    turn_model(r, gobj);
}

/// The super missile's start, `it_802B6A60`: flying ahead at the attributes' `x2C`, its
/// script's variables cleared, its life, the super state and its trail.
#[doc(alias = "it_802B6A60")]
pub fn start_super(r: &mut Running, gobj: GObjId) {
    let speed = reg(attr_f32(r, gobj, attr::SUPER_SPEED));
    let life = attr_f32(r, gobj, attr::SUPER_LIFE);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(speed, reg(ip.facing_dir)));
    ip.x40_vel.y = 0.0;
    ip.xDB8_itcmd_var3 = 0;
    ip.xDB4_itcmd_var2 = 0;
    ip.xDB0_itcmd_var1 = 0;
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    lib26b1::disable_pickup(r.game, gobj);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    let j = grandchild(r, gobj);
    ef::spawn(r, GFX_SUPER, Some(gobj), &[Arg::JObj(Some(j))]);
}

/// `itSamusmissile_UnkMotion1_Phys`: after its wait (`xDB0`), speeding up by the attributes'
/// `x30` up to `x34`.
#[doc(alias = "itSamusmissile_UnkMotion1_Phys")]
pub fn physics_super(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).xDB0_itcmd_var1 == 0 {
        return;
    }
    let accel = reg(attr_f32(r, gobj, attr::SUPER_ACCEL));
    let max = attr_f32(r, gobj, attr::SUPER_SPEED_MAX);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmadds(accel, reg(ip.facing_dir), reg(ip.x40_vel.x)));
    let x = reg(ip.x40_vel.x);
    let speed = if x < fp::frsp(0.0) { fp::fneg(x) } else { x };
    if speed > reg(max) {
        ip.x40_vel.x = if ip.facing_dir == 1.0 {
            max
        } else {
            mem(fp::fneg(reg(max)))
        };
    }
}

/// Hitting, clanking or meeting a shield, `it_2725_Logic52_DmgDealt`, `_Clanked` and
/// `_HitShield`: it bursts unless it is bursting.
#[doc(alias = "it_2725_Logic52_DmgDealt")]
#[doc(alias = "it_2725_Logic52_Clanked")]
#[doc(alias = "it_2725_Logic52_HitShield")]
pub fn hit(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    let msid = ip.msid;
    if ip.xDD4_itemVar.samus_missile().is_smash_missile.get() == 0 {
        if msid != 2 {
            burst_homing(r, gobj);
        }
    } else if msid != 3 {
        burst_super(r, gobj);
    }
    false
}

/// Bouncing off a shield, `it_2725_Logic52_ShieldBounced`: a super missile bounces, its
/// model turned along its new course.
#[doc(alias = "it_2725_Logic52_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    if item_mut(r.game, gobj)
        .xDD4_itemVar
        .samus_missile()
        .is_smash_missile
        .get()
        == 0
    {
        return false;
    }
    lib2725::bounce_off_shield(r.game, gobj);
    let ip = item(r.game, gobj);
    let a = reg(math::atan2(ip.x40_vel.x, ip.x40_vel.y));
    let turn = if ip.facing_dir == 1.0 {
        fp::fsubs(a, fp::fdivs(PI_F, 2.0))
    } else {
        fp::fsubs(fp::fmuls(fp::frsp(270.0), DEG_TO_RAD), a)
    };
    let root = jobj_of(r.game, gobj).expect("a missile without a model");
    let child = r.game.objs.jobjs[root]
        .child
        .expect("the missile's model's child");
    jobj::set_rotation_x(&mut r.game.objs.jobjs, child, mem(turn));
    false
}

/// Reflected, `it_2725_Logic52_Reflected`: it no longer bursts on its owner's next shots,
/// flies back (a super missile at full speed, rising if it fell), faces its way, and gets
/// its trail anew.
#[doc(alias = "it_2725_Logic52_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    let speed = reg(attr_f32(r, gobj, attr::SUPER_SPEED));
    item_mut(r.game, gobj).xDB8_itcmd_var3 = 1;
    lib2725::reflected_back(r.game, gobj);
    let ip = item_mut(r.game, gobj);
    let smash = ip.xDD4_itemVar.samus_missile().is_smash_missile.get() != 0;
    if smash {
        ip.x40_vel.x = mem(fp::fmuls(speed, reg(ip.facing_dir)));
    }
    if ip.x40_vel.y < 0.0 {
        ip.x40_vel.y = mem(fp::fmuls(fp::fneg(reg(ip.x40_vel.y)), reg(ip.xC70)));
    }
    let yaw = mem(fp::fmuls(fp::frsp(HALF_PI), reg(ip.facing_dir)));
    let root = jobj_of(r.game, gobj).expect("a missile without a model");
    jobj::set_rotation_y(&mut r.game.objs.jobjs, root, yaw);
    ef::eflib::destroy_all(r, gobj);
    let j = grandchild(r, gobj);
    let gfx = if smash { GFX_SUPER } else { GFX_HOMING };
    ef::spawn(r, gfx, Some(gobj), &[Arg::JObj(Some(j))]);
    false
}

/// The burst's common start: its model hidden, `xDD0` bit 3, vanishing, still, no effects.
fn burst_start(r: &mut Running, gobj: GObjId) {
    lib26b1::hide(r.game, gobj);
    lib26b1::set_xdd0_b3(r.game, gobj);
    lib2725::start_vanishing(r.game, r.data, gobj);
    lib2725::clear_velocity(r.game, gobj);
    ef::eflib::destroy_all(r, gobj);
}

/// The homing missile's burst, `it_802B701C`: its blast (`it_80272B40`) and state 2.
#[doc(alias = "it_802B701C")]
pub fn burst_homing(r: &mut Running, gobj: GObjId) {
    burst_start(r, gobj);
    lib2725::blast_small(r, gobj);
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// The super missile's burst, `it_802B70A0`: its blast (`it_80272A60`) and state 3.
#[doc(alias = "it_802B70A0")]
pub fn burst_super(r: &mut Running, gobj: GObjId) {
    burst_start(r, gobj);
    lib2725::blast_large(r, gobj);
    change_state(r, gobj, 3, change::ANIM_UPDATE);
}

/// Forgets a GObj the missile points to, `it_2725_Logic52_EvtUnk`: and its owner, if that.
// Ports `it_2725_Logic52_EvtUnk`; aliased once a check reaches it.
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
    let v = item_mut(r.game, gobj).xDD4_itemVar.samus_missile();
    if v.owner.get() == other {
        v.owner.set(None);
    }
}
