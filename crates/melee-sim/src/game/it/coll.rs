//! Item hitboxes, hurtboxes and pushes, `melee/it/itcoll.c`: hitboxes following their joints,
//! the victims they remember, hurtboxes from the article, hits between items and from
//! fighters' hitboxes, the damage log they leave, and items keeping clear of fighters and of
//! each other on the ground.

use gekko_fp as fp;
use melee_data::archive::Archives;

use super::data::common;
use super::item::{article, common_data, is_fighter};
use super::{flag, item, item_mut, jobj_of, set_flag};
use crate::fpu::{mem, reg};
use crate::game::code;
use crate::game::state::{Game, Running};
use crate::game::structs::_itECB;
use crate::hsd::gobj::GObjId;
use crate::lb::hitcapsule;
use crate::vec::Vec3;

/// `HurtCapsuleState`: `HurtCapsule_Enabled` is 0.
pub const HURT_ENABLED: i32 = 0;

/// Starts a hit check's damage log over, `it_8026F9A0`.
#[doc(alias = "it_8026F9A0")]
pub fn clear_damage_log(game: &mut Game) {
    game.it.damage_log_len = 0;
}

/// Gives a hitbox that joins a group the victims its group's other boxes remember, else none,
/// `it_8026FCF8`: an item with an ignore id shares them with every item of that id.
#[doc(alias = "it_8026FCF8")]
pub fn share_group_victims(game: &mut Game, gobj: GObjId, idx: usize) {
    let id = item(game, gobj).xAC4_ignoreItemID;
    if id != 0 {
        let group = item(game, gobj).x5D4_hitboxes[idx].hit.x4;
        let mut cur = game.world.first_in(super::P_LINK_ITEM);
        while let Some(g) = cur {
            let other = item(game, g);
            if other.xAC4_ignoreItemID == id {
                let src = (0..4).find(|&i| {
                    let h = &other.x5D4_hitboxes[i].hit;
                    !(g == gobj && i == idx) && h.state != hitcapsule::DISABLED && h.x4 == group
                });
                if let Some(i) = src {
                    let h = other.x5D4_hitboxes[i].hit.clone();
                    hitcapsule::copy_victims(&h, &mut item_mut(game, gobj).x5D4_hitboxes[idx].hit);
                    return;
                }
            }
            cur = game.world.gobjs[g.0].next;
        }
    }
    hitcapsule::clear_victims(&mut item_mut(game, gobj).x5D4_hitboxes[idx].hit);
}

/// Fighters' hitboxes against the item's hurtboxes, `it_802703E8`.
pub fn hit_by_fighters(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).xAC8_hurtboxNum == 0 {
        return;
    }
    code::unported_call("it_802703E8 (fighters' hitboxes against an item)");
}

/// Other items' hitboxes against the item's hitboxes (clanks) and hurtboxes, `it_802706D0`.
#[doc(alias = "it_802706D0")]
pub fn hit_by_items(r: &mut Running, gobj: GObjId) {
    let game = &*r.game;
    let me = item(game, gobj);
    let mut passed = false;
    let mut cur = game.world.first_in(super::P_LINK_ITEM);
    while let Some(g) = cur {
        cur = game.world.gobjs[g.0].next;
        if g == gobj {
            passed = true;
            continue;
        }
        let other = item(game, g);
        let skip = (me.owner.is_none()
            && other.owner.is_none()
            && !other.xDCD_flag.x0.b7
            && !flag(me.xDCE_flag, 2))
            || (is_fighter(game, other.owner)
                && is_fighter(game, me.owner)
                && r.is_same_player(me.owner, other.owner)
                && !other.xDCD_flag.x0.b7
                && !flag(me.xDCE_flag, 2))
            || (super::owner::is_team_battle(game)
                && !super::owner::friendly_fire(game)
                && !other.xDCD_flag.x0.b6
                && !flag(me.xDCE_flag, 1)
                && other.x20_team_id == me.x20_team_id);
        if skip {
            continue;
        }
        let checks_clanks = passed && !flag(me.xDD0_flag, 1);
        let any_box = me
            .x5D4_hitboxes
            .iter()
            .any(|h| h.hit.state != hitcapsule::DISABLED);
        let other_box = other
            .x5D4_hitboxes
            .iter()
            .any(|h| h.hit.state != hitcapsule::DISABLED);
        if (checks_clanks && any_box) || (other_box && me.xAC8_hurtboxNum != 0) {
            code::unported_call("it_802706D0 (an item's hitboxes against another item)");
        }
        if other_box && checks_clanks {
            code::unported_call("it_802706D0 (clanks between items)");
        }
    }
}

