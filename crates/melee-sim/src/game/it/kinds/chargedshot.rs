//! Samus's Charge Shot and Mewtwo's Shadow Ball as one charged shot
//! (`melee/it/kinds/itsamuschargeshot.c` and `itmewtwoshadowball.c`, near-copies of each
//! other): a ball in its maker's hand that grows with the charge (state 0) until it's fired,
//! then a shot of the charge's size and speed (states 1 to 8, by charge) that flies until its
//! life runs out or it meets the stage. Kirby's copies run the same code.
//!
//! Each shot's [`ChargedShot`] holds what differs: its variables (the same up to `x2C`, typed
//! apart: [`Vars`]), its maker's queries, its glow when full, how far its charge's state goes,
//! and the Shadow Ball's named branches: it wobbles as it flies (its speed times the wobble's
//! size, its first frame still), keeps its facing when it turns back, and sits back in its
//! maker's hands by his scale. What only one of them does stays its own: the Charge Shot's
//! glow in the hand, the Shadow Ball's wobble, burst and the forward throw's ball.

use gekko_fp as fp;

use super::super::item::{article, change, change_state, pick_up, spawn_air};
use super::super::vars::ItemVars;
use super::super::{ground, item, item_mut, jobj_of, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::ef::{self, Arg};
use crate::game::handles::CodeRef;
use crate::game::state::Running;
use crate::game::structs::{SpawnItem, itMewtwoShadowball_ItemVars, itSamusChargeshot_ItemVars};

use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::math;
use crate::mp::coll::{
    CEILING_HUG, CEILING_PUSH, FLOOR_HUG, FLOOR_PUSH, LEFT_WALL_MASK, RIGHT_WALL_MASK,
};
use crate::vec::Vec3;

/// The attributes both shots have (`itSamusChargeShot_Attributes`,
/// `itMewtwoShadowball_DatAttrs`).
pub mod attr {
    pub const LIFETIME: u32 = 0x0;
    pub const SPEED_MIN: u32 = 0x8;
    pub const SPEED_MAX: u32 = 0xC;
    pub const DAMAGE_MIN: u32 = 0x10;
    pub const DAMAGE_MAX: u32 = 0x14;
    pub const SCALE_MIN: u32 = 0x18;
    pub const SCALE_MAX: u32 = 0x1C;
}

/// `M_PI`, `M_TAU` and `M_PI / 2`.
const PI: f64 = std::f64::consts::PI;
pub const TAU: f64 = std::f64::consts::TAU;
const HALF_PI: f64 = std::f64::consts::FRAC_PI_2;

/// The variables' fields both shots have, at the same places from `x4` to `x2C` (the Charge
/// Shot's `xDD8` to `xE00`, the Shadow Ball's `x4` to `x2C`).
pub trait Vars {
    /// `x4`: the angle it flies at.
    fn angle(&mut self) -> f32;
    fn set_angle(&mut self, v: f32);
    /// `x8`: its speed.
    fn speed(&mut self) -> f32;
    fn set_speed(&mut self, v: f32);
    /// `xC`: cleared as it's fired (the Shadow Ball's thrown ball sets it).
    fn set_xc(&mut self, v: f32);
    /// `x10`: its size as fired.
    fn size(&mut self) -> f32;
    fn set_size(&mut self, v: f32);
    /// `x14`: fired.
    fn fired(&mut self) -> i32;
    fn set_fired(&mut self, v: i32);
    /// `x18` and `x1C`: its charge and the charge's levels.
    fn charge(&mut self) -> i32;
    fn set_charge(&mut self, v: i32);
    fn levels(&mut self) -> i32;
    fn set_levels(&mut self, v: i32);
    /// `x20`: the sparkles' frame count.
    fn sparkle(&mut self) -> i32;
    fn set_sparkle(&mut self, v: i32);
    /// `x24`: its damage.
    fn set_damage(&mut self, v: u32);
    /// `x28`: its effects are out.
    fn set_effects(&mut self, v: i32);
    /// `x2C`: its maker.
    fn maker(&mut self) -> Option<GObjId>;
    fn set_maker(&mut self, v: Option<GObjId>);
}

impl Vars for itSamusChargeshot_ItemVars {
    fn angle(&mut self) -> f32 {
        self.angle.get()
    }
    fn set_angle(&mut self, v: f32) {
        self.angle.set(v);
    }
    fn speed(&mut self) -> f32 {
        self.speed.get()
    }
    fn set_speed(&mut self, v: f32) {
        self.speed.set(v);
    }
    fn set_xc(&mut self, v: f32) {
        self.unread_e0.set(v);
    }
    fn size(&mut self) -> f32 {
        self.size.get()
    }
    fn set_size(&mut self, v: f32) {
        self.size.set(v);
    }
    fn fired(&mut self) -> i32 {
        self.fired.get()
    }
    fn set_fired(&mut self, v: i32) {
        self.fired.set(v);
    }
    fn charge(&mut self) -> i32 {
        self.charge.get()
    }
    fn set_charge(&mut self, v: i32) {
        self.charge.set(v);
    }
    fn levels(&mut self) -> i32 {
        self.levels.get()
    }
    fn set_levels(&mut self, v: i32) {
        self.levels.set(v);
    }
    fn sparkle(&mut self) -> i32 {
        self.sparkle.get()
    }
    fn set_sparkle(&mut self, v: i32) {
        self.sparkle.set(v);
    }
    fn set_damage(&mut self, v: u32) {
        self.unread_damage.set(v as i32);
    }
    fn set_effects(&mut self, v: i32) {
        self.effects.set(v);
    }
    fn maker(&mut self) -> Option<GObjId> {
        self.maker.get()
    }
    fn set_maker(&mut self, v: Option<GObjId>) {
        self.maker.set(v);
    }
}

impl Vars for itMewtwoShadowball_ItemVars {
    fn angle(&mut self) -> f32 {
        self.motion.x()
    }
    fn set_angle(&mut self, v: f32) {
        self.motion.set_x(v);
    }
    fn speed(&mut self) -> f32 {
        self.motion.y()
    }
    fn set_speed(&mut self, v: f32) {
        self.motion.set_y(v);
    }
    fn set_xc(&mut self, v: f32) {
        self.motion.set_z(v);
    }
    fn size(&mut self) -> f32 {
        self.size.get()
    }
    fn set_size(&mut self, v: f32) {
        self.size.set(v);
    }
    fn fired(&mut self) -> i32 {
        self.fired.get()
    }
    fn set_fired(&mut self, v: i32) {
        self.fired.set(v);
    }
    fn charge(&mut self) -> i32 {
        self.charge.get()
    }
    fn set_charge(&mut self, v: i32) {
        self.charge.set(v);
    }
    fn levels(&mut self) -> i32 {
        self.levels.get()
    }
    fn set_levels(&mut self, v: i32) {
        self.levels.set(v);
    }
    fn sparkle(&mut self) -> i32 {
        self.sparkle.get()
    }
    fn set_sparkle(&mut self, v: i32) {
        self.sparkle.set(v);
    }
    fn set_damage(&mut self, v: u32) {
        self.unread_damage.set(v);
    }
    fn set_effects(&mut self, v: i32) {
        self.unread_effects.set(v);
    }
    fn maker(&mut self) -> Option<GObjId> {
        self.maker.get()
    }
    fn set_maker(&mut self, v: Option<GObjId>) {
        self.maker.set(v);
    }
}

/// What the maker of a held shot says (its queries, picked by the shot's kind).
pub enum Hold {
    /// The shot's kind isn't one its maker charges (no query is made).
    Unknown,
    /// The maker lost it (hit, out of the charge's states, putting it away or holding none).
    Lost,
    /// The charge and its levels.
    Charge(i32, i32),
}

/// The charge's state when fired: the charge's levels' (the Shadow Ball), or at most the
/// eighth (the Charge Shot: within 0 and 7).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Top {
    Levels,
    Eighth,
}

