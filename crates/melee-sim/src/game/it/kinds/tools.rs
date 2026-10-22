//! Flat Zone's falling tools, `melee/it/kinds/ittools.c` (`It_Kind_Tools`): the hammers,
//! wrenches and the rest that the stage's helmet event drops from the top of the screen
//! (`gr::flatzone`). A tool falls spinning in its own way (states 0 to 4, by its tool) until it
//! hits the ground, a fighter, a shield or another hitbox, then lies there blinking until it
//! goes (states 5 to 9). A fighter's hit knocks a falling tool back up and away, harder the
//! more it has taken.

use gekko_fp as fp;

use super::super::Var;
use super::super::ground::air_touches;
use super::super::item::{article, change, change_state, is_fighter};
use super::super::{coll, hitbox, item, item_mut, jobj_of, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::mirror::{Cx, Mem, Mirror};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

use super::TOOLS;

pub const DMG_DEALT: u32 = 0x802E_F074;
pub const ANIM_FALL: u32 = 0x802E_F128;
pub const PHYS_FALL: u32 = 0x802E_F27C;
pub const COLL_FALL: u32 = 0x802E_F2BC;
pub const ANIM_LIE: u32 = 0x802E_F3C4;
pub const PHYS_LIE: u32 = 0x802E_F468;
pub const COLL_LIE: u32 = 0x802E_F46C;
pub const DMG_RECEIVED: u32 = 0x802E_F700;
pub const CLANKED: u32 = 0x802E_F724;
pub const HIT_SHIELD: u32 = 0x802E_F7CC;
pub const ABSORBED: u32 = 0x802E_F874;
pub const SHIELD_BOUNCED: u32 = 0x802E_F924;
pub const REFLECTED: u32 = 0x802E_F944;

/// The first state of the lying ones: a tool's lying state is its falling one plus this.
const LIE: i32 = 5;

/// `itToolsAttributes`' fields, by offset: the life a tool falls with (`x0`), the life it lies
/// with (`x4`), a hit's damage's weight (`x8`) and the damage's cap (`xC`, a word).
mod a {
    pub const FALL_LIFE: u32 = 0;
    pub const LIE_LIFE: u32 = 4;
    pub const DAMAGE_SCALE: u32 = 8;
    pub const DAMAGE_MAX: u32 = 0xC;
    /// The tools' own attributes (`itToolsMotionAttrs`, 0x1C bytes each) from here.
    pub const MOTIONS: u32 = 0x10;
}

/// `itToolsMotionAttrs`' fields, by offset: the fall's starting speed (`x0`), its gravity and
/// top speed (`x4`, `x8`), the spin a tick (`xC`), a fighter's hit's upward speed (`x10`), the
/// sideways speed past which hits don't count (`x14`) and the hit's sideways speed's weight
/// (`x18`).
mod m {
    pub const START_SPEED: u32 = 0;
    pub const GRAVITY: u32 = 4;
    pub const MAX_SPEED: u32 = 8;
    pub const SPIN: u32 = 0xC;
    pub const HIT_RISE: u32 = 0x10;
    pub const HIT_MAX_SPEED: u32 = 0x14;
    pub const HIT_SPEED: u32 = 0x18;
}

/// Runs one of the tools' predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM_FALL => anim_fall(r, gobj),
        COLL_FALL => coll_fall(r, gobj),
        ANIM_LIE => anim_lie(r, gobj),
        COLL_LIE => coll_lie(r, gobj),
        DMG_DEALT => dealt(r, gobj),
        DMG_RECEIVED => received(r, gobj),
        CLANKED => clanked(r, gobj),
        HIT_SHIELD => hit_shield(r, gobj),
        ABSORBED => absorbed(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        REFLECTED => reflected(r, gobj),
        _ => return None,
    })
}

/// Runs one of the tools' events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PHYS_FALL => phys_fall(r, gobj),
        PHYS_LIE => phys_lie(r, gobj),
        _ => return false,
    }
    true
}

