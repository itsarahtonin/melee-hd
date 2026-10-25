//! A coin match's coins, `melee/it/kinds/it_2E5A.c` (`It_Kind_Unk4`): coins a fighter hit or KO'd
//! drops (or a broken item scatters) come out as bronze, silver and gold coins (the attributes'
//! three tiers), which bounce, settle and wait to be picked up by touch (`ft_8007C77C`) until
//! their lifetime runs out.

use super::super::item::{article, change, change_state, debug_owner_bubbles, spawn_air};
use super::super::{flag, hitbox, item, item_mut, jobj_of, lib26b1, lib2725, maplib, set_flag};
use super::foods::set_model;
use crate::fpu::{mem, reg};
use crate::game::handles::CodeRef;
use crate::game::it::CoinFlags;
use crate::game::state::Running;
use crate::game::structs::{_itECB, SpawnItem};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::lb::joint::world_position;
use crate::math;
use crate::vec::Vec3;
use gekko_fp as fp;
use melee_data::archive::DataRef;

pub const ANIM0: u32 = 0x802E_67A8;
pub const PHYS0: u32 = 0x802E_67F4;
pub const COLL0: u32 = 0x802E_6824;
pub const LANDED: u32 = 0x802E_6888;
pub const ANIM2: u32 = 0x802E_68EC;
pub const PHYS2: u32 = 0x802E_690C;
pub const COLL2: u32 = 0x802E_6910;
pub const DMG_DEALT: u32 = 0x802E_6A40;
pub const ACCESSORY: u32 = 0x802E_6A74;

/// `it_2E5A_Attrs`' offsets: the lifetime (`x0`), the waits before the others (`x4`) and the
/// owner (`x8`) may pick a coin up, the scatter's speeds (`xC` to `x24`), the pickup radius
/// (`x28`), the per-standing multipliers (`x2C`) and the three tiers (`x3C`, 0x2C bytes each).
const A_LIFETIME: u32 = 0x0;
const A_OWNER_WAIT: u32 = 0x4;
const A_OTHERS_WAIT: u32 = 0x8;
const A_XC: u32 = 0xC;
const A_X10: u32 = 0x10;
const A_LAND_X: u32 = 0x14;
const A_X18: u32 = 0x18;
const A_X1C: u32 = 0x1C;
const A_X20: u32 = 0x20;
const A_X24: u32 = 0x24;
const A_RADIUS: u32 = 0x28;
const A_MULT: u32 = 0x2C;
const A_TIERS: u32 = 0x3C;
const TIER: u32 = 0x2C;
/// A tier's offsets: its model, its state description (from its animated joint), its
/// `xD84`, the coins it's worth, its scale and its box.
const T_JOINT: u32 = 0x0;
const T_DESC: u32 = 0x4;
const T_XD84: u32 = 0x10;
const T_THRESHOLD: u32 = 0x14;
const T_SCALE: u32 = 0x18;
const T_ECB: u32 = 0x1C;

/// The coin's pickup sound (also its hit's).
const SFX_COIN: i32 = 147;

/// A scatter's settings (`it_802E5FXX_struct`): who made it, the angle and speed it flies at,
/// the coins left to drop, whether it flies, whether its coins wait to be picked up and the
/// dropper's standing.
struct Scatter {
    owner: Option<GObjId>,
    angle: f32,
    speed: f32,
    left: i32,
    flies: bool,
    pickable: i32,
    standing: i32,
}

/// Runs one of the coin's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 => anim0(r, gobj),
        COLL0 => coll0(r, gobj),
        ANIM2 => anim2(r, gobj),
        COLL2 => coll2(r, gobj),
        DMG_DEALT => dmg_dealt(r, gobj),
        _ => return None,
    })
}

/// Runs one of the coin's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PHYS0 => gravity(r, gobj),
        LANDED => landed(r, gobj),
        PHYS2 => still(r, gobj),
        ACCESSORY => follow_bone(r, gobj),
        _ => return false,
    }
    true
}

fn attrs(r: &Running, gobj: GObjId) -> DataRef {
    article(item(r.game, gobj))
        .special(r.data)
        .expect("a coin without attributes")
}

fn attr_f(r: &Running, gobj: GObjId, off: u32) -> f64 {
    reg(r.data.f32(attrs(r, gobj).at(off)))
}

fn tier(r: &Running, gobj: GObjId, t: i32) -> DataRef {
    attrs(r, gobj).at(A_TIERS + TIER * t as u32)
}

