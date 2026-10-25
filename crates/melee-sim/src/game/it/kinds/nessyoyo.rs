//! Ness's yo-yo, `melee/it/kinds/itnessyoyo.c`: a string of links (`ItemLink`, see
//! [`super::super::link`]) from his hand, the yo-yo itself the tip's model. It hangs at the
//! hand (state 0), spins out (state 1) and back, is let go of for a charge and pulled along the
//! ground (state 2), and reels in (state 3), each frame its links lined up between their
//! neighbours and Ness's yo-yo hitbox at the tip.
//!
//! The yo-yo's variables (`itNessYoyo_ItemVars`): the smash that made it (`x0`), the reel's
//! speed (`x4`), the tip (`x8`, the last link made) and the link at the hand (`xC`), Ness
//! (`x10`), and the tip's model (`x18`).

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::item::{article, change, change_state, destroy, pick_up, spawn_air};
use super::super::link::{self, apply_vel, direction, link as lk, link_mut, move_coll, place_coll};
use super::super::maplib::with_stage_coll;
use super::super::{item, item_mut, lib26b1};
use crate::fpu::{mem, reg};
use crate::game::code::ParticleHooks;
use crate::game::ft::ness::attack_hi4;
use crate::game::handles::{CodeRef, ItemLinkId};
use crate::game::state::{Obj, Running, UserData};
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::{self as gobjs, GObjId};
use crate::hsd::jobj::{self, JObjId};
use crate::mtx::Mtx34;
use crate::vec::Vec3;
use melee_data::archive::DataRef;

pub const HITLAG_START: u32 = 0x802B_E598;
pub const HITLAG_END: u32 = 0x802B_E5B8;
pub const SET_SPIN: u32 = 0x802B_E5D8;
pub const MAKE_LINKS: u32 = 0x802B_E65C;
pub const REMOVE: u32 = 0x802B_E958;
pub const SPAWN: u32 = 0x802B_E9D8;
pub const PHYS_0: u32 = 0x802B_EB38;
pub const PHYS_1: u32 = 0x802B_EB5C;
pub const PHYS_2: u32 = 0x802B_EC58;
pub const PHYS_3: u32 = 0x802B_ED54;
pub const ANIM_3: u32 = 0x802B_EE88;
pub const LINK_COLL: u32 = 0x802B_F030;
pub const PULL_IN: u32 = 0x802B_F180;
pub const SPIN_OUT_STEP: u32 = 0x802B_F28C;
pub const DRAG: u32 = 0x802B_F4A0;
pub const REEL: u32 = 0x802B_F800;
pub const HANG: u32 = 0x802B_F900;
pub const LINE_UP: u32 = 0x802B_FAFC;
#[doc(alias = "itNessYoyo_Logic59_PickedUp")]
pub const PICKED_UP: u32 = 0x802B_FE34;
pub const RELEASE: u32 = 0x802B_FE5C;
pub const NUDGE: u32 = 0x802B_FEC4;
pub const SPIN_OUT: u32 = 0x802C_0010;
pub const EVT_UNK: u32 = 0x802C_016C;

/// The link GObjs' process link.
const LINK_P_LINK: u8 = 0xA;

/// `ftNs_MS_AttackHi4`, the up smash's first state; `0x159` is the down smash's charge.
const MS_ATTACK_HI4: i32 = 0x156;
const MS_FLIPPED: i32 = 0x159;

/// `itYoyoAttributes`' offsets.
pub mod attr {
    pub const CHARGE_SPAWN_POS: u32 = 0x0;
    pub const UPSMASH_CHARGE_STRING_LENGTH: u32 = 0x4;
    pub const DOWNSMASH_CHARGE_STRING_LENGTH: u32 = 0x8;
    pub const SIZE: u32 = 0xC;
    pub const UNK1: u32 = 0x10;
    pub const SPIN_TEXANIM_SPEED: u32 = 0x18;
    pub const UNK_TEXANIM_SPEED: u32 = 0x1C;
    pub const UNK_TEXANIM_MOD: u32 = 0x20;
    pub const RELEASE_VEL_X: u32 = 0x24;
    pub const PULL_STRENGTH: u32 = 0x28;
    pub const UNK3_MOD: u32 = 0x2C;
    pub const RELEASE_VEL_Y: u32 = 0x30;
    pub const GRAVITY: u32 = 0x34;
    pub const GRAVITY_MOD: u32 = 0x38;
    pub const PULL_STRENGTH_2: u32 = 0x3C;
    pub const UPSMASH_YOYO_ROT_FRAME: u32 = 0x40;
    pub const UPSMASH_YOYO_NUDGE_FRAME: u32 = 0x44;
    pub const DOWNSMASH_YOYO_ROT_FRAME: u32 = 0x48;
    pub const DOWNSMASH_YOYO_NUDGE_FRAME: u32 = 0x4C;
    pub const STRING_JOINT: u32 = 0x50;
    pub const YOYO_JOINT: u32 = 0x54;
    pub const YOYO_MATANIM: u32 = 0x58;
}

