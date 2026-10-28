//! Items made of links (the Ice Climbers' string, Ness's yo-yo, Samus's grapple beam, Link's
//! hookshot, Sheik's chain): each link is an `ItemLink`, the user data (kind 6) of its own GObj
//! of class 7 that holds the link's model, from `item_link_alloc_data`
//! ([`Globals::links`](super::Globals::links)). The links point to their neighbours; the item's
//! variables point to the ends.
//!
//! These are the helpers the hookshot's unit (`itlinkhookshot.c`) keeps for every such item,
//! and the link's reset (`it_80275328`, `it_2725.c`).

use gekko_fp as fp;

use super::super::handles::ItemLinkId;
use super::super::mirror::{Cx, Mem, Mirror};
use super::super::state::{Game, Running};
use super::super::structs::ItemLink;
use super::maplib::with_stage_coll;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::math;
use crate::mtx::Mtx34;
use crate::vec::Vec3;

/// The GObj class of links (`GObj_Create(7, ...)`).
pub const CLASS: u16 = 7;
/// Links' user data kind.
pub const USER_DATA_KIND: u8 = 6;
/// A link's user data release, `it_802A2474`.
pub const FREE: u32 = 0x802A_2474;
/// A link's render callback, `it_802A24A0`.
pub const RENDER: u32 = 0x802A_24A0;

/// A link's `x20`, which only the reset (`it_80275328`) writes, as zero, and nothing reads.
/// Link's hookshot sets its links up without the reset, so a hookshot link in a block new from
/// the heap keeps the heap's bytes there, which melee-hd doesn't model: `None` until written
/// (or imported), and the comparer checks it only then (docs/engine.md).
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct LinkX20(pub Option<Vec3>);

impl Mirror for LinkX20 {
    const SIZE: u32 = 0xC;

    fn import(m: &Mem, at: u32, cx: &mut dyn Cx) -> Self {
        LinkX20(Some(Vec3::import(m, at, cx)))
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        if let Some(v) = &self.0 {
            v.compare(m, at, path, cx);
        }
    }
}

impl crate::game::mirror::LayOut for LinkX20 {
    /// Once written; until then the block's bytes, which the layout keeps.
    fn lay(&self, l: &mut crate::hsd::export::Lay, at: u32, cx: &dyn crate::game::mirror::LayCx) {
        if let Some(v) = &self.0 {
            crate::game::mirror::LayOut::lay(v, l, at, cx);
        }
    }
}

/// The link.
pub fn link(game: &Game, l: ItemLinkId) -> &ItemLink {
    &game.it.links[l.0]
}

pub fn link_mut(game: &mut Game, l: ItemLinkId) -> &mut ItemLink {
    &mut game.it.links[l.0]
}

/// The link a GObj of class 7 holds.
pub fn link_of(game: &Game, gobj: GObjId) -> ItemLinkId {
    match game.world.gobj(gobj).user_data {
        Some(super::super::state::UserData::ItemLink(l)) => l,
        ref other => panic!("a link GObj whose user data is {other:?}"),
    }
}

/// Gives a link back to `item_link_alloc_data`, `it_802A2474` (the links' user data release).
#[doc(alias = "it_802A2474")]
pub fn free(game: &mut Game, data: &melee_data::archive::Archives, l: ItemLinkId) {
    // The block holds the link as it was (docs/engine.md, a session's heap leftovers).
    let g = &*game;
    crate::hsd::export::with_image(|img| crate::game::export::item_link(g, data, img, l));
    game.it.links.free(l.0);
}

/// A new link's block, `HSD_ObjAlloc(&item_link_alloc_data)`: a freed one keeps its bytes,
/// and one new from the heap in a session what the scenes before left there (docs/engine.md,
/// a session's heap leftovers).
pub fn alloc(game: &mut Game, data: &melee_data::archive::Archives) -> ItemLinkId {
    let l = ItemLinkId(game.it.links.alloc_stale(ItemLink::default()));
    let addr = game.it.links.addr(l.0);
    if let Some(left) =
        crate::game::leftovers::import(data, addr, crate::game::leftovers::pool_grew())
    {
        game.it.links[l.0] = left;
    }
    l
}

