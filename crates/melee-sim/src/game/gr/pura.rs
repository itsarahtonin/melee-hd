//! Poké Floats, `melee/gr/grpura.c`: the stage (map 4), whose one animation carries every float
//! past; the floats themselves (maps 2, 3 and 5 to 26), each a model that copies a joint of the
//! stage's and draws with a toon texture; the sky (map 1), fading between four colors; the
//! background (map 0) and the lights (map 27).
//!
//! The stage frames each float with a camera subject while it shows, and moves the collision
//! vertices that ride on the floats' joints every tick.

use gekko_fp as fp;

use super::super::camera;
use super::super::handles::CodeRef;
use super::super::state::{Game, Running};
use super::super::structs::_GXColor;
use super::data::{PURA_CALLBACKS, StageCallbacks};
use super::ground::{ground_of, jobj_of, map_gobj, nth_joint, update_map_coll};
use super::{anime, setup, stage, zako};
use crate::fpu::{mem, reg};
use crate::hsd::dobj::DObjId;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::tobj::ImageRef;
use crate::lb::joint::world_position;

/// `fn_802130D0`, the floats' render callback.
pub const RENDER_FLOAT: u32 = 0x8021_30D0;

/// `grPu_803E7620`, the floats' toon texture's image descriptor in the executable.
const TOON_IMAGE: u32 = 0x803E_7620;

/// Where the setup's objects' scale joints' descriptors sit on the stack
/// ([`setup::SETUP_FRAME`]): the setup's own, and the floats', which the stage's setup makes,
/// below it by the create function's frame (0x20), the stage's setup's (0x20) and
/// `grPura_802125F0`'s (0x68).
const INIT_FRAME: u32 = setup::SETUP_FRAME;
const FLOAT_FRAME: u32 = setup::SETUP_FRAME - 0xA8;

/// The sky's four colors (`grPu_803E6AA0`), red, green, blue and alpha.
const SKY: [[u8; 4]; 4] = [
    [0x00, 0x00, 0x00, 0xFF],
    [0x00, 0x00, 0x50, 0xFF],
    [0x1E, 0x46, 0x00, 0xFF],
    [0x87, 0x87, 0x00, 0xFF],
];

/// How long the sky takes to fade from one color to the next, in ticks.
const SKY_FADE: i16 = 0xE10;

/// The floats (`grPu_803E6AB0`): the stage's part each copies, its scale's factor, and its
/// map id; the last entry ends the table.
const FLOATS: [(i32, f32, i32); 27] = {
    let mut t = [(0, 1.0, 0); 27];
    let maps = [
        2, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 13, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
        25, 26,
    ];
    let mut i = 0;
    while i < 26 {
        t[i] = (i as i32, 1.0, maps[i]);
        i += 1;
    }
    t[26] = (-1, 1.0, -1);
    t
};

/// The collision vertices that ride on the floats (`grPu_803E6C0C`): each vertex (`x`), and the
/// map (`y`) and part (`z`) of the joint it follows; a vertex of -1 ends the table.
const VTX: [(i16, i16, i16); VTX_COUNT] = [
    (20, 2, 21),
    (21, 2, 22),
    (22, 2, 23),
    (23, 2, 24),
    (24, 2, 25),
    (25, 2, 26),
    (26, 2, 27),
    (39, 2, 28),
    (284, 12, 21),
    (285, 12, 23),
    (267, 12, 24),
    (261, 12, 25),
    (262, 12, 26),
    (275, 12, 27),
    (276, 12, 28),
    (277, 12, 29),
    (269, 12, 30),
    (270, 12, 31),
    (272, 12, 32),
    (286, 12, 33),
    (180, 8, 23),
    (181, 8, 51),
    (182, 8, 27),
    (183, 8, 30),
    (184, 8, 31),
    (513, 25, 10),
    (514, 25, 13),
    (520, 25, 16),
    (521, 25, 19),
    (522, 25, 22),
    (496, 25, 25),
    (497, 25, 28),
    (500, 25, 31),
    (523, 25, 32),
    (524, 25, 29),
    (527, 25, 26),
    (528, 25, 23),
    (525, 25, 20),
    (526, 25, 17),
    (494, 25, 14),
    (495, 25, 11),
    (-1, 0, 0),
];

/// The vertex table's entries the stage's code walks.
pub const VTX_COUNT: usize = 0x2A;

/// The camera subjects the stage keeps, one per float joint.
const SUBJECTS: usize = 25;