/// The coin's spawn description (`it_802E614C`): from `pos` at `vel`, parents `p1` and `p2`.
fn spawn_desc(p1: Option<GObjId>, p2: Option<GObjId>, pos: Vec3, vel: Vec3) -> SpawnItem {
    let mut s = SpawnItem {
        kind: super::UNK4,
        ..Default::default()
    };
    s.prev_pos = pos;
    s.pos = s.prev_pos;
    s.x3C_damage = 0;
    s.vel = vel;
    s.x0_parent_gobj = p1;
    s.x4_parent_gobj2 = p2;
    s.x44_flag[0] |= 0x80;
    s.x40 = 0;
    s
}

/// `count` coins scattered from item `from` at `pos` and `vel`, not flying on their own and
/// to be picked up at once, `it_802E5F00`.
#[doc(alias = "it_802E5F00")]
pub fn scatter(r: &mut Running, from: GObjId, pos: Vec3, vel: Vec3, count: i32) {
    let mut spawn = spawn_desc(None, Some(from), pos, vel);
    spawn.facing_dir = -1.0;
    let mut s = Scatter {
        owner: Some(from),
        angle: 0.0,
        speed: 0.0,
        left: count,
        flies: false,
        pickable: 1,
        standing: 0,
    };
    // Its count isn't returned: the original returns a register its caller left.
    drop_all(r, &mut s, &mut spawn);
}

/// `count` coins a fighter drops from `pos`, flying out at `angle` (from straight up) and
/// `speed`, `pickable` saying whether they wait to be picked up; how many coins they're worth,
/// `it_802E5F8C`.
#[doc(alias = "it_802E5F8C")]
pub fn drop_from(
    r: &mut Running,
    fighter: GObjId,
    pos: Vec3,
    count: i32,
    pickable: i32,
    angle: f32,
    speed: f32,
) -> i32 {
    let mut spawn = spawn_desc(Some(fighter), Some(fighter), pos, Vec3::new(0.0, 0.0, 0.0));
    let tau = std::f64::consts::TAU;
    let mut a = fp::frsp(fp::fadd(std::f64::consts::FRAC_PI_2, reg(angle)));
    while a < 0.0 {
        a = fp::frsp(fp::fadd(a, tau));
    }
    while a > tau {
        a = fp::frsp(fp::fsub(a, tau));
    }
    spawn.facing_dir = if a > std::f64::consts::PI { -1.0 } else { 1.0 };
    // `gm_8016C6C0`: the dropper's player's standing.
    let slot = i32::from(r.game.fighter(fighter).player_idx);
    let standing = r.player_rank(slot);
    let mut s = Scatter {
        owner: Some(fighter),
        angle: mem(a),
        speed,
        left: count,
        flies: true,
        pickable,
        standing,
    };
    drop_all(r, &mut s, &mut spawn)
}

/// Makes the scatter's coins until none are left, `it_802E609C`: the first coin made sets the
/// count a flying scatter is worth (by the dropper's standing and a random spread); each coin
/// takes the largest tier the count left allows. The worth (or, none made, the count).
#[doc(alias = "it_802E609C")]
fn drop_all(r: &mut Running, s: &mut Scatter, spawn: &mut SpawnItem) -> i32 {
    let mut worth = None;
    while s.left != 0 {
        limit_coins(r);
        let gobj = spawn_air(r, spawn)
            .unwrap_or_else(|| panic!("it_802E609C: no coin made (the original loops forever)"));
        if s.flies && worth.is_none() {
            s.left = adjusted_count(r, gobj, s.left, s.standing);
            worth = Some(s.left);
        }
        setup(r, gobj, s);
        start(r, gobj);
        debug_owner_bubbles(r.game, gobj, s.owner);
        lib2725::first_frame(r, gobj);
    }
    // With no coin to make the original returns the register its caller left: `it_802E5F8C`'s
    // count (its `r30`), the only caller that uses it.
    worth.unwrap_or(s.left)
}

/// The count a fighter's coins are worth, `it_802E61C4`: times the standing's multiplier, then
/// spread up to the attributes' percentage, at least one.
#[doc(alias = "it_802E61C4")]
pub fn adjusted_count(r: &mut Running, gobj: GObjId, count: i32, standing: i32) -> i32 {
    let a = attrs(r, gobj);
    let mult = reg(r.data.f32(a.at(A_MULT + 4 * standing as u32)));
    let n = fp::fctiwz(fp::fmuls(fp::frsp(f64::from(count)), mult));
    let rng = reg(r.game.objs.rng.draw_f32());
    let spread = fp::fmuls(attr_f(r, gobj, A_X24), rng);
    let n = fp::fctiwz(fp::fmuls(
        fp::frsp(f64::from(n)),
        fp::fmadds(0.009999999776482582, spread, 1.0),
    ));
    n.max(1)
}