/// Resets a link for its GObj, `it_80275328`: no neighbours, still, at the origin, no model.
#[doc(alias = "it_80275328")]
pub fn reset(game: &mut Game, gobj: GObjId, l: ItemLinkId) {
    let k = link_mut(game, l);
    k.prev = None;
    k.next = None;
    k.vel = Vec3::new(0.0, 0.0, 0.0);
    k.pos = Vec3::new(0.0, 0.0, 0.0);
    k.x20 = LinkX20(Some(Vec3::new(0.0, 0.0, 0.0)));
    k.x2C_b0 = false;
    k.x2C_b1 = false;
    k.x2C_b2 = false;
    k.x1CC = 0;
    k.gobj = Some(gobj);
    k.jobj = None;
}

/// Whether a link's render callback draws its model, `it_802A24A0`'s decision: once the link
/// is out.
#[doc(alias = "it_802A24A0")]
pub fn drawn(game: &Game, gobj: GObjId) -> bool {
    link(game, link_of(game, gobj)).x2C_b0
}

impl Running<'_> {
    /// A link's render callback, `it_802A24A0`: its model ([`drawn`]).
    pub fn render_link(&mut self, gobj: GObjId, pass: i32) {
        if drawn(self.game, gobj) {
            self.render_jobj(gobj, pass);
        }
    }
}

/// Sets up a link's collision at its position, `it_802A24D0`: a fixed square ECB of `size`.
#[doc(alias = "it_802A24D0")]
pub fn init_coll(r: &mut Running, l: ItemLinkId, size: f32) {
    let k = link_mut(r.game, l);
    k.coll_data.cur_pos = k.pos;
    k.coll_data.last_pos = k.coll_data.cur_pos;
    let mut cd = k.coll_data.clone();
    with_stage_coll(r.game, r.data, |c| c.init(&mut cd));
    cd.x34_flags.b1234 = 5;
    cd.set_ecb_source_box(None, size, size, size, size);
    link_mut(r.game, l).coll_data = cd;
}

/// Sets up a link's collision at its position with a fixed square ECB of `size`, keeping the
/// collision kind the init gives (the grapple beam's `samus_grapple_init_link`, which
/// `it_802A24D0` is but for the kind).
pub fn init_coll_fixed(r: &mut Running, l: ItemLinkId, size: f32) {
    let k = link_mut(r.game, l);
    k.coll_data.cur_pos = k.pos;
    k.coll_data.last_pos = k.coll_data.cur_pos;
    let mut cd = k.coll_data.clone();
    with_stage_coll(r.game, r.data, |c| c.init(&mut cd));
    cd.set_ecb_source_box(None, size, size, size, size);
    link_mut(r.game, l).coll_data = cd;
}

/// The unit direction from `b` to `a` written to `dir`, and the distance (in a register),
/// `it_802A3C98`.
#[doc(alias = "it_802A3C98")]
pub fn direction(a: Vec3, b: Vec3, dir: &mut Vec3) -> f64 {
    dir.x = mem(fp::fsubs(reg(a.x), reg(b.x)));
    dir.y = mem(fp::fsubs(reg(a.y), reg(b.y)));
    dir.z = mem(fp::fsubs(reg(a.z), reg(b.z)));
    let (x, y, z) = (reg(dir.x), reg(dir.y), reg(dir.z));
    let len = math::sqrt_reg(fp::fadds(
        fp::fmuls(z, z),
        fp::fadds(fp::fmuls(x, x), fp::fmuls(y, y)),
    ));
    let inv = if len == 0.0 {
        fp::frsp(0.0)
    } else {
        fp::frsp(fp::fdiv(1.0, len))
    };
    dir.x = mem(fp::fmuls(reg(dir.x), inv));
    dir.y = mem(fp::fmuls(reg(dir.y), inv));
    dir.z = mem(fp::fmuls(reg(dir.z), inv));
    len
}

