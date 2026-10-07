//! Setting the stage up, from `melee/gr/ground.c`: `stage_info`'s reset, the stage objects'
//! creation and destruction, the stage's points of interest, camera limits and blast zones, its
//! fog and lights, collision, and the start of the match.

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::super::handles::{CodeRef, GroundId, StartHookId};
use super::super::state::{
    Game, OBJ_FOG, OBJ_JOBJ, OBJ_LIGHT, Obj, Running, USER_DATA_GROUND, UserData,
};
use super::super::structs::Ground;
use super::data::{StageCallbacks, stage_data};
use super::datfiles::{self, GrJoint, MapHead};
use super::ground::{self, ground_of, jobj_of, nth_joint, scale};
use super::{StartHook, dynamic_attr, material};
use crate::fpu::{copy, mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::{aobj, fog, lobj};
use crate::mtx::Mtx34;
use crate::vec::Vec3;

/// `It_Kind_Old_Kuri`: the first kind the stage item table holds.
pub const IT_KIND_OLD_KURI: i32 = 208;

/// Resets `stage_info` and forgets the stage's archives, `Ground_801BFFB0`.
#[doc(alias = "Ground_801BFFB0")]
pub fn reset(game: &mut Game) {
    datfiles::clear(game);
    let i = &mut game.stage_info;
    i.flags = 0;
    i.x6D0 = -1;
    i.x6D2 = 0;
    i.x6D4 = 0;
    i.x6D8 = 0;
    i.x6DC = 0;
    i.x724 = -10000.0;
    i.x12C = None;
    i.unk8C.b3 = false;
    i.unk8C.b2 = true;
    i.on_touch_line = None;
    i.on_check_shadow_render = None;
    i.unk8C.b4 = false;
    i.unk8C.b5 = false;
    i.unk8C.b6 = false;
    i.x714 = -1;
    i.x90 = None;
    i.unk8C.b7 = false;
    i.x720 = -1;
    i.x94 = None;
    i.map_gobjs = [None; 64];
    i.x280 = [None; 261];
    i.x694 = Default::default();
    i.x6A4 = None;
    let c = &mut i.cam_info;
    c.cam_bounds.left = -170.0;
    c.cam_bounds.right = 170.0;
    c.cam_bounds.top = 120.0;
    c.cam_bounds.bottom = -60.0;
    c.cam_x_offset = 0.0;
    c.cam_y_offset = 0.0;
    c.cam_vertical_tilt = 30.0;
    c.cam_pan_degrees = -10.0;
    c.x20 = mem(fp::frsp(0.2));
    c.x24 = mem(fp::frsp(0.2));
    c.cam_zoom_rate = 82.0;
    c.cam_max_depth = 1000.0;
    i.blast_zone.left = -99999.0;
    i.blast_zone.right = 99999.0;
    i.blast_zone.top = 99999.0;
    i.blast_zone.bottom = -99999.0;
    i.grkind = -1;
    i.x6D2 = 0;
    i.x6D4 = 0;
    i.x98 = -1;
    i.x9C = 0;
    i.x72C = None;
    i.x6E0 = 0.0;
    i.x708 = 0;
    i.x740 = 0;
}

/// Resets the fighters' stage devices, `ftCo_800C06C0`.
#[doc(alias = "ftCo_800C06C0")]
pub fn reset_devices(game: &mut Game) {
    game.gr.devices = Default::default();
}

/// Prepares for a stage: the fighters' devices reset and the per-map collision switches
/// allocated, all on, `Ground_801C0378`.
#[doc(alias = "Ground_801C0378")]
pub fn prepare(game: &mut Game) {
    reset_devices(game);
    game.gr.coll_off = Some([0; 64]);
}

/// Sets the camera's angles (`Ground_801C38D0`).
#[doc(alias = "Ground_801C38D0")]
pub fn set_cam_angles(game: &mut Game, tilt: f32, pan: f32, a: f32, b: f32) {
    let c = &mut game.stage_info.cam_info;
    c.cam_vertical_tilt = tilt;
    c.cam_pan_degrees = pan;
    c.x20 = a;
    c.x24 = b;
}

/// `Ground_801C38EC`.
#[doc(alias = "Ground_801C38EC")]
pub fn set_cam_depth(game: &mut Game, depth: f32, zoom: f32) {
    let c = &mut game.stage_info.cam_info;
    c.cam_max_depth = depth;
    c.cam_zoom_rate = zoom;
}

/// The pause camera's limits and the camera's angle limits, `Ground_801C3900`.
#[doc(alias = "Ground_801C3900")]
#[allow(clippy::too_many_arguments)]
pub fn set_pause_cam(
    game: &mut Game,
    x3c: f32,
    min: f32,
    init: f32,
    max: f32,
    up: f32,
    down: f32,
    left: f32,
    right: f32,
) {
    let c = &mut game.stage_info.cam_info;
    c.x3C = x3c;
    c.pausecam_zpos_min = min;
    c.pausecam_zpos_init = init;
    c.pausecam_zpos_max = max;
    c.cam_angle_up = up;
    c.cam_angle_down = down;
    c.cam_angle_left = left;
    c.cam_angle_right = right;
}

/// The fixed camera, `Ground_801C392C`.
#[doc(alias = "Ground_801C392C")]
pub fn set_fixed_cam(game: &mut Game, pos: Vec3, fov: f32, vert: f32, horz: f32) {
    let c = &mut game.stage_info.cam_info;
    c.fixed_cam_pos = pos;
    c.fixed_cam_fov = fov;
    c.fixed_cam_vert_angle = vert;
    c.fixed_cam_horz_angle = horz;
}

/// The camera's fixed zoom, `Ground_801C3950`.
#[doc(alias = "Ground_801C3950")]
pub fn set_cam_fixed_zoom(game: &mut Game, zoom: f32) {
    game.stage_info.cam_info.cam_fixed_zoom = zoom;
}

/// How closely the camera tracks, `Ground_801C3960`.
#[doc(alias = "Ground_801C3960")]
pub fn set_cam_track_ratio(game: &mut Game, ratio: f32) {
    game.stage_info.cam_info.cam_track_ratio = ratio;
}

/// How smoothly the camera tracks, `Ground_801C3970`.
#[doc(alias = "Ground_801C3970")]
pub fn set_cam_track_smooth(game: &mut Game, smooth: f32) {
    game.stage_info.cam_info.cam_track_smooth = smooth;
}

/// The camera limits from points of interest 0x95 and 0x96 around 0x94, or a stage's
/// defaults, `Ground_801C39C0`.
#[doc(alias = "Ground_801C39C0")]
pub fn measure_cam_bounds(game: &mut Game, data: &Archives) {
    let found = (|| {
        let a = ground::point(game, data, 0x95)?;
        let b = ground::point(game, data, 0x96)?;
        let c = ground::point(game, data, 0x94)?;
        Some((a, b, c))
    })();
    let sub = |x: f32, y: f32| mem(fp::fsubs(reg(x), reg(y)));
    let (left, right, bottom, top, cx, cy) = match found {
        Some((a, b, c)) => {
            let (l, r) = if reg(a.x) < reg(b.x) {
                (sub(a.x, c.x), sub(b.x, c.x))
            } else {
                (sub(b.x, c.x), sub(a.x, c.x))
            };
            let (bo, t) = if reg(a.y) < reg(b.y) {
                (sub(a.y, c.y), sub(b.y, c.y))
            } else {
                (sub(b.y, c.y), sub(a.y, c.y))
            };
            (l, r, bo, t, c.x, c.y)
        }
        None => match game.stage_info.grkind {
            2 => (-170.0, 170.0, -100.0, 120.0, 0.0, 180.0),
            14 => (-470.0, 470.0, -60.0, 120.0, 0.0, 300.0),
            26 => (-470.0, 470.0, -60.0, 180.0, 0.0, 130.0),
            25 => (-200.0, 200.0, -60.0, 200.0, 0.0, 130.0),
            3 => (-200.0, 200.0, -160.0, 150.0, 0.0, 0.0),
            11 => (-170.0, 170.0, -60.0, 180.0, 0.0, 0.0),
            18 => (-170.0, 170.0, 10.0, 180.0, 0.0, 0.0),
            _ => (-170.0, 170.0, -60.0, 120.0, 0.0, 0.0),
        },
    };
    let c = &mut game.stage_info.cam_info;
    c.cam_bounds.top = top;
    c.cam_bounds.bottom = bottom;
    c.cam_bounds.left = left;
    c.cam_bounds.right = right;
    c.cam_x_offset = cx;
    c.cam_y_offset = cy;
}

/// The blast zones from points of interest 0x97 and 0x98, relative to the camera's center,
/// or a stage's defaults, `Ground_801C3BB4`.
#[doc(alias = "Ground_801C3BB4")]
pub fn measure_blast_zones(game: &mut Game, data: &Archives) {
    let found = (|| {
        let a = ground::point(game, data, 0x97)?;
        let b = ground::point(game, data, 0x98)?;
        Some((a, b))
    })();
    let (cx, cy) = (
        game.stage_info.cam_info.cam_x_offset,
        game.stage_info.cam_info.cam_y_offset,
    );
    let sub = |x: f32, y: f32| mem(fp::fsubs(reg(x), reg(y)));
    let (left, right, top, bottom) = match found {
        Some((a, b)) => {
            let (l, r) = if reg(a.x) < reg(b.x) {
                (sub(a.x, cx), sub(b.x, cx))
            } else {
                (sub(b.x, cx), sub(a.x, cx))
            };
            let (bo, t) = if reg(a.y) < reg(b.y) {
                (sub(a.y, cy), sub(b.y, cy))
            } else {
                (sub(b.y, cy), sub(a.y, cy))
            };
            (l, r, t, bo)
        }
        None => match game.stage_info.grkind {
            2 => (-250.0, 250.0, 180.0, -150.0),
            14 => (-550.0, 550.0, 200.0, -100.0),
            26 => (-600.0, 600.0, 200.0, -100.0),
            7 => (-550.0, 550.0, 200.0, -150.0),
            25 => (-300.0, 300.0, 300.0, -160.0),
            3 => (-300.0, 300.0, 270.0, -240.0),
            11 => (-300.0, 300.0, 210.0, -240.0),
            _ => (-250.0, 250.0, 200.0, -100.0),
        },
    };
    let b = &mut game.stage_info.blast_zone;
    b.top = top;
    b.bottom = bottom;
    b.left = left;
    b.right = right;
}

/// Queues a process to call with a stage object when the match starts, `Ground_801C10B8`.
#[doc(alias = "Ground_801C10B8")]
pub fn add_start_hook(game: &mut Game, gobj: GObjId, func: CodeRef) {
    let next = game.stage_info.x6A4;
    let id = game.gr.start_hooks.alloc(StartHook {
        next,
        gobj: Some(gobj),
        func,
    });
    game.stage_info.x6A4 = Some(StartHookId(id));
}

/// The collision joints of a map: the archive's for it, then the stage's own.
fn map_coll_joints(game: &Game, data: &Archives, map_id: i32) -> (Vec<GrJoint>, Vec<GrJoint>) {
    let from_archive = match datfiles::archive_for(game, data, map_id) {
        Some(head) => {
            let (list, n) = head.entry(data, map_id).coll_joints(data);
            (0..n.max(0) as u32)
                .map(|i| GrJoint::read(data, list.expect("collision joints").at(6 * i)))
                .collect()
        }
        None => Vec::new(),
    };
    (
        from_archive,
        stage_data(game.stage_info.grkind).joints.to_vec(),
    )
}

/// Binds a collision joint to the `z`th joint of a model, moves it there and settles it
/// (`mpLib_800552B0`, `mpLib_80055E9C`, `mpLib_80057424`).
fn bind_coll_joint(game: &mut Game, data: &Archives, j: GrJoint, root: Option<JObjId>) {
    struct Tree<'a>(&'a crate::hsd::jobj::JObjs);
    impl crate::mp::JObjTree<JObjId> for Tree<'_> {
        fn child(&self, j: JObjId) -> Option<JObjId> {
            self.0[j].child
        }
        fn next(&self, j: JObjId) -> Option<JObjId> {
            self.0[j].next
        }
        fn parent(&self, j: JObjId) -> Option<JObjId> {
            self.0[j].parent
        }
        fn flags(&self, j: JObjId) -> u32 {
            self.0[j].flags
        }
    }
    let Game { geometry, objs, .. } = game;
    let g = geometry.as_mut().expect("no collision geometry");
    g.bind_jobj(j.x.into(), &Tree(&objs.jobjs), root, j.z.into());
    ground::update_coll_joint(game, data, j.x.into());
    game.geometry.as_mut().unwrap().settle_vertices(j.x.into());
}

