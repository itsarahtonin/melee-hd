//! The beam sword, `melee/it/kinds/itsword.c`: a battering item whose blade grows and shrinks
//! (three eased values scale its joints) as it's picked up, swung and let go of.

use gekko_fp as fp;

use super::super::ground::{fall, ground_or_fall, ground_skid};
use super::super::item::{article, attr_of, change, change_state, common_data};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::state::{Game, Running};
use crate::game::structs::{SpawnItem, itSword_ItemVars};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::vec::Vec3;
use melee_data::archive::{Archives, DataRef};

pub const SPAWNED: u32 = 0x8028_5338;
pub const LANDED: u32 = 0x8028_5424;
pub const ANIM0: u32 = 0x8028_5580;
pub const PHYS0: u32 = 0x8028_55C8;
pub const COLL0: u32 = 0x8028_55CC;
pub const FELL: u32 = 0x8028_55F8;
pub const ANIM3: u32 = 0x8028_5744;
pub const PHYS3: u32 = 0x8028_578C;
pub const COLL3: u32 = 0x8028_57D8;
pub const PICKED_UP: u32 = 0x8028_5804;
pub const ANIM2: u32 = 0x8028_5920;
pub const PHYS2: u32 = 0x8028_5AF4;
pub const COLL2: u32 = 0x8028_5AF8;
pub const DROPPED: u32 = 0x8028_5B00;
pub const THROWN: u32 = 0x8028_5C78;
pub const ENTERED_AIR: u32 = 0x8028_5DF0;
pub const ANIM4: u32 = 0x8028_5F2C;
pub const PHYS4: u32 = 0x8028_5F74;
pub const COLL4: u32 = 0x8028_5F78;
pub const DMG_DEALT: u32 = 0x8028_5FAC;
pub const REFLECTED: u32 = 0x8028_5FE0;
pub const CLANKED: u32 = 0x8028_6000;
pub const HIT_SHIELD: u32 = 0x8028_6024;
pub const SHIELD_BOUNCED: u32 = 0x8028_6048;
/// The Home-Run Bat's render callback (`it_803F1418`'s entry for kind 11), in this unit.
pub const BAT_RENDER: u32 = 0x8028_4E10;

/// Runs one of the sword's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM3 | ANIM4 => ease(r, gobj),
        ANIM2 => anim2(r, gobj),
        COLL2 => still(r, gobj),
        COLL0 => coll0(r, gobj),
        COLL3 => coll3(r, gobj),
        COLL4 => coll4(r, gobj),
        DMG_DEALT => dmg_dealt(r, gobj),
        REFLECTED => reflected(r, gobj),
        CLANKED | HIT_SHIELD => bounce(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        _ => return None,
    })
}

/// Runs one of the sword's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        LANDED => landed(r, gobj),
        PHYS0 | PHYS2 | PHYS4 => idle(r, gobj),
        FELL => fell(r, gobj),
        PHYS3 => phys3(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED | THROWN => dropped(r, gobj),
        ENTERED_AIR => entered_air(r, gobj),
        _ => return false,
    }
    true
}

/// The sword's variables.
fn vars(game: &mut Game, gobj: GObjId) -> &mut itSword_ItemVars {
    item_mut(game, gobj).xDD4_itemVar.sword()
}

/// `itSword_UnkArticle1`: the blade's swung lengths (`x10`, `x14`) and the frames it takes
/// (`x18`).
fn attrs(r: &Running, gobj: GObjId) -> (f32, f32, i32) {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a sword without attributes");
    (
        r.data.f32(a.at(0x10)),
        r.data.f32(a.at(0x14)),
        r.data.u32(a.at(0x18)) as i32,
    )
}

/// `(to - from) / frames`.
fn step(to: f32, from: f32, frames: i32) -> f32 {
    mem(fp::fdivs(
        fp::fsubs(reg(to), reg(from)),
        fp::frsp(f64::from(frames)),
    ))
}

