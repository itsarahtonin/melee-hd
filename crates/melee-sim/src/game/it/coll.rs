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
use crate::game::ft::common::{GA_AIR, GA_GROUND};
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

/// `HitElement_Catch` and `HitElement_Inert`.
const ELEMENT_CATCH: u32 = 8;
const ELEMENT_INERT: u32 = 11;

/// Other items' hitboxes against the item's hitboxes (clanks) and hurtboxes, `it_802706D0`:
/// of the items it may meet (not its own player's or a teammate's, unless their flags say),
/// the later ones' boxes clank with its own that can (`it_804D6D1C` marks those); an inert
/// box only touches. The hits on its hurtboxes and an inert box's touch of them stop at their
/// seams.
#[doc(alias = "it_802706D0")]
pub fn hit_by_items(r: &mut Running, gobj: GObjId) {
    use super::P_LINK_ITEM;
    use hitcapsule::Victim;
    let me_id = super::item_of(r.game, gobj);
    let mut passed = false;
    let mut count = 0;
    let mut cur = r.game.world.first_in(P_LINK_ITEM);
    while let Some(g) = cur {
        cur = r.game.world.gobjs[g.0].next;
        if g == gobj {
            passed = true;
            continue;
        }
        let game = &*r.game;
        let me = item(game, gobj);
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
            || (game.vs_is_teams()
                && !game.vs_friendly_fire()
                && !other.xDCD_flag.x0.b6
                && !flag(me.xDCE_flag, 1)
                && other.x20_team_id == me.x20_team_id);
        if skip {
            continue;
        }
        let other_id = super::item_of(game, g);
        let clanks = passed && !flag(me.xDD0_flag, 1);
        if clanks {
            count = 0;
            for k in 0..4 {
                let (me, other) = (item(r.game, gobj), item(r.game, g));
                let h = &me.x5D4_hitboxes[k].hit;
                let can = h.state != hitcapsule::DISABLED
                    && h.element != ELEMENT_CATCH
                    && ((h.x40_b2 && other.ground_or_air == GA_AIR)
                        || (h.x40_b3 && other.ground_or_air == GA_GROUND))
                    && !hitcapsule::has_victim(h, Victim::Item(other_id));
                r.game.it.clank_boxes[k] = u8::from(can);
                if can {
                    count += 1;
                }
            }
        }
        for hi in 0..4 {
            let (me, other) = (item(r.game, gobj), item(r.game, g));
            let h = &other.x5D4_hitboxes[hi].hit;
            if h.state == hitcapsule::DISABLED
                || !h.x42_b7
                || (!(h.x40_b2 && me.ground_or_air == GA_AIR)
                    && !(h.x40_b3 && me.ground_or_air == GA_GROUND))
                || hitcapsule::has_victim(h, Victim::Item(me_id))
            {
                continue;
            }
            if clanks && count != 0 {
                let mut clanked = false;
                for i in 0..4 {
                    if r.game.it.clank_boxes[i] == 0 {
                        continue;
                    }
                    let (me, other) = (item(r.game, gobj), item(r.game, g));
                    let (theirs, mine) = (&other.x5D4_hitboxes[hi].hit, &me.x5D4_hitboxes[i].hit);
                    if theirs.element == ELEMENT_INERT || mine.element == ELEMENT_INERT {
                        let inert_theirs = theirs.element == ELEMENT_INERT;
                        if theirs.element != mine.element && boxes_touch(r.game, g, hi, gobj, i) {
                            if inert_theirs {
                                let o = item_mut(r.game, g);
                                set_flag(&mut o.xDCE_flag, 6, true);
                                o.toucher = Some(gobj);
                            } else {
                                let ent = item(r.game, g).entity;
                                let m = item_mut(r.game, gobj);
                                set_flag(&mut m.xDCE_flag, 6, true);
                                m.toucher = ent;
                            }
                            clanked = true;
                            break;
                        }
                    } else if theirs.x40_b0 && mine.x40_b0 && boxes_touch(r.game, g, hi, gobj, i) {
                        clank(r, g, hi, gobj, i);
                        clanked = true;
                        break;
                    }
                }
                if clanked {
                    continue;
                }
            }
            let me = item(r.game, gobj);
            if me.xAC8_hurtboxNum == 0 {
                continue;
            }
            if item(r.game, g).x5D4_hitboxes[hi].hit.element == ELEMENT_INERT {
                code::unported_call("lbColl_80008248 (an inert box touching an item's hurtbox)");
            } else if me.xD0C != 2 {
                for n in 0..usize::from(me.xAC8_hurtboxNum) {
                    let mut hit = item(r.game, g).x5D4_hitboxes[hi].hit.clone();
                    let (hs, ms) = (item(r.game, g).scl, item(r.game, gobj).scl);
                    let mut hurt = item(r.game, gobj).xACC_itemHurtbox[n].clone();
                    let touch = hitcapsule::hits_hurtbox(
                        &mut r.game.objs,
                        r.data,
                        &mut hit,
                        &mut hurt,
                        None,
                        false,
                        hs,
                        ms,
                        0.0,
                    );
                    item_mut(r.game, gobj).xACC_itemHurtbox[n] = hurt;
                    let h = &mut item_mut(r.game, g).x5D4_hitboxes[hi].hit;
                    h.hurt_coll_pos = hit.hurt_coll_pos;
                    h.coll_distance = hit.coll_distance;
                    if touch {
                        code::unported_call("it_802706D0's hit on an item's hurtbox");
                    }
                }
            }
        }
    }
}