/// Binds a map's collision joints to its model and switches them on, `Ground_InitMapColl`:
/// the archive's joints for the map, then the stage's own. Whether there were any.
#[doc(alias = "Ground_InitMapColl")]
pub fn init_map_coll(game: &mut Game, data: &Archives, j: Option<JObjId>, map_id: i32) -> bool {
    let (from_archive, own) = map_coll_joints(game, data, map_id);
    let mut any = false;
    for c in from_archive {
        bind_coll_joint(game, data, c, j);
        any = true;
    }
    for c in own.into_iter().filter(|c| i32::from(c.y) == map_id) {
        bind_coll_joint(game, data, c, j);
        any = true;
    }
    switch_on(game, data, map_id);
    any
}

/// Calls `f` with each collision joint of a map, the stage's own first, `Ground_801C3128`.
#[doc(alias = "Ground_801C3128")]
fn each_coll_joint(
    game: &mut Game,
    data: &Archives,
    map_id: i32,
    mut f: impl FnMut(&mut Game, i32),
) -> bool {
    let (from_archive, own) = map_coll_joints(game, data, map_id);
    let mut any = false;
    for c in own.iter().filter(|c| i32::from(c.y) == map_id) {
        f(game, c.x.into());
        any = true;
    }
    for c in &from_archive {
        f(game, c.x.into());
        any = true;
    }
    any
}