/// `itTools_ItemVars`: which tool it is (`x0`, 0 to 4), its states' and attributes' index.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct ToolsVars {
    pub x0: Var<i32>,
}

impl ToolsVars {
    /// Marks every field stale, as a new item finds its variables.
    pub fn mark_stale(&mut self) {
        self.x0.mark_stale();
    }
}

impl Mirror for ToolsVars {
    const SIZE: u32 = 4;

    fn import(m: &Mem, at: u32, _: &mut dyn Cx) -> Self {
        ToolsVars {
            x0: Var::new(m.u32(at) as i32),
        }
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        let p = format!("{path}.x0");
        if self.x0.live(&p, cx) && *self.x0.peek() != m.u32(at) as i32 {
            cx.mismatch(
                &p,
                format!("{}", self.x0.peek()),
                format!("{}", m.u32(at) as i32),
            );
        }
    }
}

/// Which tool it is.
fn tool(r: &mut Running, gobj: GObjId) -> i32 {
    item_mut(r.game, gobj).xDD4_itemVar.tools().x0.get()
}

/// The word at `off` of the tools' attributes.
fn attr(r: &Running, gobj: GObjId, off: u32) -> u32 {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a tool without attributes");
    r.data.u32(a.at(off))
}

fn attr_f(r: &Running, gobj: GObjId, off: u32) -> f32 {
    f32::from_bits(attr(r, gobj, off))
}

/// The float at `off` of this tool's own attributes (`motions[x0]`).
fn motion(r: &mut Running, gobj: GObjId, off: u32) -> f32 {
    let t = tool(r, gobj);
    let at = a::MOTIONS.wrapping_add((t as u32).wrapping_mul(0x1C));
    attr_f(r, gobj, at + off)
}

/// A tool of kind `which` falling from `pos`, `it_802EEFA8`.
#[doc(alias = "it_802EEFA8")]
pub fn spawn(r: &mut Running, pos: Vec3, which: i32, facing_dir: f32) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind: TOOLS,
        prev_pos: pos,
        pos,
        facing_dir,
        ..Default::default()
    };
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = None;
    spawn.x4_parent_gobj2 = spawn.x0_parent_gobj;
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let gobj = super::super::item::spawn_air(r, &mut spawn)?;
    start(r, gobj, which);
    Some(gobj)
}

/// Starts falling as tool `which`, `it_802EF098`: the attributes' life, the tool's starting
/// speed straight down, out of reach, in the tool's falling state.
#[doc(alias = "it_802EF098")]
pub fn start(r: &mut Running, gobj: GObjId, which: i32) {
    item_mut(r.game, gobj).xDD4_itemVar.tools().x0.set(which);
    let life = attr_f(r, gobj, a::FALL_LIFE);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    let speed = motion(r, gobj, m::START_SPEED);
    let ip = item_mut(r.game, gobj);
    ip.xD5C = 0;
    ip.x40_vel.x = 0.0;
    ip.x40_vel.y = mem(fp::fneg(reg(speed)));
    lib26b1::disable_pickup(r.game, gobj);
    let t = tool(r, gobj);
    change_state(r, gobj, t, change::ANIM_UPDATE);
}

/// A hit it dealt lays it down, `itTools_Logic22_DmgDealt`.
#[doc(alias = "itTools_Logic22_DmgDealt")]
pub fn dealt(r: &mut Running, gobj: GObjId) -> bool {
    lie(r, gobj);
    false
}

/// Falling, `itTools_UnkMotion4_Anim` (states 0 to 4): it spins its way, and lies down when
/// its life runs out.
#[doc(alias = "itTools_UnkMotion4_Anim")]
pub fn anim_fall(r: &mut Running, gobj: GObjId) -> bool {
    let j = jobj_of(r.game, gobj).expect("jobj.h: jobj");
    let spin = motion(r, gobj, m::SPIN);
    let rz = reg(r.game.objs.jobjs[j].rotate.z);
    let rz = fp::fmadds(reg(spin), reg(item(r.game, gobj).facing_dir), rz);
    jobj::set_rotation_z(&mut r.game.objs.jobjs, j, mem(rz));
    let ip = item_mut(r.game, gobj);
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    if reg(ip.xD44_lifeTimer) <= 0.0 {
        ip.xD44_lifeTimer = 0.0;
        lie(r, gobj);
    }
    false
}

