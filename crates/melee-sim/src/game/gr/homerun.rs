//! The Home-Run Contest's stage, `melee/gr/grhomerun.c`: the platform Sandbag stands on (map 0)
//! and the field beyond it (map 10). The field is made of stretches (maps 1, 2 and 4 to 9,
//! four kinds in turn, the last of each run of `ticks` labelled with its distance), made and
//! destroyed as the camera moves so only those within 2400 of its interest exist; behind them
//! the backdrop's 16 pieces (map 4) and four backgrounds (map 3) that follow the camera at
//! 0.85 of its pace. The field measures Sandbag's distance from the platform's edge in the
//! saved language's unit (`stage_info.x6E0`) and marks when it lands (`stage_info.flags`'
//! 0x80). The bat drops at point 0x7F.

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::handles::CodeRef;
use super::super::state::{Game, OBJ_CAMERA, Obj, Running};
use super::data::{HOMERUN_CALLBACKS, StageCallbacks};
use super::ground::{
    ground_of, jobj_of, nth_joint, p1_fighter, p1_fighter2, point_joint, scale, set_blast_right,
    set_cam_right, update_map_coll,
};
use super::{anime, setup, stage};
use crate::fpu::{copy, mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::sis::TextId;
use crate::lb::joint::world_position;
use crate::vec::Vec3;

/// `fn_8021EB10`, the labels' camera's render callback.
pub const TEXT_CAMERA_RENDER: u32 = 0x8021_EB10;
/// `fn_8021E994`, the field's collision joint's callback (`mpJointSetCb1`).
pub const FIELD_TOUCHED: u32 = 0x8021_E994;

/// `Gr_Homerun_Parts_Max`, `Gr_Homerun_Parts_Back`.
const PARTS: usize = 64;
const BACK: usize = 16;

/// A stretch's length in the stage's units, before its scale and the stage's.
const STRETCH: f32 = 160.0;
/// The backgrounds' width.
const BG_WIDTH: f32 = 2150.99;
/// How far from the camera's interest stretches exist.
const REACH: f32 = 2400.0;

/// The field's map id.
const MAP_FIELD: i32 = 10;

/// Where `Ground_GetStageGObj` keeps a new object's scale joint's descriptor (the stage's
/// create function `grHomeRun_8021C82C`, 0x20, under each caller): from the setup (0x18), the
/// field's setup inside the field's creation (0xC0, then for a stretch
/// `grHomeRun_8021E500`'s 0x50, then the create function again), and the field's process
/// (0xB0, then 0x50). ssbm-rs's run of `m6-homerun/hr-hit` inserts the joints at 0x804EE9F0
/// (the platform, the field), 0x804EE910 (the backdrop, the backgrounds), 0x804EE8C0 (the
/// first stretches) and 0x804EE930 (a stretch the process makes).
const INIT_FRAME: u32 = setup::init_frame(0x18);
const FIELD_INIT_FRAME: u32 = INIT_FRAME - 0xC0 - 0x20;
const FIELD_INIT_PART_FRAME: u32 = INIT_FRAME - 0xC0 - 0x50 - 0x20;
const PROC_PART_FRAME: u32 = super::corneria::PROC_LOOP - 0xB0 - 0x50 - 0x20;

/// The stage's statics (`grHr_804D6AD8` to `grHr_804D6AE4`, `yakumono_param`): the unit's
/// size in metres or feet (`scale`), a label's step (`max_dist`), stretches per label
/// (`ticks`), the field's scale (`ratio`) and the stage's parameters.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Globals {
    pub scale: f32,
    pub ticks: i32,
    pub max_dist: f32,
    pub ratio: f32,
    pub yakumono: Option<DataRef>,
}

/// A stage object's process or callback, by address: the stage's.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x8021_CB18 | 0x8021_DEE8 | 0x8021_E06C | 0x8021_E1F0 | 0x8021_E374 | 0x8021_E4F8 => {
            empty(r, gobj)
        }
        0x8021_CB1C | 0x8021_DEB0 | 0x8021_DEEC | 0x8021_E070 | 0x8021_E1F4 | 0x8021_E378
        | 0x8021_E4FC => destroyed(r, gobj),
        0x8021_C914 => init_platform(r, gobj),
        0x8021_CB20 => init_field(r, gobj),
        0x8021_D680 => proc_field(r, gobj),
        0x8021_DEB4 | 0x8021_E038 | 0x8021_E1BC | 0x8021_E340 | 0x8021_E4C4 => init_plain(r, gobj),
        0x8021_DEF0 | 0x8021_E074 | 0x8021_E1F8 | 0x8021_E37C => init_label(r, gobj),
        0x8021_DF50 | 0x8021_E0D4 | 0x8021_E258 | 0x8021_E3DC => proc_label(r, gobj),
        0x8021_E008 | 0x8021_E18C | 0x8021_E310 | 0x8021_E494 => destroy_label(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x8021_C754 => on_init(r),
        0x8021_C7FC => on_load(r),
        0x8021_C800 => on_start(r),
        _ => return false,
    }
    true
}