/// Switches a map's collision joints back on if they were off, `Ground_801C3214`.
#[doc(alias = "Ground_801C3214")]
pub fn switch_on(game: &mut Game, data: &Archives, map_id: i32) -> bool {
    let off = game
        .gr
        .coll_off
        .as_mut()
        .expect("Ground_804D6950 isn't allocated");
    if off[map_id as usize] != 1 {
        return false;
    }
    off[map_id as usize] = 0;
    each_coll_joint(game, data, map_id, |game, j| {
        game.geometry.as_mut().unwrap().enable_joint(j)
    })
}

/// Switches a map's collision joints off if they were on, `Ground_801C3260`.
#[doc(alias = "Ground_801C3260")]
pub fn switch_off(game: &mut Game, data: &Archives, map_id: i32) -> bool {
    let off = game
        .gr
        .coll_off
        .as_mut()
        .expect("Ground_804D6950 isn't allocated");
    if off[map_id as usize] != 0 {
        return false;
    }
    off[map_id as usize] = 1;
    each_coll_joint(game, data, map_id, |game, j| {
        game.geometry.as_mut().unwrap().disable_joint(j)
    })
}

/// Fills `stage_info.x280`'s points of interest with a model's joints, `Ground_801C34AC`: the
/// archive's index table for the model's descriptor lists (joint index, slot) pairs, the
/// joints counted depth first from the root.
#[doc(alias = "Ground_801C34AC")]
pub fn fill_points(
    game: &mut Game,
    data: &Archives,
    map_id: i32,
    root: Option<JObjId>,
    joint: Option<DataRef>,
) {
    let Some(head) = datfiles::archive_for(game, data, map_id) else {
        return;
    };
    let (Some(root), Some(joint)) = (root, joint) else {
        return;
    };
    let (tables, count) = head.index_tables(data);
    if count == 0 {
        return;
    }
    let tables = tables.expect("index tables");
    let Some(entry) = (0..count as u32)
        .map(|i| tables.at(0xC * i))
        .find(|e| data.ptr(*e) == Some(joint))
    else {
        return;
    };
    let pairs = data.ptr(entry.at(4));
    let n = data.u32(entry.at(8)) as i32;
    let jobjs = &game.objs.jobjs;
    let mut cur = Some(root);
    let mut prev: i32 = -1;
    for k in 0..n.max(0) as u32 {
        let pair = pairs.expect("index pairs").at(4 * k);
        let target = i32::from(data.u16(pair) as i16);
        let slot = data.u16(pair.at(2)) as i16;
        let mut i;
        if prev > target || prev == -1 {
            cur = Some(root);
            i = 0;
        } else {
            i = prev;
        }
        while let Some(j) = cur {
            if i == target {
                break;
            }
            cur = next_in_walk(jobjs, j);
            i += 1;
        }
        prev = i;
        game.stage_info.x280[slot as usize] = cur;
    }
}

