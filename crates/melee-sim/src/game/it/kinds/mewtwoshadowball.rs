//! Mewtwo's Shadow Ball, `melee/it/kinds/itmewtwoshadowball.c`: a ball in his hands that
//! grows with his charge (state 0) until he fires it, then a ball of the charge's size and
//! speed (states 1 to 8, by charge) that wobbles as it flies (`it_802C4D10`) until its life runs
//! out or it meets the stage, where it bursts (states 10 to 17). His forward throw fires a
//! full one at once (state 9). The Charge Shot's code ([`super::chargedshot`]) with its own
//! wobble, burst and thrown ball.
//!
//! Kirby's copy (`It_Kind_Kirby_MewtwoShadowBall`) runs the same code with his own checks
//! (`ft::kirby::special_mewtwo`).

use crate::lb::hitcapsule::HitCapsuleState;
use gekko_fp as fp;

use super::super::item::{change, change_state, play_sfx, play_sfx1};
use super::super::vars::ItemVars;
use super::super::{item_mut, jobj_of, lib26b1, lib2725};
use super::chargedshot::{self as cs, ChargedShot, Hold, TAU, Top, Vars, attr_f32, lerp_int};
use crate::fpu::{mem, reg};
use crate::game::ef::{self, Arg};
use crate::game::ft::kirby;
use crate::game::ft::mewtwo::special_n::{self, ShadowBall};
use crate::game::it::ItKind;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::math;
use crate::vec::Vec3;

pub const WOBBLE: u32 = 0x802C_4D10;
pub const HIT_STAGE: u32 = 0x802C_4F50;
pub const SPAWN_HELD: u32 = 0x802C_5000;
pub const SPAWN_FIRED: u32 = 0x802C_519C;
pub const FIRE: u32 = 0x802C_53F0;
pub const DESTROYED: u32 = 0x802C_56A0;
pub const REMOVE: u32 = 0x802C_573C;
pub const PICKED_UP: u32 = 0x802C_578C;
pub const ANIM_0: u32 = 0x802C_57C8;
pub const PHYS_0: u32 = 0x802C_5B0C;
pub const COLL_0: u32 = 0x802C_5B10;
pub const FIRED: u32 = 0x802C_5B18;
pub const ANIM_8: u32 = 0x802C_5BF8;
pub const PHYS_8: u32 = 0x802C_5CF4;
pub const COLL_8: u32 = 0x802C_5D64;
pub const ANIM_9: u32 = 0x802C_5DAC;
pub const PHYS_9: u32 = 0x802C_5DCC;
pub const COLL_9: u32 = 0x802C_5DD0;
pub const SPARKLE: u32 = 0x802C_5E18;
pub const BURST: u32 = 0x802C_5E5C;
pub const ANIM_17: u32 = 0x802C_5F7C;
pub const PHYS_17: u32 = 0x802C_6094;
pub const COLL_17: u32 = 0x802C_60AC;
pub const DMG_DEALT: u32 = 0x802C_60B4;
pub const CLANKED: u32 = 0x802C_60BC;
pub const ABSORBED: u32 = 0x802C_60C4;
pub const REFLECTED: u32 = 0x802C_60CC;
pub const HIT_SHIELD: u32 = 0x802C_6138;
pub const SHIELD_BOUNCED: u32 = 0x802C_6140;
#[doc(alias("EVT_UNK"))]
pub const FORGET: u32 = 0x802C_61D4;

/// `itMewtwoShadowball_DatAttrs` past the Charge Shot's ([`cs::attr`]).
mod attr {
    /// The wobble's period (an `int`).
    pub const WOBBLE_FRAMES: u32 = 0x20;
    /// The burst's hitbox scale and life.
    pub const BURST_SCALE: u32 = 0x24;
    pub const BURST_LIFETIME: u32 = 0x28;
    /// The thrown ball's speed.
    pub const THROWN_SPEED: u32 = 0x2C;
}

/// The wobble's sounds.
const SFX_WOBBLE: [i32; 3] = [0x30DAA, 0x30DAD, 0x30DB0];
/// The burst's effect and sound.
const GFX_BURST: i32 = 0x40E;
const SFX_BURST: i32 = 0x74;
/// `@218` (`M_PI / 4`).
const QUARTER_PI: f64 = std::f64::consts::FRAC_PI_4;

