//! PK Thunder's ball, `melee/it/kinds/itnesspkthunderball.c`: Ness steers it with the stick
//! while he holds it (`ftNs_SpecialHi_CheckSpecialHiHold`), it keeps its last sixteen positions
//! and angles for its trails ([`super::nesspkthundertrail`]), and it goes when its life runs
//! out, it meets the stage, Ness lets go, or it hits something.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::item::{article, change, change_state, debug_owner_bubbles, spawn_air};
use super::super::{ground, item, item_mut, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::math;
use crate::mp::coll::{
    CEILING_HUG, CEILING_PUSH, FLOOR_HUG, FLOOR_PUSH, LEFT_WALL_MASK, RIGHT_WALL_MASK,
};
use crate::vec::Vec3;

pub const TRAIL_POS: u32 = 0x802A_B3F0;
pub const SPEED: u32 = 0x802A_B468;
pub const HIT_STAGE: u32 = 0x802A_B4B8;
pub const OWNER: u32 = 0x802A_B568;
pub const SPAWN: u32 = 0x802A_B58C;
pub const DESTROYED: u32 = 0x802A_B90C;
pub const REMOVE: u32 = 0x802A_B9C0;
pub const START: u32 = 0x802A_BA4C;
pub const ANIM: u32 = 0x802A_BBB0;
pub const PHYS: u32 = 0x802A_BCDC;
pub const COLL: u32 = 0x802A_C000;
#[doc(alias = "itNessPKThunderball_Logic26_DmgDealt")]
pub const DMG_DEALT: u32 = 0x802A_C050;
pub const CLANKED: u32 = 0x802A_C074;
pub const REFLECTED: u32 = 0x802A_C098;
pub const ABSORBED: u32 = 0x802A_C338;
pub const SHIELD_BOUNCED: u32 = 0x802A_C35C;
pub const HIT_SHIELD: u32 = 0x802A_C3F8;
pub const EVT_UNK: u32 = 0x802A_C41C;

/// `itPKThunderAttributes`: the lifetime, the speed, the first angle (degrees), the stick's
/// threshold and the turn (degrees a frame).
mod attr {
    pub const LIFETIME: u32 = 0x0;
    pub const SPEED: u32 = 0x4;
    pub const SPAWN_ANGLE: u32 = 0x8;
    pub const STICK_THRESHOLD: u32 = 0xC;
    pub const TURN_RADIUS: u32 = 0x10;
}

/// `MTXDegToRad`'s factor, a float.
const DEG_TO_RAD: f32 = 0.017_453_292;

/// The stage kind whose camera ignores the ball (Home-Run Contest's, `Stage_80225194` 0x54).
const STKIND_NO_CAMERA: i32 = 0x54;

/// Runs one of the ball's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => animate(r, gobj),
        COLL => collide(r, gobj),
        DMG_DEALT | CLANKED | ABSORBED | HIT_SHIELD => {
            destroyed(r, gobj);
            true
        }
        REFLECTED => reflected(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        _ => return None,
    })
}

/// Runs one of the ball's events, if `addr` is one.
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
        .expect("PK Thunder without attributes");
    r.data.f32(attrs.at(off))
}

/// Where trail `idx` is, `it_802AB3F0`: the ball's position `2 × idx` frames back, or the
/// origin for no ball.
#[doc(alias = "it_802AB3F0")]
pub fn trail_pos(r: &mut Running, gobj: GObjId, idx: i32) -> Vec3 {
    item_mut(r.game, gobj).xDD4_itemVar.pk_thunder().positions[(idx * 2) as usize].get()
}

/// The ball's speed, `it_802AB468`.
#[doc(alias = "it_802AB468")]
pub fn speed(r: &mut Running, gobj: GObjId) -> f32 {
    item_mut(r.game, gobj).xDD4_itemVar.pk_thunder().xEEC.get()
}

/// Whether the ball met the stage head on, `it_802AB4B8`: the airborne pass, then the
/// ceiling rising or the floor falling, and the wall it flies at.
#[doc(alias = "it_802AB4B8")]
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

/// The ball's owner, `it_802AB568`.
#[doc(alias = "it_802AB568")]
pub fn owner(r: &Running, gobj: GObjId) -> Option<GObjId> {
    item(r.game, gobj).owner
}