/// The joint after `j` depth first, not entering instances' children (as `Ground_801C4100`).
fn next_in_walk(jobjs: &crate::hsd::jobj::JObjs, j: JObjId) -> Option<JObjId> {
    let o = &jobjs[j];
    if o.flags & jobj::INSTANCE == 0
        && let Some(c) = o.child
    {
        return Some(c);
    }
    if let Some(n) = o.next {
        return Some(n);
    }
    let mut up = j;
    loop {
        let p = jobjs[up].parent?;
        if let Some(n) = jobjs[p].next {
            return Some(n);
        }
        up = p;
    }
}

/// Marks a joint's descendants' and its own display objects' materials `1 << 26`,
/// `lb_8000F9F8` (through `lb_80011C18`).
#[doc(alias = "lb_8000F9F8")]
pub fn set_apart(game: &mut Game, j: JObjId) {
    crate::lb::joint::set_mobj_flags(&mut game.objs, j, 1 << 26);
}

/// The joint a stage object's model hangs under, scaling it by the stage's scale (the inline
/// `get_jobj_inline`, which loads it from a descriptor on its stack). The original also puts
/// the joint in HSD's ID table under that stack address, which nothing can look up.
fn scale_joint(game: &mut Game, s: f32) -> JObjId {
    let j = jobj::alloc(&mut game.objs.jobjs);
    let o = &mut game.objs.jobjs[j];
    o.rotate.x = 0.0;
    o.rotate.y = 0.0;
    o.rotate.z = 0.0;
    o.scale = Vec3::new(copy(s), copy(s), copy(s));
    o.translate = Vec3::default();
    o.mtx = Mtx34::IDENTITY;
    o.scl = None;
    j
}

/// A new stage object's user data (`alloc_user_data_ground`), attached to its GObj.
///
/// The original leaves the block uncleared; Slippi's playback codes zero it (a determinism
/// fix). At stage creation the heap's blocks are fresh, zero memory, so both start as zero
/// here (see [`super::vars`]).
fn new_ground(game: &mut Game, gobj: GObjId) -> GroundId {
    let id = GroundId(game.grounds.alloc(Ground::default()));
    game.world
        .init_user_data(gobj, USER_DATA_GROUND, UserData::Ground(id));
    id
}

