//! Mewtwo's Shadow Ball, `melee/it/kinds/itmewtwoshadowball.c`: a ball in his hands that
//! grows with his charge (state 0) until he fires it, then a ball of the charge's size and
//! speed (states 1 to 8, by charge) that wobbles as it flies (`it_802C4D10`) until its life runs
//! out or it meets the stage, where it bursts (states 10 to 17). His forward throw fires a
//! full one at once (state 9).
//!
//! Kirby's copy (`It_Kind_Kirby_MewtwoShadowBall`) runs the same code with his own checks
//! (`ft::kirby::special_mewtwo`).

use gekko_fp as fp;

use super::super::item::{article, change, change_state, pick_up, play_sfx, play_sfx1, spawn_air};
use super::super::{ground, item, item_mut, jobj_of, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::ef::{self, Arg};
use crate::game::handles::CodeRef;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::lb::hitcapsule;
use crate::math;
use crate::mp::coll::{
    CEILING_HUG, CEILING_PUSH, FLOOR_HUG, FLOOR_PUSH, LEFT_WALL_MASK, RIGHT_WALL_MASK,
};
use crate::vec::Vec3;

/// `It_Kind_Mewtwo_ShadowBall`.
pub const KIND: i32 = super::MEWTWO_SHADOWBALL;
/// `It_Kind_Kirby_MewtwoShadowBall`.
const KIRBY_SHADOWBALL: i32 = 144;

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
pub const EVT_UNK: u32 = 0x802C_61D4;

/// `itMewtwoShadowball_DatAttrs`.
mod attr {
    pub const LIFETIME: u32 = 0x0;
    pub const SPEED_MIN: u32 = 0x8;
    pub const SPEED_MAX: u32 = 0xC;
    pub const DAMAGE_MIN: u32 = 0x10;
    pub const DAMAGE_MAX: u32 = 0x14;
    pub const SCALE_MIN: u32 = 0x18;
    pub const SCALE_MAX: u32 = 0x1C;
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
/// `@218` (`M_PI / 4`), `@247` (`M_TAU`) and `M_PI`.
const QUARTER_PI: f64 = std::f64::consts::FRAC_PI_4;
const TAU: f64 = std::f64::consts::TAU;
const PI: f64 = std::f64::consts::PI;

/// Runs one of the ball's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM_0 => animate_held(r, gobj),
        COLL_0 | COLL_17 => no_coll(r, gobj),
        ANIM_8 => animate_flying(r, gobj),
        COLL_8 | COLL_9 => coll_flying(r, gobj),
        ANIM_9 => animate_thrown(r, gobj),
        ANIM_17 => animate_burst(r, gobj),
        DMG_DEALT | CLANKED | ABSORBED | HIT_SHIELD => hit_ends(r, gobj),
        REFLECTED => reflected(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        _ => return None,
    })
}

/// Runs one of the ball's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => destroyed(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        PHYS_0 | PHYS_9 => no_physics(r, gobj),
        PHYS_8 => physics_flying(r, gobj),
        PHYS_17 => physics_burst(r, gobj),
        SPARKLE => sparkle(r, gobj),
        _ => return false,
    }
    true
}

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let attrs = article(item(r.game, gobj))
        .special(r.data)
        .expect("the Shadow Ball without attributes");
    r.data.f32(attrs.at(off))
}

fn attr_i32(r: &Running, gobj: GObjId, off: u32) -> i32 {
    attr_f32(r, gobj, off).to_bits() as i32
}

/// The model's grandchild, which the ball scales and moves (`itGetJObjGrandchild`).
fn grandchild(r: &Running, gobj: GObjId) -> JObjId {
    let root = jobj_of(r.game, gobj).expect("the Shadow Ball without a model");
    let child = r.game.objs.jobjs[root]
        .child
        .expect("the ball's model's child");
    r.game.objs.jobjs[child]
        .child
        .expect("the ball's model's grandchild")
}

/// From the attributes' `lo` to `hi` by `charge` of `max` (both integers).
fn lerp_int(r: &Running, gobj: GObjId, lo: u32, hi: u32, charge: i32, max: i32) -> f64 {
    let lo = reg(attr_f32(r, gobj, lo));
    let hi = reg(attr_f32(r, gobj, hi));
    fp::fmadds(
        fp::frsp(f64::from(charge)),
        fp::fdivs(fp::fsubs(hi, lo), fp::frsp(f64::from(max))),
        lo,
    )
}