/// A stage object's process or callback, by address: Poké Floats'.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x8021_1F24 | 0x8021_1F28 | 0x8021_1F60 | 0x8021_1F64 | 0x8021_2020 | 0x8021_228C
        | 0x8021_25EC => empty(r, gobj),
        0x8021_1EF0 => init_main(r, gobj),
        0x8021_1F2C => init_lights(r, gobj),
        0x8021_1F68 => init_stage(r, gobj),
        0x8021_1FD8 => proc_stage(r, gobj),
        0x8021_2024 => init_sky(r, gobj),
        0x8021_20E0 => proc_sky(r, gobj),
        0x8021_2290 => init_float(r, gobj),
        0x8021_231C => proc_float(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x8021_1D00 => on_init(r),
        0x8021_1DD8 => on_load(r),
        0x8021_1DDC => on_start(r),
        _ => return false,
    }
    true
}

/// The stage's render callbacks, by address.
pub fn run_render(r: &mut Running, gobj: GObjId, cb: u32, pass: i32) -> bool {
    match cb {
        RENDER_FLOAT => render_float(r, gobj, pass),
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing.
#[doc(alias = "stageGObj0_GObjProc@80211F24")]
#[doc(alias = "stageGObj0_Callback3@80211F28")]
#[doc(alias = "stageGObj27_GObjProc@80211F60")]
#[doc(alias = "stageGObj27_Callback3@80211F64")]
#[doc(alias = "stageGObj4_Callback3@80212020")]
#[doc(alias = "stageGObj1_Callback3@8021228C")]
#[doc(alias = "stageGObj2_Callback3@802125EC")]
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// Creates one of the stage's objects and sets it up, `setupStageCallbacks` (grpura.c's).
#[doc(alias = "setupStageCallbacks@80211E08")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

/// The stage's objects' setup functions and processes, by map id (`stage_callbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &PURA_CALLBACKS[map_id as usize]
}

/// Poké Floats' setup, `grPura_80211D00`: its parameters, maps 0, 1 and 4 (which makes the
/// floats), camera limits and blast zones, then the camera limits pulled in to 0.8 of their
/// distance from their center.
#[doc(alias = "grPura_80211D00")]
pub fn on_init(r: &mut Running) {
    r.game.gr.pura_yakumono = super::ground::yakumono_param(r.game);
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    for map_id in [0, 1, 4] {
        create(r, map_id, INIT_FRAME);
    }
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
    let c = stage::cam_center(r.game);
    let scaled =
        |edge: f32, center: f32| mem(fp::fmuls(0.8f32 as f64, fp::fsubs(reg(edge), reg(center))));
    let top = scaled(stage::cam_top(r.game), c.y);
    super::ground::set_cam_top(r.game, top);
    let bottom = scaled(stage::cam_bottom(r.game), c.y);
    super::ground::set_cam_bottom(r.game, bottom);
    let left = scaled(stage::cam_left(r.game), c.x);
    super::ground::set_cam_left(r.game, left);
    let right = scaled(stage::cam_right(r.game), c.x);
    super::ground::set_cam_right(r.game, right);
}

/// `grPura_80211CFC` does nothing (only a demo runs it).
#[doc(alias = "grPura_80211CFC")]
pub fn on_demo_init(_r: &mut Running) {}

/// `grPura_80211DD8` does nothing.
#[doc(alias = "grPura_80211DD8")]
pub fn on_load(_r: &mut Running) {}

/// Poké Floats' start, `grPura_80211DDC`: the enemy spawner, without enemies.
#[doc(alias = "grPura_80211DDC")]
pub fn on_start(r: &mut Running) {
    zako::start(r.game, None);
}

fn map_id(game: &Game, gobj: GObjId) -> i32 {
    game.grounds[ground_of(game, gobj).0].map_id
}

/// The background's setup (map 0), `stageGObj0_OnInit`: its animation.
#[doc(alias = "stageGObj0_OnInit@80211EF0")]
pub fn init_main(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// The lights' setup (map 27), `stageGObj27_OnInit`: their animation.
#[doc(alias = "stageGObj27_OnInit@80211F2C")]
pub fn init_lights(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// The stage's setup (map 4), `stageGObj4_OnInit`: its collision and animation, the floats'
/// camera subjects, the floats, and the joints the floats' collision vertices follow.
#[doc(alias = "stageGObj4_OnInit@80211F68")]
pub fn init_stage(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    anime::start_map(r.game, r.data, gobj, m, 0);
    init_subjects(r, gobj);
    make_floats(r, gobj);
    find_vtx_joints(r);
}

/// The stage's process, `stageGObj4_GObjProc`: the subjects follow the floats, the collision
/// follows the model and the floats' vertices their joints, joint 24's lines are reclassified,
/// and the wind sources age.
#[doc(alias = "stageGObj4_GObjProc@80211FD8")]
pub fn proc_stage(r: &mut Running, gobj: GObjId) {
    update_subjects(r, gobj);
    update_map_coll(r.game, r.data, gobj);
    move_vertices(r);
    r.game.geometry_mut().update_dynamics_and_islands(0x18);
    let level = r.game.gr.wind_level;
    r.game.gr.wind_level = crate::lb::wind::tick(&mut r.game.gr.winds, level);
}

/// Sets the stage's fog and the camera's background to a color.
fn set_sky(game: &mut Game, c: [u8; 4]) {
    let [r, g, b, a] = c;
    super::ground::set_fog_color(game, _GXColor { r, g, b, a });
    camera::set_background_color(&mut game.camera.cam, r, g, b);
}

/// A color other than `from` (`xC6`), drawn until it differs.
fn next_color(r: &mut Running, g: super::super::handles::GroundId) {
    loop {
        let c = r.game.objs.rng.below(4);
        let v = r.game.grounds[g.0].u.pura();
        v.xC6 = c as i16;
        if v.xC4 != v.xC6 {
            break;
        }
    }
}

/// The sky's setup (map 1), `stageGObj1_OnInit`: its animation, drawn behind, a random color
/// and another to fade to, the fog and background in the first.
#[doc(alias = "stageGObj1_OnInit@80212024")]
pub fn init_sky(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    r.game.grounds[g.0].x11_flags.b012 = 2;
    let c = r.game.objs.rng.below(4);
    r.game.grounds[g.0].u.pura().xC4 = c as i16;
    next_color(r, g);
    let from = r.game.grounds[g.0].u.pura().xC4;
    set_sky(r.game, SKY[from as usize]);
    r.game.grounds[g.0].u.pura().xC8 = 0;
}

/// The sky's process, `stageGObj1_GObjProc`: over 3,600 ticks the fog and background fade
/// from one color to the next; then the next becomes the first and another is drawn.
#[doc(alias = "stageGObj1_GObjProc@802120E0")]
pub fn proc_sky(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.pura();
    if v.xC8 < SKY_FADE {
        let (to, mut c) = (SKY[v.xC6 as usize], SKY[v.xC4 as usize]);
        let cur = v.xC8;
        let t = fp::fdivs(fp::frsp(f64::from(cur)), 3600.0f32 as f64);
        v.xC8 = cur.wrapping_add(1);
        let lerp = |to: u8, from: u8| {
            let d = fp::frsp(f64::from(i32::from(to) - i32::from(from)));
            fp::fctiwz(fp::fmadds(t, d, fp::frsp(f64::from(from)))) as u8
        };
        for k in 0..3 {
            c[k] = lerp(to[k], c[k]);
        }
        set_sky(r.game, c);
        return;
    }
    v.xC4 = v.xC6;
    next_color(r, g);
    r.game.grounds[g.0].u.pura().xC8 = 0;
}

/// A float's setup (maps 2, 3 and 5 to 26), `stageGObj2_OnInit`: drawn by
/// [`render_float`], its materials toon-shaded (render mode bit 12) and their TEV compiled
/// with the float's toon texture, and its animation.
#[doc(alias = "stageGObj2_OnInit@80212290")]
pub fn init_float(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj).expect("a float's joint");
    r.game.world.gobj_mut(gobj).render = Some(CodeRef(RENDER_FLOAT));
    crate::hsd::mobj::set_toon_texture_image(&mut r.game.objs, Some(ImageRef::Static(TOON_IMAGE)));
    crate::lb::joint::set_mobj_flags(&mut r.game.objs, j, 0x1000);
    compile_tree(r, j);
    crate::hsd::mobj::set_toon_texture_image(&mut r.game.objs, None);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// A float's process, `stageGObj2_GObjProc`: its model takes the stage joint's translation,
/// rotation and scale, and shows or hides as the joint does.
#[doc(alias = "stageGObj2_GObjProc@8021231C")]
pub fn proc_float(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let src = r.game.grounds[g.0]
        .u
        .pura_float()
        .xC8
        .expect("a float's stage joint");
    let j = jobj_of(r.game, gobj).expect("a float's joint");
    let jobjs = &mut r.game.objs.jobjs;
    let t = jobjs[src].translate;
    jobj::set_translate(jobjs, j, t);
    let (q, stale) = (jobjs[src].rotate, jobjs[src].stale_w);
    jobj::set_srt(jobjs, j, |o| {
        o.rotate = q;
        o.stale_w = stale;
    });
    let s = jobjs[src].scale;
    jobj::set_scale(jobjs, j, s);
    let src_hidden = jobj::get_flags(jobjs, Some(src)) & jobj::HIDDEN != 0;
    let hidden = jobj::get_flags(jobjs, Some(j)) & jobj::HIDDEN != 0;
    if src_hidden && !hidden {
        jobj::set_flags_all(jobjs, Some(j), jobj::HIDDEN);
    } else if !src_hidden && hidden {
        jobj::clear_flags_all(jobjs, Some(j), jobj::HIDDEN);
    }
}

/// Makes the floats, `grPura_802125F0`: each copies its part of the stage (`stage`), starts
/// where that part is, hidden if it is, its model's first child at the origin, and takes the
/// part's scale (times its factor below 2) on both.
#[doc(alias = "grPura_802125F0")]
pub fn make_floats(r: &mut Running, stage: GObjId) {
    for &(part, factor, map) in FLOATS.iter() {
        if map == -1 {
            continue;
        }
        let gobj = create(r, map, FLOAT_FRAME).expect("grpura.c: gobj");
        let g = ground_of(r.game, gobj);
        r.game.grounds[g.0].u.pura_float().xC4 = part as i16;
        let joint = setup::coll_joint_part(r.game, r.data, 4, part);
        let src = nth_joint(r.game, stage, joint);
        r.game.grounds[g.0].u.pura_float().xC8 = src;
        let src = src.expect("a float's stage joint");
        let j = jobj_of(r.game, gobj).expect("a float's joint");
        let jobjs = &mut r.game.objs.jobjs;
        let t = jobjs[src].translate;
        jobj::set_srt(jobjs, j, |o| o.translate.x = t.x);
        jobj::set_srt(jobjs, j, |o| o.translate.y = t.y);
        jobj::set_srt(jobjs, j, |o| o.translate.z = t.z);
        if jobj::get_flags(jobjs, Some(src)) & jobj::HIDDEN != 0 {
            jobj::set_flags_all(jobjs, Some(j), jobj::HIDDEN);
        }
        let child = jobjs[j].child.expect("a float's model");
        jobj::set_srt(jobjs, child, |o| o.translate.x = 0.0);
        jobj::set_srt(jobjs, child, |o| o.translate.y = 0.0);
        jobj::set_srt(jobjs, child, |o| o.translate.z = 0.0);
        let mut scale = jobjs[src].scale.x;
        if reg(scale) < 2.0 {
            scale = mem(fp::fmuls(reg(scale), reg(factor)));
        }
        for o in [src, j] {
            jobj::set_srt(jobjs, o, |o| o.scale.x = scale);
            jobj::set_srt(jobjs, o, |o| o.scale.y = scale);
            jobj::set_srt(jobjs, o, |o| o.scale.z = scale);
        }
    }
}

/// The floats' camera subjects, `grPura_80212CD4`: one for each of the first 25 joints under
/// the stage's first joint, at the joint, framed while it shows, 30 units each way.
#[doc(alias = "grPura_80212CD4")]
pub fn init_subjects(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let root = jobj_of(r.game, gobj);
    *r.game.grounds[g.0].u.pura_stage() = Default::default();
    let Some(first) = root.and_then(|j| r.game.objs.jobjs[j].child) else {
        return;
    };
    let mut node = r.game.objs.jobjs[first].child;
    let mut i = 0;
    while i < SUBJECTS
        && let Some(n) = node
    {
        let b = camera::alloc_box(&mut r.game.camera.boxes, camera::ACTIVE);
        let v = r.game.grounds[g.0].u.pura_stage();
        v.x128[i] = Some(b);
        v.xC4[i] = Some(n);
        let pos = world_position(&mut r.game.objs, r.data, Some(n), None);
        let hidden = jobj::get_flags(&r.game.objs.jobjs, Some(n)) & jobj::HIDDEN != 0;
        let s = &mut r.game.camera.boxes[b];
        s.pos = pos;
        s.state = if hidden {
            camera::INACTIVE
        } else {
            camera::ACTIVE
        };
        s.target_ext.v.x = 30.0;
        s.target_ext.v.y = -30.0;
        s.target_ext.h.x = -30.0;
        s.target_ext.h.y = 30.0;
        s.ext.h = s.target_ext.h.clone();
        s.ext.v = s.target_ext.v;
        i += 1;
        node = r.game.objs.jobjs[n].next;
    }
}

/// The subjects follow their joints, `grPura_80212EF4`: each at its joint, framed while the
/// joint shows.
#[doc(alias = "grPura_80212EF4")]
pub fn update_subjects(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    for i in 0..SUBJECTS {
        let v = r.game.grounds[g.0].u.pura_stage();
        let (Some(j), Some(b)) = (v.xC4[i], v.x128[i]) else {
            continue;
        };
        let pos = world_position(&mut r.game.objs, r.data, Some(j), None);
        let hidden = jobj::get_flags(&r.game.objs.jobjs, Some(j)) & jobj::HIDDEN != 0;
        let s = &mut r.game.camera.boxes[b];
        s.pos = pos;
        s.state = if hidden {
            camera::INACTIVE
        } else {
            camera::ACTIVE
        };
    }
}

/// The joints the floats' collision vertices follow, `grPura_80212FC0`: for each vertex, its
/// map's part, if the map has an object.
#[doc(alias = "grPura_80212FC0")]
pub fn find_vtx_joints(r: &mut Running) {
    if r.game.gr.pura_vtx_jobjs.len() < VTX_COUNT {
        r.game.gr.pura_vtx_jobjs.resize(VTX_COUNT, None);
    }
    for (i, &(vtx, map, part)) in VTX.iter().enumerate() {
        if vtx == -1 {
            continue;
        }
        if let Some(m) = map_gobj(r.game, i32::from(map)) {
            r.game.gr.pura_vtx_jobjs[i] = nth_joint(r.game, m, i32::from(part));
        }
    }
}

/// The floats' collision vertices move to their joints, `grPura_80213030`, and joints 0, 9,
/// 24 and 5 take their new bounds.
#[doc(alias = "grPura_80213030")]
pub fn move_vertices(r: &mut Running) {
    for (i, &(vtx, _, _)) in VTX.iter().enumerate() {
        let Some(j) = r.game.gr.pura_vtx_jobjs.get(i).copied().flatten() else {
            continue;
        };
        let pos = world_position(&mut r.game.objs, r.data, Some(j), None);
        r.game
            .geometry_mut()
            .set_vertex_pos(i32::from(vtx), pos.x, pos.y);
    }
    let geo = r.game.geometry_mut();
    for joint in [0, 9, 0x18, 5] {
        geo.update_bounding(joint);
    }
}

/// The stage's shadow check, `grPura_802130C8`: every joint takes the shadows.
#[doc(alias = "grPura_802130C8")]
pub fn check_shadow_render() -> bool {
    true
}

/// The floats' render callback, `fn_802130D0`: drawn as any stage object is
/// (`grDisplay_801C5DB0`), with the floats' toon texture.
#[doc(alias = "fn_802130D0")]
pub fn render_float(r: &mut Running, gobj: GObjId, pass: i32) {
    crate::hsd::mobj::set_toon_texture_image(&mut r.game.objs, Some(ImageRef::Static(TOON_IMAGE)));
    r.render_stage_object(gobj, pass);
    crate::hsd::mobj::set_toon_texture_image(&mut r.game.objs, None);
}

/// One display object's material's TEV compiled, `grPura_80213224`.
#[doc(alias = "grPura_80213224")]
pub fn compile_one(r: &mut Running, d: Option<DObjId>) {
    if let Some(d) = d {
        let m = r.game.objs.dobjs[d].mobj;
        crate::hsd::mobj::compile_tev(&mut r.game.objs, m);
    }
}

/// A list of display objects' materials' TEV compiled, `grPura_80213128`: the rest of the
/// list's first (recursively), then each of the list's in turn, then the first's again.
#[doc(alias = "grPura_80213128")]
pub fn compile_dobjs(r: &mut Running, d: DObjId) {
    if let Some(n) = r.game.objs.dobjs[d].next {
        compile_dobjs(r, n);
    }
    let mut cur = Some(d);
    while let Some(x) = cur {
        compile_one(r, Some(x));
        cur = r.game.objs.dobjs[x].next;
    }
    let m = r.game.objs.dobjs[d].mobj;
    if m.is_some() {
        crate::hsd::mobj::compile_tev(&mut r.game.objs, m);
    }
}

/// A joint's siblings' and descendants' materials' TEV compiled, then its own,
/// `grPura_80213250`.
#[doc(alias = "grPura_80213250")]
pub fn compile_tree(r: &mut Running, j: JObjId) {
    if let Some(c) = r.game.objs.jobjs[j].child {
        compile_tree(r, c);
    }
    if let Some(n) = r.game.objs.jobjs[j].next {
        compile_tree(r, n);
    }
    if let Some(d) = jobj::get_dobj(&r.game.objs.jobjs, Some(j)) {
        compile_dobjs(r, d);
    }
}