/// The blade's length eases toward `to` over `frames` from the start, `it_80284FC4`.
#[doc(alias = "it_80284FC4")]
pub fn grow(game: &mut Game, gobj: GObjId, frames: i32, to: f32) {
    let v = vars(game, gobj);
    v.x4.set(0);
    v.x8.set(frames);
    v.xC.set(1);
    v.x10.set(1.0);
    v.x14.set(to);
    v.x18.set(step(v.x14.get(), v.x10.get(), frames));
}

/// The blade's length eases back to 1 over `frames`, `it_80285024` (`inlineA1` with 4).
#[doc(alias = "it_80285024")]
pub fn shrink(game: &mut Game, gobj: GObjId, frames: i32) {
    let v = vars(game, gobj);
    v.x4.set(frames);
    v.x8.set(0);
    v.xC.set(-1);
    v.x14.set(1.0);
    v.x18.set(step(v.x14.get(), v.x10.get(), v.x4.get()));
}

/// One eased value: a count stepping toward its end, and a value stepping toward its target.
fn ease_one(
    count: &mut i32,
    end: i32,
    dir: &mut i32,
    value: &mut f32,
    target: f32,
    delta: &mut f32,
) {
    *count = count.wrapping_add(*dir);
    if *dir > 0 {
        if *count > end {
            *count = end;
            *dir = 0;
        }
    } else if *dir < 0 && *count < end {
        *count = end;
        *dir = 0;
    }
    *value = mem(fp::fadds(reg(*value), reg(*delta)));
    if reg(*delta) > 0.0 {
        if reg(*value) > reg(target) {
            *value = target;
            *delta = 0.0;
        }
    } else if reg(*delta) < 0.0 && reg(*value) < reg(target) {
        *value = target;
        *delta = 0.0;
    }
}

/// Steps the blade's length, `it_80285084`.
#[doc(alias = "it_80285084")]
pub fn ease_length(game: &mut Game, gobj: GObjId) {
    let v = vars(game, gobj);
    let (end, target) = (v.x8.get(), v.x14.get());
    let (mut c, mut d, mut x, mut dx) = (v.x4.get(), v.xC.get(), v.x10.get(), v.x18.get());
    ease_one(&mut c, end, &mut d, &mut x, target, &mut dx);
    v.x4.set(c);
    v.xC.set(d);
    v.x10.set(x);
    v.x18.set(dx);
}

/// Steps the blade's width, `it_80285140`.
#[doc(alias = "it_80285140")]
pub fn ease_width(game: &mut Game, gobj: GObjId) {
    let v = vars(game, gobj);
    let (end, target) = (v.x20.get(), v.x2C.get());
    let (mut c, mut d, mut x, mut dx) = (v.x1C.get(), v.x24.get(), v.x28.get(), v.x30.get());
    ease_one(&mut c, end, &mut d, &mut x, target, &mut dx);
    v.x1C.set(c);
    v.x24.set(d);
    v.x28.set(x);
    v.x30.set(dx);
}

/// Steps the swing's stretch, `it_802851FC`.
#[doc(alias = "it_802851FC")]
pub fn ease_swing(game: &mut Game, gobj: GObjId) {
    let v = vars(game, gobj);
    let (end, target) = (v.x38.get(), v.x44.get());
    let (mut c, mut d, mut x, mut dx) = (v.x34.get(), v.x3C.get(), v.x40.get(), v.x48.get());
    ease_one(&mut c, end, &mut d, &mut x, target, &mut dx);
    v.x34.set(c);
    v.x3C.set(d);
    v.x40.set(x);
    v.x48.set(dx);
}

/// Scales the blade's joints from the eased values, `it_80284E30`.
#[doc(alias = "it_80284E30")]
pub fn scale_blade(r: &mut Running, gobj: GObjId) {
    let msid = item(r.game, gobj).msid;
    let t = item(r.game, gobj)
        .xBBC_dynamicBoneTable
        .expect("a sword without a bone table");
    let bones = r.game.it.bone_tables[t.0].bones;
    let v = vars(r.game, gobj);
    let y = fp::fmuls(
        f64::from(0.3_f32),
        fp::fmuls(
            reg(v.x28.get()),
            fp::fmuls(reg(v.x4C.get()), reg(v.x10.get())),
        ),
    );
    let scale = if msid == 2 {
        let xz = mem(fp::fmadds(1.5, reg(v.x40.get()), y));
        Vec3::new(xz, mem(fp::fadds(y, reg(v.x40.get()))), xz)
    } else {
        Vec3::new(1.0, mem(y), 1.0)
    };
    let jobjs = &mut r.game.objs.jobjs;
    let b6 = bones[6].expect("bone 6");
    jobj::set_srt(jobjs, b6, |o| o.scale.y = mem(y));
    jobj::set_scale(jobjs, bones[3].expect("bone 3"), scale);
}