/// The ball's wobble as it flies, `it_802C4D10`: every `x20` frames (after its first) a turn
/// of up to an eighth either way and a new swing (`x44` undoing `x40` over half the period)
/// with one of three sounds; the model's grandchild is moved off the ball's path by the swing.
/// (Transliterated by ssbm-rs from its machine code: the return value is a register its C
/// never sets, which no caller reads.)
#[doc(alias = "it_802C4D10")]
pub fn wobble(r: &mut Running, gobj: GObjId) {
    let j = grandchild(r, gobj);
    let period = attr_i32(r, gobj, attr::WOBBLE_FRAMES);
    let v = item_mut(r.game, gobj).xDD4_itemVar.mewtwo_shadowball();
    if v.x4C.get() > 0 {
        let n = v.x48.get().wrapping_add(1);
        v.x48.set(n);
        if v.x48.get() >= period {
            v.x48.set(0);
            let rand = reg(r.game.objs.rng.draw_f32());
            let v = item_mut(r.game, gobj).xDD4_itemVar.mewtwo_shadowball();
            let turn = fp::fmuls(2.0, fp::fsubs(rand, 0.5));
            let a = fp::frsp(fp::fmadd(QUARTER_PI, turn, reg(v.x3C.get())));
            v.x3C.set(mem(a));
            let half = fp::fmuls(0.5, fp::frsp(f64::from(period)));
            let swing = fp::fdivs(fp::fneg(reg(v.x40.get())), half);
            v.x44.set(mem(swing));
            let i = r.game.objs.rng.below(3);
            play_sfx1(r, gobj, SFX_WOBBLE[i as usize], 127, 64);
        }
        let v = item_mut(r.game, gobj).xDD4_itemVar.mewtwo_shadowball();
        let x40 = fp::fadds(reg(v.x40.get()), reg(v.x44.get()));
        v.x40.set(mem(x40));
        v.x30.set_x(0.0);
        let s = reg(math::sin(v.x3C.get()));
        let v = item_mut(r.game, gobj).xDD4_itemVar.mewtwo_shadowball();
        let amp = fp::fmuls(reg(v.x40.get()), reg(v.x50.get()));
        let y = fp::fmadds(amp, s, reg(v.x30.y()));
        v.x30.set_y(mem(y));
        let c = reg(math::cos(v.x3C.get()));
        let v = item_mut(r.game, gobj).xDD4_itemVar.mewtwo_shadowball();
        let amp = fp::fmuls(reg(v.x40.get()), reg(v.x50.get()));
        let z = fp::fmadds(amp, c, reg(v.x30.z()));
        v.x30.set_z(mem(z));
    } else {
        v.x30.set(Vec3::new(0.0, 0.0, 0.0));
    }
    let v = item_mut(r.game, gobj).xDD4_itemVar.mewtwo_shadowball();
    let t = v.x30.get();
    jobj::set_translate(&mut r.game.objs.jobjs, j, t);
    let v = item_mut(r.game, gobj).xDD4_itemVar.mewtwo_shadowball();
    let n = v.x4C.get().wrapping_add(1);
    v.x4C.set(n);
}

/// Whether the ball flew into the stage, `it_802C4F50`: a ceiling rising, a floor falling,
/// the wall it flies toward.
#[doc(alias = "it_802C4F50")]
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

/// `Item_InitSpawnPositionFromParent` and `Item_InitSpawnCommonFields(.., true)`.
fn spawn_item(r: &mut Running, parent: GObjId, pos: Vec3, kind: i32, facing: f32) -> SpawnItem {
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = lib26b1::parent_ecb_center(r.game, parent);
    spawn.facing_dir = facing;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(parent);
    spawn.x4_parent_gobj2 = spawn.x0_parent_gobj;
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    spawn
}

/// `Item_ClearCmdVars`.
fn clear_cmd_vars(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.xDB8_itcmd_var3 = 0;
    ip.xDB4_itcmd_var2 = 0;
    ip.xDB0_itcmd_var1 = 0;
    ip.xDAC_itcmd_var0 = 0;
}