/// The processes that do nothing: the platform's, the plain stretches' and the backgrounds'.
#[doc(alias = "grHomeRun_8021CB18")]
#[doc(alias = "grHomeRun_8021DEE8")]
#[doc(alias = "grHomeRun_8021E06C")]
#[doc(alias = "grHomeRun_8021E1F0")]
#[doc(alias = "grHomeRun_8021E374")]
#[doc(alias = "grHomeRun_8021E4F8")]
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// The destroy callbacks that do nothing (the platform's, the field's, the plain stretches'
/// and the backgrounds'): only `Ground_801C4A08` runs them, for a stretch the field's process
/// leaves behind, past about 2,600 units of the camera's travel.
// Ports grHomeRun_8021CB1C; aliased once a check reaches it.
// Ports grHomeRun_8021DEB0; aliased once a check reaches it.
// Ports grHomeRun_8021DEEC; aliased once a check reaches it.
// Ports grHomeRun_8021E070; aliased once a check reaches it.
// Ports grHomeRun_8021E1F4; aliased once a check reaches it.
// Ports grHomeRun_8021E378; aliased once a check reaches it.
// Ports grHomeRun_8021E4FC; aliased once a check reaches it.
pub fn destroyed(_r: &mut Running, _gobj: GObjId) {}

/// The objects' predicates (`callback1`), which nothing calls and answer false.
// Ports grHomeRun_8021CB10; aliased once a check reaches it.
// Ports grHomeRun_8021D678; aliased once a check reaches it.
// Ports grHomeRun_8021DEE0; aliased once a check reaches it.
// Ports grHomeRun_8021DF48; aliased once a check reaches it.
// Ports grHomeRun_8021E064; aliased once a check reaches it.
// Ports grHomeRun_8021E0CC; aliased once a check reaches it.
// Ports grHomeRun_8021E1E8; aliased once a check reaches it.
// Ports grHomeRun_8021E250; aliased once a check reaches it.
// Ports grHomeRun_8021E36C; aliased once a check reaches it.
// Ports grHomeRun_8021E3D4; aliased once a check reaches it.
// Ports grHomeRun_8021E4F0; aliased once a check reaches it.
pub fn answers_false(_gobj: GObjId) -> bool {
    false
}

/// The stage's `callback4`, `grHomeRun_8021C824`: false.
// Ports grHomeRun_8021C824; aliased once a check reaches it.
pub fn callback4() -> bool {
    false
}

/// The stage's line-touch dynamics, `grHomeRun_8021EEB4`: none.
#[doc(alias = "grHomeRun_8021EEB4")]
pub fn on_touch_line(_line: i32) -> Option<DataRef> {
    None
}

/// Whether a joint takes a fighter's shadow, `grHomeRun_8021EEBC`: only below the fighter.
#[doc(alias = "grHomeRun_8021EEBC")]
pub fn check_shadow_render(
    game: &mut Game,
    data: &melee_data::archive::Archives,
    pos: Vec3,
    j: Option<JObjId>,
) -> bool {
    let v = world_position(&mut game.objs, data, j, None);
    pos.y > v.y
}

/// The eye's floor for the camera, `grHomeRun_8021EF10`: 5 in the stage's scale.
#[doc(alias = "grHomeRun_8021EF10")]
pub fn camera_floor(game: &Game) -> f64 {
    fp::fmuls(5.0, reg(scale(game)))
}

/// The stage's objects' setup functions and processes, by map id (`grHr_StageCallbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &HOMERUN_CALLBACKS[map_id as usize]
}

/// Creates one of the stage's objects and sets it up, `grHomeRun_8021C82C` (the stage's copy
/// of `Ground_SetupStageCallbacks`), its scale joint's descriptor at `frame` on the stack.
#[doc(alias = "grHomeRun_8021C82C")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