/// The width eases back to 1 over 4 frames (`inlineA2`).
fn narrow(game: &mut Game, gobj: GObjId) {
    let v = vars(game, gobj);
    v.x1C.set(4);
    v.x20.set(0);
    v.x24.set(-1);
    v.x2C.set(1.0);
    v.x30.set(step(v.x2C.get(), v.x28.get(), v.x1C.get()));
}

/// The width eases from 1 to 2 over 4 frames (`inlineC1`).
fn widen(game: &mut Game, gobj: GObjId) {
    let v = vars(game, gobj);
    v.x1C.set(0);
    v.x20.set(4);
    v.x24.set(1);
    v.x28.set(1.0);
    v.x2C.set(2.0);
    v.x30.set(step(v.x2C.get(), v.x28.get(), 4));
}

/// The swing eases back to its rest length (`inlineA3`).
fn unswing(r: &mut Running, gobj: GObjId) {
    let (rest, _, frames) = attrs(r, gobj);
    let v = vars(r.game, gobj);
    v.x34.set(frames);
    v.x38.set(0);
    v.x3C.set(-1);
    v.x44.set(rest);
    v.x48.set(step(rest, v.x40.get(), frames));
}

/// The swing eases out to its full length (`inlineD1`).
fn swing(r: &mut Running, gobj: GObjId) {
    let (rest, full, frames) = attrs(r, gobj);
    let v = vars(r.game, gobj);
    v.x34.set(0);
    v.x38.set(frames);
    v.x3C.set(1);
    v.x40.set(rest);
    v.x44.set(full);
    v.x48.set(step(full, rest, frames));
}

/// Steps every eased value and scales the blade (`inlineA4`); the states' animation,
/// `itSword_UnkMotion0_Anim` (and 3, 4).
#[doc(alias = "itSword_UnkMotion0_Anim")]
#[doc(alias = "itSword_UnkMotion3_Anim")]
#[doc(alias = "itSword_UnkMotion4_Anim")]
pub fn ease(r: &mut Running, gobj: GObjId) -> bool {
    ease_length(r.game, gobj);
    ease_width(r.game, gobj);
    ease_swing(r.game, gobj);
    scale_blade(r, gobj);
    false
}

/// Everything eases back to rest (`inlineA1` to `A4`).
fn rest(r: &mut Running, gobj: GObjId) {
    shrink(r.game, gobj, 4);
    narrow(r.game, gobj);
    unswing(r, gobj);
    ease(r, gobj);
}

/// `itSword_Logic12_Spawned`: popping up, falling, its blade at rest.
#[doc(alias = "itSword_Logic12_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    lib26b1::enable_pickup(r.game, gobj);
    let vy = {
        let a = article(item(r.game, gobj))
            .special(r.data)
            .expect("a sword without attributes");
        r.data.f32(a.at(0xC))
    };
    item_mut(r.game, gobj).x40_vel = Vec3::new(0.0, vy, 0.0);
    let t = item(r.game, gobj)
        .xBBC_dynamicBoneTable
        .expect("a sword without a bone table");
    let b6 = r.game.it.bone_tables[t.0].bones[6].expect("bone 6");
    let y = r.game.objs.jobjs[b6].scale.y;
    let v = vars(r.game, gobj);
    v.x0.set(0);
    v.x4.set(0);
    v.x8.set(0);
    v.xC.set(0);
    v.x10.set(1.0);
    v.x14.set(1.0);
    v.x18.set(0.0);
    v.x1C.set(0);
    v.x20.set(0);
    v.x24.set(0);
    v.x28.set(1.0);
    v.x2C.set(1.0);
    v.x30.set(0.0);
    v.x34.set(0);
    v.x38.set(0);
    v.x3C.set(0);
    v.x40.set(1.0);
    v.x44.set(1.0);
    v.x48.set(0.0);
    v.x50.set(0.0);
    v.x4C.set(y);
    v.x54.set(0);
    fell(r, gobj);
}