/// Makes the ball in `parent`'s hands at `pos`, `it_802C5000`: its lifetime, not fired, a
/// random first wobble, his model's scale kept, and held at his part `part`.
#[doc(alias = "it_802C5000")]
pub fn spawn_held(
    r: &mut Running,
    parent: GObjId,
    pos: Vec3,
    part: i32,
    kind: i32,
    facing_dir: f32,
) -> Option<GObjId> {
    let mut spawn = spawn_item(r, parent, pos, kind, facing_dir);
    let b = spawn_air(r, &mut spawn)?;
    clear_cmd_vars(r, b);
    let life = attr_f32(r, b, attr::LIFETIME);
    lib2725::set_lifetime(r.game, r.data, b, life);
    let period = attr_i32(r, b, attr::WOBBLE_FRAMES);
    let v = item_mut(r.game, b).xDD4_itemVar.mewtwo_shadowball();
    v.x14.set(0);
    v.x4.set_z(0.0);
    v.x28.set(0);
    v.x2C.set(Some(parent));
    v.x30.set(Vec3::new(0.0, 0.0, 0.0));
    v.x48.set(period);
    let rand = reg(r.game.objs.rng.draw_f32());
    let v = item_mut(r.game, b).xDD4_itemVar.mewtwo_shadowball();
    v.x3C.set(mem(fp::frsp(fp::fmul(TAU, rand))));
    let rand = reg(r.game.objs.rng.draw_f32());
    let v = item_mut(r.game, b).xDD4_itemVar.mewtwo_shadowball();
    v.x40.set(mem(fp::fadds(2.5, fp::fsubs(rand, 0.5))));
    let half = fp::fmuls(0.5, fp::frsp(f64::from(v.x48.get())));
    let swing = fp::fdivs(fp::fneg(reg(v.x40.get())), half);
    v.x44.set(mem(swing));
    v.x4C.set(0);
    v.x50.set(0.0);
    let scale = mem(Running::model_scale(r.game.fighter(parent)));
    let v = item_mut(r.game, b).xDD4_itemVar.mewtwo_shadowball();
    v.x54.set(scale);
    v.x58.set(Vec3::new(0.0, 0.0, 0.0));
    pick_up(r, b, parent, part);
    Some(b)
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
    let mut spawn = spawn_item(r, parent, pos, kind, facing_dir);
    let b = spawn_air(r, &mut spawn)?;
    change_state(r, b, 9, change::ANIM_UPDATE);
    clear_cmd_vars(r, b);
    let life = attr_f32(r, b, attr::LIFETIME);
    item_mut(r.game, b)
        .xDD4_itemVar
        .mewtwo_shadowball()
        .x0
        .set(life);
    lib2725::set_lifetime(r.game, r.data, b, life);
    let period = attr_i32(r, b, attr::WOBBLE_FRAMES);
    let speed = attr_f32(r, b, attr::THROWN_SPEED);
    let scale = mem(Running::model_scale(r.game.fighter(parent)));
    let v = item_mut(r.game, b).xDD4_itemVar.mewtwo_shadowball();
    v.x14.set(1);
    v.x4.set_z(1.0);
    v.x28.set(0);
    v.x2C.set(Some(parent));
    v.x30.set(Vec3::new(0.0, 0.0, 0.0));
    v.x48.set(period);
    v.x4.set_x(angle);
    v.x3C.set(angle);
    v.x4.set_y(speed);
    v.x40.set(speed);
    v.x44.set(0.0);
    v.x4C.set(0);
    v.x50.set(1.0);
    v.x54.set(scale);
    v.x58.set(Vec3::new(0.0, 0.0, 0.0));
    v.x30.set(Vec3::new(0.0, 0.0, 0.0));
    v.x18.set(0);
    v.x1C.set(max);
    v.x20.set(0);
    let (charge, levels) = (v.x18.get(), v.x1C.get());
    let size = mem(lerp_int(
        r,
        b,
        attr::SCALE_MIN,
        attr::SCALE_MAX,
        charge,
        levels,
    ));
    let damage = lerp_int(r, b, attr::DAMAGE_MIN, attr::DAMAGE_MAX, charge, levels);
    let v = item_mut(r.game, b).xDD4_itemVar.mewtwo_shadowball();
    v.x10.set(size);
    v.x24.set(crate::ppc::cvt_fp2unsigned(damage));
    let (speed, angle) = (reg(v.x4.y()), v.x4.x());
    let c = reg(math::cos(angle));
    item_mut(r.game, b).x40_vel.x = mem(fp::fmuls(speed, c));
    let angle = item_mut(r.game, b).xDD4_itemVar.mewtwo_shadowball().x4.x();
    let s = reg(math::sin(angle));
    let ip = item_mut(r.game, b);
    ip.x40_vel.y = mem(fp::fmuls(speed, s));
    ip.x40_vel.z = 0.0;
    lib26b1::disable_pickup(r.game, b);
    Some(b)
}