/// One shot's settings.
pub struct ChargedShot {
    /// Its variables' shared fields.
    pub vars: fn(&mut ItemVars) -> &mut dyn Vars,
    /// Its maker lets an unfired shot go (`ftSs_SpecialN_801291F0`, `ftMt_SpecialN_SetNULL`),
    /// by the shot's kind.
    pub released: fn(&mut Running, GObjId, i32),
    /// Its maker's queries while it holds the shot.
    pub hold: fn(&mut Running, GObjId, i32) -> Hold,
    /// Its sparkles' address, the fired shot's accessory.
    pub sparkle: u32,
    /// How far the charge's state goes.
    pub top: Top,
    /// Its sparkles when full, by its kind (the Charge Shot's; the Shadow Ball tests and
    /// spawns none).
    pub full_sparkles: fn(i32) -> &'static [i32],
    /// The Shadow Ball's: it wobbles (its speed times the wobble's size, `x50`; it flies from
    /// its second frame, `x4C`), and sits back in its maker's hands by his scale.
    pub wobbles: bool,
    /// The Charge Shot's: it turns to face the way it flies back or bounces off a shield.
    pub turns: bool,
}

/// The shot's shared variables.
pub fn vars<'r>(r: &'r mut Running, gobj: GObjId, s: &ChargedShot) -> &'r mut dyn Vars {
    (s.vars)(&mut item_mut(r.game, gobj).vars)
}