/// The Shadow Ball: its maker's queries, no sparkles, its state up to the charge's levels, and
/// it wobbles, keeping its facing.
pub const SHADOW_BALL: ChargedShot = ChargedShot {
    vars: shared,
    released,
    hold,
    sparkle: SPARKLE,
    top: Top::Levels,
    full_sparkles: |_| &[],
    wobbles: true,
    turns: false,
};

/// Runs one of the ball's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM_0 => cs::animate_held(r, gobj, &SHADOW_BALL),
        COLL_0 | COLL_17 => no_coll(r, gobj),
        ANIM_8 => cs::animate_flying(r, gobj, &SHADOW_BALL),
        COLL_8 | COLL_9 => coll_flying(r, gobj),
        ANIM_9 => animate_thrown(r, gobj),
        ANIM_17 => animate_burst(r, gobj),
        DMG_DEALT | CLANKED | ABSORBED | HIT_SHIELD => cs::hit_ends(r, gobj),
        REFLECTED => cs::reflected(r, gobj, &SHADOW_BALL),
        SHIELD_BOUNCED => cs::shield_bounced(r, gobj, &SHADOW_BALL),
        _ => return None,
    })
}

/// Runs one of the ball's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => cs::destroyed(r, gobj, &SHADOW_BALL),
        PICKED_UP => picked_up(r, gobj),
        PHYS_0 | PHYS_9 => no_physics(r, gobj),
        PHYS_8 => physics_flying(r, gobj),
        PHYS_17 => physics_burst(r, gobj),
        SPARKLE => cs::sparkle(r, gobj, &SHADOW_BALL),
        _ => return false,
    }
    true
}

/// The fields the Charge Shot has too.
fn shared(v: &mut ItemVars) -> &mut dyn Vars {
    v.mewtwo_shadowball()
}

fn attr_i32(r: &Running, gobj: GObjId, off: u32) -> i32 {
    attr_f32(r, gobj, off).to_bits() as i32
}

/// Whose Shadow Ball a ball of `kind` is: Mewtwo's or Kirby's copy's (the original's queries
/// of its maker are picked by the ball's kind).
fn maker_ball(kind: i32) -> Option<&'static ShadowBall> {
    match ItKind::of(kind) {
        ItKind::MewtwoShadowBall => Some(&special_n::MEWTWO),
        ItKind::KirbyMewtwoShadowBall => Some(&kirby::special_mewtwo::KIRBY),
        _ => None,
    }
}

/// Its maker lets it go, `ftMt_SpecialN_SetNULL`.
fn released(r: &mut Running, maker: GObjId, kind: i32) {
    if let Some(b) = maker_ball(kind) {
        r.mt_n_ball_released(maker, b);
    }
}

/// Its maker's charge, unless he lost it (hit, out of the charge's states, putting it away or
/// holding none).
fn hold(r: &mut Running, maker: GObjId, kind: i32) -> Hold {
    let Some(b) = maker_ball(kind) else {
        return Hold::Unknown;
    };
    if r.mt_n_ball_lost(Some(maker), b) || r.mt_n_ball_put_away(Some(maker), b) {
        return Hold::Lost;
    }
    match r.mt_n_charge_of(Some(maker), b) {
        Some((charge, max)) => Hold::Charge(charge, max),
        None => Hold::Lost,
    }
}

