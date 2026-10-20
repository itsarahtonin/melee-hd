//! Stage materials and color overlays, `melee/gr/grmaterial.c`.

use melee_data::archive::{Archives, DataRef};

use super::super::state::Game;
use super::super::structs::ColorOverlay;
use super::ground::ground_of;

use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::mobj::MObjClass;
use crate::lb::colanim;

/// `JOBJ_SPLINE | JOBJ_PTCL`: joints whose contents aren't display objects.
const NOT_DOBJS: u32 = jobj::SPLINE | jobj::PTCL;

pub use crate::lb::colanim::clear as clear_overlay;

/// Runs a stage object's color overlay for a tick (`lb_80014258`, [`colanim::step`]): its
/// own commands would go to `grMaterial_804D456C`.
pub fn run_overlay(data: &Archives, co: &mut ColorOverlay) -> bool {
    colanim::step(data, co, &mut |_, opcode| {
        unimplemented!("color script command {opcode} (grMaterial_804D456C)")
    })
}

/// Starts a color script on a stage object's overlay and runs its first tick,
/// `grMaterial_801C9604`.
#[doc(alias = "grMaterial_801C9604")]
pub fn start_script(
    game: &mut Game,
    data: &Archives,
    gobj: GObjId,
    script: Option<DataRef>,
    pri: i32,
) {
    let g = ground_of(game, gobj);
    let gp = &mut game.grounds[g.0];
    let co = &mut gp.color_overlay;
    co.script.frame_count = pri as u32;
    co.script.cursor = script;
    co.script.timer = 0;
    co.script.loop_count = 0;
    co.x7C_flag2 = false;
    co.x7C_color_enable = false;
    gp.x10_flags.b4 = false;
    run_ground_overlay(game, data, gobj);
}

/// Resets a stage object's color overlay, `grMaterial_801C95C4`.
#[doc(alias = "grMaterial_801C95C4")]
pub fn reset_overlay(game: &mut Game, gobj: GObjId) {
    let g = ground_of(game, gobj);
    let gp = &mut game.grounds[g.0];
    clear_overlay(&mut gp.color_overlay);
    gp.x10_flags.b4 = true;
}

/// Runs a stage object's color overlay for one tick, `grMaterial_801C9698`: `x10_flags.b4`
/// marks it ended.
#[doc(alias = "grMaterial_801C9698")]
pub fn run_ground_overlay(game: &mut Game, data: &Archives, gobj: GObjId) {
    let g = ground_of(game, gobj);
    let gp = &mut game.grounds[g.0];
    if run_overlay(data, &mut gp.color_overlay) {
        gp.x10_flags.b4 = true;
    }
}

/// Sets render-mode bits on a joint's own display objects' materials, `grMaterial_801C87D0`.
#[doc(alias = "grMaterial_801C87D0")]
pub fn set_own_flags(game: &mut Game, j: JObjId, flags: u32) {
    let objs = &mut game.objs;
    let mut d = jobj::get_dobj(&objs.jobjs, Some(j));
    while let Some(dobj) = d {
        let m = objs.dobjs[dobj].mobj;
        crate::hsd::mobj::set_flags(&mut objs.mobjs, m, flags);
        d = objs.dobjs[dobj].next;
    }
}

/// Sets render-mode bits on a joint tree's materials, `grMaterial_801C8858`: the joint's own,
/// then each child tree's.
#[doc(alias = "grMaterial_801C8858")]
pub fn set_tree_flags(game: &mut Game, j: Option<JObjId>, flags: u32) {
    let Some(j) = j else { return };
    set_own_flags(game, j, flags);
    let mut c = game.objs.jobjs[j].child;
    while let Some(child) = c {
        set_tree_flags(game, Some(child), flags);
        c = game.objs.jobjs[child].next;
    }
}

/// Gives a joint's own display objects' materials the stage class (`gr_mobj`), unless the
/// joint holds a spline or particles.
fn own_materials_to_stage_class(game: &mut Game, j: JObjId) {
    let objs = &mut game.objs;
    if objs.jobjs[j].flags & NOT_DOBJS != 0 {
        return;
    }
    let mut d = jobj::get_dobj(&objs.jobjs, Some(j));
    while let Some(dobj) = d {
        if let Some(m) = objs.dobjs[dobj].mobj {
            crate::hsd::mobj::change_class(&mut objs.mobjs, m, MObjClass::Ground);
        }
        d = objs.dobjs[dobj].next;
    }
}

/// Gives a joint tree's materials the stage class, `grMaterial_801C92C0`: the joint and its
/// descendants (the original recurses through its children's children, which reaches them all
/// the same).
#[doc(alias = "grMaterial_801C92C0")]
pub fn tree_to_stage_class(game: &mut Game, j: Option<JObjId>) {
    let Some(j) = j else { return };
    own_materials_to_stage_class(game, j);
    let mut c = game.objs.jobjs[j].child;
    while let Some(child) = c {
        tree_to_stage_class(game, Some(child));
        c = game.objs.jobjs[child].next;
    }
}

