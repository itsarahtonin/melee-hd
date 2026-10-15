//! The pause screen (`melee/gm/gmpause.c`): a hidden model the match loads at its start,
//! shown while a player pauses, whose control stick follows the pauser's.

use gekko_fp as fp;
use melee_data::archive::{ArchiveId, Archives};

use super::super::state::{Game, OBJ_JOBJ, Obj, Running};
use crate::fpu::{mem, reg};
use crate::hsd::Objects;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId, JObjs};

/// The pause screen's process, `fn_801A0E34`.
pub const PROC: u32 = 0x801A_0E34;

/// No one pauses (`lbl_80479B10.slot`).
pub const NO_PAUSER: i32 = 99;

/// `JOBJ_HIDDEN`.
const HIDDEN: u32 = 1 << 4;

/// `HSD_GObj_JObjCallback`, the pause screen's render callback.
const JOBJ_RENDER: u32 = 0x8039_1070;

/// π/180 as the stick's rotation has it (`MTXDegToRad`'s float, widened).
const DEG_TO_RAD: f64 = 0.017_453_292_384_743_69;

/// `PauseData` (`lbl_80479B10`) and the screen's archive and GObj.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct PauseScreen {
    pub background: Option<JObjId>,
    pub analog_stick: Option<JObjId>,
    pub lras: Option<JObjId>,
    pub z: Option<JObjId>,
    pub analog_stick_outline: Option<JObjId>,
    /// The pauser's port, or [`NO_PAUSER`].
    pub slot: i32,
    /// `GmPause.dat` (`lbl_804D6700`).
    pub archive: Option<ArchiveId>,
    /// The screen's GObj (`lbl_804D6704`).
    pub gobj: Option<GObjId>,
}

/// The joint `index` steps into the tree from `root` in pre-order, for each index in turn,
/// starting again from the root when an index goes back, `lb_80011E24`.
#[doc(alias = "lb_80011E24")]
pub fn tree_nodes(jobjs: &JObjs, root: Option<JObjId>, indices: &[i32]) -> Vec<Option<JObjId>> {
    let mut out = Vec::new();
    let Some(root) = root else { return out };
    let (mut prev, mut j) = (-1, Some(root));
    for &target in indices {
        let mut cur = if prev > target || prev == -1 {
            j = Some(root);
            0
        } else {
            prev
        };
        while let Some(n) = j {
            if cur == target {
                break;
            }
            let o = &jobjs[n];
            j = if o.flags & jobj::INSTANCE == 0 && o.child.is_some() {
                o.child
            } else if o.next.is_some() {
                o.next
            } else {
                let mut saved = n;
                loop {
                    let Some(p) = jobjs[saved].parent else {
                        break None;
                    };
                    if let Some(next) = jobjs[p].next {
                        break Some(next);
                    }
                    saved = p;
                }
            };
            cur += 1;
        }
        out.push(j);
        prev = cur;
    }
    out
}

/// Turns the stick on the pause screen with the pauser's control stick, `fn_801A0E34`.
#[doc(alias = "fn_801A0E34")]
pub fn proc(r: &mut Running, _gobj: GObjId) {
    let p = &r.game.gm.pause;
    if p.slot == NO_PAUSER {
        return;
    }
    let pad = r.game.pads.master[usize::from(p.slot as u8)];
    let x = fp::fmuls(10.0, reg(pad.nml_stick_x));
    let y = fp::fmuls(10.0, reg(pad.nml_stick_y));
    let stick = p.analog_stick.expect("the pause screen's stick");
    let jobjs = &mut r.game.objs.jobjs;
    jobj::set_angle(jobjs, stick, |q| q.y = mem(fp::fmuls(DEG_TO_RAD, x)));
    jobj::set_angle(jobjs, stick, |q| {
        q.x = mem(fp::fneg(fp::fmuls(DEG_TO_RAD, y)))
    });
}