/// Creates a stage object for map `map_id`, `Ground_GetStageGObj`: its user data, its model
/// from the stage's archive (or an empty joint for a map past the archive's) under a joint
/// that scales it, its two processes, and its place in `stage_info.map_gobjs`.
#[doc(alias = "Ground_GetStageGObj")]
pub fn stage_gobj(r: &mut Running, map_id: i32) -> Option<GObjId> {
    let (game, data) = (&mut *r.game, r.data);
    let gobj = game.world.create(3, 5, 0);
    let g = new_ground(game, gobj);
    {
        let gp = &mut game.grounds[g.0];
        gp.map_id = map_id;
        gp.gobj = Some(gobj);
        gp.x10_flags.b0 = false;
        gp.x10_flags.b1 = false;
        gp.x10_flags.b2 = true;
        gp.x8_callback = None;
        gp.xC_callback = None;
        gp.x1C_callback = None;
        gp.x10_flags.b5 = false;
        gp.x10_flags.b6 = false;
        gp.x11_flags.b012 = 0;
        gp.x10_flags.b7 = false;
        gp.x18 = None;
        gp.x10_flags.b3 = false;
        gp.x20 = [-1; 8];
    }
    material::reset_overlay(game, gobj);
    let main = datfiles::main_head(game);
    let root = if map_id < main.count(data) {
        let head = datfiles::archive_for(game, data, map_id).expect("ground.c: archive");
        let entry = head.entry(data, map_id);
        let joint = entry.joint(data);
        let model = jobj::load_joint(&mut game.objs, data, joint);
        fill_points(game, data, map_id, model, joint);
        let sj = scale_joint(game, scale(game));
        jobj::add_next(&mut game.objs.jobjs, model, Some(sj));
        let cb = &stage_data(game.stage_info.grkind).callbacks[map_id as usize];
        if cb.flags & StageCallbacks::CAMERA != 0 && entry.camera(data).is_some() {
            unimplemented!("stage objects with a camera of their own (lb_80013B14)");
        }
        game.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(sj));
        let (list, n) = entry.set_apart(data);
        for k in 0..n.max(0) as u32 {
            let idx = data.u16(list.expect("set-apart joints").at(2 * k)) as i16;
            if let Some(j) = nth_joint(game, gobj, idx.into()) {
                set_apart(game, j);
            }
        }
        sj
    } else {
        let model = jobj::alloc(&mut game.objs.jobjs);
        game.objs.jobjs[model].mtx = Mtx34::IDENTITY;
        game.objs.jobjs[model].scl = None;
        let sj = scale_joint(game, scale(game));
        jobj::add_next(&mut game.objs.jobjs, Some(model), Some(sj));
        game.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(sj));
        sj
    };
    let _ = root;
    game.world.setup_proc(gobj, CodeRef(0x801C_1CD0), 1);
    game.world.setup_proc(gobj, CodeRef(0x801C_1D38), 4);
    game.stage_info.map_gobjs[map_id as usize] = Some(gobj);
    Some(gobj)
}

/// Creates a stage object and sets it up from its callbacks (each stage's copy of
/// `grLast_8021A7F4`): its render callback, destroy callback, setup and process.
pub fn create_from_callbacks(r: &mut Running, map_id: i32, cb: &StageCallbacks) -> Option<GObjId> {
    let gobj = stage_gobj(r, map_id)?;
    let g = ground_of(r.game, gobj);
    let gp = &mut r.game.grounds[g.0];
    gp.x8_callback = None;
    gp.xC_callback = None;
    // grDisplay_801C5DB0 renders it.
    r.game.world.setup_gx_link(gobj, CodeRef(0x801C_5DB0), 3, 0);
    if cb.callback3 != 0 {
        r.game.grounds[g.0].x1C_callback = Some(CodeRef(cb.callback3));
    }
    if cb.on_init != 0 {
        super::super::code::run_proc(r, gobj, CodeRef(cb.on_init));
    }
    if cb.gobj_proc != 0 {
        r.game.world.setup_proc(gobj, CodeRef(cb.gobj_proc), 4);
    }
    Some(gobj)
}

/// The stage's fog descriptor: the archive's for the object the callbacks mark (the inline
/// `foo` in `Ground_801C1E94`).
fn fog_desc(game: &Game, data: &Archives) -> Option<DataRef> {
    let main = datfiles::main_head(game);
    let callbacks = stage_data(game.stage_info.grkind).callbacks;
    let n = main.count(data);
    (0..n)
        .find(|&i| callbacks[i as usize].flags & StageCallbacks::FOG != 0)
        .and_then(|i| {
            let head = datfiles::archive_for(game, data, i).expect("ground.c: fog archive");
            head.entry(data, i).fog(data)
        })
}

/// Creates the stage's fog, `Ground_801C1E94`: its GObj, its range in the stage's scale, and
/// the camera's clear color from it (black without one).
#[doc(alias = "Ground_801C1E94")]
pub fn create_fog(game: &mut Game, data: &Archives) {
    let Some(desc) = fog_desc(game, data) else {
        crate::game::camera::set_background_color(&mut game.camera.cam, 0, 0, 0);
        return;
    };
    let gobj = game.world.create(0xA, 0xB, 0);
    let f = fog::load_desc(&mut game.objs.fogs, data, desc);
    game.world.set_obj(gobj, OBJ_FOG, Obj::Fog(f));
    // Ground_801C1E2C applies it.
    game.world.setup_gx_link(gobj, CodeRef(0x801C_1E2C), 0, 0);
    let s = reg(scale(game));
    let o = &mut game.objs.fogs[f];
    o.start = mem(fp::fmuls(reg(o.start), s));
    o.end = mem(fp::fmuls(reg(o.end), s));
    let [r, g, b, _] = o.color;
    crate::game::camera::set_background_color(&mut game.camera.cam, r, g, b);
    game.stage_info.x12C = Some(gobj);
}

