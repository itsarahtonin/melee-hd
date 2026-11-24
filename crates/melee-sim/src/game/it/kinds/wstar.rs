//! The Warp Star, `melee/it/kinds/itwstar.c`: it falls, then rests spinning; picked up, it
//! carries its rider off and up (the fighter's Warp Star states, `ft::co::warpstar`), comes
//! down from the top with it (state 3) and crashes where it lands, its blast an attack of its
//! rider's (state 5, until it's gone). Its spawn, rest, fall, pickup, drop and skid are the
//! power-ups' ([`powerup`]); its ride is its own.

use super::super::item::{article, change, change_state};
use super::super::{effect, item, item_mut, lib26b1, lib2725};
use super::powerup::{self, PowerUp};
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

/// The Warp Star: its spawn clears its hurt flag and sets its ride's scales; its glow on as
/// it's picked up and its effects gone as it's let go of.
pub const WARP_STAR: PowerUp = PowerUp {
    clears_hurt: true,
    on_spawn: Some(ride_scales),
    tilts: false,
    on_pickup: Some(glow),
    on_drop: Some(eflib::destroy_all),
};

/// Runs one of the Warp Star's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM1 | ANIM3 | ANIM4 | COLL5 => powerup::still(r, gobj),
        COLL0 => powerup::rests(r, gobj, &WARP_STAR),
        COLL1 => powerup::lands(r, gobj),
        COLL4 => powerup::skids(r, gobj),
        ANIM5 => crashed(r, gobj),
        _ => return None,
    })
}

/// Runs one of the Warp Star's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => powerup::spawned(r, gobj, &WARP_STAR),
        LANDED => powerup::landed(r, gobj),
        PHYS0 => resting(r, gobj),
        FELL => powerup::fell(r, gobj),
        PHYS1 => powerup::gravity(r, gobj),
        PICKED_UP => powerup::picked_up(r, gobj, &WARP_STAR),
        DROPPED => powerup::dropped(r, gobj, &WARP_STAR),
        ENTERED_AIR => powerup::entered_air(r, gobj),
        PHYS4 | PHYS5 => powerup::idle(r, gobj),
        _ => return false,
    }
    true
}

/// Its ride's scales at 1 as it spawns (`itWStar_Logic29_Spawned`'s own part).
fn ride_scales(r: &mut Running, gobj: GObjId) {
    let v = item_mut(r.game, gobj).vars.wstar();
    v.hit.set(1.0);
    v.speed.set(1.0);
}

/// Its glow on bone 3 as it's picked up (`it_3F14_Logic29_PickedUp`'s own part).
fn glow(r: &mut Running, gobj: GObjId) {
    let j = lib2725::bone(r.game, r.data, gobj, 3);
    effect::queue(r, gobj, SpawnKind::Attach, 0x43D, j, &[]);
}

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
    let last = i32::from(r.game.it.warp_star_last_anim);
    let candidates: Vec<i32> = (0..count).filter(|&i| i != last).collect();
    let n = candidates.len() as i32;
    let picked = candidates[r.game.objs.rng.below(n) as usize];
    r.game.it.warp_star_last_anim = picked as i8;
    item_mut(r.game, gobj).vars.wstar().ride_anim.set(picked);
    let i = item_mut(r.game, gobj).vars.wstar().ride_anim.get();
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
    item_mut(r.game, gobj).vars.wstar().speed.set(speed);
    lib2725::set_uniform_scale(r.game, j, speed);
    item_mut(r.game, gobj).vars.wstar().hit.set(hit_scale);
}

/// The ride's collision box at the star's size, `it_802944AC`.
#[doc(alias = "it_802944AC")]
pub fn ride_box(r: &mut Running, gobj: GObjId) -> crate::game::structs::ftCollisionBox {
    use crate::fpu::{mem, reg};
    use gekko_fp as fp;
    let s = reg(item_mut(r.game, gobj).vars.wstar().speed.get());
    let f = |off: u32| mem(fp::fmuls(reg(r.data.f32(attr(r, gobj, off))), s));
    use crate::game::structs::Vec2;
    crate::game::structs::ftCollisionBox {
        top: f(0xC),
        bottom: f(0x10),
        left: Vec2 {
            x: f(0x14),
            y: f(0x18),
        },
        right: Vec2 {
            x: f(0x1C),
            y: f(0x20),
        },
    }
}

/// Coming down with its rider, `it_802946B0`: state 3, sized and its hitboxes scaled, an
/// attack of its own (flags 0x44005F, id 0x5E).
#[doc(alias = "it_802946B0")]
pub fn descend(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 3, change::ANIM_UPDATE);
    let j = lib2725::attach_bone(r.game, r.data, gobj).expect("a Warp Star without its bone");
    let v = item_mut(r.game, gobj).vars.wstar();
    let size = v.speed.get();
    lib2725::set_uniform_scale(r.game, j, size);
    let hit = item_mut(r.game, gobj).vars.wstar().hit.get();
    super::super::hitbox::scale_hitboxes(r.game, gobj, crate::fpu::reg(hit));
    set_attack_flags(r, gobj, 0x0044_005F);
    set_attack_id(r, gobj, 0x5E);
}