/// Runs one of the yo-yo's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM_3 => animate(r, gobj),
        _ => return None,
    })
}

/// Runs one of the yo-yo's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PHYS_0 => hang(r, gobj),
        PHYS_1 => physics_1(r, gobj),
        PHYS_2 => physics_2(r, gobj),
        PHYS_3 => physics_3(r, gobj),
        PICKED_UP => change_state(r, gobj, 0, change::ANIM_UPDATE),
        _ => return false,
    }
    true
}

fn attrs(r: &Running, gobj: GObjId) -> DataRef {
    article(item(r.game, gobj))
        .special(r.data)
        .expect("a yo-yo without attributes")
}

/// A float of the yo-yo's attributes.
pub fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    r.data.f32(attrs(r, gobj).at(off))
}

/// A word of the yo-yo's attributes.
pub fn attr_i32(r: &Running, gobj: GObjId, off: u32) -> i32 {
    r.data.u32(attrs(r, gobj).at(off)) as i32
}

/// The owner's height scale (`ftLib_GetScale`).
fn owner_scale(r: &Running, gobj: GObjId) -> f32 {
    r.game
        .ft_scale(item(r.game, gobj).owner.expect("a yo-yo without its owner"))
}

/// The yo-yo pauses with Ness's hitlag, `it_802BE598` (`it_8026B724`).
#[doc(alias = "it_802BE598")]
pub fn hitlag_start(r: &mut Running, gobj: GObjId) {
    super::seakchain::hitlag_start(r, gobj);
}

/// The yo-yo resumes, `it_802BE5B8` (`it_8026B73C`).
#[doc(alias = "it_802BE5B8")]
pub fn hitlag_end(r: &mut Running, gobj: GObjId) {
    super::seakchain::hitlag_end(r, gobj);
}

/// The yo-yo's spin texture from `frame` (0 still, 1 spinning), `it_802BE5D8`.
#[doc(alias = "it_802BE5D8")]
pub fn set_spin(r: &mut Running, gobj: GObjId, frame: f32) {
    let j = item_mut(r.game, gobj).xDD4_itemVar.ness_yoyo().x18.get();
    let mat = r.data.ptr(attrs(r, gobj).at(attr::YOYO_MATANIM));
    jobj::remove_anim_all(&mut r.game.objs, j);
    jobj::add_anim_all(&mut r.game.objs, r.data, j, None, mat, None);
    crate::lb::anim::set_framerate(&mut r.game.objs, j, 1.0);
    jobj::req_anim_all(&mut r.game.objs, j, frame);
    jobj::anim_all(&mut r.game.objs, r.data, &mut ParticleHooks, j);
}

