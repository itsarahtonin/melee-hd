//! Princess Peach's Castle, `melee/gr/grcastle.c`: the castle (map 3) with its flags on
//! dynamics chains, the scenery (maps 0 and 4), the lifts that rise under a fighter's weight
//! (map 6), the three switches (maps 18 to 20) that appear one at a time on a part of the
//! castle or the lifts and, hit, bring out their blocks (maps 17, 7 and 5) for a while, and
//! the Bullet Bills (maps 8 to 16, one per path), each with its blaster (map 2) and its blast
//! (map 1).
//!
//! The castle steers a wind source that sways its flags, and the moat registers a wind
//! device that slows a fighter's fall into it.

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::super::camera;
use super::super::handles::{CodeRef, GroundId};
use super::super::state::{Game, Running};
use super::data::{CASTLE_CALLBACKS, StageCallbacks};
use super::ground::{ground_of, jobj_of, map_gobj, nth_joint, update_map_coll};
use super::{CollUser, anime, grlib, material, setup, zako};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::quat::Quat;
use crate::vec::Vec3;

/// `fn_801D0924`, the castle's render callback.
pub const RENDER_STAGE: u32 = 0x801D_0924;
/// `grCastle_801D0520`, the blocks' render callback.
pub const RENDER_BLOCKS: u32 = 0x801D_0520;
/// `grCastle_801CF750`: a lift's collision joint touched.
pub const LIFT_TOUCHED: u32 = 0x801C_F750;
/// `grCastle_801D09B8`, the moat's wind device's callback.
pub const MOAT: u32 = 0x801D_09B8;
/// `fn_801CE3A0` and `fn_801CE9DC`: a Bullet Bill's and a blast's item dealt a hit.
pub const BILL_DEALT: u32 = 0x801C_E3A0;
pub const BLAST_DEALT: u32 = 0x801C_E9DC;
/// `fn_801CFAFC` and `fn_801CFB68`: a switch's item hit, or touched.
pub const SWITCH_HIT: u32 = 0x801C_FAFC;
pub const SWITCH_TOUCHED: u32 = 0x801C_FB68;
/// `grCs_803B7F28`: each block's item hit (`grCastle_801D0550` to `801D0680`).
const BLOCK_HIT: [u32; 5] = [
    0x801D_0550,
    0x801D_059C,
    0x801D_05E8,
    0x801D_0634,
    0x801D_0680,
];
/// `grCs_803B7F3C`: each block's item touched (`grCastle_801D06CC` to `801D08AC`).
const BLOCK_TOUCHED: [u32; 5] = [
    0x801D_06CC,
    0x801D_0744,
    0x801D_07BC,
    0x801D_0834,
    0x801D_08AC,
];
/// `grCastle_801CD610` and `grCastle_801CD9B4`: the castle's and the lifts' start hooks.
const START_STAGE: u32 = 0x801C_D610;
const START_LIFTS: u32 = 0x801C_D9B4;

/// The stack address of the scale joint's descriptor for the objects the castle's setup makes
/// (`grCastle_801CD37C`, a 0x20-byte frame).
const INIT_FRAME: u32 = setup::init_frame(0x20);

/// The stack addresses of the scale joint's descriptor (`INIT_FRAME`'s) for the objects
/// made after the setup, each by how much deeper its create runs than the setup's: the
/// switches from the castle's start hook (0x60), their blocks from the switches' setup (0xA8),
/// a Bullet Bill from the castle's process (as deep as the setup), its blaster and blast from
/// its own (0x20).
const SWITCH_FRAME: u32 = INIT_FRAME - 0x60;
const BLOCKS_FRAME: u32 = INIT_FRAME - 0xA8;
const BILL_FRAME: u32 = INIT_FRAME;
const BLASTER_FRAME: u32 = INIT_FRAME - 0x20;
const BLAST_FRAME: u32 = INIT_FRAME - 0x20;

/// The flags' chains: the part each hangs from, and its description (`grCs_803B7EA8`, all
/// `dynamicsdata_flag6`).
const FLAGS: [(i32, i16); 4] = [(66, 6), (76, 6), (85, 6), (94, 6)];

/// `grCs_803B7EC8`, a switch's blink as the original reads it: its 19 halfwords' bytes, each
/// a wait in half ticks between turning shown and hidden, until the -1.
const BLINK: [i8; 38] = {
    let words: [u16; 19] = [
        0x201, 0x201, 0x201, 0x201, 0x201, 0x201, 0x101, 0x101, 0x101, 0x101, 0x101, 0x101, 0x101,
        0x101, 0x101, 0x102, 0x102, 0x102, 0x1FF,
    ];
    let mut b = [0i8; 38];
    let mut i = 0;
    while i < 19 {
        b[2 * i] = (words[i] >> 8) as i8;
        b[2 * i + 1] = words[i] as i8;
        i += 1;
    }
    b
};

/// `grCs_803B7EFC`: where a switch may appear, a map and its part.
const SWITCH_SPOTS: [(i32, i32); 11] = [
    (3, 16),
    (3, 17),
    (3, 14),
    (3, 15),
    (3, 18),
    (3, 19),
    (3, 21),
    (3, 22),
    (3, 20),
    (6, 2),
    (6, 5),
];

/// `grCs_803B7F50`: a bumped block's rise, tick by tick, to the -1 that ends it.
const BUMP: [f32; 6] = [4.0, 6.0, 7.0, 6.0, 4.0, -1.0];

/// `JOBJ_HIDDEN`.
const HIDDEN: u32 = jobj::HIDDEN;

/// `CmQuakeKind`'s `QuakeKind_Small`.
const QUAKE_SMALL: i32 = 2;

/// grcastle.c's statics besides `yakumono_param`.
#[derive(melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq)]
pub struct Globals {
    /// The wind source the castle steers (`grCs_804D6974`).
    pub wind: Option<u8>,
    /// Its turn a tick (`grCs_804D45E0`), a degree either way.
    pub step: f32,
    /// Whether it gusts (`grCs_804D45E4`).
    pub gusts: u8,
}

impl Default for Globals {
    /// The statics as the executable holds them.
    fn default() -> Self {
        Globals {
            wind: None,
            step: 0.017_453_292,
            gusts: 1,
        }
    }
}

/// The fields of `grCastle_YakumonoParam` the code reads, by offset.
#[derive(Clone, Copy)]
struct Param(DataRef);

impl Param {
    fn h(self, data: &Archives, off: u32) -> i16 {
        data.u16(self.0.at(off)) as i16
    }
    fn f(self, data: &Archives, off: u32) -> f32 {
        data.f32(self.0.at(off))
    }
    /// A Bullet Bill's path's entry (`entries[i]`, 0x14 bytes from 0x5C).
    fn entry(self, i: i16) -> u32 {
        0x5C + 0x14 * i as u32
    }
}

fn param(game: &Game) -> Param {
    Param(game.gr.castle_yakumono.expect("grcastle.c: yakumono_param"))
}

/// A stage object's process or callback, by address: Princess Peach's Castle's.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x801C_D608 | 0x801C_D60C | 0x801C_DF50 | 0x801C_E7E4 | 0x801C_E8E4 | 0x801C_EAC8
        | 0x801C_F0F0 | 0x801C_F74C => empty(r, gobj),
        0x801C_D5BC => init_back(r, gobj),
        START_STAGE => start_stage(r, gobj),
        0x801C_D658 => init_stage(r, gobj),
        0x801C_D8A8 => proc_stage(r, gobj),
        0x801C_D960 => destroy_stage(r, gobj),
        START_LIFTS => start_lifts(r, gobj),
        0x801C_DA0C => init_lifts(r, gobj),
        0x801C_DC44 => proc_lifts(r, gobj),
        0x801C_E260 => init_bill(r, gobj),
        0x801C_E578 => proc_bill(r, gobj),
        0x801C_E7E8 => init_blaster(r, gobj),
        0x801C_E860 => proc_blaster(r, gobj),
        0x801C_E8E8 => init_blast(r, gobj),
        0x801C_E9E8 => proc_blast(r, gobj),
        0x801C_EACC => init_blocks(r, gobj),
        0x801C_EF04 => proc_blocks(r, gobj),
        0x801C_F0F4 => init_switch(r, gobj),
        0x801C_F308 => proc_switch(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x801C_D37C => on_init(r),
        0x801C_D4A0 => on_load(r),
        0x801C_D4A4 => on_start(r),
        _ => return false,
    }
    true
}