/// Whether item `a`'s box `i` touches item `b`'s box `j` (`lbColl_80007AFC`), each keeping
/// where it met the other.
fn boxes_touch(game: &mut Game, a: GObjId, i: usize, b: GObjId, j: usize) -> bool {
    let mut sa = hitcapsule::shape(&item(game, a).x5D4_hitboxes[i].hit);
    let mut sb = hitcapsule::shape(&item(game, b).x5D4_hitboxes[j].hit);
    let (ka, kb) = (item(game, a).scl, item(game, b).scl);
    let touch = crate::lb::collision::hitboxes_touch(&mut sa, &mut sb, ka, kb);
    item_mut(game, a).x5D4_hitboxes[i].hit.hurt_coll_pos = sa.contact;
    item_mut(game, b).x5D4_hitboxes[j].hit.hurt_coll_pos = sb.contact;
    touch
}

/// Two items' boxes clank, `it_8026FE68`: each one whose damage the other's outweighs (by
/// more than `ItemCommonData` xB4 less) remembers the other, keeps the damage, the other's
/// kind and position and which way it was pushed, with the clank's effect between them.
// Ports `it_8026FE68`; aliased once a check reaches it.
fn clank(r: &mut Running, a: GObjId, i: usize, b: GObjId, j: usize) {
    use hitcapsule::Victim;
    let mid = {
        let (h1, h3) = (
            &item(r.game, a).x5D4_hitboxes[i].hit,
            &item(r.game, b).x5D4_hitboxes[j].hit,
        );
        let m = |p: f32, q: f32| mem(fp::fmuls(0.5, fp::fadds(reg(p), reg(q))));
        Vec3::new(
            m(h1.hurt_coll_pos.x, h3.hurt_coll_pos.x),
            m(h1.hurt_coll_pos.y, h3.hurt_coll_pos.y),
            m(h1.hurt_coll_pos.z, h3.hurt_coll_pos.z),
        )
    };
    let c = common_data(r.game);
    let (margin, slow) = (c.i32(r.data, common::XB4), c.f32(r.data, common::XD4));
    // The second item's box first, then the first's.
    for (me, mi, other, oi, clear) in [(b, j, a, i, true), (a, i, b, j, false)] {
        let mine = reg(item(r.game, me).x5D4_hitboxes[mi].hit.damage);
        let theirs = reg(item(r.game, other).x5D4_hitboxes[oi].hit.damage);
        if fp::fctiwz(mine).wrapping_sub(margin) >= fp::fctiwz(theirs) {
            continue;
        }
        let dmg = fp::fctiwz(mine);
        let kind = if item(r.game, me).x5D4_hitboxes[mi].hit.x41_b5 {
            4
        } else {
            3
        };
        let victim = Victim::Item(super::item_of(r.game, other));
        r.item_add_victim(me, mi, kind, victim, clear);
        if dmg > item(r.game, me).xC48 {
            let (okind, opos) = (item(r.game, other).kind, item(r.game, other).pos);
            let ip = item_mut(r.game, me);
            ip.xC48 = dmg;
            ip.xCF4_fighterGObjUnk = None;
            ip.xC38 = okind;
            ip.xCD4 = opos;
            let vx = reg(ip.x40_vel.x);
            let mag = if vx < 0.0 { fp::fneg(vx) } else { vx };
            ip.xCB8_outDamageDirection = if mag < reg(slow) {
                if reg(ip.pos.x) > reg(opos.x) {
                    -1.0
                } else {
                    1.0
                }
            } else if vx < 0.0 {
                -1.0
            } else {
                1.0
            };
        }
        let ent = item(r.game, me).entity;
        crate::game::ef::spawn(r, 0x41C, ent, &[crate::game::ef::Arg::Vec(mid)]);
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
            // `Fighter_804D6524`'s `x4` scales an item's damage by its owner's height.
            let p = crate::game::ft::common_ptr(game, data, crate::game::ft::SCALE_PARAMS)
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

/// The team byte of an owner: a fighter's team (`ftLib_GetTeam`), which the original also
/// reads from an item through the fighter's layout.
fn owner_team(game: &Game, owner: GObjId) -> u8 {
    if !is_fighter(game, Some(owner)) {
        code::unported_call("ftLib_GetTeam on an item");
    }
    super::owner::team(game, Some(owner)) as u8
}

/// Takes whoever last hit it as its owner (the fighter, else the item), `it_8027236C`, and
/// starts a new attack for it.
#[doc(alias = "it_8027236C")]
pub fn take_attacker(game: &mut Game, gobj: GObjId) -> Option<GObjId> {
    let ip = item(game, gobj);
    let by = ip.xCEC_fighterGObj.or(ip.xCF0_itemGObj);
    let team = by.map_or(255, |o| owner_team(game, o));
    let ip = item_mut(game, gobj);
    ip.owner = by;
    ip.x20_team_id = team;
    super::lib279c::restart_attack(game, gobj);
    item(game, gobj).owner
}

/// Takes the fighter that jumped on it as its owner, `it_802723FC`.
#[doc(alias = "it_802723FC")]
pub fn take_stomper(game: &mut Game, gobj: GObjId) -> Option<GObjId> {
    let by = item(game, gobj).xCFC;
    let (owner, team) = if is_fighter(game, by) {
        (by, owner_team(game, by.unwrap()))
    } else {
        (None, 255)
    };
    let ip = item_mut(game, gobj);
    ip.owner = owner;
    ip.x20_team_id = team;
    owner
}
