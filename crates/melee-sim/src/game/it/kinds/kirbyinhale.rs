//! An item Kirby's inhale holds, `melee/it/kinds/itkirby_2F23.c`: the inhale's catch box takes
//! an item (only a container's monster is catchable, `it_80275270`), which it pulls to his
//! mouth in a state table of its own, shrinking it as it nears, then keeps hidden at his
//! position until he spits it out as a star or swallows it (`it_2F28.c`,
//! [`super::kirbyspitstar`]). Its variables are the last words of the union, past the
//! monster's own (`itKirby2F23_ItemVars`).

use gekko_fp as fp;

use super::super::item::change_state;
use super::super::{hitbox, item, item_mut, jobj_of, lib26b1, lib2725, maplib, zako};
use crate::fpu::{mem, reg};
use crate::game::handles::{CodeRef, ItemStatesRef};
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

/// The pulled item's state callbacks (state 0), and the held item's (state 1).
pub const PULL_ANIM: u32 = 0x802F_2578;
pub const PULL_PHYS: u32 = 0x802F_2580;
pub const PULL_COLL: u32 = 0x802F_2584;
pub const HELD_ANIM: u32 = 0x802F_2888;
pub const HELD_PHYS: u32 = 0x802F_2890;
pub const HELD_COLL: u32 = 0x802F_2894;
/// Its `on_accessory` while pulled (`it_802F258C`) and while held (`it_802F289C`).
pub const PULL: u32 = 0x802F_258C;
pub const HELD: u32 = 0x802F_289C;

/// `it_803F9450`, the inhaled item's state table.
const STATES: u32 = 0x803F_9450;

/// `itKirby2F23_ItemVars`' words: what's left of its way to his mouth (`x1C8`, `x1CC`), the
/// range it shrinks within and how much (`x1D0`, `x1D4`), its pull's speeds across and up
/// (`x1D8`, `x1DC`), its scale when caught (`x1E8`) and his hold's time (`x1F4`).
mod off {
    pub const X1C8: usize = 0x1C8;
    pub const X1CC: usize = 0x1CC;
    pub const X1D0: usize = 0x1D0;
    pub const X1D4: usize = 0x1D4;
    pub const X1D8: usize = 0x1D8;
    pub const X1DC: usize = 0x1DC;
    pub const X1E8: usize = 0x1E8;
    pub const X1F4: usize = 0x1F4;
}

/// Runs one of the inhaled item's predicates, if `addr` is one.
pub fn run_pred(_r: &mut Running, _gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        PULL_ANIM => pull_anim(),
        PULL_COLL => pull_coll(),
        HELD_ANIM => held_anim(),
        HELD_COLL => held_coll(),
        _ => return None,
    })
}

/// Runs one of the inhaled item's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PULL_PHYS => pull_phys(),
        HELD_PHYS => held_phys(),
        PULL => pull(r, gobj),
        HELD => held(r, gobj),
        _ => return false,
    }
    true
}

/// The squared distance from the item to `pos`, `it_802F23AC`.
#[doc(alias = "it_802F23AC")]
pub fn distance2(r: &Running, gobj: GObjId, pos: Vec3) -> f64 {
    let p = item(r.game, gobj).pos;
    let d = |a: f32, b: f32| fp::fsubs(reg(a), reg(b));
    let (dx, dy, dz) = (d(p.x, pos.x), d(p.y, pos.y), d(p.z, pos.z));
    fp::fadds(
        fp::fmuls(dz, dz),
        fp::fadds(fp::fmuls(dx, dx), fp::fmuls(dy, dy)),
    )
}

/// Caught by Kirby's inhale, `it_802F23EC`: his, facing `facing_dir`, in its pulled state in
/// the air, blinking and still; its way to his mouth, its shrink and its pull's speeds from
/// his attributes, its scale kept, and pulled each frame ([`pull`]).
#[doc(alias = "it_802F23EC")]
pub fn catch(r: &mut Running, gobj: GObjId, owner: GObjId, facing_dir: f32) {
    let ip = item_mut(r.game, gobj);
    ip.atk_victim = Some(owner);
    ip.grab_victim = Some(owner);
    zako::set_source_player(r.game, gobj, owner);
    let ip = item_mut(r.game, gobj);
    super::super::set_flag(&mut ip.xDD0_flag, 1, false);
    ip.facing_dir = facing_dir;
    ip.xBC_itemStateContainer = Some(ItemStatesRef(STATES));
    change_state(r, gobj, 0, 0);
    maplib::enter_air(item_mut(r.game, gobj));
    // `it_802F23EC_inline`.
    let range = r.kb_pull_range(owner);
    let shrink = r.kb_pull_shrink(owner);
    let mouth = r.kb_mouth(owner);
    let pos = item(r.game, gobj).pos;
    let (sx, sy) = r.kb_pull_speeds(owner);
    let hold = r.kb_hold_time(owner);
    let j = jobj_of(r.game, gobj).expect("an item without a model");
    let scale = r.game.objs.jobjs[j].scale;
    let v = zako::bytes(r.game, gobj);
    v.set_f32(off::X1D0, range);
    v.set_f32(off::X1D4, shrink);
    v.set_f32(off::X1C8, mem(fp::fsubs(reg(pos.x), reg(mouth.x))));
    v.set_f32(off::X1CC, mem(fp::fsubs(reg(pos.y), reg(mouth.y))));
    v.set_f32(off::X1D8, sx);
    v.set_f32(off::X1DC, sy);
    v.set_f32(off::X1F4, hold);
    v.set_vec(off::X1E8, scale);
    hitbox::start_blink(r.game, gobj);
    lib2725::set_xd09(r.game, gobj, true);
    lib2725::clear_stomp(r.game, gobj);
    lib26b1::clear_dcc_b3(r.game, gobj);
    lib2725::stop_all(r.game, gobj);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(PULL));
}