/// The coin's speed out of a fighter at `angle` and `speed`, `it_802E628C`.
#[doc(alias = "it_802E628C")]
pub fn fly_out(r: &mut Running, gobj: GObjId, angle: f32, speed: f32) {
    let pi = std::f64::consts::PI;
    let rng = reg(r.game.objs.rng.draw_f32());
    let xc = attr_f(r, gobj, A_XC);
    let side = fp::frsp(fp::fsub(reg(angle), fp::fdiv(pi, 2.0)));
    let third = fp::fdivs(fp::frsp(pi), fp::frsp(3.0));
    let a = reg(angle);
    let mut vx = fp::fmuls(rng, xc);
    vx = fp::fmuls(vx, reg(item(r.game, gobj).facing_dir));
    if (a >= third && a <= fp::fmuls(2.0, third))
        || (a >= fp::fmuls(4.0, third) && a <= fp::fmuls(5.0, third))
    {
        let x10 = attr_f(r, gobj, A_X10);
        vx = fp::fmadds(x10, reg(math::cos(mem(side))), vx);
    }
    let (x20, x18) = (attr_f(r, gobj, A_X20), attr_f(r, gobj, A_X18));
    let vy = fp::fadds(x18, fp::fadds(fp::fmuls(rng, x20), reg(speed)));
    item_mut(r.game, gobj).x40_vel = Vec3::new(mem(vx), mem(vy), 0.0);
}

/// Sets a new coin up from its scatter, `it_802E6380`: the largest tier the count left allows
/// (and takes it off the count), its flight, lifetime, pickup waits, look, scale and box, and
/// for a coin to pick up, the bone its pickup follows; the coins left.
#[doc(alias = "it_802E6380")]
fn setup(r: &mut Running, gobj: GObjId, s: &mut Scatter) -> i32 {
    let a = attrs(r, gobj);
    let threshold = |t: u32| r.data.u32(a.at(A_TIERS + TIER * t + T_THRESHOLD)) as i32;
    let t = if s.left < threshold(2) {
        if s.left < threshold(1) { 0 } else { 1 }
    } else {
        2
    };
    item_mut(r.game, gobj).xDD4_itemVar.coin().x4.set(t);
    let worth = threshold(t as u32);
    s.left = s.left.wrapping_sub(worth);
    item_mut(r.game, gobj).xDD4_itemVar.coin().x0.set(worth);
    if s.flies {
        s.speed = mem(fp::fmuls(reg(s.speed), attr_f(r, gobj, A_X1C)));
        fly_out(r, gobj, s.angle, s.speed);
    }
    let life = r.data.f32(a.at(A_LIFETIME));
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    let (w4, w8) = (
        r.data.f32(a.at(A_OWNER_WAIT)),
        r.data.f32(a.at(A_OTHERS_WAIT)),
    );
    let td = tier(r, gobj, t);
    let (xd84, scale) = (r.data.u32(td.at(T_XD84)) as i32, r.data.f32(td.at(T_SCALE)));
    let joint = r.data.ptr(td.at(T_JOINT));
    let e = td.at(T_ECB);
    let ecb = _itECB {
        top: r.data.f32(e),
        bottom: r.data.f32(e.at(4)),
        right: r.data.f32(e.at(8)),
        left: r.data.f32(e.at(12)),
    };
    let ip = item_mut(r.game, gobj);
    let v = ip.xDD4_itemVar.coin();
    v.x10.set(w4);
    v.x14.set(w8);
    v.x8.set(s.pickable);
    ip.xDCD_flag.x0.b6 = true;
    ip.xD84 = xd84;
    ip.scl = scale;
    set_model(r, gobj, joint);
    maplib::set_box(r.game, gobj, ecb);
    let ip = item_mut(r.game, gobj);
    ip.xC0C = ip.xC1C.clone();
    ip.xC1C.bottom = mem(fp::fmuls(reg(ip.xC1C.bottom), reg(ip.scl)));
    ip.xDD4_itemVar.coin().xC.set(0);
    // The inline: not to be picked up yet, and the bone a pickup follows.
    ip.xDD4_itemVar.coin().x18.set(CoinFlags::default());
    if item_mut(r.game, gobj).xDD4_itemVar.coin().x8.get() != 0 {
        let radius = attr_f(r, gobj, A_RADIUS);
        let j = jobj_of(r.game, gobj);
        let ip = item_mut(r.game, gobj);
        let r0 = mem(fp::fdivs(
            fp::fmuls(0.0039059999398887157, radius),
            reg(ip.scl),
        ));
        let sub = &mut ip.xDD4_itemVar.coin().sub;
        sub.x4.set(j);
        sub.x0.set(r0);
        ip.x3C = r0;
        let p = world_position(&mut r.game.objs, r.data, j, None);
        let ip = item_mut(r.game, gobj);
        let sub = &mut ip.xDD4_itemVar.coin().sub;
        sub.x8.set(p);
        sub.x14.set(p);
        if r.game.it.db_menu.ShowCoinPickupRange {
            item_mut(r.game, gobj).xDAA[0] |= 0x80;
        }
    }
    s.left
}