/// Makes the ball at Ness's hand, `it_802AB58C`: every kept position at `pos`, every angle the
/// first one, no trails, Ness its owner (`xF04`).
#[doc(alias = "it_802AB58C")]
pub fn spawn(r: &mut Running, owner: GObjId, pos: Vec3, facing_dir: f32) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind: super::NESS_PK_THUNDER,
        ..Default::default()
    };
    // `Item_InitSpawnPositionFromParent`.
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = lib26b1::parent_ecb_center(r.game, owner);
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(owner);
    spawn.x4_parent_gobj2 = spawn.x0_parent_gobj;
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = super::super::item::next_attack_id(r.game) as _;
    let ball = spawn_air(r, &mut spawn)?;
    let life = attr_f32(r, ball, attr::LIFETIME);
    let first = mem(fp::fmuls(
        f64::from(DEG_TO_RAD),
        reg(attr_f32(r, ball, attr::SPAWN_ANGLE)),
    ));
    let ip = item_mut(r.game, ball);
    // `Item_ClearCmdVars`.
    ip.xDB8_itcmd_var3 = 0;
    ip.xDB4_itcmd_var2 = 0;
    ip.xDB0_itcmd_var1 = 0;
    ip.xDAC_itcmd_var0 = 0;
    lib2725::set_lifetime(r.game, r.data, ball, life);
    let v = item_mut(r.game, ball).xDD4_itemVar.pk_thunder();
    v.xEF4.set(None);
    for p in &mut v.positions {
        p.set(pos);
    }
    for a in &mut v.angles {
        a.set(first);
    }
    for t in &mut v.xDD4 {
        t.set(None);
    }
    v.xEFC.set(0);
    v.xEF8.set(0);
    v.xF00.set(0);
    v.xF04.set(Some(owner));
    v.xF08.set(None);
    start(r, ball);
    item_mut(r.game, ball).xDCC_flag.b3 = false;
    if crate::game::gr::stage::stkind(r.game) == STKIND_NO_CAMERA {
        super::super::item::remove_camera_box(r.game, ball);
    }
    Some(ball)
}

/// The ball's end, `it_802AB90C`: its hitboxes off, Ness told (if he still steers his own),
/// its trails let go of.
#[doc(alias = "it_802AB90C")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    lib2725::clear_hitboxes(r.game, r.data, gobj);
    let ip = item_mut(r.game, gobj);
    let owner = ip.owner;
    let v = ip.xDD4_itemVar.pk_thunder();
    if let Some(ness) = v.xF04.get() {
        if v.xF00.get() == 0 && Some(ness) == owner {
            r.ness_thunder_remove(ness);
        }
        item_mut(r.game, gobj)
            .xDD4_itemVar
            .pk_thunder()
            .xF04
            .set(None);
    }
    for i in 0..6 {
        let t = item_mut(r.game, gobj).xDD4_itemVar.pk_thunder().xDD4[i].get();
        if let Some(t) = t {
            super::nesspkthundertrail::let_go(r, t);
            item_mut(r.game, gobj).xDD4_itemVar.pk_thunder().xDD4[i].set(None);
        }
    }
    item_mut(r.game, gobj).xDC8_word.flags.x13 = false;
}

/// Ness hit while steering, `it_802AB9C0`: unless reflected, the ball loses its owner, as do
/// its trails.
#[doc(alias = "it_802AB9C0")]
pub fn remove(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    if ip.xDD4_itemVar.pk_thunder().xF00.get() != 0 {
        return;
    }
    ip.owner = None;
    ip.xDC8_word.flags.x13 = false;
    for i in 0..6 {
        let t = item_mut(r.game, gobj).xDD4_itemVar.pk_thunder().xDD4[i].get();
        if let Some(t) = t {
            super::nesspkthundertrail::orphan(r, t);
        }
    }
}