/// Fires the ball from `pos` at `angle`, `it_802C53F0`: its charge (`charge` within 0 and
/// `max`) sets its speed, damage and size; while its owner is the one who made it, it leaves
/// his hands ([`fired`], `it_8027429C`), flies his way, and is fired (`x14`).
#[doc(alias = "it_802C53F0")]
pub fn fire(r: &mut Running, gobj: GObjId, pos: Vec3, angle: f32, charge: f32, max: f32) {
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .mewtwo_shadowball()
        .x4
        .set_x(angle);
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
    let v = item_mut(r.game, gobj).xDD4_itemVar.mewtwo_shadowball();
    v.x18.set(fp::fctiwz(charge));
    v.x1C.set(fp::fctiwz(max));
    let ip = item_mut(r.game, gobj);
    let owner = ip.owner;
    let maker = ip.xDD4_itemVar.mewtwo_shadowball().x2C.get();
    let Some(maker) = maker else { return };
    if owner != Some(maker) {
        return;
    }
    fired(r, gobj);
    // `HSD_MtxGetRotation` of the held part's matrix: its result goes unused.
    super::super::release::let_go(r, gobj, Vec3::new(0.0, 0.0, 0.0));
    item_mut(r.game, gobj).xDC8_word.flags.x14 = false;
    lib26b1::disable_pickup(r.game, gobj);
    let lerp = |r: &Running, lo, hi| {
        let (lo, hi) = (reg(attr_f32(r, gobj, lo)), reg(attr_f32(r, gobj, hi)));
        fp::fmadds(charge, fp::fdivs(fp::fsubs(hi, lo), max), lo)
    };
    let speed = lerp(r, attr::SPEED_MIN, attr::SPEED_MAX);
    let damage = lerp(r, attr::DAMAGE_MIN, attr::DAMAGE_MAX);
    let v = item_mut(r.game, gobj).xDD4_itemVar.mewtwo_shadowball();
    v.x4.set_y(mem(speed));
    v.x24.set(crate::ppc::cvt_fp2unsigned(damage));
    v.x4.set_z(0.0);
    let (level, levels) = (v.x18.get(), v.x1C.get());
    let size = mem(lerp_int(
        r,
        gobj,
        attr::SCALE_MIN,
        attr::SCALE_MAX,
        level,
        levels,
    ));
    let facing = r.game.ft_facing_dir(maker);
    let ip = item_mut(r.game, gobj);
    let v = ip.xDD4_itemVar.mewtwo_shadowball();
    v.x10.set(size);
    v.x20.set(0);
    ip.facing_dir = facing;
    ip.pos = pos;
    let v = ip.xDD4_itemVar.mewtwo_shadowball();
    v.x28.set(0);
    let speed = fp::fmuls(reg(v.x4.y()), reg(v.x50.get()));
    let c = reg(math::cos(v.x4.x()));
    item_mut(r.game, gobj).x40_vel.x = mem(fp::fmuls(speed, c));
    let v = item_mut(r.game, gobj).xDD4_itemVar.mewtwo_shadowball();
    let speed = fp::fmuls(reg(v.x4.y()), reg(v.x50.get()));
    let s = reg(math::sin(v.x4.x()));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = mem(fp::fmuls(speed, s));
    ip.x40_vel.z = 0.0;
    let v = ip.xDD4_itemVar.mewtwo_shadowball();
    v.x14.set(1);
    v.x4C.set(0);
    let root = jobj_of(r.game, gobj).expect("the Shadow Ball without a model");
    let child = r.game.objs.jobjs[root].child;
    let child = child.expect("HSD_JObjSetScale on a null joint");
    jobj::set_scale(&mut r.game.objs.jobjs, child, Vec3::new(1.0, 1.0, 1.0));
}