/// What the item's damage log decides: who hit it hardest, from where, and the knockback,
/// `it_80270E30`.
#[doc(alias = "it_80270E30")]
pub fn take_hits(r: &mut Running, _gobj: GObjId) {
    if r.game.it.damage_log_len != 0 {
        code::unported_call("it_80270E30 (an item taking hits)");
    }
}

/// Brings a hitbox's position up to date, `it_8027129C`: a new one starts where its joint
/// is, a live one remembers where it was.
#[doc(alias = "it_8027129C")]
pub fn update_hitbox(game: &mut Game, data: &Archives, gobj: GObjId, idx: usize) {
    let hit = &item(game, gobj).x5D4_hitboxes[idx].hit;
    match hit.state {
        hitcapsule::ENABLED => {
            let (j, off) = (hit.jobj, hit.b_offset);
            let p = crate::lb::joint::world_position(&mut game.objs, data, j, Some(off));
            let ip = item_mut(game, gobj);
            let hit = &mut ip.x5D4_hitboxes[idx].hit;
            hit.x4C = p;
            hit.x58 = p;
            hit.state = hitcapsule::UNK2;
            set_flag(&mut ip.xDAA, 2, true);
        }
        hitcapsule::UNK2 | hitcapsule::UNK3 => {
            let (j, off) = (hit.jobj, hit.b_offset);
            let ip = item_mut(game, gobj);
            let hit = &mut ip.x5D4_hitboxes[idx].hit;
            if hit.state == hitcapsule::UNK2 {
                hit.state = hitcapsule::UNK3;
            }
            hit.x58 = hit.x4C;
            let p = crate::lb::joint::world_position(&mut game.objs, data, j, Some(off));
            item_mut(game, gobj).x5D4_hitboxes[idx].hit.x4C = p;
        }
        _ => {}
    }
}

/// Brings every hitbox's position up to date, `it_8027137C`.
#[doc(alias = "it_8027137C")]
pub fn update_hitboxes(game: &mut Game, data: &Archives, gobj: GObjId) {
    for i in 0..4 {
        update_hitbox(game, data, gobj, i);
    }
}

/// Ages the victims each hitbox remembers, `it_8027146C`.
#[doc(alias = "it_8027146C")]
pub fn age_hit_victims(game: &mut Game, gobj: GObjId) {
    for h in &mut item_mut(game, gobj).x5D4_hitboxes {
        hitcapsule::age_victims(&mut h.hit);
    }
}

/// Marks the hurtboxes for a position update, `it_802714C0`.
#[doc(alias = "it_802714C0")]
pub fn refresh_hurtboxes(game: &mut Game, gobj: GObjId) {
    let ip = item_mut(game, gobj);
    let n = ip.xAC8_hurtboxNum as usize;
    for h in &mut ip.xACC_itemHurtbox[..n] {
        h.skip_update_pos = false;
    }
    set_flag(&mut ip.xDAA, 1, true);
}

/// Sets every hurtbox's state, `it_80271508`.
#[doc(alias = "it_80271508")]
pub fn set_hurtbox_state(game: &mut Game, gobj: GObjId, state: i32) {
    let ip = item_mut(game, gobj);
    let n = ip.xAC8_hurtboxNum as usize;
    for h in &mut ip.xACC_itemHurtbox[..n] {
        h.state = state;
    }
}

