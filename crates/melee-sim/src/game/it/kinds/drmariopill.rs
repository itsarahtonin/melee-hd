//! Dr. Mario's Megavitamin, `melee/it/kinds/itdrmariopill.c`: thrown, it falls and bounces
//! along the stage until its lifetime runs out or a bounce leaves it too slow; in his taunt it
//! sits on his position instead. Its color combination is its animation's frame.

use gekko_fp as fp;

use super::super::item::{
    article, attr_of, change, change_state, common_data, destroy, play_sfx, spawn_air,
};
use super::super::{data, ground, item, item_mut, jobj_of, lib26b1, lib2725, maplib};
use crate::fpu::{mem, reg};
use crate::game::handles::CodeRef;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

pub const ACCESSORY: u32 = 0x802C_061C;
pub const MOTION0_ANIM: u32 = 0x802C_0C0C;
pub const MOTION0_PHYS: u32 = 0x802C_0C68;
pub const MOTION0_COLL: u32 = 0x802C_0CC4;
pub const MOTION2_ANIM: u32 = 0x802C_0E48;
pub const MOTION2_PHYS: u32 = 0x802C_1114;
pub const MOTION2_COLL: u32 = 0x802C_1148;
pub const PICKED_UP: u32 = 0x802C_11E0;
pub const MOTION6_ANIM: u32 = 0x802C_1234;
pub const MOTION6_PHYS: u32 = 0x802C_1340;
pub const MOTION6_COLL: u32 = 0x802C_1360;
#[doc(alias = "itDrMarioPill_DmgDealt")]
pub const DMG_DEALT: u32 = 0x802C_1384;
#[doc(alias = "itDrMarioPill_Reflected")]
pub const REFLECTED: u32 = 0x802C_13CC;
#[doc(alias = "itDrMarioPill_Clanked")]
pub const CLANKED: u32 = 0x802C_1400;
#[doc(alias = "itDrMarioPill_HitShield")]
pub const HIT_SHIELD: u32 = 0x802C_145C;
#[doc(alias = "itDrMarioPill_Absorbed")]
pub const ABSORBED: u32 = 0x802C_14B8;
#[doc(alias = "itDrMarioPill_ShieldBounced")]
pub const SHIELD_BOUNCED: u32 = 0x802C_1514;
pub const EVT_UNK: u32 = 0x802C_1570;

/// `itDrMarioPillAttributes`: the speed, the angle, the lifetime and the slowest speed a
/// bounce keeps it.
mod attr {
    pub const SPEED: u32 = 0x0;
    pub const ANGLE: u32 = 0x4;
    pub const LIFETIME: u32 = 0x8;
    pub const MIN_SPEED: u32 = 0x10;
}

/// The bounce's and the hit's sound.
const SFX: i32 = 0x15FAE;

/// The taunt's pill's lifetime.
const APPEAL_LIFETIME: f32 = 1200.0;

/// Runs one of the pill's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        MOTION0_ANIM => motion0_anim(r, gobj),
        MOTION0_COLL => motion0_coll(r, gobj),
        MOTION2_ANIM => motion2_anim(r, gobj),
        MOTION2_COLL => {
            motion2_phys(r, gobj);
            false
        }
        MOTION6_ANIM => motion6_anim(r, gobj),
        MOTION6_COLL => {
            lib26b1::disable_pickup(r.game, gobj);
            false
        }
        DMG_DEALT => {
            lib26b1::disable_pickup(r.game, gobj);
            play_sfx(r, gobj, SFX, 0x7F, 0x40);
            true
        }
        REFLECTED => {
            lib26b1::disable_pickup(r.game, gobj);
            lib2725::reflected_back(r.game, gobj)
        }
        CLANKED | HIT_SHIELD | ABSORBED | SHIELD_BOUNCED => {
            lib26b1::disable_pickup(r.game, gobj);
            lib26b1::disable_pickup(r.game, gobj);
            play_sfx(r, gobj, SFX, 0x7F, 0x40);
            true
        }
        _ => return None,
    })
}

/// Runs one of the pill's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        ACCESSORY => follow_owner(r, gobj),
        MOTION0_PHYS => motion0_phys(r, gobj),
        MOTION2_PHYS => motion2_phys(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        MOTION6_PHYS => lib26b1::disable_pickup(r.game, gobj),
        _ => return false,
    }
    true
}

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let attrs = article(item(r.game, gobj))
        .special(r.data)
        .expect("a pill without attributes");
    r.data.f32(attrs.at(off))
}