/// The ball's end, `it_2725_Logic101_Destroyed`: its effects gone; unfired, its maker lets
/// it go (`ftMt_SpecialN_SetNULL`) if he still owns it; then no owner.
#[doc(alias = "it_2725_Logic101_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    ef::eflib::destroy_all(r, gobj);
    let ip = item_mut(r.game, gobj);
    let (owner, kind) = (ip.owner, ip.kind);
    let v = ip.xDD4_itemVar.mewtwo_shadowball();
    if v.x14.get() == 0 {
        if let Some(maker) = v.x2C.get()
            && owner == Some(maker)
        {
            match kind {
                KIND => r.mt_n_ball_released(maker),
                KIRBY_SHADOWBALL => r.kb_mt_ball_released(maker),
                _ => {}
            }
        }
        item_mut(r.game, gobj)
            .xDD4_itemVar
            .mewtwo_shadowball()
            .x2C
            .set(None);
    }
    let ip = item_mut(r.game, gobj);
    ip.owner = None;
    ip.xDC8_word.flags.x13 = false;
}

/// The ball taken away from his hands, `it_802C573C`: its effects gone, and the item.
#[doc(alias = "it_802C573C")]
pub fn remove(r: &mut Running, gobj: GObjId) {
    ef::eflib::destroy_all(r, gobj);
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .mewtwo_shadowball()
        .x28
        .set(0);
    super::super::item::destroy(r, gobj);
}

/// In his hands, `it_2725_Logic101_PickedUp`: the held state, no wobble.
#[doc(alias = "it_2725_Logic101_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .mewtwo_shadowball()
        .x50
        .set(0.0);
}

/// `itMewtwoshadowball_UnkMotion0_Anim`: while its maker holds it, the ball goes once he
/// loses it (hit, out of the charge's states, putting it away or holding none) and otherwise
/// takes his charge; it grows with the charge, and its grandchild is set back by its first
/// offset times his scale.
#[doc(alias = "itMewtwoshadowball_UnkMotion0_Anim")]
pub fn animate_held(r: &mut Running, gobj: GObjId) -> bool {
    let j = grandchild(r, gobj);
    let ip = item_mut(r.game, gobj);
    let (owner, kind) = (ip.owner, ip.kind);
    let maker = ip.xDD4_itemVar.mewtwo_shadowball().x2C.get();
    if maker.is_some() && owner == maker {
        match kind {
            KIND => {
                if r.mt_n_ball_lost(maker) || r.mt_n_ball_put_away(maker) {
                    return true;
                }
                let Some((charge, max)) = r.mt_n_charge_of(maker) else {
                    return true;
                };
                let v = item_mut(r.game, gobj).xDD4_itemVar.mewtwo_shadowball();
                v.x18.set(charge);
                v.x1C.set(max);
            }
            KIRBY_SHADOWBALL => {
                if r.kb_mt_ball_lost(maker) || r.kb_mt_ball_put_away(maker) {
                    return true;
                }
                let Some((charge, max)) = r.kb_mt_charge_of(maker) else {
                    return true;
                };
                let v = item_mut(r.game, gobj).xDD4_itemVar.mewtwo_shadowball();
                v.x18.set(charge);
                v.x1C.set(max);
            }
            _ => {}
        }
    }
    let v = item_mut(r.game, gobj).xDD4_itemVar.mewtwo_shadowball();
    let (charge, max) = (v.x18.get(), v.x1C.get());
    let s = mem(lerp_int(
        r,
        gobj,
        attr::SCALE_MIN,
        attr::SCALE_MAX,
        charge,
        max,
    ));
    jobj::set_scale(&mut r.game.objs.jobjs, j, Vec3::new(s, s, s));
    let v = item_mut(r.game, gobj).xDD4_itemVar.mewtwo_shadowball();
    if v.x58.x() == 0.0 && v.x58.y() == 0.0 && v.x58.z() == 0.0 {
        let t = r.game.objs.jobjs[j].translate;
        item_mut(r.game, gobj)
            .xDD4_itemVar
            .mewtwo_shadowball()
            .x58
            .set(t);
    }
    let v = item_mut(r.game, gobj).xDD4_itemVar.mewtwo_shadowball();
    let k = reg(v.x54.get());
    let o = v.x58.get();
    let t = Vec3::new(
        mem(fp::fmuls(fp::fneg(reg(o.x)), k)),
        mem(fp::fmuls(fp::fneg(reg(o.y)), k)),
        mem(fp::fmuls(fp::fneg(reg(o.z)), k)),
    );
    jobj::set_translate(&mut r.game.objs.jobjs, j, t);
    false
}

