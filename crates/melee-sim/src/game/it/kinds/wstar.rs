//! The Warp Star, `melee/it/kinds/itwstar.c`: it falls, then rests spinning; picked up, it
//! carries its rider off and up (the fighter's Warp Star states, `ft::co::warpstar`), comes
//! down from the top with it (state 3) and crashes where it lands, its blast an attack of its
//! rider's (state 5, until it's gone).

use super::super::ground::{fall, ground_or_fall, ground_skid};
use super::super::item::{article, change, change_state};
use super::super::{effect, item, item_mut, lib26b1, lib2725, set_flag};
use crate::game::ef::SpawnKind;
use crate::game::ef::eflib;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const SPAWNED: u32 = 0x8029_451C;
pub const LANDED: u32 = 0x8029_455C;
pub const ANIM0: u32 = 0x8029_45AC;
pub const PHYS0: u32 = 0x8029_45B4;
pub const COLL0: u32 = 0x8029_45F8;
pub const FELL: u32 = 0x8029_4624;
pub const ANIM1: u32 = 0x8029_464C;
pub const PHYS1: u32 = 0x8029_4654;
pub const COLL1: u32 = 0x8029_4684;
pub const PICKED_UP: u32 = 0x8029_4728;
pub const ANIM3: u32 = 0x8029_4788;
pub const DROPPED: u32 = 0x8029_4790;
pub const ENTERED_AIR: u32 = 0x8029_4A28;
pub const ANIM4: u32 = 0x8029_4A50;
pub const PHYS4: u32 = 0x8029_4A58;
pub const COLL4: u32 = 0x8029_4A5C;
pub const ANIM5: u32 = 0x8029_49C0;
pub const PHYS5: u32 = 0x8029_4A1C;
pub const COLL5: u32 = 0x8029_4A20;

/// Runs one of the Warp Star's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM1 | ANIM3 | ANIM4 => still(r, gobj),
        COLL0 => coll0(r, gobj),
        COLL1 => coll1(r, gobj),
        COLL4 => coll4(r, gobj),
        ANIM5 => crashed(r, gobj),
        COLL5 => still(r, gobj),
        _ => return None,
    })
}

/// Runs one of the Warp Star's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        LANDED => landed(r, gobj),
        PHYS0 => resting(r, gobj),
        FELL => fell(r, gobj),
        PHYS1 => gravity(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED => dropped(r, gobj),
        ENTERED_AIR => entered_air(r, gobj),
        PHYS4 | PHYS5 => idle(r, gobj),
        _ => return false,
    }
    true
}

/// `itWStar_Logic29_Spawned`: its ride's scales at 1, falling.
#[doc(alias = "itWStar_Logic29_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    set_flag(&mut ip.xDCE_flag, 7, false);
    let v = ip.xDD4_itemVar.wstar();
    v.xDD8.set(1.0);
    v.xDD4.set(1.0);
    fell(r, gobj);
}

/// Stops and stands, `it_8029455C`.
#[doc(alias = "it_8029455C")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    lib2725::clear_velocity(r.game, gobj);
    lib26b1::enable_pickup(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// The states that animate nothing, `itWstar_UnkMotion0_Anim` (1, 3 for 2 and 3, 4).
#[doc(alias = "itWstar_UnkMotion0_Anim")]
#[doc(alias = "itWstar_UnkMotion5_Coll")]
#[doc(alias = "itWstar_UnkMotion1_Anim")]
#[doc(alias = "itWstar_UnkMotion3_Anim")]
#[doc(alias = "itWstar_UnkMotion4_Anim")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// Moves nothing, `itWstar_UnkMotion4_Phys` (and the crash's).
#[doc(alias = "itWstar_UnkMotion4_Phys")]
#[doc(alias = "itWstar_UnkMotion5_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// `itWstar_UnkMotion0_Phys`: resting, it spins at its attributes' rate.
#[doc(alias = "itWstar_UnkMotion0_Phys")]
pub fn resting(r: &mut Running, gobj: GObjId) {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a Warp Star without attributes");
    let rate = r.data.f32(a);
    lib2725::clamp_fall(r.game, gobj, rate);
    lib2725::spin(r.game, r.data, gobj);
}

/// `itWstar_UnkMotion0_Coll`: falls off ledges.
#[doc(alias = "itWstar_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    false
}

/// Falls, `it_80294624`.
#[doc(alias = "it_80294624")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// Gravity, `itWstar_UnkMotion1_Phys`.
#[doc(alias = "itWstar_UnkMotion1_Phys")]
pub fn gravity(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
}

/// `itWstar_UnkMotion1_Coll`: lands.
#[doc(alias = "itWstar_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, landed);
    false
}