/// Whether a light's animation loops, by the archive's shadow entries, `Ground_801C43C4`.
#[doc(alias = "Ground_801C43C4")]
pub fn light_anim_loops(game: &Game, data: &Archives, anim: Option<DataRef>) -> bool {
    let (list, n) = datfiles::main_head(game).shadows(data);
    let Some(anim) = anim else { return false };
    if n == 0 {
        return false;
    }
    let list = list.expect("shadow entries");
    for i in 0..n as u32 {
        let e = list.at(8 * i);
        if data.ptr(e) == Some(anim) {
            return data.u8(e.at(4)) & 0x80 != 0;
        }
    }
    panic!("ground.c: a light animation without a shadow entry");
}

/// The light list the stage uses: the archive's for the object the callbacks mark, through
/// `Ground_801C20E0`, or the defaults (`Ground_803E06C8`).
fn light_list(game: &Game, data: &Archives) -> Option<DataRef> {
    let main = datfiles::main_head(game);
    let callbacks = stage_data(game.stage_info.grkind).callbacks;
    let n = main.count(data);
    for i in 0..n {
        if callbacks[i as usize].flags & StageCallbacks::LIGHTS != 0 {
            let head = datfiles::archive_for(game, data, i).expect("ground.c: light archive");
            let lights = head
                .entry(data, i)
                .lights(data)
                .expect("ground.c: lightset");
            light_overrides(data, head, lights);
            return Some(lights);
        }
    }
    None
}

/// The lights fighters use: the stage's own (`stage_info.map_plit`, through
/// `Ground_801C20E0`), or none for the defaults, `Ground_801C49B4`.
#[doc(alias = "Ground_801C49B4")]
pub fn fighter_light_list(game: &Game, data: &Archives) -> Option<DataRef> {
    let list = game.stage_info.map_plit?;
    light_overrides(data, datfiles::main_head(game), list);
    Some(list)
}

/// Applies the archive's light overrides to a light list, `Ground_801C20E0`: the list is used
/// as it is unless an override for one of its lights sets a flag, which would make the
/// original edit the loaded list and descriptors (the stages ported have none).
#[doc(alias = "Ground_801C20E0")]
fn light_overrides(data: &Archives, head: MapHead, lights: DataRef) {
    assert!(data.ptr(lights).is_some(), "ground.c: *lightset");
    let (list, n) = head.light_overrides(data);
    let mut at = lights;
    while let Some(entry) = data.ptr(at) {
        let desc = data.ptr(entry);
        for i in 0..n.max(0) as u32 {
            let o = list.expect("light overrides").at(8 * i);
            if data.ptr(o) == desc && data.u8(o.at(4)) & 0xE0 != 0 {
                unimplemented!("Ground_801C20E0's light overrides write into loaded data");
            }
        }
        at = at.at(4);
    }
}

/// Loads a light list (`LightList**`: entries of a descriptor and its animations) into a chain
/// of lights, `lb_80011AC4`.
#[doc(alias = "lb_80011AC4")]
pub fn load_light_list(game: &mut Game, data: &Archives, list: DataRef) -> Option<lobj::LObjId> {
    let mut first = None;
    let mut prev: Option<lobj::LObjId> = None;
    let mut at = list;
    while let Some(entry) = data.ptr(at) {
        let cur = lobj::load_desc(&mut game.objs, data, data.ptr(entry));
        if let Some(anims) = data.ptr(entry.at(4)) {
            lobj::add_anim_all(&mut game.objs, data, cur, data.ptr(anims));
        }
        match prev {
            Some(p) => game.objs.lobjs[p].next = cur,
            None => first = cur,
        }
        prev = cur;
        at = at.at(4);
    }
    first
}