/// The ball's wobble as it flies, `it_802C4D10`: every `x20` frames (after its first) a turn
/// of up to an eighth either way and a new swing (`x44` undoing `x40` over half the period)
/// with one of three sounds; the model's grandchild is moved off the ball's path by the swing.
/// (Transliterated by ssbm-rs from its machine code: the return value is a register its C
/// never sets, which no caller reads.)
#[doc(alias = "it_802C4D10")]
pub fn wobble(r: &mut Running, gobj: GObjId) {
    let j = cs::grandchild(r, gobj);
    let period = attr_i32(r, gobj, attr::WOBBLE_FRAMES);
    let v = item_mut(r.game, gobj).vars.mewtwo_shadowball();
    if v.flight_frames.get() > 0 {
        let n = v.wobble_frame.get().wrapping_add(1);
        v.wobble_frame.set(n);
        if v.wobble_frame.get() >= period {
            v.wobble_frame.set(0);
            let rand = reg(r.game.objs.rng.draw_f32());
            let v = item_mut(r.game, gobj).vars.mewtwo_shadowball();
            let turn = fp::fmuls(2.0, fp::fsubs(rand, 0.5));
            let a = fp::frsp(fp::fmadd(QUARTER_PI, turn, reg(v.wobble_angle.get())));
            v.wobble_angle.set(mem(a));
            let half = fp::fmuls(0.5, fp::frsp(f64::from(period)));
            let swing = fp::fdivs(fp::fneg(reg(v.wobble_size.get())), half);
            v.wobble_step.set(mem(swing));
            let i = r.game.objs.rng.below(3);
            play_sfx1(r, gobj, SFX_WOBBLE[i as usize], 127, 64);
        }
        let v = item_mut(r.game, gobj).vars.mewtwo_shadowball();
        let wobble = fp::fadds(reg(v.wobble_size.get()), reg(v.wobble_step.get()));
        v.wobble_size.set(mem(wobble));
        v.wobble_offset.set_x(0.0);
        let s = reg(math::sin(v.wobble_angle.get()));
        let v = item_mut(r.game, gobj).vars.mewtwo_shadowball();
        let amp = fp::fmuls(reg(v.wobble_size.get()), reg(v.wobble_scale.get()));
        let y = fp::fmadds(amp, s, reg(v.wobble_offset.y()));
        v.wobble_offset.set_y(mem(y));
        let c = reg(math::cos(v.wobble_angle.get()));
        let v = item_mut(r.game, gobj).vars.mewtwo_shadowball();
        let amp = fp::fmuls(reg(v.wobble_size.get()), reg(v.wobble_scale.get()));
        let z = fp::fmadds(amp, c, reg(v.wobble_offset.z()));
        v.wobble_offset.set_z(mem(z));
    } else {
        v.wobble_offset.set(Vec3::new(0.0, 0.0, 0.0));
    }
    let v = item_mut(r.game, gobj).vars.mewtwo_shadowball();
    let t = v.wobble_offset.get();
    jobj::set_translate(&mut r.game.objs.jobjs, j, t);
    let v = item_mut(r.game, gobj).vars.mewtwo_shadowball();
    let n = v.flight_frames.get().wrapping_add(1);
    v.flight_frames.set(n);
}

/// Makes the ball in `parent`'s hands at `pos` ([`cs::spawn_held`]): not fired, a random first
/// wobble, his model's scale kept.
pub fn spawn_held(
    r: &mut Running,
    parent: GObjId,
    pos: Vec3,
    part: i32,
    kind: i32,
    facing_dir: f32,
) -> Option<GObjId> {
    cs::spawn_held(r, parent, pos, part, kind, facing_dir, init)
}

/// Its variables as it's made in `parent`'s hands.
fn init(r: &mut Running, b: GObjId, parent: GObjId) {
    let period = attr_i32(r, b, attr::WOBBLE_FRAMES);
    let v = item_mut(r.game, b).vars.mewtwo_shadowball();
    v.fired.set(0);
    v.motion.set_z(0.0);
    v.unread_effects.set(0);
    v.maker.set(Some(parent));
    v.wobble_offset.set(Vec3::new(0.0, 0.0, 0.0));
    v.wobble_frame.set(period);
    let rand = reg(r.game.objs.rng.draw_f32());
    let v = item_mut(r.game, b).vars.mewtwo_shadowball();
    v.wobble_angle.set(mem(fp::frsp(fp::fmul(TAU, rand))));
    let rand = reg(r.game.objs.rng.draw_f32());
    let v = item_mut(r.game, b).vars.mewtwo_shadowball();
    v.wobble_size.set(mem(fp::fadds(2.5, fp::fsubs(rand, 0.5))));
    let half = fp::fmuls(0.5, fp::frsp(f64::from(v.wobble_frame.get())));
    let swing = fp::fdivs(fp::fneg(reg(v.wobble_size.get())), half);
    v.wobble_step.set(mem(swing));
    v.flight_frames.set(0);
    v.wobble_scale.set(0.0);
    let scale = mem(Running::model_scale(r.game.fighter(parent)));
    let v = item_mut(r.game, b).vars.mewtwo_shadowball();
    v.held_scale.set(scale);
    v.held_offset.set(Vec3::new(0.0, 0.0, 0.0));
}