/// Lands, stopped, `it_80285424`.
#[doc(alias = "it_80285424")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.x40_vel = Vec3::new(0.0, 0.0, 0.0);
    ip.xD50_landNum = 0;
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    rest(r, gobj);
}

/// Falls, `it_802855F8`.
#[doc(alias = "it_802855F8")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    item_mut(r.game, gobj).xD50_landNum = 0;
    rest(r, gobj);
}

/// `itSword_UnkMotion2_Coll`: held, nothing.
#[doc(alias = "itSword_UnkMotion2_Coll")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states that move nothing, `itSword_UnkMotion0_Phys` (2, 4).
#[doc(alias = "itSword_UnkMotion0_Phys")]
#[doc(alias = "itSword_UnkMotion2_Phys")]
#[doc(alias = "itSword_UnkMotion4_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// `itSword_UnkMotion0_Coll`: falls off ledges.
#[doc(alias = "itSword_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    false
}

/// `itSword_UnkMotion3_Phys`: falling and spinning (`Item_ApplyFallingPhysics`).
#[doc(alias = "itSword_UnkMotion3_Phys")]
pub fn phys3(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, super::super::data::attr::FALL_SPEED);
    let max = a.f32(r.data, super::super::data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
    let x68 = common_data(r.game).f32(r.data, super::super::data::common::X68);
    lib2725::clamp_fall(r.game, gobj, x68);
}

/// `itSword_UnkMotion3_Coll`: lands.
#[doc(alias = "itSword_UnkMotion3_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, landed);
    false
}

fn show(r: &mut Running, gobj: GObjId) {
    let child = jobj_of(r.game, gobj).and_then(|j| r.game.objs.jobjs[j].child);
    jobj::clear_flags_all(&mut r.game.objs.jobjs, child, jobj::HIDDEN);
}

/// Shown and held, its blade widening, `itSword_Logic12_PickedUp`.
#[doc(alias = "itSword_Logic12_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    show(r, gobj);
    change_state(r, gobj, 2, change::ANIM_UPDATE);
    item_mut(r.game, gobj).xD50_landNum = 0;
    shrink(r.game, gobj, 4);
    widen(r.game, gobj);
    ease(r, gobj);
}

/// Whether a fighter swings a sword (motion states 120 to 123), `ftLib_IsSwordSwing`.
#[doc(alias = "ftLib_IsSwordSwing")]
pub fn is_sword_swing(game: &Game, fighter: GObjId) -> bool {
    (120..=123).contains(&super::super::owner::fighter(game, fighter).motion_id)
}

/// Held: the blade shrinks unless swung, and the swing stretches out and back,
/// `itSword_UnkMotion2_Anim`.
#[doc(alias = "itSword_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    let owner = item(r.game, gobj).owner;
    if let Some(o) = owner
        && !is_sword_swing(r.game, o)
    {
        shrink(r.game, gobj, 4);
    }
    let (_, _, frames) = attrs(r, gobj);
    let v = vars(r.game, gobj);
    if v.x3C.get() == 0 && v.x34.get() == v.x38.get() {
        if v.x38.get() == 0 {
            swing(r, gobj);
        } else if v.x38.get() == frames {
            unswing(r, gobj);
        }
    }
    ease(r, gobj)
}

/// Let go of or thrown, it falls, its blade at rest, `itSword_Logic12_Dropped` and
/// `itSword_Logic12_Thrown`.
#[doc(alias = "itSword_Logic12_Dropped")]
#[doc(alias = "itSword_Logic12_Thrown")]
pub fn dropped(r: &mut Running, gobj: GObjId) {
    lib26b1::enable_pickup(r.game, gobj);
    show(r, gobj);
    change_state(r, gobj, 3, change::ANIM_UPDATE | change::DROP_UPDATE);
    rest(r, gobj);
}

