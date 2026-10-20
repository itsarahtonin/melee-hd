//! Setting the stage up, from `melee/gr/ground.c`: `stage_info`'s reset, the stage objects'
//! creation and destruction, the stage's points of interest, camera limits and blast zones, its
//! fog and lights, collision, and the start of the match.

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::super::code::unported_call;
use super::super::handles::{CodeRef, GroundId, StartHookId};
use super::super::state::{
    Game, OBJ_FOG, OBJ_JOBJ, OBJ_LIGHT, Obj, Running, USER_DATA_GROUND, UserData,
};
use super::super::structs::{Ground, GroundParam};
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
use melee_data::archive::ArchiveId;

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
    crate::hsd::heap::mem_alloc(64);
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
    game.gr
        .start_hooks
        .set_addr(id, crate::hsd::heap::mem_alloc(0xC));
    game.stage_info.x6A4 = Some(StartHookId(id));
}

/// The collision joints of a map: the archive's for it (where its list is and how many it
/// has, read with [`archive_coll_joint`]), then the stage's own.
fn map_coll_joints(
    game: &Game,
    data: &Archives,
    map_id: i32,
) -> ((Option<DataRef>, u32), &'static [GrJoint]) {
    let from_archive = match datfiles::archive_for(game, data, map_id) {
        Some(head) => {
            let (list, n) = head.entry(data, map_id).coll_joints(data);
            (list, n.max(0) as u32)
        }
        None => (None, 0),
    };
    (from_archive, stage_data(game.stage_info.grkind).joints)
}

/// The collision joint bound to part `part` of map `map_id`, `Ground_801C32D4`: the last of
/// the stage's own joints for that map and part, then the last of the map's archive's for the
/// part, or -1.
#[doc(alias = "Ground_801C32D4")]
pub fn coll_joint_id(game: &Game, data: &Archives, map_id: i32, part: i32) -> i32 {
    coll_joint_id_in(&game.gr, game.stage_info.grkind, data, map_id, part)
}

/// [`coll_joint_id`] from the stage code's globals and the stage's kind, for callers that hold
/// those alone (a collision joint's callback).
pub fn coll_joint_id_in(
    gr: &super::Globals,
    grkind: i32,
    data: &Archives,
    map_id: i32,
    part: i32,
) -> i32 {
    let from_archive = match datfiles::archive_for_in(gr, data, map_id) {
        Some(head) => {
            let (list, n) = head.entry(data, map_id).coll_joints(data);
            (list, n.max(0) as u32)
        }
        None => (None, 0),
    };
    let ((list, n), own) = (from_archive, stage_data(grkind).joints);
    let mut result = -1;
    for j in own {
        if i32::from(j.y) == map_id && i32::from(j.z) == part {
            result = i32::from(j.x);
        }
    }
    for i in 0..n {
        let j = archive_coll_joint(data, list, i);
        if i32::from(j.z) == part {
            result = i32::from(j.x);
        }
    }
    result
}

/// The part of map `map_id` collision joint `joint` is bound to, `Ground_801C33C0`
/// ([`coll_joint_id`] the other way): the last of the stage's own joints for that map and
/// joint, then the last of the map's archive's for the joint, or -1.
#[doc(alias = "Ground_801C33C0")]
pub fn coll_joint_part(game: &Game, data: &Archives, map_id: i32, joint: i32) -> i32 {
    let ((list, n), own) = map_coll_joints(game, data, map_id);
    let mut result = -1;
    for j in own {
        if i32::from(j.y) == map_id && i32::from(j.x) == joint {
            result = i32::from(j.z);
        }
    }
    for i in 0..n {
        let j = archive_coll_joint(data, list, i);
        if i32::from(j.x) == joint {
            result = i32::from(j.z);
        }
    }
    result
}

/// The `i`th collision joint of an archive's list.
fn archive_coll_joint(data: &Archives, list: Option<DataRef>, i: u32) -> GrJoint {
    GrJoint::read(data, list.expect("collision joints").at(6 * i))
}