/// Picked up, with its glow on bone 3, `it_3F14_Logic29_PickedUp`.
#[doc(alias = "it_3F14_Logic29_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
    let j = lib2725::bone(r.game, r.data, gobj, 3);
    effect::queue(r, gobj, SpawnKind::Attach, 0x43D, j, &[]);
}

/// Let go of, it falls, its effects gone, `itWStar_Logic29_Dropped`.
#[doc(alias = "itWStar_Logic29_Dropped")]
pub fn dropped(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE | change::DROP_UPDATE);
    eflib::destroy_all(r, gobj);
}

/// Skidding off a slope, `itWStar_Logic29_EnteredAir`.
#[doc(alias = "itWStar_Logic29_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 4, change::ANIM_UPDATE);
}

/// `itWstar_UnkMotion4_Coll`: skidding, it stops or falls.
#[doc(alias = "itWstar_UnkMotion4_Coll")]
pub fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, landed, fell);
    false
}

/// `itWstarAttributes`' word at `off`.
fn attr(r: &Running, gobj: GObjId, off: u32) -> melee_data::archive::DataRef {
    article(item(r.game, gobj))
        .special(r.data)
        .expect("a Warp Star without attributes")
        .at(off)
}

/// The ride's animation for its rider, `it_80294364`: a random one of the attributes'
/// `x24_count`, never the last one picked (`it_804D6D00`), with its sound.
#[doc(alias = "it_80294364")]
pub fn ride_anim(r: &mut Running, gobj: GObjId) -> Option<melee_data::archive::DataRef> {
    let count = r.data.u32(attr(r, gobj, 0x24)) as i32;
    let last = i32::from(r.game.it.x6d00);
    let candidates: Vec<i32> = (0..count).filter(|&i| i != last).collect();
    let n = candidates.len() as i32;
    let picked = candidates[r.game.objs.rng.below(n) as usize];
    r.game.it.x6d00 = picked as i8;
    item_mut(r.game, gobj).xDD4_itemVar.wstar().xDDC.set(picked);
    let i = item_mut(r.game, gobj).xDD4_itemVar.wstar().xDDC.get();
    let entries = attr(r, gobj, 0x28);
    let sfx = r.data.u32(entries.at(8 * i as u32 + 4)) as i32;
    super::super::item::play_sfx(r, gobj, sfx, 0x7F, 0x40);
    r.data.ptr(entries.at(8 * picked as u32))
}

/// Sizes the ride to its rider, `it_80294430`: the star scaled by the rider's `height` over
/// the attributes' `x4` (times `x8` and the item's scale), and its hitboxes by `hit_scale`.
#[doc(alias = "it_80294430")]
pub fn size_for_rider(r: &mut Running, gobj: GObjId, height: f32, hit_scale: f32) {
    use crate::fpu::{mem, reg};
    use gekko_fp as fp;
    let (x4, x8) = (r.data.f32(attr(r, gobj, 4)), r.data.f32(attr(r, gobj, 8)));
    let item_scale = super::super::item::attr_of(item(r.game, gobj))
        .f32(r.data, super::super::data::attr::SCALE);
    let j = lib2725::attach_bone(r.game, r.data, gobj).expect("a Warp Star without its bone");
    let speed = fp::fmuls(reg(x8), fp::fdivs(reg(height), reg(x4)));
    let speed = mem(fp::fmuls(speed, reg(item_scale)));
    item_mut(r.game, gobj).xDD4_itemVar.wstar().xDD4.set(speed);
    lib2725::set_uniform_scale(r.game, j, speed);
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .wstar()
        .xDD8
        .set(hit_scale);
}

/// The ride's collision box at the star's size, `it_802944AC`.
#[doc(alias = "it_802944AC")]
pub fn ride_box(r: &mut Running, gobj: GObjId) -> crate::game::structs::ftCollisionBox {
    use crate::fpu::{mem, reg};
    use gekko_fp as fp;
    let s = reg(item_mut(r.game, gobj).xDD4_itemVar.wstar().xDD4.get());
    let f = |off: u32| mem(fp::fmuls(reg(r.data.f32(attr(r, gobj, off))), s));
    let mut b = crate::game::structs::ftCollisionBox::default();
    b.top = f(0xC);
    b.bottom = f(0x10);
    b.left.x = f(0x14);
    b.left.y = f(0x18);
    b.right.x = f(0x1C);
    b.right.y = f(0x20);
    b
}

/// Coming down with its rider, `it_802946B0`: state 3, sized and its hitboxes scaled, an
/// attack of its own (flags 0x44005F, id 0x5E).
#[doc(alias = "it_802946B0")]
pub fn descend(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 3, change::ANIM_UPDATE);
    let j = lib2725::attach_bone(r.game, r.data, gobj).expect("a Warp Star without its bone");
    let v = item_mut(r.game, gobj).xDD4_itemVar.wstar();
    let size = v.xDD4.get();
    lib2725::set_uniform_scale(r.game, j, size);
    let hit = item_mut(r.game, gobj).xDD4_itemVar.wstar().xDD8.get();
    super::super::hitbox::scale_hitboxes(r.game, gobj, crate::fpu::reg(hit));
    set_attack_flags(r, gobj, 0x0044_005F);
    set_attack_id(r, gobj, 0x5E);
}