/// Counts the coin's life down and its pickup waits, `it_802E657C`: whether it's gone.
#[doc(alias = "it_802E657C")]
pub fn tick(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    let t = reg(ip.xD44_lifeTimer);
    if t <= 0.0 {
        return true;
    }
    ip.xD44_lifeTimer = mem(fp::fsubs(t, 1.0));
    if ip.xDD4_itemVar.coin().x8.get() != 1 {
        return false;
    }
    let v = ip.xDD4_itemVar.coin();
    let w = reg(v.x10.get());
    if w <= 0.0 {
        if !v.x18.get().others {
            v.x18.update(|f| CoinFlags { others: true, ..f });
        }
    } else {
        v.x10.set(mem(fp::fsubs(w, 1.0)));
    }
    let w = reg(v.x14.get());
    if w <= 0.0 {
        if !ip.xDCD_flag.x0.b5 {
            hitbox::set_hit_ids(r.game, gobj);
            item_mut(r.game, gobj)
                .xDD4_itemVar
                .coin()
                .x18
                .update(|f| CoinFlags { owner: true, ..f });
        }
    } else {
        v.x14.set(mem(fp::fsubs(w, 1.0)));
    }
    false
}

/// Makes room for a coin, `it_802E6658`: near the coins' limit (the common data's `x20`, less
/// two), the last coin out goes.
#[doc(alias = "it_802E6658")]
pub fn limit_coins(r: &mut Running) {
    let limit = super::super::item::common_data(r.game).u32(r.data, 0x20);
    if r.game.it.counts.x50 as u32 >= limit.wrapping_sub(2) {
        let last = super::super::spawn::find_kind(r.game, super::UNK4);
        super::super::item::destroy(r, last.expect("it_802E6658: no coin to take off"));
    }
}

/// The tier's state description and animations from its start (`it_2E5A_ApplyStateDesc`).
fn apply_tier_desc(r: &mut Running, gobj: GObjId, t: i32) {
    let desc = tier(r, gobj, t).at(T_DESC);
    item_mut(r.game, gobj).xD0_itemStateDesc = Some(desc);
    super::super::item::set_state_anims(r, gobj, desc);
    let j = jobj_of(r.game, gobj);
    jobj::anim_all(
        &mut r.game.objs,
        r.data,
        &mut crate::game::code::ParticleHooks,
        j,
    );
}

/// Its state after the setup, `it_802E66A0`: a coin to pick up flies (state 0, its tier's look
/// once) with its bone followed; another falls (state 1).
#[doc(alias = "it_802E66A0")]
pub fn start(r: &mut Running, gobj: GObjId) {
    if item_mut(r.game, gobj).xDD4_itemVar.coin().x8.get() == 1 {
        fly_state(r, gobj);
        return;
    }
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    let t = item_mut(r.game, gobj).xDD4_itemVar.coin().x4.get();
    apply_tier_desc(r, gobj, t);
}

/// State 0, the tier's look the first time, the bone followed.
fn fly_state(r: &mut Running, gobj: GObjId) {
    if item_mut(r.game, gobj).xDD4_itemVar.coin().xC.get() != 0 {
        change_state(r, gobj, 0, change::HIT_PRESERVE | 1);
    } else {
        change_state(r, gobj, 0, change::ANIM_UPDATE);
        item_mut(r.game, gobj).xDD4_itemVar.coin().xC.set(1);
        let t = item_mut(r.game, gobj).xDD4_itemVar.coin().x4.get();
        apply_tier_desc(r, gobj, t);
    }
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(ACCESSORY));
}