/// Falling, `itTools_UnkMotion4_Phys`: the tool's gravity and top speed.
#[doc(alias = "itTools_UnkMotion4_Phys")]
pub fn phys_fall(r: &mut Running, gobj: GObjId) {
    let accel = motion(r, gobj, m::GRAVITY);
    let max = motion(r, gobj, m::MAX_SPEED);
    lib2725::fall(r.game, gobj, accel, max);
}

/// Falling, `itTools_UnkMotion4_Coll`: moving, it lies down on a floor or a ceiling it meets.
#[doc(alias = "itTools_UnkMotion4_Coll")]
pub fn coll_fall(r: &mut Running, gobj: GObjId) -> bool {
    let v = item(r.game, gobj).x40_vel;
    if (reg(v.x) != 0.0 || reg(v.y) != 0.0) && air_touches(r, gobj) & 3 != 0 {
        lie(r, gobj);
    }
    false
}

/// Lies down where it is, `it_802EF320`: blinking, out of reach, stopped, with the
/// attributes' lying life (an even number of ticks), in the tool's lying state.
#[doc(alias = "it_802EF320")]
pub fn lie(r: &mut Running, gobj: GObjId) {
    hitbox::start_blink(r.game, gobj);
    lib26b1::disable_pickup(r.game, gobj);
    let life = attr_f(r, gobj, a::LIE_LIFE);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = 0.0;
    ip.x40_vel.x = 0.0;
    ip.xD44_lifeTimer = mem(fp::frsp(f64::from(fp::fctiwz(reg(life)) & 0xFFFE)));
    let t = tool(r, gobj);
    change_state(r, gobj, t + LIE, change::ANIM_UPDATE);
}

/// Lying, `itTools_UnkMotion9_Anim` (states 5 to 9): it blinks, two ticks shown and two
/// hidden (posed at its state's first frame when shown), and goes when its life runs out.
#[doc(alias = "itTools_UnkMotion9_Anim")]
pub fn anim_lie(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    if reg(ip.xD44_lifeTimer) <= 0.0 {
        ip.xD44_lifeTimer = 0.0;
        return true;
    }
    if fp::fctiwz(reg(ip.xD44_lifeTimer)) & 2 != 0 {
        lib26b1::show(r.game, gobj);
        let t = tool(r, gobj);
        super::drmariopill::set_pose(r, gobj, t + LIE, 0.0);
    } else {
        lib26b1::hide(r.game, gobj);
    }
    false
}

/// Lying, `itTools_UnkMotion9_Phys`: nothing moves it.
#[doc(alias = "itTools_UnkMotion9_Phys")]
pub fn phys_lie(_: &mut Running, _: GObjId) {}

/// Lying, `itTools_UnkMotion9_Coll`: moving, it lies down again on what it meets.
#[doc(alias = "itTools_UnkMotion9_Coll")]
pub fn coll_lie(r: &mut Running, gobj: GObjId) -> bool {
    let v = item(r.game, gobj).x40_vel;
    if (reg(v.x) != 0.0 || reg(v.y) != 0.0) && air_touches(r, gobj) & 3 != 0 {
        lie(r, gobj);
    }
    false
}