/// Makes the yo-yo's links, `it_802BE65C`: `x0` of them at the yo-yo, each a GObj with the
/// string's model (the tip's the yo-yo's, scaled with Ness), hanging from `bone`. (The original
/// returns the tip's model, which its caller drops.)
#[doc(alias = "it_802BE65C")]
pub fn make_links(r: &mut Running, gobj: GObjId, bone: Option<JObjId>) {
    let n = attr_i32(r, gobj, attr::CHARGE_SPAWN_POS);
    let mut prev_link: Option<ItemLinkId> = None;
    let (mut head, mut tail) = (None, None);
    for i in 0..n {
        let g = r.game.world.create(link::CLASS, LINK_P_LINK, 0);
        let l = link::alloc(r.game, r.data);
        link::reset(r.game, g, l);
        r.game
            .world
            .init_user_data(g, link::USER_DATA_KIND, UserData::ItemLink(l));
        let pos = item(r.game, gobj).pos;
        let set = |r: &mut Running, b0: bool| {
            let k = link_mut(r.game, l);
            k.jobj = bone;
            k.gobj = Some(g);
            k.vel = Vec3::new(0.0, 0.0, 0.0);
            k.pos = pos;
            k.x2C_b0 = b0;
        };
        let (joint, render) = if i == 0 {
            link_mut(r.game, l).next = None;
            head = Some(l);
            set(r, false);
            link::init_coll(r, l, 1.0);
            (attr::STRING_JOINT, link::RENDER)
        } else if i == n - 1 {
            link_mut(r.game, prev_link.unwrap()).prev = Some(l);
            link_mut(r.game, l).prev = None;
            tail = Some(l);
            link_mut(r.game, l).next = prev_link;
            set(r, true);
            let size = mem(fp::fmuls(2.0, reg(owner_scale(r, gobj))));
            link::init_coll(r, l, size);
            (attr::YOYO_JOINT, crate::game::render::JOBJ_CALLBACK)
        } else {
            link_mut(r.game, prev_link.unwrap()).prev = Some(l);
            link_mut(r.game, l).next = prev_link;
            set(r, false);
            link::init_coll(r, l, 1.0);
            (attr::STRING_JOINT, link::RENDER)
        };
        let desc = r.data.ptr(attrs(r, gobj).at(joint));
        let model = jobj::load_joint(&mut r.game.objs, r.data, desc).expect("a link's model");
        let kind = super::super::item::jobj_kind(r.game);
        r.game.world.set_obj(g, kind, Obj::JObj(model));
        r.game.world.setup_gx_link(g, CodeRef(render), 6, 0);
        if i == n - 1 {
            item_mut(r.game, gobj)
                .xDD4_itemVar
                .ness_yoyo()
                .x18
                .set(Some(model));
        }
        prev_link = Some(l);
    }
    let v = item_mut(r.game, gobj).xDD4_itemVar.ness_yoyo();
    v.x8.set(tail);
    v.xC.set(head);
}

/// The yo-yo's links go and its owner lets go of it (`it_802BE958_inline`).
fn release_links(r: &mut Running, gobj: GObjId) {
    if let Some(owner) = item(r.game, gobj).owner {
        r.ness_yoyo_item_set_flag(owner);
    }
    item_mut(r.game, gobj).owner = None;
    let mut cur = item_mut(r.game, gobj).xDD4_itemVar.ness_yoyo().x8.get();
    while let Some(l) = cur {
        let g = lk(r.game, l).gobj.expect("a link without its GObj");
        cur = lk(r.game, l).next;
        gobjs::free(r, g);
    }
}

/// The yo-yo gone, `it_802BE958`.
#[doc(alias = "it_802BE958")]
pub fn remove(r: &mut Running, gobj: GObjId) {
    release_links(r, gobj);
    destroy(r, gobj);
}

/// Makes the yo-yo at Ness's hand, `it_802BE9D8`: for the smash `msid` (facing back for the
/// down smash's charge), its links, in his hand, its texture still.
#[doc(alias = "it_802BE9D8")]
pub fn spawn(
    r: &mut Running,
    owner: GObjId,
    facing_dir: f32,
    pos: Vec3,
    msid: i32,
) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind: super::NESS_YOYO,
        ..Default::default()
    };
    // `Item_InitSpawnPosition(&spawn, pos, false)`.
    spawn.prev_pos = pos;
    spawn.pos = spawn.prev_pos;
    // `Item_InitSpawnCommonFields(&spawn, owner, facing_dir, true)`.
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(owner);
    spawn.x4_parent_gobj2 = spawn.x0_parent_gobj;
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    let ip = item_mut(r.game, gobj);
    let v = ip.xDD4_itemVar.ness_yoyo();
    v.x10.set(Some(owner));
    v.x0.set(msid);
    if v.x0.get() == MS_FLIPPED {
        ip.facing_dir = mem(fp::fmuls(reg(ip.facing_dir), fp::fneg(1.0)));
    }
    let bone = r.bone_joint(owner, crate::game::ft::ness::PART_R2ND_NA);
    make_links(r, gobj, bone);
    pick_up(r, gobj, owner, crate::game::ft::ness::PART_R2ND_NA);
    set_spin(r, gobj, 0.0);
    Some(gobj)
}