/// Binds a collision joint to the `z`th joint of a model, moves it there and settles it
/// (`mpLib_800552B0`, `mpLib_80055E9C`, `mpLib_80057424`).
fn bind_coll_joint(game: &mut Game, data: &Archives, j: GrJoint, root: Option<JObjId>) {
    game.gr
        .freed_coll_jobjs
        .retain(|&(k, _)| k != i32::from(j.x));
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
    let ((list, n), own) = map_coll_joints(game, data, map_id);
    let mut any = false;
    for i in 0..n {
        bind_coll_joint(game, data, archive_coll_joint(data, list, i), j);
        any = true;
    }
    for &c in own.iter().filter(|c| i32::from(c.y) == map_id) {
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
    let ((list, n), own) = map_coll_joints(game, data, map_id);
    let mut any = false;
    for c in own.iter().filter(|c| i32::from(c.y) == map_id) {
        f(game, c.x.into());
        any = true;
    }
    for i in 0..n {
        f(game, archive_coll_joint(data, list, i).x.into());
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

/// Where `Ground_GetStageGObj` keeps the descriptor of a stage object's scale joint on the
/// stack ([`scale_joint`]) when a stage's setup makes its objects, through the stage's own
/// create function: the stack address is the joint's ID table key. Other callers have deeper
/// frames, given at their calls.
pub const SETUP_FRAME: u32 = 0x804E_EA00;
/// How much lower the descriptor of the scale joint of an object past the archive's maps
/// sits (the inline's second copy in `Ground_GetStageGObj`).
const EMPTY_MAP_SLOT: u32 = 0x44;

/// The joint a stage object's model hangs under, scaling it by the stage's scale (the inline
/// `get_jobj_inline`, which loads it from a descriptor on its stack). The original also puts
/// the joint in HSD's ID table under that stack address (`frame`), which nothing can look up
/// but which takes an entry ([`crate::hsd::id::stack_key`]).
fn scale_joint(game: &mut Game, s: f32, frame: u32) -> JObjId {
    let j = jobj::alloc(&mut game.objs);
    let key = crate::hsd::id::stack_key(frame);
    game.objs.ids.insert(key, j);
    game.objs.jobjs[j].id = Some(key);
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

/// `sizeof(Ground)`, the block a stage object's user data takes from the scene's heap.
pub const GROUND_SIZE: u32 = 0x204;

/// A new stage object's user data (`alloc_user_data_ground`), attached to its GObj.
///
/// The original leaves the block uncleared; Slippi's playback codes zero it (a determinism
/// fix). At stage creation the heap's blocks are fresh, zero memory, so both start as zero
/// here (see [`super::vars`]).
fn new_ground(game: &mut Game, gobj: GObjId) -> GroundId {
    let id = GroundId(game.grounds.alloc(Ground::default()));
    game.grounds
        .set_addr(id.0, crate::hsd::heap::mem_alloc(GROUND_SIZE));
    game.world
        .init_user_data(gobj, USER_DATA_GROUND, UserData::Ground(id));
    id
}

/// Creates a stage object for map `map_id`, `Ground_GetStageGObj`: its user data, its model
/// from the stage's archive (or an empty joint for a map past the archive's) under a joint
/// that scales it, its two processes, and its place in `stage_info.map_gobjs`. `frame` is
/// where its call puts the scale joint's descriptor on the stack ([`SETUP_FRAME`]).
#[doc(alias = "Ground_GetStageGObj")]
pub fn stage_gobj(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
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
        let sj = scale_joint(game, scale(game), frame);
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
        let model = jobj::alloc(&mut game.objs);
        game.objs.jobjs[model].mtx = Mtx34::IDENTITY;
        game.objs.jobjs[model].scl = None;
        let sj = scale_joint(game, scale(game), frame - EMPTY_MAP_SLOT);
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

/// Creates a stage object from a joint descriptor, `Ground_801C1A20`: as
/// [`stage_gobj`] makes one, but its model from `joint` (its points of interest filled for
/// `map_id`, -1 for none), no set-apart joints, and no place in `stage_info.map_gobjs`.
/// `frame` is where its call puts the scale joint's descriptor on the stack.
#[doc(alias = "Ground_801C1A20")]
pub fn stage_gobj_from_joint(
    r: &mut Running,
    joint: DataRef,
    map_id: i32,
    frame: u32,
) -> Option<GObjId> {
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
        gp.x18 = None;
        gp.x10_flags.b3 = false;
        gp.x20 = [-1; 8];
    }
    material::reset_overlay(game, gobj);
    let model = jobj::load_joint(&mut game.objs, data, Some(joint));
    fill_points(game, data, map_id, model, Some(joint));
    let sj = scale_joint(game, scale(game), frame);
    jobj::add_next(&mut game.objs.jobjs, model, Some(sj));
    game.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(sj));
    game.world.setup_proc(gobj, CodeRef(0x801C_1CD0), 1);
    game.world.setup_proc(gobj, CodeRef(0x801C_1D38), 4);
    Some(gobj)
}

/// Creates a stage object and sets it up from its callbacks (each stage's copy of
/// `grLast_8021A7F4`): its render callback, destroy callback, setup and process.
pub fn create_from_callbacks(
    r: &mut Running,
    map_id: i32,
    cb: &StageCallbacks,
    frame: u32,
) -> Option<GObjId> {
    let gobj = stage_gobj(r, map_id, frame)?;
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
/// `Ground_801C20E0` (each entry with the flags its light has once the edit is done), or the
/// defaults (`Ground_803E06C8`).
fn light_list(game: &Game, data: &Archives) -> Option<Vec<(DataRef, u16)>> {
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
            return Some(light_overrides(data, head, lights));
        }
    }
    None
}

/// The lights fighters use, loaded (`lb_80011AC4` on `Ground_801C49B4`'s list): the stage's
/// own (`stage_info.map_plit`) as its light overrides leave them, or none, for the defaults.
#[doc(alias = "Ground_801C49B4")]
pub fn load_fighter_lights(game: &mut Game, data: &Archives) -> Option<Option<lobj::LObjId>> {
    let list = game.stage_info.map_plit?;
    let entries = overridden_lights(data, datfiles::main_head(game), list);
    let mut first = None;
    let mut prev: Option<lobj::LObjId> = None;
    for (entry, flags) in entries {
        let desc = data.ptr(entry).expect("a light list entry without a light");
        let l = lobj::alloc(&mut game.objs.lobjs);
        lobj::load_flags(&mut game.objs, data, l, desc, flags);
        if let Some(anims) = data.ptr(entry.at(4)) {
            lobj::add_anim_all(&mut game.objs, data, Some(l), data.ptr(anims));
        }
        match prev {
            Some(p) => game.objs.lobjs[p].next = Some(l),
            None => first = Some(l),
        }
        prev = Some(l);
    }
    Some(first)
}

/// The light list as `Ground_801C20E0` leaves it in the loaded data: when an override for
/// one of its lights sets a flag, every light but the ambient ones keeps only if an override
/// sets one of its flags, with its diffuse (4), specular (8) and 0x400 flags as the override
/// says. The original edits the list and descriptors in place, which leaves them the same
/// when it runs again; here each entry comes back with the flags its light has.
fn overridden_lights(data: &Archives, head: MapHead, lights: DataRef) -> Vec<(DataRef, u16)> {
    assert!(data.ptr(lights).is_some(), "ground.c: *lightset");
    let (list, n) = head.light_overrides(data);
    let find = |desc: Option<DataRef>| -> Option<u8> {
        (0..n.max(0) as u32)
            .map(|i| list.expect("light overrides").at(8 * i))
            .find(|&o| data.ptr(o) == desc)
            .map(|o| data.u8(o.at(4)) & 0xE0)
    };
    let mut entries = Vec::new();
    let mut at = lights;
    while let Some(entry) = data.ptr(at) {
        entries.push(entry);
        at = at.at(4);
    }
    let matched = entries
        .iter()
        .any(|&e| find(data.ptr(e)).is_some_and(|bits| bits != 0));
    let mut out = Vec::new();
    for e in entries {
        let desc = data.ptr(e).expect("a light list entry without a light");
        let mut flags = data.u16(desc.at(8));
        if matched && flags & 3 != 0 {
            match find(Some(desc)) {
                Some(bits) if bits != 0 => {
                    let set = |f: &mut u16, on: bool, bit: u16| {
                        if on {
                            *f |= bit;
                        } else {
                            *f &= !bit;
                        }
                    };
                    set(&mut flags, bits & 0x40 != 0, 4);
                    set(&mut flags, bits & 0x80 != 0, 8);
                    set(&mut flags, bits & 0x20 != 0, 0x400);
                }
                _ => continue,
            }
        }
        out.push((e, flags));
    }
    out
}

/// Applies the archive's light overrides to a light list, `Ground_801C20E0`: the list as it
/// is unless an override for one of its lights sets a flag; then the original edits the loaded
/// list and descriptors in place ([`overridden_lights`]), which here is the entries it leaves
/// and their lights' flags.
#[doc(alias = "Ground_801C20E0")]
fn light_overrides(data: &Archives, head: MapHead, lights: DataRef) -> Vec<(DataRef, u16)> {
    overridden_lights(data, head, lights)
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
    let Some(entries) = light_list(game, data) else {
        unimplemented!("the default lights (Ground_803E06C8) live in the executable");
    };
    let gobj = game.world.create(0xD, 3, 0);
    // `lb_80011AC4` on the list as the overrides leave it.
    let mut first = None;
    let mut prev: Option<lobj::LObjId> = None;
    for &(entry, flags) in &entries {
        let desc = data.ptr(entry).expect("a light list entry without a light");
        // `HSD_LObjLoadDesc` would load a chain of descriptors; the stages' have none.
        assert!(
            data.ptr(desc.at(4)).is_none(),
            "a stage light's descriptor with a next"
        );
        let l = lobj::alloc(&mut game.objs.lobjs);
        lobj::load_flags(&mut game.objs, data, l, desc, flags);
        if let Some(anims) = data.ptr(entry.at(4)) {
            lobj::add_anim_all(&mut game.objs, data, Some(l), data.ptr(anims));
        }
        match prev {
            Some(p) => game.objs.lobjs[p].next = Some(l),
            None => first = Some(l),
        }
        prev = Some(l);
    }
    let first = first.expect("ground.c: lobj");
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
    let first_entry = entries[0].0;
    if data.ptr(first_entry.at(4)).is_some() {
        let mut k = 0;
        let mut cur = Some(first);
        while let Some(l) = cur {
            let entry = entries.get(k).map(|e| e.0);
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
            k += 1;
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
pub(crate) fn light_anims(game: &Game, first: lobj::LObjId, _mask: u32) -> Vec<aobj::AObjId> {
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
#[doc(alias = "Stage_8022524C")]
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
    r.game.player_camera();
}

/// Runs the stage's load hook, `Ground_OnLoad`.
#[doc(alias = "Ground_OnLoad")]
#[doc(alias = "Stage_80225298")]
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
        crate::hsd::heap::mem_free(r.game.gr.start_hooks.addr(h.0));
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
    r.stop_object_sounds(Some(g));
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
    if let Some(root) = jobj {
        let objs = &r.game.objs;
        let geo = r.game.geometry.as_mut().expect("no collision geometry");
        for (id, joint) in geo.joints.iter_mut().enumerate() {
            if let Some(j) = joint.jobj
                && super::anime::root_of(objs, j) == root
            {
                r.game.gr.freed_coll_jobjs.retain(|&(k, _)| k != id as i32);
                r.game
                    .gr
                    .freed_coll_jobjs
                    .push((id as i32, objs.jobjs.pool.addr(j.0)));
                joint.jobj = None;
            }
        }
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

/// `St_Kind_Heal`, whose stage archive (none) comes from the preload.
const ST_KIND_HEAL: i32 = 0x55;

/// Picks the match's stage and loads it, `Stage_802251E8`: the stage kind's ground kind.
#[doc(alias = "Stage_802251E8")]
pub fn select(r: &mut Running, stkind: i32, archive: Option<ArchiveId>) {
    r.game.gr.stkind = stkind;
    let grkind = crate::lb::audio::ground_kind(stkind) as i32;
    load(r, grkind, stkind, archive);
}

/// Loads a stage, `Ground_801C0754`: `stage_info` reset, the stage's archive and what it
/// exports, the stage's item frequencies, its line and shadow callbacks, and the trophies'
/// bookkeeping.
#[doc(alias = "Ground_801C0754")]
pub fn load(r: &mut Running, grkind: i32, stkind: i32, archive: Option<ArchiveId>) {
    reset(r.game);
    r.game.stage_info.grkind = grkind;
    let stage = stage_data(grkind);
    load_dat(
        r,
        stage
            .file
            .map(|f| (f, archive.expect("the stage's archive"))),
        stkind != ST_KIND_HEAL,
    );
    item_frequencies(r.game, r.data, stkind);
    let code = |a: u32| (a != 0).then_some(CodeRef(a));
    r.game.stage_info.on_touch_line = code(stage.on_touch_line);
    r.game.stage_info.on_check_shadow_render = code(stage.on_check_shadow_render);
    // `Ground_801C5878`: `tyDisplay_8031C2CC`, then, outside the single-player modes, no
    // single-player display.
    r.load_trophy_data();
    if !super::super::gm::is_1p_mode(r.game) {
        r.game.stage_info.x6E4[0] = -1;
    }
}

/// Loads the stage's archive into the table's first free slot, `grDatFiles_801C6038` (with
/// its main stage's exports, as every stage's own load does): a disc load unless it was
/// preloaded, its `map_head`, `stage_info`'s pointers into it, its particle bank (64) and its
/// marked materials. No archive gives the defaults.
#[doc(alias = "grDatFiles_801C6038")]
pub fn load_dat(r: &mut Running, file: Option<(&str, ArchiveId)>, _from_disc: bool) {
    let slot = r
        .game
        .gr
        .archives
        .iter()
        .position(Option::is_none)
        .expect("the original's assertion failed: grdatfiles.c:229");
    let Some((name, archive)) = file else {
        unimplemented!("a stage without an archive (grDatFiles_803E0848's defaults)");
    };
    // `lbArchive_800171CC`: the preload holds no stage archive in a match, so it loads.
    let addr = r.disc_load(name, super::super::ft::data::MAIN_MEMORY);
    let data = r.data;
    let a = data.get(archive);
    let public = |n: &str| a.public(n).map(|offset| DataRef { archive, offset });
    let map_head = public("map_head");
    r.game.gr.archives[slot] = Some(super::StageArchive {
        archive,
        map_head,
        kind: 0,
        addr,
    });
    let i = &mut r.game.stage_info;
    i.coll_data = public("coll_data");
    i.param = public("grGroundParam");
    i.itemdata = public("itemdata");
    i.ald_yaku_all = public("ALDYakuAll");
    i.map_ptcl = public("map_ptcl");
    i.map_texg = public("map_texg");
    i.yakumono_param = public("yakumono_param");
    i.map_plit = public("map_plit");
    i.quake_model_set = public("quake_model_set");
    r.game.gr.param = i.param.map(|p| read_ground_param(data, p));
    if let (Some(ptcl), Some(texg)) = (i.map_ptcl, i.map_texg) {
        crate::hsd::particle::load_data_bank(&mut r.game.objs, data, 0x40, ptcl, texg, None, None);
    }
    mark_materials(r.game, data, map_head);
}

/// Marks the materials the `map_head` lists (`unk28`) `1 << 26`, `grDatFiles_801C6228`: in
/// the loaded descriptors, which [`crate::hsd::Objects::rendermode_patches`] holds.
#[doc(alias = "grDatFiles_801C6228")]
pub fn mark_materials(game: &mut Game, data: &Archives, map_head: Option<DataRef>) {
    let Some(h) = map_head else { return };
    let (Some(list), n) = (data.ptr(h.at(0x28)), data.u32(h.at(0x2C)) as i32) else {
        return;
    };
    for k in 0..n.max(0) as u32 {
        if let Some(d) = data.ptr(list.at(4 * k)) {
            *game.objs.rendermode_patches.entry(d).or_default() |= 1 << 26;
        }
    }
}

/// The stage's `grGroundParam`, copied out of its archive: its one pointer, the stage
/// parameters, points into the same archive.
fn read_ground_param(data: &Archives, at: DataRef) -> GroundParam {
    use super::super::mirror::{Cx, Mem, Mirror};
    struct Pointers(melee_data::archive::ArchiveId);
    impl Cx for Pointers {
        fn import_handle(&mut self, kind: &'static str, _: u32) -> Option<u32> {
            panic!("grGroundParam holds a {kind}")
        }
        fn import_data(&mut self, offset: u32) -> Option<DataRef> {
            (offset != 0).then_some(DataRef {
                archive: self.0,
                offset,
            })
        }
        fn compare_handle(&mut self, _: &str, _: &'static str, _: Option<u32>, _: u32) {
            unreachable!()
        }
        fn compare_data(&mut self, _: &str, _: Option<DataRef>, _: u32) {
            unreachable!()
        }
        fn mismatch(&mut self, _: &str, _: String, _: String) {
            unreachable!()
        }
    }
    let mem = Mem {
        bytes: data.get(at.archive).data(),
        base: 0,
    };
    GroundParam::import(&mem, at.offset, &mut Pointers(at.archive))
}

/// The stage's item frequencies for stage kind `stkind`, `Ground_801C28CC`: each of the
/// parameters' 35 rates times the stage's entry's, into `stage_info.xA0` on.
#[doc(alias = "Ground_801C28CC")]
pub fn item_frequencies(game: &mut Game, data: &Archives, stkind: i32) {
    let p = game.gr.param.as_ref().expect("stage_info.param is null");
    let param = game.stage_info.param.expect("stage_info.param is null");
    let entries = p.stage_params;
    for i in 0..p.stage_param_count.max(0) as u32 {
        let e = entries.expect("no stage parameters").at(0x64 * i);
        if data.u32(e) as i32 == stkind {
            for j in 0..35u32 {
                let a = data.u16(param.at(2 * (53 + j))) as i16;
                let b = data.u16(e.at(2 * (13 + j))) as i16;
                let v = i32::from(a) * i32::from(b);
                let si = &mut game.stage_info;
                if j == 0 {
                    si.xA0 = v;
                } else {
                    let at = 4 * (j as usize - 1);
                    si.xA4_pad[at..at + 4].copy_from_slice(&v.to_be_bytes());
                }
            }
            return;
        }
    }
    panic!(
        "stage {stkind} has no stage parameters among {}",
        p.stage_param_count
    );
}

/// `CKind_Mars` and `CKind_CLink`, whose unlocking lets two stages play their alternate songs.
const CKIND_MARS: u8 = 0x09;
const CKIND_CLINK: u8 = 0x15;

/// `BGM_Undefined`.
const BGM_UNDEFINED: i32 = -1;

/// The stage's music by `flags`, `Ground_801C28AC` (`Ground_801C24F8`): the stage kind's
/// parameters pick its usual or its alternate song (bits 1 and 2; with bit 4 they choose, the
/// alternate by chance or unlock, bit 0x40 forcing it), from its sudden death's (bit 0x10),
/// its single player's (0x20) or its own; and whether the alternate plays.
#[doc(alias = "Ground_801C28AC")]
#[doc(alias = "Ground_801C24F8")]
pub fn music(game: &mut Game, data: &Archives, stkind: i32, mut flags: u32) -> (i32, bool) {
    let p = game.gr.param.as_ref().expect("stage_info.param is null");
    let (entries, count) = (p.stage_params, p.stage_param_count);
    let (mut bgm, mut alternate) = (BGM_UNDEFINED, false);
    for i in 0..count.max(0) as u32 {
        let e = entries.expect("no stage parameters").at(0x64 * i);
        if data.u32(e) as i32 != stkind {
            continue;
        }
        let w = |o: u32| data.u32(e.at(o));
        let chance = i32::from(data.u16(e.at(0x16)) as i16);
        if flags & 4 != 0 {
            let forced = flags & 0x40 != 0;
            let roll = |game: &mut Game, unlocked: bool| {
                unlocked && (chance > game.objs.rng.below(100) || forced)
            };
            let alt = match data.u16(e.at(0x14)) as i16 {
                0 => Some(forced),
                1 => Some(game.objs.rng.below(0xC) == 0 || forced),
                8 => Some(false),
                2 => Some(roll(game, true)),
                3 => {
                    let unlocked = game.gm.unlocks.ckind_unlocked(CKIND_MARS);
                    Some(roll(game, unlocked))
                }
                4 => {
                    let unlocked = game.gm.unlocks.ckind_unlocked(CKIND_CLINK);
                    Some(roll(game, unlocked))
                }
                5 => unported_call("Toy_803048C0(0x11A)"),
                6 => {
                    let all = game.gm.unlocks.all_characters();
                    Some(roll(game, all))
                }
                7 => {
                    let all = game.gm.unlocks.all_stages();
                    Some(roll(game, all))
                }
                _ => None,
            };
            match alt {
                Some(true) => flags |= 2,
                Some(false) => flags |= 1,
                None => {}
            }
        }
        let si = &mut game.stage_info;
        let pick = |usual: u32, alt: u32| -> (i32, bool) {
            if alt != BGM_UNDEFINED as u32 {
                (alt as i32, true)
            } else {
                (usual as i32, false)
            }
        };
        if flags & 0x10 != 0 {
            si.unk8C.b0 = false;
            if flags & 1 != 0 {
                bgm = w(0xC) as i32;
            } else if flags & 2 != 0 {
                (bgm, alternate) = pick(w(0xC), w(0x10));
            }
        } else if flags & 0x20 != 0 {
            if flags & 1 != 0 {
                bgm = w(0xC) as i32;
                si.unk8C.b0 = false;
            } else if flags & 2 != 0 {
                (bgm, alternate) = pick(w(0xC), w(0x10));
                si.unk8C.b0 = alternate;
            }
        } else if flags & 1 != 0 {
            bgm = w(4) as i32;
            si.unk8C.b0 = false;
        } else if flags & 2 != 0 {
            (bgm, alternate) = pick(w(4), w(8));
            si.unk8C.b0 = alternate;
        }
        break;
    }
    assert!(
        bgm != BGM_UNDEFINED,
        "the original's assertion failed: bgm!=BGM_Undefined"
    );
    if bgm == -2 {
        unported_call("lbAudioAx_8002305C");
    }
    (bgm, alternate)
}