/// `it_2E5A_UnkMotion1_Anim` (states 0 and 1): spinning faster the faster it falls, its life.
#[doc(alias = "it_2E5A_UnkMotion1_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    let vy = reg(ip.x40_vel.y);
    let abs = if vy < fp::frsp(0.0) { fp::fneg(vy) } else { vy };
    ip.x5D0_animFrameSpeed = mem(fp::frsp(fp::fmadd(0.1, abs, 1.0)));
    tick(r, gobj)
}

/// The coin's landing, `it_802E5AC4`: the pass down to the floor by its box's bottom, then on a
/// floor (with `bounce`) tiny speeds dropped, a slow coin stopped, a fast one bounced off the
/// floor (its bounce's sound, effect and hits); whether it settled.
#[doc(alias = "it_802E5AC4")]
pub fn lands(r: &mut Running, gobj: GObjId, bounce: bool) -> bool {
    maplib::prep_coll(r.game, r.data, gobj);
    let mut cd = item(r.game, gobj).x378_itemColl.clone();
    let bottom = reg(item(r.game, gobj).xC1C.bottom);
    cd.cur_pos.y = mem(fp::fsubs(reg(cd.cur_pos.y), bottom));
    let landed = maplib::with_stage_coll(r.game, r.data, |c| c.any_frame(&mut cd));
    cd.cur_pos.y = mem(fp::fadds(reg(cd.cur_pos.y), bottom));
    let ip = item_mut(r.game, gobj);
    ip.x378_itemColl = cd;
    ip.pos = ip.x378_itemColl.cur_pos;
    if !(landed && bounce) {
        return landed;
    }
    let ca = super::super::item::attr_of(item(r.game, gobj));
    let (keep, stop) = (
        reg(ca.f32(r.data, super::super::data::attr::X58)),
        reg(ca.f32(r.data, super::super::data::attr::X5C)),
    );
    let ip = item_mut(r.game, gobj);
    let abs = |v: f32| {
        let v = reg(v);
        if v < fp::frsp(0.0) { fp::fneg(v) } else { v }
    };
    let tiny = f64::from(1.0e-5_f32);
    if abs(ip.x40_vel.x) <= tiny {
        ip.x40_vel.x = 0.0;
    }
    if abs(ip.x40_vel.y) <= tiny {
        ip.x40_vel.y = 0.0;
    }
    let slow = abs(ip.x40_vel.x) <= stop && abs(ip.x40_vel.y) <= stop;
    if slow || ip.xDCD_flag.x0.b4 || keep == 0.0 {
        ip.x40_vel = Vec3::new(0.0, 0.0, 0.0);
        ip.xD50_landNum = 0;
        return landed;
    }
    let vel = ip.x40_vel;
    let cd = ip.x378_itemColl.clone();
    let mut motion = Vec3::new(0.0, 0.0, 0.0);
    let mut out = Vec3::new(0.0, 0.0, 0.0);
    let speed = reg(math::sqrt_accurate(mem(fp::fadds(
        fp::fmuls(reg(vel.x), reg(vel.x)),
        fp::fmuls(reg(vel.y), reg(vel.y)),
    ))));
    let n = cd.floor.normal;
    if fp::fmadds(reg(vel.x), reg(n.x), fp::fmuls(reg(vel.y), reg(n.y))) < 0.0 {
        let mut m = vel;
        m.mirror_xy(n);
        out = out.add_xy(m);
        let moving = maplib::with_stage_coll(r.game, r.data, |c| c.surface_speed(&cd, &cd.floor));
        if let Some(v) = moving {
            motion = motion.add_xy(v);
        }
        out.normalize_xy();
        let len = math::sqrt(mem(fp::fadds(
            fp::fmuls(reg(out.z), reg(out.z)),
            fp::fadds(
                fp::fmuls(reg(out.x), reg(out.x)),
                fp::fmuls(reg(out.y), reg(out.y)),
            ),
        )));
        if reg(len) < f64::from(0.01_f32) {
            out.x = vel.x;
            out.y = mem(fp::fmuls(fp::fneg(1.0), reg(vel.y)));
        }
        let k = fp::fmuls(speed, keep);
        out.x = mem(fp::fmuls(reg(out.x), k));
        out.y = mem(fp::fmuls(reg(out.y), k));
        let ip = item_mut(r.game, gobj);
        ip.x64_vec_unk2 = motion;
        ip.x40_vel = out;
    }
    if maplib::unstick(r.game, gobj, 1) {
        lib2725::bounce_sfx(r, gobj);
        maplib::bounce_effect(r, gobj, 0);
        hitbox::scale_damage(r.game, r.data, gobj, keep);
    }
    let xd84 = r.data.u32(tier(r, gobj, 0).at(T_XD84)) as i32;
    let land_x = attr_f(r, gobj, A_LAND_X);
    let ip = item_mut(r.game, gobj);
    if ip.xD50_landNum == 1 {
        ip.xD84 = xd84;
    }
    ip.x40_vel.x = mem(fp::fmuls(reg(ip.x40_vel.x), land_x));
    ip.xD50_landNum = ip.xD50_landNum.wrapping_add(1);
    if ip.xD50_landNum >= 3 && !flag(ip.xDCF_flag, 0) {
        set_flag(&mut ip.xDCF_flag, 0, true);
    }
    false
}