/// Where a link's joint is (`PSMTXConcat` of its matrix with an identity).
fn joint_point(r: &mut Running, l: ItemLinkId) -> Vec3 {
    let j = lk(r.game, l).jobj;
    jobj::setup_matrix(&mut r.game.objs, r.data, j);
    let j = j.expect("a link without its hand's joint");
    let m = r.game.objs.jobjs[j].mtx.concat(&Mtx34::IDENTITY);
    Vec3::new(m.m[0][3], m.m[1][3], m.m[2][3])
}

/// The tip and the link at the hand.
fn ends(r: &mut Running, gobj: GObjId) -> (ItemLinkId, ItemLinkId) {
    let v = item_mut(r.game, gobj).xDD4_itemVar.ness_yoyo();
    (
        v.x8.get().expect("a yo-yo without its tip"),
        v.xC.get().expect("a yo-yo without its hand's link"),
    )
}

/// `itNessyoyo_UnkMotion1_Phys`: spinning out, the tip at Ness's yo-yo hitbox and the links
/// strung toward the hand.
#[doc(alias = "itNessyoyo_UnkMotion1_Phys")]
pub fn physics_1(r: &mut Running, gobj: GObjId) {
    let (tip, _) = ends(r, gobj);
    let pos = joint_point(r, tip);
    spin_out_step(r, gobj, tip, pos);
    line_up(r, gobj, pos);
}

/// `itNessyoyo_UnkMotion2_Phys`: dragged along the ground.
#[doc(alias = "itNessyoyo_UnkMotion2_Phys")]
pub fn physics_2(r: &mut Running, gobj: GObjId) {
    let (tip, _) = ends(r, gobj);
    let pos = joint_point(r, tip);
    drag(r, gobj, tip, pos);
    line_up(r, gobj, pos);
}

/// `itNessyoyo_UnkMotion3_Phys`: reeling in; once all in, spinning out again from the hand.
#[doc(alias = "itNessyoyo_UnkMotion3_Phys")]
pub fn physics_3(r: &mut Running, gobj: GObjId) {
    let (_, hand) = ends(r, gobj);
    let pos = joint_point(r, hand);
    let speed = item_mut(r.game, gobj).xDD4_itemVar.ness_yoyo().x4.get();
    if reel(r, gobj, hand, pos, speed) {
        spin_out(r, gobj, Vec3::new(0.0, 0.0, 0.0));
        let v = item_mut(r.game, gobj).xDD4_itemVar.ness_yoyo();
        let ness = v.x10.get().expect("a yo-yo without Ness");
        let tip = v.x8.get().expect("a yo-yo without its tip");
        let p = attack_hi4::hitbox_pos(r.game.fighter(ness));
        link_mut(r.game, tip).pos = p;
    }
    line_up(r, gobj, pos);
}

/// `itNessyoyo_UnkMotion3_Anim` (every state's): the yo-yo's model spinning by Ness's rate;
/// once Ness leaves the smashes, the yo-yo goes.
#[doc(alias = "itNessyoyo_UnkMotion3_Anim")]
pub fn animate(r: &mut Running, gobj: GObjId) -> bool {
    let v = item_mut(r.game, gobj).xDD4_itemVar.ness_yoyo();
    let ness = v.x10.get().expect("a yo-yo without Ness");
    let model = v.x18.get();
    let child = model.and_then(|m| r.game.objs.jobjs[m].child);
    if let Some(c) = child {
        let rot = r.game.objs.jobjs[c].rotate.x;
        let rate = attack_hi4::spin_rate(r.game.fighter(ness));
        let rot = mem(fp::fadds(reg(rot), reg(rate)));
        jobj::set_rotation_x(&mut r.game.objs.jobjs, c, rot);
    }
    let stays = match item(r.game, gobj).owner {
        Some(o) => (0x156..=0x15B).contains(&r.game.ft_motion_id(o)),
        None => false,
    };
    if stays {
        return false;
    }
    release_links(r, gobj);
    true
}