/// The item's attack flags `flags`, `it_8027B288`: a new record unless its last byte is the
/// same nonzero one, and the record of its fighter owner's attack, else a cleared one.
#[doc(alias = "it_8027B288")]
pub fn set_attack_flags(r: &mut Running, gobj: GObjId, flags: u32) {
    let b = flags.to_be_bytes();
    if b[3] == 0 || b[3] != item(r.game, gobj).xD90[3] {
        let rec = r.game.next_attack_record();
        item_mut(r.game, gobj).xDA8_short = rec;
    }
    item_mut(r.game, gobj).xD90 = b;
    let owner = item(r.game, gobj).owner;
    if super::super::item::is_fighter(r.game, owner) {
        let f = super::super::owner::fighter_of(r.game, owner.expect("an owner"));
        let rec = r.game.fighters[f.0].x2074.clone();
        let ip = item_mut(r.game, gobj);
        ip.xD94 = rec.x2074_vec;
        ip.xD9C = rec.x207C;
        ip.xDA4_word = u32::from_be_bytes(rec.x10);
        return;
    }
    super::super::lib279c::clear_attack_record(item_mut(r.game, gobj));
}

/// The item's attack id `id`, `it_8027B330`: a new attack instance for id 1 or a new id.
#[doc(alias = "it_8027B330")]
pub fn set_attack_id(r: &mut Running, gobj: GObjId, id: i32) {
    if id == 1 || id != item(r.game, gobj).xD88_attackID {
        item_mut(r.game, gobj).xD88_attackID = id;
        let inst = r.game.next_attack_instance();
        item_mut(r.game, gobj).xD8C_attack_instance = inst;
    }
}

/// The crash where its rider lands at `pos`, `it_802947CC`: stopped, state 5, let go of
/// (`it_8027429C`, its attack kept), no pickup, hidden at `pos`, vanishing, its effects
/// gone, a blast of wind, the blast's effect, and its hitboxes scaled.
#[doc(alias = "it_802947CC")]
pub fn crash(r: &mut Running, gobj: GObjId, pos: crate::vec::Vec3) {
    lib2725::clear_velocity(r.game, gobj);
    item_mut(r.game, gobj).xDAC_itcmd_var0 = 0;
    change_state(r, gobj, 5, change::ANIM_UPDATE);
    let ip = item(r.game, gobj);
    let saved = (
        ip.xD90,
        ip.xD94.clone(),
        ip.xD9C.clone(),
        ip.xDA4_word,
        ip.xDA8_short,
    );
    let vel = ip.x40_vel;
    super::super::release::let_go(r, gobj, vel);
    let ip = item_mut(r.game, gobj);
    ip.xD90 = saved.0;
    ip.xD94 = saved.1;
    ip.xD9C = saved.2;
    ip.xDA4_word = saved.3;
    ip.xDA8_short = saved.4;
    lib26b1::disable_pickup(r.game, gobj);
    item_mut(r.game, gobj).pos = pos;
    let j = super::super::jobj_of(r.game, gobj).expect("a Warp Star without a model");
    super::super::item::set_translate(r.game, j, pos);
    crate::hsd::jobj::set_flags_all(&mut r.game.objs.jobjs, Some(j), crate::hsd::jobj::HIDDEN);
    lib26b1::set_xdd0_b3(r.game, gobj);
    lib2725::start_vanishing(r.game, r.data, gobj);
    eflib::destroy_all(r, gobj);
    let p = item(r.game, gobj).pos;
    crate::lb::wind::blast(&mut r.game.gr.winds, p, 0x78, 1.0, 0.02, 1.047_197_6);
    lib2725::blast_effect(r, gobj);
    let hit = item_mut(r.game, gobj).xDD4_itemVar.wstar().xDD8.get();
    super::super::hitbox::scale_hitboxes(r.game, gobj, crate::fpu::reg(hit));
}

/// `itWstar_UnkMotion5_Anim`: the crash's hitboxes scaled again when its script asks, gone
/// once its time's up.
#[doc(alias = "itWstar_UnkMotion5_Anim")]
pub fn crashed(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).xDAC_itcmd_var0 != 0 {
        let hit = item_mut(r.game, gobj).xDD4_itemVar.wstar().xDD8.get();
        super::super::hitbox::scale_hitboxes(r.game, gobj, crate::fpu::reg(hit));
        item_mut(r.game, gobj).xDAC_itcmd_var0 = 0;
    }
    lib2725::tick_life(r.game, gobj)
}