/// The full ball his forward throw fires from `pos` at `angle`, `it_802C519C`: already fired
/// (state 9), at the attributes' thrown speed (`x2C`), its damage and size for no charge of
/// `max`.
#[doc(alias = "it_802C519C")]
pub fn spawn_fired(
    r: &mut Running,
    parent: GObjId,
    pos: Vec3,
    kind: i32,
    max: i32,
    angle: f32,
    facing_dir: f32,
) -> Option<GObjId> {
    let b = cs::make(r, parent, pos, ItKind::from_raw(kind), facing_dir)?;
    change_state(r, b, 9, change::ANIM_UPDATE);
    cs::clear_cmd_vars(r, b);
    let life = attr_f32(r, b, cs::attr::LIFETIME);
    item_mut(r.game, b)
        .vars
        .mewtwo_shadowball()
        .unread_life
        .set(life);
    lib2725::set_lifetime(r.game, r.data, b, life);
    let period = attr_i32(r, b, attr::WOBBLE_FRAMES);
    let speed = attr_f32(r, b, attr::THROWN_SPEED);
    let scale = mem(Running::model_scale(r.game.fighter(parent)));
    let v = item_mut(r.game, b).vars.mewtwo_shadowball();
    v.fired.set(1);
    v.motion.set_z(1.0);
    v.unread_effects.set(0);
    v.maker.set(Some(parent));
    v.wobble_offset.set(Vec3::new(0.0, 0.0, 0.0));
    v.wobble_frame.set(period);
    v.motion.set_x(angle);
    v.wobble_angle.set(angle);
    v.motion.set_y(speed);
    v.wobble_size.set(speed);
    v.wobble_step.set(0.0);
    v.flight_frames.set(0);
    v.wobble_scale.set(1.0);
    v.held_scale.set(scale);
    v.held_offset.set(Vec3::new(0.0, 0.0, 0.0));
    v.wobble_offset.set(Vec3::new(0.0, 0.0, 0.0));
    v.charge.set(0);
    v.levels.set(max);
    v.sparkle.set(0);
    let (charge, levels) = (v.charge.get(), v.levels.get());
    let size = mem(lerp_int(
        r,
        b,
        cs::attr::SCALE_MIN,
        cs::attr::SCALE_MAX,
        charge,
        levels,
    ));
    let damage = lerp_int(
        r,
        b,
        cs::attr::DAMAGE_MIN,
        cs::attr::DAMAGE_MAX,
        charge,
        levels,
    );
    let v = item_mut(r.game, b).vars.mewtwo_shadowball();
    v.size.set(size);
    v.unread_damage.set(crate::ppc::cvt_fp2unsigned(damage));
    let (speed, angle) = (reg(v.motion.y()), v.motion.x());
    let c = reg(math::cos(angle));
    item_mut(r.game, b).vel.x = mem(fp::fmuls(speed, c));
    let angle = item_mut(r.game, b).vars.mewtwo_shadowball().motion.x();
    let s = reg(math::sin(angle));
    let ip = item_mut(r.game, b);
    ip.vel.y = mem(fp::fmuls(speed, s));
    ip.vel.z = 0.0;
    lib26b1::disable_pickup(r.game, b);
    Some(b)
}

/// Fires the ball ([`cs::fire`]).
pub fn fire(r: &mut Running, gobj: GObjId, pos: Vec3, angle: f32, charge: f32, max: f32) {
    cs::fire(r, gobj, pos, angle, charge, max, &SHADOW_BALL);
}

/// The ball taken away from his hands ([`cs::remove`]).
pub fn remove(r: &mut Running, gobj: GObjId) {
    cs::remove(r, gobj, &SHADOW_BALL);
}

/// In his hands, `it_2725_Logic101_PickedUp`: the held state, no wobble.
#[doc(alias = "it_2725_Logic101_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    item_mut(r.game, gobj)
        .vars
        .mewtwo_shadowball()
        .wobble_scale
        .set(0.0);
}

/// `itMewtwoshadowball_UnkMotion8_Phys`: after its first frame its speed along its angle,
/// then the wobble.
#[doc(alias = "itMewtwoshadowball_UnkMotion8_Phys")]
pub fn physics_flying(r: &mut Running, gobj: GObjId) {
    if item_mut(r.game, gobj)
        .vars
        .mewtwo_shadowball()
        .flight_frames
        .get()
        > 0
    {
        cs::fly(r, gobj, &SHADOW_BALL);
    }
    wobble(r, gobj);
}