pub fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let attrs = article(item(r.game, gobj))
        .special(r.data)
        .expect("a charged shot without attributes");
    r.data.f32(attrs.at(off))
}

/// The model's grandchild, which the shot scales (`itGetJObjGrandchild`).
pub fn grandchild(r: &Running, gobj: GObjId) -> JObjId {
    let root = jobj_of(r.game, gobj).expect("a charged shot without a model");
    let child = r.game.objs.jobjs[root]
        .child
        .expect("the shot's model's child");
    r.game.objs.jobjs[child]
        .child
        .expect("the shot's model's grandchild")
}

/// From the attributes' `lo` to `hi` by `charge` of `max` (both integers).
pub fn lerp_int(r: &Running, gobj: GObjId, lo: u32, hi: u32, charge: i32, max: i32) -> f64 {
    let lo = reg(attr_f32(r, gobj, lo));
    let hi = reg(attr_f32(r, gobj, hi));
    fp::fmadds(
        fp::frsp(f64::from(charge)),
        fp::fdivs(fp::fsubs(hi, lo), fp::frsp(f64::from(max))),
        lo,
    )
}

/// Whether the shot flew into the stage, `it_802B5518` and `it_802C4F50`: a ceiling rising, a
/// floor falling, the wall it flies toward.
#[doc(alias = "it_802B5518")]
#[doc(alias = "it_802C4F50")]
pub fn hit_stage(r: &mut Running, gobj: GObjId) -> bool {
    ground::air(r, gobj);
    let ip = item(r.game, gobj);
    let env = ip.coll.env_flags;
    let mut hit = false;
    if ip.vel.y > 0.0 {
        if env & (CEILING_PUSH | CEILING_HUG) != 0 {
            hit = true;
        }
    } else if env & (FLOOR_PUSH | FLOOR_HUG) != 0 {
        hit = true;
    }
    if ip.vel.x > 0.0 {
        if env & LEFT_WALL_MASK != 0 {
            hit = true;
        }
    } else if env & RIGHT_WALL_MASK != 0 {
        hit = true;
    }
    hit
}

/// A shot of `kind` made at `pos` by `parent` (`Item_InitSpawnPositionFromParent` and
/// `Item_InitSpawnCommonFields(.., true)`).
pub fn make(r: &mut Running, parent: GObjId, pos: Vec3, kind: i32, facing: f32) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = lib26b1::parent_ecb_center(r.game, parent);
    spawn.facing_dir = facing;
    spawn.damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.owner = Some(parent);
    spawn.maker = spawn.owner;
    spawn.flags[0] |= 0x80;
    spawn.ignore_id = 0;
    spawn_air(r, &mut spawn)
}

