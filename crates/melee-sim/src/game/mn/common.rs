//! Helpers the rules' and names' screens share: their joints' animation, lookups and
//! translations as the screens' code calls them (`HSD_JObj*`'s inlines, `lb_80011E24`).

use super::super::gm::title::AnimLoop;
use super::super::state::{Obj, Running};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};

/// `JOBJ_HIDDEN`.
pub(crate) const HIDDEN: u32 = 1 << 4;
/// `TOBJ_MASK`.
pub(crate) const TOBJ_MASK: u32 = jobj::type_mask::TOBJ;

/// `GM_TOURNAMENT`.
pub(crate) const GM_TOURNAMENT: u8 = 0x1B;

pub(crate) const fn al(start: f32, end: f32, loop_frame: f32) -> AnimLoop {
    AnimLoop {
        start,
        end,
        loop_frame,
    }
}

/// A GObj's joint tree.
pub(crate) fn jobj_of(r: &Running, g: GObjId) -> JObjId {
    match r.game.world.gobj(g).obj {
        Some(Obj::JObj(j)) => j,
        other => panic!("a menu GObj holds {other:?}"),
    }
}

/// `HSD_JObjAnimAll`.
pub(crate) fn anim_all(r: &mut Running, j: Option<JObjId>) {
    jobj::anim_all(
        &mut r.game.objs,
        r.data,
        &mut super::super::code::ParticleHooks,
        j,
    );
}

/// `HSD_JObjAnim`.
pub(crate) fn anim(r: &mut Running, j: Option<JObjId>) {
    jobj::anim(
        &mut r.game.objs,
        r.data,
        &mut super::super::code::ParticleHooks,
        j,
    );
}

/// `HSD_JObjReqAnimAll`.
pub(crate) fn req_all(r: &mut Running, j: Option<JObjId>, frame: f32) {
    jobj::req_anim_all(&mut r.game.objs, j, frame);
}

/// `HSD_JObjReqAnim`.
pub(crate) fn req(r: &mut Running, j: Option<JObjId>, frame: f32) {
    jobj::req_anim(&mut r.game.objs, j, frame);
}

/// `HSD_JObjReqAnimAll` then `HSD_JObjAnimAll`.
pub(crate) fn set_frame_all(r: &mut Running, j: Option<JObjId>, frame: f32) {
    req_all(r, j, frame);
    anim_all(r, j);
}

/// `HSD_JObjReqAnim` then `HSD_JObjAnim`.
pub(crate) fn set_frame(r: &mut Running, j: Option<JObjId>, frame: f32) {
    req(r, j, frame);
    anim(r, j);
}

/// `lb_80011E24(root, &out, index, -1)`: the joint `index` steps into the tree in pre-order.
pub(crate) fn node(r: &Running, root: Option<JObjId>, index: i32) -> Option<JObjId> {
    let [n] = super::super::gm::pause::tree_nodes(&r.game.objs.jobjs, root, &[index]);
    n
}

/// `HSD_JObjSetFlagsAll(j, JOBJ_HIDDEN)`.
pub(crate) fn hide(r: &mut Running, j: Option<JObjId>) {
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, HIDDEN);
}

/// `HSD_JObjClearFlagsAll(j, JOBJ_HIDDEN)`.
pub(crate) fn show(r: &mut Running, j: Option<JObjId>) {
    jobj::clear_flags_all(&mut r.game.objs.jobjs, j, HIDDEN);
}

/// A joint's first child, none for none (`HSD_JObjGetChild` and its null tests).
pub(crate) fn child(r: &Running, j: Option<JObjId>) -> Option<JObjId> {
    j.and_then(|j| r.game.objs.jobjs[j].child)
}

/// A joint's next sibling, none for none.
pub(crate) fn next(r: &Running, j: Option<JObjId>) -> Option<JObjId> {
    j.and_then(|j| r.game.objs.jobjs[j].next)
}

/// `HSD_JObjGetTranslationX`.
pub(crate) fn tx(r: &Running, j: Option<JObjId>) -> f32 {
    r.game.objs.jobjs[j.expect("the original's assertion failed: jobj")]
        .translate
        .x
}

/// `HSD_JObjGetTranslationY`.
pub(crate) fn ty(r: &Running, j: Option<JObjId>) -> f32 {
    r.game.objs.jobjs[j.expect("the original's assertion failed: jobj")]
        .translate
        .y
}

/// `HSD_JObjSetTranslateX`.
pub(crate) fn set_tx(r: &mut Running, j: Option<JObjId>, v: f32) {
    let j = j.expect("the original's assertion failed: jobj");
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.x = v);
}

/// `HSD_JObjSetTranslateY`.
pub(crate) fn set_ty(r: &mut Running, j: Option<JObjId>, v: f32) {
    let j = j.expect("the original's assertion failed: jobj");
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.y = v);
}

/// `HSD_JObjAddTranslationY(j, a * b)`, which the compiler fuses: `a * b + translate.y`.
pub(crate) fn add_ty_madd(r: &mut Running, j: Option<JObjId>, a: f64, b: f64) {
    let j = j.expect("the original's assertion failed: jobj");
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| {
        o.translate.y = crate::fpu::mem(gekko_fp::fmadds(a, b, crate::fpu::reg(o.translate.y)))
    });
}

/// `HSD_JObjAddChild`.
pub(crate) fn add_child(r: &mut Running, parent: Option<JObjId>, c: Option<JObjId>) {
    jobj::add_child(&mut r.game.objs.jobjs, parent, c);
}

/// A model loaded with its animations (`HSD_JObjLoadJoint`, `HSD_JObjAddAnimAll`).
pub(crate) fn load_model(r: &mut Running, m: super::super::gm::title::ModelDesc) -> JObjId {
    let data = r.data;
    let j = jobj::load_joint(&mut r.game.objs, data, m.joint).expect("a menu model");
    jobj::add_anim_all(
        &mut r.game.objs,
        data,
        Some(j),
        m.anim,
        m.matanim,
        m.shapeanim,
    );
    j
}

/// `mn_8022F298` on an optional joint.
pub(crate) fn frame_of(r: &Running, j: Option<JObjId>) -> f32 {
    super::super::gm::title::anim_frame(r, j.expect("a menu joint"))
}

/// The game mode running, `gm_GetCurrentGameMode`.
#[doc(alias = "gm_GetCurrentGameMode")]
pub(crate) fn game_mode(r: &Running) -> u8 {
    r.game.machine.routing.curr_mode
}