/// Gives the item its article's hurtboxes and collision dynamics, `it_8027163C`.
#[doc(alias = "it_8027163C")]
pub fn init_hurtboxes(r: &mut Running, gobj: GObjId) {
    let data = r.data;
    let a = article(item(r.game, gobj));
    let root = jobj_of(r.game, gobj);
    let table = item(r.game, gobj).xBBC_dynamicBoneTable;
    let bone = |game: &Game, id: u32| {
        let t = table.expect("item can't init hit!");
        game.it.bone_tables[t.0].bones[id as usize]
    };
    match a.hurtbones(data) {
        Some(list) => {
            let n = data.u32(list);
            assert!(n <= 2, "item hit num over!");
            item_mut(r.game, gobj).xAC8_hurtboxNum = n as u8;
            let descs = data.ptr(list.at(4)).expect("hurtbox descriptions");
            for i in 0..n {
                let d = descs.at(0x20 * i);
                let id = data.u32(d);
                let b = if id != 0 { bone(r.game, id) } else { root };
                let v = |o: u32| {
                    Vec3::new(
                        data.f32(d.at(o)),
                        data.f32(d.at(o + 4)),
                        data.f32(d.at(o + 8)),
                    )
                };
                let hurt = &mut item_mut(r.game, gobj).xACC_itemHurtbox[i as usize];
                hurt.state = HURT_ENABLED;
                hurt.bone = b;
                hurt.a_offset = v(4);
                hurt.b_offset = v(0x10);
                hurt.scale = data.f32(d.at(0x1C));
            }
        }
        None => item_mut(r.game, gobj).xAC8_hurtboxNum = 0,
    }
    if let Some(dyns) = a.dynamics(data) {
        let n = data.u32(dyns.at(8));
        assert!(n <= 2, "item dynamics hit num over!");
        item_mut(r.game, gobj).xB68 = n as u8;
        let descs = data.ptr(dyns.at(0xC));
        for i in 0..n {
            let d = descs.expect("collision dynamics descriptions").at(0x14 * i);
            let id = data.u32(d);
            let b = bone(r.game, id);
            let v = &mut item_mut(r.game, gobj).xB6C_vars[i as usize];
            v.xB90 = id;
            v.xB7C = b;
            v.xB6C = Vec3::new(data.f32(d.at(4)), data.f32(d.at(8)), data.f32(d.at(0xC)));
            v.xB78 = data.f32(d.at(0x10));
        }
    }
}

/// Turns the item's ECB by `angle` about z and takes the box around it, `it_80271830`.
#[doc(alias = "it_80271830")]
pub fn rotate_ecb(ecb: &mut _itECB, angle: f32) {
    let tau = std::f64::consts::TAU;
    let mut a = reg(angle);
    while a < 0.0 {
        a = fp::frsp(fp::fadd(a, tau));
    }
    while a > tau {
        a = fp::frsp(fp::fsub(a, tau));
    }
    let axis = Vec3::new(0.0, 0.0, 1.0);
    let turned = |v: Vec3| {
        let mut v = v;
        v.rotate_about(axis, mem(a));
        v
    };
    let top = turned(Vec3::new(0.0, ecb.top, 0.0));
    let bottom = turned(Vec3::new(0.0, ecb.bottom, 0.0));
    let right = turned(Vec3::new(ecb.right, 0.0, 0.0));
    let left = turned(Vec3::new(ecb.left, 0.0, 0.0));
    // The machine code keeps the first on a NaN where the C's ternaries take the second.
    let max = |a: f32, b: f32| if reg(a) <= reg(b) { b } else { a };
    let min = |a: f32, b: f32| if reg(a) >= reg(b) { b } else { a };
    ecb.left = max(max(max(top.x, bottom.x), right.x), left.x);
    ecb.top = max(max(max(top.y, bottom.y), right.y), left.y);
    ecb.right = min(min(min(top.x, bottom.x), right.x), left.x);
    ecb.bottom = min(min(min(top.y, bottom.y), right.y), left.y);
}

/// Sets the item's push box from its base box, turned with its facing and its spun joint,
/// `it_80271A58`.
#[doc(alias = "it_80271A58")]
pub fn update_ecb(game: &mut Game, data: &Archives, gobj: GObjId) {
    let ip = item_mut(game, gobj);
    ip.xBEC = ip.xBDC.clone();
    if reg(ip.facing_dir) == 1.0 && ip.xDC8_word.flags.x19 {
        let (l, rt) = (ip.xBEC.left, ip.xBEC.right);
        ip.xBEC.right = mem(fp::fneg(reg(l)));
        ip.xBEC.left = mem(fp::fneg(reg(rt)));
    }
    let j = super::lib2725::spin_joint(game, data, gobj).expect("jobj");
    let q = game.objs.jobjs[j].rotate;
    let ip = item_mut(game, gobj);
    let angle = match ip.xDC8_word.flags.x17 {
        0 => q.z,
        1 => q.x,
        _ => q.y,
    };
    rotate_ecb(&mut ip.xBEC, angle);
}