/// The stage's render callbacks, by address.
pub fn run_render(r: &mut Running, gobj: GObjId, cb: u32, pass: i32) -> bool {
    match cb {
        RENDER_STAGE => render_stage(r, gobj, pass),
        RENDER_BLOCKS => render_blocks(r, gobj, pass),
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing.
#[doc(alias = "grCastle_801CD608")]
#[doc(alias = "grCastle_801CD60C")]
#[doc(alias = "grCastle_801CDF50")]
#[doc(alias = "grCastle_801CE7E4")]
#[doc(alias = "grCastle_801CE8E4")]
#[doc(alias = "grCastle_801CEAC8")]
#[doc(alias = "grCastle_801CF0F0")]
#[doc(alias = "grCastle_801CF74C")]
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// Creates one of the stage's objects and sets it up, `grCastle_801CD4D0`.
#[doc(alias = "grCastle_801CD4D0")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

/// The stage's objects' setup functions and processes, by map id (`grCs_StageCallbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &CASTLE_CALLBACKS[map_id as usize]
}

fn map_id(game: &Game, gobj: GObjId) -> i32 {
    game.grounds[ground_of(game, gobj).0].map_id
}

/// Princess Peach's Castle's setup, `grCastle_801CD37C`: its parameters; maps 0, 4, 3 and 6;
/// the collision joints the switches' blocks and the lifts bring out kept out of the search;
/// camera limits and blast zones; and the wind source the castle steers, blowing along x,
/// waving 60 degrees a tick.
#[doc(alias = "grCastle_801CD37C")]
pub fn on_init(r: &mut Running) {
    r.game.gr.castle_yakumono = super::ground::yakumono_param(r.game);
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    for map_id in [0, 4, 3, 6] {
        create(r, map_id, INIT_FRAME);
    }
    for joint in [0, 1, 2, 6, 7, 8, 9, 10, 11, 12, 13, 14] {
        r.game.geometry_mut().disable_joint(joint);
    }
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
    let w = crate::lb::wind::push(
        &mut r.game.gr.winds,
        Vec3::new(1.0, 0.0, 0.0),
        -1,
        0.5,
        0.0,
        std::f32::consts::FRAC_PI_3,
        -10000.0,
        10000.0,
        10000.0,
        -10000.0,
    );
    r.game.gr.castle.wind = w;
}

/// The demo's setup, `grCastle_801CD338`: the lifts' second part hidden.
#[doc(alias = "grCastle_801CD338")]
pub fn on_demo_init(r: &mut Running) {
    if let Some(gobj) = map_gobj(r.game, 6) {
        let j = nth_joint(r.game, gobj, 4);
        if j.is_some() {
            jobj::set_flags_all(&mut r.game.objs.jobjs, j, HIDDEN);
        }
    }
}

/// `grCastle_801CD4A0` does nothing.
#[doc(alias = "grCastle_801CD4A0")]
pub fn on_load(_r: &mut Running) {}

/// Princess Peach's Castle's start, `grCastle_801CD4A4`: the enemy spawner, without enemies.
#[doc(alias = "grCastle_801CD4A4")]
pub fn on_start(r: &mut Running) {
    zako::start(r.game, None);
}

/// The scenery's setup (map 4), `grCastle_801CD5BC`: its animation, drawn behind.
#[doc(alias = "grCastle_801CD5BC")]
pub fn init_back(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    r.game.grounds[g.0].x11_flags.b012 = 2;
}

/// The castle's start hook, `grCastle_801CD610`: the moat as the fighters' wind device, the
/// switches, and the first wait for a Bullet Bill.
#[doc(alias = "grCastle_801CD610")]
pub fn start_stage(r: &mut Running, gobj: GObjId) {
    super::oldpupupu::register_wind(r.game, gobj, 6, CodeRef(MOAT));
    init_switches(r, gobj);
    reset_bill_wait(r, gobj);
}

/// The castle's setup (map 3), `grCastle_801CD658`: its animation; the fountain's spray (the
/// stage's effect 0x7536); the flags' dynamics chains from the archive's
/// `dynamicsdata_flag6`; its render callback and start hook; no switches yet and no Bullet
/// Bill due; it receives shadows.
#[doc(alias = "grCastle_801CD658")]
pub fn init_stage(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    let s = reg(super::ground::scale(r.game));
    let pos = Vec3::new(
        mem(fp::fmuls(reg(-257.0), s)),
        mem(fp::fmuls(reg(13.5), s)),
        mem(fp::fmuls(reg(-252.0), s)),
    );
    grlib::generator_at(r.game, r.data, 0x7536, 0x1E, pos);
    for d in r.game.grounds[g.0].u.castle().dynamics.iter_mut() {
        d.data = None;
    }
    let archive = super::datfiles::main_archive(r.game).archive;
    let file = r.data.get(archive);
    let found = [
        "dynamicsdata_flag3",
        "dynamicsdata_flag4",
        "dynamicsdata_flag6",
    ]
    .map(|name| file.public(name).map(|offset| DataRef { archive, offset }));
    if let [Some(flag3), Some(flag4), Some(flag6)] = found {
        for (i, &(depth, ty)) in FLAGS.iter().enumerate() {
            let root = nth_joint(r.game, gobj, depth);
            let src = match ty {
                3 => Some(flag3),
                4 => Some(flag4),
                6 => Some(flag6),
                _ => None,
            };
            match (root, src) {
                (Some(_), Some(src)) => chain(r, root, src, g, i),
                _ => r.game.grounds[g.0].u.castle().dynamics[i].data = None,
            }
        }
    }
    r.game.world.gobj_mut(gobj).render = Some(CodeRef(RENDER_STAGE));
    setup::add_start_hook(r.game, gobj, CodeRef(START_STAGE));
    let v = r.game.grounds[g.0].u.castle();
    v.switches = [None; 3];
    v.set_bill_due(false);
    r.game.grounds[g.0].x10_flags.b5 = true;
}

/// Builds flag `i`'s dynamics chain down `root` (`grLib_801C9B20`'s body): as many links as
/// its description has (`lb_8000FD48`), and their parameters (`lb_80011710`).
pub fn chain(r: &mut Running, root: Option<JObjId>, src: DataRef, g: GroundId, i: usize) {
    let n = r.data.u32(src.at(4));
    let mut desc = r.game.grounds[g.0].u.castle().dynamics[i].clone();
    r.game.load_dynamics_chain(r.data, root, &mut desc, n);
    r.game.set_dynamics_params(r.data, src, &mut desc);
    r.game.grounds[g.0].u.castle().dynamics[i] = desc;
}

/// The castle's process, `grCastle_801CD8A8`: the switches' and the Bullet Bills' timers, the
/// collision following the model, the wind sources aging and the castle's wind steered; each
/// flag turns into the wind, then swings, the wind's strength drawn again after each.
#[doc(alias = "grCastle_801CD8A8")]
pub fn proc_stage(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    switch_timer(r, gobj);
    bill_timer(r, gobj);
    update_map_coll(r.game, r.data, gobj);
    let level = r.game.gr.wind_level;
    r.game.gr.wind_level = crate::lb::wind::tick(&mut r.game.gr.winds, level);
    steer_wind(r);
    for i in 0..12 {
        if let Some(d) = r.game.grounds[g.0].u.castle().dynamics[i].data {
            let link = r.game.fighter_dynamics[d.0].desc.jobj;
            let pole = r.game.objs.jobjs[link.expect("a flag's link")].parent;
            turn_to_wind(r, pole);
        }
    }
    for i in 0..12 {
        if r.game.grounds[g.0].u.castle().dynamics[i].data.is_some() {
            swing(r, g, i);
            rescale_wind(r);
        }
    }
}

/// Swings flag `i`'s chain a frame (`grLib_801C9B8C`'s body: `lb_8001044C` with nothing to
/// collide with and no floor).
pub fn swing(r: &mut Running, g: GroundId, i: usize) {
    let desc = r.game.grounds[g.0].u.castle().dynamics[i].clone();
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

/// The castle's destroy callback, `grCastle_801CD960`: each flag's chain freed.
#[doc(alias = "grCastle_801CD960")]
pub fn destroy_stage(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    for i in 0..12 {
        let mut desc = r.game.grounds[g.0].u.castle().dynamics[i].clone();
        grlib::free_chain(r.game, &mut desc);
        r.game.grounds[g.0].u.castle().dynamics[i] = desc;
    }
}

/// The lifts' start hook, `grCastle_801CD9B4`: their collision joints (4 and 5) tell them
/// what stands on them.
#[doc(alias = "grCastle_801CD9B4")]
pub fn start_lifts(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    for joint in [4, 5] {
        r.game.geometry_mut().set_ledge_callback(
            joint,
            Some(CollUser::Ground(g)),
            Some(CodeRef(LIFT_TOUCHED)),
        );
    }
}

/// The lifts' setup (map 6), `grCastle_801CDA0C`: their collision; each lift (parts 1 and 4)
/// at the bottom (or the top, when the parameters' `x18` is 0), at rest, its collision joint's
/// vertices settled there, ready to rise; their start hook; the collision following the model.
#[doc(alias = "grCastle_801CDA0C")]
pub fn init_lifts(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    r.game.grounds[g.0].x10_flags.b5 = true;
    let p = param(r.game);
    for (i, (part, joint)) in [(1, 4), (4, 5)].into_iter().enumerate() {
        let lift = nth_joint(r.game, gobj, part);
        let pos = if reg(p.f(r.data, 0x18)) != 0.0 {
            p.f(r.data, 0x14)
        } else {
            p.f(r.data, 0x10)
        };
        let l = &mut r.game.grounds[g.0].u.castle_lifts().plat[i];
        l.jobj = lift;
        l.pos = pos;
        l.state = 0;
        let lj = lift.expect("grcastle.c: a lift's joint");
        jobj::set_srt(&mut r.game.objs.jobjs, lj, |o| o.translate.y = pos);
        r.game.geometry_mut().settle_vertices(joint);
        let timer = fp::fctiwz(reg(p.f(r.data, 0x38))) as i16;
        let l = &mut r.game.grounds[g.0].u.castle_lifts().plat[i];
        l.timer = timer;
        l.wind = 0.0;
    }
    setup::add_start_hook(r.game, gobj, CodeRef(START_LIFTS));
    update_map_coll(r.game, r.data, gobj);
}

/// The lifts' process, `grCastle_801CDC44`. A lift at rest stood on (state 0) waits its
/// parameters' ticks before rising (state 2), as fast as the weight on it allows, to the top;
/// left alone (state 1) it waits, then sinks (state 3) back to the bottom. Then its weight
/// clears, and the collision follows the model.
#[doc(alias = "grCastle_801CDC44")]
pub fn proc_lifts(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let p = param(r.game);
    let d = r.data;
    for i in 0..2 {
        let l = &mut r.game.grounds[g.0].u.castle_lifts().plat[i];
        let wind = reg(l.wind);
        let mut moved = None;
        match l.state {
            0 => {
                if wind > 0.0 {
                    let cnt = l.timer;
                    l.timer = cnt.wrapping_add(1);
                    if fp::frsp(f64::from(cnt)) > reg(p.f(d, 0x38)) {
                        l.state = 2;
                    }
                } else {
                    l.timer = 0;
                    l.state = 1;
                }
            }
            1 => {
                if wind > 0.0 {
                    l.timer = 0;
                    l.state = 0;
                } else {
                    let cnt = l.timer;
                    l.timer = cnt.wrapping_add(1);
                    if fp::frsp(f64::from(cnt)) > reg(p.f(d, 0x3C)) {
                        l.state = 3;
                    }
                }
            }
            2 => {
                if wind > 0.0 {
                    let mut speed = fp::fmadds(wind, reg(p.f(d, 0x20)), reg(p.f(d, 0x24)));
                    if speed > reg(p.f(d, 0x28)) {
                        speed = reg(p.f(d, 0x28));
                    }
                    let top = p.f(d, 0x10);
                    let cur = reg(l.pos);
                    l.pos = if fp::fsubs(reg(top), cur) < speed {
                        top
                    } else {
                        mem(fp::fadds(cur, speed))
                    };
                    moved = Some(l.pos);
                } else {
                    l.timer = 0;
                    l.state = 1;
                }
            }
            3 => {
                if wind > 0.0 {
                    l.timer = 0;
                    l.state = 0;
                } else {
                    let mut speed = fp::fmadds(wind, reg(p.f(d, 0x2C)), reg(p.f(d, 0x30)));
                    if speed > reg(p.f(d, 0x34)) {
                        speed = reg(p.f(d, 0x34));
                    }
                    let bottom = p.f(d, 0x14);
                    l.pos = if fp::fsubs(reg(l.pos), reg(bottom)) < speed {
                        bottom
                    } else {
                        mem(fp::fsubs(reg(l.pos), speed))
                    };
                    moved = Some(l.pos);
                }
            }
            _ => {}
        }
        let l = &mut r.game.grounds[g.0].u.castle_lifts().plat[i];
        if let Some(y) = moved {
            let lj = l.jobj.expect("grcastle.c: a lift's joint");
            jobj::set_srt(&mut r.game.objs.jobjs, lj, |o| o.translate.y = y);
        }
        r.game.grounds[g.0].u.castle_lifts().plat[i].wind = 0.0;
    }
    update_map_coll(r.game, r.data, gobj);
}

/// Where the first Bullet Bill flying is, `grCastle_801CDF54`: its part 0's place, on Princess
/// Peach's Castle only.
#[doc(alias = "grCastle_801CDF54")]
pub fn bill_position(game: &mut Game, data: &Archives) -> Option<Vec3> {
    if game.stage_info.grkind != super::GR_KIND_CASTLE {
        return None;
    }
    for i in 0..9 {
        if let Some(gobj) = map_gobj(game, i + 8) {
            let j = nth_joint(game, gobj, 0);
            return Some(crate::lb::joint::world_position(
                &mut game.objs,
                data,
                j,
                None,
            ));
        }
    }
    None
}

/// A Bullet Bill comes due, `grCastle_801CDFD8`: a wait of the parameters' `x8` plus a draw
/// below `xA`, and no path flown lately.
#[doc(alias = "grCastle_801CDFD8")]
pub fn reset_bill_wait(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    r.game.grounds[g.0].u.castle().set_bill_due(true);
    let p = param(r.game);
    let range = i32::from(p.h(r.data, 0xA));
    let rand = if range != 0 {
        r.game.objs.rng.below(range)
    } else {
        0
    };
    let wait = i32::from(p.h(r.data, 8)).wrapping_add(rand) as i16;
    let v = r.game.grounds[g.0].u.castle();
    v.bill_wait = wait;
    v.recent = [-1; 3];
    v.recent_at = 0;
}

/// A Bullet Bill's path, `grCastle_801CE054`: a draw among the nine not among the last three
/// flown, noted in turn among those.
#[doc(alias = "grCastle_801CE054")]
pub fn pick_path(r: &mut Running, gobj: GObjId) -> i32 {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.castle();
    let mut paths = [0i16; 9];
    let mut count = 0;
    for path in 0..9i16 {
        if !v.recent.contains(&path) {
            paths[count] = path;
            count += 1;
        }
    }
    // With no path left the original would take its array's first entry from a stack slot it
    // never set; at most three of the nine are ever excluded (docs/melee-bugs.md).
    assert!(
        count != 0,
        "grCastle_801CE054: every path among the last three"
    );
    let idx = r.game.objs.rng.below(count as i32);
    let picked = paths[idx as usize];
    let v = r.game.grounds[g.0].u.castle();
    v.recent[v.recent_at as usize] = picked;
    v.recent_at = if v.recent_at == 2 { 0 } else { v.recent_at + 1 };
    i32::from(picked)
}

/// The Bullet Bills' timer, `grCastle_801CE19C`: once one is due and the wait runs out, one
/// flies a path not flown lately, the castle's sound panned to it, sent by the castle.
#[doc(alias = "grCastle_801CE19C")]
pub fn bill_timer(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.castle();
    if !v.bill_due() {
        return;
    }
    let t = v.bill_wait;
    v.bill_wait = t.wrapping_sub(1);
    if t >= 0 {
        return;
    }
    let path = pick_path(r, gobj);
    let new = create(r, path + 8, BILL_FRAME);
    r.game.grounds[g.0].u.castle().set_bill_due(false);
    if let Some(new) = new {
        let ng = ground_of(r.game, new);
        r.object_sound(Some(g), 0, 0x53021);
        let p = param(r.game);
        let i = r.game.grounds[ng.0].u.castle_bill().path;
        let pan = p.f(r.data, p.entry(i) + 4);
        r.object_sound_pan(Some(g), 0, pan);
        r.game.grounds[ng.0].u.castle_bill().stage = Some(gobj);
    }
}

/// A camera subject framing a stage object, its extent from the parameters at `at`.
fn new_subject(r: &mut Running, at: u32) -> crate::game::handles::CameraBoxId {
    let p = param(r.game);
    let ext = [0, 4, 8, 0xC].map(|o| p.f(r.data, at + o));
    // Camera_80029044(2): framed only inside the camera bounds.
    let b = camera::alloc_box(&mut r.game.camera.boxes, camera::AUTO);
    let s = &mut r.game.camera.boxes[b];
    s.target_ext.h.x = ext[0];
    s.target_ext.h.y = ext[1];
    s.target_ext.v.x = ext[2];
    s.target_ext.v.y = ext[3];
    b
}

/// A camera subject follows a stage object's part 0.
fn follow(r: &mut Running, gobj: GObjId, subject: Option<crate::game::handles::CameraBoxId>) {
    let Some(b) = subject else { return };
    let j = nth_joint(r.game, gobj, 0);
    let pos = crate::lb::joint::world_position(&mut r.game.objs, r.data, j, None);
    let s = &mut r.game.camera.boxes[b];
    s.pos = pos;
    s.bone_pos = pos;
}

/// A Bullet Bill's setup (maps 8 to 16), `grCastle_801CE260`: its collision and flight; its
/// path, no blaster yet and the path's wait before it; a camera subject; its materials in the
/// stage class; its hitbox item on part 0, which ownerless items may hit; its sound.
#[doc(alias = "grCastle_801CE260")]
pub fn init_bill(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    anime::start_map(r.game, r.data, gobj, m, 0);
    let p = param(r.game);
    let path = (m - 8) as i16;
    let delay = p.h(r.data, p.entry(path));
    let v = r.game.grounds[g.0].u.castle_bill();
    v.set_done(false);
    v.path = path;
    v.blaster = None;
    v.delay = delay;
    let b = new_subject(r, 0x118);
    r.game.grounds[g.0].u.castle_bill().subject = Some(b);
    material::model_to_stage_class(r.game, j);
    let part = nth_joint(r.game, gobj, 0);
    let item = material::stage_item(r, 0, 3, g, part, Some(CodeRef(BILL_DEALT)), None, None);
    r.game.grounds[g.0].u.castle_bill().item = item;
    super::super::it::hitbox::let_own_items_hit(r.game, item.expect("grcastle.c: an item"));
    r.object_sound(Some(g), 0, 0x53025);
}

/// A Bullet Bill's or a blast's item dealt a hit, `fn_801CE3A0`, `fn_801CE9DC`: nothing.
#[doc(alias = "fn_801CE3A0")]
#[doc(alias = "fn_801CE9DC")]
pub fn item_nothing(_r: &mut Running, _item: GObjId, _g: GroundId) {}

/// Puts a Bullet Bill's blaster at its part 1, the parameters' height above, turned to its
/// path's rotation, `grCastle_801CE3AC`.
#[doc(alias = "grCastle_801CE3AC")]
pub fn place_blaster(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.castle_bill();
    let Some(blaster) = v.blaster else { return };
    let path = v.path;
    let bj = jobj_of(r.game, blaster).expect("grcastle.c: the blaster's joint");
    let p = param(r.game);
    let offset = Vec3::new(0.0, p.f(r.data, 0x110), 0.0);
    let part = nth_joint(r.game, gobj, 1);
    let pos = crate::lb::joint::world_position(&mut r.game.objs, r.data, part, Some(offset));
    jobj::set_translate(&mut r.game.objs.jobjs, bj, pos);
    let e = p.entry(path);
    let rot = Quat {
        x: p.f(r.data, e + 8),
        y: p.f(r.data, e + 0xC),
        z: p.f(r.data, e + 0x10),
        w: 0.0,
    };
    jobj::set_rotation(&mut r.game.objs.jobjs, bj, rot);
}

/// A Bullet Bill's process, `grCastle_801CE578`. Its camera subject follows it; once its
/// path's wait runs out its blaster comes, with a sound. When its flight ends it lingers the
/// parameters' ticks with a color script, its blaster gone and its sound stopped; then its
/// item goes, its blast comes where it is, with a wind blast, the castle waits for the next,
/// and it goes with its camera subject.
#[doc(alias = "grCastle_801CE578")]
pub fn proc_bill(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let subject = r.game.grounds[g.0].u.castle_bill().subject;
    follow(r, gobj, subject);
    let v = r.game.grounds[g.0].u.castle_bill();
    if v.blaster.is_none() {
        let t = v.delay;
        v.delay = t.wrapping_sub(1);
        if t < 0 {
            let blaster = create(r, 2, BLASTER_FRAME);
            r.game.grounds[g.0].u.castle_bill().blaster = blaster;
            r.stage_sound(0x53026);
            place_blaster(r, gobj);
        }
    }
    let p = param(r.game);
    if !r.game.grounds[g.0].u.castle_bill().done() {
        if anime::finished(r.game, gobj, 0, 1) {
            let linger = p.h(r.data, 0x58);
            let v = r.game.grounds[g.0].u.castle_bill();
            v.linger = linger;
            v.set_done(true);
            let script = r.data.ptr(p.0.at(0x114));
            material::start_script(r.game, r.data, gobj, script, 0);
            // The blaster goes, but the Bullet Bill keeps pointing at it.
            let blaster = r.game.grounds[g.0].u.castle_bill().blaster;
            if blaster.is_some() {
                setup::destroy(r, blaster);
            }
            r.stop_object_sound(Some(g), 0);
        }
        return;
    }
    let v = r.game.grounds[g.0].u.castle_bill();
    let t = v.linger;
    v.linger = t.wrapping_sub(1);
    if t >= 0 {
        return;
    }
    if let Some(item) = r.game.grounds[g.0].u.castle_bill().item {
        material::destroy_item(r, item);
    }
    r.game.grounds[g.0].u.castle_bill().item = None;
    if let Some(blast) = create(r, 1, BLAST_FRAME) {
        let bj = jobj_of(r.game, blast).expect("grcastle.c: the blast's joint");
        let part = nth_joint(r.game, gobj, 0);
        let pos = crate::lb::joint::world_position(&mut r.game.objs, r.data, part, None);
        jobj::set_translate(&mut r.game.objs.jobjs, bj, pos);
        crate::lb::wind::blast(
            &mut r.game.gr.winds,
            pos,
            0xB4,
            20.0,
            0.1,
            std::f32::consts::FRAC_PI_3,
        );
    }
    let stage = r.game.grounds[g.0].u.castle_bill().stage;
    let sg = ground_of(r.game, stage.expect("grcastle.c: the Bullet Bill's castle"));
    r.game.grounds[sg.0].u.castle().set_bill_due(true);
    let range = i32::from(p.h(r.data, 0xE));
    let rand = if range != 0 {
        r.game.objs.rng.below(range)
    } else {
        0
    };
    let wait = i32::from(p.h(r.data, 0xC)).wrapping_add(rand) as i16;
    r.game.grounds[sg.0].u.castle().bill_wait = wait;
    if let Some(b) = r.game.grounds[g.0].u.castle_bill().subject {
        camera::free_box(&mut r.game.camera.boxes, b);
        r.game.grounds[g.0].u.castle_bill().subject = None;
    }
    setup::destroy(r, Some(gobj));
}

/// A blaster's setup (map 2), `grCastle_801CE7E8`: its collision, and its first animation with
/// its ticks.
#[doc(alias = "grCastle_801CE7E8")]
pub fn init_blaster(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    let p = param(r.game);
    let timer = p.h(r.data, 0x12C);
    let v = r.game.grounds[g.0].u.castle_blaster();
    v.anim = 0;
    v.timer = timer;
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// A blaster's process, `grCastle_801CE860`: its four animations in turn, each for its
/// parameters' ticks; the ground shakes all the while.
#[doc(alias = "grCastle_801CE860")]
pub fn proc_blaster(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.castle_blaster();
    if (v.anim as i32) < 3 {
        let t = v.timer;
        v.timer = t.wrapping_sub(1);
        if t < 0 {
            v.anim = v.anim.wrapping_add(1);
            let anim = v.anim;
            let m = map_id(r.game, gobj);
            anime::start_map(r.game, r.data, gobj, m, anim as i32);
            let p = param(r.game);
            let timer = p.h(r.data, 0x12C + 2 * anim);
            r.game.grounds[g.0].u.castle_blaster().timer = timer;
        }
    }
    camera::request_quake(r, camera::QUAKE_LOOP, None);
}

/// A blast's setup (map 1), `grCastle_801CE8E8`: its collision and animation, a camera
/// subject, its hitbox item on part 0, which ownerless items may hit, and its sound.
#[doc(alias = "grCastle_801CE8E8")]
pub fn init_blast(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    anime::start_map(r.game, r.data, gobj, m, 0);
    let b = new_subject(r, 0x134);
    r.game.grounds[g.0].u.castle_blast().subject = Some(b);
    let part = nth_joint(r.game, gobj, 0);
    let item = material::stage_item(r, 0, 4, g, part, Some(CodeRef(BLAST_DEALT)), None, None);
    r.game.grounds[g.0].u.castle_blast().item = item;
    super::super::it::hitbox::let_own_items_hit(r.game, item.expect("grcastle.c: an item"));
    r.object_sound(Some(g), 0, 0x53024);
}

/// A blast's process, `grCastle_801CE9E8`: its camera subject follows it; once its animation
/// ends, it goes with its item and camera subject.
#[doc(alias = "grCastle_801CE9E8")]
pub fn proc_blast(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let subject = r.game.grounds[g.0].u.castle_blast().subject;
    follow(r, gobj, subject);
    if !anime::finished(r.game, gobj, 0, 1) {
        return;
    }
    if let Some(item) = r.game.grounds[g.0].u.castle_blast().item {
        material::destroy_item(r, item);
    }
    r.game.grounds[g.0].u.castle_blast().item = None;
    if let Some(b) = r.game.grounds[g.0].u.castle_blast().subject {
        camera::free_box(&mut r.game.camera.boxes, b);
        r.game.grounds[g.0].u.castle_blast().subject = None;
    }
    setup::destroy(r, Some(gobj));
}

/// A block of a switch's blocks: its slot, its part, its ? and empty models' parts (or none)
/// and its collision joint.
type BlockPart = (usize, i32, Option<(i32, i32)>, i32);

/// A switch's blocks' setup (maps 17, 7 and 5), `grCastle_801CEACC`: their collision, hidden,
/// at rest; each block's joint, its ? and empty models, its resting height and its collision
/// joint (map 7 hides its third block for good, its joint out of the search); all shrunk away,
/// the collision following the model, and their render callback.
#[doc(alias = "grCastle_801CEACC")]
pub fn init_blocks(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    r.game.grounds[g.0].x10_flags.b5 = true;
    r.game.grounds[g.0].u.castle_blocks().state = 0;
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, HIDDEN);
    let v = r.game.grounds[g.0].u.castle_blocks();
    v.flicker = 0;
    for i in 0..5 {
        v.jobjs[i] = None;
        v.effect_a[i] = None;
        v.effect_b[i] = None;
        v.items[i] = None;
        v.joints[i] = -1;
        v.bump[i] = 0;
        v.step[i] = 0;
        v.base_y[i] = 0.0;
    }
    // Each block: its slot, its part and its ? and empty models' (or none), its joint.
    let layout: &[BlockPart] = match m {
        17 => &[
            (0, 1, Some((2, 3)), 11),
            (1, 4, Some((5, 6)), 12),
            (2, 7, None, 13),
            (3, 8, None, 14),
        ],
        7 => &[
            (0, 1, Some((2, 3)), 6),
            (1, 4, Some((5, 6)), 7),
            (3, 10, None, 9),
            (4, 11, None, 10),
        ],
        5 => &[(0, 1, Some((2, 3)), 0), (1, 4, None, 1), (2, 5, None, 2)],
        _ => &[],
    };
    for &(i, part, effects, joint) in layout {
        let bj = nth_joint(r.game, gobj, part);
        let (ea, eb) = match effects {
            Some((a, b)) => (nth_joint(r.game, gobj, a), nth_joint(r.game, gobj, b)),
            None => (None, None),
        };
        let v = r.game.grounds[g.0].u.castle_blocks();
        v.jobjs[i] = bj;
        v.effect_a[i] = ea;
        v.effect_b[i] = eb;
        if effects.is_some() {
            let y = r.game.objs.jobjs[bj.expect("grcastle.c: a block's joint")]
                .translate
                .y;
            r.game.grounds[g.0].u.castle_blocks().base_y[i] = y;
        }
        r.game.grounds[g.0].u.castle_blocks().joints[i] = joint;
        if m == 7 && i == 1 {
            let third = nth_joint(r.game, gobj, 7);
            jobj::set_flags_all(&mut r.game.objs.jobjs, third, HIDDEN);
            r.game.geometry_mut().disable_joint(8);
        }
    }
    grow(r, gobj, false);
    update_map_coll(r.game, r.data, gobj);
    r.game.world.gobj_mut(gobj).render = Some(CodeRef(RENDER_BLOCKS));
}

/// The blocks' process, `grCastle_801CEF04`. Brought out (state 1), they grow back for a draw
/// between the parameters' `x40` and `x42` ticks (state 2), their items going when it runs out;
/// they flicker for the parameters' `x44` ticks (state 3), then flicker while they shrink away
/// (state 4) and rest (state 0). Bumped blocks rise and fall back meanwhile; the collision
/// follows the model.
#[doc(alias = "grCastle_801CEF04")]
pub fn proc_blocks(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let p = param(r.game);
    match r.game.grounds[g.0].u.castle_blocks().state {
        1 => {
            r.game.grounds[g.0].u.castle_blocks().flicker = 0;
            let (hi, lo) = (i32::from(p.h(r.data, 0x42)), i32::from(p.h(r.data, 0x40)));
            let timer = grlib::rand_range(&mut r.game.objs.rng, hi, lo);
            r.game.grounds[g.0].u.castle_blocks().timer = timer;
            if grow(r, gobj, true) {
                r.game.grounds[g.0].u.castle_blocks().state = 2;
            }
            bump(r, gobj);
        }
        2 => {
            let v = r.game.grounds[g.0].u.castle_blocks();
            let t = v.timer;
            v.timer = t.wrapping_sub(1);
            if t < 0 {
                v.state = 3;
                v.timer = 0;
                for i in 0..5 {
                    if let Some(item) = r.game.grounds[g.0].u.castle_blocks().items[i] {
                        material::destroy_item(r, item);
                        r.game.grounds[g.0].u.castle_blocks().items[i] = None;
                    }
                }
            }
            bump(r, gobj);
        }
        3 => {
            let limit = i32::from(p.h(r.data, 0x44));
            let v = r.game.grounds[g.0].u.castle_blocks();
            let t = v.timer;
            v.timer = t.wrapping_add(1);
            if t > limit {
                v.state = 4;
            } else {
                v.flicker = i16::from(v.flicker == 0);
            }
            bump(r, gobj);
        }
        4 => {
            let v = r.game.grounds[g.0].u.castle_blocks();
            v.flicker = i16::from(v.flicker == 0);
            if grow(r, gobj, false) {
                let v = r.game.grounds[g.0].u.castle_blocks();
                v.flicker = 0;
                v.state = 0;
            }
        }
        _ => {}
    }
    update_map_coll(r.game, r.data, gobj);
}

/// A block's model that grows and shrinks: its ? model while shown or its empty one, if it has
/// them, else its own.
fn sized_model(r: &Running, b: Option<JObjId>, ea: Option<JObjId>, eb: Option<JObjId>) -> JObjId {
    let m = match (ea, eb) {
        (Some(_), Some(_)) => {
            if jobj::get_flags(&r.game.objs.jobjs, ea) & HIDDEN != 0 {
                eb
            } else {
                ea
            }
        }
        _ => b,
    };
    m.expect("grcastle.c: a block's model")
}

/// Grows the blocks back (`grow`) or shrinks them away a tick, `grCastle_801CFBD4`; whether
/// all are done. A hidden block shows at its smallest, its ? model with items on (or its empty
/// one), unless a Bullet Bill flies within 40 of it, and rests unbumped; a growing one that
/// reaches full size joins the collision and, showing its ? model, gets its hitbox item. A
/// shrinking one hides at its smallest; each leaves the collision.
#[doc(alias = "grCastle_801CFBD4")]
#[doc(alias = "grCastle_801D0298")]
pub fn grow(r: &mut Running, gobj: GObjId, grow: bool) -> bool {
    let g = ground_of(r.game, gobj);
    let p = param(r.game);
    let mut done = true;
    for i in 0..5 {
        let v = r.game.grounds[g.0].u.castle_blocks();
        let Some(bj) = v.jobjs[i] else { continue };
        let (ea, eb) = (v.effect_a[i], v.effect_b[i]);
        let hidden = jobj::get_flags(&r.game.objs.jobjs, Some(bj)) & HIDDEN != 0;
        if grow {
            if hidden {
                if !near_bill(r, bj) {
                    let jobjs = &mut r.game.objs.jobjs;
                    jobj::clear_flags(jobjs, Some(bj), HIDDEN);
                    let small = 0.000_1;
                    if let (Some(a), Some(b)) = (ea, eb) {
                        let items_on = r.game.vs_item_freq() != -1 && r.game.vs_x6() == 0;
                        let jobjs = &mut r.game.objs.jobjs;
                        let (shown, hidden) = if items_on { (a, b) } else { (b, a) };
                        jobj::clear_flags(jobjs, Some(shown), HIDDEN);
                        jobj::set_srt(jobjs, shown, |o| o.scale.y = small);
                        jobj::set_flags(jobjs, Some(hidden), HIDDEN);
                        jobj::set_srt(jobjs, hidden, |o| o.scale.y = 1.0);
                    } else {
                        jobj::set_srt(jobjs, bj, |o| o.scale.y = small);
                    }
                }
                let v = r.game.grounds[g.0].u.castle_blocks();
                v.bump[i] = 0;
                done = false;
                v.step[i] = 0;
            } else {
                let target = sized_model(r, Some(bj), ea, eb);
                let mut scale = reg(r.game.objs.jobjs[target].scale.y);
                if scale < 1.0 {
                    scale = fp::fadds(scale, reg(p.f(r.data, 0x48)));
                    if scale >= 1.0 {
                        scale = 1.0;
                        let joint = r.game.grounds[g.0].u.castle_blocks().joints[i];
                        r.game.geometry_mut().enable_joint(joint);
                        if ea.is_some() && jobj::get_flags(&r.game.objs.jobjs, ea) & HIDDEN == 0 {
                            let item = material::stage_item(
                                r,
                                0,
                                2,
                                g,
                                Some(target),
                                None,
                                Some(CodeRef(BLOCK_HIT[i])),
                                Some(CodeRef(BLOCK_TOUCHED[i])),
                            );
                            r.game.grounds[g.0].u.castle_blocks().items[i] = item;
                            let item = item.expect("grcastle.c: a block's item");
                            material::set_item_hurtbox(r.game, item, Vec3::ZERO, Vec3::ZERO, 8.0);
                            material::item_hurtbox_on(r.game, item);
                        }
                    }
                    let y = mem(scale);
                    jobj::set_srt(&mut r.game.objs.jobjs, target, |o| o.scale.y = y);
                    done = false;
                }
            }
        } else {
            if !hidden {
                let target = sized_model(r, Some(bj), ea, eb);
                let small = reg(0.000_1_f32);
                let mut scale = reg(r.game.objs.jobjs[target].scale.y);
                if scale > small {
                    scale = fp::fsubs(scale, reg(p.f(r.data, 0x4C)));
                    if scale <= small {
                        scale = small;
                        let jobjs = &mut r.game.objs.jobjs;
                        jobj::set_flags(jobjs, Some(bj), HIDDEN);
                        if ea.is_some() {
                            jobj::set_flags(jobjs, ea, HIDDEN);
                        }
                        if eb.is_some() {
                            jobj::set_flags(jobjs, eb, HIDDEN);
                        }
                    }
                    let y = mem(scale);
                    jobj::set_srt(&mut r.game.objs.jobjs, target, |o| o.scale.y = y);
                    done = false;
                }
            }
            let joint = r.game.grounds[g.0].u.castle_blocks().joints[i];
            r.game.geometry_mut().disable_joint(joint);
        }
    }
    done
}

/// Whether a Bullet Bill flies within 40 of a block (the inline `isNearTarget`).
fn near_bill(r: &mut Running, block: JObjId) -> bool {
    let Some(target) = bill_position(r.game, r.data) else {
        return false;
    };
    let pos = crate::lb::joint::world_position(&mut r.game.objs, r.data, Some(block), None);
    let dx = fp::fsubs(reg(pos.x), reg(target.x));
    let dy = fp::fsubs(reg(pos.y), reg(target.y));
    let d = crate::math::sqrt(mem(fp::fadds(fp::fmuls(dx, dx), fp::fmuls(dy, dy))));
    reg(d) < 40.0
}

/// The bumped blocks' rise a tick, `grCastle_801D02B8`: each to its resting height plus the
/// tick's rise, and back to rest at the end. At its peak, while the blocks are out, a ? block
/// drops a random item (none with items off) and turns empty.
#[doc(alias = "grCastle_801D02B8")]
pub fn bump(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let p = param(r.game);
    for i in 0..5 {
        let v = r.game.grounds[g.0].u.castle_blocks();
        if v.bump[i] != 1 {
            continue;
        }
        let step = v.step[i];
        let yoff = BUMP[step as usize];
        let bj = v.jobjs[i].expect("grcastle.c: a block's joint");
        let base = v.base_y[i];
        if reg(yoff) != -1.0 {
            let y = mem(fp::fadds(reg(base), reg(yoff)));
            jobj::set_srt(&mut r.game.objs.jobjs, bj, |o| o.translate.y = y);
            let v = r.game.grounds[g.0].u.castle_blocks();
            let out = v.state == 1 || (v.state.wrapping_sub(2) as u16) <= 1;
            if out && step == 3 {
                let (ea, eb) = (v.effect_a[i], v.effect_b[i]);
                if ea.is_some() {
                    let mut pos =
                        crate::lb::joint::world_position(&mut r.game.objs, r.data, ea, None);
                    pos.y = mem(fp::fadds(reg(pos.y), 5.0));
                    let vel = Vec3::new(0.0, p.f(r.data, 0x50), 0.0);
                    super::super::it::drop::drop_random(r, pos, Some(vel));
                    let jobjs = &mut r.game.objs.jobjs;
                    jobj::set_flags(jobjs, ea, HIDDEN);
                    jobj::clear_flags(jobjs, eb, HIDDEN);
                }
            }
        } else {
            jobj::set_srt(&mut r.game.objs.jobjs, bj, |o| o.translate.y = base);
            r.game.grounds[g.0].u.castle_blocks().bump[i] = 0;
        }
        let v = r.game.grounds[g.0].u.castle_blocks();
        v.step[i] = v.step[i].wrapping_add(1);
    }
}

/// The blocks' render callback, `grCastle_801D0520`: drawn unless they flicker off.
#[doc(alias = "grCastle_801D0520")]
pub fn render_blocks(r: &mut Running, gobj: GObjId, pass: i32) {
    let g = ground_of(r.game, gobj);
    if r.game.grounds[g.0].u.castle_blocks().flicker == 0 {
        r.render_stage_object(gobj, pass);
    }
}

/// A block hit or touched (`grCastle_801D0550_sub`): it bumps, its item goes, with a sound.
fn knock(r: &mut Running, g: GroundId, i: usize) {
    let v = r.game.grounds[g.0].u.castle_blocks();
    v.bump[i] = 1;
    let item = v.items[i].expect("grcastle.c: a block's item");
    material::destroy_item(r, item);
    r.game.grounds[g.0].u.castle_blocks().items[i] = None;
    r.stage_sound(0x53020);
}

/// Which block a hit callback is for.
pub fn block_hit_index(cb: u32) -> Option<usize> {
    BLOCK_HIT.iter().position(|&a| a == cb)
}

/// Which block a touch callback is for.
pub fn block_touched_index(cb: u32) -> Option<usize> {
    BLOCK_TOUCHED.iter().position(|&a| a == cb)
}

/// A block's item hit, `grCastle_801D0550` to `801D0680`: the block bumps.
#[doc(alias = "grCastle_801D0550")]
#[doc(alias = "grCastle_801D059C")]
#[doc(alias = "grCastle_801D05E8")]
#[doc(alias = "grCastle_801D0634")]
#[doc(alias = "grCastle_801D0680")]
pub fn block_hit(r: &mut Running, g: GroundId, i: usize) {
    knock(r, g, i);
}

/// A block's item touched, `grCastle_801D06CC` to `801D08AC`: by a fighter or an item other
/// than a stage's, the block bumps.
#[doc(alias = "grCastle_801D06CC")]
#[doc(alias = "grCastle_801D0744")]
#[doc(alias = "grCastle_801D07BC")]
#[doc(alias = "grCastle_801D0834")]
#[doc(alias = "grCastle_801D08AC")]
pub fn block_touched(r: &mut Running, g: GroundId, toucher: Option<GObjId>, i: usize) {
    let by = toucher.expect("grcastle.c: a toucher");
    if super::super::it::item::is_fighter(r.game, Some(by))
        || super::super::it::lib26b1::kind(r.game, by) != super::super::it::kinds::PKIND_RANDOM
    {
        knock(r, g, i);
    }
}

/// A switch's setup (maps 18 to 20), `grCastle_801CF0F4`: its collision, hidden, at rest, with
/// no item, at one and a half times the stage's scale; and its blocks (maps 17, 7 and 5).
#[doc(alias = "grCastle_801CF0F4")]
pub fn init_switch(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    let v = r.game.grounds[g.0].u.castle_switch();
    v.state = 0;
    v.item = None;
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, HIDDEN);
    let s = mem(fp::fmuls(1.5, reg(super::ground::scale(r.game))));
    let sj = j.expect("grcastle.c: the switch's joint");
    let jobjs = &mut r.game.objs.jobjs;
    jobj::set_srt(jobjs, sj, |o| o.scale.x = s);
    jobj::set_srt(jobjs, sj, |o| o.scale.y = s);
    jobj::set_srt(jobjs, sj, |o| o.scale.z = s);
    let blocks = match m {
        20 => 17,
        19 => 7,
        18 => 5,
        _ => return,
    };
    let b = create(r, blocks, BLOCKS_FRAME);
    r.game.grounds[g.0].u.castle_switch().blocks = b;
}

/// A switch's process, `grCastle_801CF308`. Sent to a spot (state 1), it shows at rest and
/// blinks in (state 2) on the blink's waits; at the end it waits to be hit (state 3) with its
/// hitbox item. Hit (state 4), its item goes, its blocks come out, and it plays its animation
/// with a sound and a small quake (state 5), hiding a while after; once its blocks rest, it
/// rests hidden (state 0). While out it sits on its spot's joint, turned as it is.
#[doc(alias = "grCastle_801CF308")]
pub fn proc_switch(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let mut placed = false;
    let mut state = r.game.grounds[g.0].u.castle_switch().state;
    if state == 1 {
        let v = r.game.grounds[g.0].u.castle_switch();
        v.blink = -1;
        v.delay = 0;
        jobj::clear_flags_all(&mut r.game.objs.jobjs, j, HIDDEN);
        anime::reset_part_srt(r.game, r.data, m, gobj, 0);
        r.game.grounds[g.0].u.castle_switch().state = 2;
        state = 2;
    }
    match state {
        2 => {
            let v = r.game.grounds[g.0].u.castle_switch();
            let delay = i32::from(v.delay);
            if delay > 0 {
                v.delay = (delay - 1) as i16;
            } else {
                v.blink = v.blink.wrapping_add(1);
                let blink = v.blink;
                if jobj::get_flags(&r.game.objs.jobjs, j) & HIDDEN != 0 {
                    jobj::clear_flags_all(&mut r.game.objs.jobjs, j, HIDDEN);
                } else {
                    jobj::set_flags_all(&mut r.game.objs.jobjs, j, HIDDEN);
                }
                let val = BLINK[blink as usize];
                if val != -1 {
                    let d = fp::fctiwz(fp::fmul(2.0, f64::from(val))) as i16;
                    r.game.grounds[g.0].u.castle_switch().delay = d;
                } else {
                    r.game.grounds[g.0].u.castle_switch().state = 3;
                    let item = material::stage_item(
                        r,
                        0,
                        1,
                        g,
                        j,
                        None,
                        Some(CodeRef(SWITCH_HIT)),
                        Some(CodeRef(SWITCH_TOUCHED)),
                    );
                    r.game.grounds[g.0].u.castle_switch().item = item;
                    let item = item.expect("grcastle.c: the switch's item");
                    let (a, b) = (Vec3::new(0.0, -1.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
                    material::set_item_hurtbox(r.game, item, a, b, 4.0);
                    material::item_hurtbox_on(r.game, item);
                    let count = param(r.game).h(r.data, 0x54);
                    r.game.grounds[g.0].u.castle_switch().count = count;
                }
            }
            placed = true;
        }
        3 => placed = true,
        4 | 5 => {
            if state == 4 {
                if let Some(item) = r.game.grounds[g.0].u.castle_switch().item {
                    material::destroy_item(r, item);
                    r.game.grounds[g.0].u.castle_switch().item = None;
                }
                let blocks = r.game.grounds[g.0].u.castle_switch().blocks;
                let bg = ground_of(r.game, blocks.expect("grcastle.c: the switch's blocks"));
                r.game.grounds[bg.0].u.castle_blocks().state = 1;
                r.game.grounds[g.0].u.castle_switch().state = 5;
                anime::start_map(r.game, r.data, gobj, m, 0);
                r.object_sound(Some(g), 0, 0x53027);
                camera::request_quake(r, QUAKE_SMALL, None);
            }
            if anime::finished(r.game, gobj, 0, 7) {
                let v = r.game.grounds[g.0].u.castle_switch();
                let c = v.count;
                v.count = c.wrapping_sub(1);
                if c < 0 {
                    jobj::set_flags_all(&mut r.game.objs.jobjs, j, HIDDEN);
                }
            }
            let blocks = r.game.grounds[g.0].u.castle_switch().blocks;
            let bg = ground_of(r.game, blocks.expect("grcastle.c: the switch's blocks"));
            if r.game.grounds[bg.0].u.castle_blocks().state == 0 {
                r.game.grounds[g.0].u.castle_switch().state = 0;
                jobj::set_flags_all(&mut r.game.objs.jobjs, j, HIDDEN);
            }
            placed = true;
        }
        _ => {}
    }
    if placed {
        let anchor = r.game.grounds[g.0].u.castle_switch().anchor;
        let pos = crate::lb::joint::world_position(&mut r.game.objs, r.data, anchor, None);
        let sj = j.expect("grcastle.c: the switch's joint");
        jobj::set_translate(&mut r.game.objs.jobjs, sj, pos);
        let a = &r.game.objs.jobjs[anchor.expect("grcastle.c: the switch's spot")];
        let (rot, stale) = (a.rotate, a.stale_w);
        jobj::set_rotation(&mut r.game.objs.jobjs, sj, rot);
        r.game.objs.jobjs[sj].stale_w = stale;
    }
}

/// A lift's collision joint's callback, `grCastle_801CF750`: a fighter (collision kind 1) on
/// lift 4's joint or the other's adds its weight (`x50`) to that lift's.
#[doc(alias = "grCastle_801CF750")]
pub fn lift_touched(gp: &mut super::super::structs::Ground, joint_id: i32, kind: u32, x50: i32) {
    let i = if joint_id == 4 { 0 } else { 1 };
    if kind == 1 {
        let l = &mut gp.u.castle_lifts().plat[i];
        l.wind = mem(fp::fadds(reg(l.wind), fp::frsp(f64::from(x50))));
    }
}

/// The switches' setup, `grCastle_801CF7B0`: the three switches (maps 20, 19 and 18), none out,
/// and a wait between the parameters' `x0` and `x2` ticks.
#[doc(alias = "grCastle_801CF7B0")]
pub fn init_switches(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    for (i, m) in [20, 19, 18].into_iter().enumerate() {
        let s = create(r, m, SWITCH_FRAME);
        r.game.grounds[g.0].u.castle().switches[i] = s;
    }
    r.game.grounds[g.0].u.castle().switch_slot = -1;
    let p = param(r.game);
    let (a, b) = (i32::from(p.h(r.data, 2)), i32::from(p.h(r.data, 0)));
    let wait = grlib::rand_range(&mut r.game.objs.rng, a, b) as i16;
    r.game.grounds[g.0].u.castle().switch_wait = wait;
}

/// The switches' timer, `grCastle_801CF868`: while none is out (or the last is back at rest),
/// the wait counts down; then a new wait, and a switch drawn by weight (the last one's divided
/// by the parameters' `x6`) goes to a spot drawn among the eleven, if its map is there.
#[doc(alias = "grCastle_801CF868")]
pub fn switch_timer(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.castle();
    let (switches, slot) = (v.switches, v.switch_slot);
    if switches.iter().all(Option::is_none) {
        return;
    }
    if slot != -1 {
        let Some(s) = switches[slot as usize] else {
            return;
        };
        let sg = ground_of(r.game, s);
        if r.game.grounds[sg.0].u.castle_switch().state != 0 {
            return;
        }
    }
    let v = r.game.grounds[g.0].u.castle();
    v.switch_wait = v.switch_wait.wrapping_sub(1);
    if v.switch_wait >= 0 {
        return;
    }
    let p = param(r.game);
    let (a, b) = (i32::from(p.h(r.data, 2)), i32::from(p.h(r.data, 0)));
    let wait = grlib::rand_range(&mut r.game.objs.rng, a, b) as i16;
    r.game.grounds[g.0].u.castle().switch_wait = wait;
    let mut weights = [100i32; 3];
    if slot != -1 {
        let div = i32::from(p.h(r.data, 6));
        weights[slot as usize] /= div;
    }
    let total = weights[0].wrapping_add(weights[1]).wrapping_add(weights[2]);
    let mut rand = if total != 0 {
        r.game.objs.rng.below(total)
    } else {
        0
    };
    let mut pick = 0;
    while pick < 3 {
        rand = rand.wrapping_sub(weights[pick]);
        if rand < 0 {
            break;
        }
        pick += 1;
    }
    if pick == 3 {
        pick = 0;
    }
    r.game.grounds[g.0].u.castle().switch_slot = pick as i16;
    let s = r.game.grounds[g.0].u.castle().switches[pick];
    let sg = ground_of(r.game, s.expect("grcastle.c: a switch"));
    let (spot_map, part) = SWITCH_SPOTS[r.game.objs.rng.below(11) as usize];
    // HSD_GObjPLinkHead[HSD_GOBJ_PLINK_GROUND].
    let mut cur = r.game.world.first_in(5);
    while let Some(e) = cur {
        if r.game.grounds[ground_of(r.game, e).0].map_id == spot_map {
            break;
        }
        cur = r.game.world.gobj(e).next;
    }
    if let Some(e) = cur {
        let anchor = nth_joint(r.game, e, part);
        let v = r.game.grounds[sg.0].u.castle_switch();
        v.anchor = anchor;
        v.state = 1;
    }
}

/// A switch's item hit, `fn_801CFAFC`: the switch is pressed, and a fighter that hit it takes
/// the parameters' `x4` hitlag.
#[doc(alias = "fn_801CFAFC")]
pub fn switch_hit(r: &mut Running, _item: GObjId, g: GroundId, _dir: Vec3, by: Option<GObjId>) {
    press(r, g, by);
}

/// A switch's item touched, `fn_801CFB68`: as [`switch_hit`].
#[doc(alias = "fn_801CFB68")]
pub fn switch_touched(r: &mut Running, _item: GObjId, g: GroundId, by: Option<GObjId>) {
    press(r, g, by);
}

fn press(r: &mut Running, g: GroundId, by: Option<GObjId>) {
    r.game.grounds[g.0].u.castle_switch().state = 4;
    if super::super::it::item::is_fighter(r.game, by) {
        let lag = fp::frsp(f64::from(param(r.game).h(r.data, 4))) as f32;
        r.game.ft_80086a4c(by.expect("a fighter"), lag);
    }
}

/// The castle's render callback, `fn_801D0924`: the castle, and its flags' chains as lines
/// when the object's debug flag is on.
#[doc(alias = "fn_801D0924")]
pub fn render_stage(r: &mut Running, gobj: GObjId, pass: i32) {
    r.render_stage_object(gobj, pass);
    let g = ground_of(r.game, gobj);
    if r.game.grounds[g.0].x10_flags.b7 {
        super::super::code::unported_call("lb_800117F4 (the flags' chains' debug lines)");
    }
}

/// The moat's wind device, `grCastle_801D09B8`: a fighter below the water's surface (14 in
/// the stage's scale) splashes as it falls in, as big as its camera box's depth, and is held
/// up a little while it falls fast.
#[doc(alias = "grCastle_801D09B8")]
// NaN takes the original's branch, which the negated comparison spells out.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn moat(r: &mut Running, _ground: GObjId, fighter: GObjId) -> Option<Vec3> {
    let surface = fp::fmuls(14.0, reg(super::ground::scale(r.game)));
    let mut pos = r.game.ft_pos(fighter);
    let prev = r.game.fighter(fighter).prev_pos;
    if !(reg(pos.y) < surface) {
        return None;
    }
    if reg(prev.y) > surface {
        let size = fp::fdivs(reg(r.game.ft_camera_depth(fighter)), 10.0);
        pos.y = mem(surface);
        splash(r, pos, mem(size));
    }
    let delta = r.game.fighter(fighter).pos_delta;
    if reg(delta.y) < fp::fneg(0.5) {
        Some(Vec3::new(0.0, 0.1, 0.0))
    } else {
        Some(Vec3::new(0.0, 0.0, 0.0))
    }
}

/// A splash at `pos`, `grCastle_801D0A9C`: the stage's effect 0x7530, scaled by `size`.
#[doc(alias = "grCastle_801D0A9C")]
pub fn splash(r: &mut Running, pos: Vec3, size: f32) {
    let Some(g) = grlib::generator_at(r.game, r.data, 0x7530, 0x1E, pos) else {
        return;
    };
    let s = r.game.objs.generators[g]
        .appsrt
        .expect("a generator's app-SRT");
    let srt = &mut r.game.objs.app_srts[s];
    srt.scale.x = mem(fp::fmuls(reg(srt.scale.x), reg(size)));
    srt.scale.y = mem(fp::fmuls(reg(srt.scale.y), reg(size)));
    srt.scale.z = mem(fp::fmuls(reg(srt.scale.z), reg(size)));
}

/// The stage's shadow check, `grCastle_801D0B0C`: none right of 192 below 137 (in the stage's
/// scale); else a fighter casts on a joint below a point one unit above its feet.
#[doc(alias = "grCastle_801D0B0C")]
pub fn check_shadow_render(game: &mut Game, data: &Archives, v: Vec3, j: Option<JObjId>) -> bool {
    let s = reg(super::ground::scale(game));
    let x = fp::fmuls(192.0, s);
    let y = fp::fmuls(137.0, s);
    if reg(v.x) > x && reg(v.y) < y {
        return false;
    }
    let pos = crate::lb::joint::world_position(&mut game.objs, data, j, None);
    fp::fadds(1.0, reg(v.y)) > reg(pos.y)
}

/// Steers the castle's wind a tick, `grCastle_801D0BBC`: it turns by its step, within 60
/// degrees of x; every 30 ticks of its blowing the step may turn around, every 300 the gusts
/// stop, or may start; its strength is drawn, stronger in gusts.
#[doc(alias = "grCastle_801D0BBC")]
pub fn steer_wind(r: &mut Running) {
    let w = r
        .game
        .gr
        .castle
        .wind
        .expect("grcastle.c: the castle's wind") as usize;
    let e = &r.game.gr.winds.entries[w];
    let mut angle = reg(crate::math::atan2(e.pos.z, e.pos.x));
    angle = fp::fadds(angle, reg(r.game.gr.castle.step));
    if angle > 1.047_197_543_084_621_4 {
        angle = reg(std::f32::consts::FRAC_PI_3);
    }
    if angle < fp::fneg(1.047_197_543_084_621_4) {
        angle = fp::fneg(reg(std::f32::consts::FRAC_PI_3));
    }
    let c = crate::math::cos(mem(angle));
    r.game.gr.winds.entries[w].pos.x = c;
    let s = crate::math::sin(mem(angle));
    r.game.gr.winds.entries[w].pos.z = s;
    let age = r.game.gr.winds.entries[w].age;
    if age.wrapping_rem(30) == 0 && reg(r.game.objs.rng.draw_f32()) > 0.5 {
        let c = &mut r.game.gr.castle;
        c.step = mem(fp::fneg(reg(c.step)));
    }
    if age.wrapping_rem(300) == 0 {
        if r.game.gr.castle.gusts != 0 {
            r.game.gr.castle.gusts = 0;
        } else if reg(r.game.objs.rng.draw_f32()) > 0.5 {
            r.game.gr.castle.gusts = 1;
        }
    }
    rescale_wind(r);
}

/// Draws the castle's wind's strength, `grCastle_801D0D24`: 0.2 to 0.5 in gusts, else below
/// 0.05.
#[doc(alias = "grCastle_801D0D24")]
pub fn rescale_wind(r: &mut Running) {
    let w = r
        .game
        .gr
        .castle
        .wind
        .expect("grcastle.c: the castle's wind") as usize;
    let rand = reg(r.game.objs.rng.draw_f32());
    let scale = if r.game.gr.castle.gusts != 0 {
        fp::frsp(fp::fmadd(0.3, rand, 0.2))
    } else {
        fp::frsp(fp::fmul(0.05, rand))
    };
    r.game.gr.winds.entries[w].scale = mem(scale);
}

/// Turns a flag's pole about y toward the wind where it stands, `grCastle_801D0D84`: by the
/// wind's strength times the sine of the angle to it, wrapped to a half turn either way; the
/// pole's quaternion flag cleared.
#[doc(alias = "grCastle_801D0D84")]
pub fn turn_to_wind(r: &mut Running, pole: Option<JObjId>) {
    let Some(j) = pole else { return };
    jobj::setup_matrix(&mut r.game.objs, r.data, Some(j));
    let o = &r.game.objs.jobjs[j];
    let pos = Vec3::new(o.mtx.m[0][3], o.mtx.m[1][3], o.mtx.m[2][3]);
    let rot_y = o.rotate.y;
    let (mut dir, wind) = crate::lb::wind::force_at(&r.game.gr.winds, pos);
    dir.y = 0.0;
    let len = dir.normalize();
    let fwd = Vec3::new(
        crate::math::cos(rot_y),
        0.0,
        mem(fp::fneg(reg(crate::math::sin(rot_y)))),
    );
    let angle = reg(fwd.angle(dir));
    let mut y = rot_y;
    if !(angle < reg(0.000_01_f32) && angle > fp::fneg(reg(0.000_01_f32))) {
        let sin = reg(crate::math::sin(mem(angle)));
        let mut correction = fp::fmuls(reg(len), fp::fmuls(reg(wind), sin));
        if correction < 0.0 {
            correction = fp::fneg(correction);
        }
        let cross = fwd.sdk_cross(dir);
        y = if reg(cross.y) > 0.0 {
            mem(fp::fadds(reg(rot_y), correction))
        } else {
            mem(fp::fsubs(reg(rot_y), correction))
        };
    }
    if reg(y) > 3.141_592_629_253_864_3 {
        y = mem(fp::frsp(fp::fsub(reg(y), 6.283_185_258_507_729)));
    } else if reg(y) < fp::fneg(3.141_592_629_253_864_3) {
        y = mem(fp::frsp(fp::fadd(reg(y), 6.283_185_258_507_729)));
    }
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.rotate.y = y);
    jobj::clear_flags_all(&mut r.game.objs.jobjs, Some(j), jobj::USE_QUATERNION);
}

/// The lowest the camera's eye goes on Princess Peach's Castle, `grCastle_801D0FF0`: 50 in the
/// stage's scale.
#[doc(alias = "grCastle_801D0FF0")]
pub fn camera_floor(game: &Game) -> f32 {
    mem(fp::fmuls(50.0, reg(super::ground::scale(game))))
}