/// A link's airborne pass, `it_802BF030`: the touch noted, sliding along a floor, nudged up
/// by `offset` off a wall or ceiling when out between links; the floors and walls touched.
#[doc(alias = "it_802BF030")]
pub fn link_coll(r: &mut Running, l: ItemLinkId, flag: bool, offset: f32) -> i32 {
    let k = link_mut(r.game, l);
    k.coll_data.last_pos = k.coll_data.cur_pos;
    k.coll_data.cur_pos = k.pos;
    let mut cd = k.coll_data.clone();
    let hit = with_stage_coll(r.game, r.data, |c| c.air_frame(&mut cd, 6, 0));
    link_mut(r.game, l).coll_data = cd;
    let k = link_mut(r.game, l);
    if hit {
        if !k.x2C_b1 && flag && !k.x2C_b2 {
            k.x2C_b2 = true;
        }
        k.x2C_b1 = true;
    } else {
        k.x2C_b1 = false;
    }
    let env = k.coll_data.env_flags;
    if env & 0x18000 != 0 {
        k.coll_data.cur_pos.x = k.pos.x;
    } else if env & 0xFFF != 0 {
        let (next, prev) = (k.next, k.prev);
        if next.is_some_and(|n| lk(r.game, n).x2C_b0) && prev.is_some() {
            let k = link_mut(r.game, l);
            k.coll_data.cur_pos.y = mem(fp::fadds(reg(k.coll_data.cur_pos.y), reg(offset)));
        }
    }
    let k = link_mut(r.game, l);
    k.pos = k.coll_data.cur_pos;
    k.coll_data.env_flags & 0x18FFF
}

/// `pos = target + dir × length`.
fn along(dir: Vec3, length: f64, target: Vec3) -> Vec3 {
    Vec3::new(
        mem(fp::fmadds(reg(dir.x), length, reg(target.x))),
        mem(fp::fmadds(reg(dir.y), length, reg(target.y))),
        mem(fp::fmadds(reg(dir.z), length, reg(target.z))),
    )
}

/// Pulls `cur` to `length` from `target` and lets the links past it settle on the stage,
/// each held within `length` of the one before, `it_802BF180`.
#[doc(alias = "it_802BF180")]
pub fn pull_in(r: &mut Running, mut cur: ItemLinkId, target: Vec3, length: f64) {
    let mut prev = lk(r.game, cur).prev;
    let mut dir = Vec3::default();
    direction(lk(r.game, cur).pos, target, &mut dir);
    link_mut(r.game, cur).pos = along(dir, length, target);
    while let Some(p) = prev {
        link_mut(r.game, p).vel.y = 0.0;
        apply_vel(r.game, p);
        move_coll(r.game, p);
        link::coll_air(r, p);
        let to = lk(r.game, cur).pos;
        if direction(lk(r.game, p).pos, to, &mut dir) > length {
            link_mut(r.game, p).pos = along(dir, length, to);
        }
        cur = p;
        prev = lk(r.game, p).prev;
    }
}

/// Spinning out, `it_802BF28C`: the tip at Ness's yo-yo hitbox, each link out held between
/// `x10` and `xC` (scaled) from the one before, the links in following once the string is
/// that long. 0 while links are still in, else 2.
#[doc(alias = "it_802BF28C")]
pub fn spin_out_step(r: &mut Running, gobj: GObjId, l: ItemLinkId, target: Vec3) -> i32 {
    let scale = reg(owner_scale(r, gobj));
    let min = fp::fmuls(reg(attr_f32(r, gobj, attr::UNK1)), scale);
    let max = fp::fmuls(reg(attr_f32(r, gobj, attr::SIZE)), scale);
    let ness = item_mut(r.game, gobj)
        .xDD4_itemVar
        .ness_yoyo()
        .x10
        .get()
        .expect("a yo-yo without Ness");
    let p = attack_hi4::hitbox_pos(r.game.fighter(ness));
    let mut cur = l;
    let k = link_mut(r.game, cur);
    k.pos = p;
    k.vel = Vec3::new(0.0, 0.0, 0.0);
    let mut next = k.next;
    let (mut dir, mut dir2) = (Vec3::default(), Vec3::default());
    while let Some(n) = next {
        if lk(r.game, n).x2C_b0 {
            link_mut(r.game, n).pos = target;
            let from = lk(r.game, cur).pos;
            let dist = direction(target, from, &mut dir);
            if dist > max {
                link_mut(r.game, n).pos = along(dir, max, from);
            } else if dist < min {
                if direction(target, target, &mut dir2) <= f64::from(0.1_f32) {
                    link_mut(r.game, n).x2C_b0 = false;
                } else {
                    link_mut(r.game, n).pos = along(dir, min, from);
                }
            }
        } else {
            let from = lk(r.game, cur).pos;
            if direction(target, from, &mut dir) > max {
                let k = link_mut(r.game, n);
                k.pos = along(dir, max, from);
                k.x2C_b0 = true;
            } else {
                return 0;
            }
        }
        cur = n;
        next = lk(r.game, n).next;
    }
    2
}