/// Skidding off a slope, `itSword_Logic12_EnteredAir`.
#[doc(alias = "itSword_Logic12_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 4, change::ANIM_UPDATE);
    rest(r, gobj);
}

/// `itSword_UnkMotion4_Coll`: skidding, it stops or falls.
#[doc(alias = "itSword_UnkMotion4_Coll")]
pub fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, landed, fell);
    false
}

/// Bounces off what it hit when thrown, `itSword_Logic12_DmgDealt`.
#[doc(alias = "itSword_Logic12_DmgDealt")]
pub fn dmg_dealt(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).msid == 3 {
        lib2725::bounce_off_victim(r.game, r.data, gobj);
    }
    false
}

/// Sent back by a reflection, `itSword_Logic12_Reflected`.
#[doc(alias = "itSword_Logic12_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj)
}

/// Bounces off a clank or a shield, `itSword_Logic12_Clanked` and `itSword_Logic12_HitShield`.
#[doc(alias = "itSword_Logic12_Clanked")]
#[doc(alias = "itSword_Logic12_HitShield")]
pub fn bounce(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_victim(r.game, r.data, gobj);
    false
}

/// Bounces off a shield, `itSword_Logic12_ShieldBounced`.
#[doc(alias = "itSword_Logic12_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_shield(r.game, gobj)
}

/// Drops a sword at `pos` (on the stage's plane), landed, `itSword_Spawn`.
#[doc(alias = "itSword_Spawn")]
pub fn spawn_at(r: &mut Running, pos: Vec3) -> Option<GObjId> {
    // `Item_InitSpawnPosition` on the plane.
    let at = Vec3::new(pos.x, pos.y, 0.0);
    let mut spawn = SpawnItem {
        kind: super::SWORD,
        pos: at,
        prev_pos: at,
        ..Default::default()
    };
    // `Item_InitSpawnCommonFields`, with a first collision.
    spawn.facing_dir = -1.0;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = None;
    spawn.x4_parent_gobj2 = None;
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let gobj = super::super::item::spawn_ground(r, &mut spawn)?;
    landed(r, gobj);
    Some(gobj)
}

impl Running<'_> {
    /// The Home-Run Bat's render callback (`it_80284E10`, though it's in the sword's unit):
    /// the common one (`it_8026EECC`). Nothing ported makes a bat in a match (only the item
    /// switch's drops do, `it_8026BE84`), so no check reaches it; aliased once one does.
    pub fn render_bat(&mut self, gobj: GObjId, pass: i32) {
        self.render_item(gobj, pass);
    }
}

/// The trail's two widths for a fighter swinging it, `it_802852B8`: from the hilt, and the
/// blade's length (`10.03 × 0.3 × x4C × x10 × x28`, bone 6's scale).
#[doc(alias = "it_802852B8")]
pub fn trail_widths(game: &mut Game, gobj: GObjId) -> (f32, f32) {
    let v = vars(game, gobj);
    let f1 = fp::fmuls(
        f64::from(0.3_f32),
        fp::fmuls(
            reg(v.x28.get()),
            fp::fmuls(reg(v.x4C.get()), reg(v.x10.get())),
        ),
    );
    (0.0, mem(fp::fmuls(f64::from(10.03_f32), f1)))
}

/// The trail's look (`itSword_UnkBytes`, at its attributes' `x1C`), `it_80285300`.
#[doc(alias = "it_80285300")]
pub fn trail_params(game: &Game, data: &Archives, gobj: GObjId) -> DataRef {
    article(item(game, gobj))
        .special(data)
        .expect("a sword without attributes")
        .at(0x1C)
}

/// The joint the trail follows, bone 5, `it_80285314`.
#[doc(alias = "it_80285314")]
pub fn trail_joint(game: &Game, data: &Archives, gobj: GObjId) -> Option<JObjId> {
    lib2725::bone(game, data, gobj, 5)
}

/// Forgets a GObj it points to, `itSword_Logic12_EvtUnk` (`it_8026B894`).
#[doc(alias = "itSword_Logic12_EvtUnk")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
