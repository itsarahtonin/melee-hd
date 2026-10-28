//! Rainbow Cruise, `melee/gr/grrcruise.c`: the ship (map 1) with its planks that drop when
//! stood on and its parts that appear as the camera reaches them, the deck that tilts toward
//! the side its riders stand on, the falling blocks (map 4), the flag on its dynamics chain
//! (map 6), the course (map 3, twice: a hidden copy whose animation leads, and the one shown,
//! which moves the stage under the camera), and the scenery (maps 0, 2 and 5).
//!
//! The ship steers a wind source that sways the flag, and the course's last step is the wind
//! the stage blows on airborne items (`grRCruise_80201918`, through `grLib_801C9E60`).

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::super::camera;
use super::super::handles::{CodeRef, GroundId};
use super::super::state::{Game, Running};
use super::super::structs::{CollData, Ground};
use super::data::{RCRUISE_CALLBACKS, StageCallbacks};
use super::ground::{ground_of, jobj_of, map_gobj, nth_joint, update_coll_joint, update_map_coll};
use super::{CollUser, anime, setup, stage, zako};
use crate::fpu::{mem, reg};
use crate::hsd::dobj;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId, JObjs};
use crate::hsd::pool::Pool;
use crate::lb::joint::world_position;
use crate::vec::Vec3;

/// `fn_80200460`: a block's collision joint touched.
pub const BLOCK_TOUCHED: u32 = 0x8020_0460;
/// `grRCruise_80200578`: the deck's collision joint touched.
pub const DECK_TOUCHED: u32 = 0x8020_0578;
/// `grRCruise_802010A4`: a plank's collision joint touched.
pub const PLANK_TOUCHED: u32 = 0x8020_10A4;
/// `fn_80201BE0`, the course's render callback.
pub const RENDER_COURSE: u32 = 0x8020_1BE0;
/// `grRCruise_801FF444`, the ship's start hook.
const START_HOOK: u32 = 0x801F_F444;

/// `DOBJ_HIDDEN`.
const DOBJ_HIDDEN: u32 = 1;

/// The blocks' parts (`grRc_804D4790`).
const BLOCKS: [i32; 3] = [0x01, 0x1A, 0x33];

/// The planks' parts (`grRc_803E4FF0`).
const PLANKS: [i32; 17] = [
    0x20, 0x29, 0x2A, 0x2B, 0x2C, 0x2D, 0x2E, 0x2F, 0x30, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27,
    0x28,
];

/// The parts that appear with the camera (`lbl_803E5014`): the part, its collision joint, and
/// whether it is there from the start.
const VANISH: [(i32, i32, bool); 20] = [
    (6, 4, false),
    (8, 5, false),
    (16, 10, false),
    (11, 6, false),
    (12, 7, false),
    (13, 8, false),
    (14, 9, false),
    (15, 8, false),
    (19, 12, true),
    (20, 13, true),
    (21, 14, true),
    (30, 15, true),
    (22, 16, true),
    (23, 17, true),
    (24, 18, true),
    (25, 19, true),
    (26, 20, true),
    (27, 21, true),
    (28, 22, true),
    (29, 23, true),
];

/// The joints the ship's start hook gives its callbacks: the deck's, then the planks'.
const DECK_JOINT: i32 = 5;
const PLANK_JOINTS: [i32; 17] = [
    27, 36, 37, 38, 39, 40, 41, 42, 43, 28, 29, 30, 31, 32, 33, 34, 35,
];

/// `grRCruise_YakumonoParam`'s fields, by offset.
mod p {
    pub const TILT_RATE: u32 = 0x0;
    pub const TILT_MAX: u32 = 0x4;
    pub const TILT_BACK: u32 = 0x8;
    pub const BLOCK_WAIT: u32 = 0xC;
    pub const BLOCK_GONE: u32 = 0x10;
    pub const BLOCK_BLINK: u32 = 0x14;
    pub const BLOCK_BACK: u32 = 0x18;
    pub const BLOCK_BACK_BLINK: u32 = 0x1C;
    pub const PLANK_WAIT: u32 = 0x20;
    pub const PLANK_SHAKE: u32 = 0x24;
    pub const PLANK_SHAKE_EVERY: u32 = 0x28;
    pub const PLANK_SHAKE_MIN: u32 = 0x2C;
    pub const PLANK_SHAKE_MAX: u32 = 0x30;
    pub const PLANK_GRAVITY: u32 = 0x34;
    pub const PLANK_FALL_MAX: u32 = 0x38;
    pub const PLANK_GONE: u32 = 0x3C;
    pub const PLANK_BACK: u32 = 0x40;
    pub const PLANK_BLINK: u32 = 0x44;
}

/// The fields of `grRCruise_YakumonoParam` the code reads, by offset.
#[derive(Clone, Copy)]
struct Param(DataRef);

impl Param {
    fn f(self, data: &Archives, off: u32) -> f32 {
        data.f32(self.0.at(off))
    }
    fn i(self, data: &Archives, off: u32) -> i32 {
        data.u32(self.0.at(off)) as i32
    }
}

fn param(game: &Game) -> Param {
    Param(
        game.gr
            .rcruise_yakumono
            .expect("grrcruise.c: yakumono_param"),
    )
}

/// A stage object's process or callback, by address: Rainbow Cruise's.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x801F_F3E8 | 0x801F_F3EC | 0x801F_F43C | 0x801F_F440 | 0x801F_F738 | 0x801F_F79C
        | 0x801F_F7A0 | 0x801F_F920 | 0x8020_0070 | 0x8020_045C => empty(r, gobj),
        0x801F_F3B4 => init_back(r, gobj),
        0x801F_F3F0 => init_far(r, gobj),
        START_HOOK => on_ship_start(r, gobj),
        0x801F_F5B4 => init_ship(r, gobj),
        0x801F_F6D4 => proc_ship(r, gobj),
        0x801F_F73C => init_ship_back(r, gobj),
        0x801F_F7A4 => init_flag(r, gobj),
        0x801F_F8E4 => proc_flag(r, gobj),
        0x801F_F924 => init_course(r, gobj),
        0x801F_FADC => proc_course(r, gobj),
        0x8020_0074 => init_blocks(r, gobj),
        0x8020_0154 => proc_blocks(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x801F_F168 => on_init(r),
        0x801F_F298 => on_load(r),
        0x801F_F29C => on_start(r),
        _ => return false,
    }
    true
}