fn globals(game: &Game) -> &Globals {
    &game.gr.homerun
}

/// `x * ratio` into a joint's scale (the inline `HSD_JObjSetScaleX` of the product).
fn scale_by_ratio(game: &mut Game, j: JObjId) {
    let ratio = reg(game.gr.homerun.ratio);
    jobj::set_srt(&mut game.objs.jobjs, j, |o| {
        o.scale.x = mem(fp::fmuls(ratio, reg(o.scale.x)));
    });
    jobj::set_srt(&mut game.objs.jobjs, j, |o| {
        o.scale.y = mem(fp::fmuls(ratio, reg(o.scale.y)));
    });
    jobj::set_srt(&mut game.objs.jobjs, j, |o| {
        o.scale.z = mem(fp::fmuls(ratio, reg(o.scale.z)));
    });
}

fn set_translate_x(game: &mut Game, j: JObjId, x: f64) {
    jobj::set_srt(&mut game.objs.jobjs, j, |o| o.translate.x = mem(x));
}

/// The stage's setup, `grHomeRun_8021C754`: its parameters, the stage's `unk8C` flags (b4 off,
/// b5 on), the field's units, the platform and the field, the bat, and the camera's limits and
/// blast zones measured, with their right ends three times as far from the camera's centre.
#[doc(alias = "grHomeRun_8021C754")]
pub fn on_init(r: &mut Running) {
    r.game.gr.homerun.yakumono = super::ground::yakumono_param(r.game);
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    units(r.game);
    create(r, 0, INIT_FRAME);
    create(r, MAP_FIELD, INIT_FRAME);
    drop_bat(r);
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
    let c = stage::cam_center(r.game);
    let right = mem(fp::fmuls(
        3.0,
        fp::fsubs(reg(stage::cam_right(r.game)), reg(c.x)),
    ));
    set_cam_right(r.game, right);
    let blast = mem(fp::fmuls(
        3.0,
        fp::fsubs(reg(stage::blast_right(r.game)), reg(c.x)),
    ));
    set_blast_right(r.game, blast);
}

/// `grHomeRun_8021C750` does nothing (only a demo runs it).
// Ports grHomeRun_8021C750; aliased once a check reaches it.
pub fn on_demo_init(_r: &mut Running) {}

/// The stage's load, `grHomeRun_8021C7FC`: nothing.
#[doc(alias = "grHomeRun_8021C7FC")]
pub fn on_load(_r: &mut Running) {}

/// The match's start, `grHomeRun_8021C800`: the enemy spawner, with no table.
#[doc(alias = "grHomeRun_8021C800")]
pub fn on_start(r: &mut Running) {
    super::zako::start(r.game, None);
}

/// The platform's setup (map 0), `grHomeRun_8021C914`: its animation, at the field's scale.
#[doc(alias = "grHomeRun_8021C914")]
pub fn init_platform(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    let j = jobj_of(r.game, gobj).expect("the platform's joint");
    scale_by_ratio(r.game, j);
}

fn map_id(game: &Game, gobj: GObjId) -> i32 {
    game.grounds[ground_of(game, gobj).0].map_id
}