/// The item's attack flags `flags`, `it_8027B288`: a new record unless its last byte is the
/// same nonzero one, and the record of its fighter owner's attack, else a cleared one.
#[doc(alias = "ft_800898A8")]
#[doc(alias = "it_8027B288")]
pub fn set_attack_flags(r: &mut Running, gobj: GObjId, flags: u32) {
    let b = flags.to_be_bytes();
    if b[3] == 0 || b[3] != item(r.game, gobj).attack_flags[3] {
        let rec = r.game.next_attack_record();
        item_mut(r.game, gobj).attack_record_short = rec;
    }
    item_mut(r.game, gobj).attack_flags = b;
    let owner = item(r.game, gobj).owner;
    if super::super::item::is_fighter(r.game, owner) {
        let f = super::super::owner::fighter_of(r.game, owner.expect("an owner"));
        let rec = r.game.fighters[f.0].attack_record.clone();
        let ip = item_mut(r.game, gobj);
        ip.attack_record_a = rec.percents;
        ip.attack_record_b = rec.kb_and_thrower;
        ip.attack_record_word = u32::from_be_bytes(rec.flags);
        return;
    }
    super::super::lib279c::clear_attack_record(item_mut(r.game, gobj));
}

/// The item's attack id `id`, `it_8027B330`: a new attack instance for id 1 or a new id.
#[doc(alias = "it_8027B330")]
pub fn set_attack_id(r: &mut Running, gobj: GObjId, id: i32) {
    if id == 1 || id != item(r.game, gobj).attack_id {
        item_mut(r.game, gobj).attack_id = id;
        let inst = r.game.next_attack_instance();
        item_mut(r.game, gobj).attack_instance = inst;
    }
}

/// The crash where its rider lands at `pos`, `it_802947CC`: stopped, state 5, let go of
/// (`it_8027429C`, its attack kept), no pickup, hidden at `pos`, vanishing, its effects
/// gone, a blast of wind, the blast's effect, and its hitboxes scaled.
#[doc(alias = "it_802947CC")]
pub fn crash(r: &mut Running, gobj: GObjId, pos: crate::vec::Vec3) {
    lib2725::clear_velocity(r.game, gobj);
    item_mut(r.game, gobj).cmd_var0 = 0;
    change_state(r, gobj, 5, change::ANIM_UPDATE);
    let ip = item(r.game, gobj);
    let saved = (
        ip.attack_flags,
        ip.attack_record_a.clone(),
        ip.attack_record_b.clone(),
        ip.attack_record_word,
        ip.attack_record_short,
    );
    let vel = ip.vel;
    super::super::release::let_go(r, gobj, vel);
    let ip = item_mut(r.game, gobj);
    ip.attack_flags = saved.0;
    ip.attack_record_a = saved.1;
    ip.attack_record_b = saved.2;
    ip.attack_record_word = saved.3;
    ip.attack_record_short = saved.4;
    lib26b1::disable_pickup(r.game, gobj);
    item_mut(r.game, gobj).pos = pos;
    let j = super::super::jobj_of(r.game, gobj).expect("a Warp Star without a model");
    super::super::item::set_translate(r.game, j, pos);
    crate::hsd::jobj::set_flags_all(&mut r.game.objs.jobjs, Some(j), crate::hsd::jobj::HIDDEN);
    lib26b1::freeze_lifetime(r.game, gobj);
    lib2725::start_vanishing(r.game, r.data, gobj);
    eflib::destroy_all(r, gobj);
    let p = item(r.game, gobj).pos;
    crate::lb::wind::blast(
        &mut r.game.gr.winds,
        p,
        0x78,
        1.0,
        0.02,
        std::f32::consts::FRAC_PI_3,
    );
    lib2725::blast_effect(r, gobj);
    let hit = item_mut(r.game, gobj).vars.wstar().hit.get();
    super::super::hitbox::scale_hitboxes(r.game, gobj, crate::fpu::reg(hit));
}

/// `itWstar_UnkMotion5_Anim`: the crash's hitboxes scaled again when its script asks, gone
/// once its time's up.
#[doc(alias = "itWstar_UnkMotion5_Anim")]
pub fn crashed(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).cmd_var0 != 0 {
        let hit = item_mut(r.game, gobj).vars.wstar().hit.get();
        super::super::hitbox::scale_hitboxes(r.game, gobj, crate::fpu::reg(hit));
        item_mut(r.game, gobj).cmd_var0 = 0;
    }
    lib2725::tick_life(r.game, gobj)
}