/// Dragged along the ground, `it_802BF4A0`: the tip pulled forward and falling, sliding on the
/// floor, the links out following and falling, the links in leaving one by one past the
/// smash's string length; 1 when a wall stops it with links still in, 0 while links are in,
/// else 2.
#[doc(alias = "it_802BF4A0")]
pub fn drag(r: &mut Running, gobj: GObjId, l: ItemLinkId, target: Vec3) -> i32 {
    let size = fp::fmuls(
        reg(attr_f32(r, gobj, attr::SIZE)),
        reg(owner_scale(r, gobj)),
    );
    let (pull, unk3) = (
        reg(attr_f32(r, gobj, attr::PULL_STRENGTH)),
        reg(attr_f32(r, gobj, attr::UNK3_MOD)),
    );
    let (gravity, gravity_mod, pull2) = (
        reg(attr_f32(r, gobj, attr::GRAVITY)),
        reg(attr_f32(r, gobj, attr::GRAVITY_MOD)),
        reg(attr_f32(r, gobj, attr::PULL_STRENGTH_2)),
    );
    let (up_len, down_len) = (
        attr_i32(r, gobj, attr::UPSMASH_CHARGE_STRING_LENGTH),
        attr_i32(r, gobj, attr::DOWNSMASH_CHARGE_STRING_LENGTH),
    );
    let facing = reg(item(r.game, gobj).facing_dir);
    let abs = |v: f64| if v < 0.0 { fp::fneg(v) } else { v };
    let mut cur = l;
    let mut next = lk(r.game, l).next;
    let k = link_mut(r.game, l);
    if abs(reg(k.vel.x)) < unk3 {
        k.vel.x = mem(fp::fmadds(pull, facing, reg(k.vel.x)));
    } else {
        k.vel.x = mem(fp::fmuls(unk3, facing));
    }
    if abs(reg(k.vel.y)) < gravity_mod {
        k.vel.y = mem(fp::fsubs(reg(k.vel.y), gravity));
    } else {
        k.vel.y = mem(fp::fneg(gravity_mod));
    }
    apply_vel(r.game, cur);
    let mut coll_flags = link_coll(r, cur, false, mem(size));
    if coll_flags & 0x18000 != 0 {
        let k = link_mut(r.game, l);
        k.coll_data.cur_pos.y = mem(fp::fadds(reg(k.coll_data.cur_pos.y), f64::from(0.001_f32)));
        k.pos.y = k.coll_data.cur_pos.y;
        if k.vel.y < 0.0 {
            k.vel.y = 0.0;
        }
    }
    coll_flags &= 0xFFF;
    let smash = item_mut(r.game, gobj).xDD4_itemVar.ness_yoyo().x0.get();
    let mut count = 0;
    let mut dir = Vec3::default();
    while let Some(n) = next {
        count += 1;
        if lk(r.game, n).x2C_b0 {
            let k = link_mut(r.game, n);
            k.vel.y = mem(fp::fsubs(reg(k.vel.y), pull2));
            apply_vel(r.game, n);
            // The original passes the previous link here.
            link_coll(r, cur, false, mem(size));
            let from = lk(r.game, cur).pos;
            if direction(lk(r.game, n).pos, from, &mut dir) > size {
                link_mut(r.game, n).pos = along(dir, size, from);
            }
            move_coll(r.game, n);
            let y = mem(fp::frsp(fp::fadd(1.5, reg(target.y))));
            link::coll_air_from(r, n, y);
        } else {
            let len = if smash == MS_ATTACK_HI4 {
                up_len
            } else {
                down_len
            };
            let from = lk(r.game, cur).pos;
            if count > len {
                link_mut(r.game, n).pos = target;
            } else if direction(target, from, &mut dir) > size {
                let k = link_mut(r.game, n);
                k.pos = along(dir, size, from);
                k.x2C_b0 = true;
                place_coll(r.game, n);
            } else if coll_flags != 0 {
                return 1;
            } else {
                return 0;
            }
        }
        cur = n;
        next = lk(r.game, n).next;
    }
    // `it_802BF4A0_adjust_tail`.
    let mut dir2 = Vec3::default();
    let mut tail = cur;
    let mut prev = lk(r.game, cur).prev;
    direction(lk(r.game, tail).pos, target, &mut dir2);
    link_mut(r.game, cur).pos = along(dir2, size, target);
    while let Some(p) = prev {
        let to = lk(r.game, tail).pos;
        if direction(lk(r.game, p).pos, to, &mut dir2) > size {
            link_mut(r.game, p).pos = along(dir2, size, to);
        }
        tail = p;
        prev = lk(r.game, p).prev;
    }
    let k = link_mut(r.game, l);
    k.vel.x = mem(fp::fmuls(reg(k.vel.x), f64::from(0.9_f32)));
    2
}