/// The field's setup (map 10), `grHomeRun_8021CB20`: its collision, the stretches' and the
/// backdrop's blocks, its scale, its flag `x10_flags.b5`, its animation, the collision's
/// callback; the labels' camera (a perspective at 30° and 1.4, drawn by
/// [`TEXT_CAMERA_RENDER`]) with font 1's canvas and the stage's strings; the stretches within
/// reach of the start, the backdrop's pieces placed leftward from four stretches back, the
/// four backgrounds side by side; and the start line's label's joint.
#[doc(alias = "grHomeRun_8021CB20")]
pub fn init_field(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    let parts_addr = crate::hsd::heap::mem_alloc(4 * PARTS as u32);
    let back_addr = crate::hsd::heap::mem_alloc(4 * BACK as u32);
    {
        let v = r.game.grounds[g.0].u.homerun();
        v.parts_addr = parts_addr;
        v.parts.clear();
        v.parts.resize(PARTS, None);
        v.back_addr = back_addr;
        v.back.clear();
        v.back.resize(BACK, None);
    }
    let j = j.expect("the field's joint");
    scale_by_ratio(r.game, j);
    r.game.grounds[g.0].x10_flags.b5 = true;
    anime::start_map(r.game, r.data, gobj, m, 0);
    let user = Some(super::CollUser::Ground(g));
    r.game
        .geometry_mut()
        .set_ledge_callback(0, user, Some(CodeRef(FIELD_TOUCHED)));
    create_text_camera(r, g);
    let s = reg(scale(r.game));
    let reach = fp::fadds(1.0, fp::fdivs(reg(REACH), fp::fmuls(reg(STRETCH), s)));
    assert!(
        fp::fmuls(2.0, reach) < 64.0,
        "the original's assertion failed: INIT_ADD_PARTS_RANGE*2<Gr_Homerun_Parts_Max"
    );
    let mut i = 0usize;
    while fp::frsp(i as f64) < reach {
        let p = place_part(r, i as i32, FIELD_INIT_PART_FRAME);
        r.game.grounds[g.0].u.homerun().parts[i] = Some(p);
        i += 1;
    }
    for i in 0..BACK {
        let piece = create(r, 4, FIELD_INIT_FRAME).expect("gp->u.map.back[i]");
        r.game.grounds[g.0].u.homerun().back[i] = Some(piece);
        let pj = jobj_of(r.game, piece).expect("jobj");
        scale_by_ratio(r.game, pj);
        let child = r.game.objs.jobjs[pj].child.expect("jobj->child");
        set_translate_x(r.game, child, 0.0);
        let ratio = reg(globals(r.game).ratio);
        let s = reg(scale(r.game));
        let x = fp::fmuls(
            ratio,
            fp::fmuls(
                fp::fadds(4.0, fp::frsp(i as f64)),
                fp::fneg(fp::fmuls(reg(STRETCH), s)),
            ),
        );
        set_translate_x(r.game, pj, x);
    }
    for (k, at) in [-1.5, -0.5, 0.5, 1.5].into_iter().enumerate() {
        let bg = create(r, 3, FIELD_INIT_FRAME).expect("gp->u.map.bg_gobj");
        r.game.grounds[g.0].u.homerun().bg[k] = Some(bg);
        let bj = jobj_of(r.game, bg).expect("jobj");
        let w = fp::fmuls(reg(BG_WIDTH), reg(scale(r.game)));
        let x = if at < 0.0 {
            fp::fmuls(fp::fneg(at), fp::fneg(w))
        } else {
            fp::fmuls(at, w)
        };
        set_translate_x(r.game, bj, x);
    }
    let label = nth_joint(r.game, gobj, 1);
    let v = r.game.grounds[g.0].u.homerun();
    v.text = None;
    v.jobj = label;
    v.landed = false;
}

/// `HSD_CameraDescPerspective` of the labels' camera.
const TEXT_CAMERA: crate::hsd::cobj::StaticDesc = crate::hsd::cobj::StaticDesc {
    flags: 0,
    projection_type: 1,
    viewport: [0, 640, 0, 640],
    scissor: [0, 480, 0, 480],
    eye: Vec3::new(0.0, 0.0, 1.0),
    interest: Vec3::new(0.0, 0.0, 0.0),
    roll: 0.0,
    near: 0.1,
    far: 32768.0,
    projection: [30.0, 1.3636, 0.0, 0.0],
};

/// The labels' camera GObj (class 17, link 19) with its camera, drawn by
/// [`TEXT_CAMERA_RENDER`] at priority 7 over GX link 1, and font 1's canvas under it with the
/// stage's strings (`SIS_GrHomerunData`).
fn create_text_camera(r: &mut Running, g: super::super::handles::GroundId) {
    let gobj = r.game.world.create(17, 19, 0);
    r.game.grounds[g.0].u.homerun().text_gobj = Some(gobj);
    let c = crate::lb::cobj::load_cobj(
        &mut r.game.objs,
        r.data,
        crate::lb::cobj::CObjDesc::Static(&TEXT_CAMERA),
    );
    crate::hsd::cobj::set_perspective(&mut r.game.objs.cobjs, Some(c), 30.0, 1.4);
    r.game.world.set_obj(gobj, OBJ_CAMERA, Obj::CObj(c));
    r.game
        .world
        .setup_gx_link_max(gobj, CodeRef(TEXT_CAMERA_RENDER), 7, false);
    r.game.world.gobj_mut(gobj).gxlink_prios = 2;
    r.add_canvas(1, Some(gobj), 9, 13, 0, 1, 0, 7);
    let archive = super::datfiles::main_archive(r.game).archive;
    r.set_font_table(1, archive, "SIS_GrHomerunData");
}