/// Gives a stage model's materials the stage class, `grMaterial_801C94D8`: the root's own,
/// then each child tree's, which is [`tree_to_stage_class`] again.
#[doc(alias = "grMaterial_801C94D8")]
pub fn model_to_stage_class(game: &mut Game, j: Option<JObjId>) {
    tree_to_stage_class(game, j);
}

/// Clears render-mode bits on a joint's own display objects' materials, `grMaterial_801C897C`.
#[doc(alias = "grMaterial_801C897C")]
pub fn clear_own_flags(game: &mut Game, j: Option<JObjId>, flags: u32) {
    let Some(j) = j else { return };
    let objs = &mut game.objs;
    let mut d = jobj::get_dobj(&objs.jobjs, Some(j));
    while let Some(dobj) = d {
        let m = objs.dobjs[dobj].mobj;
        crate::hsd::mobj::clear_flags(&mut objs.mobjs, m, flags);
        d = objs.dobjs[dobj].next;
    }
}

/// Clears render-mode bits on a joint tree's materials, `grMaterial_801C8A04`: the joint's
/// own, then each child tree's.
#[doc(alias = "grMaterial_801C8A04")]
pub fn clear_tree_flags(game: &mut Game, j: Option<JObjId>, flags: u32) {
    let Some(j) = j else { return };
    clear_own_flags(game, Some(j), flags);
    let mut c = game.objs.jobjs[j].child;
    while let Some(child) = c {
        clear_tree_flags(game, Some(child), flags);
        c = game.objs.jobjs[child].next;
    }
}

// The stage items' glue: a stage object's hurtbox items (`it::kinds::yaku`).

/// A hurtbox item on a stage object's joint, `grMaterial_801C8CFC` (`it_802E6AEC` with no
/// point and no fall): its first number, its state, its object, the joint, and the stage's
/// callbacks for a hit it deals, one it takes and a touch.
#[doc(alias = "grMaterial_801C8CFC")]
#[allow(clippy::too_many_arguments)]
pub fn stage_item(
    r: &mut super::super::state::Running,
    x0: i32,
    state: i32,
    g: super::super::handles::GroundId,
    joint: Option<JObjId>,
    dealt: Option<super::super::handles::CodeRef>,
    received: Option<super::super::handles::CodeRef>,
    touched: Option<super::super::handles::CodeRef>,
) -> Option<GObjId> {
    super::super::it::kinds::yaku::spawn(
        r,
        Some(g),
        x0,
        state,
        joint,
        None,
        false,
        dealt,
        received,
        touched,
    )
}

/// Sets a stage item's hurtbox, `grMaterial_801C8DE0`: its two ends' offsets and its radius.
#[doc(alias = "grMaterial_801C8DE0")]
pub fn set_item_hurtbox(
    game: &mut Game,
    item: GObjId,
    a: crate::vec::Vec3,
    b: crate::vec::Vec3,
    scale: f32,
) {
    let h = &mut super::super::it::item_mut(game, item).xACC_itemHurtbox[0];
    h.a_offset = a;
    h.b_offset = b;
    h.scale = scale;
}

/// Turns a stage item's hurtbox on, `grMaterial_801C8E08` (`it_802756E0`, its blink ended).
#[doc(alias = "grMaterial_801C8E08")]
pub fn item_hurtbox_on(game: &mut Game, item: GObjId) {
    super::super::it::hitbox::end_blink(game, item);
}

/// Turns a stage item's hurtbox off, `grMaterial_801C8E28` (`it_802756D0`, blinking).
#[doc(alias = "grMaterial_801C8E28")]
pub fn item_hurtbox_off(game: &mut Game, item: GObjId) {
    super::super::it::hitbox::start_blink(game, item);
}

/// Puts a stage item on the ground or in the air, `grMaterial_801C8E68`.
#[doc(alias = "grMaterial_801C8E68")]
pub fn item_ground_or_air(game: &mut Game, item: GObjId, ground_or_air: i32) {
    super::super::it::item_mut(game, item).ground_or_air = ground_or_air;
}

/// Destroys a stage item, `grMaterial_801C8CDC` (`Item_8026A8EC`).
#[doc(alias = "grMaterial_801C8CDC")]
pub fn destroy_item(r: &mut super::super::state::Running, item: GObjId) {
    super::super::it::item::destroy(r, item);
}

/// Puts a stage item in another state, `grMaterial_801C8D98`: its hitboxes cleared, then the
/// state with its animation (`ITEM_ANIM_UPDATE`).
#[doc(alias = "grMaterial_801C8D98")]
pub fn item_state(r: &mut super::super::state::Running, item: GObjId, state: i32) {
    super::super::it::lib2725::clear_hitboxes(r.game, r.data, item);
    super::super::it::item::change_state(
        r,
        item,
        state,
        super::super::it::item::change::ANIM_UPDATE,
    );
}

/// Whether a stage item's state script has ended, `grMaterial_801C8E48`.
#[doc(alias = "grMaterial_801C8E48")]
pub fn item_script_done(game: &Game, item: GObjId) -> bool {
    super::super::it::item(game, item)
        .x524_cmd
        .script
        .cursor
        .is_none()
}