/// The link's collision starts again at its position, `it_802A43B8`.
#[doc(alias = "it_802A43B8")]
pub fn place_coll(game: &mut Game, l: ItemLinkId) {
    let k = link_mut(game, l);
    k.coll_data.cur_pos = k.pos;
    k.coll_data.last_pos = k.coll_data.cur_pos;
}

/// The link's collision moves to its position, `it_802A43EC`.
#[doc(alias = "it_802A43EC")]
pub fn move_coll(game: &mut Game, l: ItemLinkId) {
    let k = link_mut(game, l);
    k.coll_data.last_pos = k.coll_data.cur_pos;
    k.coll_data.cur_pos = k.pos;
}

/// The link moves by its velocity, `it_802A4420`.
#[doc(alias = "it_802A4420")]
pub fn apply_vel(game: &mut Game, l: ItemLinkId) {
    let k = link_mut(game, l);
    k.pos.x = mem(fp::fadds(reg(k.pos.x), reg(k.vel.x)));
    k.pos.y = mem(fp::fadds(reg(k.pos.y), reg(k.vel.y)));
    k.pos.z = mem(fp::fadds(reg(k.pos.z), reg(k.vel.z)));
}

/// Points a joint along `dir` (normalized in place) at `pos`, `it_802A6DC8`: its own matrix,
/// independent of its parent's.
#[doc(alias = "it_802A6DC8")]
pub fn orient(game: &mut Game, j: JObjId, pos: Vec3, dir: &mut Vec3) {
    dir.normalize();
    let mut up = if f64::from(dir.y) > 0.9999 {
        Vec3::new(-1.0, 0.0, 0.0)
    } else if f64::from(dir.y) < fp::fneg(0.9999) {
        Vec3::new(1.0, 0.0, 0.0)
    } else {
        Vec3::new(0.0, 1.0, 0.0)
    };
    let mut side = up.sdk_cross(*dir);
    up = dir.sdk_cross(side);
    side.normalize();
    up.normalize();
    let mut m = Mtx34::IDENTITY;
    m.m[0][0] = side.x;
    m.m[1][0] = side.y;
    m.m[2][0] = side.z;
    m.m[0][1] = up.x;
    m.m[1][1] = up.y;
    m.m[2][1] = up.z;
    m.m[0][2] = dir.x;
    m.m[1][2] = dir.y;
    m.m[2][2] = dir.z;
    m.m[0][3] = pos.x;
    m.m[1][3] = pos.y;
    m.m[2][3] = pos.z;
    let jobjs = &mut game.objs.jobjs;
    jobjs[j].mtx = m;
    jobjs[j].flags |= jobj::USER_DEF_MTX | jobj::MTX_INDEP_PARENT | jobj::MTX_INDEP_SRT;
    jobj::set_mtx_dirty(jobjs, Some(j));
}

/// Points a joint along `dir` (normalized in place) at `pos`, scaled by `scale`,
/// `it_802A6F80`: its own matrix, independent of its parent's.
#[doc(alias = "it_802A6F80")]
pub fn orient_scaled(game: &mut Game, j: JObjId, pos: Vec3, dir: &mut Vec3, scale: f32) {
    dir.normalize();
    let mut up = if f64::from(dir.y) > 0.9999 {
        Vec3::new(-1.0, 0.0, 0.0)
    } else if f64::from(dir.y) < fp::fneg(0.9999) {
        Vec3::new(1.0, 0.0, 0.0)
    } else {
        Vec3::new(0.0, 1.0, 0.0)
    };
    let mut side = up.sdk_cross(*dir);
    up = dir.sdk_cross(side);
    side.normalize();
    up.normalize();
    let mut s = Mtx34::IDENTITY;
    s.m[0][0] = scale;
    s.m[1][1] = scale;
    s.m[2][2] = scale;
    let mut m = Mtx34::IDENTITY;
    m.m[0][0] = side.x;
    m.m[1][0] = side.y;
    m.m[2][0] = side.z;
    m.m[0][1] = up.x;
    m.m[1][1] = up.y;
    m.m[2][1] = up.z;
    m.m[0][2] = dir.x;
    m.m[1][2] = dir.y;
    m.m[2][2] = dir.z;
    m.m[0][3] = pos.x;
    m.m[1][3] = pos.y;
    m.m[2][3] = pos.z;
    let m = m.concat(&s);
    let jobjs = &mut game.objs.jobjs;
    jobjs[j].mtx = m;
    jobjs[j].flags |= jobj::USER_DEF_MTX | jobj::MTX_INDEP_PARENT | jobj::MTX_INDEP_SRT;
    jobj::set_mtx_dirty(jobjs, Some(j));
}