/// The field's process (map 10), `grHomeRun_8021D680`: with a first player (else nothing),
/// the camera following no fighter; Sandbag's distance measured, and its landing marked
/// below 4 when it touched the field since the last tick; the start line's label made once;
/// the stretches made and destroyed at each end to keep those within reach of the camera's
/// interest; the backgrounds kept around 0.85 of the interest; the wind and collision, and
/// the field's line 3 kept 20,000 long.
#[doc(alias = "grHomeRun_8021D680")]
pub fn proc_field(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let Some(fighter) = p1_fighter(r.game) else {
        return;
    };
    // `ftLib_GetCameraSubject`: the fighter's camera box, made inactive.
    let inactive = |game: &mut Game, f: GObjId| {
        if let Some(b) = game.fighter(f).x890_cameraBox {
            let s = &mut game.camera.boxes[b];
            s.state = super::super::camera::INACTIVE;
            s.force_inactive = true;
        }
    };
    inactive(r.game, fighter);
    if let Some(f2) = p1_fighter2(r.game) {
        inactive(r.game, f2);
    }
    if let Some(sandbag) = super::super::gm::home_run::sandbag(r.game) {
        let pos = r.game.ft_pos(sandbag);
        measure(r.game, pos.x);
        if pos.y < 4.0 && r.game.grounds[g.0].u.homerun().landed {
            mark_landed(r.game);
        }
    }
    r.game.grounds[g.0].u.homerun().landed = false;
    let v = r.game.grounds[g.0].u.homerun();
    if let (Some(label), None) = (v.jobj, v.text) {
        let t = label_text(r, 0);
        r.game.grounds[g.0].u.homerun().text = Some(t);
        place_text(r, t, label);
    }
    let interest = super::super::camera::interest(&r.game.camera.cam);
    let s = |r: &Running| fp::fmuls(reg(STRETCH), reg(scale(r.game)));
    let mut x0 = fp::fctiwz(fp::fsubs(
        fp::fdivs(fp::fsubs(reg(interest.x), reg(REACH)), s(r)),
        1.0,
    ));
    if x0 < 0 {
        x0 = 0;
    }
    let x1 = fp::fctiwz(fp::fadds(
        1.0,
        fp::fdivs(fp::fadds(reg(REACH), reg(interest.x)), s(r)),
    ));
    let first = r.game.grounds[g.0].u.homerun().parts[0].expect("the first stretch");
    let first_index = i32::from(part_index(r.game, first));
    if first_index < x0 {
        setup::destroy(r, Some(first));
        let parts = &mut r.game.grounds[g.0].u.homerun().parts;
        parts.copy_within(1..PARTS, 0);
        parts[PARTS - 1] = None;
    } else if first_index > x0 {
        let p = place_part(r, first_index - 1, PROC_PART_FRAME);
        let parts = &mut r.game.grounds[g.0].u.homerun().parts;
        parts.copy_within(0..PARTS - 1, 1);
        parts[0] = Some(p);
    }
    let parts = &r.game.grounds[g.0].u.homerun().parts;
    let i = (1..PARTS)
        .rev()
        .find(|&i| parts[i].is_some())
        .expect("the original's assertion failed: gobj2");
    let last = parts[i].expect("a stretch");
    let last_index = i32::from(part_index(r.game, last));
    if last_index > x1 {
        r.game.grounds[g.0].u.homerun().parts[i] = None;
        setup::destroy(r, Some(last));
    } else if last_index < x1 {
        let p = place_part(r, last_index + 1, PROC_PART_FRAME);
        r.game.grounds[g.0].u.homerun().parts[i + 1] = Some(p);
    }
    let w = |r: &Running| fp::fmuls(reg(BG_WIDTH), reg(scale(r.game)));
    let mut x = fp::fmuls(0.850_000_023_841_857_9, reg(interest.x));
    loop {
        let diff = fp::fsubs(x, reg(interest.x));
        if diff < fp::fmuls(0.5, fp::fneg(w(r))) {
            x = fp::fmadds(reg(BG_WIDTH), reg(scale(r.game)), x);
        } else if diff > fp::fmuls(0.5, w(r)) {
            x = fp::fnmsubs(reg(BG_WIDTH), reg(scale(r.game)), x);
        } else {
            break;
        }
    }
    let bg = r.game.grounds[g.0].u.homerun().bg;
    for (k, b) in bg.into_iter().enumerate() {
        let bj = jobj_of(r.game, b.expect("a background")).expect("jobj");
        let at = [1.5, 0.5, 0.5, 1.5][k];
        let tx = if k < 2 {
            fp::fnmsubs(at, w(r), x)
        } else {
            fp::fmadds(at, w(r), x)
        };
        set_translate_x(r.game, bj, tx);
    }
    super::ground::quake_list_tick(r.game);
    update_map_coll(r.game, r.data, gobj);
    let geo = r.game.geometry_mut();
    geo.offset_line(3, 0.0, 0.0, 20000.0, 0.0);
    geo.update_bounding(0);
}