/// Reeling in, `it_802BF800`: the links within `dist` of the hand go in, the first out pulled
/// in by up to the link length; whether all are in.
#[doc(alias = "it_802BF800")]
pub fn reel(r: &mut Running, gobj: GObjId, l: ItemLinkId, pos: Vec3, dist: f32) -> bool {
    let size = fp::fmuls(
        reg(attr_f32(r, gobj, attr::SIZE)),
        reg(owner_scale(r, gobj)),
    );
    let mut prev = lk(r.game, l).prev;
    let mut cur = l;
    while let Some(p) = prev {
        if lk(r.game, cur).x2C_b0 {
            break;
        }
        cur = p;
        prev = lk(r.game, p).prev;
    }
    let mut dir = Vec3::default();
    let mut len = direction(lk(r.game, cur).pos, pos, &mut dir);
    let dist = reg(dist);
    while let Some(p) = prev {
        if !(dist > len) {
            break;
        }
        link_mut(r.game, cur).x2C_b0 = false;
        len = direction(lk(r.game, p).pos, pos, &mut dir);
        cur = p;
        prev = lk(r.game, p).prev;
    }
    let step = if fp::fsubs(len, dist) > size {
        size
    } else {
        fp::fsubs(len, dist)
    };
    pull_in(r, cur, pos, step);
    prev.is_none()
}

/// Hanging, `it_802BF900` (`itNessyoyo_UnkMotion0_Phys`): the tip's model at the hand, 1.8
/// along its z axis and scaled, with Ness.
#[doc(alias = "it_802BF900")]
#[doc(alias = "itNessyoyo_UnkMotion0_Phys")]
pub fn hang(r: &mut Running, gobj: GObjId) {
    let (tip, _) = ends(r, gobj);
    let target = match r
        .game
        .world
        .gobj(lk(r.game, tip).gobj.expect("a link without its GObj"))
        .obj
    {
        Some(Obj::JObj(j)) => j,
        ref other => panic!("a yo-yo's tip holds {other:?}"),
    };
    let scl = reg(owner_scale(r, gobj));
    let j = lk(r.game, tip).jobj;
    jobj::setup_matrix(&mut r.game.objs, r.data, j);
    let j = j.expect("a link without its hand's joint");
    let mut trans = Mtx34::IDENTITY;
    trans.m[0][3] = mem(fp::fmuls(0.0, scl));
    trans.m[1][3] = mem(fp::fmuls(0.0, scl));
    trans.m[2][3] = mem(fp::fmuls(f64::from(1.8_f32), scl));
    let trans = r.game.objs.jobjs[j].mtx.concat(&trans);
    let mut scale = Mtx34::IDENTITY;
    for row in &mut scale.m {
        for v in row.iter_mut().take(3) {
            *v = mem(fp::fmuls(reg(*v), scl));
        }
    }
    let m = trans.concat(&scale);
    let jobjs = &mut r.game.objs.jobjs;
    jobjs[target].mtx = m;
    jobjs[target].flags |= jobj::USER_DEF_MTX | jobj::MTX_INDEP_PARENT | jobj::MTX_INDEP_SRT;
    jobj::set_mtx_dirty(jobjs, Some(target));
}