/// `itKirby_2F23_UnkMotion0_Anim`.
#[doc(alias = "itKirby_2F23_UnkMotion0_Anim")]
fn pull_anim() -> bool {
    false
}

/// `itKirby_2F23_UnkMotion0_Phys`.
#[doc(alias = "itKirby_2F23_UnkMotion0_Phys")]
fn pull_phys() {}

/// `itKirby_2F23_UnkMotion0_Coll`.
#[doc(alias = "itKirby_2F23_UnkMotion0_Coll")]
fn pull_coll() -> bool {
    false
}

/// Pulled toward his mouth, `it_802F258C`: within its range it shrinks with what's left of the
/// way (`it_802F258C_scale`); it closes by at most its pull's speeds, and sits that far from
/// his mouth (`it_802F258C_update`).
#[doc(alias = "it_802F258C")]
fn pull(r: &mut Running, gobj: GObjId) {
    let owner = item(r.game, gobj)
        .grab_victim
        .expect("an inhaled item without Kirby");
    let v = zako::bytes(r.game, gobj);
    let (mut ox, mut oy) = (v.f32(off::X1C8), v.f32(off::X1CC));
    let dist = Vec3::new(ox, oy, 0.0).length();
    let range = v.f32(off::X1D0);
    if reg(dist) < reg(range) {
        let f = fp::fdivs(reg(dist), reg(range));
        let d = reg(v.f32(off::X1D4));
        let f = fp::fmuls(f, d);
        let s = reg(mem(fp::fadds(fp::fsubs(1.0, d), f)));
        let k = v.vec(off::X1E8);
        let scale = Vec3::new(
            mem(fp::fmuls(s, reg(k.x))),
            mem(fp::fmuls(s, reg(k.y))),
            mem(fp::fmuls(s, reg(k.z))),
        );
        let j = jobj_of(r.game, gobj).expect("an item without a model");
        jobj::set_scale(&mut r.game.objs.jobjs, j, scale);
    }
    let v = zako::bytes(r.game, gobj);
    let clamp = |o: f32, max: f32| {
        let o64 = reg(o);
        let abs = if o64 < fp::frsp(0.0) {
            fp::fneg(o64)
        } else {
            o64
        };
        if abs > reg(max) {
            let sign = if o64 < 0.0 { -1.0 } else { 1.0 };
            mem(fp::fmuls(reg(max), fp::frsp(sign)))
        } else {
            o
        }
    };
    ox = clamp(ox, v.f32(off::X1D8));
    oy = clamp(oy, v.f32(off::X1DC));
    let left_x = mem(fp::fsubs(reg(v.f32(off::X1C8)), reg(ox)));
    let left_y = mem(fp::fsubs(reg(v.f32(off::X1CC)), reg(oy)));
    v.set_f32(off::X1C8, left_x);
    v.set_f32(off::X1CC, left_y);
    let mouth = r.kb_mouth(owner);
    let ip = item_mut(r.game, gobj);
    ip.pos.x = mem(fp::fadds(reg(mouth.x), reg(left_x)));
    ip.pos.y = mem(fp::fadds(reg(mouth.y), reg(left_y)));
    ip.pos.z = mouth.z;
}

/// In his mouth, `it_802F2810`: its held state, hidden at his position, blinking, in the air
/// and still.
#[doc(alias = "it_802F2810")]
pub fn hold(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, 0);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(HELD));
    lib26b1::hide(r.game, gobj);
    hitbox::start_blink(r.game, gobj);
    maplib::enter_air(item_mut(r.game, gobj));
    lib2725::set_xd09(r.game, gobj, true);
    lib2725::stop_all(r.game, gobj);
}

/// `itKirby_2F23_UnkMotion1_Anim`.
#[doc(alias = "itKirby_2F23_UnkMotion1_Anim")]
fn held_anim() -> bool {
    false
}

/// `itKirby_2F23_UnkMotion1_Phys`.
#[doc(alias = "itKirby_2F23_UnkMotion1_Phys")]
fn held_phys() {}

/// `itKirby_2F23_UnkMotion1_Coll`.
#[doc(alias = "itKirby_2F23_UnkMotion1_Coll")]
fn held_coll() -> bool {
    false
}

/// Held, `it_802F289C`: at his position (`ftKb_SpecialN_800F5800`).
#[doc(alias = "it_802F289C")]
#[doc(alias = "ftKb_SpecialN_800F5800")]
fn held(r: &mut Running, gobj: GObjId) {
    let owner = item(r.game, gobj)
        .grab_victim
        .expect("an inhaled item without Kirby");
    let pos = r.game.fighter(owner).cur_pos;
    item_mut(r.game, gobj).pos = pos;
}