/// The item's model in its state `state`'s pose at `frame`, `it_80273670`: the pose's
/// animations added and run once, then removed (the pill's color is its frame).
#[doc(alias = "it_80273670")]
pub fn set_pose(r: &mut Running, gobj: GObjId, state: i32, frame: f32) {
    let data = r.data;
    let descs = article(item(r.game, gobj))
        .states(data)
        .expect("an article without states");
    let desc = descs.at(0x10 * state as u32);
    item_mut(r.game, gobj).xD0_itemStateDesc = Some(desc);
    super::super::item::set_state_anims_at(r, gobj, desc, frame);
    let model = jobj_of(r.game, gobj);
    jobj::anim_all(
        &mut r.game.objs,
        data,
        &mut crate::game::code::ParticleHooks,
        model,
    );
    jobj::remove_anim_all(&mut r.game.objs, model);
    item_mut(r.game, gobj).x524_cmd.script.cursor = None;
}

/// A new pill's spawn fields: from `pos`, its owner's ECB center, facing `facing_dir`, with a
/// first collision.
fn spawn_fields(r: &Running, parent: GObjId, pos: Vec3, kind: i32, facing_dir: f32) -> SpawnItem {
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = super::super::owner::ecb_center(r.game, parent);
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(parent);
    spawn.x4_parent_gobj2 = Some(parent);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    spawn
}

/// Throws a pill of color combination `color` for `parent` from `pos`,
/// `itDrMarioPill_Spawn`.
#[doc(alias = "itDrMarioPill_Spawn")]
pub fn spawn(r: &mut Running, parent: GObjId, pos: Vec3, color: i32, kind: i32, facing_dir: f32) {
    let mut spawn = spawn_fields(r, parent, pos, kind, facing_dir);
    let Some(gobj) = spawn_air(r, &mut spawn) else {
        return;
    };
    setup(r, gobj);
    set_pose(r, gobj, 0, mem(fp::frsp(f64::from(color))));
    super::super::item::debug_owner_bubbles(r.game, gobj, Some(parent));
    super::super::lib2725::first_frame(r, gobj);
    lib26b1::disable_pickup(r.game, gobj);
}

/// The thrown pill's start, `itDrMarioPill_802C0B5C`: its speed at its angle, ahead, its
/// lifetime, its first state.
#[doc(alias = "itDrMarioPill_802C0B5C")]
fn setup(r: &mut Running, gobj: GObjId) {
    let speed = reg(attr_f32(r, gobj, attr::SPEED));
    let angle = attr_f32(r, gobj, attr::ANGLE);
    let lifetime = attr_f32(r, gobj, attr::LIFETIME);
    let c = reg(crate::math::cos(angle));
    let s = reg(crate::math::sin(angle));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(ip.facing_dir), fp::fmuls(speed, c)));
    ip.x40_vel.y = mem(fp::fmuls(speed, s));
    ip.x40_vel.z = 0.0;
    lib2725::set_lifetime(r.game, r.data, gobj, lifetime);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    lib26b1::disable_pickup(r.game, gobj);
}

/// The taunt's pill on its owner, `itDrMarioPill_802C061C` (its accessory callback): at the
/// owner's position, turned to its facing.
#[doc(alias = "itDrMarioPill_802C061C")]
fn follow_owner(r: &mut Running, gobj: GObjId) {
    let owner = item_mut(r.game, gobj).xDD4_itemVar.drmario_pill().x4.get();
    if let Some(o) = owner {
        let v = r.game.ft_pos(o);
        item_mut(r.game, gobj).pos = v;
        let j = jobj_of(r.game, gobj).expect("a pill without a model");
        let facing = item_mut(r.game, gobj).xDD4_itemVar.drmario_pill().x0.get();
        super::super::item::set_translate(r.game, j, v);
        let ry = mem(fp::frsp(fp::fmul(
            fp::fdiv(std::f64::consts::PI, 2.0),
            reg(facing),
        )));
        lib2725::set_rotation_y(r.game, j, ry);
        lib2725::set_rotation_z(r.game, j, 0.0);
    }
    lib26b1::disable_pickup(r.game, gobj);
}