/// Creates the stage's lights, `Ground_801C466C`: the stage's light list (or the defaults),
/// positions in the stage's scale, animations started at rate 1 and looped as the archive
/// says, run once, and their process.
#[doc(alias = "Ground_801C466C")]
pub fn create_lights(game: &mut Game, data: &Archives) {
    let Some(list) = light_list(game, data) else {
        unimplemented!("the default lights (Ground_803E06C8) live in the executable");
    };
    let gobj = game.world.create(0xD, 3, 0);
    let first = load_light_list(game, data, list).expect("ground.c: lobj");
    game.world.set_obj(gobj, OBJ_LIGHT, Obj::LObj(first));
    // Ground_801C4640 sets them up for rendering.
    game.world.setup_gx_link(gobj, CodeRef(0x801C_4640), 0, 0);
    let s = reg(scale(game));
    let scaled = |v: Vec3| {
        Vec3::new(
            mem(fp::fmuls(reg(v.x), s)),
            mem(fp::fmuls(reg(v.y), s)),
            mem(fp::fmuls(reg(v.z), s)),
        )
    };
    let mut cur = Some(first);
    while let Some(l) = cur {
        if let Some(p) = lobj::position(&mut game.objs, data, Some(l)) {
            lobj::set_position(&mut game.objs, l, scaled(p));
        }
        if let Some(i) = lobj::interest(&mut game.objs, data, Some(l)) {
            lobj::set_interest(&mut game.objs, l, scaled(i));
        }
        cur = game.objs.lobjs[l].next;
    }
    lobj::req_anim_all(&mut game.objs, Some(first), 0.0);
    for a in light_anims(game, first, u32::MAX) {
        aobj::set_rate(&mut game.objs.aobjs, Some(a), 1.0);
    }
    let first_entry = data.ptr(list).expect("ground.c: a light");
    if data.ptr(first_entry.at(4)).is_some() {
        let mut entry_at = list;
        let mut cur = Some(first);
        while let Some(l) = cur {
            let entry = data.ptr(entry_at);
            let anim = entry
                .and_then(|e| data.ptr(e.at(4)))
                .and_then(|a| data.ptr(a));
            if light_anim_loops(game, data, anim) {
                if let Some(a) = game.objs.lobjs[l].aobj {
                    aobj::set_flags(&mut game.objs.aobjs, Some(a), aobj::LOOP);
                }
                for w in [game.objs.lobjs[l].position, game.objs.lobjs[l].interest]
                    .into_iter()
                    .flatten()
                {
                    for a in wobj_anims(game, w) {
                        aobj::set_flags(&mut game.objs.aobjs, Some(a), aobj::LOOP);
                    }
                }
            }
            cur = game.objs.lobjs[l].next;
            entry_at = entry_at.at(4);
        }
    }
    lobj::anim_all(&mut game.objs, data, Some(first));
    game.world.setup_proc(gobj, CodeRef(0x801C_461C), 0);
}

/// A world position's animations as `HSD_ForeachAnim` visits them: its own, then its
/// references'.
fn wobj_anims(game: &Game, w: crate::hsd::wobj::WObjId) -> Vec<aobj::AObjId> {
    let o = &game.objs.wobjs[w.0];
    let mut out: Vec<_> = o.aobj.into_iter().collect();
    let mut r = o.robj;
    while let Some(ro) = r {
        if let Some(a) = game.objs.robjs[ro].aobj {
            out.push(a);
        }
        r = game.objs.robjs[ro].next;
    }
    out
}

/// A light chain's animations as `HSD_ForeachAnim` visits them: each light's, then its
/// position's and interest's.
fn light_anims(game: &Game, first: lobj::LObjId, _mask: u32) -> Vec<aobj::AObjId> {
    let mut out = Vec::new();
    let mut cur = Some(first);
    while let Some(l) = cur {
        let o = &game.objs.lobjs[l];
        out.extend(o.aobj);
        for w in [o.position, o.interest].into_iter().flatten() {
            out.extend(wobj_anims(game, w));
        }
        cur = o.next;
    }
    out
}

/// Loads the stage's collision, `mpLibLoad`: the dynamic attribute areas reset and the
/// geometry loaded from the stage's `coll_data` in its scale. The update counter carries on.
#[doc(alias = "mpLibLoad")]
pub fn load_collision(game: &mut Game, data: &Archives) {
    dynamic_attr::reset(&mut game.gr.dynamic_attrs);
    let generation = game
        .geometry
        .as_ref()
        .map_or(game.gr.coll_generation, |g| g.generation);
    let mut g = super::Geometry::load(
        data,
        game.stage_info.coll_data,
        scale(game),
        game.stage_info.grkind,
    );
    g.generation = generation;
    game.geometry = Some(g);
}

/// Creates the stage's collision GObj and resets the stage's boxes, `mpLib_80058820`.
#[doc(alias = "mpLib_80058820")]
pub fn create_collision_gobj(game: &mut Game) {
    let gobj = game.world.create(1, 6, 0);
    game.world.setup_proc(gobj, CodeRef(0x8005_87FC), 4);
    game.geometry
        .as_mut()
        .expect("no collision geometry")
        .reset_bounds();
}

