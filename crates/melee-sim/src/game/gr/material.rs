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
            objs.mobjs[m].class = MObjClass::Ground;
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