/// The taunt's pill for `parent` at `pos`, `itDrMarioPill_Appeal_Spawn`: it sits on its owner
/// (`itDrMarioPill_802C061C`), blinking, until the taunt lets go of it.
#[doc(alias = "itDrMarioPill_Appeal_Spawn")]
pub fn appeal_spawn(
    r: &mut Running,
    parent: GObjId,
    pos: Vec3,
    color: i32,
    kind: i32,
    facing_dir: f32,
) -> Option<GObjId> {
    let mut spawn = spawn_fields(r, parent, pos, kind, facing_dir);
    let gobj = spawn_air(r, &mut spawn)?;
    appeal_setup(r, gobj);
    set_pose(r, gobj, 1, mem(fp::frsp(f64::from(color))));
    lib26b1::disable_pickup(r.game, gobj);
    change_state(r, gobj, 2, change::ANIM_UPDATE);
    let ip = item_mut(r.game, gobj);
    ip.xDB8_itcmd_var3 = 0;
    ip.xDB4_itcmd_var2 = 0;
    ip.xDB0_itcmd_var1 = 0;
    ip.xDAC_itcmd_var0 = 0;
    ip.xDC8_word.flags.x19 = false;
    ip.on_accessory = Some(CodeRef(ACCESSORY));
    ip.xDC8_word.flags.x13 = true;
    ip.owner = Some(parent);
    let v = ip.xDD4_itemVar.drmario_pill();
    v.x4.set(Some(parent));
    v.x0.set(facing_dir);
    lib2725::stop_spin(r.game, r.data, gobj);
    super::super::hitbox::start_blink(r.game, gobj);
    lib26b1::disable_pickup(r.game, gobj);
    super::super::item::debug_owner_bubbles(r.game, gobj, Some(parent));
    super::super::lib2725::first_frame(r, gobj);
    lib26b1::disable_pickup(r.game, gobj);
    Some(gobj)
}

/// `itDrMarioPill_UnkMotion0_Anim`: its lifetime.
#[doc(alias = "itDrMarioPill_UnkMotion0_Anim")]
fn motion0_anim(r: &mut Running, gobj: GObjId) -> bool {
    lib26b1::disable_pickup(r.game, gobj);
    let ip = item_mut(r.game, gobj);
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    reg(ip.xD44_lifeTimer) <= 0.0
}

/// `itDrMarioPill_UnkMotion0_Phys`: gravity (`Item_ApplyFallingPhysics`).
#[doc(alias = "itDrMarioPill_UnkMotion0_Phys")]
fn motion0_phys(r: &mut Running, gobj: GObjId) {
    lib26b1::disable_pickup(r.game, gobj);
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, data::attr::FALL_SPEED);
    let max = a.f32(r.data, data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
    let x68 = common_data(r.game).f32(r.data, data::common::X68);
    lib2725::clamp_fall(r.game, gobj, x68);
}

/// `itDrmariopill_UnkMotion0_Coll`: it stays in the air; when it meets the stage it bounces,
/// and ends if the bounce leaves it slower than its attributes allow, else with a sound.
#[doc(alias = "itDrmariopill_UnkMotion0_Coll")]
fn motion0_coll(r: &mut Running, gobj: GObjId) -> bool {
    ground::air_stay(r, gobj);
    lib26b1::disable_pickup(r.game, gobj);
    if !maplib::reflect_velocity(r.game, r.data, gobj) {
        return false;
    }
    let min = attr_f32(r, gobj, attr::MIN_SPEED);
    let v = item(r.game, gobj).x40_vel;
    let sq = fp::fadds(fp::fmuls(reg(v.x), reg(v.x)), fp::fmuls(reg(v.y), reg(v.y)));
    if reg(crate::math::sqrt_accurate(mem(sq))) < reg(min) {
        return true;
    }
    play_sfx(r, gobj, SFX, 0x7F, 0x40);
    false
}

/// Takes the taunt's pill away, `itDrMarioPill_802C0DBC`.
#[doc(alias = "itDrMarioPill_802C0DBC")]
pub fn remove(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .drmario_pill()
        .x4
        .set(None);
    destroy(r, gobj);
}

/// The taunt's pill's start, `itDrMarioPill_802C0DF8`: its lifetime and the pose state.
#[doc(alias = "itDrMarioPill_802C0DF8")]
fn appeal_setup(r: &mut Running, gobj: GObjId) {
    lib2725::set_lifetime(r.game, r.data, gobj, APPEAL_LIFETIME);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    lib26b1::disable_pickup(r.game, gobj);
    lib26b1::disable_pickup(r.game, gobj);
}