/// Sets the stage up once its archive is loaded, `Ground_801C0800`: the camera from its
/// parameters, its items' descriptions, its collision, fog and lights, then the stage's own
/// setup.
#[doc(alias = "Ground_801C0800")]
pub fn setup(r: &mut Running, grkind: i32) {
    let (game, data) = (&mut *r.game, r.data);
    let p = game.gr.param.clone().expect("stage_info.param is null");
    let f = |x: i32| mem(fp::frsp(f64::from(x)));
    set_cam_angles(game, f(p.x8.into()), f(p.x14), p.x1C, p.x18);
    set_cam_depth(game, f(p.x10), f(p.xC));
    set_cam_track_smooth(game, p.x28);
    set_pause_cam(
        game,
        f(p.x2E.into()),
        f(p.x30),
        f(p.x34),
        f(p.x38),
        p.x3C,
        p.x40,
        p.x44,
        p.x48,
    );
    set_fixed_cam(game, Vec3::new(p.x50, p.x54, p.x58), p.x5C, p.x60, p.x64);
    set_cam_track_ratio(game, p.x20);
    set_cam_fixed_zoom(game, p.x24);
    if let Some(items) = game.stage_info.itemdata {
        let mut at = items;
        while let Some(item) = data.ptr(at) {
            let kind = data.u32(item) as i32;
            game.gr.stage_articles[(kind - IT_KIND_OLD_KURI) as usize] = data.ptr(item.at(4));
            at = at.at(4);
        }
    }
    if let Some(yaku) = game.stage_info.ald_yaku_all {
        let mut i = 1;
        while let Some(script) = data.ptr(yaku.at(4 * i)) {
            game.gr.yaku_scripts.retain(|&(k, _)| u32::from(k) != i);
            game.gr.yaku_scripts.push((i as u8, script));
            i += 1;
        }
    }
    if let (Some(ptcl), Some(texg)) = (game.stage_info.map_ptcl, game.stage_info.map_texg) {
        crate::hsd::particle::load_data_bank(&mut game.objs, data, 0x1E, ptcl, texg, None, None);
    }
    load_collision(game, data);
    create_collision_gobj(game);
    create_fog(game, data);
    create_lights(game, data);
    super::super::code::run_event(r, stage_data(grkind).on_init);
}

/// Runs the stage's load hook, `Ground_OnLoad`.
#[doc(alias = "Ground_OnLoad")]
pub fn on_load(r: &mut Running, grkind: i32) {
    super::super::code::run_event(r, stage_data(grkind).on_load);
}

/// Starts the match on the stage, `Ground_801C0FB8`: the stage's start hook, the queued start
/// hooks (each process called with its object, then freed), and the watcher's GObj.
#[doc(alias = "Ground_801C0FB8")]
pub fn start(r: &mut Running, grkind: i32) {
    super::super::code::run_event(r, stage_data(grkind).on_start);
    let mut cur = r.game.stage_info.x6A4;
    while let Some(h) = cur {
        let hook = r.game.gr.start_hooks[h.0].clone();
        cur = hook.next;
        super::super::code::run_proc(r, hook.gobj.expect("a start hook's GObj"), hook.func);
        r.game.gr.start_hooks.free(h.0);
    }
    r.game.stage_info.x6A4 = None;
    let gobj = r.game.world.create(3, 5, 0);
    r.game.world.setup_proc(gobj, CodeRef(0x801C_0C2C), 10);
}

/// Destroys a stage object, `Ground_801C4A08`: its destroy callback, its map slot, its sounds,
/// its camera GObj, its collision (switched off) and its points of interest, then the GObj.
#[doc(alias = "Ground_801C4A08")]
pub fn destroy(r: &mut Running, gobj: Option<GObjId>) {
    let Some(gobj) = gobj else { return };
    let g = ground_of(r.game, gobj);
    let jobj = jobj_of(r.game, gobj);
    let map_id = r.game.grounds[g.0].map_id;
    if let Some(cb) = r.game.grounds[g.0].x1C_callback {
        super::super::code::run_proc(r, gobj, cb);
    }
    if r.game.stage_info.map_gobjs[map_id as usize] == Some(gobj) {
        r.game.stage_info.map_gobjs[map_id as usize] = None;
    }
    if r.game.grounds[g.0].x20.iter().any(|&s| s != -1) {
        unimplemented!("a stage object's sounds (Ground_801C55AC)");
    }
    if r.game.grounds[g.0].x18.is_some() {
        unimplemented!("a stage object's camera GObj");
    }
    if jobj.is_some() && r.game.gr.coll_off.as_ref().expect("Ground_804D6950")[map_id as usize] == 0
    {
        switch_off(r.game, r.data, map_id);
    }
    if let Some(head) = datfiles::archive_for(r.game, r.data, map_id) {
        let joint = head.entry(r.data, map_id).joint(r.data);
        forget_points(r.game, map_id, jobj, joint, head, r.data);
    }
    crate::hsd::gobj::free(r, gobj);
}

/// Forgets the points of interest a model held, `Ground_801C36F4`: if the archive indexes the
/// model's descriptor, every point whose joint's root is the model.
#[doc(alias = "Ground_801C36F4")]
fn forget_points(
    game: &mut Game,
    _map_id: i32,
    root: Option<JObjId>,
    joint: Option<DataRef>,
    head: MapHead,
    data: &Archives,
) {
    let (Some(root), Some(joint)) = (root, joint) else {
        return;
    };
    let (tables, count) = head.index_tables(data);
    if count == 0 {
        return;
    }
    let tables = tables.expect("index tables");
    if !(0..count as u32).any(|i| data.ptr(tables.at(0xC * i)) == Some(joint)) {
        return;
    }
    for i in 0..0x57 * 3 {
        if let Some(mut j) = game.stage_info.x280[i] {
            while let Some(p) = game.objs.jobjs[j].parent {
                j = p;
            }
            if j == root {
                game.stage_info.x280[i] = None;
            }
        }
    }
}