/// `itMewtwoshadowball_UnkMotion8_Coll` and `_UnkMotion9_Coll`: it bursts on the stage.
#[doc(alias = "itMewtwoshadowball_UnkMotion8_Coll")]
#[doc(alias = "itMewtwoshadowball_UnkMotion9_Coll")]
pub fn coll_flying(r: &mut Running, gobj: GObjId) -> bool {
    if cs::hit_stage(r, gobj) {
        burst(r, gobj);
    }
    false
}

/// The ball bursts, `it_802C5E5C`: hidden, its burst's life, its effects gone, the burst's
/// state for its charge (10 to 17), its hitbox's scale times `x24`, the burst's effect at the
/// grandchild and its sound.
#[doc(alias = "it_802C5E5C")]
pub fn burst(r: &mut Running, gobj: GObjId) {
    let root = jobj_of(r.game, gobj);
    jobj::set_flags_all(&mut r.game.objs.jobjs, root, jobj::HIDDEN);
    let life = attr_f32(r, gobj, attr::BURST_LIFETIME);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    ef::eflib::destroy_all(r, gobj);
    let level = cs::clamp_charge(r, gobj, &SHADOW_BALL);
    change_state(r, gobj, level.wrapping_add(10), change::ANIM_UPDATE);
    let k = reg(attr_f32(r, gobj, attr::BURST_SCALE));
    let ip = item_mut(r.game, gobj);
    ip.on_accessory = None;
    let s = mem(fp::fmuls(reg(ip.hitboxes[0].hit.scale), k));
    ip.vars.mewtwo_shadowball().burst_scale.set(s);
    let j = cs::grandchild(r, gobj);
    let pos = crate::lb::joint::world_position(&mut r.game.objs, r.data, Some(j), None);
    ef::spawn(r, GFX_BURST, Some(gobj), &[Arg::Vec(pos)]);
    play_sfx(r, gobj, SFX_BURST, 127, 64);
}

/// `itMewtwoshadowball_UnkMotion17_Anim`: the burst's size and hitbox scale, and its life
/// counts down.
#[doc(alias = "itMewtwoshadowball_UnkMotion17_Anim")]
pub fn animate_burst(r: &mut Running, gobj: GObjId) -> bool {
    let j = cs::grandchild(r, gobj);
    let ip = item_mut(r.game, gobj);
    let s = ip.vars.mewtwo_shadowball().size.get();
    jobj::set_scale(&mut r.game.objs.jobjs, j, Vec3::new(s, s, s));
    let ip = item_mut(r.game, gobj);
    if ip.hitboxes[0].hit.state != HitCapsuleState::Disabled {
        let s = ip.vars.mewtwo_shadowball().burst_scale.get();
        ip.hitboxes[0].hit.scale = s;
    }
    lib2725::tick_lifetime(r.game, gobj)
}

/// `itMewtwoshadowball_UnkMotion9_Anim`: the thrown ball's life counts down.
#[doc(alias = "itMewtwoshadowball_UnkMotion9_Anim")]
pub fn animate_thrown(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::tick_lifetime(r.game, gobj)
}

/// `itMewtwoshadowball_UnkMotion17_Phys`: the burst stays put (`itResetVelocity`).
#[doc(alias = "itMewtwoshadowball_UnkMotion17_Phys")]
pub fn physics_burst(r: &mut Running, gobj: GObjId) {
    lib2725::clear_velocity(r.game, gobj);
}

/// The held and thrown states' physics, `itMewtwoshadowball_UnkMotion0_Phys` and
/// `_UnkMotion9_Phys`: nothing.
#[doc(alias = "itMewtwoshadowball_UnkMotion0_Phys")]
#[doc(alias = "itMewtwoshadowball_UnkMotion9_Phys")]
pub fn no_physics(_r: &mut Running, _gobj: GObjId) {}

/// The held and bursting states' collisions, `itMewtwoshadowball_UnkMotion0_Coll` and
/// `_UnkMotion17_Coll`: nothing, never done.
#[doc(alias = "itMewtwoshadowball_UnkMotion0_Coll")]
#[doc(alias = "itMewtwoshadowball_UnkMotion17_Coll")]
pub fn no_coll(_r: &mut Running, _gobj: GObjId) -> bool {
    false
}