/// The taunt's pill lets go of its owner (`itDrMarioPill_Motion2_Anim`'s inline): the owner
/// forgets it if it holds it; true when it ends.
fn release(r: &mut Running, gobj: GObjId) -> bool {
    let owner = item_mut(r.game, gobj).xDD4_itemVar.drmario_pill().x4.get();
    if let Some(o) = owner
        && item(r.game, gobj).owner == Some(o)
    {
        r.dr_forget_appeal_pill(o);
    }
    let ip = item_mut(r.game, gobj);
    ip.xDD4_itemVar.drmario_pill().x4.set(None);
    ip.on_accessory = None;
    let ip = item_mut(r.game, gobj);
    ip.xDD4_itemVar.drmario_pill().x4.set(None);
    ip.on_accessory = None;
    true
}

/// `itDrMarioPill_Motion2_Anim`: blinking; shown while the taunt's script says; it ends
/// when its owner leaves the taunt (or loses it), else follows the owner's model scale.
#[doc(alias = "itDrMarioPill_Motion2_Anim")]
fn motion2_anim(r: &mut Running, gobj: GObjId) -> bool {
    super::super::hitbox::start_blink(r.game, gobj);
    lib26b1::disable_pickup(r.game, gobj);
    let model = jobj_of(r.game, gobj);
    let child = model.and_then(|j| r.game.objs.jobjs[j].child);
    lib26b1::disable_pickup(r.game, gobj);
    if item(r.game, gobj).xDAC_itcmd_var0 == 1 {
        jobj::clear_flags(&mut r.game.objs.jobjs, child, 1 << 4);
    } else {
        jobj::set_flags(&mut r.game.objs.jobjs, child, 1 << 4);
    }
    let owner = item_mut(r.game, gobj).xDD4_itemVar.drmario_pill().x4.get();
    let Some(o) = owner else {
        return release(r, gobj);
    };
    if r.dr_appeal_pill_done(o) {
        return release(r, gobj);
    }
    item_mut(r.game, gobj).xDAC_itcmd_var0 = r.dr_appeal_cmd_var1(o);
    if item(r.game, gobj).owner == Some(o) {
        let scale = mem(crate::game::state::Running::model_scale(r.game.fighter(o)));
        let j = model.expect("a pill without a model");
        lib2725::set_uniform_scale(r.game, j, scale);
        false
    } else {
        release(r, gobj)
    }
}

/// `itDrMarioPill_Motion2_Phys` (and `itDrMariopill_Motion2_Coll`): blinking.
#[doc(alias = "itDrMarioPill_Motion2_Phys")]
#[doc(alias = "itDrMariopill_Motion2_Coll")]
fn motion2_phys(r: &mut Running, gobj: GObjId) {
    super::super::hitbox::start_blink(r.game, gobj);
    lib26b1::disable_pickup(r.game, gobj);
}

/// `itDrMarioPill_PickedUp`: its lifetime and the held state.
// Ports `itDrMarioPill_PickedUp`; aliased once a check reaches it.
fn picked_up(r: &mut Running, gobj: GObjId) {
    let lifetime = attr_f32(r, gobj, attr::LIFETIME);
    lib2725::set_lifetime(r.game, r.data, gobj, lifetime);
    change_state(r, gobj, 6, change::ANIM_UPDATE);
    lib26b1::disable_pickup(r.game, gobj);
}

/// `itDrMarioPill_Motion6_Anim`: held by a frozen or sleeping owner, it takes the owner's
/// scale.
// Ports `itDrMarioPill_Motion6_Anim`; aliased once a check reaches it.
fn motion6_anim(r: &mut Running, gobj: GObjId) -> bool {
    lib26b1::disable_pickup(r.game, gobj);
    let owner = item_mut(r.game, gobj).xDD4_itemVar.drmario_pill().x4.get();
    if let Some(o) = owner {
        let m = r.game.fighter(o).motion_id;
        // `ftCo_800BF228`: frozen in a KO toward the screen, or asleep.
        if m == crate::game::ft::co::ids::ms::DEAD_UP_FALL_HIT_CAMERA_ICE
            || m == crate::game::ft::co::ids::ms::SLEEP
        {
            let src = jobj_of(r.game, o).expect("an owner without a model");
            let s = r.game.objs.jobjs[src].scale.y;
            let j = jobj_of(r.game, gobj).expect("a pill without a model");
            lib2725::set_uniform_scale(r.game, j, s);
        }
    }
    false
}