/// Whether `target` lies in the box at (`x`, `y`) the two ECBs make together
/// (`itColl_chkECBOverlap`).
pub fn ecbs_overlap(x: f32, y: f32, a: &_itECB, b: &_itECB, target: Vec3) -> bool {
    let (x, y) = (reg(x), reg(y));
    let top = fp::fadds(y, fp::fadds(reg(a.top), reg(b.top)));
    let bottom = fp::fadds(y, fp::fadds(reg(a.bottom), reg(b.bottom)));
    let right = fp::fadds(x, fp::fadds(reg(a.right), reg(b.right)));
    let left = fp::fadds(x, fp::fadds(reg(a.left), reg(b.left)));
    let (tx, ty) = (reg(target.x), reg(target.y));
    top >= ty && bottom <= ty && right <= tx && left >= tx
}

/// Which way to push apart from a thing at `other_x`: away from it, or a coin toss (or the way
/// it isn't already nudged) when they're level.
pub fn push_dir(
    game: &mut Game,
    me_x: f32,
    other_x: f32,
    level: impl FnOnce(&mut Game) -> f64,
) -> f64 {
    let d = fp::fsubs(reg(me_x), reg(other_x));
    let mag = if d < fp::frsp(0.0) { fp::fneg(d) } else { d };
    if mag < f64::from(0.001_f32) {
        level(game)
    } else if fp::fsubs(reg(me_x), reg(other_x)) < 0.0 {
        fp::fneg(1.0)
    } else {
        1.0
    }
}

/// Pushes a grounded item clear of the fighters, `it_80271B60`.
#[doc(alias = "it_80271B60")]
pub fn push_from_fighters(game: &mut Game, data: &Archives, gobj: GObjId) {
    if flag(game.it.ft_track.x154, 0) {
        return;
    }
    let j = jobj_of(game, gobj).expect("jobj");
    let pos = game.objs.jobjs[j].translate;
    let push = common_data(game).f32(data, common::X7C);
    for i in 0..game.it.ft_track.count as usize {
        let t = &game.it.ft_track;
        let (ecb, fx, fy) = (
            t.ecb_offset_arr[i].clone(),
            t.ft_pos_arr[i].x,
            t.ft_pos_arr[i].y,
        );
        if ecbs_overlap(fx, fy, &item(game, gobj).xBEC, &ecb, pos) {
            let dir = push_dir(game, pos.x, fx, |g| {
                if g.objs.rng.below(2) != 0 {
                    1.0
                } else {
                    fp::fneg(1.0)
                }
            });
            let ip = item_mut(game, gobj);
            ip.x70_nudge.x = mem(fp::fmuls(reg(push), dir));
            ip.xDC0 |= 1;
        }
    }
}

/// Pushes a grounded item clear of other grounded items (`it_80271D2C`), or only of heavy
/// ones (`it_80271F78`).
#[doc(alias = "it_80271D2C")]
#[doc(alias = "it_80271F78")]
pub fn push_from_items(game: &mut Game, data: &Archives, gobj: GObjId, heavy_only: bool) {
    let j = jobj_of(game, gobj).expect("jobj");
    let pos = game.objs.jobjs[j].translate;
    let push = common_data(game).f32(data, common::X7C);
    let mut cur = game.world.first_in(super::P_LINK_ITEM);
    while let Some(g) = cur {
        let other = item(game, g);
        let me = item(game, gobj);
        let candidate = g != gobj
            && !other.xDC8_word.flags.x13
            && other.ground_or_air == 0
            && !flag(other.xDD1_flag, 0)
            && if heavy_only {
                super::lib26b1::is_heavy(game, data, g)
            } else {
                other.hold_kind != 3 || me.xDC8_word.flags.x1E
            };
        if candidate {
            let oj = jobj_of(game, g).expect("jobj");
            let opos = game.objs.jobjs[oj].translate;
            if ecbs_overlap(opos.x, opos.y, &me.xBEC, &other.xBEC, pos) {
                let (x1b, nudge) = (other.xDC8_word.flags.x1B, other.x70_nudge.x);
                let dir = push_dir(game, pos.x, opos.x, |gm| {
                    if !x1b {
                        if reg(nudge) < 0.0 { 1.0 } else { fp::fneg(1.0) }
                    } else if gm.objs.rng.below(2) != 0 {
                        1.0
                    } else {
                        fp::fneg(1.0)
                    }
                });
                let ip = item_mut(game, gobj);
                ip.x70_nudge.x = mem(fp::fmuls(reg(push), dir));
                ip.xDC0 |= 2;
            }
        }
        cur = game.world.gobjs[g.0].next;
    }
}