/// Lines the links' models up from the first link out at the hand end, `it_802A7168`: each at
/// its position, along the line from the next link out (or the hand's point `at`) to the one
/// before (or itself), scaled by `scale`.
#[doc(alias = "it_802A7168")]
pub fn line_up_models(r: &mut Running, hand: ItemLinkId, at: Vec3, scale: f32) {
    let mut l = Some(hand);
    while let Some(k) = l {
        if link(r.game, k).x2C_b0 {
            break;
        }
        l = link(r.game, k).prev;
    }
    while let Some(k) = l {
        let lk = link(r.game, k);
        let from = match lk.next {
            Some(n) if link(r.game, n).x2C_b0 => link(r.game, n).pos,
            _ => at,
        };
        let to = match lk.prev {
            Some(p) => link(r.game, p).pos,
            None => lk.pos,
        };
        let pos = lk.pos;
        let g = lk.gobj.expect("a link without its GObj");
        let j = match r.game.world.gobj(g).obj {
            Some(super::super::state::Obj::JObj(j)) => j,
            ref o => panic!("a link GObj holding {o:?}"),
        };
        jobj::set_translate(&mut r.game.objs.jobjs, j, pos);
        let mut dir = Vec3::new(
            mem(fp::fsubs(reg(to.x), reg(from.x))),
            mem(fp::fsubs(reg(to.y), reg(from.y))),
            mem(fp::fsubs(reg(to.z), reg(from.z))),
        );
        orient_scaled(r.game, j, pos, &mut dir, scale);
        l = link(r.game, k).prev;
    }
}

/// The touch flags the links' airborne passes report: floors and walls.
const FLOOR_AND_WALLS: i32 = 0x8000 | 0x10000 | 0x3F | 0xFC0;

/// Runs the airborne pass (`mpColl_800471F8`) on a link's collision data, from where its last
/// pass left it to its position, which takes where the pass leaves it.
fn coll_air_pass(r: &mut Running, l: ItemLinkId) -> i32 {
    let k = link_mut(r.game, l);
    k.coll_data.cur_pos = k.pos;
    let mut cd = k.coll_data.clone();
    let hit = with_stage_coll(r.game, r.data, |c| c.air_frame(&mut cd, 6, 0));
    let k = link_mut(r.game, l);
    k.coll_data = cd;
    k.x2C_b1 = hit;
    k.pos = k.coll_data.cur_pos;
    k.coll_data.env_flags & FLOOR_AND_WALLS
}

/// A link's airborne pass from its last to its position, `it_802A3D90`: the floors and walls it
/// touched.
#[doc(alias = "it_802A3D90")]
pub fn coll_air(r: &mut Running, l: ItemLinkId) -> i32 {
    let k = link_mut(r.game, l);
    k.coll_data.last_pos = k.coll_data.cur_pos;
    coll_air_pass(r, l)
}

/// [`coll_air`] from `y` instead of its last height, `it_802A42F4`.
#[doc(alias = "it_802A42F4")]
pub fn coll_air_from(r: &mut Running, l: ItemLinkId, y: f32) -> i32 {
    let k = link_mut(r.game, l);
    k.coll_data.last_pos = k.coll_data.cur_pos;
    k.coll_data.last_pos.y = y;
    coll_air_pass(r, l)
}