/// Fired, `it_802C5B18`: its lifetime, its effects gone, the state for its charge (1 to 8),
/// the sparkles hooked, and its wobble's size by the charge (half to whole).
#[doc(alias = "it_802C5B18")]
pub fn fired(r: &mut Running, gobj: GObjId) {
    let life = attr_f32(r, gobj, attr::LIFETIME);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    ef::eflib::destroy_all(r, gobj);
    let level = clamp_charge(r, gobj);
    change_state(r, gobj, level.wrapping_add(1), change::ANIM_UPDATE);
    let ip = item_mut(r.game, gobj);
    ip.on_accessory = Some(CodeRef(SPARKLE));
    let v = ip.xDD4_itemVar.mewtwo_shadowball();
    let part = fp::fdivs(
        fp::frsp(f64::from(v.x18.get())),
        fp::frsp(f64::from(v.x1C.get())),
    );
    v.x50.set(mem(fp::fmadds(0.5, part, 0.5)));
}

/// No effects out (`x28`), and the charge kept within 0 and its levels; the charge.
fn clamp_charge(r: &mut Running, gobj: GObjId) -> i32 {
    let v = item_mut(r.game, gobj).xDD4_itemVar.mewtwo_shadowball();
    v.x28.set(0);
    if v.x18.get() < 0 {
        v.x18.set(0);
    }
    if v.x18.get() >= v.x1C.get() {
        let m = v.x1C.get();
        v.x18.set(m);
    }
    v.x18.get()
}

/// `itMewtwoshadowball_UnkMotion8_Anim`: its size, and its life counts down.
#[doc(alias = "itMewtwoshadowball_UnkMotion8_Anim")]
pub fn animate_flying(r: &mut Running, gobj: GObjId) -> bool {
    let j = grandchild(r, gobj);
    let s = item_mut(r.game, gobj)
        .xDD4_itemVar
        .mewtwo_shadowball()
        .x10
        .get();
    jobj::set_scale(&mut r.game.objs.jobjs, j, Vec3::new(s, s, s));
    lib2725::tick_lifetime(r.game, gobj)
}

/// `itMewtwoshadowball_UnkMotion8_Phys`: after its first frame its speed along its angle,
/// then the wobble.
#[doc(alias = "itMewtwoshadowball_UnkMotion8_Phys")]
pub fn physics_flying(r: &mut Running, gobj: GObjId) {
    let v = item_mut(r.game, gobj).xDD4_itemVar.mewtwo_shadowball();
    if v.x4C.get() > 0 {
        let speed = reg(v.x4.y());
        let c = reg(math::cos(v.x4.x()));
        item_mut(r.game, gobj).x40_vel.x = mem(fp::fmuls(speed, c));
        let v = item_mut(r.game, gobj).xDD4_itemVar.mewtwo_shadowball();
        let speed = reg(v.x4.y());
        let s = reg(math::sin(v.x4.x()));
        item_mut(r.game, gobj).x40_vel.y = mem(fp::fmuls(speed, s));
    }
    wobble(r, gobj);
}

/// `itMewtwoshadowball_UnkMotion8_Coll` and `_UnkMotion9_Coll`: it bursts on the stage.
#[doc(alias = "itMewtwoshadowball_UnkMotion8_Coll")]
#[doc(alias = "itMewtwoshadowball_UnkMotion9_Coll")]
pub fn coll_flying(r: &mut Running, gobj: GObjId) -> bool {
    if hit_stage(r, gobj) {
        burst(r, gobj);
    }
    false
}