/// The flight's start, `it_802ABA4C`: state 0, its lifetime, every angle the first one, and
/// straight up at its speed.
#[doc(alias = "it_802ABA4C")]
pub fn start(r: &mut Running, gobj: GObjId) {
    // `Item_ClearFlagsAndEnterState(gobj, ip, 0)`.
    lib26b1::disable_pickup(r.game, gobj);
    item_mut(r.game, gobj).xDC8_word.flags.x13 = false;
    lib2725::show(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    let life = attr_f32(r, gobj, attr::LIFETIME);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    let first = mem(fp::fmuls(
        f64::from(DEG_TO_RAD),
        reg(attr_f32(r, gobj, attr::SPAWN_ANGLE)),
    ));
    let speed = attr_f32(r, gobj, attr::SPEED);
    let ip = item_mut(r.game, gobj);
    let v = ip.xDD4_itemVar.pk_thunder();
    for a in &mut v.angles {
        a.set(first);
    }
    v.xEEC.set(speed);
    ip.x40_vel.y = speed;
    ip.x40_vel.z = 0.0;
    ip.x40_vel.x = 0.0;
    let ness = item_mut(r.game, gobj).xDD4_itemVar.pk_thunder().xF04.get();
    debug_owner_bubbles(r.game, gobj, ness);
}

/// `itNesspkthunderball_UnkMotion0_Anim`: a trail every other frame for its first six; while
/// Ness steers it, it goes once he stops holding it, and once someone else owns it; else it
/// goes when its life runs out.
#[doc(alias = "itNesspkthunderball_UnkMotion0_Anim")]
pub fn animate(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    let owner = ip.owner;
    let (pos, facing, ignore) = (ip.pos, ip.facing_dir, ip.xAC4_ignoreItemID);
    let v = ip.xDD4_itemVar.pk_thunder();
    let frame = v.xEF8.get();
    if matches!(frame, 0 | 2 | 4 | 6 | 8 | 10) && v.xEFC.get() <= 5 {
        let ness = v.xF04.get();
        let idx = v.xEFC.get();
        let parent = if owner == ness { ness } else { None };
        // PK Thunder's own spawn sets its id; a stale one would pass on a stack word.
        let ignore = ignore.read(&mut r.game.objs.stale_reads).unwrap_or(0) as i32;
        let t = super::nesspkthundertrail::spawn(r, parent, gobj, pos, idx, ignore, facing);
        let v = item_mut(r.game, gobj).xDD4_itemVar.pk_thunder();
        v.xDD4[idx as usize].set(t);
        v.xEFC.update(|n| n.wrapping_add(1));
    }
    let ip = item_mut(r.game, gobj);
    let owner = ip.owner;
    let v = ip.xDD4_itemVar.pk_thunder();
    v.xEF8.update(|n| n.wrapping_add(1));
    if v.xF00.get() == 0
        && let Some(ness) = v.xF04.get()
    {
        if owner == Some(ness) {
            if !r.ness_special_hi_holding(ness) {
                return true;
            }
        } else {
            destroyed(r, gobj);
            return true;
        }
    }
    lib2725::tick_lifetime(r.game, gobj)
}

/// `itNesspkthunderball_UnkMotion0_Phys`: the angles move down their history; while Ness
/// steers it, a stick past the threshold turns it toward the stick (by the turn rate, or in
/// proportion under 45°), and its velocity follows its angle; its positions move down too.
#[doc(alias = "itNesspkthunderball_UnkMotion0_Phys")]
pub fn physics(r: &mut Running, gobj: GObjId) {
    let threshold = reg(attr_f32(r, gobj, attr::STICK_THRESHOLD));
    let turn = reg(attr_f32(r, gobj, attr::TURN_RADIUS));
    let ip = item_mut(r.game, gobj);
    let owner = ip.owner;
    let v = ip.xDD4_itemVar.pk_thunder();
    let ness = v.xF04.get();
    let steered = v.xF00.get() == 0
        && ness.is_some()
        && owner == ness
        && r.ness_special_hi_holding(ness.unwrap());
    shift_angles(r, gobj);
    if steered {
        let (sx, sy) = r.game.ft_lstick(ness.unwrap());
        let stick = Vec3::new(sx, sy, 0.0);
        let abs = |x: f64| if x < 0.0 { fp::fneg(x) } else { x };
        if abs(reg(sx)) > threshold || abs(reg(sy)) > threshold {
            let ip = item_mut(r.game, gobj);
            let vel = ip.x40_vel;
            let angle = reg(vel.angle(stick));
            let cross = vel.cross_normalized(stick);
            let deg = f64::from(DEG_TO_RAD);
            let v = ip.xDD4_itemVar.pk_thunder();
            let big = fp::fmuls(45.0, deg);
            if angle >= big {
                if cross.z > 0.0 {
                    v.angles[0].update(|a| mem(fp::fmadds(deg, turn, reg(a))));
                } else if cross.z < 0.0 {
                    v.angles[0].update(|a| mem(fp::fnmsubs(deg, turn, reg(a))));
                }
            }
            if angle < big {
                let step = fp::fdivs(angle, fp::fdivs(45.0, turn));
                if cross.z > 0.0 {
                    v.angles[0].update(|a| mem(fp::fadds(reg(a), step)));
                } else if cross.z < 0.0 {
                    v.angles[0].update(|a| mem(fp::fsubs(reg(a), step)));
                }
            }
            let tau = std::f64::consts::TAU;
            while reg(v.angles[0].get()) < fp::fneg(tau) {
                v.angles[0].update(|a| mem(fp::frsp(fp::fadd(reg(a), tau))));
            }
            while reg(v.angles[0].get()) > tau {
                v.angles[0].update(|a| mem(fp::frsp(fp::fsub(reg(a), tau))));
            }
            set_velocity(r, gobj);
        }
    }
    let ip = item_mut(r.game, gobj);
    let pos = ip.pos;
    let v = ip.xDD4_itemVar.pk_thunder();
    for i in (0..15).rev() {
        let p = v.positions[i].get();
        v.positions[i + 1].set(p);
    }
    v.positions[0].set(pos);
}

/// The `ShiftAngles` inline.
fn shift_angles(r: &mut Running, gobj: GObjId) {
    let v = item_mut(r.game, gobj).xDD4_itemVar.pk_thunder();
    for i in (0..15).rev() {
        let a = v.angles[i].get();
        v.angles[i + 1].set(a);
    }
}

/// The velocity along `angles[0]` at its speed.
fn set_velocity(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    let v = ip.xDD4_itemVar.pk_thunder();
    let (speed, a) = (v.xEEC.get(), v.angles[0].get());
    ip.x40_vel.x = mem(fp::fmuls(reg(speed), reg(math::cos(a))));
    let v = ip.xDD4_itemVar.pk_thunder();
    let (speed, a) = (v.xEEC.get(), v.angles[0].get());
    ip.x40_vel.y = mem(fp::fmuls(reg(speed), reg(math::sin(a))));
    ip.x40_vel.z = 0.0;
}

/// `itNesspkthunderball_UnkMotion0_Coll`: meeting the stage, its hitboxes off and it goes.
#[doc(alias = "itNesspkthunderball_UnkMotion0_Coll")]
pub fn collide(r: &mut Running, gobj: GObjId) -> bool {
    if hit_stage(r, gobj) {
        lib2725::clear_hitboxes(r.game, r.data, gobj);
        return true;
    }
    false
}

/// Reflected, `it_802AC098`: Ness lets go of it, it turns back (and no longer steers), its
/// trails go and start over, its history all its head's.
#[doc(alias = "it_802AC098")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    let owner = ip.owner;
    let v = ip.xDD4_itemVar.pk_thunder();
    if let Some(ness) = v.xF04.get() {
        if owner == Some(ness) {
            r.ness_thunder_check_own(ness, gobj);
        }
        item_mut(r.game, gobj)
            .xDD4_itemVar
            .pk_thunder()
            .xF04
            .set(None);
    }
    let v = item_mut(r.game, gobj).xDD4_itemVar.pk_thunder();
    v.xF00.set(1);
    v.angles[0].update(|a| mem(fp::frsp(fp::fadd(reg(a), std::f64::consts::PI))));
    v.angles[0].update(super::foxlaser::normalize);
    for i in 0..6 {
        let t = item_mut(r.game, gobj).xDD4_itemVar.pk_thunder().xDD4[i].get();
        if let Some(t) = t {
            super::nesspkthundertrail::let_go(r, t);
            item_mut(r.game, gobj).xDD4_itemVar.pk_thunder().xDD4[i].set(None);
        }
    }
    let v = item_mut(r.game, gobj).xDD4_itemVar.pk_thunder();
    v.xEF8.set(0);
    v.xEFC.set(0);
    for i in 1..16 {
        let p = v.positions[0].get();
        v.positions[i].set(p);
        let a = v.angles[0].get();
        v.angles[i].set(a);
    }
    set_velocity(r, gobj);
    false
}

/// Bouncing off a shield, `it_802AC35C`: its speed mirrored and its angle along it.
#[doc(alias = "it_802AC35C")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    let n = ip.xC58;
    ip.x40_vel.mirror_xy(n);
    ip.x40_vel.z = 0.0;
    let a = math::atan2(ip.x40_vel.y, ip.x40_vel.x);
    let v = ip.xDD4_itemVar.pk_thunder();
    v.angles[0].set(a);
    v.angles[0].update(super::foxlaser::normalize);
    false
}

/// Forgets a GObj the ball points to, `it_802AC41C` (`it_8026B894`).
// Ports `it_802AC41C`; aliased once a check reaches it.
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