/// Shows the pause screen for the pauser at port `slot`, with the controls the rules list
/// (`flag`: 1 L+R+A+Start, 2 Z, 4 the stick), `gm_801A0FEC`.
#[doc(alias = "gm_801A0FEC")]
pub fn show(objs: &mut Objects, data: &Archives, p: &mut PauseScreen, slot: i32, flag: u8) {
    p.slot = slot;
    let bg = p.background;
    jobj::req_anim_all(objs, bg, mem(fp::frsp(f64::from(slot + 1))));
    let j = &mut objs.jobjs;
    if flag != 0 {
        jobj::clear_flags_all(j, bg, HIDDEN);
        for part in [p.lras, p.z, p.analog_stick, p.analog_stick_outline] {
            jobj::set_flags_all(j, part, HIDDEN);
        }
        if flag & 1 != 0 {
            jobj::clear_flags_all(j, p.lras, HIDDEN);
        }
        if flag & 2 != 0 {
            jobj::clear_flags_all(j, p.z, HIDDEN);
        }
        if flag & 4 != 0 {
            jobj::clear_flags_all(j, p.analog_stick, HIDDEN);
            jobj::clear_flags_all(j, p.analog_stick_outline, HIDDEN);
        }
    } else {
        jobj::set_flags_all(j, bg, HIDDEN);
    }
    jobj::anim_all(objs, data, &mut crate::game::code::ParticleHooks, bg);
}

impl Game {
    /// Hides the pause screen, `gm_801A10FC`.
    #[doc(alias = "gm_801A10FC")]
    pub fn hide_pause(&mut self, _slot: i32) {
        self.gm.pause.slot = NO_PAUSER;
        jobj::set_flags_all(&mut self.objs.jobjs, self.gm.pause.background, HIDDEN);
    }
}

/// Builds the hidden pause screen from `GmPause.dat`'s `ScGamPause_scene_data` (already
/// loaded as `archive`), `fn_801A1134`: its model on a GObj of its own, posed at frame 1, with
/// the stick's process.
#[doc(alias = "fn_801A1134")]
pub fn create(r: &mut Running, archive: ArchiveId) {
    let data = r.data;
    let a = data.get(archive);
    let scene = a
        .public("ScGamPause_scene_data")
        .expect("GmPause.dat's scene");
    let at = melee_data::archive::DataRef {
        archive,
        offset: scene,
    };
    // `SceneDesc::models[0]`, a `DynamicModelDesc`: joint, AnimJoints, MatAnimJoints,
    // ShapeAnimJoints.
    let models = data.ptr(at).expect("the scene's models");
    let model = data.ptr(models).expect("the scene's first model");
    let game = &mut *r.game;
    let gobj = game.world.create(0xE, 2, 0);
    game.gm.pause.archive = Some(archive);
    game.gm.pause.gobj = Some(gobj);
    let root = jobj::load_joint(&mut game.objs, data, data.ptr(model));
    let found = tree_nodes(&game.objs.jobjs, root, &[1]);
    let stick = found[0];
    let lras = tree_nodes(&game.objs.jobjs, root, &[9])[0];
    let z = tree_nodes(&game.objs.jobjs, root, &[10])[0];
    let outline = tree_nodes(&game.objs.jobjs, root, &[11])[0];
    let p = &mut game.gm.pause;
    p.background = root;
    p.lras = lras;
    p.z = z;
    p.analog_stick_outline = outline;
    p.analog_stick = stick.and_then(|s| game.objs.jobjs[s].child);
    game.world.set_obj(
        gobj,
        OBJ_JOBJ,
        Obj::JObj(root.expect("the pause screen's model")),
    );
    game.world
        .setup_gx_link(gobj, super::super::handles::CodeRef(JOBJ_RENDER), 0xB, 0);
    super::vs::add_model_anim(&mut game.objs, data, root, model, 0);
    jobj::req_anim_all(&mut game.objs, root, 1.0);
    jobj::anim_all(
        &mut game.objs,
        data,
        &mut crate::game::code::ParticleHooks,
        root,
    );
    jobj::set_flags_all(&mut game.objs.jobjs, root, HIDDEN);
    game.world
        .setup_proc(gobj, super::super::handles::CodeRef(PROC), 0);
    game.gm.pause.slot = NO_PAUSER;
}