/// `it_2E5A_UnkMotion0_Coll` (states 0 and 1): once it settles, it rests.
#[doc(alias = "it_2E5A_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    if lands(r, gobj, true) {
        landed(r, gobj);
        let ip = item_mut(r.game, gobj);
        ip.xD50_landNum = 0;
        set_flag(&mut ip.xDCF_flag, 0, false);
    }
    false
}

/// Rests, `it_802E6888`: still, its bone followed if it's one to pick up.
#[doc(alias = "it_802E6888")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.x40_vel = Vec3::new(0.0, 0.0, 0.0);
    ip.x5D0_animFrameSpeed = 1.0;
    change_state(r, gobj, 2, change::HIT_PRESERVE | 1);
    if item_mut(r.game, gobj).xDD4_itemVar.coin().x8.get() == 1 {
        item_mut(r.game, gobj).on_accessory = Some(CodeRef(ACCESSORY));
    }
}

/// `it_2E5A_UnkMotion2_Coll`: a coin to pick up that's lost its floor flies again.
#[doc(alias = "it_2E5A_UnkMotion2_Coll")]
pub fn coll2(r: &mut Running, gobj: GObjId) -> bool {
    if item_mut(r.game, gobj).xDD4_itemVar.coin().x8.get() == 1 && !lands(r, gobj, false) {
        start(r, gobj);
    }
    false
}

/// `it_2E5A_UnkMotion2_Anim`: its life.
#[doc(alias = "it_2E5A_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    tick(r, gobj)
}

/// `it_2E5A_UnkMotion1_Phys` (states 0 and 1): gravity.
#[doc(alias = "it_2E5A_UnkMotion1_Phys")]
pub fn gravity(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
}

/// `it_2E5A_UnkMotion2_Phys`: nothing.
#[doc(alias = "it_2E5A_UnkMotion2_Phys")]
pub fn still(_: &mut Running, _: GObjId) {}

/// Its hit: the pickup sound, and it goes, `it_2E5A_Logic115_DmgDealt`.
#[doc(alias = "it_2E5A_Logic115_DmgDealt")]
pub fn dmg_dealt(r: &mut Running, _gobj: GObjId) -> bool {
    r.sfx_play_track(SFX_COIN, 127, 64, 26);
    true
}

/// The bone its pickup follows, last frame's and now, `it_802E6A74`.
#[doc(alias = "it_802E6A74")]
pub fn follow_bone(r: &mut Running, gobj: GObjId) {
    if item_mut(r.game, gobj).xDD4_itemVar.coin().x8.get() == 0 {
        return;
    }
    let sub = &mut item_mut(r.game, gobj).xDD4_itemVar.coin().sub;
    let now = sub.x8.get();
    sub.x14.set(now);
    let j = sub.x4.get();
    let p = world_position(&mut r.game.objs, r.data, j, None);
    item_mut(r.game, gobj).xDD4_itemVar.coin().sub.x8.set(p);
}

/// How many coins the coin is worth, `it_802E5EF4`.
#[doc(alias = "it_802E5EF4")]
pub fn worth(r: &mut Running, gobj: GObjId) -> i32 {
    item_mut(r.game, gobj).xDD4_itemVar.coin().x0.get()
}

/// Forgets a GObj the coin points to, `it_2E5A_Logic115_EvtUnk` (`it_8026B894`).
// Ports `it_2E5A_Logic115_EvtUnk`; aliased once a check reaches it (nothing dispatches the
// items' `evt_unk` yet).
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