/// Clears the nudge and pushes a grounded item clear of fighters and items, `it_802721B8`.
#[doc(alias = "it_802721B8")]
pub fn keep_clear(game: &mut Game, data: &Archives, gobj: GObjId) {
    let ip = item_mut(game, gobj);
    ip.x70_nudge = Vec3::new(0.0, 0.0, 0.0);
    ip.xDC0 = 0;
    let f = &ip.xDC8_word.flags;
    if !f.x13 && f.x1A && ip.ground_or_air == 0 {
        if f.x1C {
            push_from_fighters(game, data, gobj);
        }
        if item(game, gobj).xDC8_word.flags.x1D {
            push_from_items(game, data, gobj, false);
        }
    }
    let ip = item(game, gobj);
    if !ip.xDC8_word.flags.x13
        && ip.ground_or_air == 0
        && super::lib26b1::is_heavy(game, data, gobj)
    {
        push_from_items(game, data, gobj, true);
    }
}

/// Clears `x1B`, `it_80272280`.
#[doc(alias = "it_80272280")]
pub fn clear_x1b(game: &mut Game, gobj: GObjId) {
    item_mut(game, gobj).xDC8_word.flags.x1B = false;
}

/// Sets `x1B`, `it_80272298`.
#[doc(alias = "it_80272298")]
pub fn set_x1b(game: &mut Game, gobj: GObjId) {
    item_mut(game, gobj).xDC8_word.flags.x1B = true;
}

/// The first item takes the fighters' positions for every item to keep clear of,
/// `it_802722B0`.
#[doc(alias = "it_802722B0")]
pub fn track_fighters(game: &mut Game, gobj: GObjId) {
    if game.world.first_in(super::P_LINK_ITEM) == Some(gobj) {
        super::owner::track_fighters(game);
        set_flag(&mut game.it.ft_track.x154, 0, false);
    }
}

/// Moves the item's collision dynamics points with their joints, `it_80272304`.
#[doc(alias = "it_80272304")]
pub fn update_dynamics_points(game: &mut Game, data: &Archives, gobj: GObjId) {
    for i in 0..item(game, gobj).xB68 as usize {
        let v = &item(game, gobj).xB6C_vars[i];
        let (j, off) = (v.xB7C, v.xB6C);
        let p = crate::lb::joint::world_position(&mut game.objs, data, j, Some(off));
        item_mut(game, gobj).xB6C_vars[i].xB84 = p;
    }
}

/// Sets a hitbox's damage, `it_80272460`: a fighter owner's height scales it and its stale
/// moves weaken it.
#[doc(alias = "it_80272460")]
pub fn set_hit_damage(game: &mut Game, data: &Archives, gobj: GObjId, idx: usize, damage: u32) {
    let mut dmg = damage;
    let owner = item(game, gobj).owner;
    if is_fighter(game, owner) {
        let f = super::owner::fighter_of(game, owner.unwrap());
        let scale_y = game.fighters[f.0].x34_scale.y;
        if reg(scale_y) != 1.0 {
            let p = game
                .it
                .ft_scale_params
                .expect("no fighter scale parameters");
            let scaled = super::owner::y_scaled(
                fp::frsp(f64::from(dmg)),
                reg(scale_y),
                reg(data.f32(p.at(4))),
            );
            dmg = crate::ppc::cvt_fp2unsigned(fp::fadds(f64::from(0.999_f32), scaled));
        }
        let ip = item(game, gobj);
        let (attack, count) = (ip.xD88_attackID, dmg);
        let d = super::owner::stale_damage(game, data, f, attack, fp::frsp(f64::from(count)));
        let hit = &mut item_mut(game, gobj).x5D4_hitboxes[idx].hit;
        hit.unk_count = count;
        hit.damage = mem(d);
        return;
    }
    let hit = &mut item_mut(game, gobj).x5D4_hitboxes[idx].hit;
    hit.unk_count = dmg;
    hit.damage = mem(fp::frsp(f64::from(dmg)));
}