/// `Item_ClearCmdVars`.
pub fn clear_cmd_vars(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.cmd_var3 = 0;
    ip.cmd_var2 = 0;
    ip.cmd_var1 = 0;
    ip.cmd_var0 = 0;
}

/// Makes the ball in `parent`'s hand at `pos`, `it_802B55C8` and `it_802C5000`: its lifetime,
/// its own variables (`init`, with its maker), and held at his part `part`.
#[doc(alias = "it_802B55C8")]
#[doc(alias = "it_802C5000")]
pub fn spawn_held(
    r: &mut Running,
    parent: GObjId,
    pos: Vec3,
    part: i32,
    kind: i32,
    facing_dir: f32,
    init: fn(&mut Running, GObjId, GObjId),
) -> Option<GObjId> {
    let shot = make(r, parent, pos, kind, facing_dir)?;
    clear_cmd_vars(r, shot);
    let life = attr_f32(r, shot, attr::LIFETIME);
    lib2725::set_lifetime(r.game, r.data, shot, life);
    init(r, shot, parent);
    pick_up(r, shot, parent, part);
    Some(shot)
}

/// Fires the shot from `pos` at `angle`, `it_802B56E4` and `it_802C53F0`: its charge
/// (`charge` within 0 and `max`) sets its speed, damage and size; while its owner is the one
/// who made it, it leaves the hand ([`fired`], `it_8027429C`), flies its maker's way, and is
/// fired. The Shadow Ball's speed is times its wobble's size, its wobble starts over and its
/// model's child is back to its size.
#[doc(alias = "it_802B56E4")]
#[doc(alias = "it_802C53F0")]
pub fn fire(
    r: &mut Running,
    gobj: GObjId,
    pos: Vec3,
    angle: f32,
    charge: f32,
    max: f32,
    s: &ChargedShot,
) {
    vars(r, gobj, s).set_angle(angle);
    let life = attr_f32(r, gobj, attr::LIFETIME);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    let mut charge = reg(charge);
    if charge < 0.0 {
        charge = 0.0;
    }
    let max = reg(max);
    if charge > max {
        charge = max;
    }
    let v = vars(r, gobj, s);
    v.set_charge(fp::fctiwz(charge));
    v.set_levels(fp::fctiwz(max));
    let owner = item(r.game, gobj).owner;
    let Some(maker) = vars(r, gobj, s).maker() else {
        return;
    };
    if owner != Some(maker) {
        return;
    }
    fired(r, gobj, s);
    // `HSD_MtxGetRotation` of the held part's matrix: its result goes unused.
    super::super::release::let_go(r, gobj, Vec3::new(0.0, 0.0, 0.0));
    item_mut(r.game, gobj).flags.flags.thrown = false;
    lib26b1::disable_pickup(r.game, gobj);
    let lerp = |r: &Running, lo, hi| {
        let (lo, hi) = (reg(attr_f32(r, gobj, lo)), reg(attr_f32(r, gobj, hi)));
        fp::fmadds(charge, fp::fdivs(fp::fsubs(hi, lo), max), lo)
    };
    let speed = lerp(r, attr::SPEED_MIN, attr::SPEED_MAX);
    let damage = lerp(r, attr::DAMAGE_MIN, attr::DAMAGE_MAX);
    let v = vars(r, gobj, s);
    v.set_speed(mem(speed));
    v.set_damage(crate::ppc::cvt_fp2unsigned(damage));
    v.set_xc(0.0);
    let (level, levels) = (v.charge(), v.levels());
    let size = mem(lerp_int(
        r,
        gobj,
        attr::SCALE_MIN,
        attr::SCALE_MAX,
        level,
        levels,
    ));
    let facing = r.game.ft_facing_dir(maker);
    let v = vars(r, gobj, s);
    v.set_size(size);
    v.set_sparkle(0);
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = facing;
    ip.pos = pos;
    vars(r, gobj, s).set_effects(0);
    let wobble = s.wobbles.then(|| shadow_ball(r, gobj).wobble_scale.get());
    let v = vars(r, gobj, s);
    let (speed, angle) = (reg(v.speed()), v.angle());
    let speed = match wobble {
        Some(w) => fp::fmuls(speed, reg(w)),
        None => speed,
    };
    let c = reg(math::cos(angle));
    let sn = reg(math::sin(angle));
    let ip = item_mut(r.game, gobj);
    ip.vel.x = mem(fp::fmuls(speed, c));
    ip.vel.y = mem(fp::fmuls(speed, sn));
    ip.vel.z = 0.0;
    vars(r, gobj, s).set_fired(1);
    if s.wobbles {
        shadow_ball(r, gobj).flight_frames.set(0);
        let root = jobj_of(r.game, gobj).expect("the Shadow Ball without a model");
        let child = r.game.objs.jobjs[root].child;
        let child = child.expect("HSD_JObjSetScale on a null joint");
        jobj::set_scale(&mut r.game.objs.jobjs, child, Vec3::new(1.0, 1.0, 1.0));
    }
}