/// A hit knocks it, `it_802EF548`, unless it already flies sideways faster than the tool's
/// limit: its life back to half, its hitbox's damage up by the hit's weighted damage (capped),
/// and a speed up and away from the fighter that hit it (from its camera bone), or against
/// the hit's direction without one, the faster the more damage its hitbox now deals.
#[doc(alias = "it_802EF548")]
pub fn knock(r: &mut Running, gobj: GObjId) {
    let limit = motion(r, gobj, m::HIT_MAX_SPEED);
    let vx = reg(item(r.game, gobj).x40_vel.x);
    let abs = if vx < 0.0 { fp::fneg(vx) } else { vx };
    if abs > reg(limit) {
        return;
    }
    let ip = item_mut(r.game, gobj);
    ip.xD44_lifeTimer = ip.xD48_halfLifeTimer;
    let fighter = ip.xCEC_fighterGObj;
    let mut damage = fp::frsp(f64::from(ip.xCA0));
    let mut dir = fp::fmuls(fp::fneg(1.0), reg(ip.xCCC_incDamageDirection));
    if let Some(f) = fighter {
        if !is_fighter(r.game, Some(f)) {
            return;
        }
        let fid = r.game.fighter_id(f);
        let pos = r.camera_bone_pos(fid);
        dir = if reg(item(r.game, gobj).pos.x) > reg(pos.x) {
            1.0
        } else {
            fp::fneg(1.0)
        };
    }
    let scale = attr_f(r, gobj, a::DAMAGE_SCALE);
    let max = fp::frsp(f64::from(attr(r, gobj, a::DAMAGE_MAX)));
    damage = fp::fmuls(damage, reg(scale));
    damage = fp::fadds(damage, reg(item(r.game, gobj).x5D4_hitboxes[0].hit.damage));
    if damage > max {
        damage = max;
    }
    coll::set_hit_damage(r.game, r.data, gobj, 0, crate::ppc::cvt_fp2unsigned(damage));
    let speed = motion(r, gobj, m::HIT_SPEED);
    let rise = motion(r, gobj, m::HIT_RISE);
    let ip = item_mut(r.game, gobj);
    let hit = reg(ip.x5D4_hitboxes[0].hit.damage);
    ip.x40_vel.x = mem(fp::fmuls(
        dir,
        fp::fmuls(fp::fadds(1.0, fp::fsubs(hit, 10.0)), reg(speed)),
    ));
    ip.x40_vel.y = rise;
}

/// A hit it took knocks it, `itTools_Logic22_DmgReceived`.
#[doc(alias = "itTools_Logic22_DmgReceived")]
pub fn received(r: &mut Running, gobj: GObjId) -> bool {
    knock(r, gobj);
    false
}

/// Clanked, it lies down, `it_2725_Logic22_Clanked`.
#[doc(alias = "it_2725_Logic22_Clanked")]
pub fn clanked(r: &mut Running, gobj: GObjId) -> bool {
    lie(r, gobj);
    false
}

/// Hitting a shield, it lies down, `it_2725_Logic22_HitShield`.
#[doc(alias = "it_2725_Logic22_HitShield")]
pub fn hit_shield(r: &mut Running, gobj: GObjId) -> bool {
    lie(r, gobj);
    false
}

/// Absorbed, it lies down with no life left, `it_2725_Logic22_Absorbed`.
#[doc(alias = "it_2725_Logic22_Absorbed")]
pub fn absorbed(r: &mut Running, gobj: GObjId) -> bool {
    lie(r, gobj);
    item_mut(r.game, gobj).xD44_lifeTimer = 0.0;
    false
}

/// Bounced off a shield, `itTools_Logic22_ShieldBounced`.
#[doc(alias = "itTools_Logic22_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_shield(r.game, gobj)
}

/// Reflected, `itTools_Logic22_Reflected` (`itReflectItemAndUpdateRotation`): sent back,
/// turned to face its way.
#[doc(alias = "itTools_Logic22_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj);
    let half_pi = fp::frsp(fp::fdiv(std::f64::consts::PI, 2.0));
    let y = mem(fp::fmuls(half_pi, reg(item(r.game, gobj).facing_dir)));
    let j = jobj_of(r.game, gobj).expect("jobj.h: jobj");
    lib2725::set_rotation_y(r.game, j, y);
    false
}

/// Forgets a GObj the tool points to, `it_802EFA24` (`it_8026B894`).
#[doc(alias = "it_802EFA24")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