/// Lines the links' models up, `it_802BFAFC`: from the first link out back to the hand, each
/// at its position, along the line between its neighbours (the yo-yo facing its way, and
/// Ness's yo-yo hitbox at it), scaled.
#[doc(alias = "it_802BFAFC")]
pub fn line_up(r: &mut Running, gobj: GObjId, target: Vec3) {
    let v = item_mut(r.game, gobj).xDD4_itemVar.ness_yoyo();
    let mut cur = v.xC.get();
    let model = v.x18.get();
    let ness = v.x10.get().expect("a yo-yo without Ness");
    while let Some(l) = cur {
        if lk(r.game, l).x2C_b0 {
            break;
        }
        cur = lk(r.game, l).prev;
    }
    let size = attr_f32(r, gobj, attr::SIZE);
    let facing = item(r.game, gobj).facing_dir;
    while let Some(l) = cur {
        let k = lk(r.game, l);
        let next_pos = match k.next {
            Some(n) if lk(r.game, n).x2C_b0 => lk(r.game, n).pos,
            _ => target,
        };
        let prev_pos = match k.prev {
            Some(p) => lk(r.game, p).pos,
            None => k.pos,
        };
        let j = match r
            .game
            .world
            .gobj(k.gobj.expect("a link without its GObj"))
            .obj
        {
            Some(Obj::JObj(j)) => j,
            ref other => panic!("a link holds {other:?}"),
        };
        let link_pos = k.pos;
        jobj::set_translate(&mut r.game.objs.jobjs, j, link_pos);
        let mut dir = prev_pos - next_pos;
        dir.normalize();
        let is_yoyo = Some(j) == model;
        if is_yoyo {
            dir.x = if facing == 1.0 { 1.0 } else { -1.0 };
            dir.z = 0.0;
            dir.y = 0.0;
            attack_hi4::set_hitbox_pos(r.game.fighter_mut(ness), link_pos);
        }
        link::orient(r.game, j, link_pos, &mut dir);
        let scale = if is_yoyo { 1.0 } else { size };
        let scale = fp::fmuls(reg(scale), reg(owner_scale(r, gobj)));
        let m = &mut r.game.objs.jobjs[j].mtx;
        for row in &mut m.m {
            for v in row.iter_mut().take(3) {
                *v = mem(fp::fmuls(reg(*v), scale));
            }
        }
        cur = lk(r.game, l).prev;
    }
}

/// Let go of for the charge, `it_802BFE5C`: the tip off at `vel`, forward the way it faces
/// (state 2).
#[doc(alias = "it_802BFE5C")]
pub fn release(r: &mut Running, gobj: GObjId, vel: Vec3) {
    let (tip, _) = ends(r, gobj);
    let facing = reg(item(r.game, gobj).facing_dir);
    let k = link_mut(r.game, tip);
    k.vel = vel;
    let vx = reg(k.vel.x);
    let vx = if vx < 0.0 { fp::fneg(vx) } else { vx };
    k.vel.x = mem(fp::fmuls(facing, vx));
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// Reeling in, `it_802BFEC4`: at a tenth a frame of the tip's distance from the hand (state 3).
#[doc(alias = "it_802BFEC4")]
pub fn nudge(r: &mut Running, gobj: GObjId) {
    let (tip, hand) = ends(r, gobj);
    let at = joint_point(r, hand);
    let p = lk(r.game, tip).pos;
    let dx = fp::fsubs(reg(p.x), reg(at.x));
    let dy = fp::fsubs(reg(p.y), reg(at.y));
    let d = crate::math::sqrt_reg(fp::fadds(fp::fmuls(dx, dx), fp::fmuls(dy, dy)));
    let v = item_mut(r.game, gobj).xDD4_itemVar.ness_yoyo();
    v.x4.set(mem(d));
    v.x4.update(|x| mem(fp::fmuls(reg(x), f64::from(0.1_f32))));
    change_state(r, gobj, 3, change::ANIM_UPDATE);
}

/// Spinning out from the hand, `it_802C0010`: the tip at the hand, out, at `vel` (state 1).
#[doc(alias = "it_802C0010")]
pub fn spin_out(r: &mut Running, gobj: GObjId, vel: Vec3) {
    let (tip, _) = ends(r, gobj);
    let pos = joint_point(r, tip);
    let k = link_mut(r.game, tip);
    k.pos = pos;
    k.coll_data.cur_pos = k.pos;
    k.coll_data.last_pos = k.coll_data.cur_pos;
    k.x2C_b0 = true;
    k.vel = vel;
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// Forgets a GObj the yo-yo points to, `it_2725_Logic59_EvtUnk` (`it_8026B894`).
// Ports `it_2725_Logic59_EvtUnk`; aliased once a check reaches it.
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