/// The Shadow Ball's own variables.
fn shadow_ball<'r>(r: &'r mut Running, gobj: GObjId) -> &'r mut itMewtwoShadowball_ItemVars {
    item_mut(r.game, gobj).vars.mewtwo_shadowball()
}

/// The shot's end, `it_2725_Logic108_Destroyed` and `it_2725_Logic101_Destroyed`: its effects
/// gone; unfired, its maker lets it go if it still owns it; then no owner.
#[doc(alias = "it_2725_Logic108_Destroyed")]
#[doc(alias = "it_2725_Logic101_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId, s: &ChargedShot) {
    ef::eflib::destroy_all(r, gobj);
    let ip = item(r.game, gobj);
    let (owner, kind) = (ip.owner, ip.kind);
    let v = vars(r, gobj, s);
    if v.fired() == 0 {
        if let Some(maker) = v.maker()
            && owner == Some(maker)
        {
            (s.released)(r, maker, kind);
        }
        vars(r, gobj, s).set_maker(None);
    }
    let ip = item_mut(r.game, gobj);
    ip.owner = None;
    ip.flags.flags.held = false;
}

/// The ball taken away from the hand, `it_802B5974` and `it_802C573C`: its effects gone, and
/// the item.
#[doc(alias = "it_802B5974")]
#[doc(alias = "it_802C573C")]
pub fn remove(r: &mut Running, gobj: GObjId, s: &ChargedShot) {
    ef::eflib::destroy_all(r, gobj);
    vars(r, gobj, s).set_effects(0);
    super::super::item::destroy(r, gobj);
}

/// `itSamuschargeshot_UnkMotion0_Anim` and `itMewtwoshadowball_UnkMotion0_Anim`: while its
/// maker holds it, the ball goes once he loses it and otherwise takes his charge; it grows with
/// the charge (and the Shadow Ball's grandchild is set back by its first offset times his
/// scale).
#[doc(alias = "itSamuschargeshot_UnkMotion0_Anim")]
#[doc(alias = "itMewtwoshadowball_UnkMotion0_Anim")]
pub fn animate_held(r: &mut Running, gobj: GObjId, s: &ChargedShot) -> bool {
    let j = grandchild(r, gobj);
    let ip = item(r.game, gobj);
    let (owner, kind) = (ip.owner, ip.kind);
    let maker = vars(r, gobj, s).maker();
    if let Some(m) = maker
        && owner == maker
    {
        match (s.hold)(r, m, kind) {
            Hold::Unknown => {}
            Hold::Lost => return true,
            Hold::Charge(charge, max) => {
                let v = vars(r, gobj, s);
                v.set_charge(charge);
                v.set_levels(max);
            }
        }
    }
    let v = vars(r, gobj, s);
    let (charge, max) = (v.charge(), v.levels());
    let sc = mem(lerp_int(
        r,
        gobj,
        attr::SCALE_MIN,
        attr::SCALE_MAX,
        charge,
        max,
    ));
    jobj::set_scale(&mut r.game.objs.jobjs, j, Vec3::new(sc, sc, sc));
    if s.wobbles {
        let v = shadow_ball(r, gobj);
        if v.held_offset.x() == 0.0 && v.held_offset.y() == 0.0 && v.held_offset.z() == 0.0 {
            let t = r.game.objs.jobjs[j].translate;
            shadow_ball(r, gobj).held_offset.set(t);
        }
        let v = shadow_ball(r, gobj);
        let k = reg(v.held_scale.get());
        let o = v.held_offset.get();
        let t = Vec3::new(
            mem(fp::fmuls(fp::fneg(reg(o.x)), k)),
            mem(fp::fmuls(fp::fneg(reg(o.y)), k)),
            mem(fp::fmuls(fp::fneg(reg(o.z)), k)),
        );
        jobj::set_translate(&mut r.game.objs.jobjs, j, t);
    }
    false
}