/// A fired ball's accessory, `fn_802C5E18`: a counter cycling every third frame (the full
/// ball's test does nothing).
#[doc(alias = "fn_802C5E18")]
pub fn sparkle(r: &mut Running, gobj: GObjId) {
    let v = item_mut(r.game, gobj).xDD4_itemVar.mewtwo_shadowball();
    let _ = v.x18.get() == v.x1C.get();
    let n = v.x20.get().wrapping_add(1);
    v.x20.set(n);
    let n = v.x20.get() % 3;
    v.x20.set(n);
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
    let level = clamp_charge(r, gobj);
    change_state(r, gobj, level.wrapping_add(10), change::ANIM_UPDATE);
    let k = reg(attr_f32(r, gobj, attr::BURST_SCALE));
    let ip = item_mut(r.game, gobj);
    ip.on_accessory = None;
    let s = mem(fp::fmuls(reg(ip.x5D4_hitboxes[0].hit.scale), k));
    ip.xDD4_itemVar.mewtwo_shadowball().x64.set(s);
    let j = grandchild(r, gobj);
    let pos = crate::lb::joint::world_position(&mut r.game.objs, r.data, Some(j), None);
    ef::spawn(r, GFX_BURST, Some(gobj), &[Arg::Vec(pos)]);
    play_sfx(r, gobj, SFX_BURST, 127, 64);
}

/// `itMewtwoshadowball_UnkMotion17_Anim`: the burst's size and hitbox scale, and its life
/// counts down.
#[doc(alias = "itMewtwoshadowball_UnkMotion17_Anim")]
pub fn animate_burst(r: &mut Running, gobj: GObjId) -> bool {
    let j = grandchild(r, gobj);
    let ip = item_mut(r.game, gobj);
    let s = ip.xDD4_itemVar.mewtwo_shadowball().x10.get();
    jobj::set_scale(&mut r.game.objs.jobjs, j, Vec3::new(s, s, s));
    let ip = item_mut(r.game, gobj);
    if ip.x5D4_hitboxes[0].hit.state != hitcapsule::DISABLED {
        let s = ip.xDD4_itemVar.mewtwo_shadowball().x64.get();
        ip.x5D4_hitboxes[0].hit.scale = s;
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

/// A hit dealt, clanked, absorbed or on a shield ends the ball,
/// `itMewtwoShadowball_Logic101_DmgDealt` and the others.
#[doc(alias = "itMewtwoShadowball_Logic101_DmgDealt")]
#[doc(alias = "itMewtwoShadowball_Logic101_Clanked")]
#[doc(alias = "itMewtwoShadowball_Logic101_Absorbed")]
#[doc(alias = "itMewtwoShadowball_Logic101_HitShield")]
pub fn hit_ends(_r: &mut Running, _gobj: GObjId) -> bool {
    true
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

/// Reflected, `it_2725_Logic101_Reflected`: it flies back (its facing unchanged).
#[doc(alias = "it_2725_Logic101_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    let v = item_mut(r.game, gobj).xDD4_itemVar.mewtwo_shadowball();
    let a = mem(fp::frsp(fp::fadd(reg(v.x4.x()), PI)));
    v.x4.set_x(a);
    let a = wrap(v.x4.x());
    v.x4.set_x(a);
    false
}

/// Bouncing off a shield, `it_2725_Logic101_ShieldBounced`: its speed mirrored off the
/// shield, its angle from the new speed.
#[doc(alias = "it_2725_Logic101_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    let normal = ip.xC58;
    ip.x40_vel.mirror_xy(normal);
    let a = math::atan2(ip.x40_vel.y, ip.x40_vel.x);
    let v = item_mut(r.game, gobj).xDD4_itemVar.mewtwo_shadowball();
    v.x4.set_x(a);
    let a = wrap(v.x4.x());
    v.x4.set_x(a);
    false
}

/// Forgets a GObj the ball points to, `itMewtwoShadowball_Logic101_EvtUnk` (`it_8026B894`).
#[doc(alias = "itMewtwoShadowball_Logic101_EvtUnk")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