/// The stage's render callbacks, by address.
pub fn run_render(r: &mut Running, gobj: GObjId, cb: u32, pass: i32) -> bool {
    match cb {
        RENDER_COURSE => render_course(r, gobj, pass),
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing.
#[doc(alias = "grRCruise_801FF3E8")]
#[doc(alias = "grRCruise_801FF3EC")]
#[doc(alias = "grRCruise_801FF43C")]
#[doc(alias = "grRCruise_801FF440")]
#[doc(alias = "grRCruise_801FF738")]
#[doc(alias = "grRCruise_801FF79C")]
#[doc(alias = "grRCruise_801FF7A0")]
#[doc(alias = "grRCruise_801FF920")]
#[doc(alias = "grRCruise_80200070")]
#[doc(alias = "grRCruise_8020045C")]
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// Creates one of the stage's objects and sets it up, `grRCruise_801FF2C8`.
#[doc(alias = "grRCruise_801FF2C8")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

/// The stage's objects' setup functions and processes, by map id (`grRc_StageCallbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &RCRUISE_CALLBACKS[map_id as usize]
}

/// Rainbow Cruise's setup, `grRCruise_801FF168`: its parameters; the course twice, the first
/// copy frozen and hidden and leading the second; the scenery, the ship, the flag and the
/// blocks; camera limits and blast zones; the course's first step; the ship's, flag's and
/// blocks' collision where their models stand; and joint 11 out of the search.
#[doc(alias = "grRCruise_801FF168")]
pub fn on_init(r: &mut Running) {
    r.game.gr.rcruise_yakumono = super::ground::yakumono_param(r.game);
    r.game.stage_info.unk8C.b4 = true;
    r.game.stage_info.unk8C.b5 = false;
    let lead = create(r, 3, setup::SETUP_FRAME).expect("grrcruise.c: the course");
    let g = ground_of(r.game, lead);
    r.game.grounds[g.0].u.scroll().xC4 |= 0x80;
    let j = jobj_of(r.game, lead);
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    let course = create(r, 3, setup::SETUP_FRAME).expect("grrcruise.c: the course");
    let g = ground_of(r.game, course);
    r.game.grounds[g.0].u.scroll().anim_gobj = Some(lead);
    create(r, 0, setup::SETUP_FRAME);
    create(r, 2, setup::SETUP_FRAME);
    create(r, 5, setup::SETUP_FRAME);
    let ship = create(r, 1, setup::SETUP_FRAME).expect("grrcruise.c: the ship");
    let flag = create(r, 6, setup::SETUP_FRAME).expect("grrcruise.c: the flag");
    let blocks = create(r, 4, setup::SETUP_FRAME).expect("grrcruise.c: the blocks");
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
    proc_course(r, course);
    for (gobj, map_id) in [(ship, 1), (flag, 6), (blocks, 4)] {
        update_map_coll(r.game, r.data, gobj);
        setup::settle_map_coll(r.game, r.data, map_id);
    }
    r.game.geometry_mut().disable_joint(11);
}

/// `grRCruise_801FF164` does nothing (only a demo runs it).
#[doc(alias = "grRCruise_801FF164")]
pub fn on_demo_init(_r: &mut Running) {}

/// `grRCruise_801FF298` does nothing.
#[doc(alias = "grRCruise_801FF298")]
pub fn on_load(_r: &mut Running) {}

/// Rainbow Cruise's start, `grRCruise_801FF29C`: the enemy spawner, without enemies.
#[doc(alias = "grRCruise_801FF29C")]
pub fn on_start(r: &mut Running) {
    zako::start(r.game, None);
}

fn map_id(game: &Game, gobj: GObjId) -> i32 {
    game.grounds[ground_of(game, gobj).0].map_id
}

/// The background's setup (map 0), `grRCruise_801FF3B4`: its animation.
#[doc(alias = "grRCruise_801FF3B4")]
pub fn init_back(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// The far scenery's setup (map 2), `grRCruise_801FF3F0`: its animation, drawn behind.
#[doc(alias = "grRCruise_801FF3F0")]
pub fn init_far(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    r.game.grounds[g.0].x11_flags.b012 = 2;
}

/// The ship's start hook, `grRCruise_801FF444`: the deck's and planks' collision joints tell
/// the ship when they are touched, and the ship starts moving its parts.
#[doc(alias = "grRCruise_801FF444")]
pub fn on_ship_start(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let geo = r.game.geometry_mut();
    geo.set_ledge_callback(
        DECK_JOINT,
        Some(CollUser::GObj(gobj)),
        Some(CodeRef(DECK_TOUCHED)),
    );
    for j in PLANK_JOINTS {
        geo.set_ledge_callback(j, Some(CollUser::GObj(gobj)), Some(CodeRef(PLANK_TOUCHED)));
    }
    r.game.grounds[g.0].u.rcruise().x10 = 0;
}

/// What a block new from the heap at `addr` held, in a session whose heap's image knows it
/// (docs/engine.md, a session's heap leftovers); none outside one, where it holds zero bytes.
fn leftover_bytes(r: &Running, addr: u32, len: u32) -> Option<Vec<u8>> {
    crate::hsd::heap::image_on()
        .then(|| crate::hsd::heap::with(|h| h.image.leftover_bytes(r.data, addr, len).0))
        .flatten()
}

/// The ship's setup (map 1), `grRCruise_801FF5B4`: its collision and animation, held until the
/// match starts; its planks' block from the heap; the parts that appear with the camera; its
/// start hook; the tilt at rest; the planks; its collision where its model stands, every joint
/// linked; and a wind source it steers, 60 degrees a frame of waving, with a step of a degree
/// and gusts on.
#[doc(alias = "grRCruise_801FF5B4")]
pub fn init_ship(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    anime::start_map(r.game, r.data, gobj, m, 0);
    r.game.grounds[g.0].u.rcruise().x10 = 1;
    // The planks' block (`Map_Chikuwa`, 0x198): the fields their setup never writes (`x1`,
    // `x10`) hold the block's leftovers (docs/engine.md, a session's heap leftovers), else zero.
    let addr = crate::hsd::heap::mem_alloc(0x198);
    let left = leftover_bytes(r, addr, 0x198);
    let v = r.game.grounds[g.0].u.rcruise();
    v.entries_addr = addr;
    v.entries = (0..PLANKS.len())
        .map(|i| {
            let b = |o: usize| left.as_ref().map_or(0, |b| b[0x18 * i + o]);
            super::vars::RCruisePlank {
                x1: b(1),
                x10: f32::from_bits(u32::from_be_bytes([b(0x10), b(0x11), b(0x12), b(0x13)])),
                ..Default::default()
            }
        })
        .collect();
    init_vanish(r, gobj);
    setup::add_start_hook(r.game, gobj, CodeRef(START_HOOK));
    reset_tilt(r, gobj);
    init_planks(r, gobj);
    update_map_coll(r.game, r.data, gobj);
    r.game.geometry_mut().link_all_joints();
    let step = mem(fp::fmuls(fp::frsp(60.0), reg(0.017_453_292_f32)));
    let w = crate::lb::wind::push(
        &mut r.game.gr.winds,
        Vec3::new(1.0, 0.0, 0.0),
        -1,
        0.4,
        0.0,
        step,
        -10000.0,
        10000.0,
        10000.0,
        -10000.0,
    );
    let v = r.game.grounds[g.0].u.rcruise();
    v.x4 = w;
    v.x8 = 0.017_453_292;
    v.xC = 1;
}

/// The ship's process, `grRCruise_801FF6D4`: once the match has started, the parts that appear
/// with the camera, the deck's tilt and the planks; then the wind sources age, the ship steers
/// its wind, and the collision follows the model.
#[doc(alias = "grRCruise_801FF6D4")]
pub fn proc_ship(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    if r.game.grounds[g.0].u.rcruise().x10 == 0 {
        update_vanish(r, gobj);
        tilt(r, gobj);
        update_planks(r, gobj);
    }
    let level = r.game.gr.wind_level;
    r.game.gr.wind_level = crate::lb::wind::tick(&mut r.game.gr.winds, level);
    steer_wind(r, gobj);
    update_map_coll(r.game, r.data, gobj);
}

/// The ship's background's setup (map 5), `grRCruise_801FF73C`: its animation, hidden but for
/// part 31.
#[doc(alias = "grRCruise_801FF73C")]
pub fn init_ship_back(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    let j = jobj_of(r.game, gobj);
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    let part = nth_joint(r.game, gobj, 31);
    jobj::clear_flags_all(&mut r.game.objs.jobjs, part, jobj::HIDDEN);
}

/// The flag's setup (map 6), `grRCruise_801FF7A4`: its collision and animation, looping; its
/// dynamics chain from the archive's `dynamicsdata_shipflag` on part 23; and parts 10 to 13
/// as the stage's points 0 to 3.
#[doc(alias = "grRCruise_801FF7A4")]
pub fn init_flag(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    anime::start_map(r.game, r.data, gobj, m, 0);
    anime::apply(
        r.game,
        j,
        true,
        0x77A4,
        anime::AnimOp::SetFlags(crate::hsd::aobj::LOOP),
    );
    let archive = super::datfiles::main_archive(r.game).archive;
    match r.data.get(archive).public("dynamicsdata_shipflag") {
        Some(offset) => {
            let src = DataRef { archive, offset };
            let root = nth_joint(r.game, gobj, 23);
            chain(r, root, src, g);
        }
        None => r.game.grounds[g.0].u.rcruise_flag().xC4.data = None,
    }
    for k in 0..4 {
        let part = nth_joint(r.game, gobj, 10 + k);
        if part.is_some() {
            super::ground::set_point_joint(r.game, k, part);
        }
    }
}

/// Builds a stage object's dynamics chain down `root` from a description, `grLib_801C9B20`: as
/// many links as it has (`lb_8000FD48`), and their parameters (`lb_80011710`).
#[doc(alias = "grLib_801C9B20")]
pub fn chain(r: &mut Running, root: Option<JObjId>, src: DataRef, g: GroundId) {
    let n = r.data.u32(src.at(4));
    let mut desc = r.game.grounds[g.0].u.rcruise_flag().xC4.clone();
    r.game.load_dynamics_chain(r.data, root, &mut desc, n);
    r.game.set_dynamics_params(r.data, src, &mut desc);
    r.game.grounds[g.0].u.rcruise_flag().xC4 = desc;
}

/// The flag's process, `grRCruise_801FF8E4`: its chain swings and the collision follows the
/// model.
#[doc(alias = "grRCruise_801FF8E4")]
pub fn proc_flag(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    swing(r, g);
    update_map_coll(r.game, r.data, gobj);
}

/// Swings a stage object's dynamics chain a frame, `grLib_801C9B8C` (`lb_8001044C` with
/// nothing to collide with and no floor).
#[doc(alias = "grLib_801C9B8C")]
pub fn swing(r: &mut Running, g: GroundId) {
    let desc = r.game.grounds[g.0].u.rcruise_flag().xC4.clone();
    r.game.swing_chain(
        r.data,
        &desc,
        &[],
        None,
        crate::game::ft::dynamics::BoneSlot::Id(0),
        0,
        false,
    );
}

/// The course's setup (map 3), `grRCruise_801FF924`: the joints the course reads, the center
/// joint's place, no offset or step yet, not frozen, its animation with its loop flag, and its
/// render callback.
#[doc(alias = "grRCruise_801FF924")]
pub fn init_course(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let part = |r: &Running, n| nth_joint(r.game, gobj, n);
    let (x38, x44) = ([part(r, 4), part(r, 5), part(r, 6)], part(r, 7));
    let int_jobj = part(r, 3);
    assert!(int_jobj.is_some(), "grrcruise.c: gp->u.scroll.int_jobj");
    let cam_jobj = part(r, 2);
    assert!(cam_jobj.is_some(), "grrcruise.c: gp->u.scroll.cam_jobj");
    let ctr_jobj = part(r, 1);
    let ctr = ctr_jobj.expect("grrcruise.c: gp->u.scroll.ctr_jobj");
    let x4 = r.game.objs.jobjs[ctr].translate;
    let v = r.game.grounds[g.0].u.scroll();
    v.x38 = x38;
    v.x44 = x44;
    v.int_jobj = int_jobj;
    v.cam_jobj = cam_jobj;
    v.ctr_jobj = ctr_jobj;
    v.x4 = x4;
    v.x10 = Vec3::ZERO;
    v.x1C = Vec3::ZERO;
    v.xC4 &= !0x80;
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    anime::apply(
        r.game,
        j,
        true,
        0x77A4,
        anime::AnimOp::SetFlags(0x2000_0000),
    );
    r.game.world.gobj_mut(gobj).render = Some(CodeRef(RENDER_COURSE));
}

/// The course's process, `grRCruise_801FFADC`, unless frozen: where the leading copy's camera
/// joint looks through its interest joint onto the stage's plane, less the camera limits'
/// center and 10 up, is the course's offset; its step from the last is kept (the wind); the
/// course's model, the ship, its background, the flag, the blocks and the scenery move by the
/// offset's opposite from the center, the course's model 350 back (times the stage's scale).
#[doc(alias = "grRCruise_801FFADC")]
pub fn proc_course(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    if r.game.grounds[g.0].u.scroll().xC4 & 0x80 != 0 {
        return;
    }
    let cam_offset = stage::cam_center(r.game);
    let lead = r.game.grounds[g.0].u.scroll().anim_gobj;
    let lead = lead.expect("grrcruise.c: gp->u.scroll.anim_gobj");
    let lg = ground_of(r.game, lead);
    let lv = r.game.grounds[lg.0].u.scroll();
    let (int_jobj, cam_jobj) = (lv.int_jobj, lv.cam_jobj);
    let center = stage::cam_center(r.game);
    let eye = world_position(&mut r.game.objs, r.data, cam_jobj, None);
    let at = world_position(&mut r.game.objs, r.data, int_jobj, None);
    let diff = at - eye;
    let t = fp::fdivs(reg(eye.z), reg(diff.z));
    let mut pos = Vec3::new(
        mem(fp::fnmsubs(reg(diff.x), t, reg(eye.x))),
        mem(fp::fnmsubs(reg(diff.y), t, reg(eye.y))),
        0.0,
    );
    pos = pos - center;
    pos.y = mem(fp::fadds(reg(pos.y), 10.0));
    let v = r.game.grounds[g.0].u.scroll();
    v.x1C = v.x10 - pos;
    v.x10 = pos;
    let neg = |c: f32| mem(fp::fmuls(reg(c), -1.0));
    let mut pos = Vec3::new(neg(pos.x), neg(pos.y), neg(pos.z));
    pos = pos + cam_offset;
    if let Some(j) = jobj_of(r.game, gobj) {
        jobj::set_translate(&mut r.game.objs.jobjs, j, pos);
        let back = fp::fmuls(-350.0, reg(super::ground::scale(r.game)));
        jobj::set_srt(&mut r.game.objs.jobjs, j, |o| {
            o.translate.z = mem(fp::fadds(reg(o.translate.z), back))
        });
    }
    for m in [2, 1, 5, 6, 4] {
        if let Some(o) = map_gobj(r.game, m)
            && let Some(j) = jobj_of(r.game, o)
        {
            jobj::set_translate(&mut r.game.objs.jobjs, j, pos);
        }
    }
}

/// The blocks' setup (map 4), `grRCruise_80200074`: their collision and animation; each block
/// stopped at its first frame, its collision joint telling the blocks when it is touched, and
/// in its first state.
#[doc(alias = "grRCruise_80200074")]
pub fn init_blocks(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    anime::start_map(r.game, r.data, gobj, m, 0);
    for (i, &part) in BLOCKS.iter().enumerate() {
        anime::start(r.game, r.data, gobj, part, 7, 1, 0.0, 0.0, false);
        let joint = setup::coll_joint_id(r.game, r.data, m, part);
        r.game.grounds[g.0].u.rcruise_blocks().x3C[i].x2 = joint as i16;
        r.game.geometry_mut().set_ledge_callback(
            joint as i16 as i32,
            Some(CollUser::GObj(gobj)),
            Some(CodeRef(BLOCK_TOUCHED)),
        );
        let model = nth_joint(r.game, gobj, part);
        let b = &mut r.game.grounds[g.0].u.rcruise_blocks().x3C[i];
        b.xC = model;
        b.x8 = 0;
        b.x4 = 0;
        b.x0 = 0;
    }
}

/// Hides a joint's display objects, or shows them (`clear`), `grRCruise_80201B60`.
#[doc(alias = "grRCruise_80201B60")]
pub fn show_dobjs(objs: &mut crate::hsd::Objects, j: Option<JObjId>, clear: bool) {
    let mut d = jobj::get_dobj(&objs.jobjs, j);
    while let Some(x) = d {
        if clear {
            dobj::clear_flags(&mut objs.dobjs, Some(x), DOBJ_HIDDEN);
        } else {
            dobj::set_flags(&mut objs.dobjs, Some(x), DOBJ_HIDDEN);
        }
        d = objs.dobjs[x].next;
    }
}

/// A block's model: its joint's first child.
fn block_model(jobjs: &JObjs, b: Option<JObjId>) -> Option<JObjId> {
    jobjs[b.expect("a block's joint")].child
}

/// Takes a block's collision out of the search and puts it back as it stands, hidden, its
/// animation back at its first frame and stopped.
fn block_reset(r: &mut Running, gobj: GObjId, g: GroundId, i: usize) {
    let b = &r.game.grounds[g.0].u.rcruise_blocks().x3C[i];
    let (joint, model) = (i32::from(b.x2), b.xC);
    r.game.geometry_mut().disable_joint(joint);
    let child = block_model(&r.game.objs.jobjs, model);
    show_dobjs(&mut r.game.objs, child, false);
    anime::req_anim(r.game, gobj, BLOCKS[i], 1, 0.0, false);
    anime::set_rate(r.game, gobj, BLOCKS[i], 1, 0.0, false);
    update_coll_joint(r.game, r.data, joint);
    r.game.geometry_mut().settle_vertices(joint);
    r.game.grounds[g.0].u.rcruise_blocks().x3C[i].x0 = 0;
}

/// The blocks' process, `grRCruise_80200154`. A block comes in (state 0: its collision in the
/// search, shown) and waits (1). Stood on, its animation plays (2); left before it ends, it
/// stops and waits (3), then blinks (4) and goes; played to the end, it blinks out (5). Gone,
/// it comes in again. Then the collision follows the model.
#[doc(alias = "grRCruise_80200154")]
pub fn proc_blocks(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let p = param(r.game);
    for (i, &part) in BLOCKS.iter().enumerate() {
        let b = &r.game.grounds[g.0].u.rcruise_blocks().x3C[i];
        let (state, joint, model) = (b.x0, i32::from(b.x2), b.xC);
        match state {
            0 => {
                r.game.geometry_mut().enable_joint(joint);
                let child = block_model(&r.game.objs.jobjs, model);
                show_dobjs(&mut r.game.objs, child, true);
                let b = &mut r.game.grounds[g.0].u.rcruise_blocks().x3C[i];
                b.x4 = 0;
                b.x0 = 1;
            }
            2 => {
                if r.game.grounds[g.0].u.rcruise_blocks().x3C[i].x8 == 0 {
                    r.game.grounds[g.0].u.rcruise_blocks().x3C[i].x4 = 0;
                    anime::set_rate(r.game, gobj, part, 1, 0.0, false);
                    r.game.grounds[g.0].u.rcruise_blocks().x3C[i].x0 = 3;
                } else if anime::finished(r.game, gobj, part, 7) {
                    let b = &mut r.game.grounds[g.0].u.rcruise_blocks().x3C[i];
                    b.x4 = 0;
                    b.x0 = 5;
                }
            }
            3 => {
                let wait = p.i(r.data, p::BLOCK_WAIT);
                let b = &mut r.game.grounds[g.0].u.rcruise_blocks().x3C[i];
                if b.x4 >= wait {
                    b.x4 = 0;
                    b.x1 &= !0x80;
                    b.x0 = 4;
                }
                b.x4 = b.x4.wrapping_add(1);
            }
            4 => {
                let (blink, gone) = (p.i(r.data, p::BLOCK_BLINK), p.i(r.data, p::BLOCK_GONE));
                let b = &mut r.game.grounds[g.0].u.rcruise_blocks().x3C[i];
                if b.x4.wrapping_rem(blink) == 0 {
                    b.x1 ^= 0x80;
                    let hide = b.x1 & 0x80 != 0;
                    let child = block_model(&r.game.objs.jobjs, model);
                    show_dobjs(&mut r.game.objs, child, !hide);
                }
                if r.game.grounds[g.0].u.rcruise_blocks().x3C[i].x4 >= gone {
                    block_reset(r, gobj, g, i);
                }
                let b = &mut r.game.grounds[g.0].u.rcruise_blocks().x3C[i];
                b.x4 = b.x4.wrapping_add(1);
            }
            5 => {
                let (blink, back) = (p.i(r.data, p::BLOCK_BACK_BLINK), p.i(r.data, p::BLOCK_BACK));
                let n = r.game.grounds[g.0].u.rcruise_blocks().x3C[i].x4;
                let child = block_model(&r.game.objs.jobjs, model);
                show_dobjs(&mut r.game.objs, child, n.wrapping_rem(blink) != 0);
                if n >= back {
                    block_reset(r, gobj, g, i);
                }
                let b = &mut r.game.grounds[g.0].u.rcruise_blocks().x3C[i];
                b.x4 = b.x4.wrapping_add(1);
            }
            _ => {}
        }
        r.game.grounds[g.0].u.rcruise_blocks().x3C[i].x8 = 0;
    }
    update_map_coll(r.game, r.data, gobj);
}

/// A block's collision joint touched, `fn_80200460`: a fighter (collision kind 1) on a block
/// that waits, stopped or blinking starts it playing, shown; every touch counts.
#[doc(alias = "fn_80200460")]
pub fn block_touched(
    objs: &mut crate::hsd::Objects,
    grounds: &mut Pool<Ground>,
    root: Option<JObjId>,
    g: GroundId,
    joint_id: i32,
    kind: u32,
) {
    if kind != 1 {
        return;
    }
    for (i, &part) in BLOCKS.iter().enumerate() {
        let b = &grounds[g.0].u.rcruise_blocks().x3C[i];
        if i32::from(b.x2) != joint_id {
            continue;
        }
        if matches!(b.x0, 1 | 3 | 4) {
            let model = b.xC;
            grounds[g.0].u.rcruise_blocks().x3C[i].x4 = 0;
            let child = block_model(&objs.jobjs, model);
            show_dobjs(objs, child, true);
            anime::set_rate_below(objs, root, part, 1, 1.0, false);
            grounds[g.0].u.rcruise_blocks().x3C[i].x0 = 2;
        }
        let b = &mut grounds[g.0].u.rcruise_blocks().x3C[i];
        b.x8 = b.x8.wrapping_add(1);
        break;
    }
}

/// The deck's tilt at rest, `grRCruise_80200540`.
#[doc(alias = "grRCruise_80200540")]
pub fn reset_tilt(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.rcruise();
    v.x18 = 0.0;
    v.x14 = 0.0;
    v.x1C = 0.0;
    v.x20 = 0.0;
    v.x34 = 0;
    v.x30 = 0;
    v.x28 = 0.0;
    v.x24 = 0.0;
    v.x38 = 0;
    v.x2C = 0;
}

/// The deck's collision joint touched, `grRCruise_80200578`: something standing on it (kinds
/// 1 to 3) more than 4 away from the deck's pivot (part 8) adds its distance, weighted by its
/// `x50` (at most 1000) over 1000, to its side's pull; every touch counts.
#[doc(alias = "grRCruise_80200578")]
pub fn deck_touched(
    objs: &mut crate::hsd::Objects,
    data: &Archives,
    grounds: &mut Pool<Ground>,
    root: Option<JObjId>,
    g: GroundId,
    coll: &CollData,
    x50: i32,
) {
    let kind = u32::from(coll.x34_flags.b1234);
    if kind != 1 && kind != 2 && kind != 3 {
        return;
    }
    let mut x50 = x50;
    if fp::frsp(f64::from(x50)) > 1000.0 {
        x50 = 1000;
    }
    let pivot = root.and_then(|r| super::ground::nth_joint_below(&objs.jobjs, r, 8));
    let pos = world_position(objs, data, pivot, None);
    let dx = fp::fsubs(reg(pos.x), reg(coll.cur_pos.x));
    let dy = fp::fsubs(reg(pos.y), reg(coll.cur_pos.y));
    let dist = crate::math::sqrt_reg(fp::fmadds(dy, dy, fp::fmuls(dx, dx)));
    let v = grounds[g.0].u.rcruise();
    if dist > 4.0 {
        let w = fp::fdivs(fp::frsp(f64::from(x50)), 1000.0);
        if reg(pos.x) < reg(coll.cur_pos.x) {
            v.x24 = mem(fp::fmadds(dist, w, reg(v.x24)));
        } else {
            v.x28 = mem(fp::fmadds(dist, w, reg(v.x28)));
        }
    }
    v.x34 = v.x34.wrapping_add(1);
}

/// The deck's tilt, `grRCruise_8020071C`. With nothing on it, it swings back level at the
/// parameters' rate until within 0.2 degrees; with riders it tilts toward the heavier side, in
/// proportion to the difference, at most the parameters' maximum. (State 1, which nothing
/// sets, eases a swing out first.) The deck's pivot (part 8) and its background's turn by the
/// tilt, and this tick's pulls start over.
#[doc(alias = "grRCruise_8020071C")]
pub fn tilt(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let p = param(r.game);
    let pivot = nth_joint(r.game, gobj, 8);
    let back = map_gobj(r.game, 5).expect("Ground_GetMapGObj(5)");
    let pivot5 = nth_joint(r.game, back, 8);
    let (rate, max, back_rate) = (
        reg(p.f(r.data, p::TILT_RATE)),
        reg(p.f(r.data, p::TILT_MAX)),
        reg(p.f(r.data, p::TILT_BACK)),
    );
    let v = r.game.grounds[g.0].u.rcruise();
    let rot = reg(v.x18);
    let abs_rot = if rot < fp::frsp(0.0) {
        fp::fneg(rot)
    } else {
        rot
    };
    let turns = fp::frsp(f64::from(fp::fctiwz(fp::fdivs(abs_rot, 360.0))));
    let wrapped = fp::fnmsubs(360.0, turns, abs_rot);
    let swing_back = |v: &mut super::vars::RCruiseVars| {
        v.x1C = mem(fp::frsp(if reg(v.x18) < 0.0 { 1.0 } else { -1.0 }));
        v.x20 = mem(fp::fmuls(back_rate, reg(v.x1C)));
        if wrapped <= reg(0.2f32) {
            v.x20 = 0.0;
            v.x2C = 0;
        }
    };
    match v.x2C {
        0 => {
            if v.x34 == 0 {
                swing_back(v);
            } else if reg(v.x24) < reg(v.x28) {
                v.x1C = 1.0;
                v.x20 = mem(fp::fmuls(rate, fp::fsubs(reg(v.x28), reg(v.x24))));
                if reg(v.x20) >= max {
                    v.x20 = mem(max);
                }
            } else {
                v.x1C = -1.0;
                v.x20 = mem(fp::fmuls(fp::fneg(rate), fp::fsubs(reg(v.x24), reg(v.x28))));
                if reg(v.x20) <= fp::fneg(max) {
                    v.x20 = mem(fp::fneg(max));
                }
            }
        }
        1 => {
            if v.x38 == 0 {
                if v.x34 == 0 {
                    swing_back(v);
                } else {
                    v.x2C = 0;
                }
            } else {
                v.x38 = v.x38.wrapping_sub(1);
                v.x20 = mem(fp::fmadds(reg(0.008f32), fp::fneg(reg(v.x1C)), reg(v.x20)));
                let s = reg(v.x20);
                let abs = if s < 0.0 { fp::fneg(s) } else { s };
                if abs <= reg(0.008f32) {
                    v.x20 = 0.0;
                }
            }
        }
        _ => {}
    }
    v.x18 = mem(fp::fadds(reg(v.x18), reg(v.x20)));
    let angle = mem(fp::fmuls(reg(0.017_453_292_f32), reg(v.x18)));
    let x34 = v.x34;
    v.x30 = x34;
    v.x34 = 0;
    v.x28 = 0.0;
    v.x24 = 0.0;
    for j in [pivot, pivot5] {
        jobj::set_rotation_z(&mut r.game.objs.jobjs, j.expect("the deck's pivot"), angle);
    }
}

/// The planks' setup, `grRCruise_80200B48`: each plank's collision joint and joint, at rest at
/// its joint's height.
#[doc(alias = "grRCruise_80200B48")]
pub fn init_planks(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    for (i, &part) in PLANKS.iter().enumerate() {
        let joint = setup::coll_joint_id(r.game, r.data, m, part);
        let j = nth_joint(r.game, gobj, part);
        let y = r.game.objs.jobjs[j.expect("a plank's joint")].translate.y;
        let e = &mut r.game.grounds[g.0].u.rcruise().entries[i];
        e.x2 = joint as i16;
        e.x14 = j;
        e.x8 = 0;
        e.x4 = 0;
        e.xC = y;
        e.x0 = 0;
    }
}

/// The planks' process, `grRCruise_80200C04`. A plank stood on (state 1) for long enough
/// shakes (2: every few ticks a random height within the parameters' range above its rest);
/// then it falls (3) until below the camera limits, where it hides and its collision goes (4);
/// after a while it is back at rest (its collision too) and blinks (5) before it can drop
/// again. A plank left before it shakes is at rest again.
#[doc(alias = "grRCruise_80200C04")]
pub fn update_planks(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let p = param(r.game);
    let pi = |off| p.i(r.data, off);
    let pf = |off| reg(p.f(r.data, off));
    for i in 0..PLANKS.len() {
        let e = &r.game.grounds[g.0].u.rcruise().entries[i];
        let (state, joint, j) = (e.x0, i32::from(e.x2), e.x14);
        match state {
            1 => {
                let wait = pi(p::PLANK_WAIT);
                let e = &mut r.game.grounds[g.0].u.rcruise().entries[i];
                if e.x8 == 0 {
                    e.x4 = 0;
                    e.x0 = 0;
                } else if e.x4 >= wait {
                    e.x0 = 2;
                } else {
                    e.x4 = e.x4.wrapping_add(1);
                }
            }
            2 => {
                let n = r.game.grounds[g.0].u.rcruise().entries[i].x4;
                if n.wrapping_rem(pi(p::PLANK_SHAKE_EVERY)) == 0 {
                    let (lo, hi) = (pf(p::PLANK_SHAKE_MIN), pf(p::PLANK_SHAKE_MAX));
                    let range = fp::fctiwz(fp::fmuls(100.0, fp::fsubs(hi, lo)));
                    let k = if range != 0 {
                        r.game.objs.rng.below(range)
                    } else {
                        0
                    };
                    let rest = reg(r.game.grounds[g.0].u.rcruise().entries[i].xC);
                    let y = fp::fadds(
                        rest,
                        fp::fadds(lo, fp::fdivs(fp::frsp(f64::from(k)), 100.0)),
                    );
                    let j = j.expect("a plank's joint");
                    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.y = mem(y));
                }
                let shake = pi(p::PLANK_SHAKE);
                let e = &mut r.game.grounds[g.0].u.rcruise().entries[i];
                if e.x4 >= shake {
                    e.x10 = 0.0;
                    e.x0 = 3;
                }
                e.x4 = e.x4.wrapping_add(1);
            }
            3 => {
                let (gravity, max) = (pf(p::PLANK_GRAVITY), pf(p::PLANK_FALL_MAX));
                let e = &mut r.game.grounds[g.0].u.rcruise().entries[i];
                e.x10 = mem(fp::fsubs(reg(e.x10), gravity));
                let s = reg(e.x10);
                let abs = if s < 0.0 { fp::fneg(s) } else { s };
                if abs > max {
                    e.x10 = mem(fp::fneg(max));
                }
                let speed = e.x10;
                let jj = j.expect("a plank's joint");
                jobj::set_srt(&mut r.game.objs.jobjs, jj, |o| {
                    o.translate.y = mem(fp::fadds(reg(o.translate.y), reg(speed)))
                });
                let bottom = stage::cam_bottom(r.game);
                if reg(r.game.objs.jobjs[jj].translate.y) <= reg(bottom) {
                    r.game.grounds[g.0].u.rcruise().entries[i].x10 = 0.0;
                    show_dobjs(&mut r.game.objs, j, false);
                    r.game.geometry_mut().disable_joint(joint);
                    let e = &mut r.game.grounds[g.0].u.rcruise().entries[i];
                    e.x4 = 0;
                    e.x0 = 4;
                }
            }
            4 => {
                let gone = pi(p::PLANK_GONE);
                let e = &r.game.grounds[g.0].u.rcruise().entries[i];
                if e.x4 >= gone {
                    let rest = e.xC;
                    let jj = j.expect("a plank's joint");
                    jobj::set_srt(&mut r.game.objs.jobjs, jj, |o| o.translate.y = rest);
                    update_coll_joint(r.game, r.data, joint);
                    r.game.geometry_mut().settle_vertices(joint);
                    show_dobjs(&mut r.game.objs, j, true);
                    r.game.geometry_mut().enable_joint(joint);
                    let e = &mut r.game.grounds[g.0].u.rcruise().entries[i];
                    e.x4 = 0;
                    e.x1 &= !0x80;
                    e.x0 = 5;
                }
                let e = &mut r.game.grounds[g.0].u.rcruise().entries[i];
                e.x4 = e.x4.wrapping_add(1);
            }
            5 => {
                let (blink, back) = (pi(p::PLANK_BLINK), pi(p::PLANK_BACK));
                let e = &mut r.game.grounds[g.0].u.rcruise().entries[i];
                if e.x4.wrapping_rem(blink) == 0 {
                    e.x1 ^= 0x80;
                    let hide = e.x1 & 0x80 != 0;
                    show_dobjs(&mut r.game.objs, j, !hide);
                }
                if r.game.grounds[g.0].u.rcruise().entries[i].x4 >= back {
                    show_dobjs(&mut r.game.objs, j, true);
                    let e = &mut r.game.grounds[g.0].u.rcruise().entries[i];
                    e.x4 = 0;
                    e.x0 = 0;
                }
                let e = &mut r.game.grounds[g.0].u.rcruise().entries[i];
                e.x4 = e.x4.wrapping_add(1);
            }
            _ => {}
        }
        r.game.grounds[g.0].u.rcruise().entries[i].x8 = 0;
    }
}

/// A plank's collision joint touched, `grRCruise_802010A4`: a fighter (collision kind 1) on a
/// plank at rest starts its count; every touch counts.
#[doc(alias = "grRCruise_802010A4")]
pub fn plank_touched(grounds: &mut Pool<Ground>, g: GroundId, joint_id: i32, kind: u32) {
    if kind != 1 {
        return;
    }
    let v = grounds[g.0].u.rcruise();
    for e in v.entries.iter_mut() {
        if i32::from(e.x2) == joint_id {
            if e.x0 == 0 {
                e.x4 = 0;
                e.x0 = 1;
            }
            e.x8 = e.x8.wrapping_add(1);
            break;
        }
    }
}

/// The ship steering its wind source, `grRCruise_80201110`: the push's direction turns by the
/// step, within 60 degrees of straight ahead; every 30 ticks of the source's age the step may
/// turn around, every 300 gusts start or stop; and the push's strength is random within the
/// gusts' or the calm's range.
#[doc(alias = "grRCruise_80201110")]
pub fn steer_wind(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.rcruise();
    let (w, step) = (v.x4.expect("the ship's wind source") as usize, v.x8);
    let e = &r.game.gr.winds.entries[w];
    let mut angle = reg(crate::math::atan2(e.pos.z, e.pos.x));
    angle = fp::fadds(angle, reg(step));
    if angle > 1.047_197_543_084_621_4 {
        angle = reg(std::f32::consts::FRAC_PI_3);
    }
    if angle < fp::fneg(1.047_197_543_084_621_4) {
        angle = fp::fneg(reg(std::f32::consts::FRAC_PI_3));
    }
    let (c, s) = (crate::math::cos(mem(angle)), crate::math::sin(mem(angle)));
    let e = &mut r.game.gr.winds.entries[w];
    e.pos.x = c;
    e.pos.z = s;
    let age = e.age;
    if age.wrapping_rem(30) == 0 && reg(r.game.objs.rng.draw_f32()) > 0.5 {
        let v = r.game.grounds[g.0].u.rcruise();
        v.x8 = mem(fp::fneg(reg(v.x8)));
    }
    if age.wrapping_rem(300) == 0 {
        if r.game.grounds[g.0].u.rcruise().xC != 0 {
            r.game.grounds[g.0].u.rcruise().xC = 0;
        } else if reg(r.game.objs.rng.draw_f32()) > 0.5 {
            r.game.grounds[g.0].u.rcruise().xC = 1;
        }
    }
    let scale = if r.game.grounds[g.0].u.rcruise().xC != 0 {
        fp::frsp(fp::fmadd(0.3, reg(r.game.objs.rng.draw_f32()), 0.2))
    } else {
        fp::frsp(fp::fmadd(0.2, reg(r.game.objs.rng.draw_f32()), 0.1))
    };
    r.game.gr.winds.entries[w].scale = mem(scale);
}

/// Sets display flags on a joint's tree as `grRCruise_80201288` walks it: the joint's first
/// display object, its first child's and that child's descendants and siblings, and its next
/// sibling's likewise.
#[doc(alias = "grRCruise_80201288")]
pub fn tree_dobj_flags(objs: &mut crate::hsd::Objects, j: Option<JObjId>, set: bool, flags: u32) {
    let Some(j) = j else { return };
    let apply = |objs: &mut crate::hsd::Objects, x: JObjId| {
        let d = jobj::get_dobj(&objs.jobjs, Some(x));
        if d.is_some() {
            if set {
                dobj::set_flags(&mut objs.dobjs, d, flags);
            } else {
                dobj::clear_flags(&mut objs.dobjs, d, flags);
            }
        }
    };
    apply(objs, j);
    for n in [objs.jobjs[j].child, objs.jobjs[j].next]
        .into_iter()
        .flatten()
    {
        apply(objs, n);
        let (c, nn) = (objs.jobjs[n].child, objs.jobjs[n].next);
        if c.is_some() {
            tree_dobj_flags(objs, c, set, flags);
        }
        if nn.is_some() {
            tree_dobj_flags(objs, nn, set, flags);
        }
    }
}

/// A part that appears: its background's copy (map 5) shows, the ship's copy's display objects
/// hide (the inline `set_hidden_a`).
fn vanish_appear(r: &mut Running, i: usize) {
    let part = VANISH[i].0;
    let (Some(back), Some(ship)) = (map_gobj(r.game, 5), map_gobj(r.game, 1)) else {
        return;
    };
    let j = nth_joint(r.game, back, part);
    if j.is_some() {
        jobj::clear_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    }
    let j = nth_joint(r.game, ship, part);
    if j.is_some() {
        tree_dobj_flags(&mut r.game.objs, j, true, DOBJ_HIDDEN);
    }
}

/// A part that goes: its background's copy hides, the ship's copy's display objects show (the
/// inline `set_hidden_b`).
fn vanish_go(r: &mut Running, i: usize) {
    let part = VANISH[i].0;
    let (Some(back), Some(ship)) = (map_gobj(r.game, 5), map_gobj(r.game, 1)) else {
        return;
    };
    let j = nth_joint(r.game, back, part);
    if j.is_some() {
        jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    }
    let j = nth_joint(r.game, ship, part);
    if j.is_some() {
        tree_dobj_flags(&mut r.game.objs, j, false, DOBJ_HIDDEN);
    }
}

/// The parts that appear with the camera, `grRCruise_80201410`: their block from the heap, each
/// part's joint; those there from the start in place (state 2), the others waiting off (state
/// 0, their first animation, their collision out of the search).
#[doc(alias = "grRCruise_80201410")]
pub fn init_vanish(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    // The halfword the setup never writes (`x2`) holds the block's leftovers, else zero.
    let addr = crate::hsd::heap::mem_alloc(0xA0);
    let left = leftover_bytes(r, addr, 0xA0);
    let v = r.game.grounds[g.0].u.rcruise();
    v.vanish_addr = addr;
    v.vanish = (0..VANISH.len())
        .map(|i| super::vars::VanishEntry {
            x2: left
                .as_ref()
                .map_or(0, |b| i16::from_be_bytes([b[8 * i + 2], b[8 * i + 3]])),
            ..Default::default()
        })
        .collect();
    for (i, &(part, joint, there)) in VANISH.iter().enumerate() {
        let j = nth_joint(r.game, gobj, part);
        assert!(j.is_some(), "grrcruise.c: gp->u.map.vanish[i].jobj");
        r.game.grounds[g.0].u.rcruise().vanish[i].jobj = j;
        if there {
            r.game.grounds[g.0].u.rcruise().vanish[i].x0 = 2;
            vanish_appear(r, i);
            r.game.geometry_mut().enable_joint(joint);
        } else {
            r.game.grounds[g.0].u.rcruise().vanish[i].x0 = 0;
            anime::start(r.game, r.data, gobj, part, 2, 1, 0.0, 1.0, true);
            r.game.geometry_mut().disable_joint(joint);
        }
    }
}

/// The parts that appear with the camera, `grRCruise_80201588`: a waiting part within 20 of
/// the camera limits starts appearing (state 1, its collision back where it stands) and is in
/// place once its animation ends (2); a part in place that leaves them starts going (3), and
/// is waiting again once that ends, its collision out of the search once no other part on
/// its joint is out.
#[doc(alias = "grRCruise_80201588")]
pub fn update_vanish(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    assert!(
        !r.game.grounds[g.0].u.rcruise().vanish.is_empty(),
        "grrcruise.c: gp->u.map.vanish"
    );
    for (i, &(part, joint, there)) in VANISH.iter().enumerate() {
        if there {
            continue;
        }
        let e = &r.game.grounds[g.0].u.rcruise().vanish[i];
        let (state, j) = (e.x0, e.jobj);
        match state {
            0 => {
                let pos = world_position(&mut r.game.objs, r.data, j, None);
                if camera::in_bounds_by(&r.game.stage_info, pos, -20.0) {
                    r.game.grounds[g.0].u.rcruise().vanish[i].x0 = 1;
                    anime::start(r.game, r.data, gobj, part, 2, 2, 0.0, 1.0, true);
                    r.game.geometry_mut().enable_joint(joint);
                    update_coll_joint(r.game, r.data, joint);
                    r.game.geometry_mut().settle_vertices(joint);
                }
            }
            1 => {
                if anime::finished(r.game, gobj, part, 2) {
                    r.game.grounds[g.0].u.rcruise().vanish[i].x0 = 2;
                    vanish_appear(r, i);
                }
            }
            2 => {
                let pos = world_position(&mut r.game.objs, r.data, j, None);
                if !camera::in_bounds_by(&r.game.stage_info, pos, -20.0) {
                    r.game.grounds[g.0].u.rcruise().vanish[i].x0 = 3;
                    anime::start(r.game, r.data, gobj, part, 2, 3, 0.0, 1.0, true);
                    vanish_go(r, i);
                }
            }
            3 if anime::finished(r.game, gobj, part, 2) => {
                r.game.grounds[g.0].u.rcruise().vanish[i].x0 = 0;
                anime::start(r.game, r.data, gobj, part, 2, 1, 0.0, 1.0, true);
                let v = r.game.grounds[g.0].u.rcruise();
                let busy = VANISH
                    .iter()
                    .zip(v.vanish.iter())
                    .any(|(d, e)| d.1 == joint && e.x0 != 0);
                if !busy {
                    r.game.geometry_mut().disable_joint(joint);
                }
            }
            _ => {}
        }
    }
}

/// The wind the course blows, `grRCruise_80201918`: its last step (map 3's), or none.
#[doc(alias = "grRCruise_80201918")]
pub fn wind(game: &Game) -> Vec3 {
    let Some(gobj) = map_gobj(game, 3) else {
        return Vec3::ZERO;
    };
    match &game.grounds[ground_of(game, gobj).0].u {
        super::GroundVars::Scroll(v) => v.x1C,
        other => panic!("Rainbow Cruise's course's variables are {other:?}"),
    }
}

/// Whether a line is one of the ship's planks', `grRCruise_80201988`: on Rainbow Cruise, a
/// line of joint 27 or 28 to 43.
#[doc(alias = "grRCruise_80201988")]
pub fn plank_line(game: &Game, line_id: i32) -> bool {
    if game.stage_info.grkind != super::GR_KIND_RCRUISE || line_id == -1 {
        return false;
    }
    let joint = (game.geometry.as_ref())
        .expect("no collision geometry")
        .joint_from_line(line_id);
    matches!(joint, 27..=43)
}

/// The course's render callback, `fn_80201BE0`: drawn as any stage object is
/// (`grDisplay_801C5DB0`), unless the game is paused for debugging or the camera is free.
#[doc(alias = "fn_80201BE0")]
pub fn render_course(r: &mut Running, gobj: GObjId, pass: i32) {
    if r.game.db_pause_flag(1) || r.game.db_pause_flag(2) || camera::is_free(&r.game.camera.cam) {
        return;
    }
    r.render_stage_object(gobj, pass);
}

/// The stage's shadow check, `grRCruise_80201C58`: every joint takes the shadows.
#[doc(alias = "grRCruise_80201C58")]
pub fn check_shadow_render() -> bool {
    true
}