/// Fired, `it_802B5CBC` and `it_802C5B18`: its lifetime, its effects gone, the state for its
/// charge (1 on), the sparkles hooked (`sparkle`, its own address); the Shadow Ball's
/// wobble's size by the charge (half to whole).
#[doc(alias = "it_802B5CBC")]
#[doc(alias = "it_802C5B18")]
pub fn fired(r: &mut Running, gobj: GObjId, s: &ChargedShot) {
    let life = attr_f32(r, gobj, attr::LIFETIME);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    ef::eflib::destroy_all(r, gobj);
    let level = clamp_charge(r, gobj, s);
    change_state(r, gobj, level.wrapping_add(1), change::ANIM_UPDATE);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(s.sparkle));
    if s.wobbles {
        let v = shadow_ball(r, gobj);
        let part = fp::fdivs(
            fp::frsp(f64::from(v.charge.get())),
            fp::frsp(f64::from(v.levels.get())),
        );
        v.wobble_scale.set(mem(fp::fmadds(0.5, part, 0.5)));
    }
}

/// No effects out, and the charge kept within 0 and its top ([`Top`]); the charge.
pub fn clamp_charge(r: &mut Running, gobj: GObjId, s: &ChargedShot) -> i32 {
    let v = vars(r, gobj, s);
    v.set_effects(0);
    if v.charge() < 0 {
        v.set_charge(0);
    }
    match s.top {
        Top::Eighth => {
            if v.charge() >= 8 {
                v.set_charge(7);
            }
        }
        Top::Levels => {
            if v.charge() >= v.levels() {
                let m = v.levels();
                v.set_charge(m);
            }
        }
    }
    v.charge()
}

/// `itSamuschargeshot_UnkMotion8_Anim` and `itMewtwoshadowball_UnkMotion8_Anim`: its size, and
/// its life counts down.
#[doc(alias = "itSamuschargeshot_UnkMotion8_Anim")]
#[doc(alias = "itMewtwoshadowball_UnkMotion8_Anim")]
pub fn animate_flying(r: &mut Running, gobj: GObjId, s: &ChargedShot) -> bool {
    let j = grandchild(r, gobj);
    let sc = vars(r, gobj, s).size();
    jobj::set_scale(&mut r.game.objs.jobjs, j, Vec3::new(sc, sc, sc));
    lib2725::tick_lifetime(r.game, gobj)
}

/// Its speed along its angle, `itSamuschargeshot_UnkMotion8_Phys` (and the Shadow Ball's
/// after its first frame).
#[doc(alias = "itSamuschargeshot_UnkMotion8_Phys")]
pub fn fly(r: &mut Running, gobj: GObjId, s: &ChargedShot) {
    let v = vars(r, gobj, s);
    let (speed, angle) = (reg(v.speed()), v.angle());
    let c = reg(math::cos(angle));
    item_mut(r.game, gobj).vel.x = mem(fp::fmuls(speed, c));
    let v = vars(r, gobj, s);
    let (speed, angle) = (reg(v.speed()), v.angle());
    let sn = reg(math::sin(angle));
    item_mut(r.game, gobj).vel.y = mem(fp::fmuls(speed, sn));
}