/// A stretch's index along the field (`homerun2.xC4`).
fn part_index(game: &mut Game, gobj: GObjId) -> u16 {
    let g = ground_of(game, gobj);
    match &mut game.grounds[g.0].u {
        super::vars::GroundVars::HomeRunLabel(v) => v.index,
        u => u.homerun_part().index,
    }
}

/// A label's text at its joint's place, a stretch to its left (the original's sums of zero
/// times the scale for the other two axes kept).
fn place_text(r: &mut Running, t: TextId, label: JObjId) {
    let pos = world_position(&mut r.game.objs, r.data, Some(label), None);
    let ratio = reg(globals(r.game).ratio);
    let s = reg(scale(r.game));
    let z = fp::fadds(reg(pos.z), 0.0);
    let y = fp::fadds(fp::fneg(reg(pos.y)), 0.0);
    let x = fp::fmadds(fp::fneg(1.0), fp::fmuls(ratio, s), reg(pos.x));
    let text = r.game.sis.text_mut(t);
    text.pos.x = mem(x);
    text.pos.y = mem(y);
    text.pos.z = mem(z);
}

/// The plain objects' setup (maps 1, 3, 4, 6 and 8), `grHomeRun_8021DEB4` and its copies:
/// their animation.
#[doc(alias = "grHomeRun_8021DEB4")]
#[doc(alias = "grHomeRun_8021E038")]
#[doc(alias = "grHomeRun_8021E1BC")]
#[doc(alias = "grHomeRun_8021E340")]
#[doc(alias = "grHomeRun_8021E4C4")]
pub fn init_plain(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// A labelled stretch's setup (maps 2, 5, 7 and 9), `grHomeRun_8021DEF0` and its copies: its
/// animation, no text yet, and its label's joint.
#[doc(alias = "grHomeRun_8021DEF0")]
#[doc(alias = "grHomeRun_8021E074")]
#[doc(alias = "grHomeRun_8021E1F8")]
#[doc(alias = "grHomeRun_8021E37C")]
pub fn init_label(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    let label = nth_joint(r.game, gobj, 1);
    let v = r.game.grounds[g.0].u.homerun_label();
    v.text = None;
    v.jobj = label;
}

/// A labelled stretch's process, `grHomeRun_8021DF50` and its copies: its label's text made
/// once, at its joint.
#[doc(alias = "grHomeRun_8021DF50")]
#[doc(alias = "grHomeRun_8021E0D4")]
#[doc(alias = "grHomeRun_8021E258")]
#[doc(alias = "grHomeRun_8021E3DC")]
pub fn proc_label(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.homerun_label();
    let (Some(label), None) = (v.jobj, v.text) else {
        return;
    };
    let n = v.label;
    let t = label_text(r, i32::from(n));
    r.game.grounds[g.0].u.homerun_label().text = Some(t);
    place_text(r, t, label);
}

/// A labelled stretch's destruction, `grHomeRun_8021E008` and its copies: its text with it.
// Ports grHomeRun_8021E008; aliased once a check reaches it.
// Ports grHomeRun_8021E18C; aliased once a check reaches it.
// Ports grHomeRun_8021E310; aliased once a check reaches it.
// Ports grHomeRun_8021E494; aliased once a check reaches it.
pub fn destroy_label(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    if let Some(t) = r.game.grounds[g.0].u.homerun_label().text {
        r.destroy_text(t);
    }
}

/// Makes the stretch at `index`, `grHomeRun_8021E500`: of the four kinds in turn by runs of
/// `ticks`, the last of each run labelled; at the field's scale, its model at its joint, placed
/// a stretch on from `index` stretches, with the distance its label shows.
#[doc(alias = "grHomeRun_8021E500")]
pub fn place_part(r: &mut Running, index: i32, frame: u32) -> GObjId {
    let ticks = globals(r.game).ticks;
    let last = ticks - 1 == index % ticks;
    let map = match (index / ticks) % 4 {
        0 => 4,
        1 => 1,
        2 => 6,
        3 => 8,
        _ => unreachable!("the original's assertion failed: 0"),
    } + i32::from(last);
    let gobj = create(r, map, frame).expect("the original's assertion failed: gobj");
    let j = jobj_of(r.game, gobj).expect("the original's assertion failed: jobj");
    scale_by_ratio(r.game, j);
    let child = r.game.objs.jobjs[j].child.expect("jobj->child");
    set_translate_x(r.game, child, 0.0);
    let ratio = reg(globals(r.game).ratio);
    let s = reg(scale(r.game));
    let offset = fp::fmuls(
        ratio,
        fp::fmuls(fp::frsp(index as f64), fp::fmuls(reg(STRETCH), s)),
    );
    let x = mem(fp::fmadds(reg(STRETCH), fp::fmuls(ratio, s), offset));
    set_translate_x(r.game, j, reg(x));
    let max = fp::fctiwz(reg(globals(r.game).max_dist));
    let label = max.wrapping_mul((index + 1) / ticks) as i16 as u16;
    let g = ground_of(r.game, gobj);
    match &mut r.game.grounds[g.0].u {
        super::vars::GroundVars::HomeRunLabel(v) => {
            v.x = x;
            v.label = label;
            v.index = index as u16;
        }
        u => {
            let v = u.homerun_part();
            v.x = x;
            v.label = label;
            v.index = index as u16;
        }
    }
    gobj
}

/// The field's collision joint's callback, `fn_8021E994`: Sandbag (player 2's fighter)
/// touching it as a floor marks the field (`xE8_flags.b0`).
#[doc(alias = "fn_8021E994")]
pub fn field_touched(
    grounds: &mut crate::hsd::pool::Pool<super::super::structs::Ground>,
    field: Option<super::super::handles::GroundId>,
    coll: &crate::game::structs::CollData,
    kind: i32,
    sandbag: Option<GObjId>,
) {
    let Some(g) = field else {
        return;
    };
    if coll.x34_flags.b1234 == 1 && coll.x0_gobj == sandbag && kind == 1 {
        grounds[g.0].u.homerun().landed = true;
    }
}

/// Sandbag's distance from its x, `grHomeRun_8021EA30`: past 70 in the field's scale, in
/// labels' steps, never below 0, in feet with English saved (the step's 0.304788), in
/// hundredths (`stage_info.x6E0`).
#[doc(alias = "grHomeRun_8021EA30")]
pub fn measure(game: &mut Game, x: f32) {
    let gl = globals(game);
    let (ratio, ticks, max) = (reg(gl.ratio), gl.ticks, reg(gl.max_dist));
    let s = reg(scale(game));
    let mut result = fp::fnmsubs(70.0, fp::fmuls(ratio, s), reg(x));
    let n = fp::fmuls(fp::frsp(f64::from(ticks)), fp::fmuls(reg(STRETCH), s));
    result = fp::fdivs(result, fp::fmuls(ratio, n));
    result = fp::fmuls(result, max);
    if result < 0.0 {
        result = 0.0;
    }
    if game.gm.unlocks.is_us() {
        result = fp::frsp(fp::fmul(result, 0.304788));
    }
    game.stage_info.x6E0 = mem(fp::fmuls(100.0, result));
}

/// Sandbag landed, `grHomeRun_8021EAF8` (`stage_info.flags`' 0x80).
#[doc(alias = "grHomeRun_8021EAF8")]
pub fn mark_landed(game: &mut Game) {
    game.stage_info.flags |= 0x80;
}

/// A label's text, `grHomeRun_8021EC58`: font 1's buffer, white, centred and fitted, its box
/// 13 by 8 and its scale 0.2 in the field's and the stage's scales, printing `n`.
#[doc(alias = "grHomeRun_8021EC58")]
pub fn label_text(r: &mut Running, n: i32) -> TextId {
    let t = r.create_text_buffer(1, 0);
    let ratio = reg(globals(r.game).ratio);
    let k = |r: &Running, v: f64| mem(fp::fmuls(v, fp::fmuls(ratio, reg(scale(r.game)))));
    let (h, w) = (k(r, 8.0), k(r, 13.0));
    let (sy, sx) = (
        k(r, 0.200_000_002_980_232_24),
        k(r, 0.200_000_002_980_232_24),
    );
    let text = r.game.sis.text_mut(t);
    text.pos.z = 0.0;
    text.box_size = [w, h];
    text.default_kerning = 0;
    text.x34 = [sx, sy];
    text.text_color = [0xFF; 4];
    text.default_alignment = 1;
    text.default_fitting = 1;
    text.x4c = 1;
    let mut s = [0u8; 0x200];
    let v = fp::fctiwz(fp::frsp(f64::from(n)));
    let len = crate::hsd::sis::printf(b"%d\0", &[crate::hsd::sis::FmtArg::Int(v)], &mut s);
    r.game
        .sis
        .add_string(t, 0.0, fp::fneg(29.0), Some(&s[..=len]));
    t
}

/// Drops the bat at point 0x7F, `grHomeRun_8021ED74` (`it_8026BE84` of kind 11, held).
#[doc(alias = "grHomeRun_8021ED74")]
pub fn drop_bat(r: &mut Running) {
    let j = point_joint(r.game, 0x7F);
    let pos = world_position(&mut r.game.objs, r.data, j, None);
    super::super::it::lib26b1::drop_by_kind(r, 11, pos, true);
}

/// The field's units, `grHomeRun_8021EDD4`: metres with Japanese saved (a label every 50),
/// else feet (a label every 100 feet, 0.3280969 the metre's), and the field's scale so that a
/// label's stretches span the distance between labels.
#[doc(alias = "grHomeRun_8021EDD4")]
pub fn units(game: &mut Game) {
    let (unit, max): (f64, f64) = if game.gm.unlocks.is_jp() {
        (0.100_000_001_490_116_12, 50.0)
    } else {
        (0.328_096_896_409_988_4, 100.0)
    };
    let ratio = fp::fdivs(max, unit);
    let s = reg(scale(game));
    let mut ticks = fp::frsp(f64::from(fp::fctiwz(fp::fadds(
        0.5,
        fp::fdivs(ratio, fp::fmuls(reg(STRETCH), s)),
    ))));
    if ticks < 1.0 {
        ticks = 1.0;
    }
    let dist = fp::fmuls(reg(STRETCH), s);
    let gl = &mut game.gr.homerun;
    gl.scale = mem(unit);
    gl.max_dist = mem(max);
    gl.ticks = fp::fctiwz(ticks);
    gl.ratio = mem(fp::fdivs(ratio, fp::fmuls(ticks, dist)));
}

impl Running<'_> {
    /// The labels' camera's render callback, `fn_8021EB10`: the game camera's flags,
    /// viewport, scissor, roll, near and far planes, perspective, eye and interest copied in,
    /// then the labels drawn through it without fog.
    #[doc(alias = "fn_8021EB10")]
    pub fn homerun_text_camera_render(&mut self, gobj: GObjId) {
        use crate::hsd::cobj;
        let Some(Obj::CObj(target)) = self.game.world.gobj(gobj).obj else {
            panic!("the labels' camera GObj holds no CObj");
        };
        if let Some(main) = super::super::camera::gobj(&self.game.camera.cam)
            && let Some(Obj::CObj(m)) = self.game.world.gobj(main).obj
        {
            let (data, objs) = (self.data, &mut self.game.objs);
            let c = &mut objs.cobjs;
            let flags = cobj::flags(c, m);
            cobj::set_flags(c, Some(target), flags);
            let vp = cobj::viewport(c, Some(m)).expect("a viewport");
            cobj::set_viewportf(c, Some(target), vp);
            let sc = cobj::scissor(c, Some(m)).expect("a scissor");
            cobj::set_scissor(c, Some(target), sc);
            c[target].up = c[m].up;
            let near = cobj::near(c, Some(m));
            cobj::set_near(c, Some(target), near);
            let far = cobj::far(c, Some(m));
            cobj::set_far(c, Some(target), far);
            let (fov, aspect) = cobj::get_perspective(c, Some(m)).expect("a perspective");
            cobj::set_perspective(c, Some(target), copy(fov), copy(aspect));
            let eye = cobj::eye_position(objs, data, m).expect("an eye");
            cobj::set_eye_position(objs, target, eye);
            let interest = cobj::interest(objs, data, m).expect("an interest");
            cobj::set_interest(objs, target, interest);
        }
        if crate::hsd::display::camera_begin(&mut self.game.objs, self.data, Some(target)) {
            crate::hsd::display::fog_set(&mut self.game.objs, None);
            self.render_links(gobj, 7);
            crate::hsd::display::camera_end(&mut self.game.objs);
        }
    }
}