/// A fired shot's sparkles (its accessory), `it_802B5EDC` and `fn_802C5E18`: when full, its
/// [`ChargedShot::full_sparkles`], and a count cycling every third frame.
#[doc(alias = "it_802B5EDC")]
#[doc(alias = "fn_802C5E18")]
pub fn sparkle(r: &mut Running, gobj: GObjId, s: &ChargedShot) {
    let j = jobj_of(r.game, gobj);
    let kind = item(r.game, gobj).kind;
    let v = vars(r, gobj, s);
    let full = v.charge() == v.levels();
    if full && v.sparkle() == 0 {
        for &gfx in (s.full_sparkles)(kind) {
            ef::spawn(r, gfx, Some(gobj), &[Arg::JObj(j)]);
        }
    }
    let v = vars(r, gobj, s);
    let n = v.sparkle().wrapping_add(1);
    v.set_sparkle(n);
    let n = v.sparkle() % 3;
    v.set_sparkle(n);
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

/// Turned to face its way: its model's yaw.
fn face(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj).expect("the Charge Shot without a model");
    let yaw = mem(fp::frsp(fp::fmul(
        HALF_PI,
        reg(item(r.game, gobj).facing_dir),
    )));
    jobj::set_rotation_y(&mut r.game.objs.jobjs, j, yaw);
}

/// Reflected, `it_2725_Logic108_Reflected` and `it_2725_Logic101_Reflected`: it flies back,
/// the Charge Shot turned round.
#[doc(alias = "it_2725_Logic108_Reflected")]
#[doc(alias = "it_2725_Logic101_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId, s: &ChargedShot) -> bool {
    if s.turns {
        let ip = item_mut(r.game, gobj);
        ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
        face(r, gobj);
    }
    let v = vars(r, gobj, s);
    let a = mem(fp::frsp(fp::fadd(reg(v.angle()), PI)));
    v.set_angle(wrap(a));
    false
}

/// Bouncing off a shield, `it_2725_Logic108_ShieldBounced` and `it_2725_Logic101_ShieldBounced`:
/// its speed mirrored off the shield, its angle (and the Charge Shot's facing) from the new
/// speed.
#[doc(alias = "it_2725_Logic108_ShieldBounced")]
#[doc(alias = "it_2725_Logic101_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId, s: &ChargedShot) -> bool {
    let ip = item_mut(r.game, gobj);
    let normal = ip.shield_normal;
    ip.vel.mirror_xy(normal);
    let a = math::atan2(ip.vel.y, ip.vel.x);
    vars(r, gobj, s).set_angle(wrap(a));
    if s.turns {
        let ip = item_mut(r.game, gobj);
        ip.facing_dir = if ip.vel.x >= 0.0 { 1.0 } else { -1.0 };
        face(r, gobj);
    }
    false
}

/// A hit dealt, clanked, absorbed or on a shield ends the shot: the Charge Shot's
/// `itSamusChargeshot_Logic108_DmgDealt` and the others, the Shadow Ball's
/// `itMewtwoShadowball_Logic101_DmgDealt` and the others.
#[doc(alias = "itMewtwoShadowball_Logic101_DmgDealt")]
#[doc(alias = "itMewtwoShadowball_Logic101_Clanked")]
#[doc(alias = "itMewtwoShadowball_Logic101_Absorbed")]
#[doc(alias = "itMewtwoShadowball_Logic101_HitShield")]
pub fn hit_ends(_r: &mut Running, _gobj: GObjId) -> bool {
    true
}

/// Forgets a GObj the shot points to, `itMewtwoShadowball_Logic101_EvtUnk` (and the Charge
/// Shot's `itSamusChargeshot_Logic108_EvtUnk`) (`it_8026B894`).
#[doc(alias = "itMewtwoShadowball_Logic101_EvtUnk")]
#[doc(alias("evt_unk"))]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
